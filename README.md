# Java Classfile Parser

[![LICENSE](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE.txt)
![Rust](https://github.com/Palmr/classfile-parser/workflows/Rust/badge.svg)
[![Crates.io Version](https://img.shields.io/crates/v/classfile-parser.svg)](https://crates.io/crates/classfile-parser)

A parser for [Java Classfiles](https://docs.oracle.com/javase/specs/jvms/se10/html/jvms-4.html), written in Rust using [nom](https://github.com/Geal/nom).

## Installation

Classfile Parser is available from crates.io and can be included in your Cargo enabled project like this:

```toml
[dependencies]
classfile-parser = "~0.3"
```

## Usage

```rust
extern crate classfile_parser;

use classfile_parser::class_parser;

fn main() {
    let classfile_bytes = include_bytes!("../path/to/JavaClass.class");
    
    match class_parser(classfile_bytes) {
        Ok((_, class_file)) => {
            println!(
                "version {},{} \
                 const_pool({}), \
                 this=const[{}], \
                 super=const[{}], \
                 interfaces({}), \
                 fields({}), \
                 methods({}), \
                 attributes({}), \
                 access({:?})",
                class_file.major_version,
                class_file.minor_version,
                class_file.const_pool_size,
                class_file.this_class,
                class_file.super_class,
                class_file.interfaces_count,
                class_file.fields_count,
                class_file.methods_count,
                class_file.attributes_count,
                class_file.access_flags
            );
        }
        Err(_) => panic!("Failed to parse"),
    };
}
```

## Implementation Status

- [x] Header
  - [x] Magic const
  - [x] Version info
- [x] Constant pool
  - [x] Constant pool size
  - [x] Constant types
    - [x] Utf8
    - [x] Integer
    - [x] Float
    - [x] Long
    - [x] Double
    - [x] Class
    - [x] String
    - [x] Fieldref
    - [x] Methodref
    - [x] InterfaceMethodref
    - [x] NameAndType
    - [x] MethodHandle
    - [x] MethodType
    - [x] InvokeDynamic
- [x] Access flags
- [x] This class
- [x] Super class
- [x] Interfaces
- [x] Fields
- [x] Methods
- [x] Attributes
  - [x] Basic attribute info block parsing
  - [ ] Known typed attributes parsing
    - [x] Critical for JVM
      - [x] ConstantValue
      - [x] Code
      - [x] StackMapTable
      - [x] Exceptions
      - [x] BootstrapMethods
    - [ ] Critical for Java SE
      - [ ] InnerClasses
      - [ ] EnclosingMethod
      - [ ] Synthetic
      - [ ] Signature
      - [ ] RuntimeVisibleAnnotations
      - [ ] RuntimeInvisibleAnnotations
      - [ ] RuntimeVisibleParameterAnnotations
      - [ ] RuntimeInvisibleParameterAnnotations
      - [ ] RuntimeVisibleTypeAnnotations
      - [ ] RuntimeInvisibleTypeAnnotations
      - [ ] AnnotationDefault
      - [ ] MethodParameters
    - [ ] Useful but not critical
      - [x] SourceFile
      - [ ] SourceDebugExtension
      - [ ] LineNumberTable
      - [ ] LocalVariableTable
      - [ ] LocalVariableTypeTable
      - [ ] Deprecated
