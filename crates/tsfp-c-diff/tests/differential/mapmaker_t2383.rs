//! T2383 Tier 1: REA Map Maker range encoding and reversible apply against lifted bodies.
#![cfg(modes_lifted)]

use super::*;
use tsfp_engine_fileio::{
    map_history::{UndoRecord, difference_ranges},
    storage_flow::MAP_PAYLOAD_BYTES,
};

unsafe extern "C" {
    fn md_run(va: u32, regs: *mut u32) -> i32;
    fn md_script(words: *const u32, entries: u32);
    fn shim_get_extra(extra: *mut u32);
    fn shim_set_extra(extra: *const u32);
}

// Each padded snapshot occupies 0x7fe0 bytes. Keep all buffers and metadata in separate
// 64-KiB windows so the differential harness never aliases a source, destination or descriptor.
const INPUT: u32 = 0x0050_0000;
const BASELINE: u32 = 0x0051_0000;
const RANGE_TABLE: u32 = 0x0052_0000;
const NODE: u32 = 0x0053_0000;
const RECORDS: u32 = 0x0054_0000;
const PAYLOAD: u32 = 0x0055_0000;
const WORKING: u32 = 0x0056_0000;

fn get32(pair: &Pair, address: u32) -> u32 {
    u32::from_le_bytes(std::array::from_fn(|i| pair.get(false, address + i as u32)))
}

fn mirror_lifted_writes(pair: &mut Pair, start: u32, length: usize) {
    for offset in 0..length {
        let address = start + offset as u32;
        let value = pair.get(false, address);
        unsafe { *pair.rust.add(address as usize) = value };
    }
}

fn run_lifted(va: u32, eax: u32, esi: u32) -> [u32; 8] {
    let mut regs = [0; 8];
    regs[0] = eax;
    regs[4] = esi;
    regs[7] = STACK;
    assert_eq!(
        unsafe { md_run(va, regs.as_mut_ptr()) },
        1,
        "unknown lifted root {va:#x}"
    );
    regs
}

#[test]
fn t1_difference_grouping_matches_lifted_3367a0_for_2048_seeds() {
    let mut pair = pair().lock().unwrap_or_else(|e| e.into_inner());
    let pair = &mut *pair;
    let mut rng = Rng(0x2383_3367);
    for seed in 0..2048 {
        let mut current = vec![0; MAP_PAYLOAD_BYTES];
        let mut baseline = vec![0; MAP_PAYLOAD_BYTES];
        current[..128].copy_from_slice(&(0..128).map(|_| rng.next() as u8).collect::<Vec<_>>());
        baseline[..128].copy_from_slice(&(0..128).map(|_| rng.next() as u8).collect::<Vec<_>>());
        let current_len = 1 + rng.below(128) as usize;
        let baseline_len = 1 + rng.below(128) as usize;
        for (index, value) in current.iter().copied().enumerate() {
            pair.put8(INPUT + index as u32, value);
        }
        for (index, value) in baseline.iter().copied().enumerate() {
            pair.put8(BASELINE + index as u32, value);
        }
        for index in 0..128u32 {
            pair.put32(RANGE_TABLE + index * 8, u32::MAX);
            pair.put32(RANGE_TABLE + index * 8 + 4, 0);
        }
        pair.put32(STACK + 4, INPUT);
        pair.put32(STACK + 8, BASELINE);
        pair.put32(STACK + 12, current_len as u32);
        pair.put32(STACK + 16, baseline_len as u32);
        unsafe { md_script(std::ptr::null(), 0) };
        let regs = run_lifted(0x3367A0, 0, RANGE_TABLE);
        let count = regs[0] as usize;
        let actual: Vec<_> = (0..count)
            .map(|index| {
                (
                    get32(pair, RANGE_TABLE + index as u32 * 8) as i32 as usize,
                    get32(pair, RANGE_TABLE + index as u32 * 8 + 4) as usize,
                )
            })
            .collect();
        let expected = difference_ranges(&current, &baseline, current_len, baseline_len)
            .expect("valid padded snapshots");
        assert_eq!(actual, expected, "REA 3367A0 seed {seed}");
        assert!(count <= 128, "range output stays inside initialized table");
    }
}

