use binrw::{BinResult, binrw};
use std::fmt::Debug;

#[derive(Clone, Debug)]
#[binrw]
pub enum ConstantInfo {
    #[brw(magic(1u8))]
    Utf8(Utf8Constant),
    #[brw(magic(3u8))]
    Integer(IntegerConstant),
    #[brw(magic(4u8))]
    Float(FloatConstant),
    #[brw(magic(5u8))]
    Long(LongConstant),
    #[brw(magic(6u8))]
    Double(DoubleConstant),
    #[brw(magic(7u8))]
    Class(ClassConstant),
    #[brw(magic(8u8))]
    String(StringConstant),
    #[brw(magic(9u8))]
    FieldRef(FieldRefConstant),
    #[brw(magic(10u8))]
    MethodRef(MethodRefConstant),
    #[brw(magic(11u8))]
    InterfaceMethodRef(InterfaceMethodRefConstant),
    #[brw(magic(12u8))]
    NameAndType(NameAndTypeConstant),
    #[brw(magic(15u8))]
    MethodHandle(MethodHandleConstant),
    #[brw(magic(16u8))]
    MethodType(MethodTypeConstant),
    #[brw(magic(18u8))]
    InvokeDynamic(InvokeDynamicConstant),
    #[brw(magic(19u8))]
    Module(ModuleConstant),
    #[brw(magic(20u8))]
    Package(PackageConstant),
    Unusable,
}

#[binrw::parser(reader)]
pub fn string_reader() -> BinResult<String> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf)?;
    let len = u16::from_be_bytes(buf);
    let mut string_bytes = vec![0; len as usize];
    let _ = reader.read_exact(&mut string_bytes);
    let utf8_string = cesu8::from_java_cesu8(&string_bytes)
        .unwrap_or_else(|_| String::from_utf8_lossy(&string_bytes));
    Ok(utf8_string.to_string())
}

#[binrw::writer(writer)]
pub fn string_writer<'a>(s: &'a String) -> BinResult<()> {
    let cesu8_bytes = cesu8::to_java_cesu8(s);
    writer.write_all(&u16::to_be_bytes(cesu8_bytes.len() as u16))?;
    writer.write_all(&cesu8_bytes)?;
    Ok(())
}

#[derive(Clone, Debug)]
#[binrw]
pub struct Utf8Constant {
    #[br(parse_with = crate::constant_info::string_reader)]
    #[bw(write_with = crate::constant_info::string_writer)]
    pub utf8_string: String,
    // pub bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct IntegerConstant {
    pub value: i32,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct FloatConstant {
    pub value: f32,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct LongConstant {
    pub value: i64,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct DoubleConstant {
    pub value: f64,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct ClassConstant {
    pub name_index: u16,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct StringConstant {
    pub string_index: u16,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct FieldRefConstant {
    pub class_index: u16,
    pub name_and_type_index: u16,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct MethodRefConstant {
    pub class_index: u16,
    pub name_and_type_index: u16,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct InterfaceMethodRefConstant {
    pub class_index: u16,
    pub name_and_type_index: u16,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct NameAndTypeConstant {
    pub name_index: u16,
    pub descriptor_index: u16,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct MethodHandleConstant {
    pub reference_kind: u8,
    pub reference_index: u16,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct MethodTypeConstant {
    pub descriptor_index: u16,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct InvokeDynamicConstant {
    pub bootstrap_method_attr_index: u16,
    pub name_and_type_index: u16,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct ModuleConstant {
    pub name_index: u16,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct PackageConstant {
    pub name_index: u16,
}
