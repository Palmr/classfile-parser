use crate::attribute_info::{
    AttributeInfo, AttributeInfoVariant, CodeAttribute, StackMapFrame, StackMapFrameInner,
    StackMapTableAttribute,
};
use crate::code_attribute::Instruction;
use crate::constant_info::ConstantInfo;
use crate::decompile::descriptor::parse_method_descriptor;
use crate::method_info::MethodAccessFlags;
use crate::ClassFile;

use super::codegen::{compute_byte_addresses, CodeGenerator};
use super::lexer::Lexer;
use super::parser::Parser;
use super::{CompileError, CompileOptions, InsertMode};

/// Extract parameter names from debug info (MethodParameters or LocalVariableTable).
///
/// Returns a Vec with one entry per declared parameter. Each entry is `Some(name)` if
/// a debug name was found, or `None` otherwise.
fn extract_param_names(
    class_file: &ClassFile,
    method_idx: usize,
    is_static: bool,
    method_descriptor: &str,
) -> Vec<Option<String>> {
    let (params, _) = match parse_method_descriptor(method_descriptor) {
        Some(p) => p,
        None => return vec![],
    };
    let param_count = params.len();
    if param_count == 0 {
        return vec![];
    }

    let method = &class_file.methods[method_idx];

    // Try MethodParameters attribute first (method-level, available with javac -parameters)
    for attr in &method.attributes {
        if let Some(AttributeInfoVariant::MethodParameters(mp)) = &attr.info_parsed {
            let mut names = Vec::with_capacity(param_count);
            for (i, p) in mp.parameters.iter().enumerate() {
                if i >= param_count {
                    break;
                }
                if p.name_index != 0 {
                    if let Some(name) = class_file.get_utf8(p.name_index) {
                        names.push(Some(name.to_string()));
                        continue;
                    }
                }
                names.push(None);
            }
            // Pad if MethodParameters has fewer entries
            while names.len() < param_count {
                names.push(None);
            }
            return names;
        }
    }

    // Fall back to LocalVariableTable (Code sub-attribute, available with javac -g)
    for attr in &method.attributes {
        if let Some(AttributeInfoVariant::Code(code)) = &attr.info_parsed {
            for sub_attr in &code.attributes {
                if let Some(AttributeInfoVariant::LocalVariableTable(lvt)) = &sub_attr.info_parsed {
                    // Build a slot→name map for parameter slots
                    // Parameter slots: if instance method, slot 0 = this, params start at 1
                    let first_param_slot: u16 = if is_static { 0 } else { 1 };
                    let mut slot_to_name = std::collections::HashMap::new();
                    for item in &lvt.items {
                        // Parameters typically have start_pc == 0
                        if item.start_pc == 0 {
                            if let Some(name) = class_file.get_utf8(item.name_index) {
                                slot_to_name.insert(item.index, name.to_string());
                            }
                        }
                    }

                    // Walk through params computing expected slots
                    let mut names = Vec::with_capacity(param_count);
                    let mut slot = first_param_slot;
                    for param in &params {
                        names.push(slot_to_name.get(&slot).cloned());
                        slot += if param.is_wide() { 2 } else { 1 };
                    }
                    return names;
                }
            }
        }
    }

    vec![None; param_count]
}

/// Strip trailing return instructions from generated code for prepend mode.
fn strip_trailing_returns(instructions: &mut Vec<Instruction>) {
    while let Some(last) = instructions.last() {
        match last {
            Instruction::Return
            | Instruction::Ireturn
            | Instruction::Lreturn
            | Instruction::Freturn
            | Instruction::Dreturn
            | Instruction::Areturn => {
                instructions.pop();
            }
            _ => break,
        }
    }
}

