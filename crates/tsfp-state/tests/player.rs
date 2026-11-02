// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED synthetic fixtures (patterned bytes); no retail data.
use std::collections::BTreeMap;
use tsfp_state::GuestMemory;
use tsfp_state::player::*;

#[derive(Clone, PartialEq)]
struct Image(BTreeMap<u32, u8>);
impl GuestMemory for Image {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.insert(a, v);
    }
}

const REC: u32 = 0x43dc_a6b0;
const STATS: u32 = 0x43d7_f3a0;

fn image() -> Image {
    let mut m = Image(BTreeMap::new());
    m.write32(RECORD_CELL, REC);
    m.write32(CONTROLLED_CELL, REC);
    m.write32(STATS_CELL, STATS);
    for i in 0..RECORD_SIZE / 4 {
        m.write32(REC + i * 4, i.wrapping_mul(0x9e37_79b1) ^ 0x1234_5678);
    }
    for i in 0..STATS_SIZE / 4 {
        m.write32(STATS + i * 4, (i as f32 * 0.5).to_bits());
    }
    m
}

/// Written record offsets of the T2057 attribution table (the drift source of truth).
fn csv_offsets(family_is_stats: bool) -> Vec<u32> {
    let csv = include_str!("../../../docs/data/t2057-write-attribution.csv");
    let mut v: Vec<u32> = csv
        .lines()
        .skip(1)
        .filter_map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            let stats = c[2].contains("stats");
            ((c[2] == "player" && !family_is_stats) || (stats && family_is_stats))
                .then(|| u32::from_str_radix(c[3].trim_start_matches("0x"), 16).unwrap())
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

#[test]
fn round_trip_is_byte_identical() {
    let m = image();
    let set = PlayerSet::load(&m);
    assert_eq!(set.diff(&m), None);
    assert!(set.controlled_is_record());
    let mut copy = m.clone();
    set.store(&mut copy);
    assert!(copy == m);
    // store into a blank image reproduces cells and bodies
    let mut blank = Image(BTreeMap::new());
    set.store(&mut blank);
    assert_eq!(PlayerSet::load(&blank), set);
}

#[test]
fn store_touches_only_declared_extents() {
    let mut m = image();
    m.write32(REC + RECORD_SIZE, 0xfeed_beef); // first byte after the record
    m.write32(STATS + STATS_SIZE, 0xcafe_f00d);
    let before = m.clone();
    PlayerSet::load(&m).store(&mut m);
    assert!(m == before);
}

#[test]
fn setters_write_through() {
    let mut m = image();
    let mut set = PlayerSet::load(&m);
    assert!(set.set_pose(&mut m, [1.5, -2.0, 3.25]));
    assert!(set.set_facing(&mut m, 0.75));
    assert_eq!(m.read32(REC + 0xa8), 1.5f32.to_bits());
    assert_eq!(m.read32(REC + 0xb0), 3.25f32.to_bits());
    assert_eq!(m.read32(REC + 0xc0), 0.75f32.to_bits());
    let p = set.prefix();
    assert_eq!((p.pose, p.facing), ([1.5, -2.0, 3.25], 0.75));
    assert_eq!(set.controlled_view().load(&m).facing, 0.75);
    // stats: player 1 stat 2 lives at (0x273 + 2) * 4
    assert_eq!(PlayerSet::stat_offset(1, 2), Some((0x273 + 2) * 4));
    assert!(set.set_stat(&mut m, 1, 2, 9.0f32.to_bits()));
    assert_eq!(m.read32(STATS + (0x273 + 2) * 4), 9.0f32.to_bits());
    assert_eq!(set.stat_bits(1, 2), Some(9.0f32.to_bits()));
    // out of range stat and missing record write nothing
    let snapshot = m.clone();
    assert!(!set.set_stat(&mut m, 2, 2, 1));
    assert!(!set.set_stat(&mut m, 0, 0x273 * 2 + 2, 1));
    let mut menu = PlayerSet::EMPTY;
    assert!(!menu.set_record_word(&mut m, 0, 7));
    assert!(m == snapshot);
    set.set_controlled(&mut m, 0x4000_0000);
    assert_eq!(m.read32(CONTROLLED_CELL), 0x4000_0000);
    assert!(!set.controlled_is_record());
    assert_eq!(set.diff(&m), None);
}

#[test]
fn drift_table_is_the_t2057_attribution() {
    let rec = csv_offsets(false);
    let stats = csv_offsets(true);
    assert_eq!(rec.len(), 214, "player written fields");
    assert_eq!(stats.len(), 16, "stats written fields");
    assert!(rec.iter().all(|o| o % 4 == 0 && *o < RECORD_SIZE));
    assert!(stats.iter().all(|o| o % 4 == 0 && *o < STATS_SIZE));
}

#[test]
fn mutation_per_owned_field_is_detected() {
    let m = image();
    let base = PlayerSet::load(&m);
    for off in csv_offsets(false) {
        let mut s = base.clone();
        s.record[(off / 4) as usize] ^= 1;
        assert_eq!(
            s.diff(&m),
            Some(PlayerDiff::Record(off)),
            "record +{off:#x}"
        );
    }
    for off in csv_offsets(true) {
        let mut s = base.clone();
        s.stats[(off / 4) as usize] ^= 1;
        assert_eq!(s.diff(&m), Some(PlayerDiff::Stats(off)), "stats +{off:#x}");
    }
    for (i, name) in ["record", "controlled", "stats"].into_iter().enumerate() {
        let mut s = base.clone();
        [&mut s.record_ptr, &mut s.controlled_ptr, &mut s.stats_ptr][i].clone_from(&0x1111_0000);
        assert_eq!(s.diff(&m), Some(PlayerDiff::Cell(name)));
    }
}
