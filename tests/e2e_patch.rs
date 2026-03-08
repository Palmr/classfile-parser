use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use std::process::Command;

use binrw::BinWrite;
use binrw::prelude::*;
use classfile_parser::attribute_info::AttributeInfoVariant;
use classfile_parser::code_attribute::Instruction;
use classfile_parser::constant_info::{ConstantInfo, StringConstant, Utf8Constant};
use classfile_parser::{ClassAccessFlags, ClassFile};

// --- Helpers ---

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
    let tmp_dir = std::env::temp_dir().join(format!("classfile_e2e_{}", test_name));
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

/// Returns (method_index, code_attribute_index) for the named method.
fn find_code_attr(class_file: &ClassFile, method_name: &str) -> (usize, usize) {
    let method_idx = class_file
        .methods
        .iter()
        .position(|m| {
            matches!(
                &class_file.const_pool[(m.name_index - 1) as usize],
                ConstantInfo::Utf8(u) if u.utf8_string == method_name
            )
        })
        .unwrap_or_else(|| panic!("method '{}' not found", method_name));

    let attr_idx = class_file.methods[method_idx]
        .attributes
        .iter()
        .position(|a| matches!(a.info_parsed, Some(AttributeInfoVariant::Code(_))))
        .expect("no Code attribute found");

    (method_idx, attr_idx)
}

// --- Tests ---

/// Test 1: Patch a float constant in the constant pool.
/// SimpleMath.floatMath divides 1.0 / 3.0 = 0.33333334.
/// Change 3.0 to 6.0, so the result becomes 1.0 / 6.0 = 0.16666667.
#[test]
fn test_e2e_patch_float_constant() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }

    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "float_const",
        "java-assets/src/SimpleMath.java",
        "SimpleMath",
    );

    let mut patched = false;
    for entry in &mut class_file.const_pool {
        if let ConstantInfo::Float(f) = entry {
            if f.value == 3.0 {
                f.value = 6.0;
                patched = true;
            }
        }
    }
    assert!(patched, "could not find float 3.0 in constant pool");

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "SimpleMath");
    assert!(
        output.contains("0.16666667"),
        "expected 0.16666667 in output: {}",
        output
    );
}

/// Test 2: Patch an instruction operand via attribute reserialization.
/// SimpleMath.intMath has `int a = 10` which compiles to `bipush 10`.
/// Change to `bipush 5` so c = 5 + 20 = 25.
#[test]
fn test_e2e_patch_instruction_operand() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }

    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "instr_operand",
        "java-assets/src/SimpleMath.java",
        "SimpleMath",
    );

    let (mi, ai) = find_code_attr(&class_file, "intMath");

    {
        let code = match &mut class_file.methods[mi].attributes[ai].info_parsed {
            Some(AttributeInfoVariant::Code(c)) => c,
            _ => unreachable!(),
        };
        let mut found = false;
        for instr in &mut code.code {
            if *instr == Instruction::Bipush(10) {
                *instr = Instruction::Bipush(5);
                found = true;
                break;
            }
        }
        assert!(found, "could not find Bipush(10) in intMath");
    }

    class_file.methods[mi].attributes[ai]
        .sync_from_parsed()
        .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "SimpleMath");
    assert!(output.contains("25"), "expected 25 in output: {}", output);
}

/// Test 3: Replace an instruction opcode via attribute reserialization.
/// SimpleMath.intMath has `int c = a + b` which compiles to `iadd`.
/// Replace with `isub` so c = 10 - 20 = -10.
#[test]
fn test_e2e_replace_instruction_opcode() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }

    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "instr_opcode",
        "java-assets/src/SimpleMath.java",
        "SimpleMath",
    );

    let (mi, ai) = find_code_attr(&class_file, "intMath");

    {
        let code = match &mut class_file.methods[mi].attributes[ai].info_parsed {
            Some(AttributeInfoVariant::Code(c)) => c,
            _ => unreachable!(),
        };
        let mut found = false;
        for instr in &mut code.code {
            if *instr == Instruction::Iadd {
                *instr = Instruction::Isub;
                found = true;
                break;
            }
        }
        assert!(found, "could not find Iadd in intMath");
    }

    class_file.methods[mi].attributes[ai]
        .sync_from_parsed()
        .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "SimpleMath");
    assert!(output.contains("-10"), "expected -10 in output: {}", output);
}

