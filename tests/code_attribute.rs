extern crate classfile_parser;

use std::io::Cursor;

use assert_matches::assert_matches;
use binrw::BinRead;
use classfile_parser::ClassFile;
use classfile_parser::attribute_info::{
    AttributeInfoVariant, ElementValue, InnerClassAccessFlags, LineNumberTableAttribute, TargetInfo,
};
use classfile_parser::code_attribute::{
    Instruction, LocalVariableTableAttribute, LocalVariableTypeTableAttribute,
};
use classfile_parser::constant_info::ConstantInfo;
use classfile_parser::method_info::MethodAccessFlags;

fn lookup_string(c: &ClassFile, index: u16) -> Option<String> {
    match &c.const_pool[(index - 1) as usize] {
        classfile_parser::constant_info::ConstantInfo::Utf8(utf8) => {
            Some(utf8.utf8_string.to_string())
        }
        _ => None,
    }
}

#[test]
fn test_simple() {
    let mut instruction = vec![0x11, 0xff, 0xfe];
    assert_eq!(
        Instruction::Sipush(-2i16),
        Instruction::read_be_args(
            &mut Cursor::new(&mut instruction),
            binrw::args! { address: 0 }
        )
        .unwrap()
    );
}

#[test]
fn test_wide() {
    let mut instruction = vec![0xc4, 0x15, 0xaa, 0xbb];
    assert_eq!(
        Instruction::IloadWide(0xaabb),
        Instruction::read_be_args(
            &mut Cursor::new(&mut instruction),
            binrw::args! { address: 0 }
        )
        .unwrap()
    );
}

#[test]
fn test_alignment() {
    let mut instructions: Vec<(u32, Vec<u8>)> = vec![
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

    let expected = Instruction::Tableswitch {
        default: 10,
        low: 20,
        high: 21,
        offsets: vec![30, 31],
    };

    for (address, instruction) in &mut instructions {
        assert_eq!(
            expected,
            Instruction::read_be_args(
                &mut Cursor::new(instruction),
                binrw::args! { address: *address }
            )
            .unwrap()
        );
    }
}

#[test]
fn test_incomplete() {
    let code = &[0x59, 0x59, 0xc4, 0x15]; // dup, dup, <incomplete iload/wide>
    let mut c = Cursor::new(code);

    assert_eq!(
        Instruction::Dup,
        Instruction::read_be_args(&mut c, binrw::args! { address: 0 }).unwrap()
    );
    assert_eq!(
        Instruction::Dup,
        Instruction::read_be_args(&mut c, binrw::args! { address: 0 }).unwrap()
    );

    let next = Instruction::read_be_args(&mut c, binrw::args! { address: 0 });
    if let binrw::Error::NoVariantMatch { pos } = next.unwrap_err() {
        assert_eq!(pos, 2);
    }
}

#[test]
fn test_class() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/Instructions.class");
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice())).unwrap();
    let method_info = class
        .methods
        .iter()
        .find(|m| m.access_flags.contains(MethodAccessFlags::STATIC))
        .unwrap();

    let code_attr = method_info.attributes.iter().find_map(|attr| {
        if let Some(AttributeInfoVariant::Code(code)) = &attr.info_parsed {
            Some(code)
        } else {
            None
        }
    });

    let code = code_attr.expect("Should have found a Code attribute");
    assert_eq!(64, code.code.len());
}

#[test]
fn method_parameters() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/BasicClass.class");
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice())).unwrap();
    let method_info = class.methods.iter().last().unwrap();

    // The class was not compiled with "javac -parameters" this required being able to find
    // MethodParameters in the class file, for example:
    // javac -parameters ./java-assets/src/uk/co/palmr/classfileparser/BasicClass.java -d ./java-assets/compiled-classes ; cp ./java-assets/compiled-classes/uk/co/palmr/classfileparser/BasicClass.class ./java-assets/compiled-classes/BasicClass.class
    assert_eq!(method_info.attributes.len(), 2);

    let method_parameters = method_info
        .attributes
        .iter()
        .find_map(|attr| {
            if let Some(AttributeInfoVariant::MethodParameters(mp)) = &attr.info_parsed {
                Some(mp)
            } else {
                None
            }
        })
        .expect("Should have found MethodParameters attribute");

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
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice())).unwrap();

    for attr in &class.attributes {
        match &attr.info_parsed {
            Some(AttributeInfoVariant::InnerClasses(inner_class_attrs)) => {
                assert_eq!(inner_class_attrs.number_of_classes, 4);

                assert_eq!(
                    inner_class_attrs.number_of_classes,
                    inner_class_attrs.classes.len() as u16
                );

                for c in &inner_class_attrs.classes {
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
                "Could not parse attribute for index {}",
                attr.attribute_name_index
            ),
        }
    }
}

