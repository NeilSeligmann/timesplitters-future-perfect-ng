// SPDX-License-Identifier: GPL-3.0-or-later
//! T2062 replay check: loads captured save and text regions (extracted by
//! `python3 -m tools.s3_save_text extract`) into `TextBank` and `SaveSlots`, stores them back into a
//! fresh copy of the sample and requires the declared bytes to be EQUAL, plus the allocation-record
//! back-link invariant that `SaveSlots::attach` maintains. `--mutate` flips one owned field per
//! sample and requires DIVERGED. Usage: save_text_check <dir> [--mutate]
use std::{env, fs, process::ExitCode};
use tsfp_state::GuestMemory;
use tsfp_state::save_text::*;

/// Sparse guest image over the three captured ranges.
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
        match self.find(a) {
            Some(i) => self.ranges[i].1[(a - self.ranges[i].0) as usize],
            None => 0, // outside the capture: records pointed to by heads may lie elsewhere
        }
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
        .expect("usage: save_text_check <dir> [--mutate]");
    let mutate = args.iter().any(|a| a == "--mutate");
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".voice_banks.bin").map(str::to_owned))
        .collect();
    names.sort();
    let (mut equal, mut diverged, mut link_bad, mut active_total, mut banks_nonzero) =
        (0, 0, 0, 0, 0);
    for name in &names {
        let read = |region: &str| fs::read(format!("{dir}/{name}.{region}.bin")).unwrap();
        let sample = Sample {
            ranges: vec![
                (VOICE_BANK_BASE, read("voice_banks")),
                (SLOT_TABLE, read("slot_table")),
                (0x7b_c9c0, read("alloc_records")),
            ],
        };
        let mut text = TextBank::load(&sample);
        let mut slots = SaveSlots::load(&sample);
        banks_nonzero += (text != TextBank::EMPTY) as u32;
        if mutate {
            // Flip one owned field the way a wrong Rust writer would.
            if text != TextBank::EMPTY {
                text.banks[0].count ^= 1;
            } else {
                slots.heads[0] ^= 0x40;
            }
        }
        // Store into a fresh copy and compare against the sample.
        let mut copy = Sample {
            ranges: sample.ranges.clone(),
        };
        text.store(&mut copy);
        slots.store(&mut copy);
        let same = copy.ranges == sample.ranges;
        let coherent = text.diff(&sample).is_none() && slots.diff(&sample).is_none();
        if same && coherent {
            equal += 1
        } else {
            diverged += 1
        }
        for i in 0..SLOT_COUNT {
            if slots.is_active(i) {
                active_total += 1;
                // back-link invariant of attach: record +0x3c points at the slot cell
                if sample.read32(slots.heads[i] + RECORD_BACK_LINK) != slot_address(i) {
                    link_bad += 1;
                }
            }
        }
    }
    println!(
        "samples {} equal {equal} diverged {diverged} active_slot_observations {active_total} back_link_violations {link_bad} samples_with_voice_banks {banks_nonzero}",
        names.len()
    );
    let ok = if mutate {
        diverged == names.len() && !names.is_empty()
    } else {
        diverged == 0 && link_bad == 0 && !names.is_empty()
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}
