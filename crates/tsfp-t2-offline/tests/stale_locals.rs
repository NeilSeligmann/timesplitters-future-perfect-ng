// SPDX-License-Identifier: GPL-3.0-or-later
//! T3767: callee argument words that lie in the caller's own uninitialised locals.
//!
//! The capture records 16 stack words per callee. Below the entry ESP of the function under test
//! that window covers its own frame, and a local it had not written yet when it made the call holds
//! stale stack from an earlier call in the capture (0x63B40: `sub esp, 0x80`, then a one-argument
//! call; captured arg 2 = 0x3F800000) but 0xCD poison offline. Before the fix every such entry was
//! `callee_mismatch@0` on both sides (capture_unfaithful). The fix compares a byte below the entry
//! ESP only when the lifted control had written it before the call (or the capture read it).
mod common;

use common::*;
use tsfp_port_runtime::abi::{ExtRegisters, GuestCall};
use tsfp_port_runtime::memory::GuestMemory;
use tsfp_t2_offline::arena::{Arena, GuestView};
use tsfp_t2_offline::spool::Entry;
use tsfp_t2_offline::stub::{CalleeStub, Convention};
use tsfp_t2_offline::{EntryResult, Executor, Options, Verdict, run_entry};

const VA_LOCALS: u32 = 0x6000;
const CALLEE: u32 = 0x5000;
const OBJECT: u32 = 0x0040_0000;
const STALE: [u32; 2] = [0x3f80_0000, 0x6573_7361];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Port {
    Faithful,
    WrongArgument,
    SkipsThePush,
    ScribblesALocal,
}

fn view(base: usize) -> GuestView {
    unsafe { GuestView::new(base) }
}

/// Lifted style: `sub esp, 8; push [esp+12]; call CALLEE; add esp, 4; mov [esp], 5; ...; ret`.
/// local0 is written only AFTER the call, local1 never.
fn reference(base: usize, r: &mut ExtRegisters, call: &mut dyn FnMut(&mut ExtRegisters)) {
    let mut m = view(base);
    r.esp -= 8;
    let object = m.read32(r.esp + 12);
    r.esp -= 4;
    m.write32(r.esp, object);
    r.esp -= 4;
    m.write32(r.esp, 0x6010); // return PC
    call(r);
    r.esp += 4;
    m.write32(r.esp, 5);
    m.write32(r.esp + 16, 9); // its own argument slot (entry ESP + 8), only after the call
    r.eax = m.read32(r.esp);
    r.esp += 8;
    r.esp += 4; // ret
}

/// Port style (the bridge pushes the return slot), with seeded behaviours.
fn port(base: usize, r: &mut ExtRegisters, guest: &mut dyn GuestCall, mode: Port) {
    let mut m = view(base);
    r.esp -= 8;
    if mode == Port::ScribblesALocal {
        m.write32(r.esp + 4, 0x1234_5678); // local1: the original never wrote it
    }
    let object = m.read32(r.esp + 12);
    r.esp -= 4;
    match mode {
        Port::WrongArgument => m.write32(r.esp, object + 4),
        Port::SkipsThePush => {}
        _ => m.write32(r.esp, object),
    }
    guest.call(CALLEE, r);
    r.esp += 4;
    m.write32(r.esp, 5);
    m.write32(r.esp + 16, 9);
    r.eax = m.read32(r.esp);
    r.esp += 8;
    r.esp += 4;
}

struct Fake {
    mode: Port,
    base: usize,
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
    fn run_lifted(&mut self, _va: u32, regs: &mut ExtRegisters, stub: &mut CalleeStub<'_>) {
        reference(self.base, regs, &mut |r| {
            stub.dispatch(CALLEE, r, Convention::ReturnPushed)
        });
    }
    fn run_port(&mut self, _va: u32, regs: &mut ExtRegisters, stub: &mut CalleeStub<'_>) {
        port(self.base, regs, stub, self.mode);
    }
}

