#![cfg(feature = "decompile")]

use std::fs;
use std::io::Cursor;

use binrw::prelude::*;
use classfile_parser::ClassFile;
use classfile_parser::decompile::cfg;
use classfile_parser::decompile::descriptor;
use classfile_parser::decompile::stack_sim;
use classfile_parser::decompile::structuring;
use classfile_parser::decompile::{self, DecompileOptions, Decompiler, RenderConfig};

fn load_class(name: &str) -> ClassFile {
    let path = format!("java-assets/compiled-classes/{}", name);
    let bytes = fs::read(&path).unwrap_or_else(|_| panic!("Failed to read {}", path));
    ClassFile::read(&mut Cursor::new(bytes)).unwrap_or_else(|_| panic!("Failed to parse {}", path))
}

// ---- Descriptor tests ----

#[test]
fn test_descriptor_parsing() {
    assert_eq!(
        descriptor::parse_method_descriptor("(Ljava/lang/String;I)V")
            .unwrap()
            .0
            .len(),
        2
    );
    assert_eq!(
        descriptor::parse_type_descriptor("[Ljava/lang/Object;").unwrap(),
        descriptor::JvmType::Array(Box::new(descriptor::JvmType::Reference(
            "java/lang/Object".into()
        )))
    );
}

// ---- Phase 1: CFG tests ----

#[test]
fn test_cfg_basic_class() {
    let class = load_class("BasicClass.class");
    for method in &class.methods {
        if let Some(code) = method.code() {
            let cfg = cfg::build_cfg(code);
            assert!(!cfg.blocks.is_empty(), "CFG should have at least one block");
            assert!(cfg.blocks.contains_key(&0), "CFG should start at address 0");
        }
    }
}

#[test]
fn test_cfg_factorial() {
    let class = load_class("Factorial.class");
    let method = class
        .find_method("factorial")
        .expect("factorial method should exist");
    let code = method.code().expect("factorial should have code");
    let cfg = cfg::build_cfg(code);

    // Factorial has branches, so it should have multiple blocks
    assert!(
        cfg.blocks.len() > 1,
        "Factorial CFG should have multiple blocks, got {}",
        cfg.blocks.len()
    );

    // Should have at least one conditional branch
    let has_conditional = cfg.blocks.values().any(|b| {
        matches!(
            b.terminator,
            classfile_parser::decompile::cfg_types::Terminator::ConditionalBranch { .. }
        )
    });
    assert!(
        has_conditional,
        "Factorial CFG should have conditional branches"
    );
}

#[test]
fn test_cfg_instructions_switches() {
    let class = load_class("Instructions.class");
    for method in &class.methods {
        if let Some(code) = method.code() {
            let cfg = cfg::build_cfg(code);
            assert!(!cfg.blocks.is_empty());
        }
    }
}

#[test]
fn test_cfg_dot_output() {
    let class = load_class("BasicClass.class");
    if let Some(code) = class.methods.first().and_then(|m| m.code()) {
        let cfg = cfg::build_cfg(code);
        let dot = cfg.to_dot();
        assert!(dot.contains("digraph CFG"));
        assert!(dot.contains("->") || cfg.blocks.len() <= 1);
    }
}

#[test]
fn test_cfg_reverse_postorder() {
    let class = load_class("Factorial.class");
    let method = class.find_method("factorial").expect("factorial method");
    let code = method.code().expect("code");
    let cfg = cfg::build_cfg(code);
    let rpo = cfg.reverse_postorder();
    assert!(!rpo.is_empty());
    assert_eq!(rpo[0], 0, "RPO should start with entry block");
}

// ---- Phase 2: Stack simulation tests ----

#[test]
fn test_stack_sim_basic_class() {
    let class = load_class("BasicClass.class");
    for method in &class.methods {
        if let Some(code) = method.code() {
            let is_static = method
                .access_flags
                .contains(classfile_parser::method_info::MethodAccessFlags::STATIC);
            let cfg = cfg::build_cfg(code);
            let simulated =
                stack_sim::simulate_all_blocks(&cfg, &class.const_pool, code, is_static);
            assert!(!simulated.is_empty(), "Should have simulated blocks");
        }
    }
}

#[test]
fn test_stack_sim_hello_world() {
    let class = load_class("HelloWorld.class");
    let method = class.find_method("main").expect("main method should exist");
    let code = method.code().expect("main should have code");
    let cfg = cfg::build_cfg(code);
    let simulated = stack_sim::simulate_all_blocks(&cfg, &class.const_pool, code, true);

    // main method should produce at least a method call statement (System.out.println)
    let total_stmts: usize = simulated.iter().map(|b| b.statements.len()).sum();
    assert!(
        total_stmts > 0,
        "Should have at least one statement in main()"
    );
}

// ---- Phase 3: Structuring tests ----

