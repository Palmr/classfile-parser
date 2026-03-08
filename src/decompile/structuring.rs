use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use super::cfg_types::*;
use super::expr::*;
use super::structured_types::*;

/// Convert a CFG with simulated blocks into a structured body.
pub fn structure_method(
    cfg: &ControlFlowGraph,
    simulated: &[SimulatedBlock],
    const_pool: &[crate::constant_info::ConstantInfo],
) -> StructuredBody {
    let sim_map: BTreeMap<BlockId, &SimulatedBlock> = simulated.iter().map(|b| (b.id, b)).collect();

    if cfg.blocks.is_empty() {
        return StructuredBody::new(vec![]);
    }

    let rpo = cfg.reverse_postorder();
    let dominators = compute_dominators(cfg, &rpo);
    let loop_headers = find_loop_headers(cfg, &dominators);
    let post_dominators = compute_post_dominators(cfg, &rpo);

    let mut ctx = StructuringContext {
        cfg,
        sim_map: &sim_map,
        dominators,
        post_dominators,
        loop_headers,
        const_pool,
        visited: HashSet::new(),
        label_counter: 0,
    };

    let stmts = ctx.structure_region(&rpo);
    StructuredBody::new(stmts)
}

struct StructuringContext<'a> {
    cfg: &'a ControlFlowGraph,
    sim_map: &'a BTreeMap<BlockId, &'a SimulatedBlock>,
    dominators: HashMap<BlockId, BlockId>,
    post_dominators: HashMap<BlockId, BlockId>,
    loop_headers: HashSet<BlockId>,
    const_pool: &'a [crate::constant_info::ConstantInfo],
    visited: HashSet<BlockId>,
    label_counter: usize,
}

impl<'a> StructuringContext<'a> {
    fn fresh_label(&mut self) -> String {
        self.label_counter += 1;
        format!("label{}", self.label_counter)
    }

