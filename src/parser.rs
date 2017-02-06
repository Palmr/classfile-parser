use nom::{
  be_u16,
  IResult,
};

use types::ClassFile;
use constant_info::constant_parser;
use field_info::field_parser;
use method_info::method_parser;
use attribute_info::attribute_parser;

named!(magic_parser, tag!(&[0xCA, 0xFE, 0xBA, 0xBE]));

pub fn class_parser(input: &[u8]) -> IResult<&[u8], ClassFile> {
  chain!(input,
    magic_parser ~
    minor_version: be_u16 ~
    major_version: be_u16 ~
    const_pool_size: be_u16 ~
    const_pool: apply!(constant_parser, (const_pool_size - 1) as usize) ~
    access_flags: be_u16 ~
    this_class: be_u16 ~
    super_class: be_u16 ~
    interfaces_count: be_u16 ~
    interfaces: count!(be_u16, interfaces_count as usize) ~
    fields_count: be_u16 ~
    fields: count!(field_parser, fields_count as usize) ~
    methods_count: be_u16 ~
    methods: count!(method_parser, methods_count as usize) ~
    attributes_count: be_u16 ~
    attributes: count!(attribute_parser, attributes_count as usize),
    || {
        ClassFile {
            minor_version: minor_version,
            major_version: major_version,
            const_pool_size: const_pool_size,
            const_pool: const_pool,
            access_flags: access_flags,
            this_class: this_class,
            super_class: super_class,
            interfaces_count: interfaces_count,
            interfaces: interfaces,
            fields_count: fields_count,
            fields: fields,
            methods_count: methods_count,
            methods: methods,
            attributes_count: attributes_count,
            attributes: attributes,
        }
    }
  )
}
