// SPDX-License-Identifier: GPL-3.0-or-later
//! T2068 replay check: loads the captured GameState regions (extracted by
//! `python3 -m tools.s3_gamestate extract`) into `GameState`, stores them into a fresh copy and
//! requires the region bytes to be EQUAL. It also replays the sample-to-sample transitions through
//! the Rust writers (`set_*`, write-through) and requires the result to equal the next capture.
//! `--mutate` flips one owned field per sample and requires DIVERGED. Usage: gamestate_check <dir> [--mutate]
use std::{collections::BTreeSet, env, fs, process::ExitCode};
use tsfp_state::GuestMemory;
use tsfp_state::game_state::*;

const REGIONS: [(&str, u32); 7] = [
    ("level_state", 0x79094c),
    ("player_counts", 0x790940),
    ("extra_players", 0x7a2950),
    ("match_state", 0x7de300),
    ("mode_flags", 0x7de400),
    ("tick_div", 0x4e8ce0),
    ("tick_scale", 0x4e7940),
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

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let dir = args
        .get(1)
        .expect("usage: gamestate_check <dir> [--mutate]");
    let mutate = args.iter().any(|a| a == "--mutate");
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".level_state.bin").map(str::to_owned))
        .collect();
    names.sort();
    let samples: Vec<Sample> = names
        .iter()
        .map(|name| Sample {
            ranges: REGIONS
                .iter()
                .map(|(r, base)| (*base, fs::read(format!("{dir}/{name}.{r}.bin")).unwrap()))
                .collect(),
        })
        .collect();
    let (mut equal, mut diverged, mut replay_equal, mut replay_bad) = (0, 0, 0, 0);
    let (mut levels, mut modes, mut players, mut flags, mut matches) = (
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
        BTreeSet::new(),
    );
    let mut prev: Option<(GameState, Sample)> = None;
    for sample in &samples {
        let mut state = GameState::load(sample);
        levels.insert(state.level);
        modes.insert(state.mode);
        players.insert(state.players);
        flags.insert(state.flags);
        matches.insert(state.match_state);
        // Writer replay: take the previous capture, apply the Rust writers for each changed cell.
        if let Some((old, old_sample)) = &prev {
            let mut m = old_sample.clone();
            let mut st = *old;
            if st.level != state.level {
                st.set_level(&mut m, state.level)
            }
            if st.players != state.players {
                st.set_players(&mut m, state.players)
            }
            if st.extra_players != state.extra_players {
                st.set_extra_players(&mut m, state.extra_players)
            }
            if st.mode != state.mode {
                st.set_mode(&mut m, state.mode)
            }
            if st.mode_sub != state.mode_sub {
                st.set_mode_sub(&mut m, state.mode_sub)
            }
            if st.flags != state.flags {
                st.set_flags(&mut m, state.flags)
            }
            if st.match_state != state.match_state {
                st.set_match_state(&mut m, state.match_state)
            }
            if st.tick_divisor != state.tick_divisor || st.tick_scale_bits != state.tick_scale_bits
            {
                st.set_tick_words(&mut m, state.tick_divisor, state.tick_scale_bits)
            }
            // The declared cells must match the new capture (other bytes are guest-owned).
            if st == state && st.diff(sample).is_none() && GameState::load(&m) == state {
                replay_equal += 1
            } else {
                replay_bad += 1
            }
        }
        prev = Some((state, sample.clone()));
        if mutate {
            state.level ^= 1;
        }
        let mut copy = sample.clone();
        state.store(&mut copy);
        if copy == *sample && state.diff(sample).is_none() {
            equal += 1
        } else {
            diverged += 1
        }
    }
    println!(
        "samples {} equal {equal} diverged {diverged} writer_replay_equal {replay_equal} writer_replay_bad {replay_bad} levels {levels:x?} modes {modes:x?} players {players:x?} flags {flags:x?} match {matches:x?}",
        samples.len()
    );
    let ok = if mutate {
        diverged == samples.len() && !samples.is_empty()
    } else {
        diverged == 0 && replay_bad == 0 && !samples.is_empty()
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}
