use std::io::{Read, Seek};

use crate::attribute_info::AttributeInfo;
use crate::constant_info::{
    ClassConstant, ConstantInfo, DoubleConstant, FieldRefConstant, FloatConstant, IntegerConstant,
    InterfaceMethodRefConstant, InvokeDynamicConstant, LongConstant, MethodHandleConstant,
    MethodRefConstant, MethodTypeConstant, NameAndTypeConstant, StringConstant, Utf8Constant,
};
use crate::field_info::FieldInfo;
use crate::method_info::MethodInfo;

use binrw::{
    BinRead, BinResult, BinWrite, Endian, VecArgs, binrw,
    meta::{EndianKind, ReadEndian},
};

/// Custom writer for the constant pool that skips Unusable sentinel entries.
///
/// On read, Long and Double constants occupy two slots in the constant pool,
/// and we insert an `Unusable` placeholder for the second slot. On write,
/// we must skip these placeholders since they are not part of the binary format.
#[binrw::writer(writer, endian)]
fn write_const_pool(pool: &Vec<ConstantInfo>) -> BinResult<()> {
    for item in pool {
        if !matches!(item, ConstantInfo::Unusable) {
            item.write_options(writer, endian, ())?;
        }
    }
    Ok(())
}

#[derive(BinWrite, Clone, Debug)]
#[brw(big, magic = b"\xca\xfe\xba\xbe")]
pub struct ClassFile {
    pub minor_version: u16,
    pub major_version: u16,
    pub const_pool_size: u16,
    #[bw(write_with = write_const_pool)]
    pub const_pool: Vec<ConstantInfo>,
    pub access_flags: ClassAccessFlags,
    pub this_class: u16,
    pub super_class: u16,
    pub interfaces_count: u16,
    pub interfaces: Vec<u16>,
    pub fields_count: u16,
    pub fields: Vec<FieldInfo>,
    pub methods_count: u16,
    pub methods: Vec<MethodInfo>,
    pub attributes_count: u16,
    pub attributes: Vec<AttributeInfo>,
}

pub trait InterpretInner {
    fn interpret_inner(&mut self, const_pool: &Vec<ConstantInfo>);
}

impl ReadEndian for ClassFile {
    const ENDIAN: EndianKind = EndianKind::Endian(Endian::Big);
}

fn const_pool_parser<R: Read + Seek>(
    r: &mut R,
    endian: Endian,
    args: VecArgs<()>,
) -> BinResult<Vec<ConstantInfo>> {
    let count = args.count.saturating_sub(1);
    // Each CP entry is at least 3 bytes (1 tag + 2 data).
    validate_count_vs_remaining(r, count, 3, "constant_pool_count")?;
    let mut v = vec![];
    while v.len() < count {
        v.push(ConstantInfo::read_options(r, endian, args.inner)?);
        if matches!(
            v.last().unwrap(),
            ConstantInfo::Double(_) | ConstantInfo::Long(_)
        ) {
            v.push(ConstantInfo::Unusable);
        }
    }

    Ok(v)
}

/// Validate that `count * min_entry_size` fits within the remaining data.
fn validate_count_vs_remaining<R: Read + Seek>(
    r: &mut R,
    count: usize,
    min_entry_size: usize,
    label: &str,
) -> BinResult<()> {
    if count == 0 {
        return Ok(());
    }
    let pos = r.stream_position().map_err(binrw::Error::Io)?;
    let end = r
        .seek(std::io::SeekFrom::End(0))
        .map_err(binrw::Error::Io)?;
    r.seek(std::io::SeekFrom::Start(pos))
        .map_err(binrw::Error::Io)?;
    let remaining = end.saturating_sub(pos) as usize;
    if count * min_entry_size > remaining {
        return Err(binrw::Error::AssertFail {
            pos,
            message: format!(
                "{} {} requires at least {} bytes but only {} remain",
                label,
                count,
                count * min_entry_size,
                remaining
            ),
        });
    }
    Ok(())
}

impl BinRead for ClassFile {
    type Args<'a> = ();

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let magic = u32::read_options(reader, Endian::Big, ())?;
        if magic != u32::from_be_bytes([0xca, 0xfe, 0xba, 0xbe]) {
            return Err(binrw::Error::BadMagic {
                pos: 0,
                found: Box::new(magic),
            });
        }

        let minor_version = u16::read_options(reader, Endian::Big, ())?;
        let major_version = u16::read_options(reader, Endian::Big, ())?;
        let const_pool_size = u16::read_options(reader, Endian::Big, ())?;
        let const_pool = const_pool_parser(
            reader,
            Endian::Big,
            VecArgs {
                count: const_pool_size as usize,
                inner: (),
            },
        )?;

