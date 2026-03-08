use crate::attribute_info::*;
use crate::constant_info::ConstantInfo;
use crate::field_info::{FieldAccessFlags, FieldInfo};
use crate::method_info::{MethodAccessFlags, MethodInfo};
use crate::types::{ClassAccessFlags, ClassFile};

use super::descriptor::{self, JvmType};
use super::expr::Expr;
use super::java_ast::*;
use super::util;

/// Build a JavaClass from a parsed ClassFile.
pub fn build_java_class(class: &ClassFile) -> JavaClass {
    let const_pool = &class.const_pool;

    // Class name
    let full_name = util::get_class_name(const_pool, class.this_class).unwrap_or("Unknown");
    let (package, simple_name) = split_class_name(full_name);

    // Super class
    let super_class = if class.super_class != 0 {
        let super_name =
            util::get_class_name(const_pool, class.super_class).unwrap_or("java/lang/Object");
        if super_name != "java/lang/Object" {
            Some(internal_name_to_java_type(super_name))
        } else {
            None
        }
    } else {
        None
    };

    // Interfaces
    let interfaces: Vec<JavaType> = class
        .interfaces
        .iter()
        .filter_map(|&idx| {
            let name = util::get_class_name(const_pool, idx)?;
            Some(internal_name_to_java_type(name))
        })
        .collect();

    // Determine class kind
    let kind = determine_class_kind(class);
    let visibility = class_visibility(class.access_flags);
    let is_final = class.access_flags.contains(ClassAccessFlags::FINAL);
    let is_abstract = class.access_flags.contains(ClassAccessFlags::ABSTRACT);

    // Check for sealed (PermittedSubclasses attribute)
    let (is_sealed, permitted_subclasses) = extract_permitted_subclasses(class);

    // Record components
    let record_components = extract_record_components(class);

    // Source file
    let source_file = extract_source_file(class);

    // Annotations
    let annotations = extract_class_annotations(class);

    // Type parameters from Signature attribute
    let type_parameters = extract_class_type_parameters(class);

    // Fields
    let fields: Vec<JavaField> = class
        .fields
        .iter()
        .map(|f| build_java_field(f, const_pool))
        .collect();

    // Methods
    let methods: Vec<JavaMethod> = class
        .methods
        .iter()
        .map(|m| build_java_method(m, const_pool, &kind))
        .collect();

    JavaClass {
        kind,
        visibility,
        is_final,
        is_abstract,
        is_sealed,
        is_static: false,
        annotations,
        type_parameters,
        package,
        name: simple_name,
        super_class,
        interfaces,
        permitted_subclasses,
        record_components,
        fields,
        methods,
        inner_classes: Vec::new(),
        source_file,
    }
}

fn determine_class_kind(class: &ClassFile) -> ClassKind {
    let flags = class.access_flags;
    if flags.contains(ClassAccessFlags::ANNOTATION) {
        ClassKind::Annotation
    } else if flags.contains(ClassAccessFlags::ENUM) {
        ClassKind::Enum
    } else if flags.contains(ClassAccessFlags::INTERFACE) {
        ClassKind::Interface
    } else if has_record_attribute(class) {
        ClassKind::Record
    } else {
        ClassKind::Class
    }
}

fn has_record_attribute(class: &ClassFile) -> bool {
    class
        .attributes
        .iter()
        .any(|a| matches!(&a.info_parsed, Some(AttributeInfoVariant::Record(_))))
}

fn class_visibility(flags: ClassAccessFlags) -> Visibility {
    if flags.contains(ClassAccessFlags::PUBLIC) {
        Visibility::Public
    } else {
        Visibility::PackagePrivate
    }
}

fn method_visibility(flags: MethodAccessFlags) -> Visibility {
    if flags.contains(MethodAccessFlags::PUBLIC) {
        Visibility::Public
    } else if flags.contains(MethodAccessFlags::PROTECTED) {
        Visibility::Protected
    } else if flags.contains(MethodAccessFlags::PRIVATE) {
        Visibility::Private
    } else {
        Visibility::PackagePrivate
    }
}

