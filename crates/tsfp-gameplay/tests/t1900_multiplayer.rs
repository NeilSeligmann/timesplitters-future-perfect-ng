// SPDX-License-Identifier: GPL-3.0-or-later
//! T1900 slice B Tier 0 smoke tests (hand-built fixtures and seeded shape loops, NOT differential).
use std::collections::HashMap;
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu},
    memory::GuestMemory,
};
use tsfp_gameplay::t1900_multiplayer as t;

#[derive(Default)]
struct Mem(HashMap<u32, u8>);
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.insert(a, v);
    }
}
/// Records (target, args[0..10]); answers with scripted EAX and ST0.
#[derive(Default)]
struct Trace {
    calls: Vec<(u32, [u32; 10])>,
    eax: HashMap<u32, u32>,
    st0: HashMap<u32, f64>,
    last_st0: f64,
}
impl Callee<Mem> for Trace {
    fn call(&mut self, m: &mut Mem, target: u32, cpu: &mut Cpu) {
        let mut args = [0; 10];
        for (i, a) in args.iter_mut().enumerate() {
            *a = m.read32(cpu.esp + 4 + 4 * i as u32);
        }
        self.calls.push((target, args));
        cpu.eax = *self.eax.get(&target).unwrap_or(&0);
        self.last_st0 = *self.st0.get(&target).unwrap_or(&0.0);
    }
}
impl t::X87Callee<Mem> for Trace {
    fn st0(&self) -> f64 {
        self.last_st0
    }
}
fn cpu() -> Cpu {
    Cpu {
        esp: 0x1000,
        ..Cpu::default()
    }
}
fn setup_args(m: &mut Mem, args: &[u32]) {
    for (i, a) in args.iter().enumerate() {
        m.write32(0x1004 + 4 * i as u32, *a);
    }
}
fn w16(m: &mut Mem, a: u32, v: u16) {
    m.write8(a, v as u8);
    m.write8(a + 1, (v >> 8) as u8);
}
fn f(v: f32) -> u32 {
    v.to_bits()
}

/// Builds object `index` in the 7A2BE4 table with record at `record`.
fn put_object(m: &mut Mem, index: u32, object: u32, record: u32) {
    m.write32(t::OBJECT_TABLE + index * 8, object);
    m.write32(object + 0x7c, record);
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
}

#[test]
fn record_flag_leaves_follow_the_object_table() {
    let mut m = Mem::default();
    put_object(&mut m, 3, 0x2000, 0x3000);
    put_object(&mut m, 1, 0x2100, 0x3100);
    put_object(&mut m, 2, 0x2200, 0x3200);
    m.write32(0x3000 + 0xa0, 0xff);
    m.write32(0x3100 + 0xa0, 0);
    m.write32(0x3200 + 0xa0, 4);
    assert_eq!(t::slot_record_flag_a0_bit2(&m, 3), 4);
    assert_eq!(t::opposing_record_flag_a0_bit2(&m, 1), 4); // arg 1 -> entry 2
    assert_eq!(t::opposing_record_flag_a0_bit2(&m, 2), 0); // else -> entry 1
    assert_eq!(t::opposing_record_flag_a0_bit2(&m, 77), 0);
    assert_eq!(t::opposing_side_object(&m, 1), 0x2200);
    assert_eq!(t::opposing_side_object(&m, 0), 0x2100);
}

#[test]
fn bit7_clear_is_a_dword_test() {
    let mut m = Mem::default();
    put_object(&mut m, 1, 0x2000, 0x3000);
    assert_eq!(t::slot_object_flag_28_bit7_clear(&m, 1), 1);
    m.write32(0x2000 + 0x28, 0x80);
    assert_eq!(t::slot_object_flag_28_bit7_clear(&m, 1), 0);
    m.write32(0x2000 + 0x28, 0x7f | 0x100);
    assert_eq!(t::slot_object_flag_28_bit7_clear(&m, 1), 1);
}

