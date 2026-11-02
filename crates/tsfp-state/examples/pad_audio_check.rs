// SPDX-License-Identifier: GPL-3.0-or-later
//! T2081 replay check: loads captured pad and audio ramp regions (extracted by `python3 -m
//! tools.s3_pad_audio extract`) into `PadState` and `AudioRamp`, stores them back and requires the
//! owned bytes to be EQUAL. A second pass applies the changed words between consecutive samples
//! through the write-through setters and requires the next sample to be reproduced. A drift pass
//! lists changed words of the input and audio blocks that no translation in crates/tsfp-ports
//! writes with a literal address. `--mutate` flips one owned word per sample and requires DIVERGED.
//! Usage: pad_audio_check <dir> [--mutate]
use std::{collections::BTreeMap, collections::BTreeSet, env, fs, process::ExitCode};
use tsfp_state::GuestMemory;
use tsfp_state::pad_audio::*;

const ENGINE_INPUT: &str = include_str!("../../tsfp-port-engine-input/src/c2r/engine_input.rs");
const AUDIO: &str = include_str!("../../tsfp-ports/src/audio.rs");
const AUDIO_REM: &str = include_str!("../../tsfp-ports/src/audio_remainder.rs");

#[derive(Clone, PartialEq)]
struct Sample(BTreeMap<u32, u8>);
impl Sample {
    fn put(&mut self, base: u32, bytes: &[u8]) {
        for (i, b) in bytes.iter().enumerate() {
            self.0.insert(base + i as u32, *b);
        }
    }
}
impl GuestMemory for Sample {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        match self.0.get_mut(&a) {
            Some(x) => *x = v,
            None => panic!("store outside the captured extents at {a:#x}"),
        }
    }
}

/// Addresses written with a literal `write32(0x6b....` by the ports.
fn literal_writes() -> BTreeSet<u32> {
    let mut set = BTreeSet::new();
    for src in [ENGINE_INPUT, AUDIO, AUDIO_REM] {
        for part in src.split("write32(0x6b").skip(1) {
            let hex: String = part.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
            if hex.len() == 4 {
                set.insert(0x6b_0000 + u32::from_str_radix(&hex, 16).unwrap());
            }
        }
    }
    set
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let dir = args
        .get(1)
        .expect("usage: pad_audio_check <dir> [--mutate]");
    let mutate = args.iter().any(|a| a == "--mutate");
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".pad_table.bin").map(str::to_owned))
        .collect();
    names.sort();
    let listed = literal_writes();
    let load = |name: &str| {
        let mut s = Sample(BTreeMap::new());
        for (region, base) in [
            ("pad_table", PAD_BASE),
            ("input_scalars", INPUT_BASE),
            ("audio_ramp", AUDIO_BASE),
        ] {
            let bytes = fs::read(format!("{dir}/{name}.{region}.bin")).unwrap();
            s.put(base, &bytes);
        }
        s
    };
    let (mut equal, mut diverged, mut transitions, mut reproduced, mut changed) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut changed_addrs: BTreeSet<u32> = BTreeSet::new();
    let (mut unlisted, mut unlisted_pad) = (BTreeSet::new(), BTreeSet::new());
    let mut prev: Option<(PadState, AudioRamp)> = None;
    let mut states = BTreeSet::new();
    let mut nonzero_samples = 0usize;
    for (n, name) in names.iter().enumerate() {
        let sample = load(name);
        let (mut p, mut a) = (PadState::load(&sample), AudioRamp::load(&sample));
        states.insert(p.state());
        if p.table.iter().any(|w| *w != 0) || a.words.iter().any(|w| *w != 0) {
            nonzero_samples += 1;
        }
        if let Some((pp, pa)) = &prev {
            transitions += 1;
            let mut m = sample.clone();
            let (mut sp, mut sa) = (*pp, *pa);
            sp.store(&mut m);
            sa.store(&mut m);
            for i in 0..PAD_WORDS {
                if sp.table[i] != p.table[i] {
                    let addr = PAD_BASE + 4 * i as u32;
                    sp.set_table(&mut m, addr, p.table[i]);
                    changed += 1;
                    changed_addrs.insert(addr);
                    unlisted_pad.insert(addr);
                }
            }
            for i in 0..INPUT_WORDS {
                if sp.input[i] != p.input[i] {
                    sp.set_input(&mut m, i, p.input[i]);
                    let addr = INPUT_BASE + 4 * i as u32;
                    changed += 1;
                    changed_addrs.insert(addr);
                    if !listed.contains(&addr) {
                        unlisted.insert(addr);
                    }
                }
            }
            for i in 0..AUDIO_WORDS {
                if sa.words[i] != a.words[i] {
                    let addr = AUDIO_BASE + 4 * i as u32;
                    sa.set(&mut m, addr, a.words[i]);
                    changed += 1;
                    changed_addrs.insert(addr);
                    if !listed.contains(&addr) {
                        unlisted.insert(addr);
                    }
                }
            }
            if sp == p
                && sa == a
                && sp.diff(&sample).is_none()
                && sa.diff(&sample).is_none()
                && m == sample
            {
                reproduced += 1;
            }
        }
        prev = Some((p, a));
        if mutate {
            match n % 3 {
                0 => p.table[n % PAD_WORDS] ^= 1,
                1 => p.input[n % INPUT_WORDS] ^= 1,
                _ => a.words[n % AUDIO_WORDS] ^= 1,
            }
        }
        let mut out = sample.clone();
        p.store(&mut out);
        a.store(&mut out);
        if out == sample && p.diff(&sample).is_none() && a.diff(&sample).is_none() {
            equal += 1
        } else {
            diverged += 1
        }
    }
    let n = names.len();
    println!(
        "samples {n} equal {equal} diverged {diverged} transitions {transitions} reproduced {reproduced} changed_words {changed} distinct_changed {} states {states:?} nonzero_samples {nonzero_samples}",
        changed_addrs.len()
    );
    println!(
        "unlisted_changed_input_audio_words {:x?} (not written by a literal-address translation)",
        unlisted
    );
    let pad_changed: Vec<_> = unlisted_pad.iter().collect();
    println!("changed_pad_table_words {:x?}", pad_changed);
    let ok = if mutate {
        diverged == n && n > 0
    } else {
        diverged == 0 && reproduced == transitions && n > 0
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}
