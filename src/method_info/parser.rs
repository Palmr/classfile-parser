use nom::{IResult, multi::count, number::complete::be_u16};

use crate::attribute_info::attribute_parser;

use crate::method_info::{MethodAccessFlags, MethodInfo};

pub fn method_parser(input: &[u8]) -> IResult<&[u8], MethodInfo> {
    let (input, access_flags) = be_u16(input)?;
    let (input, name_index) = be_u16(input)?;
    let (input, descriptor_index) = be_u16(input)?;
    let (input, attributes_count) = be_u16(input)?;
    let (input, attributes) = count(attribute_parser, attributes_count as usize)(input)?;
    Ok((
        input,
        MethodInfo {
            access_flags: MethodAccessFlags::from_bits_truncate(access_flags),
            name_index,
            descriptor_index,
            attributes_count,
            attributes,
        },
    ))
}
