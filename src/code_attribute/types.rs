use binrw::binrw;

#[derive(Clone, Debug, Eq, PartialEq)]
#[binrw]
#[br(return_unexpected_error, import { address: u32 })]
#[bw(import { address: u32 })]
#[brw(big)]
pub enum Instruction {
    #[brw(magic = 0x00u8)]
    Nop,
    #[brw(magic = 0x32u8)]
    Aaload,
    #[brw(magic = 0x53u8)]
    Aastore,
    #[brw(magic = 0x01u8)]
    Aconstnull,
    #[brw(magic = 0x19u8)]
    Aload(u8),
    #[brw(magic = 0x2au8)]
    Aload0,
    #[brw(magic = 0x2bu8)]
    Aload1,
    #[brw(magic = 0x2cu8)]
    Aload2,
    #[brw(magic = 0x2du8)]
    Aload3,
    #[brw(magic = 0xbdu8)]
    Anewarray(u16),
    #[brw(magic = 0xb0u8)]
    Areturn,
    #[brw(magic = 0xbeu8)]
    Arraylength,
    #[brw(magic = 0x3au8)]
    Astore(u8),
    #[brw(magic = 0x4bu8)]
    Astore0,
    #[brw(magic = 0x4cu8)]
    Astore1,
    #[brw(magic = 0x4du8)]
    Astore2,
    #[brw(magic = 0x4eu8)]
    Astore3,
    #[brw(magic = 0xbfu8)]
    Athrow,
    #[brw(magic = 0x33u8)]
    Baload,
    #[brw(magic = 0x54u8)]
    Bastore,
    #[brw(magic = 0x10u8)]
    Bipush(i8),
    #[brw(magic = 0x34u8)]
    Caload,
    #[brw(magic = 0x55u8)]
    Castore,
    #[brw(magic = 0xc0u8)]
    Checkcast(u16),
    #[brw(magic = 0x90u8)]
    D2f,
    #[brw(magic = 0x8eu8)]
    D2i,
    #[brw(magic = 0x8fu8)]
    D2l,
    #[brw(magic = 0x63u8)]
    Dadd,
    #[brw(magic = 0x31u8)]
    Daload,
    #[brw(magic = 0x52u8)]
    Dastore,
    #[brw(magic = 0x98u8)]
    Dcmpg,
    #[brw(magic = 0x97u8)]
    Dcmpl,
    #[brw(magic = 0x0eu8)]
    Dconst0,
    #[brw(magic = 0x0fu8)]
    Dconst1,
    #[brw(magic = 0x6fu8)]
    Ddiv,
    #[brw(magic = 0x18u8)]
    Dload(u8),
    #[brw(magic = 0x26u8)]
    Dload0,
    #[brw(magic = 0x27u8)]
    Dload1,
    #[brw(magic = 0x28u8)]
    Dload2,
    #[brw(magic = 0x29u8)]
    Dload3,
    #[brw(magic = 0x6bu8)]
    Dmul,
    #[brw(magic = 0x77u8)]
    Dneg,
    #[brw(magic = 0x73u8)]
    Drem,
    #[brw(magic = 0xafu8)]
    Dreturn,
    #[brw(magic = 0x39u8)]
    Dstore(u8),
    #[brw(magic = 0x47u8)]
    Dstore0,
    #[brw(magic = 0x48u8)]
    Dstore1,
    #[brw(magic = 0x49u8)]
    Dstore2,
    #[brw(magic = 0x4au8)]
    Dstore3,
    #[brw(magic = 0x67u8)]
    Dsub,
    #[brw(magic = 0x59u8)]
    Dup,
    #[brw(magic = 0x5au8)]
    Dupx1,
    #[brw(magic = 0x5bu8)]
    Dupx2,
    #[brw(magic = 0x5cu8)]
    Dup2,
    #[brw(magic = 0x5du8)]
    Dup2x1,
    #[brw(magic = 0x5eu8)]
    Dup2x2,
    #[brw(magic = 0x8du8)]
    F2d,
    #[brw(magic = 0x8bu8)]
    F2i,
    #[brw(magic = 0x8cu8)]
    F2l,
    #[brw(magic = 0x62u8)]
    Fadd,
    #[brw(magic = 0x30u8)]
    Faload,
    #[brw(magic = 0x51u8)]
    Fastore,
    #[brw(magic = 0x96u8)]
    Fcmpg,
    #[brw(magic = 0x95u8)]
    Fcmpl,
    #[brw(magic = 0xbu8)]
    Fconst0,
    #[brw(magic = 0xcu8)]
    Fconst1,
    #[brw(magic = 0xdu8)]
    Fconst2,
    #[brw(magic = 0x6eu8)]
    Fdiv,
    #[brw(magic = 0x17u8)]
    Fload(u8),
    #[brw(magic = 0x22u8)]
    Fload0,
    #[brw(magic = 0x23u8)]
    Fload1,
    #[brw(magic = 0x24u8)]
    Fload2,
    #[brw(magic = 0x25u8)]
    Fload3,
    #[brw(magic = 0x6au8)]
    Fmul,
    #[brw(magic = 0x76u8)]
    Fneg,
    #[brw(magic = 0x72u8)]
    Frem,
    #[brw(magic = 0xaeu8)]
    Freturn,
    #[brw(magic = 0x38u8)]
    Fstore(u8),
    #[brw(magic = 0x43u8)]
    Fstore0,
    #[brw(magic = 0x44u8)]
    Fstore1,
    #[brw(magic = 0x45u8)]
    Fstore2,
    #[brw(magic = 0x46u8)]
    Fstore3,
    #[brw(magic = 0x66u8)]
    Fsub,
    #[brw(magic = 0xb4u8)]
    Getfield(u16),
    #[brw(magic = 0xb2u8)]
    Getstatic(u16),
    #[brw(magic = 0xa7u8)]
    Goto(i16),
    #[brw(magic = 0xc8u8)]
    GotoW(i32),
    #[brw(magic = 0x91u8)]
    I2b,
    #[brw(magic = 0x92u8)]
    I2c,
    #[brw(magic = 0x87u8)]
    I2d,
    #[brw(magic = 0x86u8)]
    I2f,
    #[brw(magic = 0x85u8)]
    I2l,
    #[brw(magic = 0x93u8)]
    I2s,
    #[brw(magic = 0x60u8)]
    Iadd,
    #[brw(magic = 0x2eu8)]
    Iaload,
    #[brw(magic = 0x7eu8)]
    Iand,
    #[brw(magic = 0x4fu8)]
    Iastore,
    #[brw(magic = 0x2u8)]
    Iconstm1,
    #[brw(magic = 0x3u8)]
    Iconst0,
    #[brw(magic = 0x4u8)]
    Iconst1,
    #[brw(magic = 0x5u8)]
    Iconst2,
    #[brw(magic = 0x6u8)]
    Iconst3,
    #[brw(magic = 0x7u8)]
    Iconst4,
    #[brw(magic = 0x8u8)]
    Iconst5,
    #[brw(magic = 0x6cu8)]
    Idiv,
    #[brw(magic = 0xa5u8)]
    IfAcmpeq(i16),
    #[brw(magic = 0xa6u8)]
    IfAcmpne(i16),
    #[brw(magic = 0x9fu8)]
    IfIcmpeq(i16),
    #[brw(magic = 0xa0u8)]
    IfIcmpne(i16),
    #[brw(magic = 0xa1u8)]
    IfIcmplt(i16),
    #[brw(magic = 0xa2u8)]
    IfIcmpge(i16),
    #[brw(magic = 0xa3u8)]
    IfIcmpgt(i16),
    #[brw(magic = 0xa4u8)]
    IfIcmple(i16),
    #[brw(magic = 0x99u8)]
    Ifeq(i16),
    #[brw(magic = 0x9au8)]
    Ifne(i16),
    #[brw(magic = 0x9bu8)]
    Iflt(i16),
    #[brw(magic = 0x9cu8)]
    Ifge(i16),
    #[brw(magic = 0x9du8)]
    Ifgt(i16),
    #[brw(magic = 0x9eu8)]
    Ifle(i16),
    #[brw(magic = 0xc7u8)]
    Ifnonnull(i16),
    #[brw(magic = 0xc6u8)]
    Ifnull(i16),
    #[brw(magic = 0x84u8)]
    Iinc { index: u8, value: i8 },
    #[brw(magic = 0x15u8)]
    Iload(u8),
    #[brw(magic = 0x1au8)]
    Iload0,
    #[brw(magic = 0x1bu8)]
    Iload1,
    #[brw(magic = 0x1cu8)]
    Iload2,
    #[brw(magic = 0x1du8)]
    Iload3,
    #[brw(magic = 0x68u8)]
    Imul,
    #[brw(magic = 0x74u8)]
    Ineg,
    #[brw(magic = 0xc1u8)]
    Instanceof(u16),
    #[brw(magic = 0xbau8)]
    Invokedynamic { index: u16, filler: u16 },
    #[brw(magic = 0xb9u8)]
    Invokeinterface { index: u16, count: u8, filler: u8 },
    #[brw(magic = 0xb7u8)]
    Invokespecial(u16),
    #[brw(magic = 0xb8u8)]
    Invokestatic(u16),
    #[brw(magic = 0xb6u8)]
    Invokevirtual(u16),
    #[brw(magic = 0x80u8)]
    Ior,
    #[brw(magic = 0x70u8)]
    Irem,
    #[brw(magic = 0xacu8)]
    Ireturn,
    #[brw(magic = 0x78u8)]
    Ishl,
    #[brw(magic = 0x7au8)]
    Ishr,
    #[brw(magic = 0x36u8)]
    Istore(u8),
    #[brw(magic = 0x3bu8)]
    Istore0,
    #[brw(magic = 0x3cu8)]
    Istore1,
    #[brw(magic = 0x3du8)]
    Istore2,
    #[brw(magic = 0x3eu8)]
    Istore3,
    #[brw(magic = 0x64u8)]
    Isub,
    #[brw(magic = 0x7cu8)]
    Iushr,
    #[brw(magic = 0x82u8)]
    Ixor,
    #[brw(magic = 0xa8u8)]
    Jsr(i16),
    #[brw(magic = 0xc9u8)]
    JsrW(i32),
    #[brw(magic = 0x8au8)]
    L2d,
    #[brw(magic = 0x89u8)]
    L2f,
    #[brw(magic = 0x88u8)]
    L2i,
    #[brw(magic = 0x61u8)]
    Ladd,
    #[brw(magic = 0x2fu8)]
    Laload,
    #[brw(magic = 0x7fu8)]
    Land,
    #[brw(magic = 0x50u8)]
    Lastore,
    #[brw(magic = 0x94u8)]
    Lcmp,
    #[brw(magic = 0x09u8)]
    Lconst0,
    #[brw(magic = 0x0au8)]
    Lconst1,
    #[brw(magic = 0x12u8)]
    Ldc(u8),
    #[brw(magic = 0x13u8)]
    LdcW(u16),
    #[brw(magic = 0x14u8)]
    Ldc2W(u16),
    #[brw(magic = 0x6du8)]
    Ldiv,
    #[brw(magic = 0x16u8)]
    Lload(u8),
    #[brw(magic = 0x1eu8)]
    Lload0,
    #[brw(magic = 0x1fu8)]
    Lload1,
    #[brw(magic = 0x20u8)]
    Lload2,
    #[brw(magic = 0x21u8)]
    Lload3,
    #[brw(magic = 0x69u8)]
    Lmul,
    #[brw(magic = 0x75u8)]
    Lneg,
    #[brw(magic = 0xabu8)]
    Lookupswitch {
        #[brw(pad_before = ((4 - (address + 1) % 4) % 4))]
        default: i32,
        npairs: u32,
        #[br(count = npairs)]
        pairs: Vec<(i32, i32)>,
    },
    #[brw(magic = 0x81u8)]
    Lor,
    #[brw(magic = 0x71u8)]
    Lrem,
    #[brw(magic = 0xadu8)]
    Lreturn,
    #[brw(magic = 0x79u8)]
    Lshl,
    #[brw(magic = 0x7bu8)]
    Lshr,
    #[brw(magic = 0x37u8)]
    Lstore(u8),
    #[brw(magic = 0x3fu8)]
    Lstore0,
    #[brw(magic = 0x40u8)]
    Lstore1,
    #[brw(magic = 0x41u8)]
    Lstore2,
    #[brw(magic = 0x42u8)]
    Lstore3,
    #[brw(magic = 0x65u8)]
    Lsub,
    #[brw(magic = 0x7du8)]
    Lushr,
    #[brw(magic = 0x83u8)]
    Lxor,
    #[brw(magic = 0xc2u8)]
    Monitorenter,
    #[brw(magic = 0xc3u8)]
    Monitorexit,
    #[brw(magic = 0xc5u8)]
    Multianewarray { index: u16, dimensions: u8 },
    #[brw(magic = 0xbbu8)]
    New(u16),
    #[brw(magic = 0xbcu8)]
    Newarray(u8),
    #[brw(magic = 0x57u8)]
    Pop,
    #[brw(magic = 0x58u8)]
    Pop2,
    #[brw(magic = 0xb5u8)]
    Putfield(u16),
    #[brw(magic = 0xb3u8)]
    Putstatic(u16),
    #[brw(magic = 0xa9u8)]
    Ret(u8),
    #[brw(magic = 0xb1u8)]
    Return,
    #[brw(magic = 0x35u8)]
    Saload,
    #[brw(magic = 0x56u8)]
    Sastore,
    #[brw(magic = 0x11u8)]
    Sipush(i16),
    #[brw(magic = 0x5fu8)]
    Swap,
    #[brw(magic = 0xaau8)]
    Tableswitch {
        #[brw(pad_before = ((4 - (address + 1) % 4) % 4))]
        default: i32,
        low: i32,
        high: i32,
        #[br(count = high - low + 1)]
        offsets: Vec<i32>,
    },
    #[brw(magic = b"\xc4\x19")]
    AloadWide(u16),
    #[brw(magic = b"\xc4\x3a")]
    AstoreWide(u16),
    #[brw(magic = b"\xc4\x18")]
    DloadWide(u16),
    #[brw(magic = b"\xc4\x39")]
    DstoreWide(u16),
    #[brw(magic = b"\xc4\x17")]
    FloadWide(u16),
    #[brw(magic = b"\xc4\x38")]
    FstoreWide(u16),
    #[brw(magic = b"\xc4\x15")]
    IloadWide(u16),
    #[brw(magic = b"\xc4\x36")]
    IstoreWide(u16),
    #[brw(magic = b"\xc4\x16")]
    LloadWide(u16),
    #[brw(magic = b"\xc4\x37")]
    LstoreWide(u16),
    #[brw(magic = b"\xc4\xa9")]
    RetWide(u16),
    #[brw(magic = b"\xc4\x84")]
    IincWide { index: u16, value: i16 },
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct LocalVariableTableAttribute {
    pub local_variable_table_length: u16,
    #[br(count = local_variable_table_length)]
    pub items: Vec<LocalVariableTableItem>,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct LocalVariableTableItem {
    pub start_pc: u16,
    pub length: u16,
    pub name_index: u16,
    pub descriptor_index: u16,
    pub index: u16,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct LocalVariableTypeTableAttribute {
    pub local_variable_type_table_length: u16,
    #[br(count = local_variable_type_table_length)]
    pub local_variable_type_table: Vec<LocalVariableTypeTableItem>,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct LocalVariableTypeTableItem {
    pub start_pc: u16,
    pub length: u16,
    pub name_index: u16,
    pub signature_index: u16,
    pub index: u16,
}
