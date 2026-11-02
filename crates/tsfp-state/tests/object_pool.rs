// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED synthetic fixtures (patterned bytes); no retail data.
use std::collections::{BTreeMap, BTreeSet};
use tsfp_state::GuestMemory;
use tsfp_state::character::{
    CharacterRetirementMembership, CharacterSet, MEMBER_COUNT, TABLE as CHARACTER_TABLE,
};
use tsfp_state::generated_views::TsfpObjectView;
use tsfp_state::object_pool::*;

const CSV: &str = include_str!("../../../docs/data/t2057-write-attribution.csv");
const RECS: u32 = 0x43b0_0000;
const ARENA: u32 = 0x43c0_0000;
const NODES: [u32; 3] = [0x43d0_0000, 0x43d0_0100, 0x43d0_0200];

#[derive(Clone, Debug)]
struct Image(BTreeMap<u32, u8>);
/// Absent bytes read as zero, so equality ignores explicit zero entries.
impl PartialEq for Image {
    fn eq(&self, o: &Self) -> bool {
        let nz = |m: &Image| {
            m.0.iter()
                .filter(|(_, v)| **v != 0)
                .map(|(k, v)| (*k, *v))
                .collect::<Vec<_>>()
        };
        nz(self) == nz(o)
    }
}
impl GuestMemory for Image {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.insert(a, v);
    }
}

fn image() -> Image {
    let mut m = Image(BTreeMap::new());
    for i in 0..HEADER_WORDS as u32 {
        m.write32(HEADER + 4 * i, 0x1000 + i);
    }
    m.write32(HEADER + 4 * LIST_LEN_INDEX as u32, 3);
    for t in 0..TABLE_COUNT {
        for c in 0..CELLS {
            m.write32(cell_addr(t, c), (t as u32) << 12 | c as u32 + 1);
        }
    }
    for (i, (a, _)) in SCALARS.iter().enumerate() {
        m.write32(*a, 0x2000 + i as u32);
    }
    m.write32(SCALARS[S_RECORDS_BASE].0, RECS);
    m.write32(SCALARS[S_USED_RECORDS].0, 3);
    m.write32(SCALARS[S_LIST_HEAD].0, NODES[0]);
    m.write32(SCALARS[S_ARENA_BASE].0, ARENA);
    m.write32(SCALARS[S_ARENA_USED].0, 0x10);
    m.write32(NODES[0], NODES[1]);
    m.write32(NODES[1], NODES[2]);
    m.write32(NODES[2], 0);
    for r in 0..3u32 {
        for j in 0..RECORD_WORDS as u32 {
            m.write32(
                rec_addr(RECS, r as usize, j * 4),
                0x9000_0000 | r << 16 | j + 1,
            );
        }
    }
    m
}

#[test]
fn round_trip_and_chain() {
    let m = image();
    let p = ObjectPool::load(&m);
    assert_eq!(p.diff(&m), None);
    assert_eq!(p.owned_records(), 3);
    assert_eq!(p.chain_len(), 3);
    assert_eq!(&p.chain[..3], &NODES);
    assert_eq!(p.list_len(), 3);
    let mut copy = Image(BTreeMap::new());
    p.store(&mut copy);
    assert_eq!(ObjectPool::load(&copy), p);
}

#[test]
fn refresh_from_guest_replaces_the_complete_owned_pool() {
    let mut pool = ObjectPool::load(&image());
    let mut next = image();
    next.write32(HEADER, 0xfeed_beef);
    next.write32(rec_addr(RECS, 1, 4), 0x1234_5678);
    pool.refresh_from_guest(&next);
    assert_eq!(pool.header[0], 0xfeed_beef);
    assert_eq!(pool.owned_record(1).unwrap()[1], 0x1234_5678);
    assert_eq!(pool, ObjectPool::load(&next));
}

#[test]
fn store_touches_only_declared_extents() {
    let mut m = image();
    m.write32(0x7b6700, 0xdead_beef); // gap between tables and elsewhere
    m.write32(RECS + 3 * RECORD_SIZE, 0xfeed_f00d); // record 3 is beyond the used count
    let p = ObjectPool::load_limited(&m, 101);
    let mut other = m.clone();
    other.write32(0x7b6700, 1);
    other.write32(RECS + 3 * RECORD_SIZE, 2);
    p.store(&mut other);
    assert_eq!(other.read32(0x7b6700), 1);
    assert_eq!(other.read32(RECS + 3 * RECORD_SIZE), 2);
}

#[test]
fn setters_write_through_and_view_agrees() {
    let mut m = image();
    let mut p = ObjectPool::load(&m);
    assert!(p.set_count(&mut m, 5, 77));
    assert!(p.set_planned(&mut m, 5, 78));
    assert!(p.set_cap(&mut m, 5, 79));
    assert!(p.set_members_ptr(&mut m, 5, 0x1234));
    assert!(p.set_header(&mut m, 1, 0xabc));
    assert!(p.set_scalar(&mut m, S_CAPACITY, 0x226));
    assert!(p.set_record_word(&mut m, 1, 0x28, 0x8000_0001));
    assert!(p.set_record_word(&mut m, 2, 0x20, 0x55));
    assert!(!p.set_record_word(&mut m, 3, 0x20, 1), "beyond used");
    assert!(!p.set_record_word(&mut m, 0, 0x2e8, 1), "past the record");
    assert_eq!(p.diff(&m), None);
    assert_eq!(m.read32(cell_addr(COUNTS, 5)), 77);
    assert_eq!(m.read32(cell_addr(CAPS, 5)), 79);
    assert_eq!(m.read32(cell_addr(MEMBERS, 5)), 0x1234);
    assert_eq!(ObjectPool::load(&m), p);
    let owned = p.owned(1).unwrap();
    assert_eq!(owned, TsfpObjectView(rec_addr(RECS, 1, 0)).load(&m));
    assert_eq!(owned.object_flags, 0x8000_0001);
    assert!(p.owned(3).is_none());
}

#[test]
fn typed_header_state_round_trips_all_eight_words() {
    let mut m = image();
    let initial_words = [0x1100, 0x1101, 0x1102, 0x1103, 0x1104, 0x1105, 3, 0x1107];
    for (index, value) in initial_words.into_iter().enumerate() {
        m.write32(HEADER + 4 * index as u32, value);
    }
    let mut p = ObjectPool::load(&m);
    let initial = PoolHeaderState {
        words: initial_words,
    };
    assert_eq!(p.header_state(), initial);
    assert_eq!(p.list_len(), initial.words[LIST_LEN_INDEX]);

    let updated = PoolHeaderState {
        words: [0x2100, 0x2101, 0x2102, 0x2103, 0x2104, 0x2105, 7, 0x2107],
    };
    p.set_header_state(&mut m, updated);
    assert_eq!(p.header_state(), updated);
    assert_eq!(p.list_len(), 7);
    for (index, value) in updated.words.into_iter().enumerate() {
        assert_eq!(m.read32(HEADER + 4 * index as u32), value);
    }
    assert_eq!(p.diff(&m), None);
}

#[test]
fn typed_update_list_state_reads_and_writes_endpoints_and_length() {
    let mut m = image();
    m.write32(HEADER + 4 * LIST_LEN_INDEX as u32, 3);
    m.write32(SCALARS[S_LIST_HEAD].0, 0x43d0_0010);
    m.write32(SCALARS[S_LIST_TAIL].0, 0x43d0_0020);
    let mut p = ObjectPool::load(&m);
    let other_header_word = p.header_state().words[0];
    let other_scalar = p.scalar(0);

    assert_eq!(
        p.update_list_state(),
        PoolUpdateListState {
            head: 0x43d0_0010,
            tail: 0x43d0_0020,
            len: 3,
        }
    );

    let updated = PoolUpdateListState {
        head: 0x43d0_0110,
        tail: 0x43d0_0120,
        len: 5,
    };
    p.set_update_list_state(&mut m, updated);

    assert_eq!(p.update_list_state(), updated);
    assert_eq!(m.read32(SCALARS[S_LIST_HEAD].0), updated.head);
    assert_eq!(m.read32(SCALARS[S_LIST_TAIL].0), updated.tail);
    assert_eq!(m.read32(HEADER + 4 * LIST_LEN_INDEX as u32), updated.len);
    assert_eq!(p.header_state().words[0], other_header_word);
    assert_eq!(m.read32(HEADER), other_header_word);
    assert_eq!(p.scalar(0), other_scalar);
    assert_eq!(m.read32(SCALARS[0].0), other_scalar);
    assert_eq!(p.diff(&m), None);
}

#[test]
fn writers_reject_invalid_indices_without_mutation() {
    let mut m = image();
    let mut p = ObjectPool::load(&m);
    let owned_before = p.clone();
    let guest_before = m.clone();

    assert!(!p.set_header(&mut m, HEADER_WORDS, 1));
    assert!(!p.set_cell(&mut m, TABLE_COUNT, 0, 1));
    assert!(!p.set_cell(&mut m, COUNTS, CELLS, 1));
    assert!(!p.set_count(&mut m, CELLS, 1));
    assert!(!p.set_planned(&mut m, CELLS, 1));
    assert!(!p.set_cap(&mut m, CELLS, 1));
    assert!(!p.set_members_ptr(&mut m, CELLS, 1));
    assert!(!p.set_scalar(&mut m, SCALARS.len(), 1));

    assert_eq!(p, owned_before);
    assert!(m == guest_before);
}

