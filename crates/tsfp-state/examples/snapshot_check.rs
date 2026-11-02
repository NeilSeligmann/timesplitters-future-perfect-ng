// SPDX-License-Identifier: GPL-3.0-or-later
//! T2080 replay check: builds `HudSnapshot` over the captured player regions (extracted by
//! `python3 -m tools.s3_player extract`, the T2071 Story dumps) and compares every captured-region
//! field with an independent direct read of the raw region bytes (what the lifted readers see).
//! Cells outside the captured regions read 0 in both and are not evidence (docs/t-s3-snapshots.md).
//! `--mutate` perturbs one raw byte after the snapshot was built from the clean image and requires
//! the comparison to FAIL for every sample. Usage: snapshot_check <dir> [--mutate]
use std::{collections::BTreeMap, env, fs, process::ExitCode};
use tsfp_state::GuestMemory;
use tsfp_state::snapshots::*;

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
        self.0.insert(a, v);
    }
}

fn le32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

/// Compares a snapshot with direct reads of the raw region bytes; returns the first mismatch.
fn compare(
    snap: &HudSnapshot,
    level: &[u8],
    counts: &[u8],
    globals: &[u8],
    record: Option<&[u8]>,
) -> Option<&'static str> {
    if snap.level != le32(level, 0) {
        return Some("level");
    }
    if snap.players != le32(counts, 0x10) {
        return Some("players");
    }
    let rec = le32(globals, 0x7c);
    let ctrl = le32(globals, 0x48);
    if snap.viewer_record != rec {
        return Some("viewer_record");
    }
    if snap.controlled != ctrl {
        return Some("controlled");
    }
    let want_state = record
        .filter(|_| ctrl == rec && rec != 0)
        .map(|r| le32(r, 0x114));
    if want_state.is_some() && snap.viewer_state != want_state {
        return Some("viewer_state");
    }
    if snap.cell("player_global_7b0cc0") != Some(le32(globals, 0xc0)) {
        return Some("player_global_7b0cc0");
    }
    // flags (0x7DE458) is not in the captured regions: read 0, so the gate reduces to level/state.
    let want_gate = !((le32(level, 0) as i32) <= 0x64 && want_state == Some(0x100));
    if want_state.is_some() && snap.prompt_allowed != want_gate {
        return Some("prompt_allowed");
    }
    None
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let dir = args.get(1).expect("usage: snapshot_check <dir> [--mutate]");
    let mutate = args.iter().any(|a| a == "--mutate");
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".addrs").map(str::to_owned))
        .collect();
    names.sort();
    let (mut equal, mut bad, mut with_record, mut gate_false, mut states) =
        (0, 0, 0, 0, BTreeMap::new());
    for name in &names {
        let read = |region: &str| fs::read(format!("{dir}/{name}.{region}.bin")).unwrap();
        let (level, counts, globals) = (
            read("level_state"),
            read("player_counts"),
            read("player_globals"),
        );
        let rec = le32(&globals, 0x7c);
        let record = (rec != 0).then(|| read("player_record"));
        let mut image = Sample(BTreeMap::new());
        image.put(0x79_094c, &level);
        image.put(0x79_0940, &counts);
        image.put(0x7b_0c00, &globals);
        if let Some(r) = &record {
            image.put(rec, r);
            with_record += 1;
        }
        let snap = HudSnapshot::build_from_guest(&image, None);
        if !snap.prompt_allowed {
            gate_false += 1;
        }
        if let Some(s) = snap.viewer_state {
            *states.entry(s).or_insert(0u32) += 1;
        }
        let (mut level, mut counts, mut globals) = (level, counts, globals);
        if mutate {
            // The consumer's view changes after the push: the snapshot must now disagree.
            match name.len() % 3 {
                0 => level[0] ^= 1,
                1 => counts[0x10] ^= 1,
                _ => globals[0xc0] ^= 1,
            }
        }
        match compare(&snap, &level, &counts, &globals, record.as_deref()) {
            None => equal += 1,
            Some(field) => {
                bad += 1;
                if !mutate {
                    println!("{name}: mismatch in {field}");
                }
            }
        }
    }
    println!(
        "samples {} equal {equal} mismatched {bad} with_record {with_record} prompt_gate_false {gate_false} viewer_states {states:?}",
        names.len()
    );
    let ok = if mutate {
        bad == names.len() && !names.is_empty()
    } else {
        bad == 0 && !names.is_empty()
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}
