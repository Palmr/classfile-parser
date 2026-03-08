use std::collections::BTreeSet;
use std::fmt::Write;

use super::descriptor;
use super::expr::*;
use super::java_ast::*;
use super::structured_types::*;

/// Configuration for rendering Java source code.
#[derive(Clone, Debug)]
pub struct RenderConfig {
    pub indent: String,
    pub max_line_width: usize,
    pub use_var: bool,
    pub include_synthetic: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            indent: "    ".into(),
            max_line_width: 120,
            use_var: false,
            include_synthetic: false,
        }
    }
}

/// Java source code renderer.
pub struct JavaRenderer {
    config: RenderConfig,
    imports: BTreeSet<String>,
    output: String,
    indent_level: usize,
}

impl JavaRenderer {
    pub fn new(config: RenderConfig) -> Self {
        Self {
            config,
            imports: BTreeSet::new(),
            output: String::new(),
            indent_level: 0,
        }
    }

    pub fn render_class(mut self, class: &JavaClass) -> String {
        // Collect imports
        self.collect_imports(class);

        // Package declaration
        if let Some(ref pkg) = class.package {
            self.writeln(&format!("package {};", pkg));
            self.newline();
        }

        // Import declarations
        if !self.imports.is_empty() {
            let imports: Vec<String> = self.imports.iter().cloned().collect();
            for import in &imports {
                self.writeln(&format!("import {};", import));
            }
            self.newline();
        }

        // Class declaration
        self.render_class_decl(class);

        self.output
    }

    fn render_class_decl(&mut self, class: &JavaClass) {
        // Annotations
        for ann in &class.annotations {
            self.write_indent();
            self.render_annotation(ann);
            self.raw_newline();
        }

        // Modifiers + kind + name
        self.write_indent();
        let mut decl = String::new();
        self.append_visibility(&mut decl, &class.visibility);
        if class.is_abstract && class.kind != ClassKind::Interface {
            decl.push_str("abstract ");
        }
        if class.is_sealed {
            decl.push_str("sealed ");
        }
        if class.is_final && class.kind != ClassKind::Enum && class.kind != ClassKind::Record {
            decl.push_str("final ");
        }
        if class.is_static {
            decl.push_str("static ");
        }

        match class.kind {
            ClassKind::Class => decl.push_str("class "),
            ClassKind::Interface => decl.push_str("interface "),
            ClassKind::Enum => decl.push_str("enum "),
            ClassKind::Annotation => decl.push_str("@interface "),
            ClassKind::Record => decl.push_str("record "),
        }

        decl.push_str(&class.name);

        // Type parameters
        if !class.type_parameters.is_empty() {
            decl.push('<');
            for (i, tp) in class.type_parameters.iter().enumerate() {
                if i > 0 {
                    decl.push_str(", ");
                }
                decl.push_str(&tp.name);
                if !tp.bounds.is_empty() {
                    decl.push_str(" extends ");
                    for (j, b) in tp.bounds.iter().enumerate() {
                        if j > 0 {
                            decl.push_str(" & ");
                        }
                        decl.push_str(&b.display_name());
                    }
                }
            }
            decl.push('>');
        }

        // Record components
        if class.kind == ClassKind::Record {
            decl.push('(');
            for (i, comp) in class.record_components.iter().enumerate() {
                if i > 0 {
                    decl.push_str(", ");
                }
                decl.push_str(&comp.component_type.display_name());
                decl.push(' ');
                decl.push_str(&comp.name);
            }
            decl.push(')');
        }

        // Extends
        if let Some(ref super_class) = class.super_class
            && class.kind == ClassKind::Class
        {
            decl.push_str(" extends ");
            decl.push_str(&super_class.display_name());
        }

        // Implements / extends (for interfaces)
        if !class.interfaces.is_empty() {
            if class.kind == ClassKind::Interface {
                decl.push_str(" extends ");
            } else {
                decl.push_str(" implements ");
            }
            for (i, iface) in class.interfaces.iter().enumerate() {
                if i > 0 {
                    decl.push_str(", ");
                }
                decl.push_str(&iface.display_name());
            }
        }

        // Permits
        if !class.permitted_subclasses.is_empty() {
            decl.push_str(" permits ");
            for (i, sub) in class.permitted_subclasses.iter().enumerate() {
                if i > 0 {
                    decl.push_str(", ");
                }
                decl.push_str(&sub.display_name());
            }
        }

        decl.push_str(" {");
        self.raw(&decl);
        self.raw_newline();
        self.indent_level += 1;

        // Enum constants
        if class.kind == ClassKind::Enum {
            self.render_enum_constants(class);
        }

        // Fields
        let visible_fields: Vec<&JavaField> = class
            .fields
            .iter()
            .filter(|f| self.should_show_field(f, &class.kind))
            .collect();
        for field in &visible_fields {
            self.render_field(field);
        }
        if !visible_fields.is_empty() {
            self.newline();
        }

        // Methods
        let visible_methods: Vec<&JavaMethod> = class
            .methods
            .iter()
            .filter(|m| self.should_show_method(m, &class.kind))
            .collect();
        for (i, method) in visible_methods.iter().enumerate() {
            self.render_method(method, &class.kind, &class.name);
            if i + 1 < visible_methods.len() {
                self.newline();
            }
        }

        // Inner classes
        for inner in &class.inner_classes {
            self.newline();
            self.render_class_decl(inner);
        }

        self.indent_level -= 1;
        self.writeln("}");
    }

