// SPDX-License-Identifier: GPL-3.0-or-later
//! T2075 replay check: loads the captured mode regions (extracted by `python3 -m tools.s3_modes
//! extract`) into `ModeRecord` / `ModeSetup`, stores them into a copy and requires the bytes to be
//! EQUAL. Also (a) recomputes with `ModeSetup::resolve` (ports of 0x6A850/0x6A7B0/0x6A810) and compares with the captured cached pointer `0x6F32F0`
//! and (b) replays sample-to-sample record transitions through the write-through setters.
//! `--mutate` flips one record word per sample and requires DIVERGED.
//! Usage: modes_check <dir> [--mutate]
use std::{env, fs, process::ExitCode};
use tsfp_state::GuestMemory;
use tsfp_state::game_state::LEVEL;
use tsfp_state::modes::*;

const STORY: u32 = 0x508390;
const BSS: u32 = 0x78cc00;
const ROWS: u32 = ROW_TABLE;

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
    let cells = read("mode_cells");
    let cur = u32::from_le_bytes([cells[0], cells[1], cells[2], cells[3]]);
    let mut ranges = vec![
        (ROWS, read("mode_rows_head")),
        (CURRENT_CELL, cells),
        (0x7de490, read("mode_sel_flag")),
        (0x4d2ce0, read("mode_memo")),
        (0x7de400, read("mode_flags")),
        (STORY, read("mode_rec_story")),
        (BSS, read("mode_rec_bss")),
        (0x790940, read("level_state")),
    ];
    // The pointer-indirect region duplicates a static record when the pointer is STORY or BSS.
    if cur != 0 && cur != STORY && cur != BSS {
        ranges.push((cur, read("mode_rec_ptr")));
    }
    Sample { ranges }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let dir = args.get(1).expect("usage: modes_check <dir> [--mutate]");
    let mutate = args.iter().any(|a| a == "--mutate");
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".mode_cells.bin").map(str::to_owned))
        .collect();
    names.sort();
    let (mut equal, mut diverged, mut select_ok, mut select_bad, mut select_na) = (0, 0, 0, 0, 0);
    let (mut replay_equal, mut replay_bad, mut populated, mut cur_set) = (0, 0, 0, 0);
    let (mut memo_ok, mut memo_bad) = (0, 0);
    let mut levels = std::collections::BTreeSet::new();
    let mut prev: Option<[ModeRecord; 2]> = None;
    for name in &names {
        let sample = load_sample(dir, name);
        levels.insert(sample.read32(LEVEL));
        let mut recs = [
            ModeRecord::load(&sample, STORY),
            ModeRecord::load(&sample, BSS),
        ];
        if recs.iter().any(|r| r.is_populated()) {
            populated += 1;
        }
        let setup = ModeSetup::load(&sample);
        if setup.current != 0 {
            cur_set += 1;
        }
        // Recompute the cached pointer from the captured memo argument with the memo cells cleared
        // (forces the 0x6A850 selection path), then check the memo-hit path on the real state.
        let arg = sample.read32(MEMO_ARG);
        if arg != 0 {
            let mut m = sample.clone();
            m.write32(MEMO_ARG, 0);
            m.write32(MEMO_MODE, 0);
            let mut s2 = ModeSetup::load(&m);
            let got = s2.resolve(&mut m, arg);
            if got == setup.current {
                select_ok += 1
            } else {
                select_bad += 1
            }
            let mut hit = sample.clone();
            let mut s3 = ModeSetup::load(&hit);
            if s3.resolve(&mut hit, arg) == setup.current && hit == sample {
                memo_ok += 1
            } else {
                memo_bad += 1
            }
        } else {
            select_na += 1
        }
        // Transition replay through the setters.
        if let Some(p) = &prev {
            let mut m = sample.clone();
            for r in p {
                r.store(&mut m);
            }
            let mut st = *p;
            for (s, want) in st.iter_mut().zip(&recs) {
                for w in 0..RECORD_WORDS {
                    if s.words[w] != want.words[w] {
                        s.set_word(&mut m, w, want.words[w]);
                    }
                }
            }
            if st == recs && st.iter().all(|r| r.diff(&sample).is_none()) {
                replay_equal += 1
            } else {
                replay_bad += 1
            }
        }
        prev = Some(recs);
        if mutate {
            for (i, r) in recs.iter_mut().enumerate() {
                r.words[(name.len() + i * 7) % RECORD_WORDS] ^= 1;
            }
        }
        let mut out = sample.clone();
        for r in &recs {
            r.store(&mut out);
        }
        if out == sample && recs.iter().all(|r| r.diff(&sample).is_none()) {
            equal += 1
        } else {
            diverged += 1
        }
    }
    let n = names.len();
    println!(
        "samples {n} equal {equal} diverged {diverged} cached_ptr_set {cur_set} select_equal {select_ok} select_bad {select_bad} select_unresolved {select_na} memo_hit_equal {memo_ok} memo_hit_bad {memo_bad} writer_replay_equal {replay_equal} writer_replay_bad {replay_bad} populated_samples {populated} levels {levels:x?}"
    );
    let ok = if mutate {
        diverged == n && n > 0
    } else {
        diverged == 0 && select_bad == 0 && memo_bad == 0 && replay_bad == 0 && n > 0
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}
