pub mod lexer;
pub mod ast;
pub mod parser;
pub mod codegen;
pub mod stack_calc;
pub mod stackmap;
pub mod patch;

use std::fmt;

use crate::attribute_info::{ExceptionEntry, StackMapTableAttribute};
use crate::code_attribute::Instruction;
use crate::ClassFile;

use self::ast::CStmt;
use self::codegen::CodeGenerator;
use self::lexer::Lexer;
use self::parser::Parser;

#[derive(Clone, Debug)]
pub enum CompileError {
    ParseError {
        line: usize,
        column: usize,
        message: String,
    },
    TypeError {
        message: String,
    },
    CodegenError {
        message: String,
    },
    MethodNotFound {
        name: String,
    },
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileError::ParseError {
                line,
                column,
                message,
            } => write!(f, "parse error at {}:{}: {}", line, column, message),
            CompileError::TypeError { message } => write!(f, "type error: {}", message),
            CompileError::CodegenError { message } => write!(f, "codegen error: {}", message),
            CompileError::MethodNotFound { name } => write!(f, "method not found: {}", name),
        }
    }
}

impl std::error::Error for CompileError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertMode {
    /// Replace the entire method body (default).
    Replace,
    /// Insert compiled code at the beginning, preserving the original body.
    Prepend,
    /// Insert compiled code at the end, after the original body.
    ///
    /// The trailing return instruction(s) of the original method are stripped
    /// so the original code falls through to the appended code. The appended
    /// code is responsible for returning.
    Append,
}

impl Default for InsertMode {
    fn default() -> Self {
        InsertMode::Replace
    }
}

#[derive(Clone)]
pub struct CompileOptions {
    pub strip_stack_map_table: bool,
    pub generate_stack_map_table: bool,
    pub insert_mode: InsertMode,
}

impl Default for CompileOptions {
    fn default() -> Self {
        CompileOptions {
            strip_stack_map_table: false,
            generate_stack_map_table: true,
            insert_mode: InsertMode::Replace,
        }
    }
}

pub struct GeneratedCode {
    pub instructions: Vec<Instruction>,
    pub max_stack: u16,
    pub max_locals: u16,
    pub exception_table: Vec<ExceptionEntry>,
    pub stack_map_table: Option<StackMapTableAttribute>,
}

/// Parse a Java method body into AST statements.
pub fn parse_method_body(source: &str) -> Result<Vec<CStmt>, CompileError> {
    let lexer = Lexer::new(source);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    parser.parse_method_body()
}

/// Generate bytecode from AST statements.
pub fn generate_bytecode(
    stmts: &[CStmt],
    class_file: &mut ClassFile,
    is_static: bool,
    method_descriptor: &str,
) -> Result<GeneratedCode, CompileError> {
    generate_bytecode_with_options(stmts, class_file, is_static, method_descriptor, false)
}

/// Generate bytecode from AST statements with options.
pub fn generate_bytecode_with_options(
    stmts: &[CStmt],
    class_file: &mut ClassFile,
    is_static: bool,
    method_descriptor: &str,
    generate_stack_map_table: bool,
) -> Result<GeneratedCode, CompileError> {
    let mut codegen =
        CodeGenerator::new_with_options(class_file, is_static, method_descriptor, generate_stack_map_table, &[])?;
    codegen.generate_body(stmts)?;
    codegen.finish()
}

/// Compile Java source and replace a method's body in the class file.
///
/// When `method_descriptor` is `Some`, the method is matched by both name and
/// descriptor, disambiguating overloaded methods. When `None`, the first method
/// with the given name is used (legacy behavior).
pub fn compile_method_body(
    source: &str,
    class_file: &mut ClassFile,
    method_name: &str,
    method_descriptor: Option<&str>,
    options: &CompileOptions,
) -> Result<(), CompileError> {
    patch::compile_method_body_impl(source, class_file, method_name, method_descriptor, options)
}

/// Compile Java source and append it after an existing method body.
///
/// The trailing return instructions of the original body are stripped
/// so the original code falls through to the appended code.
pub fn append_method_body(
    source: &str,
    class_file: &mut ClassFile,
    method_name: &str,
    method_descriptor: Option<&str>,
    options: &CompileOptions,
) -> Result<(), CompileError> {
    let mut opts = options.clone();
    opts.insert_mode = InsertMode::Append;
    patch::compile_method_body_impl(source, class_file, method_name, method_descriptor, &opts)
}

/// Compile Java source and prepend it to an existing method body.
///
/// The compiled code is inserted before the original instructions.
/// Trailing return instructions are stripped so the prepended code
/// falls through to the original body.
pub fn prepend_method_body(
    source: &str,
    class_file: &mut ClassFile,
    method_name: &str,
    method_descriptor: Option<&str>,
    options: &CompileOptions,
) -> Result<(), CompileError> {
    let mut opts = options.clone();
    opts.insert_mode = InsertMode::Prepend;
    patch::compile_method_body_impl(source, class_file, method_name, method_descriptor, &opts)
}

