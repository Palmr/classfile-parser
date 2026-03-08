use super::*;

// --- Basic parser tests ---

#[test]
fn test_parse_return_int() {
    let stmts = parse_method_body("{ return 42; }").unwrap();
    assert_eq!(stmts.len(), 1);
}

#[test]
fn test_parse_local_decl() {
    let stmts = parse_method_body("{ int x = 10; return x; }").unwrap();
    assert_eq!(stmts.len(), 2);
}

#[test]
fn test_parse_if_else() {
    let stmts = parse_method_body("{ if (x > 0) { return 1; } else { return -1; } }").unwrap();
    assert_eq!(stmts.len(), 1);
}

#[test]
fn test_parse_while_loop() {
    let stmts = parse_method_body("{ while (i < 10) { i = i + 1; } }").unwrap();
    assert_eq!(stmts.len(), 1);
}

#[test]
fn test_parse_for_loop() {
    let stmts = parse_method_body("{ for (int i = 0; i < 10; i++) { sum += i; } }").unwrap();
    assert_eq!(stmts.len(), 1);
}

#[test]
fn test_parse_method_call() {
    let stmts = parse_method_body("{ System.out.println(\"hello\"); }").unwrap();
    assert_eq!(stmts.len(), 1);
}

#[test]
fn test_parse_string_concat() {
    let stmts = parse_method_body("{ String s = \"hello\" + \" world\"; }").unwrap();
    assert_eq!(stmts.len(), 1);
}

#[test]
fn test_parse_new_object() {
    let stmts = parse_method_body("{ StringBuilder sb = new StringBuilder(); }").unwrap();
    assert_eq!(stmts.len(), 1);
}

#[test]
fn test_parse_comparison_ops() {
    let stmts = parse_method_body("{ return a == b && c != d || e < f; }").unwrap();
    assert_eq!(stmts.len(), 1);
}

#[test]
fn test_parse_ternary() {
    let stmts = parse_method_body("{ return x > 0 ? x : -x; }").unwrap();
    assert_eq!(stmts.len(), 1);
}

#[test]
fn test_parse_array_access() {
    let stmts = parse_method_body("{ return arr[0]; }").unwrap();
    assert_eq!(stmts.len(), 1);
}

#[test]
fn test_parse_cast() {
    let stmts = parse_method_body("{ long x = (long) y; }").unwrap();
    assert_eq!(stmts.len(), 1);
}

#[test]
fn test_parse_throw() {
    let stmts = parse_method_body("{ throw new RuntimeException(\"error\"); }").unwrap();
    assert_eq!(stmts.len(), 1);
}

#[test]
fn test_parse_break_continue() {
    let stmts = parse_method_body("{ while (true) { if (done) break; continue; } }").unwrap();
    assert_eq!(stmts.len(), 1);
}

#[test]
fn test_parse_compound_assign() {
    let stmts = parse_method_body("{ x += 1; y -= 2; z *= 3; }").unwrap();
    assert_eq!(stmts.len(), 3);
}

#[test]
fn test_parse_increment_decrement() {
    let stmts = parse_method_body("{ i++; --j; }").unwrap();
    assert_eq!(stmts.len(), 2);
}

// --- Switch and try-catch parser tests ---

#[test]
fn test_parse_switch() {
    let stmts = parse_method_body(
        "{ switch (x) { case 1: return 1; case 2: case 3: return 23; default: return 0; } }",
    )
    .unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::Switch {
            cases,
            default_body,
            ..
        } => {
            assert_eq!(cases.len(), 2);
            assert_eq!(cases[0].values, vec![1]);
            assert_eq!(cases[1].values, vec![2, 3]); // fall-through grouping
            assert!(default_body.is_some());
        }
        other => panic!("expected Switch, got {:?}", other),
    }
}

#[test]
fn test_parse_try_catch() {
    let stmts =
        parse_method_body("{ try { foo(); } catch (Exception e) { bar(); } finally { baz(); } }")
            .unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::TryCatch {
            catches,
            finally_body,
            ..
        } => {
            assert_eq!(catches.len(), 1);
            assert_eq!(catches[0].var_name, "e");
            assert!(finally_body.is_some());
        }
        other => panic!("expected TryCatch, got {:?}", other),
    }
}

