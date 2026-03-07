use super::*;

// ---------------------------------------------------------------------------
// Category 1: Complex control flow
// ---------------------------------------------------------------------------

/// Nested loops with break/continue interacting across levels
#[test]
fn test_stress_nested_loop_break_continue() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_nested_break", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int result = 0;
            for (int i = 0; i < 10; i++) {
                if (i == 3) continue;
                if (i == 7) break;
                int j = 0;
                while (j < 5) {
                    if (j == 2) {
                        j++;
                        continue;
                    }
                    result = result + 1;
                    j++;
                }
                // i runs 0,1,2,4,5,6 (skip 3, break at 7) = 6 iterations
                // j runs 0,1,3,4 (skip 2) = 4 per outer = 24 total
            }
            System.out.println(result);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "24");
}

/// Deeply nested if-else chain (fizzbuzz)
#[test]
fn test_stress_fizzbuzz() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_fizzbuzz", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            for (int i = 1; i <= 20; i++) {
                if (i % 15 == 0) {
                    System.out.println("FizzBuzz");
                } else if (i % 3 == 0) {
                    System.out.println("Fizz");
                } else if (i % 5 == 0) {
                    System.out.println("Buzz");
                } else {
                    System.out.println(i);
                }
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    let expected = "1\n2\nFizz\n4\nBuzz\nFizz\n7\n8\nFizz\nBuzz\n11\nFizz\n13\n14\nFizzBuzz\n16\n17\nFizz\n19\nBuzz";
    assert_eq!(output, expected);
}

/// Switch with many sparse cases (triggers lookupswitch)
#[test]
fn test_stress_switch_lookup() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_switch_lookup", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int x = 500;
            switch (x) {
                case 1:
                    System.out.println("one");
                    break;
                case 100:
                    System.out.println("hundred");
                    break;
                case 500:
                    System.out.println("five-hundred");
                    break;
                case 9999:
                    System.out.println("nine-thousand");
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
    assert_eq!(output, "five-hundred");
}

/// Switch with dense cases (triggers tableswitch)
#[test]
fn test_stress_switch_table() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_switch_table", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int sum = 0;
            for (int i = 0; i < 8; i++) {
                switch (i) {
                    case 0: sum = sum + 1; break;
                    case 1: sum = sum + 2; break;
                    case 2: sum = sum + 4; break;
                    case 3: sum = sum + 8; break;
                    case 4: sum = sum + 16; break;
                    case 5: sum = sum + 32; break;
                    case 6: sum = sum + 64; break;
                    case 7: sum = sum + 128; break;
                    default: break;
                }
            }
            System.out.println(sum);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    // 1+2+4+8+16+32+64+128 = 255
    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "255");
}

// ---------------------------------------------------------------------------
// Category 2: Complex arithmetic and type mixing
// ---------------------------------------------------------------------------

/// Mixed-type arithmetic with widening conversions
#[test]
fn test_stress_mixed_type_arithmetic() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_mixed_arith", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int i = 10;
            long l = 20L;
            float f = 1.5f;
            double d = 2.5;

            // int + long = long (i2l widening)
            long sum1 = i + l;
            System.out.println(sum1);

            // int + float = float (i2f widening)
            float sum2 = i + f;
            System.out.println(sum2);

            // long + double = double (l2d widening)
            double sum3 = l + d;
            System.out.println(sum3);

            // int * double = double (i2d widening)
            double prod = i * d;
            System.out.println(prod);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "30\n11.5\n22.5\n25.0");
}

/// Compound assignment with different types
#[test]
fn test_stress_compound_assign_types() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_compound_types", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int a = 100;
            a += 50;
            a -= 25;
            a *= 2;
            a /= 5;
            a %= 7;
            System.out.println(a);

            long b = 1000000000L;
            b += 2000000000L;
            System.out.println(b);

            double c = 10.0;
            c *= 3.14;
            System.out.println(c);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    // a: (100+50-25)*2/5 = 250/5 = 50; 50%7 = 1
    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "1\n3000000000\n31.400000000000002");
}

/// Bitwise operations
#[test]
fn test_stress_bitwise_ops() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_bitwise", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int a = 0xFF;
            int b = 0x0F;
            System.out.println(a & b);
            System.out.println(a | b);
            System.out.println(a ^ b);
            System.out.println(~b);

            // Shift operations
            int c = 1;
            System.out.println(c << 10);
            System.out.println(1024 >> 3);
            System.out.println(-1 >>> 28);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    // 0xFF & 0x0F = 15, 0xFF | 0x0F = 255, 0xFF ^ 0x0F = 240, ~0x0F = -16
    // 1 << 10 = 1024, 1024 >> 3 = 128, -1 >>> 28 = 15
    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "15\n255\n240\n-16\n1024\n128\n15");
}

