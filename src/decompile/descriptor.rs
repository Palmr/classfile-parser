/// JVM type descriptor and method descriptor parser.

/// Represents a JVM type from a descriptor string.
#[derive(Clone, Debug, PartialEq)]
pub enum JvmType {
    Int,
    Long,
    Float,
    Double,
    Byte,
    Char,
    Short,
    Boolean,
    Void,
    Reference(String),
    Array(Box<JvmType>),
    Null,
    Unknown,
}

impl JvmType {
    /// Returns true if this type occupies two slots on the JVM stack.
    pub fn is_wide(&self) -> bool {
        matches!(self, JvmType::Long | JvmType::Double)
    }

    /// Returns the JVM descriptor string for this type.
    pub fn to_descriptor(&self) -> String {
        match self {
            JvmType::Int => "I".into(),
            JvmType::Long => "J".into(),
            JvmType::Float => "F".into(),
            JvmType::Double => "D".into(),
            JvmType::Byte => "B".into(),
            JvmType::Char => "C".into(),
            JvmType::Short => "S".into(),
            JvmType::Boolean => "Z".into(),
            JvmType::Void => "V".into(),
            JvmType::Reference(name) => format!("L{};", name),
            JvmType::Array(inner) => format!("[{}", inner.to_descriptor()),
            JvmType::Null | JvmType::Unknown => "Ljava/lang/Object;".into(),
        }
    }

    /// Returns the simple (unqualified) name for display.
    pub fn simple_name(&self) -> String {
        match self {
            JvmType::Int => "int".into(),
            JvmType::Long => "long".into(),
            JvmType::Float => "float".into(),
            JvmType::Double => "double".into(),
            JvmType::Byte => "byte".into(),
            JvmType::Char => "char".into(),
            JvmType::Short => "short".into(),
            JvmType::Boolean => "boolean".into(),
            JvmType::Void => "void".into(),
            JvmType::Reference(name) => internal_to_source_name(name),
            JvmType::Array(inner) => format!("{}[]", inner.simple_name()),
            JvmType::Null => "null".into(),
            JvmType::Unknown => "/* unknown */".into(),
        }
    }
}

/// Parse a single type descriptor starting at position `pos` in `desc`.
/// Returns (JvmType, next_position).
pub fn parse_type_at(desc: &str, pos: usize) -> Option<(JvmType, usize)> {
    let bytes = desc.as_bytes();
    if pos >= bytes.len() {
        return None;
    }
    match bytes[pos] {
        b'B' => Some((JvmType::Byte, pos + 1)),
        b'C' => Some((JvmType::Char, pos + 1)),
        b'D' => Some((JvmType::Double, pos + 1)),
        b'F' => Some((JvmType::Float, pos + 1)),
        b'I' => Some((JvmType::Int, pos + 1)),
        b'J' => Some((JvmType::Long, pos + 1)),
        b'S' => Some((JvmType::Short, pos + 1)),
        b'Z' => Some((JvmType::Boolean, pos + 1)),
        b'V' => Some((JvmType::Void, pos + 1)),
        b'L' => {
            let semi = desc[pos + 1..].find(';')?;
            let class_name = &desc[pos + 1..pos + 1 + semi];
            Some((
                JvmType::Reference(class_name.to_string()),
                pos + 1 + semi + 1,
            ))
        }
        b'[' => {
            let (inner, next) = parse_type_at(desc, pos + 1)?;
            Some((JvmType::Array(Box::new(inner)), next))
        }
        _ => None,
    }
}

/// Parse a full type descriptor string.
pub fn parse_type_descriptor(desc: &str) -> Option<JvmType> {
    let (ty, _) = parse_type_at(desc, 0)?;
    Some(ty)
}

/// Parse a method descriptor, e.g. "(II)V" -> ([Int, Int], Void)
pub fn parse_method_descriptor(desc: &str) -> Option<(Vec<JvmType>, JvmType)> {
    if !desc.starts_with('(') {
        return None;
    }
    let close = desc.find(')')?;
    let mut params = Vec::new();
    let mut pos = 1;
    while pos < close {
        let (ty, next) = parse_type_at(desc, pos)?;
        params.push(ty);
        pos = next;
    }
    let (ret, _) = parse_type_at(desc, close + 1)?;
    Some((params, ret))
}

/// Convert internal class name to source name.
pub fn internal_to_source_name(name: &str) -> String {
    name.replace('/', ".")
}

/// Get just the simple class name from an internal name.
pub fn simple_class_name(name: &str) -> &str {
    match name.rfind('/') {
        Some(pos) => &name[pos + 1..],
        None => name,
    }
}

/// Get the package from an internal name.
pub fn package_name(name: &str) -> Option<&str> {
    match name.rfind('/') {
        Some(pos) => Some(&name[..pos]),
        None => None,
    }
}

/// Convert a newarray type code to JvmType.
pub fn newarray_type(atype: u8) -> JvmType {
    match atype {
        4 => JvmType::Boolean,
        5 => JvmType::Char,
        6 => JvmType::Float,
        7 => JvmType::Double,
        8 => JvmType::Byte,
        9 => JvmType::Short,
        10 => JvmType::Int,
        11 => JvmType::Long,
        _ => JvmType::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_primitives() {
        assert_eq!(parse_type_descriptor("I"), Some(JvmType::Int));
        assert_eq!(parse_type_descriptor("J"), Some(JvmType::Long));
        assert_eq!(parse_type_descriptor("D"), Some(JvmType::Double));
        assert_eq!(parse_type_descriptor("V"), Some(JvmType::Void));
        assert_eq!(parse_type_descriptor("Z"), Some(JvmType::Boolean));
    }

    #[test]
    fn test_parse_reference() {
        assert_eq!(
            parse_type_descriptor("Ljava/lang/String;"),
            Some(JvmType::Reference("java/lang/String".into()))
        );
    }

    #[test]
    fn test_parse_array() {
        assert_eq!(
            parse_type_descriptor("[I"),
            Some(JvmType::Array(Box::new(JvmType::Int)))
        );
        assert_eq!(
            parse_type_descriptor("[[Ljava/lang/Object;"),
            Some(JvmType::Array(Box::new(JvmType::Array(Box::new(
                JvmType::Reference("java/lang/Object".into())
            )))))
        );
    }

    #[test]
    fn test_parse_method_descriptor() {
        let (params, ret) = parse_method_descriptor("(II)V").unwrap();
        assert_eq!(params, vec![JvmType::Int, JvmType::Int]);
        assert_eq!(ret, JvmType::Void);

        let (params, ret) = parse_method_descriptor("(Ljava/lang/String;I)[B").unwrap();
        assert_eq!(
            params,
            vec![JvmType::Reference("java/lang/String".into()), JvmType::Int]
        );
        assert_eq!(ret, JvmType::Array(Box::new(JvmType::Byte)));

        let (params, ret) = parse_method_descriptor("()V").unwrap();
        assert_eq!(params, vec![]);
        assert_eq!(ret, JvmType::Void);
    }

    #[test]
    fn test_internal_to_source() {
        assert_eq!(
            internal_to_source_name("java/lang/String"),
            "java.lang.String"
        );
        assert_eq!(simple_class_name("java/lang/String"), "String");
        assert_eq!(package_name("java/lang/String"), Some("java/lang"));
        assert_eq!(package_name("NoPackage"), None);
    }
}
