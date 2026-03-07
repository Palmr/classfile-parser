use super::*;

/// Like compile_and_load but passes `-g` to javac so LocalVariableTable is present.
#[allow(unused)]
fn compile_and_load_debug(
    test_name: &str,
    java_src: &str,
    class_name: &str,
) -> (std::path::PathBuf, std::path::PathBuf, ClassFile) {
    let tmp_dir = std::env::temp_dir().join(format!("classfile_compile_{}", test_name));
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).unwrap();

    let compile = Command::new("javac")
        .arg("-g")
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
    let class_file = ClassFile::read(&mut Cursor::new(&class_bytes)).expect("failed to parse class");

    (tmp_dir, class_path, class_file)
}

#[test]
fn test_param_access_positional_arg0() {
    if !java_available() {
        eprintln!("SKIP: java/javac not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("param_positional", "java-assets/src/ParamAccess.java", "ParamAccess");

    // main(String[] args): arg0 is args (String[])
    // Use arg0.length to verify array param access works
    compile_method_body(
        r#"{ System.out.println(arg0.length); }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "ParamAccess");
    assert_eq!(output, "0");
}

#[test]
fn test_param_access_debug_name() {
    if !java_available() {
        eprintln!("SKIP: java/javac not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load_debug("param_debug", "java-assets/src/ParamAccess.java", "ParamAccess");

    // With -g, the original parameter name "args" is available via LocalVariableTable
    compile_method_body(
        r#"{ System.out.println(args.length); }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "ParamAccess");
    assert_eq!(output, "0");
}

#[test]
fn test_param_access_wide_types() {
    if !java_available() {
        eprintln!("SKIP: java/javac not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("param_wide", "java-assets/src/ParamAccess.java", "ParamAccess");

    // wideParams(int a, long b, String c)
    // arg0 = int (slot 0, 1 wide), arg1 = long (slot 1, 2 wide), arg2 = String (slot 3)
    compile_method_body(
        r#"{ System.out.println(arg0); System.out.println(arg1); System.out.println(arg2); }"#,
        &mut class_file,
        "wideParams",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    // main calls wideParams — the original Java source already does not call it,
    // so we provide a main that calls it directly via invokestatic
    compile_method_body(
        r#"{ ParamAccess.wideParams(42, 123456789L, "hello"); }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "ParamAccess");
    assert_eq!(output, "42\n123456789\nhello");
}

#[test]
fn test_param_access_instance_method() {
    if !java_available() {
        eprintln!("SKIP: java/javac not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("param_instance", "java-assets/src/ParamAccess.java", "ParamAccess");

    // instanceMethod(String name): this = slot 0, arg0 = name (slot 1)
    compile_method_body(
        r#"{ System.out.println(arg0); }"#,
        &mut class_file,
        "instanceMethod",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    // Patch main to create instance and call instanceMethod
    compile_method_body(
        r#"{
            ParamAccess obj = new ParamAccess();
            obj.instanceMethod("world");
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "ParamAccess");
    assert_eq!(output, "world");
}
