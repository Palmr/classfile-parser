extern crate classfile_parser;
extern crate nom;

use classfile_parser::class_parser;

#[test]
fn test_attribute_stack_map_table() {
    let stack_map_class = include_bytes!("../java-assets/compiled-classes/Factorial.class");
    let res = class_parser(stack_map_class);
    match res {
        Result::Ok((_, c)) => {
            use classfile_parser::attribute_info::code_attribute_parser;
            use classfile_parser::attribute_info::stack_map_table_attribute_parser;

            assert_eq!(c.methods.len(), 2);
            assert_eq!(c.methods.len(), c.methods_count as usize);

            let method = &c.methods[1];
            assert_eq!(method.attributes.len(), 1);
            assert_eq!(method.attributes.len(), method.attributes_count as usize);

            let code = match code_attribute_parser(&method.attributes[0].info) {
                Result::Ok((_, c)) => c,
                _ => panic!("Could not get code attribute"),
            };
            assert_eq!(code.attributes_count, 1);

            let p = stack_map_table_attribute_parser(&code.attributes[0].info);
            match p {
                Result::Ok((_, a)) => {
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