/// Compile and patch a single method body in a class file.
///
/// Generates a valid StackMapTable by default so the patched class passes
/// full JVM bytecode verification.
///
/// # Forms
///
/// ```ignore
/// // With StackMapTable generation (passes full verification):
/// patch_method!(class_file, "main", r#"{ System.out.println("hello"); }"#)?;
///
/// // Without StackMapTable (requires -noverify or -XX:-BytecodeVerification*):
/// patch_method!(class_file, "main", r#"{ System.out.println("hello"); }"#, no_verify)?;
/// ```
#[macro_export]
macro_rules! patch_method {
    ($class_file:expr, $method:expr, $source:expr) => {
        $crate::compile::compile_method_body(
            $source,
            &mut $class_file,
            $method,
            None,
            &$crate::compile::CompileOptions {
                generate_stack_map_table: true,
                ..$crate::compile::CompileOptions::default()
            },
        )
    };
    ($class_file:expr, $method:expr, $source:expr, no_verify) => {
        $crate::compile::compile_method_body(
            $source,
            &mut $class_file,
            $method,
            None,
            &$crate::compile::CompileOptions::default(),
        )
    };
}

/// Compile and patch multiple method bodies in a class file.
///
/// Each method is compiled and patched in order. If any method fails,
/// the error is returned immediately and subsequent methods are not patched.
///
/// Generates a valid StackMapTable by default.
///
/// ```ignore
/// patch_methods!(class_file, {
///     "main"   => r#"{ System.out.println("hello"); }"#,
///     "helper" => r#"{ return 42; }"#,
/// })?;
///
/// // Without StackMapTable:
/// patch_methods!(class_file, no_verify, {
///     "main" => r#"{ System.out.println("hello"); }"#,
/// })?;
/// ```
#[macro_export]
macro_rules! patch_methods {
    ($class_file:expr, { $($method:expr => $source:expr),+ $(,)? }) => {{
        (|| -> Result<(), $crate::compile::CompileError> {
            $(
                $crate::patch_method!($class_file, $method, $source)?;
            )+
            Ok(())
        })()
    }};
    ($class_file:expr, no_verify, { $($method:expr => $source:expr),+ $(,)? }) => {{
        (|| -> Result<(), $crate::compile::CompileError> {
            $(
                $crate::patch_method!($class_file, $method, $source, no_verify)?;
            )+
            Ok(())
        })()
    }};
}

/// Prepend compiled Java source to the beginning of a method body.
///
/// The original method code is preserved; the new code runs first and
/// falls through to the original instructions.
///
/// ```ignore
/// prepend_method!(class_file, "main", r#"{ System.out.println("entering main"); }"#)?;
/// ```
#[macro_export]
macro_rules! prepend_method {
    ($class_file:expr, $method:expr, $source:expr) => {
        $crate::compile::prepend_method_body(
            $source,
            &mut $class_file,
            $method,
            None,
            &$crate::compile::CompileOptions {
                generate_stack_map_table: true,
                insert_mode: $crate::compile::InsertMode::Prepend,
                ..$crate::compile::CompileOptions::default()
            },
        )
    };
    ($class_file:expr, $method:expr, $source:expr, no_verify) => {
        $crate::compile::prepend_method_body(
            $source,
            &mut $class_file,
            $method,
            None,
            &$crate::compile::CompileOptions {
                insert_mode: $crate::compile::InsertMode::Prepend,
                ..$crate::compile::CompileOptions::default()
            },
        )
    };
}

/// Append compiled Java source after the end of a method body.
///
/// The original method's trailing return is stripped so it falls through
/// to the appended code. The appended code is responsible for returning.
///
/// ```ignore
/// append_method!(class_file, "main", r#"{ System.out.println("exiting main"); }"#)?;
/// ```
#[macro_export]
macro_rules! append_method {
    ($class_file:expr, $method:expr, $source:expr) => {
        $crate::compile::append_method_body(
            $source,
            &mut $class_file,
            $method,
            &$crate::compile::CompileOptions {
                generate_stack_map_table: true,
                insert_mode: $crate::compile::InsertMode::Append,
                ..$crate::compile::CompileOptions::default()
            },
        )
    };
    ($class_file:expr, $method:expr, $source:expr, no_verify) => {
        $crate::compile::append_method_body(
            $source,
            &mut $class_file,
            $method,
            &$crate::compile::CompileOptions {
                insert_mode: $crate::compile::InsertMode::Append,
                ..$crate::compile::CompileOptions::default()
            },
        )
    };
}
