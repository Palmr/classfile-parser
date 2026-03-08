use std::collections::{BTreeMap, BTreeSet};

use crate::attribute_info::CodeAttribute;
use crate::code_attribute::Instruction;

use super::cfg_types::*;
use super::util::{compute_addresses, instruction_byte_size};

/// Build a control flow graph from a CodeAttribute.
pub fn build_cfg(code_attr: &CodeAttribute) -> ControlFlowGraph {
    let addressed = compute_addresses(&code_attr.code);
    if addressed.is_empty() {
        return ControlFlowGraph {
            blocks: BTreeMap::new(),
            entry: 0,
            exception_edges: Vec::new(),
        };
    }

    // Step 1: Identify block leaders
    let mut leaders = BTreeSet::new();
    leaders.insert(0u32);

    for ex in &code_attr.exception_table {
        leaders.insert(ex.handler_pc as u32);
    }

    for &(addr, instr) in &addressed {
        let next_addr = addr + instruction_byte_size(instr, addr);
        match instr {
            Instruction::Goto(offset) => {
                leaders.insert((addr as i64 + *offset as i64) as u32);
                leaders.insert(next_addr);
            }
            Instruction::GotoW(offset) => {
                leaders.insert((addr as i64 + *offset as i64) as u32);
                leaders.insert(next_addr);
            }
            Instruction::Ifeq(off)
            | Instruction::Ifne(off)
            | Instruction::Iflt(off)
            | Instruction::Ifge(off)
            | Instruction::Ifgt(off)
            | Instruction::Ifle(off)
            | Instruction::IfIcmpeq(off)
            | Instruction::IfIcmpne(off)
            | Instruction::IfIcmplt(off)
            | Instruction::IfIcmpge(off)
            | Instruction::IfIcmpgt(off)
            | Instruction::IfIcmple(off)
            | Instruction::IfAcmpeq(off)
            | Instruction::IfAcmpne(off)
            | Instruction::Ifnull(off)
            | Instruction::Ifnonnull(off) => {
                leaders.insert((addr as i64 + *off as i64) as u32);
                leaders.insert(next_addr);
            }
            Instruction::Tableswitch {
                default, offsets, ..
            } => {
                leaders.insert((addr as i64 + *default as i64) as u32);
                for off in offsets {
                    leaders.insert((addr as i64 + *off as i64) as u32);
                }
                leaders.insert(next_addr);
            }
            Instruction::Lookupswitch { default, pairs, .. } => {
                leaders.insert((addr as i64 + *default as i64) as u32);
                for (_, off) in pairs {
                    leaders.insert((addr as i64 + *off as i64) as u32);
                }
                leaders.insert(next_addr);
            }
            Instruction::Return
            | Instruction::Ireturn
            | Instruction::Lreturn
            | Instruction::Freturn
            | Instruction::Dreturn
            | Instruction::Areturn
            | Instruction::Athrow => {
                leaders.insert(next_addr);
            }
            Instruction::Jsr(off) => {
                leaders.insert((addr as i64 + *off as i64) as u32);
                leaders.insert(next_addr);
            }
            Instruction::JsrW(off) => {
                leaders.insert((addr as i64 + *off as i64) as u32);
                leaders.insert(next_addr);
            }
            Instruction::Ret(_) | Instruction::RetWide(_) => {
                leaders.insert(next_addr);
            }
            _ => {}
        }
    }

    // Step 2: Build basic blocks
    let leader_vec: Vec<u32> = leaders.iter().copied().collect();
    let mut blocks = BTreeMap::new();
    let addr_to_idx: BTreeMap<u32, usize> = addressed
        .iter()
        .enumerate()
        .map(|(i, (a, _))| (*a, i))
        .collect();

    for (li, &leader_addr) in leader_vec.iter().enumerate() {
        if !addr_to_idx.contains_key(&leader_addr) {
            continue;
        }

        let start_idx = addr_to_idx[&leader_addr];
        let end_idx = if li + 1 < leader_vec.len() {
            addr_to_idx
                .get(&leader_vec[li + 1])
                .copied()
                .unwrap_or(addressed.len())
        } else {
            addressed.len()
        };

        if start_idx >= end_idx {
            continue;
        }

        let block_instrs: Vec<AddressedInstruction> = addressed[start_idx..end_idx]
            .iter()
            .map(|(a, i)| AddressedInstruction {
                address: *a,
                instruction: (*i).clone(),
            })
            .collect();

        let last = block_instrs.last().unwrap();
        let last_addr = last.address;
        let last_next = last_addr + instruction_byte_size(&last.instruction, last_addr);

        let terminator = build_terminator(&last.instruction, last_addr, last_next);

        blocks.insert(
            leader_addr,
            BasicBlock {
                id: leader_addr,
                instructions: block_instrs,
                terminator,
            },
        );
    }

    // Step 3: Exception edges
    let exception_edges: Vec<ExceptionEdge> = code_attr
        .exception_table
        .iter()
        .map(|e| ExceptionEdge {
            start_pc: e.start_pc,
            end_pc: e.end_pc,
            handler_block: e.handler_pc as u32,
            catch_type: e.catch_type,
        })
        .collect();

    ControlFlowGraph {
        blocks,
        entry: 0,
        exception_edges,
    }
}

