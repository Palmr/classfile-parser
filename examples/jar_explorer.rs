//! TUI JAR Explorer — interactive browser for Java `.jar` files.
//!
//! ```sh
//! cargo run --example jar_explorer --features tui-example -- path/to/file.jar
//! ```

use std::collections::BTreeMap;
use std::io::{self, Cursor};

use binrw::BinRead;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph};
use tui_textarea::{CursorMove, TextArea};

use classfile_parser::attribute_info::{
    AttributeInfoVariant, CodeAttribute, ExceptionEntry, LineNumberTableAttribute,
};
use classfile_parser::code_attribute::Instruction;
use classfile_parser::compile::{CompileOptions, compile_method_body, prepend_method_body};
use classfile_parser::constant_info::ConstantInfo;
use classfile_parser::field_info::FieldAccessFlags;
use classfile_parser::jar_utils::{JarFile, JarManifest};
use classfile_parser::method_info::MethodAccessFlags;
use classfile_parser::spring_utils::{SpringBootFormat, detect_format};
use classfile_parser::{ClassAccessFlags, ClassFile};

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

struct TreeNode {
    label: String,
    entry_path: Option<String>,
    depth: usize,
    expanded: bool,
    is_dir: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Tree,
    Viewer,
}

#[derive(Clone, PartialEq, Eq)]
enum VimMode {
    Normal,
    Search,
    Pending(char),
}

enum EditState<'a> {
    SelectMethod {
        entry_path: String,
        methods: Vec<(String, String)>, // (name, formatted_signature)
        selected: usize,
    },
    EditCode {
        entry_path: String,
        method_name: String,
        editor: TextArea<'a>,
        error_message: Option<String>,
    },
}

struct App<'a> {
    jar: JarFile,
    jar_path: String,
    spring_format: Option<SpringBootFormat>,
    tree: Vec<TreeNode>,
    tree_selected: usize,
    tree_scroll: usize,
    viewer: TextArea<'a>,
    vim_mode: VimMode,
    search_buffer: String,
    focus: Focus,
    viewer_title: String,
    loaded_entry: Option<String>,
    status_message: String,
    should_quit: bool,
    edit_state: Option<EditState<'a>>,
    has_unsaved_changes: bool,
}

// ---------------------------------------------------------------------------
// Tree building
// ---------------------------------------------------------------------------

fn build_tree(jar: &JarFile) -> Vec<TreeNode> {
    let mut nodes: Vec<TreeNode> = Vec::new();
    let mut dir_indices: BTreeMap<String, usize> = BTreeMap::new();

    for name in jar.entry_names() {
        let parts: Vec<&str> = name.split('/').collect();

        // Ensure all ancestor directories exist
        let mut accumulated = String::new();
        for (i, &part) in parts.iter().enumerate() {
            if i < parts.len() - 1 {
                // directory component
                if !accumulated.is_empty() {
                    accumulated.push('/');
                }
                accumulated.push_str(part);
                let dir_key = accumulated.clone();
                if !dir_indices.contains_key(&dir_key) {
                    let idx = nodes.len();
                    nodes.push(TreeNode {
                        label: format!("{}/", part),
                        entry_path: None,
                        depth: i,
                        expanded: true,
                        is_dir: true,
                    });
                    dir_indices.insert(dir_key, idx);
                }
            }
        }

        // Leaf file entry
        let depth = parts.len() - 1;
        nodes.push(TreeNode {
            label: parts.last().unwrap_or(&name).to_string(),
            entry_path: Some(name.to_string()),
            depth,
            expanded: false,
            is_dir: false,
        });
    }

    nodes
}

fn visible_indices(tree: &[TreeNode]) -> Vec<usize> {
    let mut visible = Vec::new();
    let mut skip_depth: Option<usize> = None;

    for (i, node) in tree.iter().enumerate() {
        if let Some(sd) = skip_depth {
            if node.depth > sd {
                continue;
            } else {
                skip_depth = None;
            }
        }
        visible.push(i);
        if node.is_dir && !node.expanded {
            skip_depth = Some(node.depth);
        }
    }
    visible
}

// ---------------------------------------------------------------------------
// Content formatters
// ---------------------------------------------------------------------------

fn get_utf8(const_pool: &[ConstantInfo], index: u16) -> String {
    if index == 0 {
        return "<none>".to_string();
    }
    match const_pool.get((index - 1) as usize) {
        Some(ConstantInfo::Utf8(u)) => u.utf8_string.clone(),
        _ => format!("#{index}"),
    }
}

fn get_class_name(const_pool: &[ConstantInfo], index: u16) -> String {
    if index == 0 {
        return "<none>".to_string();
    }
    match const_pool.get((index - 1) as usize) {
        Some(ConstantInfo::Class(c)) => get_utf8(const_pool, c.name_index),
        _ => format!("#{index}"),
    }
}

fn get_name_and_type(const_pool: &[ConstantInfo], index: u16) -> (String, String) {
    match const_pool.get((index - 1) as usize) {
        Some(ConstantInfo::NameAndType(nat)) => (
            get_utf8(const_pool, nat.name_index),
            get_utf8(const_pool, nat.descriptor_index),
        ),
        _ => (format!("#{index}"), String::new()),
    }
}

fn resolve_ref(const_pool: &[ConstantInfo], class_idx: u16, nat_idx: u16) -> String {
    let class = get_class_name(const_pool, class_idx);
    let (name, desc) = get_name_and_type(const_pool, nat_idx);
    format!("{class}.{name}:{desc}")
}

fn format_method_access(flags: MethodAccessFlags) -> String {
    let mut parts = Vec::new();
    if flags.contains(MethodAccessFlags::PUBLIC) {
        parts.push("public");
    }
    if flags.contains(MethodAccessFlags::PRIVATE) {
        parts.push("private");
    }
    if flags.contains(MethodAccessFlags::PROTECTED) {
        parts.push("protected");
    }
    if flags.contains(MethodAccessFlags::STATIC) {
        parts.push("static");
    }
    if flags.contains(MethodAccessFlags::FINAL) {
        parts.push("final");
    }
    if flags.contains(MethodAccessFlags::SYNCHRONIZED) {
        parts.push("synchronized");
    }
    if flags.contains(MethodAccessFlags::NATIVE) {
        parts.push("native");
    }
    if flags.contains(MethodAccessFlags::ABSTRACT) {
        parts.push("abstract");
    }
    parts.join(" ")
}

fn format_field_access(flags: FieldAccessFlags) -> String {
    let mut parts = Vec::new();
    if flags.contains(FieldAccessFlags::PUBLIC) {
        parts.push("public");
    }
    if flags.contains(FieldAccessFlags::PRIVATE) {
        parts.push("private");
    }
    if flags.contains(FieldAccessFlags::PROTECTED) {
        parts.push("protected");
    }
    if flags.contains(FieldAccessFlags::STATIC) {
        parts.push("static");
    }
    if flags.contains(FieldAccessFlags::FINAL) {
        parts.push("final");
    }
    if flags.contains(FieldAccessFlags::VOLATILE) {
        parts.push("volatile");
    }
    if flags.contains(FieldAccessFlags::TRANSIENT) {
        parts.push("transient");
    }
    parts.join(" ")
}

