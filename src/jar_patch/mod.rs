use std::fmt;

use crate::compile::{CompileError, CompileOptions, compile_method_body};
use crate::jar_utils::{JarError, JarFile};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum JarPatchError {
    Jar(JarError),
    Compile(CompileError),
}

impl fmt::Display for JarPatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JarPatchError::Jar(e) => write!(f, "jar error: {e}"),
            JarPatchError::Compile(e) => write!(f, "compile error: {e}"),
        }
    }
}

impl std::error::Error for JarPatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            JarPatchError::Jar(e) => Some(e),
            JarPatchError::Compile(e) => Some(e),
        }
    }
}

impl From<JarError> for JarPatchError {
    fn from(e: JarError) -> Self {
        JarPatchError::Jar(e)
    }
}

impl From<CompileError> for JarPatchError {
    fn from(e: CompileError) -> Self {
        JarPatchError::Compile(e)
    }
}

pub type JarPatchResult<T> = Result<T, JarPatchError>;

// ---------------------------------------------------------------------------
// Functions
// ---------------------------------------------------------------------------

/// Compile Java source and replace a single method's body in a class inside
/// a JAR.
///
/// Parses the class from the JAR, patches the specified method with the
/// compiled source, and writes the modified class back.
pub fn patch_jar_method(
    jar: &mut JarFile,
    class_path: &str,
    method_name: &str,
    source: &str,
    options: &CompileOptions,
) -> JarPatchResult<()> {
    let mut class_file = jar.parse_class(class_path)?;
    compile_method_body(source, &mut class_file, method_name, None, options)?;
    jar.set_class(class_path, &class_file)?;
    Ok(())
}

/// Compile and patch multiple methods in a single class inside a JAR.
///
/// The class is parsed once, all methods are patched in order, and the
/// modified class is written back once. If any method fails, the error is
/// returned immediately and the JAR entry is not updated.
pub fn patch_jar_class(
    jar: &mut JarFile,
    class_path: &str,
    patches: &[(&str, &str)],
    options: &CompileOptions,
) -> JarPatchResult<()> {
    let mut class_file = jar.parse_class(class_path)?;
    for &(method_name, source) in patches {
        compile_method_body(source, &mut class_file, method_name, None, options)?;
    }
    jar.set_class(class_path, &class_file)?;
    Ok(())
}

/// Compile and patch methods across multiple classes in a JAR.
///
/// Each entry is `(class_path, &[(method_name, source)])`. Classes are
/// processed one at a time. If any class fails, the error is returned
/// immediately.
pub fn patch_jar_classes(
    jar: &mut JarFile,
    patches: &[(&str, &[(&str, &str)])],
    options: &CompileOptions,
) -> JarPatchResult<()> {
    for &(class_path, method_patches) in patches {
        patch_jar_class(jar, class_path, method_patches, options)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Macros
// ---------------------------------------------------------------------------

/// Compile and patch a single method in a class inside a JAR.
///
/// Generates a valid StackMapTable by default so the patched class passes
/// full JVM bytecode verification.
///
/// # Forms
///
/// ```ignore
/// // With StackMapTable (default):
/// patch_jar_method!(jar, "com/example/Main.class", "main", r#"{ ... }"#)?;
///
/// // Without StackMapTable (requires -noverify):
/// patch_jar_method!(jar, "com/example/Main.class", "main", r#"{ ... }"#, no_verify)?;
/// ```
#[macro_export]
macro_rules! patch_jar_method {
    ($jar:expr, $class_path:expr, $method:expr, $source:expr) => {
        $crate::jar_patch::patch_jar_method(
            &mut $jar,
            $class_path,
            $method,
            $source,
            &$crate::compile::CompileOptions {
                generate_stack_map_table: true,
                ..$crate::compile::CompileOptions::default()
            },
        )
    };
    ($jar:expr, $class_path:expr, $method:expr, $source:expr, no_verify) => {
        $crate::jar_patch::patch_jar_method(
            &mut $jar,
            $class_path,
            $method,
            $source,
            &$crate::compile::CompileOptions::default(),
        )
    };
}

/// Compile and patch multiple methods in a single class inside a JAR.
///
/// The class is parsed once, all methods are patched, and the class is
/// written back once. Generates a valid StackMapTable by default.
///
/// ```ignore
/// patch_jar_class!(jar, "com/example/Main.class", {
///     "main"   => r#"{ System.out.println("hello"); }"#,
///     "helper" => r#"{ return 42; }"#,
/// })?;
///
/// // Without StackMapTable:
/// patch_jar_class!(jar, "com/example/Main.class", no_verify, {
///     "main" => r#"{ ... }"#,
/// })?;
/// ```
#[macro_export]
macro_rules! patch_jar_class {
    ($jar:expr, $class_path:expr, { $($method:expr => $source:expr),+ $(,)? }) => {
        $crate::jar_patch::patch_jar_class(
            &mut $jar,
            $class_path,
            &[ $( ($method, $source) ),+ ],
            &$crate::compile::CompileOptions {
                generate_stack_map_table: true,
                ..$crate::compile::CompileOptions::default()
            },
        )
    };
    ($jar:expr, $class_path:expr, no_verify, { $($method:expr => $source:expr),+ $(,)? }) => {
        $crate::jar_patch::patch_jar_class(
            &mut $jar,
            $class_path,
            &[ $( ($method, $source) ),+ ],
            &$crate::compile::CompileOptions::default(),
        )
    };
}

/// Compile and patch methods across multiple classes in a JAR.
///
/// Each class is parsed once, its methods are patched, and the class is
/// written back before moving to the next. Generates a valid StackMapTable
/// by default.
///
/// ```ignore
/// patch_jar!(jar, {
///     "com/example/Main.class" => {
///         "main"   => r#"{ System.out.println("hello"); }"#,
///         "helper" => r#"{ return 42; }"#,
///     },
///     "com/example/Util.class" => {
///         "compute" => r#"{ return 0; }"#,
///     },
/// })?;
///
/// // Without StackMapTable:
/// patch_jar!(jar, no_verify, {
///     "com/example/Main.class" => {
///         "main" => r#"{ ... }"#,
///     },
/// })?;
/// ```
#[macro_export]
macro_rules! patch_jar {
    ($jar:expr, { $($class_path:expr => { $($method:expr => $source:expr),+ $(,)? }),+ $(,)? }) => {{
        (|| -> Result<(), $crate::jar_patch::JarPatchError> {
            $(
                $crate::jar_patch::patch_jar_class(
                    &mut $jar,
                    $class_path,
                    &[ $( ($method, $source) ),+ ],
                    &$crate::compile::CompileOptions {
                        generate_stack_map_table: true,
                        ..$crate::compile::CompileOptions::default()
                    },
                )?;
            )+
            Ok(())
        })()
    }};
    ($jar:expr, no_verify, { $($class_path:expr => { $($method:expr => $source:expr),+ $(,)? }),+ $(,)? }) => {{
        (|| -> Result<(), $crate::jar_patch::JarPatchError> {
            $(
                $crate::jar_patch::patch_jar_class(
                    &mut $jar,
                    $class_path,
                    &[ $( ($method, $source) ),+ ],
                    &$crate::compile::CompileOptions::default(),
                )?;
            )+
            Ok(())
        })()
    }};
}
