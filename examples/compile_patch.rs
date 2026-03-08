//! Example: patch a compiled Java class using the compile feature.
//!
//! This example:
//! 1. Compiles a minimal Java class with `javac`
//! 2. Parses the resulting `.class` file with `ClassFile::from_bytes`
//! 3. Replaces method bodies using `patch_method!` / `patch_methods!`
//! 4. Writes the modified class back with `ClassFile::to_bytes`
//! 5. Runs it with `java` to show the new behavior
//!
//! Run with:
//!   cargo run --example compile_patch --features compile

use std::fs;
use std::process::Command;

use classfile_parser::{ClassFile, patch_method, patch_methods};

fn main() {
    // ── Step 1: Create and compile a Java class with two methods ─────────
    let tmp_dir = std::env::temp_dir().join("classfile_compile_example");
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).unwrap();

    let java_src = tmp_dir.join("HelloWorld.java");
    fs::write(
        &java_src,
        r#"
public class HelloWorld {
    public static void greet() {
        System.out.println("original greet");
    }

    public static void main(String[] args) {
        System.out.println("original main");
        greet();
    }
}
"#,
    )
    .unwrap();

    let javac = Command::new("javac")
        .arg("-d")
        .arg(&tmp_dir)
        .arg(&java_src)
        .output()
        .expect("javac not found — make sure a JDK is on your PATH");
    assert!(
        javac.status.success(),
        "javac failed: {}",
        String::from_utf8_lossy(&javac.stderr)
    );
    println!("Compiled HelloWorld.java");

    // ── Step 2: Parse the .class file ────────────────────────────────────
    let class_path = tmp_dir.join("HelloWorld.class");
    let bytes = fs::read(&class_path).unwrap();
    let mut class_file = ClassFile::from_bytes(&bytes).expect("failed to parse class");
    println!("Parsed HelloWorld.class ({} bytes)", bytes.len());

    // ── Step 3a: Patch a single method ───────────────────────────────────
    //
    // patch_method! compiles a Java method body and replaces the named method.
    // By default it generates a StackMapTable so the class passes full JVM
    // bytecode verification. Use `no_verify` as a 4th argument to skip that.
    patch_method!(
        class_file,
        "greet",
        r#"{
        System.out.println("patched greet!");
    }"#
    )
    .unwrap();
    println!("Patched greet()");

    // ── Step 3b: Patch multiple methods at once ──────────────────────────
    //
    // patch_methods! patches several methods in one call. Methods are compiled
    // in order; an error on any method stops immediately.
    patch_methods!(class_file, {
        "main" => r#"{
            int x = 42;
            switch (x) {
                case 1: System.out.println("one"); break;
                case 42: System.out.println("forty-two"); break;
                default: System.out.println("other"); break;
            }

            try {
                throw new RuntimeException("boom");
            } catch (RuntimeException e) {
                System.out.println("caught exception");
            }

            greet();
        }"#,
    })
    .unwrap();
    println!("Patched main()");

    // ── Step 4: Write back and run ───────────────────────────────────────
    let patched_bytes = class_file.to_bytes().expect("failed to serialize");
    fs::write(&class_path, &patched_bytes).unwrap();
    println!("Wrote patched class ({} bytes)\n", patched_bytes.len());

    let run = Command::new("java")
        .arg("-cp")
        .arg(&tmp_dir)
        .arg("HelloWorld")
        .output()
        .expect("java not found");
    if run.status.success() {
        println!("{}", String::from_utf8_lossy(&run.stdout).trim());
    } else {
        eprintln!(
            "java failed: {}",
            String::from_utf8_lossy(&run.stderr).trim()
        );
        std::process::exit(1);
    }

    let _ = fs::remove_dir_all(&tmp_dir);
}