/// A real capture of the reference, then the stale locals the original stack held at the call.
fn captured(arena: &mut Arena) -> Entry {
    let reads = vec![words(ESP, &[RETURN_PC, OBJECT])];
    let outcome = Outcome {
        eax: 1,
        writes: vec![],
    };
    let mut entry = capture(
        arena,
        VA_LOCALS,
        regs(ESP),
        reads,
        vec![outcome],
        &mut |base, r, rec| {
            reference(base, r, &mut |r| {
                rec.call(CALLEE, r, Convention::ReturnPushed)
            })
        },
    );
    let args = &mut entry.callees[0].args;
    assert_eq!(args[0], OBJECT, "arg 0 is the pushed object");
    assert_eq!(
        args[3], RETURN_PC,
        "arg 3 is the function's own return slot"
    );
    args[1] = STALE[0];
    args[2] = STALE[1];
    entry
}

fn judge(entry: &Entry, mode: Port, options: Options) -> EntryResult {
    let mut arena = arena();
    let mut fake = Fake { mode, base: 0 };
    fake.bind(arena.base());
    run_entry(&mut fake, &mut arena, 0, entry, VA_LOCALS, options)
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
fn stale_locals_in_the_argument_window_do_not_make_the_capture_unfaithful() {
    let mut arena = arena();
    let entry = captured(&mut arena);
    let result = judge(&entry, Port::Faithful, Options::default());
    assert_eq!(class(&result.lifted), "equal", "{}", result.detail);
    assert_eq!(class(&result.rust), "equal", "{}", result.detail);
}

#[test]
fn a_wrong_pushed_argument_is_still_a_callee_mismatch() {
    let mut arena = arena();
    let entry = captured(&mut arena);
    let result = judge(&entry, Port::WrongArgument, Options::default());
    assert_eq!(class(&result.lifted), "equal");
    assert_eq!(class(&result.rust), "callee_mismatch");
    assert!(result.detail.contains("arg 0"), "{}", result.detail);
}

#[test]
fn a_port_that_never_writes_the_argument_is_still_a_callee_mismatch() {
    // The lifted control wrote the slot before the call, so the port is held to it.
    let mut arena = arena();
    let entry = captured(&mut arena);
    let result = judge(&entry, Port::SkipsThePush, Options::default());
    assert_eq!(class(&result.lifted), "equal");
    assert_eq!(class(&result.rust), "callee_mismatch");
    assert!(result.detail.contains("arg 0"), "{}", result.detail);
}

#[test]
fn a_port_scratch_write_to_a_local_the_original_never_wrote_is_not_compared() {
    let mut arena = arena();
    let entry = captured(&mut arena);
    let result = judge(&entry, Port::ScribblesALocal, Options::default());
    assert_eq!(class(&result.rust), "equal", "{}", result.detail);
}

#[test]
fn without_the_lifted_control_the_window_stays_strict() {
    // --rust-only: nothing tells which locals the original had written, so all are compared.
    let mut arena = arena();
    let entry = captured(&mut arena);
    let options = Options {
        lifted: false,
        ..Options::default()
    };
    let result = judge(&entry, Port::Faithful, options);
    assert_eq!(class(&result.rust), "callee_mismatch");
    assert!(result.detail.contains("arg 1"), "{}", result.detail);
}

#[test]
fn a_captured_read_below_the_entry_esp_is_compared() {
    // A callee that read the stale local made it part of the read-set: then it is known input.
    let mut arena = arena();
    let mut entry = captured(&mut arena);
    entry.read_runs.push(words(ESP - 4, &[STALE[1]]));
    assert_eq!(
        class(&judge(&entry, Port::Faithful, Options::default()).rust),
        "equal"
    );
    entry.callees[0].args[2] = STALE[1] ^ 1;
    let result = judge(&entry, Port::Faithful, Options::default());
    assert_eq!(class(&result.lifted), "callee_mismatch");
    assert!(result.detail.contains("arg 2"), "{}", result.detail);
}

#[test]
fn a_slot_above_the_entry_esp_written_only_after_the_call_is_not_compared() {
    // 0x26D20: the function writes its own argument slot (caller frame, entry ESP + 8) after the
    // call, so the slot is in its write-set, but at the call it still held the caller's value.
    let mut arena = arena();
    let mut entry = captured(&mut arena);
    assert!(
        entry.write_runs.iter().any(|w| w.addr == ESP + 8),
        "in the write-set"
    );
    entry.callees[0].args[5] = 0x0028_3ae0; // the caller's value of entry ESP + 8 at the call
    let result = judge(&entry, Port::Faithful, Options::default());
    assert_eq!(class(&result.rust), "equal", "{}", result.detail);
}
