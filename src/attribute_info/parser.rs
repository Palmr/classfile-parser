use nom::{be_u16, be_u32, be_u8, Err, ErrorKind};

use attribute_info::types::StackMapFrame::*;
use attribute_info::*;

pub fn attribute_parser(input: &[u8]) -> Result<(&[u8], AttributeInfo), Err<&[u8]>> {
    do_parse!(
        input,
        attribute_name_index: be_u16
            >> attribute_length: be_u32
            >> info: take!(attribute_length)
            >> (AttributeInfo {
                attribute_name_index,
                attribute_length,
                info: info.to_owned(),
            })
    )
}

pub fn exception_entry_parser(input: &[u8]) -> Result<(&[u8], ExceptionEntry), Err<&[u8]>> {
    do_parse!(
        input,
        start_pc: be_u16
            >> end_pc: be_u16
            >> handler_pc: be_u16
            >> catch_type: be_u16
            >> (ExceptionEntry {
                start_pc,
                end_pc,
                handler_pc,
                catch_type,
            })
    )
}

pub fn code_attribute_parser(input: &[u8]) -> Result<(&[u8], CodeAttribute), Err<&[u8]>> {
    do_parse!(
        input,
        max_stack: be_u16
            >> max_locals: be_u16
            >> code_length: be_u32
            >> code: take!(code_length)
            >> exception_table_length: be_u16
            >> exception_table: count!(exception_entry_parser, exception_table_length as usize)
            >> attributes_count: be_u16
            >> attributes: count!(attribute_parser, attributes_count as usize)
            >> (CodeAttribute {
                max_stack,
                max_locals,
                code_length,
                code: code.to_owned(),
                exception_table_length,
                exception_table,
                attributes_count,
                attributes,
            })
    )
}

fn same_frame_parser(input: &[u8], frame_type: u8) -> Result<(&[u8], StackMapFrame), Err<&[u8]>> {
    value!(input, SameFrame { frame_type })
}

fn verification_type(v: u8) -> Option<VerificationTypeInfo> {
    use self::VerificationTypeInfo::*;
    match v {
        0 => Some(Top),
        1 => Some(Integer),
        2 => Some(Float),
        3 => Some(Double),
        4 => Some(Long),
        5 => Some(Null),
        6 => Some(UninitializedThis),
        7 => Some(Object),
        8 => Some(Uninitialized),
        _ => None,
    }
}

fn verification_type_parser(input: &[u8]) -> Result<(&[u8], VerificationTypeInfo), Err<&[u8]>> {
    match verification_type(input[0]) {
        Some(x) => Result::Ok((&input[1..], x)),
        _ => Result::Err(Err::Error(error_position!(input, ErrorKind::Custom(1)))),
    }
}

fn same_locals_1_stack_item_frame_parser(
    input: &[u8],
    frame_type: u8,
) -> Result<(&[u8], StackMapFrame), Err<&[u8]>> {
    do_parse!(
        input,
        stack: verification_type_parser >> (SameLocals1StackItemFrame { frame_type, stack })
    )
}

fn same_locals_1_stack_item_frame_extended_parser(
    input: &[u8],
    frame_type: u8,
) -> Result<(&[u8], StackMapFrame), Err<&[u8]>> {
    do_parse!(
        input,
        offset_delta: be_u16
            >> stack: verification_type_parser
            >> (SameLocals1StackItemFrameExtended {
                frame_type,
                offset_delta,
                stack
            })
    )
}

fn chop_frame_parser(input: &[u8], frame_type: u8) -> Result<(&[u8], StackMapFrame), Err<&[u8]>> {
    do_parse!(
        input,
        offset_delta: be_u16
            >> (ChopFrame {
                frame_type,
                offset_delta
            })
    )
}

fn same_frame_extended_parser(
    input: &[u8],
    frame_type: u8,
) -> Result<(&[u8], StackMapFrame), Err<&[u8]>> {
    do_parse!(
        input,
        offset_delta: be_u16
            >> (SameFrameExtended {
                frame_type,
                offset_delta
            })
    )
}

fn append_frame_parser(input: &[u8], frame_type: u8) -> Result<(&[u8], StackMapFrame), Err<&[u8]>> {
    do_parse!(
        input,
        offset_delta: be_u16
            >> locals: count!(verification_type_parser, (frame_type - 251) as usize)
            >> (AppendFrame {
                frame_type,
                offset_delta,
                locals
            })
    )
}

fn full_frame_parser(input: &[u8], frame_type: u8) -> Result<(&[u8], StackMapFrame), Err<&[u8]>> {
    do_parse!(
        input,
        offset_delta: be_u16
            >> number_of_locals: be_u16
            >> locals: count!(verification_type_parser, number_of_locals as usize)
            >> number_of_stack_items: be_u16
            >> stack: count!(verification_type_parser, number_of_stack_items as usize)
            >> (FullFrame {
                frame_type,
                offset_delta,
                number_of_locals,
                locals,
                number_of_stack_items,
                stack,
            })
    )
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
        _ => Result::Err(Err::Error(error_position!(input, ErrorKind::Custom(2)))),
    }
}

fn stack_map_frame_entry_parser(input: &[u8]) -> Result<(&[u8], StackMapFrame), Err<&[u8]>> {
    do_parse!(
        input,
        frame_type: be_u8 >> stack_frame: apply!(stack_frame_parser, frame_type) >> (stack_frame)
    )
}

pub fn stack_map_table_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], StackMapTableAttribute), Err<&[u8]>> {
    do_parse!(
        input,
        number_of_entries: be_u16
            >> entries: count!(stack_map_frame_entry_parser, number_of_entries as usize)
            >> (StackMapTableAttribute {
                number_of_entries,
                entries,
            })
    )
}

pub fn exceptions_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], ExceptionsAttribute), Err<&[u8]>> {
    do_parse!(
        input,
        exception_table_length: be_u16
            >> exception_table: count!(be_u16, exception_table_length as usize)
            >> (ExceptionsAttribute {
                exception_table_length,
                exception_table,
            })
    )
}

pub fn constant_value_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], ConstantValueAttribute), Err<&[u8]>> {
    do_parse!(
        input,
        constant_value_index: be_u16
            >> (ConstantValueAttribute {
                constant_value_index,
            })
    )
}

fn bootstrap_method_parser(input: &[u8]) -> Result<(&[u8], BootstrapMethod), Err<&[u8]>> {
    do_parse!(
        input,
        bootstrap_method_ref: be_u16
            >> num_bootstrap_arguments: be_u16
            >> bootstrap_arguments: count!(be_u16, num_bootstrap_arguments as usize)
            >> (BootstrapMethod {
                bootstrap_method_ref,
                num_bootstrap_arguments,
                bootstrap_arguments,
            })
    )
}

pub fn bootstrap_methods_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], BootstrapMethodsAttribute), Err<&[u8]>> {
    do_parse!(
        input,
        num_bootstrap_methods: be_u16
            >> bootstrap_methods: count!(bootstrap_method_parser, num_bootstrap_methods as usize)
            >> (BootstrapMethodsAttribute {
                num_bootstrap_methods,
                bootstrap_methods,
            })
    )
}

pub fn sourcefile_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], SourceFileAttribute), Err<&[u8]>> {
    do_parse!(
        input,
        attribute_name_index: be_u16
            >> attribute_length: be_u32
            >> sourcefile_index: be_u16
            >> (SourceFileAttribute {
                attribute_name_index,
                attribute_length,
                sourcefile_index
            })
    )
}
