// SPDX-License-Identifier: GPL-3.0-or-later
//! T3402 Tier 1: Rust dependency prefix against lifted `0x1C8540`.
//! List/name-table data and mount outcomes are FABRICATED; the original body is loaded from
//! `TSFP_LIFTED_DIR` by the c-diff build and is never copied into tracked files.
#![cfg(w4_lifted)]

use std::collections::BTreeMap;
use std::ptr;

use tsfp_engine_anim::dependency_prefix::{DependencyWorld, run_dependency_prefix};

unsafe extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
    fn shim_set_base(base: *mut u8);
    fn shim_script(words: *const u32, entries: u32);
    fn shim_script_writes(words: *const u32, entries: u32);
    fn shim_trace_len() -> u32;
    fn shim_trace_entry(index: u32) -> *const u32;
    fn w4_run(va: u32, regs: *mut u32) -> i32;
}

const ARENA_LEN: usize = (1usize << 32) + 0x1_0000;
const STACK: u32 = 0x0030_0000;
const LIST: u32 = 0x0002_0000;
const NAMES: u32 = 0x0003_0000;
const FALLBACK: u32 = 0x0004_0000;
const NAME_TABLE: u32 = 0x004f_a140;
const LEVEL: u32 = 0x0079_094c;

#[derive(Clone)]
struct Inputs {
    list: Option<Vec<u32>>,
    names: BTreeMap<u32, Vec<u8>>,
    fallback: Vec<u8>,
    append_after_mount: Option<u32>,
    rewrite_row_after_mount: Option<(i32, u32)>,
    rename_index_after_mount: Option<(u32, u32)>,
    writes: Vec<(u32, u32, u32)>,
}

impl DependencyWorld for Inputs {
    type MountResult = bool;

    fn list_present(&mut self) -> bool {
        self.list.is_some()
    }

    fn list_count(&mut self) -> i32 {
        self.list.as_ref().map_or(0, |list| list.len() as i32)
    }

    fn list_index(&mut self, row: i32) -> u32 {
        self.list.as_ref().unwrap()[row as usize]
    }

    fn name_bytes(&mut self, index: u32) -> Option<Vec<u8>> {
        self.names.get(&index).cloned()
    }

    fn fallback_name(&mut self) -> Vec<u8> {
        self.fallback.clone()
    }

    fn mount(&mut self, _resource_index: u32, _name: &[u8]) -> Self::MountResult {
        if let Some(index) = self.append_after_mount.take() {
            self.list.as_mut().unwrap().push(index);
        }
        if let Some((row, index)) = self.rewrite_row_after_mount.take() {
            self.list.as_mut().unwrap()[row as usize] = index;
        }
        if let Some((changed_index, replacement_index)) = self.rename_index_after_mount.take() {
            if let Some(name) = self.names.get(&replacement_index).cloned() {
                self.names.insert(changed_index, name);
            } else {
                self.names.remove(&changed_index);
            }
        }
        false
    }
}

fn arena() -> *mut u8 {
    let base = unsafe { mmap(ptr::null_mut(), ARENA_LEN, 3, 0x2 | 0x20 | 0x4000, -1, 0) };
    assert_ne!(base as isize, -1, "lifted guest arena mmap failed");
    unsafe { shim_set_base(base) };
    base
}

fn put32(base: *mut u8, address: u32, value: u32) {
    unsafe {
        ptr::copy_nonoverlapping(value.to_le_bytes().as_ptr(), base.add(address as usize), 4);
    }
}

fn put_cstr(base: *mut u8, address: u32, value: &[u8]) {
    let mut bytes = value.to_vec();
    bytes.push(0);
    unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), base.add(address as usize), bytes.len()) };
}

fn get_cstr(base: *mut u8, address: u32) -> Vec<u8> {
    (0..256)
        .map(|offset| unsafe { ptr::read(base.add(address as usize + offset)) })
        .take_while(|byte| *byte != 0)
        .collect()
}

