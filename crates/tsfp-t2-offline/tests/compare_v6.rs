// SPDX-License-Identifier: GPL-3.0-or-later
//! compare v6 (2026-10-26, INFERRED), port side only:
//! - an `fpo_leaf` root pushes EBP as a callee argument: the lifted body holds the entry
//!   `seh_ebp` there (0x15E830: 0x164010 arg 0, 0x15D1F0 arg 3), the port the real entry EBP;
//! - a callee the port runs natively (rule 20, 0x10D070 calls the RNG port 0x153BD0 directly) is
//!   dropped from the sequence when declared for the root (`--inlined-callees`).
//!
//! - compare v7 (2026-10-26, INFERRED): a callee argument word holding the g_ebp an EARLIER
//!   callee leaked (`compare::leaked_ebp`) is accepted where the port passes the entry EBP, for
//!   any root (the tests below the `v7` marker).
//!
//! Every accepted case has a near miss that still diverges.
mod common;

use common::*;
use tsfp_port_runtime::abi::{ExtRegisters, GuestCall};
use tsfp_port_runtime::memory::GuestMemory;
use tsfp_t2_offline::arena::{Arena, GuestView};
use tsfp_t2_offline::compare::RetDead;
use tsfp_t2_offline::spool::Entry;
use tsfp_t2_offline::stub::{CalleeStub, Convention, Inlined};
use tsfp_t2_offline::{EntryResult, Executor, Options, Verdict, run_entry};

const VA_ROOT: u32 = 0x7000;
const RNG: u32 = 0x5100;
const DRAW: u32 = 0x5200;
const OTHER: u32 = 0x5300;
const SEED: u32 = 0x0050_0000;
const SEED_NEXT: u32 = 0x1357_9bdf;
const SEH: u32 = 0x4015_fce8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Port {
    /// Pushes the real entry EBP before DRAW (rule 2 cases).
    PushesEbp,
    /// Pushes EBP + 4: a genuinely wrong argument.
    PushesWrongEbp,
    /// Computes RNG natively (writes the seed), then calls DRAW.
    InlinesRng,
    /// Calls RNG through the guest like the original, then DRAW.
    CallsRng,
    /// Native RNG that forgets the seed write.
    InlinesRngNoWrite,
    /// Native RNG, then calls OTHER instead of DRAW.
    InlinesRngCallsOther,
    /// Native RNG and no DRAW call at all.
    InlinesRngSkipsDraw,
    /// Calls RNG through the guest, then pushes EBP + 4 before DRAW.
    CallsRngPushesWrongEbp,
}

fn view(base: usize) -> GuestView {
    unsafe { GuestView::new(base) }
}

/// Lifted style (return PC pushed): optionally `push arg; call RNG`, then `push ebp_value; call
/// DRAW`; returns DRAW's EAX.
fn reference(
    base: usize,
    r: &mut ExtRegisters,
    rng: bool,
    ebp_value: u32,
    call: &mut dyn FnMut(u32, &mut ExtRegisters),
) {
    let mut m = view(base);
    if rng {
        r.esp -= 4;
        m.write32(r.esp, SEED);
        r.esp -= 4;
        m.write32(r.esp, 0x7010);
        call(RNG, r);
        r.esp += 4;
    }
    r.esp -= 4;
    m.write32(r.esp, ebp_value);
    r.esp -= 4;
    m.write32(r.esp, 0x7020);
    call(DRAW, r);
    r.esp += 4;
    r.esp += 4; // ret
}

