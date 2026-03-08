use super::expr::*;
use super::structured_types::*;

/// Options controlling which desugaring passes to apply.
#[derive(Clone, Debug)]
pub struct DesugarOptions {
    pub foreach: bool,
    pub try_resources: bool,
    pub enum_switch: bool,
    pub string_switch: bool,
    pub assert: bool,
    pub autobox: bool,
    pub synthetic_accessors: bool,
}

impl Default for DesugarOptions {
    fn default() -> Self {
        Self {
            foreach: true,
            try_resources: true,
            enum_switch: true,
            string_switch: true,
            assert: true,
            autobox: true,
            synthetic_accessors: true,
        }
    }
}

/// Run all enabled desugaring passes on a structured body.
pub fn desugar(body: &mut StructuredBody, options: &DesugarOptions) {
    for stmt in &mut body.statements {
        desugar_stmt(stmt, options);
    }
}

fn desugar_stmt(stmt: &mut StructuredStmt, options: &DesugarOptions) {
    match stmt {
        StructuredStmt::Block(stmts) => {
            for s in stmts.iter_mut() {
                desugar_stmt(s, options);
            }
            if options.foreach {
                desugar_foreach_in_block(stmts);
            }
            if options.assert {
                desugar_assert_in_block(stmts);
            }
        }
        StructuredStmt::If {
            then_body,
            else_body,
            condition,
            ..
        } => {
            desugar_stmt(then_body, options);
            if let Some(eb) = else_body {
                desugar_stmt(eb, options);
            }
            if options.autobox {
                desugar_autobox_expr(condition);
            }
        }
        StructuredStmt::While {
            body, condition, ..
        } => {
            desugar_stmt(body, options);
            if options.autobox {
                desugar_autobox_expr(condition);
            }
        }
        StructuredStmt::DoWhile {
            body, condition, ..
        } => {
            desugar_stmt(body, options);
            if options.autobox {
                desugar_autobox_expr(condition);
            }
        }
        StructuredStmt::For {
            init,
            body,
            update,
            condition,
            ..
        } => {
            if let Some(i) = init {
                desugar_stmt(i, options);
            }
            desugar_stmt(body, options);
            if let Some(u) = update {
                desugar_stmt(u, options);
            }
            if options.autobox {
                desugar_autobox_expr(condition);
            }
        }
        StructuredStmt::ForEach { body, .. } => {
            desugar_stmt(body, options);
        }
        StructuredStmt::Switch {
            cases,
            default,
            expr,
            ..
        } => {
            for case in cases.iter_mut() {
                desugar_stmt(&mut case.body, options);
            }
            if let Some(d) = default {
                desugar_stmt(d, options);
            }
            if options.autobox {
                desugar_autobox_expr(expr);
            }
        }
        StructuredStmt::TryCatch {
            try_body,
            catches,
            finally_body,
            ..
        } => {
            desugar_stmt(try_body, options);
            for c in catches.iter_mut() {
                desugar_stmt(&mut c.body, options);
            }
            if let Some(f) = finally_body {
                desugar_stmt(f, options);
            }
        }
        StructuredStmt::TryWithResources { body, catches, .. } => {
            desugar_stmt(body, options);
            for c in catches.iter_mut() {
                desugar_stmt(&mut c.body, options);
            }
        }
        StructuredStmt::Synchronized { body, .. } => {
            desugar_stmt(body, options);
        }
        StructuredStmt::Labeled { body, .. } => {
            desugar_stmt(body, options);
        }
        StructuredStmt::Simple(s) => {
            if options.autobox {
                desugar_autobox_stmt(s);
            }
        }
        _ => {}
    }
}

/// Detect Iterator-based for-each pattern in a block:
/// ```java
/// Iterator iter = coll.iterator();
/// while (iter.hasNext()) { T x = (T) iter.next(); ... }
/// ```
/// Rewrites to ForEach.
fn desugar_foreach_in_block(stmts: &mut Vec<StructuredStmt>) {
    let mut i = 0;
    while i + 1 < stmts.len() {
        let is_foreach = {
            if let (
                StructuredStmt::Simple(Stmt::LocalStore {
                    var: iter_var,
                    value: iter_init,
                }),
                StructuredStmt::While { condition, body },
            ) = (&stmts[i], &stmts[i + 1])
            {
                is_iterator_call(iter_init)
                    && is_has_next_call(condition, iter_var)
                    && find_next_call_in_body(body, iter_var).is_some()
            } else {
                false
            }
        };

        if is_foreach
            && let StructuredStmt::Simple(Stmt::LocalStore {
                value: iter_init, ..
            }) = &stmts[i]
        {
            let iterable = extract_iterator_receiver(iter_init)
                .unwrap_or_else(|| Expr::Unresolved("/* iterable */".into()));

            if let StructuredStmt::While { body, .. } = &stmts[i + 1]
                && let Some((loop_var, remaining_body)) = find_next_call_in_body(
                    body,
                    &LocalVar {
                        index: 0,
                        name: None,
                        ty: super::descriptor::JvmType::Unknown,
                    },
                )
            {
                let foreach = StructuredStmt::ForEach {
                    var: loop_var,
                    iterable,
                    body: Box::new(remaining_body),
                };
                stmts.splice(i..=i + 1, std::iter::once(foreach));
                continue;
            }
        }
        i += 1;
    }
}

