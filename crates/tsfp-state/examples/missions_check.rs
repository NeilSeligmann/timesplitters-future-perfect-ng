// SPDX-License-Identifier: GPL-3.0-or-later
//! T2079 replay check: loads the captured missions regions (extracted by `python3 -m tools.s3_missions
//! extract`) into `PickupCells`, `Visited`, `PickupRow` / `LinkRow` and the mode record script lists,
//! stores them into a copy and requires the bytes to be EQUAL, replays sample-to-sample changes
//! through the write-through setters, and checks the documented invariants (counts within the
//! allocation capacities, main table sorted by key). `--mutate` flips one owned value per sample and
//! requires DIVERGED.
//! Usage: missions_check <dir> [--mutate]
use std::{env, fs, process::ExitCode};
use tsfp_state::GuestMemory;
use tsfp_state::game_state::LEVEL;
use tsfp_state::missions::*;
use tsfp_state::modes::ModeRecord;

const STORY: u32 = 0x508390;
const MAIN_ROWS: usize = 256;
const LINK_ROWS: usize = 128;
const T34_ROWS: usize = 64;
const T38_ROWS: usize = 20;

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
    let ptrs = read("ptr_cells");
    let ptr = |i: usize| u32::from_le_bytes(ptrs[4 * i..4 * i + 4].try_into().unwrap());
    let mut ranges = vec![
        (MAIN_PTR, ptrs.clone()),
        (DRAW_CELLS, read("draw_cells")),
        (0x7b65bc, read("cnt_65bc")),
        (0x7b665c, read("cnt_665c")),
        (0x7b6770, read("cnt_6770")),
        (VISITED, read("visited")),
        (STORY, read("mode_story")),
        (0x790940, read("level_state")),
    ];
    for (i, region) in [
        (0, "rows_main"),
        (1, "rows_link"),
        (2, "rows_t34"),
        (3, "rows_t38"),
    ] {
        if ptr(i) != 0 {
            ranges.push((ptr(i), read(region)));
        }
    }
    Sample { ranges }
}

