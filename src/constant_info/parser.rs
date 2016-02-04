use nom::{  IResult,
            be_u8, be_u16,
            be_i32, be_f32,
            ErrorKind, Err};

use constant_info::{
    ConstantInfo,
    Utf8Constant,
    IntegerConstant,
    FloatConstant,
    ClassConstant,
    StringConstant,
    FieldRefConstant,
    MethodRefConstant,
    InterfaceMethodRefConstant,
    NameAndTypeConstant,
};

named!(const_utf8<&[u8], ConstantInfo>, chain!(
    // tag!([0x01]) ~
    length: be_u16 ~
    utf8_str: take_str!(length),
    || {
        ConstantInfo::Utf8(
            Utf8Constant {
                utf8_string: utf8_str.to_owned(),
            }
        )
    }
));

named!(const_integer<&[u8], ConstantInfo>, chain!(
    // tag!([0x03]) ~
    value: be_i32,
    || {
        ConstantInfo::Integer(
            IntegerConstant {
                value: value,
            }
        )
    }
));

named!(const_float<&[u8], ConstantInfo>, chain!(
    // tag!([0x04]) ~
    value: be_f32,
    || {
        ConstantInfo::Float(
            FloatConstant {
                value: value,
            }
        )
    }
));

named!(const_class<&[u8], ConstantInfo>, chain!(
    // tag: tag!([0x07]) ~
    name_index: be_u16,
    || {
        ConstantInfo::Class(
            ClassConstant {
                name_index: name_index,
            }
        )
    }
));

named!(const_string<&[u8], ConstantInfo>, chain!(
    // tag: tag!([0x08]) ~
    string_index: be_u16,
    || {
        ConstantInfo::String(
            StringConstant {
                string_index: string_index,
            }
        )
    }
));

named!(const_field_ref<&[u8], ConstantInfo>, chain!(
    // tag: tag!([0x09]) ~
    class_index: be_u16 ~
    name_and_type_index: be_u16,
    || {
        ConstantInfo::FieldRef(
            FieldRefConstant {
                class_index: class_index,
                name_and_type_index: name_and_type_index,
            }
        )
    }
));

named!(const_method_ref<&[u8], ConstantInfo>, chain!(
    // tag: tag!([0x0A]) ~
    class_index: be_u16 ~
    name_and_type_index: be_u16,
    || {
        ConstantInfo::MethodRef(
            MethodRefConstant {
                class_index: class_index,
                name_and_type_index: name_and_type_index,
            }
        )
    }
));

named!(const_interface_method_ref<&[u8], ConstantInfo>, chain!(
    // tag: tag!([0x0B]) ~
    class_index: be_u16 ~
    name_and_type_index: be_u16,
    || {
        ConstantInfo::InterfaceMethodRef(
            InterfaceMethodRefConstant {
                class_index: class_index,
                name_and_type_index: name_and_type_index,
            }
        )
    }
));

named!(const_name_and_type<&[u8], ConstantInfo>, chain!(
    // tag: tag!([0x0C]) ~
    name_index: be_u16 ~
    descriptor_index: be_u16,
    || {
        ConstantInfo::NameAndType(
            NameAndTypeConstant {
                name_index: name_index,
                descriptor_index: descriptor_index,
            }
        )
    }
));

fn const_block_parser(input: &[u8], const_type: u8) -> IResult<&[u8], ConstantInfo> {
    match const_type {
        1 => const_utf8(input),
        3 => const_integer(input),
        4 => const_float(input),
        // // 5 => //CONSTANT_Long,
        // // 6 => //CONSTANT_Double,
        7 => const_class(input),
        8 => const_string(input),
        9 => const_field_ref(input),
        10 => const_method_ref(input),
        11 => const_interface_method_ref(input),
        12 => const_name_and_type(input),
        // // 15 => //CONSTANT_MethodHandle,
        // // 16 => //CONSTANT_MethodType,
        // // 18 => //CONSTANT_InvokeDynamic,
        _ => IResult::Error(Err::Position(ErrorKind::Alt, input)),
    }
}

pub fn constant_parser(input: &[u8]) -> IResult<&[u8], ConstantInfo> {
    chain!(input,
        const_type: be_u8 ~
        const_block: apply!(const_block_parser, const_type),
        || {
            const_block
        }
    )
}