/// Extract the offset_delta from a StackMapFrame.
fn frame_offset_delta(frame: &StackMapFrame) -> u16 {
    match &frame.inner {
        StackMapFrameInner::SameFrame {} => frame.frame_type as u16,
        StackMapFrameInner::SameLocals1StackItemFrame { .. } => (frame.frame_type - 64) as u16,
        StackMapFrameInner::SameLocals1StackItemFrameExtended { offset_delta, .. }
        | StackMapFrameInner::ChopFrame { offset_delta, .. }
        | StackMapFrameInner::SameFrameExtended { offset_delta, .. }
        | StackMapFrameInner::AppendFrame { offset_delta, .. }
        | StackMapFrameInner::FullFrame { offset_delta, .. } => *offset_delta,
    }
}

/// Convert StackMapTable frames from delta-encoded to absolute bytecode offsets.
fn frames_to_absolute(smt: &StackMapTableAttribute) -> Vec<(u32, StackMapFrame)> {
    let mut result = Vec::new();
    let mut prev_offset: i64 = -1;
    for frame in &smt.entries {
        let delta = frame_offset_delta(frame) as i64;
        let abs_offset = (prev_offset + delta + 1) as u32;
        prev_offset = abs_offset as i64;
        result.push((abs_offset, frame.clone()));
    }
    result
}

/// Re-encode a frame with a new offset_delta, choosing the most compact representation.
fn reencode_frame_with_delta(frame: &StackMapFrame, new_delta: u16) -> StackMapFrame {
    match &frame.inner {
        StackMapFrameInner::SameFrame {} | StackMapFrameInner::SameFrameExtended { .. } => {
            if new_delta <= 63 {
                StackMapFrame {
                    frame_type: new_delta as u8,
                    inner: StackMapFrameInner::SameFrame {},
                }
            } else {
                StackMapFrame {
                    frame_type: 251,
                    inner: StackMapFrameInner::SameFrameExtended {
                        offset_delta: new_delta,
                    },
                }
            }
        }
        StackMapFrameInner::SameLocals1StackItemFrame { stack }
        | StackMapFrameInner::SameLocals1StackItemFrameExtended { stack, .. } => {
            if new_delta <= 63 {
                StackMapFrame {
                    frame_type: 64 + new_delta as u8,
                    inner: StackMapFrameInner::SameLocals1StackItemFrame {
                        stack: stack.clone(),
                    },
                }
            } else {
                StackMapFrame {
                    frame_type: 247,
                    inner: StackMapFrameInner::SameLocals1StackItemFrameExtended {
                        offset_delta: new_delta,
                        stack: stack.clone(),
                    },
                }
            }
        }
        StackMapFrameInner::ChopFrame { .. } => StackMapFrame {
            frame_type: frame.frame_type,
            inner: StackMapFrameInner::ChopFrame {
                offset_delta: new_delta,
            },
        },
        StackMapFrameInner::AppendFrame { locals, .. } => StackMapFrame {
            frame_type: frame.frame_type,
            inner: StackMapFrameInner::AppendFrame {
                offset_delta: new_delta,
                locals: locals.clone(),
            },
        },
        StackMapFrameInner::FullFrame {
            number_of_locals,
            locals,
            number_of_stack_items,
            stack,
            ..
        } => StackMapFrame {
            frame_type: 255,
            inner: StackMapFrameInner::FullFrame {
                offset_delta: new_delta,
                number_of_locals: *number_of_locals,
                locals: locals.clone(),
                number_of_stack_items: *number_of_stack_items,
                stack: stack.clone(),
            },
        },
    }
}

/// Take frames with absolute offsets, re-compute deltas, and re-encode.
fn reencode_frames_absolute(frames: &[(u32, StackMapFrame)]) -> Vec<StackMapFrame> {
    let mut result = Vec::new();
    let mut prev_offset: i64 = -1;
    for (abs_offset, frame) in frames {
        let new_delta = (*abs_offset as i64 - prev_offset - 1) as u16;
        prev_offset = *abs_offset as i64;
        result.push(reencode_frame_with_delta(frame, new_delta));
    }
    result
}

