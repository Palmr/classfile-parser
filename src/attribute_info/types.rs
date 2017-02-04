pub struct AttributeInfo {
    pub attribute_name_index: u16,
    pub attribute_length: u32,
    pub info: Vec<u8>,
}

pub struct ExceptionEntry {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    pub catch_type: u16
}

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

pub struct ExceptionsAttribute {
    pub exception_table_length: u16,
    pub exception_table: Vec<u16>,
}

pub struct ConstantValueAttribute {
    pub constant_value_index: u16,
}
