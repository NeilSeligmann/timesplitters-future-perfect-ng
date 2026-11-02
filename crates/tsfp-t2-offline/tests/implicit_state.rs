// SPDX-License-Identifier: GPL-3.0-or-later
//! T3740 fidelity: the implicit thread state of record format 2 (`g_seh_ebp`, DF, x87 compare
//! state, flag bridge, FS base). The reference body below is shaped like a lifted `fpo_leaf`
//! function: `ebp = g_seh_ebp` (the caller's frame), a read through it, then a call to an SEH
//! prolog that publishes a new `g_seh_ebp` and leaves `g_ebp` for the caller. Before format 2 the
//! capture had no `g_seh_ebp`, the runner left it 0 and the lifted control failed on EXIT EBP:
//! 890 of 1,595 unfaithful entries in the 2026-10-25 laptop sample.
mod common;

use common::*;
use tsfp_port_runtime::abi::{ExtRegisters, GuestCall};
use tsfp_port_runtime::memory::GuestMemory;
use tsfp_t2_offline::arena::GuestView;
use tsfp_t2_offline::spool::{self, CalleeRecord, Entry, Implicit, encode_record, encode_regs};
use tsfp_t2_offline::stub::CalleeStub;
use tsfp_t2_offline::{Executor, Options, Verdict, run_entry};

const VA: u32 = 0x7000;
const PROLOG: u32 = 0x7100;
const FRAME: u32 = 0x0041_8800; // the caller's frame, inherited through g_seh_ebp
const NEW_FRAME: u32 = 0x0041_7f00; // what the SEH prolog publishes
const LOCAL: u32 = 0x1234_5678;

fn implicit(seh_ebp: u32, df: u32) -> Implicit {
    [seh_ebp, df, 0, 0, 0x41, 0x41, 0x9000, 0]
}

/// The lifted-style reference: every global it touches lives in `globals`.
fn reference(base: usize, r: &mut ExtRegisters, globals: &mut Implicit, stub: &mut CalleeStub<'_>) {
    let mut m = unsafe { GuestView::new(base) };
    let ebp = globals[0]; // ebp = g_seh_ebp; /* fpo_leaf: inherit caller's frame */
    r.esp -= 4;
    m.write32(r.esp, 0x7005); // the return PC of the call
    stub.dispatch_lifted(PROLOG, r, globals);
    r.eax = m.read32(ebp - 4);
    r.esp += 4; // ret
}

#[derive(Default)]
struct Lifted {
    base: usize,
    globals: Implicit,
    /// The pre-T3740 runner: implicit state never loaded (stays what the host left, 0).
    ignore_entry_state: bool,
}

impl Executor for Lifted {
    fn bind(&mut self, base: usize) {
        self.base = base;
    }
    fn has_port(&self, _va: u32) -> bool {
        false
    }
    fn lifted_stubs_callees(&self) -> bool {
        true
    }
    fn run_lifted(&mut self, _va: u32, regs: &mut ExtRegisters, stub: &mut CalleeStub<'_>) {
        reference(self.base, regs, &mut self.globals, stub);
    }
    fn run_port(&mut self, _va: u32, _regs: &mut ExtRegisters, _stub: &mut CalleeStub<'_>) {
        unreachable!("no port")
    }
    fn set_implicit(&mut self, implicit: &Implicit) {
        self.globals = if self.ignore_entry_state {
            [0; 8]
        } else {
            *implicit
        };
    }
    fn implicit(&self) -> Option<Implicit> {
        Some(self.globals)
    }
}

