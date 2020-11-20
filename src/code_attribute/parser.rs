use code_attribute::types::Instruction;
use nom::{be_i16, be_i32, be_i8, be_u16, be_u32, be_u8, IResult, Offset};

fn offset<'a>(remaining: &'a [u8], input: &[u8]) -> IResult<&'a [u8], usize> {
    Ok((remaining, input.offset(remaining)))
}

fn align(input: &[u8], address: usize) -> IResult<&[u8], &[u8]> {
    take!(input, (4 - address % 4) % 4)
}

fn lookupswitch_parser(input: &[u8]) -> IResult<&[u8], Instruction> {
    do_parse!(
        input,
        default: be_i32
            >> npairs: be_u32
            >> pairs:
                count!(
                    do_parse!(lookup: be_i32 >> offset: be_i32 >> (lookup, offset)),
                    npairs as usize
                )
            >> (Instruction::Lookupswitch { default, pairs })
    )
}

fn tableswitch_parser(input: &[u8]) -> IResult<&[u8], Instruction> {
    do_parse!(
        input,
        default: be_i32
            >> low: be_i32
            >> high: be_i32
            >> offsets: count!(be_i32, (high - low + 1) as usize)
            >> (Instruction::Tableswitch {
                default,
                low,
                high,
                offsets,
            })
    )
}

pub fn code_parser(input: &[u8]) -> IResult<&[u8], Vec<(usize, Instruction)>> {
    many0!(
        input,
        complete!(do_parse!(
            address: apply!(offset, input)
                >> instruction: apply!(instruction_parser, address)
                >> (address, instruction)
        ))
    )
}

