#![cfg(all(feature = "compile", feature = "jar-utils"))]

use std::fs;
use std::io::Write;
use std::process::Command;

use classfile_parser::compile::CompileOptions;
use classfile_parser::jar_patch::{self, JarPatchError};
use classfile_parser::jar_utils::JarFile;
// Macros are at crate root via #[macro_export]
use classfile_parser::patch_jar;
use classfile_parser::patch_jar_class;
use classfile_parser::patch_jar_method;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn read_class_bytes(name: &str) -> Vec<u8> {
    let path = format!("java-assets/compiled-classes/{name}");
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
}

fn build_jar(entries: &[(&str, &[u8])]) -> JarFile {
    use std::io::Cursor;
    use zip::CompressionMethod;
    use zip::write::SimpleFileOptions;

    let mut buf = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buf);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, data) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
    }
    JarFile::from_bytes(&buf.into_inner()).unwrap()
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

fn compile_java(
    test_name: &str,
    java_src: &str,
    class_name: &str,
) -> (std::path::PathBuf, Vec<u8>) {
    let tmp_dir = std::env::temp_dir().join(format!("classfile_jar_patch_{test_name}"));
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).unwrap();

    let javac = Command::new("javac")
        .arg("-d")
        .arg(&tmp_dir)
        .arg(java_src)
        .output()
        .expect("failed to run javac");
    assert!(
        javac.status.success(),
        "javac failed: {}",
        String::from_utf8_lossy(&javac.stderr)
    );

    let class_path = tmp_dir.join(format!("{class_name}.class"));
    let class_bytes = fs::read(&class_path).unwrap();
    (tmp_dir, class_bytes)
}

fn run_jar(jar: &JarFile, class_name: &str, test_name: &str) -> String {
    let tmp_dir = std::env::temp_dir().join(format!("classfile_jar_patch_run_{test_name}"));
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).unwrap();

    let jar_path = tmp_dir.join("test.jar");
    jar.save(&jar_path).unwrap();

    let run = Command::new("java")
        .arg("-cp")
        .arg(&jar_path)
        .arg(class_name)
        .output()
        .expect("failed to run java");
    let _ = fs::remove_dir_all(&tmp_dir);
    assert!(
        run.status.success(),
        "java failed (exit {}): stderr={}",
        run.status,
        String::from_utf8_lossy(&run.stderr)
    );
    String::from_utf8_lossy(&run.stdout).trim().to_string()
}

// ---------------------------------------------------------------------------
// Error case tests
// ---------------------------------------------------------------------------

#[test]
fn test_patch_jar_method_class_not_found() {
    let class_bytes = read_class_bytes("HelloWorld.class");
    let mut jar = build_jar(&[("HelloWorld.class", &class_bytes)]);

    let result = jar_patch::patch_jar_method(
        &mut jar,
        "DoesNotExist.class",
        "main",
        r#"{ return; }"#,
        &CompileOptions::default(),
    );
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), JarPatchError::Jar(_)));
}

