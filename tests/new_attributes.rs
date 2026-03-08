extern crate classfile_parser;

use std::fs::File;
use std::io::Cursor;
use std::io::prelude::*;

use binrw::BinWrite;
use binrw::prelude::*;
use classfile_parser::ClassFile;
use classfile_parser::attribute_info::AttributeInfoVariant;

fn load_class(path: &str) -> ClassFile {
    let mut contents: Vec<u8> = Vec::new();
    let mut f = File::open(path).unwrap();
    f.read_to_end(&mut contents).unwrap();
    ClassFile::read(&mut Cursor::new(contents)).expect("failed to parse class file")
}

fn find_attr<'a>(
    attrs: &'a [classfile_parser::attribute_info::AttributeInfo],
    name: &str,
    class: &ClassFile,
) -> Option<&'a AttributeInfoVariant> {
    for attr in attrs {
        if let Some(ref parsed) = attr.info_parsed {
            let attr_name = match &class.const_pool[(attr.attribute_name_index - 1) as usize] {
                classfile_parser::constant_info::ConstantInfo::Utf8(u) => u.utf8_string.as_str(),
                _ => continue,
            };
            if attr_name == name {
                return Some(parsed);
            }
        }
    }
    None
}

fn round_trip(path: &str) {
    let mut original_bytes: Vec<u8> = Vec::new();
    let mut f = File::open(path).unwrap();
    f.read_to_end(&mut original_bytes).unwrap();

    let parsed = ClassFile::read(&mut Cursor::new(&original_bytes)).expect("failed to parse");

    let mut written_bytes = Cursor::new(Vec::new());
    parsed.write(&mut written_bytes).expect("failed to write");
    let written_bytes = written_bytes.into_inner();

    assert_eq!(
        original_bytes.len(),
        written_bytes.len(),
        "round-trip length mismatch for {}: original={}, written={}",
        path,
        original_bytes.len(),
        written_bytes.len()
    );
    assert_eq!(
        original_bytes, written_bytes,
        "round-trip bytes differ for {}",
        path
    );
}

// --- NestMembers (on the outer class) ---

#[test]
fn nest_members() {
    let c = load_class("java-assets/compiled-classes/NestExample.class");
    let attr = find_attr(&c.attributes, "NestMembers", &c)
        .expect("NestMembers attribute not found on NestExample");

    match attr {
        AttributeInfoVariant::NestMembers(nm) => {
            assert_eq!(nm.number_of_classes, 1);
            assert_eq!(nm.classes.len(), 1);
            // The single nest member should point to NestExample$Inner
        }
        other => panic!("Expected NestMembers, got {:?}", other),
    }
}

#[test]
fn nest_members_round_trip() {
    round_trip("java-assets/compiled-classes/NestExample.class");
}

// --- NestHost (on the inner class) ---

#[test]
fn nest_host() {
    let c = load_class("java-assets/compiled-classes/NestExample$Inner.class");
    let attr = find_attr(&c.attributes, "NestHost", &c)
        .expect("NestHost attribute not found on NestExample$Inner");

    match attr {
        AttributeInfoVariant::NestHost(nh) => {
            // host_class_index should point to NestExample class constant
            assert!(nh.host_class_index > 0);
        }
        other => panic!("Expected NestHost, got {:?}", other),
    }
}

#[test]
fn nest_host_round_trip() {
    round_trip("java-assets/compiled-classes/NestExample$Inner.class");
}

// --- Record ---

#[test]
fn record_attribute() {
    let c = load_class("java-assets/compiled-classes/RecordExample.class");
    let attr = find_attr(&c.attributes, "Record", &c)
        .expect("Record attribute not found on RecordExample");

    match attr {
        AttributeInfoVariant::Record(rec) => {
            assert_eq!(rec.components_count, 2);
            assert_eq!(rec.components.len(), 2);

            // First component: int x
            let comp0 = &rec.components[0];
            let name0 = match &c.const_pool[(comp0.name_index - 1) as usize] {
                classfile_parser::constant_info::ConstantInfo::Utf8(u) => u.utf8_string.as_str(),
                _ => panic!("expected Utf8"),
            };
            assert_eq!(name0, "x");

            // Second component: String name
            let comp1 = &rec.components[1];
            let name1 = match &c.const_pool[(comp1.name_index - 1) as usize] {
                classfile_parser::constant_info::ConstantInfo::Utf8(u) => u.utf8_string.as_str(),
                _ => panic!("expected Utf8"),
            };
            assert_eq!(name1, "name");

            // Record components may have Signature sub-attributes that should be interpreted
            for comp in &rec.components {
                for attr in &comp.attributes {
                    assert!(
                        attr.info_parsed.is_some(),
                        "Record component sub-attribute should have info_parsed populated"
                    );
                }
            }
        }
        other => panic!("Expected Record, got {:?}", other),
    }
}