pub fn instruction_parser(input: &[u8], address: usize) -> IResult<&[u8], Instruction> {
    switch!(input, be_u8,
        0x32 => value!(Instruction::Aaload) |
        0x53 => value!(Instruction::Aastore) |
        0x01 => value!(Instruction::Aconstnull) |
        0x19 => map!(be_u8, Instruction::Aload) |
        0x2a => value!(Instruction::Aload0) |
        0x2b => value!(Instruction::Aload1) |
        0x2c => value!(Instruction::Aload2) |
        0x2d => value!(Instruction::Aload3) |
        0xbd => map!(be_u16, Instruction::Anewarray) |
        0xb0 => value!(Instruction::Areturn) |
        0xbe => value!(Instruction::Arraylength) |
        0x3a => map!(be_u8, Instruction::Astore) |
        0x4b => value!(Instruction::Astore0) |
        0x4c => value!(Instruction::Astore1) |
        0x4d => value!(Instruction::Astore2) |
        0x4e => value!(Instruction::Astore3) |
        0xbf => value!(Instruction::Athrow) |
        0x33 => value!(Instruction::Baload) |
        0x54 => value!(Instruction::Bastore) |
        0x10 => map!(be_i8, Instruction::Bipush) |
        0x34 => value!(Instruction::Caload) |
        0x55 => value!(Instruction::Castore) |
        0xc0 => map!(be_u16, Instruction::Checkcast) |
        0x90 => value!(Instruction::D2f) |
        0x8e => value!(Instruction::D2i) |
        0x8f => value!(Instruction::D2l) |
        0x63 => value!(Instruction::Dadd) |
        0x31 => value!(Instruction::Daload) |
        0x52 => value!(Instruction::Dastore) |
        0x98 => value!(Instruction::Dcmpg) |
        0x97 => value!(Instruction::Dcmpl) |
        0x0e => value!(Instruction::Dconst0) |
        0x0f => value!(Instruction::Dconst1) |
        0x6f => value!(Instruction::Ddiv) |
        0x18 => map!(be_u8, Instruction::Dload) |
        0x26 => value!(Instruction::Dload0) |
        0x27 => value!(Instruction::Dload1) |
        0x28 => value!(Instruction::Dload2) |
        0x29 => value!(Instruction::Dload3) |
        0x6b => value!(Instruction::Dmul) |
        0x77 => value!(Instruction::Dneg) |
        0x73 => value!(Instruction::Drem) |
        0xaf => value!(Instruction::Dreturn) |
        0x39 => map!(be_u8, Instruction::Dstore) |
        0x47 => value!(Instruction::Dstore0) |
        0x48 => value!(Instruction::Dstore1) |
        0x49 => value!(Instruction::Dstore2) |
        0x4a => value!(Instruction::Dstore3) |
        0x67 => value!(Instruction::Dsub) |
        0x59 => value!(Instruction::Dup) |
        0x5a => value!(Instruction::Dupx1) |
        0x5b => value!(Instruction::Dupx2) |
        0x5c => value!(Instruction::Dup2) |
        0x5d => value!(Instruction::Dup2x1) |
        0x5e => value!(Instruction::Dup2x2) |
        0x8d => value!(Instruction::F2d) |
        0x8b => value!(Instruction::F2i) |
        0x8c => value!(Instruction::F2l) |
        0x62 => value!(Instruction::Fadd) |
        0x30 => value!(Instruction::Faload) |
        0x51 => value!(Instruction::Fastore) |
        0x96 => value!(Instruction::Fcmpg) |
        0x95 => value!(Instruction::Fcmpl) |
        0x0b => value!(Instruction::Fconst0) |
        0x0c => value!(Instruction::Fconst1) |
        0x0d => value!(Instruction::Fconst2) |
        0x6e => value!(Instruction::Fdiv) |
        0x17 => map!(be_u8, Instruction::Fload) |
        0x22 => value!(Instruction::Fload0) |
        0x23 => value!(Instruction::Fload1) |
        0x24 => value!(Instruction::Fload2) |
        0x25 => value!(Instruction::Fload3) |
        0x6a => value!(Instruction::Fmul) |
        0x76 => value!(Instruction::Fneg) |
        0x72 => value!(Instruction::Frem) |
        0xae => value!(Instruction::Freturn) |
        0x38 => map!(be_u8, Instruction::Fstore) |
        0x43 => value!(Instruction::Fstore0) |
        0x44 => value!(Instruction::Fstore1) |
        0x45 => value!(Instruction::Fstore2) |
        0x46 => value!(Instruction::Fstore3) |
        0x66 => value!(Instruction::Fsub) |
        0xb4 => map!(be_u16, Instruction::Getfield) |
        0xb2 => map!(be_u16, Instruction::Getstatic) |
        0xa7 => map!(be_i16, Instruction::Goto) |
        0xc8 => map!(be_i32, Instruction::GotoW) |
        0x91 => value!(Instruction::I2b) |
        0x92 => value!(Instruction::I2c) |
        0x87 => value!(Instruction::I2d) |
        0x86 => value!(Instruction::I2f) |
        0x85 => value!(Instruction::I2l) |
        0x93 => value!(Instruction::I2s) |
        0x60 => value!(Instruction::Iadd) |
        0x2e => value!(Instruction::Iaload) |
        0x7e => value!(Instruction::Iand) |
        0x4f => value!(Instruction::Iastore) |
        0x02 => value!(Instruction::Iconstm1) |
        0x03 => value!(Instruction::Iconst0) |
        0x04 => value!(Instruction::Iconst1) |
        0x05 => value!(Instruction::Iconst2) |
        0x06 => value!(Instruction::Iconst3) |
        0x07 => value!(Instruction::Iconst4) |
        0x08 => value!(Instruction::Iconst5) |
        0x6c => value!(Instruction::Idiv) |
        0xa5 => map!(be_i16, Instruction::IfAcmpeq) |
        0xa6 => map!(be_i16, Instruction::IfAcmpne) |
        0x9f => map!(be_i16, Instruction::IfIcmpeq) |
        0xa0 => map!(be_i16, Instruction::IfIcmpne) |
        0xa1 => map!(be_i16, Instruction::IfIcmplt) |
        0xa2 => map!(be_i16, Instruction::IfIcmpge) |
        0xa3 => map!(be_i16, Instruction::IfIcmpgt) |
        0xa4 => map!(be_i16, Instruction::IfIcmple) |
        0x99 => map!(be_i16, Instruction::Ifeq) |
        0x9a => map!(be_i16, Instruction::Ifne) |
        0x9b => map!(be_i16, Instruction::Iflt) |
        0x9c => map!(be_i16, Instruction::Ifge) |
        0x9d => map!(be_i16, Instruction::Ifgt) |
        0x9e => map!(be_i16, Instruction::Ifle) |
        0xc7 => map!(be_i16, Instruction::Ifnonnull) |
        0xc6 => map!(be_i16, Instruction::Ifnull) |
        0x84 => do_parse!(index: be_u8 >> value: be_i8 >> (Instruction::Iinc{index, value})) |
        0x15 => map!(be_u8, Instruction::Iload) |
        0x1a => value!(Instruction::Iload0) |
        0x1b => value!(Instruction::Iload1) |
        0x1c => value!(Instruction::Iload2) |
        0x1d => value!(Instruction::Iload3) |
        0x68 => value!(Instruction::Imul) |
        0x74 => value!(Instruction::Ineg) |
        0xc1 => map!(be_u16, Instruction::Instanceof) |
        0xba => do_parse!(index: be_u16 >> tag!(&[0, 0]) >> (Instruction::Invokedynamic(index))) |
        0xb9 => do_parse!(index: be_u16 >> count: be_u8 >> tag!(&[0]) >> (Instruction::Invokeinterface{index, count})) |
        0xb7 => map!(be_u16, Instruction::Invokespecial) |
        0xb8 => map!(be_u16, Instruction::Invokestatic) |
        0xb6 => map!(be_u16, Instruction::Invokevirtual) |
        0x80 => value!(Instruction::Ior) |
        0x70 => value!(Instruction::Irem) |
        0xac => value!(Instruction::Ireturn) |
        0x78 => value!(Instruction::Ishl) |
        0x7a => value!(Instruction::Ishr) |
        0x36 => map!(be_u8, Instruction::Istore) |
        0x3b => value!(Instruction::Istore0) |
        0x3c => value!(Instruction::Istore1) |
        0x3d => value!(Instruction::Istore2) |
        0x3e => value!(Instruction::Istore3) |
        0x64 => value!(Instruction::Isub) |
        0x7c => value!(Instruction::Iushr) |
        0x82 => value!(Instruction::Ixor) |
        0xa8 => map!(be_i16, Instruction::Jsr) |
        0xc9 => map!(be_i32, Instruction::JsrW) |
        0x8a => value!(Instruction::L2d) |
        0x89 => value!(Instruction::L2f) |
        0x88 => value!(Instruction::L2i) |
        0x61 => value!(Instruction::Ladd) |
        0x2f => value!(Instruction::Laload) |
        0x7f => value!(Instruction::Land) |
        0x50 => value!(Instruction::Lastore) |
        0x94 => value!(Instruction::Lcmp) |
        0x09 => value!(Instruction::Lconst0) |
        0x0a => value!(Instruction::Lconst1) |
        0x12 => map!(be_u8, Instruction::Ldc) |
        0x13 => map!(be_u16, Instruction::LdcW) |
        0x14 => map!(be_u16, Instruction::Ldc2W) |
        0x6d => value!(Instruction::Ldiv) |
        0x16 => map!(be_u8, Instruction::Lload) |
        0x1e => value!(Instruction::Lload0) |
        0x1f => value!(Instruction::Lload1) |
        0x20 => value!(Instruction::Lload2) |
        0x21 => value!(Instruction::Lload3) |
        0x69 => value!(Instruction::Lmul) |
        0x75 => value!(Instruction::Lneg) |
        0xab => preceded!(apply!(align, address + 1), lookupswitch_parser) |
        0x81 => value!(Instruction::Lor) |
        0x71 => value!(Instruction::Lrem) |
        0xad => value!(Instruction::Lreturn) |
        0x79 => value!(Instruction::Lshl) |
        0x7b => value!(Instruction::Lshr) |
        0x37 => map!(be_u8, Instruction::Lstore) |
        0x3f => value!(Instruction::Lstore0) |
        0x40 => value!(Instruction::Lstore1) |
        0x41 => value!(Instruction::Lstore2) |
        0x42 => value!(Instruction::Lstore3) |
        0x65 => value!(Instruction::Lsub) |
        0x7d => value!(Instruction::Lushr) |
        0x83 => value!(Instruction::Lxor) |
        0xc2 => value!(Instruction::Monitorenter) |
        0xc3 => value!(Instruction::Monitorexit) |
        0xc5 => do_parse!(index: be_u16 >> dimensions: be_u8 >> (Instruction::Multianewarray{index, dimensions})) |
        0xbb => map!(be_u16, Instruction::New) |
        0xbc => map!(be_u8, Instruction::Newarray) |
        0x00 => value!(Instruction::Nop) |
        0x57 => value!(Instruction::Pop) |
        0x58 => value!(Instruction::Pop2) |
        0xb5 => map!(be_u16, Instruction::Putfield) |
        0xb3 => map!(be_u16, Instruction::Putstatic) |
        0xa9 => map!(be_u8, Instruction::Ret) |
        0xb1 => value!(Instruction::Return) |
        0x35 => value!(Instruction::Saload) |
        0x56 => value!(Instruction::Sastore) |
        0x11 => map!(be_i16, Instruction::Sipush) |
        0x5f => value!(Instruction::Swap) |
        0xaa => preceded!(apply!(align, address + 1), tableswitch_parser) |
        0xc4 => switch!(be_u8,
            0x19 => map!(be_u16, Instruction::AloadWide) |
            0x3a => map!(be_u16, Instruction::AstoreWide) |
            0x18 => map!(be_u16, Instruction::DloadWide) |
            0x39 => map!(be_u16, Instruction::DstoreWide) |
            0x17 => map!(be_u16, Instruction::FloadWide) |
            0x38 => map!(be_u16, Instruction::FstoreWide) |
            0x15 => map!(be_u16, Instruction::IloadWide) |
            0x36 => map!(be_u16, Instruction::IstoreWide) |
            0x16 => map!(be_u16, Instruction::LloadWide) |
            0x37 => map!(be_u16, Instruction::LstoreWide) |
            0xa9 => map!(be_u16, Instruction::RetWide) |
            0x84 => do_parse!(index: be_u16 >> value: be_i16 >> (Instruction::IincWide{index, value}))
        )
    )
}