fn format_class_access(flags: ClassAccessFlags) -> String {
    let mut parts = Vec::new();
    if flags.contains(ClassAccessFlags::PUBLIC) {
        parts.push("public");
    }
    if flags.contains(ClassAccessFlags::FINAL) {
        parts.push("final");
    }
    if flags.contains(ClassAccessFlags::ABSTRACT) {
        parts.push("abstract");
    }
    if flags.contains(ClassAccessFlags::INTERFACE) {
        parts.push("interface");
    }
    if flags.contains(ClassAccessFlags::ENUM) {
        parts.push("enum");
    }
    if flags.contains(ClassAccessFlags::ANNOTATION) {
        parts.push("annotation");
    }
    if flags.contains(ClassAccessFlags::MODULE) {
        parts.push("module");
    }
    if flags.contains(ClassAccessFlags::SYNTHETIC) {
        parts.push("synthetic");
    }
    parts.join(" ")
}

fn descriptor_to_readable(desc: &str) -> String {
    // Simple best-effort conversion of JVM type descriptors to readable form.
    let mut out = String::new();
    let mut chars = desc.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            'B' => out.push_str("byte"),
            'C' => out.push_str("char"),
            'D' => out.push_str("double"),
            'F' => out.push_str("float"),
            'I' => out.push_str("int"),
            'J' => out.push_str("long"),
            'S' => out.push_str("short"),
            'Z' => out.push_str("boolean"),
            'V' => out.push_str("void"),
            '[' => {
                let inner = descriptor_to_readable(&chars.collect::<String>());
                return format!("{out}{inner}[]");
            }
            'L' => {
                let class_name: String = chars.by_ref().take_while(|&ch| ch != ';').collect();
                out.push_str(&class_name.replace('/', "."));
            }
            '(' => out.push('('),
            ')' => out.push(')'),
            _ => {
                out.push(c);
            }
        }
    }
    out
}

fn format_instruction(instr: &Instruction, const_pool: &[ConstantInfo]) -> String {
    match instr {
        // Invoke instructions — resolve to symbolic names
        Instruction::Invokevirtual(idx) => {
            if let Some(ConstantInfo::MethodRef(mr)) = const_pool.get((*idx - 1) as usize) {
                format!(
                    "invokevirtual {}",
                    resolve_ref(const_pool, mr.class_index, mr.name_and_type_index)
                )
            } else {
                format!("invokevirtual #{idx}")
            }
        }
        Instruction::Invokespecial(idx) => {
            if let Some(ConstantInfo::MethodRef(mr)) = const_pool.get((*idx - 1) as usize) {
                format!(
                    "invokespecial {}",
                    resolve_ref(const_pool, mr.class_index, mr.name_and_type_index)
                )
            } else {
                format!("invokespecial #{idx}")
            }
        }
        Instruction::Invokestatic(idx) => {
            if let Some(ConstantInfo::MethodRef(mr)) = const_pool.get((*idx - 1) as usize) {
                format!(
                    "invokestatic {}",
                    resolve_ref(const_pool, mr.class_index, mr.name_and_type_index)
                )
            } else {
                format!("invokestatic #{idx}")
            }
        }
        Instruction::Invokeinterface { index, count, .. } => {
            if let Some(ConstantInfo::InterfaceMethodRef(mr)) =
                const_pool.get((*index - 1) as usize)
            {
                format!(
                    "invokeinterface {} count={count}",
                    resolve_ref(const_pool, mr.class_index, mr.name_and_type_index)
                )
            } else {
                format!("invokeinterface #{index} count={count}")
            }
        }
        Instruction::Invokedynamic { index, .. } => {
            if let Some(ConstantInfo::InvokeDynamic(id)) = const_pool.get((*index - 1) as usize) {
                let (name, desc) = get_name_and_type(const_pool, id.name_and_type_index);
                format!(
                    "invokedynamic #{} {name}:{desc}",
                    id.bootstrap_method_attr_index
                )
            } else {
                format!("invokedynamic #{index}")
            }
        }

        // Field access
        Instruction::Getfield(idx)
        | Instruction::Getstatic(idx)
        | Instruction::Putfield(idx)
        | Instruction::Putstatic(idx) => {
            let opname = match instr {
                Instruction::Getfield(_) => "getfield",
                Instruction::Getstatic(_) => "getstatic",
                Instruction::Putfield(_) => "putfield",
                Instruction::Putstatic(_) => "putstatic",
                _ => unreachable!(),
            };
            if let Some(ConstantInfo::FieldRef(fr)) = const_pool.get((*idx - 1) as usize) {
                format!(
                    "{opname} {}",
                    resolve_ref(const_pool, fr.class_index, fr.name_and_type_index)
                )
            } else {
                format!("{opname} #{idx}")
            }
        }

        // Type operations
        Instruction::New(idx) => format!("new {}", get_class_name(const_pool, *idx)),
        Instruction::Checkcast(idx) => format!("checkcast {}", get_class_name(const_pool, *idx)),
        Instruction::Instanceof(idx) => format!("instanceof {}", get_class_name(const_pool, *idx)),
        Instruction::Anewarray(idx) => format!("anewarray {}", get_class_name(const_pool, *idx)),

        // LDC
        Instruction::Ldc(idx) => format!("ldc {}", format_constant(const_pool, *idx as u16)),
        Instruction::LdcW(idx) => format!("ldc_w {}", format_constant(const_pool, *idx)),
        Instruction::Ldc2W(idx) => format!("ldc2_w {}", format_constant(const_pool, *idx)),

        // Fallback: Debug formatting
        other => format!("{:?}", other).to_lowercase(),
    }
}

fn format_constant(const_pool: &[ConstantInfo], index: u16) -> String {
    match const_pool.get((index - 1) as usize) {
        Some(ConstantInfo::String(s)) => {
            format!("\"{}\"", get_utf8(const_pool, s.string_index))
        }
        Some(ConstantInfo::Integer(i)) => format!("{}", i.value),
        Some(ConstantInfo::Float(f)) => format!("{}f", f.value),
        Some(ConstantInfo::Long(l)) => format!("{}L", l.value),
        Some(ConstantInfo::Double(d)) => format!("{}d", d.value),
        Some(ConstantInfo::Class(c)) => format!("class {}", get_utf8(const_pool, c.name_index)),
        Some(ConstantInfo::MethodType(mt)) => {
            format!("methodtype {}", get_utf8(const_pool, mt.descriptor_index))
        }
        _ => format!("#{index}"),
    }
}

