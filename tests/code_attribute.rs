//only works for nightly builds at the moment
//#![feature(assert_matches)]

extern crate classfile_parser;

//use std::assert_matches::assert_matches;

use classfile_parser::attribute_info::{
    DefaultAnnotation, ElementValue, InnerClassAccessFlags, TargetInfo, code_attribute_parser,
    element_value_parser, enclosing_method_attribute_parser, inner_classes_attribute_parser,
    line_number_table_attribute_parser, method_parameters_attribute_parser,
    runtime_invisible_annotations_attribute_parser,
    runtime_invisible_parameter_annotations_attribute_parser,
    runtime_visible_annotations_attribute_parser,
    runtime_visible_parameter_annotations_attribute_parser,
    runtime_visible_type_annotations_attribute_parser, signature_attribute_parser,
    source_debug_extension_parser,
};
use classfile_parser::class_parser;
use classfile_parser::code_attribute::{
    Instruction, LocalVariableTableAttribute, code_parser, instruction_parser,
    local_variable_type_table_parser,
};
use classfile_parser::constant_info::{ConstantInfo, Utf8Constant};
use classfile_parser::method_info::MethodAccessFlags;

#[test]
fn test_simple() {
    let instruction = &[0x11, 0xff, 0xfe];
    assert_eq!(
        Ok((&[][..], Instruction::Sipush(-2i16))),
        instruction_parser(instruction, 0)
    );
}

#[test]
fn test_wide() {
    let instruction = &[0xc4, 0x15, 0xaa, 0xbb];
    assert_eq!(
        Ok((&[][..], Instruction::IloadWide(0xaabb))),
        instruction_parser(instruction, 0)
    );
}

#[test]
fn test_alignment() {
    let instructions = vec![
        (
            3,
            vec![
                0xaa, 0, 0, 0, 10, 0, 0, 0, 20, 0, 0, 0, 21, 0, 0, 0, 30, 0, 0, 0, 31,
            ],
        ),
        (
            0,
            vec![
                0xaa, 0, 0, 0, 0, 0, 0, 10, 0, 0, 0, 20, 0, 0, 0, 21, 0, 0, 0, 30, 0, 0, 0, 31,
            ],
        ),
    ];
    let expected = Ok((
        &[][..],
        Instruction::Tableswitch {
            default: 10,
            low: 20,
            high: 21,
            offsets: vec![30, 31],
        },
    ));
    for (address, instruction) in instructions {
        assert_eq!(expected, instruction_parser(&instruction, address));
    }
}

#[test]
fn test_incomplete() {
    let code = &[0x59, 0x59, 0xc4, 0x15]; // dup, dup, <incomplete iload/wide>
    let expected = Ok((
        &[0xc4, 0x15][..],
        vec![(0, Instruction::Dup), (1, Instruction::Dup)],
    ));
    assert_eq!(expected, code_parser(code));
}

#[test]
fn test_class() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/Instructions.class");
    let (_, class) = class_parser(class_bytes).unwrap();
    let method_info = &class
        .methods
        .iter()
        .find(|m| m.access_flags.contains(MethodAccessFlags::STATIC))
        .unwrap();
    let (_, code_attribute) = code_attribute_parser(&method_info.attributes[0].info).unwrap();

    let parsed = code_parser(&code_attribute.code);

    assert!(parsed.is_ok());
    assert_eq!(64, parsed.unwrap().1.len());
}

fn lookup_string(c: &classfile_parser::ClassFile, index: u16) -> Option<String> {
    let con = &c.const_pool[(index - 1) as usize];
    match con {
        classfile_parser::constant_info::ConstantInfo::Utf8(utf8) => {
            Some(utf8.utf8_string.to_string())
        }
        _ => None,
    }
}

#[test]
fn method_parameters() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/BasicClass.class");
    let (_, class) = class_parser(class_bytes).unwrap();
    let method_info = &class.methods.iter().last().unwrap();

    // The class was not compiled with "javac -parameters" this required being able to find
    // MethodParameters in the class file, for example:
    // javac -parameters ./java-assets/src/uk/co/palmr/classfileparser/BasicClass.java -d ./java-assets/compiled-classes ; cp ./java-assets/compiled-classes/uk/co/palmr/classfileparser/BasicClass.class ./java-assets/compiled-classes/BasicClass.class
    assert_eq!(method_info.attributes.len(), 2);
    let (_, method_parameters) =
        method_parameters_attribute_parser(&method_info.attributes[1].info).unwrap();
    assert_eq!(
        lookup_string(
            &class,
            method_parameters.parameters.first().unwrap().name_index
        ),
        Some("a".to_string())
    );
    assert_eq!(
        lookup_string(
            &class,
            method_parameters.parameters.get(1).unwrap().name_index
        ),
        Some("b".to_string())
    );
}

