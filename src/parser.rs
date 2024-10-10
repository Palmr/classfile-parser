use nom::*;

use crate::attribute_info::attribute_parser;
use crate::constant_info::constant_parser;
use crate::field_info::field_parser;
use crate::method_info::method_parser;
use crate::types::{ClassAccessFlags, ClassFile};
use nom::bytes::complete::tag;
use nom::multi::count;

fn magic_parser(input: &[u8]) -> IResult<&[u8], &[u8]> {
    tag(&[0xCA, 0xFE, 0xBA, 0xBE])(input)
}

/// Parse a byte array into a ClassFile. This will probably be deprecated in 0.4.0 in as it returns
/// a nom IResult type, which exposes the internal parsing library and not a good idea.
///
/// If you want to call it directly, as it is the only way to parse a byte slice directly, you must
/// unwrap the result yourself.
///
/// ```rust
/// let classfile_bytes = include_bytes!("../java-assets/compiled-classes/BasicClass.class");
///
/// match classfile_parser::class_parser(classfile_bytes) {
///     Ok((_, class_file)) => {
///         println!("version {},{}", class_file.major_version, class_file.minor_version);
///     }
///     Err(_) => panic!("Failed to parse"),
/// };
/// ```
pub fn class_parser(input: &[u8]) -> IResult<&[u8], ClassFile> {
    use nom::number::complete::be_u16;
    let (input, _) = magic_parser(input)?;
    let (input, minor_version) = be_u16(input)?;
    let (input, major_version) = be_u16(input)?;
    let (input, const_pool_size) = be_u16(input)?;
    let (input, const_pool) = constant_parser(input, (const_pool_size - 1) as usize)?;
    let (input, access_flags) = be_u16(input)?;
    let (input, this_class) = be_u16(input)?;
    let (input, super_class) = be_u16(input)?;
    let (input, interfaces_count) = be_u16(input)?;
    let (input, interfaces) = count(be_u16, interfaces_count as usize)(input)?;
    let (input, fields_count) = be_u16(input)?;
    let (input, fields) = count(field_parser, fields_count as usize)(input)?;
    let (input, methods_count) = be_u16(input)?;
    let (input, methods) = count(method_parser, methods_count as usize)(input)?;
    let (input, attributes_count) = be_u16(input)?;
    let (input, attributes) = count(attribute_parser, attributes_count as usize)(input)?;
    Ok((
        input,
        ClassFile {
            minor_version,
            major_version,
            const_pool_size,
            const_pool,
            access_flags: ClassAccessFlags::from_bits_truncate(access_flags),
            this_class,
            super_class,
            interfaces_count,
            interfaces,
            fields_count,
            fields,
            methods_count,
            methods,
            attributes_count,
            attributes,
        },
    ))
}
