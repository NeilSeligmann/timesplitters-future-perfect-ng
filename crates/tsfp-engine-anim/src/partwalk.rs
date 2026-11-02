//! Last Tier 0 translations of the engine/anim list (T1668 close): `0x60880`, `0x257DC0`, `0x8FE30`, `0x1C8520`. **Tier 0**: translated from the
//! disassembly, compile, unit smoke tests only, no differential against the original (verification debt in `docs/t1668-anim-tiers.md`).
use crate::keyrec::clamp_frame;
use crate::pose::{Mat4, Sample, quat_matrix};

/// `0x60880(set, index, frame, out, element)`: byte offset (from the key set record `S`) of one element record of the clamped `frame`.
/// Streamed sets: `element` must be `1..=count` (`S+0x18`), the record is `stream_base + element * 24 - 0x18` where `stream_base` is the value of `0x60410`.
/// In memory sets: `S` holds element tables at `+0x14 + 4 * element`; a zero entry fails, else the offset is `4 * (trunc(entry / 4) + 6 * frame)`.
pub enum KeySet<'a> {
    Streamed { count: i32, stream_base: &'a dyn Fn(i32) -> u32 },
    Memory { element_entries: &'a [u32] },
}

pub fn element_record(set: &KeySet, frame: i32, key_count: i32, element: i32) -> Option<u32> {
    let frame = clamp_frame(frame, key_count);
    match set {
        KeySet::Streamed { count, stream_base } => {
            (element >= 1 && element <= *count).then(|| stream_base(frame).wrapping_add((element as u32).wrapping_mul(24)).wrapping_sub(0x18))
        }
        KeySet::Memory { element_entries } => {
            let entry = *element_entries.get(usize::try_from(element).ok()?)? as i32;
            (entry != 0).then(|| 4 * (entry / 4 + 6 * frame) as u32)
        }
    }
}

/// `0x257DC0`: node 0 sample of a clip -> `(position, direction)`, the direction being the forward basis `(0, 0, 1)` rotated by the sample quaternion
/// (row vector convention, translation zero: third matrix row, ASSUMED from `0x151E80` = point transform).
pub fn node_world_point(sample: &Sample) -> ([f32; 3], [f32; 3]) {
    let matrix = quat_matrix(sample.quaternion());
    (sample.position(), [matrix[8], matrix[9], matrix[10]])
}

/// One part for [`build_matrix_slots`]: `kind` (byte `+0x48`), own matrix `cell` (`+0x49`), `child` (`+0x4B`).
#[derive(Debug, Clone, Copy)]
pub struct PartKind {
    pub kind: u8,
    pub cell: i8,
    pub child: i8,
}

/// Result of `0x8FE30`: `slot_of_part[i]` (list at `[ctx+0x8C]+4`) and `cell_of_slot[slot]` (list at `[ctx+0x8C]`).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct MatrixSlots {
    pub slot_of_part: Vec<i32>,
    pub cell_of_slot: Vec<i32>,
}

/// `0x8FE30`: kind 0 parts get consecutive slots (and, with `source` matrices, `dst[cell] = source[slot]`); kind 2 followers copy their first child's
/// matrix (`dst[cell] = dst[child cell]`) and share its slot.
pub fn build_matrix_slots(parts: &[PartKind], source: Option<&[Mat4]>, dst: &mut [Mat4]) -> MatrixSlots {
    let mut slots = MatrixSlots { slot_of_part: vec![0; parts.len()], cell_of_slot: Vec::new() };
    for (index, part) in parts.iter().enumerate() {
        if part.kind != 0 {
            continue;
        }
        let slot = slots.cell_of_slot.len();
        if let Some(source) = source {
            dst[part.cell as usize] = source[slot];
        }
        slots.slot_of_part[index] = slot as i32;
        slots.cell_of_slot.push(i32::from(part.cell));
    }
    for (index, part) in parts.iter().enumerate() {
        if part.kind != 2 {
            continue;
        }
        let child = part.child as usize;
        if source.is_some() {
            dst[part.cell as usize] = dst[parts[child].cell as usize];
        }
        slots.slot_of_part[index] = slots.slot_of_part[child];
    }
    slots
}

/// Effects of `0x1C8520`: mount the pak at `0x47E60C`, run `0x1C97F0` (106 named clip registrations), set `[0x75A028] = 1`.
#[derive(Debug, PartialEq, Eq)]
pub enum AmbientStep {
    MountPak(u32),
    RegisterNamedClips,
    SetReadyFlag,
}

pub fn ambient_pak_steps() -> [AmbientStep; 3] {
    [AmbientStep::MountPak(0x47E60C), AmbientStep::RegisterNamedClips, AmbientStep::SetReadyFlag]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn element_records_follow_both_layouts() {
        let stream = |frame: i32| 1000 + frame as u32;
        let streamed = KeySet::Streamed { count: 3, stream_base: &stream };
        assert_eq!(element_record(&streamed, 2, 10, 1), Some(1002 + 24 - 0x18));
        assert_eq!(element_record(&streamed, 2, 10, 4), None);
        let memory = KeySet::Memory { element_entries: &[0, 0, 9] };
        assert_eq!(element_record(&memory, 1, 10, 1), None);
        assert_eq!(element_record(&memory, 1, 10, 2), Some(4 * (2 + 6)));
    }

    #[test]
    fn slots_are_consecutive_and_followers_share() {
        let identity: Mat4 = std::array::from_fn(|i| if i % 5 == 0 { 1.0 } else { 0.0 });
        let mut moved = identity;
        moved[12] = 5.0;
        let parts = [PartKind { kind: 0, cell: 0, child: -1 }, PartKind { kind: 2, cell: 1, child: 0 }, PartKind { kind: 0, cell: 2, child: -1 }];
        let mut dst = [identity; 3];
        let slots = build_matrix_slots(&parts, Some(&[moved, identity]), &mut dst);
        assert_eq!(slots.cell_of_slot, [0, 2]);
        assert_eq!(slots.slot_of_part, [0, 0, 1]);
        assert_eq!(dst[1][12], 5.0);
        assert_eq!(ambient_pak_steps()[2], AmbientStep::SetReadyFlag);
        let (position, direction) = node_world_point(&Sample { bits: [1.0f32.to_bits(), 0, 0, 0, 0, 0, 1.0f32.to_bits()] });
        assert_eq!((position, direction), ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]));
    }
}
