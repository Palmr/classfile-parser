use crate::code_attribute::Instruction;

/// Compute max_stack by walking instructions and tracking stack depth.
pub fn compute_max_stack(instructions: &[Instruction]) -> u16 {
    let mut depth: i32 = 0;
    let mut max_depth: i32 = 0;

    for instr in instructions {
        depth += stack_delta(instr);
        if depth > max_depth {
            max_depth = depth;
        }
        // Clamp to prevent underflow from unreachable code
        if depth < 0 {
            depth = 0;
        }
    }

    // Safety margin of +2: the linear walk doesn't model control flow, so it can
    // underestimate the stack at branch merge points. For example, an exception handler
    // pushes one value (the exception) that the linear walk doesn't see, and certain
    // patterns (dup + method call) can temporarily exceed the tracked depth. The +2
    // covers these cases conservatively without requiring a full CFG analysis.
    let result = max_depth + 2;
    result.max(1) as u16
}

/// Returns the net stack depth change for an instruction.
fn stack_delta(instr: &Instruction) -> i32 {
    match instr {
        // Constants: push 1
        Instruction::Aconstnull
        | Instruction::Iconstm1
        | Instruction::Iconst0
        | Instruction::Iconst1
        | Instruction::Iconst2
        | Instruction::Iconst3
        | Instruction::Iconst4
        | Instruction::Iconst5
        | Instruction::Fconst0
        | Instruction::Fconst1
        | Instruction::Fconst2
        | Instruction::Bipush(_)
        | Instruction::Sipush(_)
        | Instruction::Ldc(_)
        | Instruction::LdcW(_) => 1,

        // Long/Double constants: push 2 (but treated as 1 category-2 value conceptually)
        // JVM spec: long/double use 2 stack slots
        Instruction::Lconst0
        | Instruction::Lconst1
        | Instruction::Dconst0
        | Instruction::Dconst1 => 2,
        Instruction::Ldc2W(_) => 2,

        // Loads: push 1 (or 2 for long/double)
        Instruction::Iload(_)
        | Instruction::Iload0
        | Instruction::Iload1
        | Instruction::Iload2
        | Instruction::Iload3
        | Instruction::Fload(_)
        | Instruction::Fload0
        | Instruction::Fload1
        | Instruction::Fload2
        | Instruction::Fload3
        | Instruction::Aload(_)
        | Instruction::Aload0
        | Instruction::Aload1
        | Instruction::Aload2
        | Instruction::Aload3
        | Instruction::IloadWide(_)
        | Instruction::FloadWide(_)
        | Instruction::AloadWide(_) => 1,

        Instruction::Lload(_)
        | Instruction::Lload0
        | Instruction::Lload1
        | Instruction::Lload2
        | Instruction::Lload3
        | Instruction::Dload(_)
        | Instruction::Dload0
        | Instruction::Dload1
        | Instruction::Dload2
        | Instruction::Dload3
        | Instruction::LloadWide(_)
        | Instruction::DloadWide(_) => 2,

        // Array loads: pop 2 (arrayref + index), push 1 (or 2)
        Instruction::Iaload
        | Instruction::Faload
        | Instruction::Aaload
        | Instruction::Baload
        | Instruction::Caload
        | Instruction::Saload => -1, // -2 + 1

        Instruction::Laload | Instruction::Daload => 0, // -2 + 2

        // Stores: pop 1 (or 2 for long/double)
        Instruction::Istore(_)
        | Instruction::Istore0
        | Instruction::Istore1
        | Instruction::Istore2
        | Instruction::Istore3
        | Instruction::Fstore(_)
        | Instruction::Fstore0
        | Instruction::Fstore1
        | Instruction::Fstore2
        | Instruction::Fstore3
        | Instruction::Astore(_)
        | Instruction::Astore0
        | Instruction::Astore1
        | Instruction::Astore2
        | Instruction::Astore3
        | Instruction::IstoreWide(_)
        | Instruction::FstoreWide(_)
        | Instruction::AstoreWide(_) => -1,

        Instruction::Lstore(_)
        | Instruction::Lstore0
        | Instruction::Lstore1
        | Instruction::Lstore2
        | Instruction::Lstore3
        | Instruction::Dstore(_)
        | Instruction::Dstore0
        | Instruction::Dstore1
        | Instruction::Dstore2
        | Instruction::Dstore3
        | Instruction::LstoreWide(_)
        | Instruction::DstoreWide(_) => -2,

        // Array stores: pop 3 (arrayref + index + value)
        Instruction::Iastore
        | Instruction::Fastore
        | Instruction::Aastore
        | Instruction::Bastore
        | Instruction::Castore
        | Instruction::Sastore => -3,

        Instruction::Lastore | Instruction::Dastore => -4, // pop arrayref + index + long/double

        // Stack manipulation
        Instruction::Pop => -1,
        Instruction::Pop2 => -2,
        Instruction::Dup => 1,
        Instruction::Dupx1 => 1,
        Instruction::Dupx2 => 1,
        Instruction::Dup2 => 2,
        Instruction::Dup2x1 => 2,
        Instruction::Dup2x2 => 2,
        Instruction::Swap => 0,

        // Arithmetic: pop 2, push 1 (net -1 for int/float)
        Instruction::Iadd
        | Instruction::Isub
        | Instruction::Imul
        | Instruction::Idiv
        | Instruction::Irem
        | Instruction::Ishl
        | Instruction::Ishr
        | Instruction::Iushr
        | Instruction::Iand
        | Instruction::Ior
        | Instruction::Ixor
        | Instruction::Fadd
        | Instruction::Fsub
        | Instruction::Fmul
        | Instruction::Fdiv
        | Instruction::Frem => -1,

        // Long/double arithmetic: pop 4, push 2 (net -2)
        Instruction::Ladd
        | Instruction::Lsub
        | Instruction::Lmul
        | Instruction::Ldiv
        | Instruction::Lrem
        | Instruction::Land
        | Instruction::Lor
        | Instruction::Lxor
        | Instruction::Dadd
        | Instruction::Dsub
        | Instruction::Dmul
        | Instruction::Ddiv
        | Instruction::Drem => -2,

        // Long shift: pop long(2) + int(1), push long(2) = -1
        Instruction::Lshl | Instruction::Lshr | Instruction::Lushr => -1,

        // Negate: pop 1, push 1 = 0
        Instruction::Ineg | Instruction::Fneg => 0,
        Instruction::Lneg | Instruction::Dneg => 0,

        // Iinc doesn't touch the stack
        Instruction::Iinc { .. } | Instruction::IincWide { .. } => 0,

        // Conversions: same stack effect as source and target sizes
        Instruction::I2l | Instruction::I2d | Instruction::F2l | Instruction::F2d => 1, // push extra slot
        Instruction::L2i | Instruction::L2f | Instruction::D2i | Instruction::D2f => -1, // lose a slot
        Instruction::I2f
        | Instruction::I2b
        | Instruction::I2c
        | Instruction::I2s
        | Instruction::F2i => 0,
        Instruction::L2d | Instruction::D2l => 0, // 2 -> 2

        // Comparisons
        Instruction::Lcmp => -3, // pop 2 longs (4 slots), push int (1) = -3
        Instruction::Fcmpl | Instruction::Fcmpg => -1, // pop 2, push 1
        Instruction::Dcmpl | Instruction::Dcmpg => -3, // pop 2 doubles (4 slots), push int

        // Branches: pop operand(s), no push
        Instruction::Ifeq(_)
        | Instruction::Ifne(_)
        | Instruction::Iflt(_)
        | Instruction::Ifge(_)
        | Instruction::Ifgt(_)
        | Instruction::Ifle(_)
        | Instruction::Ifnull(_)
        | Instruction::Ifnonnull(_) => -1,

        Instruction::IfIcmpeq(_)
        | Instruction::IfIcmpne(_)
        | Instruction::IfIcmplt(_)
        | Instruction::IfIcmpge(_)
        | Instruction::IfIcmpgt(_)
        | Instruction::IfIcmple(_)
        | Instruction::IfAcmpeq(_)
        | Instruction::IfAcmpne(_) => -2,

        Instruction::Goto(_) | Instruction::GotoW(_) => 0,

        // Returns
        Instruction::Return => 0,
        Instruction::Ireturn | Instruction::Freturn | Instruction::Areturn => -1,
        Instruction::Lreturn | Instruction::Dreturn => -2,

        // Field access
        Instruction::Getstatic(_) => 1,  // push value
        Instruction::Putstatic(_) => -1, // pop value
        Instruction::Getfield(_) => 0,   // pop objectref, push value
        Instruction::Putfield(_) => -2,  // pop objectref + value

        // Method invocations: complex, approximate conservatively
        // For MVP, assume methods consume args and push at most 1
        Instruction::Invokevirtual(_) | Instruction::Invokespecial(_) => {
            // Pops objectref + args, pushes return. Approximate: -1 (net for void methods with objectref)
            // This is approximate; the actual delta depends on the method descriptor.
            -1
        }
        Instruction::Invokestatic(_) => {
            // Pops args, pushes return. Approximate: 0
            0
        }
        Instruction::Invokeinterface { .. } => -1,
        Instruction::Invokedynamic { .. } => 0,

        // Object creation
        Instruction::New(_) => 1,
        Instruction::Newarray(_) => 0, // pop count, push arrayref
        Instruction::Anewarray(_) => 0,
        Instruction::Arraylength => 0, // pop arrayref, push length

        Instruction::Athrow => -1,
        Instruction::Checkcast(_) => 0,
        Instruction::Instanceof(_) => 0, // pop ref, push int

        Instruction::Monitorenter | Instruction::Monitorexit => -1,

        Instruction::Multianewarray { dimensions, .. } => {
            1 - (*dimensions as i32) // pop N counts, push arrayref
        }

        // Switch
        Instruction::Tableswitch { .. } | Instruction::Lookupswitch { .. } => -1,

        // JSR/RET (legacy)
        Instruction::Jsr(_) | Instruction::JsrW(_) => 1,
        Instruction::Ret(_) | Instruction::RetWide(_) => 0,

        // Nop
        Instruction::Nop => 0,
    }
}