#[test]
fn typed_cell_state_round_trips_all_five_parallel_tables() {
    let mut m = image();
    // The shared patterned fixture intentionally gives each table a distinct
    // value; seed this cell with the explicit state under test.
    m.write32(cell_addr(COUNTS, 5), 6);
    m.write32(cell_addr(PLANNED, 5), 7);
    m.write32(cell_addr(CAPS, 5), 8);
    m.write32(cell_addr(MEMBERS, 5), ARENA + 0x20);
    m.write32(cell_addr(AUX, 5), 9);
    let mut p = ObjectPool::load(&m);
    let initial = PoolCellState {
        count: 6,
        planned: 7,
        capacity: 8,
        members_ptr: ARENA + 0x20,
        auxiliary: 9,
    };
    assert_eq!(p.cell_state(5), Some(initial));

    // The adapter preserves distinct words verbatim; it does not enforce
    // unmeasured allocator invariants such as count <= capacity.
    let updated = PoolCellState {
        count: 11,
        planned: 12,
        capacity: 4,
        members_ptr: ARENA + 0x40,
        auxiliary: 13,
    };
    assert!(p.set_cell_state(&mut m, 5, updated));
    assert_eq!(p.cell_state(5), Some(updated));
    assert_eq!(m.read32(cell_addr(COUNTS, 5)), updated.count);
    assert_eq!(m.read32(cell_addr(PLANNED, 5)), updated.planned);
    assert_eq!(m.read32(cell_addr(CAPS, 5)), updated.capacity);
    assert_eq!(m.read32(cell_addr(MEMBERS, 5)), updated.members_ptr);
    assert_eq!(m.read32(cell_addr(AUX, 5)), updated.auxiliary);
    assert_eq!(p.diff(&m), None);

    let owned_before = p.clone();
    let guest_before = m.clone();
    assert_eq!(p.cell_state(CELLS), None);
    assert!(!p.set_cell_state(&mut m, CELLS, initial));
    assert_eq!(p, owned_before);
    assert!(m == guest_before);
}

#[test]
fn opaque_auxiliary_cell_writer_preserves_neighboring_tables() {
    let mut m = image();
    let mut p = ObjectPool::load(&m);
    let before = p.cell_state(214).unwrap();
    let neighboring_aux = m.read32(cell_addr(AUX, 213));

    assert!(p.set_cell_auxiliary(&mut m, 214, 0xDEAD_BEEF));
    let after = p.cell_state(214).unwrap();
    assert_eq!(after.auxiliary, 0xDEAD_BEEF);
    assert_eq!(after.count, before.count);
    assert_eq!(after.planned, before.planned);
    assert_eq!(after.capacity, before.capacity);
    assert_eq!(after.members_ptr, before.members_ptr);
    assert_eq!(m.read32(cell_addr(AUX, 214)), 0xDEAD_BEEF);
    assert_eq!(m.read32(cell_addr(AUX, 213)), neighboring_aux);
    assert_eq!(p.diff(&m), None);

    let owned_before = p.clone();
    let guest_before = m.clone();
    assert!(!p.set_cell_auxiliary(&mut m, CELLS, 1));
    assert_eq!(p, owned_before);
    assert!(m == guest_before);
}

#[test]
fn typed_cell_auxiliary_state_updates_only_the_selected_cell_word() {
    let mut m = image();
    let mut pool = ObjectPool::load(&m);
    let cell = 37;
    let before = pool.cell_state(cell).unwrap();
    let next = 0xA55A_1020;

    assert!(pool.set_cell_auxiliary(&mut m, cell, next));
    let after = pool.cell_state(cell).unwrap();
    assert_eq!(after.auxiliary, next);
    assert_eq!(after.count, before.count);
    assert_eq!(after.planned, before.planned);
    assert_eq!(after.capacity, before.capacity);
    assert_eq!(after.members_ptr, before.members_ptr);
    assert_eq!(m.read32(cell_addr(AUX, cell)), next);

    let other = cell + 1;
    assert_eq!(
        pool.cell_state(other).unwrap().auxiliary,
        m.read32(cell_addr(AUX, other))
    );
    assert_eq!(pool.diff(&m), None);
}

#[test]
fn typed_scalar_state_round_trips_and_writes_all_scalar_words() {
    let mut m = image();
    let mut p = ObjectPool::load(&m);
    let initial = PoolScalarState {
        arena_used: 0x10,
        arena_base: ARENA,
        capacity: 0x2002,
        records_base: RECS,
        aux_base: 0x2004,
        pool_flag: 0x2005,
        bitmap_base: 0x2006,
        chunk_index: 0x2007,
        used_records: 3,
        first_record: 0x2009,
        list_head: NODES[0],
        list_tail: 0x200b,
    };
    assert_eq!(p.scalar_state(), initial);

    let updated = PoolScalarState {
        arena_used: 0x18,
        arena_base: ARENA,
        capacity: 0x300,
        records_base: RECS,
        aux_base: 0x44,
        pool_flag: 0x55,
        bitmap_base: 0x66,
        chunk_index: 7,
        used_records: 3,
        first_record: 0x77,
        list_head: NODES[1],
        list_tail: NODES[2],
    };
    p.set_scalar_state(&mut m, updated);
    assert_eq!(p.scalar_state(), updated);
    let expected = [
        updated.arena_used,
        updated.arena_base,
        updated.capacity,
        updated.records_base,
        updated.aux_base,
        updated.pool_flag,
        updated.bitmap_base,
        updated.chunk_index,
        updated.used_records,
        updated.first_record,
        updated.list_head,
        updated.list_tail,
    ];
    for (index, value) in expected.into_iter().enumerate() {
        assert_eq!(m.read32(SCALARS[index].0), value, "scalar {index}");
    }
}

#[test]
fn typed_scalar_state_updates_preserve_other_owned_regions() {
    let mut m = image();
    let mut pool = ObjectPool::load(&m);
    let before = pool.scalar_state();
    let header_before = pool.header_state();
    let cell_before = pool.cell_state(17).unwrap();
    let record_before = pool.record_state(1).unwrap();
    let changed = PoolScalarState {
        arena_used: before.arena_used.wrapping_add(4),
        ..before
    };
    pool.set_scalar_state(&mut m, changed);
    assert_eq!(pool.scalar_state(), changed);
    assert_eq!(m.read32(SCALARS[S_ARENA_USED].0), changed.arena_used);
    assert_eq!(pool.header_state(), header_before);
    assert_eq!(pool.cell_state(17), Some(cell_before));
    assert_eq!(pool.record_state(1), Some(record_before));
    assert_eq!(pool.diff(&m), None);
}

#[test]
fn ordered_members_are_owned_and_write_through() {
    let mut m = image();
    m.write32(cell_addr(COUNTS, 7), 3);
    m.write32(cell_addr(MEMBERS, 7), ARENA + 8);
    m.write32(ARENA + 8, 0x111);
    m.write32(ARENA + 12, 0x222);
    m.write32(ARENA + 16, 0x333);
    let mut p = ObjectPool::load(&m);
    assert_eq!(p.member_at(7, 0), Some(0x111));
    assert_eq!(p.member_at(7, 2), Some(0x333));
    assert_eq!(p.member_at(7, 3), None);
    assert!(p.set_member(&mut m, 7, 1, 0x444));
    assert_eq!(p.member_at(7, 1), Some(0x444));
    assert_eq!(m.read32(ARENA + 12), 0x444);
    assert_eq!(ObjectPool::load(&m), p);
}

#[test]
fn complete_cell_members_state_is_owned_and_write_through() {
    let mut m = image();
    m.write32(cell_addr(COUNTS, 7), 3);
    m.write32(cell_addr(MEMBERS, 7), ARENA + 8);
    m.write32(ARENA + 4, 0xaaaa);
    m.write32(ARENA + 8, 0x111);
    m.write32(ARENA + 12, 0x222);
    m.write32(ARENA + 16, 0x333);
    m.write32(ARENA + 20, 0xbbbb);
    let mut p = ObjectPool::load(&m);
    assert_eq!(
        p.cell_members_state(7),
        Some(PoolCellMembersState {
            words: vec![0x111, 0x222, 0x333],
        })
    );

    let updated = PoolCellMembersState {
        words: vec![0x444, 0x555, 0x666],
    };
    assert!(p.set_cell_members_state(&mut m, 7, updated.clone()));
    assert_eq!(p.cell_members_state(7), Some(updated));
    for (ordinal, value) in [0x444, 0x555, 0x666].into_iter().enumerate() {
        assert_eq!(p.member_at(7, ordinal), Some(value));
        assert_eq!(m.read32(ARENA + 8 + ordinal as u32 * 4), value);
    }
    assert_eq!(m.read32(ARENA + 4), 0xaaaa);
    assert_eq!(m.read32(ARENA + 20), 0xbbbb);
}

#[test]
fn complete_cell_members_state_rejects_invalid_ranges_without_mutation() {
    let mut m = image();
    m.write32(cell_addr(COUNTS, 7), 3);
    m.write32(cell_addr(MEMBERS, 7), ARENA + 8);
    m.write32(ARENA + 8, 0x111);
    m.write32(ARENA + 12, 0x222);
    m.write32(ARENA + 16, 0x333);
    let mut p = ObjectPool::load(&m);
    let before = p.cell_members_state(7);
    assert!(!p.set_cell_members_state(&mut m, 7, PoolCellMembersState { words: vec![1, 2] },));
    assert_eq!(p.cell_members_state(7), before);
    assert_eq!(m.read32(ARENA + 8), 0x111);
    assert_eq!(m.read32(ARENA + 12), 0x222);
    assert_eq!(m.read32(ARENA + 16), 0x333);
    assert_eq!(p.cell_members_state(CELLS), None);

    p.set_members_ptr(&mut m, 7, ARENA + 1);
    assert_eq!(p.cell_members_state(7), None);
    assert!(!p.set_cell_members_state(
        &mut m,
        7,
        PoolCellMembersState {
            words: vec![9, 8, 7]
        },
    ));
}