    fn render_enum_constants(&mut self, class: &JavaClass) {
        let enum_fields: Vec<&JavaField> =
            class.fields.iter().filter(|f| f.is_enum_constant).collect();

        if !enum_fields.is_empty() {
            for (i, field) in enum_fields.iter().enumerate() {
                self.write_indent();
                self.raw(&field.name);
                if i + 1 < enum_fields.len() {
                    self.raw(",");
                } else {
                    self.raw(";");
                }
                self.raw_newline();
            }
            self.newline();
        }
    }

    fn render_field(&mut self, field: &JavaField) {
        for ann in &field.annotations {
            self.write_indent();
            self.render_annotation(ann);
            self.raw_newline();
        }

        self.write_indent();
        let mut decl = String::new();
        self.append_visibility(&mut decl, &field.visibility);
        if field.is_static {
            decl.push_str("static ");
        }
        if field.is_final {
            decl.push_str("final ");
        }
        if field.is_volatile {
            decl.push_str("volatile ");
        }
        if field.is_transient {
            decl.push_str("transient ");
        }
        decl.push_str(&field.field_type.display_name());
        decl.push(' ');
        decl.push_str(&field.name);

        if let Some(ref init) = field.initializer {
            decl.push_str(" = ");
            decl.push_str(&self.render_expr(init));
        }

        decl.push(';');
        self.raw(&decl);
        self.raw_newline();
    }