        let access_flags = ClassAccessFlags::read_options(reader, Endian::Big, ())?;
        let this_class = u16::read_options(reader, Endian::Big, ())?;
        let super_class = u16::read_options(reader, Endian::Big, ())?;
        let interfaces_count = u16::read_options(reader, Endian::Big, ())?;
        // Each interface is a u16 (2 bytes)
        validate_count_vs_remaining(reader, interfaces_count as usize, 2, "interfaces_count")?;
        let interfaces = Vec::<u16>::read_options(
            reader,
            Endian::Big,
            VecArgs {
                count: interfaces_count as usize,
                inner: (),
            },
        )?;
        let fields_count = u16::read_options(reader, Endian::Big, ())?;
        // Each field is at least 8 bytes (flags + name_idx + desc_idx + attr_count)
        validate_count_vs_remaining(reader, fields_count as usize, 8, "fields_count")?;
        let mut fields = Vec::<FieldInfo>::read_options(
            reader,
            Endian::Big,
            VecArgs {
                count: fields_count as usize,
                inner: (),
            },
        )?;

        let methods_count = u16::read_options(reader, Endian::Big, ())?;
        // Each method is at least 8 bytes (flags + name_idx + desc_idx + attr_count)
        validate_count_vs_remaining(reader, methods_count as usize, 8, "methods_count")?;
        let mut methods = Vec::<MethodInfo>::read_options(
            reader,
            Endian::Big,
            VecArgs {
                count: methods_count as usize,
                inner: (),
            },
        )?;

        let attributes_count = u16::read_options(reader, Endian::Big, ())?;
        // Each attribute is at least 6 bytes (name_idx + length)
        validate_count_vs_remaining(reader, attributes_count as usize, 6, "attributes_count")?;
        let mut attributes = Vec::<AttributeInfo>::read_options(
            reader,
            Endian::Big,
            VecArgs {
                count: attributes_count as usize,
                inner: (),
            },
        )?;

        for field in &mut fields {
            field.interpret_inner(&const_pool);
        }

        for method in &mut methods {
            method.interpret_inner(&const_pool);
        }

        for attr in &mut attributes {
            attr.interpret_inner(&const_pool);
        }

        Ok(ClassFile {
            minor_version,
            major_version,
            const_pool_size,
            const_pool,
            access_flags,
            this_class,
            super_class,
            interfaces_count,
            interfaces,
            fields_count,
            fields,
            methods_count,
            methods,
            attributes_count,
            attributes,
        })
    }
}

impl ClassFile {
    /// Recalculates all count fields from actual vector lengths.
    /// Call this after adding or removing entries from const_pool, interfaces,
    /// fields, methods, or attributes.
    pub fn sync_counts(&mut self) {
        fn checked_u16(val: usize, field: &str) -> u16 {
            u16::try_from(val)
                .unwrap_or_else(|_| panic!("{} count {} exceeds u16::MAX", field, val))
        }
        self.const_pool_size = checked_u16(self.const_pool.len() + 1, "const_pool");
        self.interfaces_count = checked_u16(self.interfaces.len(), "interfaces");
        self.fields_count = checked_u16(self.fields.len(), "fields");
        self.methods_count = checked_u16(self.methods.len(), "methods");
        self.attributes_count = checked_u16(self.attributes.len(), "attributes");
    }

    /// Look up a UTF-8 constant pool entry by its 1-based index.
    /// Returns `None` if the index is out of range or does not point to a Utf8 entry.
    pub fn get_utf8(&self, index: u16) -> Option<&str> {
        match self.const_pool.get((index - 1) as usize)? {
            ConstantInfo::Utf8(u) => Some(&u.utf8_string),
            _ => None,
        }
    }

    /// Find the 1-based constant pool index of a UTF-8 entry matching the given string.
    pub fn find_utf8_index(&self, value: &str) -> Option<u16> {
        for (i, entry) in self.const_pool.iter().enumerate() {
            if let ConstantInfo::Utf8(u) = entry
                && u.utf8_string == value
            {
                return Some((i + 1) as u16);
            }
        }
        None
    }

    /// Find a method by name.
    pub fn find_method(&self, name: &str) -> Option<&MethodInfo> {
        self.methods
            .iter()
            .find(|m| self.get_utf8(m.name_index) == Some(name))
    }

    /// Find a method by name, returning a mutable reference.
    pub fn find_method_mut(&mut self, name: &str) -> Option<&mut MethodInfo> {
        let idx = self
            .methods
            .iter()
            .position(|m| self.get_utf8(m.name_index) == Some(name))?;
        Some(&mut self.methods[idx])
    }