#[test]
fn inner_classes() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/InnerClasses.class");
    let (_, class) = class_parser(class_bytes).unwrap();

    for attr in &class.attributes {
        match lookup_string(&class, attr.attribute_name_index) {
            Some(x) if x == "InnerClasses" => {
                let (_, inner_class_attrs) = inner_classes_attribute_parser(&attr.info).unwrap();

                assert_eq!(inner_class_attrs.number_of_classes, 4);

                assert_eq!(
                    inner_class_attrs.number_of_classes,
                    inner_class_attrs.classes.len() as u16
                );

                for c in inner_class_attrs.classes {
                    dbg!(&class.const_pool[(c.inner_class_info_index - 1) as usize]);

                    // only == 0 when this class is a top-level class or interface, or when it's
                    // a local class or an anonymous class.
                    if c.outer_class_info_index != 0 {
                        assert_ne!(c.inner_class_info_index, c.outer_class_info_index);

                        dbg!(&class.const_pool[(c.outer_class_info_index - 1) as usize]);
                    }

                    // only == 0 when this class is anonymous
                    if c.inner_name_index != 0 {
                        dbg!(&class.const_pool[(c.inner_name_index - 1) as usize]);
                    }

                    dbg!(InnerClassAccessFlags::from_bits_truncate(
                        c.inner_class_access_flags
                    ));
                }
                //uncomment to see dbg output from above
                //assert!(false);
            }
            Some(_) => {}
            None => panic!(
                "Could not find attribute name for index {}",
                attr.attribute_name_index
            ),
        }
    }
}

#[test]
// test for enclosing method attribute, which only applies to local and anonymous classes
fn enclosing_method() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/InnerClasses$2.class");
    let (_, class) = class_parser(class_bytes).unwrap();

    for attr in &class.attributes {
        match lookup_string(&class, attr.attribute_name_index) {
            Some(x) if x == "EnclosingMethod" => {
                assert_eq!(attr.attribute_length, 4);

                let (_, inner_class_attrs) = enclosing_method_attribute_parser(&attr.info).unwrap();

                match &class.const_pool[(inner_class_attrs.class_index - 1) as usize] {
                    classfile_parser::constant_info::ConstantInfo::Class(class_constant) => {
                        /* nightly only rn
                         * use regular asserts + decomposition instead
                        let _expected = String::from("InnerClasses");
                        assert_matches!(
                            &class.const_pool[(class_constant.name_index - 1) as usize],
                            ConstantInfo::Utf8(classfile_parser::constant_info::Utf8Constant {
                                utf8_string: _expected,
                            })
                        ); */
                        if let ConstantInfo::Utf8(inner_str) =
                            &class.const_pool[(class_constant.name_index - 1) as usize]
                        {
                            assert_eq!(
                                inner_str.utf8_string,
                                binrw::NullWideString::from("InnerClasses")
                            );
                        }

                        dbg!(&class.const_pool[(class_constant.name_index - 1) as usize]);
                    }
                    _ => panic!("Expected Class constant"),
                }

                match &class.const_pool[(inner_class_attrs.method_index - 1) as usize] {
                    classfile_parser::constant_info::ConstantInfo::NameAndType(
                        name_and_type_constant,
                    ) => {
                        /*
                        let mut _expected = String::from("sayHello");
                        assert_matches!(
                            &class.const_pool[(name_and_type_constant.name_index - 1) as usize],
                            ConstantInfo::Utf8(classfile_parser::constant_info::Utf8Constant {
                                utf8_string: _expected,
                            })
                        );
                        */
                        if let ConstantInfo::Utf8(inner_str) =
                            &class.const_pool[(name_and_type_constant.name_index - 1) as usize]
                        {
                            assert_eq!(
                                inner_str.utf8_string,
                                binrw::NullWideString::from("sayHello")
                            );
                        }
                        dbg!(&class.const_pool[(name_and_type_constant.name_index - 1) as usize]);

                        /*
                        _expected = String::from("()V");
                        assert_matches!(
                            &class.const_pool
                                [(name_and_type_constant.descriptor_index - 1) as usize],
                            ConstantInfo::Utf8(classfile_parser::constant_info::Utf8Constant {
                                utf8_string: _expected,
                            })
                        );
                        */
                        if let ConstantInfo::Utf8(inner_str) = &class.const_pool
                            [(name_and_type_constant.descriptor_index - 1) as usize]
                        {
                            assert_eq!(inner_str.utf8_string, binrw::NullWideString::from("()V"));
                        }
                        dbg!(
                            &class.const_pool
                                [(name_and_type_constant.descriptor_index - 1) as usize]
                        );
                    }
                    _ => panic!("Expected NameAndType constant"),
                }

                //uncomment to see dbg output from above
                //assert!(false);
            }
            Some(_) => {}
            None => panic!(
                "Could not find attribute name for index {}",
                attr.attribute_name_index
            ),
        }
    }
}

