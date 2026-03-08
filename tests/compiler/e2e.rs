use super::*;

// --- Codegen unit tests ---

#[test]
fn test_codegen_return_42() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (_, _, mut class_file) = compile_and_load(
        "codegen_ret42",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    let stmts = parse_method_body("{ return; }").unwrap();
    let generated =
        generate_bytecode(&stmts, &mut class_file, true, "([Ljava/lang/String;)V").unwrap();

    // Should contain Return instruction
    assert!(
        generated
            .instructions
            .iter()
            .any(|i| matches!(i, Instruction::Return))
    );
    assert!(generated.max_stack >= 1);
    assert!(generated.max_locals >= 1);
}

// --- Basic E2E tests ---

#[test]
fn test_compile_e2e_hello_compiled() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("e2e_hello", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{ System.out.println("Compiled!"); }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(
        output, "Compiled!",
        "expected 'Compiled!' but got: {}",
        output
    );
}

#[test]
fn test_compile_e2e_return_value() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_retval",
        "java-assets/src/SimpleMath.java",
        "SimpleMath",
    );

    // Replace intMath to return a different formula: a constant
    compile_method_body(
        r#"{ System.out.println(99); }"#,
        &mut class_file,
        "intMath",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "SimpleMath");
    assert!(output.contains("99"), "expected 99 in output: {}", output);
}

#[test]
fn test_compile_e2e_if_else() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_ifelse",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            int x = 10;
            if (x > 5) {
                System.out.println("big");
            } else {
                System.out.println("small");
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "big", "expected 'big' but got: {}", output);
}

#[test]
fn test_compile_e2e_while_loop() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("e2e_while", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int sum = 0;
            int i = 1;
            while (i <= 10) {
                sum = sum + i;
                i = i + 1;
            }
            System.out.println(sum);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "55", "expected '55' but got: {}", output);
}

#[test]
fn test_compile_e2e_for_loop() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("e2e_for", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int sum = 0;
            for (int i = 1; i <= 5; i = i + 1) {
                sum = sum + i;
            }
            System.out.println(sum);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "15", "expected '15' but got: {}", output);
}

#[test]
fn test_compile_e2e_arithmetic() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("e2e_arith", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int a = 10;
            int b = 3;
            int c = a * b + a / b - a % b;
            System.out.println(c);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    // 10*3 + 10/3 - 10%3 = 30 + 3 - 1 = 32
    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "32", "expected '32' but got: {}", output);
}

#[test]
fn test_compile_e2e_nested_if() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_nested_if",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            int x = 15;
            if (x > 10) {
                if (x > 20) {
                    System.out.println("very big");
                } else {
                    System.out.println("medium");
                }
            } else {
                System.out.println("small");
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "medium", "expected 'medium' but got: {}", output);
}

// --- Switch E2E tests ---

#[test]
fn test_compile_e2e_switch() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_switch",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            int x = 2;
            switch (x) {
                case 1:
                    System.out.println("one");
                    break;
                case 2:
                    System.out.println("two");
                    break;
                case 3:
                    System.out.println("three");
                    break;
                default:
                    System.out.println("other");
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "two", "expected 'two' but got: {}", output);
}

#[test]
fn test_compile_e2e_switch_default() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_switch_default",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            int x = 99;
            switch (x) {
                case 1:
                    System.out.println("one");
                    break;
                case 2:
                    System.out.println("two");
                    break;
                default:
                    System.out.println("default");
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "default", "expected 'default' but got: {}", output);
}

// --- Try-catch E2E tests ---

#[test]
fn test_compile_e2e_try_catch() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_try_catch",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            try {
                throw new RuntimeException("boom");
            } catch (RuntimeException e) {
                System.out.println("caught");
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "caught", "expected 'caught' but got: {}", output);
}

#[test]
fn test_compile_e2e_try_finally() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_try_finally",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            try {
                System.out.println("try");
            } finally {
                System.out.println("finally");
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(
        output, "try\nfinally",
        "expected 'try\\nfinally' but got: {}",
        output
    );
}

