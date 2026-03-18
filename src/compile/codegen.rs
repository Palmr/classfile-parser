use crate::ClassFile;
use crate::attribute_info::{
    AttributeInfo, AttributeInfoVariant, BootstrapMethod, BootstrapMethodsAttribute, CodeAttribute,
};
use crate::code_attribute::Instruction;
use crate::decompile::descriptor::{JvmType, parse_method_descriptor};
use crate::decompile::util::instruction_byte_size;
use crate::method_info::{MethodAccessFlags, MethodInfo};

use super::CompileError;
use super::ast::*;
use super::stackmap::{FrameTracker, VType};

/// Tracks local variable allocation.
struct LocalAllocator {
    /// (name, type, slot, vtype)
    locals: Vec<(String, TypeName, u16, VType)>,
    next_slot: u16,
}

impl LocalAllocator {
    fn new(
        is_static: bool,
        method_descriptor: &str,
        class_file: &mut ClassFile,
        param_names: &[Option<String>],
    ) -> Result<Self, CompileError> {
        let mut next_slot: u16 = 0;
        let mut locals = Vec::new();

        if !is_static {
            let vtype = VType::Object(class_file.this_class);
            locals.push(("this".into(), TypeName::Class("this".into()), 0, vtype));
            next_slot = 1;
        }

        // Parse method descriptor to pre-allocate parameter slots
        let (params, _) = parse_method_descriptor(method_descriptor).ok_or_else(|| {
            CompileError::CodegenError {
                message: format!("invalid method descriptor: {}", method_descriptor),
            }
        })?;

        for (i, param) in params.iter().enumerate() {
            let ty = jvm_type_to_type_name(param);
            let vtype = jvm_type_to_vtype_resolved(param, class_file);
            let slot = next_slot;
            // Always register positional name arg{i}
            let positional = format!("arg{}", i);
            locals.push((positional.clone(), ty.clone(), slot, vtype.clone()));
            // If a debug name is available and differs from the positional name, register as alias
            if let Some(Some(debug_name)) = param_names.get(i)
                && debug_name != &positional
            {
                locals.push((debug_name.clone(), ty, slot, vtype));
            }
            next_slot += if param.is_wide() { 2 } else { 1 };
        }

        Ok(LocalAllocator { locals, next_slot })
    }

    fn allocate(&mut self, name: &str, ty: &TypeName) -> u16 {
        let vtype = type_name_to_vtype(ty);
        self.allocate_with_vtype(name, ty, vtype)
    }

    fn allocate_with_vtype(&mut self, name: &str, ty: &TypeName, vtype: VType) -> u16 {
        let slot = self.next_slot;
        let width = type_slot_width(ty);
        self.locals
            .push((name.to_string(), ty.clone(), slot, vtype));
        self.next_slot += width;
        slot
    }

    fn find(&self, name: &str) -> Option<(u16, &TypeName)> {
        // Search from the end to support shadowing
        for (n, ty, slot, _) in self.locals.iter().rev() {
            if n == name {
                return Some((*slot, ty));
            }
        }
        None
    }

    fn max_locals(&self) -> u16 {
        self.next_slot
    }

    /// Save the current locals state for later restoration (scope support).
    /// `next_slot` is NOT saved — it only ever increases to ensure max_locals is correct.
    fn save(&self) -> Vec<(String, TypeName, u16, VType)> {
        self.locals.clone()
    }

    /// Restore locals to a previous state (scope exit). Body-scoped locals are removed,
    /// but `next_slot` stays at its high-water mark so max_locals remains correct.
    fn restore(&mut self, saved: Vec<(String, TypeName, u16, VType)>) {
        self.locals = saved;
    }

    /// Get current locals as VType array for StackMapTable generation.
    /// Returns the compressed format required by StackMapTable: Long/Double
    /// implicitly cover 2 slots, so continuation Top entries are omitted.
    fn current_locals_vtypes(&self) -> Vec<VType> {
        let mut slot_vtypes = vec![VType::Top; self.next_slot as usize];
        for (_, _, slot, vtype) in &self.locals {
            slot_vtypes[*slot as usize] = vtype.clone();
        }
        // Convert slot-indexed array to StackMapTable format:
        // Skip the implicit continuation slot after Long/Double.
        let mut vtypes = Vec::new();
        let mut i = 0;
        while i < slot_vtypes.len() {
            vtypes.push(slot_vtypes[i].clone());
            if slot_vtypes[i] == VType::Long || slot_vtypes[i] == VType::Double {
                i += 2; // skip continuation slot
            } else {
                i += 1;
            }
        }
        // Trim trailing Top values
        while vtypes.last() == Some(&VType::Top) {
            vtypes.pop();
        }
        vtypes
    }
}

struct BreakableContext {
    break_label: usize,
    is_loop: bool,
    continue_label: Option<usize>,
}

enum SwitchPatchKind {
    Table {
        low: i32,
        high: i32,
        /// Labels for offsets[0..=(high-low)], then default_label
        case_labels: Vec<usize>,
        default_label: usize,
    },
    Lookup {
        /// (match_value, label)
        pairs: Vec<(i32, usize)>,
        default_label: usize,
    },
}

struct SwitchPatch {
    instr_idx: usize,
    kind: SwitchPatchKind,
}

struct PendingExceptionEntry {
    start_label: usize,
    end_label: usize,
    handler_label: usize,
    catch_type: u16,
}

pub struct CodeGenerator<'a> {
    class_file: &'a mut ClassFile,
    instructions: Vec<Instruction>,
    locals: LocalAllocator,
    labels: Vec<Option<usize>>, // label_id → instruction index (None = unresolved)
    patches: Vec<(usize, usize)>, // (instruction_index, target_label_id)
    switch_patches: Vec<SwitchPatch>,
    breakable_stack: Vec<BreakableContext>,
    pending_exceptions: Vec<PendingExceptionEntry>,
    is_static: bool,
    return_type: JvmType,
    frame_tracker: Option<FrameTracker>,
    /// Labels that are exception handler entry points: (label_id, exception_vtype, locals_at_try_start).
    exception_handler_labels: Vec<(usize, VType, Vec<VType>)>,
    /// Labels whose frame locals should be overridden (not taken from current allocator).
    label_locals_override: Vec<(usize, Vec<VType>)>,
    /// Labels whose frame stack should be overridden (not empty).
    label_stack_override: Vec<(usize, Vec<VType>)>,
}

impl<'a> CodeGenerator<'a> {
    pub fn new(
        class_file: &'a mut ClassFile,
        is_static: bool,
        method_descriptor: &str,
        param_names: &[Option<String>],
    ) -> Result<Self, CompileError> {
        Self::new_with_options(class_file, is_static, method_descriptor, false, param_names)
    }

    pub fn new_with_options(
        class_file: &'a mut ClassFile,
        is_static: bool,
        method_descriptor: &str,
        generate_stack_map_table: bool,
        param_names: &[Option<String>],
    ) -> Result<Self, CompileError> {
        let locals = LocalAllocator::new(is_static, method_descriptor, class_file, param_names)?;
        let (params, ret) = parse_method_descriptor(method_descriptor).ok_or_else(|| {
            CompileError::CodegenError {
                message: format!("invalid method descriptor: {}", method_descriptor),
            }
        })?;

        let frame_tracker = if generate_stack_map_table {
            // Build initial locals VTypes from method descriptor
            let mut initial = Vec::new();
            if !is_static {
                // 'this' reference — resolve the class
                initial.push(VType::Object(class_file.this_class));
            }
            for param in &params {
                initial.push(jvm_type_to_vtype_resolved(param, class_file));
                // Note: Long/Double implicitly cover 2 local variable slots in
                // StackMapTable encoding. Do NOT add explicit Top continuation —
                // the JVM spec says the next entry maps to slot N+2 automatically.
            }
            Some(FrameTracker::new(initial))
        } else {
            None
        };

        Ok(CodeGenerator {
            class_file,
            instructions: Vec::new(),
            locals,
            labels: Vec::new(),
            patches: Vec::new(),
            switch_patches: Vec::new(),
            breakable_stack: Vec::new(),
            pending_exceptions: Vec::new(),
            is_static,
            return_type: ret,
            frame_tracker,
            exception_handler_labels: Vec::new(),
            label_locals_override: Vec::new(),
            label_stack_override: Vec::new(),
        })
    }

    fn new_label(&mut self) -> usize {
        let id = self.labels.len();
        self.labels.push(None);
        id
    }

    fn bind_label(&mut self, label_id: usize) {
        let instr_idx = self.instructions.len();
        self.labels[label_id] = Some(instr_idx);

        // Record frame snapshot for StackMapTable generation
        if self.frame_tracker.is_some() {
            // Check if this label is an exception handler entry point
            let exception_info = self
                .exception_handler_labels
                .iter()
                .find(|(lid, _, _)| *lid == label_id)
                .map(|(_, vtype, saved_locals)| (vtype.clone(), saved_locals.clone()));

            let (locals, stack) = if let Some((vtype, saved_locals)) = exception_info {
                // Exception handlers use the locals from try-start, not current allocator state
                (saved_locals, vec![vtype])
            } else {
                // Check for explicit locals override (e.g., merge points after try-catch)
                let overridden_locals = self
                    .label_locals_override
                    .iter()
                    .find(|(lid, _)| *lid == label_id)
                    .map(|(_, locals)| locals.clone());
                let locals =
                    overridden_locals.unwrap_or_else(|| self.locals.current_locals_vtypes());
                // Check for explicit stack override (e.g., expression-level merge points)
                let stack = self
                    .label_stack_override
                    .iter()
                    .find(|(lid, _)| *lid == label_id)
                    .map(|(_, stack)| stack.clone())
                    .unwrap_or_default();
                (locals, stack)
            };

            // We need to compute the bytecode offset for this instruction index.
            // Since instructions haven't been patched yet, compute from current instructions.
            let offset = compute_byte_offset_at(&self.instructions, instr_idx);

            if let Some(ref mut tracker) = self.frame_tracker {
                tracker.record_frame(offset, locals, stack);
            }
        }
    }

    fn emit(&mut self, instr: Instruction) -> usize {
        let idx = self.instructions.len();
        self.instructions.push(instr);
        idx
    }

    fn emit_branch(&mut self, instr_fn: fn(i16) -> Instruction, target_label: usize) {
        let idx = self.emit(instr_fn(0)); // placeholder
        self.patches.push((idx, target_label));
    }

    fn emit_goto(&mut self, target_label: usize) {
        self.emit_branch(Instruction::Goto, target_label);
    }

    /// Returns true if the last emitted instruction is an unconditional control transfer
    /// (goto, return, athrow). Used to avoid emitting dead code that would require
    /// a StackMapTable frame the JVM verifier would complain about.
    fn last_is_unconditional_transfer(&self) -> bool {
        match self.instructions.last() {
            Some(Instruction::Goto(_))
            | Some(Instruction::GotoW(_))
            | Some(Instruction::Return)
            | Some(Instruction::Ireturn)
            | Some(Instruction::Lreturn)
            | Some(Instruction::Freturn)
            | Some(Instruction::Dreturn)
            | Some(Instruction::Areturn)
            | Some(Instruction::Athrow) => true,
            _ => false,
        }
    }

    /// Resolve all branch patches using byte addresses.
    fn resolve_patches(&mut self) -> Result<(), CompileError> {
        // Compute byte addresses for each instruction
        let addresses = compute_byte_addresses(&self.instructions);

        // Compute end address (address after last instruction)
        let end_addr = if self.instructions.is_empty() {
            0i32
        } else {
            let last = *addresses.last().unwrap();
            (last + instruction_byte_size(self.instructions.last().unwrap(), last as u32)) as i32
        };

        for &(instr_idx, label_id) in &self.patches {
            let source_addr = addresses[instr_idx] as i32;
            let target_addr = resolve_label_addr(label_id, &self.labels, &addresses, end_addr)?;
            let offset = target_addr - source_addr;
            let offset16 = offset as i16;

            self.instructions[instr_idx] =
                patch_branch_offset(&self.instructions[instr_idx], offset16)?;
        }

        // Resolve switch patches
        for patch in &self.switch_patches {
            let source_addr = addresses[patch.instr_idx] as i32;
            match &patch.kind {
                SwitchPatchKind::Table {
                    low,
                    high,
                    case_labels,
                    default_label,
                } => {
                    let default_offset =
                        resolve_label_addr(*default_label, &self.labels, &addresses, end_addr)?
                            - source_addr;
                    let mut offsets = Vec::new();
                    for label_id in case_labels {
                        let addr =
                            resolve_label_addr(*label_id, &self.labels, &addresses, end_addr)?;
                        offsets.push(addr - source_addr);
                    }
                    self.instructions[patch.instr_idx] = Instruction::Tableswitch {
                        default: default_offset,
                        low: *low,
                        high: *high,
                        offsets,
                    };
                }
                SwitchPatchKind::Lookup {
                    pairs,
                    default_label,
                } => {
                    let default_offset =
                        resolve_label_addr(*default_label, &self.labels, &addresses, end_addr)?
                            - source_addr;
                    let mut resolved_pairs = Vec::new();
                    for (value, label_id) in pairs {
                        let addr =
                            resolve_label_addr(*label_id, &self.labels, &addresses, end_addr)?;
                        resolved_pairs.push((*value, addr - source_addr));
                    }
                    self.instructions[patch.instr_idx] = Instruction::Lookupswitch {
                        default: default_offset,
                        npairs: resolved_pairs.len() as u32,
                        pairs: resolved_pairs,
                    };
                }
            }
        }

        Ok(())
    }