#[test]
fn limited_record_owner_covers_requested_prefix() {
    let mut m = image();
    m.write32(SCALARS[S_RECORDS_BASE].0, RECS);
    m.write32(SCALARS[S_CAPACITY].0, 101);
    m.write32(0x007b_4fac, 101);
    m.write32(rec_addr(RECS, 100, 0x20), 0xfeed_beef);
    assert_eq!(ObjectPool::empty_limited(101).records.len(), 101);
    let p = ObjectPool::load_limited(&m, 101);
    assert_eq!(p.owned_records(), 101);
    assert_eq!(p.records.get(100).unwrap()[0x20 / 4], 0xfeed_beef);

    let limited = ObjectPool::load_limited(&m, 16);
    assert_eq!(limited.owned_records(), 16);
}

#[test]
fn member_arena_bounds_and_unaligned_pointer_are_safe() {
    let mut m = image();
    m.write32(SCALARS[S_ARENA_USED].0, ARENA_CAP as u32);
    m.write32(cell_addr(COUNTS, 9), 2);
    m.write32(cell_addr(MEMBERS, 9), ARENA + ((ARENA_CAP - 2) as u32) * 4);
    m.write32(ARENA + ((ARENA_CAP - 2) as u32) * 4, 0xabc);
    m.write32(ARENA + ((ARENA_CAP - 1) as u32) * 4, 0xdef);
    let mut p = ObjectPool::load(&m);
    assert_eq!(p.arena_len, ARENA_CAP);
    assert_eq!(p.member_at(9, 0), Some(0xabc));
    assert_eq!(p.member_at(9, 1), Some(0xdef));
    assert!(!p.set_member(&mut m, 9, 2, 1));
    p.set_members_ptr(&mut m, 9, ARENA + 1);
    assert_eq!(p.member_at(9, 0), None);
    assert!(!p.set_member(&mut m, 9, 0, 1));
}

#[test]
fn member_accessors_reject_cells_outside_owned_tables_without_mutation() {
    let mut m = image();
    let mut p = ObjectPool::load(&m);
    let before_pool = p.clone();
    let before_memory = m.clone();

    for cell in [CELLS, usize::MAX] {
        assert_eq!(p.member_at(cell, 0), None);
        assert!(!p.set_member(&mut m, cell, 0, 0x1234));
    }

    assert_eq!(p, before_pool);
    assert_eq!(m, before_memory);
    assert_eq!(p.diff(&m), None);
}

#[test]
fn add_member_grows_and_overflows() {
    let mut m = Image(BTreeMap::new());
    m.write32(SCALARS[S_ARENA_BASE].0, ARENA);
    let mut p = ObjectPool::load(&m);
    // empty cell: first add grows to cap 8, arena_used 8
    assert!(p.add_member(&mut m, 2, 0xaa));
    assert_eq!((p.count(2), p.cap(2), p.scalar(S_ARENA_USED)), (1, 8, 8));
    assert_eq!(p.members_ptr(2), ARENA);
    for k in 1..8 {
        assert!(p.add_member(&mut m, 2, 0xaa + k));
    }
    assert_eq!((p.count(2), p.cap(2), p.scalar(S_ARENA_USED)), (8, 8, 8));
    // full: grows to 16, existing members copied to the new array
    assert!(p.add_member(&mut m, 2, 0xff));
    assert_eq!((p.count(2), p.cap(2), p.scalar(S_ARENA_USED)), (9, 16, 24));
    assert_eq!(p.members_ptr(2), ARENA + 8 * 4);
    assert_eq!(m.read32(p.members_ptr(2) + 4 * 7), 0xaa + 7);
    assert_eq!(m.read32(p.members_ptr(2) + 4 * 8), 0xff);
    assert_eq!(p.diff(&m), None);
    // overflow: capacity word is updated, nothing else, add refused
    p.set_scalar(&mut m, S_ARENA_USED, ARENA_LIMIT - 4);
    p.set_count(&mut m, 4, 16);
    p.set_cap(&mut m, 4, 16);
    let before = (p.count(4), p.members_ptr(4), p.scalar(S_ARENA_USED));
    assert!(!p.add_member(&mut m, 4, 1));
    assert_eq!(p.cap(4), 24);
    assert_eq!(
        (p.count(4), p.members_ptr(4), p.scalar(S_ARENA_USED)),
        before
    );
    assert_eq!(p.diff(&m), None);
}

#[test]
fn add_member_rejects_cells_outside_owned_tables_without_mutation() {
    let mut m = image();
    let mut p = ObjectPool::load(&m);
    let before_pool = p.clone();
    let before_memory = m.clone();

    assert!(!p.add_member(&mut m, CELLS, 0x1234));
    assert!(!p.add_member(&mut m, usize::MAX, 0x5678));

    assert_eq!(p, before_pool);
    assert_eq!(m, before_memory);
    assert_eq!(p.diff(&m), None);
}

#[test]
fn push_node_and_clear_list() {
    let mut m = image();
    let mut p = ObjectPool::load(&m);
    let new = 0x43d0_0300;
    p.push_node(&mut m, new);
    assert_eq!(p.list_len(), 4);
    assert_eq!(p.scalar(S_LIST_HEAD), new);
    assert_eq!(m.read32(new), NODES[0]);
    assert_eq!(p.chain_len(), 4);
    assert_eq!(p.diff(&m), None);
    p.clear_list(&mut m);
    assert_eq!((p.list_len(), p.chain_len()), (0, 0));
    assert_eq!(m.read32(SCALARS[S_LIST_HEAD].0), 0);
    assert_eq!(p.diff(&m), None);
    p.clear_tables(&mut m);
    assert_eq!(m.read32(cell_addr(CAPS, 0xd5)), 0);
    assert_ne!(
        m.read32(cell_addr(CAPS, 0xd6)),
        0,
        "gap cells are not cleared"
    );
    assert_eq!(p.diff(&m), None);
}

/// Owned extents as (start, end) guest ranges, record bytes excluded.
fn extents() -> Vec<(u32, u32)> {
    let mut v = vec![(HEADER, HEADER + 4 * HEADER_WORDS as u32)];
    for t in 0..TABLE_COUNT {
        v.push((cell_addr(t, 0), cell_addr(t, CELLS)));
    }
    v
}

#[test]
fn drift_against_t2057() {
    let mut pool_fields = BTreeSet::new();
    let mut funcs = BTreeSet::new();
    let mut object_fields = BTreeSet::new();
    for l in CSV.lines().skip(1) {
        let c: Vec<&str> = l.split(',').collect();
        let off = u32::from_str_radix(c[3].trim_start_matches("0x"), 16).unwrap();
        match c[2] {
            "objpool" => {
                pool_fields.insert(off);
                funcs.insert(c[1].to_owned());
            }
            "object" => {
                object_fields.insert(off);
            }
            _ => {}
        }
    }
    assert_eq!(pool_fields.len(), 495);
    assert_eq!(funcs.len(), 10, "9 lifted writers plus the memcpy helper");
    let ext = extents();
    for off in &pool_fields {
        assert_eq!(off % 4, 0);
        let a = HEADER + off;
        assert!(
            ext.iter().any(|(s, e)| a >= *s && a < *e),
            "written pool dword {a:#x} is outside the owned extents"
        );
    }
    // The object window of the capture is 0x1584 bytes, 7.4 record strides: fields past 0x2E8 are
    // writes to the neighbouring records (INFERRED), all dwords stay aligned inside the window.
    assert_eq!(object_fields.len(), 879);
    assert!(object_fields.iter().all(|o| o % 4 == 0 && *o < 0x1584));
    assert!(object_fields.iter().any(|o| *o >= RECORD_SIZE));
    let in_record = object_fields.iter().filter(|o| **o < RECORD_SIZE).count();
    assert_eq!(
        in_record, 62,
        "distinct written dwords inside one record stride"
    );
}

#[test]
fn mutation_each_owned_field_is_located() {
    let m = image();
    let p = ObjectPool::load_limited(&m, 101);
    let mut checks = 0usize;
    for i in 0..HEADER_WORDS {
        let mut q = p.clone();
        q.header[i] ^= 1;
        assert_eq!(q.diff(&m), Some(PoolDiff::Header(i)));
        checks += 1;
    }
    for t in 0..TABLE_COUNT {
        for c in 0..CELLS {
            let mut q = p.clone();
            q.tables[t][c] ^= 1;
            assert_eq!(q.diff(&m), Some(PoolDiff::Cell(t, c)));
            checks += 1;
        }
    }
    for i in 0..SCALARS.len() {
        let mut q = p.clone();
        q.scalars[i] ^= 4; // keep the base and the used count in the same page-like range
        // changing base, used count or the head changes where the rest is read: only require a diff
        assert!(q.diff(&m).is_some(), "scalar {i}");
        checks += 1;
    }
    for i in 0..p.chain_len() {
        let mut q = p.clone();
        q.chain_next[i] ^= 1;
        assert_eq!(q.diff(&m), Some(PoolDiff::Chain(i)));
        checks += 1;
    }
    for r in 0..p.owned_records() {
        for j in 0..RECORD_WORDS {
            let mut q = p.clone();
            q.records[r][j] ^= 1;
            assert_eq!(q.diff(&m), Some(PoolDiff::Record(r, j as u32 * 4)));
            checks += 1;
        }
    }
    assert!(checks > 1000, "{checks}");
}