fn is_iterator_call(expr: &Expr) -> bool {
    matches!(expr, Expr::MethodCall { method_name, .. } if method_name == "iterator")
}

fn is_has_next_call(expr: &Expr, _iter_var: &LocalVar) -> bool {
    matches!(expr, Expr::MethodCall { method_name, .. } if method_name == "hasNext")
}

fn extract_iterator_receiver(expr: &Expr) -> Option<Expr> {
    if let Expr::MethodCall {
        object: Some(obj),
        method_name,
        ..
    } = expr
        && method_name == "iterator"
    {
        return Some(*obj.clone());
    }
    None
}

fn find_next_call_in_body(
    _body: &StructuredStmt,
    _iter_var: &LocalVar,
) -> Option<(LocalVar, StructuredStmt)> {
    // TODO: Look for `T x = (T) iter.next()` as the first statement of the body
    None
}

/// Detect `if (!$assertionsDisabled && !cond) throw new AssertionError(msg)` pattern.
fn desugar_assert_in_block(stmts: &mut Vec<StructuredStmt>) {
    let mut i = 0;
    while i < stmts.len() {
        let replacement = match &stmts[i] {
            StructuredStmt::If {
                condition,
                then_body,
                else_body: None,
            } => {
                if let Some((assert_cond, assert_msg)) = match_assert_pattern(condition, then_body)
                {
                    Some(StructuredStmt::Assert {
                        condition: assert_cond,
                        message: assert_msg,
                    })
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(repl) = replacement {
            stmts[i] = repl;
        }
        i += 1;
    }
}

fn match_assert_pattern(
    _condition: &Expr,
    _then_body: &StructuredStmt,
) -> Option<(Expr, Option<Expr>)> {
    // TODO: Match the pattern:
    // condition = !$assertionsDisabled (a FieldGet for a static boolean field named "$assertionsDisabled")
    // combined with the actual assertion condition
    // then_body = throw new AssertionError(...)
    None
}

/// Desugar autoboxing/unboxing in expressions.
fn desugar_autobox_expr(expr: &mut Expr) {
    *expr = desugar_autobox_inner(expr.clone());
}

fn desugar_autobox_inner(expr: Expr) -> Expr {
    match expr {
        // Integer.valueOf(n) -> n
        Expr::MethodCall {
            kind: InvokeKind::Static,
            ref class_name,
            ref method_name,
            ref args,
            ..
        } if method_name == "valueOf" && is_wrapper_class(class_name) && args.len() == 1 => {
            desugar_autobox_inner(args[0].clone())
        }
        // n.intValue() / n.longValue() / etc -> n
        Expr::MethodCall {
            ref object,
            ref method_name,
            ref args,
            ..
        } if is_unbox_method(method_name) && args.is_empty() && object.is_some() => {
            desugar_autobox_inner(*object.as_ref().unwrap().clone())
        }
        // Recurse into sub-expressions
        Expr::BinaryOp { op, left, right } => Expr::BinaryOp {
            op,
            left: Box::new(desugar_autobox_inner(*left)),
            right: Box::new(desugar_autobox_inner(*right)),
        },
        Expr::UnaryOp { op, operand } => Expr::UnaryOp {
            op,
            operand: Box::new(desugar_autobox_inner(*operand)),
        },
        other => other,
    }
}

fn desugar_autobox_stmt(stmt: &mut Stmt) {
    match stmt {
        Stmt::LocalStore { value, .. } => *value = desugar_autobox_inner(value.clone()),
        Stmt::FieldStore { value, .. } => *value = desugar_autobox_inner(value.clone()),
        Stmt::ArrayStore { value, .. } => *value = desugar_autobox_inner(value.clone()),
        Stmt::ExprStmt(e) => *e = desugar_autobox_inner(e.clone()),
        Stmt::Return(Some(e)) => *e = desugar_autobox_inner(e.clone()),
        Stmt::Throw(e) => *e = desugar_autobox_inner(e.clone()),
        _ => {}
    }
}

fn is_wrapper_class(name: &str) -> bool {
    matches!(
        name,
        "java/lang/Integer"
            | "java/lang/Long"
            | "java/lang/Float"
            | "java/lang/Double"
            | "java/lang/Byte"
            | "java/lang/Short"
            | "java/lang/Character"
            | "java/lang/Boolean"
    )
}

fn is_unbox_method(name: &str) -> bool {
    matches!(
        name,
        "intValue"
            | "longValue"
            | "floatValue"
            | "doubleValue"
            | "byteValue"
            | "shortValue"
            | "charValue"
            | "booleanValue"
    )
}
