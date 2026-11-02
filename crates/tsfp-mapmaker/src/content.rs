//! mapmaker/content (T1899, batch 1). Tier 0, confidence INFERRED. Not yet ported (verification
//! debt): 0x2fa150 (tileset context point collection, 458 instructions).
use alloc::vec::Vec;
use tsfp_gameplay::memory::GuestMemory;

/// Selected weapon-set preset index (0x312ad0 reads it).
pub const WEAPON_SET_PRESET_INDEX: u32 = 0x0078_4028;

/// Callee outside the slice.
pub trait ContentHost {
    /// 0x312a60: base address of the weapon set list record (slots at +8, stride 4).
    fn weapon_set_list_base(&mut self) -> u32;
}

/// 0x312ad0 `game_mapedit_weapon_set_preset_index_get`.
pub fn weapon_set_preset_index(m: &impl GuestMemory) -> u32 {
    m.read32(WEAPON_SET_PRESET_INDEX)
}

/// 0x312ae0 `game_mapedit_weapon_set_list_set_slot_value`: store `value` in slot `slot`.
///
/// The host supplies the guest list base. Slots are four-byte words beginning at
/// byte 8; address arithmetic wraps at 32 bits, and no slot-count check is made.
///
/// ```
/// use tsfp_mapmaker::content::{weapon_set_list_set_slot, ContentHost};
/// use tsfp_gameplay::memory::GuestMemory;
///
/// struct Memory(std::collections::HashMap<u32, u8>);
/// impl GuestMemory for Memory {
///     fn read8(&self, address: u32) -> u8 { *self.0.get(&address).unwrap_or(&0) }
///     fn write8(&mut self, address: u32, value: u8) { self.0.insert(address, value); }
/// }
/// struct Host;
/// impl ContentHost for Host {
///     fn weapon_set_list_base(&mut self) -> u32 { 0x100 }
/// }
/// let mut memory = Memory(Default::default());
/// weapon_set_list_set_slot(&mut memory, &mut Host, 2, 0x1234_5678);
/// assert_eq!(memory.read32(0x110), 0x1234_5678);
/// ```
pub fn weapon_set_list_set_slot(
    m: &mut impl GuestMemory,
    h: &mut impl ContentHost,
    slot: u32,
    value: u32,
) {
    let base = h.weapon_set_list_base();
    m.write32(
        base.wrapping_add(slot.wrapping_mul(4)).wrapping_add(8),
        value,
    );
}

/// Store a little-endian word through the guest byte interface.
pub(crate) fn write16(m: &mut impl GuestMemory, address: u32, value: u16) {
    m.write8(address, value as u8);
    m.write8(address.wrapping_add(1), (value >> 8) as u8);
}