#[test]
fn opposing_search_tail_call_rewrites_arg() {
    for (side, mapped) in [(1, 2), (2, 1), (0, 1), (9, 1)] {
        let (mut m, mut tr, mut c) = (Mem::default(), Trace::default(), cpu());
        setup_args(&mut m, &[side]);
        t::find_first_for_opposing_side(&mut tr, &mut m, &mut c);
        assert_eq!(tr.calls[0].0, 0x274510);
        assert_eq!(tr.calls[0].1[0], mapped);
    }
}

#[test]
fn allocate_side_tables_sizes_and_stores() {
    let (mut m, mut tr, mut c) = (Mem::default(), Trace::default(), cpu());
    m.write8(t::SIDE_COUNT_BYTE, 3);
    tr.eax.insert(0x3dfa0, 0x9000);
    t::allocate_side_tables(&mut tr, &mut m, &mut c);
    assert_eq!(tr.calls.len(), 2);
    assert_eq!(&tr.calls[0].1[..2], &[0x100, 1]);
    assert_eq!(&tr.calls[1].1[..2], &[48, 1]); // 3*12=36+15=51 & !15 = 48
    assert_eq!(m.read32(t::SIDE_TABLE_PTR), 0x9000);
    assert_eq!(m.read32(t::SIDE_BLOCK_PTR), 0x9000);
    assert_eq!(c.esp, 0x1000);
    // Negative side counts sign-extend: -1 * 12 + 15 = 3 -> 0.
    m.write8(t::SIDE_COUNT_BYTE, 0xff);
    t::allocate_side_tables(&mut tr, &mut m, &mut c);
    assert_eq!(tr.calls[3].1[0], 0);
}

#[test]
fn side_flag_refresh_branches() {
    let mut m = Mem::default();
    m.write32(t::SIDE_TABLE_PTR, 0x8000);
    m.write32(t::SIDE_STAMP, 1234);
    put_object(&mut m, 2, 0x2000, 0x3000);
    // Gated off: no side count, or wrong mode.
    t::side_flag_refresh(&mut m, 2);
    assert_eq!(m.read32(0x8000 + 32 - 8), 0);
    m.write8(t::SIDE_COUNT_BYTE, 2);
    t::side_flag_refresh(&mut m, 2);
    assert_eq!(m.read32(0x8000 + 24), 0);
    m.write8(t::MODE_BYTE, 2);
    // bit 7 clear -> flag 0 == stored 0: unchanged, stamp untouched.
    t::side_flag_refresh(&mut m, 2);
    assert_eq!(m.read32(0x8000 + 28), 0);
    // bit 7 set -> flag 1, stamp refreshed.
    m.write32(0x2000 + 0x28, 0x80);
    t::side_flag_refresh(&mut m, 2);
    assert_eq!((m.read32(0x8000 + 24), m.read32(0x8000 + 28)), (1, 1234));
    // Same state again keeps an older stamp.
    m.write32(t::SIDE_STAMP, 99);
    t::side_flag_refresh(&mut m, 2);
    assert_eq!(m.read32(0x8000 + 28), 1234);
    // Back to clear flips to 0 and restamps.
    m.write32(0x2000 + 0x28, 0);
    t::side_flag_refresh(&mut m, 2);
    assert_eq!((m.read32(0x8000 + 24), m.read32(0x8000 + 28)), (0, 99));
}

#[test]
fn find_named_entry_hits_misses_and_empty() {
    let mut m = Mem::default();
    assert_eq!(t::find_named_entry(&m, 5), 0);
    m.write32(t::NAMED_TABLE_COUNT, 3);
    for (i, key) in [10u32, 20, 30].into_iter().enumerate() {
        m.write32(t::NAMED_TABLE + 8 * i as u32, key);
    }
    m.write32(t::NAMED_TABLE + 24, 40); // beyond the count
    assert_eq!(t::find_named_entry(&m, 20), t::NAMED_TABLE + 8);
    assert_eq!(t::find_named_entry(&m, 40), 0);
    m.write32(t::NAMED_TABLE_COUNT, u32::MAX); // negative count
    assert_eq!(t::find_named_entry(&m, 10), 0);
}

