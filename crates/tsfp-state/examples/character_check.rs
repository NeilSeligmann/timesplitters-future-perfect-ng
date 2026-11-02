// SPDX-License-Identifier: GPL-3.0-or-later
//! T2077 replay check: loads captured character regions (extracted by `python3 -m tools.s3_character
//! extract`) into `CharacterSet`, stores them back into a fresh copy of the sample and requires the
//! declared bytes to be EQUAL. A second pass applies the changed dwords between consecutive samples
//! through the write-through setter and requires the next sample to be reproduced, and counts changed
//! dwords that the T2057 table does not list (drift against real data). `--mutate` flips one dword per
//! sample (rotating over the T2057 table) and requires DIVERGED. Usage: character_check <dir> [--mutate]
use std::{collections::BTreeMap, collections::BTreeSet, env, fs, process::ExitCode};
use tsfp_state::GuestMemory;
use tsfp_state::character::*;

const CSV: &str = include_str!("../../../docs/data/t2057-write-attribution.csv");
const TABLE_LEN: usize = 0x80;

#[derive(Clone, PartialEq)]
struct Sample(BTreeMap<u32, u8>);
impl Sample {
    fn put(&mut self, base: u32, bytes: &[u8]) {
        for (i, b) in bytes.iter().enumerate() {
            self.0.insert(base.wrapping_add(i as u32), *b);
        }
    }
}
impl GuestMemory for Sample {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        // Stay inside the captured extents: a store outside them is a failure.
        match self.0.get_mut(&a) {
            Some(x) => *x = v,
            None => panic!("store outside the captured extents at {a:#x}"),
        }
    }
}

fn fields() -> Vec<u32> {
    let mut v: Vec<u32> = CSV
        .lines()
        .skip(1)
        .filter_map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            (c[2] == "char")
                .then(|| u32::from_str_radix(c[3].trim_start_matches("0x"), 16).unwrap())
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let dir = args
        .get(1)
        .expect("usage: character_check <dir> [--mutate]");
    let mutate = args.iter().any(|a| a == "--mutate");
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".addrs").map(str::to_owned))
        .collect();
    names.sort();
    let table = fields();
    let listed: BTreeSet<u32> = table.iter().copied().collect();
    let (mut equal, mut diverged, mut bodies, mut flips, mut max_live) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    let (mut transitions, mut reproduced, mut changed, mut unlisted) =
        (0usize, 0usize, 0usize, 0usize);
    let mut unlisted_offsets = BTreeSet::new();
    let mut prev: Option<(Sample, CharacterSet)> = None;
    for (n, name) in names.iter().enumerate() {
        let read = |region: &str| fs::read(format!("{dir}/{name}.{region}.bin")).unwrap();
        let addrs: BTreeMap<String, u32> = fs::read_to_string(format!("{dir}/{name}.addrs"))
            .unwrap()
            .lines()
            .map(|l| {
                let (k, v) = l.split_once('=').unwrap();
                (
                    k.to_owned(),
                    u32::from_str_radix(v.trim_start_matches("0x"), 16).unwrap(),
                )
            })
            .collect();
        let table_bytes = read("npc_table");
        assert_eq!(table_bytes.len(), TABLE_LEN);
        let mut image = Sample(BTreeMap::new());
        image.put(TABLE, &table_bytes);
        for slot in 0..SLOTS.min(14) {
            let ptr = u32::from_le_bytes(table_bytes[slot * 4..][..4].try_into().unwrap());
            let base = addrs[&format!("char{slot}")];
            assert_eq!(base, ptr, "{name}: slot {slot} range follows the cell");
            if ptr != 0 {
                image.put(ptr, &read(&format!("char{slot}")));
                bodies += 1;
            }
        }
        let mut set = CharacterSet::load(&image);
        max_live = max_live.max(set.live());
        // Transition pass: previous owned set + changed dwords through set_word == this sample.
        if !mutate {
            if let Some((pimage, mut pset)) = prev.take() {
                if pset.ptrs == set.ptrs {
                    transitions += 1;
                    let mut scratch = pimage.clone();
                    for slot in 0..SLOTS {
                        if set.ptrs[slot] == 0 {
                            continue;
                        }
                        for j in 0..BODY_WORDS {
                            let off = j as u32 * 4;
                            if pset.bodies[slot][j] != set.bodies[slot][j] {
                                changed += 1;
                                if !listed.contains(&off) {
                                    unlisted += 1;
                                    unlisted_offsets.insert(off);
                                }
                                pset.set_word(&mut scratch, slot, off, set.bodies[slot][j]);
                            }
                        }
                    }
                    if pset.diff(&image).is_none() && scratch == image {
                        reproduced += 1;
                    }
                }
            }
            prev = Some((image.clone(), set.clone()));
        }
        if mutate {
            let live: Vec<usize> = (0..SLOTS).filter(|s| set.ptrs[*s] != 0).collect();
            if live.is_empty() {
                set.ptrs[0] ^= 0x10; // no character yet: corrupt a cell instead
            } else {
                let slot = live[n % live.len()];
                set.bodies[slot][(table[n % table.len()] / 4) as usize] ^= 1;
                flips += 1;
            }
        }
        let mut copy = image.clone();
        let wrote = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            set.store(&mut copy);
        }));
        let same = wrote.is_ok() && copy == image;
        let coherent = set.diff(&image).is_none();
        if same && coherent {
            equal += 1
        } else {
            diverged += 1
        }
    }
    println!(
        "samples {} equal {equal} diverged {diverged} bodies {bodies} max_live {max_live} flips {flips} transitions {transitions} reproduced {reproduced} changed_dwords {changed} unlisted_changed {unlisted} unlisted_offsets {:x?}",
        names.len(),
        unlisted_offsets
    );
    let ok = if mutate {
        diverged == names.len() && !names.is_empty()
    } else {
        diverged == 0 && reproduced == transitions && transitions > 0 && !names.is_empty()
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}
