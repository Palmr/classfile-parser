extern crate nom;
extern crate classfile_parser;

use nom::IResult;

use classfile_parser::class_parser;
use classfile_parser::constant_info::ConstantInfo;


#[test]
fn test_attribute_bootstrap_methods() {
    let stack_map_class = include_bytes!("../java-assets/compiled-classes/BootstrapMethods.class");
    let res = class_parser(stack_map_class);
    match res {
        IResult::Done(_, c) => {
            println!("Valid class file, version {},{} const_pool({}), this=const[{}], super=const[{}], interfaces({}), fields({}), methods({}), attributes({}), access({:?})", c.major_version, c.minor_version, c.const_pool_size, c.this_class, c.super_class, c.interfaces_count, c.fields_count, c.methods_count, c.attributes_count, c.access_flags);

            let mut bootstrap_method_const_index = 0;

            println!("Constant pool:");
            for (const_index, const_item) in c.const_pool.iter().enumerate() {
                println!("\t[{}] = {:?}", (const_index + 1), const_item);
                match *const_item {
                    ConstantInfo::Utf8(ref c) => {
                        if c.utf8_string == "BootstrapMethods" {
                            bootstrap_method_const_index = (const_index + 1) as u16;
                        }
                    },
                    _ => {},
                }
            }
            println!("Bootstrap Methods constant index = {}", bootstrap_method_const_index);

            println!("Attributes:");
            for (attribute_index, attribute_item) in c.attributes.iter().enumerate() {
                println!("\t[{}] = {:?}", (attribute_index + 1), attribute_item);
                if attribute_item.attribute_name_index == bootstrap_method_const_index {
                    use classfile_parser::attribute_info::bootstrap_methods_attribute_parser;
                    let bsm = bootstrap_methods_attribute_parser(&attribute_item.info);
                    println!("\t\t{:?}", bsm);
                }
            }
        },
        _ => panic!("Not a class file"),
    }
}
