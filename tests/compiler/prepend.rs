use super::*;

// --- Prepend mode tests ---

#[test]
fn test_prepend_println() {
    if !java_available() {
        eprintln!("SKIP: java/javac not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("prepend_println", "java-assets/src/PrependTest.java", "PrependTest");

    // Replace main to just print "original", then prepend "before"
    compile_method_body(
        r#"{ System.out.println("original"); }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    prepend_method_body(
        r#"{ System.out.println("before"); }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "PrependTest");
    assert_eq!(output, "before\noriginal");
}

#[test]
fn test_prepend_with_param_access() {
    if !java_available() {
        eprintln!("SKIP: java/javac not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("prepend_param", "java-assets/src/PrependTest.java", "PrependTest");

    // Prepend code that prints the parameter before original body runs
    prepend_method_body(
        r#"{ System.out.println(arg0); }"#,
        &mut class_file,
        "withParams",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    // Patch main to call withParams
    compile_method_body(
        r#"{ PrependTest.withParams("world"); }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "PrependTest");
    assert_eq!(output, "world\nhello world");
}

#[test]
fn test_prepend_with_local_variable() {
    if !java_available() {
        eprintln!("SKIP: java/javac not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("prepend_local", "java-assets/src/PrependTest.java", "PrependTest");

    // Prepend code that declares a local variable
    prepend_method_body(
        r#"{ int y = 99; System.out.println("y=" + y); }"#,
        &mut class_file,
        "withLocal",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    // Patch main to call withLocal
    compile_method_body(
        r#"{ PrependTest.withLocal(); }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "PrependTest");
    assert_eq!(output, "y=99\nx=10");
}

#[test]
fn test_prepend_to_method_with_try_catch() {
    if !java_available() {
        eprintln!("SKIP: java/javac not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("prepend_trycatch", "java-assets/src/PrependTest.java", "PrependTest");

    prepend_method_body(
        r#"{ System.out.println("before try"); }"#,
        &mut class_file,
        "withTryCatch",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    // Patch main to call withTryCatch
    compile_method_body(
        r#"{ PrependTest.withTryCatch(); }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "PrependTest");
    assert_eq!(output, "before try\ntry");
}

#[test]
fn test_prepend_with_branches() {
    if !java_available() {
        eprintln!("SKIP: java/javac not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("prepend_branches", "java-assets/src/PrependTest.java", "PrependTest");

    // Prepend an if/else that has branch targets (requires StackMapTable merge)
    prepend_method_body(
        r#"{ if (arg0 > 0) { System.out.println("positive"); } else { System.out.println("non-positive"); } }"#,
        &mut class_file,
        "withBranch",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    // Patch main to call withBranch
    compile_method_body(
        r#"{ PrependTest.withBranch(5); }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "PrependTest");
    assert_eq!(output, "positive\nn=5");
}

#[test]
fn test_prepend_macro() {
    if !java_available() {
        eprintln!("SKIP: java/javac not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("prepend_macro", "java-assets/src/PrependTest.java", "PrependTest");

    // Replace main to just print "original", then prepend with macro
    compile_method_body(
        r#"{ System.out.println("original"); }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    classfile_parser::prepend_method!(
        class_file,
        "main",
        r#"{ System.out.println("macro prepend"); }"#
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "PrependTest");
    assert_eq!(output, "macro prepend\noriginal");
}

// --- StackMapTable edge case tests ---

/// Regression test: wide local (long) followed by non-wide local + branch.
/// Verifies StackMapTable encoding doesn't include explicit Top continuation slots.
#[test]
fn test_wide_local_then_narrow_with_branch() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("wide_narrow_branch", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            long x = 100L;
            int y = 42;
            if (y > 10) {
                System.out.println(x + y);
            } else {
                System.out.println(y);
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "142");
}

/// StackMapTable edge case: many statements before a branch pushes the frame delta past 63,
/// which should trigger SameFrameExtended instead of SameFrame.
#[test]
fn test_stackmap_large_delta_extended_frame() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("smt_extended", "java-assets/src/HelloWorld.java", "HelloWorld");

    // Generate enough bytecode before the branch to push the delta past 63.
    // Each println is ~8 bytes (getstatic 3 + ldc 2 + invokevirtual 3 = 8).
    // We need > 63 bytes before the if statement to force SameFrameExtended.
    compile_method_body(
        r#"{
            System.out.println("a");
            System.out.println("b");
            System.out.println("c");
            System.out.println("d");
            System.out.println("e");
            System.out.println("f");
            System.out.println("g");
            System.out.println("h");
            System.out.println("i");
            int x = 1;
            if (x > 0) {
                System.out.println("yes");
            } else {
                System.out.println("no");
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert!(output.ends_with("yes"), "expected 'yes' at end, got: {}", output);
}

/// StackMapTable edge case: prepending code that pushes existing frames past the
/// SameFrame threshold, verifying re-encoding from SameFrame to SameFrameExtended.
#[test]
fn test_prepend_stackmap_reencoding_threshold() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("smt_reenc", "java-assets/src/HelloWorld.java", "HelloWorld");

    // First replace with code that has a branch near the SameFrame limit
    compile_method_body(
        r#"{
            int x = 1;
            if (x > 0) {
                System.out.println("original-yes");
            } else {
                System.out.println("original-no");
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    // Now prepend enough code to push the existing frames past offset 63
    let mut opts = CompileOptions::default();
    opts.insert_mode = classfile_parser::compile::InsertMode::Prepend;
    compile_method_body(
        r#"{
            System.out.println("p1");
            System.out.println("p2");
            System.out.println("p3");
            System.out.println("p4");
            System.out.println("p5");
            System.out.println("p6");
            System.out.println("p7");
            System.out.println("p8");
        }"#,
        &mut class_file,
        "main",
        None,
        &opts,
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert!(
        output.ends_with("original-yes"),
        "expected 'original-yes' at end, got: {}",
        output
    );
}
