use std::collections::BTreeMap;

use crate::code_attribute::Instruction;

// Re-export CompareOp from expr to avoid duplication
pub use super::expr::CompareOp;

/// Block ID is the bytecode offset of the first instruction in the block.
pub type BlockId = u32;

/// An instruction paired with its bytecode address.
#[derive(Clone, Debug)]
pub struct AddressedInstruction {
    pub address: u32,
    pub instruction: Instruction,
}

/// How a basic block ends.
#[derive(Clone, Debug)]
pub enum Terminator {
    FallThrough {
        target: BlockId,
    },
    Goto {
        target: BlockId,
    },
    ConditionalBranch {
        condition: BranchCondition,
        if_true: BlockId,
        if_false: BlockId,
    },
    TableSwitch {
        default: BlockId,
        low: i32,
        high: i32,
        targets: Vec<BlockId>,
    },
    LookupSwitch {
        default: BlockId,
        pairs: Vec<(i32, BlockId)>,
    },
    Return,
    Throw,
    Jsr {
        target: BlockId,
        return_addr: BlockId,
    },
}

/// The condition for a conditional branch.
#[derive(Clone, Debug)]
pub enum BranchCondition {
    IntZero(CompareOp),
    IntCompare(CompareOp),
    RefCompare(CompareOp),
    RefNull(bool),
}

/// A basic block in the CFG.
#[derive(Clone, Debug)]
pub struct BasicBlock {
    pub id: BlockId,
    pub instructions: Vec<AddressedInstruction>,
    pub terminator: Terminator,
}

/// An exception handler edge.
#[derive(Clone, Debug)]
pub struct ExceptionEdge {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_block: BlockId,
    pub catch_type: u16,
}

/// The control flow graph for a single method.
#[derive(Clone, Debug)]
pub struct ControlFlowGraph {
    pub blocks: BTreeMap<BlockId, BasicBlock>,
    pub entry: BlockId,
    pub exception_edges: Vec<ExceptionEdge>,
}

impl ControlFlowGraph {
    /// Get all successor block IDs for a given block.
    pub fn successors(&self, block_id: BlockId) -> Vec<BlockId> {
        match &self.blocks[&block_id].terminator {
            Terminator::FallThrough { target } => vec![*target],
            Terminator::Goto { target } => vec![*target],
            Terminator::ConditionalBranch {
                if_true, if_false, ..
            } => vec![*if_true, *if_false],
            Terminator::TableSwitch {
                default, targets, ..
            } => {
                let mut succs: Vec<BlockId> = targets.clone();
                succs.push(*default);
                succs.sort();
                succs.dedup();
                succs
            }
            Terminator::LookupSwitch { default, pairs, .. } => {
                let mut succs: Vec<BlockId> = pairs.iter().map(|(_, t)| *t).collect();
                succs.push(*default);
                succs.sort();
                succs.dedup();
                succs
            }
            Terminator::Return | Terminator::Throw => vec![],
            Terminator::Jsr {
                target,
                return_addr,
            } => vec![*target, *return_addr],
        }
    }

    /// Get all predecessor block IDs for a given block.
    pub fn predecessors(&self, target: BlockId) -> Vec<BlockId> {
        self.blocks
            .keys()
            .filter(|&&b| self.successors(b).contains(&target))
            .copied()
            .collect()
    }

    /// Returns block IDs in reverse postorder.
    pub fn reverse_postorder(&self) -> Vec<BlockId> {
        let mut visited = std::collections::HashSet::new();
        let mut postorder = Vec::new();
        self.dfs_postorder(self.entry, &mut visited, &mut postorder);
        postorder.reverse();
        postorder
    }

    fn dfs_postorder(
        &self,
        block: BlockId,
        visited: &mut std::collections::HashSet<BlockId>,
        postorder: &mut Vec<BlockId>,
    ) {
        if !visited.insert(block) {
            return;
        }
        for succ in self.successors(block) {
            self.dfs_postorder(succ, visited, postorder);
        }
        postorder.push(block);
    }

    /// Generate a DOT graph for visualization.
    pub fn to_dot(&self) -> String {
        let mut dot = String::from("digraph CFG {\n");
        for (id, block) in &self.blocks {
            let label = format!("B{} ({} instrs)", id, block.instructions.len());
            dot.push_str(&format!("  B{} [label=\"{}\"];\n", id, label));
            for succ in self.successors(*id) {
                dot.push_str(&format!("  B{} -> B{};\n", id, succ));
            }
        }
        for edge in &self.exception_edges {
            dot.push_str(&format!(
                "  B{} -> B{} [style=dashed, label=\"catch\"];\n",
                edge.start_pc, edge.handler_block
            ));
        }
        dot.push_str("}\n");
        dot
    }
}