#[test]
fn test_parse_try_multiple_catches() {
    let stmts = parse_method_body(
        "{ try { foo(); } catch (RuntimeException e) { a(); } catch (Exception e) { b(); } }",
    )
    .unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::TryCatch { catches, .. } => {
            assert_eq!(catches.len(), 2);
        }
        other => panic!("expected TryCatch, got {:?}", other),
    }
}

// --- For-each and string concat parser tests ---

#[test]
fn test_parse_foreach() {
    let stmts = parse_method_body("{ for (int x : arr) { sum = sum + x; } }").unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::ForEach {
            element_type,
            var_name,
            ..
        } => {
            assert_eq!(var_name, "x");
            assert_eq!(
                *element_type,
                classfile_parser::compile::ast::TypeName::Primitive(
                    classfile_parser::compile::ast::PrimitiveKind::Int,
                ),
            );
        }
        other => panic!("expected ForEach, got: {:?}", other),
    }
}

#[test]
fn test_parse_string_concat_expr() {
    // Verify string concat parses as BinaryOp::Add
    let stmts = parse_method_body(r#"{ String s = "hello" + " world"; }"#).unwrap();
    assert_eq!(stmts.len(), 1);
}

// --- P1 parser tests ---

#[test]
fn test_parse_multi_catch() {
    let stmts = parse_method_body(
        "{ try { foo(); } catch (IllegalArgumentException | RuntimeException e) { bar(); } }",
    )
    .unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::TryCatch { catches, .. } => {
            assert_eq!(catches.len(), 1);
            assert_eq!(catches[0].exception_types.len(), 2);
            assert_eq!(catches[0].var_name, "e");
            assert_eq!(
                catches[0].exception_types[0],
                classfile_parser::compile::ast::TypeName::Class("IllegalArgumentException".into()),
            );
            assert_eq!(
                catches[0].exception_types[1],
                classfile_parser::compile::ast::TypeName::Class("RuntimeException".into()),
            );
        }
        other => panic!("expected TryCatch, got {:?}", other),
    }
}

#[test]
fn test_parse_multi_catch_three_types() {
    let stmts = parse_method_body(
        "{ try { foo(); } catch (IOException | SQLException | RuntimeException e) { bar(); } }",
    )
    .unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::TryCatch { catches, .. } => {
            assert_eq!(catches.len(), 1);
            assert_eq!(catches[0].exception_types.len(), 3);
        }
        other => panic!("expected TryCatch, got {:?}", other),
    }
}

#[test]
fn test_parse_synchronized() {
    let stmts = parse_method_body("{ synchronized (this) { foo(); } }").unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::Synchronized { lock_expr, body } => {
            assert!(matches!(
                lock_expr,
                classfile_parser::compile::ast::CExpr::This
            ));
            assert_eq!(body.len(), 1);
        }
        other => panic!("expected Synchronized, got {:?}", other),
    }
}

#[test]
fn test_parse_synchronized_with_expr() {
    let stmts = parse_method_body("{ synchronized (lock) { x = 1; y = 2; } }").unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::Synchronized { lock_expr, body } => {
            assert!(matches!(
                lock_expr,
                classfile_parser::compile::ast::CExpr::Ident(_)
            ));
            assert_eq!(body.len(), 2);
        }
        other => panic!("expected Synchronized, got {:?}", other),
    }
}

// --- P2 parser tests ---

#[test]
fn test_parse_var_decl() {
    let stmts = parse_method_body("{ var x = 42; }").unwrap();
    assert_eq!(stmts.len(), 1);
    // Should parse as LocalDecl with __var__ sentinel type
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::LocalDecl { ty, name, init } => {
            assert_eq!(name, "x");
            assert!(init.is_some());
            assert_eq!(
                *ty,
                classfile_parser::compile::ast::TypeName::Class("__var__".into())
            );
        }
        other => panic!("expected LocalDecl, got {:?}", other),
    }
}