// --- StackMapTable generation tests ---

#[test]
fn test_compile_e2e_stackmap_if_else() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_smt_ifelse",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            int x = 10;
            if (x > 5) {
                System.out.println("big");
            } else {
                System.out.println("small");
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "big", "expected 'big' but got: {}", output);
}

#[test]
fn test_compile_e2e_stackmap_while() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_smt_while",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            int sum = 0;
            int i = 1;
            while (i <= 10) {
                sum = sum + i;
                i = i + 1;
            }
            System.out.println(sum);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "55", "expected '55' but got: {}", output);
}

#[test]
fn test_compile_e2e_stackmap_try_catch() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_smt_trycatch",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            try {
                throw new RuntimeException("boom");
            } catch (RuntimeException e) {
                System.out.println("caught");
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "caught", "expected 'caught' but got: {}", output);
}

// --- Typed arithmetic tests ---

#[test]
fn test_compile_e2e_long_arithmetic() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_long_arith",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            long a = 1000000000L;
            long b = 2000000000L;
            long c = a + b;
            System.out.println(c);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "3000000000");
}

#[test]
fn test_compile_e2e_double_arithmetic() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_double_arith",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            double a = 1.5;
            double b = 2.5;
            System.out.println(a + b);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "4.0");
}

#[test]
fn test_compile_e2e_float_arithmetic() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_float_arith",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            float a = 1.5f;
            float b = 2.5f;
            System.out.println(a * b);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "3.75");
}

#[test]
fn test_compile_e2e_widening() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_widening",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            int a = 10;
            long b = 20L;
            long c = a + b;
            System.out.println(c);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "30");
}

#[test]
fn test_compile_e2e_long_comparison() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_long_cmp",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            long x = 5L;
            if (x > 3L) {
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
    assert_eq!(output, "yes");
}

#[test]
fn test_compile_e2e_cast_types() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_cast_types",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            double d = 3.14;
            int i = (int) d;
            System.out.println(i);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "3");
}

#[test]
fn test_compile_e2e_unary_neg_long() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_neg_long",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            long x = 10L;
            System.out.println(-x);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "-10");
}

// --- String concatenation tests ---

#[test]
fn test_compile_e2e_string_concat() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_str_concat",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            System.out.println("hello" + " " + "world");
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "hello world");
}

#[test]
fn test_compile_e2e_string_concat_int() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_str_concat_int",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            int n = 42;
            String s = "n=" + n;
            System.out.println(s);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "n=42");
}

#[test]
fn test_compile_e2e_string_concat_chain() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_str_concat_chain",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            String s = "a" + "b" + "c" + "d";
            System.out.println(s);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "abcd");
}

// --- Typed array tests ---

#[test]
fn test_compile_e2e_typed_array_long() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_arr_long",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            long[] arr = new long[2];
            arr[0] = 100L;
            arr[1] = 200L;
            System.out.println(arr[0] + arr[1]);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "300");
}

#[test]
fn test_compile_e2e_typed_array_double() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_arr_double",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            double[] arr = new double[2];
            arr[0] = 1.5;
            arr[1] = 2.5;
            System.out.println(arr[0] + arr[1]);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "4.0");
}

// --- For-each tests ---

#[test]
fn test_compile_e2e_foreach_array() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_foreach",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            int[] arr = new int[3];
            arr[0] = 10;
            arr[1] = 20;
            arr[2] = 30;
            int sum = 0;
            for (int x : arr) {
                sum = sum + x;
            }
            System.out.println(sum);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "60");
}

// --- P1 E2E tests ---

#[test]
fn test_compile_e2e_multi_catch() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_multi_catch",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            try {
                throw new RuntimeException("boom");
            } catch (IllegalArgumentException | RuntimeException e) {
                System.out.println("caught multi");
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(
        output, "caught multi",
        "expected 'caught multi' but got: {}",
        output
    );
}

