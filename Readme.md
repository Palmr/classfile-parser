# Java Classfile parser

A parser for [Java Classfile][https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html], written in Rust using [nom][https://github.com/Geal/nom].

## Implementation Status

- [x] Header
  - [x] Magic const
  - [x] Version info
- [ ] Constant pool
  - [x] Constant pool size
  - [ ] Constant types
    - [x] Utf8
    - [ ] Integer
    - [ ] Float
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
- [ ] This class
- [ ] Super class
- [ ] Interfaces
  - [ ] TODO
- [ ] Methods
  - [ ] TODO
- [ ] Attributes
  - [ ] TODO