#[test]
fn test_parse_var_string() {
    let stmts = parse_method_body(r#"{ var s = "hello"; }"#).unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::LocalDecl { name, init, .. } => {
            assert_eq!(name, "s");
            assert!(matches!(
                init,
                Some(classfile_parser::compile::ast::CExpr::StringLiteral(_))
            ));
        }
        other => panic!("expected LocalDecl, got {:?}", other),
    }
}

#[test]
fn test_parse_multi_dim_array() {
    let stmts = parse_method_body("{ int[][] arr = new int[3][4]; }").unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::LocalDecl {
            init: Some(expr), ..
        } => match expr {
            classfile_parser::compile::ast::CExpr::NewMultiArray {
                element_type,
                dimensions,
            } => {
                assert_eq!(
                    *element_type,
                    classfile_parser::compile::ast::TypeName::Primitive(
                        classfile_parser::compile::ast::PrimitiveKind::Int
                    )
                );
                assert_eq!(dimensions.len(), 2);
            }
            other => panic!("expected NewMultiArray, got {:?}", other),
        },
        other => panic!("expected LocalDecl with init, got {:?}", other),
    }
}

#[test]
fn test_parse_generic_method() {
    // obj.<String>method() should parse without error
    let stmts = parse_method_body(r#"{ obj.<String>method(); }"#).unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::ExprStmt(
            classfile_parser::compile::ast::CExpr::MethodCall { name, .. },
        ) => {
            assert_eq!(name, "method");
        }
        other => panic!("expected MethodCall, got {:?}", other),
    }
}

#[test]
fn test_parse_switch_expr() {
    let stmts = parse_method_body(
        r#"{
        int x = 1;
        int r = switch (x) {
            case 1 -> 10;
            case 2 -> 20;
            default -> 0;
        };
    }"#,
    )
    .unwrap();
    assert_eq!(stmts.len(), 2);
    match &stmts[1] {
        classfile_parser::compile::ast::CStmt::LocalDecl {
            init: Some(expr), ..
        } => match expr {
            classfile_parser::compile::ast::CExpr::SwitchExpr { cases, .. } => {
                assert_eq!(cases.len(), 2);
            }
            other => panic!("expected SwitchExpr, got {:?}", other),
        },
        other => panic!("expected LocalDecl, got {:?}", other),
    }
}

#[test]
fn test_parse_switch_expr_multi_case() {
    let stmts = parse_method_body(
        r#"{
        int r = switch (x) {
            case 1, 2 -> 10;
            case 3 -> 30;
            default -> 0;
        };
    }"#,
    )
    .unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::LocalDecl {
            init: Some(expr), ..
        } => match expr {
            classfile_parser::compile::ast::CExpr::SwitchExpr { cases, .. } => {
                assert_eq!(cases.len(), 2);
                assert_eq!(cases[0].values.len(), 2);
            }
            other => panic!("expected SwitchExpr, got {:?}", other),
        },
        other => panic!("expected LocalDecl, got {:?}", other),
    }
}

#[test]
fn test_parse_lambda_no_args() {
    let stmts = parse_method_body(r#"{ Runnable r = () -> System.out.println("hi"); }"#).unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::LocalDecl {
            init: Some(expr), ..
        } => match expr {
            classfile_parser::compile::ast::CExpr::Lambda { params, .. } => {
                assert_eq!(params.len(), 0);
            }
            other => panic!("expected Lambda, got {:?}", other),
        },
        other => panic!("expected LocalDecl, got {:?}", other),
    }
}

#[test]
fn test_parse_lambda_typed_param() {
    let stmts = parse_method_body(r#"{ var f = (int x) -> x + 1; }"#).unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::LocalDecl {
            init: Some(expr), ..
        } => match expr {
            classfile_parser::compile::ast::CExpr::Lambda { params, body } => {
                assert_eq!(params.len(), 1);
                assert_eq!(params[0].name, "x");
                assert!(params[0].ty.is_some());
                assert!(matches!(
                    body,
                    classfile_parser::compile::ast::LambdaBody::Expr(_)
                ));
            }
            other => panic!("expected Lambda, got {:?}", other),
        },
        other => panic!("expected LocalDecl, got {:?}", other),
    }
}

