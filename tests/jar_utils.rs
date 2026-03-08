#![cfg(feature = "jar-utils")]

extern crate classfile_parser;

use std::io::{Cursor, Write};

use classfile_parser::ClassFile;
use classfile_parser::jar_utils::{JarFile, JarManifest};

use binrw::BinRead;
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read a .class file from disk as raw bytes.
fn read_class_bytes(name: &str) -> Vec<u8> {
    let path = format!("java-assets/compiled-classes/{name}");
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"))
}

/// Build a minimal JAR in-memory with the given entries.
fn build_jar(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buf);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, data) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
    }
    buf.into_inner()
}

// ===========================================================================
// Manifest tests
// ===========================================================================

#[test]
fn test_manifest_parse() {
    let manifest_data = b"Manifest-Version: 1.0\r\nCreated-By: test\r\n\r\nName: com/example/Foo.class\r\nSHA-256-Digest: abc123\r\n";
    let manifest = JarManifest::parse(manifest_data).unwrap();

    assert_eq!(manifest.main_attr("Manifest-Version"), Some("1.0"));
    assert_eq!(manifest.main_attr("Created-By"), Some("test"));

    // Case-insensitive lookup
    assert_eq!(manifest.main_attr("manifest-version"), Some("1.0"));
    assert_eq!(manifest.main_attr("CREATED-BY"), Some("test"));

    let section = manifest.entry_section("com/example/Foo.class").unwrap();
    assert_eq!(section.get("SHA-256-Digest"), Some("abc123"));
}

#[test]
fn test_manifest_continuation_lines() {
    // A long value that wraps past 72 bytes
    let long_value = "a]".repeat(40); // 80 chars

    // Manually wrap: first line up to 72 bytes, then continuation
    let full_line = format!("Long-Key: {}", long_value);
    let first_72 = &full_line[..72];
    let rest = &full_line[72..];
    let wrapped = format!("Manifest-Version: 1.0\r\n{}\r\n {}\r\n", first_72, rest);

    let manifest = JarManifest::parse(wrapped.as_bytes()).unwrap();
    assert_eq!(manifest.main_attr("Long-Key"), Some(long_value.as_str()));
}

#[test]
fn test_manifest_round_trip() {
    let original = b"Manifest-Version: 1.0\r\nCreated-By: test\r\n\r\nName: com/example/A.class\r\nDigest: aaa\r\n\r\nName: com/example/B.class\r\nDigest: bbb\r\n";
    let manifest = JarManifest::parse(original).unwrap();
    let serialized = manifest.to_bytes();
    let reparsed = JarManifest::parse(&serialized).unwrap();

    assert_eq!(manifest.main_attributes, reparsed.main_attributes);
    assert_eq!(manifest.entries.len(), reparsed.entries.len());
    for (name, attrs) in &manifest.entries {
        assert_eq!(reparsed.entry_section(name).unwrap(), attrs);
    }
}

#[test]
fn test_default_manifest() {
    let m = JarManifest::default_manifest();
    assert_eq!(m.main_attr("Manifest-Version"), Some("1.0"));
    assert!(m.entries.is_empty());
}

// ===========================================================================
// JarFile I/O tests
// ===========================================================================

#[test]
fn test_read_and_list_entries() {
    let class_bytes = read_class_bytes("BasicClass.class");
    let jar_bytes = build_jar(&[
        ("com/example/BasicClass.class", &class_bytes),
        ("META-INF/MANIFEST.MF", b"Manifest-Version: 1.0\r\n"),
        ("readme.txt", b"hello"),
    ]);

    let jar = JarFile::from_bytes(&jar_bytes).unwrap();

    let names: Vec<&str> = jar.entry_names().collect();
    assert_eq!(names.len(), 3);
    assert!(names.contains(&"com/example/BasicClass.class"));
    assert!(names.contains(&"META-INF/MANIFEST.MF"));
    assert!(names.contains(&"readme.txt"));

    let class_names: Vec<&str> = jar.class_names().collect();
    assert_eq!(class_names.len(), 1);
    assert_eq!(class_names[0], "com/example/BasicClass.class");
}

#[test]
fn test_round_trip_jar() {
    let class_bytes = read_class_bytes("BasicClass.class");
    let manifest = b"Manifest-Version: 1.0\r\n";
    let jar_bytes = build_jar(&[
        ("com/example/BasicClass.class", &class_bytes),
        ("META-INF/MANIFEST.MF", manifest),
    ]);

    let jar = JarFile::from_bytes(&jar_bytes).unwrap();
    let rewritten = jar.to_bytes().unwrap();
    let jar2 = JarFile::from_bytes(&rewritten).unwrap();

    // Same entries
    let names1: Vec<&str> = jar.entry_names().collect();
    let names2: Vec<&str> = jar2.entry_names().collect();
    assert_eq!(names1, names2);

    // Same content
    for name in &names1 {
        assert_eq!(jar.get_entry(name), jar2.get_entry(name));
    }
}