fn party_fixture(m: &mut Mem, second: bool) {
    m.write8(t::MODE_BYTE, 0xa);
    m.write32(t::PLAYER_COUNT, 2);
    m.write32(t::PLAYER_ARRAY_PTR, 0x5000);
    m.write32(0x5000 + 8, 0x6000);
    if second {
        m.write32(0x5000 + 0x158c, 0x7000);
    }
    m.write32(0x5000 + 0xc, 0xff);
    m.write32(0x5000 + 0x1590, 0xff);
    m.write32(t::PARTY_FLAG, 5);
}

#[test]
fn leave_party_mode_with_and_without_second_player() {
    let (mut m, mut tr, mut c) = (Mem::default(), Trace::default(), cpu());
    party_fixture(&mut m, true);
    t::leave_party_mode(&mut tr, &mut m, &mut c);
    assert_eq!(tr.calls.len(), 2);
    assert_eq!(&tr.calls[0].1[..2], &[0, 0x6224]);
    assert_eq!(&tr.calls[1].1[..2], &[1, 0x7224]);
    assert_eq!(m.read32(0x6000 + 0x21c), 1);
    assert_eq!(m.read32(0x7000 + 0x21c), 1);
    assert_eq!(m.read32(0x5000 + 0xc), 0xfe);
    assert_eq!(m.read32(0x5000 + 0x1590), 0xfd);
    assert_eq!(m.read32(t::PARTY_FLAG), 0);
    assert_eq!(c.esp, 0x1000);

    let (mut m, mut tr, mut c) = (Mem::default(), Trace::default(), cpu());
    party_fixture(&mut m, false);
    t::leave_party_mode(&mut tr, &mut m, &mut c);
    assert_eq!(tr.calls.len(), 1);
    assert_eq!(m.read32(t::PARTY_FLAG), 0);
}

#[test]
fn leave_party_mode_gates() {
    for (mode, players) in [(9u8, 2u32), (0xa, 1), (0xa, 0), (0xa, u32::MAX)] {
        let (mut m, mut tr, mut c) = (Mem::default(), Trace::default(), cpu());
        party_fixture(&mut m, true);
        m.write8(t::MODE_BYTE, mode);
        m.write32(t::PLAYER_COUNT, players);
        t::leave_party_mode(&mut tr, &mut m, &mut c);
        assert!(tr.calls.is_empty());
        assert_eq!(m.read32(t::PARTY_FLAG), 5);
    }
}

#[test]
fn party_mode_active_branches() {
    let run = |mode: u8, players: u32, getter: u32, flags: u32| {
        let (mut m, mut tr, mut c) = (Mem::default(), Trace::default(), cpu());
        m.write8(t::MODE_BYTE, mode);
        m.write32(t::PLAYER_COUNT, players);
        m.write32(t::MODE_FLAGS, flags);
        tr.eax.insert(0xcb0f0, getter);
        t::party_mode_active(&mut tr, &mut m, &mut c);
        (c.eax, tr.calls.len())
    };
    assert_eq!(run(9, 2, 1, 0x200), (0, 0));
    assert_eq!(run(0xa, 1, 1, 0x200), (0, 0));
    assert_eq!(run(0xa, 2, 7, 0), (1, 1));
    assert_eq!(run(0xa, 2, 0, 0x200), (1, 1));
    assert_eq!(run(0xa, 2, 0, 0x100), (0, 1));
    assert_eq!(run(0xa, 2, 0, 0xfffffdff), (0, 1));
}

fn roster(m: &mut Mem, count: u32, slots: &[u32]) {
    m.write32(t::PLAYER_COUNT, count);
    m.write32(t::ROSTER_PTR, 0x20000);
    for (i, slot) in slots.iter().enumerate() {
        m.write32(0x20000 + 0xea0 * i as u32 + 8, *slot);
    }
    m.write32(t::COUNT_THRESHOLD, f(0.0));
}

