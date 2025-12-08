use nom::{
    Err as BaseErr,
    bytes::complete::take,
    combinator::{map, success},
    error::{Error, ErrorKind},
    multi::count,
    number::complete::{be_u8, be_u16, be_u32},
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

pub fn inner_classes_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], InnerClassesAttribute), Err<&[u8]>> {
    let (input, number_of_classes) = be_u16(input)?;
    let (input, classes) = count(inner_class_info_parser, number_of_classes as usize)(input)?;
    let ret = (
        input,
        InnerClassesAttribute {
            number_of_classes,
            classes,
        },
    );

    Ok(ret)
}

pub fn inner_class_info_parser(input: &[u8]) -> Result<(&[u8], InnerClassInfo), Err<&[u8]>> {
    let (input, inner_class_info_index) = be_u16(input)?;
    let (input, outer_class_info_index) = be_u16(input)?;
    let (input, inner_name_index) = be_u16(input)?;
    let (input, inner_class_access_flags) = be_u16(input)?;
    Ok((
        input,
        InnerClassInfo {
            inner_class_info_index,
            outer_class_info_index,
            inner_name_index,
            inner_class_access_flags,
        },
    ))
}

pub fn enclosing_method_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], EnclosingMethodAttribute), Err<&[u8]>> {
    let (input, class_index) = be_u16(input)?;
    let (input, method_index) = be_u16(input)?;
    Ok((
        input,
        EnclosingMethodAttribute {
            class_index,
            method_index,
        },
    ))
}

pub fn signature_attribute_parser(input: &[u8]) -> Result<(&[u8], SignatureAttribute), Err<&[u8]>> {
    let (input, signature_index) = be_u16(input)?;
    Ok((input, SignatureAttribute { signature_index }))
}

pub fn runtime_visible_annotations_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], RuntimeVisibleAnnotationsAttribute), Err<&[u8]>> {
    let (input, num_annotations) = be_u16(input)?;
    let (input, annotations) = count(annotation_parser, num_annotations as usize)(input)?;
    Ok((
        input,
        RuntimeVisibleAnnotationsAttribute {
            num_annotations,
            annotations,
        },
    ))
}

pub fn runtime_invisible_annotations_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], RuntimeInvisibleAnnotationsAttribute), Err<&[u8]>> {
    let (input, num_annotations) = be_u16(input)?;
    let (input, annotations) = count(annotation_parser, num_annotations as usize)(input)?;
    Ok((
        input,
        RuntimeInvisibleAnnotationsAttribute {
            num_annotations,
            annotations,
        },
    ))
}

pub fn runtime_visible_parameter_annotations_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], RuntimeVisibleParameterAnnotationsAttribute), Err<&[u8]>> {
    let (input, num_parameters) = be_u8(input)?;
    let (input, parameter_annotations) = count(
        runtime_visible_annotations_attribute_parser,
        num_parameters as usize,
    )(input)?;
    Ok((
        input,
        RuntimeVisibleParameterAnnotationsAttribute {
            num_parameters,
            parameter_annotations,
        },
    ))
}

pub fn runtime_invisible_parameter_annotations_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], RuntimeInvisibleParameterAnnotationsAttribute), Err<&[u8]>> {
    let (input, num_parameters) = be_u8(input)?;
    let (input, parameter_annotations) = count(
        runtime_invisible_annotations_attribute_parser,
        num_parameters as usize,
    )(input)?;
    Ok((
        input,
        RuntimeInvisibleParameterAnnotationsAttribute {
            num_parameters,
            parameter_annotations,
        },
    ))
}
pub fn runtime_visible_type_annotations_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], RuntimeVisibleTypeAnnotationsAttribute), Err<&[u8]>> {
    let (input, num_annotations) = be_u16(input)?;
    let (input, type_annotations) = count(type_annotation_parser, num_annotations as usize)(input)?;

    Ok((
        input,
        RuntimeVisibleTypeAnnotationsAttribute {
            num_annotations,
            type_annotations,
        },
    ))
}

pub fn runtime_invisible_type_annotations_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], RuntimeInvisibleTypeAnnotationsAttribute), Err<&[u8]>> {
    let (input, num_annotations) = be_u16(input)?;
    let (input, type_annotations) = count(type_annotation_parser, num_annotations as usize)(input)?;

    Ok((
        input,
        RuntimeInvisibleTypeAnnotationsAttribute {
            num_annotations,
            type_annotations,
        },
    ))
}

