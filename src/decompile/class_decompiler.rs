use std::fmt;

use crate::attribute_info::AttributeInfoVariant;
use crate::method_info::MethodAccessFlags;
use crate::types::ClassFile;

use super::cfg;
use super::desugar::{self, DesugarOptions};
use super::java_ast::*;
use super::renderer::{JavaRenderer, RenderConfig};
use super::stack_sim;
use super::structuring;
use super::type_inference;
use super::util;

/// Error type for decompilation failures.
#[derive(Clone, Debug)]
pub enum DecompileError {
    /// The class file has no methods to decompile.
    NoCode,
    /// A specific method failed to decompile.
    MethodError {
        method_name: String,
        message: String,
    },
    /// General error.
    General(String),
}

impl fmt::Display for DecompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DecompileError::NoCode => write!(f, "no code to decompile"),
            DecompileError::MethodError {
                method_name,
                message,
            } => {
                write!(
                    f,
                    "failed to decompile method '{}': {}",
                    method_name, message
                )
            }
            DecompileError::General(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for DecompileError {}

/// Options controlling the decompilation process.
#[derive(Clone, Debug)]
pub struct DecompileOptions {
    pub render_config: RenderConfig,
    pub inline_inner_classes: bool,
    pub include_synthetic: bool,
    pub recover_lambdas: bool,
    pub desugar_enum_switch: bool,
    pub desugar_string_switch: bool,
    pub desugar_try_resources: bool,
    pub desugar_foreach: bool,
    pub desugar_assert: bool,
    pub desugar_autobox: bool,
}

impl Default for DecompileOptions {
    fn default() -> Self {
        Self {
            render_config: RenderConfig::default(),
            inline_inner_classes: true,
            include_synthetic: false,
            recover_lambdas: true,
            desugar_enum_switch: true,
            desugar_string_switch: true,
            desugar_try_resources: true,
            desugar_foreach: true,
            desugar_assert: true,
            desugar_autobox: true,
        }
    }
}

/// The main decompiler entry point.
pub struct Decompiler {
    options: DecompileOptions,
}

impl Decompiler {
    pub fn new(options: DecompileOptions) -> Self {
        Self { options }
    }

    /// Decompile a single ClassFile to Java source.
    pub fn decompile(&self, class: &ClassFile) -> Result<String, DecompileError> {
        let java_class = self.build_ast(class)?;
        let mut config = self.options.render_config.clone();
        config.include_synthetic = self.options.include_synthetic;
        let renderer = JavaRenderer::new(config);
        Ok(renderer.render_class(&java_class))
    }

    /// Decompile a single method by name.
    pub fn decompile_method(
        &self,
        class: &ClassFile,
        method_name: &str,
    ) -> Result<String, DecompileError> {
        let java_class = self.build_ast(class)?;
        let method = java_class
            .methods
            .iter()
            .find(|m| m.name == method_name)
            .ok_or_else(|| {
                DecompileError::General(format!("method '{}' not found", method_name))
            })?;

        let mut config = self.options.render_config.clone();
        config.include_synthetic = self.options.include_synthetic;
        let renderer = JavaRenderer::new(config);
        // Render just the method in a minimal class wrapper
        let wrapper = JavaClass {
            kind: java_class.kind.clone(),
            visibility: java_class.visibility.clone(),
            is_final: false,
            is_abstract: false,
            is_sealed: false,
            is_static: false,
            annotations: Vec::new(),
            type_parameters: Vec::new(),
            package: java_class.package.clone(),
            name: java_class.name.clone(),
            super_class: None,
            interfaces: Vec::new(),
            permitted_subclasses: Vec::new(),
            record_components: Vec::new(),
            fields: Vec::new(),
            methods: vec![method.clone()],
            inner_classes: Vec::new(),
            source_file: None,
        };
        Ok(renderer.render_class(&wrapper))
    }

    /// Decompile with inner classes provided.
    pub fn decompile_with_inner_classes(
        &self,
        outer: &ClassFile,
        inner: &[&ClassFile],
    ) -> Result<String, DecompileError> {
        let mut java_class = self.build_ast(outer)?;

        // Build and attach inner classes
        for inner_class in inner {
            match self.build_ast(inner_class) {
                Ok(mut inner_ast) => {
                    inner_ast.is_static = is_static_inner(inner_class);
                    java_class.inner_classes.push(inner_ast);
                }
                Err(e) => {
                    // Per-class error recovery: add a stub with a comment
                    let name =
                        util::get_class_name(&inner_class.const_pool, inner_class.this_class)
                            .unwrap_or("Unknown");
                    let simple = name.rsplit('/').next().unwrap_or(name);
                    let simple = simple.rsplit('$').next().unwrap_or(simple);
                    java_class.inner_classes.push(JavaClass {
                        kind: ClassKind::Class,
                        visibility: Visibility::PackagePrivate,
                        is_final: false,
                        is_abstract: false,
                        is_sealed: false,
                        is_static: false,
                        annotations: Vec::new(),
                        type_parameters: Vec::new(),
                        package: None,
                        name: simple.to_string(),
                        super_class: None,
                        interfaces: Vec::new(),
                        permitted_subclasses: Vec::new(),
                        record_components: Vec::new(),
                        fields: Vec::new(),
                        methods: vec![JavaMethod {
                            visibility: Visibility::PackagePrivate,
                            is_static: false,
                            is_final: false,
                            is_abstract: false,
                            is_synchronized: false,
                            is_native: false,
                            is_default: false,
                            is_synthetic: false,
                            is_bridge: false,
                            annotations: Vec::new(),
                            type_parameters: Vec::new(),
                            return_type: JavaType::Void,
                            name: "/* error */".into(),
                            parameters: Vec::new(),
                            throws: Vec::new(),
                            body: None,
                            error: Some(format!("Decompilation failed: {}", e)),
                        }],
                        inner_classes: Vec::new(),
                        source_file: None,
                    });
                }
            }
        }

        let mut config = self.options.render_config.clone();
        config.include_synthetic = self.options.include_synthetic;
        let renderer = JavaRenderer::new(config);
        Ok(renderer.render_class(&java_class))
    }

    /// Build the Java AST from a ClassFile, decompiling all method bodies.
    fn build_ast(&self, class: &ClassFile) -> Result<JavaClass, DecompileError> {
        let mut java_class = type_inference::build_java_class(class);

        // Decompile each method's body
        for (i, method) in class.methods.iter().enumerate() {
            if i >= java_class.methods.len() {
                break;
            }

            let method_name = util::get_utf8(&class.const_pool, method.name_index)
                .unwrap_or("unknown")
                .to_string();

            // Skip abstract and native methods
            if method.access_flags.contains(MethodAccessFlags::ABSTRACT)
                || method.access_flags.contains(MethodAccessFlags::NATIVE)
            {
                continue;
            }

            let code_attr = match method.code() {
                Some(code) => code,
                None => continue,
            };

            // Per-method error recovery: if decompilation fails, store the error
            match self.decompile_method_body(
                code_attr,
                &class.const_pool,
                method.access_flags.contains(MethodAccessFlags::STATIC),
            ) {
                Ok(body) => {
                    java_class.methods[i].body = Some(body);
                }
                Err(e) => {
                    // Build bytecode fallback comment
                    let mut error_msg = format!(
                        "Decompilation failed for method '{}': {}\nBytecode:",
                        method_name, e
                    );
                    let addressed = super::util::compute_addresses(&code_attr.code);
                    for (addr, instr) in addressed.iter().take(20) {
                        error_msg.push_str(&format!("\n  {:04}: {:?}", addr, instr));
                    }
                    if addressed.len() > 20 {
                        error_msg.push_str(&format!(
                            "\n  ... ({} more instructions)",
                            addressed.len() - 20
                        ));
                    }
                    java_class.methods[i].error = Some(error_msg);
                }
            }
        }

        Ok(java_class)
    }

    fn decompile_method_body(
        &self,
        code_attr: &crate::attribute_info::CodeAttribute,
        const_pool: &[crate::constant_info::ConstantInfo],
        is_static: bool,
    ) -> Result<super::structured_types::StructuredBody, DecompileError> {
        // Phase 1: Build CFG
        let cfg = cfg::build_cfg(code_attr);

        if cfg.blocks.is_empty() {
            return Ok(super::structured_types::StructuredBody::new(vec![]));
        }

        // Phase 2: Stack simulation
        let simulated = stack_sim::simulate_all_blocks(&cfg, const_pool, code_attr, is_static);

        // Phase 3: Control flow structuring
        let mut body = structuring::structure_method(&cfg, &simulated, const_pool);

        // Phase 3b: Desugaring
        let desugar_options = DesugarOptions {
            foreach: self.options.desugar_foreach,
            try_resources: self.options.desugar_try_resources,
            enum_switch: self.options.desugar_enum_switch,
            string_switch: self.options.desugar_string_switch,
            assert: self.options.desugar_assert,
            autobox: self.options.desugar_autobox,
            synthetic_accessors: true,
        };
        desugar::desugar(&mut body, &desugar_options);

        Ok(body)
    }
}

fn is_static_inner(class: &ClassFile) -> bool {
    // Check InnerClasses attribute for the static flag
    for attr in &class.attributes {
        if let Some(AttributeInfoVariant::InnerClasses(ic)) = &attr.info_parsed {
            for entry in &ic.classes {
                if entry.inner_class_info_index == class.this_class {
                    return (entry.inner_class_access_flags & 0x0008) != 0; // ACC_STATIC
                }
            }
        }
    }
    false
}

/// Convenience function: decompile a ClassFile with default options.
pub fn decompile(class: &ClassFile) -> Result<String, DecompileError> {
    let decompiler = Decompiler::new(DecompileOptions::default());
    decompiler.decompile(class)
}
