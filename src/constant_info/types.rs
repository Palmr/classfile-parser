
pub enum ConstantInfo {
    Utf8(Utf8Constant),
    Integer(IntegerConstant),
    Float(FloatConstant),
    Long(LongConstant),
    Double(DoubleConstant),
    Class(ClassConstant),
    String(StringConstant),
    FieldRef(FieldRefConstant),
    MethodRef(MethodRefConstant),
    InterfaceMethodRef(InterfaceMethodRefConstant),
    NameAndType(NameAndTypeConstant),
//    MethodHandle(MethodHandleConstant),
//    MethodType(MethodTypeConstant),
//    InvokeDynamic(InvokeDynamicConstant),
    Unusable
}

impl ConstantInfo {
    pub fn to_string(&self) -> String {
        match *self {
            ConstantInfo::Utf8(ref s) => format!("Utf8Constant[utf8_string=\"{}\"]", s.utf8_string),
            ConstantInfo::Integer(ref s) => format!("IntegerConstant[value=\"{}\"]", s.value),
            ConstantInfo::Float(ref s) => format!("FloatConstant[value=\"{}\"]", s.value),
            ConstantInfo::Long(ref s) => format!("LongConstant[value=\"{}\"]", s.value),
            ConstantInfo::Double(ref s) => format!("DoubleConstant[value=\"{}\"]", s.value),
            ConstantInfo::Class(ref s) => format!("ClassConstant[name_index={}]", s.name_index),
            ConstantInfo::String(ref s) => format!("StringConstant[string_index={}]", s.string_index),
            ConstantInfo::FieldRef(ref s) => format!("FieldRefConstant[class_index={}, name_and_type_index={}]", s.class_index, s.name_and_type_index),
            ConstantInfo::MethodRef(ref s) => format!("MethodRefConstant[class_index={}, name_and_type_index={}]", s.class_index, s.name_and_type_index),
            ConstantInfo::InterfaceMethodRef(ref s) => format!("InterfaceMethodRefConstant[class_index={}, name_and_type_index={}]", s.class_index, s.name_and_type_index),
            ConstantInfo::NameAndType(ref s) => format!("NameAndTypeConstant[name_index={}, descriptor_index={}]", s.name_index, s.descriptor_index),
            ConstantInfo::Unusable => format!("Unusable[]"),
        }
    }
}

pub struct Utf8Constant {
    pub utf8_string: String,
}

pub struct IntegerConstant {
    pub value: i32,
}
pub struct FloatConstant {
    pub value: f32,
}

pub struct LongConstant {
    pub value: i64,
}
pub struct DoubleConstant {
    pub value: f64,
}

pub struct ClassConstant {
    pub name_index: u16,
}

pub struct StringConstant {
    pub string_index: u16,
}

pub struct FieldRefConstant {
    pub class_index: u16,
    pub name_and_type_index: u16,
}

pub struct MethodRefConstant {
    pub class_index: u16,
    pub name_and_type_index: u16,
}

pub struct InterfaceMethodRefConstant {
    pub class_index: u16,
    pub name_and_type_index: u16,
}

pub struct NameAndTypeConstant {
    pub name_index: u16,
    pub descriptor_index: u16,
}
