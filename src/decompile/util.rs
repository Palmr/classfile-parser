use crate::code_attribute::Instruction;
use crate::constant_info::ConstantInfo;

/// Returns the byte size of an instruction in the code array.
/// `address` is the bytecode offset of this instruction (needed for switch alignment).
pub fn instruction_byte_size(instr: &Instruction, address: u32) -> u32 {
    match instr {
        Instruction::Nop => 1,
        Instruction::Aconstnull => 1,
        Instruction::Iconstm1
        | Instruction::Iconst0
        | Instruction::Iconst1
        | Instruction::Iconst2
        | Instruction::Iconst3
        | Instruction::Iconst4
        | Instruction::Iconst5 => 1,
        Instruction::Lconst0 | Instruction::Lconst1 => 1,
        Instruction::Fconst0 | Instruction::Fconst1 | Instruction::Fconst2 => 1,
        Instruction::Dconst0 | Instruction::Dconst1 => 1,
        Instruction::Bipush(_) => 2,
        Instruction::Sipush(_) => 3,
        Instruction::Ldc(_) => 2,
        Instruction::LdcW(_) => 3,
        Instruction::Ldc2W(_) => 3,
        Instruction::Iload(_)
        | Instruction::Lload(_)
        | Instruction::Fload(_)
        | Instruction::Dload(_)
        | Instruction::Aload(_) => 2,
        Instruction::Iload0 | Instruction::Iload1 | Instruction::Iload2 | Instruction::Iload3 => 1,
        Instruction::Lload0 | Instruction::Lload1 | Instruction::Lload2 | Instruction::Lload3 => 1,
        Instruction::Fload0 | Instruction::Fload1 | Instruction::Fload2 | Instruction::Fload3 => 1,
        Instruction::Dload0 | Instruction::Dload1 | Instruction::Dload2 | Instruction::Dload3 => 1,
        Instruction::Aload0 | Instruction::Aload1 | Instruction::Aload2 | Instruction::Aload3 => 1,
        Instruction::Iaload
        | Instruction::Laload
        | Instruction::Faload
        | Instruction::Daload
        | Instruction::Aaload
        | Instruction::Baload
        | Instruction::Caload
        | Instruction::Saload => 1,
        Instruction::Istore(_)
        | Instruction::Lstore(_)
        | Instruction::Fstore(_)
        | Instruction::Dstore(_)
        | Instruction::Astore(_) => 2,
        Instruction::Istore0
        | Instruction::Istore1
        | Instruction::Istore2
        | Instruction::Istore3 => 1,
        Instruction::Lstore0
        | Instruction::Lstore1
        | Instruction::Lstore2
        | Instruction::Lstore3 => 1,
        Instruction::Fstore0
        | Instruction::Fstore1
        | Instruction::Fstore2
        | Instruction::Fstore3 => 1,
        Instruction::Dstore0
        | Instruction::Dstore1
        | Instruction::Dstore2
        | Instruction::Dstore3 => 1,
        Instruction::Astore0
        | Instruction::Astore1
        | Instruction::Astore2
        | Instruction::Astore3 => 1,
        Instruction::Iastore
        | Instruction::Lastore
        | Instruction::Fastore
        | Instruction::Dastore
        | Instruction::Aastore
        | Instruction::Bastore
        | Instruction::Castore
        | Instruction::Sastore => 1,
        Instruction::Pop => 1,
        Instruction::Pop2 => 1,
        Instruction::Dup => 1,
        Instruction::Dupx1 => 1,
        Instruction::Dupx2 => 1,
        Instruction::Dup2 => 1,
        Instruction::Dup2x1 => 1,
        Instruction::Dup2x2 => 1,
        Instruction::Swap => 1,
        Instruction::Iadd | Instruction::Ladd | Instruction::Fadd | Instruction::Dadd => 1,
        Instruction::Isub | Instruction::Lsub | Instruction::Fsub | Instruction::Dsub => 1,
        Instruction::Imul | Instruction::Lmul | Instruction::Fmul | Instruction::Dmul => 1,
        Instruction::Idiv | Instruction::Ldiv | Instruction::Fdiv | Instruction::Ddiv => 1,
        Instruction::Irem | Instruction::Lrem | Instruction::Frem | Instruction::Drem => 1,
        Instruction::Ineg | Instruction::Lneg | Instruction::Fneg | Instruction::Dneg => 1,
        Instruction::Ishl | Instruction::Lshl => 1,
        Instruction::Ishr | Instruction::Lshr => 1,
        Instruction::Iushr | Instruction::Lushr => 1,
        Instruction::Iand | Instruction::Land => 1,
        Instruction::Ior | Instruction::Lor => 1,
        Instruction::Ixor | Instruction::Lxor => 1,
        Instruction::Iinc { .. } => 3,
        Instruction::I2l | Instruction::I2f | Instruction::I2d => 1,
        Instruction::L2i | Instruction::L2f | Instruction::L2d => 1,
        Instruction::F2i | Instruction::F2l | Instruction::F2d => 1,
        Instruction::D2i | Instruction::D2l | Instruction::D2f => 1,
        Instruction::I2b | Instruction::I2c | Instruction::I2s => 1,
        Instruction::Lcmp => 1,
        Instruction::Fcmpl | Instruction::Fcmpg => 1,
        Instruction::Dcmpl | Instruction::Dcmpg => 1,
        Instruction::Ifeq(_)
        | Instruction::Ifne(_)
        | Instruction::Iflt(_)
        | Instruction::Ifge(_)
        | Instruction::Ifgt(_)
        | Instruction::Ifle(_) => 3,
        Instruction::IfIcmpeq(_)
        | Instruction::IfIcmpne(_)
        | Instruction::IfIcmplt(_)
        | Instruction::IfIcmpge(_)
        | Instruction::IfIcmpgt(_)
        | Instruction::IfIcmple(_) => 3,
        Instruction::IfAcmpeq(_) | Instruction::IfAcmpne(_) => 3,
        Instruction::Goto(_) => 3,
        Instruction::Jsr(_) => 3,
        Instruction::Ret(_) => 2,
        Instruction::Tableswitch { low, high, .. } => {
            let padding = (4 - (address + 1) % 4) % 4;
            // 1 (opcode) + padding + 4 (default) + 4 (low) + 4 (high) + 4*(high-low+1)
            1 + padding + 4 + 4 + 4 + 4 * (high - low + 1) as u32
        }
        Instruction::Lookupswitch { npairs, .. } => {
            let padding = (4 - (address + 1) % 4) % 4;
            // 1 (opcode) + padding + 4 (default) + 4 (npairs) + 8*npairs
            1 + padding + 4 + 4 + 8 * npairs
        }
        Instruction::Getstatic(_)
        | Instruction::Putstatic(_)
        | Instruction::Getfield(_)
        | Instruction::Putfield(_) => 3,
        Instruction::Invokevirtual(_)
        | Instruction::Invokespecial(_)
        | Instruction::Invokestatic(_) => 3,
        Instruction::Invokeinterface { .. } => 5,
        Instruction::Invokedynamic { .. } => 5,
        Instruction::New(_) => 3,
        Instruction::Newarray(_) => 2,
        Instruction::Anewarray(_) => 3,
        Instruction::Arraylength => 1,
        Instruction::Athrow => 1,
        Instruction::Checkcast(_) => 3,
        Instruction::Instanceof(_) => 3,
        Instruction::Monitorenter | Instruction::Monitorexit => 1,
        Instruction::Multianewarray { .. } => 4,
        Instruction::Ifnull(_) | Instruction::Ifnonnull(_) => 3,
        Instruction::GotoW(_) => 5,
        Instruction::JsrW(_) => 5,
        Instruction::Areturn
        | Instruction::Ireturn
        | Instruction::Lreturn
        | Instruction::Freturn
        | Instruction::Dreturn
        | Instruction::Return => 1,
        // wide instructions: 2 bytes magic + 2 bytes index
        Instruction::IloadWide(_)
        | Instruction::LloadWide(_)
        | Instruction::FloadWide(_)
        | Instruction::DloadWide(_)
        | Instruction::AloadWide(_) => 4,
        Instruction::IstoreWide(_)
        | Instruction::LstoreWide(_)
        | Instruction::FstoreWide(_)
        | Instruction::DstoreWide(_)
        | Instruction::AstoreWide(_) => 4,
        Instruction::RetWide(_) => 4,
        Instruction::IincWide { .. } => 6,
    }
}