/// What the format 2 capture records for one call (built by hand from the reference semantics).
fn captured() -> Entry {
    let mut entry_regs = regs(ESP);
    entry_regs.ebp = FRAME; // g_ebp == g_seh_ebp in the host between frames
    let mut callee_entry = entry_regs;
    callee_entry.esp = ESP - 4;
    let mut callee_exit = entry_regs;
    callee_exit.ebp = NEW_FRAME; // g_ebp after the prolog returned
    let mut exit_regs = callee_exit;
    exit_regs.eax = LOCAL;
    exit_regs.esp = ESP + 4;
    Entry {
        va: VA,
        caller_va: 0x0001_0000,
        return_address: RETURN_PC,
        poll: 1,
        guest_thread: 1,
        flags: 0,
        entry_raw: encode_regs(&entry_regs),
        entry_regs,
        exit_regs,
        read_runs: vec![words(FRAME - 4, &[LOCAL])],
        write_runs: vec![],
        callees: vec![CalleeRecord {
            va: PROLOG,
            entry_regs: callee_entry,
            args: vec![],
            exit_regs: callee_exit,
            write_runs: vec![],
            exit_implicit: Some(implicit(NEW_FRAME, 1)),
        }],
        entry_implicit: Some(implicit(FRAME, 1)),
        exit_implicit: Some(implicit(NEW_FRAME, 1)),
        clock_events: None,
    }
}

#[test]
fn clock_event_spool_is_bounded_and_unreplayable_until_supported() {
    use tsfp_t2_offline::digest::input_digest;
    use tsfp_t2_offline::spool::{self, ClockEvent, Run};
    let mut entry = captured();
    let old = input_digest(&entry);
    entry.clock_events = Some(vec![ClockEvent {
        after_callees: 1,
        value: 0x1234_5678_9abc_def0,
        writes: vec![Run {
            addr: 0xfe00_0270,
            data: vec![1, 2, 3, 4],
        }],
    }]);
    let bytes = spool::encode_record(&entry);
    let decoded = spool::decode_record(&bytes).unwrap();
    assert_eq!(entry.clock_events, decoded.clock_events);
    assert_ne!(old, input_digest(&decoded));
    assert!(spool::decode_record(&bytes[..bytes.len() - 1]).is_err());
    let mut exec = Lifted::default();
    let (verdict, detail) = lifted_verdict(&mut exec, &decoded);
    assert!(
        detail.contains("clock_events_unsupported"),
        "{verdict:?} {detail}"
    );

    // Same zero-register, single-event input encoded by tools.t2corpus.keys.
    entry.entry_raw = [0; 240];
    entry.read_runs.clear();
    entry.entry_implicit = Some([0; 8]);
    entry.clock_events.as_mut().unwrap()[0].after_callees = 0;
    assert_eq!(
        input_digest(&entry),
        "60d0ea04eff74453086e0631e185f2cdaddb1ecb6c3cf536e333e4c7fa5fcb3e"
    );
}

fn lifted_verdict(exec: &mut Lifted, entry: &Entry) -> (Verdict, String) {
    let mut arena = arena();
    exec.bind(arena.base());
    let options = Options {
        lifted: true,
        rust: false,
        from: 0,
        ..Options::default()
    };
    let result = run_entry(exec, &mut arena, 0, entry, VA, options);
    (result.lifted, result.detail)
}

fn class(verdict: &Verdict) -> &'static str {
    match verdict {
        Verdict::Judged(j) => j.class.name(),
        Verdict::Skipped => "skipped",
        _ => "other",
    }
}

#[test]
fn callee_stub_exposes_captured_seh_ebp_to_the_port() {
    let entry = captured();
    let arena = arena();
    let stub = CalleeStub::new(&entry, &arena);
    assert_eq!(stub.seh_ebp(), Some(FRAME));
}

#[test]
fn clock_stub_replays_writes_and_rejects_wrong_order() {
    use tsfp_t2_offline::spool::{ClockEvent, Run};
    let mut entry = captured();
    entry.callees.clear();
    entry.clock_events = Some(vec![ClockEvent {
        after_callees: 0,
        value: 1234,
        writes: vec![Run { addr: FRAME, data: vec![7, 8, 9, 10] }],
    }]);
    let mut arena = arena();
    arena.prepare(&entry.read_runs).unwrap();
    let mut stub = CalleeStub::new(&entry, &arena);
    arena.run(&mut || {
        assert_eq!(unsafe { stub.clock_read() }, Ok(1234));
        assert_eq!(unsafe { arena.read32(FRAME) }, 0x0a09_0807);
    });
    assert!(stub.clock_finish().is_ok());

    entry.callees = captured().callees;
    entry.clock_events.as_mut().unwrap()[0].after_callees = 1;
    let mut stub = CalleeStub::new(&entry, &arena);
    arena.run(&mut || {
        assert!(unsafe { stub.clock_read() }.unwrap_err().contains("expected 1"));
    });
    assert!(stub.clock_finish().is_err());
}

