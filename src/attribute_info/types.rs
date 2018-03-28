#[derive(Debug)]
pub struct AttributeInfo {
    pub attribute_name_index: u16,
    pub attribute_length: u32,
    pub info: Vec<u8>,
}

#[derive(Debug)]
pub struct ExceptionEntry {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    pub catch_type: u16
}

#[derive(Debug)]
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

#[derive(Copy,Clone,Debug)]
#[repr(u8)]
pub enum VerificationTypeInfo {
    Top               = 0,
    Integer           = 1,
    Float             = 2,
    Double            = 3,
    Long              = 4,
    Null              = 5,
    UninitializedThis = 6,
    Object            = 7,
    Uninitialized     = 8,
}

#[derive(Debug)]
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

#[derive(Debug)]
pub struct StackMapTableAttribute {
    pub number_of_entries: u16,
    pub entries: Vec<StackMapFrame>,
}

#[derive(Debug)]
pub struct ExceptionsAttribute {
    pub exception_table_length: u16,
    pub exception_table: Vec<u16>,
}

#[derive(Debug)]
pub struct ConstantValueAttribute {
    pub constant_value_index: u16,
}

#[derive(Debug)]
pub struct BootstrapMethod {
    pub bootstrap_method_ref: u16,
    pub num_bootstrap_arguments: u16,
    pub bootstrap_arguments: Vec<u16>,
}

#[derive(Debug)]
pub struct BootstrapMethodsAttribute {
    pub num_bootstrap_methods: u16,
    pub bootstrap_methods: Vec<BootstrapMethod>,
}