fn format_class(jar: &JarFile, path: &str) -> Vec<String> {
    let data = match jar.get_entry(path) {
        Some(d) => d,
        None => return vec![format!("Entry not found: {path}")],
    };

    let cf = match ClassFile::read(&mut Cursor::new(data)) {
        Ok(c) => c,
        Err(e) => return vec![format!("Failed to parse class: {e}")],
    };

    // Try decompilation first
    match classfile_parser::decompile::decompile(&cf) {
        Ok(source) => source.lines().map(|l| l.to_string()).collect(),
        Err(e) => {
            let mut lines = vec![
                format!("// Decompilation failed: {e}"),
                "// Falling back to bytecode view".to_string(),
                String::new(),
            ];
            lines.extend(format_class_bytecode(&cf));
            lines
        }
    }
}

fn format_class_bytecode(cf: &ClassFile) -> Vec<String> {
    let cp = &cf.const_pool;
    let mut lines = Vec::new();

    let this_class = get_class_name(cp, cf.this_class);
    let super_class = get_class_name(cp, cf.super_class);
    let java_version = match cf.major_version {
        45 => "1.1",
        46 => "1.2",
        47 => "1.3",
        48 => "1.4",
        49 => "5",
        50 => "6",
        51 => "7",
        52 => "8",
        53 => "9",
        54 => "10",
        55 => "11",
        56 => "12",
        57 => "13",
        58 => "14",
        59 => "15",
        60 => "16",
        61 => "17",
        62 => "18",
        63 => "19",
        64 => "20",
        65 => "21",
        66 => "22",
        67 => "23",
        68 => "24",
        _ => "?",
    };

    lines.push(format!("=== Class: {} ===", this_class));
    lines.push(format!(
        "Version:  {}.{} (Java {java_version})",
        cf.major_version, cf.minor_version
    ));
    lines.push(format!(
        "Access:   {}",
        format_class_access(cf.access_flags)
    ));
    lines.push(format!("Super:    {super_class}"));

    // Interfaces
    if !cf.interfaces.is_empty() {
        lines.push(String::new());
        lines.push(format!("--- Interfaces ({}) ---", cf.interfaces.len()));
        for &iface in &cf.interfaces {
            lines.push(format!("  {}", get_class_name(cp, iface)));
        }
    }

    // Fields
    if !cf.fields.is_empty() {
        lines.push(String::new());
        lines.push(format!("--- Fields ({}) ---", cf.fields.len()));
        for field in &cf.fields {
            let name = get_utf8(cp, field.name_index);
            let desc = get_utf8(cp, field.descriptor_index);
            let access = format_field_access(field.access_flags);
            lines.push(format!(
                "  {access} {} {name}",
                descriptor_to_readable(&desc)
            ));
        }
    }

    // Methods
    if !cf.methods.is_empty() {
        lines.push(String::new());
        lines.push(format!("--- Methods ({}) ---", cf.methods.len()));
        for method in &cf.methods {
            let name = get_utf8(cp, method.name_index);
            let desc = get_utf8(cp, method.descriptor_index);
            let access = format_method_access(method.access_flags);
            lines.push(format!(
                "  {access} {name}{}",
                descriptor_to_readable(&desc)
            ));

            if let Some(code) = method.code() {
                format_code_body(&mut lines, code, cp);
            }
        }
    }

    // Constant pool
    lines.push(String::new());
    lines.push(format!("--- Constant Pool ({}) ---", cp.len()));
    for (i, entry) in cp.iter().enumerate() {
        let idx = i + 1;
        let desc = match entry {
            ConstantInfo::Utf8(u) => format!("Utf8 \"{}\"", u.utf8_string),
            ConstantInfo::Integer(v) => format!("Integer {}", v.value),
            ConstantInfo::Float(v) => format!("Float {}", v.value),
            ConstantInfo::Long(v) => format!("Long {}", v.value),
            ConstantInfo::Double(v) => format!("Double {}", v.value),
            ConstantInfo::Class(c) => format!("Class #{}", c.name_index),
            ConstantInfo::String(s) => format!("String #{}", s.string_index),
            ConstantInfo::FieldRef(r) => {
                format!("Fieldref #{}.#{}", r.class_index, r.name_and_type_index)
            }
            ConstantInfo::MethodRef(r) => {
                format!("Methodref #{}.#{}", r.class_index, r.name_and_type_index)
            }
            ConstantInfo::InterfaceMethodRef(r) => format!(
                "InterfaceMethodref #{}.#{}",
                r.class_index, r.name_and_type_index
            ),
            ConstantInfo::NameAndType(n) => {
                format!("NameAndType #{}.#{}", n.name_index, n.descriptor_index)
            }
            ConstantInfo::MethodHandle(h) => {
                format!(
                    "MethodHandle kind={} #{}",
                    h.reference_kind, h.reference_index
                )
            }
            ConstantInfo::MethodType(t) => format!("MethodType #{}", t.descriptor_index),
            ConstantInfo::InvokeDynamic(d) => format!(
                "InvokeDynamic #{}:#{}",
                d.bootstrap_method_attr_index, d.name_and_type_index
            ),
            ConstantInfo::Module(m) => format!("Module #{}", m.name_index),
            ConstantInfo::Package(p) => format!("Package #{}", p.name_index),
            ConstantInfo::Unusable => "  (unusable)".to_string(),
        };
        lines.push(format!("  #{idx:<4} {desc}"));
    }

    // Class-level attributes
    if !cf.attributes.is_empty() {
        lines.push(String::new());
        lines.push(format!("--- Attributes ({}) ---", cf.attributes.len()));
        for attr in &cf.attributes {
            let attr_name = get_utf8(cp, attr.attribute_name_index);
            lines.push(format!("  {attr_name} ({} bytes)", attr.attribute_length));
        }
    }

    lines
}

fn format_code_body(lines: &mut Vec<String>, code: &CodeAttribute, cp: &[ConstantInfo]) {
    lines.push(format!(
        "    max_stack={}, max_locals={}, code_length={}",
        code.max_stack, code.max_locals, code.code_length
    ));

    // Find line number table if present
    let line_table: Option<&LineNumberTableAttribute> =
        code.attributes.iter().find_map(|a| match &a.info_parsed {
            Some(AttributeInfoVariant::LineNumberTable(t)) => Some(t),
            _ => None,
        });

    // Compute per-instruction byte addresses
    let mut address = 0u32;
    for instr in &code.code {
        let line_info = line_table.and_then(|lt| {
            lt.line_number_table
                .iter()
                .find(|e| e.start_pc as u32 == address)
                .map(|e| e.line_number)
        });
        let line_prefix = match line_info {
            Some(ln) => format!("L{ln:<4}"),
            None => "     ".to_string(),
        };
        lines.push(format!(
            "    {line_prefix} {address:04}: {}",
            format_instruction(instr, cp)
        ));
        address += instruction_byte_size(instr, address);
    }

    // Exception table
    if !code.exception_table.is_empty() {
        lines.push(format!(
            "    Exception table ({}):",
            code.exception_table.len()
        ));
        for ExceptionEntry {
            start_pc,
            end_pc,
            handler_pc,
            catch_type,
        } in &code.exception_table
        {
            let catch = if *catch_type == 0 {
                "any".to_string()
            } else {
                get_class_name(cp, *catch_type)
            };
            lines.push(format!(
                "      {start_pc}-{end_pc} -> {handler_pc} catch {catch}"
            ));
        }
    }
}

