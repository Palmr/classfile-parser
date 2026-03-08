use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use std::process::Command;

use binrw::BinWrite;
use binrw::prelude::*;
use classfile_parser::ClassFile;
use classfile_parser::code_attribute::Instruction;
use classfile_parser::constant_info::ConstantInfo;

// --- Helpers ---

fn load_basic_class() -> ClassFile {
    let mut contents: Vec<u8> = Vec::new();
    std::fs::File::open("java-assets/compiled-classes/BasicClass.class")
        .unwrap()
        .read_to_end(&mut contents)
        .unwrap();
    ClassFile::read(&mut Cursor::new(&contents)).expect("failed to parse BasicClass")
}

fn java_available() -> bool {
    Command::new("javac")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
        && Command::new("java")
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
}

fn compile_and_load(
    test_name: &str,
    java_src: &str,
    class_name: &str,
) -> (std::path::PathBuf, std::path::PathBuf, ClassFile) {
    let tmp_dir = std::env::temp_dir().join(format!("classfile_helpers_{}", test_name));
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).unwrap();

    let compile = Command::new("javac")
        .arg("-d")
        .arg(&tmp_dir)
        .arg(java_src)
        .output()
        .expect("failed to run javac");
    assert!(
        compile.status.success(),
        "javac failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let class_path = tmp_dir.join(format!("{}.class", class_name));
    let mut class_bytes = Vec::new();
    std::fs::File::open(&class_path)
        .expect("failed to open compiled class")
        .read_to_end(&mut class_bytes)
        .unwrap();
    let class_file =
        ClassFile::read(&mut Cursor::new(&class_bytes)).expect("failed to parse class");

    (tmp_dir, class_path, class_file)
}