/// Compute byte addresses for each instruction in a code array.
/// Returns Vec<(address, &Instruction)>.
pub fn compute_addresses(code: &[Instruction]) -> Vec<(u32, &Instruction)> {
    let mut result = Vec::with_capacity(code.len());
    let mut address = 0u32;
    for instr in code {
        result.push((address, instr));
        address += instruction_byte_size(instr, address);
    }
    result
}

/// Look up a UTF-8 constant pool entry by 1-based index.
pub fn get_utf8(const_pool: &[ConstantInfo], index: u16) -> Option<&str> {
    match const_pool.get((index as usize).checked_sub(1)?)? {
        ConstantInfo::Utf8(u) => Some(&u.utf8_string),
        _ => None,
    }
}

/// Resolve a Class constant to its name string.
pub fn get_class_name(const_pool: &[ConstantInfo], class_index: u16) -> Option<&str> {
    match const_pool.get((class_index as usize).checked_sub(1)?)? {
        ConstantInfo::Class(c) => get_utf8(const_pool, c.name_index),
        _ => None,
    }
}

/// Resolve a NameAndType constant to (name, descriptor).
pub fn get_name_and_type(const_pool: &[ConstantInfo], nat_index: u16) -> Option<(&str, &str)> {
    match const_pool.get((nat_index as usize).checked_sub(1)?)? {
        ConstantInfo::NameAndType(nat) => {
            let name = get_utf8(const_pool, nat.name_index)?;
            let desc = get_utf8(const_pool, nat.descriptor_index)?;
            Some((name, desc))
        }
        _ => None,
    }
}