#[test]
fn typed_record_state_round_trips_the_complete_owned_record() {
    let mut m = image();
    let mut pool = ObjectPool::load(&m);
    let before_zero = pool.record_state(0).unwrap();
    let before_two = pool.record_state(2).unwrap();
    let updated = PoolRecordState {
        words: core::array::from_fn(|word| 0xd100_0000 | word as u32),
    };

    assert!(pool.set_record_state(&mut m, 1, updated.clone()));
    assert_eq!(pool.record_state(1), Some(updated.clone()));
    assert_eq!(pool.record_state(0), Some(before_zero));
    assert_eq!(pool.record_state(2), Some(before_two));
    for (word, value) in updated.words.into_iter().enumerate() {
        assert_eq!(m.read32(rec_addr(RECS, 1, word as u32 * 4)), value);
    }
    assert_eq!(pool.diff(&m), None);
}

#[test]
fn typed_record_state_rejects_records_outside_the_owned_prefix() {
    let mut m = image();
    let mut pool = ObjectPool::load(&m);
    let owned_before = pool.clone();
    let guest_before = m.clone();
    let invalid = PoolRecordState {
        words: [0xfeed_beef; RECORD_WORDS],
    };

    assert_eq!(pool.record_state(pool.owned_records()), None);
    assert!(!pool.set_record_state(&mut m, pool.owned_records(), invalid));
    assert_eq!(pool, owned_before);
    assert!(m == guest_before);
}

#[test]
fn opaque_extension_address_writer_is_bounded_and_write_through() {
    let mut m = image();
    let mut pool = ObjectPool::load(&m);
    let offset = RECORD_EXTENSION_ADDRESS_OFFSET;
    let record = 1;
    let base = rec_addr(RECS, record, 0);
    let before_zero = pool.record_state(0).unwrap();
    let before_two = pool.record_state(2).unwrap();
    let before_left = m.read32(base + offset - 4);
    let before_right = m.read32(base + offset + 4);
    let value = PoolRecordExtensionAddress(0x1234_5678);

    assert_eq!(
        pool.record_extension_address(record),
        Some(PoolRecordExtensionAddress(m.read32(base + offset)))
    );
    assert!(pool.set_record_extension_address(&mut m, record, value));
    assert_eq!(pool.record_extension_address(record), Some(value));
    assert_eq!(
        pool.owned_record(record).unwrap()[(offset / 4) as usize],
        value.0
    );
    assert_eq!(m.read32(base + offset), value.0);
    assert_eq!(m.read32(base + offset - 4), before_left);
    assert_eq!(m.read32(base + offset + 4), before_right);
    assert_eq!(pool.record_state(0), Some(before_zero));
    assert_eq!(pool.record_state(2), Some(before_two));
    assert_eq!(pool.diff(&m), None);

    let owned_before = pool.clone();
    let guest_before = m.clone();
    let invalid = pool.owned_records();
    assert_eq!(pool.record_extension_address(invalid), None);
    assert!(!pool.set_record_extension_address(
        &mut m,
        invalid,
        PoolRecordExtensionAddress(0xfeed_beef),
    ));
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);
}

#[test]
fn character_retirement_unlinks_before_clearing_owned_object_extension() {
    let mut m = image();
    let record = 1;
    let object = rec_addr(RECS, record, 0);
    let extension = 0x4400_0000;
    let moved = 0x4500_0000;
    m.write32(object + RECORD_EXTENSION_ADDRESS_OFFSET, extension);
    m.write32(MEMBER_COUNT, 3);
    m.write32(CHARACTER_TABLE, 0x4100_0000);
    m.write32(CHARACTER_TABLE + 4, extension);
    m.write32(CHARACTER_TABLE + 8, moved);
    for word in 0..tsfp_state::character::BODY_WORDS as u32 {
        m.write32(extension + word * 4, 0x7000_0000 | word);
        m.write32(moved + word * 4, 0x8000_0000 | word);
    }
    let mut objects = ObjectPool::load(&m);
    let mut characters = CharacterSet::load(&m);

    assert_eq!(
        characters.remove_object_extension_and_clear_pointer(&mut m, &mut objects, record),
        Some(CharacterRetirementMembership::Removed { slot: 1 })
    );
    assert_eq!(characters.member_count, 2);
    assert_eq!(m.read32(MEMBER_COUNT), 2);
    assert_eq!(characters.ptrs[1], moved);
    assert_eq!(m.read32(CHARACTER_TABLE + 4), moved);
    assert_eq!(m.read32(CHARACTER_TABLE + 8), moved); // inactive tail remains untouched
    assert_eq!(characters.body_word(1, 0), 0x8000_0000);
    assert_eq!(
        objects.record_extension_address(record),
        Some(PoolRecordExtensionAddress(0))
    );
    assert_eq!(m.read32(object + RECORD_EXTENSION_ADDRESS_OFFSET), 0);
    assert_eq!(characters.diff(&m), None);
    assert_eq!(objects.diff(&m), None);
}

#[test]
fn character_retirement_preserves_unlisted_or_invalid_extension_records() {
    let mut m = image();
    let mut objects = ObjectPool::load(&m);
    let record = 1;
    let extension = objects.record_extension_address(record).unwrap();
    m.write32(MEMBER_COUNT, 1);
    m.write32(CHARACTER_TABLE, 0x4100_0000);
    let mut characters = CharacterSet::load(&m);
    let chars_before = characters.clone();
    let objects_before = objects.clone();
    let memory_before = m.clone();

    assert_eq!(
        characters.remove_object_extension_and_clear_pointer(&mut m, &mut objects, record),
        Some(CharacterRetirementMembership::NotMember)
    );
    assert_eq!(characters, chars_before);
    assert_eq!(objects, objects_before);
    assert!(m == memory_before);
    assert_eq!(objects.record_extension_address(record), Some(extension));

    let invalid_record = objects.owned_records();
    assert_eq!(
        characters.remove_object_extension_and_clear_pointer(&mut m, &mut objects, invalid_record),
        None
    );
    assert_eq!(characters, chars_before);
    assert_eq!(objects, objects_before);
    assert!(m == memory_before);
}

#[test]
fn character_retirement_removes_first_duplicate_extension_and_keeps_alias_body() {
    let mut m = image();
    let record = 1;
    let object = rec_addr(RECS, record, 0);
    let extension = 0x4400_0000;
    let other = 0x4500_0000;
    m.write32(object + RECORD_EXTENSION_ADDRESS_OFFSET, extension);
    m.write32(MEMBER_COUNT, 3);
    m.write32(CHARACTER_TABLE, extension);
    m.write32(CHARACTER_TABLE + 4, other);
    m.write32(CHARACTER_TABLE + 8, extension);
    m.write32(extension, 0x1234_5678);
    m.write32(other, 0x8765_4321);
    let mut objects = ObjectPool::load(&m);
    let mut characters = CharacterSet::load(&m);

    assert_eq!(
        characters.remove_object_extension_and_clear_pointer(&mut m, &mut objects, record),
        Some(CharacterRetirementMembership::Removed { slot: 0 })
    );
    assert_eq!(characters.member_count, 2);
    assert_eq!(characters.ptrs[0], extension); // the duplicate was swap-moved from the tail
    assert_eq!(characters.ptrs[1], other);
    assert_eq!(characters.body_word(0, 0), 0x1234_5678);
    assert_eq!(m.read32(CHARACTER_TABLE), extension);
    assert_eq!(m.read32(CHARACTER_TABLE + 4), other);
    assert_eq!(m.read32(CHARACTER_TABLE + 8), extension); // inactive tail is unchanged
    assert_eq!(
        objects.record_extension_address(record),
        Some(PoolRecordExtensionAddress(0))
    );
    assert_eq!(characters.diff(&m), None);
    assert_eq!(objects.diff(&m), None);
}

#[test]
fn measured_record_position_round_trips_and_is_bounded() {
    let mut m = image();
    let mut pool = ObjectPool::load(&m);
    let record = 1;
    let base = rec_addr(RECS, record, 0);
    let before_zero = pool.record_state(0).unwrap();
    let before_two = pool.record_state(2).unwrap();
    let before_left = m.read32(base + RECORD_POSITION_OFFSET - 4);
    let before_right = m.read32(base + RECORD_POSITION_OFFSET + 12);
    let position = PoolRecordPosition {
        x: f32::from_bits(0x4123_4567),
        y: f32::from_bits(0x8000_0000),
        z: f32::from_bits(0x7fc1_2345),
    };

    assert!(pool.set_record_position(&mut m, record, position));
    let stored = pool.record_position(record).unwrap();
    assert_eq!(stored.x.to_bits(), position.x.to_bits());
    assert_eq!(stored.y.to_bits(), position.y.to_bits());
    assert_eq!(stored.z.to_bits(), position.z.to_bits());
    for (component, value) in [position.x, position.y, position.z].into_iter().enumerate() {
        let offset = RECORD_POSITION_OFFSET + component as u32 * 4;
        assert_eq!(m.read32(base + offset), value.to_bits());
        assert_eq!(
            pool.owned_record(record).unwrap()[(offset / 4) as usize],
            value.to_bits()
        );
    }
    assert_eq!(m.read32(base + RECORD_POSITION_OFFSET - 4), before_left);
    assert_eq!(m.read32(base + RECORD_POSITION_OFFSET + 12), before_right);
    assert_eq!(pool.record_state(0), Some(before_zero));
    assert_eq!(pool.record_state(2), Some(before_two));
    assert_eq!(pool.diff(&m), None);

    let owned_before = pool.clone();
    let guest_before = m.clone();
    let invalid = pool.owned_records();
    assert_eq!(pool.record_position(invalid), None);
    assert!(!pool.set_record_position(&mut m, invalid, PoolRecordPosition::default()));
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);
}