#[test]
fn roster_count_counts_positive_stats_and_skips_null_entries() {
    let (mut m, mut tr, mut c) = (Mem::default(), Trace::default(), cpu());
    roster(&mut m, 2, &[0, 1, 2]);
    m.write32(t::ROSTER_EXTRA_COUNT, 1);
    setup_args(&mut m, &[77]);
    tr.st0.insert(0x238580, 1.5);
    t::count_roster_with_positive_stat(&mut tr, &mut m, &mut c);
    assert_eq!(c.eax, 3);
    assert_eq!(tr.calls.len(), 3);
    assert_eq!(&tr.calls[2].1[..2], &[77, 2]);
    // A null roster pointer skips every entry.
    let (mut m, mut tr, mut c) = (Mem::default(), Trace::default(), cpu());
    roster(&mut m, 2, &[0, 1]);
    m.write32(t::ROSTER_PTR, 0);
    // entry address 0xea0 * i is non-null for i > 0 even so, so only entry 0 is skipped
    tr.st0.insert(0x238580, 1.0);
    t::count_roster_with_positive_stat(&mut tr, &mut m, &mut c);
    assert_eq!(c.eax, 1);
}

#[test]
fn roster_count_threshold_nan_and_empty() {
    let (mut m, mut tr, mut c) = (Mem::default(), Trace::default(), cpu());
    roster(&mut m, 1, &[3]);
    m.write32(t::COUNT_THRESHOLD, f(2.0));
    tr.st0.insert(0x238580, 2.0); // equal is not above
    t::count_roster_with_positive_stat(&mut tr, &mut m, &mut c);
    assert_eq!(c.eax, 0);
    tr.st0.insert(0x238580, f64::NAN);
    t::count_roster_with_positive_stat(&mut tr, &mut m, &mut c);
    assert_eq!(c.eax, 0);
    m.write32(t::PLAYER_COUNT, 0);
    let calls = tr.calls.len();
    t::count_roster_with_positive_stat(&mut tr, &mut m, &mut c);
    assert_eq!((c.eax, tr.calls.len()), (0, calls));
}

#[test]
fn roster_count_mode_13_exclusion() {
    let (mut m, mut tr, mut c) = (Mem::default(), Trace::default(), cpu());
    roster(&mut m, 1, &[0, 5]); // entry 0 below player count, entry 1 above
    m.write32(t::ROSTER_EXTRA_COUNT, 1);
    m.write8(t::MODE_BYTE_2, 0x13);
    w16(&mut m, 0x20000 + 0xea0 + 0x48, 0xfffe);
    tr.st0.insert(0x238580, 9.0);
    tr.eax.insert(0xd6c90, 1); // excluded type
    t::count_roster_with_positive_stat(&mut tr, &mut m, &mut c);
    assert_eq!(c.eax, 1); // only entry 0 counted, entry 1 skipped
    let d6c90 = tr.calls.iter().find(|call| call.0 == 0xd6c90).unwrap();
    assert_eq!(d6c90.1[0], 0xffff_fffe); // sign-extended i16
    tr.eax.insert(0xd6c90, 0);
    t::count_roster_with_positive_stat(&mut tr, &mut m, &mut c);
    assert_eq!(c.eax, 2);
}

/// Fixture for the ranking function: stats come from `counts`, keyed by the key argument.
fn rank(counts: [u32; 7], kind: u32, flag_word: u32, used_state: ()) -> [i32; 7] {
    let _ = used_state;
    let (mut m, mut tr, mut c) = (Mem::default(), Trace::default(), cpu());
    let (out, keys) = (0x30000u32, 0x31000u32);
    setup_args(&mut m, &[out, keys]);
    for i in 0..7u32 {
        m.write32(keys + 4 * i, i + 100);
    }
    m.write32(t::SCORE_WEIGHT_STEP, f(1.0));
    m.write32(t::SCORE_WEIGHT_BASE, f(0.0));
    m.write8(t::MODE_BYTE, 1);
    m.write8(t::MODE_BYTE_2, 1);
    m.write32(t::RULE_KIND_TABLE + 0x20, kind);
    m.write32(0x40000 + 20 + 0x24, flag_word);
    tr.eax.insert(0x69640, 0x40000);
    // 2F1FC0 is answered per key through a scripted table.
    let mut script = ScriptedCounts { inner: tr, counts };
    t::rank_categories_by_roster_score(&mut script, &mut m, &mut c);
    assert_eq!(c.esp, 0x1000);
    std::array::from_fn(|i| m.read32(out + 4 * i as u32) as i32)
}