/// Cast chain: int -> long -> double -> int
#[test]
fn test_stress_cast_chain() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_cast_chain", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int a = 42;
            long b = (long) a;
            double c = (double) b;
            float f = (float) c;
            int d = (int) f;
            System.out.println(d);

            // Truncation: double -> int
            double pi = 3.14159;
            int truncated = (int) pi;
            System.out.println(truncated);

            // Large long -> int truncation
            long big = 3000000000L;
            int small = (int) big;
            System.out.println(small);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    // 42 survives round-trip, pi truncates to 3, 3000000000 wraps to -1294967296
    assert_eq!(output, "42\n3\n-1294967296");
}

/// Pre/post increment/decrement combinations
#[test]
fn test_stress_increment_decrement() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_incdec", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int a = 10;
            a++;
            ++a;
            System.out.println(a);

            a--;
            --a;
            System.out.println(a);

            // For loop with increment
            int sum = 0;
            for (int i = 0; i < 5; i++) {
                sum += i;
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
    assert_eq!(output, "12\n10\n10");
}

// ---------------------------------------------------------------------------
// Category 3: String operations
// ---------------------------------------------------------------------------

/// String concat with all primitive types
#[test]
fn test_stress_string_concat_all_types() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_str_all", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int i = 42;
            long l = 123456789L;
            float f = 3.14f;
            double d = 2.718;
            boolean b = true;
            char c = 'X';
            String s = "i=" + i + " l=" + l + " f=" + f + " d=" + d + " b=" + b + " c=" + c;
            System.out.println(s);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "i=42 l=123456789 f=3.14 d=2.718 b=true c=X");
}

/// String concat in loop (builds string progressively)
#[test]
fn test_stress_string_concat_loop() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_str_loop", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            String s = "";
            for (int i = 0; i < 5; i++) {
                s = s + i;
            }
            System.out.println(s);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "01234");
}

// ---------------------------------------------------------------------------
// Category 4: Arrays
// ---------------------------------------------------------------------------

/// Array operations: create, fill, read, foreach
#[test]
fn test_stress_array_comprehensive() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_arr_comp", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int[] arr = new int[10];
            for (int i = 0; i < 10; i++) {
                arr[i] = i * i;
            }
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

    // 0+1+4+9+16+25+36+49+64+81 = 285
    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "285");
}

/// Multi-dimensional array: 3x3 matrix multiplication-ish
#[test]
fn test_stress_multi_dim_array_compute() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_mdarray", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int[][] m = new int[3][3];
            int val = 1;
            for (int i = 0; i < 3; i++) {
                for (int j = 0; j < 3; j++) {
                    m[i][j] = val;
                    val++;
                }
            }
            // Compute trace (diagonal sum)
            int trace = m[0][0] + m[1][1] + m[2][2];
            System.out.println(trace);

            // Print all values
            for (int i = 0; i < 3; i++) {
                for (int j = 0; j < 3; j++) {
                    System.out.println(m[i][j]);
                }
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    // Matrix: [[1,2,3],[4,5,6],[7,8,9]], trace = 1+5+9 = 15
    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "15\n1\n2\n3\n4\n5\n6\n7\n8\n9");
}

/// Typed arrays: long[], double[], boolean[], char[]
#[test]
fn test_stress_typed_arrays() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_typed_arr", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            long[] longs = new long[2];
            longs[0] = 9999999999L;
            longs[1] = 1L;
            System.out.println(longs[0] + longs[1]);

            double[] doubles = new double[2];
            doubles[0] = 1.1;
            doubles[1] = 2.2;
            System.out.println(doubles[0] + doubles[1]);

            boolean[] bools = new boolean[2];
            bools[0] = true;
            bools[1] = false;
            System.out.println(bools[0]);
            System.out.println(bools[1]);

            char[] chars = new char[3];
            chars[0] = 'H';
            chars[1] = 'i';
            chars[2] = '!';
            System.out.println("" + chars[0] + chars[1] + chars[2]);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "10000000000\n3.3000000000000003\ntrue\nfalse\nHi!");
}

// ---------------------------------------------------------------------------
// Category 5: Exception handling
// ---------------------------------------------------------------------------

