use super::expr::Expr;
use super::structured_types::StructuredBody;

/// Primitive types in Java.
#[derive(Clone, Debug, PartialEq)]
pub enum PrimitiveType {
    Boolean,
    Byte,
    Char,
    Short,
    Int,
    Long,
    Float,
    Double,
}

/// A Java type as it appears in source code (with generics).
#[derive(Clone, Debug, PartialEq)]
pub enum JavaType {
    Primitive(PrimitiveType),
    ClassType {
        package: Option<String>,
        name: String,
        type_args: Vec<JavaType>,
    },
    ArrayType(Box<JavaType>),
    WildcardType {
        bound: Option<Box<JavaType>>,
        is_upper: bool,
    },
    TypeVariable(String),
    Void,
}

impl JavaType {
    /// Get the simple display name for this type.
    pub fn display_name(&self) -> String {
        match self {
            JavaType::Primitive(p) => match p {
                PrimitiveType::Boolean => "boolean".into(),
                PrimitiveType::Byte => "byte".into(),
                PrimitiveType::Char => "char".into(),
                PrimitiveType::Short => "short".into(),
                PrimitiveType::Int => "int".into(),
                PrimitiveType::Long => "long".into(),
                PrimitiveType::Float => "float".into(),
                PrimitiveType::Double => "double".into(),
            },
            JavaType::ClassType {
                name, type_args, ..
            } => {
                if type_args.is_empty() {
                    name.clone()
                } else {
                    let args: Vec<String> = type_args.iter().map(|a| a.display_name()).collect();
                    format!("{}<{}>", name, args.join(", "))
                }
            }
            JavaType::ArrayType(inner) => format!("{}[]", inner.display_name()),
            JavaType::WildcardType { bound, is_upper } => match bound {
                Some(b) => {
                    if *is_upper {
                        format!("? extends {}", b.display_name())
                    } else {
                        format!("? super {}", b.display_name())
                    }
                }
                None => "?".into(),
            },
            JavaType::TypeVariable(name) => name.clone(),
            JavaType::Void => "void".into(),
        }
    }

    /// Check if this is the java.lang.Object type.
    pub fn is_object(&self) -> bool {
        matches!(self, JavaType::ClassType { name, .. } if name == "Object")
    }
}

/// Visibility level.
#[derive(Clone, Debug, PartialEq)]
pub enum Visibility {
    Public,
    Protected,
    PackagePrivate,
    Private,
}

/// What kind of class-like entity this is.
#[derive(Clone, Debug, PartialEq)]
pub enum ClassKind {
    Class,
    Interface,
    Enum,
    Annotation,
    Record,
}

/// A generic type parameter declaration.
#[derive(Clone, Debug)]
pub struct TypeParameter {
    pub name: String,
    pub bounds: Vec<JavaType>,
}

/// A Java annotation usage.
#[derive(Clone, Debug)]
pub struct JavaAnnotation {
    pub type_name: String,
    pub arguments: Vec<AnnotationArgument>,
}

/// An annotation argument.
#[derive(Clone, Debug)]
pub enum AnnotationArgument {
    /// Named argument: `@Foo(name = value)`
    Named {
        name: String,
        value: AnnotationValue,
    },
    /// Unnamed (single-element): `@Foo(value)`
    Unnamed(AnnotationValue),
}

/// An annotation element value.
#[derive(Clone, Debug)]
pub enum AnnotationValue {
    IntLiteral(i32),
    LongLiteral(i64),
    FloatLiteral(f32),
    DoubleLiteral(f64),
    StringLiteral(String),
    BooleanLiteral(bool),
    CharLiteral(char),
    ClassLiteral(String),
    EnumConstant {
        type_name: String,
        const_name: String,
    },
    AnnotationLiteral(JavaAnnotation),
    ArrayLiteral(Vec<AnnotationValue>),
}

/// A Java class / interface / enum / record / annotation.
#[derive(Clone, Debug)]
pub struct JavaClass {
    pub kind: ClassKind,
    pub visibility: Visibility,
    pub is_final: bool,
    pub is_abstract: bool,
    pub is_sealed: bool,
    pub is_static: bool,
    pub annotations: Vec<JavaAnnotation>,
    pub type_parameters: Vec<TypeParameter>,
    pub package: Option<String>,
    pub name: String,
    pub super_class: Option<JavaType>,
    pub interfaces: Vec<JavaType>,
    pub permitted_subclasses: Vec<JavaType>,
    pub record_components: Vec<RecordComponent>,
    pub fields: Vec<JavaField>,
    pub methods: Vec<JavaMethod>,
    pub inner_classes: Vec<JavaClass>,
    pub source_file: Option<String>,
}

/// A record component declaration.
#[derive(Clone, Debug)]
pub struct RecordComponent {
    pub annotations: Vec<JavaAnnotation>,
    pub component_type: JavaType,
    pub name: String,
}

/// A method parameter declaration.
#[derive(Clone, Debug)]
pub struct JavaParameter {
    pub annotations: Vec<JavaAnnotation>,
    pub param_type: JavaType,
    pub name: String,
    pub is_final: bool,
    pub is_varargs: bool,
}

/// A Java method declaration.
#[derive(Clone, Debug)]
pub struct JavaMethod {
    pub visibility: Visibility,
    pub is_static: bool,
    pub is_final: bool,
    pub is_abstract: bool,
    pub is_synchronized: bool,
    pub is_native: bool,
    pub is_default: bool,
    pub is_synthetic: bool,
    pub is_bridge: bool,
    pub annotations: Vec<JavaAnnotation>,
    pub type_parameters: Vec<TypeParameter>,
    pub return_type: JavaType,
    pub name: String,
    pub parameters: Vec<JavaParameter>,
    pub throws: Vec<JavaType>,
    pub body: Option<StructuredBody>,
    /// If decompilation failed, this holds the error message and bytecode fallback.
    pub error: Option<String>,
}

/// A Java field declaration.
#[derive(Clone, Debug)]
pub struct JavaField {
    pub visibility: Visibility,
    pub is_static: bool,
    pub is_final: bool,
    pub is_volatile: bool,
    pub is_transient: bool,
    pub is_synthetic: bool,
    pub is_enum_constant: bool,
    pub annotations: Vec<JavaAnnotation>,
    pub field_type: JavaType,
    pub name: String,
    pub initializer: Option<Expr>,
}
