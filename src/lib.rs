use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

#[macro_use]
extern crate nom;
#[macro_use]
extern crate bitflags;

use nom::IResult;

pub mod constant_info;
pub mod attribute_info;
pub mod method_info;
pub mod field_info;

pub mod types;
pub mod parser;

pub use parser::class_parser;
pub use types::*;

pub fn parse_class(class_name: &str) -> Result<ClassFile, String> {
    let class_file_name = &format!("{}.class", class_name);
    let path = Path::new(class_file_name);
    let display = path.display();

    let mut file = match File::open(&path) {
        Err(why) => return Err(format!("Unable to open {}: {}", display, Error::description(&why))),
        Ok(file) => file,
    };

    let mut class_bytes = Vec::new();
    match file.read_to_end(&mut class_bytes) {
        Err(why) => return Err(format!("Unable to read {}: {}", display, Error::description(&why))),
        Ok(_) => {},
    };

    let parsed_class = class_parser(&class_bytes);
    match parsed_class {
        IResult::Done(_, c) => Ok(c),
        _ => Err("Failed to parse class?".to_string()),
    }
}
