extern crate classfile_parser;

use binrw::BinWrite;
use binrw::prelude::*;
use classfile_parser::ClassFile;
use classfile_parser::attribute_info::AttributeInfoVariant;
use classfile_parser::constant_info::ConstantInfo;
use std::fs::File;
use std::io::Cursor;
use std::io::prelude::*;

#[test]
fn test_valid_class() {
    let mut contents: Vec<u8> = Vec::new();
    let mut valid_class = File::open("java-assets/compiled-classes/BasicClass.class").unwrap();
    valid_class.read_to_end(&mut contents).unwrap();
    let res = ClassFile::read(&mut Cursor::new(&mut contents));
    dbg!(&res);
    match res {
        Result::Ok(c) => {
            println!(
                "Valid class file, version {},{} const_pool({}), this=const[{}], super=const[{}], interfaces({}), fields({}), methods({}), attributes({}), access({:?})",
                c.major_version,
                c.minor_version,
                c.const_pool_size,
                c.this_class,
                c.super_class,
                c.interfaces_count,
                c.fields_count,
                c.methods_count,
                c.attributes_count,
                c.access_flags
            );

            let mut code_const_index = 0;

            println!("Constant pool:");
            for (const_index, const_item) in c.const_pool.iter().enumerate() {
                println!("\t[{}] = {:?}", (const_index + 1), const_item);
                if let ConstantInfo::Utf8(ref c) = *const_item {
                    if c.utf8_string == "Code" {
                        code_const_index = (const_index + 1) as u16;
                    }
                }
            }
            println!("Code index = {}", code_const_index);

            println!("Interfaces:");
            for (interface_index, i) in c.interfaces.iter().enumerate() {
                println!(
                    "\t[{}] = const[{}] = {:?}",
                    interface_index,
                    i,
                    c.const_pool[(i - 1) as usize]
                );
            }
            println!("Fields:");
            for (field_index, f) in c.fields.iter().enumerate() {
                println!(
                    "\t[{}] Name(const[{}] = {:?}) - access({:?})",
                    field_index,
                    f.name_index,
                    c.const_pool[(f.name_index - 1) as usize],
                    f.access_flags
                );
            }
            println!("Methods:");
            for (method_index, m) in c.methods.iter().enumerate() {
                println!(
                    "\t[{}] Name(const[{}] = {:?}) - access({:?})",
                    method_index,
                    m.name_index,
                    c.const_pool[(m.name_index - 1) as usize],
                    m.access_flags
                );

                for a in &m.attributes {
                    if a.attribute_name_index == code_const_index {
                        println!("\t\tCode attr found, len = {}", a.attribute_length);
                        match a.info_parsed.as_ref().unwrap() {
                            AttributeInfoVariant::Code(code) => {
                                println!("\t\t\tCode! code_length = {}", code.code_length);
                            }
                            _ => panic!("Not a valid code attr?"),
                        }
                    } else {
                        println!("\t\tAttribute: {:?}", a);
                    }
                }
            }
        }
        _ => panic!("Not a class file"),
    };
}

#[test]
fn test_utf_string_constants() {
    let mut contents: Vec<u8> = Vec::new();
    let mut utf8_class = File::open("java-assets/compiled-classes/UnicodeStrings.class").unwrap();
    utf8_class.read_to_end(&mut contents).unwrap();
    let res = ClassFile::read(&mut Cursor::new(contents));
    match res {
        Result::Ok(c) => {
            if let ConstantInfo::Utf8(ref con) = c.const_pool[13] {
                assert_eq!(con.utf8_string, "2H₂ + O₂ ⇌ 2H₂O, R = 4.7 kΩ, ⌀ 200 mm");
            }

            if let ConstantInfo::Utf8(ref con) = c.const_pool[21] {
                assert_eq!(
                    con.utf8_string,
                    "ᚻᛖ ᚳᚹᚫᚦ ᚦᚫᛏ ᚻᛖ ᛒᚢᛞᛖ ᚩᚾ ᚦᚫᛗ ᛚᚪᚾᛞᛖ ᚾᚩᚱᚦᚹᛖᚪᚱᛞᚢᛗ ᚹᛁᚦ ᚦᚪ ᚹᛖᛥᚫ"
                );
            }

            if let ConstantInfo::Utf8(ref con) = c.const_pool[23] {
                assert_eq!(con.utf8_string, "⡌⠁⠧⠑ ⠼⠁⠒  ⡍⠜⠇⠑⠹⠰⠎ ⡣⠕⠌");
            }

            if let ConstantInfo::Utf8(ref con) = c.const_pool[25] {
                assert_eq!(con.utf8_string, "\0𠜎");
            }

            if let ConstantInfo::Utf8(ref con) = c.const_pool[27] {
                assert_eq!(con.utf8_string, "X���X");
                assert_eq!(con.utf8_string.len(), 11);
            }

            for (const_index, const_item) in c.const_pool.iter().enumerate() {
                println!("\t[{}] = {:?}", (const_index + 1), const_item);
            }
        }

        _ => panic!("Not a class file"),
    }
}

#[test]
fn test_malformed_class() {
    let mut contents: Vec<u8> = Vec::new();
    let mut invalid_class = File::open("java-assets/compiled-classes/malformed.class").unwrap();
    invalid_class.read_to_end(&mut contents).unwrap();
    let res = ClassFile::read(&mut Cursor::new(contents));
    if res.is_ok() {
        panic!("The file is not valid and shouldn't be parsed")
    };
}

