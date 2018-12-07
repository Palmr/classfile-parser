extern crate classfile_parser;

use classfile_parser::attribute_info::code_attribute_parser;
use classfile_parser::class_parser;
use classfile_parser::code_attribute::{code_parser, instruction_parser, Instruction};
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

    assert_eq!(true, parsed.is_ok());
    assert_eq!(64, parsed.unwrap().1.len());
}