#[test]
fn initializer_3b3_adjustment_updates_only_object_y_and_is_bounded() {
    let mut m = image();
    let mut pool = ObjectPool::load(&m);
    let record = 1;
    let base = rec_addr(RECS, record, 0);
    let position = PoolRecordPosition {
        x: f32::from_bits(0x7fc1_2345),
        y: 3.25,
        z: f32::from_bits(0x8000_0000),
    };
    let original = PoolRecordOriginalPosition {
        components: [1.0, 2.0, 3.0],
    };
    assert!(pool.set_record_model_key(&mut m, record, 0x03b3));
    assert!(pool.set_record_position(&mut m, record, position));
    assert!(pool.set_record_original_position(&mut m, record, original));
    let before_zero = pool.record_state(0).unwrap();
    let before_two = pool.record_state(2).unwrap();
    let before_left = m.read32(base + RECORD_POSITION_OFFSET - 4);
    let before_right = m.read32(base + RECORD_POSITION_OFFSET + 12);

    let adjusted = pool
        .apply_initializer_3b3_y_adjustment(&mut m, record, 0.75)
        .unwrap();
    assert_eq!(adjusted.x.to_bits(), position.x.to_bits());
    assert_eq!(adjusted.y.to_bits(), 2.5f32.to_bits());
    assert_eq!(adjusted.z.to_bits(), position.z.to_bits());
    assert_eq!(
        pool.record_position(record).unwrap().y.to_bits(),
        2.5f32.to_bits()
    );
    assert_eq!(
        m.read32(base + RECORD_POSITION_OFFSET + 4),
        2.5f32.to_bits()
    );
    assert_eq!(
        pool.owned_record(record).unwrap()[((RECORD_POSITION_OFFSET + 4) / 4) as usize],
        2.5f32.to_bits()
    );
    assert_eq!(pool.record_original_position(record), Some(original));
    assert_eq!(
        m.read32(base + RECORD_POSITION_OFFSET),
        position.x.to_bits()
    );
    assert_eq!(
        m.read32(base + RECORD_POSITION_OFFSET + 8),
        position.z.to_bits()
    );
    assert_eq!(m.read32(base + RECORD_POSITION_OFFSET - 4), before_left);
    assert_eq!(
        m.read32(base + RECORD_POSITION_OFFSET + 8 + 4),
        before_right
    );
    assert_eq!(pool.record_state(0), Some(before_zero));
    assert_eq!(pool.record_state(2), Some(before_two));
    assert_eq!(pool.diff(&m), None);

    assert!(pool.set_record_model_key(&mut m, record, 0x0482));
    let owned_before = pool.clone();
    let guest_before = m.clone();
    assert_eq!(
        pool.apply_initializer_3b3_y_adjustment(&mut m, record, 99.0),
        None
    );
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);

    let invalid = pool.owned_records();
    assert_eq!(
        pool.apply_initializer_3b3_y_adjustment(&mut m, invalid, 99.0),
        None
    );
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);
}

#[test]
fn initializer_original_position_round_trips_and_is_bounded() {
    let mut m = image();
    let mut pool = ObjectPool::load(&m);
    let record = 1;
    let base = rec_addr(RECS, record, 0);
    let before_zero = pool.record_state(0).unwrap();
    let before_two = pool.record_state(2).unwrap();
    let before_left = m.read32(base + RECORD_ORIGINAL_POSITION_OFFSETS[0] - 4);
    let before_right = m.read32(base + RECORD_ORIGINAL_POSITION_OFFSETS[2] + 4);
    let position = PoolRecordOriginalPosition {
        components: [
            f32::from_bits(0x4123_4567),
            f32::from_bits(0x8000_0000),
            f32::from_bits(0x7fc1_2345),
        ],
    };

    assert!(pool.set_record_original_position(&mut m, record, position));
    let stored = pool.record_original_position(record).unwrap();
    for (component, (offset, value)) in RECORD_ORIGINAL_POSITION_OFFSETS
        .into_iter()
        .zip(position.components)
        .enumerate()
    {
        assert_eq!(stored.components[component].to_bits(), value.to_bits());
        assert_eq!(m.read32(base + offset), value.to_bits());
        assert_eq!(
            pool.owned_record(record).unwrap()[(offset / 4) as usize],
            value.to_bits()
        );
    }
    assert_eq!(
        m.read32(base + RECORD_ORIGINAL_POSITION_OFFSETS[0] - 4),
        before_left
    );
    assert_eq!(
        m.read32(base + RECORD_ORIGINAL_POSITION_OFFSETS[2] + 4),
        before_right
    );
    assert_eq!(pool.record_state(0), Some(before_zero));
    assert_eq!(pool.record_state(2), Some(before_two));
    assert_eq!(pool.diff(&m), None);

    let owned_before = pool.clone();
    let guest_before = m.clone();
    let invalid = pool.owned_records();
    assert_eq!(pool.record_original_position(invalid), None);
    assert!(!pool.set_record_original_position(
        &mut m,
        invalid,
        PoolRecordOriginalPosition::default()
    ));
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);
}

#[test]
fn measured_initializer_position_bundle_updates_both_bounded_triples() {
    let mut m = image();
    let mut pool = ObjectPool::load(&m);
    let record = 1;
    let base = rec_addr(RECS, record, 0);
    let before_zero = pool.record_state(0).unwrap();
    let before_two = pool.record_state(2).unwrap();
    let neighbors = [
        m.read32(base + RECORD_POSITION_OFFSET - 4),
        m.read32(base + RECORD_POSITION_OFFSET + 12),
        m.read32(base + RECORD_ORIGINAL_POSITION_OFFSETS[0] - 4),
        m.read32(base + RECORD_ORIGINAL_POSITION_OFFSETS[2] + 4),
    ];
    let fields = PoolRecordInitializerPosition {
        current: PoolRecordPosition {
            x: f32::from_bits(0x4123_4567),
            y: f32::from_bits(0x8000_0000),
            z: f32::from_bits(0x7fc1_2345),
        },
        original: PoolRecordOriginalPosition {
            components: [
                f32::from_bits(0xc123_4567),
                f32::from_bits(0x0000_0000),
                f32::from_bits(0x7fc5_4321),
            ],
        },
    };

    assert!(pool.set_record_initializer_position(&mut m, record, fields));
    let current = pool.record_position(record).unwrap();
    for (actual, expected) in [current.x, current.y, current.z].into_iter().zip([
        fields.current.x,
        fields.current.y,
        fields.current.z,
    ]) {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
    for (actual, expected) in pool
        .record_original_position(record)
        .unwrap()
        .components
        .into_iter()
        .zip(fields.original.components)
    {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
    for (offset, value) in [
        (RECORD_POSITION_OFFSET, fields.current.x),
        (RECORD_POSITION_OFFSET + 4, fields.current.y),
        (RECORD_POSITION_OFFSET + 8, fields.current.z),
    ] {
        assert_eq!(
            pool.owned_record(record).unwrap()[(offset / 4) as usize],
            value.to_bits()
        );
        assert_eq!(m.read32(base + offset), value.to_bits());
    }
    for (offset, value) in RECORD_ORIGINAL_POSITION_OFFSETS
        .into_iter()
        .zip(fields.original.components)
    {
        assert_eq!(
            pool.owned_record(record).unwrap()[(offset / 4) as usize],
            value.to_bits()
        );
        assert_eq!(m.read32(base + offset), value.to_bits());
    }
    assert_eq!(
        [
            m.read32(base + RECORD_POSITION_OFFSET - 4),
            m.read32(base + RECORD_POSITION_OFFSET + 12),
            m.read32(base + RECORD_ORIGINAL_POSITION_OFFSETS[0] - 4),
            m.read32(base + RECORD_ORIGINAL_POSITION_OFFSETS[2] + 4),
        ],
        neighbors
    );
    assert_eq!(pool.record_state(0), Some(before_zero));
    assert_eq!(pool.record_state(2), Some(before_two));
    assert_eq!(pool.diff(&m), None);

    let owned_before = pool.clone();
    let guest_before = m.clone();
    let invalid = pool.owned_records();
    assert!(!pool.set_record_initializer_position(
        &mut m,
        invalid,
        PoolRecordInitializerPosition::default()
    ));
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);
}

#[test]
fn measured_3b3_initializer_position_bundle_snapshots_then_adjusts() {
    let mut m = image();
    let mut pool = ObjectPool::load(&m);
    let record = 1;
    let base = rec_addr(RECS, record, 0);
    assert!(pool.set_record_model_key(&mut m, record, 0x03b3));

    let before_zero = pool.record_state(0).unwrap();
    let before_two = pool.record_state(2).unwrap();
    let neighbors = [
        m.read32(base + RECORD_POSITION_OFFSET - 4),
        m.read32(base + RECORD_POSITION_OFFSET + 12),
        m.read32(base + RECORD_ORIGINAL_POSITION_OFFSETS[0] - 4),
        m.read32(base + RECORD_ORIGINAL_POSITION_OFFSETS[2] + 4),
    ];
    let input = PoolRecordInitializerPosition {
        current: PoolRecordPosition {
            x: 11.25,
            y: 20.5,
            z: f32::from_bits(0x8000_0000),
        },
        original: PoolRecordOriginalPosition {
            components: [
                f32::from_bits(0x7fc1_2345),
                -8.0,
                f32::from_bits(0x4123_4567),
            ],
        },
    };
    let mut expected = input;
    expected.current.y -= 4.75;

    let applied = pool
        .apply_initializer_3b3_position_bundle(&mut m, record, input, 4.75)
        .unwrap();
    assert_eq!(applied.current.x.to_bits(), expected.current.x.to_bits());
    assert_eq!(applied.current.y.to_bits(), expected.current.y.to_bits());
    assert_eq!(applied.current.z.to_bits(), expected.current.z.to_bits());
    for (actual, expected) in pool
        .record_original_position(record)
        .unwrap()
        .components
        .into_iter()
        .zip(input.original.components)
    {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
    for (offset, value) in [
        (RECORD_POSITION_OFFSET, expected.current.x),
        (RECORD_POSITION_OFFSET + 4, expected.current.y),
        (RECORD_POSITION_OFFSET + 8, expected.current.z),
    ] {
        assert_eq!(m.read32(base + offset), value.to_bits());
        assert_eq!(
            pool.owned_record(record).unwrap()[(offset / 4) as usize],
            value.to_bits()
        );
    }
    for (offset, value) in RECORD_ORIGINAL_POSITION_OFFSETS
        .into_iter()
        .zip(input.original.components)
    {
        assert_eq!(m.read32(base + offset), value.to_bits());
        assert_eq!(
            pool.owned_record(record).unwrap()[(offset / 4) as usize],
            value.to_bits()
        );
    }
    assert_eq!(
        [
            m.read32(base + RECORD_POSITION_OFFSET - 4),
            m.read32(base + RECORD_POSITION_OFFSET + 12),
            m.read32(base + RECORD_ORIGINAL_POSITION_OFFSETS[0] - 4),
            m.read32(base + RECORD_ORIGINAL_POSITION_OFFSETS[2] + 4),
        ],
        neighbors
    );
    assert_eq!(pool.record_state(0), Some(before_zero));
    assert_eq!(pool.record_state(2), Some(before_two));
    assert_eq!(pool.diff(&m), None);

    assert!(pool.set_record_model_key(&mut m, record, 0x0482));
    let owned_before = pool.clone();
    let guest_before = m.clone();
    assert_eq!(
        pool.apply_initializer_3b3_position_bundle(&mut m, record, input, 99.0),
        None
    );
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);

    let invalid = pool.owned_records();
    assert_eq!(
        pool.apply_initializer_3b3_position_bundle(&mut m, invalid, input, 99.0),
        None
    );
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);
}