fn seeded_world(seed: u32) -> Inputs {
    let mut state = seed.wrapping_add(0x1c85_4001);
    let mut next = || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        state
    };
    let count = (next() % 6) as usize;
    let list = if seed % 31 == 0 {
        None
    } else {
        Some(
            (0..count)
                .map(|_| match next() % 9 {
                    0 => 0,
                    1 => 0x38,
                    _ => 2 + next() % 12,
                })
                .collect(),
        )
    };
    let mut names = BTreeMap::new();
    for index in 0..14u32 {
        // Repeated names exercise first-occurrence, byte-exact deduplication.
        let class = (next() % 5) as u8;
        names.insert(index, format!("dep_{class}").into_bytes());
    }
    names.insert(0x38, b"dependency_marker".to_vec());
    Inputs {
        list,
        names,
        fallback: b"fallback_resource".to_vec(),
        append_after_mount: None,
        rewrite_row_after_mount: None,
        rename_index_after_mount: None,
        writes: Vec::new(),
    }
}

fn setup_guest(base: *mut u8, level: i32, inputs: &Inputs, seed: u32) {
    unsafe {
        ptr::write_bytes(base.add(NAME_TABLE as usize), 0, 0x2000);
        ptr::write_bytes(base.add(LIST as usize), 0, 0x100);
        ptr::write_bytes(base.add(NAMES as usize), 0, 0x2000);
        ptr::write_bytes(base.add(FALLBACK as usize), 0, 0x100);
        ptr::write_bytes(base.add(STACK as usize - 0x1000), 0, 0x2000);
    }
    put32(base, LEVEL, level as u32);
    if let Some(rows) = &inputs.list {
        put32(base, LIST + 4, rows.len() as u32);
        for (row, index) in rows.iter().enumerate() {
            put32(base, LIST + 8 + row as u32 * 4, *index);
        }
    }
    for (&index, name) in &inputs.names {
        let pointer = NAMES + index * 0x100;
        put32(base, NAME_TABLE + index * 0x3c, pointer);
        put_cstr(base, pointer, name);
    }
    put32(base, NAME_TABLE + 0x3c, FALLBACK);
    put_cstr(base, FALLBACK, &inputs.fallback);

    let list_pointer = if inputs.list.is_some() { LIST } else { 0 };
    let mount_return = seed & 1;
    let mut script = [[mount_return, 0, 0]; 64];
    script[0] = [list_pointer, 0, 0];
    unsafe { shim_script(script.as_ptr().cast(), script.len() as u32) };
    let writes = inputs
        .writes
        .iter()
        .flat_map(|&(call, address, value)| [call, address, value])
        .collect::<Vec<_>>();
    unsafe { shim_script_writes(writes.as_ptr(), inputs.writes.len() as u32) };
}

fn trace_mounts(base: *mut u8) -> Vec<Vec<u8>> {
    let mut mounts = Vec::new();
    for index in 0..unsafe { shim_trace_len() } {
        let row = unsafe { shim_trace_entry(index) };
        let target = unsafe { *row };
        // The prefix uses separate call sites for per-row and fixed-fallback
        // mounts. Other 0x5A130 calls in the root belong to neighboring work.
        let caller = unsafe { *row.add(2) };
        if target == 0x5a130 && matches!(caller, 0x1c8648 | 0x1c8692) {
            let path = unsafe { *row.add(3) };
            mounts.push(if path == 0 {
                Vec::new()
            } else {
                get_cstr(base, path)
            });
        }
    }
    mounts
}

fn compare_case(base: *mut u8, level: i32, inputs: Inputs, seed: u32, label: &str) {
    let mut rust_inputs = inputs.clone();
    let expected = run_dependency_prefix(level, &mut rust_inputs)
        .attempts
        .into_iter()
        .map(|attempt| attempt.name)
        .collect::<Vec<_>>();

    setup_guest(base, level, &inputs, seed);
    let mut regs = [0u32; 8];
    regs[7] = STACK;
    assert_eq!(unsafe { w4_run(0x1c8540, regs.as_mut_ptr()) }, 1, "{label}");

    assert_eq!(trace_mounts(base), expected, "{label}");
}