fn instruction_byte_size(instr: &Instruction, address: u32) -> u32 {
    match instr {
        // 1-byte instructions (no operands)
        Instruction::Nop
        | Instruction::Aconstnull
        | Instruction::Aload0
        | Instruction::Aload1
        | Instruction::Aload2
        | Instruction::Aload3
        | Instruction::Astore0
        | Instruction::Astore1
        | Instruction::Astore2
        | Instruction::Astore3
        | Instruction::Aaload
        | Instruction::Aastore
        | Instruction::Areturn
        | Instruction::Arraylength
        | Instruction::Athrow
        | Instruction::Baload
        | Instruction::Bastore
        | Instruction::Caload
        | Instruction::Castore
        | Instruction::D2f
        | Instruction::D2i
        | Instruction::D2l
        | Instruction::Dadd
        | Instruction::Daload
        | Instruction::Dastore
        | Instruction::Dcmpg
        | Instruction::Dcmpl
        | Instruction::Dconst0
        | Instruction::Dconst1
        | Instruction::Ddiv
        | Instruction::Dload0
        | Instruction::Dload1
        | Instruction::Dload2
        | Instruction::Dload3
        | Instruction::Dmul
        | Instruction::Dneg
        | Instruction::Drem
        | Instruction::Dreturn
        | Instruction::Dstore0
        | Instruction::Dstore1
        | Instruction::Dstore2
        | Instruction::Dstore3
        | Instruction::Dsub
        | Instruction::Dup
        | Instruction::Dupx1
        | Instruction::Dupx2
        | Instruction::Dup2
        | Instruction::Dup2x1
        | Instruction::Dup2x2
        | Instruction::F2d
        | Instruction::F2i
        | Instruction::F2l
        | Instruction::Fadd
        | Instruction::Faload
        | Instruction::Fastore
        | Instruction::Fcmpg
        | Instruction::Fcmpl
        | Instruction::Fconst0
        | Instruction::Fconst1
        | Instruction::Fconst2
        | Instruction::Fdiv
        | Instruction::Fload0
        | Instruction::Fload1
        | Instruction::Fload2
        | Instruction::Fload3
        | Instruction::Fmul
        | Instruction::Fneg
        | Instruction::Frem
        | Instruction::Freturn
        | Instruction::Fstore0
        | Instruction::Fstore1
        | Instruction::Fstore2
        | Instruction::Fstore3
        | Instruction::Fsub
        | Instruction::I2b
        | Instruction::I2c
        | Instruction::I2d
        | Instruction::I2f
        | Instruction::I2l
        | Instruction::I2s
        | Instruction::Iadd
        | Instruction::Iaload
        | Instruction::Iand
        | Instruction::Iastore
        | Instruction::Iconstm1
        | Instruction::Iconst0
        | Instruction::Iconst1
        | Instruction::Iconst2
        | Instruction::Iconst3
        | Instruction::Iconst4
        | Instruction::Iconst5
        | Instruction::Idiv
        | Instruction::Iload0
        | Instruction::Iload1
        | Instruction::Iload2
        | Instruction::Iload3
        | Instruction::Imul
        | Instruction::Ineg
        | Instruction::Ior
        | Instruction::Irem
        | Instruction::Ireturn
        | Instruction::Ishl
        | Instruction::Ishr
        | Instruction::Istore0
        | Instruction::Istore1
        | Instruction::Istore2
        | Instruction::Istore3
        | Instruction::Isub
        | Instruction::Iushr
        | Instruction::Ixor
        | Instruction::L2d
        | Instruction::L2f
        | Instruction::L2i
        | Instruction::Ladd
        | Instruction::Laload
        | Instruction::Land
        | Instruction::Lastore
        | Instruction::Lcmp
        | Instruction::Lconst0
        | Instruction::Lconst1
        | Instruction::Ldiv
        | Instruction::Lload0
        | Instruction::Lload1
        | Instruction::Lload2
        | Instruction::Lload3
        | Instruction::Lmul
        | Instruction::Lneg
        | Instruction::Lor
        | Instruction::Lrem
        | Instruction::Lreturn
        | Instruction::Lshl
        | Instruction::Lshr
        | Instruction::Lstore0
        | Instruction::Lstore1
        | Instruction::Lstore2
        | Instruction::Lstore3
        | Instruction::Lsub
        | Instruction::Lushr
        | Instruction::Lxor
        | Instruction::Monitorenter
        | Instruction::Monitorexit
        | Instruction::Pop
        | Instruction::Pop2
        | Instruction::Return
        | Instruction::Saload
        | Instruction::Sastore
        | Instruction::Swap => 1,

        // 2-byte instructions (1 byte operand)
        Instruction::Aload(_)
        | Instruction::Astore(_)
        | Instruction::Bipush(_)
        | Instruction::Dload(_)
        | Instruction::Dstore(_)
        | Instruction::Fload(_)
        | Instruction::Fstore(_)
        | Instruction::Iload(_)
        | Instruction::Istore(_)
        | Instruction::Ldc(_)
        | Instruction::Lload(_)
        | Instruction::Lstore(_)
        | Instruction::Newarray(_)
        | Instruction::Ret(_) => 2,

        // 3-byte instructions (2 byte operand)
        Instruction::Anewarray(_)
        | Instruction::Checkcast(_)
        | Instruction::Getfield(_)
        | Instruction::Getstatic(_)
        | Instruction::Goto(_)
        | Instruction::IfAcmpeq(_)
        | Instruction::IfAcmpne(_)
        | Instruction::IfIcmpeq(_)
        | Instruction::IfIcmpne(_)
        | Instruction::IfIcmplt(_)
        | Instruction::IfIcmpge(_)
        | Instruction::IfIcmpgt(_)
        | Instruction::IfIcmple(_)
        | Instruction::Ifeq(_)
        | Instruction::Ifne(_)
        | Instruction::Iflt(_)
        | Instruction::Ifge(_)
        | Instruction::Ifgt(_)
        | Instruction::Ifle(_)
        | Instruction::Ifnonnull(_)
        | Instruction::Ifnull(_)
        | Instruction::Instanceof(_)
        | Instruction::Invokespecial(_)
        | Instruction::Invokestatic(_)
        | Instruction::Invokevirtual(_)
        | Instruction::Jsr(_)
        | Instruction::LdcW(_)
        | Instruction::Ldc2W(_)
        | Instruction::New(_)
        | Instruction::Putfield(_)
        | Instruction::Putstatic(_)
        | Instruction::Sipush(_)
        | Instruction::Iinc { .. } => 3,

        // 4-byte instructions
        Instruction::Multianewarray { .. } => 4,

        // 5-byte instructions
        Instruction::GotoW(_)
        | Instruction::JsrW(_)
        | Instruction::Invokedynamic { .. }
        | Instruction::Invokeinterface { .. } => 5,

        // Wide instructions: 2 (magic) + 2 (index) = 4, except IincWide = 6
        Instruction::AloadWide(_)
        | Instruction::AstoreWide(_)
        | Instruction::DloadWide(_)
        | Instruction::DstoreWide(_)
        | Instruction::FloadWide(_)
        | Instruction::FstoreWide(_)
        | Instruction::IloadWide(_)
        | Instruction::IstoreWide(_)
        | Instruction::LloadWide(_)
        | Instruction::LstoreWide(_)
        | Instruction::RetWide(_) => 4,

        Instruction::IincWide { .. } => 6,

        // Variable-length: tableswitch
        Instruction::Tableswitch { low, high, .. } => {
            let padding = (4 - (address + 1) % 4) % 4;
            // 1 (opcode) + padding + 4 (default) + 4 (low) + 4 (high) + 4*(high-low+1)
            1 + padding + 4 + 4 + 4 + 4 * ((*high - *low + 1) as u32)
        }

        // Variable-length: lookupswitch
        Instruction::Lookupswitch { npairs, .. } => {
            let padding = (4 - (address + 1) % 4) % 4;
            // 1 (opcode) + padding + 4 (default) + 4 (npairs) + 8*npairs
            1 + padding + 4 + 4 + 8 * npairs
        }
    }
}

