//! Runtime buffer planning and table init of the animation system (T1668, rule 10 batch 6). **Tier 1** for [`instance_arena_size`] and [`track_buffer_plan`] (`tests/buffers_golden.rs`, 400 seeded worlds each against the
//! original under Unicorn, allocator stubs); **Tier 0** for [`track_entry`] and [`reset_tables`] (translated, unit smoke tests only, see `docs/t1668-anim-tiers.md`).
//!
//! * [`instance_arena_size`] = the size computation of `0x965F0(context, instance)` (instance matrix arena; the model is `[instance+4]`): `((nodes + 2 * parts) * 4 + 0x8F) & !0xF`. The routine
//!   allocates that from the arena (`0x3DFA0`) unless the model's cached buffer (`model+0x44`, whose first dword holds its size) is large enough,
//!   points `instance+0x20` at it, zeroes `+0x60..0x78` and sets `[buf] = buf + 0x80` (matrices), `[buf+4] = that + nodes * 4`,
//!   `[buf+0x5C] = [buf+4] + parts * 4`, then builds the matrix slots with `0x8FE30`.
//! * [`track_buffer_plan`] = the allocation list of `0x96A40(pose, part, lod, mask)`: for each flag bit of `mask` a buffer sized from the vertex total
//!   `V` (`0x949C0` of the group records) and the group count `G` is stored in the pose object's slot table (`[pose+0x1B8]`, 24 bytes per
//!   `lod + 3 * part`).
//! * [`track_entry`] = `0x59110`'s non-streamed path (`clip+0x28` node records of 32 bytes, `+0x14` track pointer, entry = 32 bytes per key).
use crate::keyrec::clamp_frame;

/// `0x965F0` arena block size in bytes.
pub fn instance_arena_size(nodes: u32, parts: u32) -> u32 {
    ((nodes + 2 * parts) * 4 + 0x8F) & !0xF
}

/// One planned allocation: the slot table field byte offset and the size in bytes (16 byte aligned unless noted).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Allocation {
    pub field: u32,
    pub size: u32,
}

/// Slot table entry index of a (part, lod) pair: `lod + 3 * part`; the entry address is `[pose+0x1B8] + 24 * index`.
pub fn slot_index(part: u32, lod: u32) -> u32 {
    lod + 3 * part
}

/// The allocations of `0x96A40` in the order the original performs them (bits 0, 2, 1, 3, 4, 5 of `mask`). `vertices` = `0x949C0` result,
/// `groups` = number of group records. Bit 5 reuses field `+8` (after bit 1) with a size that leaves two bytes of header, the original then
/// advances the stored pointer by 2.
pub fn track_buffer_plan(vertices: u32, groups: u32, mask: u32) -> Vec<Allocation> {
    let mut plan = Vec::new();
    let aligned = |n: u32| (n + 0xF) & !0xF;
    if mask & 1 != 0 {
        plan.push(Allocation { field: 4, size: aligned(vertices * 16) });
    }
    if mask & 4 != 0 {
        plan.push(Allocation { field: 0x10, size: (vertices * 4 + 0x1F) & !0x1F });
    }
    if mask & 2 != 0 {
        plan.push(Allocation { field: 8, size: aligned(vertices * 3 * 4) });
    }
    if mask & 8 != 0 {
        plan.push(Allocation { field: 0, size: aligned(groups * 16) });
    }
    if mask & 0x10 != 0 {
        plan.push(Allocation { field: 0xC, size: aligned(vertices * 16) });
    }
    if mask & 0x20 != 0 {
        plan.push(Allocation { field: 8, size: (vertices * 3 * 2 + 0x15) & !0xF });
    }
    plan
}

/// `0x59110` non-streamed: the 8 dword entry of `node`'s track for `key` (clamped to the clip's key count `keys`); `None` when the node has no track.
pub fn track_entry<'a>(tracks: &'a [Option<Vec<[u32; 8]>>], node: usize, key: i32, keys: i32) -> Option<&'a [u32; 8]> {
    let track = tracks.get(node)?.as_ref()?;
    let key = if key < keys { key } else { keys - 1 };
    track.get(usize::try_from(key).ok()?)
}

/// `0x5C420` table reset: animation id to slot table (`0x7E03C0`, 0x601 dwords + 1 word of 0xFFFF), clip pointer table (`0x7E2AA0`, 0x3AD dwords of 0),
/// policy table (`0x7DE660`, 0x758 dwords of 0); the clip table global is set to `0x7E2AA4`.
pub const ID_SLOT_ENTRIES: usize = 0x601 * 2 + 1;
pub const CLIP_TABLE_DWORDS: usize = 0x3AD;
pub const POLICY_DWORDS: usize = 0x758;

pub fn reset_tables() -> (Vec<i16>, Vec<u32>, Vec<u32>) {
    (vec![-1; ID_SLOT_ENTRIES], vec![0; CLIP_TABLE_DWORDS], vec![0; POLICY_DWORDS])
}

pub fn frame_in_range(frame: i32, count: i32) -> i32 {
    clamp_frame(frame, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arena_size_is_sixteen_aligned() {
        assert_eq!(instance_arena_size(19, 24), (((19 + 48) * 4 + 0x8F) & !0xF));
        assert_eq!(instance_arena_size(1, 1) % 16, 0);
    }

    #[test]
    fn plan_follows_the_mask_bits_in_order() {
        let plan = track_buffer_plan(45, 3, 0x3F);
        let fields: Vec<u32> = plan.iter().map(|a| a.field).collect();
        assert_eq!(fields, [4, 0x10, 8, 0, 0xC, 8]);
        assert_eq!(plan[0].size, 720);
        assert!(track_buffer_plan(45, 3, 0).is_empty());
        assert_eq!(slot_index(2, 1), 7);
    }

    #[test]
    fn track_entry_clamps_the_key() {
        let tracks = vec![Some(vec![[1; 8], [2; 8]]), None];
        assert_eq!(track_entry(&tracks, 0, 9, 2), Some(&[2; 8]));
        assert_eq!(track_entry(&tracks, 1, 0, 2), None);
        assert_eq!(reset_tables().0.len(), 0xC03);
    }
}
