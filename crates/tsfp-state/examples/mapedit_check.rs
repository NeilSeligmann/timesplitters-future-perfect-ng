// SPDX-License-Identifier: GPL-3.0-or-later
//! T2076 replay check: loads captured editor objects (`python3 -m tools.s3_mapedit extract`, files
//! `<sample>.mapedit_ptr.bin` = pointer cell, `<sample>.mapedit_obj.bin` = 0x11950 bytes behind it) into
//! `MapEdit`, stores them into a copy and requires EQUAL, then replays sample-to-sample changes through
//! the write-through setters. `--mutate` flips one object word per sample and requires DIVERGED.
//! A zero pointer is the Story-route zero state: no object, counted as absent.
//! Usage: mapedit_check <dir> [--mutate]
use std::{env, fs, process::ExitCode};
use tsfp_state::GuestMemory;
use tsfp_state::mapedit::*;

#[derive(Clone)]
struct Sample {
    base: u32,
    obj: Vec<u8>,
}
impl GuestMemory for Sample {
    fn read8(&self, a: u32) -> u8 {
        self.obj
            .get(a.wrapping_sub(self.base) as usize)
            .copied()
            .unwrap_or(0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        if let Some(b) = self.obj.get_mut(a.wrapping_sub(self.base) as usize) {
            *b = v;
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let dir = args.get(1).expect("usage: mapedit_check <dir> [--mutate]");
    let mutate = args.iter().any(|a| a == "--mutate");
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".mapedit_ptr.bin").map(str::to_owned))
        .collect();
    names.sort();
    let (mut equal, mut diverged, mut absent) = (0, 0, 0);
    let (mut replay_equal, mut replay_bad, mut changed) = (0, 0, 0);
    let mut prev: Option<(MapEdit, Sample)> = None;
    for (n, name) in names.iter().enumerate() {
        let p = fs::read(format!("{dir}/{name}.mapedit_ptr.bin")).unwrap();
        let ptr = u32::from_le_bytes([p[0], p[1], p[2], p[3]]);
        if ptr == 0 {
            absent += 1;
            prev = None;
            continue;
        }
        let obj = fs::read(format!("{dir}/{name}.mapedit_obj.bin")).unwrap();
        let sample = Sample { base: ptr, obj };
        let loaded = MapEdit::load(&sample, ptr);
        let mut o = loaded.clone();
        if mutate {
            o.words[(n * 31) % OBJECT_WORDS] ^= 1;
        }
        let mut copy = sample.clone();
        o.store(&mut copy);
        if copy.obj == sample.obj {
            equal += 1;
        } else {
            diverged += 1;
        }
        if let Some((pv, pg)) = &prev
            && pv.base == ptr
        {
            let mut cur = pv.clone();
            let mut g = pg.clone();
            let mut moved = false;
            for i in 0..OBJECT_WORDS {
                if cur.words[i] != loaded.words[i] {
                    moved = true;
                    cur.set32(&mut g, 4 * i as u32, loaded.words[i]);
                }
            }
            changed += moved as u32;
            if cur.diff(&sample).is_none() && g.obj == sample.obj {
                replay_equal += 1;
            } else {
                replay_bad += 1;
            }
        }
        prev = Some((loaded, sample));
    }
    println!(
        "samples {} absent(ptr=0) {absent} store equal {equal} diverged {diverged} replay equal {replay_equal} bad {replay_bad} changed-transitions {changed}",
        names.len()
    );
    if names.is_empty() {
        println!("no samples: not acceptance");
        return ExitCode::from(2);
    }
    if absent == names.len() {
        println!("VACUOUS: pointer 0 in every sample (zero state only), no object compared");
        return ExitCode::SUCCESS;
    }
    if mutate {
        return if diverged > 0 && equal == 0 {
            println!("DIVERGED");
            ExitCode::SUCCESS
        } else {
            println!("mutation not detected");
            ExitCode::from(1)
        };
    }
    if diverged == 0 && replay_bad == 0 {
        println!("EQUAL");
        ExitCode::SUCCESS
    } else {
        println!("NOT EQUAL");
        ExitCode::from(1)
    }
}