/// All rows the capture covers, as owned handles.
struct Rows {
    main: Vec<PickupRow>,
    link: Vec<LinkRow>,
    t34: Vec<PickupRow>,
    t38: Vec<PickupRow>,
}
fn load_rows(s: &Sample, c: &PickupCells) -> Rows {
    let rows = |p: u32, n: usize| -> Vec<PickupRow> {
        if p == 0 {
            return vec![];
        }
        (0..n as u32)
            .map(|i| PickupRow::load(s, p + i * ROW_SIZE))
            .collect()
    };
    Rows {
        main: rows(c.ptrs[0], MAIN_ROWS),
        link: if c.ptrs[1] == 0 {
            vec![]
        } else {
            (0..LINK_ROWS as u32)
                .map(|i| LinkRow::load(s, c.ptrs[1] + i * LINK_SIZE))
                .collect()
        },
        t34: rows(c.ptrs[2], T34_ROWS),
        t38: rows(c.ptrs[3], T38_ROWS),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let dir = args.get(1).expect("usage: missions_check <dir> [--mutate]");
    let mutate = args.iter().any(|a| a == "--mutate");
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter_map(|n| n.strip_suffix(".ptr_cells.bin").map(str::to_owned))
        .collect();
    names.sort();
    let (mut equal, mut diverged, mut replay_ok, mut replay_bad) = (0, 0, 0, 0);
    let (mut inv_ok, mut inv_bad, mut sorted_ok, mut sorted_bad) = (0, 0, 0, 0);
    let (mut tables_set, mut visited_set, mut rows_seen, mut changed_cells) = (0, 0, 0, 0);
    let mut levels = std::collections::BTreeSet::new();
    let mut max_counts = (0u32, 0u32, 0u32);
    let (mut draw_ok, mut draw_bad, mut cursor_moves) = (0, 0, 0);
    let mut prev_draw: Option<DrawCursor> = None;
    let mut prev: Option<(PickupCells, Visited, Rows)> = None;
    for name in &names {
        let s = load_sample(dir, name);
        levels.insert(s.read32(LEVEL));
        let mut cells = PickupCells::load(&s);
        let mut visited = Visited::load(&s);
        let mut draw = DrawCursor::load(&s);
        if let Some(p) = &prev_draw {
            let mut m = s.clone();
            let mut d = *p;
            p.store(&mut m);
            for w in 0..4 {
                d.set_word(&mut m, w, draw.words[w]);
            }
            if d == draw && draw.diff(&m).is_none() {
                draw_ok += 1
            } else {
                draw_bad += 1
            }
            if draw.cursor() != p.cursor() {
                cursor_moves += 1
            }
        }
        prev_draw = Some(draw);
        let mut rows = load_rows(&s, &cells);
        let rec = ModeRecord::load(&s, STORY);
        let _ = rec.rules();
        if cells.ptrs[0] != 0 {
            tables_set += 1;
        }
        if visited.bytes.iter().any(|b| *b != 0) {
            visited_set += 1;
        }
        rows_seen += rows.main.iter().filter(|r| r.words != [0; 8]).count();
        max_counts = (
            max_counts.0.max(cells.main_count),
            max_counts.1.max(cells.link_count),
            max_counts.2.max(cells.t34_count),
        );
        // Invariants: counts within the allocation capacities of 0x9A140 (t34 capacity depends on the level).
        let t34_cap = if s.read32(LEVEL) == 0x64 {
            T34_CAPACITY_LEVEL100
        } else {
            T34_CAPACITY
        };
        if cells.main_count <= MAIN_CAPACITY
            && cells.link_count <= LINK_CAPACITY
            && cells.t34_count <= t34_cap
        {
            inv_ok += 1
        } else {
            inv_bad += 1
        }
        // Main table sorted ascending by key over the captured prefix (0x9CC40 binary-searches it).
        let n = (cells.main_count as usize)
            .min(MAIN_ROWS)
            .min(rows.main.len());
        if rows.main[..n].windows(2).all(|w| w[0].key() <= w[1].key()) {
            sorted_ok += 1
        } else {
            sorted_bad += 1
        }
        // Replay: apply the previous sample's state, then the setters up to this sample, require EQUAL.
        if let Some((pc, pv, pr)) = &prev {
            let mut m = s.clone();
            pc.store(&mut m);
            pv.store(&mut m);
            for r in pr.main.iter().chain(&pr.t34).chain(&pr.t38) {
                r.store(&mut m);
            }
            for r in &pr.link {
                r.store(&mut m);
            }
            let mut st = *pc;
            if st != cells {
                changed_cells += 1;
            }
            for i in 0..5 {
                if st.ptrs[i] != cells.ptrs[i] {
                    st.set_ptr(&mut m, i, cells.ptrs[i]);
                }
            }
            st.set_main_count(&mut m, cells.main_count);
            st.set_link_count(&mut m, cells.link_count);
            st.set_t34_count(&mut m, cells.t34_count);
            for i in 0..8 {
                st.set_aux(&mut m, i, cells.aux[i]);
            }
            let mut sv = *pv;
            for i in 0..VISITED_SIZE {
                sv.mark(&mut m, i, visited.bytes[i]);
            }
            // Rows whose base is unchanged replay word by word; moved tables are compared as loaded.
            let mut rows_ok = true;
            for (old, new) in pr
                .main
                .iter()
                .zip(&rows.main)
                .chain(pr.t34.iter().zip(&rows.t34))
                .chain(pr.t38.iter().zip(&rows.t38))
            {
                if old.base == new.base {
                    let mut o = *old;
                    for w in 0..8 {
                        if o.words[w] != new.words[w] {
                            o.set_word(&mut m, w, new.words[w]);
                        }
                    }
                    rows_ok &= o == *new;
                }
            }
            for (old, new) in pr.link.iter().zip(&rows.link) {
                if old.base == new.base {
                    let mut o = *old;
                    for w in 0..5 {
                        if o.words[w] != new.words[w] {
                            o.set_word(&mut m, w, new.words[w]);
                        }
                    }
                    rows_ok &= o == *new;
                }
            }
            if st == cells
                && sv == visited
                && rows_ok
                && cells.diff(&m).is_none()
                && visited.diff(&m).is_none()
            {
                replay_ok += 1
            } else {
                replay_bad += 1
            }
        }
        prev = Some((cells, visited, load_rows(&s, &cells)));
        if mutate {
            let k = name.len();
            match k % 3 {
                0 => cells.aux[k % 8] ^= 1,
                1 => cells.main_count ^= 1,
                _ => visited.bytes[k % VISITED_SIZE] ^= 1,
            }
            if let Some(r) = rows.main.get_mut(k % MAIN_ROWS) {
                r.words[k % 8] ^= 1;
            }
        }
        if mutate {
            draw.words[name.len() % 4] ^= 1;
        }
        let mut out = s.clone();
        draw.store(&mut out);
        cells.store(&mut out);
        visited.store(&mut out);
        for r in rows.main.iter().chain(&rows.t34).chain(&rows.t38) {
            r.store(&mut out);
        }
        for r in &rows.link {
            r.store(&mut out);
        }
        if out == s
            && draw.diff(&s).is_none()
            && cells.diff(&s).is_none()
            && visited.diff(&s).is_none()
        {
            equal += 1
        } else {
            diverged += 1
        }
    }
    let n = names.len();
    println!(
        "samples {n} equal {equal} diverged {diverged} replay_equal {replay_ok} replay_bad {replay_bad} draw_replay_equal {draw_ok} draw_replay_bad {draw_bad} cursor_moves {cursor_moves} invariants_ok {inv_ok} invariants_bad {inv_bad} main_sorted {sorted_ok} main_unsorted {sorted_bad} tables_allocated {tables_set} visited_nonzero {visited_set} cell_changes {changed_cells} main_rows_nonzero_total {rows_seen} max_counts(main,link,t34) {max_counts:?} levels {levels:x?}"
    );
    let ok = if mutate {
        diverged == n && n > 0
    } else {
        diverged == 0 && replay_bad == 0 && draw_bad == 0 && inv_bad == 0 && n > 0
    };
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}
