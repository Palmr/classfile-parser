//! Example: patch methods inside a JAR file using the jar-patch feature.
//!
//! This example:
//! 1. Compiles a Java class with `javac`
//! 2. Packs the `.class` file into a JAR using `JarFile`
//! 3. Patches method bodies using `patch_jar!`
//! 4. Saves the modified JAR and runs it with `java`
//!
//! Run with:
//!   cargo run --example jar_patch --features jar-patch

use std::fs;
use std::process::Command;

use classfile_parser::jar_utils::JarFile;
use classfile_parser::{patch_jar, patch_jar_method};

fn main() {
    // ── Step 1: Create and compile a Java class ──────────────────────────
    let tmp_dir = std::env::temp_dir().join("classfile_jar_patch_example");
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

    // ── Step 2: Pack into a JAR ──────────────────────────────────────────
    let class_bytes = fs::read(tmp_dir.join("HelloWorld.class")).unwrap();
    let mut jar = JarFile::new();
    jar.set_entry("HelloWorld.class", class_bytes);
    println!("Packed into JAR ({} entries)", jar.entry_names().count());

    // ── Step 3a: Patch a single method ───────────────────────────────────
    patch_jar_method!(
        jar,
        "HelloWorld.class",
        "greet",
        r#"{
        System.out.println("patched greet!");
    }"#
    )
    .unwrap();
    println!("Patched greet()");

    // ── Step 3b: Patch multiple methods across classes ───────────────────
    //
    // patch_jar! batches by class — each class is parsed once, all its
    // methods are patched, and the class is written back once.
    patch_jar!(jar, {
        "HelloWorld.class" => {
            "main" => r#"{
                System.out.println("patched main!");
                greet();
            }"#,
        },
    })
    .unwrap();
    println!("Patched main()");

    // ── Step 4: Save and run ─────────────────────────────────────────────
    let jar_path = tmp_dir.join("patched.jar");
    jar.save(&jar_path).unwrap();
    println!("Saved {}\n", jar_path.display());

    let run = Command::new("java")
        .arg("-cp")
        .arg(&jar_path)
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
