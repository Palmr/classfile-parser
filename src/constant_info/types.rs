#[derive(Clone, Debug)]
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
    MethodHandle(MethodHandleConstant),
    MethodType(MethodTypeConstant),
    InvokeDynamic(InvokeDynamicConstant),
    Unusable,
}

#[derive(Clone, Debug)]
pub struct Utf8Constant {
    pub utf8_string: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct IntegerConstant {
    pub value: i32,
}

#[derive(Clone, Debug)]
pub struct FloatConstant {
    pub value: f32,
}

#[derive(Clone, Debug)]
pub struct LongConstant {
    pub value: i64,
}

#[derive(Clone, Debug)]
pub struct DoubleConstant {
    pub value: f64,
}

#[derive(Clone, Debug)]
pub struct ClassConstant {
    pub name_index: u16,
}

#[derive(Clone, Debug)]
pub struct StringConstant {
    pub string_index: u16,
}

#[derive(Clone, Debug)]
pub struct FieldRefConstant {
    pub class_index: u16,
    pub name_and_type_index: u16,
}

#[derive(Clone, Debug)]
pub struct MethodRefConstant {
    pub class_index: u16,
    pub name_and_type_index: u16,
}

#[derive(Clone, Debug)]
pub struct InterfaceMethodRefConstant {
    pub class_index: u16,
    pub name_and_type_index: u16,
}

#[derive(Clone, Debug)]
pub struct NameAndTypeConstant {
    pub name_index: u16,
    pub descriptor_index: u16,
}

#[derive(Clone, Debug)]
pub struct MethodHandleConstant {
    pub reference_kind: u8,
    pub reference_index: u16,
}

#[derive(Clone, Debug)]
pub struct MethodTypeConstant {
    pub descriptor_index: u16,
}

#[derive(Clone, Debug)]
pub struct InvokeDynamicConstant {
    pub bootstrap_method_attr_index: u16,
    pub name_and_type_index: u16,
}