    pub fn generate_body(&mut self, stmts: &[CStmt]) -> Result<(), CompileError> {
        for stmt in stmts {
            self.gen_stmt(stmt)?;
        }
        // If the method returns void, ensure there's a trailing return.
        // Any label pointing to `instructions.len()` (end of code) needs a
        // valid instruction there, so always emit Return for void methods
        // when the last emitted instruction isn't already a return/throw,
        // OR when there are labels that point to the end of instructions.
        if self.return_type == JvmType::Void {
            let has_label_at_end = self.labels.contains(&Some(self.instructions.len()));
            let needs_return = self.instructions.is_empty()
                || has_label_at_end
                || !matches!(
                    self.instructions.last(),
                    Some(Instruction::Return)
                        | Some(Instruction::Ireturn)
                        | Some(Instruction::Lreturn)
                        | Some(Instruction::Freturn)
                        | Some(Instruction::Dreturn)
                        | Some(Instruction::Areturn)
                        | Some(Instruction::Athrow)
                );
            if needs_return {
                self.emit(Instruction::Return);
            }
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<super::GeneratedCode, CompileError> {
        self.resolve_patches()?;
        let max_stack = super::stack_calc::compute_max_stack(&self.instructions);
        let max_locals = self.locals.max_locals();
        let exception_table = self.build_exception_table()?;
        let stack_map_table = self.frame_tracker.take().and_then(|t| t.build());
        Ok(super::GeneratedCode {
            instructions: self.instructions,
            max_stack,
            max_locals,
            exception_table,
            stack_map_table,
        })
    }

    fn build_exception_table(
        &self,
    ) -> Result<Vec<crate::attribute_info::ExceptionEntry>, CompileError> {
        use crate::attribute_info::ExceptionEntry;
        let addresses = compute_byte_addresses(&self.instructions);
        let end_addr = {
            if self.instructions.is_empty() {
                0u16
            } else {
                let last = addresses.last().copied().unwrap_or(0);
                let last_instr = &self.instructions[self.instructions.len() - 1];
                (last + instruction_byte_size(last_instr, last as u32)) as u16
            }
        };

        let mut entries = Vec::new();
        for pending in &self.pending_exceptions {
            let start_instr =
                self.labels[pending.start_label].ok_or_else(|| CompileError::CodegenError {
                    message: "unresolved exception start label".into(),
                })?;
            let end_instr =
                self.labels[pending.end_label].ok_or_else(|| CompileError::CodegenError {
                    message: "unresolved exception end label".into(),
                })?;
            let handler_instr =
                self.labels[pending.handler_label].ok_or_else(|| CompileError::CodegenError {
                    message: "unresolved exception handler label".into(),
                })?;

            let start_pc = if start_instr < addresses.len() {
                addresses[start_instr] as u16
            } else {
                end_addr
            };
            let end_pc = if end_instr < addresses.len() {
                addresses[end_instr] as u16
            } else {
                end_addr
            };
            let handler_pc = if handler_instr < addresses.len() {
                addresses[handler_instr] as u16
            } else {
                end_addr
            };

            entries.push(ExceptionEntry {
                start_pc,
                end_pc,
                handler_pc,
                catch_type: pending.catch_type,
            });
        }
        Ok(entries)
    }

    // --- Statement codegen ---

    fn gen_stmt(&mut self, stmt: &CStmt) -> Result<(), CompileError> {
        match stmt {
            CStmt::LocalDecl { ty, name, init } => {
                let resolved_ty = if is_var_sentinel(ty) {
                    match init {
                        Some(expr) => self.infer_expr_type(expr),
                        None => {
                            return Err(CompileError::CodegenError {
                                message: "'var' requires an initializer".into(),
                            });
                        }
                    }
                } else {
                    ty.clone()
                };
                let vtype = type_name_to_vtype_resolved(&resolved_ty, self.class_file);
                if let Some(expr) = init {
                    // Generate initializer BEFORE allocating the slot so that
                    // branch targets inside the initializer (ternaries, switch
                    // expressions, comparisons) don't include the unassigned
                    // local in their StackMapTable frames.
                    self.gen_expr(expr)?;
                    let slot = self.locals.allocate_with_vtype(name, &resolved_ty, vtype);
                    self.emit_store(&resolved_ty, slot);
                } else {
                    self.locals.allocate_with_vtype(name, &resolved_ty, vtype);
                }
                Ok(())
            }
            CStmt::ExprStmt(expr) => {
                self.gen_expr(expr)?;
                // Pop the value if the expression leaves one on the stack
                if self.expr_leaves_value(expr) {
                    let ty = self.infer_expr_type(expr);
                    match &ty {
                        TypeName::Primitive(PrimitiveKind::Long)
                        | TypeName::Primitive(PrimitiveKind::Double) => {
                            self.emit(Instruction::Pop2);
                        }
                        _ => {
                            self.emit(Instruction::Pop);
                        }
                    }
                }
                Ok(())
            }
            CStmt::Return(None) => {
                self.emit(Instruction::Return);
                Ok(())
            }
            CStmt::Return(Some(expr)) => {
                self.gen_expr(expr)?;
                let ret_instr = match &self.return_type {
                    JvmType::Int
                    | JvmType::Boolean
                    | JvmType::Byte
                    | JvmType::Char
                    | JvmType::Short => Instruction::Ireturn,
                    JvmType::Long => Instruction::Lreturn,
                    JvmType::Float => Instruction::Freturn,
                    JvmType::Double => Instruction::Dreturn,
                    JvmType::Reference(_) | JvmType::Array(_) | JvmType::Null => {
                        Instruction::Areturn
                    }
                    JvmType::Void => Instruction::Return,
                    JvmType::Unknown => Instruction::Areturn,
                };
                self.emit(ret_instr);
                Ok(())
            }
            CStmt::If {
                condition,
                then_body,
                else_body,
            } => {
                let false_label = self.new_label();
                let pre_branch_locals = self.locals.current_locals_vtypes();
                self.label_locals_override
                    .push((false_label, pre_branch_locals.clone()));
                self.gen_condition(condition, false_label, false)?;
                let saved = self.locals.save();
                for s in then_body {
                    self.gen_stmt(s)?;
                }
                if let Some(else_stmts) = else_body {
                    let end_label = self.new_label();
                    self.label_locals_override
                        .push((end_label, pre_branch_locals));
                    self.emit_goto(end_label);
                    self.locals.restore(saved.clone());
                    self.bind_label(false_label);
                    for s in else_stmts {
                        self.gen_stmt(s)?;
                    }
                    self.locals.restore(saved);
                    self.bind_label(end_label);
                } else {
                    self.locals.restore(saved);
                    self.bind_label(false_label);
                }
                Ok(())
            }
            CStmt::While { condition, body } => {
                let top_label = self.new_label();
                let end_label = self.new_label();
                let pre_body_locals = self.locals.current_locals_vtypes();
                self.label_locals_override
                    .push((end_label, pre_body_locals));
                self.breakable_stack.push(BreakableContext {
                    break_label: end_label,
                    is_loop: true,
                    continue_label: Some(top_label),
                });
                self.bind_label(top_label);
                self.gen_condition(condition, end_label, false)?;
                let saved = self.locals.save();
                for s in body {
                    self.gen_stmt(s)?;
                }
                self.locals.restore(saved);
                self.emit_goto(top_label);
                self.bind_label(end_label);
                self.breakable_stack.pop();
                Ok(())
            }
            CStmt::For {
                init,
                condition,
                update,
                body,
            } => {
                if let Some(init_stmt) = init {
                    self.gen_stmt(init_stmt)?;
                }
                let top_label = self.new_label();
                let update_label = self.new_label();
                let end_label = self.new_label();
                let pre_body_locals = self.locals.current_locals_vtypes();
                self.label_locals_override
                    .push((end_label, pre_body_locals.clone()));
                self.label_locals_override
                    .push((update_label, pre_body_locals));
                self.breakable_stack.push(BreakableContext {
                    break_label: end_label,
                    is_loop: true,
                    continue_label: Some(update_label),
                });
                self.bind_label(top_label);
                if let Some(cond) = condition {
                    self.gen_condition(cond, end_label, false)?;
                }
                let saved = self.locals.save();
                for s in body {
                    self.gen_stmt(s)?;
                }
                self.locals.restore(saved);
                self.bind_label(update_label);
                if let Some(upd) = update {
                    self.gen_stmt(upd)?;
                }
                self.emit_goto(top_label);
                self.bind_label(end_label);
                self.breakable_stack.pop();
                Ok(())
            }
            CStmt::Block(stmts) => {
                let saved = self.locals.save();
                for s in stmts {
                    self.gen_stmt(s)?;
                }
                self.locals.restore(saved);
                Ok(())
            }
            CStmt::Throw(expr) => {
                self.gen_expr(expr)?;
                self.emit(Instruction::Athrow);
                Ok(())
            }
            CStmt::Break => {
                let label = self
                    .breakable_stack
                    .last()
                    .ok_or_else(|| CompileError::CodegenError {
                        message: "break outside loop or switch".into(),
                    })?
                    .break_label;
                self.emit_goto(label);
                Ok(())
            }
            CStmt::Continue => {
                // Search backwards for the first loop context
                let label = self
                    .breakable_stack
                    .iter()
                    .rev()
                    .find(|ctx| ctx.is_loop)
                    .and_then(|ctx| ctx.continue_label)
                    .ok_or_else(|| CompileError::CodegenError {
                        message: "continue outside loop".into(),
                    })?;
                self.emit_goto(label);
                Ok(())
            }
            CStmt::Switch {
                expr,
                cases,
                default_body,
            } => {
                self.gen_switch(expr, cases, default_body.as_deref())?;
                Ok(())
            }
            CStmt::TryCatch {
                try_body,
                catches,
                finally_body,
            } => {
                self.gen_try_catch(try_body, catches, finally_body.as_deref())?;
                Ok(())
            }
            CStmt::ForEach {
                element_type,
                var_name,
                iterable,
                body,
            } => {
                self.gen_foreach(element_type, var_name, iterable, body)?;
                Ok(())
            }
            CStmt::Synchronized { lock_expr, body } => {
                self.gen_synchronized(lock_expr, body)?;
                Ok(())
            }
        }
    }

    // --- Expression codegen ---

    fn gen_expr(&mut self, expr: &CExpr) -> Result<(), CompileError> {
        match expr {
            CExpr::IntLiteral(v) => {
                self.emit_int_const(*v);
                Ok(())
            }
            CExpr::LongLiteral(v) => {
                self.emit_long_const(*v);
                Ok(())
            }
            CExpr::FloatLiteral(v) => {
                self.emit_float_const(*v as f32);
                Ok(())
            }
            CExpr::DoubleLiteral(v) => {
                self.emit_double_const(*v);
                Ok(())
            }
            CExpr::BoolLiteral(b) => {
                self.emit(if *b {
                    Instruction::Iconst1
                } else {
                    Instruction::Iconst0
                });
                Ok(())
            }
            CExpr::StringLiteral(s) => {
                let cp_idx = self.class_file.get_or_add_string(s);
                self.emit_ldc(cp_idx);
                Ok(())
            }
            CExpr::CharLiteral(c) => {
                self.emit_int_const(*c as i64);
                Ok(())
            }
            CExpr::NullLiteral => {
                self.emit(Instruction::Aconstnull);
                Ok(())
            }
            CExpr::Ident(name) => {
                let (slot, ty) =
                    self.locals
                        .find(name)
                        .ok_or_else(|| CompileError::CodegenError {
                            message: format!("undefined variable: {}", name),
                        })?;
                let ty = ty.clone();
                self.emit_load(&ty, slot);
                Ok(())
            }
            CExpr::This => {
                if self.is_static {
                    return Err(CompileError::CodegenError {
                        message: "'this' not available in static method".into(),
                    });
                }
                self.emit(Instruction::Aload0);
                Ok(())
            }
            CExpr::BinaryOp { op, left, right } => {
                let left_ty = self.infer_expr_type(left);
                let right_ty = self.infer_expr_type(right);

                // String concatenation: either operand is a String
                if *op == BinOp::Add && (is_string_type(&left_ty) || is_string_type(&right_ty)) {
                    return self.gen_string_concat(expr);
                }

                let promoted = promote_numeric_type(&left_ty, &right_ty);
                self.gen_expr(left)?;
                self.emit_widen_if_needed(&left_ty, &promoted);
                self.gen_expr(right)?;
                // Shift ops: right operand is always int, no widening
                if !matches!(op, BinOp::Shl | BinOp::Shr | BinOp::Ushr) {
                    self.emit_widen_if_needed(&right_ty, &promoted);
                }
                self.emit_typed_binary_op(op, &promoted)?;
                Ok(())
            }
            CExpr::UnaryOp { op, operand } => {
                let ty = self.infer_expr_type(operand);
                self.gen_expr(operand)?;
                match op {
                    UnaryOp::Neg => {
                        if is_long_type(&ty) {
                            self.emit(Instruction::Lneg);
                        } else if is_float_type(&ty) {
                            self.emit(Instruction::Fneg);
                        } else if is_double_type(&ty) {
                            self.emit(Instruction::Dneg);
                        } else {
                            self.emit(Instruction::Ineg);
                        }
                    }
                    UnaryOp::BitNot => {
                        // ~x == x ^ -1
                        if is_long_type(&ty) {
                            let cp_idx = self.class_file.get_or_add_long(-1);
                            self.emit(Instruction::Ldc2W(cp_idx));
                            self.emit(Instruction::Lxor);
                        } else {
                            self.emit(Instruction::Iconstm1);
                            self.emit(Instruction::Ixor);
                        }
                    }
                }
                Ok(())
            }
            CExpr::Comparison { op, left, right } => {
                // Evaluate comparison to 0/1 using branches
                let left_ty = self.infer_expr_type(left);
                let right_ty = self.infer_expr_type(right);
                let promoted = promote_numeric_type(&left_ty, &right_ty);
                self.gen_expr(left)?;
                self.emit_widen_if_needed(&left_ty, &promoted);
                self.gen_expr(right)?;
                self.emit_widen_if_needed(&right_ty, &promoted);
                let true_label = self.new_label();
                let end_label = self.new_label();
                // end_label has Integer on stack (result of iconst_0 or iconst_1)
                self.label_stack_override
                    .push((end_label, vec![VType::Integer]));
                if is_reference_type(&promoted) {
                    // Reference equality: use if_acmpeq / if_acmpne
                    let branch = match op {
                        CompareOp::Eq => Instruction::IfAcmpeq as fn(i16) -> Instruction,
                        CompareOp::Ne => Instruction::IfAcmpne,
                        _ => {
                            return Err(CompileError::CodegenError {
                                message: "cannot use relational operator on reference types".into(),
                            });
                        }
                    };
                    self.emit_branch(branch, true_label);
                } else if is_int_type(&promoted) {
                    let branch = match op {
                        CompareOp::Eq => Instruction::IfIcmpeq as fn(i16) -> Instruction,
                        CompareOp::Ne => Instruction::IfIcmpne,
                        CompareOp::Lt => Instruction::IfIcmplt,
                        CompareOp::Le => Instruction::IfIcmple,
                        CompareOp::Gt => Instruction::IfIcmpgt,
                        CompareOp::Ge => Instruction::IfIcmpge,
                    };
                    self.emit_branch(branch, true_label);
                } else {
                    // long/float/double: emit compare instruction, then branch on int result
                    self.emit_typed_compare(&promoted, op);
                    let branch = match op {
                        CompareOp::Eq => Instruction::Ifeq as fn(i16) -> Instruction,
                        CompareOp::Ne => Instruction::Ifne,
                        CompareOp::Lt => Instruction::Iflt,
                        CompareOp::Le => Instruction::Ifle,
                        CompareOp::Gt => Instruction::Ifgt,
                        CompareOp::Ge => Instruction::Ifge,
                    };
                    self.emit_branch(branch, true_label);
                }
                self.emit(Instruction::Iconst0);
                self.emit_goto(end_label);
                self.bind_label(true_label);
                self.emit(Instruction::Iconst1);
                self.bind_label(end_label);
                Ok(())
            }
            CExpr::LogicalAnd(left, right) => {
                let false_label = self.new_label();
                let end_label = self.new_label();
                // end_label has Integer on stack (result of iconst_0 or iconst_1)
                self.label_stack_override
                    .push((end_label, vec![VType::Integer]));
                self.gen_condition(left, false_label, false)?;
                self.gen_condition(right, false_label, false)?;
                self.emit(Instruction::Iconst1);
                self.emit_goto(end_label);
                self.bind_label(false_label);
                self.emit(Instruction::Iconst0);
                self.bind_label(end_label);
                Ok(())
            }
            CExpr::LogicalOr(left, right) => {
                let true_label = self.new_label();
                let false_label = self.new_label();
                let end_label = self.new_label();
                // end_label has Integer on stack (result of iconst_0 or iconst_1)
                self.label_stack_override
                    .push((end_label, vec![VType::Integer]));
                self.gen_condition(left, true_label, true)?;
                self.gen_condition(right, false_label, false)?;
                self.bind_label(true_label);
                self.emit(Instruction::Iconst1);
                self.emit_goto(end_label);
                self.bind_label(false_label);
                self.emit(Instruction::Iconst0);
                self.bind_label(end_label);
                Ok(())
            }
            CExpr::LogicalNot(operand) => {
                let true_label = self.new_label();
                let end_label = self.new_label();
                // end_label has Integer on stack (result of iconst_0 or iconst_1)
                self.label_stack_override
                    .push((end_label, vec![VType::Integer]));
                self.gen_condition(operand, true_label, true)?;
                // Condition was false, so !cond is true
                self.emit(Instruction::Iconst1);
                self.emit_goto(end_label);
                self.bind_label(true_label);
                // Condition was true, so !cond is false
                self.emit(Instruction::Iconst0);
                self.bind_label(end_label);
                Ok(())
            }
            CExpr::Assign { target, value } => {
                // Special-case array stores: emit arrayref, index, value, then xastore
                if let CExpr::ArrayAccess { array, index } = target.as_ref() {
                    let array_ty = self.infer_expr_type(array);
                    let elem_ty = match &array_ty {
                        TypeName::Array(inner) => inner.as_ref().clone(),
                        _ => TypeName::Primitive(PrimitiveKind::Int),
                    };
                    self.gen_expr(array)?;
                    self.gen_expr(index)?;
                    self.gen_expr(value)?;
                    // Duplicate the value under [arrayref, index] so that the expression
                    // result remains on the stack after the array store. This matches Java
                    // semantics: `counter[0] = x` evaluates to the stored value.
                    // dup_x2: category-1 value under two category-1 values → value stays on top
                    // dup2_x2: category-2 value (long/double) under two category-1 values
                    if type_slot_width(&elem_ty) == 2 {
                        self.emit(Instruction::Dup2x2);
                    } else {
                        self.emit(Instruction::Dupx2);
                    }
                    self.emit_array_store(&array_ty);
                } else {
                    self.gen_expr(value)?;
                    if type_slot_width(&self.infer_expr_type(value)) == 2 {
                        self.emit(Instruction::Dup2);
                    } else {
                        self.emit(Instruction::Dup);
                    }
                    self.gen_store_target(target)?;
                }
                Ok(())
            }
            CExpr::CompoundAssign { op, target, value } => {
                // Load current, compute, dup, store
                let target_ty = self.infer_expr_type(target);
                let value_ty = self.infer_expr_type(value);
                let promoted = promote_numeric_type(&target_ty, &value_ty);
                self.gen_expr(target)?;
                self.emit_widen_if_needed(&target_ty, &promoted);
                self.gen_expr(value)?;
                if !matches!(op, BinOp::Shl | BinOp::Shr | BinOp::Ushr) {
                    self.emit_widen_if_needed(&value_ty, &promoted);
                }
                self.emit_typed_binary_op(op, &promoted)?;
                // Narrow back to target type if needed (e.g. int += double would need d2i)
                if numeric_rank(&promoted) > numeric_rank(&target_ty) {
                    self.emit_narrow(&promoted, &target_ty);
                }
                if type_slot_width(&target_ty) == 2 {
                    self.emit(Instruction::Dup2);
                } else {
                    self.emit(Instruction::Dup);
                }
                self.gen_store_target(target)?;
                Ok(())
            }
            CExpr::PreIncrement(operand) => {
                if let CExpr::Ident(name) = operand.as_ref() {
                    let (slot, ty) =
                        self.locals
                            .find(name)
                            .ok_or_else(|| CompileError::CodegenError {
                                message: format!("undefined variable: {}", name),
                            })?;
                    let ty = ty.clone();
                    let slot = slot;
                    if is_int_type(&ty) && slot <= 255 {
                        self.emit(Instruction::Iinc {
                            index: slot as u8,
                            value: 1,
                        });
                        self.emit_load(&ty, slot);
                    } else {
                        self.emit_load(&ty, slot);
                        self.emit_typed_const_one(&ty);
                        self.emit_typed_binary_op(&BinOp::Add, &ty)?;
                        if type_slot_width(&ty) == 2 {
                            self.emit(Instruction::Dup2);
                        } else {
                            self.emit(Instruction::Dup);
                        }
                        self.emit_store(&ty, slot);
                    }
                    Ok(())
                } else {
                    Err(CompileError::CodegenError {
                        message: "pre-increment requires simple variable".into(),
                    })
                }
            }
            CExpr::PreDecrement(operand) => {
                if let CExpr::Ident(name) = operand.as_ref() {
                    let (slot, ty) =
                        self.locals
                            .find(name)
                            .ok_or_else(|| CompileError::CodegenError {
                                message: format!("undefined variable: {}", name),
                            })?;
                    let ty = ty.clone();
                    let slot = slot;
                    if is_int_type(&ty) && slot <= 255 {
                        self.emit(Instruction::Iinc {
                            index: slot as u8,
                            value: -1,
                        });
                        self.emit_load(&ty, slot);
                    } else {
                        self.emit_load(&ty, slot);
                        self.emit_typed_const_one(&ty);
                        self.emit_typed_binary_op(&BinOp::Sub, &ty)?;
                        if type_slot_width(&ty) == 2 {
                            self.emit(Instruction::Dup2);
                        } else {
                            self.emit(Instruction::Dup);
                        }
                        self.emit_store(&ty, slot);
                    }
                    Ok(())
                } else {
                    Err(CompileError::CodegenError {
                        message: "pre-decrement requires simple variable".into(),
                    })
                }
            }
            CExpr::PostIncrement(operand) => {
                if let CExpr::Ident(name) = operand.as_ref() {
                    let (slot, ty) =
                        self.locals
                            .find(name)
                            .ok_or_else(|| CompileError::CodegenError {
                                message: format!("undefined variable: {}", name),
                            })?;
                    let ty = ty.clone();
                    let slot = slot;
                    self.emit_load(&ty, slot);
                    if is_int_type(&ty) && slot <= 255 {
                        self.emit(Instruction::Iinc {
                            index: slot as u8,
                            value: 1,
                        });
                    } else {
                        if type_slot_width(&ty) == 2 {
                            self.emit(Instruction::Dup2);
                        } else {
                            self.emit(Instruction::Dup);
                        }
                        self.emit_typed_const_one(&ty);
                        self.emit_typed_binary_op(&BinOp::Add, &ty)?;
                        self.emit_store(&ty, slot);
                    }
                    Ok(())
                } else {
                    Err(CompileError::CodegenError {
                        message: "post-increment requires simple variable".into(),
                    })
                }
            }
            CExpr::PostDecrement(operand) => {
                if let CExpr::Ident(name) = operand.as_ref() {
                    let (slot, ty) =
                        self.locals
                            .find(name)
                            .ok_or_else(|| CompileError::CodegenError {
                                message: format!("undefined variable: {}", name),
                            })?;
                    let ty = ty.clone();
                    let slot = slot;
                    self.emit_load(&ty, slot);
                    if is_int_type(&ty) && slot <= 255 {
                        self.emit(Instruction::Iinc {
                            index: slot as u8,
                            value: -1,
                        });
                    } else {
                        if type_slot_width(&ty) == 2 {
                            self.emit(Instruction::Dup2);
                        } else {
                            self.emit(Instruction::Dup);
                        }
                        self.emit_typed_const_one(&ty);
                        self.emit_typed_binary_op(&BinOp::Sub, &ty)?;
                        self.emit_store(&ty, slot);
                    }
                    Ok(())
                } else {
                    Err(CompileError::CodegenError {
                        message: "post-decrement requires simple variable".into(),
                    })
                }
            }
            CExpr::MethodCall { object, name, args } => {
                self.gen_method_call(object.as_deref(), name, args)
            }
            CExpr::StaticMethodCall {
                class_name,
                name,
                args,
            } => self.gen_static_method_call(class_name, name, args),
            CExpr::FieldAccess { object, name } => self.gen_field_access(object, name),
            CExpr::StaticFieldAccess { class_name, name } => {
                self.gen_static_field_access(class_name, name)
            }
            CExpr::NewObject { class_name, args } => {
                let internal = resolve_class_name(class_name);
                let class_idx = self.class_file.get_or_add_class(&internal);
                self.emit(Instruction::New(class_idx));
                self.emit(Instruction::Dup);
                for arg in args {
                    self.gen_expr(arg)?;
                }
                // Default constructor descriptor — try to infer from arg count
                let descriptor = self.infer_constructor_descriptor(args)?;
                let method_idx =
                    self.class_file
                        .get_or_add_method_ref(&internal, "<init>", &descriptor);
                self.emit(Instruction::Invokespecial(method_idx));
                Ok(())
            }
            CExpr::NewArray { element_type, size } => {
                self.gen_expr(size)?;
                match element_type {
                    TypeName::Primitive(kind) => {
                        let atype = match kind {
                            PrimitiveKind::Boolean => 4,
                            PrimitiveKind::Char => 5,
                            PrimitiveKind::Float => 6,
                            PrimitiveKind::Double => 7,
                            PrimitiveKind::Byte => 8,
                            PrimitiveKind::Short => 9,
                            PrimitiveKind::Int => 10,
                            PrimitiveKind::Long => 11,
                            PrimitiveKind::Void => {
                                return Err(CompileError::CodegenError {
                                    message: "cannot create array of void".into(),
                                });
                            }
                        };
                        self.emit(Instruction::Newarray(atype));
                    }
                    TypeName::Class(name) => {
                        let internal = resolve_class_name(name);
                        let class_idx = self.class_file.get_or_add_class(&internal);
                        self.emit(Instruction::Anewarray(class_idx));
                    }
                    TypeName::Array(_) => {
                        // Multi-dimensional: create array of arrays
                        let descriptor = type_name_to_descriptor(element_type);
                        let class_idx = self.class_file.get_or_add_class(&descriptor);
                        self.emit(Instruction::Anewarray(class_idx));
                    }
                }
                Ok(())
            }
            CExpr::NewMultiArray {
                element_type,
                dimensions,
            } => {
                for dim in dimensions {
                    self.gen_expr(dim)?;
                }
                let mut desc = String::new();
                for _ in 0..dimensions.len() {
                    desc.push('[');
                }
                desc.push_str(&type_name_to_descriptor(element_type));
                let class_idx = self.class_file.get_or_add_class(&desc);
                self.emit(Instruction::Multianewarray {
                    index: class_idx,
                    dimensions: dimensions.len() as u8,
                });
                Ok(())
            }
            CExpr::ArrayAccess { array, index } => {
                self.gen_expr(array)?;
                self.gen_expr(index)?;
                let array_ty = self.infer_expr_type(array);
                self.emit_array_load(&array_ty);
                Ok(())
            }
            CExpr::Cast { ty, operand } => {
                let src_ty = self.infer_expr_type(operand);
                self.gen_expr(operand)?;
                match ty {
                    TypeName::Primitive(kind) => {
                        let src_rank = numeric_rank(&src_ty);
                        let _dst_rank = numeric_rank(ty);
                        // Same rank or both int-like: may still need narrowing
                        match (src_rank, kind) {
                            // Source is int-like
                            (0, PrimitiveKind::Long) => {
                                self.emit(Instruction::I2l);
                            }
                            (0, PrimitiveKind::Float) => {
                                self.emit(Instruction::I2f);
                            }
                            (0, PrimitiveKind::Double) => {
                                self.emit(Instruction::I2d);
                            }
                            (0, PrimitiveKind::Byte) => {
                                self.emit(Instruction::I2b);
                            }
                            (0, PrimitiveKind::Char) => {
                                self.emit(Instruction::I2c);
                            }
                            (0, PrimitiveKind::Short) => {
                                self.emit(Instruction::I2s);
                            }
                            (0, PrimitiveKind::Int) | (0, PrimitiveKind::Boolean) => {}
                            // Source is long
                            (1, PrimitiveKind::Int) => {
                                self.emit(Instruction::L2i);
                            }
                            (1, PrimitiveKind::Float) => {
                                self.emit(Instruction::L2f);
                            }
                            (1, PrimitiveKind::Double) => {
                                self.emit(Instruction::L2d);
                            }
                            (1, PrimitiveKind::Byte) => {
                                self.emit(Instruction::L2i);
                                self.emit(Instruction::I2b);
                            }
                            (1, PrimitiveKind::Char) => {
                                self.emit(Instruction::L2i);
                                self.emit(Instruction::I2c);
                            }
                            (1, PrimitiveKind::Short) => {
                                self.emit(Instruction::L2i);
                                self.emit(Instruction::I2s);
                            }
                            (1, PrimitiveKind::Long) => {}
                            // Source is float
                            (2, PrimitiveKind::Int) => {
                                self.emit(Instruction::F2i);
                            }
                            (2, PrimitiveKind::Long) => {
                                self.emit(Instruction::F2l);
                            }
                            (2, PrimitiveKind::Double) => {
                                self.emit(Instruction::F2d);
                            }
                            (2, PrimitiveKind::Byte) => {
                                self.emit(Instruction::F2i);
                                self.emit(Instruction::I2b);
                            }
                            (2, PrimitiveKind::Char) => {
                                self.emit(Instruction::F2i);
                                self.emit(Instruction::I2c);
                            }
                            (2, PrimitiveKind::Short) => {
                                self.emit(Instruction::F2i);
                                self.emit(Instruction::I2s);
                            }
                            (2, PrimitiveKind::Float) => {}
                            // Source is double
                            (3, PrimitiveKind::Int) => {
                                self.emit(Instruction::D2i);
                            }
                            (3, PrimitiveKind::Long) => {
                                self.emit(Instruction::D2l);
                            }
                            (3, PrimitiveKind::Float) => {
                                self.emit(Instruction::D2f);
                            }
                            (3, PrimitiveKind::Byte) => {
                                self.emit(Instruction::D2i);
                                self.emit(Instruction::I2b);
                            }
                            (3, PrimitiveKind::Char) => {
                                self.emit(Instruction::D2i);
                                self.emit(Instruction::I2c);
                            }
                            (3, PrimitiveKind::Short) => {
                                self.emit(Instruction::D2i);
                                self.emit(Instruction::I2s);
                            }
                            (3, PrimitiveKind::Double) => {}
                            (_, PrimitiveKind::Void) => {
                                return Err(CompileError::CodegenError {
                                    message: "cannot cast to void".into(),
                                });
                            }
                            _ => {} // same type, no-op
                        }
                        Ok(())
                    }
                    TypeName::Class(name) => {
                        let internal = resolve_class_name(name);
                        let class_idx = self.class_file.get_or_add_class(&internal);
                        self.emit(Instruction::Checkcast(class_idx));
                        Ok(())
                    }
                    TypeName::Array(_) => {
                        let descriptor = type_name_to_descriptor(ty);
                        let class_idx = self.class_file.get_or_add_class(&descriptor);
                        self.emit(Instruction::Checkcast(class_idx));
                        Ok(())
                    }
                }
            }
            CExpr::Instanceof { operand, ty } => {
                self.gen_expr(operand)?;
                match ty {
                    TypeName::Class(name) => {
                        let internal = resolve_class_name(name);
                        let class_idx = self.class_file.get_or_add_class(&internal);
                        self.emit(Instruction::Instanceof(class_idx));
                    }
                    TypeName::Array(_) => {
                        let descriptor = type_name_to_descriptor(ty);
                        let class_idx = self.class_file.get_or_add_class(&descriptor);
                        self.emit(Instruction::Instanceof(class_idx));
                    }
                    _ => {
                        return Err(CompileError::CodegenError {
                            message: "instanceof requires class or array type".into(),
                        });
                    }
                }
                Ok(())
            }
            CExpr::Ternary {
                condition,
                then_expr,
                else_expr,
            } => {
                let false_label = self.new_label();
                let end_label = self.new_label();
                // Both branches push a result value before reaching end_label
                let result_vtype =
                    type_name_to_vtype_resolved(&self.infer_expr_type(then_expr), self.class_file);
                let result_stack = vec![result_vtype];
                self.label_stack_override.push((false_label, Vec::new()));
                self.label_stack_override.push((end_label, result_stack));
                self.gen_condition(condition, false_label, false)?;
                self.gen_expr(then_expr)?;
                self.emit_goto(end_label);
                self.bind_label(false_label);
                self.gen_expr(else_expr)?;
                self.bind_label(end_label);
                Ok(())
            }
            CExpr::SwitchExpr {
                expr,
                cases,
                default_expr,
            } => self.gen_switch_expr(expr, cases, default_expr),
            CExpr::Lambda { params, body } => self.gen_lambda(params, body),
            CExpr::MethodRef {
                class_name,
                method_name,
            } => self.gen_method_ref(class_name, method_name),
        }
    }

    // --- Condition codegen (emit direct branch instructions) ---

    /// Generate condition code. If `jump_on_true`, jumps to `target_label` when condition is true.
    /// Otherwise, jumps to `target_label` when condition is false.
    fn gen_condition(
        &mut self,
        expr: &CExpr,
        target_label: usize,
        jump_on_true: bool,
    ) -> Result<(), CompileError> {
        match expr {
            CExpr::Comparison { op, left, right } => {
                // Check for null comparisons
                if matches!(right.as_ref(), CExpr::NullLiteral) {
                    self.gen_expr(left)?;
                    let branch = if jump_on_true {
                        match op {
                            CompareOp::Eq => Instruction::Ifnull as fn(i16) -> Instruction,
                            CompareOp::Ne => Instruction::Ifnonnull,
                            _ => {
                                return Err(CompileError::CodegenError {
                                    message: "cannot compare null with relational operator".into(),
                                });
                            }
                        }
                    } else {
                        match op {
                            CompareOp::Eq => Instruction::Ifnonnull as fn(i16) -> Instruction,
                            CompareOp::Ne => Instruction::Ifnull,
                            _ => {
                                return Err(CompileError::CodegenError {
                                    message: "cannot compare null with relational operator".into(),
                                });
                            }
                        }
                    };
                    self.emit_branch(branch, target_label);
                    return Ok(());
                }
                if matches!(left.as_ref(), CExpr::NullLiteral) {
                    self.gen_expr(right)?;
                    let branch = if jump_on_true {
                        match op {
                            CompareOp::Eq => Instruction::Ifnull as fn(i16) -> Instruction,
                            CompareOp::Ne => Instruction::Ifnonnull,
                            _ => {
                                return Err(CompileError::CodegenError {
                                    message: "cannot compare null with relational operator".into(),
                                });
                            }
                        }
                    } else {
                        match op {
                            CompareOp::Eq => Instruction::Ifnonnull as fn(i16) -> Instruction,
                            CompareOp::Ne => Instruction::Ifnull,
                            _ => {
                                return Err(CompileError::CodegenError {
                                    message: "cannot compare null with relational operator".into(),
                                });
                            }
                        }
                    };
                    self.emit_branch(branch, target_label);
                    return Ok(());
                }

                let left_ty = self.infer_expr_type(left);
                let right_ty = self.infer_expr_type(right);
                let promoted = promote_numeric_type(&left_ty, &right_ty);
                self.gen_expr(left)?;
                self.emit_widen_if_needed(&left_ty, &promoted);
                self.gen_expr(right)?;
                self.emit_widen_if_needed(&right_ty, &promoted);

                if is_reference_type(&promoted) {
                    // Reference equality: use if_acmpeq / if_acmpne
                    let branch = match (op, jump_on_true) {
                        (CompareOp::Eq, true) | (CompareOp::Ne, false) => {
                            Instruction::IfAcmpeq as fn(i16) -> Instruction
                        }
                        (CompareOp::Ne, true) | (CompareOp::Eq, false) => {
                            Instruction::IfAcmpne as fn(i16) -> Instruction
                        }
                        _ => {
                            return Err(CompileError::CodegenError {
                                message: "cannot use relational operator on reference types".into(),
                            });
                        }
                    };
                    self.emit_branch(branch, target_label);
                } else if is_int_type(&promoted) {
                    let branch = if jump_on_true {
                        match op {
                            CompareOp::Eq => Instruction::IfIcmpeq as fn(i16) -> Instruction,
                            CompareOp::Ne => Instruction::IfIcmpne,
                            CompareOp::Lt => Instruction::IfIcmplt,
                            CompareOp::Le => Instruction::IfIcmple,
                            CompareOp::Gt => Instruction::IfIcmpgt,
                            CompareOp::Ge => Instruction::IfIcmpge,
                        }
                    } else {
                        match op {
                            CompareOp::Eq => Instruction::IfIcmpne as fn(i16) -> Instruction,
                            CompareOp::Ne => Instruction::IfIcmpeq,
                            CompareOp::Lt => Instruction::IfIcmpge,
                            CompareOp::Le => Instruction::IfIcmpgt,
                            CompareOp::Gt => Instruction::IfIcmple,
                            CompareOp::Ge => Instruction::IfIcmplt,
                        }
                    };
                    self.emit_branch(branch, target_label);
                } else {
                    // long/float/double: compare instruction reduces to int, then branch
                    self.emit_typed_compare(&promoted, op);
                    let branch = if jump_on_true {
                        match op {
                            CompareOp::Eq => Instruction::Ifeq as fn(i16) -> Instruction,
                            CompareOp::Ne => Instruction::Ifne,
                            CompareOp::Lt => Instruction::Iflt,
                            CompareOp::Le => Instruction::Ifle,
                            CompareOp::Gt => Instruction::Ifgt,
                            CompareOp::Ge => Instruction::Ifge,
                        }
                    } else {
                        match op {
                            CompareOp::Eq => Instruction::Ifne as fn(i16) -> Instruction,
                            CompareOp::Ne => Instruction::Ifeq,
                            CompareOp::Lt => Instruction::Ifge,
                            CompareOp::Le => Instruction::Ifgt,
                            CompareOp::Gt => Instruction::Ifle,
                            CompareOp::Ge => Instruction::Iflt,
                        }
                    };
                    self.emit_branch(branch, target_label);
                }
                Ok(())
            }
            CExpr::LogicalAnd(left, right) => {
                if jump_on_true {
                    // a && b is true: both must be true
                    let skip = self.new_label();
                    self.gen_condition(left, skip, false)?;
                    self.gen_condition(right, target_label, true)?;
                    self.bind_label(skip);
                } else {
                    // a && b is false: either is false
                    self.gen_condition(left, target_label, false)?;
                    self.gen_condition(right, target_label, false)?;
                }
                Ok(())
            }
            CExpr::LogicalOr(left, right) => {
                if jump_on_true {
                    // a || b is true: either is true
                    self.gen_condition(left, target_label, true)?;
                    self.gen_condition(right, target_label, true)?;
                } else {
                    // a || b is false: both must be false
                    let skip = self.new_label();
                    self.gen_condition(left, skip, true)?;
                    self.gen_condition(right, target_label, false)?;
                    self.bind_label(skip);
                }
                Ok(())
            }
            CExpr::LogicalNot(operand) => self.gen_condition(operand, target_label, !jump_on_true),
            CExpr::BoolLiteral(true) => {
                if jump_on_true {
                    self.emit_goto(target_label);
                }
                Ok(())
            }
            CExpr::BoolLiteral(false) => {
                if !jump_on_true {
                    self.emit_goto(target_label);
                }
                Ok(())
            }
            _ => {
                // Generic: evaluate to int, branch on 0/non-0
                self.gen_expr(expr)?;
                let branch = if jump_on_true {
                    Instruction::Ifne as fn(i16) -> Instruction
                } else {
                    Instruction::Ifeq as fn(i16) -> Instruction
                };
                self.emit_branch(branch, target_label);
                Ok(())
            }
        }
    }

    // --- Switch codegen ---

    fn gen_switch(
        &mut self,
        expr: &CExpr,
        cases: &[SwitchCase],
        default_body: Option<&[CStmt]>,
    ) -> Result<(), CompileError> {
        self.gen_expr(expr)?;

        let end_label = self.new_label();
        let default_label = self.new_label();

        // Collect all (value, case_index) pairs
        let mut value_to_case: Vec<(i32, usize)> = Vec::new();
        for (case_idx, case) in cases.iter().enumerate() {
            for &v in &case.values {
                value_to_case.push((v as i32, case_idx));
            }
        }
        value_to_case.sort_by_key(|&(v, _)| v);

        // Create labels for each case body
        let case_labels: Vec<usize> = cases.iter().map(|_| self.new_label()).collect();

        // Override all branch-target labels with pre-switch locals
        let pre_case_locals = self.locals.current_locals_vtypes();
        for &label in &case_labels {
            self.label_locals_override
                .push((label, pre_case_locals.clone()));
        }
        self.label_locals_override
            .push((default_label, pre_case_locals.clone()));
        self.label_locals_override
            .push((end_label, pre_case_locals));

        // Decide tableswitch vs lookupswitch
        let use_table = if value_to_case.is_empty() {
            false
        } else {
            let low = value_to_case.first().unwrap().0;
            let high = value_to_case.last().unwrap().0;
            let range = (high as i64 - low as i64 + 1) as usize;
            range <= 2 * value_to_case.len()
        };

        if use_table && !value_to_case.is_empty() {
            let low = value_to_case.first().unwrap().0;
            let high = value_to_case.last().unwrap().0;

            // Build offset labels array: for each index in [low..=high], map to case label or default
            let mut offset_labels: Vec<usize> = Vec::new();
            let mut val_idx = 0;
            for v in low..=high {
                if val_idx < value_to_case.len() && value_to_case[val_idx].0 == v {
                    offset_labels.push(case_labels[value_to_case[val_idx].1]);
                    val_idx += 1;
                } else {
                    offset_labels.push(default_label);
                }
            }

            // Emit placeholder tableswitch
            let placeholder = Instruction::Tableswitch {
                default: 0,
                low,
                high,
                offsets: vec![0i32; offset_labels.len()],
            };
            let instr_idx = self.emit(placeholder);
            self.switch_patches.push(SwitchPatch {
                instr_idx,
                kind: SwitchPatchKind::Table {
                    low,
                    high,
                    case_labels: offset_labels,
                    default_label,
                },
            });
        } else {
            // Lookupswitch
            let pair_labels: Vec<(i32, usize)> = value_to_case
                .iter()
                .map(|&(v, case_idx)| (v, case_labels[case_idx]))
                .collect();

            let placeholder = Instruction::Lookupswitch {
                default: 0,
                npairs: pair_labels.len() as u32,
                pairs: pair_labels.iter().map(|&(v, _)| (v, 0i32)).collect(),
            };
            let instr_idx = self.emit(placeholder);
            self.switch_patches.push(SwitchPatch {
                instr_idx,
                kind: SwitchPatchKind::Lookup {
                    pairs: pair_labels,
                    default_label,
                },
            });
        }

        // Push breakable context
        self.breakable_stack.push(BreakableContext {
            break_label: end_label,
            is_loop: false,
            continue_label: None,
        });

        // Emit case bodies
        for (i, case) in cases.iter().enumerate() {
            self.bind_label(case_labels[i]);
            for s in &case.body {
                self.gen_stmt(s)?;
            }
        }

        // Emit default body
        self.bind_label(default_label);
        if let Some(body) = default_body {
            for s in body {
                self.gen_stmt(s)?;
            }
        }

        self.bind_label(end_label);
        self.breakable_stack.pop();
        Ok(())
    }

    fn gen_switch_expr(
        &mut self,
        expr: &CExpr,
        cases: &[SwitchExprCase],
        default_expr: &CExpr,
    ) -> Result<(), CompileError> {
        self.gen_expr(expr)?;

        let end_label = self.new_label();
        let default_label = self.new_label();

        // Collect all (value, case_index) pairs
        let mut value_to_case: Vec<(i32, usize)> = Vec::new();
        for (case_idx, case) in cases.iter().enumerate() {
            for &v in &case.values {
                value_to_case.push((v as i32, case_idx));
            }
        }
        value_to_case.sort_by_key(|&(v, _)| v);

        let case_labels: Vec<usize> = cases.iter().map(|_| self.new_label()).collect();

        // Override all branch-target labels with pre-switch locals
        let pre_case_locals = self.locals.current_locals_vtypes();
        for &label in &case_labels {
            self.label_locals_override
                .push((label, pre_case_locals.clone()));
        }
        self.label_locals_override
            .push((default_label, pre_case_locals.clone()));
        self.label_locals_override
            .push((end_label, pre_case_locals));

        // end_label has the switch result on the stack
        let result_vtype =
            type_name_to_vtype_resolved(&self.infer_expr_type(&cases[0].expr), self.class_file);
        self.label_stack_override
            .push((end_label, vec![result_vtype]));

        // Decide tableswitch vs lookupswitch
        let use_table = if value_to_case.is_empty() {
            false
        } else {
            let low = value_to_case.first().unwrap().0;
            let high = value_to_case.last().unwrap().0;
            let range = (high as i64 - low as i64 + 1) as usize;
            range <= 2 * value_to_case.len()
        };

        if use_table && !value_to_case.is_empty() {
            let low = value_to_case.first().unwrap().0;
            let high = value_to_case.last().unwrap().0;

            let mut offset_labels: Vec<usize> = Vec::new();
            let mut val_idx = 0;
            for v in low..=high {
                if val_idx < value_to_case.len() && value_to_case[val_idx].0 == v {
                    offset_labels.push(case_labels[value_to_case[val_idx].1]);
                    val_idx += 1;
                } else {
                    offset_labels.push(default_label);
                }
            }

            let placeholder = Instruction::Tableswitch {
                default: 0,
                low,
                high,
                offsets: vec![0i32; offset_labels.len()],
            };
            let instr_idx = self.emit(placeholder);
            self.switch_patches.push(SwitchPatch {
                instr_idx,
                kind: SwitchPatchKind::Table {
                    low,
                    high,
                    case_labels: offset_labels,
                    default_label,
                },
            });
        } else {
            let pair_labels: Vec<(i32, usize)> = value_to_case
                .iter()
                .map(|&(v, case_idx)| (v, case_labels[case_idx]))
                .collect();

            let placeholder = Instruction::Lookupswitch {
                default: 0,
                npairs: pair_labels.len() as u32,
                pairs: pair_labels.iter().map(|&(v, _)| (v, 0i32)).collect(),
            };
            let instr_idx = self.emit(placeholder);
            self.switch_patches.push(SwitchPatch {
                instr_idx,
                kind: SwitchPatchKind::Lookup {
                    pairs: pair_labels,
                    default_label,
                },
            });
        }

        // Emit case expression bodies — each pushes a value then jumps to end
        for (i, case) in cases.iter().enumerate() {
            self.bind_label(case_labels[i]);
            self.gen_expr(&case.expr)?;
            self.emit_goto(end_label);
        }

        // Default
        self.bind_label(default_label);
        self.gen_expr(default_expr)?;

        self.bind_label(end_label);
        Ok(())
    }

    // --- Lambda codegen ---

    fn gen_lambda(
        &mut self,
        params: &[LambdaParam],
        body: &LambdaBody,
    ) -> Result<(), CompileError> {
        // Generate a synthetic static method name
        let lambda_idx = self.class_file.methods.len();
        let lambda_name = format!("lambda${}", lambda_idx);

        // Build lambda method descriptor from params
        // All typed params contribute to descriptor; untyped params default to Object
        let mut param_descs = Vec::new();
        for p in params {
            let desc = match &p.ty {
                Some(ty) => type_name_to_descriptor(ty),
                None => "Ljava/lang/Object;".to_string(),
            };
            param_descs.push(desc);
        }

        // Infer return type from body
        let return_desc = match body {
            LambdaBody::Expr(expr) => {
                let ty = self.infer_expr_type(expr);
                type_name_to_descriptor(&ty)
            }
            LambdaBody::Block(_) => "V".to_string(), // Block lambdas default to void
        };

        let lambda_descriptor = format!("({}){}", param_descs.join(""), return_desc);

        // Build the SAM method descriptor (functional interface method type)
        // This is the same as the lambda descriptor for simple cases
        let sam_descriptor = lambda_descriptor.clone();

        // Create the synthetic method body
        let stmts: Vec<CStmt> = match body {
            LambdaBody::Expr(expr) => {
                if return_desc == "V" {
                    vec![CStmt::ExprStmt(expr.as_ref().clone())]
                } else {
                    vec![CStmt::Return(Some(expr.as_ref().clone()))]
                }
            }
            LambdaBody::Block(stmts) => stmts.clone(),
        };

        // Generate bytecode for the synthetic method
        let mut lambda_codegen = CodeGenerator::new(
            self.class_file,
            true, // lambda methods are static
            &lambda_descriptor,
            &[], // lambda params don't have debug names
        )?;
        lambda_codegen.generate_body(&stmts)?;
        let generated = lambda_codegen.finish()?;

        // Create Code attribute
        let code_name_idx = self.class_file.get_or_add_utf8("Code");
        let exception_table_length = generated.exception_table.len() as u16;
        let code_attr = CodeAttribute {
            max_stack: generated.max_stack,
            max_locals: generated.max_locals,
            code_length: 0,
            code: generated.instructions,
            exception_table_length,
            exception_table: generated.exception_table,
            attributes_count: 0,
            attributes: Vec::new(),
        };

        let mut attr_info = AttributeInfo {
            attribute_name_index: code_name_idx,
            attribute_length: 0,
            info: vec![],
            info_parsed: Some(AttributeInfoVariant::Code(code_attr)),
        };
        attr_info
            .sync_from_parsed()
            .map_err(|e| CompileError::CodegenError {
                message: format!("sync_from_parsed for lambda Code failed: {}", e),
            })?;

        // Create the MethodInfo for the synthetic method
        let name_idx = self.class_file.get_or_add_utf8(&lambda_name);
        let desc_idx = self.class_file.get_or_add_utf8(&lambda_descriptor);

        let method_info = MethodInfo {
            access_flags: MethodAccessFlags::PRIVATE
                | MethodAccessFlags::STATIC
                | MethodAccessFlags::SYNTHETIC,
            name_index: name_idx,
            descriptor_index: desc_idx,
            attributes_count: 1,
            attributes: vec![attr_info],
        };
        self.class_file.methods.push(method_info);
        self.class_file.sync_counts();

        // Set up bootstrap method for LambdaMetafactory.metafactory
        let bootstrap_idx = self.get_or_add_lambda_bootstrap()?;

        // Build the invokedynamic constant
        // The invokedynamic call site produces the functional interface instance.
        // Use the correct SAM name for the guessed functional interface.
        let fi_class = self.guess_functional_interface(&param_descs, &return_desc);
        let invoke_name = match fi_class.as_str() {
            "java/lang/Runnable" => "run",
            "java/util/function/Supplier" => "get",
            "java/util/function/Consumer" => "accept",
            "java/util/function/Predicate" => "test",
            _ => "apply",
        };
        let invoke_desc = format!("()L{};", fi_class);
        let indy_idx =
            self.class_file
                .get_or_add_invoke_dynamic(bootstrap_idx, invoke_name, &invoke_desc);

        // Add bootstrap method arguments:
        // 1. MethodType: SAM method type (e.g., "()V" for Runnable.run)
        // 2. MethodHandle: impl method (our synthetic lambda method)
        // 3. MethodType: instantiated method type (same as SAM type for non-generic)
        let this_class_name = self.get_this_class_name()?;
        let impl_method_ref = self.class_file.get_or_add_method_ref(
            &this_class_name,
            &lambda_name,
            &lambda_descriptor,
        );
        let impl_handle = self.class_file.get_or_add_method_handle(
            6, // REF_invokeStatic
            impl_method_ref,
        );
        let sam_method_type = self.class_file.get_or_add_method_type(&sam_descriptor);
        let instantiated_method_type = self.class_file.get_or_add_method_type(&sam_descriptor);

        // Update the bootstrap method entry with the arguments
        self.update_bootstrap_args(
            bootstrap_idx,
            &[sam_method_type, impl_handle, instantiated_method_type],
        )?;

        // Emit the invokedynamic instruction
        self.emit(Instruction::Invokedynamic {
            index: indy_idx,
            filler: 0,
        });

        Ok(())
    }

    fn gen_method_ref(&mut self, class_name: &str, method_name: &str) -> Result<(), CompileError> {
        let internal_class = resolve_class_name(class_name);

        // Resolve the method descriptor from the constant pool or well-known methods
        let impl_descriptor = self.find_method_ref_descriptor(&internal_class, method_name)?;
        let (params, ret) = parse_method_descriptor(&impl_descriptor).ok_or_else(|| {
            CompileError::CodegenError {
                message: format!(
                    "invalid method descriptor for {}::{}: {}",
                    class_name, method_name, impl_descriptor
                ),
            }
        })?;

        // Build erased SAM descriptor: all reference params → Object, primitives stay
        let erased_params: Vec<String> = params.iter().map(erase_to_object).collect();
        let erased_ret = erase_to_object(&ret);
        let sam_desc = format!("({}){}", erased_params.join(""), erased_ret);

        // Determine functional interface from param/return types
        let param_descs: Vec<String> = params.iter().map(|p| p.to_descriptor()).collect();
        let ret_desc = ret.to_descriptor();
        let fi_class = self.guess_functional_interface(&param_descs, &ret_desc);
        let sam_name = match fi_class.as_str() {
            "java/lang/Runnable" => "run",
            "java/util/function/Consumer" => "accept",
            "java/util/function/Predicate" => "test",
            "java/util/function/Supplier" => "get",
            _ => "apply",
        };

        // invokedynamic descriptor: () -> FunctionalInterface
        let indy_desc = format!("()L{};", fi_class);

        let bootstrap_idx = self.get_or_add_lambda_bootstrap()?;

        let indy_idx =
            self.class_file
                .get_or_add_invoke_dynamic(bootstrap_idx, sam_name, &indy_desc);

        let method_ref =
            self.class_file
                .get_or_add_method_ref(&internal_class, method_name, &impl_descriptor);
        let impl_handle = self.class_file.get_or_add_method_handle(6, method_ref);
        let sam_type = self.class_file.get_or_add_method_type(&sam_desc);
        let inst_type = self.class_file.get_or_add_method_type(&impl_descriptor);

        self.update_bootstrap_args(bootstrap_idx, &[sam_type, impl_handle, inst_type])?;

        self.emit(Instruction::Invokedynamic {
            index: indy_idx,
            filler: 0,
        });

        Ok(())
    }

    /// Resolve a method descriptor for a method reference (Class::method).
    /// Searches the constant pool first, then falls back to well-known methods.
    fn find_method_ref_descriptor(
        &self,
        internal_class: &str,
        method_name: &str,
    ) -> Result<String, CompileError> {
        use crate::constant_info::ConstantInfo;
        let pool = &self.class_file.const_pool;

        // Search constant pool for MethodRef or InterfaceMethodRef matching class + name
        for entry in pool.iter() {
            let (class_idx, nat_idx) = match entry {
                ConstantInfo::MethodRef(r) => (r.class_index, r.name_and_type_index),
                ConstantInfo::InterfaceMethodRef(r) => (r.class_index, r.name_and_type_index),
                _ => continue,
            };
            // Check class name matches
            if let Some(ConstantInfo::Class(cls)) = pool.get((class_idx - 1) as usize)
                && let Some(cls_name) = self.class_file.get_utf8(cls.name_index)
                && cls_name != internal_class
            {
                continue;
            }
            // Check method name and get descriptor
            if let Some(ConstantInfo::NameAndType(nat)) = pool.get((nat_idx - 1) as usize)
                && let Some(name) = self.class_file.get_utf8(nat.name_index)
                && name == method_name
                && let Some(desc) = self.class_file.get_utf8(nat.descriptor_index)
            {
                return Ok(desc.to_string());
            }
        }

        // Well-known method descriptors
        match (internal_class, method_name) {
            ("java/lang/String", "valueOf") => Ok("(Ljava/lang/Object;)Ljava/lang/String;".into()),
            ("java/lang/Integer", "parseInt") => Ok("(Ljava/lang/String;)I".into()),
            ("java/lang/Integer", "valueOf") => Ok("(I)Ljava/lang/Integer;".into()),
            ("java/lang/Long", "parseLong") => Ok("(Ljava/lang/String;)J".into()),
            ("java/lang/Long", "valueOf") => Ok("(J)Ljava/lang/Long;".into()),
            ("java/lang/Double", "parseDouble") => Ok("(Ljava/lang/String;)D".into()),
            ("java/lang/Double", "valueOf") => Ok("(D)Ljava/lang/Double;".into()),
            ("java/lang/Boolean", "parseBoolean") => Ok("(Ljava/lang/String;)Z".into()),
            ("java/lang/System", "exit") => Ok("(I)V".into()),
            _ => Err(CompileError::CodegenError {
                message: format!(
                    "cannot resolve method descriptor for {}::{}; \
                     ensure the method is called elsewhere so its descriptor is in the constant pool",
                    internal_class, method_name
                ),
            }),
        }
    }

    /// Guess the functional interface class based on parameter/return types.
    fn guess_functional_interface(&self, param_descs: &[String], return_desc: &str) -> String {
        match (param_descs.len(), return_desc) {
            (0, "V") => "java/lang/Runnable".into(),
            (0, _) => "java/util/function/Supplier".into(),
            (1, "V") => "java/util/function/Consumer".into(),
            (1, "Z") => "java/util/function/Predicate".into(),
            (1, _) => "java/util/function/Function".into(),
            (2, _) => "java/util/function/BiFunction".into(),
            _ => "java/lang/Runnable".into(),
        }
    }

    /// Find or create the bootstrap method entry for LambdaMetafactory.metafactory.
    fn get_or_add_lambda_bootstrap(&mut self) -> Result<u16, CompileError> {
        // Build the method handle for LambdaMetafactory.metafactory
        let metafactory_ref = self.class_file.get_or_add_method_ref(
            "java/lang/invoke/LambdaMetafactory",
            "metafactory",
            "(Ljava/lang/invoke/MethodHandles$Lookup;Ljava/lang/String;Ljava/lang/invoke/MethodType;Ljava/lang/invoke/MethodType;Ljava/lang/invoke/MethodHandle;Ljava/lang/invoke/MethodType;)Ljava/lang/invoke/CallSite;",
        );
        let metafactory_handle = self.class_file.get_or_add_method_handle(
            6, // REF_invokeStatic
            metafactory_ref,
        );

        // Find existing BootstrapMethods attribute on the class, or create one
        let bsm_attr_idx = self.class_file.attributes.iter().position(|a| {
            matches!(
                &a.info_parsed,
                Some(AttributeInfoVariant::BootstrapMethods(_))
            )
        });

        if let Some(idx) = bsm_attr_idx {
            // Each lambda/method-reference needs its own bootstrap method entry.
            // Do NOT reuse entries — different lambdas use different impl method handles.

            // Add new bootstrap method entry
            let new_bm = BootstrapMethod {
                bootstrap_method_ref: metafactory_handle,
                num_bootstrap_arguments: 0,
                bootstrap_arguments: Vec::new(),
            };

            if let Some(AttributeInfoVariant::BootstrapMethods(bsm)) =
                &mut self.class_file.attributes[idx].info_parsed
            {
                let new_idx = bsm.bootstrap_methods.len() as u16;
                bsm.bootstrap_methods.push(new_bm);
                bsm.num_bootstrap_methods = bsm.bootstrap_methods.len() as u16;
                return Ok(new_idx);
            }
            unreachable!()
        } else {
            // Create new BootstrapMethods attribute
            let name_idx = self.class_file.get_or_add_utf8("BootstrapMethods");
            let bsm_attr = BootstrapMethodsAttribute {
                num_bootstrap_methods: 1,
                bootstrap_methods: vec![BootstrapMethod {
                    bootstrap_method_ref: metafactory_handle,
                    num_bootstrap_arguments: 0,
                    bootstrap_arguments: Vec::new(),
                }],
            };
            let mut attr_info = AttributeInfo {
                attribute_name_index: name_idx,
                attribute_length: 0,
                info: vec![],
                info_parsed: Some(AttributeInfoVariant::BootstrapMethods(bsm_attr)),
            };
            attr_info
                .sync_from_parsed()
                .map_err(|e| CompileError::CodegenError {
                    message: format!("sync_from_parsed for BootstrapMethods failed: {}", e),
                })?;
            self.class_file.attributes.push(attr_info);
            self.class_file.sync_counts();
            Ok(0)
        }
    }

    /// Update bootstrap method arguments for a specific bootstrap method index.
    fn update_bootstrap_args(
        &mut self,
        bootstrap_idx: u16,
        args: &[u16],
    ) -> Result<(), CompileError> {
        let bsm_attr_idx = self
            .class_file
            .attributes
            .iter()
            .position(|a| {
                matches!(
                    &a.info_parsed,
                    Some(AttributeInfoVariant::BootstrapMethods(_))
                )
            })
            .ok_or_else(|| CompileError::CodegenError {
                message: "BootstrapMethods attribute not found".into(),
            })?;

        if let Some(AttributeInfoVariant::BootstrapMethods(bsm)) =
            &mut self.class_file.attributes[bsm_attr_idx].info_parsed
        {
            let bm = &mut bsm.bootstrap_methods[bootstrap_idx as usize];
            bm.bootstrap_arguments = args.to_vec();
            bm.num_bootstrap_arguments = args.len() as u16;
        }

        // Re-sync the attribute
        self.class_file.attributes[bsm_attr_idx]
            .sync_from_parsed()
            .map_err(|e| CompileError::CodegenError {
                message: format!("sync_from_parsed for BootstrapMethods failed: {}", e),
            })?;
        self.class_file.sync_counts();

        Ok(())
    }

    // --- For-each codegen ---

    fn gen_foreach(
        &mut self,
        element_type: &TypeName,
        var_name: &str,
        iterable: &CExpr,
        body: &[CStmt],
    ) -> Result<(), CompileError> {
        let iterable_ty = self.infer_expr_type(iterable);

        if matches!(iterable_ty, TypeName::Array(_)) {
            self.gen_foreach_array(element_type, var_name, iterable, body)
        } else {
            self.gen_foreach_iterable(element_type, var_name, iterable, body)
        }
    }

    fn gen_foreach_array(
        &mut self,
        element_type: &TypeName,
        var_name: &str,
        iterable: &CExpr,
        body: &[CStmt],
    ) -> Result<(), CompileError> {
        let array_ty = self.infer_expr_type(iterable);

        // Save allocator state before for-each internal locals so they don't leak
        let saved_locals = self.locals.save();

        // Allocate temp slots (but NOT the loop variable yet — it must come after
        // the loop-top label so the stack map frame doesn't include it on the first entry)
        let arr_vtype = type_name_to_vtype_resolved(&array_ty, self.class_file);
        let arr_slot = self
            .locals
            .allocate_with_vtype("__foreach_arr", &array_ty, arr_vtype);
        let len_slot = self
            .locals
            .allocate("__foreach_len", &TypeName::Primitive(PrimitiveKind::Int));
        let idx_slot = self
            .locals
            .allocate("__foreach_idx", &TypeName::Primitive(PrimitiveKind::Int));

        // Evaluate iterable, store array ref
        self.gen_expr(iterable)?;
        self.emit_store(&array_ty, arr_slot);

        // arraylength → len
        self.emit_load(&array_ty, arr_slot);
        self.emit(Instruction::Arraylength);
        self.emit_store(&TypeName::Primitive(PrimitiveKind::Int), len_slot);

        // i = 0
        self.emit(Instruction::Iconst0);
        self.emit_store(&TypeName::Primitive(PrimitiveKind::Int), idx_slot);

        // Loop labels
        let top_label = self.new_label();
        let update_label = self.new_label();
        let end_label = self.new_label();

        // Override end_label frame to use pre-loop-variable locals
        let pre_loop_locals = self.locals.current_locals_vtypes();
        self.label_locals_override
            .push((end_label, pre_loop_locals));

        self.breakable_stack.push(BreakableContext {
            break_label: end_label,
            is_loop: true,
            continue_label: Some(update_label),
        });

        // top: if (i >= len) goto end
        self.bind_label(top_label);
        self.emit_load(&TypeName::Primitive(PrimitiveKind::Int), idx_slot);
        self.emit_load(&TypeName::Primitive(PrimitiveKind::Int), len_slot);
        self.emit_branch(Instruction::IfIcmpge, end_label);

        // Allocate loop variable AFTER the loop-top frame is recorded
        let elem_vtype = type_name_to_vtype_resolved(element_type, self.class_file);
        let var_slot = self
            .locals
            .allocate_with_vtype(var_name, element_type, elem_vtype);

        // var = arr[i]
        self.emit_load(&array_ty, arr_slot);
        self.emit_load(&TypeName::Primitive(PrimitiveKind::Int), idx_slot);
        self.emit_array_load(&array_ty);
        self.emit_store(element_type, var_slot);

        // body
        for stmt in body {
            self.gen_stmt(stmt)?;
        }

        // update: i++
        self.bind_label(update_label);
        if idx_slot <= 255 {
            self.emit(Instruction::Iinc {
                index: idx_slot as u8,
                value: 1,
            });
        } else {
            self.emit_load(&TypeName::Primitive(PrimitiveKind::Int), idx_slot);
            self.emit(Instruction::Iconst1);
            self.emit(Instruction::Iadd);
            self.emit_store(&TypeName::Primitive(PrimitiveKind::Int), idx_slot);
        }
        self.emit_goto(top_label);

        // Restore allocator so for-each internal locals don't leak
        self.locals.restore(saved_locals);
        self.bind_label(end_label);
        self.breakable_stack.pop();
        Ok(())
    }

    fn gen_foreach_iterable(
        &mut self,
        element_type: &TypeName,
        var_name: &str,
        iterable: &CExpr,
        body: &[CStmt],
    ) -> Result<(), CompileError> {
        // Save allocator state before for-each internal locals so they don't leak
        let saved_locals = self.locals.save();

        let iter_ty = TypeName::Class("java/util/Iterator".into());
        let iter_vtype = type_name_to_vtype_resolved(&iter_ty, self.class_file);
        let iter_slot = self
            .locals
            .allocate_with_vtype("__foreach_iter", &iter_ty, iter_vtype);

        // iterable.iterator() → iter_slot
        self.gen_expr(iterable)?;
        let iterator_idx = self.class_file.get_or_add_interface_method_ref(
            "java/lang/Iterable",
            "iterator",
            "()Ljava/util/Iterator;",
        );
        self.emit(Instruction::Invokeinterface {
            index: iterator_idx,
            count: 1,
            filler: 0,
        });
        self.emit_store(&iter_ty, iter_slot);

        let top_label = self.new_label();
        let end_label = self.new_label();

        // Override end_label frame to use pre-loop-variable locals
        let pre_loop_locals = self.locals.current_locals_vtypes();
        self.label_locals_override
            .push((end_label, pre_loop_locals));

        self.breakable_stack.push(BreakableContext {
            break_label: end_label,
            is_loop: true,
            continue_label: Some(top_label),
        });

        // top: if (!iter.hasNext()) goto end
        self.bind_label(top_label);
        self.emit_load(&iter_ty, iter_slot);
        let has_next_idx =
            self.class_file
                .get_or_add_interface_method_ref("java/util/Iterator", "hasNext", "()Z");
        self.emit(Instruction::Invokeinterface {
            index: has_next_idx,
            count: 1,
            filler: 0,
        });
        self.emit_branch(Instruction::Ifeq, end_label);

        // Allocate loop variable AFTER the loop-top frame is recorded
        let elem_vtype = type_name_to_vtype_resolved(element_type, self.class_file);
        let var_slot = self
            .locals
            .allocate_with_vtype(var_name, element_type, elem_vtype);

        // var = (ElementType) iter.next()
        self.emit_load(&iter_ty, iter_slot);
        let next_idx = self.class_file.get_or_add_interface_method_ref(
            "java/util/Iterator",
            "next",
            "()Ljava/lang/Object;",
        );
        self.emit(Instruction::Invokeinterface {
            index: next_idx,
            count: 1,
            filler: 0,
        });
        // Checkcast if element type is not Object
        if let TypeName::Class(name) = element_type
            && name != "Object"
            && name != "java/lang/Object"
            && name != "java.lang.Object"
        {
            let internal = resolve_class_name(name);
            let class_idx = self.class_file.get_or_add_class(&internal);
            self.emit(Instruction::Checkcast(class_idx));
        }
        self.emit_store(element_type, var_slot);

        // body
        for stmt in body {
            self.gen_stmt(stmt)?;
        }
        self.emit_goto(top_label);

        // Restore allocator so for-each internal locals don't leak
        self.locals.restore(saved_locals);
        self.bind_label(end_label);
        self.breakable_stack.pop();
        Ok(())
    }

    // --- Try-catch codegen ---

    fn gen_try_catch(
        &mut self,
        try_body: &[CStmt],
        catches: &[CatchClause],
        finally_body: Option<&[CStmt]>,
    ) -> Result<(), CompileError> {
        let try_start = self.new_label();
        let try_end = self.new_label();
        let after_all = self.new_label();

        // Capture locals at try-start for exception handler frames and merge point
        let locals_at_try_start = self.locals.current_locals_vtypes();
        // The after_all merge point must use try-start locals, since the try-body exit path
        // doesn't have catch-allocated locals.
        self.label_locals_override
            .push((after_all, locals_at_try_start.clone()));

        // Emit try body
        self.bind_label(try_start);
        for s in try_body {
            self.gen_stmt(s)?;
        }
        self.bind_label(try_end);

        // Inline finally at end of try (if present), then goto after_all
        if let Some(fin_body) = finally_body {
            for s in fin_body {
                self.gen_stmt(s)?;
            }
        }
        self.emit_goto(after_all);

        // Save allocator state before catch/finally handlers so their locals don't leak
        let saved_locals = self.locals.save();

        // Emit each catch handler
        let mut catch_handler_labels = Vec::new();
        for catch in catches {
            // Restore allocator for each catch handler so previous catch locals don't leak
            self.locals.restore(saved_locals.clone());
            let handler_label = self.new_label();
            catch_handler_labels.push(handler_label);
            // Register as exception handler for frame tracking.
            // For multi-catch, use java/lang/Throwable as the stack map type since
            // we cannot compute the least common ancestor without class hierarchy
            // knowledge. For single-catch, use the exact exception type.
            let default_type = TypeName::Class("java/lang/Exception".into());
            let first_type = catch.exception_types.first().unwrap_or(&default_type);
            let frame_class_name = if catch.exception_types.len() > 1 {
                "java/lang/Throwable"
            } else {
                match first_type {
                    TypeName::Class(name) => name.as_str(),
                    _ => "java/lang/Exception",
                }
            };
            let internal = resolve_class_name(frame_class_name);
            let ex_class_idx = self.class_file.get_or_add_class(&internal);
            self.exception_handler_labels.push((
                handler_label,
                VType::Object(ex_class_idx),
                locals_at_try_start.clone(),
            ));
            self.bind_label(handler_label);

            // astore exception to a local.
            // For multi-catch, the JVM verifier treats the exception variable as the
            // LCA type (java/lang/Throwable), matching the stack-map frame type.
            // Using a more-specific type like the first declared type would produce
            // a verifier error because the StackMapTable frame recorded Throwable on
            // the stack, so after astore the local is Throwable — not IllegalArgException.
            let (local_type, ex_vtype) = if catch.exception_types.len() > 1 {
                let throwable_ty = TypeName::Class("java/lang/Throwable".into());
                let vtype = type_name_to_vtype_resolved(&throwable_ty, self.class_file);
                (throwable_ty, vtype)
            } else {
                let vtype = type_name_to_vtype_resolved(first_type, self.class_file);
                (first_type.clone(), vtype)
            };
            let ex_slot = self
                .locals
                .allocate_with_vtype(&catch.var_name, &local_type, ex_vtype);
            self.emit_store(&local_type, ex_slot);

            // Emit catch body, tracking whether it ends with an unconditional transfer.
            let before_body = self.instructions.len();
            for s in &catch.body {
                self.gen_stmt(s)?;
            }
            let body_ends_with_transfer =
                self.instructions.len() > before_body && self.last_is_unconditional_transfer();

            // Inline finally (if present)
            if let Some(fin_body) = finally_body {
                for s in fin_body {
                    self.gen_stmt(s)?;
                }
            }
            // Skip goto if the catch body already produced an unconditional transfer
            // (e.g., `continue`, `break`, `return`, `throw`). Emitting a dead goto
            // after an unconditional branch requires a StackMapTable frame that the
            // JVM verifier would complain about.
            if !body_ends_with_transfer {
                self.emit_goto(after_all);
            }
        }

        // If finally present: emit catch-all handler that stores exception, runs finally, rethrows
        let catch_all_label = if finally_body.is_some() {
            self.locals.restore(saved_locals.clone());
            let label = self.new_label();
            // Register as exception handler for frame tracking
            let throwable_idx = self.class_file.get_or_add_class("java/lang/Throwable");
            self.exception_handler_labels.push((
                label,
                VType::Object(throwable_idx),
                locals_at_try_start.clone(),
            ));
            self.bind_label(label);
            let ex_ty = TypeName::Class("java/lang/Throwable".into());
            let ex_vtype = type_name_to_vtype_resolved(&ex_ty, self.class_file);
            let ex_slot = self
                .locals
                .allocate_with_vtype("__finally_ex", &ex_ty, ex_vtype);
            self.emit_store(&ex_ty, ex_slot);
            if let Some(fin_body) = finally_body {
                for s in fin_body {
                    self.gen_stmt(s)?;
                }
            }
            self.emit_load(&ex_ty, ex_slot);
            self.emit(Instruction::Athrow);
            Some(label)
        } else {
            None
        };

        // Restore allocator so catch/finally locals don't leak into subsequent code
        self.locals.restore(saved_locals);

        self.bind_label(after_all);

        // Register exception table entries (one per exception type per catch)
        for (i, catch) in catches.iter().enumerate() {
            for exc_type in &catch.exception_types {
                let internal = resolve_class_name(match exc_type {
                    TypeName::Class(name) => name.as_str(),
                    _ => "java/lang/Exception",
                });
                let catch_type = self.class_file.get_or_add_class(&internal);
                self.pending_exceptions.push(PendingExceptionEntry {
                    start_label: try_start,
                    end_label: try_end,
                    handler_label: catch_handler_labels[i],
                    catch_type,
                });
            }
        }

        // Catch-all for finally
        if let Some(catch_all) = catch_all_label {
            self.pending_exceptions.push(PendingExceptionEntry {
                start_label: try_start,
                end_label: try_end,
                handler_label: catch_all,
                catch_type: 0, // 0 = catch all
            });
        }

        Ok(())
    }

    // --- Synchronized codegen ---

    fn gen_synchronized(&mut self, lock_expr: &CExpr, body: &[CStmt]) -> Result<(), CompileError> {
        let lock_ty = TypeName::Class("java/lang/Object".into());
        let lock_vtype = type_name_to_vtype_resolved(&lock_ty, self.class_file);
        let lock_slot = self
            .locals
            .allocate_with_vtype("__sync_lock", &lock_ty, lock_vtype);

        // Evaluate lock expression, dup, store to temp, monitorenter
        self.gen_expr(lock_expr)?;
        self.emit(Instruction::Dup);
        self.emit_store(&lock_ty, lock_slot);
        self.emit(Instruction::Monitorenter);

        let try_start = self.new_label();
        let try_end = self.new_label();
        let catch_handler = self.new_label();
        let after_all = self.new_label();

        // Capture locals at try-start for exception handler frames
        let locals_at_try_start = self.locals.current_locals_vtypes();
        self.label_locals_override
            .push((after_all, locals_at_try_start.clone()));

        // Try body
        self.bind_label(try_start);
        for s in body {
            self.gen_stmt(s)?;
        }
        self.bind_label(try_end);

        // Normal exit: monitorexit + goto after_all
        self.emit_load(&lock_ty, lock_slot);
        self.emit(Instruction::Monitorexit);
        self.emit_goto(after_all);

        // Save allocator state before catch-all handler so its locals don't leak
        let saved_locals = self.locals.save();

        // Catch-all handler: store exception, monitorexit, rethrow
        let throwable_idx = self.class_file.get_or_add_class("java/lang/Throwable");
        self.exception_handler_labels.push((
            catch_handler,
            VType::Object(throwable_idx),
            locals_at_try_start,
        ));
        self.bind_label(catch_handler);

        let ex_ty = TypeName::Class("java/lang/Throwable".into());
        let sync_ex_vtype = type_name_to_vtype_resolved(&ex_ty, self.class_file);
        let ex_slot = self
            .locals
            .allocate_with_vtype("__sync_ex", &ex_ty, sync_ex_vtype);
        self.emit_store(&ex_ty, ex_slot);
        self.emit_load(&lock_ty, lock_slot);
        self.emit(Instruction::Monitorexit);
        self.emit_load(&ex_ty, ex_slot);
        self.emit(Instruction::Athrow);

        // Restore allocator so catch-all locals don't leak
        self.locals.restore(saved_locals);

        self.bind_label(after_all);

        // Exception table: [try_start, try_end) → catch_handler, catch_type=0 (catch all)
        self.pending_exceptions.push(PendingExceptionEntry {
            start_label: try_start,
            end_label: try_end,
            handler_label: catch_handler,
            catch_type: 0,
        });

        Ok(())
    }

    // --- Method/field resolution ---

    fn gen_method_call(
        &mut self,
        object: Option<&CExpr>,
        name: &str,
        args: &[CExpr],
    ) -> Result<(), CompileError> {
        match object {
            Some(obj) => {
                // Check for string concatenation: "something" + ... results in StringBuilder pattern
                // But first, check if this is a chain like System.out.println
                // Try to resolve as chain: detect Class.field.method pattern
                if let Some((class_name, field_chain, is_static_root)) = self.resolve_dot_chain(obj)
                    && is_static_root
                {
                    if field_chain.is_empty() {
                        // Direct static method call: ClassName.method(args)
                        return self.gen_static_method_call(&class_name, name, args);
                    }
                    // Static field access chain, e.g. System.out.println
                    self.gen_static_chain_method_call(&class_name, &field_chain, name, args)?;
                    return Ok(());
                }

                // Regular instance method call
                self.gen_expr(obj)?;
                for arg in args {
                    self.gen_expr(arg)?;
                }
                // Try to find the method ref in the constant pool
                let descriptor = self.find_method_descriptor_in_pool(name, args)?;
                let class_name = self.infer_receiver_class(obj)?;
                if self.is_interface_method(&class_name, name) {
                    let method_idx = self.class_file.get_or_add_interface_method_ref(
                        &class_name,
                        name,
                        &descriptor,
                    );
                    let count = compute_invokeinterface_count(&descriptor);
                    self.emit(Instruction::Invokeinterface {
                        index: method_idx,
                        count,
                        filler: 0,
                    });
                } else {
                    let method_idx =
                        self.class_file
                            .get_or_add_method_ref(&class_name, name, &descriptor);
                    self.emit(Instruction::Invokevirtual(method_idx));
                }
                Ok(())
            }
            None => {
                // Unqualified method call - call on `this`
                if !self.is_static {
                    self.emit(Instruction::Aload0); // this
                }
                for arg in args {
                    self.gen_expr(arg)?;
                }
                let descriptor = self.find_method_descriptor_in_pool(name, args)?;
                let this_class = self.get_this_class_name()?;
                let method_idx =
                    self.class_file
                        .get_or_add_method_ref(&this_class, name, &descriptor);
                if self.is_static {
                    self.emit(Instruction::Invokestatic(method_idx));
                } else {
                    self.emit(Instruction::Invokevirtual(method_idx));
                }
                Ok(())
            }
        }
    }

    fn gen_static_method_call(
        &mut self,
        class_name: &str,
        name: &str,
        args: &[CExpr],
    ) -> Result<(), CompileError> {
        for arg in args {
            self.gen_expr(arg)?;
        }
        let internal = resolve_class_name(class_name);
        let descriptor = self.find_method_descriptor_in_pool(name, args)?;
        let method_idx = self
            .class_file
            .get_or_add_method_ref(&internal, name, &descriptor);
        self.emit(Instruction::Invokestatic(method_idx));
        Ok(())
    }

    fn gen_field_access(&mut self, object: &CExpr, name: &str) -> Result<(), CompileError> {
        // Check if this is a static field access (e.g., System.out)
        if let CExpr::Ident(ident_name) = object {
            // Check constant pool for a FieldRef with this class name
            let resolved = resolve_class_name(ident_name);
            if self.has_field_ref_for_class(&resolved, name)
                || ident_name
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_uppercase())
            {
                return self.gen_static_field_access(ident_name, name);
            }
        }
        // Handle array.length → arraylength instruction
        if name == "length" {
            let ty = self.infer_expr_type(object);
            if matches!(ty, TypeName::Array(_)) {
                self.gen_expr(object)?;
                self.emit(Instruction::Arraylength);
                return Ok(());
            }
        }
        self.gen_expr(object)?;
        let class_name = self.infer_receiver_class(object)?;
        let descriptor = self.find_field_descriptor_in_pool(&class_name, name)?;
        let field_idx = self
            .class_file
            .get_or_add_field_ref(&class_name, name, &descriptor);
        self.emit(Instruction::Getfield(field_idx));
        Ok(())
    }

    fn gen_static_field_access(
        &mut self,
        class_name: &str,
        name: &str,
    ) -> Result<(), CompileError> {
        let internal = resolve_class_name(class_name);
        let descriptor = self.find_field_descriptor_in_pool(&internal, name)?;
        let field_idx = self
            .class_file
            .get_or_add_field_ref(&internal, name, &descriptor);
        self.emit(Instruction::Getstatic(field_idx));
        Ok(())
    }

    /// Handle chains like System.out.println(x):
    /// System is the class, out is the static field, println is the method
    fn gen_static_chain_method_call(
        &mut self,
        class_name: &str,
        field_chain: &[String],
        method_name: &str,
        args: &[CExpr],
    ) -> Result<(), CompileError> {
        let internal = resolve_class_name(class_name);

        // Start with the static field access
        let first_field = &field_chain[0];
        let field_desc = self.find_field_descriptor_in_pool(&internal, first_field)?;
        let field_idx = self
            .class_file
            .get_or_add_field_ref(&internal, first_field, &field_desc);
        self.emit(Instruction::Getstatic(field_idx));

        // For subsequent fields in the chain, use getfield
        let mut current_type_desc = field_desc;
        for field_name in &field_chain[1..] {
            let field_class = descriptor_to_internal(&current_type_desc)?;
            let fd = self.find_field_descriptor_in_pool(&field_class, field_name)?;
            let fi = self
                .class_file
                .get_or_add_field_ref(&field_class, field_name, &fd);
            self.emit(Instruction::Getfield(fi));
            current_type_desc = fd;
        }

        // Now emit args and invoke the method on the field's type
        for arg in args {
            self.gen_expr(arg)?;
        }

        let receiver_class = descriptor_to_internal(&current_type_desc)?;
        let method_desc = self.find_method_descriptor_in_pool(method_name, args)?;
        let method_idx =
            self.class_file
                .get_or_add_method_ref(&receiver_class, method_name, &method_desc);
        self.emit(Instruction::Invokevirtual(method_idx));
        Ok(())
    }

    /// Try to resolve a dot-chain to (root_class, [field_names], is_static).
    fn resolve_dot_chain(&self, expr: &CExpr) -> Option<(String, Vec<String>, bool)> {
        let mut fields = Vec::new();
        let mut current = expr;

        loop {
            match current {
                CExpr::FieldAccess { object, name } => {
                    fields.push(name.clone());
                    current = object;
                }
                CExpr::Ident(name) => {
                    // Check if this is a class name (starts with uppercase)
                    if name.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                        fields.reverse();
                        return Some((name.clone(), fields, true));
                    }
                    return None;
                }
                _ => return None,
            }
        }
    }

