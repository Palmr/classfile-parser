#![cfg(feature = "compile")]

use std::fs;
use std::io::{Cursor, Read};
use std::process::Command;

use binrw::BinWrite;
use binrw::prelude::*;
use classfile_parser::ClassFile;
use classfile_parser::code_attribute::Instruction;
use classfile_parser::compile::{
    CompileOptions, compile_method_body, generate_bytecode, parse_method_body, prepend_method_body,
};

mod e2e;
mod param_access;
mod parser;
mod prepend;
mod stress;

// --- Test helpers ---

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

#[allow(unused)]
fn compile_and_load(
    test_name: &str,
    java_src: &str,
    class_name: &str,
) -> (std::path::PathBuf, std::path::PathBuf, ClassFile) {
    let tmp_dir = std::env::temp_dir().join(format!("classfile_compile_{}", test_name));
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
    tmp_dir: &std::path::Path,
    class_path: &std::path::Path,
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
