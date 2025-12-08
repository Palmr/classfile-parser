use binrw::binrw;

#[derive(Clone, Debug)]
#[binrw]
#[brw(big)]
pub struct AttributeInfo {
    pub attribute_name_index: u16,
    pub attribute_length: u32,
    #[br(args { count: attribute_length.try_into().unwrap() })]
    pub info: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct ExceptionEntry {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    pub catch_type: u16,
}

#[derive(Clone, Debug)]
pub struct CodeAttribute {
    pub max_stack: u16,
    pub max_locals: u16,
    pub code_length: u32,
    pub code: Vec<u8>,
    pub exception_table_length: u16,
    pub exception_table: Vec<ExceptionEntry>,
    pub attributes_count: u16,
    pub attributes: Vec<AttributeInfo>,
}

#[derive(Clone, Debug)]
pub struct MethodParametersAttribute {
    pub parameters_count: u8,
    pub parameters: Vec<ParameterAttribute>,
}

#[derive(Clone, Debug)]
pub struct ParameterAttribute {
    pub name_index: u16,
    pub access_flags: u16,
}

#[derive(Clone, Debug)]
pub struct InnerClassesAttribute {
    pub number_of_classes: u16,
    pub classes: Vec<InnerClassInfo>,
}

#[derive(Clone, Debug)]
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
pub struct EnclosingMethodAttribute {
    pub class_index: u16,
    pub method_index: u16,
}

// in all reality this struct isn't required b/c it's zero sized
// "Synthetic" is a marker attribute
#[derive(Clone, Debug)]
pub struct SyntheticAttribute {}

#[derive(Clone, Debug)]
pub struct SignatureAttribute {
    pub signature_index: u16,
}

#[derive(Clone, Debug)]
pub struct RuntimeVisibleAnnotationsAttribute {
    pub num_annotations: u16,
    pub annotations: Vec<RuntimeAnnotation>,
}

#[derive(Clone, Debug)]
pub struct RuntimeInvisibleAnnotationsAttribute {
    pub num_annotations: u16,
    pub annotations: Vec<RuntimeAnnotation>,
}

#[derive(Clone, Debug)]
pub struct RuntimeVisibleParameterAnnotationsAttribute {
    pub num_parameters: u8,
    pub parameter_annotations: Vec<RuntimeVisibleAnnotationsAttribute>,
}

#[derive(Clone, Debug)]
pub struct RuntimeInvisibleParameterAnnotationsAttribute {
    pub num_parameters: u8,
    pub parameter_annotations: Vec<RuntimeInvisibleAnnotationsAttribute>,
}

#[derive(Clone, Debug)]
pub struct RuntimeVisibleTypeAnnotationsAttribute {
    pub num_annotations: u16,
    pub type_annotations: Vec<TypeAnnotation>,
}

#[derive(Clone, Debug)]
pub struct RuntimeInvisibleTypeAnnotationsAttribute {
    pub num_annotations: u16,
    pub type_annotations: Vec<TypeAnnotation>,
}

#[derive(Clone, Debug)]
pub struct TypeAnnotation {
    pub target_type: u8,
    pub target_info: TargetInfo,
    pub target_path: TypePath,
    pub type_index: u16,
    pub num_element_value_pairs: u16,
    pub element_value_pairs: Vec<ElementValuePair>,
}

#[derive(Clone, Debug)]
pub enum TargetInfo {
    TypeParameter {
        type_parameter_index: u8,
    },
    SuperType {
        supertype_index: u16,
    },
    TypeParameterBound {
        type_parameter_index: u8,
        bound_index: u8,
    },
    Empty,
    FormalParameter {
        formal_parameter_index: u8,
    },
    Throws {
        throws_type_index: u16,
    },
    LocalVar {
        table_length: u16,
        tables: Vec<LocalVarTableAnnotation>,
    },
    Catch {
        exception_table_index: u16,
    },
    Offset {
        offset: u16,
    },
    TypeArgument {
        offset: u16,
        type_argument_index: u8,
    },
}

#[derive(Clone, Debug)]
pub struct TypePath {
    pub path_length: u8,
    pub paths: Vec<TypePathEntry>,
}

#[derive(Clone, Debug)]
pub struct TypePathEntry {
    pub type_path_kind: u8,
    pub type_argument_index: u8,
}

#[derive(Clone, Debug)]
pub struct LocalVarTableAnnotation {
    pub start_pc: u16,
    pub length: u16,
    pub index: u16,
}

#[derive(Clone, Debug)]
pub struct RuntimeAnnotation {
    pub type_index: u16,
    pub num_element_value_pairs: u16,
    pub element_value_pairs: Vec<ElementValuePair>,
}

pub type DefaultAnnotation = ElementValue;

#[derive(Clone, Debug)]
pub struct ElementValuePair {
    pub element_name_index: u16,
    pub value: ElementValue,
}

#[derive(Clone, Debug)]
pub enum ElementValue {
    // pub tag: u8,
    ConstValueIndex { tag: char, value: u16 },
    EnumConst(EnumConstValue),
    ClassInfoIndex(u16),
    AnnotationValue(RuntimeAnnotation),
    ElementArray(ElementArrayValue),
}

#[derive(Clone, Debug)]
pub struct ElementArrayValue {
    pub num_values: u16,
    pub values: Vec<ElementValue>,
}

#[derive(Clone, Debug)]
pub struct EnumConstValue {
    pub type_name_index: u16,
    pub const_name_index: u16,
}

#[derive(Clone, Debug)]
pub struct SourceDebugExtensionAttribute {
    // Per the spec:
    // The debug_extension array holds extended debugging information which has no
    // semantic effect on the Java Virtual Machine. The information is represented
    // using a modified UTF-8 string with no terminating zero byte.
    pub debug_extension: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct LineNumberTable {
    pub line_number_table_length: u16,
    pub line_number_table: Vec<LineNumberTableEntry>,
}

#[derive(Clone, Debug)]
pub struct LineNumberTableEntry {
    pub start_pc: u16,
    pub line_number: u16,
}

#[derive(Clone, Debug)]
pub enum VerificationTypeInfo {
    Top,
    Integer,
    Float,
    Double,
    Long,
    Null,
    UninitializedThis,
    Object {
        /// An index into the constant pool for the class of the object
        class: u16,
    },
    Uninitialized {
        /// Offset into associated code array of a new instruction
        /// that created the object being stored here.
        offset: u16,
    },
}

#[derive(Clone, Debug)]
pub enum StackMapFrame {
    SameFrame {
        frame_type: u8,
    },
    SameLocals1StackItemFrame {
        frame_type: u8,
        stack: VerificationTypeInfo,
    },
    SameLocals1StackItemFrameExtended {
        frame_type: u8,
        offset_delta: u16,
        stack: VerificationTypeInfo,
    },
    ChopFrame {
        frame_type: u8,
        offset_delta: u16,
    },
    SameFrameExtended {
        frame_type: u8,
        offset_delta: u16,
    },
    AppendFrame {
        frame_type: u8,
        offset_delta: u16,
        locals: Vec<VerificationTypeInfo>,
    },
    FullFrame {
        frame_type: u8,
        offset_delta: u16,
        number_of_locals: u16,
        locals: Vec<VerificationTypeInfo>,
        number_of_stack_items: u16,
        stack: Vec<VerificationTypeInfo>,
    },
}

#[derive(Clone, Debug)]
pub struct StackMapTableAttribute {
    pub number_of_entries: u16,
    pub entries: Vec<StackMapFrame>,
}

#[derive(Clone, Debug)]
pub struct ExceptionsAttribute {
    pub exception_table_length: u16,
    pub exception_table: Vec<u16>,
}

#[derive(Clone, Debug)]
pub struct ConstantValueAttribute {
    pub constant_value_index: u16,
}

#[derive(Clone, Debug)]
pub struct BootstrapMethod {
    pub bootstrap_method_ref: u16,
    pub num_bootstrap_arguments: u16,
    pub bootstrap_arguments: Vec<u16>,
}

#[derive(Clone, Debug)]
pub struct BootstrapMethodsAttribute {
    pub num_bootstrap_methods: u16,
    pub bootstrap_methods: Vec<BootstrapMethod>,
}

/// The SourceFile attribute is an optional fixed-length attribute in the attributes table of a ClassFile structure (§4.1).
///
/// There may be at most one SourceFile attribute in the attributes table of a ClassFile structure.
/// [see more](https://docs.oracle.com/javase/specs/jvms/se8/html/jvms-4.html#jvms-4.7.10)
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SourceFileAttribute {
    /// The value of the sourcefile_index item must be a valid index into the constant_pool table.
    /// The constant_pool entry at that index must be a CONSTANT_Utf8_info structure representing a string.
    pub sourcefile_index: u16,
}