    // --- Store target ---

    fn gen_store_target(&mut self, target: &CExpr) -> Result<(), CompileError> {
        match target {
            CExpr::Ident(name) => {
                let (slot, ty) =
                    self.locals
                        .find(name)
                        .ok_or_else(|| CompileError::CodegenError {
                            message: format!("undefined variable: {}", name),
                        })?;
                let ty = ty.clone();
                self.emit_store(&ty, slot);
                Ok(())
            }
            CExpr::FieldAccess { object, name } => {
                // value is on stack, we need [objectref, value] for putfield
                let class_name = self.infer_receiver_class(object)?;
                let descriptor = self.find_field_descriptor_in_pool(&class_name, name)?;
                let field_idx =
                    self.class_file
                        .get_or_add_field_ref(&class_name, name, &descriptor);

                let value_ty = descriptor_to_type(&descriptor);
                if type_slot_width(&value_ty) == 2 {
                    // Category-2 value (long/double): Swap won't work.
                    // Store value to temp, push objectref, reload value.
                    let tmp_vtype = type_name_to_vtype_resolved(&value_ty, self.class_file);
                    let temp = self
                        .locals
                        .allocate_with_vtype("__field_tmp", &value_ty, tmp_vtype);
                    self.emit_store(&value_ty, temp);
                    self.gen_expr(object)?;
                    self.emit_load(&value_ty, temp);
                } else {
                    // Category-1: Swap works fine
                    self.gen_expr(object)?;
                    self.emit(Instruction::Swap);
                }
                self.emit(Instruction::Putfield(field_idx));
                Ok(())
            }
            CExpr::ArrayAccess { array, index } => {
                // Stack has: [..., value]. We need: [..., arrayref, index, value]
                // Strategy: store value to temp, push arrayref+index, reload value, store
                let array_ty = self.infer_expr_type(array);
                let elem_ty = match &array_ty {
                    TypeName::Array(inner) => inner.as_ref().clone(),
                    _ => TypeName::Primitive(PrimitiveKind::Int),
                };
                let arr_tmp_vtype = type_name_to_vtype_resolved(&elem_ty, self.class_file);
                let temp =
                    self.locals
                        .allocate_with_vtype("__arr_store_tmp", &elem_ty, arr_tmp_vtype);
                self.emit_store(&elem_ty, temp);
                self.gen_expr(array)?;
                self.gen_expr(index)?;
                self.emit_load(&elem_ty, temp);
                self.emit_array_store(&array_ty);
                Ok(())
            }
            _ => Err(CompileError::CodegenError {
                message: "invalid assignment target".into(),
            }),
        }
    }