fn port(base: usize, r: &mut ExtRegisters, guest: &mut dyn GuestCall, mode: Port) {
    let mut m = view(base);
    let entry_ebp = r.ebp;
    match mode {
        Port::InlinesRng | Port::InlinesRngCallsOther | Port::InlinesRngSkipsDraw => {
            m.write32(SEED, SEED_NEXT)
        }
        Port::CallsRng | Port::CallsRngPushesWrongEbp => {
            r.esp -= 4;
            m.write32(r.esp, SEED);
            guest.call(RNG, r);
            r.esp += 4;
        }
        _ => {}
    }
    if mode == Port::InlinesRngSkipsDraw {
        r.eax = 9;
        r.esp += 4;
        return;
    }
    r.esp -= 4;
    let pushed = if matches!(mode, Port::PushesWrongEbp | Port::CallsRngPushesWrongEbp) {
        entry_ebp + 4
    } else {
        entry_ebp
    };
    m.write32(r.esp, pushed);
    let target = if mode == Port::InlinesRngCallsOther {
        OTHER
    } else {
        DRAW
    };
    guest.call(target, r);
    r.esp += 4;
    r.esp += 4;
}

struct Fake {
    mode: Port,
    /// What the lifted control pushes for EBP (SEH like the capture, unless a test changes it).
    lifted_push: u32,
    rng: bool,
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
        let rng = self.rng;
        reference(self.base, regs, rng, self.lifted_push, &mut |va, r| {
            stub.dispatch(va, r, Convention::ReturnPushed)
        });
    }
    fn run_port(&mut self, _va: u32, regs: &mut ExtRegisters, stub: &mut CalleeStub<'_>) {
        port(self.base, regs, stub, self.mode);
    }
}

/// A capture of the lifted reference: it pushes the entry seh_ebp (SEH) where hardware pushes EBP.
fn captured(arena: &mut Arena, rng: bool) -> Entry {
    let reads = vec![words(ESP, &[RETURN_PC]), words(SEED, &[0])];
    let mut outcomes = Vec::new();
    if rng {
        outcomes.push(Outcome {
            eax: 0,
            writes: vec![words(SEED, &[SEED_NEXT])],
        });
    }
    outcomes.push(Outcome {
        eax: 9,
        writes: vec![],
    });
    let mut entry = capture(
        arena,
        VA_ROOT,
        regs(ESP),
        reads,
        outcomes,
        &mut |base, r, rec| {
            reference(base, r, rng, SEH, &mut |va, r| {
                rec.call(va, r, Convention::ReturnPushed)
            })
        },
    );
    let mut implicit = [0u32; 8];
    implicit[0] = SEH; // IMPLICIT_NAMES[0] = seh_ebp
    entry.entry_implicit = Some(implicit);
    entry.exit_implicit = Some(implicit);
    let draw = entry.callees.last().expect("DRAW recorded");
    assert_eq!(draw.va, DRAW);
    assert_eq!(draw.args[0], SEH, "the lifted body pushed the seh_ebp");
    assert_ne!(SEH, entry.entry_regs.ebp, "the test needs seh_ebp != EBP");
    entry
}

fn judge(entry: &Entry, mode: Port, rng: bool, options: Options) -> EntryResult {
    judge_with(entry, mode, rng, options, SEH)
}

fn judge_with(
    entry: &Entry,
    mode: Port,
    rng: bool,
    options: Options,
    lifted_push: u32,
) -> EntryResult {
    let mut arena = arena();
    let mut fake = Fake {
        mode,
        lifted_push,
        rng,
        base: 0,
    };
    fake.bind(arena.base());
    run_entry(&mut fake, &mut arena, 0, entry, VA_ROOT, options)
}

fn class(verdict: &Verdict) -> &'static str {
    match verdict {
        Verdict::Judged(j) => j.class.name(),
        Verdict::NoAbiEntry => "no_abi_entry",
        Verdict::Skipped => "skipped",
        Verdict::NotRun => "null",
    }
}

fn fpo_leaf() -> Options {
    Options {
        ret_dead: RetDead {
            fpo_leaf: true,
            ..RetDead::default()
        },
        ..Options::default()
    }
}

fn inlined(text: &str) -> Options {
    Options {
        inlined: Inlined::parse(text).expect("parse"),
        ..fpo_leaf()
    }
}

// Rule 2: the lifted seh_ebp in a callee argument word of an fpo_leaf root.