#[test]
fn lifted_dependency_prefix_matches_two_thousand_fabricated_lists() {
    let base = arena();
    for seed in 0..2_000u32 {
        compare_case(
            base,
            0x64,
            seeded_world(seed),
            seed,
            &format!("seed {seed}"),
        );
    }

    // Pin the source edges that random tables rarely exercise: a non-null
    // earlier index-zero name participates in duplicate checks, while a later
    // duplicate 0x38 row does not suppress fallback unless it was attempted.
    let mut zero_name = BTreeMap::new();
    zero_name.insert(0, b"shared".to_vec());
    zero_name.insert(2, b"shared".to_vec());
    compare_case(
        base,
        0x64,
        Inputs {
            list: Some(vec![0, 2]),
            names: zero_name,
            fallback: b"fallback_resource".to_vec(),
            append_after_mount: None,
            rewrite_row_after_mount: None,
            rename_index_after_mount: None,
            writes: Vec::new(),
        },
        0,
        "earlier index-zero name participates in deduplication",
    );

    let mut marker_duplicate = BTreeMap::new();
    marker_duplicate.insert(2, b"shared".to_vec());
    marker_duplicate.insert(0x38, b"shared".to_vec());
    compare_case(
        base,
        0x64,
        Inputs {
            list: Some(vec![2, 0x38]),
            names: marker_duplicate,
            fallback: b"fallback_resource".to_vec(),
            append_after_mount: None,
            rewrite_row_after_mount: None,
            rename_index_after_mount: None,
            writes: Vec::new(),
        },
        0,
        "duplicate marker row does not suppress fallback",
    );

    let mut unique_marker = BTreeMap::new();
    unique_marker.insert(0x38, b"dependency_marker".to_vec());
    compare_case(
        base,
        0x64,
        Inputs {
            list: Some(vec![0x38]),
            names: unique_marker,
            fallback: b"fallback_resource".to_vec(),
            append_after_mount: None,
            rewrite_row_after_mount: None,
            rename_index_after_mount: None,
            writes: Vec::new(),
        },
        0,
        "unique marker suppresses fallback even when mount returns false",
    );

    let mut callback_names = BTreeMap::new();
    callback_names.insert(2, b"first".to_vec());
    callback_names.insert(3, b"second".to_vec());
    callback_names.insert(0x38, b"marker".to_vec());
    compare_case(
        base,
        0x64,
        Inputs {
            list: Some(vec![2]),
            names: callback_names,
            fallback: b"fallback_resource".to_vec(),
            append_after_mount: Some(3),
            rewrite_row_after_mount: Some((0, 0x38)),
            rename_index_after_mount: None,
            // Call 0 returns the list; call 1 is the first mount callback.
            // That callback grows the list and changes its attempted row to 0x38.
            writes: vec![(1, LIST + 4, 2), (1, LIST + 8, 0x38), (1, LIST + 12, 3)],
        },
        0,
        "mount callback mutation rereads list count and attempted marker row",
    );

    let mut renamed_callback_names = BTreeMap::new();
    renamed_callback_names.insert(2, b"first".to_vec());
    renamed_callback_names.insert(3, b"second".to_vec());
    compare_case(
        base,
        0x64,
        Inputs {
            list: Some(vec![2, 3]),
            names: renamed_callback_names,
            fallback: b"fallback_resource".to_vec(),
            append_after_mount: None,
            rewrite_row_after_mount: None,
            rename_index_after_mount: Some((2, 3)),
            // The first 0x5A130 callback redirects index 2 to index 3's name.
            // The later duplicate scan must reread that earlier name pointer.
            writes: vec![(1, NAME_TABLE + 2 * 0x3c, NAMES + 3 * 0x100)],
        },
        0,
        "mount callback name-table mutation is observed by later deduplication",
    );

    compare_case(
        base,
        0x64,
        Inputs {
            list: None,
            names: BTreeMap::new(),
            fallback: b"fallback_resource".to_vec(),
            append_after_mount: None,
            rewrite_row_after_mount: None,
            rename_index_after_mount: None,
            writes: Vec::new(),
        },
        0,
        "null list bypasses fallback",
    );
    compare_case(
        base,
        0x64,
        Inputs {
            list: Some(Vec::new()),
            names: BTreeMap::new(),
            fallback: b"fallback_resource".to_vec(),
            append_after_mount: None,
            rewrite_row_after_mount: None,
            rename_index_after_mount: None,
            writes: Vec::new(),
        },
        0,
        "non-null empty list attempts fallback",
    );
    compare_case(
        base,
        0x65,
        Inputs {
            list: Some(vec![0x38]),
            names: BTreeMap::from([(0x38, b"dependency_marker".to_vec())]),
            fallback: b"fallback_resource".to_vec(),
            append_after_mount: None,
            rewrite_row_after_mount: None,
            rename_index_after_mount: None,
            writes: Vec::new(),
        },
        0,
        "level above prefix limit bypasses list and mounts",
    );
}