#[test]
fn record_round_trip() {
    round_trip("java-assets/compiled-classes/RecordExample.class");
}

// --- PermittedSubclasses ---

#[test]
fn permitted_subclasses() {
    let c = load_class("java-assets/compiled-classes/SealedExample.class");
    let attr = find_attr(&c.attributes, "PermittedSubclasses", &c)
        .expect("PermittedSubclasses attribute not found on SealedExample");

    match attr {
        AttributeInfoVariant::PermittedSubclasses(ps) => {
            assert_eq!(ps.number_of_classes, 2);
            assert_eq!(ps.classes.len(), 2);
        }
        other => panic!("Expected PermittedSubclasses, got {:?}", other),
    }
}

#[test]
fn permitted_subclasses_round_trip() {
    round_trip("java-assets/compiled-classes/SealedExample.class");
}

// --- ModulePackages (byte-level test) ---

#[test]
fn module_packages_parse() {
    use classfile_parser::attribute_info::ModulePackagesAttribute;

    // ModulePackages { package_count: 2, package_index: [5, 10] }
    let bytes: Vec<u8> = vec![
        0x00, 0x02, // package_count = 2
        0x00, 0x05, // package_index[0] = 5
        0x00, 0x0A, // package_index[1] = 10
    ];

    let parsed = ModulePackagesAttribute::read(&mut Cursor::new(&bytes)).expect("failed to parse");
    assert_eq!(parsed.package_count, 2);
    assert_eq!(parsed.package_index, vec![5, 10]);

    // Round-trip
    let mut written = Cursor::new(Vec::new());
    parsed.write(&mut written).expect("failed to write");
    assert_eq!(written.into_inner(), bytes);
}

// --- ModuleMainClass (byte-level test) ---

#[test]
fn module_main_class_parse() {
    use classfile_parser::attribute_info::ModuleMainClassAttribute;

    // ModuleMainClass { main_class_index: 42 }
    let bytes: Vec<u8> = vec![0x00, 0x2A]; // 42

    let parsed = ModuleMainClassAttribute::read(&mut Cursor::new(&bytes)).expect("failed to parse");
    assert_eq!(parsed.main_class_index, 42);

    // Round-trip
    let mut written = Cursor::new(Vec::new());
    parsed.write(&mut written).expect("failed to write");
    assert_eq!(written.into_inner(), bytes);
}

// --- Code sub-attribute interpretation ---

#[test]
fn code_sub_attributes_are_interpreted() {
    // Verify that sub-attributes inside CodeAttribute now have info_parsed populated
    let c = load_class("java-assets/compiled-classes/BasicClass.class");

    for method in &c.methods {
        for attr in &method.attributes {
            if let Some(AttributeInfoVariant::Code(ref code)) = attr.info_parsed {
                for sub_attr in &code.attributes {
                    assert!(
                        sub_attr.info_parsed.is_some(),
                        "Code sub-attribute (name_index={}) should have info_parsed populated",
                        sub_attr.attribute_name_index
                    );
                    // Verify it's not Unknown
                    if let Some(AttributeInfoVariant::Unknown(name)) = &sub_attr.info_parsed {
                        panic!("Code sub-attribute '{}' parsed as Unknown", name);
                    }
                }
            }
        }
    }
}

#[test]
fn code_sub_attribute_line_number_table() {
    // Verify LineNumberTable inside Code is now directly accessible via info_parsed
    let c = load_class("java-assets/compiled-classes/BasicClass.class");

    let mut found_line_number_table = false;
    for method in &c.methods {
        for attr in &method.attributes {
            if let Some(AttributeInfoVariant::Code(ref code)) = attr.info_parsed {
                for sub_attr in &code.attributes {
                    if let Some(AttributeInfoVariant::LineNumberTable(ref lnt)) =
                        sub_attr.info_parsed
                    {
                        found_line_number_table = true;
                        assert!(
                            lnt.line_number_table_length > 0,
                            "LineNumberTable should have entries"
                        );
                        assert_eq!(
                            lnt.line_number_table.len(),
                            lnt.line_number_table_length as usize
                        );
                    }
                }
            }
        }
    }
    assert!(
        found_line_number_table,
        "Should have found at least one LineNumberTable sub-attribute"
    );
}