    fn structure_region(&mut self, order: &[BlockId]) -> Vec<StructuredStmt> {
        let mut result = Vec::new();

        for &block_id in order {
            if self.visited.contains(&block_id) {
                continue;
            }
            self.visited.insert(block_id);

            let sim = match self.sim_map.get(&block_id) {
                Some(s) => *s,
                None => continue,
            };

            // Emit the block's statements
            for stmt in &sim.statements {
                result.push(StructuredStmt::Simple(stmt.clone()));
            }

            // Structure the terminator
            match &sim.terminator {
                Terminator::Return | Terminator::Throw => {
                    // Statements already include the Return/Throw
                }
                Terminator::FallThrough { target } => {
                    if !self.visited.contains(target) && self.loop_headers.contains(target) {
                        // Back-edge to a loop header — emit continue
                        result.push(StructuredStmt::Continue { label: None });
                    }
                    // Otherwise the next block in order will handle it
                }
                Terminator::Goto { target } => {
                    if self.visited.contains(target) {
                        if self.loop_headers.contains(target) {
                            result.push(StructuredStmt::Continue { label: None });
                        } else {
                            result.push(StructuredStmt::UnstructuredGoto { target: *target });
                        }
                    }
                    // Forward goto is handled by visiting the target later
                }
                Terminator::ConditionalBranch {
                    condition: _,
                    if_true,
                    if_false,
                } => {
                    let cond_expr = sim
                        .branch_condition
                        .clone()
                        .unwrap_or_else(|| Expr::Unresolved("/* condition */".into()));

                    let if_true = *if_true;
                    let if_false = *if_false;

                    // Check if this is a loop header
                    if self.loop_headers.contains(&block_id) {
                        let body_entry = if_true;
                        let exit = if_false;

                        // Check which branch goes back (is the loop body)
                        let (body_start, loop_exit, negate) = if self.dominates(block_id, if_true)
                            && !self.visited.contains(&if_true)
                        {
                            (if_true, if_false, false)
                        } else if self.dominates(block_id, if_false)
                            && !self.visited.contains(&if_false)
                        {
                            (if_false, if_true, true)
                        } else {
                            (body_entry, exit, false)
                        };

                        let condition = if negate {
                            negate_expr(cond_expr)
                        } else {
                            cond_expr
                        };

                        // Collect loop body blocks
                        let loop_body_order = self.collect_loop_body(block_id, body_start);
                        let body_stmts = self.structure_region(&loop_body_order);

                        let body = if body_stmts.is_empty() {
                            StructuredStmt::Block(vec![])
                        } else {
                            StructuredStmt::Block(body_stmts)
                        };

                        result.push(StructuredStmt::While {
                            condition,
                            body: Box::new(body),
                        });

                        // Continue with the loop exit
                        if !self.visited.contains(&loop_exit) {
                            let exit_stmts = self.structure_region(&[loop_exit]);
                            result.extend(exit_stmts);
                        }
                    } else {
                        // If-else structure
                        let join_point = self.post_dominators.get(&block_id).copied();

                        let then_stmts = if !self.visited.contains(&if_true) {
                            let then_order = self.collect_until(if_true, join_point);
                            self.structure_region(&then_order)
                        } else {
                            vec![]
                        };

                        let else_stmts = if !self.visited.contains(&if_false) {
                            let else_order = self.collect_until(if_false, join_point);
                            self.structure_region(&else_order)
                        } else {
                            vec![]
                        };

                        let then_body = Box::new(StructuredStmt::Block(then_stmts));
                        let else_body = if else_stmts.is_empty() {
                            None
                        } else {
                            Some(Box::new(StructuredStmt::Block(else_stmts)))
                        };

                        result.push(StructuredStmt::If {
                            condition: cond_expr,
                            then_body,
                            else_body,
                        });

                        // Continue with the join point
                        if let Some(jp) = join_point
                            && !self.visited.contains(&jp)
                        {
                            let jp_stmts = self.structure_region(&[jp]);
                            result.extend(jp_stmts);
                        }
                    }
                }
                Terminator::TableSwitch {
                    default,
                    low,
                    high: _,
                    targets,
                } => {
                    let switch_expr = sim
                        .exit_stack
                        .last()
                        .cloned()
                        .unwrap_or(Expr::Unresolved("/* switch expr */".into()));

                    let mut cases = Vec::new();
                    for (i, &target) in targets.iter().enumerate() {
                        let value = *low + i as i32;
                        if !self.visited.contains(&target) {
                            self.visited.insert(target);
                            let body_stmts = if let Some(s) = self.sim_map.get(&target) {
                                s.statements
                                    .iter()
                                    .map(|st| StructuredStmt::Simple(st.clone()))
                                    .collect()
                            } else {
                                vec![]
                            };
                            cases.push(SwitchCase {
                                values: vec![SwitchValue::Int(value)],
                                body: StructuredStmt::Block(body_stmts),
                                falls_through: false,
                            });
                        }
                    }

                    let default_body = if !self.visited.contains(default) {
                        self.visited.insert(*default);
                        self.sim_map.get(default).map(|s| {
                            Box::new(StructuredStmt::Block(
                                s.statements
                                    .iter()
                                    .map(|st| StructuredStmt::Simple(st.clone()))
                                    .collect(),
                            ))
                        })
                    } else {
                        None
                    };

                    result.push(StructuredStmt::Switch {
                        expr: switch_expr,
                        cases,
                        default: default_body,
                    });
                }
                Terminator::LookupSwitch { default, pairs } => {
                    let switch_expr = sim
                        .exit_stack
                        .last()
                        .cloned()
                        .unwrap_or(Expr::Unresolved("/* switch expr */".into()));

                    let mut cases = Vec::new();
                    for (key, target) in pairs {
                        if !self.visited.contains(target) {
                            self.visited.insert(*target);
                            let body_stmts = if let Some(s) = self.sim_map.get(target) {
                                s.statements
                                    .iter()
                                    .map(|st| StructuredStmt::Simple(st.clone()))
                                    .collect()
                            } else {
                                vec![]
                            };
                            cases.push(SwitchCase {
                                values: vec![SwitchValue::Int(*key)],
                                body: StructuredStmt::Block(body_stmts),
                                falls_through: false,
                            });
                        }
                    }

                    let default_body = if !self.visited.contains(default) {
                        self.visited.insert(*default);
                        self.sim_map.get(default).map(|s| {
                            Box::new(StructuredStmt::Block(
                                s.statements
                                    .iter()
                                    .map(|st| StructuredStmt::Simple(st.clone()))
                                    .collect(),
                            ))
                        })
                    } else {
                        None
                    };

                    result.push(StructuredStmt::Switch {
                        expr: switch_expr,
                        cases,
                        default: default_body,
                    });
                }
                Terminator::Jsr { .. } => {
                    result.push(StructuredStmt::Comment("/* jsr subroutine */".into()));
                }
            }
        }

        // Handle exception edges -> try-catch
        self.structure_exception_handlers(&mut result);

        result
    }

