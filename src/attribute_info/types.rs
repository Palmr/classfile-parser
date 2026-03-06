use std::io::{Cursor, Seek};

use binrw::{binrw, io::TakeSeekExt, BinRead, BinResult, BinWrite, Endian};

use crate::{
    code_attribute::{Instruction, LocalVariableTableAttribute, LocalVariableTypeTableAttribute},
    constant_info::{ConstantInfo, Utf8Constant},
    InterpretInner,
};

/// Custom parser for reading instructions from a code array.
///
/// Replaces `binrw::helpers::until_eof` because `Instruction` uses
/// `return_unexpected_error` which produces `NoVariantMatch` on EOF.
/// `until_eof` checks `err.is_eof()` to detect end-of-stream, but
/// `NoVariantMatch.is_eof()` returns false, causing a spurious error.
///
/// This parser also computes the correct per-instruction `address`
/// (offset within the code array) needed for tableswitch/lookupswitch
/// alignment padding.
#[binrw::parser(reader, endian)]
fn parse_code_instructions(code_start: u64) -> BinResult<Vec<Instruction>> {
    let mut instructions = Vec::new();
    loop {
        let pos = reader.stream_position()?;
        let address = (pos - code_start) as u32;
        match Instruction::read_options(reader, endian, binrw::args! { address: address }) {
            Ok(instruction) => instructions.push(instruction),
            Err(err) => {
                reader.seek(std::io::SeekFrom::Start(pos))?;
                let mut buf = [0u8; 1];
                if reader.read(&mut buf)? == 0 {
                    return Ok(instructions);
                }
                return Err(err);
            }
        }
    }
}

