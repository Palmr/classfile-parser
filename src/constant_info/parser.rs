use crate::constant_info::*;
use nom::{
    Err,
    bytes::complete::take,
    combinator::map,
    error::{Error, ErrorKind},
    number::complete::{be_f32, be_f64, be_i32, be_i64, be_u8, be_u16},
};

fn utf8_constant(input: &[u8]) -> Utf8Constant {
    let utf8_string =
        cesu8::from_java_cesu8(input).unwrap_or_else(|_| String::from_utf8_lossy(input));
    Utf8Constant {
        utf8_string: utf8_string.to_string().into(),
    }
}

fn const_utf8(input: &[u8]) -> ConstantInfoResult<'_> {
    let (input, length) = be_u16(input)?;
    let (input, constant) = map(take(length), utf8_constant)(input)?;
    Ok((input, ConstantInfo::Utf8(constant)))
}

fn const_integer(input: &[u8]) -> ConstantInfoResult<'_> {
    let (input, value) = be_i32(input)?;
    Ok((input, ConstantInfo::Integer(IntegerConstant { value })))
}

fn const_float(input: &[u8]) -> ConstantInfoResult<'_> {
    let (input, value) = be_f32(input)?;
    Ok((input, ConstantInfo::Float(FloatConstant { value })))
}

fn const_long(input: &[u8]) -> ConstantInfoResult<'_> {
    let (input, value) = be_i64(input)?;
    Ok((input, ConstantInfo::Long(LongConstant { value })))
}

fn const_double(input: &[u8]) -> ConstantInfoResult<'_> {
    let (input, value) = be_f64(input)?;
    Ok((input, ConstantInfo::Double(DoubleConstant { value })))
}

fn const_class(input: &[u8]) -> ConstantInfoResult<'_> {
    let (input, name_index) = be_u16(input)?;
    Ok((input, ConstantInfo::Class(ClassConstant { name_index })))
}

fn const_string(input: &[u8]) -> ConstantInfoResult<'_> {
    let (input, string_index) = be_u16(input)?;
    Ok((input, ConstantInfo::String(StringConstant { string_index })))
}

fn const_field_ref(input: &[u8]) -> ConstantInfoResult<'_> {
    let (input, class_index) = be_u16(input)?;
    let (input, name_and_type_index) = be_u16(input)?;
    Ok((
        input,
        ConstantInfo::FieldRef(FieldRefConstant {
            class_index,
            name_and_type_index,
        }),
    ))
}

fn const_method_ref(input: &[u8]) -> ConstantInfoResult<'_> {
    let (input, class_index) = be_u16(input)?;
    let (input, name_and_type_index) = be_u16(input)?;
    Ok((
        input,
        ConstantInfo::MethodRef(MethodRefConstant {
            class_index,
            name_and_type_index,
        }),
    ))
}

fn const_interface_method_ref(input: &[u8]) -> ConstantInfoResult<'_> {
    let (input, class_index) = be_u16(input)?;
    let (input, name_and_type_index) = be_u16(input)?;
    Ok((
        input,
        ConstantInfo::InterfaceMethodRef(InterfaceMethodRefConstant {
            class_index,
            name_and_type_index,
        }),
    ))
}

fn const_name_and_type(input: &[u8]) -> ConstantInfoResult<'_> {
    let (input, name_index) = be_u16(input)?;
    let (input, descriptor_index) = be_u16(input)?;
    Ok((
        input,
        ConstantInfo::NameAndType(NameAndTypeConstant {
            name_index,
            descriptor_index,
        }),
    ))
}

fn const_method_handle(input: &[u8]) -> ConstantInfoResult<'_> {
    let (input, reference_kind) = be_u8(input)?;
    let (input, reference_index) = be_u16(input)?;
    Ok((
        input,
        ConstantInfo::MethodHandle(MethodHandleConstant {
            reference_kind,
            reference_index,
        }),
    ))
}

fn const_method_type(input: &[u8]) -> ConstantInfoResult<'_> {
    let (input, descriptor_index) = be_u16(input)?;
    Ok((
        input,
        ConstantInfo::MethodType(MethodTypeConstant { descriptor_index }),
    ))
}

fn const_invoke_dynamic(input: &[u8]) -> ConstantInfoResult<'_> {
    let (input, bootstrap_method_attr_index) = be_u16(input)?;
    let (input, name_and_type_index) = be_u16(input)?;
    Ok((
        input,
        ConstantInfo::InvokeDynamic(InvokeDynamicConstant {
            bootstrap_method_attr_index,
            name_and_type_index,
        }),
    ))
}

type ConstantInfoResult<'a> = Result<(&'a [u8], ConstantInfo), Err<Error<&'a [u8]>>>;
type ConstantInfoVecResult<'a> = Result<(&'a [u8], Vec<ConstantInfo>), Err<Error<&'a [u8]>>>;

fn const_block_parser(input: &[u8], const_type: u8) -> ConstantInfoResult<'_> {
    match const_type {
        1 => const_utf8(input),
        3 => const_integer(input),
        4 => const_float(input),
        5 => const_long(input),
        6 => const_double(input),
        7 => const_class(input),
        8 => const_string(input),
        9 => const_field_ref(input),
        10 => const_method_ref(input),
        11 => const_interface_method_ref(input),
        12 => const_name_and_type(input),
        15 => const_method_handle(input),
        16 => const_method_type(input),
        18 => const_invoke_dynamic(input),
        _ => Result::Err(Err::Error(error_position!(input, ErrorKind::Alt))),
    }
}

fn single_constant_parser(input: &[u8]) -> ConstantInfoResult<'_> {
    let (input, const_type) = be_u8(input)?;
    let (input, const_block) = const_block_parser(input, const_type)?;
    Ok((input, const_block))
}

pub fn constant_parser(i: &[u8], const_pool_size: usize) -> ConstantInfoVecResult<'_> {
    let mut index = 0;
    let mut input = i;
    let mut res = Vec::with_capacity(const_pool_size);
    while index < const_pool_size {
        match single_constant_parser(input) {
            Ok((i, o)) => {
                // Long and Double Entries have twice the size
                // see https://docs.oracle.com/javase/specs/jvms/se6/html/ClassFile.doc.html#1348
                let uses_two_entries =
                    matches!(o, ConstantInfo::Long(..) | ConstantInfo::Double(..));

                res.push(o);
                if uses_two_entries {
                    res.push(ConstantInfo::Unusable);
                    index += 1;
                }
                input = i;
                index += 1;
            }
            _ => return Result::Err(Err::Error(error_position!(input, ErrorKind::Alt))),
        }
    }
    Ok((input, res))
}