/// Test 4: Add new constant pool entries and redirect an ldc instruction.
/// HelloWorld.main loads "Hello World!" via ldc.
/// Add "Injected!" to the pool and redirect the ldc to it.
#[test]
fn test_e2e_add_constant_and_redirect_ldc() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }

    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "redirect_ldc",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    // Add new Utf8 constant
    let utf8_cp_index = (class_file.const_pool.len() + 1) as u16;
    class_file.const_pool.push(ConstantInfo::Utf8(Utf8Constant {
        utf8_string: String::from("Injected!"),
    }));

    // Add new String constant referencing the Utf8
    let string_cp_index = (class_file.const_pool.len() + 1) as u16;
    class_file
        .const_pool
        .push(ConstantInfo::String(StringConstant {
            string_index: utf8_cp_index,
        }));

    class_file.sync_counts();

    assert!(
        string_cp_index <= 255,
        "string constant pool index {} exceeds u8 range",
        string_cp_index
    );

    let (mi, ai) = find_code_attr(&class_file, "main");

    {
        let code = match &mut class_file.methods[mi].attributes[ai].info_parsed {
            Some(AttributeInfoVariant::Code(c)) => c,
            _ => unreachable!(),
        };
        let mut found = false;
        for instr in &mut code.code {
            if let Instruction::Ldc(_) = instr {
                *instr = Instruction::Ldc(string_cp_index as u8);
                found = true;
                break;
            }
        }
        assert!(found, "could not find Ldc in main");
    }

    class_file.methods[mi].attributes[ai]
        .sync_from_parsed()
        .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(
        output, "Injected!",
        "expected 'Injected!' but got: {}",
        output
    );
}

/// Test 5: Patch class access flags.
/// Toggle FINAL on HelloWorld. It should still load and run
/// (FINAL just prevents subclassing).
#[test]
fn test_e2e_patch_access_flags() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }

    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "access_flags",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    assert!(
        !class_file.access_flags.contains(ClassAccessFlags::FINAL),
        "HelloWorld should not already be FINAL"
    );

    class_file.access_flags |= ClassAccessFlags::FINAL;

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(
        output, "Hello World!",
        "expected 'Hello World!' but got: {}",
        output
    );

    // Re-parse and verify FINAL flag is set
    let mut class_bytes = Vec::new();
    std::fs::File::open(&class_path)
        .unwrap()
        .read_to_end(&mut class_bytes)
        .unwrap();
    let reparsed = ClassFile::read(&mut Cursor::new(&class_bytes)).expect("failed to re-parse");
    assert!(
        reparsed.access_flags.contains(ClassAccessFlags::FINAL),
        "FINAL flag should be set in re-parsed class"
    );
}