    fn structure_exception_handlers(&self, _result: &mut Vec<StructuredStmt>) {
        // Exception handler structuring is done at a higher level in class_decompiler
        // when we have full context. For now, exception edges create additional entry
        // points that are visited as part of the normal flow.
    }

    fn dominates(&self, a: BlockId, b: BlockId) -> bool {
        let mut current = b;
        loop {
            if current == a {
                return true;
            }
            match self.dominators.get(&current) {
                Some(&dom) if dom != current => current = dom,
                _ => return false,
            }
        }
    }

    fn collect_loop_body(&self, header: BlockId, body_start: BlockId) -> Vec<BlockId> {
        // Collect all blocks reachable from body_start that are dominated by header
        let mut body = Vec::new();
        let mut worklist = vec![body_start];
        let mut seen = HashSet::new();
        seen.insert(header); // Don't re-visit the header

        while let Some(bid) = worklist.pop() {
            if !seen.insert(bid) {
                continue;
            }
            if !self.cfg.blocks.contains_key(&bid) {
                continue;
            }
            body.push(bid);
            for succ in self.cfg.successors(bid) {
                if !seen.contains(&succ) {
                    worklist.push(succ);
                }
            }
        }
        body.sort();
        body
    }

    fn collect_until(&self, start: BlockId, stop: Option<BlockId>) -> Vec<BlockId> {
        let mut result = Vec::new();
        let mut worklist = vec![start];
        let mut seen = HashSet::new();

        while let Some(bid) = worklist.pop() {
            if let Some(stop_id) = stop
                && bid == stop_id
            {
                continue;
            }
            if !seen.insert(bid) {
                continue;
            }
            if !self.cfg.blocks.contains_key(&bid) {
                continue;
            }
            result.push(bid);
            for succ in self.cfg.successors(bid) {
                if !seen.contains(&succ) {
                    worklist.push(succ);
                }
            }
        }
        result.sort();
        result
    }
}

/// Negate an expression (for inverting branch conditions).
pub fn negate_expr(expr: Expr) -> Expr {
    match expr {
        Expr::Compare { op, left, right } => Expr::Compare {
            op: op.negate(),
            left,
            right,
        },
        Expr::UnaryOp {
            op: UnaryOp::Not,
            operand,
        } => *operand,
        other => Expr::UnaryOp {
            op: UnaryOp::Not,
            operand: Box::new(other),
        },
    }
}