    /// Find a field by name.
    pub fn find_field(&self, name: &str) -> Option<&FieldInfo> {
        self.fields
            .iter()
            .find(|f| self.get_utf8(f.name_index) == Some(name))
    }

    /// Find a field by name, returning a mutable reference.
    pub fn find_field_mut(&mut self, name: &str) -> Option<&mut FieldInfo> {
        let idx = self
            .fields
            .iter()
            .position(|f| self.get_utf8(f.name_index) == Some(name))?;
        Some(&mut self.fields[idx])
    }

    /// Add a UTF-8 constant to the pool. Returns the 1-based index.
    /// Always adds a new entry (no dedup). Does NOT call `sync_counts()`.
    pub fn add_utf8(&mut self, value: &str) -> u16 {
        let index = (self.const_pool.len() + 1) as u16;
        self.const_pool.push(ConstantInfo::Utf8(Utf8Constant {
            utf8_string: String::from(value),
        }));
        index
    }

    /// Get or add a UTF-8 constant. Returns existing index if found, otherwise adds.
    pub fn get_or_add_utf8(&mut self, value: &str) -> u16 {
        if let Some(idx) = self.find_utf8_index(value) {
            idx
        } else {
            self.add_utf8(value)
        }
    }

    /// Add a String constant (Utf8 + String pair). Returns the String constant's 1-based index.
    pub fn add_string(&mut self, value: &str) -> u16 {
        let utf8_index = self.add_utf8(value);
        let string_index = (self.const_pool.len() + 1) as u16;
        self.const_pool.push(ConstantInfo::String(StringConstant {
            string_index: utf8_index,
        }));
        string_index
    }

    /// Get or add a String constant, deduplicating both the Utf8 and String entries.
    pub fn get_or_add_string(&mut self, value: &str) -> u16 {
        let utf8_index = self.get_or_add_utf8(value);
        // Search for an existing String constant pointing to this Utf8
        for (i, entry) in self.const_pool.iter().enumerate() {
            if let ConstantInfo::String(s) = entry
                && s.string_index == utf8_index
            {
                return (i + 1) as u16;
            }
        }
        let string_index = (self.const_pool.len() + 1) as u16;
        self.const_pool.push(ConstantInfo::String(StringConstant {
            string_index: utf8_index,
        }));
        string_index
    }

    /// Add a Class constant (Utf8 + Class pair). `name` in internal form (e.g. `"java/lang/String"`).
    /// Returns the Class constant's 1-based index.
    pub fn add_class(&mut self, name: &str) -> u16 {
        let utf8_index = self.add_utf8(name);
        let class_index = (self.const_pool.len() + 1) as u16;
        self.const_pool.push(ConstantInfo::Class(ClassConstant {
            name_index: utf8_index,
        }));
        class_index
    }

    /// Get or add a Class constant, deduplicating both the Utf8 and Class entries.
    pub fn get_or_add_class(&mut self, name: &str) -> u16 {
        let utf8_index = self.get_or_add_utf8(name);
        for (i, entry) in self.const_pool.iter().enumerate() {
            if let ConstantInfo::Class(c) = entry
                && c.name_index == utf8_index
            {
                return (i + 1) as u16;
            }
        }
        let class_index = (self.const_pool.len() + 1) as u16;
        self.const_pool.push(ConstantInfo::Class(ClassConstant {
            name_index: utf8_index,
        }));
        class_index
    }

    /// Add a NameAndType constant. Utf8 entries for `name` and `descriptor` are deduped
    /// via `get_or_add_utf8`. Always adds a new NameAndType entry.
    pub fn add_name_and_type(&mut self, name: &str, descriptor: &str) -> u16 {
        let name_index = self.get_or_add_utf8(name);
        let descriptor_index = self.get_or_add_utf8(descriptor);
        let nat_index = (self.const_pool.len() + 1) as u16;
        self.const_pool
            .push(ConstantInfo::NameAndType(NameAndTypeConstant {
                name_index,
                descriptor_index,
            }));
        nat_index
    }

    /// Get or add a NameAndType constant, deduplicating.
    pub fn get_or_add_name_and_type(&mut self, name: &str, descriptor: &str) -> u16 {
        let name_index = self.get_or_add_utf8(name);
        let descriptor_index = self.get_or_add_utf8(descriptor);
        for (i, entry) in self.const_pool.iter().enumerate() {
            if let ConstantInfo::NameAndType(nat) = entry
                && nat.name_index == name_index
                && nat.descriptor_index == descriptor_index
            {
                return (i + 1) as u16;
            }
        }
        let index = (self.const_pool.len() + 1) as u16;
        self.const_pool
            .push(ConstantInfo::NameAndType(NameAndTypeConstant {
                name_index,
                descriptor_index,
            }));
        index
    }