struct ScriptedCounts {
    inner: Trace,
    counts: [u32; 7],
}
impl Callee<Mem> for ScriptedCounts {
    fn call(&mut self, m: &mut Mem, target: u32, cpu: &mut Cpu) {
        self.inner.call(m, target, cpu);
        if target == 0x2f1fc0 {
            let key = m.read32(cpu.esp + 4);
            cpu.eax = self.counts[(key - 100) as usize];
        }
    }
}

#[test]
fn ranking_orders_by_weighted_score_descending() {
    // weight = 7 - i, so scores = count * (7 - i).
    let out = rank([1, 1, 1, 1, 1, 1, 1], 0, 0, ());
    assert_eq!(out, [0, 1, 2, 3, 4, 5, 6]);
    // Score ties: later category wins ties (count*weight equal at 0:2*5 vs 1:... build one).
    let out = rank([0, 5, 0, 0, 0, 0, 0], 0, 0, ());
    assert_eq!(out[0], 1);
    assert_eq!(&out[1..], &[-1; 6]);
    let tie = rank([2, 0, 0, 0, 0, 0, 0], 0, 0, ());
    assert_eq!(tie[0], 0);
}

#[test]
fn ranking_tie_goes_to_later_category() {
    // category 0 weight 7, category 1 weight 6: 6*? choose counts 6 and 7 -> 42 vs 42.
    let out = rank([6, 7, 0, 0, 0, 0, 0], 0, 0, ());
    assert_eq!(out[0], 1);
    assert_eq!(out[1], 0);
}

#[test]
fn ranking_category_six_forced_to_zero_by_rule_kind() {
    let plain = rank([0, 0, 0, 0, 0, 0, 9], 0, 0, ());
    assert_eq!(plain[0], 6);
    assert_eq!(rank([0, 0, 0, 0, 0, 0, 9], 1, 0, ()), [-1; 7]);
    assert_eq!(rank([0, 0, 0, 0, 0, 0, 9], 2, 1, ()), [-1; 7]);
    assert_eq!(rank([0, 0, 0, 0, 0, 0, 9], 2, 0, ())[0], 6);
    assert_eq!(rank([0, 0, 0, 0, 0, 0, 9], 3, 1, ())[0], 6);
}

#[test]
fn ranking_negative_counts_never_qualify() {
    let out = rank([u32::MAX, 0, 0, 0, 0, 0, 0], 0, 0, ());
    assert_eq!(out, [-1; 7]);
}

/// Tier 0 shape test over a few thousand seeds: output entries are -1 or distinct categories.
#[test]
fn ranking_seeded_shape() {
    let mut rng = Rng(7);
    for _ in 0..3000 {
        let counts: [u32; 7] = std::array::from_fn(|_| match rng.next() % 4 {
            0 => 0,
            1 => rng.next() % 3,
            2 => rng.next() % 50,
            _ => rng.next(),
        });
        let out = rank(counts, rng.next() % 4, rng.next() % 2, ());
        let mut seen = 0u32;
        for value in out {
            assert!((-1..7).contains(&value));
            if value >= 0 {
                assert_eq!(seen & (1 << value), 0, "duplicate category");
                seen |= 1 << value;
            }
        }
    }
}