    fn render_method(&mut self, method: &JavaMethod, class_kind: &ClassKind, class_name: &str) {
        // Annotations
        for ann in &method.annotations {
            self.write_indent();
            self.render_annotation(ann);
            self.raw_newline();
        }

        self.write_indent();
        let mut decl = String::new();
        self.append_visibility(&mut decl, &method.visibility);
        if method.is_default {
            decl.push_str("default ");
        }
        if method.is_static {
            decl.push_str("static ");
        }
        if method.is_abstract && *class_kind != ClassKind::Interface {
            decl.push_str("abstract ");
        }
        if method.is_final {
            decl.push_str("final ");
        }
        if method.is_synchronized {
            decl.push_str("synchronized ");
        }
        if method.is_native {
            decl.push_str("native ");
        }

        // Type parameters
        if !method.type_parameters.is_empty() {
            decl.push('<');
            for (i, tp) in method.type_parameters.iter().enumerate() {
                if i > 0 {
                    decl.push_str(", ");
                }
                decl.push_str(&tp.name);
                if !tp.bounds.is_empty() {
                    decl.push_str(" extends ");
                    for (j, b) in tp.bounds.iter().enumerate() {
                        if j > 0 {
                            decl.push_str(" & ");
                        }
                        decl.push_str(&b.display_name());
                    }
                }
            }
            decl.push_str("> ");
        }

        let is_constructor = method.name == "<init>";
        let is_static_init = method.name == "<clinit>";

        if is_static_init {
            decl.clear();
            self.raw("static");
        } else if is_constructor {
            decl.push_str(class_name);
        } else {
            decl.push_str(&method.return_type.display_name());
            decl.push(' ');
            decl.push_str(&method.name);
        }

        if !is_static_init {
            decl.push('(');
            for (i, param) in method.parameters.iter().enumerate() {
                if i > 0 {
                    decl.push_str(", ");
                }
                for ann in &param.annotations {
                    self.render_annotation_to_string(ann, &mut decl);
                    decl.push(' ');
                }
                if param.is_final {
                    decl.push_str("final ");
                }
                if param.is_varargs {
                    // Replace last [] with ...
                    let type_str = param.param_type.display_name();
                    if let Some(stripped) = type_str.strip_suffix("[]") {
                        decl.push_str(stripped);
                        decl.push_str("...");
                    } else {
                        decl.push_str(&type_str);
                    }
                } else {
                    decl.push_str(&param.param_type.display_name());
                }
                decl.push(' ');
                decl.push_str(&param.name);
            }
            decl.push(')');
        }

        // Throws
        if !method.throws.is_empty() {
            decl.push_str(" throws ");
            for (i, t) in method.throws.iter().enumerate() {
                if i > 0 {
                    decl.push_str(", ");
                }
                decl.push_str(&t.display_name());
            }
        }

        self.raw(&decl);

        // Body
        if let Some(ref error) = method.error {
            self.raw(" {");
            self.raw_newline();
            self.indent_level += 1;
            for line in error.lines() {
                self.writeln(&format!("// {}", line));
            }
            self.indent_level -= 1;
            self.writeln("}");
        } else if let Some(ref body) = method.body {
            self.raw(" {");
            self.raw_newline();
            self.indent_level += 1;
            self.render_body(body);
            self.indent_level -= 1;
            self.writeln("}");
        } else if method.is_abstract
            || method.is_native
            || (*class_kind == ClassKind::Interface && !method.is_default && !method.is_static)
        {
            self.raw(";");
            self.raw_newline();
        } else {
            self.raw(" {");
            self.raw_newline();
            self.writeln("}");
        }
    }

    fn render_body(&mut self, body: &StructuredBody) {
        for stmt in &body.statements {
            self.render_structured_stmt(stmt);
        }
    }