    /// Get or add a MethodRef constant, deduplicating.
    pub fn get_or_add_method_ref(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
    ) -> u16 {
        let class_index = self.get_or_add_class(class_name);
        let nat_index = self.get_or_add_name_and_type(method_name, descriptor);
        for (i, entry) in self.const_pool.iter().enumerate() {
            if let ConstantInfo::MethodRef(r) = entry
                && r.class_index == class_index
                && r.name_and_type_index == nat_index
            {
                return (i + 1) as u16;
            }
        }
        let index = (self.const_pool.len() + 1) as u16;
        self.const_pool
            .push(ConstantInfo::MethodRef(MethodRefConstant {
                class_index,
                name_and_type_index: nat_index,
            }));
        index
    }

    /// Get or add a FieldRef constant, deduplicating.
    pub fn get_or_add_field_ref(
        &mut self,
        class_name: &str,
        field_name: &str,
        descriptor: &str,
    ) -> u16 {
        let class_index = self.get_or_add_class(class_name);
        let nat_index = self.get_or_add_name_and_type(field_name, descriptor);
        for (i, entry) in self.const_pool.iter().enumerate() {
            if let ConstantInfo::FieldRef(r) = entry
                && r.class_index == class_index
                && r.name_and_type_index == nat_index
            {
                return (i + 1) as u16;
            }
        }
        let index = (self.const_pool.len() + 1) as u16;
        self.const_pool
            .push(ConstantInfo::FieldRef(FieldRefConstant {
                class_index,
                name_and_type_index: nat_index,
            }));
        index
    }

    /// Get or add an InterfaceMethodRef constant, deduplicating.
    pub fn get_or_add_interface_method_ref(
        &mut self,
        class_name: &str,
        method_name: &str,
        descriptor: &str,
    ) -> u16 {
        let class_index = self.get_or_add_class(class_name);
        let nat_index = self.get_or_add_name_and_type(method_name, descriptor);
        for (i, entry) in self.const_pool.iter().enumerate() {
            if let ConstantInfo::InterfaceMethodRef(r) = entry
                && r.class_index == class_index
                && r.name_and_type_index == nat_index
            {
                return (i + 1) as u16;
            }
        }
        let index = (self.const_pool.len() + 1) as u16;
        self.const_pool.push(ConstantInfo::InterfaceMethodRef(
            InterfaceMethodRefConstant {
                class_index,
                name_and_type_index: nat_index,
            },
        ));
        index
    }

    /// Get or add an Integer constant, deduplicating.
    pub fn get_or_add_integer(&mut self, value: i32) -> u16 {
        for (i, entry) in self.const_pool.iter().enumerate() {
            if let ConstantInfo::Integer(c) = entry
                && c.value == value
            {
                return (i + 1) as u16;
            }
        }
        let index = (self.const_pool.len() + 1) as u16;
        self.const_pool
            .push(ConstantInfo::Integer(IntegerConstant { value }));
        index
    }

    /// Get or add a Float constant, deduplicating.
    pub fn get_or_add_float(&mut self, value: f32) -> u16 {
        for (i, entry) in self.const_pool.iter().enumerate() {
            if let ConstantInfo::Float(c) = entry
                && c.value.to_bits() == value.to_bits()
            {
                return (i + 1) as u16;
            }
        }
        let index = (self.const_pool.len() + 1) as u16;
        self.const_pool
            .push(ConstantInfo::Float(FloatConstant { value }));
        index
    }

    /// Get or add a Long constant, deduplicating. Adds Unusable sentinel.
    pub fn get_or_add_long(&mut self, value: i64) -> u16 {
        for (i, entry) in self.const_pool.iter().enumerate() {
            if let ConstantInfo::Long(c) = entry
                && c.value == value
            {
                return (i + 1) as u16;
            }
        }
        let index = (self.const_pool.len() + 1) as u16;
        self.const_pool
            .push(ConstantInfo::Long(LongConstant { value }));
        self.const_pool.push(ConstantInfo::Unusable);
        index
    }

    /// Get or add a Double constant, deduplicating. Adds Unusable sentinel.
    pub fn get_or_add_double(&mut self, value: f64) -> u16 {
        for (i, entry) in self.const_pool.iter().enumerate() {
            if let ConstantInfo::Double(c) = entry
                && c.value.to_bits() == value.to_bits()
            {
                return (i + 1) as u16;
            }
        }
        let index = (self.const_pool.len() + 1) as u16;
        self.const_pool
            .push(ConstantInfo::Double(DoubleConstant { value }));
        self.const_pool.push(ConstantInfo::Unusable);
        index
    }