#[test]
fn inherited_frame_is_restored_and_the_lifted_control_is_faithful() {
    let (verdict, detail) = lifted_verdict(&mut Lifted::default(), &captured());
    assert_eq!(class(&verdict), "equal", "{detail}");
}

#[test]
fn without_the_entry_state_the_control_fails_like_the_laptop_sample() {
    // The defect T3740 fixes: g_seh_ebp left 0, the frameless read goes through address -4.
    let mut exec = Lifted {
        ignore_entry_state: true,
        ..Lifted::default()
    };
    let (verdict, detail) = lifted_verdict(&mut exec, &captured());
    assert_ne!(class(&verdict), "equal", "{detail}");
}

#[test]
fn callee_exit_state_reaches_the_caller() {
    // The prolog's published frame is what the caller sees after the call: g_ebp (EBP) and
    // g_seh_ebp both come from the callee record, not from the caller's own frame.
    let mut entry = captured();
    entry.callees[0].exit_implicit = Some(implicit(FRAME, 1));
    let (verdict, detail) = lifted_verdict(&mut Lifted::default(), &entry);
    assert_eq!(class(&verdict), "exit_regs", "{detail}");
    assert!(detail.contains("seh_ebp"), "{detail}");

    let mut entry = captured();
    entry.callees[0].exit_regs.ebp = FRAME;
    let (verdict, detail) = lifted_verdict(&mut Lifted::default(), &entry);
    assert_eq!(class(&verdict), "exit_regs", "{detail}");
    assert!(detail.contains("ebp"), "{detail}");
}

#[test]
fn exit_implicit_state_is_compared_for_the_lifted_control() {
    let mut entry = captured();
    entry.exit_implicit = Some(implicit(NEW_FRAME, 0)); // DF
    let (verdict, detail) = lifted_verdict(&mut Lifted::default(), &entry);
    assert_eq!(class(&verdict), "exit_regs", "{detail}");
    assert!(detail.contains("df"), "{detail}");
}

#[test]
fn format_one_records_are_skipped_as_old_format_never_judged() {
    let mut old = captured();
    old.entry_implicit = None;
    old.exit_implicit = None;
    let raw = encode_record(&old);
    let decoded = spool::decode_record(&raw).unwrap();
    assert_eq!(spool::flag_names(decoded.flags), vec!["old_format"]);
    assert_eq!(decoded.entry_implicit, None);
    let (verdict, detail) = lifted_verdict(&mut Lifted::default(), &decoded);
    assert_eq!(class(&verdict), "skipped");
    assert_eq!(detail, "capture_incomplete:old_format");

    let current = spool::decode_record(&encode_record(&captured())).unwrap();
    assert_eq!(current.flags, 0);
    assert_eq!(current, captured());
}

#[test]
fn format_two_spools_from_the_python_writer_parse_with_matching_digests() {
    // tests/fixtures/python_writer_v2.spool: tools/t2corpus/spool.py encode_record of a format 2
    // record (one callee); the digest is tools/t2corpus/keys.py input_digest of it.
    let data = include_bytes!("fixtures/python_writer_v2.spool");
    let parsed = spool::parse(data).unwrap();
    let entry = parsed.records[0].as_ref().unwrap();
    assert_eq!(entry.flags, 0);
    assert_eq!(entry.entry_implicit.unwrap()[..3], [0xe880, 1, 0]);
    assert_eq!(entry.exit_implicit.unwrap()[2], 0xffff_ffff);
    assert_eq!(entry.callees[0].exit_implicit.unwrap()[0], 0x7f00);
    assert_eq!(
        tsfp_t2_offline::digest::input_digest(entry),
        "da8c99ec41ef1dbb62fb1a9f9ccdd556b53c2d2477901aa6de3b757d5dc5a18d"
    );
    assert_eq!(encode_record(entry).len(), 1145);
}