/// Compute immediate dominators using a simple iterative algorithm.
fn compute_dominators(cfg: &ControlFlowGraph, rpo: &[BlockId]) -> HashMap<BlockId, BlockId> {
    let mut doms: HashMap<BlockId, BlockId> = HashMap::new();
    let entry = cfg.entry;
    doms.insert(entry, entry);

    let rpo_index: HashMap<BlockId, usize> = rpo.iter().enumerate().map(|(i, &b)| (b, i)).collect();

    let mut changed = true;
    while changed {
        changed = false;
        for &b in rpo {
            if b == entry {
                continue;
            }
            let preds = cfg.predecessors(b);
            let mut new_idom: Option<BlockId> = None;
            for p in &preds {
                if !doms.contains_key(p) {
                    continue;
                }
                new_idom = Some(match new_idom {
                    None => *p,
                    Some(current) => intersect(&doms, &rpo_index, current, *p),
                });
            }
            if let Some(idom) = new_idom
                && doms.get(&b) != Some(&idom)
            {
                doms.insert(b, idom);
                changed = true;
            }
        }
    }

    doms
}

fn intersect(
    doms: &HashMap<BlockId, BlockId>,
    rpo_index: &HashMap<BlockId, usize>,
    mut b1: BlockId,
    mut b2: BlockId,
) -> BlockId {
    while b1 != b2 {
        let idx1 = rpo_index.get(&b1).copied().unwrap_or(usize::MAX);
        let idx2 = rpo_index.get(&b2).copied().unwrap_or(usize::MAX);
        if idx1 > idx2 {
            b1 = *doms.get(&b1).unwrap_or(&b1);
        } else {
            b2 = *doms.get(&b2).unwrap_or(&b2);
        }
    }
    b1
}

/// Compute post-dominators (dominators of the reverse CFG).
fn compute_post_dominators(cfg: &ControlFlowGraph, _rpo: &[BlockId]) -> HashMap<BlockId, BlockId> {
    // Find exit blocks (Return/Throw terminators)
    let _exit_blocks: Vec<BlockId> = cfg
        .blocks
        .iter()
        .filter(|(_, b)| matches!(b.terminator, Terminator::Return | Terminator::Throw))
        .map(|(&id, _)| id)
        .collect();

    // Simple post-dominator: for each block with a ConditionalBranch,
    // find the nearest block where both branches reconverge
    let mut post_doms = HashMap::new();

    for (&block_id, block) in &cfg.blocks {
        if let Terminator::ConditionalBranch {
            if_true, if_false, ..
        } = &block.terminator
        {
            // Find the first block reachable from both branches
            let reachable_true = reachable_set(cfg, *if_true);
            let reachable_false = reachable_set(cfg, *if_false);
            let common: BTreeSet<BlockId> = reachable_true
                .intersection(&reachable_false)
                .copied()
                .collect();
            // The post-dominator is the first common block in order
            if let Some(&first_common) = common.iter().next() {
                post_doms.insert(block_id, first_common);
            }
        }
    }

    post_doms
}

fn reachable_set(cfg: &ControlFlowGraph, start: BlockId) -> BTreeSet<BlockId> {
    let mut visited = BTreeSet::new();
    let mut worklist = vec![start];
    while let Some(b) = worklist.pop() {
        if !visited.insert(b) {
            continue;
        }
        if let Some(_block) = cfg.blocks.get(&b) {
            for succ in cfg.successors(b) {
                worklist.push(succ);
            }
        }
    }
    visited
}

/// Find natural loop headers (blocks that are targets of back-edges).
fn find_loop_headers(
    cfg: &ControlFlowGraph,
    dominators: &HashMap<BlockId, BlockId>,
) -> HashSet<BlockId> {
    let mut headers = HashSet::new();
    for &block_id in cfg.blocks.keys() {
        for succ in cfg.successors(block_id) {
            // A back-edge is an edge where the target dominates the source
            let mut current = block_id;
            loop {
                if current == succ {
                    headers.insert(succ);
                    break;
                }
                match dominators.get(&current) {
                    Some(&dom) if dom != current => current = dom,
                    _ => break,
                }
            }
        }
    }
    headers
}