/// Compile Java source and replace a method's body in the class file.
///
/// When `method_descriptor` is `Some`, the method is matched by both name and
/// descriptor, disambiguating overloaded methods. When `None`, the first method
/// with the given name is used.
pub fn compile_method_body_impl(
    source: &str,
    class_file: &mut ClassFile,
    method_name: &str,
    method_descriptor: Option<&str>,
    options: &CompileOptions,
) -> Result<(), CompileError> {
    // Find the method by name (and optionally descriptor)
    let method_idx = class_file
        .methods
        .iter()
        .position(|m| {
            let name_matches = matches!(
                &class_file.const_pool[(m.name_index - 1) as usize],
                ConstantInfo::Utf8(u) if u.utf8_string == method_name
            );
            if !name_matches {
                return false;
            }
            // If a descriptor is provided, also check it matches
            if let Some(desc) = method_descriptor {
                matches!(
                    &class_file.const_pool[(m.descriptor_index - 1) as usize],
                    ConstantInfo::Utf8(u) if u.utf8_string == desc
                )
            } else {
                true
            }
        })
        .ok_or_else(|| CompileError::MethodNotFound {
            name: if let Some(desc) = method_descriptor {
                format!("{method_name}{desc}")
            } else {
                method_name.to_string()
            },
        })?;

    // Get method info
    let is_static = class_file.methods[method_idx]
        .access_flags
        .contains(MethodAccessFlags::STATIC);
    let descriptor_index = class_file.methods[method_idx].descriptor_index;
    let method_descriptor = class_file
        .get_utf8(descriptor_index)
        .ok_or_else(|| CompileError::CodegenError {
            message: "could not resolve method descriptor".into(),
        })?
        .to_string();

    // Extract parameter names from debug info before mutably borrowing for codegen
    let param_names = extract_param_names(class_file, method_idx, is_static, &method_descriptor);

    // Parse source
    let lexer = Lexer::new(source);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    let stmts = parser.parse_method_body()?;

    // Generate bytecode
    let mut codegen = CodeGenerator::new_with_options(
        class_file,
        is_static,
        &method_descriptor,
        options.generate_stack_map_table,
        &param_names,
    )?;
    codegen.generate_body(&stmts)?;
    let generated = codegen.finish()?;

    match options.insert_mode {
        InsertMode::Replace => replace_method_body(class_file, method_idx, generated, options)?,
        InsertMode::Prepend => prepend_to_method_body(class_file, method_idx, generated, options)?,
        InsertMode::Append => append_to_method_body(class_file, method_idx, generated, options)?,
    }

    class_file.sync_counts();
    Ok(())
}