    // --- Helper: does this expression leave a value on the stack? ---

    fn expr_leaves_value(&self, expr: &CExpr) -> bool {
        match expr {
            CExpr::Assign { .. } => {
                // All assignments leave the assigned value on the stack (arrays use dup_x2).
                true
            }
            CExpr::CompoundAssign { .. } => true,
            CExpr::PostIncrement(_) | CExpr::PostDecrement(_) => true,
            CExpr::PreIncrement(_) | CExpr::PreDecrement(_) => true,
            CExpr::MethodCall {
                object, name, args, ..
            } => {
                // Check if the method returns void by looking up in pool
                // For simplicity, assume non-void unless we can determine otherwise
                // Well-known void methods:
                if name == "println" || name == "print" || name == "close" || name == "flush" {
                    // These are commonly void but we still need to check
                    // For now, check if the call is on a PrintStream-like object
                    if let Some(obj) = object
                        && self.is_print_stream_chain(obj)
                    {
                        return false;
                    }
                }
                // Try to find method descriptor to check return type
                if let Ok(desc) = self.find_method_descriptor_in_pool(name, args)
                    && desc.ends_with(")V")
                {
                    return false;
                }
                true
            }
            CExpr::StaticMethodCall { name, args, .. } => {
                if let Ok(desc) = self.find_method_descriptor_in_pool(name, args)
                    && desc.ends_with(")V")
                {
                    return false;
                }
                true
            }
            _ => true,
        }
    }