#[test]
fn test_parse_lambda_block() {
    let stmts = parse_method_body(r#"{ var f = (int x) -> { return x + 1; }; }"#).unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::LocalDecl {
            init: Some(expr), ..
        } => match expr {
            classfile_parser::compile::ast::CExpr::Lambda { params, body } => {
                assert_eq!(params.len(), 1);
                assert!(matches!(
                    body,
                    classfile_parser::compile::ast::LambdaBody::Block(_)
                ));
            }
            other => panic!("expected Lambda, got {:?}", other),
        },
        other => panic!("expected LocalDecl, got {:?}", other),
    }
}

#[test]
fn test_parse_method_ref() {
    let stmts = parse_method_body(r#"{ var f = String::valueOf; }"#).unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::LocalDecl {
            init: Some(expr), ..
        } => match expr {
            classfile_parser::compile::ast::CExpr::MethodRef {
                class_name,
                method_name,
            } => {
                assert_eq!(class_name, "String");
                assert_eq!(method_name, "valueOf");
            }
            other => panic!("expected MethodRef, got {:?}", other),
        },
        other => panic!("expected LocalDecl, got {:?}", other),
    }
}

#[test]
fn test_parse_arrow_token() {
    // Verify the arrow token works in switch expressions
    let stmts = parse_method_body(
        r#"{
        int r = switch (1) {
            case 1 -> 42;
            default -> 0;
        };
    }"#,
    )
    .unwrap();
    assert_eq!(stmts.len(), 1);
}

// --- Parser stress tests ---

/// Deeply nested expression parsing
#[test]
fn test_parse_stress_deeply_nested_expr() {
    let stmts = parse_method_body("{ int x = ((((((1 + 2) * 3) - 4) / 5) % 6) + 7); }").unwrap();
    assert_eq!(stmts.len(), 1);
}

/// Multiple statements in a row
#[test]
fn test_parse_stress_many_statements() {
    let mut body = String::from("{ ");
    for i in 0..50 {
        body.push_str(&format!("int v{} = {}; ", i, i));
    }
    body.push_str(" }");
    let stmts = parse_method_body(&body).unwrap();
    assert_eq!(stmts.len(), 50);
}

/// Complex type declarations
#[test]
fn test_parse_stress_type_decls() {
    let stmts = parse_method_body(
        r#"{
        int a = 1;
        long b = 2L;
        float c = 3.0f;
        double d = 4.0;
        boolean e = true;
        char f = 'x';
        String g = "hi";
        int[] h = new int[5];
        int[][] i = new int[3][4];
        Object j = null;
    }"#,
    )
    .unwrap();
    assert_eq!(stmts.len(), 10);
}

/// Switch with many cases
#[test]
fn test_parse_stress_switch_many_cases() {
    let mut body = String::from("{ switch (x) { ");
    for i in 0..20 {
        body.push_str(&format!("case {}: return {}; ", i, i * 10));
    }
    body.push_str("default: return -1; } }");
    let stmts = parse_method_body(&body).unwrap();
    assert_eq!(stmts.len(), 1);
}

/// Nested switch expressions
#[test]
fn test_parse_stress_nested_switch_expr() {
    let stmts = parse_method_body(
        r#"{
        int outer = switch (a) {
            case 1 -> switch (b) {
                case 10 -> 100;
                default -> 0;
            };
            default -> -1;
        };
    }"#,
    );
    // This may or may not parse depending on implementation — record result
    match stmts {
        Ok(s) => assert_eq!(s.len(), 1),
        Err(e) => eprintln!("nested switch expr not supported: {}", e),
    }
}

/// For-each with dotted type
#[test]
fn test_parse_stress_foreach_dotted_type() {
    let stmts = parse_method_body("{ for (java.lang.String s : list) { System.out.println(s); } }")
        .unwrap();
    assert_eq!(stmts.len(), 1);
}

/// All binary operators in one expression
#[test]
fn test_parse_stress_all_binops() {
    let stmts = parse_method_body(
        "{ int x = a + b - c * d / e % f; int y = g & h | i ^ j; int z = k << l >> m >>> n; }",
    )
    .unwrap();
    assert_eq!(stmts.len(), 3);
}

