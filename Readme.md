# Java Classfile parser

[![Build Status](https://travis-ci.org/Palmr/classfile-parser.svg?branch=master)](https://travis-ci.org/Palmr/classfile-parser)

A parser for [Java Classfiles](https://docs.oracle.com/javase/specs/jvms/se8/html/jvms-4.html), written in Rust using [nom](https://github.com/Geal/nom).

## Implementation Status

- [x] Header
  - [x] Magic const
  - [x] Version info
- [ ] Constant pool
  - [x] Constant pool size
  - [ ] Constant types
    - [x] Utf8
    - [x] Integer
    - [x] Float
    - [ ] Long
    - [ ] Double
    - [x] Class
    - [x] String
    - [x] Fieldref
    - [x] Methodref
    - [x] InterfaceMethodref
    - [x] NameAndType
    - [ ] MethodHandle
    - [ ] MethodType
    - [ ] InvokeDynamic
- [ ] Access flags
    - [x] Stubbed as commented enums
    - [ ] Implement code to test the enum against values
- [x] This class
- [x] Super class
- [x] Interfaces
- [x] Fields
- [x] Methods
- [x] Attributes
  - [ ] Basic attribute info block parsing
  - [ ] Known typed attributes parsing
