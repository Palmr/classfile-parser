use nom::{
  be_u16, be_u32,
  IResult,
};

use attribute_info::{   AttributeInfo,
                        CodeAttribute,
                        ExceptionEntry,
                        ExceptionsAttribute,
                        ConstantValueAttribute
                        };

pub fn attribute_parser(input: &[u8]) -> IResult<&[u8], AttributeInfo> {
    chain!(input,
        attribute_name_index: be_u16 ~
        attribute_length: be_u32 ~
        info: take!(attribute_length),
        || {
            AttributeInfo {
                attribute_name_index: attribute_name_index,
                attribute_length: attribute_length,
                info: info.to_owned(),
            }
        }
    )
}

pub fn exception_entry_parser(input: &[u8]) -> IResult<&[u8], ExceptionEntry> {
    chain!(input,
        start_pc: be_u16 ~
        end_pc: be_u16 ~
        handler_pc: be_u16 ~
        catch_type: be_u16,
        || {
            ExceptionEntry {
                start_pc: start_pc,
                end_pc: end_pc,
                handler_pc: handler_pc,
                catch_type: catch_type,
            }
        }
    )
}

pub fn code_attribute_parser(input: &[u8]) -> IResult<&[u8], CodeAttribute> {
    chain!(input,
        max_stack: be_u16 ~
        max_locals: be_u16 ~
        code_length: be_u32 ~
        code: take!(code_length) ~
        exception_table_length: be_u16 ~
        exception_table: count!(exception_entry_parser, exception_table_length as usize) ~
        attributes_count: be_u16 ~
        attributes: count!(attribute_parser, attributes_count as usize),
        || {
            CodeAttribute {
                max_stack: max_stack,
                max_locals: max_locals,
                code_length: code_length,
                code: code.to_owned(),
                exception_table_length: exception_table_length,
                exception_table: exception_table,
                attributes_count: attributes_count,
                attributes: attributes,
            }
        }
    )
}

pub fn exceptions_attribute_parser(input: &[u8]) -> IResult<&[u8], ExceptionsAttribute> {
    chain!(input,
        exception_table_length: be_u16 ~
        exception_table: count!(be_u16, exception_table_length as usize),
        || {
            ExceptionsAttribute {
                exception_table_length: exception_table_length,
                exception_table: exception_table,
            }
        }
    )
}

pub fn constant_value_attribute_parser(input: &[u8]) -> IResult<&[u8], ConstantValueAttribute> {
    chain!(input,
        constant_value_index: be_u16,
        || {
            ConstantValueAttribute {
                constant_value_index: constant_value_index,
            }
        }
    )
}
