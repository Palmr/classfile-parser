#![cfg(feature = "spring-utils")]

extern crate classfile_parser;

use std::io::{Cursor, Write};

use classfile_parser::jar_utils::{JarFile, JarManifest};
use classfile_parser::spring_utils::{
    ClasspathIndex, LayersIndex, SpringBootFormat, SpringBootJar,
};

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

/// Build a manifest for a Spring Boot JAR/WAR.
fn spring_manifest(format: SpringBootFormat, start_class: &str) -> Vec<u8> {
    let launcher = match format {
        SpringBootFormat::Jar => "org.springframework.boot.loader.JarLauncher",
        SpringBootFormat::War => "org.springframework.boot.loader.WarLauncher",
    };
    let mut m = JarManifest::default_manifest();
    m.set_main_attr("Main-Class", launcher);
    m.set_main_attr("Start-Class", start_class);
    m.set_main_attr("Spring-Boot-Version", "3.1.0");
    m.set_main_attr("Spring-Boot-Classes", format.classes_dir());
    m.set_main_attr("Spring-Boot-Lib", format.lib_dir());
    m.to_bytes()
}

/// Build a small JAR in-memory (for nesting inside a fat JAR).
fn build_inner_jar(entries: &[(&str, &[u8])]) -> Vec<u8> {
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

/// Build a complete Spring Boot fat JAR in-memory.
///
/// `app_classes`: entries under `{prefix}/classes/`
/// `nested_jars`: entries under `{prefix}/lib/` (name, jar bytes)
/// `loader_classes`: entries under `org/springframework/boot/loader/`
/// `classpath_idx`: optional classpath.idx content
/// `layers_idx`: optional layers.idx content
fn build_spring_jar(
    format: SpringBootFormat,
    start_class: &str,
    app_classes: &[(&str, &[u8])],
    app_resources: &[(&str, &[u8])],
    nested_jars: &[(&str, &[u8])],
    loader_classes: &[&str],
    classpath_idx: Option<&[u8]>,
    layers_idx: Option<&[u8]>,
) -> Vec<u8> {
    let manifest = spring_manifest(format, start_class);
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();

    // Manifest
    entries.push(("META-INF/MANIFEST.MF".to_string(), manifest));

    // App classes
    let classes_dir = format.classes_dir();
    for (name, data) in app_classes {
        entries.push((format!("{classes_dir}{name}"), data.to_vec()));
    }

    // App resources
    for (name, data) in app_resources {
        entries.push((format!("{classes_dir}{name}"), data.to_vec()));
    }

    // Nested JARs
    let lib_dir = format.lib_dir();
    for (name, data) in nested_jars {
        entries.push((format!("{lib_dir}{name}"), data.to_vec()));
    }

    // Loader classes
    for name in loader_classes {
        entries.push((
            format!("org/springframework/boot/loader/{name}"),
            b"fake-loader-class".to_vec(),
        ));
    }

    // Index files
    if let Some(data) = classpath_idx {
        entries.push((format!("{}/classpath.idx", format.prefix()), data.to_vec()));
    }
    if let Some(data) = layers_idx {
        entries.push((format!("{}/layers.idx", format.prefix()), data.to_vec()));
    }

    // Build the ZIP
    let mut buf = Cursor::new(Vec::new());
    {
        let mut writer = zip::ZipWriter::new(&mut buf);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, data) in &entries {
            writer.start_file(name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
    }
    buf.into_inner()
}

// ===========================================================================
// Detection tests
// ===========================================================================

#[test]
fn test_detect_jar_format() {
    let jar_bytes = build_spring_jar(
        SpringBootFormat::Jar,
        "com.example.App",
        &[],
        &[],
        &[],
        &[],
        None,
        None,
    );
    let spring = SpringBootJar::from_bytes(&jar_bytes).unwrap();
    assert!(spring.is_some());
    assert_eq!(spring.unwrap().format(), SpringBootFormat::Jar);
}

#[test]
fn test_detect_war_format() {
    let jar_bytes = build_spring_jar(
        SpringBootFormat::War,
        "com.example.App",
        &[],
        &[],
        &[],
        &[],
        None,
        None,
    );
    let spring = SpringBootJar::from_bytes(&jar_bytes).unwrap();
    assert!(spring.is_some());
    assert_eq!(spring.unwrap().format(), SpringBootFormat::War);
}

#[test]
fn test_detect_spring_boot_3_2_launcher() {
    // Use the Spring Boot 3.2+ launcher class name
    let mut m = JarManifest::default_manifest();
    m.set_main_attr(
        "Main-Class",
        "org.springframework.boot.loader.launch.JarLauncher",
    );
    m.set_main_attr("Start-Class", "com.example.App");
    let manifest_bytes = m.to_bytes();

    let inner_jar = build_inner_jar(&[("META-INF/MANIFEST.MF", &manifest_bytes)]);
    let jar = JarFile::from_bytes(&inner_jar).unwrap();
    let format = classfile_parser::spring_utils::detect_format(&jar);
    assert_eq!(format, Some(SpringBootFormat::Jar));
}

#[test]
fn test_detect_not_spring_boot() {
    // Plain JAR with no Spring Boot launcher
    let mut m = JarManifest::default_manifest();
    m.set_main_attr("Main-Class", "com.example.Main");
    let manifest_bytes = m.to_bytes();

    let inner_jar = build_inner_jar(&[("META-INF/MANIFEST.MF", &manifest_bytes)]);
    let result = SpringBootJar::from_bytes(&inner_jar).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_detect_no_start_class() {
    // Has JarLauncher but no Start-Class → not detected
    let mut m = JarManifest::default_manifest();
    m.set_main_attr("Main-Class", "org.springframework.boot.loader.JarLauncher");
    let manifest_bytes = m.to_bytes();

    let inner_jar = build_inner_jar(&[("META-INF/MANIFEST.MF", &manifest_bytes)]);
    let result = SpringBootJar::from_bytes(&inner_jar).unwrap();
    assert!(result.is_none());
}

// ===========================================================================
// Manifest shortcut tests
// ===========================================================================

#[test]
fn test_manifest_shortcuts() {
    let jar_bytes = build_spring_jar(
        SpringBootFormat::Jar,
        "com.example.MyApp",
        &[],
        &[],
        &[],
        &[],
        None,
        None,
    );
    let spring = SpringBootJar::from_bytes(&jar_bytes).unwrap().unwrap();

    assert_eq!(
        spring.start_class().unwrap(),
        Some("com.example.MyApp".to_string())
    );
    assert_eq!(
        spring.spring_boot_version().unwrap(),
        Some("3.1.0".to_string())
    );
    assert_eq!(
        spring.spring_boot_classes_path().unwrap(),
        Some("BOOT-INF/classes/".to_string())
    );
    assert_eq!(
        spring.spring_boot_lib_path().unwrap(),
        Some("BOOT-INF/lib/".to_string())
    );
}

#[test]
fn test_manifest_shortcuts_missing() {
    // Build a minimal manifest without version/paths
    let mut m = JarManifest::default_manifest();
    m.set_main_attr("Main-Class", "org.springframework.boot.loader.JarLauncher");
    m.set_main_attr("Start-Class", "com.example.App");
    let manifest_bytes = m.to_bytes();

    let jar_bytes = build_inner_jar(&[("META-INF/MANIFEST.MF", &manifest_bytes)]);
    let spring = SpringBootJar::from_bytes(&jar_bytes).unwrap().unwrap();

    assert_eq!(
        spring.start_class().unwrap(),
        Some("com.example.App".to_string())
    );
    assert_eq!(spring.spring_boot_version().unwrap(), None);
    assert_eq!(spring.spring_boot_classes_path().unwrap(), None);
    assert_eq!(spring.spring_boot_lib_path().unwrap(), None);
}

// ===========================================================================
// Entry iteration tests
// ===========================================================================

#[test]
fn test_app_class_names() {
    let class_bytes = read_class_bytes("BasicClass.class");
    let jar_bytes = build_spring_jar(
        SpringBootFormat::Jar,
        "com.example.App",
        &[
            ("com/example/Foo.class", &class_bytes),
            ("com/example/Bar.class", &class_bytes),
        ],
        &[("application.properties", b"key=value")],
        &[],
        &[],
        None,
        None,
    );
    let spring = SpringBootJar::from_bytes(&jar_bytes).unwrap().unwrap();

    let class_names: Vec<&str> = spring.app_class_names().collect();
    assert_eq!(class_names.len(), 2);
    assert!(
        class_names
            .iter()
            .all(|n| n.starts_with("BOOT-INF/classes/") && n.ends_with(".class"))
    );
}

#[test]
fn test_app_resource_names() {
    let class_bytes = read_class_bytes("BasicClass.class");
    let jar_bytes = build_spring_jar(
        SpringBootFormat::Jar,
        "com.example.App",
        &[("com/example/Foo.class", &class_bytes)],
        &[
            ("application.properties", b"key=value"),
            ("static/index.html", b"<html></html>"),
        ],
        &[],
        &[],
        None,
        None,
    );
    let spring = SpringBootJar::from_bytes(&jar_bytes).unwrap().unwrap();

    let resources: Vec<&str> = spring.app_resource_names().collect();
    assert_eq!(resources.len(), 2);
    assert!(resources.iter().all(|n| !n.ends_with(".class")));
}

#[test]
fn test_loader_class_names() {
    let jar_bytes = build_spring_jar(
        SpringBootFormat::Jar,
        "com.example.App",
        &[],
        &[],
        &[],
        &["JarLauncher.class", "LaunchedURLClassLoader.class"],
        None,
        None,
    );
    let spring = SpringBootJar::from_bytes(&jar_bytes).unwrap().unwrap();

    let loaders: Vec<&str> = spring.loader_class_names().collect();
    assert_eq!(loaders.len(), 2);
    assert!(
        loaders
            .iter()
            .all(|n| n.starts_with("org/springframework/boot/loader/"))
    );
}

#[test]
fn test_nested_jar_names() {
    let inner = build_inner_jar(&[("com/lib/A.class", b"fake-class")]);
    let inner2 = build_inner_jar(&[("com/lib/B.class", b"fake-class")]);
    let jar_bytes = build_spring_jar(
        SpringBootFormat::Jar,
        "com.example.App",
        &[],
        &[],
        &[("dep-a.jar", &inner), ("dep-b.jar", &inner2)],
        &[],
        None,
        None,
    );
    let spring = SpringBootJar::from_bytes(&jar_bytes).unwrap().unwrap();

    let nested: Vec<&str> = spring.nested_jar_names().collect();
    assert_eq!(nested.len(), 2);
    assert!(
        nested
            .iter()
            .all(|n| n.starts_with("BOOT-INF/lib/") && n.ends_with(".jar"))
    );
}

// ===========================================================================
// Nested JAR tests
// ===========================================================================

#[test]
fn test_open_nested_jar() {
    let inner = build_inner_jar(&[
        ("com/lib/Helper.class", b"fake-class"),
        ("META-INF/MANIFEST.MF", b"Manifest-Version: 1.0\r\n"),
    ]);
    let jar_bytes = build_spring_jar(
        SpringBootFormat::Jar,
        "com.example.App",
        &[],
        &[],
        &[("helper-lib.jar", &inner)],
        &[],
        None,
        None,
    );
    let spring = SpringBootJar::from_bytes(&jar_bytes).unwrap().unwrap();

    let nested = spring
        .open_nested_jar("BOOT-INF/lib/helper-lib.jar")
        .unwrap();
    let names: Vec<&str> = nested.entry_names().collect();
    assert_eq!(names.len(), 2);
    assert!(nested.contains_entry("com/lib/Helper.class"));
}

#[test]
fn test_open_nested_jar_not_found() {
    let jar_bytes = build_spring_jar(
        SpringBootFormat::Jar,
        "com.example.App",
        &[],
        &[],
        &[],
        &[],
        None,
        None,
    );
    let spring = SpringBootJar::from_bytes(&jar_bytes).unwrap().unwrap();

    let result = spring.open_nested_jar("BOOT-INF/lib/nonexistent.jar");
    assert!(result.is_err());
}

#[test]
fn test_parse_nested_class() {
    let class_bytes = read_class_bytes("BasicClass.class");
    let inner = build_inner_jar(&[("com/dep/BasicClass.class", &class_bytes)]);
    let jar_bytes = build_spring_jar(
        SpringBootFormat::Jar,
        "com.example.App",
        &[],
        &[],
        &[("dep.jar", &inner)],
        &[],
        None,
        None,
    );
    let spring = SpringBootJar::from_bytes(&jar_bytes).unwrap().unwrap();

    let class_file = spring
        .parse_nested_class("BOOT-INF/lib/dep.jar", "com/dep/BasicClass.class")
        .unwrap();
    assert!(class_file.major_version >= 45);
}

#[test]
fn test_open_all_nested_jars() {
    let inner1 = build_inner_jar(&[("A.class", b"fake")]);
    let inner2 = build_inner_jar(&[("B.class", b"fake")]);
    let inner3 = build_inner_jar(&[("C.class", b"fake")]);
    let jar_bytes = build_spring_jar(
        SpringBootFormat::Jar,
        "com.example.App",
        &[],
        &[],
        &[("a.jar", &inner1), ("b.jar", &inner2), ("c.jar", &inner3)],
        &[],
        None,
        None,
    );
    let spring = SpringBootJar::from_bytes(&jar_bytes).unwrap().unwrap();

    let all = spring.open_all_nested_jars();
    assert_eq!(all.len(), 3);
    assert!(all.iter().all(|(_, r)| r.is_ok()));
}

// ===========================================================================
// App class tests
// ===========================================================================

#[test]
fn test_parse_app_class() {
    let class_bytes = read_class_bytes("BasicClass.class");
    let jar_bytes = build_spring_jar(
        SpringBootFormat::Jar,
        "com.example.App",
        &[("com/example/BasicClass.class", &class_bytes)],
        &[],
        &[],
        &[],
        None,
        None,
    );
    let spring = SpringBootJar::from_bytes(&jar_bytes).unwrap().unwrap();

    let cf = spring
        .parse_app_class("BOOT-INF/classes/com/example/BasicClass.class")
        .unwrap();
    assert!(cf.major_version >= 45);
}

#[test]
fn test_parse_all_app_classes() {
    let class_bytes = read_class_bytes("BasicClass.class");
    let jar_bytes = build_spring_jar(
        SpringBootFormat::Jar,
        "com.example.App",
        &[
            ("com/example/A.class", &class_bytes),
            ("com/example/B.class", &class_bytes),
        ],
        &[("application.properties", b"key=val")],
        &[],
        &[],
        None,
        None,
    );
    let spring = SpringBootJar::from_bytes(&jar_bytes).unwrap().unwrap();

    let results = spring.parse_all_app_classes();
    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|(_, r)| r.is_ok()));
}

// ===========================================================================
// ClasspathIndex tests
// ===========================================================================

#[test]
fn test_classpath_index_parse() {
    let data = b"- \"BOOT-INF/lib/spring-core.jar\"\n- \"BOOT-INF/lib/spring-web.jar\"\n";
    let idx = ClasspathIndex::parse(data).unwrap();

    assert_eq!(idx.len(), 2);
    assert_eq!(idx.entries()[0], "BOOT-INF/lib/spring-core.jar");
    assert_eq!(idx.entries()[1], "BOOT-INF/lib/spring-web.jar");
    assert!(idx.contains("BOOT-INF/lib/spring-core.jar"));
    assert!(!idx.contains("BOOT-INF/lib/nonexistent.jar"));
    assert!(!idx.is_empty());
}

#[test]
fn test_classpath_index_round_trip() {
    let data = b"- \"BOOT-INF/lib/a.jar\"\n- \"BOOT-INF/lib/b.jar\"\n- \"BOOT-INF/lib/c.jar\"\n";
    let idx = ClasspathIndex::parse(data).unwrap();
    let serialized = idx.to_bytes();
    assert_eq!(serialized, data.to_vec());
}

#[test]
fn test_classpath_index_from_jar() {
    let cp_data = b"- \"BOOT-INF/lib/dep-a.jar\"\n- \"BOOT-INF/lib/dep-b.jar\"\n";
    let jar_bytes = build_spring_jar(
        SpringBootFormat::Jar,
        "com.example.App",
        &[],
        &[],
        &[],
        &[],
        Some(cp_data),
        None,
    );
    let spring = SpringBootJar::from_bytes(&jar_bytes).unwrap().unwrap();

    let idx = spring.classpath_index().unwrap().unwrap();
    assert_eq!(idx.len(), 2);
    assert!(idx.contains("BOOT-INF/lib/dep-a.jar"));
}

#[test]
fn test_classpath_index_missing() {
    let jar_bytes = build_spring_jar(
        SpringBootFormat::Jar,
        "com.example.App",
        &[],
        &[],
        &[],
        &[],
        None,
        None,
    );
    let spring = SpringBootJar::from_bytes(&jar_bytes).unwrap().unwrap();

    let idx = spring.classpath_index().unwrap();
    assert!(idx.is_none());
}

// ===========================================================================
// LayersIndex tests
// ===========================================================================

#[test]
fn test_layers_index_parse() {
    let data = b"- \"dependencies\":\n  - \"BOOT-INF/lib/\"\n- \"application\":\n  - \"BOOT-INF/classes/\"\n  - \"BOOT-INF/lib/app-dep.jar\"\n";
    let idx = LayersIndex::parse(data).unwrap();

    assert_eq!(idx.len(), 2);
    assert_eq!(idx.layers()[0].name, "dependencies");
    assert_eq!(idx.layers()[0].paths, vec!["BOOT-INF/lib/"]);
    assert_eq!(idx.layers()[1].name, "application");
    assert_eq!(idx.layers()[1].paths.len(), 2);
}

#[test]
fn test_layers_index_round_trip() {
    let data = "- \"dependencies\":\n  - \"BOOT-INF/lib/\"\n- \"application\":\n  - \"BOOT-INF/classes/\"\n";
    let idx = LayersIndex::parse(data.as_bytes()).unwrap();
    let serialized = idx.to_bytes();
    assert_eq!(String::from_utf8(serialized).unwrap(), data);
}

#[test]
fn test_layers_index_empty_layer() {
    let data = b"- \"empty-layer\":\n- \"nonempty\":\n  - \"BOOT-INF/classes/\"\n";
    let idx = LayersIndex::parse(data).unwrap();

    assert_eq!(idx.len(), 2);
    assert!(idx.find_layer("empty-layer").unwrap().paths.is_empty());
    assert_eq!(idx.find_layer("nonempty").unwrap().paths.len(), 1);
}

#[test]
fn test_layers_index_find_layer() {
    let data = b"- \"deps\":\n  - \"BOOT-INF/lib/\"\n- \"app\":\n  - \"BOOT-INF/classes/\"\n";
    let idx = LayersIndex::parse(data).unwrap();

    assert!(idx.find_layer("deps").is_some());
    assert!(idx.find_layer("app").is_some());
    assert!(idx.find_layer("nonexistent").is_none());

    let names: Vec<&str> = idx.layer_names().collect();
    assert_eq!(names, vec!["deps", "app"]);
}

#[test]
fn test_layers_index_layer_for_path() {
    let data = b"- \"dependencies\":\n  - \"BOOT-INF/lib/\"\n- \"application\":\n  - \"BOOT-INF/classes/\"\n";
    let idx = LayersIndex::parse(data).unwrap();

    assert_eq!(
        idx.layer_for_path("BOOT-INF/lib/spring-core.jar"),
        Some("dependencies")
    );
    assert_eq!(
        idx.layer_for_path("BOOT-INF/classes/com/example/App.class"),
        Some("application")
    );
    assert_eq!(idx.layer_for_path("META-INF/MANIFEST.MF"), None);
}

// ===========================================================================
// WAR variant test
// ===========================================================================

#[test]
fn test_war_variant() {
    let class_bytes = read_class_bytes("BasicClass.class");
    let inner = build_inner_jar(&[("com/lib/A.class", b"fake")]);
    let jar_bytes = build_spring_jar(
        SpringBootFormat::War,
        "com.example.WarApp",
        &[("com/example/Svc.class", &class_bytes)],
        &[("application.yml", b"server:\n  port: 8080\n")],
        &[("dep.jar", &inner)],
        &["JarLauncher.class"],
        None,
        None,
    );
    let spring = SpringBootJar::from_bytes(&jar_bytes).unwrap().unwrap();

    assert_eq!(spring.format(), SpringBootFormat::War);
    assert_eq!(
        spring.start_class().unwrap(),
        Some("com.example.WarApp".to_string())
    );

    let classes: Vec<&str> = spring.app_class_names().collect();
    assert_eq!(classes.len(), 1);
    assert!(classes[0].starts_with("WEB-INF/classes/"));

    let resources: Vec<&str> = spring.app_resource_names().collect();
    assert_eq!(resources.len(), 1);

    let nested: Vec<&str> = spring.nested_jar_names().collect();
    assert_eq!(nested.len(), 1);
    assert!(nested[0].starts_with("WEB-INF/lib/"));
}

// ===========================================================================
// Integration test
// ===========================================================================

#[test]
fn test_full_fat_jar_analysis() {
    let class_bytes = read_class_bytes("BasicClass.class");
    let factorial_bytes = read_class_bytes("Factorial.class");

    // Build nested JARs with real classes
    let dep_jar = build_inner_jar(&[("com/dep/BasicClass.class", &class_bytes)]);
    let util_jar = build_inner_jar(&[("com/util/Factorial.class", &factorial_bytes)]);

    let cp_idx = b"- \"BOOT-INF/lib/dep.jar\"\n- \"BOOT-INF/lib/util.jar\"\n";
    let layers_idx = "- \"dependencies\":\n  - \"BOOT-INF/lib/\"\n- \"application\":\n  - \"BOOT-INF/classes/\"\n";

    let jar_bytes = build_spring_jar(
        SpringBootFormat::Jar,
        "com.example.Main",
        &[
            ("com/example/Main.class", &class_bytes),
            ("com/example/Service.class", &factorial_bytes),
        ],
        &[("application.properties", b"spring.application.name=test")],
        &[("dep.jar", &dep_jar), ("util.jar", &util_jar)],
        &["JarLauncher.class", "LaunchedURLClassLoader.class"],
        Some(cp_idx),
        Some(layers_idx.as_bytes()),
    );

    let spring = SpringBootJar::from_bytes(&jar_bytes).unwrap().unwrap();

    // Format
    assert_eq!(spring.format(), SpringBootFormat::Jar);

    // Manifest
    assert_eq!(
        spring.start_class().unwrap(),
        Some("com.example.Main".to_string())
    );
    assert_eq!(
        spring.spring_boot_version().unwrap(),
        Some("3.1.0".to_string())
    );

    // App classes
    let app_classes: Vec<&str> = spring.app_class_names().collect();
    assert_eq!(app_classes.len(), 2);
    let parsed = spring.parse_all_app_classes();
    assert_eq!(parsed.len(), 2);
    assert!(parsed.iter().all(|(_, r)| r.is_ok()));

    // App resources
    let resources: Vec<&str> = spring.app_resource_names().collect();
    assert_eq!(resources.len(), 1);

    // Loader classes
    let loaders: Vec<&str> = spring.loader_class_names().collect();
    assert_eq!(loaders.len(), 2);

    // Nested JARs
    let nested: Vec<&str> = spring.nested_jar_names().collect();
    assert_eq!(nested.len(), 2);

    let all_nested = spring.open_all_nested_jars();
    assert_eq!(all_nested.len(), 2);
    assert!(all_nested.iter().all(|(_, r)| r.is_ok()));

    // Parse class from nested JAR
    let cf = spring
        .parse_nested_class("BOOT-INF/lib/dep.jar", "com/dep/BasicClass.class")
        .unwrap();
    assert!(cf.major_version >= 45);

    // Classpath index
    let cp = spring.classpath_index().unwrap().unwrap();
    assert_eq!(cp.len(), 2);
    assert!(cp.contains("BOOT-INF/lib/dep.jar"));
    assert!(cp.contains("BOOT-INF/lib/util.jar"));

    // Layers index
    let layers = spring.layers_index().unwrap().unwrap();
    assert_eq!(layers.len(), 2);
    assert_eq!(
        layers.layer_for_path("BOOT-INF/lib/dep.jar"),
        Some("dependencies")
    );
    assert_eq!(
        layers.layer_for_path("BOOT-INF/classes/com/example/Main.class"),
        Some("application")
    );
}