fn write_and_run(
    tmp_dir: &Path,
    class_path: &Path,
    class_file: &ClassFile,
    class_name: &str,
) -> String {
    let mut out = Cursor::new(Vec::new());
    class_file.write(&mut out).expect("failed to write class");
    fs::write(class_path, out.into_inner()).expect("failed to write class file");

    let run = Command::new("java")
        .arg("-cp")
        .arg(tmp_dir)
        .arg(class_name)
        .output()
        .expect("failed to run java");
    assert!(
        run.status.success(),
        "java failed (exit {}): stderr={}",
        run.status,
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8_lossy(&run.stdout).trim().to_string()
}

// --- Unit tests: Constant pool helpers ---

#[test]
fn test_add_utf8() {
    let mut cf = load_basic_class();
    let original_len = cf.const_pool.len();
    let idx = cf.add_utf8("test_string_42");
    assert_eq!(idx, (original_len + 1) as u16);
    assert_eq!(cf.const_pool.len(), original_len + 1);
    assert_eq!(cf.get_utf8(idx), Some("test_string_42"));
}

#[test]
fn test_get_or_add_utf8_existing() {
    let mut cf = load_basic_class();
    // "Code" is always present in any class file
    let existing_idx = cf.find_utf8_index("Code").expect("Code should exist");
    let original_len = cf.const_pool.len();
    let idx = cf.get_or_add_utf8("Code");
    assert_eq!(idx, existing_idx);
    assert_eq!(cf.const_pool.len(), original_len, "pool should not grow");
}

#[test]
fn test_get_or_add_utf8_new() {
    let mut cf = load_basic_class();
    let original_len = cf.const_pool.len();
    let idx = cf.get_or_add_utf8("brand_new_entry");
    assert_eq!(idx, (original_len + 1) as u16);
    assert_eq!(cf.const_pool.len(), original_len + 1);
    assert_eq!(cf.get_utf8(idx), Some("brand_new_entry"));
}

#[test]
fn test_add_string() {
    let mut cf = load_basic_class();
    let original_len = cf.const_pool.len();
    let string_idx = cf.add_string("hello");
    // Should have added 2 entries: Utf8 + String
    assert_eq!(cf.const_pool.len(), original_len + 2);
    assert_eq!(string_idx, (original_len + 2) as u16);
    // The String should point to the Utf8
    let utf8_idx = (original_len + 1) as u16;
    match &cf.const_pool[(string_idx - 1) as usize] {
        ConstantInfo::String(s) => assert_eq!(s.string_index, utf8_idx),
        other => panic!("expected String constant, got {:?}", other),
    }
    assert_eq!(cf.get_utf8(utf8_idx), Some("hello"));
}

#[test]
fn test_get_or_add_string_dedup() {
    let mut cf = load_basic_class();
    let idx1 = cf.add_string("dedup_me");
    let len_after_first = cf.const_pool.len();
    let idx2 = cf.get_or_add_string("dedup_me");
    assert_eq!(idx1, idx2, "should return same index");
    assert_eq!(
        cf.const_pool.len(),
        len_after_first,
        "pool should not grow on dedup"
    );
}

#[test]
fn test_add_class() {
    let mut cf = load_basic_class();
    let original_len = cf.const_pool.len();
    let class_idx = cf.add_class("com/example/Test");
    assert_eq!(cf.const_pool.len(), original_len + 2);
    assert_eq!(class_idx, (original_len + 2) as u16);
    match &cf.const_pool[(class_idx - 1) as usize] {
        ConstantInfo::Class(c) => {
            assert_eq!(cf.get_utf8(c.name_index), Some("com/example/Test"));
        }
        other => panic!("expected Class constant, got {:?}", other),
    }
}

#[test]
fn test_get_or_add_class_dedup() {
    let mut cf = load_basic_class();
    let idx1 = cf.add_class("com/example/Foo");
    let len_after_first = cf.const_pool.len();
    let idx2 = cf.get_or_add_class("com/example/Foo");
    assert_eq!(idx1, idx2, "should return same index");
    assert_eq!(
        cf.const_pool.len(),
        len_after_first,
        "pool should not grow on dedup"
    );
}

#[test]
fn test_add_name_and_type() {
    let mut cf = load_basic_class();
    let original_len = cf.const_pool.len();
    let nat_idx = cf.add_name_and_type("myMethod", "(I)V");
    // get_or_add_utf8 may reuse existing entries; count new entries
    assert!(cf.const_pool.len() > original_len);
    match &cf.const_pool[(nat_idx - 1) as usize] {
        ConstantInfo::NameAndType(nat) => {
            assert_eq!(cf.get_utf8(nat.name_index), Some("myMethod"));
            assert_eq!(cf.get_utf8(nat.descriptor_index), Some("(I)V"));
        }
        other => panic!("expected NameAndType constant, got {:?}", other),
    }
}

// --- Unit tests: sync_all ---

#[test]
fn test_sync_all() {
    let mut cf = load_basic_class();

    // Modify an instruction in a method's code
    let method = cf.find_method_mut("<init>").expect("should have <init>");
    let code = method.code_mut().expect("should have code");
    // The constructor should have an Aload0 instruction
    assert!(
        code.code.iter().any(|i| *i == Instruction::Aload0),
        "expected Aload0 in <init>"
    );

    // Add a utf8 constant
    cf.add_utf8("sync_all_test");

    // Call sync_all and verify it doesn't error
    cf.sync_all().expect("sync_all should succeed");

    // Verify counts are correct
    assert_eq!(cf.const_pool_size, (cf.const_pool.len() + 1) as u16);
    assert_eq!(cf.methods_count, cf.methods.len() as u16);
    assert_eq!(cf.fields_count, cf.fields.len() as u16);
    assert_eq!(cf.attributes_count, cf.attributes.len() as u16);

    // Round-trip: write and re-parse
    let mut out = Cursor::new(Vec::new());
    cf.write(&mut out).expect("failed to write");
    let bytes = out.into_inner();
    let reparsed =
        ClassFile::read(&mut Cursor::new(&bytes)).expect("failed to re-parse after sync_all");
    assert_eq!(reparsed.const_pool.len(), cf.const_pool.len());
}

// --- Unit tests: with_code ---

#[test]
fn test_with_code() {
    let mut cf = load_basic_class();
    let method = cf.find_method_mut("<init>").expect("should have <init>");

    let result = method.with_code(|code| {
        // Find and count Aload0 instructions
        code.code
            .iter()
            .filter(|i| **i == Instruction::Aload0)
            .count()
    });

    match result {
        Some(Ok(count)) => assert!(count > 0, "should have found Aload0"),
        Some(Err(e)) => panic!("sync failed: {:?}", e),
        None => panic!("expected Code attribute"),
    }
}

#[test]
fn test_with_code_none() {
    let mut cf = load_basic_class();
    // Add a dummy method with no Code attribute (simulating abstract)
    use classfile_parser::method_info::MethodAccessFlags;
    use classfile_parser::method_info::MethodInfo;
    let name_idx = cf.add_utf8("abstractMethod");
    let desc_idx = cf.get_or_add_utf8("()V");
    cf.methods.push(MethodInfo {
        access_flags: MethodAccessFlags::ABSTRACT | MethodAccessFlags::PUBLIC,
        name_index: name_idx,
        descriptor_index: desc_idx,
        attributes_count: 0,
        attributes: vec![],
    });
    cf.sync_counts();

    let method = cf.find_method_mut("abstractMethod").expect("should find");
    let result = method.with_code(|_code| ());
    assert!(result.is_none(), "abstract method should return None");
}

// --- Unit tests: instruction helpers ---

#[test]
fn test_find_instruction() {
    let cf = load_basic_class();
    let method = cf.find_method("<init>").expect("should have <init>");
    let code = method.code().expect("should have code");

    let found = code.find_instruction(|i| *i == Instruction::Aload0);
    assert!(found.is_some(), "should find Aload0");
    let (idx, instr) = found.unwrap();
    assert_eq!(*instr, Instruction::Aload0);
    assert_eq!(idx, 0, "Aload0 should be first instruction in <init>");
}

#[test]
fn test_find_instructions() {
    let cf = load_basic_class();
    let method = cf.find_method("<init>").expect("should have <init>");
    let code = method.code().expect("should have code");

    // Find all return-type instructions
    let returns = code.find_instructions(|i| matches!(i, Instruction::Return));
    assert!(
        !returns.is_empty(),
        "should find at least one Return instruction"
    );
}

#[test]
fn test_replace_instruction() {
    let mut cf = load_basic_class();
    let method = cf.find_method_mut("<init>").expect("should have <init>");
    let code = method.code_mut().expect("should have code");

    // Find Aload0 and replace with Nop
    let (idx, _) = code
        .find_instruction(|i| *i == Instruction::Aload0)
        .expect("should find Aload0");
    code.replace_instruction(idx, Instruction::Nop);
    assert_eq!(code.code[idx], Instruction::Nop);
}

#[test]
fn test_nop_out() {
    let mut cf = load_basic_class();
    let method = cf.find_method_mut("<init>").expect("should have <init>");
    let code = method.code_mut().expect("should have code");

    // Record original code_length by syncing
    code.sync_lengths().expect("sync_lengths");
    let original_code_length = code.code_length;

    // nop_out the first 2 instructions
    let original_count = code.code.len();
    code.nop_out(0..2).expect("nop_out should succeed");

    // Sync and verify code_length is preserved
    code.sync_lengths().expect("sync_lengths after nop_out");
    assert_eq!(
        code.code_length, original_code_length,
        "code_length should be preserved after nop_out"
    );

    // Verify the nop'd region is all Nop
    // The first N entries (where N = byte size of original 2 instructions) should be Nop
    assert!(
        code.code.len() >= original_count,
        "instruction count should grow or stay same"
    );
    for i in &code.code[..code.code.len() - (original_count - 2)] {
        assert_eq!(*i, Instruction::Nop, "nop'd region should be all Nop");
    }
}

// --- E2E tests ---

/// Rewrite of test_e2e_add_constant_and_redirect_ldc using helpers:
/// add_string + with_code + find_instruction + replace_instruction + sync_all
#[test]
fn test_e2e_helpers_redirect_ldc() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }

    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "helpers_ldc",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    // Add "Injected!" string to pool using helper
    let string_idx = class_file.add_string("Injected!");
    assert!(string_idx <= 255, "index must fit in u8 for ldc");

    // Find main and redirect the Ldc
    let method = class_file
        .find_method_mut("main")
        .expect("should have main");
    let result = method.with_code(|code| {
        let (idx, _) = code
            .find_instruction(|i| matches!(i, Instruction::Ldc(_)))
            .expect("should find Ldc");
        code.replace_instruction(idx, Instruction::Ldc(string_idx as u8));
    });
    result
        .expect("should have code")
        .expect("sync should succeed");

    class_file.sync_all().expect("sync_all");

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(
        output, "Injected!",
        "expected 'Injected!' but got: {}",
        output
    );
}