#[test]
fn synthetic_attribute() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/InnerClasses$2.class");
    let (_, class) = class_parser(class_bytes).unwrap();
    let synthetic_attrs = class
        .attributes
        .iter()
        .filter(
            |attribute_info| match lookup_string(&class, attribute_info.attribute_name_index) {
                Some(s) if s == "Synethic" => true,
                Some(_) => false,
                None => panic!(
                    "Could not find attribute name for index {}",
                    attribute_info.attribute_name_index
                ),
            },
        )
        .collect::<Vec<_>>();

    for attr in &synthetic_attrs {
        assert_eq!(attr.attribute_length, 0);
    }
}

//works on both method attributes and ClassFile attributes
#[test]
fn signature_attribute() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/BootstrapMethods.class");
    let (_, class) = class_parser(class_bytes).unwrap();
    let signature_attrs = class
        .methods
        .iter()
        .flat_map(|method_info| &method_info.attributes)
        .filter(
            |attribute_info| match lookup_string(&class, attribute_info.attribute_name_index) {
                Some(s) if s == "Signature" => {
                    eprintln!("Got a signature attr!");
                    true
                }
                Some(_) => false,
                None => panic!(
                    "Could not find attribute name for index {}",
                    attribute_info.attribute_name_index
                ),
            },
        )
        .collect::<Vec<_>>();

    for attr in &signature_attrs {
        let (_, signature_attr) = signature_attribute_parser(&attr.info).unwrap();
        let signature_string = lookup_string(&class, signature_attr.signature_index).unwrap();
        dbg!(signature_string);
    }

    //uncomment to see dbg output from above
    //assert!(false);
}

#[test]
fn local_variable_table() {
    // The class was not compiled with "javac -g"
    let class_bytes = include_bytes!("../java-assets/compiled-classes/LocalVariableTable.class");
    let (_, class) = class_parser(class_bytes).unwrap();
    let method_info = &class.methods.iter().last().unwrap();

    let code_attribute = method_info
        .attributes
        .iter()
        .find_map(|attribute_info| {
            match lookup_string(&class, attribute_info.attribute_name_index)?.as_str() {
                "Code" => {
                    classfile_parser::attribute_info::code_attribute_parser(&attribute_info.info)
                        .ok()
                }
                _ => None,
            }
        })
        .map(|i| i.1)
        .unwrap();

    let local_variable_table_attribute: LocalVariableTableAttribute = code_attribute
        .attributes
        .iter()
        .find_map(|attribute_info| {
            match lookup_string(&class, attribute_info.attribute_name_index)?.as_str() {
                "LocalVariableTable" => {
                    classfile_parser::code_attribute::local_variable_table_parser(
                        &attribute_info.info,
                    )
                    .ok()
                }
                _ => None,
            }
        })
        .map(|a| a.1)
        .unwrap();

    let types: Vec<String> = local_variable_table_attribute
        .items
        .iter()
        .filter_map(|i| lookup_string(&class, i.descriptor_index))
        .collect();

    // All used types in method code block of last method
    assert_eq!(
        types,
        vec![
            "LLocalVariableTable;".to_string(),
            "Ljava/util/HashMap;".to_string(),
            "I".to_string()
        ]
    );
}

