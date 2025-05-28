extern crate classfile_parser;

use classfile_parser::attribute_info::{code_attribute_parser, method_parameters_attribute_parser};
use classfile_parser::class_parser;
use classfile_parser::code_attribute::{
    code_parser, instruction_parser, Instruction, LocalVariableTableAttribute,
};
use classfile_parser::method_info::MethodAccessFlags;

#[test]
fn test_simple() {
    let instruction = &[0x11, 0xff, 0xfe];
    assert_eq!(
        Ok((&[][..], Instruction::Sipush(-2i16))),
        instruction_parser(instruction, 0)
    );
}

#[test]
fn test_wide() {
    let instruction = &[0xc4, 0x15, 0xaa, 0xbb];
    assert_eq!(
        Ok((&[][..], Instruction::IloadWide(0xaabb))),
        instruction_parser(instruction, 0)
    );
}

#[test]
fn test_alignment() {
    let instructions = vec![
        (
            3,
            vec![
                0xaa, 0, 0, 0, 10, 0, 0, 0, 20, 0, 0, 0, 21, 0, 0, 0, 30, 0, 0, 0, 31,
            ],
        ),
        (
            0,
            vec![
                0xaa, 0, 0, 0, 0, 0, 0, 10, 0, 0, 0, 20, 0, 0, 0, 21, 0, 0, 0, 30, 0, 0, 0, 31,
            ],
        ),
    ];
    let expected = Ok((
        &[][..],
        Instruction::Tableswitch {
            default: 10,
            low: 20,
            high: 21,
            offsets: vec![30, 31],
        },
    ));
    for (address, instruction) in instructions {
        assert_eq!(expected, instruction_parser(&instruction, address));
    }
}

#[test]
fn test_incomplete() {
    let code = &[0x59, 0x59, 0xc4, 0x15]; // dup, dup, <incomplete iload/wide>
    let expected = Ok((
        &[0xc4, 0x15][..],
        vec![(0, Instruction::Dup), (1, Instruction::Dup)],
    ));
    assert_eq!(expected, code_parser(code));
}

#[test]
fn test_class() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/Instructions.class");
    let (_, class) = class_parser(class_bytes).unwrap();
    let method_info = &class
        .methods
        .iter()
        .find(|m| m.access_flags.contains(MethodAccessFlags::STATIC))
        .unwrap();
    let (_, code_attribute) = code_attribute_parser(&method_info.attributes[0].info).unwrap();

    let parsed = code_parser(&code_attribute.code);

    assert!(parsed.is_ok());
    assert_eq!(64, parsed.unwrap().1.len());
}

fn lookup_string(c: &classfile_parser::ClassFile, index: u16) -> Option<String> {
    let con = &c.const_pool[(index - 1) as usize];
    match con {
        classfile_parser::constant_info::ConstantInfo::Utf8(utf8) => Some(utf8.utf8_string.clone()),
        _ => None,
    }
}

#[test]
fn method_parameters() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/BasicClass.class");
    let (_, class) = class_parser(class_bytes).unwrap();
    let method_info = &class.methods.iter().last().unwrap();

    // The class was not compiled with "javac -parameters" this required being able to find
    // MethodParameters in the class file, for example:
    // javac -parameters ./java-assets/src/uk/co/palmr/classfileparser/BasicClass.java -d ./java-assets/compiled-classes ; cp ./java-assets/compiled-classes/uk/co/palmr/classfileparser/BasicClass.class ./java-assets/compiled-classes/BasicClass.class
    assert_eq!(method_info.attributes.len(), 2);
    let (_, method_parameters) =
        method_parameters_attribute_parser(&method_info.attributes[1].info).unwrap();
    assert_eq!(
        lookup_string(
            &class,
            method_parameters.parameters.get(0).unwrap().name_index
        ),
        Some("a".to_string())
    );
    assert_eq!(
        lookup_string(
            &class,
            method_parameters.parameters.get(1).unwrap().name_index
        ),
        Some("b".to_string())
    );
}

#[test]
fn local_variable_table() {
    // The class was not compiled with "javac -g"
    let class_bytes = include_bytes!("../java-assets/compiled-classes/LocalVariableTable.class");
    let (_, class) = class_parser(class_bytes).unwrap();
    let method_info = &class.methods.iter().last().unwrap();

    let code_attribute = method_info
        .attributes
        .iter()
        .find_map(|attribute_info| {
            match lookup_string(&class, attribute_info.attribute_name_index)?.as_str() {
                "Code" => {
                    classfile_parser::attribute_info::code_attribute_parser(&attribute_info.info)
                        .ok()
                }
                _ => None,
            }
        })
        .map(|i| i.1)
        .unwrap();

    let local_variable_table_attribute: LocalVariableTableAttribute = code_attribute
        .attributes
        .iter()
        .find_map(|attribute_info| {
            match lookup_string(&class, attribute_info.attribute_name_index)?.as_str() {
                "LocalVariableTable" => {
                    classfile_parser::code_attribute::local_variable_table_parser(
                        &attribute_info.info,
                    )
                    .ok()
                }
                _ => None,
            }
        })
        .map(|a| a.1)
        .unwrap();

    let types: Vec<String> = local_variable_table_attribute
        .items
        .iter()
        .filter_map(|i| lookup_string(&class, i.descriptor_index))
        .collect();

    // All used types in method code block of last method
    assert_eq!(
        types,
        vec![
            "LLocalVariableTable;".to_string(),
            "Ljava/util/HashMap;".to_string(),
            "I".to_string()
        ]
    );
}

#[test]
fn source_file() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/BasicClass.class");
    let (_, class) = class_parser(class_bytes).unwrap();

    let source = class
        .attributes
        .iter()
        .find_map(|attribute_info| {
            match lookup_string(&class, attribute_info.attribute_name_index)?.as_str() {
                "SourceFile" => classfile_parser::attribute_info::sourcefile_attribute_parser(
                    &attribute_info.info,
                )
                .ok(),
                o => {
                    dbg!(o);
                    None
                }
            }
        })
        .map(|i| i.1)
        .unwrap();

    let s = lookup_string(&class, source.sourcefile_index).unwrap();

    assert_eq!(s, "BasicClass.java");
}