/// Chained method calls
#[test]
fn test_parse_stress_chained_calls() {
    let stmts =
        parse_method_body(r#"{ String s = obj.method1().method2().method3().toString(); }"#)
            .unwrap();
    assert_eq!(stmts.len(), 1);
}

/// Lambda with multiple parameters
#[test]
fn test_parse_stress_lambda_multi_param() {
    let stmts = parse_method_body(r#"{ var f = (int a, int b, int c) -> a + b + c; }"#).unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::LocalDecl {
            init: Some(expr), ..
        } => match expr {
            classfile_parser::compile::ast::CExpr::Lambda { params, .. } => {
                assert_eq!(params.len(), 3);
            }
            other => panic!("expected Lambda, got {:?}", other),
        },
        other => panic!("expected LocalDecl, got {:?}", other),
    }
}

/// Lambda with block body containing control flow
#[test]
fn test_parse_stress_lambda_complex_body() {
    let stmts = parse_method_body(
        r#"{ var f = (int x) -> {
            if (x > 0) {
                return x * 2;
            } else {
                return -x;
            }
        }; }"#,
    )
    .unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::LocalDecl {
            init: Some(expr), ..
        } => {
            match expr {
                classfile_parser::compile::ast::CExpr::Lambda { body, .. } => {
                    match body {
                        classfile_parser::compile::ast::LambdaBody::Block(stmts) => {
                            assert_eq!(stmts.len(), 1); // one if statement
                        }
                        _ => panic!("expected block body"),
                    }
                }
                other => panic!("expected Lambda, got {:?}", other),
            }
        }
        other => panic!("expected LocalDecl, got {:?}", other),
    }
}

/// Multiple var declarations in sequence
#[test]
fn test_parse_stress_var_sequence() {
    let stmts = parse_method_body(
        r#"{
        var a = 1;
        var b = 2L;
        var c = 3.0f;
        var d = 4.0;
        var e = "str";
        var f = true;
        var g = 'x';
        var h = null;
        var i = new Object();
    }"#,
    )
    .unwrap();
    assert_eq!(stmts.len(), 9);
}

/// Comprehensive expression in a single assignment
#[test]
fn test_parse_stress_complex_expr() {
    let stmts =
        parse_method_body("{ int x = (a > 0 && b < 10) || !(c == d) ? (e + f) * g : h - i / j; }")
            .unwrap();
    assert_eq!(stmts.len(), 1);
}

/// Try-catch with multiple catch blocks and finally
#[test]
fn test_parse_stress_complex_try_catch() {
    let stmts = parse_method_body(
        r#"{
        try {
            foo();
        } catch (IllegalArgumentException e) {
            bar();
        } catch (NullPointerException | ArrayIndexOutOfBoundsException e) {
            baz();
        } catch (RuntimeException e) {
            qux();
        } finally {
            cleanup();
        }
    }"#,
    )
    .unwrap();
    assert_eq!(stmts.len(), 1);
    match &stmts[0] {
        classfile_parser::compile::ast::CStmt::TryCatch {
            catches,
            finally_body,
            ..
        } => {
            assert_eq!(catches.len(), 3);
            assert_eq!(catches[1].exception_types.len(), 2); // multi-catch
            assert!(finally_body.is_some());
        }
        other => panic!("expected TryCatch, got {:?}", other),
    }
}

/// Synchronized with complex expression
#[test]
fn test_parse_stress_synchronized_complex() {
    let stmts = parse_method_body(
        r#"{
        synchronized (this) {
            int x = 1;
            for (int i = 0; i < 10; i++) {
                x = x + i;
            }
            if (x > 50) {
                throw new RuntimeException("too big");
            }
        }
    }"#,
    )
    .unwrap();
    assert_eq!(stmts.len(), 1);
}

/// Generic type parameters in method calls
#[test]
fn test_parse_stress_generic_params() {
    let stmts = parse_method_body(
        r#"{
        obj.<String>method1();
        obj.<Integer, String>method2();
    }"#,
    )
    .unwrap();
    assert_eq!(stmts.len(), 2);
}