pub fn type_annotation_parser(input: &[u8]) -> Result<(&[u8], TypeAnnotation), Err<&[u8]>> {
    let (input, target_type) = be_u8(input)?;
    let mut target_info: TargetInfo = TargetInfo::Empty;
    match target_type {
        0x0 | 0x1 => {
            let (_input, type_parameter_index) = be_u8(input)?;
            target_info = TargetInfo::TypeParameter {
                type_parameter_index,
            };
        }
        0x10 => {
            let (_input, supertype_index) = be_u16(input)?;
            target_info = TargetInfo::SuperType { supertype_index };
        }
        0x11..=0x12 => {
            let (input, type_parameter_index) = be_u8(input)?;
            let (_input, bound_index) = be_u8(input)?;
            target_info = TargetInfo::TypeParameterBound {
                type_parameter_index,
                bound_index,
            }
        }
        0x13..=0x15 => {
            // Empty target_info
        }
        0x16 => {
            let (_input, formal_parameter_index) = be_u8(input)?;
            target_info = TargetInfo::FormalParameter {
                formal_parameter_index,
            };
        }
        0x17 => {
            let (_input, throws_type_index) = be_u16(input)?;
            target_info = TargetInfo::Throws { throws_type_index };
        }
        0x40 | 0x41 => {
            let (input, table_length) = be_u16(input)?;
            let (_input, tables) = count(
                local_variable_table_annotation_parser,
                table_length as usize,
            )(input)?;
            target_info = TargetInfo::LocalVar {
                table_length,
                tables,
            };
        }
        0x42 => {
            let (_input, exception_table_index) = be_u16(input)?;
            target_info = TargetInfo::Catch {
                exception_table_index,
            }
        }
        0x43..=0x46 => {
            let (_input, offset) = be_u16(input)?;
            target_info = TargetInfo::Offset { offset }
        }
        0x47..=0x4B => {
            let (input, offset) = be_u16(input)?;
            let (_input, type_argument_index) = be_u8(input)?;
            target_info = TargetInfo::TypeArgument {
                offset,
                type_argument_index,
            };
        }
        _ => {
            eprintln!(
                "Parsing RuntimeVisibleTypeAnnotationsAttribute with target_type = {}",
                target_type
            );
        }
    }
    let (input, target_path) = target_path_parser(input)?;
    let (input, type_index) = be_u16(input)?;
    let (input, num_element_value_pairs) = be_u16(input)?;
    let (input, element_value_pairs) =
        count(element_value_pair_parser, num_element_value_pairs as usize)(input)?;

    Ok((
        input,
        TypeAnnotation {
            target_type,
            target_info,
            target_path,
            type_index,
            num_element_value_pairs,
            element_value_pairs,
        },
    ))
}

fn target_path_parser(input: &[u8]) -> Result<(&[u8], TypePath), Err<&[u8]>> {
    let (input, path_length) = be_u8(input)?;
    let (input, paths) = count(
        |input| {
            let (input, type_path_kind) = be_u8(input)?;
            let (input, type_argument_index) = be_u8(input)?;
            Ok((
                input,
                TypePathEntry {
                    type_path_kind,
                    type_argument_index,
                },
            ))
        },
        path_length as usize,
    )(input)?;
    Ok((input, TypePath { path_length, paths }))
}

pub fn local_variable_table_annotation_parser(
    input: &[u8],
) -> Result<(&[u8], LocalVarTableAnnotation), Err<&[u8]>> {
    let (input, start_pc) = be_u16(input)?;
    let (input, length) = be_u16(input)?;
    let (input, index) = be_u16(input)?;
    Ok((
        input,
        LocalVarTableAnnotation {
            start_pc,
            length,
            index,
        },
    ))
}
fn annotation_parser(input: &[u8]) -> Result<(&[u8], RuntimeAnnotation), Err<&[u8]>> {
    let (input, type_index) = be_u16(input)?;
    let (input, num_element_value_pairs) = be_u16(input)?;
    eprintln!(
        "Parsing annotation with type index = {}, and {} element value pairs",
        type_index, num_element_value_pairs
    );
    let (input, element_value_pairs) =
        count(element_value_pair_parser, num_element_value_pairs as usize)(input)?;
    Ok((
        input,
        RuntimeAnnotation {
            type_index,
            num_element_value_pairs,
            element_value_pairs,
        },
    ))
}