#[test]
fn test_patch_jar_method_method_not_found() {
    let class_bytes = read_class_bytes("HelloWorld.class");
    let mut jar = build_jar(&[("HelloWorld.class", &class_bytes)]);

    let result = jar_patch::patch_jar_method(
        &mut jar,
        "HelloWorld.class",
        "doesNotExist",
        r#"{ return; }"#,
        &CompileOptions::default(),
    );
    assert!(result.is_err());
    match result.unwrap_err() {
        JarPatchError::Compile(classfile_parser::compile::CompileError::MethodNotFound {
            name,
        }) => {
            assert_eq!(name, "doesNotExist");
        }
        other => panic!("expected MethodNotFound, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Function tests
// ---------------------------------------------------------------------------

#[test]
fn test_patch_jar_method_basic() {
    let class_bytes = read_class_bytes("HelloWorld.class");
    let mut jar = build_jar(&[("HelloWorld.class", &class_bytes)]);

    // Patch should succeed — we're replacing main with a simple body
    jar_patch::patch_jar_method(
        &mut jar,
        "HelloWorld.class",
        "main",
        r#"{ System.out.println("patched"); }"#,
        &CompileOptions::default(),
    )
    .unwrap();

    // The class entry should be updated
    let updated_bytes = jar.get_entry("HelloWorld.class").unwrap();
    assert_ne!(
        updated_bytes,
        &class_bytes[..],
        "class bytes should differ after patching"
    );
}

#[test]
fn test_patch_jar_class_multiple_methods() {
    let class_bytes = read_class_bytes("HelloWorld.class");
    let mut jar = build_jar(&[("HelloWorld.class", &class_bytes)]);

    jar_patch::patch_jar_class(
        &mut jar,
        "HelloWorld.class",
        &[("main", r#"{ System.out.println("one"); }"#)],
        &CompileOptions::default(),
    )
    .unwrap();

    let updated_bytes = jar.get_entry("HelloWorld.class").unwrap();
    assert_ne!(updated_bytes, &class_bytes[..]);
}

// ---------------------------------------------------------------------------
// Macro syntax tests
// ---------------------------------------------------------------------------

#[test]
fn test_macro_patch_jar_method() {
    let class_bytes = read_class_bytes("HelloWorld.class");
    let mut jar = build_jar(&[("HelloWorld.class", &class_bytes)]);

    patch_jar_method!(jar, "HelloWorld.class", "main", r#"{ return; }"#).unwrap();

    let updated = jar.get_entry("HelloWorld.class").unwrap();
    assert_ne!(updated, &class_bytes[..]);
}

#[test]
fn test_macro_patch_jar_method_no_verify() {
    let class_bytes = read_class_bytes("HelloWorld.class");
    let mut jar = build_jar(&[("HelloWorld.class", &class_bytes)]);

    patch_jar_method!(jar, "HelloWorld.class", "main", r#"{ return; }"#, no_verify).unwrap();
}

#[test]
fn test_macro_patch_jar_class() {
    let class_bytes = read_class_bytes("HelloWorld.class");
    let mut jar = build_jar(&[("HelloWorld.class", &class_bytes)]);

    patch_jar_class!(jar, "HelloWorld.class", {
        "main" => r#"{ return; }"#,
    })
    .unwrap();
}

#[test]
fn test_macro_patch_jar_class_no_verify() {
    let class_bytes = read_class_bytes("HelloWorld.class");
    let mut jar = build_jar(&[("HelloWorld.class", &class_bytes)]);

    patch_jar_class!(jar, "HelloWorld.class", no_verify, {
        "main" => r#"{ return; }"#,
    })
    .unwrap();
}

#[test]
fn test_macro_patch_jar_multi_class() {
    let hello_bytes = read_class_bytes("HelloWorld.class");
    let basic_bytes = read_class_bytes("BasicClass.class");
    let mut jar = build_jar(&[
        ("HelloWorld.class", &hello_bytes),
        ("BasicClass.class", &basic_bytes),
    ]);

    patch_jar!(jar, {
        "HelloWorld.class" => {
            "main" => r#"{ return; }"#,
        },
    })
    .unwrap();
}

#[test]
fn test_macro_patch_jar_multi_class_no_verify() {
    let hello_bytes = read_class_bytes("HelloWorld.class");
    let mut jar = build_jar(&[("HelloWorld.class", &hello_bytes)]);

    patch_jar!(jar, no_verify, {
        "HelloWorld.class" => {
            "main" => r#"{ return; }"#,
        },
    })
    .unwrap();
}

// ---------------------------------------------------------------------------
// E2E tests (require javac + java)
// ---------------------------------------------------------------------------

#[test]
fn test_e2e_patch_jar_hello_world() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (_tmp, class_bytes) = compile_java(
        "e2e_jar_hello",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    let mut jar = JarFile::new();
    jar.set_entry("HelloWorld.class", class_bytes);

    patch_jar_method!(
        jar,
        "HelloWorld.class",
        "main",
        r#"{
        System.out.println("jar-patched!");
    }"#
    )
    .unwrap();

    let output = run_jar(&jar, "HelloWorld", "e2e_jar_hello");
    assert_eq!(output, "jar-patched!");
}

#[test]
fn test_e2e_patch_jar_multi_method() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (_tmp, class_bytes) = compile_java(
        "e2e_jar_multi",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    let mut jar = JarFile::new();
    jar.set_entry("HelloWorld.class", class_bytes);

    patch_jar_class!(jar, "HelloWorld.class", {
        "main" => r#"{
            System.out.println("multi-patched");
        }"#,
    })
    .unwrap();

    let output = run_jar(&jar, "HelloWorld", "e2e_jar_multi");
    assert_eq!(output, "multi-patched");
}

#[test]
fn test_e2e_patch_jar_macro() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (_tmp, class_bytes) = compile_java(
        "e2e_jar_macro",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    let mut jar = JarFile::new();
    jar.set_entry("HelloWorld.class", class_bytes);

    patch_jar!(jar, {
        "HelloWorld.class" => {
            "main" => r#"{
                System.out.println("full-macro");
            }"#,
        },
    })
    .unwrap();

    let output = run_jar(&jar, "HelloWorld", "e2e_jar_macro");
    assert_eq!(output, "full-macro");
}

#[test]
fn test_e2e_patch_jar_save_and_load() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (_tmp, class_bytes) = compile_java(
        "e2e_jar_save",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    let mut jar = JarFile::new();
    jar.set_entry("HelloWorld.class", class_bytes);

    patch_jar_method!(
        jar,
        "HelloWorld.class",
        "main",
        r#"{
        System.out.println("saved-and-loaded");
    }"#
    )
    .unwrap();

    // Save to disk, re-open, verify contents survived round-trip
    let tmp_dir = std::env::temp_dir().join("classfile_jar_patch_save_test");
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).unwrap();
    let jar_path = tmp_dir.join("test.jar");

    jar.save(&jar_path).unwrap();
    let reloaded = JarFile::open(&jar_path).unwrap();

    let output = run_jar(&reloaded, "HelloWorld", "e2e_jar_save");
    assert_eq!(output, "saved-and-loaded");

    let _ = fs::remove_dir_all(&tmp_dir);
}

// ---------------------------------------------------------------------------
// Stress tests: JAR patching with complex method bodies
// ---------------------------------------------------------------------------

/// Jar patch with array operations, for-each, and string concat
#[test]
fn test_stress_jar_patch_complex_body() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (_tmp, class_bytes) = compile_java(
        "stress_jar_complex",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    let mut jar = JarFile::new();
    jar.set_entry("HelloWorld.class", class_bytes);

    patch_jar_method!(
        jar,
        "HelloWorld.class",
        "main",
        r#"{
        int[] arr = new int[10];
        for (int i = 0; i < 10; i++) {
            arr[i] = i * i;
        }
        int sum = 0;
        for (int x : arr) {
            sum = sum + x;
        }
        System.out.println("sum=" + sum);
    }"#
    )
    .unwrap();

    let output = run_jar(&jar, "HelloWorld", "stress_jar_complex");
    assert_eq!(output, "sum=285");
}

/// Jar patch with try-catch-finally
#[test]
fn test_stress_jar_patch_try_catch() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (_tmp, class_bytes) = compile_java(
        "stress_jar_trycatch",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    let mut jar = JarFile::new();
    jar.set_entry("HelloWorld.class", class_bytes);

    patch_jar_method!(
        jar,
        "HelloWorld.class",
        "main",
        r#"{
        try {
            System.out.println("before");
            throw new RuntimeException("test");
        } catch (RuntimeException e) {
            System.out.println("caught");
        } finally {
            System.out.println("finally");
        }
    }"#
    )
    .unwrap();

    let output = run_jar(&jar, "HelloWorld", "stress_jar_trycatch");
    assert_eq!(output, "before\ncaught\nfinally");
}