/// Replace the entire method body with newly generated bytecode.
fn replace_method_body(
    class_file: &mut ClassFile,
    method_idx: usize,
    generated: super::GeneratedCode,
    options: &CompileOptions,
) -> Result<(), CompileError> {
    // Build StackMapTable sub-attribute if generated
    let smt_sub_attr = if options.generate_stack_map_table {
        if let Some(smt) = generated.stack_map_table {
            let smt_name_idx = class_file.get_or_add_utf8("StackMapTable");
            let mut smt_attr = AttributeInfo {
                attribute_name_index: smt_name_idx,
                attribute_length: 0,
                info: vec![],
                info_parsed: Some(AttributeInfoVariant::StackMapTable(smt)),
            };
            smt_attr.sync_from_parsed().map_err(|e| CompileError::CodegenError {
                message: format!("sync_from_parsed for StackMapTable failed: {}", e),
            })?;
            Some(smt_attr)
        } else {
            None
        }
    } else {
        None
    };

    // Find or create Code attribute
    let code_attr_idx = class_file.methods[method_idx]
        .attributes
        .iter()
        .position(|a| matches!(a.info_parsed, Some(AttributeInfoVariant::Code(_))));

    if let Some(attr_idx) = code_attr_idx {
        let code = match &mut class_file.methods[method_idx].attributes[attr_idx].info_parsed {
            Some(AttributeInfoVariant::Code(c)) => c,
            _ => unreachable!(),
        };

        // Replace instructions and update stack/locals
        code.code = generated.instructions;
        code.max_stack = generated.max_stack;
        code.max_locals = generated.max_locals;
        code.exception_table = generated.exception_table;
        code.exception_table_length = code.exception_table.len() as u16;

        // Strip debug and verification sub-attributes that reference old bytecode offsets
        code.attributes.retain(|a| {
            !matches!(
                a.info_parsed,
                Some(AttributeInfoVariant::LineNumberTable(_))
                    | Some(AttributeInfoVariant::LocalVariableTable(_))
                    | Some(AttributeInfoVariant::LocalVariableTypeTable(_))
            )
        });
        // Always strip old StackMapTable
        code.attributes.retain(|a| {
            !matches!(
                a.info_parsed,
                Some(AttributeInfoVariant::StackMapTable(_))
            )
        });
        // Attach new StackMapTable if generated
        if let Some(smt_attr) = smt_sub_attr {
            code.attributes.push(smt_attr);
        }
        code.attributes_count = code.attributes.len() as u16;

        // Sync
        class_file.methods[method_idx].attributes[attr_idx]
            .sync_from_parsed()
            .map_err(|e| CompileError::CodegenError {
                message: format!("sync_from_parsed failed: {}", e),
            })?;
    } else {
        // Create a new Code attribute
        let code_name_idx = class_file.get_or_add_utf8("Code");
        let exception_table_length = generated.exception_table.len() as u16;

        let mut sub_attrs = Vec::new();
        if let Some(smt_attr) = smt_sub_attr {
            sub_attrs.push(smt_attr);
        }

        let code_attr = CodeAttribute {
            max_stack: generated.max_stack,
            max_locals: generated.max_locals,
            code_length: 0, // will be set by sync
            code: generated.instructions,
            exception_table_length,
            exception_table: generated.exception_table,
            attributes_count: sub_attrs.len() as u16,
            attributes: sub_attrs,
        };

        let mut attr_info = AttributeInfo {
            attribute_name_index: code_name_idx,
            attribute_length: 0,
            info: vec![],
            info_parsed: Some(AttributeInfoVariant::Code(code_attr)),
        };
        attr_info.sync_from_parsed().map_err(|e| CompileError::CodegenError {
            message: format!("sync_from_parsed failed: {}", e),
        })?;

        class_file.methods[method_idx].attributes.push(attr_info);
        class_file.methods[method_idx].attributes_count =
            class_file.methods[method_idx].attributes.len() as u16;
    }

    Ok(())
}