#[test]
fn runtime_visible_annotations() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/Annotations.class");
    let (_, class) = class_parser(class_bytes).unwrap();
    let runtime_visible_annotations_attribute = class
        .methods
        .iter()
        .flat_map(|m| &m.attributes)
        .filter(|attribute_info| matches!(lookup_string(&class, attribute_info.attribute_name_index), Some(s) if s == "RuntimeVisibleAnnotations"))
        .collect::<Vec<_>>();

    assert_eq!(runtime_visible_annotations_attribute.len(), 1);
    let f = runtime_visible_annotations_attribute.first().unwrap();

    let visible_annotations = runtime_visible_annotations_attribute_parser(&f.info);
    let inner = &visible_annotations.unwrap();
    assert!(&inner.0.is_empty());

    /*
    let should_be = RuntimeVisibleTypeAnnotationsAttribute {
        num_annotations: 1,
        annotations: vec![RuntimeAnnotation {
            type_index: 30,
            num_element_value_pairs: 1,
            element_value_pairs: vec![ElementValuePair {
                element_name_index: 31,
                value: ElementValue::ConstValueIndex {
                    tag: 's',
                    value: 32,
                },
            }],
        }],
    };
    */

    assert_eq!(inner.1.num_annotations, 1);
    assert_eq!(inner.1.annotations.len(), 1);
    assert_eq!(inner.1.annotations[0].type_index, 46);
    assert_eq!(inner.1.annotations[0].num_element_value_pairs, 1);
    assert_eq!(inner.1.annotations[0].element_value_pairs.len(), 1);
    assert_eq!(
        inner.1.annotations[0].element_value_pairs[0].element_name_index,
        37
    );

    match inner.1.annotations[0].element_value_pairs[0].value {
        ElementValue::ConstValueIndex { tag, value } => {
            assert_eq!(tag, 's');
            assert_eq!(value, 47);
        }
        _ => panic!("Expected ConstValueIndex"),
    }
}

#[test]
fn runtime_invisible_annotations() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/Annotations.class");
    let (_, class) = class_parser(class_bytes).unwrap();
    let runtime_invisible_annotations_attribute = class
        .methods
        .iter()
        .flat_map(|m| &m.attributes)
        .filter(|attribute_info| matches!(lookup_string(&class, attribute_info.attribute_name_index), Some(s) if s == "RuntimeInvisibleAnnotations"))
        .collect::<Vec<_>>();

    assert_eq!(runtime_invisible_annotations_attribute.len(), 1);
    let f = runtime_invisible_annotations_attribute.first().unwrap();

    let invisible_annotations = runtime_invisible_annotations_attribute_parser(&f.info);
    let inner = &invisible_annotations.unwrap();
    assert!(&inner.0.is_empty());

    /*
    let should_be = RuntimeVisibleTypeAnnotationsAttribute {
        num_annotations: 1,
        annotations: vec![RuntimeAnnotation {
            type_index: 30,
            num_element_value_pairs: 1,
            element_value_pairs: vec![ElementValuePair {
                element_name_index: 31,
                value: ElementValue::ConstValueIndex {
                    tag: 's',
                    value: 32,
                },
            }],
        }],
    };
    */

    assert_eq!(inner.1.num_annotations, 1);
    assert_eq!(inner.1.annotations.len(), 1);
    assert_eq!(inner.1.annotations[0].type_index, 49);
    assert_eq!(inner.1.annotations[0].num_element_value_pairs, 1);
    assert_eq!(inner.1.annotations[0].element_value_pairs.len(), 1);
    assert_eq!(
        inner.1.annotations[0].element_value_pairs[0].element_name_index,
        37
    );

    match inner.1.annotations[0].element_value_pairs[0].value {
        ElementValue::ConstValueIndex { tag, value } => {
            assert_eq!(tag, 's');
            assert_eq!(value, 50);
        }
        _ => panic!("Expected ConstValueIndex"),
    }
}