#[test]
fn test_compile_e2e_synchronized() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_synchronized",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            Object lock = new Object();
            synchronized (lock) {
                System.out.println("locked");
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "locked", "expected 'locked' but got: {}", output);
}

// --- P2 E2E tests ---

#[test]
fn test_compile_e2e_var() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("e2e_var", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            var x = 10;
            var s = "hello";
            System.out.println(x);
            System.out.println(s);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(
        output, "10\nhello",
        "expected '10\\nhello' but got: {}",
        output
    );
}

#[test]
fn test_compile_e2e_var_long() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_var_long",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            var x = 100L;
            System.out.println(x);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "100", "expected '100' but got: {}", output);
}

#[test]
fn test_compile_e2e_multi_dim_array() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_multi_dim",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            int[][] arr = new int[2][3];
            arr[0][0] = 42;
            arr[1][2] = 99;
            System.out.println(arr[0][0]);
            System.out.println(arr[1][2]);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "42\n99", "expected '42\\n99' but got: {}", output);
}

#[test]
fn test_compile_e2e_switch_expr() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_switch_expr",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            int x = 2;
            int r = switch (x) {
                case 1 -> 10;
                case 2 -> 20;
                case 3 -> 30;
                default -> 0;
            };
            System.out.println(r);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "20", "expected '20' but got: {}", output);
}

#[test]
fn test_compile_e2e_switch_expr_default() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_switch_expr_default",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            int x = 99;
            int r = switch (x) {
                case 1 -> 10;
                default -> 0;
            };
            System.out.println(r);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "0", "expected '0' but got: {}", output);
}

// --- Additional E2E tests ---

/// Test: Switch fall-through — case 2 executes case 2 and case 3 bodies (no break between).
#[test]
fn test_compile_e2e_switch_fallthrough() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_switch_fallthrough",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            int x = 2;
            int acc = 0;
            switch (x) {
                case 1:
                    acc = acc + 1;
                case 2:
                    acc = acc + 10;
                case 3:
                    acc = acc + 100;
                    break;
                default:
                    acc = -1;
            }
            System.out.println(acc);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "110", "expected '110' but got: {}", output);
}

/// Test: For-each over an Iterable (java.util.List) via invokeinterface iterator/hasNext/next.
#[test]
fn test_compile_e2e_foreach_list() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_foreach_list",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            java.util.List<String> list = new java.util.ArrayList<>();
            list.add("alpha");
            list.add("beta");
            list.add("gamma");
            for (String s : list) {
                System.out.println(s);
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(
        output, "alpha\nbeta\ngamma",
        "expected three lines but got: {}",
        output
    );
}

/// Test: Null concatenation — null references stringify to "null" in String concatenation.
#[test]
fn test_compile_e2e_null_concatenation() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_null_concat",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            String s = null;
            String result = "value=" + s;
            System.out.println(result);
            System.out.println("literal=" + null);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(
        output, "value=null\nliteral=null",
        "expected null concat output but got: {}",
        output
    );
}

/// Test: Multi-catch second type — verifies both types in a multi-catch are matched.
#[test]
fn test_compile_e2e_multicatch_second_type() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_multicatch_second",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            try {
                throw new NullPointerException("npe");
            } catch (IllegalArgumentException | NullPointerException e) {
                System.out.println("caught: " + e.getClass().getSimpleName());
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(
        output, "caught: NullPointerException",
        "expected multi-catch output but got: {}",
        output
    );
}

/// Test: Try-catch in loop with continue — catch handler executes continue to the loop.
#[test]
fn test_compile_e2e_trycatch_in_loop_continue() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_trycatch_loop",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            int count = 0;
            for (int i = 0; i < 5; i++) {
                try {
                    if (i == 2) throw new RuntimeException("skip");
                    count = count + 1;
                } catch (RuntimeException e) {
                    continue;
                }
            }
            System.out.println(count);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "4", "expected '4' but got: {}", output);
}

