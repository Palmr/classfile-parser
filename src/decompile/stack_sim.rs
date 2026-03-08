use crate::attribute_info::{AttributeInfoVariant, CodeAttribute};
use crate::code_attribute::Instruction;
use crate::constant_info::ConstantInfo;

use super::cfg_types::*;
use super::descriptor::*;
use super::expr::*;
use super::util;

use std::collections::HashMap;

/// Build a lookup table from local variable index to name using the
/// LocalVariableTable sub-attribute of the CodeAttribute (when available).
fn build_local_name_table(
    code_attr: &CodeAttribute,
    const_pool: &[ConstantInfo],
) -> HashMap<u16, String> {
    let mut names = HashMap::new();
    for attr in &code_attr.attributes {
        if let Some(AttributeInfoVariant::LocalVariableTable(lvt)) = &attr.info_parsed {
            for item in &lvt.items {
                if let Some(name) = util::get_utf8(const_pool, item.name_index) {
                    names.insert(item.index, name.to_string());
                }
            }
        }
    }
    names
}

/// Create a LocalVar with optional name lookup.
fn make_local(index: u16, ty: JvmType, names: &HashMap<u16, String>) -> LocalVar {
    LocalVar {
        index,
        name: names.get(&index).cloned(),
        ty,
    }
}

/// Resolve a constant pool field reference to (class_name, field_name, field_type).
fn resolve_field_ref(const_pool: &[ConstantInfo], index: u16) -> (String, String, JvmType) {
    if let Some((class_name, field_name, descriptor)) = util::resolve_ref(const_pool, index) {
        let field_type = parse_type_descriptor(descriptor).unwrap_or(JvmType::Unknown);
        (class_name.to_string(), field_name.to_string(), field_type)
    } else {
        (
            format!("<class?#{}>", index),
            format!("<field?#{}>", index),
            JvmType::Unknown,
        )
    }
}

/// Resolve a constant pool method reference to (class_name, method_name, descriptor_str, param_types, return_type).
fn resolve_method_ref(
    const_pool: &[ConstantInfo],
    index: u16,
) -> (String, String, String, Vec<JvmType>, JvmType) {
    if let Some((class_name, method_name, descriptor)) = util::resolve_ref(const_pool, index) {
        let (params, ret) =
            parse_method_descriptor(descriptor).unwrap_or_else(|| (vec![], JvmType::Unknown));
        (
            class_name.to_string(),
            method_name.to_string(),
            descriptor.to_string(),
            params,
            ret,
        )
    } else {
        (
            format!("<class?#{}>", index),
            format!("<method?#{}>", index),
            String::new(),
            vec![],
            JvmType::Unknown,
        )
    }
}

/// Load a constant from the constant pool by index (for ldc/ldc_w/ldc2_w).
fn load_constant(const_pool: &[ConstantInfo], index: u16) -> Expr {
    match const_pool.get((index as usize).wrapping_sub(1)) {
        Some(ConstantInfo::Integer(c)) => Expr::IntLiteral(c.value),
        Some(ConstantInfo::Float(c)) => Expr::FloatLiteral(c.value),
        Some(ConstantInfo::Long(c)) => Expr::LongLiteral(c.value),
        Some(ConstantInfo::Double(c)) => Expr::DoubleLiteral(c.value),
        Some(ConstantInfo::String(c)) => {
            if let Some(s) = util::get_utf8(const_pool, c.string_index) {
                Expr::StringLiteral(s.to_string())
            } else {
                Expr::Unresolved(format!("string_cp#{}", c.string_index))
            }
        }
        Some(ConstantInfo::Class(c)) => {
            if let Some(name) = util::get_utf8(const_pool, c.name_index) {
                Expr::ClassLiteral(internal_to_source_name(name).to_string())
            } else {
                Expr::Unresolved(format!("class_cp#{}", c.name_index))
            }
        }
        _ => Expr::Unresolved(format!("cp#{}", index)),
    }
}