#[test]
fn measured_3b3_initializer_object_fields_compose_flags_and_positions() {
    let mut m = image();
    let mut pool = ObjectPool::load(&m);
    let record = 1;
    let base = rec_addr(RECS, record, 0);
    let initial_flags = 0xa500_0001;
    assert!(pool.set_record_model_key(&mut m, record, 0x03b3));
    assert!(pool.set_record_object_flags(&mut m, record, PoolRecordObjectFlags(initial_flags),));
    let input = PoolRecordInitializerPosition {
        current: PoolRecordPosition {
            x: 6.5,
            y: -2.0,
            z: 14.25,
        },
        original: PoolRecordOriginalPosition {
            components: [1.25, -3.5, 8.0],
        },
    };

    let applied = pool
        .apply_initializer_3b3_object_fields(&mut m, record, input, 0.75)
        .unwrap();
    assert_eq!(
        applied.object_flags,
        PoolRecordObjectFlags(initial_flags | INITIALIZER_OBJECT_FLAGS)
    );
    assert_eq!(
        applied.position.current.x.to_bits(),
        input.current.x.to_bits()
    );
    assert_eq!(applied.position.current.y.to_bits(), (-2.75f32).to_bits());
    assert_eq!(
        applied.position.current.z.to_bits(),
        input.current.z.to_bits()
    );
    for (actual, expected) in applied
        .position
        .original
        .components
        .into_iter()
        .zip(input.original.components)
    {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
    assert_eq!(
        pool.record_object_flags(record),
        Some(PoolRecordObjectFlags(
            initial_flags | INITIALIZER_OBJECT_FLAGS
        ))
    );
    assert_eq!(
        m.read32(base + RECORD_OBJECT_FLAGS_OFFSET),
        initial_flags | INITIALIZER_OBJECT_FLAGS
    );
    let current = pool.record_position(record).unwrap();
    assert_eq!(current.x.to_bits(), input.current.x.to_bits());
    assert_eq!(current.y.to_bits(), (-2.75f32).to_bits());
    assert_eq!(current.z.to_bits(), input.current.z.to_bits());
    for (actual, expected) in pool
        .record_original_position(record)
        .unwrap()
        .components
        .into_iter()
        .zip(input.original.components)
    {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
    assert_eq!(pool.diff(&m), None);

    assert!(pool.set_record_model_key(&mut m, record, 0x0482));
    let owned_before = pool.clone();
    let guest_before = m.clone();
    assert!(
        pool.apply_initializer_3b3_object_fields(&mut m, record, input, 99.0)
            .is_none()
    );
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);

    let invalid = pool.owned_records();
    assert!(
        pool.apply_initializer_3b3_object_fields(&mut m, invalid, input, 99.0)
            .is_none()
    );
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);
}

#[test]
fn measured_482_initializer_object_fields_copy_unadjusted_positions() {
    let mut m = image();
    let mut pool = ObjectPool::load(&m);
    let record = 1;
    let base = rec_addr(RECS, record, 0);
    let initial_flags = 0xa500_0001;
    let extension = 0x4567_89ab;
    assert!(pool.set_record_model_key(&mut m, record, 0x0482));
    assert!(pool.set_record_object_flags(&mut m, record, PoolRecordObjectFlags(initial_flags)));
    assert!(pool.set_record_extension_address(
        &mut m,
        record,
        PoolRecordExtensionAddress(extension),
    ));
    let input = PoolRecordInitializerPosition {
        current: PoolRecordPosition {
            x: 6.5,
            y: -2.0,
            z: 14.25,
        },
        original: PoolRecordOriginalPosition {
            components: [1.25, -3.5, 8.0],
        },
    };

    let applied = pool
        .apply_initializer_482_object_fields(&mut m, record, input)
        .unwrap();
    assert_eq!(
        applied.object_flags,
        PoolRecordObjectFlags(initial_flags | INITIALIZER_OBJECT_FLAGS)
    );
    assert_eq!(
        applied.position.current.x.to_bits(),
        input.current.x.to_bits()
    );
    assert_eq!(
        applied.position.current.y.to_bits(),
        input.current.y.to_bits()
    );
    assert_eq!(
        applied.position.current.z.to_bits(),
        input.current.z.to_bits()
    );
    for (actual, expected) in applied
        .position
        .original
        .components
        .into_iter()
        .zip(input.original.components)
    {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
    assert_eq!(pool.record_object_flags(record), Some(applied.object_flags));
    assert_eq!(
        m.read32(base + RECORD_OBJECT_FLAGS_OFFSET),
        initial_flags | INITIALIZER_OBJECT_FLAGS
    );
    assert_eq!(m.read32(base + RECORD_EXTENSION_ADDRESS_OFFSET), extension);
    assert_eq!(pool.diff(&m), None);

    assert!(pool.set_record_model_key(&mut m, record, 0x03b3));
    let owned_before = pool.clone();
    let guest_before = m.clone();
    assert!(
        pool.apply_initializer_482_object_fields(&mut m, record, input)
            .is_none()
    );
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);

    let invalid = pool.owned_records();
    assert!(
        pool.apply_initializer_482_object_fields(&mut m, invalid, input)
            .is_none()
    );
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);
}

#[test]
fn measured_553_initializer_object_fields_copy_unadjusted_positions() {
    let mut m = image();
    let mut pool = ObjectPool::load(&m);
    let record = 1;
    let base = rec_addr(RECS, record, 0);
    let initial_flags = 0xa500_0001;
    let extension = 0x4567_89ab;
    assert!(pool.set_record_model_key(&mut m, record, 0x0553));
    assert!(pool.set_record_object_flags(&mut m, record, PoolRecordObjectFlags(initial_flags)));
    assert!(pool.set_record_extension_address(
        &mut m,
        record,
        PoolRecordExtensionAddress(extension),
    ));
    let input = PoolRecordInitializerPosition {
        current: PoolRecordPosition {
            x: 6.5,
            y: -2.0,
            z: 14.25,
        },
        original: PoolRecordOriginalPosition {
            components: [1.25, -3.5, 8.0],
        },
    };

    let applied = pool
        .apply_initializer_553_object_fields(&mut m, record, input)
        .unwrap();
    assert_eq!(
        applied.object_flags,
        PoolRecordObjectFlags(
            initial_flags | INITIALIZER_OBJECT_FLAGS | MODEL_0553_INITIALIZER_OBJECT_FLAGS
        )
    );
    assert_eq!(
        applied.position.current.x.to_bits(),
        input.current.x.to_bits()
    );
    assert_eq!(
        applied.position.current.y.to_bits(),
        input.current.y.to_bits()
    );
    assert_eq!(
        applied.position.current.z.to_bits(),
        input.current.z.to_bits()
    );
    for (actual, expected) in applied
        .position
        .original
        .components
        .into_iter()
        .zip(input.original.components)
    {
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
    assert_eq!(pool.record_object_flags(record), Some(applied.object_flags));
    assert_eq!(
        m.read32(base + RECORD_OBJECT_FLAGS_OFFSET),
        initial_flags | INITIALIZER_OBJECT_FLAGS | MODEL_0553_INITIALIZER_OBJECT_FLAGS
    );
    assert_eq!(m.read32(base + RECORD_EXTENSION_ADDRESS_OFFSET), extension);
    assert_eq!(pool.diff(&m), None);

    assert!(pool.set_record_model_key(&mut m, record, 0x03b3));
    let owned_before = pool.clone();
    let guest_before = m.clone();
    assert!(
        pool.apply_initializer_553_object_fields(&mut m, record, input)
            .is_none()
    );
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);

    let invalid = pool.owned_records();
    assert!(
        pool.apply_initializer_553_object_fields(&mut m, invalid, input)
            .is_none()
    );
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);
}