    fn is_print_stream_chain(&self, expr: &CExpr) -> bool {
        // Detect System.out pattern
        if let CExpr::FieldAccess { object, name } = expr
            && (name == "out" || name == "err")
            && let CExpr::Ident(class_name) = object.as_ref()
            && class_name == "System"
        {
            return true;
        }
        false
    }

    // --- Instruction emission helpers ---

    fn emit_int_const(&mut self, value: i64) {
        match value {
            -1 => {
                self.emit(Instruction::Iconstm1);
            }
            0 => {
                self.emit(Instruction::Iconst0);
            }
            1 => {
                self.emit(Instruction::Iconst1);
            }
            2 => {
                self.emit(Instruction::Iconst2);
            }
            3 => {
                self.emit(Instruction::Iconst3);
            }
            4 => {
                self.emit(Instruction::Iconst4);
            }
            5 => {
                self.emit(Instruction::Iconst5);
            }
            v if (-128..=127).contains(&v) => {
                self.emit(Instruction::Bipush(v as i8));
            }
            v if (-32768..=32767).contains(&v) => {
                self.emit(Instruction::Sipush(v as i16));
            }
            v => {
                let cp_idx = self.class_file.get_or_add_integer(v as i32);
                self.emit_ldc(cp_idx);
            }
        }
    }

