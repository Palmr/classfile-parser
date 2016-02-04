use nom::{
  be_u16, be_u32,
  IResult,
};

use attribute_info::AttributeInfo;

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