/// Simulate a single basic block, converting bytecode instructions into
/// expression trees and statement lists.
pub fn simulate_block(
    block: &BasicBlock,
    const_pool: &[ConstantInfo],
    code_attr: &CodeAttribute,
    is_static: bool,
) -> SimulatedBlock {
    let local_names = build_local_name_table(code_attr, const_pool);
    let mut stack: Vec<Expr> = Vec::new();
    let mut stmts: Vec<Stmt> = Vec::new();
    let mut branch_condition: Option<Expr> = None;

    /// Pop from the stack or return an Unresolved placeholder.
    macro_rules! pop {
        ($stack:expr) => {
            $stack
                .pop()
                .unwrap_or(Expr::Unresolved("stack_underflow".to_string()))
        };
    }

    for addressed in &block.instructions {
        let instr = &addressed.instruction;
        match instr {
            // ============================================================
            // Constants
            // ============================================================
            Instruction::Iconstm1 => stack.push(Expr::IntLiteral(-1)),
            Instruction::Iconst0 => stack.push(Expr::IntLiteral(0)),
            Instruction::Iconst1 => stack.push(Expr::IntLiteral(1)),
            Instruction::Iconst2 => stack.push(Expr::IntLiteral(2)),
            Instruction::Iconst3 => stack.push(Expr::IntLiteral(3)),
            Instruction::Iconst4 => stack.push(Expr::IntLiteral(4)),
            Instruction::Iconst5 => stack.push(Expr::IntLiteral(5)),

            Instruction::Lconst0 => stack.push(Expr::LongLiteral(0)),
            Instruction::Lconst1 => stack.push(Expr::LongLiteral(1)),

            Instruction::Fconst0 => stack.push(Expr::FloatLiteral(0.0)),
            Instruction::Fconst1 => stack.push(Expr::FloatLiteral(1.0)),
            Instruction::Fconst2 => stack.push(Expr::FloatLiteral(2.0)),

            Instruction::Dconst0 => stack.push(Expr::DoubleLiteral(0.0)),
            Instruction::Dconst1 => stack.push(Expr::DoubleLiteral(1.0)),

            Instruction::Aconstnull => stack.push(Expr::NullLiteral),

            Instruction::Bipush(val) => stack.push(Expr::IntLiteral(*val as i32)),
            Instruction::Sipush(val) => stack.push(Expr::IntLiteral(*val as i32)),

            Instruction::Ldc(idx) => stack.push(load_constant(const_pool, *idx as u16)),
            Instruction::LdcW(idx) => stack.push(load_constant(const_pool, *idx)),
            Instruction::Ldc2W(idx) => stack.push(load_constant(const_pool, *idx)),

            // ============================================================
            // Loads
            // ============================================================
            Instruction::Iload(idx) => {
                stack.push(Expr::LocalLoad(make_local(
                    *idx as u16,
                    JvmType::Int,
                    &local_names,
                )));
            }
            Instruction::Iload0 => {
                stack.push(Expr::LocalLoad(make_local(0, JvmType::Int, &local_names)));
            }
            Instruction::Iload1 => {
                stack.push(Expr::LocalLoad(make_local(1, JvmType::Int, &local_names)));
            }
            Instruction::Iload2 => {
                stack.push(Expr::LocalLoad(make_local(2, JvmType::Int, &local_names)));
            }
            Instruction::Iload3 => {
                stack.push(Expr::LocalLoad(make_local(3, JvmType::Int, &local_names)));
            }

            Instruction::Lload(idx) => {
                stack.push(Expr::LocalLoad(make_local(
                    *idx as u16,
                    JvmType::Long,
                    &local_names,
                )));
            }
            Instruction::Lload0 => {
                stack.push(Expr::LocalLoad(make_local(0, JvmType::Long, &local_names)));
            }
            Instruction::Lload1 => {
                stack.push(Expr::LocalLoad(make_local(1, JvmType::Long, &local_names)));
            }
            Instruction::Lload2 => {
                stack.push(Expr::LocalLoad(make_local(2, JvmType::Long, &local_names)));
            }
            Instruction::Lload3 => {
                stack.push(Expr::LocalLoad(make_local(3, JvmType::Long, &local_names)));
            }

            Instruction::Fload(idx) => {
                stack.push(Expr::LocalLoad(make_local(
                    *idx as u16,
                    JvmType::Float,
                    &local_names,
                )));
            }
            Instruction::Fload0 => {
                stack.push(Expr::LocalLoad(make_local(0, JvmType::Float, &local_names)));
            }
            Instruction::Fload1 => {
                stack.push(Expr::LocalLoad(make_local(1, JvmType::Float, &local_names)));
            }
            Instruction::Fload2 => {
                stack.push(Expr::LocalLoad(make_local(2, JvmType::Float, &local_names)));
            }
            Instruction::Fload3 => {
                stack.push(Expr::LocalLoad(make_local(3, JvmType::Float, &local_names)));
            }

            Instruction::Dload(idx) => {
                stack.push(Expr::LocalLoad(make_local(
                    *idx as u16,
                    JvmType::Double,
                    &local_names,
                )));
            }
            Instruction::Dload0 => {
                stack.push(Expr::LocalLoad(make_local(
                    0,
                    JvmType::Double,
                    &local_names,
                )));
            }
            Instruction::Dload1 => {
                stack.push(Expr::LocalLoad(make_local(
                    1,
                    JvmType::Double,
                    &local_names,
                )));
            }
            Instruction::Dload2 => {
                stack.push(Expr::LocalLoad(make_local(
                    2,
                    JvmType::Double,
                    &local_names,
                )));
            }
            Instruction::Dload3 => {
                stack.push(Expr::LocalLoad(make_local(
                    3,
                    JvmType::Double,
                    &local_names,
                )));
            }

            Instruction::Aload(idx) => {
                let idx16 = *idx as u16;
                if idx16 == 0 && !is_static {
                    stack.push(Expr::This);
                } else {
                    stack.push(Expr::LocalLoad(make_local(
                        idx16,
                        JvmType::Reference("java/lang/Object".to_string()),
                        &local_names,
                    )));
                }
            }
            Instruction::Aload0 => {
                if !is_static {
                    stack.push(Expr::This);
                } else {
                    stack.push(Expr::LocalLoad(make_local(
                        0,
                        JvmType::Reference("java/lang/Object".to_string()),
                        &local_names,
                    )));
                }
            }
            Instruction::Aload1 => {
                stack.push(Expr::LocalLoad(make_local(
                    1,
                    JvmType::Reference("java/lang/Object".to_string()),
                    &local_names,
                )));
            }
            Instruction::Aload2 => {
                stack.push(Expr::LocalLoad(make_local(
                    2,
                    JvmType::Reference("java/lang/Object".to_string()),
                    &local_names,
                )));
            }
            Instruction::Aload3 => {
                stack.push(Expr::LocalLoad(make_local(
                    3,
                    JvmType::Reference("java/lang/Object".to_string()),
                    &local_names,
                )));
            }

            // Wide loads
            Instruction::IloadWide(idx) => {
                stack.push(Expr::LocalLoad(make_local(
                    *idx,
                    JvmType::Int,
                    &local_names,
                )));
            }
            Instruction::LloadWide(idx) => {
                stack.push(Expr::LocalLoad(make_local(
                    *idx,
                    JvmType::Long,
                    &local_names,
                )));
            }
            Instruction::FloadWide(idx) => {
                stack.push(Expr::LocalLoad(make_local(
                    *idx,
                    JvmType::Float,
                    &local_names,
                )));
            }
            Instruction::DloadWide(idx) => {
                stack.push(Expr::LocalLoad(make_local(
                    *idx,
                    JvmType::Double,
                    &local_names,
                )));
            }
            Instruction::AloadWide(idx) => {
                if *idx == 0 && !is_static {
                    stack.push(Expr::This);
                } else {
                    stack.push(Expr::LocalLoad(make_local(
                        *idx,
                        JvmType::Reference("java/lang/Object".to_string()),
                        &local_names,
                    )));
                }
            }

            // ============================================================
            // Stores
            // ============================================================
            Instruction::Istore(idx) => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(*idx as u16, JvmType::Int, &local_names),
                    value: val,
                });
            }
            Instruction::Istore0 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(0, JvmType::Int, &local_names),
                    value: val,
                });
            }
            Instruction::Istore1 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(1, JvmType::Int, &local_names),
                    value: val,
                });
            }
            Instruction::Istore2 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(2, JvmType::Int, &local_names),
                    value: val,
                });
            }
            Instruction::Istore3 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(3, JvmType::Int, &local_names),
                    value: val,
                });
            }

            Instruction::Lstore(idx) => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(*idx as u16, JvmType::Long, &local_names),
                    value: val,
                });
            }
            Instruction::Lstore0 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(0, JvmType::Long, &local_names),
                    value: val,
                });
            }
            Instruction::Lstore1 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(1, JvmType::Long, &local_names),
                    value: val,
                });
            }
            Instruction::Lstore2 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(2, JvmType::Long, &local_names),
                    value: val,
                });
            }
            Instruction::Lstore3 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(3, JvmType::Long, &local_names),
                    value: val,
                });
            }

            Instruction::Fstore(idx) => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(*idx as u16, JvmType::Float, &local_names),
                    value: val,
                });
            }
            Instruction::Fstore0 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(0, JvmType::Float, &local_names),
                    value: val,
                });
            }
            Instruction::Fstore1 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(1, JvmType::Float, &local_names),
                    value: val,
                });
            }
            Instruction::Fstore2 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(2, JvmType::Float, &local_names),
                    value: val,
                });
            }
            Instruction::Fstore3 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(3, JvmType::Float, &local_names),
                    value: val,
                });
            }

            Instruction::Dstore(idx) => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(*idx as u16, JvmType::Double, &local_names),
                    value: val,
                });
            }
            Instruction::Dstore0 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(0, JvmType::Double, &local_names),
                    value: val,
                });
            }
            Instruction::Dstore1 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(1, JvmType::Double, &local_names),
                    value: val,
                });
            }
            Instruction::Dstore2 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(2, JvmType::Double, &local_names),
                    value: val,
                });
            }
            Instruction::Dstore3 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(3, JvmType::Double, &local_names),
                    value: val,
                });
            }

            Instruction::Astore(idx) => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(
                        *idx as u16,
                        JvmType::Reference("java/lang/Object".to_string()),
                        &local_names,
                    ),
                    value: val,
                });
            }
            Instruction::Astore0 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(
                        0,
                        JvmType::Reference("java/lang/Object".to_string()),
                        &local_names,
                    ),
                    value: val,
                });
            }
            Instruction::Astore1 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(
                        1,
                        JvmType::Reference("java/lang/Object".to_string()),
                        &local_names,
                    ),
                    value: val,
                });
            }
            Instruction::Astore2 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(
                        2,
                        JvmType::Reference("java/lang/Object".to_string()),
                        &local_names,
                    ),
                    value: val,
                });
            }
            Instruction::Astore3 => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(
                        3,
                        JvmType::Reference("java/lang/Object".to_string()),
                        &local_names,
                    ),
                    value: val,
                });
            }

            // Wide stores
            Instruction::IstoreWide(idx) => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(*idx, JvmType::Int, &local_names),
                    value: val,
                });
            }
            Instruction::LstoreWide(idx) => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(*idx, JvmType::Long, &local_names),
                    value: val,
                });
            }
            Instruction::FstoreWide(idx) => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(*idx, JvmType::Float, &local_names),
                    value: val,
                });
            }
            Instruction::DstoreWide(idx) => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(*idx, JvmType::Double, &local_names),
                    value: val,
                });
            }
            Instruction::AstoreWide(idx) => {
                let val = pop!(stack);
                stmts.push(Stmt::LocalStore {
                    var: make_local(
                        *idx,
                        JvmType::Reference("java/lang/Object".to_string()),
                        &local_names,
                    ),
                    value: val,
                });
            }

            // ============================================================
            // Array loads
            // ============================================================
            Instruction::Iaload => {
                let index = pop!(stack);
                let array = pop!(stack);
                stack.push(Expr::ArrayLoad {
                    array: Box::new(array),
                    index: Box::new(index),
                    element_type: JvmType::Int,
                });
            }
            Instruction::Laload => {
                let index = pop!(stack);
                let array = pop!(stack);
                stack.push(Expr::ArrayLoad {
                    array: Box::new(array),
                    index: Box::new(index),
                    element_type: JvmType::Long,
                });
            }
            Instruction::Faload => {
                let index = pop!(stack);
                let array = pop!(stack);
                stack.push(Expr::ArrayLoad {
                    array: Box::new(array),
                    index: Box::new(index),
                    element_type: JvmType::Float,
                });
            }
            Instruction::Daload => {
                let index = pop!(stack);
                let array = pop!(stack);
                stack.push(Expr::ArrayLoad {
                    array: Box::new(array),
                    index: Box::new(index),
                    element_type: JvmType::Double,
                });
            }
            Instruction::Aaload => {
                let index = pop!(stack);
                let array = pop!(stack);
                stack.push(Expr::ArrayLoad {
                    array: Box::new(array),
                    index: Box::new(index),
                    element_type: JvmType::Reference("java/lang/Object".to_string()),
                });
            }
            Instruction::Baload => {
                let index = pop!(stack);
                let array = pop!(stack);
                stack.push(Expr::ArrayLoad {
                    array: Box::new(array),
                    index: Box::new(index),
                    element_type: JvmType::Byte,
                });
            }
            Instruction::Caload => {
                let index = pop!(stack);
                let array = pop!(stack);
                stack.push(Expr::ArrayLoad {
                    array: Box::new(array),
                    index: Box::new(index),
                    element_type: JvmType::Char,
                });
            }
            Instruction::Saload => {
                let index = pop!(stack);
                let array = pop!(stack);
                stack.push(Expr::ArrayLoad {
                    array: Box::new(array),
                    index: Box::new(index),
                    element_type: JvmType::Short,
                });
            }

            // ============================================================
            // Array stores
            // ============================================================
            Instruction::Iastore
            | Instruction::Lastore
            | Instruction::Fastore
            | Instruction::Dastore
            | Instruction::Aastore
            | Instruction::Bastore
            | Instruction::Castore
            | Instruction::Sastore => {
                let value = pop!(stack);
                let index = pop!(stack);
                let array = pop!(stack);
                stmts.push(Stmt::ArrayStore {
                    array,
                    index,
                    value,
                });
            }

            // ============================================================
            // Stack manipulation
            // ============================================================
            Instruction::Pop => {
                let val = pop!(stack);
                // If the popped value has side effects, emit it as a statement.
                if has_side_effects(&val) {
                    stmts.push(Stmt::ExprStmt(val));
                }
            }
            Instruction::Pop2 => {
                // Pop2 removes top one or two computational units.
                // We treat it as two pops for simplicity.
                let val1 = pop!(stack);
                if has_side_effects(&val1) {
                    stmts.push(Stmt::ExprStmt(val1));
                }
                if !stack.is_empty() {
                    let val2 = pop!(stack);
                    if has_side_effects(&val2) {
                        stmts.push(Stmt::ExprStmt(val2));
                    }
                }
            }
            Instruction::Dup => {
                let val = pop!(stack);
                let dup = Expr::Dup(Box::new(val.clone()));
                stack.push(val);
                stack.push(dup);
            }
            Instruction::Dupx1 => {
                // ..., value2, value1 -> ..., value1, value2, value1
                let val1 = pop!(stack);
                let val2 = pop!(stack);
                let dup = Expr::Dup(Box::new(val1.clone()));
                stack.push(dup);
                stack.push(val2);
                stack.push(val1);
            }
            Instruction::Dupx2 => {
                // ..., value3, value2, value1 -> ..., value1, value3, value2, value1
                let val1 = pop!(stack);
                let val2 = pop!(stack);
                let val3 = pop!(stack);
                let dup = Expr::Dup(Box::new(val1.clone()));
                stack.push(dup);
                stack.push(val3);
                stack.push(val2);
                stack.push(val1);
            }
            Instruction::Dup2 => {
                // ..., value2, value1 -> ..., value2, value1, value2, value1
                let val1 = pop!(stack);
                let val2 = pop!(stack);
                let dup2 = Expr::Dup(Box::new(val2.clone()));
                let dup1 = Expr::Dup(Box::new(val1.clone()));
                stack.push(val2);
                stack.push(val1);
                stack.push(dup2);
                stack.push(dup1);
            }
            Instruction::Dup2x1 => {
                // ..., value3, value2, value1 -> ..., value2, value1, value3, value2, value1
                let val1 = pop!(stack);
                let val2 = pop!(stack);
                let val3 = pop!(stack);
                let dup2 = Expr::Dup(Box::new(val2.clone()));
                let dup1 = Expr::Dup(Box::new(val1.clone()));
                stack.push(dup2);
                stack.push(dup1);
                stack.push(val3);
                stack.push(val2);
                stack.push(val1);
            }
            Instruction::Dup2x2 => {
                // ..., value4, value3, value2, value1 -> ..., value2, value1, value4, value3, value2, value1
                let val1 = pop!(stack);
                let val2 = pop!(stack);
                let val3 = pop!(stack);
                let val4 = pop!(stack);
                let dup2 = Expr::Dup(Box::new(val2.clone()));
                let dup1 = Expr::Dup(Box::new(val1.clone()));
                stack.push(dup2);
                stack.push(dup1);
                stack.push(val4);
                stack.push(val3);
                stack.push(val2);
                stack.push(val1);
            }
            Instruction::Swap => {
                let val1 = pop!(stack);
                let val2 = pop!(stack);
                stack.push(val1);
                stack.push(val2);
            }

            // ============================================================
            // Arithmetic
            // ============================================================
            Instruction::Iadd | Instruction::Ladd | Instruction::Fadd | Instruction::Dadd => {
                let right = pop!(stack);
                let left = pop!(stack);
                stack.push(Expr::BinaryOp {
                    op: BinOp::Add,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Instruction::Isub | Instruction::Lsub | Instruction::Fsub | Instruction::Dsub => {
                let right = pop!(stack);
                let left = pop!(stack);
                stack.push(Expr::BinaryOp {
                    op: BinOp::Sub,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Instruction::Imul | Instruction::Lmul | Instruction::Fmul | Instruction::Dmul => {
                let right = pop!(stack);
                let left = pop!(stack);
                stack.push(Expr::BinaryOp {
                    op: BinOp::Mul,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Instruction::Idiv | Instruction::Ldiv | Instruction::Fdiv | Instruction::Ddiv => {
                let right = pop!(stack);
                let left = pop!(stack);
                stack.push(Expr::BinaryOp {
                    op: BinOp::Div,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Instruction::Irem | Instruction::Lrem | Instruction::Frem | Instruction::Drem => {
                let right = pop!(stack);
                let left = pop!(stack);
                stack.push(Expr::BinaryOp {
                    op: BinOp::Rem,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }

            Instruction::Ineg | Instruction::Lneg | Instruction::Fneg | Instruction::Dneg => {
                let operand = pop!(stack);
                stack.push(Expr::UnaryOp {
                    op: UnaryOp::Neg,
                    operand: Box::new(operand),
                });
            }

            // ============================================================
            // Bitwise / shift
            // ============================================================
            Instruction::Ishl | Instruction::Lshl => {
                let right = pop!(stack);
                let left = pop!(stack);
                stack.push(Expr::BinaryOp {
                    op: BinOp::Shl,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Instruction::Ishr | Instruction::Lshr => {
                let right = pop!(stack);
                let left = pop!(stack);
                stack.push(Expr::BinaryOp {
                    op: BinOp::Shr,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Instruction::Iushr | Instruction::Lushr => {
                let right = pop!(stack);
                let left = pop!(stack);
                stack.push(Expr::BinaryOp {
                    op: BinOp::Ushr,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Instruction::Iand | Instruction::Land => {
                let right = pop!(stack);
                let left = pop!(stack);
                stack.push(Expr::BinaryOp {
                    op: BinOp::And,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Instruction::Ior | Instruction::Lor => {
                let right = pop!(stack);
                let left = pop!(stack);
                stack.push(Expr::BinaryOp {
                    op: BinOp::Or,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Instruction::Ixor | Instruction::Lxor => {
                let right = pop!(stack);
                let left = pop!(stack);
                stack.push(Expr::BinaryOp {
                    op: BinOp::Xor,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }

            // ============================================================
            // Conversions (casts)
            // ============================================================
            Instruction::I2l => {
                let operand = pop!(stack);
                stack.push(Expr::Cast {
                    target_type: JvmType::Long,
                    operand: Box::new(operand),
                });
            }
            Instruction::I2f => {
                let operand = pop!(stack);
                stack.push(Expr::Cast {
                    target_type: JvmType::Float,
                    operand: Box::new(operand),
                });
            }
            Instruction::I2d => {
                let operand = pop!(stack);
                stack.push(Expr::Cast {
                    target_type: JvmType::Double,
                    operand: Box::new(operand),
                });
            }
            Instruction::L2i => {
                let operand = pop!(stack);
                stack.push(Expr::Cast {
                    target_type: JvmType::Int,
                    operand: Box::new(operand),
                });
            }
            Instruction::L2f => {
                let operand = pop!(stack);
                stack.push(Expr::Cast {
                    target_type: JvmType::Float,
                    operand: Box::new(operand),
                });
            }
            Instruction::L2d => {
                let operand = pop!(stack);
                stack.push(Expr::Cast {
                    target_type: JvmType::Double,
                    operand: Box::new(operand),
                });
            }
            Instruction::F2i => {
                let operand = pop!(stack);
                stack.push(Expr::Cast {
                    target_type: JvmType::Int,
                    operand: Box::new(operand),
                });
            }
            Instruction::F2l => {
                let operand = pop!(stack);
                stack.push(Expr::Cast {
                    target_type: JvmType::Long,
                    operand: Box::new(operand),
                });
            }
            Instruction::F2d => {
                let operand = pop!(stack);
                stack.push(Expr::Cast {
                    target_type: JvmType::Double,
                    operand: Box::new(operand),
                });
            }
            Instruction::D2i => {
                let operand = pop!(stack);
                stack.push(Expr::Cast {
                    target_type: JvmType::Int,
                    operand: Box::new(operand),
                });
            }
            Instruction::D2l => {
                let operand = pop!(stack);
                stack.push(Expr::Cast {
                    target_type: JvmType::Long,
                    operand: Box::new(operand),
                });
            }
            Instruction::D2f => {
                let operand = pop!(stack);
                stack.push(Expr::Cast {
                    target_type: JvmType::Float,
                    operand: Box::new(operand),
                });
            }
            Instruction::I2b => {
                let operand = pop!(stack);
                stack.push(Expr::Cast {
                    target_type: JvmType::Byte,
                    operand: Box::new(operand),
                });
            }
            Instruction::I2c => {
                let operand = pop!(stack);
                stack.push(Expr::Cast {
                    target_type: JvmType::Char,
                    operand: Box::new(operand),
                });
            }
            Instruction::I2s => {
                let operand = pop!(stack);
                stack.push(Expr::Cast {
                    target_type: JvmType::Short,
                    operand: Box::new(operand),
                });
            }

            // ============================================================
            // Comparisons (push -1/0/1 result)
            // ============================================================
            Instruction::Lcmp => {
                let right = pop!(stack);
                let left = pop!(stack);
                stack.push(Expr::CmpResult {
                    kind: CmpKind::LCmp,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Instruction::Fcmpl => {
                let right = pop!(stack);
                let left = pop!(stack);
                stack.push(Expr::CmpResult {
                    kind: CmpKind::FCmpL,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Instruction::Fcmpg => {
                let right = pop!(stack);
                let left = pop!(stack);
                stack.push(Expr::CmpResult {
                    kind: CmpKind::FCmpG,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Instruction::Dcmpl => {
                let right = pop!(stack);
                let left = pop!(stack);
                stack.push(Expr::CmpResult {
                    kind: CmpKind::DCmpL,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Instruction::Dcmpg => {
                let right = pop!(stack);
                let left = pop!(stack);
                stack.push(Expr::CmpResult {
                    kind: CmpKind::DCmpG,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }

            // ============================================================
            // Conditional branches (set branch_condition)
            // ============================================================
            Instruction::Ifeq(_) => {
                let val = pop!(stack);
                branch_condition = Some(make_if_zero_cond(val, CompareOp::Eq));
            }
            Instruction::Ifne(_) => {
                let val = pop!(stack);
                branch_condition = Some(make_if_zero_cond(val, CompareOp::Ne));
            }
            Instruction::Iflt(_) => {
                let val = pop!(stack);
                branch_condition = Some(make_if_zero_cond(val, CompareOp::Lt));
            }
            Instruction::Ifge(_) => {
                let val = pop!(stack);
                branch_condition = Some(make_if_zero_cond(val, CompareOp::Ge));
            }
            Instruction::Ifgt(_) => {
                let val = pop!(stack);
                branch_condition = Some(make_if_zero_cond(val, CompareOp::Gt));
            }
            Instruction::Ifle(_) => {
                let val = pop!(stack);
                branch_condition = Some(make_if_zero_cond(val, CompareOp::Le));
            }

            Instruction::IfIcmpeq(_) => {
                let right = pop!(stack);
                let left = pop!(stack);
                branch_condition = Some(Expr::Compare {
                    op: CompareOp::Eq,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Instruction::IfIcmpne(_) => {
                let right = pop!(stack);
                let left = pop!(stack);
                branch_condition = Some(Expr::Compare {
                    op: CompareOp::Ne,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Instruction::IfIcmplt(_) => {
                let right = pop!(stack);
                let left = pop!(stack);
                branch_condition = Some(Expr::Compare {
                    op: CompareOp::Lt,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Instruction::IfIcmpge(_) => {
                let right = pop!(stack);
                let left = pop!(stack);
                branch_condition = Some(Expr::Compare {
                    op: CompareOp::Ge,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Instruction::IfIcmpgt(_) => {
                let right = pop!(stack);
                let left = pop!(stack);
                branch_condition = Some(Expr::Compare {
                    op: CompareOp::Gt,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Instruction::IfIcmple(_) => {
                let right = pop!(stack);
                let left = pop!(stack);
                branch_condition = Some(Expr::Compare {
                    op: CompareOp::Le,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }

            Instruction::IfAcmpeq(_) => {
                let right = pop!(stack);
                let left = pop!(stack);
                branch_condition = Some(Expr::Compare {
                    op: CompareOp::Eq,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }
            Instruction::IfAcmpne(_) => {
                let right = pop!(stack);
                let left = pop!(stack);
                branch_condition = Some(Expr::Compare {
                    op: CompareOp::Ne,
                    left: Box::new(left),
                    right: Box::new(right),
                });
            }

            Instruction::Ifnull(_) => {
                let val = pop!(stack);
                branch_condition = Some(Expr::Compare {
                    op: CompareOp::Eq,
                    left: Box::new(val),
                    right: Box::new(Expr::NullLiteral),
                });
            }
            Instruction::Ifnonnull(_) => {
                let val = pop!(stack);
                branch_condition = Some(Expr::Compare {
                    op: CompareOp::Ne,
                    left: Box::new(val),
                    right: Box::new(Expr::NullLiteral),
                });
            }

            // ============================================================
            // Unconditional branches (goto, tableswitch, lookupswitch)
            // These are terminators; the stack state is the exit_stack.
            // ============================================================
            Instruction::Goto(_) | Instruction::GotoW(_) => {
                // No stack effect; terminator is already recorded in the block.
            }

            Instruction::Tableswitch { .. } | Instruction::Lookupswitch { .. } => {
                // The switch key is popped from the stack.
                let _key = pop!(stack);
            }

            // ============================================================
            // iinc
            // ============================================================
            Instruction::Iinc { index, value } => {
                stmts.push(Stmt::Iinc {
                    var: make_local(*index as u16, JvmType::Int, &local_names),
                    amount: *value as i32,
                });
            }
            Instruction::IincWide { index, value } => {
                stmts.push(Stmt::Iinc {
                    var: make_local(*index, JvmType::Int, &local_names),
                    amount: *value as i32,
                });
            }

            // ============================================================
            // Field access
            // ============================================================
            Instruction::Getfield(idx) => {
                let (class_name, field_name, field_type) = resolve_field_ref(const_pool, *idx);
                let object = pop!(stack);
                stack.push(Expr::FieldGet {
                    object: Some(Box::new(object)),
                    class_name,
                    field_name,
                    field_type,
                });
            }
            Instruction::Getstatic(idx) => {
                let (class_name, field_name, field_type) = resolve_field_ref(const_pool, *idx);
                stack.push(Expr::FieldGet {
                    object: None,
                    class_name,
                    field_name,
                    field_type,
                });
            }
            Instruction::Putfield(idx) => {
                let (class_name, field_name, field_type) = resolve_field_ref(const_pool, *idx);
                let value = pop!(stack);
                let object = pop!(stack);
                stmts.push(Stmt::FieldStore {
                    object: Some(object),
                    class_name,
                    field_name,
                    field_type,
                    value,
                });
            }
            Instruction::Putstatic(idx) => {
                let (class_name, field_name, field_type) = resolve_field_ref(const_pool, *idx);
                let value = pop!(stack);
                stmts.push(Stmt::FieldStore {
                    object: None,
                    class_name,
                    field_name,
                    field_type,
                    value,
                });
            }

            // ============================================================
            // Method invocation
            // ============================================================
            Instruction::Invokevirtual(idx) => {
                let (class_name, method_name, descriptor, param_types, return_type) =
                    resolve_method_ref(const_pool, *idx);
                let args = pop_args(&mut stack, &param_types);
                let object = pop!(stack);
                let call = Expr::MethodCall {
                    kind: InvokeKind::Virtual,
                    object: Some(Box::new(object)),
                    class_name,
                    method_name,
                    descriptor,
                    args,
                    return_type: return_type.clone(),
                };
                push_or_emit_call(call, &return_type, &mut stack, &mut stmts);
            }

            Instruction::Invokespecial(idx) => {
                let (class_name, method_name, descriptor, param_types, return_type) =
                    resolve_method_ref(const_pool, *idx);
                let args = pop_args(&mut stack, &param_types);
                let receiver = pop!(stack);

                if method_name == "<init>" {
                    // Detect new;dup;invokespecial <init> pattern: collapse to Expr::New.
                    match receiver {
                        Expr::Dup(inner) => match *inner {
                            Expr::UninitNew { class_name: ref cn } => {
                                let new_expr = Expr::New {
                                    class_name: cn.clone(),
                                    constructor_descriptor: descriptor,
                                    args,
                                };
                                // The dup placed a copy on the stack; replace the original
                                // UninitNew that is still on the stack with the New expression.
                                replace_uninit_new(&mut stack, cn, &new_expr);
                                stack.push(new_expr);
                            }
                            _ => {
                                // Calling <init> on something other than a fresh `new` (e.g., super() or this())
                                let call = Expr::MethodCall {
                                    kind: InvokeKind::Special,
                                    object: Some(Box::new(Expr::Dup(inner))),
                                    class_name,
                                    method_name,
                                    descriptor,
                                    args,
                                    return_type: return_type.clone(),
                                };
                                stmts.push(Stmt::ExprStmt(call));
                            }
                        },
                        Expr::UninitNew { ref class_name } => {
                            // new without dup; the result is discarded or stored immediately.
                            let new_expr = Expr::New {
                                class_name: class_name.clone(),
                                constructor_descriptor: descriptor,
                                args,
                            };
                            stack.push(new_expr);
                        }
                        Expr::This => {
                            // super.<init> or this() call
                            let call = Expr::MethodCall {
                                kind: InvokeKind::Special,
                                object: Some(Box::new(Expr::This)),
                                class_name,
                                method_name,
                                descriptor,
                                args,
                                return_type: return_type.clone(),
                            };
                            stmts.push(Stmt::ExprStmt(call));
                        }
                        _ => {
                            // Generic invokespecial <init> on unknown receiver
                            let call = Expr::MethodCall {
                                kind: InvokeKind::Special,
                                object: Some(Box::new(receiver)),
                                class_name,
                                method_name,
                                descriptor,
                                args,
                                return_type: return_type.clone(),
                            };
                            stmts.push(Stmt::ExprStmt(call));
                        }
                    }
                } else {
                    // Non-<init> invokespecial (private methods, super calls)
                    let call = Expr::MethodCall {
                        kind: InvokeKind::Special,
                        object: Some(Box::new(receiver)),
                        class_name,
                        method_name,
                        descriptor,
                        args,
                        return_type: return_type.clone(),
                    };
                    push_or_emit_call(call, &return_type, &mut stack, &mut stmts);
                }
            }

            Instruction::Invokestatic(idx) => {
                let (class_name, method_name, descriptor, param_types, return_type) =
                    resolve_method_ref(const_pool, *idx);
                let args = pop_args(&mut stack, &param_types);
                let call = Expr::MethodCall {
                    kind: InvokeKind::Static,
                    object: None,
                    class_name,
                    method_name,
                    descriptor,
                    args,
                    return_type: return_type.clone(),
                };
                push_or_emit_call(call, &return_type, &mut stack, &mut stmts);
            }

            Instruction::Invokeinterface { index, .. } => {
                let (class_name, method_name, descriptor, param_types, return_type) =
                    resolve_method_ref(const_pool, *index);
                let args = pop_args(&mut stack, &param_types);
                let object = pop!(stack);
                let call = Expr::MethodCall {
                    kind: InvokeKind::Interface,
                    object: Some(Box::new(object)),
                    class_name,
                    method_name,
                    descriptor,
                    args,
                    return_type: return_type.clone(),
                };
                push_or_emit_call(call, &return_type, &mut stack, &mut stmts);
            }

            Instruction::Invokedynamic { index, .. } => {
                // Resolve the InvokeDynamic constant pool entry.
                let (bootstrap_index, method_name, descriptor, param_types, return_type) =
                    resolve_invokedynamic(const_pool, *index);
                let captures = pop_args(&mut stack, &param_types);
                let expr = Expr::InvokeDynamic {
                    bootstrap_index,
                    method_name,
                    descriptor: descriptor.clone(),
                    captures,
                };
                if return_type == JvmType::Void {
                    stmts.push(Stmt::ExprStmt(expr));
                } else {
                    stack.push(expr);
                }
            }

            // ============================================================
            // Object creation
            // ============================================================
            Instruction::New(idx) => {
                let class_name = util::get_class_name(const_pool, *idx)
                    .unwrap_or("<unknown>")
                    .to_string();
                stack.push(Expr::UninitNew { class_name });
            }

            Instruction::Newarray(atype) => {
                let length = pop!(stack);
                let element_type = newarray_type(*atype);
                stack.push(Expr::NewArray {
                    element_type,
                    length: Box::new(length),
                });
            }

            Instruction::Anewarray(idx) => {
                let length = pop!(stack);
                let class_name = util::get_class_name(const_pool, *idx)
                    .unwrap_or("java/lang/Object")
                    .to_string();
                stack.push(Expr::NewArray {
                    element_type: JvmType::Reference(class_name),
                    length: Box::new(length),
                });
            }

            Instruction::Multianewarray { index, dimensions } => {
                let dim_count = *dimensions as usize;
                let mut dims = Vec::with_capacity(dim_count);
                for _ in 0..dim_count {
                    dims.push(pop!(stack));
                }
                dims.reverse();
                let class_name = util::get_class_name(const_pool, *index)
                    .unwrap_or("[Ljava/lang/Object;")
                    .to_string();
                let element_type = parse_type_descriptor(&class_name).unwrap_or(JvmType::Unknown);
                stack.push(Expr::NewMultiArray {
                    element_type,
                    dimensions: dims,
                });
            }

            // ============================================================
            // Misc object operations
            // ============================================================
            Instruction::Arraylength => {
                let array = pop!(stack);
                stack.push(Expr::ArrayLength {
                    array: Box::new(array),
                });
            }

            Instruction::Checkcast(idx) => {
                let operand = pop!(stack);
                let class_name = util::get_class_name(const_pool, *idx)
                    .unwrap_or("java/lang/Object")
                    .to_string();
                stack.push(Expr::Cast {
                    target_type: JvmType::Reference(class_name),
                    operand: Box::new(operand),
                });
            }

            Instruction::Instanceof(idx) => {
                let operand = pop!(stack);
                let class_name = util::get_class_name(const_pool, *idx)
                    .unwrap_or("java/lang/Object")
                    .to_string();
                stack.push(Expr::Instanceof {
                    operand: Box::new(operand),
                    check_type: class_name,
                });
            }

            // ============================================================
            // Monitor
            // ============================================================
            Instruction::Monitorenter => {
                let object = pop!(stack);
                stmts.push(Stmt::Monitor {
                    enter: true,
                    object,
                });
            }
            Instruction::Monitorexit => {
                let object = pop!(stack);
                stmts.push(Stmt::Monitor {
                    enter: false,
                    object,
                });
            }

            // ============================================================
            // Returns
            // ============================================================
            Instruction::Return => {
                stmts.push(Stmt::Return(None));
            }
            Instruction::Ireturn
            | Instruction::Lreturn
            | Instruction::Freturn
            | Instruction::Dreturn
            | Instruction::Areturn => {
                let val = pop!(stack);
                stmts.push(Stmt::Return(Some(val)));
            }

            // ============================================================
            // Throw
            // ============================================================
            Instruction::Athrow => {
                let val = pop!(stack);
                stmts.push(Stmt::Throw(val));
            }

            // ============================================================
            // Nop
            // ============================================================
            Instruction::Nop => {}

            // ============================================================
            // jsr/ret (legacy, used for finally blocks in old javac)
            // ============================================================
            Instruction::Jsr(_) | Instruction::JsrW(_) => {
                // Push the return address as an unresolved marker.
                stack.push(Expr::Unresolved("jsr_return_address".to_string()));
            }
            Instruction::Ret(_) | Instruction::RetWide(_) => {
                // ret returns to the jsr caller; no stack effect modeled.
            }

            // Catch-all for any unhandled instruction
            #[allow(unreachable_patterns)]
            other => {
                stack.push(Expr::Unresolved(format!("{:?}", other)));
            }
        }
    }

    SimulatedBlock {
        id: block.id,
        statements: stmts,
        exit_stack: stack,
        terminator: block.terminator.clone(),
        branch_condition,
    }
}

/// Simulate all blocks in a control flow graph.
pub fn simulate_all_blocks(
    cfg: &ControlFlowGraph,
    const_pool: &[ConstantInfo],
    code_attr: &CodeAttribute,
    is_static: bool,
) -> Vec<SimulatedBlock> {
    cfg.blocks
        .values()
        .map(|block| simulate_block(block, const_pool, code_attr, is_static))
        .collect()
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Build a branch condition for `if<cond>` opcodes that compare against zero.
/// If the operand is already a CmpResult (from lcmp/fcmp/dcmp), we can
/// fold the comparison into a direct Compare expression.
fn make_if_zero_cond(val: Expr, op: CompareOp) -> Expr {
    match val {
        Expr::CmpResult {
            kind: _,
            ref left,
            ref right,
        } => {
            // The cmp result is compared to 0 with the given op.
            // We can fold this: e.g., `fcmpl(a, b) < 0` becomes `a < b`.
            Expr::Compare {
                op,
                left: left.clone(),
                right: right.clone(),
            }
        }
        _ => Expr::Compare {
            op,
            left: Box::new(val),
            right: Box::new(Expr::IntLiteral(0)),
        },
    }
}

/// Pop `n` arguments from the stack (right-to-left in JVM order).
/// Returns them in left-to-right order for display.
fn pop_args(stack: &mut Vec<Expr>, param_types: &[JvmType]) -> Vec<Expr> {
    let n = param_types.len();
    let mut args = Vec::with_capacity(n);
    for _ in 0..n {
        args.push(
            stack
                .pop()
                .unwrap_or(Expr::Unresolved("missing_arg".to_string())),
        );
    }
    args.reverse();
    args
}

/// If a method returns void, emit the call as a statement; otherwise push the result.
fn push_or_emit_call(
    call: Expr,
    return_type: &JvmType,
    stack: &mut Vec<Expr>,
    stmts: &mut Vec<Stmt>,
) {
    if *return_type == JvmType::Void {
        stmts.push(Stmt::ExprStmt(call));
    } else {
        stack.push(call);
    }
}

/// Replace the topmost UninitNew with a matching class name on the stack.
/// This handles the pattern: new Foo -> dup -> args -> invokespecial <init>
/// After we collapse the dup+invokespecial into Expr::New, we need to
/// replace the original UninitNew that was left below the dup.
fn replace_uninit_new(stack: &mut Vec<Expr>, class_name: &str, replacement: &Expr) {
    for item in stack.iter_mut().rev() {
        if let Expr::UninitNew { class_name: cn } = item
            && cn == class_name
        {
            *item = replacement.clone();
            return;
        }
    }
}

/// Resolve an InvokeDynamic constant pool entry.
/// Returns (bootstrap_method_attr_index, method_name, descriptor, param_types, return_type).
fn resolve_invokedynamic(
    const_pool: &[ConstantInfo],
    index: u16,
) -> (u16, String, String, Vec<JvmType>, JvmType) {
    match const_pool.get((index as usize).wrapping_sub(1)) {
        Some(ConstantInfo::InvokeDynamic(indy)) => {
            if let Some((name, desc)) =
                util::get_name_and_type(const_pool, indy.name_and_type_index)
            {
                let (params, ret) =
                    parse_method_descriptor(desc).unwrap_or_else(|| (vec![], JvmType::Unknown));
                (
                    indy.bootstrap_method_attr_index,
                    name.to_string(),
                    desc.to_string(),
                    params,
                    ret,
                )
            } else {
                (
                    indy.bootstrap_method_attr_index,
                    format!("<indy#{}>", index),
                    String::new(),
                    vec![],
                    JvmType::Unknown,
                )
            }
        }
        _ => (
            0,
            format!("<indy?#{}>", index),
            String::new(),
            vec![],
            JvmType::Unknown,
        ),
    }
}

/// Heuristic: does this expression likely have side effects?
/// Used to decide whether to emit a popped value as a statement.
fn has_side_effects(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::MethodCall { .. }
            | Expr::New { .. }
            | Expr::InvokeDynamic { .. }
            | Expr::Unresolved(_)
    )
}