/// Custom writer for serializing instructions back into the code array.
///
/// Tracks the running byte address so that tableswitch/lookupswitch
/// padding is computed correctly.
#[binrw::writer(writer, endian)]
fn write_code_instructions(code: &Vec<Instruction>) -> BinResult<()> {
    let start = writer.stream_position()?;
    for instruction in code {
        let pos = writer.stream_position()?;
        let address = (pos - start) as u32;
        instruction.write_options(writer, endian, binrw::args! { address: address })?;
    }
    Ok(())
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct AttributeInfo {
    pub attribute_name_index: u16,
    pub attribute_length: u32,
    #[br(count = attribute_length)]
    pub info: Vec<u8>,
    #[brw(ignore)]
    pub info_parsed: Option<AttributeInfoVariant>,
}

impl InterpretInner for AttributeInfo {
    fn interpret_inner(&mut self, constant_pool: &Vec<ConstantInfo>) {
        if self.info_parsed.is_some() {
            return; // already parsed
        }

        if self.info.len() != self.attribute_length as usize {
            return; // malformed: length mismatch, leave as raw bytes
        }

        // Bounds-checked constant pool access
        let idx = self.attribute_name_index.wrapping_sub(1) as usize;
        let attr_name = match constant_pool.get(idx) {
            Some(ConstantInfo::Utf8(Utf8Constant { utf8_string })) => utf8_string.clone(),
            _ => return, // index out of bounds or not UTF-8, leave as raw bytes
        };

        /// Helper: try to read a binrw type from `info`, returning `None` on failure.
        macro_rules! try_read {
            ($ty:ty, $variant:ident) => {
                <$ty>::read(&mut Cursor::new(&mut self.info))
                    .ok()
                    .map(AttributeInfoVariant::$variant)
            };
            (be: $ty:ty, $variant:ident) => {
                <$ty>::read_be(&mut Cursor::new(&mut self.info))
                    .ok()
                    .map(AttributeInfoVariant::$variant)
            };
        }

        self.info_parsed = match attr_name.as_str() {
            "ConstantValue" => try_read!(be: ConstantValueAttribute, ConstantValue),
            "Code" => {
                match CodeAttribute::read(&mut Cursor::new(&mut self.info)) {
                    Ok(mut code) => {
                        for attr in &mut code.attributes {
                            attr.interpret_inner(constant_pool);
                        }
                        Some(AttributeInfoVariant::Code(code))
                    }
                    Err(_) => None,
                }
            }
            "StackMapTable" => try_read!(StackMapTableAttribute, StackMapTable),
            "BootstrapMethods" => try_read!(BootstrapMethodsAttribute, BootstrapMethods),
            "Exceptions" => try_read!(ExceptionsAttribute, Exceptions),
            "InnerClasses" => try_read!(InnerClassesAttribute, InnerClasses),
            "EnclosingMethod" => try_read!(EnclosingMethodAttribute, EnclosingMethod),
            "Synthetic" => try_read!(SyntheticAttribute, Synthetic),
            "Signature" => try_read!(SignatureAttribute, Signature),
            "SourceFile" => try_read!(SourceFileAttribute, SourceFile),
            "LineNumberTable" => try_read!(LineNumberTableAttribute, LineNumberTable),
            "LocalVariableTable" => try_read!(LocalVariableTableAttribute, LocalVariableTable),
            "LocalVariableTypeTable" => try_read!(LocalVariableTypeTableAttribute, LocalVariableTypeTable),
            "SourceDebugExtension" => try_read!(SourceDebugExtensionAttribute, SourceDebugExtension),
            "Deprecated" => try_read!(DeprecatedAttribute, Deprecated),
            "RuntimeVisibleAnnotations" => try_read!(RuntimeVisibleAnnotationsAttribute, RuntimeVisibleAnnotations),
            "RuntimeInvisibleAnnotations" => try_read!(RuntimeInvisibleAnnotationsAttribute, RuntimeInvisibleAnnotations),
            "RuntimeVisibleParameterAnnotations" => try_read!(RuntimeVisibleParameterAnnotationsAttribute, RuntimeVisibleParameterAnnotations),
            "RuntimeInvisibleParameterAnnotations" => try_read!(RuntimeInvisibleParameterAnnotationsAttribute, RuntimeInvisibleParameterAnnotations),
            "RuntimeVisibleTypeAnnotations" => try_read!(RuntimeVisibleTypeAnnotationsAttribute, RuntimeVisibleTypeAnnotations),
            "RuntimeInvisibleTypeAnnotations" => try_read!(RuntimeInvisibleTypeAnnotationsAttribute, RuntimeInvisibleTypeAnnotations),
            "AnnotationDefault" => try_read!(AnnotationDefaultAttribute, AnnotationDefault),
            "MethodParameters" => try_read!(MethodParametersAttribute, MethodParameters),
            "Module" => try_read!(ModuleAttribute, Module),
            "ModulePackages" => try_read!(ModulePackagesAttribute, ModulePackages),
            "ModuleMainClass" => try_read!(ModuleMainClassAttribute, ModuleMainClass),
            "NestHost" => try_read!(NestHostAttribute, NestHost),
            "NestMembers" => try_read!(NestMembersAttribute, NestMembers),
            "Record" => {
                match RecordAttribute::read(&mut Cursor::new(&mut self.info)) {
                    Ok(mut record) => {
                        for component in &mut record.components {
                            for attr in &mut component.attributes {
                                attr.interpret_inner(constant_pool);
                            }
                        }
                        Some(AttributeInfoVariant::Record(record))
                    }
                    Err(_) => None,
                }
            }
            "PermittedSubclasses" => try_read!(PermittedSubclassesAttribute, PermittedSubclasses),
            unhandled => Some(AttributeInfoVariant::Unknown(String::from(unhandled))),
        };
    }
}

impl AttributeInfo {
    /// Serializes `info_parsed` back into `info` bytes and updates `attribute_length`.
    ///
    /// Call this after modifying `info_parsed` to keep the raw bytes in sync.
    pub fn sync_from_parsed(&mut self) -> BinResult<()> {
        let new_info = match &mut self.info_parsed {
            Some(parsed) => {
                let mut cursor = Cursor::new(Vec::new());
                match parsed {
                    AttributeInfoVariant::Code(v) => {
                        v.sync_lengths()?;
                        for attr in &mut v.attributes {
                            attr.sync_from_parsed()?;
                        }
                        v.write(&mut cursor)?;
                    }
                    AttributeInfoVariant::ConstantValue(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::StackMapTable(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::Exceptions(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::InnerClasses(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::EnclosingMethod(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::Synthetic(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::Signature(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::SourceFile(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::SourceDebugExtension(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::LineNumberTable(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::LocalVariableTable(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::LocalVariableTypeTable(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::Deprecated(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::RuntimeVisibleAnnotations(v) => {
                        v.write(&mut cursor)?
                    }
                    AttributeInfoVariant::RuntimeInvisibleAnnotations(v) => {
                        v.write(&mut cursor)?
                    }
                    AttributeInfoVariant::RuntimeVisibleParameterAnnotations(v) => {
                        v.write(&mut cursor)?
                    }
                    AttributeInfoVariant::RuntimeInvisibleParameterAnnotations(v) => {
                        v.write(&mut cursor)?
                    }
                    AttributeInfoVariant::RuntimeVisibleTypeAnnotations(v) => {
                        v.write(&mut cursor)?
                    }
                    AttributeInfoVariant::RuntimeInvisibleTypeAnnotations(v) => {
                        v.write(&mut cursor)?
                    }
                    AttributeInfoVariant::AnnotationDefault(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::BootstrapMethods(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::MethodParameters(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::Module(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::ModulePackages(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::ModuleMainClass(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::NestHost(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::NestMembers(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::Record(v) => {
                        for component in &mut v.components {
                            for attr in &mut component.attributes {
                                attr.sync_from_parsed()?;
                            }
                        }
                        v.write(&mut cursor)?;
                    }
                    AttributeInfoVariant::PermittedSubclasses(v) => v.write(&mut cursor)?,
                    AttributeInfoVariant::Unknown(_) => return Ok(()),
                }
                cursor.into_inner()
            }
            None => return Ok(()),
        };
        self.attribute_length = new_info.len() as u32;
        self.info = new_info;
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub enum AttributeInfoVariant {
    ConstantValue(ConstantValueAttribute),
    Code(CodeAttribute),
    StackMapTable(StackMapTableAttribute),
    Exceptions(ExceptionsAttribute),
    InnerClasses(InnerClassesAttribute),
    EnclosingMethod(EnclosingMethodAttribute),
    Synthetic(SyntheticAttribute),
    Signature(SignatureAttribute),
    SourceFile(SourceFileAttribute),
    SourceDebugExtension(SourceDebugExtensionAttribute),
    LineNumberTable(LineNumberTableAttribute),
    LocalVariableTable(LocalVariableTableAttribute),
    LocalVariableTypeTable(LocalVariableTypeTableAttribute),
    Deprecated(DeprecatedAttribute),
    RuntimeVisibleAnnotations(RuntimeVisibleAnnotationsAttribute),
    RuntimeInvisibleAnnotations(RuntimeInvisibleAnnotationsAttribute),
    RuntimeVisibleParameterAnnotations(RuntimeVisibleParameterAnnotationsAttribute),
    RuntimeInvisibleParameterAnnotations(RuntimeInvisibleParameterAnnotationsAttribute),
    RuntimeVisibleTypeAnnotations(RuntimeVisibleTypeAnnotationsAttribute),
    RuntimeInvisibleTypeAnnotations(RuntimeInvisibleTypeAnnotationsAttribute),
    AnnotationDefault(AnnotationDefaultAttribute),
    BootstrapMethods(BootstrapMethodsAttribute),
    MethodParameters(MethodParametersAttribute),
    Module(ModuleAttribute),
    ModulePackages(ModulePackagesAttribute),
    ModuleMainClass(ModuleMainClassAttribute),
    NestHost(NestHostAttribute),
    NestMembers(NestMembersAttribute),
    Record(RecordAttribute),
    PermittedSubclasses(PermittedSubclassesAttribute),
    Unknown(String),
}

#[derive(Clone, Debug)]
#[binrw]
pub struct ExceptionEntry {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    pub catch_type: u16,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big, stream = s)]
pub struct CodeAttribute {
    pub max_stack: u16,
    pub max_locals: u16,
    pub code_length: u32,
    #[br(map_stream = |s| s.take_seek(code_length as u64), parse_with = parse_code_instructions, args(s.stream_position()?))]
    #[bw(write_with = write_code_instructions)]
    pub code: Vec<Instruction>,
    pub exception_table_length: u16,
    #[br(count = exception_table_length)]
    pub exception_table: Vec<ExceptionEntry>,
    pub attributes_count: u16,
    #[br(count = attributes_count)]
    pub attributes: Vec<AttributeInfo>,
}

impl CodeAttribute {
    /// Recalculates `code_length`, `exception_table_length`, and `attributes_count`
    /// from actual vector contents. Call this after modifying instructions or other
    /// code attribute internals.
    pub fn sync_lengths(&mut self) -> BinResult<()> {
        let mut buf = Cursor::new(Vec::new());
        for instruction in &self.code {
            let pos = buf.stream_position()?;
            let address = pos as u32;
            instruction
                .write_options(&mut buf, Endian::Big, binrw::args! { address: address })?;
        }
        self.code_length = buf.into_inner().len() as u32;
        self.exception_table_length = self.exception_table.len() as u16;
        self.attributes_count = self.attributes.len() as u16;
        Ok(())
    }

    /// Find the first instruction matching a predicate. Returns `(index, &Instruction)`.
    pub fn find_instruction<F>(&self, predicate: F) -> Option<(usize, &Instruction)>
    where
        F: Fn(&Instruction) -> bool,
    {
        self.code
            .iter()
            .enumerate()
            .find(|(_, instr)| predicate(instr))
    }

    /// Find all instructions matching a predicate. Returns `Vec<(index, &Instruction)>`.
    pub fn find_instructions<F>(&self, predicate: F) -> Vec<(usize, &Instruction)>
    where
        F: Fn(&Instruction) -> bool,
    {
        self.code
            .iter()
            .enumerate()
            .filter(|(_, instr)| predicate(instr))
            .collect()
    }

    /// Replace the instruction at `index`.
    pub fn replace_instruction(&mut self, index: usize, replacement: Instruction) {
        self.code[index] = replacement;
    }

    /// Replace a range of instructions with the exact number of Nop instructions
    /// needed to preserve `code_length`. Handles variable-length instructions
    /// (tableswitch, lookupswitch) by serializing to compute byte sizes.
    pub fn nop_out(&mut self, range: std::ops::Range<usize>) -> BinResult<()> {
        let mut buf = Cursor::new(Vec::new());
        // Serialize instructions before range to find byte offset
        for instr in &self.code[..range.start] {
            let address = buf.stream_position()? as u32;
            instr.write_options(&mut buf, Endian::Big, binrw::args! { address })?;
        }
        // Serialize instructions in range to find byte count
        let range_start_pos = buf.stream_position()?;
        for instr in &self.code[range.clone()] {
            let address = buf.stream_position()? as u32;
            instr.write_options(&mut buf, Endian::Big, binrw::args! { address })?;
        }
        let byte_count = (buf.stream_position()? - range_start_pos) as usize;
        self.code
            .splice(range, vec![Instruction::Nop; byte_count]);
        Ok(())
    }
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct MethodParametersAttribute {
    pub parameters_count: u8,
    #[br(count = parameters_count)]
    pub parameters: Vec<ParameterAttribute>,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct ParameterAttribute {
    pub name_index: u16,
    pub access_flags: u16,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct InnerClassesAttribute {
    pub number_of_classes: u16,
    #[br(count = number_of_classes)]
    pub classes: Vec<InnerClassInfo>,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct InnerClassInfo {
    pub inner_class_info_index: u16,
    pub outer_class_info_index: u16,
    pub inner_name_index: u16,
    pub inner_class_access_flags: u16,
}

bitflags! {
    #[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
    pub struct InnerClassAccessFlags: u16 {
        const PUBLIC = 0x0001;     //	Declared public; may be accessed from outside its package.
        const PRIVATE = 0x0002;    //	Declared private; may not be accessed from outside its package.
        const PROTECTED = 0x0004;  //	Declared praotected; may only be accessed within children.
        const STATIC = 0x0008;     //	Declared static.
        const FINAL = 0x0010;      //	Declared final; no subclasses allowed.
        const INTERFACE = 0x0200;  //	Is an interface, not a class.
        const ABSTRACT = 0x0400;   //	Declared abstract; must not be instantiated.
        const SYNTHETIC = 0x1000;  //	Declared synthetic; not present in the source code.
        const ANNOTATION = 0x2000; //	Declared as an annotation type.
        const ENUM = 0x4000;       //	Declared as an enum type.
    }
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct EnclosingMethodAttribute {
    pub class_index: u16,
    pub method_index: u16,
}

// in all reality this struct isn't required b/c it's zero sized
// "Deprecated" is a marker attribute
#[derive(Clone, Debug)]
#[binrw]
pub struct DeprecatedAttribute {}

// in all reality this struct isn't required b/c it's zero sized
// "Synthetic" is a marker attribute
#[derive(Clone, Debug)]
#[binrw]
pub struct SyntheticAttribute {}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct SignatureAttribute {
    pub signature_index: u16,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct RuntimeVisibleAnnotationsAttribute {
    pub num_annotations: u16,
    #[br(count = num_annotations)]
    pub annotations: Vec<RuntimeAnnotation>,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct RuntimeInvisibleAnnotationsAttribute {
    pub num_annotations: u16,
    #[br(count = num_annotations)]
    pub annotations: Vec<RuntimeAnnotation>,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct RuntimeVisibleParameterAnnotationsAttribute {
    pub num_parameters: u8,
    #[br(count = num_parameters)]
    pub parameter_annotations: Vec<RuntimeVisibleAnnotationsAttribute>,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct RuntimeInvisibleParameterAnnotationsAttribute {
    pub num_parameters: u8,
    #[br(count = num_parameters)]
    pub parameter_annotations: Vec<RuntimeInvisibleAnnotationsAttribute>,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct RuntimeVisibleTypeAnnotationsAttribute {
    pub num_annotations: u16,
    #[br(count = num_annotations)]
    pub type_annotations: Vec<TypeAnnotation>,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct RuntimeInvisibleTypeAnnotationsAttribute {
    pub num_annotations: u16,
    #[br(count = num_annotations)]
    pub type_annotations: Vec<TypeAnnotation>,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct TypeAnnotation {
    pub target_type: u8,
    #[br(args(target_type))]
    pub target_info: TargetInfo,
    pub target_path: TypePath,
    pub type_index: u16,
    pub num_element_value_pairs: u16,
    #[br(count = num_element_value_pairs)]
    pub element_value_pairs: Vec<ElementValuePair>,
}

#[derive(Clone, Debug)]
#[binrw]
#[br(import(target_type: u8))]
pub enum TargetInfo {
    #[br(pre_assert(target_type == 0x00 || target_type == 0x01))]
    TypeParameter {
        type_parameter_index: u8,
    },
    #[br(pre_assert(target_type == 0x10))]
    SuperType {
        supertype_index: u16,
    },
    #[br(pre_assert(target_type == 0x11 || target_type == 0x12))]
    TypeParameterBound {
        type_parameter_index: u8,
        bound_index: u8,
    },
    #[br(pre_assert((0x13..=0x15).contains(&target_type)))]
    Empty,
    #[br(pre_assert(target_type == 0x16))]
    FormalParameter {
        formal_parameter_index: u8,
    },
    #[br(pre_assert(target_type == 0x17))]
    Throws {
        throws_type_index: u16,
    },
    #[br(pre_assert(target_type == 0x40 || target_type == 0x41))]
    LocalVar {
        table_length: u16,
        #[br(count = table_length)]
        tables: Vec<LocalVarTableAnnotation>,
    },
    #[br(pre_assert(target_type == 0x42))]
    Catch {
        exception_table_index: u16,
    },
    #[br(pre_assert((0x43..=0x46).contains(&target_type)))]
    Offset {
        offset: u16,
    },
    #[br(pre_assert((0x47..=0x4B).contains(&target_type)))]
    TypeArgument {
        offset: u16,
        type_argument_index: u8,
    },
}

#[derive(Clone, Debug)]
#[binrw]
pub struct TypePath {
    pub path_length: u8,
    #[br(count = path_length)]
    pub paths: Vec<TypePathEntry>,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct TypePathEntry {
    pub type_path_kind: u8,
    pub type_argument_index: u8,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct LocalVarTableAnnotation {
    pub start_pc: u16,
    pub length: u16,
    pub index: u16,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct RuntimeAnnotation {
    pub type_index: u16,
    pub num_element_value_pairs: u16,
    #[br(count = num_element_value_pairs)]
    pub element_value_pairs: Vec<ElementValuePair>,
}

pub type AnnotationDefaultAttribute = ElementValue;

#[derive(Clone, Debug)]
#[binrw]
pub struct ElementValuePair {
    pub element_name_index: u16,
    pub value: ElementValue,
}

#[binrw::parser(reader)]
fn custom_char_parser() -> BinResult<char> {
    let mut buf = [0u8; 1];
    reader.read_exact(&mut buf)?;
    let c = u8::from_be_bytes(buf) as char;
    Ok(c)
}

#[binrw::writer(writer)]
pub fn custom_char_writer(c: &char) -> BinResult<()> {
    writer.write_all(c.to_string().as_bytes())?;
    Ok(())
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub enum ElementValue {
    // pub tag: u8,
    ConstValueIndex(ConstValueIndexValue),
    EnumConst(EnumConstValue),
    ClassInfoIndex(u16),
    AnnotationValue(RuntimeAnnotation),
    ElementArray(ElementArrayValue),
}

#[derive(Clone, Debug)]
#[binrw]
pub struct ConstValueIndexValue {
    #[br(parse_with = custom_char_parser)]
    #[bw(write_with = custom_char_writer)]
    pub tag: char,
    pub value: u16,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct ElementArrayValue {
    pub num_values: u16,
    #[br(count = num_values)]
    pub values: Vec<ElementValue>,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct EnumConstValue {
    pub type_name_index: u16,
    pub const_name_index: u16,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct SourceDebugExtensionAttribute {
    // Per the spec:
    // The debug_extension array holds extended debugging information which has no
    // semantic effect on the Java Virtual Machine. The information is represented
    // using a modified UTF-8 string with no terminating zero byte.
    // pub debug_extension: Vec<u8>,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct LineNumberTableAttribute {
    pub line_number_table_length: u16,
    #[br(count = line_number_table_length)]
    pub line_number_table: Vec<LineNumberTableEntry>,
}

#[derive(Clone, Debug)]
#[binrw]
pub struct LineNumberTableEntry {
    pub start_pc: u16,
    pub line_number: u16,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub enum VerificationTypeInfo {
    #[brw(magic = 0u8)]
    Top,
    #[brw(magic = 1u8)]
    Integer,
    #[brw(magic = 2u8)]
    Float,
    #[brw(magic = 3u8)]
    Double,
    #[brw(magic = 4u8)]
    Long,
    #[brw(magic = 5u8)]
    Null,
    #[brw(magic = 6u8)]
    UninitializedThis,
    #[brw(magic = 7u8)]
    Object {
        /// An index into the constant pool for the class of the object
        class: u16,
    },
    #[brw(magic = 8u8)]
    Uninitialized {
        /// Offset into associated code array of a new instruction
        /// that created the object being stored here.
        offset: u16,
    },
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct StackMapFrame {
    pub frame_type: u8,
    #[br(args(frame_type))]
    pub inner: StackMapFrameInner,
}

#[derive(Clone, Debug)]
#[binrw]
#[br(import(frame_type: u8))]
pub enum StackMapFrameInner {
    #[br(pre_assert((0..=63).contains(&frame_type)))]
    SameFrame {
        //frame_type: u8,
    },
    #[br(pre_assert((64..=127).contains(&frame_type)))]
    SameLocals1StackItemFrame {
        //frame_type: u8,
        stack: VerificationTypeInfo,
    },
    #[br(pre_assert(frame_type == 247))]
    SameLocals1StackItemFrameExtended {
        //frame_type: u8,
        offset_delta: u16,
        stack: VerificationTypeInfo,
    },
    #[br(pre_assert((248..=250).contains(&frame_type)))]
    ChopFrame {
        //frame_type: u8,
        offset_delta: u16,
    },
    #[br(pre_assert(frame_type == 251))]
    SameFrameExtended {
        //frame_type: u8,
        offset_delta: u16,
    },
    #[br(pre_assert((252..=254).contains(&frame_type)))]
    AppendFrame {
        //frame_type: u8,
        offset_delta: u16,
        #[br(count = frame_type - 251)]
        locals: Vec<VerificationTypeInfo>,
    },
    #[br(pre_assert(frame_type == 255))]
    FullFrame {
        //frame_type: u8,
        offset_delta: u16,
        number_of_locals: u16,
        #[br(count = number_of_locals)]
        locals: Vec<VerificationTypeInfo>,
        number_of_stack_items: u16,
        #[br(count = number_of_stack_items)]
        stack: Vec<VerificationTypeInfo>,
    },
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct StackMapTableAttribute {
    pub number_of_entries: u16,
    #[br(count = number_of_entries)]
    pub entries: Vec<StackMapFrame>,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct ExceptionsAttribute {
    pub exception_table_length: u16,
    #[br(count = exception_table_length)]
    pub exception_table: Vec<u16>,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct ConstantValueAttribute {
    pub constant_value_index: u16,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct BootstrapMethod {
    pub bootstrap_method_ref: u16,
    pub num_bootstrap_arguments: u16,
    #[br(count = num_bootstrap_arguments)]
    pub bootstrap_arguments: Vec<u16>,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct BootstrapMethodsAttribute {
    pub num_bootstrap_methods: u16,
    #[br(count = num_bootstrap_methods)]
    pub bootstrap_methods: Vec<BootstrapMethod>,
}

/// The SourceFile attribute is an optional fixed-length attribute in the attributes table of a ClassFile structure (§4.1).
///
/// There may be at most one SourceFile attribute in the attributes table of a ClassFile structure.
/// [see more](https://docs.oracle.com/javase/specs/jvms/se8/html/jvms-4.html#jvms-4.7.10)
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[binrw]
#[brw(big)]
pub struct SourceFileAttribute {
    /// The value of the sourcefile_index item must be a valid index into the constant_pool table.
    /// The constant_pool entry at that index must be a CONSTANT_Utf8_info structure representing a string.
    pub sourcefile_index: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[binrw]
#[brw(big)]
pub struct ModuleAttribute {
    pub module_name_index: u16,
    pub module_flags: u16,
    pub module_version_index: u16,
    pub requires_count: u16,
    #[br(count = requires_count)]
    pub requires: Vec<ModuleRequiresAttribute>,
    pub exports_count: u16,
    #[br(count = exports_count)]
    pub exports: Vec<ModuleExportsAttribute>,
    pub opens_count: u16,
    #[br(count = opens_count)]
    pub opens: Vec<ModuleOpensAttribute>,
    pub uses_count: u16,
    #[br(count = uses_count)]
    pub uses: Vec<u16>,
    pub provides_count: u16,
    #[br(count = provides_count)]
    pub provides: Vec<ModuleProvidesAttribute>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[binrw]
#[brw(big)]
pub struct ModuleRequiresAttribute {
    pub requires_index: u16,
    pub requires_flags: u16,
    pub requires_version_index: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[binrw]
#[brw(big)]
pub struct ModuleExportsAttribute {
    pub exports_index: u16,
    pub exports_flags: u16,
    pub exports_to_count: u16,
    #[br(count = exports_to_count)]
    pub exports_to_index: Vec<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[binrw]
#[brw(big)]
pub struct ModuleOpensAttribute {
    pub opens_index: u16,
    pub opens_flags: u16,
    pub opens_to_count: u16,
    #[br(count = opens_to_count)]
    pub opens_to_index: Vec<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[binrw]
#[brw(big)]
pub struct ModuleProvidesAttribute {
    pub provides_index: u16,
    pub provides_with_count: u16,
    #[br(count = provides_with_count)]
    pub provides_with_index: Vec<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[binrw]
#[brw(big)]
pub struct ModulePackagesAttribute {
    pub package_count: u16,
    #[br(count = package_count)]
    pub package_index: Vec<u16>,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[binrw]
#[brw(big)]
pub struct ModuleMainClassAttribute {
    pub main_class_index: u16,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[binrw]
#[brw(big)]
pub struct NestHostAttribute {
    pub host_class_index: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[binrw]
#[brw(big)]
pub struct NestMembersAttribute {
    pub number_of_classes: u16,
    #[br(count = number_of_classes)]
    pub classes: Vec<u16>,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct RecordAttribute {
    pub components_count: u16,
    #[br(count = components_count)]
    pub components: Vec<RecordComponentInfo>,
}

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct RecordComponentInfo {
    pub name_index: u16,
    pub descriptor_index: u16,
    pub attributes_count: u16,
    #[br(count = attributes_count)]
    pub attributes: Vec<AttributeInfo>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[binrw]
#[brw(big)]
pub struct PermittedSubclassesAttribute {
    pub number_of_classes: u16,
    #[br(count = number_of_classes)]
    pub classes: Vec<u16>,
}