/// Jar patch with switch expression and var keyword
#[test]
fn test_stress_jar_patch_modern_java() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (_tmp, class_bytes) = compile_java(
        "stress_jar_modern",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    let mut jar = JarFile::new();
    jar.set_entry("HelloWorld.class", class_bytes);

    patch_jar_method!(
        jar,
        "HelloWorld.class",
        "main",
        r#"{
        var x = 3;
        var result = switch (x) {
            case 1 -> "one";
            case 2 -> "two";
            case 3 -> "three";
            default -> "other";
        };
        System.out.println(result);
    }"#
    )
    .unwrap();

    let output = run_jar(&jar, "HelloWorld", "stress_jar_modern");
    assert_eq!(output, "three");
}

/// Jar patch with bubble sort algorithm
#[test]
fn test_stress_jar_patch_bubble_sort() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (_tmp, class_bytes) = compile_java(
        "stress_jar_sort",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    let mut jar = JarFile::new();
    jar.set_entry("HelloWorld.class", class_bytes);

    patch_jar_method!(
        jar,
        "HelloWorld.class",
        "main",
        r#"{
        int[] arr = new int[5];
        arr[0] = 42;
        arr[1] = 17;
        arr[2] = 99;
        arr[3] = 3;
        arr[4] = 55;
        for (int i = 0; i < 5; i++) {
            for (int j = 0; j < 4 - i; j++) {
                if (arr[j] > arr[j + 1]) {
                    int temp = arr[j];
                    arr[j] = arr[j + 1];
                    arr[j + 1] = temp;
                }
            }
        }
        for (int x : arr) {
            System.out.println(x);
        }
    }"#
    )
    .unwrap();

    let output = run_jar(&jar, "HelloWorld", "stress_jar_sort");
    assert_eq!(output, "3\n17\n42\n55\n99");
}