#[test]
fn runtime_visible_parameter_annotations() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/Annotations.class");
    let (_, class) = class_parser(class_bytes).unwrap();
    let runtime_visible_annotations_attribute = class
        .methods
        .iter()
        .flat_map(|m| &m.attributes)
        .filter(|attribute_info| matches!(lookup_string(&class, attribute_info.attribute_name_index), Some(s) if s == "RuntimeVisibleParameterAnnotations"))
        .collect::<Vec<_>>();

    assert_eq!(runtime_visible_annotations_attribute.len(), 1);
    let f = runtime_visible_annotations_attribute.first().unwrap();

    let visible_annotations = runtime_visible_parameter_annotations_attribute_parser(&f.info);
    let inner = &visible_annotations.unwrap();
    assert!(&inner.0.is_empty());

    assert_eq!(inner.1.num_parameters, 2);
    assert_eq!(inner.1.parameter_annotations.len(), 2);
    assert_eq!(inner.1.parameter_annotations[0].num_annotations, 1);
    assert_eq!(inner.1.parameter_annotations[0].annotations.len(), 1);

    match inner.1.parameter_annotations[0].annotations[0].element_value_pairs[0].value {
        ElementValue::ConstValueIndex { tag, value } => {
            assert_eq!(tag, 's');
            assert_eq!(value, 53);
        }
        _ => panic!(
            "expected ConstValueIndex, got {:?}",
            inner.1.parameter_annotations[0].annotations[0].element_value_pairs[0].value
        ),
    }
}

#[test]
fn runtime_invisible_parameter_annotations() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/Annotations.class");
    let (_, class) = class_parser(class_bytes).unwrap();
    let runtime_invisible_annotations_attribute = class
        .methods
        .iter()
        .flat_map(|m| &m.attributes)
        .filter(|attribute_info| matches!(lookup_string(&class, attribute_info.attribute_name_index), Some(s) if s == "RuntimeInvisibleParameterAnnotations"))
        .collect::<Vec<_>>();

    assert_eq!(runtime_invisible_annotations_attribute.len(), 1);
    let f = runtime_invisible_annotations_attribute.first().unwrap();

    let invisible_annotations = runtime_invisible_parameter_annotations_attribute_parser(&f.info);
    let inner = &invisible_annotations.unwrap();
    assert!(&inner.0.is_empty());

    assert_eq!(inner.1.num_parameters, 2);
    assert_eq!(inner.1.parameter_annotations.len(), 2);
    assert_eq!(inner.1.parameter_annotations[1].num_annotations, 1);
    assert_eq!(inner.1.parameter_annotations[1].annotations.len(), 1);

    match inner.1.parameter_annotations[1].annotations[0].element_value_pairs[0].value {
        ElementValue::ConstValueIndex { tag, value } => {
            assert_eq!(tag, 's');
            assert_eq!(value, 50);
        }
        _ => panic!(
            "expected ConstValueIndex, got {:?}",
            inner.1.parameter_annotations[0].annotations[0].element_value_pairs[0].value
        ),
    }
}

#[test]
fn runtime_visible_type_annotations() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/Annotations.class");
    let (_, class) = class_parser(class_bytes).unwrap();
    let runtime_visible_type_annotations_attribute = class
        .fields
        .iter()
        .flat_map(|f| &f.attributes)
        .filter(|attribute_info| matches!(lookup_string(&class, attribute_info.attribute_name_index), Some(s) if s == "RuntimeVisibleTypeAnnotations"))
        .collect::<Vec<_>>();

    assert_eq!(runtime_visible_type_annotations_attribute.len(), 1);
    let f = runtime_visible_type_annotations_attribute.first().unwrap();

    let visible_annotations = runtime_visible_type_annotations_attribute_parser(&f.info);
    let inner = &visible_annotations.unwrap();
    assert_eq!(inner.1.num_annotations, 1);
    assert_eq!(inner.1.type_annotations.len(), 1);
    assert_eq!(inner.1.type_annotations[0].target_type, 19);
    //assert_matches!(inner.1.type_annotations[0].target_info, TargetInfo::Empty);
    assert_eq!(inner.1.type_annotations[0].target_path.path_length, 0);
    assert_eq!(inner.1.type_annotations[0].target_path.paths.len(), 0);
    assert_eq!(inner.1.type_annotations[0].type_index, 36);
    assert_eq!(inner.1.type_annotations[0].num_element_value_pairs, 1);
    assert_eq!(inner.1.type_annotations[0].element_value_pairs.len(), 1);
    assert_eq!(
        inner.1.type_annotations[0].element_value_pairs[0].element_name_index,
        37
    );
    /*
    assert_matches!(
        inner.1.type_annotations[0].element_value_pairs[0].value,
        ElementValue::ConstValueIndex {
            tag: 's',
            value: 38
        }
    );
    */
}

