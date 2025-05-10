use nom::{
    bytes::complete::take,
    combinator::{map, success},
    error::{Error, ErrorKind},
    multi::count,
    number::complete::{be_u16, be_u32, be_u8},
    Err as BaseErr,
};

use crate::attribute_info::types::StackMapFrame::*;
use crate::attribute_info::*;

// Using a type alias here evades a Clippy warning about complex types.
type Err<E> = BaseErr<Error<E>>;

pub fn attribute_parser(input: &[u8]) -> Result<(&[u8], AttributeInfo), Err<&[u8]>> {
    let (input, attribute_name_index) = be_u16(input)?;
    let (input, attribute_length) = be_u32(input)?;
    let (input, info) = take(attribute_length)(input)?;
    Ok((
        input,
        AttributeInfo {
            attribute_name_index,
            attribute_length,
            info: info.to_owned(),
        },
    ))
}

pub fn exception_entry_parser(input: &[u8]) -> Result<(&[u8], ExceptionEntry), Err<&[u8]>> {
    let (input, start_pc) = be_u16(input)?;
    let (input, end_pc) = be_u16(input)?;
    let (input, handler_pc) = be_u16(input)?;
    let (input, catch_type) = be_u16(input)?;
    Ok((
        input,
        ExceptionEntry {
            start_pc,
            end_pc,
            handler_pc,
            catch_type,
        },
    ))
}

pub fn code_attribute_parser(input: &[u8]) -> Result<(&[u8], CodeAttribute), Err<&[u8]>> {
    let (input, max_stack) = be_u16(input)?;
    let (input, max_locals) = be_u16(input)?;
    let (input, code_length) = be_u32(input)?;
    let (input, code) = take(code_length)(input)?;
    let (input, exception_table_length) = be_u16(input)?;
    let (input, exception_table) =
        count(exception_entry_parser, exception_table_length as usize)(input)?;
    let (input, attributes_count) = be_u16(input)?;
    let (input, attributes) = count(attribute_parser, attributes_count as usize)(input)?;
    Ok((
        input,
        CodeAttribute {
            max_stack,
            max_locals,
            code_length,
            code: code.to_owned(),
            exception_table_length,
            exception_table,
            attributes_count,
            attributes,
        },
    ))
}

pub fn method_parameters_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], MethodParametersAttribute), Err<&[u8]>> {
    let (input, parameters_count) = be_u8(input)?;
    let (input, parameters) = count(parameters_parser, parameters_count as usize)(input)?;
    Ok((
        input,
        MethodParametersAttribute {
            parameters_count,
            parameters,
        },
    ))
}

pub fn parameters_parser(input: &[u8]) -> Result<(&[u8], ParameterAttribute), Err<&[u8]>> {
    let (input, name_index) = be_u16(input)?;
    let (input, access_flags) = be_u16(input)?;
    Ok((
        input,
        ParameterAttribute {
            name_index,
            access_flags,
        },
    ))
}

fn same_frame_parser(input: &[u8], frame_type: u8) -> Result<(&[u8], StackMapFrame), Err<&[u8]>> {
    success(SameFrame { frame_type })(input)
}

fn verification_type_parser(input: &[u8]) -> Result<(&[u8], VerificationTypeInfo), Err<&[u8]>> {
    use self::VerificationTypeInfo::*;
    let v = input[0];
    let new_input = &input[1..];
    match v {
        0 => Ok((new_input, Top)),
        1 => Ok((new_input, Integer)),
        2 => Ok((new_input, Float)),
        3 => Ok((new_input, Double)),
        4 => Ok((new_input, Long)),
        5 => Ok((new_input, Null)),
        6 => Ok((new_input, UninitializedThis)),
        7 => map(be_u16, |class| Object { class })(new_input),
        8 => map(be_u16, |offset| Uninitialized { offset })(new_input),
        _ => Result::Err(Err::Error(error_position!(input, ErrorKind::NoneOf))),
    }
}

fn same_locals_1_stack_item_frame_parser(
    input: &[u8],
    frame_type: u8,
) -> Result<(&[u8], StackMapFrame), Err<&[u8]>> {
    let (input, stack) = verification_type_parser(input)?;
    Ok((input, SameLocals1StackItemFrame { frame_type, stack }))
}

fn same_locals_1_stack_item_frame_extended_parser(
    input: &[u8],
    frame_type: u8,
) -> Result<(&[u8], StackMapFrame), Err<&[u8]>> {
    let (input, offset_delta) = be_u16(input)?;
    let (input, stack) = verification_type_parser(input)?;
    Ok((
        input,
        SameLocals1StackItemFrameExtended {
            frame_type,
            offset_delta,
            stack,
        },
    ))
}

