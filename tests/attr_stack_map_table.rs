extern crate classfile_parser;
extern crate nom;

use classfile_parser::attribute_info::AttributeInfo;
use classfile_parser::class_parser;
use classfile_parser::constant_info::ConstantInfo;

#[test]
fn test_attribute_stack_map_table() {
    let stack_map_class = include_bytes!("../java-assets/compiled-classes/Factorial.class");
    let res = class_parser(stack_map_class);
    match res {
        Ok((_, c)) => {
            use classfile_parser::attribute_info::code_attribute_parser;
            use classfile_parser::attribute_info::stack_map_table_attribute_parser;

            let mut stack_map_table_index = 0;
            println!("Constant pool:");
            for (const_index, const_item) in c.const_pool.iter().enumerate() {
                println!("\t[{}] = {:?}", (const_index + 1), const_item);
                if let ConstantInfo::Utf8(ref c) = *const_item
                    && c.utf8_string.to_string() == "StackMapTable"
                {
                    if stack_map_table_index != 0 {
                        panic!("Should not find more than one StackMapTable constant");
                    }
                    stack_map_table_index = (const_index + 1) as u16;
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

            let code = match code_attribute_parser(&method.attributes[0].info) {
                Ok((_, c)) => c,
                _ => panic!("Could not get code attribute"),
            };

            let mut stack_map_table_attr_index = 0;
            println!("Code Attrs:");
            for (idx, code_attr) in code.attributes.iter().enumerate() {
                println!("\t[{}] = {:?}", idx, code_attr);
                let AttributeInfo {
                    ref attribute_name_index,
                    attribute_length: _,
                    info: _,
                } = *code_attr;
                if attribute_name_index == &stack_map_table_index {
                    stack_map_table_attr_index = idx;
                }
            }

            let attribute_info_bytes = &code.attributes[stack_map_table_attr_index].info;
            let p = stack_map_table_attribute_parser(attribute_info_bytes);
            match p {
                Ok((data_rem, a)) => {
                    // We should have used all the data in the stack map attribute
                    assert!(data_rem.is_empty());

                    assert_eq!(a.entries.len(), a.number_of_entries as usize);
                    assert_eq!(a.entries.len(), 2);

                    use classfile_parser::attribute_info::StackMapFrame::*;
                    match a.entries[0] {
                        SameFrame { .. } => {}
                        _ => panic!("unexpected frame type for frame 0"),
                    };
                    match a.entries[1] {
                        SameLocals1StackItemFrame { .. } => {}
                        _ => panic!("unexpected frame type for frame 1: {:?}", &a.entries[1]),
                    };
                }
                _ => panic!("failed to parse StackMapTable"),
            };
        }
        _ => panic!("not a class file"),
    };
}