/// Try-catch-finally with exception in different places
#[test]
fn test_stress_try_catch_complex() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_trycatch", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            // Test 1: exception caught, finally runs
            try {
                System.out.println("try1");
                throw new RuntimeException("err");
            } catch (RuntimeException e) {
                System.out.println("catch1");
            } finally {
                System.out.println("finally1");
            }

            // Test 2: no exception, finally still runs
            try {
                System.out.println("try2");
            } catch (RuntimeException e) {
                System.out.println("catch2-WRONG");
            } finally {
                System.out.println("finally2");
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "try1\ncatch1\nfinally1\ntry2\nfinally2");
}

/// Multi-catch with multiple exception types
#[test]
fn test_stress_multi_catch_variants() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_multi_catch", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            // First, throw IllegalArgumentException
            try {
                throw new IllegalArgumentException("bad arg");
            } catch (IllegalArgumentException | NullPointerException e) {
                System.out.println("caught-multi-1");
            }

            // Then throw NullPointerException
            try {
                throw new NullPointerException("null");
            } catch (IllegalArgumentException | NullPointerException e) {
                System.out.println("caught-multi-2");
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "caught-multi-1\ncaught-multi-2");
}

/// Nested try-catch
#[test]
fn test_stress_nested_try_catch() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_nested_try", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            try {
                System.out.println("outer-try");
                try {
                    System.out.println("inner-try");
                    throw new RuntimeException("inner");
                } catch (RuntimeException e) {
                    System.out.println("inner-catch");
                }
                System.out.println("after-inner");
            } catch (Exception e) {
                System.out.println("outer-catch-WRONG");
            } finally {
                System.out.println("outer-finally");
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "outer-try\ninner-try\ninner-catch\nafter-inner\nouter-finally");
}

// ---------------------------------------------------------------------------
// Category 6: Ternary and logical operators
// ---------------------------------------------------------------------------

/// Nested ternary expressions
#[test]
fn test_stress_nested_ternary() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_ternary", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            for (int i = 0; i < 5; i++) {
                String s = i < 2 ? "low" : (i < 4 ? "mid" : "high");
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
    assert_eq!(output, "low\nlow\nmid\nmid\nhigh");
}

/// Complex boolean short-circuit evaluation
#[test]
fn test_stress_short_circuit() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_shortcircuit", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int a = 5;
            int b = 10;
            int c = 15;

            // Complex short-circuit: (a > 3 && b < 20) || (c == 0)
            if ((a > 3 && b < 20) || c == 0) {
                System.out.println("yes1");
            } else {
                System.out.println("no1");
            }

            // Should short-circuit: false && ... should not evaluate right
            if (a > 100 && b > 0) {
                System.out.println("yes2");
            } else {
                System.out.println("no2");
            }

            // Should short-circuit: true || ... should not evaluate right
            if (a == 5 || b > 1000) {
                System.out.println("yes3");
            } else {
                System.out.println("no3");
            }

            // Negation
            if (!(a > 10)) {
                System.out.println("yes4");
            } else {
                System.out.println("no4");
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "yes1\nno2\nyes3\nyes4");
}

// ---------------------------------------------------------------------------
// Category 7: Object creation and method calls
// ---------------------------------------------------------------------------

/// StringBuilder method chaining
#[test]
fn test_stress_stringbuilder_chain() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_sb_chain", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            StringBuilder sb = new StringBuilder();
            sb.append("Hello");
            sb.append(" ");
            sb.append("World");
            sb.append("!");
            String result = sb.toString();
            System.out.println(result);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "Hello World!");
}

/// Creating and using ArrayList — requires new constant pool entries for
/// classes not already referenced in HelloWorld.class (expected to fail
/// until the compiler supports adding new class/method refs to the pool).
#[test]
fn test_stress_arraylist() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_arraylist", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            java.util.ArrayList list = new java.util.ArrayList();
            list.add("alpha");
            list.add("beta");
            list.add("gamma");
            int sz = list.size();
            System.out.println(sz);
            Object item = list.get(1);
            System.out.println(item);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .expect("ArrayList compilation should succeed with descriptor inference");

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "3\nbeta", "ArrayList add/size/get should work");
}

/// StringBuilder.length() — tests well-known method descriptor heuristic
#[test]
fn test_stress_sb_length() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_sb_length", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            StringBuilder sb = new StringBuilder();
            sb.append("hello");
            sb.append(" world");
            System.out.println(sb.length());
            System.out.println(sb.toString());
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .expect("StringBuilder length compilation should succeed");

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "11\nhello world", "sb.length() should return 11");
}

