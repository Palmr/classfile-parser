# classfile-parser — binrw Refactoring

Fork of [Palmr/classfile-parser](https://github.com/Palmr/classfile-parser) being refactored from `nom` to `binrw` for parsing Java `.class` files.

## Specification

Java Class File Format: https://docs.oracle.com/javase/specs/jvms/se10/html/jvms-4.html

## Build & Test

```sh
cargo test              # run all tests (builds on stable Rust)
cargo test test_valid_class -- --nocapture  # run a specific test with output
```

## Project Structure

```
src/
├── lib.rs                      # Library entry point, re-exports
├── types.rs                    # ClassFile struct, custom BinRead impl, ClassAccessFlags, helper methods
├── attribute_info/
│   ├── mod.rs                  # Re-exports
│   └── types.rs                # AttributeInfo + all 30 attribute variant types (binrw)
├── code_attribute/
│   ├── mod.rs                  # Re-exports
│   └── types.rs                # Instruction enum (200+ opcodes), LocalVariable* types (binrw)
├── constant_info/
│   ├── mod.rs                  # Re-exports
│   └── types.rs                # ConstantInfo enum with all 19 constant types (binrw)
├── field_info/
│   ├── mod.rs                  # Re-exports
│   └── types.rs                # FieldInfo struct (binrw)
└── method_info/
    ├── mod.rs                  # Re-exports
    └── types.rs                # MethodInfo struct + code() helpers (binrw)

tests/
├── classfile.rs                # Main classfile parsing tests + round-trip (7 passing)
├── code_attribute.rs           # Instruction + attribute integration tests (22 passing)
├── attr_stack_map_table.rs     # Stack map table tests (1 passing)
├── attr_bootstrap_methods.rs   # Bootstrap method tests (2 passing)
├── e2e_patch.rs                # E2E patching tests: instructions, constants, flags, methods (20 passing)
├── module_attribute.rs         # Module attribute parsing + round-trip tests (7 passing)
├── new_attributes.rs           # NestHost/NestMembers, Record, PermittedSubclasses, ModulePackages/ModuleMainClass, sub-attribute interpretation tests (12 passing)
├── compiler/                   # Compiler test suite (152 passing)
│   ├── main.rs                 # Shared helpers (java_available, compile_and_load, write_and_run)
│   ├── parser.rs               # Parser tests — no Java needed (51 passing)
│   ├── e2e.rs                  # Codegen + E2E compile tests (47 passing)
│   ├── stress.rs               # Stress tests — algorithms, edge cases, feature combos (41 passing)
│   ├── param_access.rs         # Parameter access tests — positional, debug names, wide types (4 passing)
│   └── prepend.rs              # Prepend mode + StackMapTable edge case tests (9 passing)
├── helpers.rs                  # Helper utility tests (17 passing)
└── jar_patch.rs                # JAR patching E2E tests (20 passing, requires jar-utils feature)

java-assets/compiled-classes/   # .class files used by tests

examples/
├── jar_explorer.rs             # TUI JAR browser with interactive code editing (tui-example feature)
├── compile_patch.rs            # Standalone compile & patch demo (compile feature)
└── jar_patch.rs                # JAR patching demo (jar-patch feature)
```

## Architecture

### Two-stage attribute parsing

Attributes are parsed in two stages:
1. `AttributeInfo` reads raw bytes via binrw (`attribute_name_index`, `attribute_length`, `info: Vec<u8>`)
2. `interpret_inner()` is called post-parse with the constant pool to resolve the attribute name string and parse `info` bytes into the correct `AttributeInfoVariant`

`interpret_inner` is called recursively: top-level attributes (on ClassFile, FieldInfo, MethodInfo) are interpreted during `ClassFile::read_options()`, and sub-attributes inside `CodeAttribute` and `RecordComponentInfo` are interpreted automatically during their parent's `interpret_inner` call. All `info_parsed` fields are populated after parsing.

### Attribute reserialization

`AttributeInfo::sync_from_parsed()` serializes `info_parsed` back into `info` bytes and updates `attribute_length`. Returns `BinResult<()>`. Call this after modifying parsed attribute contents (e.g., changing instructions in a CodeAttribute). For Code attributes, this automatically calls `CodeAttribute::sync_lengths()` to recalculate `code_length`, `exception_table_length`, and `attributes_count`, and recursively calls `sync_from_parsed()` on Code sub-attributes. For Record attributes, sub-attributes on each component are also synced.

`ClassFile::sync_counts()` recalculates all top-level count fields (`const_pool_size`, `interfaces_count`, `fields_count`, `methods_count`, `attributes_count`) from actual vector lengths. Call this after adding or removing entries.

### Higher-level API

`ClassFile` provides convenience methods for navigating the class structure:
- `get_utf8(index)` — look up a UTF-8 constant by 1-based index
- `find_utf8_index(value)` — find the index of a UTF-8 constant by value
- `find_method(name)` / `find_method_mut(name)` — find a method by name
- `find_field(name)` / `find_field_mut(name)` — find a field by name

`MethodInfo` provides:
- `code()` / `code_mut()` — get the Code attribute contents
- `code_attribute_info()` / `code_attribute_info_mut()` — get the wrapping AttributeInfo (for calling `sync_from_parsed()`)

### InterpretInner trait

`ClassFile::read_options()` calls `interpret_inner(&const_pool)` on fields, methods, and attributes after initial parsing. This is needed because attribute parsing requires the constant pool to determine the attribute type by name.

### Instruction parsing

The `Instruction` enum uses binrw magic bytes for each opcode. Special cases:
- `tableswitch` / `lookupswitch` require alignment padding relative to the code start address (passed via `import { address: u32 }`)
- `wide` instructions use 2-byte magic (`b"\xc4\xXX"`)
- Instructions are parsed via a custom `parse_code_instructions` function (not `binrw::helpers::until_eof`) on a length-limited `TakeSeek` sub-stream

### Why not `until_eof` for instruction parsing

The `Instruction` enum uses `return_unexpected_error` for concise error messages. This produces `Error::NoVariantMatch` on failure, but `NoVariantMatch.is_eof()` returns `false`. The built-in `until_eof` helper relies on `is_eof()` to detect end-of-stream, so it propagates the error instead of stopping gracefully. The custom `parse_code_instructions` parser handles EOF via a manual 1-byte read check and also computes the correct per-instruction `address` for switch alignment.

### TargetInfo discriminant

`TargetInfo` uses `#[br(import(target_type: u8))]` with `pre_assert` on each variant to dispatch based on the `target_type` byte from the enclosing `TypeAnnotation`. This is needed because `TargetInfo` has no magic bytes — the discriminant comes from a sibling field.

## Compile Feature Architecture

The `compile` feature (`src/compile/`) is a mini Java-to-bytecode compiler for replacing method bodies. Pipeline: Lexer → Parser → AST → CodeGen.

### Files

- `lexer.rs` — Tokenizes Java source (keywords, operators, literals, identifiers)
- `parser.rs` — Recursive-descent parser producing AST nodes
- `ast.rs` — Statement and Expression enums (LocalDecl, If, While, For, Switch, TryCatch, MethodCall, FieldAccess, etc.)
- `codegen.rs` — Emits JVM instructions from AST; manages locals, labels, branch patching, exception tables
- `stackmap.rs` — Tracks verification types (VType) at branch targets; builds StackMapTableAttribute
- `stack_calc.rs` — Computes max stack depth by walking generated instructions
- `patch.rs` — Replaces a method's CodeAttribute in a ClassFile with newly compiled bytecode
- `mod.rs` — Public API: `compile_method_body()`, `CompileOptions`, `CompileError`, `patch_method!`, `patch_methods!`

### CodeGen internals

- **Labels**: Branch targets use abstract label IDs, resolved to byte offsets after all instructions are emitted
- **Locals**: `LocalAllocator` assigns JVM local slots, tracking types AND resolved VTypes for StackMapTable; parameters pre-allocated from method descriptor with VTypes resolved against the constant pool at allocation time via `type_name_to_vtype_resolved()`. Supports `save()`/`restore()` for scope-aware local lifetime tracking.
- **Scope management**: `LocalAllocator::save()`/`restore()` ensures locals declared in inner scopes (if-then/else branches, loop bodies, catch handlers, for-each temps, synchronized handlers) don't leak into StackMapTable frames for subsequent code. `LocalDecl` allocates slots AFTER generating the initializer expression so branch targets inside initializers don't include the unassigned local.
- **Switch**: Heuristic selects `tableswitch` vs `lookupswitch` based on density (ratio of cases to range)
- **Try-catch-finally**: Finally blocks are inlined at each exit path; exception table entries built during codegen; `label_locals_override` preserves try-start locals at merge points. Multi-catch uses `java/lang/Throwable` as the stack map type since the LCA cannot be computed without class hierarchy knowledge.
- **StackMapTable**: `CompileOptions::default()` generates StackMapTable frames (all tests run with full JVM verification). `label_locals_override` is used across all control flow structures (if/else, while, for, for-each, switch, switch-expr, try-catch, synchronized) to capture pre-branch locals for merge-point frames. `label_stack_override` handles expression-level merge points (comparisons, ternaries, logical ops, switch expressions) where values are on the stack. `FrameTracker::record_frame()` keeps the last frame when multiple labels share the same bytecode offset.

### What the compiler supports today

**Statements**: local declarations (including `var`), expression statements, return (typed), if/else, while, for (traditional), for-each, switch (tableswitch + lookupswitch), try-catch-finally (multiple catches, multi-catch), throw, break, continue, blocks, synchronized

**Expressions**: int/long/float/double/boolean/char/string/null literals, identifiers, `this`, all binary arithmetic (+, -, *, /, %), bitwise (&, |, ^, ~, <<, >>, >>>), comparisons (==, !=, <, <=, >, >=), logical (&&, || with short-circuit), ternary, switch expressions (arrow syntax), assignment (simple + compound), pre/post increment/decrement, method calls (with invokeinterface detection, generic type params), field access (including `array.length`), array access/creation (including multi-dimensional), object instantiation, casts, instanceof, lambda expressions, method references

**Parameter access**: Method parameters are accessible by positional name (`arg0`, `arg1`, ...) always. When debug info is present (`javac -g` for `LocalVariableTable`, or `javac -parameters` for `MethodParameters`), original parameter names (e.g., `args`, `name`) are also available as aliases. Wide types (long/double) correctly consume 2 slots. Instance methods have `this` at slot 0; `arg0` is the first declared parameter.

**Patching**: `compile_method_body()`, `patch_method!`, `patch_methods!`, `prepend_method_body()`, `prepend_method!`, `patch_jar_method!`, `patch_jar_class!`, `patch_jar!`

**Prepend mode**: `prepend_method_body()` / `prepend_method!` inserts compiled code at the beginning of an existing method body, preserving the original instructions. Handles exception table offset adjustment, StackMapTable frame merging (delta re-encoding), and debug attribute stripping. Trailing returns are auto-stripped so prepended code falls through to the original body. Controlled by `InsertMode::Prepend` in `CompileOptions`.

## JAR Explorer (`examples/jar_explorer.rs`)

TUI application for browsing and editing Java `.jar` files. Run with:
```sh
cargo run --example jar_explorer --features tui-example -- path/to/file.jar
```

**Browsing**: Tree-based file navigation with expand/collapse. Views `.class` files (decompiled Java or bytecode listing), manifests, text files, nested JARs, hex dumps. Spring Boot format detection. Vim-like navigation (hjkl, gg/G, /search, n/N).

**Editing**: Press `e` on a loaded `.class` file to enter edit mode. Select a method from the list, type Java source in the editor (`{ ... }` block), then `Ctrl+S` to compile & replace or `Ctrl+P` to compile & prepend. Errors are shown inline; fix and retry. Press `W` to save the modified JAR as `<name>.patched.jar`.

**Key bindings**: Tree: `hjkl` navigate, `Enter`/`l` open, `e` edit, `W` save, `Tab` switch to viewer. Viewer: vim movement, `/` search, `Tab` back to tree. Edit: `j/k` select method, `Enter` open editor, `Ctrl+S` replace, `Ctrl+P` prepend, `Esc` cancel.

## Compiler Roadmap

Prioritized list of missing features. Items marked [done] have been implemented.

### P0 — High impact (blocks common patching patterns)

1. [done] **String concatenation with `+`** — `StringBuilder` codegen: `new StringBuilder().append(a).append(b).toString()`. Flattens chained `+` into a single StringBuilder. Type-aware append descriptors via `infer_expr_type`.

2. [done] **Long/float/double arithmetic** — Type-dispatched binary ops (`ladd`/`fadd`/`dadd` etc.), widening conversions (`i2l`, `i2f`, `i2d`, `l2f`, `l2d`, `f2d`), typed comparisons (`lcmp`, `fcmpl`/`fcmpg`, `dcmpl`/`dcmpg`), typed casts, typed unary ops, typed compound assign and increment/decrement.

3. [done] **For-each loops** — `for (Type x : iterable)` with both array mode (arraylength + index counter + typed array load) and Iterable mode (invokeinterface iterator/hasNext/next + checkcast).

4. [done] **Type-aware array load/store** — Correct instruction per element type: `iaload`/`iastore`, `laload`/`lastore`, `faload`/`fastore`, `daload`/`dastore`, `baload`/`bastore`, `caload`/`castore`, `saload`/`sastore`, `aaload`/`aastore`. Fixed array-store stack ordering in assignments.

**Foundation**: `infer_expr_type` — expression type inference used by all P0 features for type-dispatched codegen, println/append descriptor selection, and widening decisions.

### P1 — Medium impact (limits what you can patch)

5. [done] **Multi-catch** — `catch (IOException | SQLException e)` with `|`-separated types in catch clause. Parser collects multiple types; codegen emits separate exception table entries per type, all pointing to the same handler.

6. [done] **Field assignment on complex receivers** — Category-2 values (long/double) on field stores now use temp-local strategy instead of `Swap` (which only works for category-1). Added `descriptor_to_type` helper.

7. [done] **Method resolution improvement** — `infer_receiver_class` now resolves method call return types from the constant pool. Instance calls check for `InterfaceMethodRef` in pool and known JDK interfaces to choose `invokeinterface` vs `invokevirtual`. Static field detection also checks `FieldRef` entries in pool before the uppercase heuristic fallback.

8. [done] **Synchronized blocks** — `synchronized (expr) { ... }` with `monitorenter`/`monitorexit` codegen. Implicit catch-all handler ensures `monitorexit` on exceptional exits (same pattern as try-finally).

### P2 — Lower priority (nice to have)

9. [done] **Lambda expressions / method references** — `invokedynamic` + `LambdaMetafactory` bootstrap method generation. Compiles lambda body into synthetic private static method, sets up bootstrap methods attribute, emits `invokedynamic`. Supports typed and inferred params, expression and block bodies. Method references via `Class::method` syntax.

10. [done] **`var` keyword** (Java 10+) — Parser emits sentinel type `TypeName::Class("__var__")`, codegen resolves via `infer_expr_type`. Requires initializer.

11. [done] **Switch expressions** (Java 14+) — Arrow syntax `case 1 -> expr;` with `SwitchExpr` AST variant. Reuses tableswitch/lookupswitch infrastructure. Each case pushes value and jumps to end. Requires default case.

12. [done] **Multi-dimensional array creation** — `new int[3][4]` using `Multianewarray` instruction. `NewMultiArray` AST variant with dimension list. Parser detects consecutive `[expr]` after first dimension.

13. [done] **Generic type parameters in method calls** — `obj.<String>method()` parse-and-discard. `skip_type_parameters()` handles nested `<>` including `>>` closing. Works in both pre-name and post-name positions in dotted postfix.

### P3 — Code insertion

14. [done] **Prepend mode** — `prepend_method_body()` / `prepend_method!` inserts compiled code before an existing method body. Handles exception table offset adjustment, StackMapTable frame merging (absolute offset conversion + delta re-encoding), debug attribute stripping. Trailing returns auto-stripped for fall-through. Append mode (insert after) deferred — requires modifying existing return instructions.

## Current State

### What's done
- All type structs: ClassFile, ConstantInfo (19 types including Module/Package), FieldInfo, MethodInfo, AttributeInfo, Instruction
- All 30 attribute variant types (including Module, ModulePackages, ModuleMainClass, NestHost, NestMembers, Record, PermittedSubclasses)
- Custom BinRead for ClassFile (handles const pool Double/Long sentinel entries)
- Full BinWrite support for all types (read-write round-trip verified)
- `sync_from_parsed()` / `sync_counts()` / `sync_lengths()` for patching and rewriting class files
- Recursive `interpret_inner` propagation to Code and Record sub-attributes
- Higher-level API helpers on ClassFile and MethodInfo
- Legacy nom parser files fully removed; builds on stable Rust
- Compile feature: lexer, parser, AST, codegen, StackMapTable generation, method patching macros
- Compile P0 complete: string concat, typed arithmetic (long/float/double), for-each loops, type-aware arrays, expression type inference
- Compile P1 complete: multi-catch, synchronized blocks, field assignment fix for cat-2 values, method resolution (invokeinterface, pool-based receiver inference, pool-based static detection, descriptor inference fallback for unknown methods)
- Compile P2 complete: `var` keyword (type inference), switch expressions (arrow syntax), multi-dimensional arrays (multianewarray), generic type params (parse-and-discard), lambda expressions (invokedynamic + synthetic methods + bootstrap methods), method references
- Constant pool helpers: `get_or_add_method_handle`, `get_or_add_method_type`, `get_or_add_invoke_dynamic`
- JAR patching: `patch_jar_method!`, `patch_jar_class!`, `patch_jar!` macros with E2E tests
- StackMapTable: VType resolution for reference-type locals (including parameters) uses constant pool indices; for-each loop variables allocated after loop-top label for correct frame generation; category-2 types (Long/Double) correctly omit implicit Top continuation slots in frame encoding
- Method descriptor inference: `find_method_descriptor_in_pool()` falls back to `infer_method_descriptor()` for methods not in the pool, with well-known collection method signatures and heuristic return types
- `infer_expr_type()` for MethodCall/StaticMethodCall now resolves return types from method descriptors when available
- Method references resolve descriptors from constant pool, with well-known fallbacks; functional interface and SAM descriptor derived from resolved types
- Robust attribute parsing: `interpret_inner` uses bounds-checked const pool access and graceful error handling (malformed attributes fall back to raw bytes instead of panicking)
- `sync_counts()` uses checked u16 conversion to prevent silent overflow
- 290+ tests passing across all test files
- Compiler tests split into submodules: `cargo test --test compiler parser::` / `e2e::` / `stress::` / `param_access::` / `prepend::`
- JAR Explorer TUI (`examples/jar_explorer.rs`): interactive browsing + code editing with compile & prepend support, save modified JARs