#[test]
fn measured_record_fields_write_through_as_bounded_typed_bundle() {
    let mut m = image();
    let mut pool = ObjectPool::load(&m);
    let record = 1;
    let base = rec_addr(RECS, record, 0);
    let before = pool.record_state(record).unwrap();
    let neighbor_before = pool.record_state(record + 1).unwrap();
    let fields = PoolRecordMeasuredFields {
        object_flags: PoolRecordObjectFlags(0x8123_4567),
        position: PoolRecordInitializerPosition {
            current: PoolRecordPosition {
                x: -0.0,
                y: 2.5,
                z: -8.25,
            },
            original: PoolRecordOriginalPosition {
                components: [1.0, -2.0, 4.5],
            },
        },
        rotation_inputs: PoolRecordRotationInputs {
            components: [12.0, -3.5, 0.25],
        },
        actor_parameters: PoolRecordActorParameters {
            at_1cc: -0.0,
            at_1d8: 6.75,
        },
    };

    assert!(pool.set_record_measured_fields(&mut m, record, fields));

    let mut expected = before.words;
    expected[(RECORD_OBJECT_FLAGS_OFFSET / 4) as usize] = fields.object_flags.0;
    for (offset, value) in RECORD_ORIGINAL_POSITION_OFFSETS
        .into_iter()
        .zip(fields.position.original.components)
    {
        expected[(offset / 4) as usize] = value.to_bits();
    }
    for (component, value) in [
        fields.position.current.x,
        fields.position.current.y,
        fields.position.current.z,
    ]
    .into_iter()
    .enumerate()
    {
        expected[((RECORD_POSITION_OFFSET + component as u32 * 4) / 4) as usize] = value.to_bits();
    }
    for (offset, value) in RECORD_ROTATION_INPUT_OFFSETS
        .into_iter()
        .zip(fields.rotation_inputs.components)
    {
        expected[(offset / 4) as usize] = value.to_bits();
    }
    for (offset, value) in RECORD_ACTOR_PARAMETER_OFFSETS.into_iter().zip([
        fields.actor_parameters.at_1cc,
        fields.actor_parameters.at_1d8,
    ]) {
        expected[(offset / 4) as usize] = value.to_bits();
    }
    assert_eq!(pool.record_state(record).unwrap().words, expected);
    assert_eq!(pool.record_object_flags(record), Some(fields.object_flags));
    assert_eq!(pool.record_position(record), Some(fields.position.current));
    assert_eq!(
        pool.record_rotation_inputs(record),
        Some(fields.rotation_inputs)
    );
    assert_eq!(
        pool.record_actor_parameters(record),
        Some(fields.actor_parameters)
    );
    assert_eq!(pool.record_state(record + 1).unwrap(), neighbor_before);
    assert_eq!(pool.diff(&m), None);

    let mut written = vec![
        (RECORD_OBJECT_FLAGS_OFFSET, fields.object_flags.0),
        (RECORD_POSITION_OFFSET, fields.position.current.x.to_bits()),
        (
            RECORD_POSITION_OFFSET + 4,
            fields.position.current.y.to_bits(),
        ),
        (
            RECORD_POSITION_OFFSET + 8,
            fields.position.current.z.to_bits(),
        ),
    ];
    written.extend(
        RECORD_ORIGINAL_POSITION_OFFSETS
            .into_iter()
            .zip(fields.position.original.components.map(f32::to_bits)),
    );
    written.extend(
        RECORD_ROTATION_INPUT_OFFSETS
            .into_iter()
            .zip(fields.rotation_inputs.components.map(f32::to_bits)),
    );
    written.extend(
        RECORD_ACTOR_PARAMETER_OFFSETS.into_iter().zip(
            [
                fields.actor_parameters.at_1cc,
                fields.actor_parameters.at_1d8,
            ]
            .map(f32::to_bits),
        ),
    );
    for (offset, expected_word) in written {
        assert_eq!(m.read32(base + offset), expected_word);
    }

    let invalid = pool.owned_records();
    let owned_before = pool.clone();
    let guest_before = m.clone();
    assert!(!pool.set_record_measured_fields(&mut m, invalid, fields));
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);
}

#[test]
fn measured_record_rotation_inputs_round_trip_and_are_bounded() {
    let mut m = image();
    let mut pool = ObjectPool::load(&m);
    let record = 1;
    let base = rec_addr(RECS, record, 0);
    let before_zero = pool.record_state(0).unwrap();
    let before_two = pool.record_state(2).unwrap();
    let before_gaps = [
        m.read32(base + 0x6c),
        m.read32(base + 0x74),
        m.read32(base + 0xbc),
        m.read32(base + 0xc4),
        m.read32(base + 0xcc),
    ];
    let inputs = PoolRecordRotationInputs {
        components: [
            f32::from_bits(0x4123_4567),
            f32::from_bits(0x8000_0000),
            f32::from_bits(0x7fc1_2345),
        ],
    };

    assert!(pool.set_record_rotation_inputs(&mut m, record, inputs));
    let stored = pool.record_rotation_inputs(record).unwrap();
    for (index, value) in inputs.components.into_iter().enumerate() {
        let offset = RECORD_ROTATION_INPUT_OFFSETS[index];
        assert_eq!(stored.components[index].to_bits(), value.to_bits());
        assert_eq!(m.read32(base + offset), value.to_bits());
        assert_eq!(
            pool.owned_record(record).unwrap()[(offset / 4) as usize],
            value.to_bits()
        );
    }
    assert_eq!(
        [
            m.read32(base + 0x6c),
            m.read32(base + 0x74),
            m.read32(base + 0xbc),
            m.read32(base + 0xc4),
            m.read32(base + 0xcc),
        ],
        before_gaps
    );
    assert_eq!(pool.record_state(0), Some(before_zero));
    assert_eq!(pool.record_state(2), Some(before_two));
    assert_eq!(pool.diff(&m), None);

    let owned_before = pool.clone();
    let guest_before = m.clone();
    let invalid = pool.owned_records();
    assert!(pool.record_rotation_inputs(invalid).is_none());
    assert!(!pool.set_record_rotation_inputs(&mut m, invalid, PoolRecordRotationInputs::default()));
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);
}

#[test]
fn measured_record_actor_parameters_round_trip_and_are_bounded() {
    let mut m = image();
    let mut pool = ObjectPool::load(&m);
    let record = 1;
    let base = rec_addr(RECS, record, 0);
    let before_zero = pool.record_state(0).unwrap();
    let before_two = pool.record_state(2).unwrap();
    let gaps = [
        m.read32(base + 0x1c8),
        m.read32(base + 0x1d0),
        m.read32(base + 0x1d4),
        m.read32(base + 0x1dc),
    ];
    let parameters = PoolRecordActorParameters {
        at_1cc: f32::from_bits(0x4123_4567),
        at_1d8: f32::from_bits(0x7fc1_2345),
    };

    assert!(pool.set_record_actor_parameters(&mut m, record, parameters));
    let stored = pool.record_actor_parameters(record).unwrap();
    for (offset, value) in RECORD_ACTOR_PARAMETER_OFFSETS
        .into_iter()
        .zip([parameters.at_1cc, parameters.at_1d8])
    {
        assert_eq!(m.read32(base + offset), value.to_bits());
        assert_eq!(
            pool.owned_record(record).unwrap()[(offset / 4) as usize],
            value.to_bits()
        );
    }
    assert_eq!(stored.at_1cc.to_bits(), parameters.at_1cc.to_bits());
    assert_eq!(stored.at_1d8.to_bits(), parameters.at_1d8.to_bits());
    assert_eq!(
        [
            m.read32(base + 0x1c8),
            m.read32(base + 0x1d0),
            m.read32(base + 0x1d4),
            m.read32(base + 0x1dc),
        ],
        gaps
    );
    assert_eq!(pool.record_state(0), Some(before_zero));
    assert_eq!(pool.record_state(2), Some(before_two));
    assert_eq!(pool.diff(&m), None);

    let owned_before = pool.clone();
    let guest_before = m.clone();
    let invalid = pool.owned_records();
    assert!(pool.record_actor_parameters(invalid).is_none());
    assert!(!pool.set_record_actor_parameters(
        &mut m,
        invalid,
        PoolRecordActorParameters::default()
    ));
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);
}

#[test]
fn measured_record_model_key_writer_preserves_halfword_and_is_bounded() {
    let mut m = image();
    let record = 1;
    let base = rec_addr(RECS, record, 0);
    let offset = RECORD_MODEL_ID_OFFSET;
    let initial_word = 0xa5c0_03b3;
    let initial: u16 = 0x03b3;
    m.write32(base + offset, initial_word);
    let mut pool = ObjectPool::load(&m);
    let before_zero = pool.record_state(0).unwrap();
    let before_two = pool.record_state(2).unwrap();
    let neighbors = [m.read32(base + offset - 4), m.read32(base + offset + 4)];

    assert_eq!(TsfpObjectView(base).model_id(&m), initial);
    let changed: u16 = 0x0553;
    assert!(pool.set_record_model_key(&mut m, record, changed));
    let expected_word = (initial_word & 0xffff_0000) | u32::from(changed);
    assert_eq!(
        pool.owned_record(record).unwrap()[(offset / 4) as usize],
        expected_word
    );
    assert_eq!(m.read32(base + offset), expected_word);
    assert_eq!(TsfpObjectView(base).model_id(&m), changed);
    assert_eq!(
        [m.read32(base + offset - 4), m.read32(base + offset + 4)],
        neighbors
    );
    assert_eq!(pool.record_state(0), Some(before_zero));
    assert_eq!(pool.record_state(2), Some(before_two));
    assert_eq!(pool.diff(&m), None);

    let owned_before = pool.clone();
    let guest_before = m.clone();
    let invalid = pool.owned_records();
    assert!(!pool.set_record_model_key(&mut m, invalid, 0x0482));
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);
}