#[test]
fn runtime_invisible_type_annotations() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/Annotations.class");
    let (_, class) = class_parser(class_bytes).unwrap();
    let runtime_invisible_type_annotations_attribute = class
        .fields
        .iter()
        .flat_map(|f| &f.attributes)
        .filter(|attribute_info| matches!(lookup_string(&class, attribute_info.attribute_name_index), Some(s) if s == "RuntimeInvisibleTypeAnnotations"))
        .collect::<Vec<_>>();
    assert_eq!(runtime_invisible_type_annotations_attribute.len(), 1);
    let f = runtime_invisible_type_annotations_attribute
        .first()
        .unwrap();

    let invisible_annotations = runtime_visible_type_annotations_attribute_parser(&f.info);
    let inner = &invisible_annotations.unwrap();
    assert_eq!(inner.1.num_annotations, 1);
    assert_eq!(inner.1.type_annotations.len(), 1);
    assert_eq!(inner.1.type_annotations[0].target_type, 19);
    //assert_matches!(inner.1.type_annotations[0].target_info, TargetInfo::Empty);
    assert_eq!(inner.1.type_annotations[0].target_path.path_length, 0);
    assert_eq!(inner.1.type_annotations[0].target_path.paths.len(), 0);
    assert_eq!(inner.1.type_annotations[0].type_index, 41);
    assert_eq!(inner.1.type_annotations[0].num_element_value_pairs, 1);
    assert_eq!(inner.1.type_annotations[0].element_value_pairs.len(), 1);
    assert_eq!(
        inner.1.type_annotations[0].element_value_pairs[0].element_name_index,
        37
    );
    /*
    assert_matches!(
        inner.1.type_annotations[0].element_value_pairs[0].value,
        ElementValue::ConstValueIndex {
            tag: 's',
            value: 42
        }
    );
    */
}

#[test]
fn default_annotation_value() {
    let class_bytes =
        include_bytes!("../java-assets/compiled-classes/Annotations$VisibleAtRuntime.class");
    let (_, class) = class_parser(class_bytes).unwrap();
    let default_annotation_attributes = class
        .methods
        .iter()
        .flat_map(|m| &m.attributes)
        .filter(|attribute_info| matches!(lookup_string(&class, attribute_info.attribute_name_index), Some(s) if s == "AnnotationDefault"))
        .collect::<Vec<_>>();
    assert_eq!(default_annotation_attributes.len(), 1);
    let f = default_annotation_attributes.first().unwrap();

    let default_annotation = element_value_parser(&f.info);
    let inner: DefaultAnnotation = default_annotation.unwrap().1 as DefaultAnnotation;
    /*
    assert_matches!(
        inner,
        ElementValue::ConstValueIndex {
            tag: 's',
            value: 10
        }
    );
    */
    if let ElementValue::ConstValueIndex { tag, value } = inner {
        assert_eq!(tag, 's');
        assert_eq!(value, 10);
    }
}

// SourceDebugExtension attributes appear to be custom/non-standard. While it would
// be nice to parse, ultimately the spec defines the attribute as a byte array that
// contains "extended debugging information which has no semantic effect on the Java
// Virtual Machine", so I will leave this test to be better developed when example
// use cases are found.
// #[test]
fn source_debug_extension() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/BasicClass.class");
    let (_, class) = class_parser(class_bytes).unwrap();
    let source_debug_extension_attribute = class
        .attributes
        .iter()
        .filter(|attribute_info| matches!(lookup_string(&class, attribute_info.attribute_name_index), Some(s) if s == "SourceDebugExtension"))
        .collect::<Vec<_>>();
    assert_eq!(source_debug_extension_attribute.len(), 1);
    let f = source_debug_extension_attribute.first().unwrap();

    let default_annotation = source_debug_extension_parser(&f.info);
    let inner = &default_annotation.unwrap();
    dbg!(inner);
}

