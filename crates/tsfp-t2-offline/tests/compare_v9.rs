// SPDX-License-Identifier: GPL-3.0-or-later
//! compare v9 (2026-10-27, T3832, I-T3832-1): the compared extent comes from the ORIGINAL only.
//!
//! T3818 measured 0x60B30 EQUAL while it skipped every owner-less cutscene actor and capped
//! skeletons at 40 nodes. These cases fail on compare v8: there the final bytes were compared, so a
//! port that skipped a write whose captured value equals the initial byte (a same-value rewrite,
//! which the capture records: the write-set is every byte written) read EQUAL, and the runner had
//! no notion of a port that did nothing.
mod common;

use common::*;
use tsfp_port_runtime::abi::ExtRegisters;
use tsfp_port_runtime::memory::GuestMemory;
use tsfp_t2_offline::arena::{Arena, GuestView};
use tsfp_t2_offline::spool::Entry;
use tsfp_t2_offline::stub::CalleeStub;
use tsfp_t2_offline::{EntryResult, Executor, Options, Verdict, run_entry};

const VA_ROOT: u32 = 0x60b30;
/// Two node pages: the port's capped loop stops before the second.
const PAGE_A: u32 = 0x0010_0000;
const PAGE_B: u32 = 0x0010_2000;
const OWNER: u32 = 0x0020_0000;

fn view(base: usize) -> GuestView {
    unsafe { GuestView::new(base) }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Port {
    /// Writes everything the original writes.
    Full,
    /// Writes page A only (a node loop clamped to a fixed count).
    Capped,
    /// Returns before any write when the owner argument is 0 (the 0x60B30 `live` bail-out).
    OwnerRequired,
    /// Returns at once.
    Empty,
}

struct Fake {
    base: usize,
    port: Port,
    /// Value the original stores at PAGE_A (1 = the initial value, a same-value rewrite).
    a_value: u32,
}

/// The original: whatever the owner, writes PAGE_A[0] and rewrites PAGE_B[0..2] with their values.
fn original(base: usize, regs: &mut ExtRegisters, a_value: u32) {
    let mut m = view(base);
    let _owner = m.read32(regs.esp + 4);
    m.write32(PAGE_A, a_value);
    m.write32(PAGE_B, 7);
    m.write32(PAGE_B + 4, 8);
    regs.esp += 4;
}

impl Executor for Fake {
    fn bind(&mut self, base: usize) {
        self.base = base;
    }
    fn has_port(&self, _va: u32) -> bool {
        true
    }
    fn lifted_stubs_callees(&self) -> bool {
        true
    }
    fn run_lifted(&mut self, _va: u32, regs: &mut ExtRegisters, _stub: &mut CalleeStub<'_>) {
        original(self.base, regs, self.a_value);
    }
    fn run_port(&mut self, _va: u32, regs: &mut ExtRegisters, _stub: &mut CalleeStub<'_>) {
        let mut m = view(self.base);
        let owner = m.read32(regs.esp + 4);
        match self.port {
            Port::Full => return original(self.base, regs, self.a_value),
            Port::Capped => m.write32(PAGE_A, self.a_value),
            Port::OwnerRequired if owner == 0 => {}
            Port::OwnerRequired => return original(self.base, regs, self.a_value),
            Port::Empty => {}
        }
        regs.esp += 4;
    }
}

/// The captured call: write-set = every byte the original wrote, as `src/host/t2_capture.c`.
fn entry(owner: u32, a_value: u32) -> Entry {
    let reads = vec![
        words(ESP, &[RETURN_PC, owner]),
        words(PAGE_A, &[1, 2]),
        words(PAGE_B, &[7, 8]),
    ];
    let entry_regs = regs(ESP);
    let mut exit_regs = entry_regs;
    exit_regs.esp += 4;
    Entry {
        va: VA_ROOT,
        caller_va: 0x0009_8120,
        return_address: RETURN_PC,
        poll: 1,
        guest_thread: 1,
        flags: 0,
        entry_raw: tsfp_t2_offline::spool::encode_regs(&entry_regs),
        entry_regs,
        exit_regs,
        read_runs: reads,
        write_runs: vec![words(PAGE_A, &[a_value]), words(PAGE_B, &[7, 8])],
        callees: Vec::new(),
        entry_implicit: Some([0; 8]),
        exit_implicit: Some([0; 8]),
        clock_events: None,
    }
}

fn judge(port: Port, owner: u32, a_value: u32) -> EntryResult {
    let mut arena: Arena = arena();
    let entry = entry(owner, a_value);
    let mut fake = Fake {
        base: 0,
        port,
        a_value,
    };
    fake.bind(arena.base());
    run_entry(
        &mut fake,
        &mut arena,
        0,
        &entry,
        VA_ROOT,
        Options::default(),
    )
}

fn class(verdict: &Verdict) -> &'static str {
    match verdict {
        Verdict::Judged(j) => j.class.name(),
        Verdict::NoAbiEntry => "no_abi_entry",
        Verdict::Skipped => "skipped",
        Verdict::NotRun => "null",
    }
}