fn build_terminator(instr: &Instruction, addr: u32, next: u32) -> Terminator {
    match instr {
        Instruction::Goto(off) => Terminator::Goto {
            target: (addr as i64 + *off as i64) as u32,
        },
        Instruction::GotoW(off) => Terminator::Goto {
            target: (addr as i64 + *off as i64) as u32,
        },
        Instruction::Ifeq(off) => Terminator::ConditionalBranch {
            condition: BranchCondition::IntZero(CompareOp::Eq),
            if_true: (addr as i64 + *off as i64) as u32,
            if_false: next,
        },
        Instruction::Ifne(off) => Terminator::ConditionalBranch {
            condition: BranchCondition::IntZero(CompareOp::Ne),
            if_true: (addr as i64 + *off as i64) as u32,
            if_false: next,
        },
        Instruction::Iflt(off) => Terminator::ConditionalBranch {
            condition: BranchCondition::IntZero(CompareOp::Lt),
            if_true: (addr as i64 + *off as i64) as u32,
            if_false: next,
        },
        Instruction::Ifge(off) => Terminator::ConditionalBranch {
            condition: BranchCondition::IntZero(CompareOp::Ge),
            if_true: (addr as i64 + *off as i64) as u32,
            if_false: next,
        },
        Instruction::Ifgt(off) => Terminator::ConditionalBranch {
            condition: BranchCondition::IntZero(CompareOp::Gt),
            if_true: (addr as i64 + *off as i64) as u32,
            if_false: next,
        },
        Instruction::Ifle(off) => Terminator::ConditionalBranch {
            condition: BranchCondition::IntZero(CompareOp::Le),
            if_true: (addr as i64 + *off as i64) as u32,
            if_false: next,
        },
        Instruction::IfIcmpeq(off) => Terminator::ConditionalBranch {
            condition: BranchCondition::IntCompare(CompareOp::Eq),
            if_true: (addr as i64 + *off as i64) as u32,
            if_false: next,
        },
        Instruction::IfIcmpne(off) => Terminator::ConditionalBranch {
            condition: BranchCondition::IntCompare(CompareOp::Ne),
            if_true: (addr as i64 + *off as i64) as u32,
            if_false: next,
        },
        Instruction::IfIcmplt(off) => Terminator::ConditionalBranch {
            condition: BranchCondition::IntCompare(CompareOp::Lt),
            if_true: (addr as i64 + *off as i64) as u32,
            if_false: next,
        },
        Instruction::IfIcmpge(off) => Terminator::ConditionalBranch {
            condition: BranchCondition::IntCompare(CompareOp::Ge),
            if_true: (addr as i64 + *off as i64) as u32,
            if_false: next,
        },
        Instruction::IfIcmpgt(off) => Terminator::ConditionalBranch {
            condition: BranchCondition::IntCompare(CompareOp::Gt),
            if_true: (addr as i64 + *off as i64) as u32,
            if_false: next,
        },
        Instruction::IfIcmple(off) => Terminator::ConditionalBranch {
            condition: BranchCondition::IntCompare(CompareOp::Le),
            if_true: (addr as i64 + *off as i64) as u32,
            if_false: next,
        },
        Instruction::IfAcmpeq(off) => Terminator::ConditionalBranch {
            condition: BranchCondition::RefCompare(CompareOp::Eq),
            if_true: (addr as i64 + *off as i64) as u32,
            if_false: next,
        },
        Instruction::IfAcmpne(off) => Terminator::ConditionalBranch {
            condition: BranchCondition::RefCompare(CompareOp::Ne),
            if_true: (addr as i64 + *off as i64) as u32,
            if_false: next,
        },
        Instruction::Ifnull(off) => Terminator::ConditionalBranch {
            condition: BranchCondition::RefNull(true),
            if_true: (addr as i64 + *off as i64) as u32,
            if_false: next,
        },
        Instruction::Ifnonnull(off) => Terminator::ConditionalBranch {
            condition: BranchCondition::RefNull(false),
            if_true: (addr as i64 + *off as i64) as u32,
            if_false: next,
        },
        Instruction::Tableswitch {
            default,
            low,
            high,
            offsets,
        } => {
            let targets: Vec<u32> = offsets
                .iter()
                .map(|off| (addr as i64 + *off as i64) as u32)
                .collect();
            Terminator::TableSwitch {
                default: (addr as i64 + *default as i64) as u32,
                low: *low,
                high: *high,
                targets,
            }
        }
        Instruction::Lookupswitch { default, pairs, .. } => {
            let abs_pairs: Vec<(i32, u32)> = pairs
                .iter()
                .map(|(key, off)| (*key, (addr as i64 + *off as i64) as u32))
                .collect();
            Terminator::LookupSwitch {
                default: (addr as i64 + *default as i64) as u32,
                pairs: abs_pairs,
            }
        }
        Instruction::Return
        | Instruction::Ireturn
        | Instruction::Lreturn
        | Instruction::Freturn
        | Instruction::Dreturn
        | Instruction::Areturn => Terminator::Return,
        Instruction::Athrow => Terminator::Throw,
        Instruction::Jsr(off) => Terminator::Jsr {
            target: (addr as i64 + *off as i64) as u32,
            return_addr: next,
        },
        Instruction::JsrW(off) => Terminator::Jsr {
            target: (addr as i64 + *off as i64) as u32,
            return_addr: next,
        },
        Instruction::Ret(_) | Instruction::RetWide(_) => Terminator::Return,
        _ => Terminator::FallThrough { target: next },
    }
}