/// instanceof + cast
#[test]
fn test_stress_instanceof_cast() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_instanceof", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            Object obj = "hello";
            if (obj instanceof String) {
                String s = (String) obj;
                System.out.println("is string: " + s);
            } else {
                System.out.println("not string");
            }

            Object num = new Integer(42);
            if (num instanceof String) {
                System.out.println("WRONG");
            } else {
                System.out.println("not a string");
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "is string: hello\nnot a string");
}

// ---------------------------------------------------------------------------
// Category 8: var keyword stress
// ---------------------------------------------------------------------------

/// var with complex type inference
#[test]
fn test_stress_var_inference() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_var_infer", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            var i = 42;
            var l = 100L;
            var f = 1.5f;
            var d = 2.718;
            var s = "hello";
            var b = true;
            var c = 'Z';

            System.out.println(i);
            System.out.println(l);
            System.out.println(f);
            System.out.println(d);
            System.out.println(s);
            System.out.println(b);
            System.out.println(c);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "42\n100\n1.5\n2.718\nhello\ntrue\nZ");
}

/// var with new object
#[test]
fn test_stress_var_new_object() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_var_newobj", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            var sb = new StringBuilder();
            sb.append("var");
            sb.append("-works");
            System.out.println(sb.toString());
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "var-works");
}

// ---------------------------------------------------------------------------
// Category 9: Switch expressions stress
// ---------------------------------------------------------------------------

/// Switch expression as method argument
#[test]
fn test_stress_switch_expr_as_arg() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_switch_arg", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            for (int i = 1; i <= 4; i++) {
                int val = switch (i) {
                    case 1 -> 100;
                    case 2 -> 200;
                    case 3 -> 300;
                    default -> -1;
                };
                System.out.println(val);
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "100\n200\n300\n-1");
}

/// Switch expression with multi-value cases
#[test]
fn test_stress_switch_expr_multi_value() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_switch_multi", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            for (int day = 1; day <= 7; day++) {
                int type = switch (day) {
                    case 1, 7 -> 0;
                    case 2, 3, 4, 5, 6 -> 1;
                    default -> -1;
                };
                System.out.println(type);
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    // day 1=0(weekend), 2-6=1(weekday), 7=0(weekend)
    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "0\n1\n1\n1\n1\n1\n0");
}

// ---------------------------------------------------------------------------
// Category 10: Complex combined scenarios
// ---------------------------------------------------------------------------

/// Bubble sort implementation
#[test]
fn test_stress_bubble_sort() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_bubblesort", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int[] arr = new int[5];
            arr[0] = 5;
            arr[1] = 3;
            arr[2] = 8;
            arr[3] = 1;
            arr[4] = 9;

            // Bubble sort
            for (int i = 0; i < 5; i++) {
                for (int j = 0; j < 4 - i; j++) {
                    if (arr[j] > arr[j + 1]) {
                        int temp = arr[j];
                        arr[j] = arr[j + 1];
                        arr[j + 1] = temp;
                    }
                }
            }

            // Print sorted
            for (int x : arr) {
                System.out.println(x);
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "1\n3\n5\n8\n9");
}

/// Fibonacci with array memoization
#[test]
fn test_stress_fibonacci() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_fibonacci", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int n = 20;
            long[] fib = new long[21];
            fib[0] = 0L;
            fib[1] = 1L;
            for (int i = 2; i <= n; i++) {
                fib[i] = fib[i - 1] + fib[i - 2];
            }
            System.out.println(fib[10]);
            System.out.println(fib[20]);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "55\n6765");
}

/// GCD computation using while loop
#[test]
fn test_stress_gcd() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_gcd", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int a = 48;
            int b = 18;
            while (b != 0) {
                int temp = b;
                b = a % b;
                a = temp;
            }
            System.out.println(a);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "6");
}

/// Power of 2 check using bitwise
#[test]
fn test_stress_power_of_two() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_pow2", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            for (int n = 1; n <= 16; n++) {
                boolean isPow2 = (n & (n - 1)) == 0;
                if (isPow2) {
                    System.out.println(n);
                }
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "1\n2\n4\n8\n16");
}

/// Complex nested feature combination: synchronized + try-catch + for-each + var
#[test]
fn test_stress_feature_combo() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_combo", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            Object lock = new Object();
            var result = 0;
            synchronized (lock) {
                int[] values = new int[5];
                for (int i = 0; i < 5; i++) {
                    values[i] = (i + 1) * 10;
                }
                try {
                    for (int v : values) {
                        result = result + v;
                    }
                } catch (Exception e) {
                    System.out.println("error");
                }
            }
            System.out.println(result);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    // 10+20+30+40+50 = 150
    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "150");
}