#[test]
fn full_port_is_equal_and_the_control_is_faithful() {
    let result = judge(Port::Full, OWNER, 9);
    assert_eq!(class(&result.lifted), "equal", "{}", result.detail);
    assert_eq!(class(&result.rust), "equal", "{}", result.detail);
    assert!(!result.no_work);
}

#[test]
fn port_writing_fewer_bytes_than_the_original_diverges() {
    // Page B holds its captured values already: v8 compared final bytes and read EQUAL.
    let result = judge(Port::Capped, OWNER, 9);
    assert_eq!(class(&result.lifted), "equal");
    assert_eq!(class(&result.rust), "write_set", "{}", result.detail);
    assert!(result.detail.contains("missing"), "{}", result.detail);
    assert!(!result.no_work, "the capped port did write page A");
}

#[test]
fn region_comes_from_the_original_when_the_port_write_set_is_smaller() {
    // The port's own write-set is page A only; the compared extent is pages A and B.
    let result = judge(Port::Capped, OWNER, 1);
    assert_eq!(class(&result.rust), "write_set", "{}", result.detail);
    assert!(
        result
            .detail
            .starts_with(&format!("write@{PAGE_B:#010x} missing")),
        "{}",
        result.detail
    );
}

#[test]
fn early_return_port_diverges_and_is_no_work() {
    // Every captured write is a same-value rewrite: the final bytes of an empty run match.
    let result = judge(Port::Empty, OWNER, 1);
    assert_eq!(class(&result.lifted), "equal");
    assert_eq!(class(&result.rust), "write_set", "{}", result.detail);
    assert!(result.no_work);
}

#[test]
fn owner_less_call_is_compared_not_skipped() {
    let result = judge(Port::OwnerRequired, 0, 1);
    assert_eq!(class(&result.lifted), "equal");
    assert_eq!(class(&result.rust), "write_set", "{}", result.detail);
    assert!(result.no_work, "the port bailed out on owner 0");
    let line = result.json_line();
    assert!(line.contains("\"no_work\": true"), "{line}");
    // With an owner the same port is the original: equal.
    let owned = judge(Port::OwnerRequired, OWNER, 1);
    assert_eq!(class(&owned.rust), "equal", "{}", owned.detail);
    assert!(owned.json_line().contains("\"no_work\": false"));
}

#[test]
fn empty_port_on_an_entry_where_the_original_did_nothing_is_equal() {
    let mut arena: Arena = arena();
    let mut entry = entry(OWNER, 1);
    entry.write_runs.clear();
    let mut fake = Fake {
        base: 0,
        port: Port::Empty,
        a_value: 1,
    };
    fake.bind(arena.base());
    let result = run_entry(
        &mut fake,
        &mut arena,
        0,
        &entry,
        VA_ROOT,
        Options {
            lifted: false,
            ..Options::default()
        },
    );
    assert_eq!(class(&result.rust), "equal", "{}", result.detail);
    assert!(!result.no_work);
}
