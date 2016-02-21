extern crate nom;
extern crate classfile_parser;

// use std::fs::File;
// use std::io::Read;

use nom::IResult;

use classfile_parser::{class_parser};


#[test]
fn test_valid_class() {
    let valid_class = include_bytes!("../assets/BasicClass.class");
    let res = class_parser(valid_class);
    match res {
        IResult::Done(_, c) => {
            println!("Valid class file, version {},{} const_pool({}), this=const[{}], super=const[{}], interfaces({}), fields({}), methods({}), attributes({})", c.major_version, c.minor_version, c.const_pool_size, c.this_class, c.super_class, c.interfaces_count, c.fields_count, c.methods_count, c.attributes_count);
            println!("Constant pool:");
            let mut const_index = 1;
            for cp in &c.const_pool {
                println!("\t[{}] = {}", const_index, cp.to_string());
                const_index += 1;
            }
            println!("Interfaces:");
            let mut interface_index = 0;
            for i in &c.interfaces {
                println!("\t[{}] = const[{}] = {}", interface_index, i, c.const_pool[(i-1) as usize].to_string());
                interface_index += 1;
            }
            println!("Fields:");
            let mut field_index = 0;
            for f in &c.fields {
                println!("\t[{}] Name(const[{}] = {})", field_index, f.name_index, c.const_pool[(f.name_index - 1) as usize].to_string());
                field_index += 1;
            }
            println!("Methods:");
            let mut method_index = 0;
            for m in &c.methods {
                println!("\t[{}] Name(const[{}] = {})", method_index, m.name_index, c.const_pool[(m.name_index - 1) as usize].to_string());
                method_index += 1;
            }
        },
        _ => panic!("Not a class file"),
    };
}

// TODO - NP - Figure out file load path
// #[test]
// fn test_regular_use() {
//     let mut f = File::open("../assets/BasicClass.class").unwrap();
//     let mut buffer = vec![0; 10];
//     let read = f.read_to_end(&mut buffer);

//     let res = class_parser(&buffer);
//     match res {
//         IResult::Done(_, c) => {
//             println!("Valid class file, version {},{} const_pool({}), this=const[{}], super=const[{}], interfaces({}), fields({}), methods({}), attributes({})", c.major_version, c.minor_version, c.const_pool_size, c.this_class, c.super_class, c.interfaces_count, c.fields_count, c.methods_count, c.attributes_count);
//         },
//         _ => panic!("argh!"),
//     };
// }

#[test]
fn test_malformed_class() {
    let malformed_class = include_bytes!("../assets/malformed.class");
    let res = class_parser(malformed_class);
    match res {
        IResult::Done(_, _) => panic!("The file is not valid and shouldn't be parsed"),
        _ => res,
    };
}

// #[test]
// fn test_constant_utf8() {
//     let hello_world_data = &[
//         // 0x01, // tag = 1
//         0x00, 0x0C, // length = 12
//         0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x20, 0x57, 0x6F, 0x72, 0x6C, 0x64, 0x21 // 'Hello world!' in UTF8
//     ];
//     let res = const_utf8(hello_world_data);

//     match res {
//         IResult::Done(_, c) =>
//         match c {
//             Constant::Utf8(ref s) =>
//                  println!("Valid UTF8 const: {}", s.utf8_string),
//             _ => panic!("It's a const, but of what type?")
//         },
//         _ => panic!("Not a UTF type const?"),
//     };
// }