fn party_snapshot_fixture(m: &mut Mem, lists: &[&[(u32, u16)]]) {
    m.write8(t::MODE_BYTE, 0xa);
    m.write32(t::PLAYER_COUNT, lists.len() as u32);
    m.write32(t::PLAYER_ARRAY_PTR, 0x50000);
    for (p, list) in lists.iter().enumerate() {
        let p = p as u32;
        let player_object = 0x60000 + 0x1000 * p;
        let player_record = 0x61000 + 0x1000 * p;
        m.write32(0x50000 + 0x14 + p * 0x1584, player_object);
        m.write32(player_object + 0x7c, player_record);
        let mut next = 0u32;
        for (j, (object_address, id)) in list.iter().enumerate().rev() {
            let node = 0x70000 + 0x100 * (p * 8 + j as u32);
            m.write32(node + 0x10, *object_address);
            m.write32(node + 0x18, next);
            w16(m, object_address + 0x94, *id);
            m.write32(object_address + 0x7c, object_address + 0x400);
            for w in 0..0x36u32 {
                m.write32(object_address + 0x400 + 4 * w, (object_address << 8) ^ w);
            }
            next = node;
        }
        m.write32(player_record + 0x534, next);
    }
}

#[test]
fn snapshot_copies_ids_records_and_terminators() {
    let mut m = Mem::default();
    party_snapshot_fixture(
        &mut m,
        &[&[(0x90000, 0xbeef), (0x91000, 7)], &[(0x92000, 9)]],
    );
    // Pre-dirty a terminator slot to see it zeroed.
    m.write32(t::SNAPSHOT_IDS + 4 * 2, 0x55);
    t::snapshot_party(&mut m);
    assert_eq!(m.read32(t::SNAPSHOT_IDS), 0xbeef);
    assert_eq!(m.read32(t::SNAPSHOT_IDS + 4), 7);
    assert_eq!(m.read32(t::SNAPSHOT_IDS + 8), 0);
    assert_eq!(m.read32(t::SNAPSHOT_IDS + 4 * 0x14), 9);
    assert_eq!(m.read32(t::SNAPSHOT_IDS + 4 * 0x15), 0);
    assert_eq!(m.read32(t::SNAPSHOT_RECORDS + 0xd8 + 4), (0x91000 << 8) ^ 1);
    assert_eq!(
        m.read32(t::SNAPSHOT_RECORDS + 0x10e0 + 0x34 * 4),
        (0x92000 << 8) ^ 0x34
    );
}

#[test]
fn snapshot_gates() {
    let mut m = Mem::default();
    party_snapshot_fixture(&mut m, &[&[(0x90000, 3)]]);
    m.write8(t::MODE_BYTE, 3);
    t::snapshot_party(&mut m);
    assert_eq!(m.read32(t::SNAPSHOT_IDS), 0);
    m.write8(t::MODE_BYTE, 0xa);
    m.write32(t::PLAYER_COUNT, 0);
    t::snapshot_party(&mut m);
    assert_eq!(m.read32(t::SNAPSHOT_IDS), 0);
}

#[test]
fn restore_rebuilds_objects_and_calls_list_insert() {
    let (mut m, mut tr, mut c) = (Mem::default(), Trace::default(), cpu());
    party_snapshot_fixture(&mut m, &[&[(0x90000, 1), (0x91000, 2)]]);
    t::snapshot_party(&mut m);
    // C1950 returns a fresh object whose record lives at 0xA0400.
    m.write32(0xa0000 + 0x7c, 0xa0400);
    tr.eax.insert(0xc1950, 0xa0000);
    t::restore_party(&mut tr, &mut m, &mut c);
    let creates: Vec<_> = tr.calls.iter().filter(|call| call.0 == 0xc1950).collect();
    let inserts: Vec<_> = tr.calls.iter().filter(|call| call.0 == 0xdb420).collect();
    assert_eq!((creates.len(), inserts.len()), (2, 2));
    assert_eq!(inserts[0].1[..2], [0x61000 + 0x304, 0xa0000]);
    assert_eq!(m.read32(0xa0400 + 4), (0x91000 << 8) ^ 1); // last record copied over
    assert_eq!(c.esp, 0x1000);
}