/// Resolve a FieldRef, MethodRef, or InterfaceMethodRef to (class_name, method_name, descriptor).
pub fn resolve_ref(const_pool: &[ConstantInfo], index: u16) -> Option<(&str, &str, &str)> {
    let entry = const_pool.get((index as usize).checked_sub(1)?)?;
    let (class_index, nat_index) = match entry {
        ConstantInfo::FieldRef(r) => (r.class_index, r.name_and_type_index),
        ConstantInfo::MethodRef(r) => (r.class_index, r.name_and_type_index),
        ConstantInfo::InterfaceMethodRef(r) => (r.class_index, r.name_and_type_index),
        _ => return None,
    };
    let class_name = get_class_name(const_pool, class_index)?;
    let (name, desc) = get_name_and_type(const_pool, nat_index)?;
    Some((class_name, name, desc))
}

/// Get a constant pool entry's value as a string for display.
pub fn format_constant(const_pool: &[ConstantInfo], index: u16) -> String {
    match const_pool.get((index as usize).wrapping_sub(1)) {
        Some(ConstantInfo::Integer(c)) => format!("{}", c.value),
        Some(ConstantInfo::Float(c)) => format!("{}f", c.value),
        Some(ConstantInfo::Long(c)) => format!("{}L", c.value),
        Some(ConstantInfo::Double(c)) => format!("{}d", c.value),
        Some(ConstantInfo::String(c)) => {
            if let Some(s) = get_utf8(const_pool, c.string_index) {
                format!("\"{}\"", s)
            } else {
                format!("<string #{}>", c.string_index)
            }
        }
        Some(ConstantInfo::Class(c)) => {
            if let Some(name) = get_utf8(const_pool, c.name_index) {
                name.to_string()
            } else {
                format!("<class #{}>", c.name_index)
            }
        }
        Some(ConstantInfo::Utf8(c)) => c.utf8_string.clone(),
        _ => format!("<cp #{}>", index),
    }
}
