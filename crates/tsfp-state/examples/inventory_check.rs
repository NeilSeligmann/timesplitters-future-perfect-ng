// SPDX-License-Identifier: GPL-3.0-or-later
//! T2072 replay check: loads the captured inventory regions (extracted by
//! `python3 -m tools.s3_inventory extract`) into `WeaponEntries` and `Inventory`, stores them into a
//! copy and requires the region bytes to be EQUAL. It also (a) recomputes the group numbers with the
//! Rust port of 0x1B6B40 from the captured flags and requires them to equal the captured ones, and
//! (b) replays sample-to-sample transitions through the write-through setters.
//! `--mutate` flips one owned cell, one group word and one ammo word per sample and requires DIVERGED.
//! Usage: inventory_check <dir> [--mutate]
use std::{env, fs, process::ExitCode};
use tsfp_state::GuestMemory;
use tsfp_state::inventory::*;

/// (name, base address or None for the ext block, ext offset).
const REGIONS: [(&str, u32, u32); 7] = [
    ("weapon_entries", ENTRY_TABLE, 0),
    ("ext_head", 0, 0),
    ("ext_owned", 0, EXT_OWNED),
    ("ext_ammo", 0, EXT_AMMO),
    ("weapon_order", 0x4fc888, 0),
    ("group_count", 0x75a020, 0),
    ("player_ptrs", 0x7356d0, 0),
];

#[derive(Clone, PartialEq)]
struct Sample {
    ranges: Vec<(u32, Vec<u8>)>,
}
impl Sample {
    fn find(&self, a: u32) -> Option<usize> {
        self.ranges
            .iter()
            .position(|(base, b)| a >= *base && ((a - *base) as usize) < b.len())
    }
}
impl GuestMemory for Sample {
    fn read8(&self, a: u32) -> u8 {
        self.find(a)
            .map_or(0, |i| self.ranges[i].1[(a - self.ranges[i].0) as usize])
    }
    fn write8(&mut self, a: u32, v: u8) {
        if let Some(i) = self.find(a) {
            let base = self.ranges[i].0;
            self.ranges[i].1[(a - base) as usize] = v;
        }
    }
}

fn load_sample(dir: &str, name: &str) -> Sample {
    let read = |r: &str| fs::read(format!("{dir}/{name}.{r}.bin")).unwrap();
    // The ext regions are pointer-indirect: place them at the pointer captured in player_ptrs.
    let ptrs = read("player_ptrs");
    let ext = u32::from_le_bytes([ptrs[8], ptrs[9], ptrs[10], ptrs[11]]);
    Sample {
        ranges: REGIONS
            .iter()
            .map(|(r, base, off)| (if *base != 0 { *base } else { ext + off }, read(r)))
            .collect(),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let dir = args
        .get(1)
        .expect("usage: inventory_check <dir> [--mutate]");
    let mutate = args.iter().any(|a| a == "--mutate");
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".player_ptrs.bin").map(str::to_owned))
        .collect();
    names.sort();
    let (mut equal, mut diverged, mut group_ok, mut group_bad) = (0, 0, 0, 0);
    let (mut replay_equal, mut replay_bad, mut null_ext, mut rebased) = (0, 0, 0, 0);
    let mut owned_seen = std::collections::BTreeSet::new();
    let mut ammo_nonzero = 0;
    let mut prev: Option<(Inventory, WeaponEntries)> = None;
    for name in &names {
        let sample = load_sample(dir, name);
        let Some(mut inv) = Inventory::load_slot(&sample, 0) else {
            null_ext += 1;
            continue;
        };
        let mut table = WeaponEntries::load(&sample);
        for (i, o) in inv.owned.iter().enumerate() {
            if *o != 0 {
                owned_seen.insert(i);
            }
        }
        ammo_nonzero += inv.ammo.iter().filter(|a| **a != 0).count();
        // (a) 0x1B6B40 port: zero the groups in a copy, recompute from flags, compare.
        let mut copy = sample.clone();
        let mut t2 = table.clone();
        for i in 0..ENTRY_COUNT {
            t2.set_group(&mut copy, i, 0);
        }
        t2.assign_groups(&mut copy);
        if t2 == table && t2.diff(&sample).is_none() {
            group_ok += 1
        } else {
            group_bad += 1
        }
        // (b) transition replay from the previous sample through the setters.
        // The ext block pointer moves while the level loads: such a transition is a new block, not a
        // write, and is counted separately.
        if let Some((pi, _)) = &prev
            && pi.base != inv.base
        {
            rebased += 1;
        } else if let Some((pi, pt)) = &prev {
            let mut m = sample.clone();
            // Reset the declared cells to the previous state, then apply writers to reach this one.
            pi.store(&mut m);
            pt.store(&mut m);
            let mut st = *pi;
            let mut tt = pt.clone();
            for e in 0..ENTRY_COUNT {
                if st.owned[e] != inv.owned[e] {
                    st.set_owned(&mut m, e, inv.owned[e])
                }
                if st.equipped[e] != inv.equipped[e] {
                    st.set_equipped(&mut m, e, inv.equipped[e])
                }
                for w in 0..15 {
                    if tt.entries[e].words[w] != table.entries[e].words[w] {
                        tt.set_word(&mut m, e, w, table.entries[e].words[w])
                    }
                }
            }
            for t in 0..AMMO_TYPES {
                if st.ammo[t] != inv.ammo[t] {
                    st.set_ammo(&mut m, t, inv.ammo[t])
                }
            }
            if st.current != inv.current {
                st.set_current(&mut m, inv.current)
            }
            if st.previous != inv.previous {
                st.set_previous(&mut m, inv.previous)
            }
            if st.throwable != inv.throwable {
                st.set_throwable(&mut m, inv.throwable)
            }
            if st.flags != inv.flags {
                st.set_flags(&mut m, inv.flags)
            }
            if st == inv && tt == table && st.diff(&sample).is_none() && tt.diff(&sample).is_none()
            {
                replay_equal += 1
            } else {
                replay_bad += 1
            }
        }
        prev = Some((inv, table.clone()));
        if mutate {
            inv.owned[1] ^= 1;
            table.entries[0].words[GROUP_WORD] ^= 1;
            inv.ammo[0] ^= 1;
        }
        let mut out = sample.clone();
        inv.store(&mut out);
        table.store(&mut out);
        if out == sample && inv.diff(&sample).is_none() && table.diff(&sample).is_none() {
            equal += 1
        } else {
            diverged += 1
        }
    }
    let n = names.len() - null_ext;
    println!(
        "samples {n} null_ext {null_ext} equal {equal} diverged {diverged} groups_recomputed_equal {group_ok} groups_bad {group_bad} writer_replay_equal {replay_equal} writer_replay_bad {replay_bad} ext_rebased {rebased} owned_entries_seen {owned_seen:?} ammo_nonzero_words {ammo_nonzero}"
    );
    let ok = if mutate {
        diverged == n && n > 0
    } else {
        diverged == 0 && group_bad == 0 && replay_bad == 0 && n > 0
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}
