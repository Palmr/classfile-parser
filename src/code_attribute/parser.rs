use crate::code_attribute::types::Instruction;
use nom::{
    Err as BaseErr, IResult, Offset,
    bytes::complete::{tag, take},
    combinator::{complete, fail, map, success},
    error::Error,
    multi::{count, many0},
    number::complete::{be_i8, be_i16, be_i32, be_u8, be_u16, be_u32},
    sequence::{pair, preceded, tuple},
};

use super::{
    LocalVariableTableAttribute, LocalVariableTableItem, LocalVariableTypeTableAttribute,
    LocalVariableTypeTableItem,
};
type Err<E> = BaseErr<Error<E>>;

fn offset<'a>(remaining: &'a [u8], input: &[u8]) -> IResult<&'a [u8], usize> {
    Ok((remaining, input.offset(remaining)))
}

fn align(address: usize) -> impl Fn(&[u8]) -> IResult<&[u8], &[u8]> {
    move |input: &[u8]| take((4 - address % 4) % 4)(input)
}

fn lookupswitch_parser(input: &[u8]) -> IResult<&[u8], Instruction> {
    // This function provides type annotations required by rustc.
    fn each_pair(input: &[u8]) -> IResult<&[u8], (i32, i32)> {
        let (input, lookup) = be_i32(input)?;
        let (input, offset) = be_i32(input)?;
        Ok((input, (lookup, offset)))
    }
    let (input, default) = be_i32(input)?;
    let (input, npairs) = be_u32(input)?;
    let (input, pairs) = count(each_pair, npairs as usize)(input)?;
    Ok((input, Instruction::Lookupswitch { default, pairs }))
}

fn tableswitch_parser(input: &[u8]) -> IResult<&[u8], Instruction> {
    let (input, default) = be_i32(input)?;
    let (input, low) = be_i32(input)?;
    let (input, high) = be_i32(input)?;
    let (input, offsets) = count(be_i32, (high - low + 1) as usize)(input)?;
    Ok((
        input,
        Instruction::Tableswitch {
            default,
            low,
            high,
            offsets,
        },
    ))
}

pub fn code_parser(outer_input: &[u8]) -> IResult<&[u8], Vec<(usize, Instruction)>> {
    many0(complete(|input| {
        let (input, address) = offset(input, outer_input)?;
        let (input, instruction) = instruction_parser(input, address)?;
        Ok((input, (address, instruction)))
    }))(outer_input)
}