#[test]
fn test_round_trip() {
    let mut original_bytes: Vec<u8> = Vec::new();
    let mut class_file = File::open("java-assets/compiled-classes/BasicClass.class").unwrap();
    class_file.read_to_end(&mut original_bytes).unwrap();

    let parsed = ClassFile::read(&mut Cursor::new(&original_bytes)).expect("failed to parse class");

    let mut written_bytes = Cursor::new(Vec::new());
    parsed
        .write(&mut written_bytes)
        .expect("failed to write class");
    let written_bytes = written_bytes.into_inner();

    assert_eq!(
        original_bytes.len(),
        written_bytes.len(),
        "written class file has different length: original={}, written={}",
        original_bytes.len(),
        written_bytes.len()
    );
    assert_eq!(
        original_bytes, written_bytes,
        "written class file bytes differ from original"
    );
}

/// Verify that sync_from_parsed() on unmodified Code attributes produces identical bytes.
#[test]
fn test_sync_from_parsed_idempotent() {
    // Note: UnicodeStrings excluded — it contains invalid UTF-8 (unpaired surrogates)
    // that get normalized to U+FFFD during parsing, so round-trip is not byte-identical.
    for class_name in &["BasicClass", "Factorial", "HelloWorld", "Instructions"] {
        let path = format!("java-assets/compiled-classes/{}.class", class_name);
        let mut original_bytes = Vec::new();
        File::open(&path)
            .unwrap_or_else(|_| panic!("failed to open {}", path))
            .read_to_end(&mut original_bytes)
            .unwrap();

        let mut class_file =
            ClassFile::read(&mut Cursor::new(&original_bytes)).expect("failed to parse");

        // Sync all Code attributes without modifying them
        for method in &mut class_file.methods {
            for attr in &mut method.attributes {
                if matches!(attr.info_parsed, Some(AttributeInfoVariant::Code(_))) {
                    attr.sync_from_parsed().expect("sync_from_parsed failed");
                }
            }
        }

        let mut written_bytes = Cursor::new(Vec::new());
        class_file
            .write(&mut written_bytes)
            .expect("failed to write");
        let written_bytes = written_bytes.into_inner();

        assert_eq!(
            original_bytes, written_bytes,
            "{}: sync_from_parsed on unmodified Code changed the output",
            class_name
        );
    }
}

/// Verify that modifying an instruction survives write → re-read.
#[test]
fn test_mutation_round_trip_instruction() {
    let mut original_bytes = Vec::new();
    File::open("java-assets/compiled-classes/BasicClass.class")
        .unwrap()
        .read_to_end(&mut original_bytes)
        .unwrap();

    let mut class_file =
        ClassFile::read(&mut Cursor::new(&original_bytes)).expect("failed to parse");

    // Find first method with a Code attribute containing Aload0
    let mut found = false;
    for method in &mut class_file.methods {
        for attr in &mut method.attributes {
            if let Some(AttributeInfoVariant::Code(ref mut code)) = attr.info_parsed {
                for instr in &mut code.code {
                    if matches!(instr, classfile_parser::code_attribute::Instruction::Aload0) {
                        *instr = classfile_parser::code_attribute::Instruction::Aload1;
                        found = true;
                        break;
                    }
                }
                if found {
                    attr.sync_from_parsed().unwrap();
                    break;
                }
            }
        }
        if found {
            break;
        }
    }
    assert!(found, "could not find Aload0 in BasicClass");

    // Write and re-read
    let mut out = Cursor::new(Vec::new());
    class_file.write(&mut out).expect("failed to write");
    let written = out.into_inner();

    let reparsed = ClassFile::read(&mut Cursor::new(&written)).expect("failed to re-parse");

    // Verify the modification survived
    let mut verified = false;
    for method in &reparsed.methods {
        for attr in &method.attributes {
            if let Some(AttributeInfoVariant::Code(ref code)) = attr.info_parsed {
                for instr in &code.code {
                    if matches!(instr, classfile_parser::code_attribute::Instruction::Aload1) {
                        verified = true;
                        break;
                    }
                }
            }
        }
    }
    assert!(verified, "Aload1 not found after round-trip");
}

/// Verify constant pool modification survives write → re-read.
#[test]
fn test_mutation_round_trip_constant_pool() {
    let mut original_bytes = Vec::new();
    File::open("java-assets/compiled-classes/BasicClass.class")
        .unwrap()
        .read_to_end(&mut original_bytes)
        .unwrap();

    let mut class_file =
        ClassFile::read(&mut Cursor::new(&original_bytes)).expect("failed to parse");

    // Modify a UTF-8 string in the constant pool
    let mut modified_value = None;
    for entry in &mut class_file.const_pool {
        if let ConstantInfo::Utf8(utf8) = entry {
            if utf8.utf8_string == "Code" {
                // Don't modify "Code" — it's used for attribute resolution.
                continue;
            }
            if utf8.utf8_string.len() > 2 {
                modified_value = Some(utf8.utf8_string.clone());
                utf8.utf8_string = "MODIFIED".to_string();
                break;
            }
        }
    }
    let original_value = modified_value.expect("no suitable UTF-8 constant found");

    let mut out = Cursor::new(Vec::new());
    class_file.write(&mut out).expect("failed to write");
    let written = out.into_inner();

    let reparsed = ClassFile::read(&mut Cursor::new(&written)).expect("failed to re-parse");

    // Verify the modification survived and original value is gone
    let mut found_modified = false;
    let mut found_original = false;
    for entry in &reparsed.const_pool {
        if let ConstantInfo::Utf8(utf8) = entry {
            if utf8.utf8_string == "MODIFIED" {
                found_modified = true;
            }
            if utf8.utf8_string == original_value {
                found_original = true;
            }
        }
    }
    assert!(found_modified, "'MODIFIED' not found after round-trip");
    assert!(
        !found_original,
        "original value '{}' still present after modification",
        original_value
    );
}