    pub fn get_or_add_method_handle(&mut self, reference_kind: u8, reference_index: u16) -> u16 {
        for (i, entry) in self.const_pool.iter().enumerate() {
            if let ConstantInfo::MethodHandle(c) = entry
                && c.reference_kind == reference_kind
                && c.reference_index == reference_index
            {
                return (i + 1) as u16;
            }
        }
        let index = (self.const_pool.len() + 1) as u16;
        self.const_pool
            .push(ConstantInfo::MethodHandle(MethodHandleConstant {
                reference_kind,
                reference_index,
            }));
        index
    }

    pub fn get_or_add_method_type(&mut self, descriptor: &str) -> u16 {
        let desc_idx = self.get_or_add_utf8(descriptor);
        for (i, entry) in self.const_pool.iter().enumerate() {
            if let ConstantInfo::MethodType(c) = entry
                && c.descriptor_index == desc_idx
            {
                return (i + 1) as u16;
            }
        }
        let index = (self.const_pool.len() + 1) as u16;
        self.const_pool
            .push(ConstantInfo::MethodType(MethodTypeConstant {
                descriptor_index: desc_idx,
            }));
        index
    }

    pub fn get_or_add_invoke_dynamic(
        &mut self,
        bootstrap_method_attr_index: u16,
        name: &str,
        descriptor: &str,
    ) -> u16 {
        let nat_idx = self.get_or_add_name_and_type(name, descriptor);
        for (i, entry) in self.const_pool.iter().enumerate() {
            if let ConstantInfo::InvokeDynamic(c) = entry
                && c.bootstrap_method_attr_index == bootstrap_method_attr_index
                && c.name_and_type_index == nat_idx
            {
                return (i + 1) as u16;
            }
        }
        let index = (self.const_pool.len() + 1) as u16;
        self.const_pool
            .push(ConstantInfo::InvokeDynamic(InvokeDynamicConstant {
                bootstrap_method_attr_index,
                name_and_type_index: nat_idx,
            }));
        index
    }

    /// Sync everything after a patching session: calls `sync_from_parsed()` on all
    /// attributes (methods, fields, class-level), then `sync_counts()`.
    pub fn sync_all(&mut self) -> BinResult<()> {
        for method in &mut self.methods {
            for attr in &mut method.attributes {
                attr.sync_from_parsed()?;
            }
        }
        for field in &mut self.fields {
            for attr in &mut field.attributes {
                attr.sync_from_parsed()?;
            }
        }
        for attr in &mut self.attributes {
            attr.sync_from_parsed()?;
        }
        self.sync_counts();
        Ok(())
    }

    /// Parse a `ClassFile` from raw `.class` bytes.
    ///
    /// ```no_run
    /// let bytes = std::fs::read("HelloWorld.class").unwrap();
    /// let class_file = classfile_parser::ClassFile::from_bytes(&bytes).unwrap();
    /// ```
    pub fn from_bytes(bytes: &[u8]) -> BinResult<Self> {
        use std::io::Cursor;
        Self::read(&mut Cursor::new(bytes))
    }

    /// Serialize this `ClassFile` back to `.class` bytes.
    ///
    /// ```no_run
    /// # let class_file = classfile_parser::ClassFile::from_bytes(&[]).unwrap();
    /// let bytes = class_file.to_bytes().unwrap();
    /// std::fs::write("HelloWorld.class", bytes).unwrap();
    /// ```
    pub fn to_bytes(&self) -> BinResult<Vec<u8>> {
        use std::io::Cursor;
        let mut out = Cursor::new(Vec::new());
        self.write(&mut out)?;
        Ok(out.into_inner())
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[binrw]
pub struct ClassAccessFlags(u16);

bitflags! {
    impl ClassAccessFlags: u16 {
        const PUBLIC = 0x0001;     //	Declared public; may be accessed from outside its package.
        const FINAL = 0x0010;      //	Declared final; no subclasses allowed.
        const SUPER = 0x0020;      //	Treat superclass methods specially when invoked by the invokespecial instruction.
        const INTERFACE = 0x0200;  //	Is an interface, not a class.
        const ABSTRACT = 0x0400;   //	Declared abstract; must not be instantiated.
        const SYNTHETIC = 0x1000;  //	Declared synthetic; not present in the source code.
        const ANNOTATION = 0x2000; //	Declared as an annotation type.
        const ENUM = 0x4000;       //	Declared as an enum type.
        const MODULE = 0x8000;     //	Declared as a module type.
    }
}