pub fn instruction_parser(input: &[u8], address: usize) -> IResult<&[u8], Instruction> {
    let (input, b0) = be_u8(input)?;
    let (input, instruction) = match b0 {
        0x32 => success(Instruction::Aaload)(input)?,
        0x53 => success(Instruction::Aastore)(input)?,
        0x01 => success(Instruction::Aconstnull)(input)?,
        0x19 => map(be_u8, Instruction::Aload)(input)?,
        0x2a => success(Instruction::Aload0)(input)?,
        0x2b => success(Instruction::Aload1)(input)?,
        0x2c => success(Instruction::Aload2)(input)?,
        0x2d => success(Instruction::Aload3)(input)?,
        0xbd => map(be_u16, Instruction::Anewarray)(input)?,
        0xb0 => success(Instruction::Areturn)(input)?,
        0xbe => success(Instruction::Arraylength)(input)?,
        0x3a => map(be_u8, Instruction::Astore)(input)?,
        0x4b => success(Instruction::Astore0)(input)?,
        0x4c => success(Instruction::Astore1)(input)?,
        0x4d => success(Instruction::Astore2)(input)?,
        0x4e => success(Instruction::Astore3)(input)?,
        0xbf => success(Instruction::Athrow)(input)?,
        0x33 => success(Instruction::Baload)(input)?,
        0x54 => success(Instruction::Bastore)(input)?,
        0x10 => map(be_i8, Instruction::Bipush)(input)?,
        0x34 => success(Instruction::Caload)(input)?,
        0x55 => success(Instruction::Castore)(input)?,
        0xc0 => map(be_u16, Instruction::Checkcast)(input)?,
        0x90 => success(Instruction::D2f)(input)?,
        0x8e => success(Instruction::D2i)(input)?,
        0x8f => success(Instruction::D2l)(input)?,
        0x63 => success(Instruction::Dadd)(input)?,
        0x31 => success(Instruction::Daload)(input)?,
        0x52 => success(Instruction::Dastore)(input)?,
        0x98 => success(Instruction::Dcmpg)(input)?,
        0x97 => success(Instruction::Dcmpl)(input)?,
        0x0e => success(Instruction::Dconst0)(input)?,
        0x0f => success(Instruction::Dconst1)(input)?,
        0x6f => success(Instruction::Ddiv)(input)?,
        0x18 => map(be_u8, Instruction::Dload)(input)?,
        0x26 => success(Instruction::Dload0)(input)?,
        0x27 => success(Instruction::Dload1)(input)?,
        0x28 => success(Instruction::Dload2)(input)?,
        0x29 => success(Instruction::Dload3)(input)?,
        0x6b => success(Instruction::Dmul)(input)?,
        0x77 => success(Instruction::Dneg)(input)?,
        0x73 => success(Instruction::Drem)(input)?,
        0xaf => success(Instruction::Dreturn)(input)?,
        0x39 => map(be_u8, Instruction::Dstore)(input)?,
        0x47 => success(Instruction::Dstore0)(input)?,
        0x48 => success(Instruction::Dstore1)(input)?,
        0x49 => success(Instruction::Dstore2)(input)?,
        0x4a => success(Instruction::Dstore3)(input)?,
        0x67 => success(Instruction::Dsub)(input)?,
        0x59 => success(Instruction::Dup)(input)?,
        0x5a => success(Instruction::Dupx1)(input)?,
        0x5b => success(Instruction::Dupx2)(input)?,
        0x5c => success(Instruction::Dup2)(input)?,
        0x5d => success(Instruction::Dup2x1)(input)?,
        0x5e => success(Instruction::Dup2x2)(input)?,
        0x8d => success(Instruction::F2d)(input)?,
        0x8b => success(Instruction::F2i)(input)?,
        0x8c => success(Instruction::F2l)(input)?,
        0x62 => success(Instruction::Fadd)(input)?,
        0x30 => success(Instruction::Faload)(input)?,
        0x51 => success(Instruction::Fastore)(input)?,
        0x96 => success(Instruction::Fcmpg)(input)?,
        0x95 => success(Instruction::Fcmpl)(input)?,
        0x0b => success(Instruction::Fconst0)(input)?,
        0x0c => success(Instruction::Fconst1)(input)?,
        0x0d => success(Instruction::Fconst2)(input)?,
        0x6e => success(Instruction::Fdiv)(input)?,
        0x17 => map(be_u8, Instruction::Fload)(input)?,
        0x22 => success(Instruction::Fload0)(input)?,
        0x23 => success(Instruction::Fload1)(input)?,
        0x24 => success(Instruction::Fload2)(input)?,
        0x25 => success(Instruction::Fload3)(input)?,
        0x6a => success(Instruction::Fmul)(input)?,
        0x76 => success(Instruction::Fneg)(input)?,
        0x72 => success(Instruction::Frem)(input)?,
        0xae => success(Instruction::Freturn)(input)?,
        0x38 => map(be_u8, Instruction::Fstore)(input)?,
        0x43 => success(Instruction::Fstore0)(input)?,
        0x44 => success(Instruction::Fstore1)(input)?,
        0x45 => success(Instruction::Fstore2)(input)?,
        0x46 => success(Instruction::Fstore3)(input)?,
        0x66 => success(Instruction::Fsub)(input)?,
        0xb4 => map(be_u16, Instruction::Getfield)(input)?,
        0xb2 => map(be_u16, Instruction::Getstatic)(input)?,
        0xa7 => map(be_i16, Instruction::Goto)(input)?,
        0xc8 => map(be_i32, Instruction::GotoW)(input)?,
        0x91 => success(Instruction::I2b)(input)?,
        0x92 => success(Instruction::I2c)(input)?,
        0x87 => success(Instruction::I2d)(input)?,
        0x86 => success(Instruction::I2f)(input)?,
        0x85 => success(Instruction::I2l)(input)?,
        0x93 => success(Instruction::I2s)(input)?,
        0x60 => success(Instruction::Iadd)(input)?,
        0x2e => success(Instruction::Iaload)(input)?,
        0x7e => success(Instruction::Iand)(input)?,
        0x4f => success(Instruction::Iastore)(input)?,
        0x02 => success(Instruction::Iconstm1)(input)?,
        0x03 => success(Instruction::Iconst0)(input)?,
        0x04 => success(Instruction::Iconst1)(input)?,
        0x05 => success(Instruction::Iconst2)(input)?,
        0x06 => success(Instruction::Iconst3)(input)?,
        0x07 => success(Instruction::Iconst4)(input)?,
        0x08 => success(Instruction::Iconst5)(input)?,
        0x6c => success(Instruction::Idiv)(input)?,
        0xa5 => map(be_i16, Instruction::IfAcmpeq)(input)?,
        0xa6 => map(be_i16, Instruction::IfAcmpne)(input)?,
        0x9f => map(be_i16, Instruction::IfIcmpeq)(input)?,
        0xa0 => map(be_i16, Instruction::IfIcmpne)(input)?,
        0xa1 => map(be_i16, Instruction::IfIcmplt)(input)?,
        0xa2 => map(be_i16, Instruction::IfIcmpge)(input)?,
        0xa3 => map(be_i16, Instruction::IfIcmpgt)(input)?,
        0xa4 => map(be_i16, Instruction::IfIcmple)(input)?,
        0x99 => map(be_i16, Instruction::Ifeq)(input)?,
        0x9a => map(be_i16, Instruction::Ifne)(input)?,
        0x9b => map(be_i16, Instruction::Iflt)(input)?,
        0x9c => map(be_i16, Instruction::Ifge)(input)?,
        0x9d => map(be_i16, Instruction::Ifgt)(input)?,
        0x9e => map(be_i16, Instruction::Ifle)(input)?,
        0xc7 => map(be_i16, Instruction::Ifnonnull)(input)?,
        0xc6 => map(be_i16, Instruction::Ifnull)(input)?,
        0x84 => map(pair(be_u8, be_i8), |(index, value)| Instruction::Iinc {
            index,
            value,
        })(input)?,
        0x15 => map(be_u8, Instruction::Iload)(input)?,
        0x1a => success(Instruction::Iload0)(input)?,
        0x1b => success(Instruction::Iload1)(input)?,
        0x1c => success(Instruction::Iload2)(input)?,
        0x1d => success(Instruction::Iload3)(input)?,
        0x68 => success(Instruction::Imul)(input)?,
        0x74 => success(Instruction::Ineg)(input)?,
        0xc1 => map(be_u16, Instruction::Instanceof)(input)?,
        0xba => map(pair(be_u16, tag(&[0, 0])), |(index, _)| {
            Instruction::Invokedynamic(index)
        })(input)?,
        0xb9 => map(tuple((be_u16, be_u8, tag(&[0]))), |(index, count, _)| {
            Instruction::Invokeinterface { index, count }
        })(input)?,
        0xb7 => map(be_u16, Instruction::Invokespecial)(input)?,
        0xb8 => map(be_u16, Instruction::Invokestatic)(input)?,
        0xb6 => map(be_u16, Instruction::Invokevirtual)(input)?,
        0x80 => success(Instruction::Ior)(input)?,
        0x70 => success(Instruction::Irem)(input)?,
        0xac => success(Instruction::Ireturn)(input)?,
        0x78 => success(Instruction::Ishl)(input)?,
        0x7a => success(Instruction::Ishr)(input)?,
        0x36 => map(be_u8, Instruction::Istore)(input)?,
        0x3b => success(Instruction::Istore0)(input)?,
        0x3c => success(Instruction::Istore1)(input)?,
        0x3d => success(Instruction::Istore2)(input)?,
        0x3e => success(Instruction::Istore3)(input)?,
        0x64 => success(Instruction::Isub)(input)?,
        0x7c => success(Instruction::Iushr)(input)?,
        0x82 => success(Instruction::Ixor)(input)?,
        0xa8 => map(be_i16, Instruction::Jsr)(input)?,
        0xc9 => map(be_i32, Instruction::JsrW)(input)?,
        0x8a => success(Instruction::L2d)(input)?,
        0x89 => success(Instruction::L2f)(input)?,
        0x88 => success(Instruction::L2i)(input)?,
        0x61 => success(Instruction::Ladd)(input)?,
        0x2f => success(Instruction::Laload)(input)?,
        0x7f => success(Instruction::Land)(input)?,
        0x50 => success(Instruction::Lastore)(input)?,
        0x94 => success(Instruction::Lcmp)(input)?,
        0x09 => success(Instruction::Lconst0)(input)?,
        0x0a => success(Instruction::Lconst1)(input)?,
        0x12 => map(be_u8, Instruction::Ldc)(input)?,
        0x13 => map(be_u16, Instruction::LdcW)(input)?,
        0x14 => map(be_u16, Instruction::Ldc2W)(input)?,
        0x6d => success(Instruction::Ldiv)(input)?,
        0x16 => map(be_u8, Instruction::Lload)(input)?,
        0x1e => success(Instruction::Lload0)(input)?,
        0x1f => success(Instruction::Lload1)(input)?,
        0x20 => success(Instruction::Lload2)(input)?,
        0x21 => success(Instruction::Lload3)(input)?,
        0x69 => success(Instruction::Lmul)(input)?,
        0x75 => success(Instruction::Lneg)(input)?,
        0xab => preceded(align(address + 1), lookupswitch_parser)(input)?,
        0x81 => success(Instruction::Lor)(input)?,
        0x71 => success(Instruction::Lrem)(input)?,
        0xad => success(Instruction::Lreturn)(input)?,
        0x79 => success(Instruction::Lshl)(input)?,
        0x7b => success(Instruction::Lshr)(input)?,
        0x37 => map(be_u8, Instruction::Lstore)(input)?,
        0x3f => success(Instruction::Lstore0)(input)?,
        0x40 => success(Instruction::Lstore1)(input)?,
        0x41 => success(Instruction::Lstore2)(input)?,
        0x42 => success(Instruction::Lstore3)(input)?,
        0x65 => success(Instruction::Lsub)(input)?,
        0x7d => success(Instruction::Lushr)(input)?,
        0x83 => success(Instruction::Lxor)(input)?,
        0xc2 => success(Instruction::Monitorenter)(input)?,
        0xc3 => success(Instruction::Monitorexit)(input)?,
        0xc5 => map(pair(be_u16, be_u8), |(index, dimensions)| {
            Instruction::Multianewarray { index, dimensions }
        })(input)?,
        0xbb => map(be_u16, Instruction::New)(input)?,
        0xbc => map(be_u8, Instruction::Newarray)(input)?,
        0x00 => success(Instruction::Nop)(input)?,
        0x57 => success(Instruction::Pop)(input)?,
        0x58 => success(Instruction::Pop2)(input)?,
        0xb5 => map(be_u16, Instruction::Putfield)(input)?,
        0xb3 => map(be_u16, Instruction::Putstatic)(input)?,
        0xa9 => map(be_u8, Instruction::Ret)(input)?,
        0xb1 => success(Instruction::Return)(input)?,
        0x35 => success(Instruction::Saload)(input)?,
        0x56 => success(Instruction::Sastore)(input)?,
        0x11 => map(be_i16, Instruction::Sipush)(input)?,
        0x5f => success(Instruction::Swap)(input)?,
        0xaa => preceded(align(address + 1), tableswitch_parser)(input)?,
        0xc4 => {
            let (input, b1) = be_u8(input)?;
            match b1 {
                0x19 => map(be_u16, Instruction::AloadWide)(input)?,
                0x3a => map(be_u16, Instruction::AstoreWide)(input)?,
                0x18 => map(be_u16, Instruction::DloadWide)(input)?,
                0x39 => map(be_u16, Instruction::DstoreWide)(input)?,
                0x17 => map(be_u16, Instruction::FloadWide)(input)?,
                0x38 => map(be_u16, Instruction::FstoreWide)(input)?,
                0x15 => map(be_u16, Instruction::IloadWide)(input)?,
                0x36 => map(be_u16, Instruction::IstoreWide)(input)?,
                0x16 => map(be_u16, Instruction::LloadWide)(input)?,
                0x37 => map(be_u16, Instruction::LstoreWide)(input)?,
                0xa9 => map(be_u16, Instruction::RetWide)(input)?,
                0x84 => map(pair(be_u16, be_i16), |(index, value)| {
                    Instruction::IincWide { index, value }
                })(input)?,
                _ => fail(input)?,
            }
        }
        _ => fail(input)?,
    };
    Ok((input, instruction))
}