/// Null handling
#[test]
fn test_stress_null_handling() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_null", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            String s = null;
            if (s == null) {
                System.out.println("is null");
            }
            s = "not null";
            if (s != null) {
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
    assert_eq!(output, "is null\nnot null");
}

// ---------------------------------------------------------------------------
// Category 11: Edge cases and boundary values
// ---------------------------------------------------------------------------

/// Large int constants
#[test]
fn test_stress_large_constants() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_large_const", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int max = 2147483647;
            int min = -2147483648;
            System.out.println(max);
            System.out.println(min);

            long lmax = 9223372036854775807L;
            System.out.println(lmax);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "2147483647\n-2147483648\n9223372036854775807");
}

/// Empty loops and blocks
#[test]
fn test_stress_empty_constructs() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_empty", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            // Empty for loop
            int i = 0;
            for (; i < 10; i++) {
            }
            System.out.println(i);

            // Empty while loop
            while (i > 10) {
            }

            // Empty block
            {
            }

            // Nested empty blocks
            {
                {
                    {
                    }
                }
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
    assert_eq!(output, "10\ndone");
}

/// Char arithmetic
#[test]
fn test_stress_char_arithmetic() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_char_arith", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            char c = 'A';
            // Char is an int in the JVM
            int code = (int) c;
            System.out.println(code);

            // Character iteration
            for (char ch = 'a'; ch <= 'e'; ch++) {
                System.out.println(ch);
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "65\na\nb\nc\nd\ne");
}

// ---------------------------------------------------------------------------
// Category 12: StackMapTable verification (full verification enabled)
// ---------------------------------------------------------------------------

/// Complex control flow with full verification
#[test]
fn test_stress_verified_complex_flow() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_verified_flow", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int sum = 0;
            for (int i = 0; i < 10; i++) {
                if (i % 2 == 0) {
                    sum = sum + i;
                } else {
                    sum = sum - 1;
                }
            }
            System.out.println(sum);
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    // Even: 0+2+4+6+8 = 20, Odd: -5 = 15
    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "15");
}

/// Switch with full verification
#[test]
fn test_stress_verified_switch() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_verified_switch", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int x = 3;
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
    assert_eq!(output, "three");
}

/// For-each with full verification — currently exposes a StackMapTable bug where
/// parameter types in locals are emitted as Null instead of their actual types.
/// The for-each loop generates extra locals (array copy, length, index) that the
/// stack map tracker doesn't fully account for with the pre-existing parameter types.
#[test]
fn test_stress_verified_foreach() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_verified_foreach", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int[] arr = new int[4];
            arr[0] = 10;
            arr[1] = 20;
            arr[2] = 30;
            arr[3] = 40;
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
    .expect("compilation should succeed");

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "100", "for-each should pass JVM verification");
}

// ---------------------------------------------------------------------------
// Category 13: Synchronized stress
// ---------------------------------------------------------------------------

/// Multiple synchronized blocks
#[test]
fn test_stress_multiple_synchronized() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_multi_sync", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            Object lock1 = new Object();
            Object lock2 = new Object();
            int value = 0;

            synchronized (lock1) {
                value = value + 10;
            }
            synchronized (lock2) {
                value = value + 20;
            }
            synchronized (lock1) {
                value = value * 2;
            }
            System.out.println(value);
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

// ---------------------------------------------------------------------------
// Category 15: Sieve of Eratosthenes (ultimate algorithm test)
// ---------------------------------------------------------------------------

#[test]
fn test_stress_sieve_of_eratosthenes() {
    if !java_available() {
        eprintln!("skipping: javac/java not found");
        return;
    }
    let (tmp_dir, class_path, mut class_file) =
        compile_and_load("stress_sieve", "java-assets/src/HelloWorld.java", "HelloWorld");

    compile_method_body(
        r#"{
            int limit = 30;
            boolean[] sieve = new boolean[31];
            // false = prime, true = composite (default false)
            sieve[0] = true;
            sieve[1] = true;

            for (int i = 2; i * i <= limit; i++) {
                if (!sieve[i]) {
                    for (int j = i * i; j <= limit; j += i) {
                        sieve[j] = true;
                    }
                }
            }

            // Print primes, one per line
            for (int i = 2; i <= limit; i++) {
                if (!sieve[i]) {
                    System.out.println(i);
                }
            }
        }"#,
        &mut class_file,
        "main",
        None,
        &CompileOptions::default(),
    )
    .unwrap();

    let output = write_and_run(&tmp_dir, &class_path, &class_file, "HelloWorld");
    assert_eq!(output, "2\n3\n5\n7\n11\n13\n17\n19\n23\n29");
}
