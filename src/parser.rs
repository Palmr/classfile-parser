use nom::*;

use attribute_info::attribute_parser;
use constant_info::constant_parser;
use field_info::field_parser;
use method_info::method_parser;
use types::{ClassAccessFlags, ClassFile};

named!(magic_parser, tag!(&[0xCA, 0xFE, 0xBA, 0xBE]));

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
    do_parse!(
        input,
        magic_parser
            >> minor_version: be_u16
            >> major_version: be_u16
            >> const_pool_size: be_u16
            >> const_pool: apply!(constant_parser, (const_pool_size - 1) as usize)
            >> access_flags: be_u16
            >> this_class: be_u16
            >> super_class: be_u16
            >> interfaces_count: be_u16
            >> interfaces: count!(be_u16, interfaces_count as usize)
            >> fields_count: be_u16
            >> fields: count!(field_parser, fields_count as usize)
            >> methods_count: be_u16
            >> methods: count!(method_parser, methods_count as usize)
            >> attributes_count: be_u16
            >> attributes: count!(attribute_parser, attributes_count as usize)
            >> (ClassFile {
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
            })
    )
}
