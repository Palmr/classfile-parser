extern crate classfile_parser;

use std::fs::File;
use std::io::Cursor;
use std::io::prelude::*;

use binrw::prelude::*;
use classfile_parser::ClassFile;
use classfile_parser::attribute_info::{AttributeInfoVariant, StackMapFrameInner};
use classfile_parser::constant_info::ConstantInfo;

#[test]
fn test_attribute_stack_map_table() {
    let mut contents: Vec<u8> = Vec::new();
    let mut stack_map_class = File::open("java-assets/compiled-classes/Factorial.class").unwrap();
    stack_map_class.read_to_end(&mut contents).unwrap();
    let res = ClassFile::read(&mut Cursor::new(&mut contents));
    match res {
        Ok(c) => {
            let mut stack_map_table_index = 0;
            println!("Constant pool:");
            for (const_index, const_item) in c.const_pool.iter().enumerate() {
                println!("\t[{}] = {:?}", (const_index + 1), const_item);
                if let ConstantInfo::Utf8(ref c) = *const_item {
                    if c.utf8_string == "StackMapTable" {
                        if stack_map_table_index != 0 {
                            panic!("Should not find more than one StackMapTable constant");
                        }
                        stack_map_table_index = (const_index + 1) as u16;
                    }
                }
            }
            println!("Methods:");
            for (method_index, method_info) in c.methods.iter().enumerate() {
                println!("\t[{}] = {:?}", method_index, method_info);
            }

            assert_eq!(c.methods.len(), 2);
            assert_eq!(c.methods.len(), c.methods_count as usize);

            let method = &c.methods[1];
            assert_eq!(method.attributes.len(), 1);
            assert_eq!(method.attributes.len(), method.attributes_count as usize);

            // The top-level method attribute should be parsed as Code
            let code = match method.attributes[0].info_parsed {
                Some(AttributeInfoVariant::Code(ref code)) => code,
                _ => panic!("Could not get code attribute"),
            };

            // Sub-attributes inside CodeAttribute now have interpret_inner called
            // automatically, so info_parsed is populated.
            let smt_attr = code
                .attributes
                .iter()
                .find(|a| a.attribute_name_index == stack_map_table_index)
                .expect("StackMapTable attribute not found");

            let smt = match smt_attr.info_parsed {
                Some(AttributeInfoVariant::StackMapTable(ref smt)) => smt,
                _ => panic!("StackMapTable sub-attribute was not parsed via interpret_inner"),
            };

            assert_eq!(smt.entries.len(), smt.number_of_entries as usize);
            assert_eq!(smt.entries.len(), 2);

            match smt.entries[0].inner {
                StackMapFrameInner::SameFrame { .. } => {}
                _ => panic!("unexpected frame type for frame 0"),
            };
            match smt.entries[1].inner {
                StackMapFrameInner::SameLocals1StackItemFrame { .. } => {}
                _ => panic!("unexpected frame type for frame 1: {:?}", &smt.entries[1]),
            };
        }
        _ => panic!("not a class file"),
    };
}