#[test]
fn fpo_leaf_root_pushing_the_entry_ebp_is_equal() {
    let mut arena = arena();
    let entry = captured(&mut arena, false);
    let result = judge(&entry, Port::PushesEbp, false, fpo_leaf());
    assert_eq!(class(&result.lifted), "equal", "{}", result.detail);
    assert_eq!(class(&result.rust), "equal", "{}", result.detail);
}

#[test]
fn the_seh_ebp_argument_is_strict_without_fpo_leaf() {
    let mut arena = arena();
    let entry = captured(&mut arena, false);
    let result = judge(&entry, Port::PushesEbp, false, Options::default());
    assert_eq!(class(&result.rust), "callee_mismatch");
    assert!(result.detail.contains("arg 0"), "{}", result.detail);
}

#[test]
fn fpo_leaf_port_pushing_neither_value_diverges() {
    let mut arena = arena();
    let entry = captured(&mut arena, false);
    let result = judge(&entry, Port::PushesWrongEbp, false, fpo_leaf());
    assert_eq!(class(&result.rust), "callee_mismatch");
    assert!(result.detail.contains("arg 0"), "{}", result.detail);
}

#[test]
fn a_captured_word_that_is_not_the_seh_ebp_stays_strict() {
    // The port pushed the entry EBP, but the capture holds another value: not the quirk.
    let mut arena = arena();
    let mut entry = captured(&mut arena, false);
    entry.callees[0].args[0] = SEH ^ 0x10;
    let result = judge(&entry, Port::PushesEbp, false, fpo_leaf());
    assert_eq!(class(&result.rust), "callee_mismatch");
}

#[test]
fn the_lifted_control_never_gets_the_seh_ebp_rule() {
    // A lifted control that pushed the real EBP where the capture holds seh_ebp is unfaithful.
    let mut arena = arena();
    let entry = captured(&mut arena, false);
    let real = entry.entry_regs.ebp;
    let result = judge_with(&entry, Port::PushesEbp, false, inlined("0x5100"), real);
    assert_eq!(
        class(&result.lifted),
        "callee_mismatch",
        "{}",
        result.detail
    );
}

// Rule 3: declared inlined callees.

#[test]
fn a_declared_native_callee_is_dropped_from_the_sequence() {
    let mut arena = arena();
    let entry = captured(&mut arena, true);
    let options = inlined(&format!("{RNG:#x}"));
    let result = judge(&entry, Port::InlinesRng, true, options);
    assert_eq!(class(&result.lifted), "equal", "{}", result.detail);
    assert_eq!(class(&result.rust), "equal", "{}", result.detail);
}

#[test]
fn a_declared_native_descendant_runs_when_not_the_next_captured_callee() {
    let mut arena = arena();
    let entry = captured(&mut arena, true);
    let mut stub = CalleeStub::new(&entry, &arena).port_rules(
        false,
        Inlined::parse(&format!("{RNG:#x},{OTHER:#x}")).expect("parse"),
    );

    assert!(!stub.should_dispatch_native(RNG));
    assert!(stub.should_dispatch_native(OTHER));
}

#[test]
fn a_declared_callee_the_port_does_call_is_still_matched() {
    let mut arena = arena();
    let entry = captured(&mut arena, true);
    let options = inlined(&format!("{RNG:#x}"));
    let result = judge(&entry, Port::CallsRng, true, options);
    assert_eq!(class(&result.rust), "equal", "{}", result.detail);
}

#[test]
fn an_undeclared_native_callee_still_diverges() {
    let mut arena = arena();
    let entry = captured(&mut arena, true);
    let result = judge(&entry, Port::InlinesRng, true, fpo_leaf());
    assert_eq!(class(&result.rust), "callee_mismatch");
    assert!(
        result.detail.contains("expected 0x5100"),
        "{}",
        result.detail
    );
    let other = inlined(&format!("{OTHER:#x}"));
    let result = judge(&entry, Port::InlinesRng, true, other);
    assert_eq!(class(&result.rust), "callee_mismatch", "{}", result.detail);
}