    fn render_structured_stmt(&mut self, stmt: &StructuredStmt) {
        match stmt {
            StructuredStmt::Simple(s) => self.render_simple_stmt(s),
            StructuredStmt::Block(stmts) => {
                for s in stmts {
                    self.render_structured_stmt(s);
                }
            }
            StructuredStmt::If {
                condition,
                then_body,
                else_body,
            } => {
                self.write_indent();
                self.raw(&format!("if ({}) {{", self.render_expr(condition)));
                self.raw_newline();
                self.indent_level += 1;
                self.render_structured_stmt(then_body);
                self.indent_level -= 1;
                if let Some(eb) = else_body {
                    self.writeln("} else {");
                    self.indent_level += 1;
                    self.render_structured_stmt(eb);
                    self.indent_level -= 1;
                }
                self.writeln("}");
            }
            StructuredStmt::While { condition, body } => {
                self.write_indent();
                self.raw(&format!("while ({}) {{", self.render_expr(condition)));
                self.raw_newline();
                self.indent_level += 1;
                self.render_structured_stmt(body);
                self.indent_level -= 1;
                self.writeln("}");
            }
            StructuredStmt::DoWhile { body, condition } => {
                self.writeln("do {");
                self.indent_level += 1;
                self.render_structured_stmt(body);
                self.indent_level -= 1;
                self.write_indent();
                self.raw(&format!("}} while ({});", self.render_expr(condition)));
                self.raw_newline();
            }
            StructuredStmt::For {
                init,
                condition,
                update,
                body,
            } => {
                self.write_indent();
                let init_str = init
                    .as_ref()
                    .map(|s| self.render_stmt_inline(s))
                    .unwrap_or_default();
                let update_str = update
                    .as_ref()
                    .map(|s| self.render_stmt_inline(s))
                    .unwrap_or_default();
                self.raw(&format!(
                    "for ({}; {}; {}) {{",
                    init_str,
                    self.render_expr(condition),
                    update_str
                ));
                self.raw_newline();
                self.indent_level += 1;
                self.render_structured_stmt(body);
                self.indent_level -= 1;
                self.writeln("}");
            }
            StructuredStmt::ForEach {
                var,
                iterable,
                body,
            } => {
                self.write_indent();
                let type_name = var.ty.simple_name();
                let var_name = var.name.as_deref().unwrap_or("item");
                self.raw(&format!(
                    "for ({} {} : {}) {{",
                    type_name,
                    var_name,
                    self.render_expr(iterable)
                ));
                self.raw_newline();
                self.indent_level += 1;
                self.render_structured_stmt(body);
                self.indent_level -= 1;
                self.writeln("}");
            }
            StructuredStmt::Switch {
                expr,
                cases,
                default,
            } => {
                self.write_indent();
                self.raw(&format!("switch ({}) {{", self.render_expr(expr)));
                self.raw_newline();
                self.indent_level += 1;
                for case in cases {
                    self.write_indent();
                    let labels: Vec<String> = case
                        .values
                        .iter()
                        .map(|v| match v {
                            SwitchValue::Int(i) => format!("{}", i),
                            SwitchValue::String(s) => format!("\"{}\"", s),
                            SwitchValue::Enum { const_name, .. } => const_name.clone(),
                        })
                        .collect();
                    for (i, label) in labels.iter().enumerate() {
                        if i > 0 {
                            self.raw_newline();
                            self.write_indent();
                        }
                        self.raw(&format!("case {}:", label));
                    }
                    self.raw_newline();
                    self.indent_level += 1;
                    self.render_structured_stmt(&case.body);
                    if !case.falls_through {
                        self.writeln("break;");
                    }
                    self.indent_level -= 1;
                }
                if let Some(def) = default {
                    self.writeln("default:");
                    self.indent_level += 1;
                    self.render_structured_stmt(def);
                    self.writeln("break;");
                    self.indent_level -= 1;
                }
                self.indent_level -= 1;
                self.writeln("}");
            }
            StructuredStmt::TryCatch {
                try_body,
                catches,
                finally_body,
            } => {
                self.writeln("try {");
                self.indent_level += 1;
                self.render_structured_stmt(try_body);
                self.indent_level -= 1;
                for catch in catches {
                    let exc_type = catch.exception_type.as_deref().unwrap_or("Throwable");
                    let var_name = catch.var.name.as_deref().unwrap_or("e");
                    self.write_indent();
                    self.raw(&format!("}} catch ({} {}) {{", exc_type, var_name));
                    self.raw_newline();
                    self.indent_level += 1;
                    self.render_structured_stmt(&catch.body);
                    self.indent_level -= 1;
                }
                if let Some(fin) = finally_body {
                    self.writeln("} finally {");
                    self.indent_level += 1;
                    self.render_structured_stmt(fin);
                    self.indent_level -= 1;
                }
                self.writeln("}");
            }
            StructuredStmt::TryWithResources {
                resources,
                body,
                catches,
            } => {
                self.write_indent();
                self.raw("try (");
                for (i, (var, init)) in resources.iter().enumerate() {
                    if i > 0 {
                        self.raw("; ");
                    }
                    let type_name = var.ty.simple_name();
                    let var_name = var.name.as_deref().unwrap_or("r");
                    self.raw(&format!(
                        "{} {} = {}",
                        type_name,
                        var_name,
                        self.render_expr(init)
                    ));
                }
                self.raw(") {");
                self.raw_newline();
                self.indent_level += 1;
                self.render_structured_stmt(body);
                self.indent_level -= 1;
                for catch in catches {
                    let exc_type = catch.exception_type.as_deref().unwrap_or("Throwable");
                    let var_name = catch.var.name.as_deref().unwrap_or("e");
                    self.write_indent();
                    self.raw(&format!("}} catch ({} {}) {{", exc_type, var_name));
                    self.raw_newline();
                    self.indent_level += 1;
                    self.render_structured_stmt(&catch.body);
                    self.indent_level -= 1;
                }
                self.writeln("}");
            }
            StructuredStmt::Synchronized { object, body } => {
                self.write_indent();
                self.raw(&format!("synchronized ({}) {{", self.render_expr(object)));
                self.raw_newline();
                self.indent_level += 1;
                self.render_structured_stmt(body);
                self.indent_level -= 1;
                self.writeln("}");
            }
            StructuredStmt::Labeled { label, body } => {
                self.writeln(&format!("{}:", label));
                self.render_structured_stmt(body);
            }
            StructuredStmt::Break { label } => {
                if let Some(l) = label {
                    self.writeln(&format!("break {};", l));
                } else {
                    self.writeln("break;");
                }
            }
            StructuredStmt::Continue { label } => {
                if let Some(l) = label {
                    self.writeln(&format!("continue {};", l));
                } else {
                    self.writeln("continue;");
                }
            }
            StructuredStmt::Assert { condition, message } => {
                self.write_indent();
                if let Some(msg) = message {
                    self.raw(&format!(
                        "assert {} : {};",
                        self.render_expr(condition),
                        self.render_expr(msg)
                    ));
                } else {
                    self.raw(&format!("assert {};", self.render_expr(condition)));
                }
                self.raw_newline();
            }
            StructuredStmt::UnstructuredGoto { target } => {
                self.writeln(&format!("// goto B{}", target));
            }
            StructuredStmt::Comment(text) => {
                self.writeln(&format!("// {}", text));
            }
        }
    }