pub fn local_variable_table_parser(
    input: &[u8],
) -> Result<(&[u8], LocalVariableTableAttribute), Err<&[u8]>> {
    let (input, local_variable_table_length) = be_u16(input)?;
    let (input, items) = count(
        variable_table_item_parser,
        local_variable_table_length as usize,
    )(input)?;
    Ok((
        input,
        LocalVariableTableAttribute {
            local_variable_table_length,
            items,
        },
    ))
}

pub fn variable_table_item_parser(
    input: &[u8],
) -> Result<(&[u8], LocalVariableTableItem), Err<&[u8]>> {
    let (input, start_pc) = be_u16(input)?;
    let (input, length) = be_u16(input)?;
    let (input, name_index) = be_u16(input)?;
    let (input, descriptor_index) = be_u16(input)?;
    let (input, index) = be_u16(input)?;
    Ok((
        input,
        LocalVariableTableItem {
            start_pc,
            length,
            name_index,
            descriptor_index,
            index,
        },
    ))
}

pub fn local_variable_type_table_parser(
    input: &[u8],
) -> Result<(&[u8], LocalVariableTypeTableAttribute), Err<&[u8]>> {
    let (input, local_variable_type_table_length) = be_u16(input)?;
    let (input, local_variable_type_table) = count(
        local_variable_type_table_item_parser,
        local_variable_type_table_length as usize,
    )(input)?;
    Ok((
        input,
        LocalVariableTypeTableAttribute {
            local_variable_type_table_length,
            local_variable_type_table,
        },
    ))
}

pub fn local_variable_type_table_item_parser(
    input: &[u8],
) -> Result<(&[u8], LocalVariableTypeTableItem), Err<&[u8]>> {
    let (input, start_pc) = be_u16(input)?;
    let (input, length) = be_u16(input)?;
    let (input, name_index) = be_u16(input)?;
    let (input, signature_index) = be_u16(input)?;
    let (input, index) = be_u16(input)?;
    Ok((
        input,
        LocalVariableTypeTableItem {
            start_pc,
            length,
            name_index,
            signature_index,
            index,
        },
    ))
}
