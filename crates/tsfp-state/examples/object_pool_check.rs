// SPDX-License-Identifier: GPL-3.0-or-later
//! T2078 replay check: loads captured object pool regions (extracted by `python3 -m tools.s3_object_pool
//! extract`) into `ObjectPool`, stores them back into a fresh copy of the sample and requires the owned
//! bytes to be EQUAL. A second pass applies the changed dwords between consecutive samples through the
//! write-through setters and requires the next sample to be reproduced, and counts changed table cells
//! and record dwords that the T2057 table does not list (drift against real data). `--mutate` flips one
//! owned dword per sample (rotating over header, tables, scalars, records) and requires DIVERGED.
//! Usage: object_pool_check <dir> [--mutate]
use std::{collections::BTreeMap, collections::BTreeSet, env, fs, process::ExitCode};
use tsfp_state::GuestMemory;
use tsfp_state::object_pool::*;

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
        match self.0.get_mut(&a) {
            Some(x) => *x = v,
            None => panic!("store outside the captured extents at {a:#x}"),
        }
    }
}

fn csv_offsets(family: &str) -> BTreeSet<u32> {
    CSV.lines()
        .skip(1)
        .filter_map(|l| {
            let c: Vec<&str> = l.split(',').collect();
            (c[2] == family)
                .then(|| u32::from_str_radix(c[3].trim_start_matches("0x"), 16).unwrap())
        })
        .collect()
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let dir = args
        .get(1)
        .expect("usage: object_pool_check <dir> [--mutate]");
    let mutate = args.iter().any(|a| a == "--mutate");
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".addrs").map(str::to_owned))
        .collect();
    names.sort();
    let pool_listed: BTreeSet<u32> = csv_offsets("objpool");
    let obj_listed: BTreeSet<u32> = csv_offsets("object")
        .into_iter()
        .map(|o| o % RECORD_SIZE)
        .collect();
    let (mut equal, mut diverged, mut flips, mut max_used, mut max_chain) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    let (mut transitions, mut reproduced, mut changed, mut rebases) =
        (0usize, 0usize, 0usize, 0usize);
    let mut beyond_window = 0usize;
    let (mut unl_cells, mut unl_records, mut scalar_changes) = (0usize, 0usize, 0usize);
    let mut unlisted_cell_offsets = BTreeSet::new();
    let mut unlisted_record_offsets = BTreeSet::new();
    let mut prev: Option<(Sample, ObjectPool)> = None;
    for (n, name) in names.iter().enumerate() {
        let read = |region: &str| fs::read(format!("{dir}/{name}.{region}.bin")).unwrap();
        let mut image = Sample(BTreeMap::new());
        image.put(HEADER, &read("pool_hdr"));
        for (t, region) in [
            "tab_counts",
            "tab_planned",
            "tab_caps",
            "tab_members",
            "tab_aux",
        ]
        .iter()
        .enumerate()
        {
            image.put(cell_addr(t, 0), &read(region));
        }
        image.put(SCALARS[0].0, &read("arena_used"));
        image.put(0x72f5e4, &read("pool_globals"));
        image.put(SCALARS[1].0, &read("arena_base"));
        image.put(SCALARS[S_USED_RECORDS].0, &read("used_records"));
        image.put(SCALARS[9].0, &read("first_record"));
        image.put(0x7b4cb8, &read("list_cells"));
        let used = u32::from_le_bytes(read("used_records")[..4].try_into().unwrap());
        let base = u32::from_le_bytes(read("pool_globals")[4..8].try_into().unwrap());
        // Records: 16 captured (records_a + records_b); the owner mirrors min(used, 16).
        let a = read("records_a");
        let b = read("records_b");
        if base != 0 {
            image.put(base, &a);
            image.put(base + a.len() as u32, &b);
        }
        // Update list: the first node is captured (16 bytes); deeper nodes are not, so make their
        // words readable as zero without capturing them (depth-1 limit, documented).
        let node0 = read("node0");
        let head = u32::from_le_bytes(read("list_cells")[8..12].try_into().unwrap());
        if head != 0 && node0.len() >= 4 {
            image.put(head, &node0);
            let next = u32::from_le_bytes(node0[..4].try_into().unwrap());
            if next != 0 {
                image.put(next, &[0; 4]);
            }
        }
        let mut set = ObjectPool::load_limited(&image, 16);
        max_used = max_used.max(used as usize);
        max_chain = max_chain.max(set.chain_len());
        if !mutate {
            if let Some((pimage, mut pset)) = prev.take() {
                transitions += 1;
                let same_base = pset.scalars[S_RECORDS_BASE] == set.scalars[S_RECORDS_BASE];
                if !same_base {
                    rebases += 1;
                }
                let mut scratch = pimage.clone();
                for i in 0..HEADER_WORDS {
                    if pset.header[i] != set.header[i] {
                        changed += 1;
                        pset.set_header(&mut scratch, i, set.header[i]);
                    }
                }
                for t in 0..TABLE_COUNT {
                    for c in 0..CELLS {
                        if pset.tables[t][c] != set.tables[t][c] {
                            changed += 1;
                            let off = cell_addr(t, c) - HEADER;
                            if off >= 0xa80 {
                                beyond_window += 1; // T2057 only traced 0x7B54C0..0x7B5F40
                            } else if !pool_listed.contains(&off) {
                                unl_cells += 1;
                                unlisted_cell_offsets.insert(off);
                            }
                            pset.set_cell(&mut scratch, t, c, set.tables[t][c]);
                        }
                    }
                }
                for i in 0..SCALARS.len() {
                    if pset.scalars[i] != set.scalars[i] {
                        changed += 1;
                        scalar_changes += 1;
                        pset.set_scalar(&mut scratch, i, set.scalars[i]);
                    }
                }
                if same_base {
                    for r in 0..set.owned_records() {
                        for j in 0..RECORD_WORDS {
                            if pset.records[r][j] != set.records[r][j] {
                                changed += 1;
                                let off = j as u32 * 4;
                                if !obj_listed.contains(&off) {
                                    unl_records += 1;
                                    unlisted_record_offsets.insert(off);
                                }
                                pset.set_record_word(&mut scratch, r, off, set.records[r][j]);
                            }
                        }
                    }
                }
                // the update list chain is derived from the owned head: a hybrid tick re-walks it
                pset.load_chain(&image);
                if !same_base {
                    // level load moved the pool: the hybrid tick reloads the records
                    pset.load_records(&image);
                }
                let bytes_equal = pset.header == set.header
                    && pset.tables == set.tables
                    && pset.scalars == set.scalars
                    && (0..set.owned_records().min(pset.owned_records()))
                        .all(|r| pset.records[r] == set.records[r]);
                if bytes_equal && pset.diff(&image).is_none() {
                    reproduced += 1;
                }
            }
            prev = Some((image.clone(), set.clone()));
        }
        if mutate {
            // Rotate over the owned classes so every class gets flipped across the run.
            match n % 4 {
                0 => set.header[n % HEADER_WORDS] ^= 1,
                1 => set.tables[n % TABLE_COUNT][(n / 3) % CELLS] ^= 1,
                2 => set.scalars[n % SCALARS.len()] ^= 1,
                _ => {
                    if set.owned_records() > 0 {
                        let r = n % set.owned_records();
                        set.records[r][(n / 5) % RECORD_WORDS] ^= 1;
                    } else {
                        set.header[0] ^= 1;
                    }
                }
            }
            flips += 1;
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
        "samples {} equal {equal} diverged {diverged} max_used {max_used} max_chain {max_chain} flips {flips} transitions {transitions} rebases {rebases} reproduced {reproduced} changed_dwords {changed} scalar_changes {scalar_changes} unlisted_cells_in_window {unl_cells} cells_beyond_t2057_window {beyond_window} unlisted_cell_offsets {:x?} unlisted_record_dwords {unl_records} unlisted_record_offsets {:x?}",
        names.len(),
        unlisted_cell_offsets,
        unlisted_record_offsets
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