/// Jar patch with synchronized + exception handling
#[test]
fn test_stress_jar_patch_sync_trycatch() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (_tmp, class_bytes) = compile_java(
        "stress_jar_sync_try",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    let mut jar = JarFile::new();
    jar.set_entry("HelloWorld.class", class_bytes);

    patch_jar_method!(
        jar,
        "HelloWorld.class",
        "main",
        r#"{
        Object lock = new Object();
        synchronized (lock) {
            try {
                System.out.println("in-sync-try");
            } catch (Exception e) {
                System.out.println("error");
            } finally {
                System.out.println("in-sync-finally");
            }
        }
    }"#
    )
    .unwrap();

    let output = run_jar(&jar, "HelloWorld", "stress_jar_sync_try");
    assert_eq!(output, "in-sync-try\nin-sync-finally");
}

/// Jar patch with multi-dimensional arrays
#[test]
fn test_stress_jar_patch_multi_dim() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (_tmp, class_bytes) = compile_java(
        "stress_jar_multidim",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    let mut jar = JarFile::new();
    jar.set_entry("HelloWorld.class", class_bytes);

    patch_jar_method!(
        jar,
        "HelloWorld.class",
        "main",
        r#"{
        int[][] grid = new int[3][3];
        int v = 1;
        for (int i = 0; i < 3; i++) {
            for (int j = 0; j < 3; j++) {
                grid[i][j] = v;
                v++;
            }
        }
        int sum = grid[0][0] + grid[1][1] + grid[2][2];
        System.out.println(sum);
    }"#
    )
    .unwrap();

    let output = run_jar(&jar, "HelloWorld", "stress_jar_multidim");
    // Trace: 1 + 5 + 9 = 15
    assert_eq!(output, "15");
}