/// Prepend newly generated bytecode before the existing method body.
fn prepend_to_method_body(
    class_file: &mut ClassFile,
    method_idx: usize,
    mut generated: super::GeneratedCode,
    options: &CompileOptions,
) -> Result<(), CompileError> {
    // Strip trailing return so prepended code falls through to original
    strip_trailing_returns(&mut generated.instructions);
    if generated.instructions.is_empty() {
        return Ok(()); // Nothing to prepend
    }

    // Find existing Code attribute (required for prepend)
    let attr_idx = class_file.methods[method_idx]
        .attributes
        .iter()
        .position(|a| matches!(a.info_parsed, Some(AttributeInfoVariant::Code(_))))
        .ok_or_else(|| CompileError::CodegenError {
            message: "method has no Code attribute to prepend to".into(),
        })?;

    // Pre-resolve StackMapTable name index before taking mutable borrow on code
    let smt_name_idx = if options.generate_stack_map_table {
        Some(class_file.get_or_add_utf8("StackMapTable"))
    } else {
        None
    };

    let code = match &mut class_file.methods[method_idx].attributes[attr_idx].info_parsed {
        Some(AttributeInfoVariant::Code(c)) => c,
        _ => unreachable!(),
    };

    // Concatenate: new instructions ++ old instructions
    let old_instructions = std::mem::take(&mut code.code);
    let new_count = generated.instructions.len();
    let mut combined = generated.instructions;
    combined.extend(old_instructions);

    // Compute byte addresses for the combined stream
    let addresses = compute_byte_addresses(&combined);
    let prepend_byte_size = if new_count < addresses.len() {
        addresses[new_count]
    } else {
        // All instructions are new (shouldn't happen, but be safe)
        *addresses.last().unwrap_or(&0)
    };

    // Shift existing exception table entries by prepend_byte_size
    for entry in &mut code.exception_table {
        entry.start_pc += prepend_byte_size as u16;
        entry.end_pc += prepend_byte_size as u16;
        entry.handler_pc += prepend_byte_size as u16;
    }

    // Merge exception tables: new first, then shifted old
    let mut merged_exceptions = generated.exception_table;
    merged_exceptions.append(&mut code.exception_table);

    // Handle StackMapTable merging
    let old_smt = code.attributes.iter().find_map(|a| match &a.info_parsed {
        Some(AttributeInfoVariant::StackMapTable(smt)) => Some(smt.clone()),
        _ => None,
    });

    // Strip old StackMapTable and debug attributes
    code.attributes.retain(|a| {
        !matches!(
            a.info_parsed,
            Some(AttributeInfoVariant::StackMapTable(_))
                | Some(AttributeInfoVariant::LineNumberTable(_))
                | Some(AttributeInfoVariant::LocalVariableTable(_))
                | Some(AttributeInfoVariant::LocalVariableTypeTable(_))
        )
    });

    // Build merged StackMapTable
    if options.generate_stack_map_table {
        let mut all_frames: Vec<(u32, StackMapFrame)> = Vec::new();

        // Add new code's frames (already at correct absolute offsets)
        if let Some(new_smt) = &generated.stack_map_table {
            all_frames.extend(frames_to_absolute(new_smt));
        }

        // Add shifted old frames
        if let Some(old_smt) = &old_smt {
            for (offset, frame) in frames_to_absolute(old_smt) {
                all_frames.push((offset + prepend_byte_size, frame));
            }
        }

        if !all_frames.is_empty() {
            all_frames.sort_by_key(|(offset, _)| *offset);
            let reencoded = reencode_frames_absolute(&all_frames);

            let smt = StackMapTableAttribute {
                number_of_entries: reencoded.len() as u16,
                entries: reencoded,
            };

            let mut smt_attr = AttributeInfo {
                attribute_name_index: smt_name_idx.unwrap(),
                attribute_length: 0,
                info: vec![],
                info_parsed: Some(AttributeInfoVariant::StackMapTable(smt)),
            };
            smt_attr
                .sync_from_parsed()
                .map_err(|e| CompileError::CodegenError {
                    message: format!("sync_from_parsed for StackMapTable failed: {}", e),
                })?;
            code.attributes.push(smt_attr);
        }
    }

    // Update CodeAttribute
    code.code = combined;
    code.max_stack = std::cmp::max(generated.max_stack, code.max_stack);
    code.max_locals = std::cmp::max(generated.max_locals, code.max_locals);
    code.exception_table = merged_exceptions;
    code.exception_table_length = code.exception_table.len() as u16;
    code.attributes_count = code.attributes.len() as u16;

    // Sync
    class_file.methods[method_idx].attributes[attr_idx]
        .sync_from_parsed()
        .map_err(|e| CompileError::CodegenError {
            message: format!("sync_from_parsed failed: {}", e),
        })?;

    Ok(())
}