/// Services outside mapmaker/content used by `collect_tileset_context_points` (INFERRED names).
pub trait TilesetHost {
    /// 0x331a20 (record id, record c) -> non zero when the record belongs to a tileset group.
    fn fn_00331a20_tileset_group(&mut self, id: u32, c: u32) -> u32;
    /// 0x331920 (record id, record c) -> non zero for the second flag.
    fn fn_00331920_group_flag(&mut self, id: u32, c: u32) -> u32;
    /// 0x332490 (0x3e9, context) -> base object id (minus 0x3e9 gives the local index).
    fn fn_00332490_object_base(&mut self, tag: u32, context: u32) -> u32;
    /// 0x630c0 resource table entry for a table address.
    fn fn_000630c0_resource_entry(&mut self, table: u32) -> u32;
    /// 0x331970 record kind predicate.
    fn fn_00331970_kind_predicate(&mut self, id: u32) -> u32;
    /// 0x331990 (record c) -> count of placed items.
    fn fn_00331990_placed_count(&mut self, c: u32) -> u32;
    /// 0x1531d0 matrix copy (destination, source).
    fn fn_001531d0_matrix_copy(&mut self, m: &mut dyn GuestMemory, destination: u32, source: u32);
    /// 0x1518f0 set the translation (vec3 0x30) of a matrix from three floats.
    fn fn_001518f0_matrix_set_translation(
        &mut self,
        m: &mut dyn GuestMemory,
        matrix: u32,
        x: f32,
        y: f32,
        z: f32,
    );
    /// 0x334b20 collect the points of a cell box: (context, packed cell (u32 + u16, 2 stack dwords by
    /// value), out array) -> number of points written. The upper 16 bits of the second dword are
    /// uninitialised stack padding in the original and are not part of the argument.
    fn fn_00334b20_collect_points(
        &mut self,
        m: &mut dyn GuestMemory,
        context: u32,
        cell_low: u32,
        cell_high: u16,
        out: u32,
    ) -> u32;
    /// 0x2f7830 and 0x2f77e0 per id flags (bit 1 and bit 0 of byte 0x14, with profile override).
    fn fn_002f7830_flag_bit1(&mut self, id: u32) -> u32;
    fn fn_002f77e0_flag_bit0(&mut self, id: u32) -> u32;
    /// 0x151e80 transform a vec3 in place by a matrix (matrix, vec3).
    fn fn_00151e80_transform_point(&mut self, m: &mut dyn GuestMemory, matrix: u32, point: u32);
    /// 0x9a630 walk the linked point group starting at `start` marking visited bytes at 0x7b67a0.
    fn fn_0009a630_walk_group(
        &mut self,
        m: &mut dyn GuestMemory,
        start: u32,
        zero_a: u32,
        last: u32,
        records: u32,
        zero_b: u32,
        link_count: u32,
        links: u32,
    );
}

const FRAME_OUT: u32 = 0x7c;
const FRAME_MATRIX: u32 = 0x3c;
const FRAME_ARRAY_B: u32 = 0xfc;
const FRAME_ARRAY_A: u32 = 0x17c;
const FRAME_RECORDS: u32 = 0x1fc;
const FRAME_LINKS: u32 = 0x5fc;
const VISITED: u32 = 0x007b_67a0;
const DESCRIPTORS: u32 = 0x0052_9510;
const RESOURCE_TABLES: u32 = 0x0078_5080;
const MATRIX_TABLE_A: u32 = 0x0052_8ca8;
const MATRIX_TABLE_B: u32 = 0x0052_8da8;
const SCALE_CONSTANT: u32 = 0x0047_5d30;
const SCALE_CONSTANT_Y: u32 = 0x0047_60cc;

/// Upper bound on the group labels returned by the tileset collector.
///
/// This is not a point count: the original caller adds it to its context
/// accumulator after each placement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroupCount(pub u32);

/// Point fields used while collecting and splitting linked groups.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollectorPoint {
    /// Original active/connected flags; bit zero selects active points.
    pub flags: u8,
    /// Signed group label from the resource point row.
    pub group: i16,
}

