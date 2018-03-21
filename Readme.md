# Java Classfile parser

[![Build Status](https://travis-ci.org/Palmr/classfile-parser.svg?branch=master)](https://travis-ci.org/Palmr/classfile-parser)

A parser for [Java 8 Classfiles](https://docs.oracle.com/javase/specs/jvms/se8/html/jvms-4.html), written in Rust using [nom](https://github.com/Geal/nom).

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
  - [ ] Basic attribute info block parsing
  - [ ] Known typed attributes parsing
    - [ ] Critical for JVM
      - [x] ConstantValue
      - [x] Code
      - [x] StackMapTable
      - [x] Exceptions
      - [ ] BootstrapMethods
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
      - [ ] SourceFile
      - [ ] SourceDebugExtension
      - [ ] LineNumberTable
      - [ ] LocalVariableTable
      - [ ] LocalVariableTypeTable
      - [ ] Deprecated