fn field_visibility(flags: FieldAccessFlags) -> Visibility {
    if flags.contains(FieldAccessFlags::PUBLIC) {
        Visibility::Public
    } else if flags.contains(FieldAccessFlags::PROTECTED) {
        Visibility::Protected
    } else if flags.contains(FieldAccessFlags::PRIVATE) {
        Visibility::Private
    } else {
        Visibility::PackagePrivate
    }
}

fn split_class_name(internal_name: &str) -> (Option<String>, String) {
    match internal_name.rfind('/') {
        Some(pos) => {
            let pkg = internal_name[..pos].replace('/', ".");
            let name = internal_name[pos + 1..].to_string();
            // Handle inner classes: Outer$Inner -> Inner
            let simple = match name.rfind('$') {
                Some(dpos) => name[dpos + 1..].to_string(),
                None => name,
            };
            (Some(pkg), simple)
        }
        None => (None, internal_name.to_string()),
    }
}

fn internal_name_to_java_type(name: &str) -> JavaType {
    let _source = descriptor::internal_to_source_name(name);
    let simple = descriptor::simple_class_name(name).to_string();
    let package = descriptor::package_name(name).map(|p| p.replace('/', "."));
    JavaType::ClassType {
        package,
        name: simple,
        type_args: Vec::new(),
    }
}

fn jvm_type_to_java_type(ty: &JvmType) -> JavaType {
    match ty {
        JvmType::Int => JavaType::Primitive(PrimitiveType::Int),
        JvmType::Long => JavaType::Primitive(PrimitiveType::Long),
        JvmType::Float => JavaType::Primitive(PrimitiveType::Float),
        JvmType::Double => JavaType::Primitive(PrimitiveType::Double),
        JvmType::Byte => JavaType::Primitive(PrimitiveType::Byte),
        JvmType::Char => JavaType::Primitive(PrimitiveType::Char),
        JvmType::Short => JavaType::Primitive(PrimitiveType::Short),
        JvmType::Boolean => JavaType::Primitive(PrimitiveType::Boolean),
        JvmType::Void => JavaType::Void,
        JvmType::Reference(name) => internal_name_to_java_type(name),
        JvmType::Array(inner) => JavaType::ArrayType(Box::new(jvm_type_to_java_type(inner))),
        JvmType::Null | JvmType::Unknown => JavaType::ClassType {
            package: Some("java.lang".into()),
            name: "Object".into(),
            type_args: Vec::new(),
        },
    }
}

fn build_java_field(field: &FieldInfo, const_pool: &[ConstantInfo]) -> JavaField {
    let name = util::get_utf8(const_pool, field.name_index)
        .unwrap_or("unknown")
        .to_string();
    let desc = util::get_utf8(const_pool, field.descriptor_index).unwrap_or("I");
    let jvm_type = descriptor::parse_type_descriptor(desc).unwrap_or(JvmType::Unknown);
    let field_type = jvm_type_to_java_type(&jvm_type);
    let flags = field.access_flags;

    // Check for ConstantValue attribute (static final initializer)
    let initializer = extract_field_initializer(field, const_pool);

    // Annotations
    let annotations = extract_field_annotations(field, const_pool);

    JavaField {
        visibility: field_visibility(flags),
        is_static: flags.contains(FieldAccessFlags::STATIC),
        is_final: flags.contains(FieldAccessFlags::FINAL),
        is_volatile: flags.contains(FieldAccessFlags::VOLATILE),
        is_transient: flags.contains(FieldAccessFlags::TRANSIENT),
        is_synthetic: flags.contains(FieldAccessFlags::SYNTHETIC),
        is_enum_constant: flags.contains(FieldAccessFlags::ENUM),
        annotations,
        field_type,
        name,
        initializer,
    }
}