#[test]
fn t1_reversible_apply_matches_lifted_3366f0_for_2048_seeds() {
    let mut pair = pair().lock().unwrap_or_else(|e| e.into_inner());
    let pair = &mut *pair;
    let prior_baseline = get32(pair, 0x0076_5e38);
    let mut prior_registers = [0; 4];
    unsafe { shim_get_extra(prior_registers.as_mut_ptr()) };
    let mut rng = Rng(0x2383_3366);
    for seed in 0..2048 {
        let mut previous = vec![0; MAP_PAYLOAD_BYTES];
        let mut current = vec![0; MAP_PAYLOAD_BYTES];
        for index in 0..160 {
            previous[index] = rng.next() as u8;
            current[index] = previous[index];
        }
        for _ in 0..1 + rng.below(5) {
            let at = rng.below(160) as usize;
            current[at] ^= (rng.next() as u8) | 1;
        }
        let old_len = 80 + rng.below(81) as usize;
        let new_len = 80 + rng.below(81) as usize;
        // This helper is byte-range history only; the REA differential owns its opaque logical
        // lengths and padded physical buffers separately from the native document codec.
        let mut record = UndoRecord::between(&current, &previous, new_len, old_len)
            .expect("valid 0x7fe0 snapshots");
        if record.ranges.is_empty() {
            // Equal logical/physical content is a valid no-op; change one byte to exercise apply.
            current[7] ^= 0x55;
            record = UndoRecord::between(&current, &previous, new_len, old_len)
                .expect("valid changed snapshot");
        }

        pair.put32(0x0076_5e38, BASELINE);
        for index in 0..MAP_PAYLOAD_BYTES {
            pair.put8(WORKING + index as u32, current[index]);
            pair.put8(BASELINE + index as u32, current[index]);
        }
        for (index, value) in record.payload.iter().copied().enumerate() {
            pair.put8(PAYLOAD + index as u32, value);
        }
        pair.put32(NODE + 4, record.ranges.len() as u32);
        pair.put32(NODE + 12, PAYLOAD);
        pair.put32(NODE + 20, RECORDS);
        for (index, range) in record.ranges.iter().enumerate() {
            pair.put32(RECORDS + index as u32 * 8, range.offset as u32);
            pair.put32(RECORDS + index as u32 * 8 + 4, range.length as u32);
        }
        pair.put32(STACK + 4, WORKING);
        assert_eq!(get32(pair, 0x0076_5e38), BASELINE);
        assert_eq!(
            pair.get(false, WORKING + 32),
            current[32],
            "working setup seed {seed}"
        );
        assert_eq!(
            pair.get(false, BASELINE + 32),
            current[32],
            "baseline setup seed {seed}"
        );
        unsafe { md_script(std::ptr::null(), 0) };
        let regs = run_lifted(0x3366F0, NODE, 0);

        let lifted_ranges: Vec<_> = (0..record.ranges.len())
            .map(|index| {
                (
                    get32(pair, RECORDS + index as u32 * 8) as usize,
                    get32(pair, RECORDS + index as u32 * 8 + 4) as usize,
                )
            })
            .collect();
        assert_eq!(
            lifted_ranges,
            record
                .ranges
                .iter()
                .map(|range| (range.offset, range.length))
                .collect::<Vec<_>>(),
            "descriptor seed {seed}"
        );

        let mut expected_working = current.clone();
        record
            .apply(&mut expected_working, &current)
            .expect("valid owned undo record");
        for (index, expected) in expected_working.iter().copied().enumerate() {
            assert_eq!(
                pair.get(false, WORKING + index as u32),
                expected,
                "working seed {seed} byte {index}, ranges {:?}",
                record.ranges,
            );
        }
        for (index, expected) in record.payload.iter().copied().enumerate() {
            assert_eq!(
                pair.get(false, PAYLOAD + index as u32),
                expected,
                "reverse payload seed {seed} byte {index}"
            );
        }
        // The lifted helper tail-returns the terminator lookup pointer. The isolated harness
        // stubs the chunk finder, so zero is expected here and is not treated as apply success.
        assert_eq!(regs[0], 0, "lookup stub result, not a decode-success flag");
    }
    // The pair is shared with unrelated lifted differential tests. Restore this process-global
    // pointer and mirror the helper's C-arena outputs into the Rust arena so later roots do not
    // inherit one-sided Map Maker fixture writes.
    mirror_lifted_writes(pair, WORKING, MAP_PAYLOAD_BYTES);
    mirror_lifted_writes(pair, PAYLOAD, MAP_PAYLOAD_BYTES);
    mirror_lifted_writes(pair, STACK - 0x100, 0x200);
    pair.put32(0x0076_5e38, prior_baseline);
    unsafe { shim_set_extra(prior_registers.as_ptr()) };
}