#[test]
fn test_add_remove_entries() {
    let jar_bytes = build_jar(&[("a.txt", b"aaa")]);
    let mut jar = JarFile::from_bytes(&jar_bytes).unwrap();

    assert!(jar.contains_entry("a.txt"));
    assert!(!jar.contains_entry("b.txt"));

    jar.set_entry("b.txt", b"bbb".to_vec());
    assert!(jar.contains_entry("b.txt"));
    assert_eq!(jar.get_entry("b.txt"), Some(b"bbb".as_slice()));

    let removed = jar.remove_entry("a.txt");
    assert_eq!(removed, Some(b"aaa".to_vec()));
    assert!(!jar.contains_entry("a.txt"));
}

#[test]
fn test_open_and_save() {
    let class_bytes = read_class_bytes("BasicClass.class");
    let jar_bytes = build_jar(&[("com/example/BasicClass.class", &class_bytes)]);

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.jar");

    // Write manually then read with open()
    std::fs::write(&path, &jar_bytes).unwrap();
    let jar = JarFile::open(&path).unwrap();
    assert!(jar.contains_entry("com/example/BasicClass.class"));

    // Save and re-open
    let path2 = dir.path().join("test2.jar");
    jar.save(&path2).unwrap();
    let jar2 = JarFile::open(&path2).unwrap();
    assert_eq!(
        jar.get_entry("com/example/BasicClass.class"),
        jar2.get_entry("com/example/BasicClass.class"),
    );
}

// ===========================================================================
// ClassFile integration tests
// ===========================================================================

#[test]
fn test_parse_class_from_jar() {
    let class_bytes = read_class_bytes("BasicClass.class");

    // Parse directly for reference
    let direct = ClassFile::read(&mut Cursor::new(&class_bytes)).unwrap();

    // Parse from JAR
    let jar_bytes = build_jar(&[("BasicClass.class", &class_bytes)]);
    let jar = JarFile::from_bytes(&jar_bytes).unwrap();
    let from_jar = jar.parse_class("BasicClass.class").unwrap();

    assert_eq!(direct.major_version, from_jar.major_version);
    assert_eq!(direct.minor_version, from_jar.minor_version);
    assert_eq!(direct.const_pool_size, from_jar.const_pool_size);
    assert_eq!(direct.methods_count, from_jar.methods_count);
    assert_eq!(direct.fields_count, from_jar.fields_count);
}

#[test]
fn test_set_class_round_trip() {
    let class_bytes = read_class_bytes("BasicClass.class");
    let jar_bytes = build_jar(&[("BasicClass.class", &class_bytes)]);
    let mut jar = JarFile::from_bytes(&jar_bytes).unwrap();

    let class_file = jar.parse_class("BasicClass.class").unwrap();
    jar.set_class("BasicClass.class", &class_file).unwrap();

    let reparsed = jar.parse_class("BasicClass.class").unwrap();
    assert_eq!(class_file.major_version, reparsed.major_version);
    assert_eq!(class_file.const_pool_size, reparsed.const_pool_size);
    assert_eq!(class_file.methods_count, reparsed.methods_count);
}

#[test]
fn test_patch_class_in_jar() {
    let class_bytes = read_class_bytes("BasicClass.class");
    let jar_bytes = build_jar(&[("BasicClass.class", &class_bytes)]);
    let mut jar = JarFile::from_bytes(&jar_bytes).unwrap();

    let mut class_file = jar.parse_class("BasicClass.class").unwrap();

    // Add a UTF-8 constant as a marker
    let marker = "jar_utils_test_marker";
    class_file.add_utf8(marker);
    class_file.sync_counts();

    jar.set_class("BasicClass.class", &class_file).unwrap();

    // Re-parse and verify the marker exists
    let reparsed = jar.parse_class("BasicClass.class").unwrap();
    assert!(
        reparsed.find_utf8_index(marker).is_some(),
        "marker constant should be present after patching"
    );
}

#[test]
fn test_parse_all_classes() {
    let basic_bytes = read_class_bytes("BasicClass.class");
    let factorial_bytes = read_class_bytes("Factorial.class");
    let jar_bytes = build_jar(&[
        ("com/example/BasicClass.class", &basic_bytes),
        ("com/example/Factorial.class", &factorial_bytes),
        ("readme.txt", b"not a class"),
    ]);

    let jar = JarFile::from_bytes(&jar_bytes).unwrap();
    let results = jar.parse_all_classes();

    assert_eq!(results.len(), 2);
    for (name, result) in &results {
        assert!(name.ends_with(".class"));
        assert!(result.is_ok(), "failed to parse {name}");
    }
}

// ===========================================================================
// Manifest-in-JAR tests
// ===========================================================================

#[test]
fn test_manifest_in_jar() {
    let manifest_data = b"Manifest-Version: 1.0\r\nMain-Class: com.example.Main\r\n";
    let jar_bytes = build_jar(&[
        ("META-INF/MANIFEST.MF", manifest_data),
        ("readme.txt", b"hi"),
    ]);

    let mut jar = JarFile::from_bytes(&jar_bytes).unwrap();

    // Read manifest
    let manifest = jar.manifest().unwrap().unwrap();
    assert_eq!(manifest.main_attr("Main-Class"), Some("com.example.Main"));

    // Modify and store
    let mut updated = manifest.clone();
    updated.set_main_attr("Main-Class", "com.example.Other");
    jar.set_manifest(&updated);

    // Re-read
    let re_manifest = jar.manifest().unwrap().unwrap();
    assert_eq!(
        re_manifest.main_attr("Main-Class"),
        Some("com.example.Other")
    );
}