/// Test 6: Remove a method and its call site.
/// SimpleMath has intMath, floatMath, and main.
/// Remove intMath from the methods array and replace its call in main
/// with nop instructions. Output should only contain "Float math:" line.
#[test]
fn test_e2e_remove_method() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }

    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "remove_method",
        "java-assets/src/SimpleMath.java",
        "SimpleMath",
    );

    let (main_mi, main_ai) = find_code_attr(&class_file, "main");

    // Replace the first 4 instructions of main (the "Integer math:" println + intMath call)
    // with nop instructions. Byte sizes: getstatic(3) + ldc(2) + invokevirtual(3) + invokestatic(3) = 11 nops.
    {
        let code = match &mut class_file.methods[main_mi].attributes[main_ai].info_parsed {
            Some(AttributeInfoVariant::Code(c)) => c,
            _ => unreachable!(),
        };

        assert!(
            matches!(&code.code[0], Instruction::Getstatic(_)),
            "expected Getstatic as first instruction, got {:?}",
            &code.code[0]
        );
        assert!(
            matches!(&code.code[1], Instruction::Ldc(_)),
            "expected Ldc as second instruction, got {:?}",
            &code.code[1]
        );
        assert!(
            matches!(&code.code[2], Instruction::Invokevirtual(_)),
            "expected Invokevirtual as third instruction, got {:?}",
            &code.code[2]
        );
        assert!(
            matches!(&code.code[3], Instruction::Invokestatic(_)),
            "expected Invokestatic as fourth instruction, got {:?}",
            &code.code[3]
        );

        // Replace first 4 instructions with 11 nops (matching total byte count: 3+2+3+3=11)
        let nops: Vec<Instruction> = vec![Instruction::Nop; 11];
        code.code.splice(0..4, nops);
    }

    class_file.methods[main_mi].attributes[main_ai]
        .sync_from_parsed()
        .unwrap();

    // Remove intMath method
    let int_math_idx = class_file
        .methods
        .iter()
        .position(|m| {
            matches!(
                &class_file.const_pool[(m.name_index - 1) as usize],
                ConstantInfo::Utf8(u) if u.utf8_string == "intMath"
            )
        })
        .expect("intMath method not found");
    class_file.methods.remove(int_math_idx);
    class_file.sync_counts();

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
    assert!(
        output.contains("0.33333"),
        "expected float result in output: {}",
        output
    );
}

#[test]
fn test_e2e_patch_string_contents() {
    // Fail if javac/java aren't available
    if Command::new("javac").arg("-version").output().is_err() {
        eprintln!("skipping test_e2e_patch: javac not found");
        assert!(false);
        return;
    }
    if Command::new("java").arg("-version").output().is_err() {
        eprintln!("skipping test_e2e_patch: java not found");
        assert!(false);
        return;
    }

    let tmp_dir = std::env::temp_dir().join("classfile_e2e_patch_test");
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).expect("failed to create temp dir");
    eprintln!("Writing to {}", &tmp_dir.display());

    let compile = Command::new("javac")
        .arg("-d")
        .arg(&tmp_dir)
        .arg("java-assets/src/HelloWorld.java")
        .output()
        .expect("failed to run javac");

    assert!(
        compile.status.success(),
        "javac failed: {}",
        String::from_utf8_lossy(&compile.stderr)
    );

    let class_path = tmp_dir.join("HelloWorld.class");
    let mut class_bytes = Vec::new();
    std::fs::File::open(&class_path)
        .expect("failed to open compiled class")
        .read_to_end(&mut class_bytes)
        .unwrap();

    let mut class_file =
        ClassFile::read(&mut Cursor::new(&class_bytes)).expect("failed to parse class");

    let mut patched = false;
    for entry in &mut class_file.const_pool {
        if let ConstantInfo::Utf8(utf8) = entry {
            if utf8.utf8_string == "Hello World!" {
                utf8.utf8_string = String::from("Patched!");
                patched = true;
            }
        }
    }
    assert!(patched, "could not find 'Hello World!' in constant pool");

    let mut out = Cursor::new(Vec::new());
    class_file
        .write(&mut out)
        .expect("failed to write patched class");
    fs::write(&class_path, out.into_inner()).expect("failed to write patched class file");

    let run = Command::new("java")
        .arg("-cp")
        .arg(&tmp_dir)
        .arg("HelloWorld")
        .output()
        .expect("failed to run java");

    assert!(
        run.status.success(),
        "java failed (exit {}): {}",
        run.status,
        String::from_utf8_lossy(&run.stderr)
    );

    let stdout = String::from_utf8_lossy(&run.stdout);
    assert_eq!(
        stdout.trim(),
        "Patched!",
        "expected 'Patched!' but got: {:?}",
        stdout
    );

    let _ = fs::remove_dir_all(&tmp_dir);
}