#[test]
fn a_dropped_callee_still_needs_its_writes() {
    let mut arena = arena();
    let entry = captured(&mut arena, true);
    let options = inlined(&format!("{RNG:#x}"));
    let result = judge(&entry, Port::InlinesRngNoWrite, true, options);
    // InlinesRngNoWrite takes the `_` arm: no seed write, no RNG call.
    assert_eq!(class(&result.rust), "write_set", "{}", result.detail);
}

#[test]
fn an_undeclared_callee_difference_after_a_dropped_one_diverges() {
    let mut arena = arena();
    let entry = captured(&mut arena, true);
    let options = inlined(&format!("{RNG:#x}"));
    let result = judge(&entry, Port::InlinesRngCallsOther, true, options);
    assert_eq!(class(&result.rust), "callee_mismatch");
    let result = judge(&entry, Port::InlinesRngSkipsDraw, true, options);
    assert_eq!(class(&result.rust), "callee_mismatch");
    assert!(
        result.detail.contains("missing call to 0x5200"),
        "{}",
        result.detail
    );
}

#[test]
fn inlined_parse_is_bounded_and_strict() {
    assert!(
        Inlined::parse("0x153bd0,0X47560")
            .expect("ok")
            .contains(0x47560)
    );
    assert!(!Inlined::parse("").expect("ok").contains(0));
    assert!(Inlined::parse("0xzz").is_err());
    let nine = (1..=9)
        .map(|i| format!("{i:#x}"))
        .collect::<Vec<_>>()
        .join(",");
    assert!(Inlined::parse(&nine).is_err());
}

// v7: a g_ebp leaked by an earlier callee in a later callee's argument window and the root exit.

const LEAK: u32 = 0x4015_ff5c;

/// The `rng` capture rewritten as if RNG leaked LEAK into the lifted body (its captured EBP
/// changed), which then pushed LEAK for DRAW and returned with it in EBP.
fn leaked_capture(arena: &mut Arena) -> Entry {
    let mut entry = captured(arena, true);
    assert_eq!(entry.callees[0].va, RNG);
    entry.callees[0].exit_regs.ebp = LEAK;
    assert_ne!(entry.callees[0].entry_regs.ebp, LEAK);
    entry.callees[1].args[0] = LEAK;
    entry.exit_regs.ebp = LEAK;
    entry
}

#[test]
fn v7_leaked_ebp_argument_and_exit_are_equal_without_fpo_leaf() {
    let mut arena = arena();
    let entry = leaked_capture(&mut arena);
    let result = judge(&entry, Port::CallsRng, true, Options::default());
    assert_eq!(class(&result.rust), "equal", "{}", result.detail);
    // The lifted control stays exact: it pushed SEH where the capture shows LEAK.
    assert_ne!(class(&result.lifted), "equal");
}

#[test]
fn v7_port_pushing_neither_value_diverges() {
    let mut arena = arena();
    let entry = leaked_capture(&mut arena);
    let result = judge(
        &entry,
        Port::CallsRngPushesWrongEbp,
        true,
        Options::default(),
    );
    assert_eq!(class(&result.rust), "callee_mismatch");
    assert!(result.detail.contains("arg 0"), "{}", result.detail);
}

#[test]
fn v7_argument_matching_no_earlier_leak_stays_strict() {
    let mut arena = arena();
    // The captured word is not the leaked value.
    let mut entry = leaked_capture(&mut arena);
    entry.callees[1].args[0] = LEAK ^ 0x10;
    let result = judge(&entry, Port::CallsRng, true, Options::default());
    assert_eq!(class(&result.rust), "callee_mismatch", "{}", result.detail);
    // No earlier callee changed EBP: the chain is missing.
    let mut entry = leaked_capture(&mut arena);
    entry.callees[0].exit_regs.ebp = entry.callees[0].entry_regs.ebp;
    let result = judge(&entry, Port::CallsRng, true, Options::default());
    assert_eq!(class(&result.rust), "callee_mismatch", "{}", result.detail);
}