    fn emit_long_const(&mut self, value: i64) {
        match value {
            0 => {
                self.emit(Instruction::Lconst0);
            }
            1 => {
                self.emit(Instruction::Lconst1);
            }
            _ => {
                let cp_idx = self.class_file.get_or_add_long(value);
                self.emit(Instruction::Ldc2W(cp_idx));
            }
        }
    }

    fn emit_float_const(&mut self, value: f32) {
        if value == 0.0 && value.is_sign_positive() {
            self.emit(Instruction::Fconst0);
        } else if value == 1.0 {
            self.emit(Instruction::Fconst1);
        } else if value == 2.0 {
            self.emit(Instruction::Fconst2);
        } else {
            let cp_idx = self.class_file.get_or_add_float(value);
            self.emit_ldc(cp_idx);
        }
    }

    fn emit_double_const(&mut self, value: f64) {
        if value == 0.0 && value.is_sign_positive() {
            self.emit(Instruction::Dconst0);
        } else if value == 1.0 {
            self.emit(Instruction::Dconst1);
        } else {
            let cp_idx = self.class_file.get_or_add_double(value);
            self.emit(Instruction::Ldc2W(cp_idx));
        }
    }

    fn emit_ldc(&mut self, cp_idx: u16) {
        if cp_idx <= 255 {
            self.emit(Instruction::Ldc(cp_idx as u8));
        } else {
            self.emit(Instruction::LdcW(cp_idx));
        }
    }

    fn emit_load(&mut self, ty: &TypeName, slot: u16) {
        if is_reference_type(ty) {
            match slot {
                0 => self.emit(Instruction::Aload0),
                1 => self.emit(Instruction::Aload1),
                2 => self.emit(Instruction::Aload2),
                3 => self.emit(Instruction::Aload3),
                s if s <= 255 => self.emit(Instruction::Aload(s as u8)),
                s => self.emit(Instruction::AloadWide(s)),
            };
        } else if is_long_type(ty) {
            match slot {
                0 => self.emit(Instruction::Lload0),
                1 => self.emit(Instruction::Lload1),
                2 => self.emit(Instruction::Lload2),
                3 => self.emit(Instruction::Lload3),
                s if s <= 255 => self.emit(Instruction::Lload(s as u8)),
                s => self.emit(Instruction::LloadWide(s)),
            };
        } else if is_float_type(ty) {
            match slot {
                0 => self.emit(Instruction::Fload0),
                1 => self.emit(Instruction::Fload1),
                2 => self.emit(Instruction::Fload2),
                3 => self.emit(Instruction::Fload3),
                s if s <= 255 => self.emit(Instruction::Fload(s as u8)),
                s => self.emit(Instruction::FloadWide(s)),
            };
        } else if is_double_type(ty) {
            match slot {
                0 => self.emit(Instruction::Dload0),
                1 => self.emit(Instruction::Dload1),
                2 => self.emit(Instruction::Dload2),
                3 => self.emit(Instruction::Dload3),
                s if s <= 255 => self.emit(Instruction::Dload(s as u8)),
                s => self.emit(Instruction::DloadWide(s)),
            };
        } else {
            // int and friends
            match slot {
                0 => self.emit(Instruction::Iload0),
                1 => self.emit(Instruction::Iload1),
                2 => self.emit(Instruction::Iload2),
                3 => self.emit(Instruction::Iload3),
                s if s <= 255 => self.emit(Instruction::Iload(s as u8)),
                s => self.emit(Instruction::IloadWide(s)),
            };
        }
    }

    fn emit_store(&mut self, ty: &TypeName, slot: u16) {
        if is_reference_type(ty) {
            match slot {
                0 => self.emit(Instruction::Astore0),
                1 => self.emit(Instruction::Astore1),
                2 => self.emit(Instruction::Astore2),
                3 => self.emit(Instruction::Astore3),
                s if s <= 255 => self.emit(Instruction::Astore(s as u8)),
                s => self.emit(Instruction::AstoreWide(s)),
            };
        } else if is_long_type(ty) {
            match slot {
                0 => self.emit(Instruction::Lstore0),
                1 => self.emit(Instruction::Lstore1),
                2 => self.emit(Instruction::Lstore2),
                3 => self.emit(Instruction::Lstore3),
                s if s <= 255 => self.emit(Instruction::Lstore(s as u8)),
                s => self.emit(Instruction::LstoreWide(s)),
            };
        } else if is_float_type(ty) {
            match slot {
                0 => self.emit(Instruction::Fstore0),
                1 => self.emit(Instruction::Fstore1),
                2 => self.emit(Instruction::Fstore2),
                3 => self.emit(Instruction::Fstore3),
                s if s <= 255 => self.emit(Instruction::Fstore(s as u8)),
                s => self.emit(Instruction::FstoreWide(s)),
            };
        } else if is_double_type(ty) {
            match slot {
                0 => self.emit(Instruction::Dstore0),
                1 => self.emit(Instruction::Dstore1),
                2 => self.emit(Instruction::Dstore2),
                3 => self.emit(Instruction::Dstore3),
                s if s <= 255 => self.emit(Instruction::Dstore(s as u8)),
                s => self.emit(Instruction::DstoreWide(s)),
            };
        } else {
            match slot {
                0 => self.emit(Instruction::Istore0),
                1 => self.emit(Instruction::Istore1),
                2 => self.emit(Instruction::Istore2),
                3 => self.emit(Instruction::Istore3),
                s if s <= 255 => self.emit(Instruction::Istore(s as u8)),
                s => self.emit(Instruction::IstoreWide(s)),
            };
        }
    }

    // --- Typed instruction emission ---

    fn emit_typed_binary_op(&mut self, op: &BinOp, ty: &TypeName) -> Result<(), CompileError> {
        if is_long_type(ty) {
            let instr = match op {
                BinOp::Add => Instruction::Ladd,
                BinOp::Sub => Instruction::Lsub,
                BinOp::Mul => Instruction::Lmul,
                BinOp::Div => Instruction::Ldiv,
                BinOp::Rem => Instruction::Lrem,
                BinOp::Shl => Instruction::Lshl,
                BinOp::Shr => Instruction::Lshr,
                BinOp::Ushr => Instruction::Lushr,
                BinOp::BitAnd => Instruction::Land,
                BinOp::BitOr => Instruction::Lor,
                BinOp::BitXor => Instruction::Lxor,
            };
            self.emit(instr);
        } else if is_float_type(ty) {
            let instr = match op {
                BinOp::Add => Instruction::Fadd,
                BinOp::Sub => Instruction::Fsub,
                BinOp::Mul => Instruction::Fmul,
                BinOp::Div => Instruction::Fdiv,
                BinOp::Rem => Instruction::Frem,
                _ => {
                    return Err(CompileError::CodegenError {
                        message: format!("bitwise/shift operator {:?} is not valid on float", op),
                    });
                }
            };
            self.emit(instr);
        } else if is_double_type(ty) {
            let instr = match op {
                BinOp::Add => Instruction::Dadd,
                BinOp::Sub => Instruction::Dsub,
                BinOp::Mul => Instruction::Dmul,
                BinOp::Div => Instruction::Ddiv,
                BinOp::Rem => Instruction::Drem,
                _ => {
                    return Err(CompileError::CodegenError {
                        message: format!("bitwise/shift operator {:?} is not valid on double", op),
                    });
                }
            };
            self.emit(instr);
        } else {
            // int and sub-int types
            let instr = match op {
                BinOp::Add => Instruction::Iadd,
                BinOp::Sub => Instruction::Isub,
                BinOp::Mul => Instruction::Imul,
                BinOp::Div => Instruction::Idiv,
                BinOp::Rem => Instruction::Irem,
                BinOp::Shl => Instruction::Ishl,
                BinOp::Shr => Instruction::Ishr,
                BinOp::Ushr => Instruction::Iushr,
                BinOp::BitAnd => Instruction::Iand,
                BinOp::BitOr => Instruction::Ior,
                BinOp::BitXor => Instruction::Ixor,
            };
            self.emit(instr);
        }
        Ok(())
    }

    /// Emit a typed comparison instruction for non-int types.
    /// For long: lcmp; for float: fcmpl/fcmpg; for double: dcmpl/dcmpg.
    /// The result is an int that can be used with ifeq/ifne/iflt/ifge/ifgt/ifle.
    fn emit_typed_compare(&mut self, ty: &TypeName, op: &CompareOp) {
        if is_long_type(ty) {
            self.emit(Instruction::Lcmp);
        } else if is_float_type(ty) {
            // fcmpg for > and >= (NaN → 1, so false branch taken correctly)
            // fcmpl for everything else (NaN → -1)
            match op {
                CompareOp::Gt | CompareOp::Ge => self.emit(Instruction::Fcmpg),
                _ => self.emit(Instruction::Fcmpl),
            };
        } else if is_double_type(ty) {
            match op {
                CompareOp::Gt | CompareOp::Ge => self.emit(Instruction::Dcmpg),
                _ => self.emit(Instruction::Dcmpl),
            };
        }
    }

    // --- String concatenation ---

    /// Check if a BinaryOp::Add expression involves string concatenation.
    fn is_string_concat(&self, expr: &CExpr) -> bool {
        if let CExpr::BinaryOp {
            op: BinOp::Add,
            left,
            right,
        } = expr
        {
            let lt = self.infer_expr_type(left);
            let rt = self.infer_expr_type(right);
            is_string_type(&lt)
                || is_string_type(&rt)
                || self.is_string_concat(left)
                || self.is_string_concat(right)
        } else {
            false
        }
    }