/// Append newly generated bytecode after the existing method body.
///
/// The trailing return instruction(s) of the original method are stripped
/// so execution falls through to the appended code. The appended code
/// must contain its own return instruction.
fn append_to_method_body(
    class_file: &mut ClassFile,
    method_idx: usize,
    generated: super::GeneratedCode,
    options: &CompileOptions,
) -> Result<(), CompileError> {
    if generated.instructions.is_empty() {
        return Ok(()); // Nothing to append
    }

    // Find existing Code attribute (required for append)
    let attr_idx = class_file.methods[method_idx]
        .attributes
        .iter()
        .position(|a| matches!(a.info_parsed, Some(AttributeInfoVariant::Code(_))))
        .ok_or_else(|| CompileError::CodegenError {
            message: "method has no Code attribute to append to".into(),
        })?;

    // Pre-resolve StackMapTable name index before taking mutable borrow on code
    let smt_name_idx = if options.generate_stack_map_table {
        Some(class_file.get_or_add_utf8("StackMapTable"))
    } else {
        None
    };

    let code = match &mut class_file.methods[method_idx].attributes[attr_idx].info_parsed {
        Some(AttributeInfoVariant::Code(c)) => c,
        _ => unreachable!(),
    };

    // Strip trailing returns from the OLD instructions so they fall through
    // to the appended code. Non-trailing returns (e.g. early returns in branches)
    // are left intact — those paths will skip the appended code.
    strip_trailing_returns(&mut code.code);

    // Concatenate: old instructions ++ new instructions
    let old_count = code.code.len();
    let mut combined = std::mem::take(&mut code.code);
    combined.extend(generated.instructions);

    // Compute byte addresses for the combined stream
    let addresses = compute_byte_addresses(&combined);
    let old_byte_size = if old_count < addresses.len() {
        addresses[old_count]
    } else {
        *addresses.last().unwrap_or(&0)
    };

    // Shift new exception table entries by old_byte_size
    let mut new_exceptions = generated.exception_table;
    for entry in &mut new_exceptions {
        entry.start_pc += old_byte_size as u16;
        entry.end_pc += old_byte_size as u16;
        entry.handler_pc += old_byte_size as u16;
    }

    // Merge exception tables: old first, then shifted new
    let mut merged_exceptions = std::mem::take(&mut code.exception_table);
    merged_exceptions.append(&mut new_exceptions);

    // Handle StackMapTable merging
    let old_smt = code.attributes.iter().find_map(|a| match &a.info_parsed {
        Some(AttributeInfoVariant::StackMapTable(smt)) => Some(smt.clone()),
        _ => None,
    });

    // Strip old StackMapTable and debug attributes
    code.attributes.retain(|a| {
        !matches!(
            a.info_parsed,
            Some(AttributeInfoVariant::StackMapTable(_))
                | Some(AttributeInfoVariant::LineNumberTable(_))
                | Some(AttributeInfoVariant::LocalVariableTable(_))
                | Some(AttributeInfoVariant::LocalVariableTypeTable(_))
        )
    });

    // Build merged StackMapTable
    if options.generate_stack_map_table {
        let mut all_frames: Vec<(u32, StackMapFrame)> = Vec::new();

        // Old frames stay at their original absolute offsets
        if let Some(old_smt) = &old_smt {
            all_frames.extend(frames_to_absolute(old_smt));
        }

        // New frames shifted by old_byte_size
        if let Some(new_smt) = &generated.stack_map_table {
            for (offset, frame) in frames_to_absolute(new_smt) {
                all_frames.push((offset + old_byte_size, frame));
            }
        }

        if !all_frames.is_empty() {
            all_frames.sort_by_key(|(offset, _)| *offset);
            let reencoded = reencode_frames_absolute(&all_frames);

            let smt = StackMapTableAttribute {
                number_of_entries: reencoded.len() as u16,
                entries: reencoded,
            };

            let mut smt_attr = AttributeInfo {
                attribute_name_index: smt_name_idx.unwrap(),
                attribute_length: 0,
                info: vec![],
                info_parsed: Some(AttributeInfoVariant::StackMapTable(smt)),
            };
            smt_attr
                .sync_from_parsed()
                .map_err(|e| CompileError::CodegenError {
                    message: format!("sync_from_parsed for StackMapTable failed: {}", e),
                })?;
            code.attributes.push(smt_attr);
        }
    }

    // Update CodeAttribute
    code.code = combined;
    code.max_stack = std::cmp::max(generated.max_stack, code.max_stack);
    code.max_locals = std::cmp::max(generated.max_locals, code.max_locals);
    code.exception_table = merged_exceptions;
    code.exception_table_length = code.exception_table.len() as u16;
    code.attributes_count = code.attributes.len() as u16;

    // Sync
    class_file.methods[method_idx].attributes[attr_idx]
        .sync_from_parsed()
        .map_err(|e| CompileError::CodegenError {
            message: format!("sync_from_parsed failed: {}", e),
        })?;

    Ok(())
}