#[test]
fn source_file() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/BasicClass.class");
    let (_, class) = class_parser(class_bytes).unwrap();

    let source = class
        .attributes
        .iter()
        .find_map(|attribute_info| {
            match lookup_string(&class, attribute_info.attribute_name_index)?.as_str() {
                "SourceFile" => classfile_parser::attribute_info::sourcefile_attribute_parser(
                    &attribute_info.info,
                )
                .ok(),
                o => {
                    dbg!(o);
                    None
                }
            }
        })
        .map(|i| i.1)
        .unwrap();

    let s = lookup_string(&class, source.sourcefile_index).unwrap();

    assert_eq!(s, "BasicClass.java");
}

#[test]
fn line_number_table() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/Instructions.class");
    let (_, class) = class_parser(class_bytes).unwrap();
    let default_annotation_attributes = class
        .methods
        .iter()
        .find(|m| m.access_flags.contains(MethodAccessFlags::STATIC))
        .unwrap();

    let (_, code_attribute) =
        code_attribute_parser(&default_annotation_attributes.attributes[0].info).unwrap();

    assert_eq!(
        code_attribute.attributes.len(),
        code_attribute.attributes_count as usize
    );

    let line_number_tables = &code_attribute
        .attributes
        .iter()
        .filter(|a| lookup_string(&class, a.attribute_name_index).unwrap() == "LineNumberTable")
        .map(|a| line_number_table_attribute_parser(&a.info).unwrap().1)
        .collect::<Vec<_>>();

    assert_eq!(line_number_tables.len(), 1);
    assert_eq!(line_number_tables[0].line_number_table_length, 12);
    assert_eq!(line_number_tables[0].line_number_table.len(), 12);
    assert_eq!(line_number_tables[0].line_number_table[0].start_pc, 0);
    assert_eq!(line_number_tables[0].line_number_table[0].line_number, 3);
}

#[test]
fn local_variable_type_table() {
    // The class was not compiled with "javac -g"
    let class_bytes = include_bytes!("../java-assets/compiled-classes/LocalVariableTable.class");
    let (_, class) = class_parser(class_bytes).unwrap();
    let method_info = &class.methods.iter().last().unwrap();

    let local_variable_table_type_attribute = method_info
        .attributes
        .iter()
        .find_map(|attribute_info| {
            match lookup_string(&class, attribute_info.attribute_name_index)?.as_str() {
                "Code" => code_attribute_parser(&attribute_info.info).ok(),
                _ => None,
            }
        })
        .map(|i| i.1)
        .unwrap()
        .attributes
        .iter()
        .find_map(|attribute_info| {
            match lookup_string(&class, attribute_info.attribute_name_index)?.as_str() {
                "LocalVariableTypeTable" => {
                    local_variable_type_table_parser(&attribute_info.info).ok()
                }
                _ => None,
            }
        })
        .map(|a| a.1)
        .unwrap();

    let types: Vec<String> = local_variable_table_type_attribute
        .local_variable_type_table
        .iter()
        .filter_map(|i| lookup_string(&class, i.signature_index))
        .collect();

    // All used types in method code block of last method
    assert_eq!(
        types,
        vec!["Ljava/util/HashMap<Ljava/lang/Integer;Ljava/lang/String;>;"]
    );
}

#[test]
fn deprecated() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/DeprecatedAnnotation.class");
    let (_, class) = class_parser(class_bytes).unwrap();

    let deprecated_class_attribute = &class
        .attributes
        .iter()
        .filter(|attribute_info| {
            matches!(
                lookup_string(&class, attribute_info.attribute_name_index)
                    .unwrap()
                    .as_str(),
                "Deprecated"
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(deprecated_class_attribute.len(), 1);

    let deprecated_method_attribute = &class
        .methods
        .iter()
        .flat_map(|m| &m.attributes)
        .filter(|attribute_info| {
            matches!(
                lookup_string(&class, attribute_info.attribute_name_index)
                    .unwrap()
                    .as_str(),
                "Deprecated"
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(deprecated_method_attribute.len(), 1);

    let deprecated_field_attribute = &class
        .fields
        .iter()
        .flat_map(|f| &f.attributes)
        .filter(|attribute_info| {
            matches!(
                lookup_string(&class, attribute_info.attribute_name_index)
                    .unwrap()
                    .as_str(),
                "Deprecated"
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(deprecated_field_attribute.len(), 1);
}