fn build_java_method(
    method: &MethodInfo,
    const_pool: &[ConstantInfo],
    class_kind: &ClassKind,
) -> JavaMethod {
    let name = util::get_utf8(const_pool, method.name_index)
        .unwrap_or("unknown")
        .to_string();
    let desc = util::get_utf8(const_pool, method.descriptor_index).unwrap_or("()V");
    let flags = method.access_flags;

    let (param_types, ret_type) =
        descriptor::parse_method_descriptor(desc).unwrap_or((vec![], JvmType::Void));

    let return_type = jvm_type_to_java_type(&ret_type);

    // Build parameters with names from MethodParameters or LocalVariableTable
    let param_names = extract_parameter_names(method, const_pool, param_types.len());
    let parameters: Vec<JavaParameter> = param_types
        .iter()
        .enumerate()
        .map(|(i, ty)| {
            let param_name = param_names
                .get(i)
                .cloned()
                .unwrap_or_else(|| format!("param{}", i));
            JavaParameter {
                annotations: Vec::new(),
                param_type: jvm_type_to_java_type(ty),
                name: param_name,
                is_final: false,
                is_varargs: i == param_types.len() - 1
                    && flags.contains(MethodAccessFlags::VARARGS),
            }
        })
        .collect();

    // Throws clause
    let throws = extract_throws(method, const_pool);

    // Annotations
    let annotations = extract_method_annotations(method, const_pool);

    let is_default = *class_kind == ClassKind::Interface
        && !flags.contains(MethodAccessFlags::ABSTRACT)
        && !flags.contains(MethodAccessFlags::STATIC);

    JavaMethod {
        visibility: method_visibility(flags),
        is_static: flags.contains(MethodAccessFlags::STATIC),
        is_final: flags.contains(MethodAccessFlags::FINAL),
        is_abstract: flags.contains(MethodAccessFlags::ABSTRACT),
        is_synchronized: flags.contains(MethodAccessFlags::SYNCHRONIZED),
        is_native: flags.contains(MethodAccessFlags::NATIVE),
        is_default,
        is_synthetic: flags.contains(MethodAccessFlags::SYNTHETIC),
        is_bridge: flags.contains(MethodAccessFlags::BRIDGE),
        annotations,
        type_parameters: Vec::new(),
        return_type,
        name,
        parameters,
        throws,
        body: None, // Populated later by the decompiler
        error: None,
    }
}

fn extract_parameter_names(
    method: &MethodInfo,
    const_pool: &[ConstantInfo],
    param_count: usize,
) -> Vec<String> {
    // Try MethodParameters attribute first
    for attr in &method.attributes {
        if let Some(AttributeInfoVariant::MethodParameters(mp)) = &attr.info_parsed {
            return mp
                .parameters
                .iter()
                .map(|p| {
                    if p.name_index != 0 {
                        util::get_utf8(const_pool, p.name_index)
                            .unwrap_or("param")
                            .to_string()
                    } else {
                        "param".to_string()
                    }
                })
                .collect();
        }
    }

    // Try LocalVariableTable from Code attribute
    if let Some(code) = method.code() {
        for attr in &code.attributes {
            if let Some(AttributeInfoVariant::LocalVariableTable(lvt)) = &attr.info_parsed {
                let is_static = method.access_flags.contains(MethodAccessFlags::STATIC);
                let start_idx: u16 = if is_static { 0 } else { 1 };
                let mut names = Vec::new();
                for i in 0..param_count {
                    let slot = start_idx + i as u16;
                    let name = lvt
                        .items
                        .iter()
                        .find(|item| item.index == slot && item.start_pc == 0)
                        .and_then(|item| util::get_utf8(const_pool, item.name_index))
                        .unwrap_or("param")
                        .to_string();
                    names.push(name);
                }
                return names;
            }
        }
    }

    // Fallback
    (0..param_count).map(|i| format!("param{}", i)).collect()
}

fn extract_throws(method: &MethodInfo, const_pool: &[ConstantInfo]) -> Vec<JavaType> {
    for attr in &method.attributes {
        if let Some(AttributeInfoVariant::Exceptions(exc)) = &attr.info_parsed {
            return exc
                .exception_table
                .iter()
                .filter_map(|&idx| {
                    let name = util::get_class_name(const_pool, idx)?;
                    Some(internal_name_to_java_type(name))
                })
                .collect();
        }
    }
    Vec::new()
}