#[test]
fn test_structuring_basic() {
    let class = load_class("BasicClass.class");
    for method in &class.methods {
        if let Some(code) = method.code() {
            let is_static = method
                .access_flags
                .contains(classfile_parser::method_info::MethodAccessFlags::STATIC);
            let cfg = cfg::build_cfg(code);
            let simulated =
                stack_sim::simulate_all_blocks(&cfg, &class.const_pool, code, is_static);
            let body = structuring::structure_method(&cfg, &simulated, &class.const_pool);
            assert!(!body.statements.is_empty() || code.code.is_empty());
        }
    }
}

// ---- Phase 6: Full decompilation tests ----

#[test]
fn test_decompile_basic_class() {
    let class = load_class("BasicClass.class");
    let result = decompile::decompile(&class).expect("decompilation should succeed");
    assert!(
        result.contains("class"),
        "Output should contain 'class' keyword"
    );
    println!("--- BasicClass decompilation ---\n{}", result);
}

#[test]
fn test_decompile_hello_world() {
    let class = load_class("HelloWorld.class");
    let result = decompile::decompile(&class).expect("decompilation should succeed");
    assert!(
        result.contains("class HelloWorld"),
        "Should contain class name"
    );
    assert!(result.contains("main"), "Should contain main method");
    println!("--- HelloWorld decompilation ---\n{}", result);
}

#[test]
fn test_decompile_factorial() {
    let class = load_class("Factorial.class");
    let result = decompile::decompile(&class).expect("decompilation should succeed");
    assert!(
        result.contains("factorial"),
        "Should contain factorial method"
    );
    println!("--- Factorial decompilation ---\n{}", result);
}

#[test]
fn test_decompile_instructions() {
    let class = load_class("Instructions.class");
    let result = decompile::decompile(&class).expect("decompilation should succeed");
    assert!(result.contains("class"), "Should produce output");
    println!("--- Instructions decompilation ---\n{}", result);
}

#[test]
fn test_decompile_record() {
    let class = load_class("RecordExample.class");
    let result = decompile::decompile(&class).expect("decompilation should succeed");
    assert!(result.contains("record"), "Should contain 'record' keyword");
    println!("--- RecordExample decompilation ---\n{}", result);
}

#[test]
fn test_decompile_sealed() {
    let class = load_class("SealedExample.class");
    let result = decompile::decompile(&class).expect("decompilation should succeed");
    assert!(result.contains("sealed"), "Should contain 'sealed' keyword");
    assert!(
        result.contains("permits"),
        "Should contain 'permits' keyword"
    );
    println!("--- SealedExample decompilation ---\n{}", result);
}

#[test]
fn test_decompile_annotations() {
    let class = load_class("Annotations.class");
    let result = decompile::decompile(&class).expect("decompilation should succeed");
    assert!(result.contains("@"), "Should contain annotation markers");
    println!("--- Annotations decompilation ---\n{}", result);
}

#[test]
fn test_decompile_with_options() {
    let class = load_class("BasicClass.class");
    let options = DecompileOptions {
        render_config: RenderConfig {
            indent: "  ".into(),
            max_line_width: 80,
            use_var: false,
            include_synthetic: true,
        },
        include_synthetic: true,
        ..Default::default()
    };
    let decompiler = Decompiler::new(options);
    let result = decompiler
        .decompile(&class)
        .expect("decompilation should succeed");
    assert!(result.contains("class"), "Should produce output");
}

#[test]
fn test_decompile_inner_classes() {
    let outer = load_class("NestExample.class");
    let inner = load_class("NestExample$Inner.class");
    let decompiler = Decompiler::new(DecompileOptions::default());
    let result = decompiler
        .decompile_with_inner_classes(&outer, &[&inner])
        .expect("decompilation should succeed");
    assert!(result.contains("class"), "Should produce output");
    println!("--- NestExample + Inner decompilation ---\n{}", result);
}

#[test]
fn test_decompile_single_method() {
    let class = load_class("HelloWorld.class");
    let decompiler = Decompiler::new(DecompileOptions::default());
    let result = decompiler
        .decompile_method(&class, "main")
        .expect("method decompilation should succeed");
    assert!(result.contains("main"), "Should contain the method");
    println!("--- HelloWorld.main() ---\n{}", result);
}

#[test]
fn test_decompile_all_test_classes() {
    // Ensure we can decompile every test class without panicking
    let classes = [
        "BasicClass.class",
        "HelloWorld.class",
        "Factorial.class",
        "Instructions.class",
        "Annotations.class",
        "DeprecatedAnnotation.class",
        "InnerClasses.class",
        "LocalVariableTable.class",
        "BootstrapMethods.class",
        "RecordExample.class",
        "SealedExample.class",
        "SealedChild1.class",
        "SealedChild2.class",
        "NestExample.class",
        "NestExample$Inner.class",
        "UnicodeStrings.class",
    ];

    for class_name in &classes {
        let class = load_class(class_name);
        match decompile::decompile(&class) {
            Ok(source) => {
                dbg!(&source);
                assert!(
                    !source.is_empty(),
                    "{} should produce non-empty output",
                    class_name
                );
            }
            Err(e) => {
                panic!("{} failed to decompile: {}", class_name, e);
            }
        }
    }
}
