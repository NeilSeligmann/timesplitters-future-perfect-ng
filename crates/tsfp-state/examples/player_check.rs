// SPDX-License-Identifier: GPL-3.0-or-later
//! T2073 replay check: loads captured player regions (extracted by `python3 -m tools.s3_player
//! extract`) into `PlayerSet`, stores them back into a fresh copy of the sample and requires the
//! declared bytes to be EQUAL, plus the aliasing invariant (controlled object bytes equal the record
//! prefix when both pointers match). `--mutate` flips one written field per sample (rotating over the
//! T2057 table) and requires DIVERGED. Usage: player_check <dir> [--mutate]
use std::{collections::BTreeMap, env, fs, process::ExitCode};
use tsfp_state::GuestMemory;
use tsfp_state::player::*;

const CSV: &str = include_str!("../../../docs/data/t2057-write-attribution.csv");

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

fn fields(stats: bool) -> Vec<u32> {
    let mut v: Vec<u32> = CSV
        .lines()
        .skip(1)
        .filter_map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            ((c[2] == "player" && !stats) || (c[2].contains("stats") && stats))
                .then(|| u32::from_str_radix(c[3].trim_start_matches("0x"), 16).unwrap())
        })
        .collect();
    v.sort();
    v.dedup();
    v
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let dir = args.get(1).expect("usage: player_check <dir> [--mutate]");
    let mutate = args.iter().any(|a| a == "--mutate");
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".addrs").map(str::to_owned))
        .collect();
    names.sort();
    let (rec_fields, stat_fields) = (fields(false), fields(true));
    let (mut equal, mut diverged, mut alias_bad, mut with_record, mut with_stats, mut aliased) =
        (0, 0, 0, 0, 0, 0);
    let (mut rec_flips, mut stat_flips) = (0, 0);
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
        let globals = read("player_globals");
        let globals_base = 0x7b_0c00;
        let get32 = |a: u32| {
            u32::from_le_bytes(
                globals[(a - globals_base) as usize..][..4]
                    .try_into()
                    .unwrap(),
            )
        };
        let (rec, ctrl) = (get32(RECORD_CELL), get32(CONTROLLED_CELL));
        let stats = addrs["player_stats"];
        let mut image = Sample(BTreeMap::new());
        image.put(globals_base, &globals);
        // The stats cell lives outside the captured globals: the capture region header gives its value.
        image.put(STATS_CELL, &stats.to_le_bytes());
        if rec != 0 {
            assert_eq!(
                addrs["player_record"], rec,
                "{name}: record range follows the cell"
            );
            image.put(rec, &read("player_record"));
            with_record += 1;
        }
        if stats != 0 {
            image.put(stats, &read("player_stats"));
            with_stats += 1;
        }
        if rec != 0 && rec == ctrl {
            aliased += 1;
            let body = read("player_record");
            if body[..CONTROLLED_SIZE as usize] != read("controlled_object")[..] {
                alias_bad += 1;
            }
        }
        let mut set = PlayerSet::load(&image);
        if mutate {
            // Flip one written field the way a wrong Rust writer would, rotating over the table.
            let total = rec_fields.len() + stat_fields.len();
            let k = n % total;
            if k < rec_fields.len() && rec != 0 {
                set.record[(rec_fields[k] / 4) as usize] ^= 1;
                rec_flips += 1;
            } else if stats != 0 && k >= rec_fields.len() {
                set.stats[(stat_fields[k - rec_fields.len()] / 4) as usize] ^= 1;
                stat_flips += 1;
            } else {
                set.record_ptr ^= 0x10; // no record in this phase: corrupt the cell instead
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
        "samples {} equal {equal} diverged {diverged} with_record {with_record} with_stats {with_stats} aliased_samples {aliased} alias_violations {alias_bad} record_flips {rec_flips} stats_flips {stat_flips}",
        names.len()
    );
    let ok = if mutate {
        diverged == names.len() && !names.is_empty()
    } else {
        diverged == 0 && alias_bad == 0 && !names.is_empty()
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}