/// Rewrite of test_e2e_remove_method using nop_out + with_code
#[test]
fn test_e2e_helpers_nop_out() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }

    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "helpers_nop",
        "java-assets/src/SimpleMath.java",
        "SimpleMath",
    );

    // Nop out the first 4 instructions of main (the "Integer math:" println + intMath call)
    let method = class_file
        .find_method_mut("main")
        .expect("should have main");
    let result = method.with_code(|code| {
        // Verify the instructions we expect
        assert!(matches!(&code.code[0], Instruction::Getstatic(_)));
        assert!(matches!(&code.code[1], Instruction::Ldc(_)));
        assert!(matches!(&code.code[2], Instruction::Invokevirtual(_)));
        assert!(matches!(&code.code[3], Instruction::Invokestatic(_)));
        code.nop_out(0..4).expect("nop_out should succeed");
    });
    result
        .expect("should have code")
        .expect("sync should succeed");

    // Remove intMath method
    let int_math_idx = class_file
        .methods
        .iter()
        .position(|m| class_file.get_utf8(m.name_index) == Some("intMath"))
        .expect("intMath not found");
    class_file.methods.remove(int_math_idx);

    class_file.sync_all().expect("sync_all");

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "SimpleMath");
    assert!(
        !output.contains("Integer math:"),
        "should not contain 'Integer math:', got: {}",
        output
    );
    assert!(
        output.contains("Float math:"),
        "expected 'Float math:' in output: {}",
        output
    );
}