/// Relabel active points outside the walked component into fresh groups.
///
/// The callback is the attachment to the graph walker. The collector owns the
/// point and group state; an ABI adapter may still provide visited bits while
/// the native resource graph and traversal attachment are being completed.
/// The returned indices identify the labels changed by the split.
pub fn split_unvisited_groups(
    points: &mut [CollectorPoint],
    initial: GroupCount,
    enabled: bool,
    mut walk: impl FnMut(usize, u32, &[CollectorPoint]) -> Vec<bool>,
) -> (GroupCount, Vec<usize>) {
    let mut groups = initial.0;
    let mut changed = Vec::new();
    if !enabled || (groups as i32) <= 0 {
        return (GroupCount(groups), changed);
    }

    let mut group = 0i32;
    while group < groups as i32 {
        let first = points.iter().position(|point| {
            point.flags & 1 != 0 && point.group >= 0 && i32::from(point.group) == group
        });
        if let Some(start) = first {
            let visited = walk(start, group as u32, points);
            let mut new_group = None;
            for (index, point) in points.iter_mut().enumerate() {
                if point.flags & 1 == 0
                    || point.group < 0
                    || i32::from(point.group) != group
                    || visited.get(index).copied().unwrap_or(false)
                {
                    continue;
                }
                let label = *new_group.get_or_insert_with(|| {
                    groups = groups.wrapping_add(1);
                    groups.wrapping_sub(1)
                });
                point.group = label as u16 as i16;
                changed.push(index);
            }
        }
        group += 1;
    }
    (GroupCount(groups), changed)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ContextPointKey(u32);

/// Rust-owned per-call selection lists for the 0x2fa150 attachment.
#[derive(Default)]
struct CollectorScratch {
    connected: Vec<ContextPointKey>,
    regroup: Vec<ContextPointKey>,
}

/// 0x2fa150 `game_mapedit_collect_tileset_context_points_in_cell_box_transform_and_mark_linked_groups`.
/// For a placed tileset record (`record`) of the editor context (`context`): builds the matrix of the
/// tileset instance, collects the context points inside the record's cell box, copies and transforms the
/// tileset's point records into a local table (0x20 byte entries, ids from 0x3e9), clears the
/// "connected" bit of points that coincide with collected points, resolves the 0x14 byte link records
/// to point indices and numbers the linked groups through `0x9a630`. Returns the number of groups
/// (the largest group number plus one). The local arrays live in the original's 0xfec byte frame, so
/// `entry_esp` is a parameter (the host receives pointers into it).
pub fn collect_tileset_context_points(
    m: &mut impl GuestMemory,
    h: &mut impl TilesetHost,
    entry_esp: u32,
    record: u32,
    context: u32,
) -> u32 {
    let s = entry_esp.wrapping_sub(0xffc);
    let (count_a, count_b, group_max, point_total) = (s + 0x1c, s + 0x20, s + 0x10, s + 0x18);
    let matrix = s + FRAME_MATRIX;
    let records = s + FRAME_RECORDS;
    let links = s + FRAME_LINKS;
    let id = m.read32(record);
    let c = m.read32(record + 8);
    let descriptor = id
        .wrapping_add(id.wrapping_mul(4))
        .wrapping_shl(6)
        .wrapping_add(DESCRIPTORS);
    m.write32(count_a, 0);
    m.write32(count_b, 0);
    let grouped = h.fn_00331a20_tileset_group(id, c);
    let linked =
        grouped != 0 && h.fn_00331920_group_flag(m.read32(record), m.read32(record + 8)) != 0;
    let linked_flag = u32::from(linked);
    m.write32(s + 0x14, linked_flag);
    let base = h
        .fn_00332490_object_base(0x3e9, context)
        .wrapping_sub(0x3e9);
    let table = u32::from(m.read8(descriptor + 0x11))
        .wrapping_shl(6)
        .wrapping_add(RESOURCE_TABLES);
    let entry = h.fn_000630c0_resource_entry(table);
    let object = m.read32(entry.wrapping_add(0x10));
    let sub_count = m.read32(object + 8);
    let source_count = m.read32(object + 4);
    let source = object.wrapping_add(0xc);
    let source_end = source.wrapping_add(source_count.wrapping_shl(5));
    m.write32(s + 0x28, sub_count);
    m.write32(point_total, source_count);
    m.write32(s + 0x2c, source);
    m.write32(s + 0x34, source_end);
    m.write32(group_max, 1);
    if (source_count as i32) > 0 {
        let mut at = source.wrapping_add(8);
        for _ in 0..source_count {
            let value = i32::from(m.read16(at) as i16).wrapping_add(1);
            if value > m.read32(group_max) as i32 {
                m.write32(group_max, value as u32);
            }
            at = at.wrapping_add(0x20);
        }
    }
    let id_now = m.read32(record);
    if h.fn_00331970_kind_predicate(id_now) == 0 && grouped == 0 {
        return m.read32(group_max);
    }
    if h.fn_00331970_kind_predicate(m.read32(record)) != 0 {
        let placed = h.fn_00331990_placed_count(m.read32(record + 8));
        m.write32(point_total, placed);
    }
    let matrix_source = if m.read8(descriptor + 0x12) & 1 == 0 {
        u32::from(m.read8(record + 7))
            .wrapping_shl(6)
            .wrapping_add(MATRIX_TABLE_A)
    } else {
        (i32::from(m.read8(record + 7) as i8) as u32)
            .wrapping_shl(6)
            .wrapping_add(MATRIX_TABLE_B)
    };
    h.fn_001531d0_matrix_copy(m, matrix, matrix_source);
    let scale = f32::from_bits(m.read32(SCALE_CONSTANT));
    let scale_y = f32::from_bits(m.read32(SCALE_CONSTANT_Y));
    let x = (i32::from(m.read8(record + 4) as i8) as f32) * scale;
    let y = (i32::from(m.read8(record + 5) as i8) as f32) * scale_y;
    let z = (i32::from(m.read8(record + 6) as i8) as f32) * scale;
    h.fn_001518f0_matrix_set_translation(m, matrix, x, y, z);

    // Collect the context points of every cell node, split by the node's id flags.
    let mut selected = CollectorScratch::default();
    let mut node = m.read32(context + 0x3c);
    let mut array_a_count = m.read32(count_b);
    while node != 0 {
        let low = m.read32(node + 0xc);
        let high = m.read16(node + 0x10);
        let found = h.fn_00334b20_collect_points(m, context, low, high, s + FRAME_OUT);
        if found != 0 {
            let node_id = m.read32(node);
            let to_a = h.fn_002f7830_flag_bit1(node_id) == 0;
            if to_a {
                if h.fn_002f77e0_flag_bit0(m.read32(node)) != 0 {
                    for k in 0..found as i32 {
                        let point = m.read32(s + FRAME_OUT + 4 * k as u32);
                        m.write32(
                            s + FRAME_ARRAY_A + 4 * array_a_count,
                            point.wrapping_sub(base),
                        );
                        selected
                            .connected
                            .push(ContextPointKey(point.wrapping_sub(base)));
                        array_a_count = array_a_count.wrapping_add(1);
                    }
                }
            } else {
                let mut array_b_count = m.read32(count_a);
                for k in 0..found as i32 {
                    let point = m.read32(s + FRAME_OUT + 4 * k as u32);
                    m.write32(
                        s + FRAME_ARRAY_B + 4 * array_b_count,
                        point.wrapping_sub(base),
                    );
                    selected
                        .regroup
                        .push(ContextPointKey(point.wrapping_sub(base)));
                    array_b_count = array_b_count.wrapping_add(1);
                }
                m.write32(count_a, array_b_count);
            }
        }
        node = m.read32(node + 0x14);
    }
    m.write32(count_b, array_a_count);

    // Copy and transform the tileset's point records.
    let total = m.read32(point_total);
    if (total as i32) > 0 {
        // scratch pointers the original keeps in its frame (offsets of the local table from the source)
        m.write32(s + 0x30, (s + 0x204).wrapping_sub(source));
        m.write32(s + 0x38, (s + 0x208).wrapping_sub(source));
        m.write32(s + 0x24, (s + 0x200).wrapping_sub(source));
    }
    for i in 0..(if (total as i32) > 0 { total } else { 0 }) {
        let src = source.wrapping_add(i.wrapping_shl(5));
        let dst = records.wrapping_add(i.wrapping_shl(5));
        m.write32(dst, i.wrapping_add(0x3e9));
        m.write32(dst + 4, m.read32(src + 4));
        write16(m, dst + 8, m.read16(src + 8));
        m.write32(dst + 0xc, m.read32(src + 0xc));
        m.write32(dst + 0x10, m.read32(src + 0x10));
        m.write32(dst + 0x14, m.read32(src + 0x14));
        if m.read32(s + 0x14) != 0 {
            let flags = m.read32(dst + 4);
            if flags & 0x40 == 0 {
                m.write32(dst + 4, flags & !1);
            }
        }
        h.fn_00151e80_transform_point(m, matrix, dst + 0xc);
        if (m.read32(count_b) as i32) > 0
            && selected.connected.iter().any(|key| key.0 == m.read32(dst))
        {
            m.write32(dst + 4, m.read32(dst + 4) & !1);
        }
        if (m.read32(count_a) as i32) > 0 {
            let id = m.read32(dst);
            for key in &selected.regroup {
                if key.0 == id {
                    let number = m.read32(group_max);
                    write16(m, dst + 8, number as u16);
                    m.write32(group_max, number.wrapping_add(1));
                }
            }
        }
    }
    // Resolve the link records after the point records.
    let link_count = m.read32(s + 0x28);
    if (link_count as i32) > 0 {
        let mut link = source_end;
        let mut entry_at = links;
        m.write32(s + 0x14, source_end.wrapping_add(4));
        m.write32(s + 0x24, link_count);
        for _ in 0..link_count {
            let flags = m.read32(link) | 0x10000;
            m.write32(entry_at, flags);
            m.write32(entry_at + 4, u32::MAX);
            m.write32(entry_at + 8, u32::MAX);
            let total = m.read32(point_total) as i32;
            for j in 0..total.max(0) as u32 {
                let point_id = m.read32(source.wrapping_add(j.wrapping_shl(5)));
                if point_id != m.read32(link + 4) {
                    if point_id == m.read32(link + 8) {
                        m.write32(entry_at + 8, j);
                    }
                } else {
                    m.write32(entry_at + 4, j);
                }
            }
            let (first, second) = (m.read32(entry_at + 4), m.read32(entry_at + 8));
            let valid = first != u32::MAX
                && second != u32::MAX
                && m.read8(records.wrapping_add(first.wrapping_shl(5)).wrapping_add(4)) & 1 != 0
                && m.read8(records.wrapping_add(second.wrapping_shl(5)).wrapping_add(4)) & 1 != 0;
            if !valid {
                m.write32(entry_at, m.read32(entry_at) & 0xfffe_ffff);
            }
            let record_id = m.read32(record);
            if h.fn_00331970_kind_predicate(record_id) != 0 {
                let mut flags = m.read32(entry_at);
                if flags & 2 != 0 {
                    let record_c = m.read32(record + 8);
                    let masked_a = record_c & 0x0800_0000;
                    let masked_b = record_c & 0x0100_0000;
                    let mut store = false;
                    if masked_a != 0 && masked_b == 0 {
                        flags &= 0xfffe_ffff;
                        store = true;
                    } else if masked_a == 0 {
                        if masked_b != 0 {
                            flags &= !2;
                            store = true;
                        }
                    } else if m.read32(link + 8) == 0x3ec || m.read32(link + 4) == 0x3ec {
                        flags &= 0xfffe_ffff;
                        store = true;
                    }
                    if store {
                        m.write32(entry_at, flags);
                    }
                }
            }
            link = link.wrapping_add(0x14);
            entry_at = entry_at.wrapping_add(0x14);
            m.write32(s + 0x14, link.wrapping_add(4));
            let left = m.read32(s + 0x24).wrapping_sub(1);
            m.write32(s + 0x24, left);
        }
    }
    // Number the linked groups.
    let total = m.read32(point_total) as i32;
    let initial_groups = GroupCount(m.read32(group_max));
    let (count_b_now, count_a_now) = (m.read32(count_b) as i32, m.read32(count_a) as i32);
    let mut active_points = (0..total.max(0) as u32)
        .map(|index| {
            let rec = records.wrapping_add(index.wrapping_shl(5));
            CollectorPoint {
                flags: m.read8(rec + 4),
                group: m.read16(rec + 8) as i16,
            }
        })
        .collect::<Vec<_>>();
    let (final_groups, relabeled) = split_unvisited_groups(
        &mut active_points,
        initial_groups,
        count_b_now.wrapping_add(count_a_now) > 0,
        |index, _group, points| {
            // Keep the ABI walker pointed at the original frame layout while
            // reflecting typed group changes between successive traversals.
            for (point, state) in points.iter().enumerate() {
                let address = records.wrapping_add((point as u32).wrapping_shl(5) + 8);
                if m.read16(address) as i16 != state.group {
                    write16(m, address, state.group as u16);
                }
            }
            for k in 0..8 {
                m.write32(VISITED + 4 * k, 0);
            }
            let link_total = m.read32(s + 0x28);
            h.fn_0009a630_walk_group(
                m,
                index as u32,
                0,
                (total as u32).wrapping_sub(1),
                records,
                0,
                link_total,
                links,
            );
            (0..total.max(0) as u32)
                .map(|point| m.read8(VISITED + point) != 0)
                .collect()
        },
    );
    for index in relabeled {
        let rec = records.wrapping_add((index as u32).wrapping_shl(5));
        write16(m, rec + 8, active_points[index].group as u16);
    }
    final_groups.0
}

#[cfg(test)]
#[path = "test_memory.rs"]
mod test_memory;

#[cfg(test)]
mod collector_tests {
    use super::{CollectorPoint, GroupCount, split_unvisited_groups};

    #[test]
    fn splits_only_active_points_outside_the_walked_component() {
        let mut points = [
            CollectorPoint { flags: 1, group: 0 },
            CollectorPoint { flags: 1, group: 0 },
            CollectorPoint { flags: 0, group: 0 },
            CollectorPoint { flags: 1, group: 1 },
        ];
        let mut walks = Vec::new();
        let (groups, changed) =
            split_unvisited_groups(&mut points, GroupCount(2), true, |start, group, _points| {
                walks.push((start, group));
                if group == 0 {
                    vec![true, false, false, false]
                } else {
                    vec![false, true, false, true]
                }
            });

        assert_eq!(groups, GroupCount(3));
        assert_eq!(changed, vec![1]);
        assert_eq!(points[1].group, 2);
        assert_eq!(points[2].group, 0);
        assert_eq!(points[3].group, 1);
        assert_eq!(walks, vec![(0, 0), (3, 1), (1, 2)]);
    }

    #[test]
    fn disabled_split_keeps_the_baseline_group_upper_bound() {
        let mut points = [CollectorPoint { flags: 1, group: 0 }];
        let (groups, changed) =
            split_unvisited_groups(&mut points, GroupCount(4), false, |_, _, _| {
                panic!("walker called with no selected context points")
            });
        assert_eq!(groups, GroupCount(4));
        assert!(changed.is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::test_memory::Mem;
    use super::*;
    struct Host;
    impl ContentHost for Host {
        fn weapon_set_list_base(&mut self) -> u32 {
            0x900
        }
    }
    #[test]
    fn weapon_set() {
        let mut m = Mem::default();
        m.write32(WEAPON_SET_PRESET_INDEX, 3);
        assert_eq!(weapon_set_preset_index(&m), 3);
        weapon_set_list_set_slot(&mut m, &mut Host, 2, 0xabcd);
        assert_eq!(m.read32(0x900 + 8 + 8), 0xabcd);
    }

    #[test]
    fn weapon_set_slot_address_and_neighbor_grid() {
        struct BaseHost(u32);
        impl ContentHost for BaseHost {
            fn weapon_set_list_base(&mut self) -> u32 {
                self.0
            }
        }
        for base in [0, 0x900, u32::MAX - 5, u32::MAX] {
            for slot in [0, 1, 2, 31, 0x4000_0000, u32::MAX] {
                let mut m = Mem::default();
                let address = base.wrapping_add(slot.wrapping_mul(4)).wrapping_add(8);
                let before = address.wrapping_sub(4);
                let after = address.wrapping_add(4);
                m.write32(before, 0xfeed_beef);
                m.write32(after, 0xface_cafe);
                weapon_set_list_set_slot(&mut m, &mut BaseHost(base), slot, slot ^ base);
                assert_eq!(
                    m.read32(address),
                    slot ^ base,
                    "base={base:#x} slot={slot:#x}"
                );
                assert_eq!(m.read32(before), 0xfeed_beef);
                assert_eq!(m.read32(after), 0xface_cafe);
            }
        }
    }
}