fn extract_field_initializer(field: &FieldInfo, const_pool: &[ConstantInfo]) -> Option<Expr> {
    for attr in &field.attributes {
        if let Some(AttributeInfoVariant::ConstantValue(cv)) = &attr.info_parsed {
            let idx = cv.constant_value_index;
            return match const_pool.get((idx as usize).checked_sub(1)?) {
                Some(ConstantInfo::Integer(c)) => Some(Expr::IntLiteral(c.value)),
                Some(ConstantInfo::Long(c)) => Some(Expr::LongLiteral(c.value)),
                Some(ConstantInfo::Float(c)) => Some(Expr::FloatLiteral(c.value)),
                Some(ConstantInfo::Double(c)) => Some(Expr::DoubleLiteral(c.value)),
                Some(ConstantInfo::String(s)) => {
                    let string = util::get_utf8(const_pool, s.string_index)?.to_string();
                    Some(Expr::StringLiteral(string))
                }
                _ => None,
            };
        }
    }
    None
}

fn extract_permitted_subclasses(class: &ClassFile) -> (bool, Vec<JavaType>) {
    for attr in &class.attributes {
        if let Some(AttributeInfoVariant::PermittedSubclasses(ps)) = &attr.info_parsed {
            let types: Vec<JavaType> = ps
                .classes
                .iter()
                .filter_map(|&idx| {
                    let name = util::get_class_name(&class.const_pool, idx)?;
                    Some(internal_name_to_java_type(name))
                })
                .collect();
            return (true, types);
        }
    }
    (false, Vec::new())
}

fn extract_record_components(class: &ClassFile) -> Vec<RecordComponent> {
    for attr in &class.attributes {
        if let Some(AttributeInfoVariant::Record(rec)) = &attr.info_parsed {
            return rec
                .components
                .iter()
                .filter_map(|c| {
                    let name = util::get_utf8(&class.const_pool, c.name_index)?.to_string();
                    let desc = util::get_utf8(&class.const_pool, c.descriptor_index)?;
                    let jvm_type = descriptor::parse_type_descriptor(desc)?;
                    Some(RecordComponent {
                        annotations: Vec::new(),
                        component_type: jvm_type_to_java_type(&jvm_type),
                        name,
                    })
                })
                .collect();
        }
    }
    Vec::new()
}

fn extract_source_file(class: &ClassFile) -> Option<String> {
    for attr in &class.attributes {
        if let Some(AttributeInfoVariant::SourceFile(sf)) = &attr.info_parsed {
            return util::get_utf8(&class.const_pool, sf.sourcefile_index).map(|s| s.to_string());
        }
    }
    None
}

fn extract_class_annotations(class: &ClassFile) -> Vec<JavaAnnotation> {
    let mut annotations = Vec::new();
    for attr in &class.attributes {
        match &attr.info_parsed {
            Some(AttributeInfoVariant::RuntimeVisibleAnnotations(ra)) => {
                for ann in &ra.annotations {
                    if let Some(a) = convert_annotation(ann, &class.const_pool) {
                        annotations.push(a);
                    }
                }
            }
            Some(AttributeInfoVariant::Deprecated(_)) => {
                annotations.push(JavaAnnotation {
                    type_name: "Deprecated".into(),
                    arguments: Vec::new(),
                });
            }
            _ => {}
        }
    }
    annotations
}

fn extract_field_annotations(
    field: &FieldInfo,
    const_pool: &[ConstantInfo],
) -> Vec<JavaAnnotation> {
    let mut annotations = Vec::new();
    for attr in &field.attributes {
        if let Some(AttributeInfoVariant::RuntimeVisibleAnnotations(ra)) = &attr.info_parsed {
            for ann in &ra.annotations {
                if let Some(a) = convert_annotation(ann, const_pool) {
                    annotations.push(a);
                }
            }
        }
    }
    annotations
}

fn extract_method_annotations(
    method: &MethodInfo,
    const_pool: &[ConstantInfo],
) -> Vec<JavaAnnotation> {
    let mut annotations = Vec::new();
    for attr in &method.attributes {
        match &attr.info_parsed {
            Some(AttributeInfoVariant::RuntimeVisibleAnnotations(ra)) => {
                for ann in &ra.annotations {
                    if let Some(a) = convert_annotation(ann, const_pool) {
                        annotations.push(a);
                    }
                }
            }
            Some(AttributeInfoVariant::Deprecated(_)) => {
                annotations.push(JavaAnnotation {
                    type_name: "Deprecated".into(),
                    arguments: Vec::new(),
                });
            }
            _ => {}
        }
    }
    annotations
}