#[test]
fn restore_running_total_quirk_offsets_later_players() {
    // Player 0 restores 2 entries, so player 1 starts reading at id slot 0x14 + 2.
    let (mut m, mut tr, mut c) = (Mem::default(), Trace::default(), cpu());
    party_snapshot_fixture(&mut m, &[&[(0x90000, 1), (0x91000, 2)], &[(0x92000, 3)]]);
    t::snapshot_party(&mut m);
    m.write32(0xa0000 + 0x7c, 0xa0400);
    tr.eax.insert(0xc1950, 0xa0000);
    t::restore_party(&mut tr, &mut m, &mut c);
    let creates = tr.calls.iter().filter(|call| call.0 == 0xc1950).count();
    assert_eq!(creates, 2); // player 1's id sits at slot 0x14, but slot 0x16 is read -> empty
}

#[test]
fn restore_gates() {
    let (mut m, mut tr, mut c) = (Mem::default(), Trace::default(), cpu());
    party_snapshot_fixture(&mut m, &[&[(0x90000, 1)]]);
    t::snapshot_party(&mut m);
    m.write8(t::MODE_BYTE, 0);
    t::restore_party(&mut tr, &mut m, &mut c);
    m.write8(t::MODE_BYTE, 0xa);
    m.write32(t::PLAYER_COUNT, 0);
    t::restore_party(&mut tr, &mut m, &mut c);
    assert!(tr.calls.is_empty());
}

/// Tier 0 shape test: random lists never panic, snapshots are zero terminated, restore creates
/// exactly the number of nonzero-id entries it reaches.
#[test]
fn snapshot_restore_seeded_shape() {
    let mut rng = Rng(99);
    for _ in 0..2000 {
        let mut m = Mem::default();
        let players = 1 + rng.next() % 2; // record snapshots of a third player would overlap 735630
        let lens: Vec<u32> = (0..players).map(|_| rng.next() % 5).collect();
        let owned: Vec<Vec<(u32, u16)>> = lens
            .iter()
            .enumerate()
            .map(|(p, &n)| {
                (0..n)
                    .map(|j| {
                        (
                            0x90000 + 0x1000 * (p as u32 * 8 + j),
                            1 + (rng.next() % 60000) as u16,
                        )
                    })
                    .collect()
            })
            .collect();
        let lists: Vec<&[(u32, u16)]> = owned.iter().map(|v| v.as_slice()).collect();
        party_snapshot_fixture(&mut m, &lists);
        t::snapshot_party(&mut m);
        for (p, n) in lens.iter().enumerate() {
            assert_eq!(m.read32(t::SNAPSHOT_IDS + 4 * (p as u32 * 0x14 + n)), 0);
        }
        let (mut tr, mut c) = (Trace::default(), cpu());
        m.write32(0xa0000 + 0x7c, 0xa0400);
        tr.eax.insert(0xc1950, 0xa0000);
        t::restore_party(&mut tr, &mut m, &mut c);
        let creates = tr.calls.iter().filter(|call| call.0 == 0xc1950).count();
        assert!(creates <= lens.iter().sum::<u32>() as usize);
        assert_eq!(c.esp, 0x1000);
    }
}

/// Tier 0 shape test: leaves never panic on arbitrary memory and honour their invariants.
#[test]
fn leaves_seeded_shape() {
    let mut rng = Rng(5);
    for _ in 0..4000 {
        let mut m = Mem::default();
        for i in 0..4u32 {
            put_object(&mut m, i, 0x2000 + 0x100 * i, 0x3000 + 0x100 * i);
            m.write32(0x2000 + 0x100 * i + 0x28, rng.next());
            m.write32(0x3000 + 0x100 * i + 0xa0, rng.next());
        }
        let index = rng.next() % 4;
        assert!(matches!(t::slot_record_flag_a0_bit2(&m, index), 0 | 4));
        assert!(t::slot_object_flag_28_bit7_clear(&m, index) <= 1);
        let side = rng.next();
        let object = t::opposing_side_object(&m, side);
        assert!(
            object == m.read32(t::OBJECT_TABLE + 8) || object == m.read32(t::OBJECT_TABLE + 16)
        );
        m.write32(t::NAMED_TABLE_COUNT, rng.next() % 10);
        let found = t::find_named_entry(&m, rng.next() % 3);
        assert!(found == 0 || (found >= t::NAMED_TABLE && (found - t::NAMED_TABLE) % 8 == 0));
    }
}
