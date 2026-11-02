// SPDX-License-Identifier: GPL-3.0-or-later
//! compare v8 (2026-10-26, T3738), port side only: a callee argument word at or above the callee's
//! proven stack arity (`--callee-arity`, `tools.t2capture.callee_arity`) is the caller's spill, not
//! an argument, and is not compared. 0x6F150 pushes ESI before the 0-argument call 0x98DA0; the
//! readable port does not spill, so that word is 0xCDCDCDCD poison on the port side.
//!
//! Fail closed: no table, a callee absent from the table, a word below the arity, and the lifted
//! control all stay strict.
mod common;

use common::*;
use tsfp_port_runtime::abi::{ExtRegisters, GuestCall};
use tsfp_port_runtime::memory::GuestMemory;
use tsfp_t2_offline::arena::{Arena, GuestView};
use tsfp_t2_offline::spool::Entry;
use tsfp_t2_offline::stub::{CalleeArity, CalleeStub, Convention};
use tsfp_t2_offline::{EntryResult, Executor, Options, Verdict, run_entry};

const VA_ROOT: u32 = 0x6000;
const CALLEE: u32 = 0x5100;
const ARG: u32 = 0x22;
const SPILL: u32 = 0x6666_0000; // the entry ESI the lifted body saves

fn view(base: usize) -> GuestView {
    unsafe { GuestView::new(base) }
}

/// Lifted style: `push esi; push ARG; call CALLEE; add esp, 4; pop esi; ret`.
fn reference(
    base: usize,
    r: &mut ExtRegisters,
    spill: u32,
    call: &mut dyn FnMut(&mut ExtRegisters),
) {
    let mut m = view(base);
    r.esp -= 4;
    m.write32(r.esp, spill);
    r.esp -= 4;
    m.write32(r.esp, ARG);
    r.esp -= 4;
    m.write32(r.esp, 0x6010);
    call(r);
    r.esp += 4; // cdecl cleanup of ARG
    r.esp += 4; // pop esi
    r.esp += 4; // ret
}

/// Readable port: reserves the same frame but never stores the spill; passes `arg`.
fn port(base: usize, r: &mut ExtRegisters, guest: &mut dyn GuestCall, arg: u32) {
    let mut m = view(base);
    r.esp -= 4; // the slot the original spills ESI into, left unwritten
    r.esp -= 4;
    m.write32(r.esp, arg);
    guest.call(CALLEE, r);
    r.esp += 4;
    r.esp += 4;
    r.esp += 4;
}

struct Fake {
    port_arg: u32,
    lifted_spill: u32,
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
        reference(self.base, regs, self.lifted_spill, &mut |r| {
            stub.dispatch(CALLEE, r, Convention::ReturnPushed)
        });
    }
    fn run_port(&mut self, _va: u32, regs: &mut ExtRegisters, stub: &mut CalleeStub<'_>) {
        port(self.base, regs, stub, self.port_arg);
    }
}

fn captured(arena: &mut Arena) -> Entry {
    let reads = vec![words(ESP, &[RETURN_PC])];
    let outcomes = vec![Outcome {
        eax: 5,
        writes: vec![],
    }];
    let entry = capture(
        arena,
        VA_ROOT,
        regs(ESP),
        reads,
        outcomes,
        &mut |base, r, rec| {
            reference(base, r, SPILL, &mut |r| {
                rec.call(CALLEE, r, Convention::ReturnPushed)
            })
        },
    );
    let call = &entry.callees[0];
    assert_eq!((call.va, call.args[0], call.args[1]), (CALLEE, ARG, SPILL));
    entry
}

fn table(text: &str) -> Option<&'static CalleeArity> {
    Some(Box::leak(Box::new(
        CalleeArity::parse(text).expect("parse"),
    )))
}

fn judge(port_arg: u32, lifted_spill: u32, arity: Option<&'static CalleeArity>) -> EntryResult {
    let mut arena = arena();
    let entry = captured(&mut arena);
    let mut fake = Fake {
        port_arg,
        lifted_spill,
        base: 0,
    };
    fake.bind(arena.base());
    let options = Options {
        arity,
        ..Options::default()
    };
    run_entry(&mut fake, &mut arena, 0, &entry, VA_ROOT, options)
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
fn v8_spill_above_the_callee_arity_is_not_compared() {
    let result = judge(ARG, SPILL, table("5100 1\n"));
    assert_eq!(class(&result.lifted), "equal", "{}", result.detail);
    assert_eq!(class(&result.rust), "equal", "{}", result.detail);
}

#[test]
fn v8_without_a_table_every_word_is_strict() {
    let result = judge(ARG, SPILL, None);
    assert_eq!(class(&result.rust), "callee_mismatch", "{}", result.detail);
    assert!(result.detail.contains("arg 1"), "{}", result.detail);
}

#[test]
fn v8_a_callee_absent_from_the_table_is_strict() {
    let result = judge(ARG, SPILL, table("5200 0\n"));
    assert_eq!(class(&result.rust), "callee_mismatch", "{}", result.detail);
    assert!(result.detail.contains("arg 1"), "{}", result.detail);
}

#[test]
fn v8_the_last_word_below_the_arity_is_still_compared() {
    // Arity 2: word 1 (the spill slot) is an argument, so the poison there diverges.
    let result = judge(ARG, SPILL, table("5100 2\n"));
    assert_eq!(class(&result.rust), "callee_mismatch", "{}", result.detail);
    assert!(result.detail.contains("arg 1"), "{}", result.detail);
}

#[test]
fn v8_a_wrong_argument_below_the_arity_diverges() {
    let result = judge(ARG + 1, SPILL, table("5100 1\n"));
    assert_eq!(class(&result.rust), "callee_mismatch", "{}", result.detail);
    assert!(result.detail.contains("arg 0"), "{}", result.detail);
}

#[test]
fn v8_the_lifted_control_never_gets_the_arity_rule() {
    // A lifted control that spills another value than the capture is unfaithful, table or not.
    let result = judge(ARG, SPILL + 4, table("5100 0\n"));
    assert_eq!(
        class(&result.lifted),
        "callee_mismatch",
        "{}",
        result.detail
    );
}

#[test]
fn v8_callee_arity_parse_is_strict() {
    let parsed = CalleeArity::parse("5100 1\n\n98da0 0\n").expect("parse");
    assert_eq!(parsed.get(0x5100), Some(1));
    assert_eq!(parsed.get(0x98da0), Some(0));
    assert_eq!(parsed.get(0x5200), None);
    assert!(CalleeArity::parse("5100\n").is_err());
    assert!(CalleeArity::parse("5100 1 2\n").is_err());
    assert!(CalleeArity::parse("zz 1\n").is_err());
    assert!(CalleeArity::parse("5100 x\n").is_err());
    assert!(CalleeArity::parse("5100 1\n5100 2\n").is_err());
}