#[test]
// test for enclosing method attribute, which only applies to local and anonymous classes
fn enclosing_method() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/InnerClasses$2.class");
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice())).unwrap();

    for attr in &class.attributes {
        match &attr.info_parsed {
            Some(AttributeInfoVariant::EnclosingMethod(enclosing)) => {
                assert_eq!(attr.attribute_length, 4);

                match &class.const_pool[(enclosing.class_index - 1) as usize] {
                    classfile_parser::constant_info::ConstantInfo::Class(class_constant) => {
                        if let ConstantInfo::Utf8(inner_str) =
                            &class.const_pool[(class_constant.name_index - 1) as usize]
                        {
                            assert_eq!(inner_str.utf8_string, String::from("InnerClasses"));
                        }

                        dbg!(&class.const_pool[(class_constant.name_index - 1) as usize]);
                    }
                    _ => panic!("Expected Class constant"),
                }

                match &class.const_pool[(enclosing.method_index - 1) as usize] {
                    classfile_parser::constant_info::ConstantInfo::NameAndType(
                        name_and_type_constant,
                    ) => {
                        if let ConstantInfo::Utf8(inner_str) = &class.const_pool
                            [(name_and_type_constant.descriptor_index - 1) as usize]
                        {
                            assert_eq!(inner_str.utf8_string, String::from("()V"));
                        }
                        dbg!(
                            &class.const_pool
                                [(name_and_type_constant.descriptor_index - 1) as usize]
                        );
                    }
                    _ => panic!("Expected NameAndType constant"),
                }
            }
            Some(_) => {}
            None => panic!(
                "Could not parse attribute for index {}",
                attr.attribute_name_index
            ),
        }
    }
}

#[test]
fn synthetic_attribute() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/InnerClasses$2.class");
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice())).unwrap();
    let synthetic_attrs = class
        .attributes
        .iter()
        .filter(|attribute_info| {
            matches!(
                &attribute_info.info_parsed,
                Some(AttributeInfoVariant::Synthetic(_))
            )
        })
        .collect::<Vec<_>>();

    for attr in &synthetic_attrs {
        assert_eq!(attr.attribute_length, 0);
    }
}

//works on both method attributes and ClassFile attributes
#[test]
fn signature_attribute() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/BootstrapMethods.class");
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice())).unwrap();
    let signature_attrs = class
        .methods
        .iter()
        .flat_map(|method_info| &method_info.attributes)
        .filter(|attribute_info| {
            if let Some(AttributeInfoVariant::Signature(_)) = &attribute_info.info_parsed {
                eprintln!("Got a signature attr!");
                true
            } else {
                false
            }
        })
        .collect::<Vec<_>>();

    for attr in &signature_attrs {
        if let Some(AttributeInfoVariant::Signature(sig)) = &attr.info_parsed {
            let signature_string = lookup_string(&class, sig.signature_index).unwrap();
            dbg!(signature_string);
        }
    }

    //uncomment to see dbg output from above
    //assert!(false);
}