fn element_value_pair_parser(input: &[u8]) -> Result<(&[u8], ElementValuePair), Err<&[u8]>> {
    let (input, element_name_index) = be_u16(input)?;
    let (input, value) = element_value_parser(input)?;
    Ok((
        input,
        ElementValuePair {
            element_name_index,
            value,
        },
    ))
}

fn array_value_parser(input: &[u8]) -> Result<(&[u8], ElementArrayValue), Err<&[u8]>> {
    let (input, num_values) = be_u16(input)?;
    let (input, values) = count(element_value_parser, num_values as usize)(input)?;
    Ok((input, ElementArrayValue { num_values, values }))
}

pub fn element_value_parser(input: &[u8]) -> Result<(&[u8], ElementValue), Err<&[u8]>> {
    let (input, tag) = be_u8(input)?;
    eprintln!("Element value parsing: tag = {}", tag as char);

    match tag as char {
        'B' | 'C' | 'I' | 'S' | 'Z' | 'D' | 'F' | 'J' | 's' => {
            let (input, const_value_index) = be_u16(input)?;
            eprintln!(
                "Element value parsing: const_value_index = {}",
                const_value_index
            );
            Ok((
                input,
                ElementValue::ConstValueIndex {
                    tag: tag as char,
                    value: const_value_index,
                },
            ))
        }
        'e' => {
            let (input, enum_const_value) = enum_const_value_parser(input)?;
            eprintln!(
                "Element value parsing: enum_const_value = {:?}",
                enum_const_value
            );
            Ok((input, ElementValue::EnumConst(enum_const_value)))
        }
        'c' => {
            let (input, class_info_index) = be_u16(input)?;
            eprintln!(
                "Element value parsing: class_info_index = {}",
                class_info_index
            );
            Ok((input, ElementValue::ClassInfoIndex(class_info_index)))
        }
        '@' => {
            let (input, annotation_value) = annotation_parser(input)?;
            eprintln!(
                "Element value parsing: annotation_value = {:?}",
                annotation_value
            );
            Ok((input, ElementValue::AnnotationValue(annotation_value)))
        }
        '[' => {
            let (input, array_value) = array_value_parser(input)?;
            eprintln!("Element value parsing: array_value = {:?}", array_value);
            Ok((input, ElementValue::ElementArray(array_value)))
        }
        _ => Result::Err(Err::Error(error_position!(input, ErrorKind::NoneOf))),
    }
}

fn enum_const_value_parser(input: &[u8]) -> Result<(&[u8], EnumConstValue), Err<&[u8]>> {
    let (input, type_name_index) = be_u16(input)?;
    let (input, const_name_index) = be_u16(input)?;
    Ok((
        input,
        EnumConstValue {
            type_name_index,
            const_name_index,
        },
    ))
}

// not even really parsing ...
pub fn source_debug_extension_parser(
    input: &[u8],
) -> Result<(&[u8], SourceDebugExtensionAttribute), Err<&[u8]>> {
    let debug_extension = Vec::from(input);
    Ok((input, SourceDebugExtensionAttribute { debug_extension }))
}

pub fn line_number_table_attribute_parser(
    input: &[u8],
) -> Result<(&[u8], LineNumberTable), Err<&[u8]>> {
    let (input, line_number_table_length) = be_u16(input)?;
    let (input, line_number_table) = count(
        line_number_table_entry_parser,
        line_number_table_length as usize,
    )(input)?;
    Ok((
        input,
        LineNumberTable {
            line_number_table_length,
            line_number_table,
        },
    ))
}

pub fn line_number_table_entry_parser(
    input: &[u8],
) -> Result<(&[u8], LineNumberTableEntry), Err<&[u8]>> {
    let (input, start_pc) = be_u16(input)?;
    let (input, line_number) = be_u16(input)?;
    Ok((
        input,
        LineNumberTableEntry {
            start_pc,
            line_number,
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
