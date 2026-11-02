//! Rust-owned attachment fields written by the 0x18AF40 -> 0xDFFA0 path.
//!
//! Original offsets stay raw and unnamed until their consumer contract is established. The row
//! key is a Rust identity supplied by the eventual actor/effect binding, never a guest address.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttachmentRowId(pub u32);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EmitterAttachmentState {
    /// Original emitter flags at byte offset +0x04.
    pub flags: u32,
    /// Original emitter +0xAC, retained bitwise because the float's meaning is unknown.
    pub field_ac_bits: u32,
    /// Original emitter +0x68, raw because its semantic role is not established.
    pub field_68_bits: u32,
    /// Original emitter +0x6C, raw because its semantic role is not established.
    pub field_6c_bits: u32,
    /// Original emitter +0x70, raw because its semantic role is not established.
    pub field_70_bits: u32,
    /// Original emitter +0x88, represented as a Rust-owned row identity.
    pub attached_row: Option<AttachmentRowId>,
}

/// Original 0x10E970: no current emitter is a no-op; otherwise write +0xAC and set flag 0x20000.
pub fn set_field_ac_10e970(current: Option<&mut EmitterAttachmentState>, value: f32) {
    if let Some(emitter) = current {
        emitter.field_ac_bits = value.to_bits();
        emitter.flags |= 0x0002_0000;
    }
}

/// Original 0x10E600: no current emitter is a no-op; otherwise write +0x88 and set flag 0x100000.
/// `row` must already have been resolved to native identity by the caller.
pub fn attach_row_10e600(current: Option<&mut EmitterAttachmentState>, row: AttachmentRowId) {
    if let Some(emitter) = current {
        emitter.attached_row = Some(row);
        emitter.flags |= 0x0010_0000;
    }
}

/// Original 0x10EAE0 writes its two distinct stack floats to emitter +0x68 and +0x6C.
pub fn set_pair_10eae0(current: Option<&mut EmitterAttachmentState>, first: f32, second: f32) {
    if let Some(emitter) = current {
        emitter.field_68_bits = first.to_bits();
        emitter.field_6c_bits = second.to_bits();
    }
}

/// Original 0x10EB30 writes its float argument to emitter +0x70.
pub fn set_field_70_10eb30(current: Option<&mut EmitterAttachmentState>, value: f32) {
    if let Some(emitter) = current {
        emitter.field_70_bits = value.to_bits();
    }
}

/// Apply only DFFA0's measured post-construction emitter writes.
/// Allocation, RNG, target-row pointer validation, and singleton ownership stay external.
pub fn configure_dffa0_emitter(
    current: Option<&mut EmitterAttachmentState>,
    randomized_first: f32,
    row: Option<AttachmentRowId>,
) {
    let Some(emitter) = current else { return };
    set_field_ac_10e970(Some(emitter), 0.5);
    set_field_70_10eb30(Some(emitter), 2.0);
    set_pair_10eae0(Some(emitter), randomized_first, 1.0);
    if let Some(row) = row {
        attach_row_10e600(Some(emitter), row);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AttachmentRowId, EmitterAttachmentState, attach_row_10e600, configure_dffa0_emitter,
        set_field_ac_10e970,
    };

    #[test]
    fn setters_update_only_the_measured_slots_and_flags() {
        let mut emitter = EmitterAttachmentState {
            flags: 0x8000_0001,
            field_ac_bits: 0x1234_5678,
            attached_row: None,
            ..EmitterAttachmentState::default()
        };
        set_field_ac_10e970(Some(&mut emitter), 0.5);
        assert_eq!(emitter.field_ac_bits, 0x3f00_0000);
        assert_eq!(emitter.flags, 0x8002_0001);
        assert_eq!(emitter.attached_row, None);

        attach_row_10e600(Some(&mut emitter), AttachmentRowId(42));
        assert_eq!(emitter.attached_row, Some(AttachmentRowId(42)));
        assert_eq!(emitter.flags, 0x8012_0001);
    }

    #[test]
    fn absent_current_emitter_is_a_no_op() {
        set_field_ac_10e970(None, f32::NAN);
        attach_row_10e600(None, AttachmentRowId(7));
    }

    #[test]
    fn dffa0_post_create_sequence_preserves_raw_values_and_optional_attachment() {
        let mut emitter = EmitterAttachmentState::default();
        configure_dffa0_emitter(Some(&mut emitter), 0.375, Some(AttachmentRowId(9)));
        assert_eq!(emitter.field_ac_bits, 0.5f32.to_bits());
        assert_eq!(emitter.field_70_bits, 2.0f32.to_bits());
        assert_eq!(emitter.field_68_bits, 0.375f32.to_bits());
        assert_eq!(emitter.field_6c_bits, 1.0f32.to_bits());
        assert_eq!(emitter.attached_row, Some(AttachmentRowId(9)));
        assert_eq!(emitter.flags, 0x0012_0000);

        let mut no_row = EmitterAttachmentState::default();
        configure_dffa0_emitter(Some(&mut no_row), 0.25, None);
        assert_eq!(no_row.attached_row, None);
        assert_eq!(no_row.flags, 0x0002_0000);
    }
}