#[test]
fn local_variable_table() {
    // The class was not compiled with "javac -g"
    let class_bytes = include_bytes!("../java-assets/compiled-classes/LocalVariableTable.class");
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice())).unwrap();
    let method_info = class.methods.iter().last().unwrap();

    let code_attribute = method_info
        .attributes
        .iter()
        .find_map(|attribute_info| {
            if let Some(AttributeInfoVariant::Code(code)) = &attribute_info.info_parsed {
                Some(code)
            } else {
                None
            }
        })
        .expect("Should have found a Code attribute");

    // Code attribute's sub-attributes do NOT have info_parsed populated, so we parse manually
    let local_variable_table_attribute: LocalVariableTableAttribute = code_attribute
        .attributes
        .iter()
        .find_map(|attribute_info| {
            match lookup_string(&class, attribute_info.attribute_name_index)?.as_str() {
                "LocalVariableTable" => {
                    LocalVariableTableAttribute::read(&mut Cursor::new(&attribute_info.info)).ok()
                }
                _ => None,
            }
        })
        .expect("Should have found a LocalVariableTable attribute");

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
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice())).unwrap();
    let runtime_visible_annotations_attribute = class
        .methods
        .iter()
        .flat_map(|m| &m.attributes)
        .filter(|attribute_info| {
            matches!(
                &attribute_info.info_parsed,
                Some(AttributeInfoVariant::RuntimeVisibleAnnotations(_))
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(runtime_visible_annotations_attribute.len(), 1);
    let f = runtime_visible_annotations_attribute.first().unwrap();

    let inner = match &f.info_parsed {
        Some(AttributeInfoVariant::RuntimeVisibleAnnotations(rva)) => rva,
        _ => panic!("Expected RuntimeVisibleAnnotations"),
    };

    assert_eq!(inner.num_annotations, 1);
    assert_eq!(inner.annotations.len(), 1);
    assert_eq!(inner.annotations[0].type_index, 46);
    assert_eq!(inner.annotations[0].num_element_value_pairs, 1);
    assert_eq!(inner.annotations[0].element_value_pairs.len(), 1);
    assert_eq!(
        inner.annotations[0].element_value_pairs[0].element_name_index,
        37
    );

    match &inner.annotations[0].element_value_pairs[0].value {
        ElementValue::ConstValueIndex(cv) => {
            assert_eq!(cv.tag, 's');
            assert_eq!(cv.value, 47);
        }
        _ => panic!("Expected ConstValueIndex"),
    }
}

#[test]
fn runtime_invisible_annotations() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/Annotations.class");
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice())).unwrap();
    let runtime_invisible_annotations_attribute = class
        .methods
        .iter()
        .flat_map(|m| &m.attributes)
        .filter(|attribute_info| {
            matches!(
                &attribute_info.info_parsed,
                Some(AttributeInfoVariant::RuntimeInvisibleAnnotations(_))
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(runtime_invisible_annotations_attribute.len(), 1);
    let f = runtime_invisible_annotations_attribute.first().unwrap();

    let inner = match &f.info_parsed {
        Some(AttributeInfoVariant::RuntimeInvisibleAnnotations(ria)) => ria,
        _ => panic!("Expected RuntimeInvisibleAnnotations"),
    };

    assert_eq!(inner.num_annotations, 1);
    assert_eq!(inner.annotations.len(), 1);
    assert_eq!(inner.annotations[0].type_index, 49);
    assert_eq!(inner.annotations[0].num_element_value_pairs, 1);
    assert_eq!(inner.annotations[0].element_value_pairs.len(), 1);
    assert_eq!(
        inner.annotations[0].element_value_pairs[0].element_name_index,
        37
    );

    match &inner.annotations[0].element_value_pairs[0].value {
        ElementValue::ConstValueIndex(cv) => {
            assert_eq!(cv.tag, 's');
            assert_eq!(cv.value, 50);
        }
        _ => panic!("Expected ConstValueIndex"),
    }
}

#[test]
fn runtime_visible_parameter_annotations() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/Annotations.class");
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice())).unwrap();
    let runtime_visible_annotations_attribute = class
        .methods
        .iter()
        .flat_map(|m| &m.attributes)
        .filter(|attribute_info| {
            matches!(
                &attribute_info.info_parsed,
                Some(AttributeInfoVariant::RuntimeVisibleParameterAnnotations(_))
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(runtime_visible_annotations_attribute.len(), 1);
    let f = runtime_visible_annotations_attribute.first().unwrap();

    let inner = match &f.info_parsed {
        Some(AttributeInfoVariant::RuntimeVisibleParameterAnnotations(rvpa)) => rvpa,
        _ => panic!("Expected RuntimeVisibleParameterAnnotations"),
    };

    assert_eq!(inner.num_parameters, 2);
    assert_eq!(inner.parameter_annotations.len(), 2);
    assert_eq!(inner.parameter_annotations[0].num_annotations, 1);
    assert_eq!(inner.parameter_annotations[0].annotations.len(), 1);

    match &inner.parameter_annotations[0].annotations[0].element_value_pairs[0].value {
        ElementValue::ConstValueIndex(cv) => {
            assert_eq!(cv.tag, 's');
            assert_eq!(cv.value, 53);
        }
        _ => panic!(
            "expected ConstValueIndex, got {:?}",
            inner.parameter_annotations[0].annotations[0].element_value_pairs[0].value
        ),
    }
}

#[test]
fn runtime_invisible_parameter_annotations() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/Annotations.class");
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice())).unwrap();
    let runtime_invisible_annotations_attribute = class
        .methods
        .iter()
        .flat_map(|m| &m.attributes)
        .filter(|attribute_info| {
            matches!(
                &attribute_info.info_parsed,
                Some(AttributeInfoVariant::RuntimeInvisibleParameterAnnotations(
                    _
                ))
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(runtime_invisible_annotations_attribute.len(), 1);
    let f = runtime_invisible_annotations_attribute.first().unwrap();

    let inner = match &f.info_parsed {
        Some(AttributeInfoVariant::RuntimeInvisibleParameterAnnotations(ripa)) => ripa,
        _ => panic!("Expected RuntimeInvisibleParameterAnnotations"),
    };

    assert_eq!(inner.num_parameters, 2);
    assert_eq!(inner.parameter_annotations.len(), 2);
    assert_eq!(inner.parameter_annotations[1].num_annotations, 1);
    assert_eq!(inner.parameter_annotations[1].annotations.len(), 1);

    match &inner.parameter_annotations[1].annotations[0].element_value_pairs[0].value {
        ElementValue::ConstValueIndex(cv) => {
            assert_eq!(cv.tag, 's');
            assert_eq!(cv.value, 50);
        }
        _ => panic!(
            "expected ConstValueIndex, got {:?}",
            inner.parameter_annotations[0].annotations[0].element_value_pairs[0].value
        ),
    }
}

#[test]
fn runtime_visible_type_annotations() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/Annotations.class");
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice())).unwrap();
    let runtime_visible_type_annotations_attribute = class
        .fields
        .iter()
        .flat_map(|f| &f.attributes)
        .filter(|attribute_info| {
            matches!(
                &attribute_info.info_parsed,
                Some(AttributeInfoVariant::RuntimeVisibleTypeAnnotations(_))
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(runtime_visible_type_annotations_attribute.len(), 1);
    let f = runtime_visible_type_annotations_attribute.first().unwrap();

    let inner = match &f.info_parsed {
        Some(AttributeInfoVariant::RuntimeVisibleTypeAnnotations(rvta)) => rvta,
        _ => panic!("Expected RuntimeVisibleTypeAnnotations"),
    };

    assert_eq!(inner.num_annotations, 1);
    assert_eq!(inner.type_annotations.len(), 1);
    assert_eq!(inner.type_annotations[0].target_type, 19);
    assert_matches!(inner.type_annotations[0].target_info, TargetInfo::Empty);
    assert_eq!(inner.type_annotations[0].target_path.path_length, 0);
    assert_eq!(inner.type_annotations[0].target_path.paths.len(), 0);
    assert_eq!(inner.type_annotations[0].type_index, 36);
    assert_eq!(inner.type_annotations[0].num_element_value_pairs, 1);
    assert_eq!(inner.type_annotations[0].element_value_pairs.len(), 1);
    assert_eq!(
        inner.type_annotations[0].element_value_pairs[0].element_name_index,
        37
    );
    match &inner.type_annotations[0].element_value_pairs[0].value {
        ElementValue::ConstValueIndex(cv) => {
            assert_eq!(cv.tag, 's');
            assert_eq!(cv.value, 38);
        }
        _ => panic!("Expected ConstValueIndex"),
    }
}

#[test]
fn runtime_invisible_type_annotations() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/Annotations.class");
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice())).unwrap();
    let runtime_invisible_type_annotations_attribute = class
        .fields
        .iter()
        .flat_map(|f| &f.attributes)
        .filter(|attribute_info| {
            matches!(
                &attribute_info.info_parsed,
                Some(AttributeInfoVariant::RuntimeInvisibleTypeAnnotations(_))
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(runtime_invisible_type_annotations_attribute.len(), 1);
    let f = runtime_invisible_type_annotations_attribute
        .first()
        .unwrap();

    let inner = match &f.info_parsed {
        Some(AttributeInfoVariant::RuntimeInvisibleTypeAnnotations(rita)) => rita,
        _ => panic!("Expected RuntimeInvisibleTypeAnnotations"),
    };

    assert_eq!(inner.num_annotations, 1);
    assert_eq!(inner.type_annotations.len(), 1);
    assert_eq!(inner.type_annotations[0].target_type, 19);
    assert_matches!(inner.type_annotations[0].target_info, TargetInfo::Empty);
    assert_eq!(inner.type_annotations[0].target_path.path_length, 0);
    assert_eq!(inner.type_annotations[0].target_path.paths.len(), 0);
    assert_eq!(inner.type_annotations[0].type_index, 41);
    assert_eq!(inner.type_annotations[0].num_element_value_pairs, 1);
    assert_eq!(inner.type_annotations[0].element_value_pairs.len(), 1);
    assert_eq!(
        inner.type_annotations[0].element_value_pairs[0].element_name_index,
        37
    );
    match &inner.type_annotations[0].element_value_pairs[0].value {
        ElementValue::ConstValueIndex(cv) => {
            assert_eq!(cv.tag, 's');
            assert_eq!(cv.value, 42);
        }
        _ => panic!("Expected ConstValueIndex"),
    }
}

#[test]
fn default_annotation_value() {
    let class_bytes =
        include_bytes!("../java-assets/compiled-classes/Annotations$VisibleAtRuntime.class");
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice())).unwrap();
    let default_annotation_attributes = class
        .methods
        .iter()
        .flat_map(|m| &m.attributes)
        .filter(|attribute_info| {
            matches!(
                &attribute_info.info_parsed,
                Some(AttributeInfoVariant::AnnotationDefault(_))
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(default_annotation_attributes.len(), 1);
    let f = default_annotation_attributes.first().unwrap();

    let inner = match &f.info_parsed {
        Some(AttributeInfoVariant::AnnotationDefault(ad)) => ad,
        _ => panic!("Expected AnnotationDefault"),
    };

    match inner {
        ElementValue::ConstValueIndex(cv) => {
            assert_eq!(cv.tag, 's');
            assert_eq!(cv.value, 10);
        }
        _ => panic!("Expected ConstValueIndex"),
    }
}

// SourceDebugExtension attributes appear to be custom/non-standard. While it would
// be nice to parse, ultimately the spec defines the attribute as a byte array that
// contains "extended debugging information which has no semantic effect on the Java
// Virtual Machine", so I will leave this test to be better developed when example
// use cases are found.
// #[test]
#[allow(dead_code)]
fn source_debug_extension() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/BasicClass.class");
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice())).unwrap();
    let source_debug_extension_attribute = class
        .attributes
        .iter()
        .filter(|attribute_info| {
            matches!(
                &attribute_info.info_parsed,
                Some(AttributeInfoVariant::SourceDebugExtension(_))
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(source_debug_extension_attribute.len(), 1);
}

#[test]
fn source_file() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/BasicClass.class");
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice())).unwrap();

    let source = class
        .attributes
        .iter()
        .find_map(|attribute_info| {
            if let Some(AttributeInfoVariant::SourceFile(sf)) = &attribute_info.info_parsed {
                Some(sf)
            } else {
                None
            }
        })
        .expect("Should have found a SourceFile attribute");

    let s = lookup_string(&class, source.sourcefile_index).unwrap();

    assert_eq!(s, "BasicClass.java");
}