    fn render_simple_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::LocalStore { var, value } => {
                let type_name = var.ty.simple_name();
                let default_name = format!("var{}", var.index);
                let var_name = var.name.as_deref().unwrap_or(&default_name);
                // TODO: Track which variables have been declared vs assigned
                self.writeln(&format!(
                    "{} {} = {};",
                    type_name,
                    var_name,
                    self.render_expr(value)
                ));
            }
            Stmt::FieldStore {
                object,
                class_name,
                field_name,
                value,
                ..
            } => {
                let target = match object {
                    Some(obj) => format!("{}.{}", self.render_expr(obj), field_name),
                    None => format!(
                        "{}.{}",
                        descriptor::simple_class_name(class_name),
                        field_name
                    ),
                };
                self.writeln(&format!("{} = {};", target, self.render_expr(value)));
            }
            Stmt::ArrayStore {
                array,
                index,
                value,
            } => {
                self.writeln(&format!(
                    "{}[{}] = {};",
                    self.render_expr(array),
                    self.render_expr(index),
                    self.render_expr(value)
                ));
            }
            Stmt::ExprStmt(expr) => {
                self.writeln(&format!("{};", self.render_expr(expr)));
            }
            Stmt::Iinc { var, amount } => {
                let default_name = format!("var{}", var.index);
                let var_name = var.name.as_deref().unwrap_or(&default_name);
                if *amount == 1 {
                    self.writeln(&format!("{}++;", var_name));
                } else if *amount == -1 {
                    self.writeln(&format!("{}--;", var_name));
                } else {
                    self.writeln(&format!("{} += {};", var_name, amount));
                }
            }
            Stmt::Return(None) => {
                self.writeln("return;");
            }
            Stmt::Return(Some(expr)) => {
                self.writeln(&format!("return {};", self.render_expr(expr)));
            }
            Stmt::Throw(expr) => {
                self.writeln(&format!("throw {};", self.render_expr(expr)));
            }
            Stmt::Monitor { enter, object } => {
                if *enter {
                    self.writeln(&format!("// monitorenter {}", self.render_expr(object)));
                } else {
                    self.writeln(&format!("// monitorexit {}", self.render_expr(object)));
                }
            }
        }
    }

    fn render_expr(&self, expr: &Expr) -> String {
        match expr {
            Expr::IntLiteral(v) => format!("{}", v),
            Expr::LongLiteral(v) => format!("{}L", v),
            Expr::FloatLiteral(v) => {
                if v.is_nan() {
                    "Float.NaN".into()
                } else if v.is_infinite() {
                    if *v > 0.0 {
                        "Float.POSITIVE_INFINITY".into()
                    } else {
                        "Float.NEGATIVE_INFINITY".into()
                    }
                } else {
                    format!("{}f", v)
                }
            }
            Expr::DoubleLiteral(v) => {
                if v.is_nan() {
                    "Double.NaN".into()
                } else if v.is_infinite() {
                    if *v > 0.0 {
                        "Double.POSITIVE_INFINITY".into()
                    } else {
                        "Double.NEGATIVE_INFINITY".into()
                    }
                } else {
                    format!("{}d", v)
                }
            }
            Expr::StringLiteral(s) => format!("\"{}\"", escape_java_string(s)),
            Expr::ClassLiteral(name) => format!("{}.class", descriptor::simple_class_name(name)),
            Expr::NullLiteral => "null".into(),
            Expr::LocalLoad(var) => var
                .name
                .as_deref()
                .unwrap_or(&format!("var{}", var.index))
                .to_string(),
            Expr::This => "this".into(),
            Expr::BinaryOp { op, left, right } => {
                let op_str = match op {
                    BinOp::Add => "+",
                    BinOp::Sub => "-",
                    BinOp::Mul => "*",
                    BinOp::Div => "/",
                    BinOp::Rem => "%",
                    BinOp::Shl => "<<",
                    BinOp::Shr => ">>",
                    BinOp::Ushr => ">>>",
                    BinOp::And => "&",
                    BinOp::Or => "|",
                    BinOp::Xor => "^",
                };
                format!(
                    "{} {} {}",
                    self.render_expr_parens(left, op),
                    op_str,
                    self.render_expr_parens(right, op)
                )
            }
            Expr::UnaryOp { op, operand } => {
                let op_str = match op {
                    UnaryOp::Neg => "-",
                    UnaryOp::Not => "!",
                };
                format!("{}{}", op_str, self.render_expr(operand))
            }
            Expr::Cast {
                target_type,
                operand,
            } => {
                format!(
                    "({}){}",
                    target_type.simple_name(),
                    self.render_expr(operand)
                )
            }
            Expr::Instanceof {
                operand,
                check_type,
            } => {
                format!(
                    "{} instanceof {}",
                    self.render_expr(operand),
                    descriptor::simple_class_name(check_type)
                )
            }
            Expr::FieldGet {
                object,
                class_name,
                field_name,
                ..
            } => match object {
                Some(obj) => format!("{}.{}", self.render_expr(obj), field_name),
                None => format!(
                    "{}.{}",
                    descriptor::simple_class_name(class_name),
                    field_name
                ),
            },
            Expr::MethodCall {
                object,
                class_name,
                method_name,
                args,
                kind,
                ..
            } => {
                let args_str: Vec<String> = args.iter().map(|a| self.render_expr(a)).collect();
                let args_joined = args_str.join(", ");
                match kind {
                    InvokeKind::Static => {
                        format!(
                            "{}.{}({})",
                            descriptor::simple_class_name(class_name),
                            method_name,
                            args_joined
                        )
                    }
                    _ => {
                        let receiver = object
                            .as_ref()
                            .map(|o| self.render_expr(o))
                            .unwrap_or_else(|| "this".into());
                        if method_name == "<init>" {
                            format!("super({})", args_joined)
                        } else {
                            format!("{}.{}({})", receiver, method_name, args_joined)
                        }
                    }
                }
            }
            Expr::New {
                class_name, args, ..
            } => {
                let args_str: Vec<String> = args.iter().map(|a| self.render_expr(a)).collect();
                format!(
                    "new {}({})",
                    descriptor::simple_class_name(class_name),
                    args_str.join(", ")
                )
            }
            Expr::NewArray {
                element_type,
                length,
            } => {
                format!(
                    "new {}[{}]",
                    element_type.simple_name(),
                    self.render_expr(length)
                )
            }
            Expr::NewMultiArray {
                element_type,
                dimensions,
            } => {
                let dims: Vec<String> = dimensions
                    .iter()
                    .map(|d| format!("[{}]", self.render_expr(d)))
                    .collect();
                format!("new {}{}", element_type.simple_name(), dims.join(""))
            }
            Expr::ArrayLength { array } => {
                format!("{}.length", self.render_expr(array))
            }
            Expr::ArrayLoad { array, index, .. } => {
                format!("{}[{}]", self.render_expr(array), self.render_expr(index))
            }
            Expr::Compare { op, left, right } => {
                format!(
                    "{} {} {}",
                    self.render_expr(left),
                    op.as_str(),
                    self.render_expr(right)
                )
            }
            Expr::CmpResult { left, right, .. } => {
                // This should be folded into a Compare during structuring
                format!(
                    "/* cmp */ {} <=> {}",
                    self.render_expr(left),
                    self.render_expr(right)
                )
            }
            Expr::InvokeDynamic {
                method_name,
                captures,
                ..
            } => {
                if captures.is_empty() {
                    format!("/* lambda */ {}()", method_name)
                } else {
                    let caps: Vec<String> = captures.iter().map(|c| self.render_expr(c)).collect();
                    format!("/* lambda */ {}({})", method_name, caps.join(", "))
                }
            }
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
            } => {
                format!(
                    "{} ? {} : {}",
                    self.render_expr(condition),
                    self.render_expr(then_expr),
                    self.render_expr(else_expr)
                )
            }
            Expr::Unresolved(msg) => msg.clone(),
            Expr::Dup(inner) => self.render_expr(inner),
            Expr::UninitNew { class_name } => format!(
                "/* uninit */ new {}",
                descriptor::simple_class_name(class_name)
            ),
        }
    }

    fn render_expr_parens(&self, expr: &Expr, _parent_op: &BinOp) -> String {
        // Add parentheses around binary operations with lower precedence
        match expr {
            Expr::BinaryOp { .. } => format!("({})", self.render_expr(expr)),
            _ => self.render_expr(expr),
        }
    }

    fn render_stmt_inline(&self, stmt: &StructuredStmt) -> String {
        match stmt {
            StructuredStmt::Simple(s) => match s {
                Stmt::LocalStore { var, value } => {
                    let default_name = format!("var{}", var.index);
                    let var_name = var.name.as_deref().unwrap_or(&default_name);
                    format!("{} = {}", var_name, self.render_expr(value))
                }
                Stmt::Iinc { var, amount } => {
                    let default_name = format!("var{}", var.index);
                    let var_name = var.name.as_deref().unwrap_or(&default_name);
                    if *amount == 1 {
                        format!("{}++", var_name)
                    } else {
                        format!("{} += {}", var_name, amount)
                    }
                }
                Stmt::ExprStmt(e) => self.render_expr(e),
                _ => "/* stmt */".into(),
            },
            _ => "/* stmt */".into(),
        }
    }

    fn render_annotation(&mut self, ann: &JavaAnnotation) {
        let mut s = String::new();
        self.render_annotation_to_string(ann, &mut s);
        self.raw(&s);
    }

    fn render_annotation_to_string(&self, ann: &JavaAnnotation, out: &mut String) {
        out.push('@');
        out.push_str(&ann.type_name);
        if !ann.arguments.is_empty() {
            out.push('(');
            for (i, arg) in ann.arguments.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                match arg {
                    AnnotationArgument::Named { name, value } => {
                        if name == "value" && ann.arguments.len() == 1 {
                            self.render_annotation_value(value, out);
                        } else {
                            out.push_str(name);
                            out.push_str(" = ");
                            self.render_annotation_value(value, out);
                        }
                    }
                    AnnotationArgument::Unnamed(value) => {
                        self.render_annotation_value(value, out);
                    }
                }
            }
            out.push(')');
        }
    }

    fn render_annotation_value(&self, value: &AnnotationValue, out: &mut String) {
        match value {
            AnnotationValue::IntLiteral(v) => write!(out, "{}", v).unwrap(),
            AnnotationValue::LongLiteral(v) => write!(out, "{}L", v).unwrap(),
            AnnotationValue::FloatLiteral(v) => write!(out, "{}f", v).unwrap(),
            AnnotationValue::DoubleLiteral(v) => write!(out, "{}d", v).unwrap(),
            AnnotationValue::StringLiteral(s) => {
                write!(out, "\"{}\"", escape_java_string(s)).unwrap()
            }
            AnnotationValue::BooleanLiteral(b) => write!(out, "{}", b).unwrap(),
            AnnotationValue::CharLiteral(c) => write!(out, "'{}'", c).unwrap(),
            AnnotationValue::ClassLiteral(c) => write!(out, "{}.class", c).unwrap(),
            AnnotationValue::EnumConstant {
                type_name,
                const_name,
            } => {
                write!(out, "{}.{}", type_name, const_name).unwrap();
            }
            AnnotationValue::AnnotationLiteral(ann) => {
                self.render_annotation_to_string(ann, out);
            }
            AnnotationValue::ArrayLiteral(values) => {
                out.push('{');
                for (i, v) in values.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    self.render_annotation_value(v, out);
                }
                out.push('}');
            }
        }
    }

    fn should_show_field(&self, field: &JavaField, class_kind: &ClassKind) -> bool {
        if !self.config.include_synthetic && field.is_synthetic {
            return false;
        }
        // Skip enum $VALUES
        if *class_kind == ClassKind::Enum && field.name == "$VALUES" {
            return false;
        }
        // Skip enum constants in fields list (rendered separately)
        if field.is_enum_constant {
            return false;
        }
        true
    }

    fn should_show_method(&self, method: &JavaMethod, class_kind: &ClassKind) -> bool {
        if !self.config.include_synthetic && (method.is_synthetic || method.is_bridge) {
            return false;
        }
        // Skip static initializer if empty
        if method.name == "<clinit>" && method.body.is_none() && method.error.is_none() {
            return false;
        }
        // Skip auto-generated enum methods
        if *class_kind == ClassKind::Enum && (method.name == "values" || method.name == "valueOf") {
            return false;
        }
        true
    }

    fn collect_imports(&mut self, class: &JavaClass) {
        self.collect_type_imports(&class.super_class);
        for iface in &class.interfaces {
            self.collect_type_import(iface);
        }
        for field in &class.fields {
            self.collect_type_import(&field.field_type);
        }
        for method in &class.methods {
            self.collect_type_import(&method.return_type);
            for param in &method.parameters {
                self.collect_type_import(&param.param_type);
            }
            for t in &method.throws {
                self.collect_type_import(t);
            }
        }
    }

    fn collect_type_imports(&mut self, ty: &Option<JavaType>) {
        if let Some(t) = ty {
            self.collect_type_import(t);
        }
    }

    fn collect_type_import(&mut self, ty: &JavaType) {
        match ty {
            JavaType::ClassType {
                package,
                name,
                type_args,
            } => {
                if let Some(pkg) = package
                    && pkg != "java.lang"
                {
                    self.imports.insert(format!("{}.{}", pkg, name));
                }
                for arg in type_args {
                    self.collect_type_import(arg);
                }
            }
            JavaType::ArrayType(inner) => self.collect_type_import(inner),
            JavaType::WildcardType { bound: Some(b), .. } => self.collect_type_import(b),
            _ => {}
        }
    }

    fn append_visibility(&self, out: &mut String, vis: &Visibility) {
        match vis {
            Visibility::Public => out.push_str("public "),
            Visibility::Protected => out.push_str("protected "),
            Visibility::Private => out.push_str("private "),
            Visibility::PackagePrivate => {}
        }
    }

    fn write_indent(&mut self) {
        for _ in 0..self.indent_level {
            self.output.push_str(&self.config.indent);
        }
    }

    fn writeln(&mut self, text: &str) {
        self.write_indent();
        self.output.push_str(text);
        self.output.push('\n');
    }

    fn newline(&mut self) {
        self.output.push('\n');
    }

    fn raw(&mut self, text: &str) {
        self.output.push_str(text);
    }

    fn raw_newline(&mut self) {
        self.output.push('\n');
    }
}

fn escape_java_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            c if c.is_control() => {
                write!(out, "\\u{:04x}", c as u32).unwrap();
            }
            c => out.push(c),
        }
    }
    out
}
