use nom::{IResult, multi::count, number::complete::be_u16};

use crate::attribute_info::attribute_parser;

use crate::field_info::{FieldAccessFlags, FieldInfo};

pub fn field_parser(input: &[u8]) -> IResult<&[u8], FieldInfo> {
    let (input, access_flags) = be_u16(input)?;
    let (input, name_index) = be_u16(input)?;
    let (input, descriptor_index) = be_u16(input)?;
    let (input, attributes_count) = be_u16(input)?;
    let (input, attributes) = count(attribute_parser, attributes_count as usize)(input)?;
    Ok((
        input,
        FieldInfo {
            access_flags: FieldAccessFlags::from_bits_truncate(access_flags),
            name_index,
            descriptor_index,
            attributes_count,
            attributes,
        },
    ))
}