#[test]
fn character_reactivation_object_flags_are_preserved_and_bounded() {
    let mut m = image();
    let record = 1;
    let base = rec_addr(RECS, record, 0);
    let offset = RECORD_OBJECT_FLAGS_OFFSET;
    let initial = PoolRecordObjectFlags(0xa500_0042);
    m.write32(base + offset, initial.0);
    let mut pool = ObjectPool::load(&m);
    let before_zero = pool.record_state(0).unwrap();
    let before_two = pool.record_state(2).unwrap();
    let before_left = m.read32(base + offset - 4);
    let before_right = m.read32(base + offset + 4);

    assert_eq!(pool.record_object_flags(record), Some(initial));
    assert!(pool.apply_character_reactivation_object_flags(&mut m, record));
    let updated = PoolRecordObjectFlags(
        initial.0 | tsfp_state::object_pool::CHARACTER_REACTIVATION_OBJECT_FLAGS,
    );
    assert_eq!(pool.record_object_flags(record), Some(updated));
    assert_eq!(m.read32(base + offset), updated.0);
    assert_eq!(m.read32(base + offset - 4), before_left);
    assert_eq!(m.read32(base + offset + 4), before_right);
    assert_eq!(pool.record_state(0), Some(before_zero));
    assert_eq!(pool.record_state(2), Some(before_two));
    assert_eq!(pool.diff(&m), None);

    let owned_before = pool.clone();
    let guest_before = m.clone();
    let invalid = pool.owned_records();
    assert_eq!(pool.record_object_flags(invalid), None);
    assert!(!pool.apply_character_reactivation_object_flags(&mut m, invalid));
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);
}

#[test]
fn measured_retirement_flag_is_applied_without_touching_other_fields() {
    let mut m = image();
    let record = 1;
    let base = rec_addr(RECS, record, 0);
    let offset = RECORD_OBJECT_FLAGS_OFFSET;
    let initial = PoolRecordObjectFlags(0x2500_0042);
    m.write32(base + offset, initial.0);
    let mut pool = ObjectPool::load(&m);
    let before_zero = pool.record_state(0).unwrap();
    let before_two = pool.record_state(2).unwrap();
    let neighbors = [m.read32(base + offset - 4), m.read32(base + offset + 4)];

    assert!(pool.mark_record_retired(&mut m, record));
    let expected = PoolRecordObjectFlags(initial.0 | OBJECT_RETIRED_FLAG);
    assert_eq!(pool.record_object_flags(record), Some(expected));
    assert_eq!(m.read32(base + offset), expected.0);
    assert_eq!(
        [m.read32(base + offset - 4), m.read32(base + offset + 4)],
        neighbors
    );
    assert_eq!(pool.record_state(0), Some(before_zero));
    assert_eq!(pool.record_state(2), Some(before_two));
    assert_eq!(pool.diff(&m), None);

    let owned_before = pool.clone();
    let guest_before = m.clone();
    let invalid = pool.owned_records();
    assert!(!pool.mark_record_retired(&mut m, invalid));
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);
}

#[test]
fn character_reactivation_applies_flags_before_optional_membership_append() {
    use tsfp_state::character::{
        CharacterReactivationMembership, CharacterSet, MEMBER_COUNT as CHARACTER_MEMBER_COUNT,
        OBJECT_EXTENSION_OFFSET, TABLE as CHARACTER_TABLE,
    };

    const EXTENSION: u32 = 0x43f0_0000;
    let mut m = image();
    let record = 1;
    let object = rec_addr(RECS, record, 0);
    m.write32(object + OBJECT_EXTENSION_OFFSET, EXTENSION);
    for word in 0..tsfp_state::character::BODY_WORDS as u32 {
        m.write32(EXTENSION + 4 * word, 0x7100_0000 | word);
    }
    m.write32(CHARACTER_MEMBER_COUNT, 0);
    let initial_flags = m.read32(object + RECORD_OBJECT_FLAGS_OFFSET);

    let mut objects = ObjectPool::load(&m);
    let mut characters = CharacterSet::load(&m);
    let result = characters
        .apply_object_reactivation(&mut m, &mut objects, record, true)
        .unwrap();

    assert_eq!(
        result.object_flags.0,
        initial_flags | CHARACTER_REACTIVATION_OBJECT_FLAGS
    );
    assert_eq!(
        result.membership,
        CharacterReactivationMembership::Appended { slot: 0 }
    );
    assert_eq!(
        m.read32(object + RECORD_OBJECT_FLAGS_OFFSET),
        result.object_flags.0
    );
    assert_eq!(m.read32(CHARACTER_MEMBER_COUNT), 1);
    assert_eq!(m.read32(CHARACTER_TABLE), EXTENSION);
    assert_eq!(characters.body_word(0, 0x20), 0x7100_0008);
    assert_eq!(objects.diff(&m), None);
    assert_eq!(characters.diff(&m), None);

    let before_table = m.read32(CHARACTER_TABLE);
    let skipped = characters
        .apply_object_reactivation(&mut m, &mut objects, record, false)
        .unwrap();
    assert_eq!(
        skipped.membership,
        CharacterReactivationMembership::BranchNotTaken
    );
    assert_eq!(m.read32(CHARACTER_MEMBER_COUNT), 1);
    assert_eq!(m.read32(CHARACTER_TABLE), before_table);
    assert_eq!(objects.diff(&m), None);
    assert_eq!(characters.diff(&m), None);

    let objects_before = objects.clone();
    let characters_before = characters.clone();
    let guest_before = m.clone();
    let invalid_record = objects.owned_records();
    assert!(
        characters
            .apply_object_reactivation(&mut m, &mut objects, invalid_record, true)
            .is_none()
    );
    assert_eq!(objects, objects_before);
    assert_eq!(characters, characters_before);
    assert_eq!(m, guest_before);
}

#[test]
fn character_reactivation_reports_bounded_append_rejection_after_flag_write() {
    use tsfp_state::character::{
        CharacterReactivationMembership, CharacterSet, MEMBER_COUNT as CHARACTER_MEMBER_COUNT,
    };

    let mut m = image();
    let record = 0;
    let initial_flags = m.read32(rec_addr(RECS, record, RECORD_OBJECT_FLAGS_OFFSET));
    m.write32(CHARACTER_MEMBER_COUNT, tsfp_state::character::SLOTS as u32);
    let mut objects = ObjectPool::load(&m);
    let mut characters = CharacterSet::load(&m);

    let result = characters
        .apply_object_reactivation(&mut m, &mut objects, record, true)
        .unwrap();

    assert_eq!(
        result.membership,
        CharacterReactivationMembership::AppendRejected
    );
    assert_eq!(
        result.object_flags.0,
        initial_flags | CHARACTER_REACTIVATION_OBJECT_FLAGS
    );
    assert_eq!(
        m.read32(CHARACTER_MEMBER_COUNT),
        tsfp_state::character::SLOTS as u32
    );
    assert_eq!(objects.diff(&m), None);
    assert_eq!(characters.diff(&m), None);
}

#[test]
fn initializer_object_flags_follow_measured_model_branch() {
    let mut m = image();
    let mut pool = ObjectPool::load(&m);
    let model_keys = [0x03b3, 0x0482, 0x0553];
    let initial_flags = [0xa500_0001, 0x5a00_0002, 0x8000_0004];
    let neighbors: Vec<_> = (0..model_keys.len())
        .map(|record| {
            let base = rec_addr(RECS, record, 0) + RECORD_OBJECT_FLAGS_OFFSET;
            (m.read32(base - 4), m.read32(base + 4))
        })
        .collect();

    for record in 0..model_keys.len() {
        assert!(pool.set_record_model_key(&mut m, record, model_keys[record]));
        assert!(pool.set_record_object_flags(
            &mut m,
            record,
            PoolRecordObjectFlags(initial_flags[record]),
        ));
    }
    for record in 0..model_keys.len() {
        let before_others: Vec<_> = (0..model_keys.len())
            .filter(|other| *other != record)
            .map(|other| (other, pool.record_state(other).unwrap()))
            .collect();
        assert!(pool.apply_initializer_object_flags(&mut m, record));
        let extra = if model_keys[record] == 0x0553 {
            MODEL_0553_INITIALIZER_OBJECT_FLAGS
        } else {
            0
        };
        let expected =
            PoolRecordObjectFlags(initial_flags[record] | INITIALIZER_OBJECT_FLAGS | extra);
        let base = rec_addr(RECS, record, 0) + RECORD_OBJECT_FLAGS_OFFSET;
        assert_eq!(pool.record_object_flags(record), Some(expected));
        assert_eq!(m.read32(base), expected.0);
        assert_eq!((m.read32(base - 4), m.read32(base + 4)), neighbors[record]);
        for (other, before) in before_others {
            assert_eq!(pool.record_state(other), Some(before));
        }
    }
    assert_eq!(pool.diff(&m), None);

    let owned_before = pool.clone();
    let guest_before = m.clone();
    let invalid = pool.owned_records();
    assert!(!pool.apply_initializer_object_flags(&mut m, invalid));
    assert_eq!(pool, owned_before);
    assert_eq!(m, guest_before);
}