    /// Flatten a chain of BinaryOp::Add nodes into a list of parts for StringBuilder.
    fn flatten_string_concat<'b>(&self, expr: &'b CExpr, parts: &mut Vec<&'b CExpr>) {
        if let CExpr::BinaryOp {
            op: BinOp::Add,
            left,
            right,
        } = expr
            && self.is_string_concat(expr)
        {
            self.flatten_string_concat(left, parts);
            self.flatten_string_concat(right, parts);
            return;
        }
        parts.push(expr);
    }

    /// Generate StringBuilder-based string concatenation.
    fn gen_string_concat(&mut self, expr: &CExpr) -> Result<(), CompileError> {
        let mut parts = Vec::new();
        self.flatten_string_concat(expr, &mut parts);

        // new StringBuilder()
        let sb_class = self.class_file.get_or_add_class("java/lang/StringBuilder");
        self.emit(Instruction::New(sb_class));
        self.emit(Instruction::Dup);
        let init_idx =
            self.class_file
                .get_or_add_method_ref("java/lang/StringBuilder", "<init>", "()V");
        self.emit(Instruction::Invokespecial(init_idx));

        // .append(part) for each part
        for part in &parts {
            let desc = self.infer_append_descriptor(part);
            self.gen_expr(part)?;
            let append_idx =
                self.class_file
                    .get_or_add_method_ref("java/lang/StringBuilder", "append", &desc);
            self.emit(Instruction::Invokevirtual(append_idx));
        }

        // .toString()
        let tostring_idx = self.class_file.get_or_add_method_ref(
            "java/lang/StringBuilder",
            "toString",
            "()Ljava/lang/String;",
        );
        self.emit(Instruction::Invokevirtual(tostring_idx));
        Ok(())
    }

    // --- Type inference ---

    /// Infer the type that an expression produces on the JVM stack.
    fn infer_expr_type(&self, expr: &CExpr) -> TypeName {
        match expr {
            CExpr::IntLiteral(_) => TypeName::Primitive(PrimitiveKind::Int),
            CExpr::LongLiteral(_) => TypeName::Primitive(PrimitiveKind::Long),
            CExpr::FloatLiteral(_) => TypeName::Primitive(PrimitiveKind::Float),
            CExpr::DoubleLiteral(_) => TypeName::Primitive(PrimitiveKind::Double),
            CExpr::BoolLiteral(_) => TypeName::Primitive(PrimitiveKind::Boolean),
            CExpr::CharLiteral(_) => TypeName::Primitive(PrimitiveKind::Char),
            CExpr::StringLiteral(_) => TypeName::Class("String".into()),
            CExpr::NullLiteral => TypeName::Class("Object".into()),
            CExpr::This => TypeName::Class("this".into()),
            CExpr::Ident(name) => {
                if let Some((_, ty)) = self.locals.find(name) {
                    ty.clone()
                } else {
                    TypeName::Primitive(PrimitiveKind::Int)
                }
            }
            CExpr::BinaryOp { op, left, right } => {
                let lt = self.infer_expr_type(left);
                let rt = self.infer_expr_type(right);
                if *op == BinOp::Add && (is_string_type(&lt) || is_string_type(&rt)) {
                    TypeName::Class("String".into())
                } else {
                    promote_numeric_type(&lt, &rt)
                }
            }
            CExpr::UnaryOp { operand, .. } => self.infer_expr_type(operand),
            CExpr::Comparison { .. }
            | CExpr::LogicalAnd(_, _)
            | CExpr::LogicalOr(_, _)
            | CExpr::LogicalNot(_)
            | CExpr::Instanceof { .. } => TypeName::Primitive(PrimitiveKind::Boolean),
            CExpr::Cast { ty, .. } => ty.clone(),
            CExpr::Assign { value, .. } => self.infer_expr_type(value),
            CExpr::CompoundAssign { target, .. } => self.infer_expr_type(target),
            CExpr::PreIncrement(e)
            | CExpr::PreDecrement(e)
            | CExpr::PostIncrement(e)
            | CExpr::PostDecrement(e) => self.infer_expr_type(e),
            CExpr::NewObject { class_name, .. } => TypeName::Class(class_name.clone()),
            CExpr::NewArray { element_type, .. } => TypeName::Array(Box::new(element_type.clone())),
            CExpr::NewMultiArray {
                element_type,
                dimensions,
            } => {
                let mut ty = element_type.clone();
                for _ in 0..dimensions.len() {
                    ty = TypeName::Array(Box::new(ty));
                }
                ty
            }
            CExpr::SwitchExpr {
                cases,
                default_expr,
                ..
            } => {
                if let Some(first_case) = cases.first() {
                    self.infer_expr_type(&first_case.expr)
                } else {
                    self.infer_expr_type(default_expr)
                }
            }
            CExpr::Lambda { .. } | CExpr::MethodRef { .. } => TypeName::Class("Object".into()),
            CExpr::ArrayAccess { array, .. } => {
                match self.infer_expr_type(array) {
                    TypeName::Array(inner) => *inner,
                    _ => TypeName::Primitive(PrimitiveKind::Int), // fallback
                }
            }
            CExpr::Ternary { then_expr, .. } => self.infer_expr_type(then_expr),
            CExpr::MethodCall { name, args, .. } | CExpr::StaticMethodCall { name, args, .. } => {
                // Try to infer return type from method descriptor
                if let Ok(desc) = self.find_method_descriptor_in_pool(name, args)
                    && let Some(ret_start) = desc.rfind(')')
                {
                    let ret_desc = &desc[ret_start + 1..];
                    return descriptor_to_type(ret_desc);
                }
                TypeName::Class("Object".into())
            }
            CExpr::FieldAccess { object, name } => {
                // array.length → int
                if name == "length" {
                    let obj_ty = self.infer_expr_type(object);
                    if matches!(obj_ty, TypeName::Array(_)) {
                        return TypeName::Primitive(PrimitiveKind::Int);
                    }
                }
                TypeName::Class("Object".into())
            }
            CExpr::StaticFieldAccess { .. } => TypeName::Class("Object".into()),
        }
    }

    /// Emit a widening conversion if `from` is narrower than `to`.
    fn emit_widen_if_needed(&mut self, from: &TypeName, to: &TypeName) {
        if numeric_rank(from) >= numeric_rank(to) {
            return;
        }
        match (numeric_rank(from), numeric_rank(to)) {
            (0, 1) => {
                self.emit(Instruction::I2l);
            } // int → long
            (0, 2) => {
                self.emit(Instruction::I2f);
            } // int → float
            (0, 3) => {
                self.emit(Instruction::I2d);
            } // int → double
            (1, 2) => {
                self.emit(Instruction::L2f);
            } // long → float
            (1, 3) => {
                self.emit(Instruction::L2d);
            } // long → double
            (2, 3) => {
                self.emit(Instruction::F2d);
            } // float → double
            _ => {}
        }
    }

    /// Emit a type-appropriate array load instruction based on the array type.
    fn emit_array_load(&mut self, array_ty: &TypeName) {
        match array_ty {
            TypeName::Array(inner) => match inner.as_ref() {
                TypeName::Primitive(PrimitiveKind::Int) => {
                    self.emit(Instruction::Iaload);
                }
                TypeName::Primitive(PrimitiveKind::Long) => {
                    self.emit(Instruction::Laload);
                }
                TypeName::Primitive(PrimitiveKind::Float) => {
                    self.emit(Instruction::Faload);
                }
                TypeName::Primitive(PrimitiveKind::Double) => {
                    self.emit(Instruction::Daload);
                }
                TypeName::Primitive(PrimitiveKind::Byte | PrimitiveKind::Boolean) => {
                    self.emit(Instruction::Baload);
                }
                TypeName::Primitive(PrimitiveKind::Char) => {
                    self.emit(Instruction::Caload);
                }
                TypeName::Primitive(PrimitiveKind::Short) => {
                    self.emit(Instruction::Saload);
                }
                _ => {
                    self.emit(Instruction::Aaload);
                }
            },
            _ => {
                self.emit(Instruction::Aaload);
            }
        }
    }

    /// Emit a type-appropriate array store instruction based on the array type.
    fn emit_array_store(&mut self, array_ty: &TypeName) {
        match array_ty {
            TypeName::Array(inner) => match inner.as_ref() {
                TypeName::Primitive(PrimitiveKind::Int) => {
                    self.emit(Instruction::Iastore);
                }
                TypeName::Primitive(PrimitiveKind::Long) => {
                    self.emit(Instruction::Lastore);
                }
                TypeName::Primitive(PrimitiveKind::Float) => {
                    self.emit(Instruction::Fastore);
                }
                TypeName::Primitive(PrimitiveKind::Double) => {
                    self.emit(Instruction::Dastore);
                }
                TypeName::Primitive(PrimitiveKind::Byte | PrimitiveKind::Boolean) => {
                    self.emit(Instruction::Bastore);
                }
                TypeName::Primitive(PrimitiveKind::Char) => {
                    self.emit(Instruction::Castore);
                }
                TypeName::Primitive(PrimitiveKind::Short) => {
                    self.emit(Instruction::Sastore);
                }
                _ => {
                    self.emit(Instruction::Aastore);
                }
            },
            _ => {
                self.emit(Instruction::Aastore);
            }
        }
    }

    /// Emit the constant `1` in the appropriate type.
    fn emit_typed_const_one(&mut self, ty: &TypeName) {
        if is_long_type(ty) {
            self.emit(Instruction::Lconst1);
        } else if is_float_type(ty) {
            self.emit(Instruction::Fconst1);
        } else if is_double_type(ty) {
            self.emit(Instruction::Dconst1);
        } else {
            self.emit(Instruction::Iconst1);
        }
    }

    /// Emit a narrowing conversion from a wider type back to a target type.
    fn emit_narrow(&mut self, from: &TypeName, to: &TypeName) {
        let from_rank = numeric_rank(from);
        let to_rank = numeric_rank(to);
        if from_rank <= to_rank {
            return;
        }
        match (from_rank, to_rank) {
            (3, 2) => {
                self.emit(Instruction::D2f);
            }
            (3, 1) => {
                self.emit(Instruction::D2l);
            }
            (3, 0) => {
                self.emit(Instruction::D2i);
            }
            (2, 1) => {
                self.emit(Instruction::F2l);
            }
            (2, 0) => {
                self.emit(Instruction::F2i);
            }
            (1, 0) => {
                self.emit(Instruction::L2i);
            }
            _ => {}
        }
    }

    // --- Type/descriptor resolution helpers ---

    fn get_this_class_name(&self) -> Result<String, CompileError> {
        use crate::constant_info::ConstantInfo;
        let this_class = self.class_file.this_class;
        match &self.class_file.const_pool[(this_class - 1) as usize] {
            ConstantInfo::Class(c) => self
                .class_file
                .get_utf8(c.name_index)
                .map(|s| s.to_string())
                .ok_or_else(|| CompileError::CodegenError {
                    message: "could not resolve this class name".into(),
                }),
            _ => Err(CompileError::CodegenError {
                message: "this_class does not point to a Class constant".into(),
            }),
        }
    }

    fn infer_receiver_class(&self, expr: &CExpr) -> Result<String, CompileError> {
        match expr {
            CExpr::This => self.get_this_class_name(),
            CExpr::Ident(name) => {
                // Check local variable type
                if let Some((_, ty)) = self.locals.find(name)
                    && let TypeName::Class(class_name) = ty
                {
                    return Ok(resolve_class_name(class_name));
                }
                // Might be a class name for static access
                if name.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
                    return Ok(resolve_class_name(name));
                }
                Err(CompileError::CodegenError {
                    message: format!("cannot infer class for receiver '{}'", name),
                })
            }
            CExpr::FieldAccess {
                object,
                name: field_name,
            } => {
                // Try to find the field's type in the constant pool
                if let Ok(class_name) = self.infer_receiver_class(object)
                    && let Ok(desc) = self.find_field_descriptor_in_pool(&class_name, field_name)
                    && let Ok(internal) = descriptor_to_internal(&desc)
                {
                    return Ok(internal);
                }
                Err(CompileError::CodegenError {
                    message: format!("cannot infer class for field access '{}'", field_name),
                })
            }
            CExpr::MethodCall {
                object, name, args, ..
            } => {
                // Try to infer from method return type in pool
                if let Some(obj) = object
                    && let Ok(_owner_class) = self.infer_receiver_class(obj)
                    && let Ok(desc) = self.find_method_descriptor_in_pool(name, args)
                {
                    // Extract return type from descriptor (everything after ')')
                    if let Some(ret_desc) = desc.rsplit(')').next()
                        && let Ok(internal) = descriptor_to_internal(ret_desc)
                    {
                        return Ok(internal);
                    }
                }
                Err(CompileError::CodegenError {
                    message: format!("cannot infer class for method call '{}'", name),
                })
            }
            CExpr::StringLiteral(_) => Ok("java/lang/String".into()),
            CExpr::NewObject { class_name, .. } => Ok(resolve_class_name(class_name)),
            _ => Err(CompileError::CodegenError {
                message: "cannot infer receiver class".into(),
            }),
        }
    }

    /// Resolve method descriptor. For well-known overloaded methods, infers from
    /// argument types. For others, searches the constant pool.
    fn find_method_descriptor_in_pool(
        &self,
        method_name: &str,
        args: &[CExpr],
    ) -> Result<String, CompileError> {
        // Well-known overloaded methods — infer from arg types first to avoid
        // picking the wrong overload from the pool
        match method_name {
            "println" => {
                return match args.len() {
                    0 => Ok("()V".into()),
                    1 => Ok(self.infer_println_descriptor(&args[0])),
                    _ => Ok("(Ljava/lang/String;)V".into()),
                };
            }
            "print" => {
                return match args.len() {
                    1 => Ok(self.infer_println_descriptor(&args[0])),
                    _ => Ok("(Ljava/lang/String;)V".into()),
                };
            }
            "append" if args.len() == 1 => return Ok(self.infer_append_descriptor(&args[0])),
            "toString" if args.is_empty() => return Ok("()Ljava/lang/String;".into()),
            "equals" if args.len() == 1 => return Ok("(Ljava/lang/Object;)Z".into()),
            "hashCode" if args.is_empty() => return Ok("()I".into()),
            "length" if args.is_empty() => return Ok("()I".into()),
            "charAt" if args.len() == 1 => return Ok("(I)C".into()),
            "substring" if args.len() == 1 => return Ok("(I)Ljava/lang/String;".into()),
            "substring" if args.len() == 2 => return Ok("(II)Ljava/lang/String;".into()),
            "valueOf" if args.len() == 1 => {
                return Ok("(Ljava/lang/Object;)Ljava/lang/String;".into());
            }
            "parseInt" if args.len() == 1 => return Ok("(Ljava/lang/String;)I".into()),
            "getClass" if args.is_empty() => return Ok("()Ljava/lang/Class;".into()),
            "getName" if args.is_empty() => return Ok("()Ljava/lang/String;".into()),
            "getSimpleName" if args.is_empty() => return Ok("()Ljava/lang/String;".into()),
            "getCanonicalName" if args.is_empty() => return Ok("()Ljava/lang/String;".into()),
            "getMessage" if args.is_empty() => return Ok("()Ljava/lang/String;".into()),
            "getCause" if args.is_empty() => return Ok("()Ljava/lang/Throwable;".into()),
            "getStackTrace" if args.is_empty() => {
                return Ok("()[Ljava/lang/StackTraceElement;".into());
            }
            "trim" if args.is_empty() => return Ok("()Ljava/lang/String;".into()),
            "toUpperCase" if args.is_empty() => return Ok("()Ljava/lang/String;".into()),
            "toLowerCase" if args.is_empty() => return Ok("()Ljava/lang/String;".into()),
            "intern" if args.is_empty() => return Ok("()Ljava/lang/String;".into()),
            "isEmpty" if args.is_empty() => return Ok("()Z".into()),
            "startsWith" if args.len() == 1 => return Ok("(Ljava/lang/String;)Z".into()),
            "endsWith" if args.len() == 1 => return Ok("(Ljava/lang/String;)Z".into()),
            "contains" if args.len() == 1 => return Ok("(Ljava/lang/CharSequence;)Z".into()),
            "replace" if args.len() == 2 => {
                return Ok(
                    "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Ljava/lang/String;".into(),
                );
            }
            "split" if args.len() == 1 => {
                return Ok("(Ljava/lang/String;)[Ljava/lang/String;".into());
            }
            "toCharArray" if args.is_empty() => return Ok("()[C".into()),
            "format" if !args.is_empty() => {
                return Ok(format!("(Ljava/lang/String;{}V", "[Ljava/lang/Object;)"));
            }
            _ => {}
        }

        // Search constant pool for matching method
        use crate::constant_info::ConstantInfo;
        let pool = &self.class_file.const_pool;

        for entry in pool.iter() {
            let nat_index = match entry {
                ConstantInfo::MethodRef(r) => r.name_and_type_index,
                ConstantInfo::InterfaceMethodRef(r) => r.name_and_type_index,
                _ => continue,
            };
            if let ConstantInfo::NameAndType(nat) = &pool[(nat_index - 1) as usize]
                && let Some(name) = self.class_file.get_utf8(nat.name_index)
                && name == method_name
                && let Some(desc) = self.class_file.get_utf8(nat.descriptor_index)
                && let Some((params, _)) = parse_method_descriptor(desc)
                && params.len() == args.len()
            {
                return Ok(desc.to_string());
            }
        }

        // Not found in pool — infer descriptor from argument types
        Ok(self.infer_method_descriptor(method_name, args))
    }

    fn infer_println_descriptor(&self, arg: &CExpr) -> String {
        match arg {
            CExpr::StringLiteral(_) => "(Ljava/lang/String;)V".into(),
            CExpr::IntLiteral(_) => "(I)V".into(),
            CExpr::LongLiteral(_) => "(J)V".into(),
            CExpr::FloatLiteral(_) => "(F)V".into(),
            CExpr::DoubleLiteral(_) => "(D)V".into(),
            CExpr::BoolLiteral(_) => "(Z)V".into(),
            CExpr::CharLiteral(_) => "(C)V".into(),
            CExpr::Ident(name) => {
                if let Some((_, ty)) = self.locals.find(name) {
                    match ty {
                        TypeName::Primitive(PrimitiveKind::Int)
                        | TypeName::Primitive(PrimitiveKind::Byte)
                        | TypeName::Primitive(PrimitiveKind::Short) => return "(I)V".into(),
                        TypeName::Primitive(PrimitiveKind::Long) => return "(J)V".into(),
                        TypeName::Primitive(PrimitiveKind::Float) => return "(F)V".into(),
                        TypeName::Primitive(PrimitiveKind::Double) => return "(D)V".into(),
                        TypeName::Primitive(PrimitiveKind::Boolean) => return "(Z)V".into(),
                        TypeName::Primitive(PrimitiveKind::Char) => return "(C)V".into(),
                        TypeName::Class(name) if name == "String" || name == "java.lang.String" => {
                            return "(Ljava/lang/String;)V".into();
                        }
                        _ => {}
                    }
                }
                "(Ljava/lang/Object;)V".into()
            }
            // For compound expressions, infer the result type
            _ => {
                let ty = self.infer_expr_type(arg);
                match &ty {
                    TypeName::Primitive(PrimitiveKind::Int)
                    | TypeName::Primitive(PrimitiveKind::Byte)
                    | TypeName::Primitive(PrimitiveKind::Short) => "(I)V".into(),
                    TypeName::Primitive(PrimitiveKind::Long) => "(J)V".into(),
                    TypeName::Primitive(PrimitiveKind::Float) => "(F)V".into(),
                    TypeName::Primitive(PrimitiveKind::Double) => "(D)V".into(),
                    TypeName::Primitive(PrimitiveKind::Boolean) => "(Z)V".into(),
                    TypeName::Primitive(PrimitiveKind::Char) => "(C)V".into(),
                    TypeName::Class(name)
                        if name == "String"
                            || name == "java.lang.String"
                            || name == "java/lang/String" =>
                    {
                        "(Ljava/lang/String;)V".into()
                    }
                    _ => "(Ljava/lang/Object;)V".into(),
                }
            }
        }
    }

    fn infer_append_descriptor(&self, arg: &CExpr) -> String {
        let ty = self.infer_expr_type(arg);
        match &ty {
            TypeName::Primitive(PrimitiveKind::Int)
            | TypeName::Primitive(PrimitiveKind::Byte)
            | TypeName::Primitive(PrimitiveKind::Short) => "(I)Ljava/lang/StringBuilder;".into(),
            TypeName::Primitive(PrimitiveKind::Long) => "(J)Ljava/lang/StringBuilder;".into(),
            TypeName::Primitive(PrimitiveKind::Float) => "(F)Ljava/lang/StringBuilder;".into(),
            TypeName::Primitive(PrimitiveKind::Double) => "(D)Ljava/lang/StringBuilder;".into(),
            TypeName::Primitive(PrimitiveKind::Boolean) => "(Z)Ljava/lang/StringBuilder;".into(),
            TypeName::Primitive(PrimitiveKind::Char) => "(C)Ljava/lang/StringBuilder;".into(),
            TypeName::Class(name)
                if name == "String" || name == "java.lang.String" || name == "java/lang/String" =>
            {
                "(Ljava/lang/String;)Ljava/lang/StringBuilder;".into()
            }
            _ => "(Ljava/lang/Object;)Ljava/lang/StringBuilder;".into(),
        }
    }

    fn infer_constructor_descriptor(&self, args: &[CExpr]) -> Result<String, CompileError> {
        if args.is_empty() {
            return Ok("()V".into());
        }
        // Build descriptor from arg types
        let mut desc = String::from("(");
        for arg in args {
            desc.push_str(&self.infer_arg_descriptor(arg));
        }
        desc.push_str(")V");
        Ok(desc)
    }

    fn infer_arg_descriptor(&self, arg: &CExpr) -> String {
        match arg {
            CExpr::StringLiteral(_) => "Ljava/lang/String;".into(),
            CExpr::IntLiteral(_) => "I".into(),
            CExpr::LongLiteral(_) => "J".into(),
            CExpr::FloatLiteral(_) => "F".into(),
            CExpr::DoubleLiteral(_) => "D".into(),
            CExpr::BoolLiteral(_) => "Z".into(),
            CExpr::CharLiteral(_) => "C".into(),
            CExpr::Ident(name) => {
                if let Some((_, ty)) = self.locals.find(name) {
                    return type_name_to_descriptor(ty);
                }
                "Ljava/lang/Object;".into()
            }
            _ => "Ljava/lang/Object;".into(),
        }
    }

    /// Infer a method descriptor from argument types and method name heuristics.
    /// Used as fallback when the method isn't found in the constant pool.
    fn infer_method_descriptor(&self, method_name: &str, args: &[CExpr]) -> String {
        // Well-known collection/generic methods with fixed descriptors (type-erased signatures)
        match (method_name, args.len()) {
            ("add", 1) => return "(Ljava/lang/Object;)Z".into(),
            ("add", 2) => return "(ILjava/lang/Object;)V".into(),
            ("get", 1) => return "(I)Ljava/lang/Object;".into(),
            ("set", 2) => return "(ILjava/lang/Object;)Ljava/lang/Object;".into(),
            ("remove", 1) => return "(Ljava/lang/Object;)Z".into(),
            ("contains", 1) => return "(Ljava/lang/Object;)Z".into(),
            ("put", 2) => return "(Ljava/lang/Object;Ljava/lang/Object;)Ljava/lang/Object;".into(),
            ("containsKey", 1) => return "(Ljava/lang/Object;)Z".into(),
            ("containsValue", 1) => return "(Ljava/lang/Object;)Z".into(),
            ("size", 0) => return "()I".into(),
            ("isEmpty", 0) => return "()Z".into(),
            ("clear", 0) => return "()V".into(),
            ("iterator", 0) => return "()Ljava/util/Iterator;".into(),
            ("hasNext", 0) => return "()Z".into(),
            ("next", 0) => return "()Ljava/lang/Object;".into(),
            ("offer", 1) => return "(Ljava/lang/Object;)Z".into(),
            _ => {}
        }

        // Generic fallback: infer from arg types
        let mut desc = String::from("(");
        for arg in args {
            desc.push_str(&self.infer_arg_descriptor(arg));
        }
        desc.push(')');

        // Heuristic return type based on well-known method names
        let ret = match method_name {
            "size" | "length" | "indexOf" | "lastIndexOf" | "compareTo" | "read" | "intValue"
            | "ordinal" | "hashCode" => "I",
            "isEmpty" | "contains" | "containsKey" | "containsValue" | "hasNext"
            | "hasPrevious" => "Z",
            "longValue" => "J",
            "floatValue" => "F",
            "doubleValue" => "D",
            "charAt" => "C",
            _ => "Ljava/lang/Object;",
        };
        desc.push_str(ret);
        desc
    }

    /// Check if the constant pool has a FieldRef for the given class and field.
    fn has_field_ref_for_class(&self, class_name: &str, field_name: &str) -> bool {
        use crate::constant_info::ConstantInfo;
        let pool = &self.class_file.const_pool;
        for entry in pool.iter() {
            if let ConstantInfo::FieldRef(r) = entry
                && let ConstantInfo::Class(c) = &pool[(r.class_index - 1) as usize]
                && let Some(cn) = self.class_file.get_utf8(c.name_index)
                && cn == class_name
                && let ConstantInfo::NameAndType(nat) = &pool[(r.name_and_type_index - 1) as usize]
                && let Some(name) = self.class_file.get_utf8(nat.name_index)
                && name == field_name
            {
                return true;
            }
        }
        false
    }

    /// Check if a class is a known interface or has InterfaceMethodRef entries in the pool.
    fn is_interface_method(&self, class_name: &str, method_name: &str) -> bool {
        // Well-known JDK interfaces
        const KNOWN_INTERFACES: &[&str] = &[
            "java/util/List",
            "java/util/Map",
            "java/util/Set",
            "java/util/Collection",
            "java/util/Iterator",
            "java/util/Iterable",
            "java/util/Enumeration",
            "java/util/Comparator",
            "java/util/Deque",
            "java/util/Queue",
            "java/util/SortedMap",
            "java/util/SortedSet",
            "java/util/NavigableMap",
            "java/util/NavigableSet",
            "java/util/concurrent/Callable",
            "java/util/concurrent/Future",
            "java/util/concurrent/BlockingQueue",
            "java/util/concurrent/BlockingDeque",
            "java/lang/Runnable",
            "java/lang/Comparable",
            "java/lang/CharSequence",
            "java/lang/Appendable",
            "java/lang/AutoCloseable",
            "java/lang/Closeable",
            "java/io/Closeable",
            "java/io/Serializable",
            "java/io/Flushable",
        ];
        if KNOWN_INTERFACES.contains(&class_name) {
            return true;
        }

        // Check the constant pool for InterfaceMethodRef entries matching this class+method
        use crate::constant_info::ConstantInfo;
        let pool = &self.class_file.const_pool;
        for entry in pool.iter() {
            if let ConstantInfo::InterfaceMethodRef(r) = entry
                && let ConstantInfo::Class(c) = &pool[(r.class_index - 1) as usize]
                && let Some(cn) = self.class_file.get_utf8(c.name_index)
                && cn == class_name
                && let ConstantInfo::NameAndType(nat) = &pool[(r.name_and_type_index - 1) as usize]
                && let Some(name) = self.class_file.get_utf8(nat.name_index)
                && name == method_name
            {
                return true;
            }
        }
        false
    }

    fn find_field_descriptor_in_pool(
        &self,
        class_name: &str,
        field_name: &str,
    ) -> Result<String, CompileError> {
        use crate::constant_info::ConstantInfo;
        let pool = &self.class_file.const_pool;

        for entry in pool.iter() {
            if let ConstantInfo::FieldRef(r) = entry {
                // Check class
                if let ConstantInfo::Class(c) = &pool[(r.class_index - 1) as usize]
                    && let Some(cn) = self.class_file.get_utf8(c.name_index)
                    && cn == class_name
                    && let ConstantInfo::NameAndType(nat) =
                        &pool[(r.name_and_type_index - 1) as usize]
                    && let Some(name) = self.class_file.get_utf8(nat.name_index)
                    && name == field_name
                    && let Some(desc) = self.class_file.get_utf8(nat.descriptor_index)
                {
                    return Ok(desc.to_string());
                }
            }
        }

        // Well-known fields
        match (class_name, field_name) {
            ("java/lang/System", "out") => Ok("Ljava/io/PrintStream;".into()),
            ("java/lang/System", "err") => Ok("Ljava/io/PrintStream;".into()),
            ("java/lang/System", "in") => Ok("Ljava/io/InputStream;".into()),
            ("java/lang/Boolean", "TRUE") => Ok("Ljava/lang/Boolean;".into()),
            ("java/lang/Boolean", "FALSE") => Ok("Ljava/lang/Boolean;".into()),
            _ => Err(CompileError::CodegenError {
                message: format!(
                    "cannot find field '{}.{}' in constant pool",
                    class_name, field_name
                ),
            }),
        }
    }
}