fn format_manifest(data: &[u8]) -> Vec<String> {
    match JarManifest::parse(data) {
        Ok(manifest) => {
            let mut lines = vec!["=== MANIFEST.MF ===".to_string(), String::new()];
            lines.push("Main Attributes:".to_string());
            for (key, value) in manifest.main_attributes.iter() {
                lines.push(format!("  {key}: {value}"));
            }
            for (name, attrs) in &manifest.entries {
                lines.push(String::new());
                lines.push(format!("Section: {name}"));
                for (key, value) in attrs.iter() {
                    lines.push(format!("  {key}: {value}"));
                }
            }
            lines
        }
        Err(e) => {
            let mut lines = vec![format!("Failed to parse manifest: {e}"), String::new()];
            lines.extend(format_text(data));
            lines
        }
    }
}

fn format_nested_jar(path: &str, data: &[u8]) -> Vec<String> {
    match JarFile::from_bytes(data) {
        Ok(nested) => {
            let entry_count = nested.entry_names().count();
            let class_count = nested.class_names().count();
            let mut lines = vec![
                format!("=== Nested JAR: {path} ==="),
                format!("Entries: {entry_count}"),
                format!("Classes: {class_count}"),
            ];

            // Show manifest if present
            if let Ok(Some(manifest)) = nested.manifest() {
                lines.push(String::new());
                lines.push("Manifest:".to_string());
                for (key, value) in manifest.main_attributes.iter() {
                    lines.push(format!("  {key}: {value}"));
                }
            }

            lines.push(String::new());
            lines.push("Entry listing:".to_string());
            for name in nested.entry_names() {
                lines.push(format!("  {name}"));
            }
            lines
        }
        Err(e) => vec![format!("Failed to open nested JAR: {e}")],
    }
}

fn format_text(data: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(data)
        .lines()
        .map(|l| l.to_string())
        .collect()
}

