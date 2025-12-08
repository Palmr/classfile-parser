//! A parser for [Java Classfiles](https://docs.oracle.com/javase/specs/jvms/se10/html/jvms-4.html)

use std::fs::File;
use std::io::{BufReader, prelude::*};
use std::path::Path;

#[macro_use]
extern crate nom;

#[macro_use]
extern crate bitflags;

pub mod attribute_info;
pub mod constant_info;
pub mod field_info;
pub mod method_info;

pub mod code_attribute;

pub mod parser;
pub mod types;

pub use parser::class_parser;
pub use types::*;

/// Attempt to parse a class file given a path to a class file (without .class extension)
///
/// ```rust
/// match classfile_parser::parse_class("./java-assets/compiled-classes/BasicClass") {
///     Ok(class_file) => {
///         println!("version {},{}", class_file.major_version, class_file.minor_version);
///     }
///     Err(ex) => panic!("Failed to parse: {}", ex),
/// };
/// ```
pub fn parse_class(class_name: &str) -> Result<ClassFile, String> {
    let class_file_name = &format!("{}.class", class_name);
    let path = Path::new(class_file_name);
    let display = path.display();

    let file = match File::open(path) {
        Err(why) => {
            return Err(format!("Unable to open {}: {}", display, &why.to_string()));
        }
        Ok(file) => file,
    };

    let mut reader = BufReader::new(file);
    parse_class_from_reader(&mut reader)
}

/// Attempt to parse a class file given a reader that implements the std::io::Read trait.
/// Parameters shouldn't be passed for the sole purpose of debug output, this should be
/// abstracted instead.
/// OLD: The file_path parameter is only used in case of errors to provide
/// reasonable error messages.
///
/// ```rust
/// let mut reader = "this_will_be_parsed_as_classfile".as_bytes();
/// let result = classfile_parser::parse_class_from_reader(&mut reader);
/// assert!(result.is_err());
/// ```
pub fn parse_class_from_reader<T: Read>(reader: &mut T) -> Result<ClassFile, String> {
    let mut class_bytes = Vec::new();
    reader
        .read_to_end(&mut class_bytes)
        .expect("cannot continue, read_to_end failed");

    let parsed_class = class_parser(&class_bytes);
    match parsed_class {
        Ok((a, c)) => {
            if !a.is_empty() {
                eprintln!(
                    "Warning: not all bytes were consumed when parsing classfile, {} bytes remaining",
                    a.len()
                );
            }

            Ok(c)
        }
        Err(e) => Err(format!("Failed to parse classfile: {}", e)),
    }
}