// --- Utility functions ---

fn resolve_label_addr(
    label_id: usize,
    labels: &[Option<usize>],
    addresses: &[u32],
    end_addr: i32,
) -> Result<i32, CompileError> {
    let target_instr = labels[label_id].ok_or_else(|| CompileError::CodegenError {
        message: format!("unresolved label {}", label_id),
    })?;
    if target_instr < addresses.len() {
        Ok(addresses[target_instr] as i32)
    } else {
        Ok(end_addr)
    }
}

/// Compute the byte offset at a specific instruction index.
fn compute_byte_offset_at(instructions: &[Instruction], target_idx: usize) -> u32 {
    let mut addr = 0u32;
    for (i, instr) in instructions.iter().enumerate() {
        if i == target_idx {
            return addr;
        }
        addr += instruction_byte_size(instr, addr);
    }
    addr // past-end offset
}

pub(crate) fn compute_byte_addresses(instructions: &[Instruction]) -> Vec<u32> {
    let mut addresses = Vec::with_capacity(instructions.len());
    let mut addr = 0u32;
    for instr in instructions {
        addresses.push(addr);
        addr += instruction_byte_size(instr, addr);
    }
    addresses
}

fn patch_branch_offset(instr: &Instruction, offset: i16) -> Result<Instruction, CompileError> {
    Ok(match instr {
        Instruction::Goto(_) => Instruction::Goto(offset),
        Instruction::Ifeq(_) => Instruction::Ifeq(offset),
        Instruction::Ifne(_) => Instruction::Ifne(offset),
        Instruction::Iflt(_) => Instruction::Iflt(offset),
        Instruction::Ifge(_) => Instruction::Ifge(offset),
        Instruction::Ifgt(_) => Instruction::Ifgt(offset),
        Instruction::Ifle(_) => Instruction::Ifle(offset),
        Instruction::IfIcmpeq(_) => Instruction::IfIcmpeq(offset),
        Instruction::IfIcmpne(_) => Instruction::IfIcmpne(offset),
        Instruction::IfIcmplt(_) => Instruction::IfIcmplt(offset),
        Instruction::IfIcmpge(_) => Instruction::IfIcmpge(offset),
        Instruction::IfIcmpgt(_) => Instruction::IfIcmpgt(offset),
        Instruction::IfIcmple(_) => Instruction::IfIcmple(offset),
        Instruction::IfAcmpeq(_) => Instruction::IfAcmpeq(offset),
        Instruction::IfAcmpne(_) => Instruction::IfAcmpne(offset),
        Instruction::Ifnull(_) => Instruction::Ifnull(offset),
        Instruction::Ifnonnull(_) => Instruction::Ifnonnull(offset),
        _ => {
            return Err(CompileError::CodegenError {
                message: format!("cannot patch branch offset on {:?}", instr),
            });
        }
    })
}

/// Resolve a simple or dotted class name to JVM internal form.
/// Erase a JvmType to its Object-erased descriptor for SAM type descriptors.
/// Reference types and arrays become `Ljava/lang/Object;`, primitives stay as-is.
fn erase_to_object(ty: &JvmType) -> String {
    match ty {
        JvmType::Reference(_) | JvmType::Array(_) | JvmType::Null | JvmType::Unknown => {
            "Ljava/lang/Object;".into()
        }
        other => other.to_descriptor(),
    }
}

pub fn resolve_class_name(name: &str) -> String {
    // Well-known short names
    match name {
        "String" => return "java/lang/String".into(),
        "Object" => return "java/lang/Object".into(),
        "System" => return "java/lang/System".into(),
        "Integer" => return "java/lang/Integer".into(),
        "Long" => return "java/lang/Long".into(),
        "Float" => return "java/lang/Float".into(),
        "Double" => return "java/lang/Double".into(),
        "Boolean" => return "java/lang/Boolean".into(),
        "Byte" => return "java/lang/Byte".into(),
        "Character" => return "java/lang/Character".into(),
        "Short" => return "java/lang/Short".into(),
        "Math" => return "java/lang/Math".into(),
        "StringBuilder" => return "java/lang/StringBuilder".into(),
        "StringBuffer" => return "java/lang/StringBuffer".into(),
        "PrintStream" => return "java/io/PrintStream".into(),
        "InputStream" => return "java/io/InputStream".into(),
        "Exception" => return "java/lang/Exception".into(),
        "RuntimeException" => return "java/lang/RuntimeException".into(),
        "NullPointerException" => return "java/lang/NullPointerException".into(),
        "IllegalArgumentException" => return "java/lang/IllegalArgumentException".into(),
        "IllegalStateException" => return "java/lang/IllegalStateException".into(),
        "UnsupportedOperationException" => return "java/lang/UnsupportedOperationException".into(),
        "IndexOutOfBoundsException" => return "java/lang/IndexOutOfBoundsException".into(),
        "ArrayList" => return "java/util/ArrayList".into(),
        "HashMap" => return "java/util/HashMap".into(),
        "List" => return "java/util/List".into(),
        "Map" => return "java/util/Map".into(),
        "Set" => return "java/util/Set".into(),
        "Arrays" => return "java/util/Arrays".into(),
        "Collections" => return "java/util/Collections".into(),
        "Class" => return "java/lang/Class".into(),
        _ => {}
    }
    // Convert dotted name to internal form
    name.replace('.', "/")
}

fn type_name_to_descriptor(ty: &TypeName) -> String {
    match ty {
        TypeName::Primitive(kind) => match kind {
            PrimitiveKind::Int => "I".into(),
            PrimitiveKind::Long => "J".into(),
            PrimitiveKind::Float => "F".into(),
            PrimitiveKind::Double => "D".into(),
            PrimitiveKind::Boolean => "Z".into(),
            PrimitiveKind::Byte => "B".into(),
            PrimitiveKind::Char => "C".into(),
            PrimitiveKind::Short => "S".into(),
            PrimitiveKind::Void => "V".into(),
        },
        TypeName::Class(name) => {
            let internal = resolve_class_name(name);
            format!("L{};", internal)
        }
        TypeName::Array(inner) => {
            format!("[{}", type_name_to_descriptor(inner))
        }
    }
}

fn descriptor_to_internal(desc: &str) -> Result<String, CompileError> {
    if desc.starts_with('L') && desc.ends_with(';') {
        Ok(desc[1..desc.len() - 1].to_string())
    } else {
        Err(CompileError::CodegenError {
            message: format!(
                "cannot convert descriptor '{}' to internal class name",
                desc
            ),
        })
    }
}

/// Convert a field descriptor string to a TypeName.
fn descriptor_to_type(desc: &str) -> TypeName {
    match desc {
        "I" => TypeName::Primitive(PrimitiveKind::Int),
        "J" => TypeName::Primitive(PrimitiveKind::Long),
        "F" => TypeName::Primitive(PrimitiveKind::Float),
        "D" => TypeName::Primitive(PrimitiveKind::Double),
        "Z" => TypeName::Primitive(PrimitiveKind::Boolean),
        "B" => TypeName::Primitive(PrimitiveKind::Byte),
        "C" => TypeName::Primitive(PrimitiveKind::Char),
        "S" => TypeName::Primitive(PrimitiveKind::Short),
        "V" => TypeName::Primitive(PrimitiveKind::Void),
        _ if desc.starts_with('L') && desc.ends_with(';') => {
            TypeName::Class(desc[1..desc.len() - 1].to_string())
        }
        _ if desc.starts_with('[') => TypeName::Array(Box::new(descriptor_to_type(&desc[1..]))),
        _ => TypeName::Class("java/lang/Object".into()),
    }
}

fn jvm_type_to_type_name(jvm_ty: &JvmType) -> TypeName {
    match jvm_ty {
        JvmType::Int => TypeName::Primitive(PrimitiveKind::Int),
        JvmType::Long => TypeName::Primitive(PrimitiveKind::Long),
        JvmType::Float => TypeName::Primitive(PrimitiveKind::Float),
        JvmType::Double => TypeName::Primitive(PrimitiveKind::Double),
        JvmType::Boolean => TypeName::Primitive(PrimitiveKind::Boolean),
        JvmType::Byte => TypeName::Primitive(PrimitiveKind::Byte),
        JvmType::Char => TypeName::Primitive(PrimitiveKind::Char),
        JvmType::Short => TypeName::Primitive(PrimitiveKind::Short),
        JvmType::Void => TypeName::Primitive(PrimitiveKind::Void),
        JvmType::Reference(name) => TypeName::Class(name.clone()),
        JvmType::Array(inner) => TypeName::Array(Box::new(jvm_type_to_type_name(inner))),
        JvmType::Null | JvmType::Unknown => TypeName::Class("java/lang/Object".into()),
    }
}

fn is_int_type(ty: &TypeName) -> bool {
    matches!(
        ty,
        TypeName::Primitive(
            PrimitiveKind::Int
                | PrimitiveKind::Boolean
                | PrimitiveKind::Byte
                | PrimitiveKind::Char
                | PrimitiveKind::Short
        )
    )
}

fn is_long_type(ty: &TypeName) -> bool {
    matches!(ty, TypeName::Primitive(PrimitiveKind::Long))
}

fn is_float_type(ty: &TypeName) -> bool {
    matches!(ty, TypeName::Primitive(PrimitiveKind::Float))
}

fn is_double_type(ty: &TypeName) -> bool {
    matches!(ty, TypeName::Primitive(PrimitiveKind::Double))
}

fn is_reference_type(ty: &TypeName) -> bool {
    matches!(ty, TypeName::Class(_) | TypeName::Array(_))
}

fn type_slot_width(ty: &TypeName) -> u16 {
    match ty {
        TypeName::Primitive(PrimitiveKind::Long | PrimitiveKind::Double) => 2,
        _ => 1,
    }
}

fn type_name_to_vtype(ty: &TypeName) -> VType {
    match ty {
        TypeName::Primitive(kind) => match kind {
            PrimitiveKind::Int
            | PrimitiveKind::Boolean
            | PrimitiveKind::Byte
            | PrimitiveKind::Char
            | PrimitiveKind::Short => VType::Integer,
            PrimitiveKind::Long => VType::Long,
            PrimitiveKind::Float => VType::Float,
            PrimitiveKind::Double => VType::Double,
            PrimitiveKind::Void => VType::Top,
        },
        TypeName::Class(_) | TypeName::Array(_) => {
            // We'd need a class_file reference to resolve the cp index.
            // Use a sentinel — this will be resolved later or use Null as fallback.
            VType::Null
        }
    }
}

/// Resolve a TypeName to a VType, using the class file to get or create constant pool entries.
fn type_name_to_vtype_resolved(ty: &TypeName, class_file: &mut ClassFile) -> VType {
    match ty {
        TypeName::Primitive(kind) => match kind {
            PrimitiveKind::Int
            | PrimitiveKind::Boolean
            | PrimitiveKind::Byte
            | PrimitiveKind::Char
            | PrimitiveKind::Short => VType::Integer,
            PrimitiveKind::Long => VType::Long,
            PrimitiveKind::Float => VType::Float,
            PrimitiveKind::Double => VType::Double,
            PrimitiveKind::Void => VType::Top,
        },
        TypeName::Class(name) => {
            let internal = resolve_class_name(name);
            let idx = class_file.get_or_add_class(&internal);
            VType::Object(idx)
        }
        TypeName::Array(_) => {
            let desc = type_name_to_descriptor(ty);
            let idx = class_file.get_or_add_class(&desc);
            VType::Object(idx)
        }
    }
}

/// Resolve a JvmType to a VType, using the class file to get or create constant pool entries.
fn jvm_type_to_vtype_resolved(jvm_ty: &JvmType, class_file: &mut ClassFile) -> VType {
    match jvm_ty {
        JvmType::Int | JvmType::Boolean | JvmType::Byte | JvmType::Char | JvmType::Short => {
            VType::Integer
        }
        JvmType::Long => VType::Long,
        JvmType::Float => VType::Float,
        JvmType::Double => VType::Double,
        JvmType::Void => VType::Top,
        JvmType::Reference(name) => {
            let idx = class_file.get_or_add_class(name);
            VType::Object(idx)
        }
        JvmType::Array(_) => {
            let ty = jvm_type_to_type_name(jvm_ty);
            let desc = type_name_to_descriptor(&ty);
            let idx = class_file.get_or_add_class(&desc);
            VType::Object(idx)
        }
        JvmType::Null | JvmType::Unknown => VType::Null,
    }
}

fn is_var_sentinel(ty: &TypeName) -> bool {
    matches!(ty, TypeName::Class(name) if name == "__var__")
}

fn is_string_type(ty: &TypeName) -> bool {
    matches!(ty, TypeName::Class(name) if name == "String" || name == "java.lang.String" || name == "java/lang/String")
}

/// Java numeric widening rank: Int(0) < Long(1) < Float(2) < Double(3).
fn numeric_rank(ty: &TypeName) -> u8 {
    match ty {
        TypeName::Primitive(PrimitiveKind::Double) => 3,
        TypeName::Primitive(PrimitiveKind::Float) => 2,
        TypeName::Primitive(PrimitiveKind::Long) => 1,
        _ => 0, // int and sub-int types
    }
}

/// Compute the `count` argument for invokeinterface: 1 (objectref) + sum of arg slot widths.
fn compute_invokeinterface_count(descriptor: &str) -> u8 {
    let (params, _) = parse_method_descriptor(descriptor).unwrap_or((Vec::new(), JvmType::Void));
    let mut count: u8 = 1; // objectref
    for param in &params {
        count += if param.is_wide() { 2 } else { 1 };
    }
    count
}

/// Return the wider of two numeric types per Java widening rules.
fn promote_numeric_type(a: &TypeName, b: &TypeName) -> TypeName {
    if numeric_rank(a) >= numeric_rank(b) {
        a.clone()
    } else {
        b.clone()
    }
}