#[test]
fn line_number_table() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/Instructions.class");
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice())).unwrap();
    let static_method = class
        .methods
        .iter()
        .find(|m| m.access_flags.contains(MethodAccessFlags::STATIC))
        .unwrap();

    let code_attribute = static_method
        .attributes
        .iter()
        .find_map(|attr| {
            if let Some(AttributeInfoVariant::Code(code)) = &attr.info_parsed {
                Some(code)
            } else {
                None
            }
        })
        .expect("Should have found a Code attribute");

    assert_eq!(
        code_attribute.attributes.len(),
        code_attribute.attributes_count as usize
    );

    // Code attribute's sub-attributes do NOT have info_parsed populated, so we parse manually
    let line_number_tables = &code_attribute
        .attributes
        .iter()
        .filter(|a| lookup_string(&class, a.attribute_name_index).unwrap() == "LineNumberTable")
        .map(|a| LineNumberTableAttribute::read(&mut Cursor::new(&a.info)).unwrap())
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
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice())).unwrap();
    let method_info = class.methods.iter().last().unwrap();

    let code_attribute = method_info
        .attributes
        .iter()
        .find_map(|attribute_info| {
            if let Some(AttributeInfoVariant::Code(code)) = &attribute_info.info_parsed {
                Some(code)
            } else {
                None
            }
        })
        .expect("Should have found a Code attribute");

    // Code attribute's sub-attributes do NOT have info_parsed populated, so we parse manually
    let local_variable_table_type_attribute = code_attribute
        .attributes
        .iter()
        .find_map(|attribute_info| {
            match lookup_string(&class, attribute_info.attribute_name_index)?.as_str() {
                "LocalVariableTypeTable" => {
                    LocalVariableTypeTableAttribute::read(&mut Cursor::new(&attribute_info.info))
                        .ok()
                }
                _ => None,
            }
        })
        .expect("Should have found a LocalVariableTypeTable attribute");

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
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice())).unwrap();

    let deprecated_class_attribute = &class
        .attributes
        .iter()
        .filter(|attribute_info| {
            matches!(
                &attribute_info.info_parsed,
                Some(AttributeInfoVariant::Deprecated(_))
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
                &attribute_info.info_parsed,
                Some(AttributeInfoVariant::Deprecated(_))
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
                &attribute_info.info_parsed,
                Some(AttributeInfoVariant::Deprecated(_))
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(deprecated_field_attribute.len(), 1);
}