/// Test: Synchronized block with exception — verifies monitorexit on exception path.
#[test]
fn test_compile_e2e_synchronized_exception_path() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_sync_exception",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            Object lock = new Object();
            try {
                synchronized (lock) {
                    throw new RuntimeException("inside sync");
                }
            } catch (RuntimeException e) {
                System.out.println("caught after sync: " + e.getMessage());
            }
            System.out.println("done");
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(
        output, "caught after sync: inside sync\ndone",
        "expected sync exception output but got: {}",
        output
    );
}

/// Test: Int overflow wraps — Java int arithmetic wraps at 32-bit boundaries.
#[test]
fn test_compile_e2e_int_overflow_wraps() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_int_overflow",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            int max = 2147483647;
            int overflow = max + 1;
            System.out.println(overflow);
            int min = -2147483648;
            int underflow = min - 1;
            System.out.println(underflow);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(
        output, "-2147483648\n2147483647",
        "expected overflow values but got: {}",
        output
    );
}

/// Test: Narrowing casts — long-to-byte, long-to-short, double-to-float.
#[test]
fn test_compile_e2e_narrowing_casts() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_narrowing",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            long big = 511L;
            byte b = (byte) big;
            System.out.println(b);
            long val = 40000L;
            short s = (short) val;
            System.out.println(s);
            double d = 1.23456789;
            float f = (float) d;
            System.out.println(f);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(
        output, "-1\n-25536\n1.2345679",
        "expected narrowing cast output but got: {}",
        output
    );
}

/// Test: Ternary side effect isolation — only the chosen branch's side effect executes.
#[test]
fn test_compile_e2e_ternary_side_effect() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_ternary_side",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            int[] counter = new int[1];
            counter[0] = 0;
            int x = 1;
            int result = (x > 0) ? (counter[0] = counter[0] + 10) : (counter[0] = counter[0] - 1);
            System.out.println(result);
            System.out.println(counter[0]);
            x = -1;
            result = (x > 0) ? (counter[0] = counter[0] + 10) : (counter[0] = counter[0] - 1);
            System.out.println(result);
            System.out.println(counter[0]);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(
        output, "10\n10\n9\n9",
        "expected ternary side effect output but got: {}",
        output
    );
}

/// Test: Boolean local from comparison — stores comparison result in a local variable.
#[test]
fn test_compile_e2e_bool_from_comparison() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_bool_cmp",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            int x = 7;
            boolean isOdd = (x % 2) != 0;
            boolean isPositive = x > 0;
            if (isOdd && isPositive) {
                System.out.println("odd and positive");
            }
            boolean b = false;
            b = true;
            System.out.println(b);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(
        output, "odd and positive\ntrue",
        "expected boolean comparison output but got: {}",
        output
    );
}

/// Test: Var inferred from expression — `var` types resolved across widening chains.
#[test]
fn test_compile_e2e_var_inferred_type() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_var_infer",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            int a = 10;
            long b = 20L;
            var sumLong = a + b;
            var doubled = sumLong * 2L;
            System.out.println(doubled);
            double d = 1.5;
            var product = sumLong * d;
            System.out.println(product);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(
        output, "60\n45.0",
        "expected var inferred type output but got: {}",
        output
    );
}

/// Test: Zero-length array and length field access.
#[test]
fn test_compile_e2e_array_length() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) = compile_and_load(
        "e2e_array_len",
        "java-assets/src/HelloWorld.java",
        "HelloWorld",
    );

    compile_method_body(
        r#"{
            int[] empty = new int[0];
            System.out.println(empty.length);
            String[] strs = new String[3];
            strs[0] = "a";
            strs[1] = "b";
            strs[2] = "c";
            if (strs.length == 3) {
                System.out.println("correct length");
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(
        output, "0\ncorrect length",
        "expected array length output but got: {}",
        output
    );
}
