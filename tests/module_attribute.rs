use std::io::Cursor;

use binrw::prelude::*;
use classfile_parser::ClassFile;
use classfile_parser::attribute_info::AttributeInfoVariant;
use classfile_parser::constant_info::ConstantInfo;

fn lookup_string(c: &ClassFile, index: u16) -> Option<String> {
    let con = &c.const_pool[(index - 1) as usize];
    match con {
        ConstantInfo::Utf8(utf8) => Some(utf8.utf8_string.to_string()),
        ConstantInfo::Module(m) => lookup_string(c, m.name_index),
        ConstantInfo::Package(p) => lookup_string(c, p.name_index),
        _ => None,
    }
}

#[test]
fn module_info() {
    let class_bytes = include_bytes!("../java-assets/compiled-classes/module-info.class");
    let class = ClassFile::read(&mut Cursor::new(class_bytes.as_slice()))
        .expect("failed to parse module-info.class");

    // module-info.class should have ACC_MODULE set
    assert!(
        class
            .access_flags
            .contains(classfile_parser::ClassAccessFlags::MODULE),
        "expected ACC_MODULE flag"
    );

    // Find the Module attribute in the class-level attributes
    let module_attr = class
        .attributes
        .iter()
        .find_map(|a| match &a.info_parsed {
            Some(AttributeInfoVariant::Module(m)) => Some(m),
            _ => None,
        })
        .expect("Module attribute not found");

    // Verify structural fields
    assert_eq!(module_attr.module_flags, 0);
    assert_eq!(module_attr.module_version_index, 0);

    assert_eq!(module_attr.requires_count, 1);
    assert_eq!(module_attr.requires.len(), 1);
    let req = &module_attr.requires[0];
    assert_eq!(req.requires_flags, 32768); // ACC_MANDATED

    assert_eq!(module_attr.exports_count, 1);
    assert_eq!(module_attr.exports.len(), 1);
    let exp = &module_attr.exports[0];
    assert_eq!(exp.exports_flags, 0);
    assert_eq!(exp.exports_to_count, 0);
    assert_eq!(exp.exports_to_index.len(), 0);

    assert_eq!(module_attr.opens_count, 0);
    assert_eq!(module_attr.opens.len(), 0);
    assert_eq!(module_attr.uses_count, 0);
    assert_eq!(module_attr.uses.len(), 0);
    assert_eq!(module_attr.provides_count, 0);
    assert_eq!(module_attr.provides.len(), 0);

    // Verify string lookups via constant pool
    assert_eq!(
        lookup_string(&class, module_attr.module_name_index)
            .unwrap()
            .as_str(),
        "my.module"
    );
    assert_eq!(
        lookup_string(&class, req.requires_index).unwrap().as_str(),
        "java.base"
    );
    assert_eq!(
        lookup_string(&class, exp.exports_index).unwrap().as_str(),
        "com/some"
    );
}

#[test]
fn module_info_round_trip() {
    let original_bytes =
        include_bytes!("../java-assets/compiled-classes/module-info.class").to_vec();
    let parsed = ClassFile::read(&mut Cursor::new(original_bytes.as_slice()))
        .expect("failed to parse module-info.class");

    let mut written_bytes = Cursor::new(Vec::new());
    parsed
        .write(&mut written_bytes)
        .expect("failed to write module-info.class");
    let written_bytes = written_bytes.into_inner();

    assert_eq!(
        original_bytes.len(),
        written_bytes.len(),
        "written class file has different length: original={}, written={}",
        original_bytes.len(),
        written_bytes.len()
    );
    assert_eq!(
        original_bytes, written_bytes,
        "written class file bytes differ from original"
    );
}