fn chop_frame_parser(input: &[u8], frame_type: u8) -> Result<(&[u8], StackMapFrame), Err<&[u8]>> {
    let (input, offset_delta) = be_u16(input)?;
    Ok((
        input,
        ChopFrame {
            frame_type,
            offset_delta,
        },
    ))
}

fn same_frame_extended_parser(
    input: &[u8],
    frame_type: u8,
) -> Result<(&[u8], StackMapFrame), Err<&[u8]>> {
    let (input, offset_delta) = be_u16(input)?;
    Ok((
        input,
        SameFrameExtended {
            frame_type,
            offset_delta,
        },
    ))
}

fn append_frame_parser(input: &[u8], frame_type: u8) -> Result<(&[u8], StackMapFrame), Err<&[u8]>> {
    let (input, offset_delta) = be_u16(input)?;
    let (input, locals) = count(verification_type_parser, (frame_type - 251) as usize)(input)?;
    Ok((
        input,
        AppendFrame {
            frame_type,
            offset_delta,
            locals,
        },
    ))
}

fn full_frame_parser(input: &[u8], frame_type: u8) -> Result<(&[u8], StackMapFrame), Err<&[u8]>> {
    let (input, offset_delta) = be_u16(input)?;
    let (input, number_of_locals) = be_u16(input)?;
    let (input, locals) = count(verification_type_parser, number_of_locals as usize)(input)?;
    let (input, number_of_stack_items) = be_u16(input)?;
    let (input, stack) = count(verification_type_parser, number_of_stack_items as usize)(input)?;
    Ok((
        input,
        FullFrame {
            frame_type,
            offset_delta,
            number_of_locals,
            locals,
            number_of_stack_items,
            stack,
        },
    ))
}

fn stack_frame_parser(input: &[u8], frame_type: u8) -> Result<(&[u8], StackMapFrame), Err<&[u8]>> {
    match frame_type {
        0..=63 => same_frame_parser(input, frame_type),
        64..=127 => same_locals_1_stack_item_frame_parser(input, frame_type),
        247 => same_locals_1_stack_item_frame_extended_parser(input, frame_type),
        248..=250 => chop_frame_parser(input, frame_type),
        251 => same_frame_extended_parser(input, frame_type),
        252..=254 => append_frame_parser(input, frame_type),
        255 => full_frame_parser(input, frame_type),
        _ => Result::Err(Err::Error(error_position!(input, ErrorKind::NoneOf))),
    }
}

fn stack_map_frame_entry_parser(input: &[u8]) -> Result<(&[u8], StackMapFrame), Err<&[u8]>> {
    let (input, frame_type) = be_u8(input)?;
    let (input, stack_frame) = stack_frame_parser(input, frame_type)?;
    Ok((input, stack_frame))
}

pub fn stack_map_table_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], StackMapTableAttribute), Err<&[u8]>> {
    let (input, number_of_entries) = be_u16(input)?;
    let (input, entries) = count(stack_map_frame_entry_parser, number_of_entries as usize)(input)?;
    Ok((
        input,
        StackMapTableAttribute {
            number_of_entries,
            entries,
        },
    ))
}

pub fn exceptions_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], ExceptionsAttribute), Err<&[u8]>> {
    let (input, exception_table_length) = be_u16(input)?;
    let (input, exception_table) = count(be_u16, exception_table_length as usize)(input)?;
    Ok((
        input,
        ExceptionsAttribute {
            exception_table_length,
            exception_table,
        },
    ))
}

pub fn constant_value_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], ConstantValueAttribute), Err<&[u8]>> {
    let (input, constant_value_index) = be_u16(input)?;
    Ok((
        input,
        ConstantValueAttribute {
            constant_value_index,
        },
    ))
}

fn bootstrap_method_parser(input: &[u8]) -> Result<(&[u8], BootstrapMethod), Err<&[u8]>> {
    let (input, bootstrap_method_ref) = be_u16(input)?;
    let (input, num_bootstrap_arguments) = be_u16(input)?;
    let (input, bootstrap_arguments) = count(be_u16, num_bootstrap_arguments as usize)(input)?;
    Ok((
        input,
        BootstrapMethod {
            bootstrap_method_ref,
            num_bootstrap_arguments,
            bootstrap_arguments,
        },
    ))
}

pub fn bootstrap_methods_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], BootstrapMethodsAttribute), Err<&[u8]>> {
    let (input, num_bootstrap_methods) = be_u16(input)?;
    let (input, bootstrap_methods) =
        count(bootstrap_method_parser, num_bootstrap_methods as usize)(input)?;
    Ok((
        input,
        BootstrapMethodsAttribute {
            num_bootstrap_methods,
            bootstrap_methods,
        },
    ))
}

pub fn sourcefile_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], SourceFileAttribute), Err<&[u8]>> {
    let (input, sourcefile_index) = be_u16(input)?;
    Ok((input, SourceFileAttribute { sourcefile_index }))
}
