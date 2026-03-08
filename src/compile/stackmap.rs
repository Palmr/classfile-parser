use crate::attribute_info::{
    StackMapFrame, StackMapFrameInner, StackMapTableAttribute, VerificationTypeInfo,
};

/// Verification type used during codegen-assisted frame tracking.
#[derive(Clone, Debug, PartialEq)]
pub enum VType {
    Top,
    Integer,
    Float,
    Long,
    Double,
    Null,
    UninitializedThis,
    Object(u16), // constant pool index for the class
}

impl VType {
    fn to_verification_type_info(&self) -> VerificationTypeInfo {
        match self {
            VType::Top => VerificationTypeInfo::Top,
            VType::Integer => VerificationTypeInfo::Integer,
            VType::Float => VerificationTypeInfo::Float,
            VType::Long => VerificationTypeInfo::Long,
            VType::Double => VerificationTypeInfo::Double,
            VType::Null => VerificationTypeInfo::Null,
            VType::UninitializedThis => VerificationTypeInfo::UninitializedThis,
            VType::Object(idx) => VerificationTypeInfo::Object { class: *idx },
        }
    }
}

/// A snapshot of the frame state at a particular bytecode offset.
#[derive(Clone, Debug)]
pub struct FrameSnapshot {
    pub bytecode_offset: u32,
    pub locals: Vec<VType>,
    pub stack: Vec<VType>,
}

/// Tracks type state during code generation for StackMapTable building.
pub struct FrameTracker {
    /// Initial locals (from method parameters).
    initial_locals: Vec<VType>,
    /// Recorded frame snapshots at branch targets / exception handlers.
    snapshots: Vec<FrameSnapshot>,
}

impl FrameTracker {
    pub fn new(initial_locals: Vec<VType>) -> Self {
        FrameTracker {
            initial_locals,
            snapshots: Vec::new(),
        }
    }

    /// Record a frame snapshot at the given bytecode offset.
    pub fn record_frame(&mut self, offset: u32, locals: Vec<VType>, stack: Vec<VType>) {
        // If a frame already exists at this offset, replace it — the last binding
        // at a given offset represents the most accurate state for subsequent code.
        if let Some(existing) = self
            .snapshots
            .iter_mut()
            .find(|s| s.bytecode_offset == offset)
        {
            existing.locals = locals;
            existing.stack = stack;
            return;
        }
        self.snapshots.push(FrameSnapshot {
            bytecode_offset: offset,
            locals,
            stack,
        });
    }

    /// Build the StackMapTableAttribute from recorded snapshots.
    pub fn build(mut self) -> Option<StackMapTableAttribute> {
        if self.snapshots.is_empty() {
            return None;
        }

        self.snapshots.sort_by_key(|s| s.bytecode_offset);
        self.snapshots.dedup_by_key(|s| s.bytecode_offset);

        let mut entries = Vec::new();
        let mut prev_offset: i64 = -1;
        let mut prev_locals = self.initial_locals.clone();

        for snapshot in &self.snapshots {
            let offset_delta = (snapshot.bytecode_offset as i64 - prev_offset - 1) as u16;
            prev_offset = snapshot.bytecode_offset as i64;

            let frame = encode_frame(
                &prev_locals,
                &snapshot.locals,
                &snapshot.stack,
                offset_delta,
            );
            entries.push(frame);
            prev_locals = snapshot.locals.clone();
        }

        Some(StackMapTableAttribute {
            number_of_entries: entries.len() as u16,
            entries,
        })
    }
}

/// Choose the most compact frame encoding.
/// `prev_locals` is the locals from the previous frame (or initial implicit frame).
fn encode_frame(
    prev_locals: &[VType],
    locals: &[VType],
    stack: &[VType],
    offset_delta: u16,
) -> StackMapFrame {
    // SameFrame: same locals, empty stack
    if stack.is_empty() && locals_match(prev_locals, locals) {
        if offset_delta <= 63 {
            return StackMapFrame {
                frame_type: offset_delta as u8,
                inner: StackMapFrameInner::SameFrame {},
            };
        } else {
            return StackMapFrame {
                frame_type: 251,
                inner: StackMapFrameInner::SameFrameExtended { offset_delta },
            };
        }
    }

    // SameLocals1StackItem: same locals, exactly 1 stack item
    if stack.len() == 1 && locals_match(prev_locals, locals) {
        let stack_item = stack[0].to_verification_type_info();
        if offset_delta <= 63 {
            return StackMapFrame {
                frame_type: 64 + offset_delta as u8,
                inner: StackMapFrameInner::SameLocals1StackItemFrame { stack: stack_item },
            };
        } else {
            return StackMapFrame {
                frame_type: 247,
                inner: StackMapFrameInner::SameLocals1StackItemFrameExtended {
                    offset_delta,
                    stack: stack_item,
                },
            };
        }
    }

    // AppendFrame: 1-3 new locals added, empty stack, prefix matches previous
    if stack.is_empty() && locals.len() > prev_locals.len() {
        let extra = locals.len() - prev_locals.len();
        if (1..=3).contains(&extra)
            && locals.len() >= prev_locals.len()
            && locals[..prev_locals.len()]
                .iter()
                .zip(prev_locals.iter())
                .all(|(a, b)| a == b)
        {
            let new_locals: Vec<VerificationTypeInfo> = locals[prev_locals.len()..]
                .iter()
                .map(|v| v.to_verification_type_info())
                .collect();
            return StackMapFrame {
                frame_type: 251 + extra as u8,
                inner: StackMapFrameInner::AppendFrame {
                    offset_delta,
                    locals: new_locals,
                },
            };
        }
    }

    // ChopFrame: 1-3 locals removed, empty stack, prefix matches previous
    if stack.is_empty() && locals.len() < prev_locals.len() {
        let chopped = prev_locals.len() - locals.len();
        if (1..=3).contains(&chopped) && locals.iter().zip(prev_locals.iter()).all(|(a, b)| a == b)
        {
            return StackMapFrame {
                frame_type: (251 - chopped) as u8,
                inner: StackMapFrameInner::ChopFrame { offset_delta },
            };
        }
    }

    // FullFrame: complete specification
    let local_vtypes: Vec<VerificationTypeInfo> = locals
        .iter()
        .map(|v| v.to_verification_type_info())
        .collect();
    let stack_vtypes: Vec<VerificationTypeInfo> = stack
        .iter()
        .map(|v| v.to_verification_type_info())
        .collect();

    StackMapFrame {
        frame_type: 255,
        inner: StackMapFrameInner::FullFrame {
            offset_delta,
            number_of_locals: local_vtypes.len() as u16,
            locals: local_vtypes,
            number_of_stack_items: stack_vtypes.len() as u16,
            stack: stack_vtypes,
        },
    }
}

fn locals_match(reference: &[VType], current: &[VType]) -> bool {
    reference.len() == current.len() && reference.iter().zip(current.iter()).all(|(a, b)| a == b)
}