fn format_hex(data: &[u8]) -> Vec<String> {
    let mut lines = Vec::new();
    for (offset, chunk) in data.chunks(16).enumerate() {
        let hex: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
        let ascii: String = chunk
            .iter()
            .map(|&b| {
                if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();

        // Pad hex to fixed width
        let hex_str = if hex.len() < 16 {
            let mut s = hex.join(" ");
            for _ in hex.len()..16 {
                s.push_str("   ");
            }
            s
        } else {
            hex.join(" ")
        };

        lines.push(format!("{:08x}  {hex_str}  |{ascii}|", offset * 16));
    }
    lines
}

fn load_entry_content(jar: &JarFile, path: &str) -> (String, Vec<String>) {
    if path.ends_with(".class") {
        let title = path.to_string();
        let content = format_class(jar, path);
        return (title, content);
    }

    let data = match jar.get_entry(path) {
        Some(d) => d,
        None => return (path.to_string(), vec![format!("Entry not found: {path}")]),
    };

    if path == "META-INF/MANIFEST.MF" || path.ends_with("/MANIFEST.MF") {
        return (path.to_string(), format_manifest(data));
    }

    if path.ends_with(".jar") {
        return (path.to_string(), format_nested_jar(path, data));
    }

    // Try text for common text extensions
    let text_exts = [
        ".properties",
        ".xml",
        ".json",
        ".txt",
        ".yml",
        ".yaml",
        ".md",
        ".html",
        ".css",
        ".js",
        ".MF",
        ".idx",
        ".factories",
        ".imports",
        ".cfg",
        ".conf",
        ".toml",
        ".ini",
        ".sql",
        ".sh",
        ".bat",
        ".gradle",
        ".kt",
        ".java",
        ".scala",
        ".groovy",
    ];

    if text_exts.iter().any(|ext| path.ends_with(ext)) {
        return (path.to_string(), format_text(data));
    }

    // Heuristic: try UTF-8, fall back to hex
    if let Ok(text) = std::str::from_utf8(data) {
        if text
            .chars()
            .take(512)
            .all(|c| !c.is_control() || c == '\n' || c == '\r' || c == '\t')
        {
            return (
                path.to_string(),
                text.lines().map(|l| l.to_string()).collect(),
            );
        }
    }

    (format!("{path} (hex)"), format_hex(data))
}

fn extract_methods(jar: &JarFile, entry_path: &str) -> Vec<(String, String)> {
    let data = match jar.get_entry(entry_path) {
        Some(d) => d,
        None => return Vec::new(),
    };
    let cf = match ClassFile::read(&mut Cursor::new(data)) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    cf.methods
        .iter()
        .map(|m| {
            let name = get_utf8(&cf.const_pool, m.name_index);
            let desc = get_utf8(&cf.const_pool, m.descriptor_index);
            let access = format_method_access(m.access_flags);
            let display = format!("{} {}{}", access, name, descriptor_to_readable(&desc));
            (name, display.trim_start().to_string())
        })
        .collect()
}

// ---------------------------------------------------------------------------
// App implementation
// ---------------------------------------------------------------------------

impl<'a> App<'a> {
    fn new(jar: JarFile, jar_path: String) -> Self {
        let spring_format = detect_format(&jar);
        let tree = build_tree(&jar);
        let mut viewer = TextArea::default();
        viewer.set_cursor_line_style(Style::default());
        viewer.set_line_number_style(Style::default().fg(Color::DarkGray));

        let mut app = App {
            jar,
            jar_path,
            spring_format,
            tree,
            tree_selected: 0,
            tree_scroll: 0,
            viewer,
            vim_mode: VimMode::Normal,
            search_buffer: String::new(),
            focus: Focus::Tree,
            viewer_title: "Viewer".to_string(),
            loaded_entry: None,
            status_message: String::new(),
            should_quit: false,
            edit_state: None,
            has_unsaved_changes: false,
        };

        // Build initial status
        app.update_status();
        app
    }

    fn update_status(&mut self) {
        // Edit mode status takes priority
        match &self.edit_state {
            Some(EditState::SelectMethod { .. }) => {
                self.status_message =
                    " SELECT METHOD | j/k:navigate  Enter:select  Esc:cancel".to_string();
                return;
            }
            Some(EditState::EditCode { .. }) => {
                self.status_message =
                    " EDIT | Ctrl+S:replace  Ctrl+P:prepend  Esc:cancel".to_string();
                return;
            }
            None => {}
        }

        let mode_str = match &self.vim_mode {
            VimMode::Normal => "NORMAL",
            VimMode::Search => "SEARCH",
            VimMode::Pending(_) => "PENDING",
        };
        let focus_str = match self.focus {
            Focus::Tree => "TREE",
            Focus::Viewer => "VIEW",
        };
        let spring = match self.spring_format {
            Some(SpringBootFormat::Jar) => " [Spring Boot JAR]",
            Some(SpringBootFormat::War) => " [Spring Boot WAR]",
            None => "",
        };
        let modified = if self.has_unsaved_changes {
            " [modified]"
        } else {
            ""
        };
        self.status_message = format!(
            " {mode_str} | {focus_str}{spring}{modified} | Tab:switch  e:edit  W:save  hjkl:move  q:quit"
        );
    }

    fn load_selected_entry(&mut self) {
        let vis = visible_indices(&self.tree);
        if vis.is_empty() {
            return;
        }
        let idx = vis[self.tree_selected.min(vis.len() - 1)];
        let node = &self.tree[idx];

        if node.is_dir {
            return;
        }

        let path = match &node.entry_path {
            Some(p) => p.clone(),
            None => return,
        };

        // Skip if already loaded
        if self.loaded_entry.as_deref() == Some(&path) {
            self.focus = Focus::Viewer;
            self.update_status();
            return;
        }

        let (title, content) = load_entry_content(&self.jar, &path);
        self.viewer_title = title;
        self.loaded_entry = Some(path);

        // Load content into TextArea
        let lines: Vec<String> = if content.is_empty() {
            vec!["(empty)".to_string()]
        } else {
            content
        };
        self.viewer = TextArea::new(lines);
        self.viewer
            .set_cursor_line_style(Style::default().bg(Color::DarkGray));
        self.viewer
            .set_line_number_style(Style::default().fg(Color::DarkGray));

        self.focus = Focus::Viewer;
        self.vim_mode = VimMode::Normal;
        self.update_status();
    }

    fn toggle_dir(&mut self) {
        let vis = visible_indices(&self.tree);
        if vis.is_empty() {
            return;
        }
        let idx = vis[self.tree_selected.min(vis.len() - 1)];
        if self.tree[idx].is_dir {
            self.tree[idx].expanded = !self.tree[idx].expanded;
        }
    }

    fn expand_dir(&mut self) {
        let vis = visible_indices(&self.tree);
        if vis.is_empty() {
            return;
        }
        let idx = vis[self.tree_selected.min(vis.len() - 1)];
        if self.tree[idx].is_dir {
            self.tree[idx].expanded = true;
        }
    }

    fn collapse_dir(&mut self) {
        let vis = visible_indices(&self.tree);
        if vis.is_empty() {
            return;
        }
        let idx = vis[self.tree_selected.min(vis.len() - 1)];
        if self.tree[idx].is_dir {
            self.tree[idx].expanded = false;
        } else {
            // Find parent directory and collapse it
            let node_depth = self.tree[idx].depth;
            if node_depth > 0 {
                // Walk backwards through visible items to find parent
                if self.tree_selected > 0 {
                    for check in (0..self.tree_selected).rev() {
                        let check_idx = vis[check];
                        if self.tree[check_idx].is_dir && self.tree[check_idx].depth < node_depth {
                            self.tree[check_idx].expanded = false;
                            self.tree_selected = check;
                            break;
                        }
                    }
                }
            }
        }
    }

    fn enter_edit_mode(&mut self) {
        let entry_path = match &self.loaded_entry {
            Some(p) if p.ends_with(".class") => p.clone(),
            _ => return,
        };
        let methods = extract_methods(&self.jar, &entry_path);
        if methods.is_empty() {
            self.status_message = " No methods found in class".to_string();
            return;
        }
        self.edit_state = Some(EditState::SelectMethod {
            entry_path,
            methods,
            selected: 0,
        });
        self.update_status();
    }

    fn apply_edit(&mut self, prepend: bool) {
        let (entry_path, method_name, source) = match &self.edit_state {
            Some(EditState::EditCode {
                entry_path,
                method_name,
                editor,
                ..
            }) => {
                let src = editor.lines().join("\n");
                (entry_path.clone(), method_name.clone(), src)
            }
            _ => return,
        };

        let data = match self.jar.get_entry(&entry_path) {
            Some(d) => d.to_vec(),
            None => {
                if let Some(EditState::EditCode { error_message, .. }) = &mut self.edit_state {
                    *error_message = Some(format!("Entry not found: {entry_path}"));
                }
                return;
            }
        };

        let mut cf = match ClassFile::read(&mut Cursor::new(&data)) {
            Ok(c) => c,
            Err(e) => {
                if let Some(EditState::EditCode { error_message, .. }) = &mut self.edit_state {
                    *error_message = Some(format!("Failed to parse class: {e}"));
                }
                return;
            }
        };

        let opts = CompileOptions::default();
        let result = if prepend {
            prepend_method_body(&source, &mut cf, &method_name, None, &opts)
        } else {
            compile_method_body(&source, &mut cf, &method_name, None, &opts)
        };

        match result {
            Ok(()) => match cf.to_bytes() {
                Ok(bytes) => {
                    self.jar.set_entry(&entry_path, bytes);
                    self.has_unsaved_changes = true;
                    let action = if prepend { "Prepended to" } else { "Replaced" };
                    self.status_message = format!(" {action} '{method_name}' | W:save JAR");
                    self.edit_state = None;
                    self.loaded_entry = None;
                    self.load_selected_entry();
                }
                Err(e) => {
                    if let Some(EditState::EditCode { error_message, .. }) = &mut self.edit_state {
                        *error_message = Some(format!("Failed to serialize class: {e}"));
                    }
                }
            },
            Err(e) => {
                if let Some(EditState::EditCode { error_message, .. }) = &mut self.edit_state {
                    *error_message = Some(format!("{e}"));
                }
            }
        }
    }

    fn save_jar(&mut self) {
        if !self.has_unsaved_changes {
            self.status_message = " No unsaved changes".to_string();
            return;
        }
        let output_path = if self.jar_path.ends_with(".jar") {
            self.jar_path.replace(".jar", ".patched.jar")
        } else {
            format!("{}.patched", self.jar_path)
        };
        match self.jar.save(&output_path) {
            Ok(()) => {
                self.has_unsaved_changes = false;
                self.status_message = format!(" Saved to {output_path}");
            }
            Err(e) => {
                self.status_message = format!(" Save failed: {e}");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Key handling
// ---------------------------------------------------------------------------

fn handle_key_event(app: &mut App, key: KeyEvent) {
    // Global shortcuts
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    // Edit mode intercepts all input
    if app.edit_state.is_some() {
        handle_edit_input(app, key);
        app.update_status();
        return;
    }

    match app.focus {
        Focus::Tree => handle_tree_input(app, key),
        Focus::Viewer => handle_vim_input(app, key),
    }
    app.update_status();
}

fn handle_tree_input(app: &mut App, key: KeyEvent) {
    let vis = visible_indices(&app.tree);
    if vis.is_empty() {
        return;
    }
    let max = vis.len().saturating_sub(1);

    match &app.vim_mode {
        VimMode::Pending('g') => {
            app.vim_mode = VimMode::Normal;
            if key.code == KeyCode::Char('g') {
                app.tree_selected = 0;
            }
            return;
        }
        VimMode::Pending(_) => {
            app.vim_mode = VimMode::Normal;
            return;
        }
        _ => {}
    }

    match key.code {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Char('j') | KeyCode::Down => {
            app.tree_selected = (app.tree_selected + 1).min(max);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.tree_selected = app.tree_selected.saturating_sub(1);
        }
        KeyCode::Char('l') | KeyCode::Right | KeyCode::Char(' ') => {
            let idx = vis[app.tree_selected.min(max)];
            if app.tree[idx].is_dir {
                app.expand_dir();
            } else {
                app.load_selected_entry();
            }
        }
        KeyCode::Char('h') | KeyCode::Left => app.collapse_dir(),
        KeyCode::Enter => {
            let idx = vis[app.tree_selected.min(max)];
            if app.tree[idx].is_dir {
                app.toggle_dir();
            } else {
                app.load_selected_entry();
            }
        }
        KeyCode::Char('g') => {
            app.vim_mode = VimMode::Pending('g');
        }
        KeyCode::Char('G') => {
            app.tree_selected = max;
        }
        KeyCode::Char('e') => {
            app.enter_edit_mode();
        }
        KeyCode::Char('W') => {
            app.save_jar();
        }
        KeyCode::Tab => {
            if app.loaded_entry.is_some() {
                app.focus = Focus::Viewer;
            }
        }
        _ => {}
    }
}

fn handle_edit_input(app: &mut App, key: KeyEvent) {
    // Determine which edit sub-state we're in
    let is_select = matches!(app.edit_state, Some(EditState::SelectMethod { .. }));

    if is_select {
        handle_edit_select(app, key);
    } else {
        handle_edit_code(app, key);
    }
}

fn handle_edit_select(app: &mut App, key: KeyEvent) {
    let method_count = match &app.edit_state {
        Some(EditState::SelectMethod { methods, .. }) => methods.len(),
        _ => return,
    };
    let max = method_count.saturating_sub(1);

    match key.code {
        KeyCode::Char('j') | KeyCode::Down => {
            if let Some(EditState::SelectMethod { selected, .. }) = &mut app.edit_state {
                *selected = (*selected + 1).min(max);
            }
        }
        KeyCode::Char('k') | KeyCode::Up => {
            if let Some(EditState::SelectMethod { selected, .. }) = &mut app.edit_state {
                *selected = selected.saturating_sub(1);
            }
        }
        KeyCode::Enter => {
            // Transition to EditCode
            if let Some(EditState::SelectMethod {
                entry_path,
                methods,
                selected,
            }) = app.edit_state.take()
            {
                let (method_name, _) = &methods[selected];
                let mut editor =
                    TextArea::new(vec!["{ ".to_string(), "  ".to_string(), "}".to_string()]);
                editor.set_cursor_line_style(Style::default().bg(Color::DarkGray));
                editor.set_cursor_style(Style::default().bg(Color::White).fg(Color::Black));
                editor.set_line_number_style(Style::default().fg(Color::DarkGray));
                // Position cursor on the middle line
                editor.move_cursor(CursorMove::Down);
                editor.move_cursor(CursorMove::End);

                app.edit_state = Some(EditState::EditCode {
                    entry_path,
                    method_name: method_name.clone(),
                    editor,
                    error_message: None,
                });
            }
        }
        KeyCode::Esc => {
            app.edit_state = None;
        }
        _ => {}
    }
}

fn handle_edit_code(app: &mut App, key: KeyEvent) {
    // Ctrl+S → replace, Ctrl+P → prepend, Escape → cancel
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('s') => {
                app.apply_edit(false);
                return;
            }
            KeyCode::Char('p') => {
                app.apply_edit(true);
                return;
            }
            _ => {}
        }
    }

    if key.code == KeyCode::Esc {
        app.edit_state = None;
        return;
    }

    // Forward to TextArea editor
    if let Some(EditState::EditCode {
        editor,
        error_message,
        ..
    }) = &mut app.edit_state
    {
        editor.input(key);
        // Clear error on new input
        if error_message.is_some() {
            *error_message = None;
        }
    }
}

fn handle_vim_input(app: &mut App, key: KeyEvent) {
    match &app.vim_mode {
        VimMode::Normal => handle_vim_normal(app, key),
        VimMode::Search => handle_vim_search(app, key),
        VimMode::Pending(c) => {
            let c = *c;
            handle_vim_pending(app, key, c);
        }
    }
}

fn handle_vim_normal(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Tab => {
            app.focus = Focus::Tree;
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.viewer.move_cursor(CursorMove::Down);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.viewer.move_cursor(CursorMove::Up);
        }
        KeyCode::Char('h') | KeyCode::Left => {
            app.viewer.move_cursor(CursorMove::Back);
        }
        KeyCode::Char('l') | KeyCode::Right => {
            app.viewer.move_cursor(CursorMove::Forward);
        }
        KeyCode::Char('w') => {
            app.viewer.move_cursor(CursorMove::WordForward);
        }
        KeyCode::Char('b') => {
            app.viewer.move_cursor(CursorMove::WordBack);
        }
        KeyCode::Char('0') => {
            app.viewer.move_cursor(CursorMove::Head);
        }
        KeyCode::Char('$') => {
            app.viewer.move_cursor(CursorMove::End);
        }
        KeyCode::Char('g') => {
            app.vim_mode = VimMode::Pending('g');
        }
        KeyCode::Char('G') => {
            app.viewer.move_cursor(CursorMove::Bottom);
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.viewer.scroll((10, 0));
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.viewer.scroll((-10, 0));
        }
        KeyCode::Char('/') => {
            app.vim_mode = VimMode::Search;
            app.search_buffer.clear();
        }
        KeyCode::Char('n') => {
            app.viewer.search_forward(false);
        }
        KeyCode::Char('N') => {
            app.viewer.search_back(false);
        }
        _ => {}
    }
}

fn handle_vim_search(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.vim_mode = VimMode::Normal;
            app.search_buffer.clear();
        }
        KeyCode::Enter => {
            if !app.search_buffer.is_empty() {
                app.viewer.set_search_pattern(&app.search_buffer).ok();
                app.viewer.search_forward(false);
            }
            app.vim_mode = VimMode::Normal;
        }
        KeyCode::Backspace => {
            app.search_buffer.pop();
        }
        KeyCode::Char(c) => {
            app.search_buffer.push(c);
        }
        _ => {}
    }
}

fn handle_vim_pending(app: &mut App, key: KeyEvent, pending: char) {
    app.vim_mode = VimMode::Normal;
    if pending == 'g' && key.code == KeyCode::Char('g') {
        app.viewer.move_cursor(CursorMove::Top);
    }
    // Any other combo just cancels the pending state
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render(app: &mut App, frame: &mut ratatui::Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(frame.area());

    let main_area = chunks[0];
    let status_area = chunks[1];

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(main_area);

    render_tree(app, frame, main_chunks[0]);
    render_viewer(app, frame, main_chunks[1]);
    render_status(app, frame, status_area);
}

fn render_tree(app: &mut App, frame: &mut ratatui::Frame, area: Rect) {
    let vis = visible_indices(&app.tree);
    let selected = app.tree_selected.min(vis.len().saturating_sub(1));

    // Adjust scroll to keep selected visible
    let inner_height = area.height.saturating_sub(2) as usize; // account for borders
    if selected < app.tree_scroll {
        app.tree_scroll = selected;
    } else if selected >= app.tree_scroll + inner_height {
        app.tree_scroll = selected - inner_height + 1;
    }

    let items: Vec<ListItem> = vis
        .iter()
        .enumerate()
        .skip(app.tree_scroll)
        .take(inner_height)
        .map(|(i, &idx)| {
            let node = &app.tree[idx];
            let indent = "  ".repeat(node.depth);
            let icon = if node.is_dir {
                if node.expanded { "[-] " } else { "[+] " }
            } else if node.label.ends_with(".class") {
                "> "
            } else {
                "  "
            };

            let color = if node.is_dir {
                Color::Yellow
            } else if node.label.ends_with(".class") {
                Color::Cyan
            } else if node.label.ends_with(".jar") {
                Color::Green
            } else {
                Color::White
            };

            let style = if i == selected {
                if app.focus == Focus::Tree {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(color).bg(Color::DarkGray)
                }
            } else {
                Style::default().fg(color)
            };

            ListItem::new(Line::from(Span::styled(
                format!("{indent}{icon}{}", node.label),
                style,
            )))
        })
        .collect();

    let title = match app.spring_format {
        Some(SpringBootFormat::Jar) => " JAR Explorer [Spring Boot] ",
        Some(SpringBootFormat::War) => " JAR Explorer [Spring Boot WAR] ",
        None => " JAR Explorer ",
    };

    let tree_block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if app.focus == Focus::Tree {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        });

    let list = List::new(items).block(tree_block);
    frame.render_widget(list, area);
}

fn render_viewer(app: &mut App, frame: &mut ratatui::Frame, area: Rect) {
    // Edit mode rendering
    match &mut app.edit_state {
        Some(EditState::SelectMethod {
            methods, selected, ..
        }) => {
            render_method_selector(frame, area, methods, *selected);
            return;
        }
        Some(EditState::EditCode {
            method_name,
            editor,
            error_message,
            ..
        }) => {
            render_code_editor(frame, area, method_name, editor, error_message.as_deref());
            return;
        }
        None => {}
    }

    let title = if app.vim_mode == VimMode::Search {
        format!(" {} | /{} ", app.viewer_title, app.search_buffer)
    } else {
        format!(" {} ", app.viewer_title)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(if app.focus == Focus::Viewer {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        });

    app.viewer.set_block(block);

    if app.focus == Focus::Viewer {
        app.viewer
            .set_cursor_line_style(Style::default().bg(Color::DarkGray));
        app.viewer
            .set_cursor_style(Style::default().bg(Color::White).fg(Color::Black));
    } else {
        app.viewer.set_cursor_line_style(Style::default());
        app.viewer.set_cursor_style(Style::default());
    }

    frame.render_widget(&app.viewer, area);
}

fn render_method_selector(
    frame: &mut ratatui::Frame,
    area: Rect,
    methods: &[(String, String)],
    selected: usize,
) {
    let inner_height = area.height.saturating_sub(2) as usize;
    let scroll = if selected >= inner_height {
        selected - inner_height + 1
    } else {
        0
    };

    let items: Vec<ListItem> = methods
        .iter()
        .enumerate()
        .skip(scroll)
        .take(inner_height)
        .map(|(i, (_, display))| {
            let style = if i == selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            ListItem::new(Line::from(Span::styled(format!("  {display}"), style)))
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Select Method ")
        .border_style(Style::default().fg(Color::Yellow));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn render_code_editor(
    frame: &mut ratatui::Frame,
    area: Rect,
    method_name: &str,
    editor: &mut TextArea,
    error_message: Option<&str>,
) {
    if let Some(err) = error_message {
        // Split: editor on top, error on bottom
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(3), Constraint::Length(3)])
            .split(area);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Edit: {method_name} "))
            .border_style(Style::default().fg(Color::Green));
        editor.set_block(block);
        editor.set_cursor_line_style(Style::default().bg(Color::DarkGray));
        editor.set_cursor_style(Style::default().bg(Color::White).fg(Color::Black));
        editor.set_line_number_style(Style::default().fg(Color::DarkGray));
        frame.render_widget(&*editor, chunks[0]);

        let error_block = Block::default()
            .borders(Borders::ALL)
            .title(" Error ")
            .border_style(Style::default().fg(Color::Red));
        let error_text = Paragraph::new(Line::from(Span::styled(
            err,
            Style::default().fg(Color::Red),
        )))
        .block(error_block);
        frame.render_widget(error_text, chunks[1]);
    } else {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Edit: {method_name} "))
            .border_style(Style::default().fg(Color::Green));
        editor.set_block(block);
        editor.set_cursor_line_style(Style::default().bg(Color::DarkGray));
        editor.set_cursor_style(Style::default().bg(Color::White).fg(Color::Black));
        editor.set_line_number_style(Style::default().fg(Color::DarkGray));
        frame.render_widget(&*editor, area);
    }
}

fn render_status(app: &App, frame: &mut ratatui::Frame, area: Rect) {
    let status = Paragraph::new(Line::from(Span::styled(
        &app.status_message,
        Style::default().fg(Color::Black).bg(Color::White),
    )));
    frame.render_widget(status, area);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: jar_explorer <path-to-jar>");
        std::process::exit(1);
    }
    let path = &args[1];

    let jar = JarFile::open(path).map_err(|e| format!("Failed to open {path}: {e}"))?;

    // Terminal setup
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(jar, path.clone());

    // Main loop
    loop {
        terminal.draw(|frame| render(&mut app, frame))?;

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                handle_key_event(&mut app, key);
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Cleanup
    terminal::disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