fn convert_annotation(
    ann: &RuntimeAnnotation,
    const_pool: &[ConstantInfo],
) -> Option<JavaAnnotation> {
    let type_desc = util::get_utf8(const_pool, ann.type_index)?;
    // Type descriptor is like "Ljava/lang/Override;" -> "Override"
    let type_name = if type_desc.starts_with('L') && type_desc.ends_with(';') {
        let internal = &type_desc[1..type_desc.len() - 1];
        descriptor::simple_class_name(internal).to_string()
    } else {
        type_desc.to_string()
    };

    let arguments: Vec<AnnotationArgument> = ann
        .element_value_pairs
        .iter()
        .filter_map(|evp| {
            let name = util::get_utf8(const_pool, evp.element_name_index)?.to_string();
            let value = convert_element_value(&evp.value, const_pool)?;
            Some(AnnotationArgument::Named { name, value })
        })
        .collect();

    Some(JavaAnnotation {
        type_name,
        arguments,
    })
}

fn convert_element_value(
    ev: &ElementValue,
    const_pool: &[ConstantInfo],
) -> Option<AnnotationValue> {
    match ev {
        ElementValue::ConstValueIndex(cv) => {
            let idx = cv.value;
            match cv.tag {
                'B' | 'C' | 'I' | 'S' | 'Z' => {
                    if let Some(ConstantInfo::Integer(c)) =
                        const_pool.get((idx as usize).checked_sub(1)?)
                    {
                        if cv.tag == 'Z' {
                            Some(AnnotationValue::BooleanLiteral(c.value != 0))
                        } else if cv.tag == 'C' {
                            Some(AnnotationValue::CharLiteral(c.value as u8 as char))
                        } else {
                            Some(AnnotationValue::IntLiteral(c.value))
                        }
                    } else {
                        None
                    }
                }
                'J' => {
                    if let Some(ConstantInfo::Long(c)) =
                        const_pool.get((idx as usize).checked_sub(1)?)
                    {
                        Some(AnnotationValue::LongLiteral(c.value))
                    } else {
                        None
                    }
                }
                'F' => {
                    if let Some(ConstantInfo::Float(c)) =
                        const_pool.get((idx as usize).checked_sub(1)?)
                    {
                        Some(AnnotationValue::FloatLiteral(c.value))
                    } else {
                        None
                    }
                }
                'D' => {
                    if let Some(ConstantInfo::Double(c)) =
                        const_pool.get((idx as usize).checked_sub(1)?)
                    {
                        Some(AnnotationValue::DoubleLiteral(c.value))
                    } else {
                        None
                    }
                }
                's' => {
                    let s = util::get_utf8(const_pool, idx)?.to_string();
                    Some(AnnotationValue::StringLiteral(s))
                }
                _ => None,
            }
        }
        ElementValue::EnumConst(ec) => {
            let type_name = util::get_utf8(const_pool, ec.type_name_index)?;
            let const_name = util::get_utf8(const_pool, ec.const_name_index)?.to_string();
            let type_simple = if type_name.starts_with('L') && type_name.ends_with(';') {
                descriptor::simple_class_name(&type_name[1..type_name.len() - 1]).to_string()
            } else {
                type_name.to_string()
            };
            Some(AnnotationValue::EnumConstant {
                type_name: type_simple,
                const_name,
            })
        }
        ElementValue::ClassInfoIndex(idx) => {
            let desc = util::get_utf8(const_pool, *idx)?.to_string();
            Some(AnnotationValue::ClassLiteral(desc))
        }
        ElementValue::AnnotationValue(ann) => {
            let a = convert_annotation(ann, const_pool)?;
            Some(AnnotationValue::AnnotationLiteral(a))
        }
        ElementValue::ElementArray(arr) => {
            let values: Vec<AnnotationValue> = arr
                .values
                .iter()
                .filter_map(|v| convert_element_value(v, const_pool))
                .collect();
            Some(AnnotationValue::ArrayLiteral(values))
        }
    }
}

fn extract_class_type_parameters(_class: &ClassFile) -> Vec<TypeParameter> {
    // TODO: Parse Signature attribute for class-level generic type parameters
    // The signature format is: <T:Ljava/lang/Object;>Ljava/lang/Object;
    Vec::new()
}
