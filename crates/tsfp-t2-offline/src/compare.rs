// SPDX-License-Identifier: GPL-3.0-or-later
//! The EQUAL rule (design section 3 plus claude-1's amendments). Any change here changes what a
//! receipt certifies, so `COMPARE_LOGIC_VERSION` must be bumped with it (T3743 folds it into the
//! receipt's `compare_logic` digest, I-T3702-8).
//!
//! An entry is EQUAL when
//! - the run returned (no fault, trap or panic);
//! - the callee sequence matched (`callee_mismatch@k` otherwise). Argument bytes count only where
//!   the capture read them or the function had written them before the call (T3767, version 4 (T3738 ret_liveness took 3):
//!   uninitialised locals and slots written after the call hold stale stack in the capture, poison
//!   offline); the port is held to the bytes the lifted control had written;
//! - the exit registers match under the mask: exact for the lifted-fidelity control; for a port
//!   EBX, ESI, EDI, EBP, ESP, EAX and EDX and, when the original returned a float (x87 TOP moved
//!   down), TOP and ST0. ECX, EFLAGS, XMM and the other x87 slots are scratch under cdecl. EAX
//!   EDX and EBP are skipped on the port side when the runner names them dead (`--ret-dead`): no
//!   caller reads them after the call (`tools.t2capture.ret_liveness`); unknown stays strict.
//!   v5 (2026-10-26, INFERRED, on top of the T3767 v4 argument rule): for a root the lifter
//!   emits as `fpo_leaf` (`--fpo-leaf`), the
//!   lifted body loads EBP from `g_seh_ebp`, so the captured exit EBP is the entry `seh_ebp`;
//!   hardware (and the port) keep the entry EBP. A port EBP equal to the entry EBP is accepted
//!   exactly when the captured exit EBP equals the entry `seh_ebp`; anything else stays strict;
//!   v6 (2026-10-26, INFERRED, `stub.rs`): for an `fpo_leaf` root a callee argument word whose
//!   captured value is the entry `seh_ebp` and whose port value is the entry EBP is accepted (the
//!   lifted body pushes the SEH EBP, hardware the real one); a callee VA the root's port declares
//!   inlined (`--inlined-callees`, `tools/data/t2capture_inlined_callees.json`) is dropped from
//!   the sequence when the port does not call it. Write-set, read-set and exit registers stay
//!   strict, and so does every undeclared callee difference;
//!   v11 (2026-10-27, INFERRED, `stub.rs`): a callee argument word that is a pointer into the root's
//!   own stack (within 64 KiB of its entry ESP) on both sides is equal when `pointer - callee entry
//!   ESP` is equal: the port may reserve a different frame (T3867). Any other difference stays strict;
//!   v7 (2026-10-26, INFERRED, any root): the lift keeps EBP as a C local and publishes `g_ebp`,
//!   which the capture records after each lifted callee; an `fpo_leaf` callee publishes
//!   `g_seh_ebp`, so the leak travels up. The leaked value is the captured exit EBP of the last
//!   callee (before the point judged) whose own captured entry and exit EBP differ
//!   (`leaked_ebp`). A port exit EBP equal to the entry EBP is accepted when the captured exit
//!   EBP is that leaked value; a callee argument word likewise (`stub.rs`). No such callee, or a
//!   captured value that is not it, stays strict;
//! - the final bytes match at every address of the captured write-set plus every byte of the
//!   pages the code under test wrote (where nothing was captured the original left the initial
//!   value), EXCLUDING the dead stack below the entry ESP (amendment 1: `[esp - 64 KiB, esp)`).
//!
//! - v9 (2026-10-27, T3832, the T3818 0x60B30 hole): the compared extent is the ORIGINAL's. Every
//!   captured write-set byte outside the dead stack must lie on a page the run wrote; a page the
//!   original wrote that the run never wrote is `write_set` (`missing`), even when its bytes still
//!   hold the captured values (a same-value rewrite the port skipped, e.g. a capped node loop).
//!   Within a written page the final bytes are compared as before (byte-level write tracking does
//!   not exist; a skipped same-value rewrite inside a page the run wrote is the documented limit).
//!   Both sides get the rule. `no_work` (runner): the port wrote nothing outside the dead stack and
//!   made no call while the original wrote memory or had callees; reported for every run entry,
//!   also where the lifted control is unfaithful, so an exclusion can never hide an empty port.
//!
//! - (lifted control only, T3740 record format 2) the implicit thread state at exit
//!   (`g_seh_ebp`, DF, x87 compare state, flag bridge, FS base) matches the capture.
//!
//! A port whose only differences are float words (or ST0) within `FP_ULP_LIMIT` units in the last
//! place of an f32 is `fp_ulp` (amendment 3), never EQUAL. The lifted control has no such leniency.
//! Only normal floats qualify (small integers are subnormals one ULP apart). A word of another
//! type that happens to look like a nearby normal float is still classified `fp_ulp`: a
//! classification error at worst, never a false EQUAL.
use std::collections::BTreeMap;

use tsfp_port_runtime::abi::ExtRegisters;

use crate::arena::{Arena, PAGE, RunEnd};
use crate::spool::{CalleeRecord, Entry, IMPLICIT_NAMES, Implicit};

/// Bump on any change to the rule above.
pub const COMPARE_LOGIC_VERSION: u32 = 12;
/// The dead-stack window below the entry ESP (same 64 KiB as the T3741 shape key's stack window).
pub const DEAD_STACK_WINDOW: u32 = 0x1_0000;
/// Largest f32 ULP distance classified as `fp_ulp`.
pub const FP_ULP_LIMIT: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Equal,
    ExitRegs,
    WriteSet,
    CalleeMismatch,
    FpUlp,
    Fault,
}

impl Class {
    pub fn name(self) -> &'static str {
        match self {
            Class::Equal => "equal",
            Class::ExitRegs => "exit_regs",
            Class::WriteSet => "write_set",
            Class::CalleeMismatch => "callee_mismatch",
            Class::FpUlp => "fp_ulp",
            Class::Fault => "fault",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Judgement {
    pub class: Class,
    pub detail: String,
}

impl Judgement {
    fn new(class: Class, detail: String) -> Self {
        Self { class, detail }
    }
}

/// Return registers no caller reads after the call (port side only; default strict).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RetDead {
    pub eax: bool,
    pub edx: bool,
    pub ebp: bool,
    /// v5: the lifted root is `fpo_leaf` (its exit EBP is the entry `seh_ebp`, see above).
    pub fpo_leaf: bool,
}

impl RetDead {
    /// Comma-separated `eax`, `edx` and/or `ebp` (the `--ret-dead` value).
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut dead = Self::default();
        for name in text.split(',').filter(|name| !name.is_empty()) {
            match name {
                "eax" => dead.eax = true,
                "edx" => dead.edx = true,
                "ebp" => dead.ebp = true,
                other => return Err(format!("bad --ret-dead register {other}")),
            }
        }
        Ok(dead)
    }
}

/// Which side is judged: the lifted control must reproduce the capture exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Lifted,
    Port,
}

pub fn in_dead_stack(addr: u64, entry_esp: u32) -> bool {
    let esp = u64::from(entry_esp);
    addr < esp && addr + u64::from(DEAD_STACK_WINDOW) >= esp
}

/// Captured write-set bytes inside the dead stack (excluded, reported as a warning count).
pub fn dead_stack_writes(entry: &Entry) -> u32 {
    let esp = entry.entry_regs.esp;
    expected_writes(entry)
        .keys()
        .filter(|&&addr| in_dead_stack(addr, esp))
        .count() as u32
}

fn expected_writes(entry: &Entry) -> BTreeMap<u64, u8> {
    let mut out = BTreeMap::new();
    for run in &entry.write_runs {
        for (offset, byte) in run.data.iter().enumerate() {
            out.insert(u64::from(run.addr) + offset as u64, *byte);
        }
    }
    out
}

/// v9: whether a page holds any byte outside the dead stack.
fn page_is_live(page: usize, entry_esp: u32) -> bool {
    let start = (page * PAGE) as u64;
    let esp = u64::from(entry_esp);
    let dead_lo = esp.saturating_sub(u64::from(DEAD_STACK_WINDOW));
    // Live unless the whole page lies in [esp - window, esp).
    !(start >= dead_lo && start + PAGE as u64 <= esp)
}

/// v9: captured write-set bytes outside the dead stack on pages the run never wrote, ascending.
pub fn unwritten_captured(entry: &Entry, arena: &Arena) -> Vec<u64> {
    let esp = entry.entry_regs.esp;
    let written: std::collections::BTreeSet<usize> = arena.written_pages().into_iter().collect();
    expected_writes(entry)
        .keys()
        .copied()
        .filter(|&addr| !in_dead_stack(addr, esp) && !written.contains(&((addr as usize) / PAGE)))
        .collect()
}

/// v9: the original did work: it wrote memory outside the dead stack or called a callee.
pub fn original_worked(entry: &Entry) -> bool {
    !entry.callees.is_empty()
        || expected_writes(entry)
            .keys()
            .any(|&addr| !in_dead_stack(addr, entry.entry_regs.esp))
}

/// v9: the run did work: it made a call, changed a byte outside the dead stack, or wrote a page
/// the original wrote (a same-value rewrite). The entry return slot `[esp, esp + 4)` alone is not
/// work: a dispatch wrapper may rewrite it.
pub fn run_worked(entry: &Entry, arena: &Arena, calls: usize) -> bool {
    if calls > 0 {
        return true;
    }
    let esp = entry.entry_regs.esp;
    let slot = u64::from(esp)..u64::from(esp) + 4;
    let written = arena.written_pages();
    let changed = written.iter().any(|&page| {
        let start = (page * PAGE) as u64;
        page_is_live(page, esp)
            && (start..start + PAGE as u64).any(|addr| {
                !in_dead_stack(addr, esp)
                    && !slot.contains(&addr)
                    && arena.byte(addr) != Some(arena.initial_byte(addr))
            })
    });
    let pages: std::collections::BTreeSet<usize> = written.into_iter().collect();
    changed
        || expected_writes(entry).keys().any(|&addr| {
            !in_dead_stack(addr, esp)
                && !slot.contains(&addr)
                && pages.contains(&((addr as usize) / PAGE))
        })
}

fn f32_ulps(want: u32, got: u32) -> Option<u32> {
    let (a, b) = (f32::from_bits(want), f32::from_bits(got));
    // Normal floats only: small integers (counters, flags) are f32 subnormals one ULP apart.
    if !a.is_normal() || !b.is_normal() || (want ^ got) & 0x8000_0000 != 0 {
        return None;
    }
    Some(want.abs_diff(got))
}

fn ulp_close_f32(want: u32, got: u32) -> bool {
    f32_ulps(want, got).is_some_and(|ulps| ulps <= FP_ULP_LIMIT)
}

/// ST0 as x87 holds it (f64 here): close when equal after rounding to f32 within the limit.
fn ulp_close_st(want: f64, got: f64) -> bool {
    ulp_close_f32((want as f32).to_bits(), (got as f32).to_bits())
}

/// v7: the captured exit EBP of the last of `callees` whose captured EBP changed (the lifted
/// `g_ebp` a callee leaked into its caller), None when no callee changed it.
pub fn leaked_ebp(callees: &[CalleeRecord]) -> Option<u32> {
    callees
        .iter()
        .rev()
        .find(|callee| callee.entry_regs.ebp != callee.exit_regs.ebp)
        .map(|callee| callee.exit_regs.ebp)
}

fn regs_detail(name: &str, want: u32, got: u32) -> String {
    format!("{name} want {want:#x} got {got:#x}")
}

/// Exit registers: Ok(None) equal, Ok(Some(detail)) only an ST0 ULP difference, Err(detail).
fn compare_regs(
    side: Side,
    entry: &ExtRegisters,
    want: &ExtRegisters,
    got: &ExtRegisters,
    ret_dead: RetDead,
    seh_ebp: Option<u32>,
    leaked: Option<u32>,
) -> Result<Option<String>, String> {
    let port = side == Side::Port;
    // v5: the lifted fpo_leaf quirk (exit EBP = entry seh_ebp) against the hardware EBP.
    // v7: a callee's leaked g_ebp (exit EBP = leaked value) against the hardware EBP.
    let lifted_seh_ebp = port
        && got.ebp == entry.ebp
        && ((ret_dead.fpo_leaf && seh_ebp.is_some_and(|seh| want.ebp == seh))
            || leaked.is_some_and(|value| want.ebp == value));
    let mut gprs = vec![
        ("ebx", want.ebx, got.ebx),
        ("esi", want.esi, got.esi),
        ("edi", want.edi, got.edi),
    ];
    if !lifted_seh_ebp && !(port && ret_dead.ebp) {
        gprs.push(("ebp", want.ebp, got.ebp));
    }
    gprs.push(("esp", want.esp, got.esp));
    // Return registers after the callee-saved ones, so a callee-saved difference is never hidden.
    if !(port && ret_dead.eax) {
        gprs.push(("eax", want.eax, got.eax));
    }
    if !(port && ret_dead.edx) {
        gprs.push(("edx", want.edx, got.edx));
    }
    if side == Side::Lifted {
        gprs.push(("ecx", want.ecx, got.ecx));
    }
    for (name, w, g) in gprs {
        if w != g {
            return Err(regs_detail(name, w, g));
        }
    }
    if want.fpu.top != got.fpu.top {
        return Err(regs_detail("fpu.top", want.fpu.top, got.fpu.top));
    }
    if side == Side::Lifted {
        if want.fpu.control != got.fpu.control {
            return Err(regs_detail(
                "fpu.control",
                want.fpu.control.into(),
                got.fpu.control.into(),
            ));
        }
        for slot in 0..8 {
            let (w, g) = (want.fpu.stack[slot], got.fpu.stack[slot]);
            if w.to_bits() != g.to_bits() {
                return Err(format!("fpu.stack[{slot}] want {w:e} got {g:e}"));
            }
        }
        let lanes = |r: &ExtRegisters| {
            r.xmm
                .iter()
                .chain(r.xmm_hi.iter())
                .flat_map(|x| x.lanes)
                .collect::<Vec<_>>()
        };
        if let Some(index) = lanes(want)
            .iter()
            .zip(lanes(got))
            .position(|(w, g)| *w != g)
        {
            return Err(format!("xmm{} lane {} differs", index / 4, index % 4));
        }
        return Ok(None);
    }
    // A float return: the original pushed ST0 (TOP moved down by one from the entry).
    if want.fpu.top == (entry.fpu.top + 7) & 7 {
        let (w, g) = (want.fpu.st(0), got.fpu.st(0));
        if w.to_bits() != g.to_bits() {
            if ulp_close_st(w, g) {
                return Ok(Some(format!("fp_ulp st0 want {w:e} got {g:e}")));
            }
            return Err(format!("st0 want {w:e} got {g:e}"));
        }
    }
    Ok(None)
}

/// Write-set: Ok(None) equal, Ok(Some(detail)) only f32 ULP differences, Err(detail).
fn compare_writes(side: Side, entry: &Entry, arena: &Arena) -> Result<Option<String>, String> {
    let esp = entry.entry_regs.esp;
    // v9: the original's extent first. A page it wrote that this run never wrote is a missing
    // write whatever the bytes hold (strict for both sides, never fp_ulp).
    let missing = unwritten_captured(entry, arena);
    if let Some(&first) = missing.first() {
        let pages: std::collections::BTreeSet<u64> =
            missing.iter().map(|addr| addr / PAGE as u64).collect();
        return Err(format!(
            "write@{first:#010x} missing: the run never wrote the page ({} captured bytes on {} unwritten pages)",
            missing.len(),
            pages.len()
        ));
    }
    let expected = expected_writes(entry);
    let want_at = |addr: u64| {
        expected
            .get(&addr)
            .copied()
            .unwrap_or_else(|| arena.initial_byte(addr))
    };
    // Differing addresses, ascending.
    let mut diffs: Vec<u64> = Vec::new();
    for (&addr, &want) in &expected {
        if !in_dead_stack(addr, esp) && arena.byte(addr) != Some(want) {
            diffs.push(addr);
        }
    }
    for page in arena.written_pages() {
        let start = (page * PAGE) as u64;
        for addr in start..start + PAGE as u64 {
            if expected.contains_key(&addr) || in_dead_stack(addr, esp) {
                continue;
            }
            if arena.byte(addr) != Some(arena.initial_byte(addr)) {
                diffs.push(addr);
            }
        }
    }
    diffs.sort_unstable();
    diffs.dedup();
    let hard = |addr: u64| {
        let got = arena
            .byte(addr)
            .map_or("unwritten".to_string(), |b| format!("{b:02x}"));
        format!(
            "write@{addr:#010x} want {:02x} got {got} ({} bytes differ)",
            want_at(addr),
            diffs.len()
        )
    };
    let Some(&first) = diffs.first() else {
        return Ok(None);
    };
    if side == Side::Lifted {
        return Err(hard(first));
    }
    // Port: every differing aligned word must be an f32 within the ULP limit.
    let mut ulp_detail = None;
    for &addr in &diffs {
        let word = addr & !3;
        let mut want = [0u8; 4];
        let mut got = [0u8; 4];
        let mut known = true;
        for i in 0..4u64 {
            want[i as usize] = want_at(word + i);
            match arena.byte(word + i) {
                Some(b) => got[i as usize] = b,
                None => known = false,
            }
        }
        let (w, g) = (u32::from_le_bytes(want), u32::from_le_bytes(got));
        if !known || !ulp_close_f32(w, g) {
            return Err(hard(addr));
        }
        ulp_detail.get_or_insert_with(|| {
            format!(
                "fp_ulp write@{word:#010x} want {:e} got {:e}",
                f32::from_bits(w),
                f32::from_bits(g)
            )
        });
    }
    Ok(ulp_detail)
}

/// Implicit state at exit, lifted control only (ports keep it in Rust state, not the globals).
/// None when equal or when the record has no implicit state (format 1 records are skipped).
pub fn compare_implicit(entry: &Entry, got: &Implicit) -> Option<String> {
    let want = entry.exit_implicit?;
    (0..7)
        .find(|&i| want[i] != got[i])
        .map(|i| regs_detail(IMPLICIT_NAMES[i], want[i], got[i]))
}

/// Judge one run of one side against the captured entry.
pub fn judge(
    side: Side,
    entry: &Entry,
    arena: &Arena,
    end: &RunEnd,
    callees: Option<String>,
    exit: &ExtRegisters,
    ret_dead: RetDead,
) -> Judgement {
    match end {
        RunEnd::Returned => {}
        RunEnd::Fault { signal, address } => {
            let guest = arena
                .guest_of(*address)
                .map_or(format!("host {address:#x}"), |g| format!("guest {g:#010x}"));
            return Judgement::new(Class::Fault, format!("signal {signal} at {guest}"));
        }
        RunEnd::Panic(text) => return Judgement::new(Class::Fault, format!("panic: {text}")),
    }
    if let Some(detail) = callees {
        return Judgement::new(Class::CalleeMismatch, detail);
    }
    let seh_ebp = entry.entry_implicit.map(|implicit| implicit[0]); // IMPLICIT_NAMES[0]
    let regs = compare_regs(
        side,
        &entry.entry_regs,
        &entry.exit_regs,
        exit,
        ret_dead,
        seh_ebp,
        leaked_ebp(&entry.callees),
    );
    let regs_ulp = match regs {
        Err(detail) => return Judgement::new(Class::ExitRegs, detail),
        Ok(ulp) => ulp,
    };
    match compare_writes(side, entry, arena) {
        Err(detail) => Judgement::new(Class::WriteSet, detail),
        Ok(write_ulp) => match regs_ulp.or(write_ulp) {
            Some(detail) => Judgement::new(Class::FpUlp, detail),
            None => Judgement::new(Class::Equal, String::new()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zero_regs() -> ExtRegisters {
        crate::spool::decode_regs(&[0u8; crate::spool::REGS_SIZE])
    }

    const DEAD_EDX: RetDead = RetDead {
        eax: false,
        edx: true,
        ebp: false,
        fpo_leaf: false,
    };
    const DEAD_BOTH: RetDead = RetDead {
        eax: true,
        edx: true,
        ebp: false,
        fpo_leaf: false,
    };
    const FPO_LEAF: RetDead = RetDead {
        eax: false,
        edx: false,
        ebp: false,
        fpo_leaf: true,
    };
    const ENTRY_EBP: u32 = 0x4109_fff0;
    const SEH_EBP: u32 = 0x4109_f590;

    fn ebp_case(captured_exit: u32, port_exit: u32) -> (ExtRegisters, ExtRegisters, ExtRegisters) {
        let entry = ExtRegisters {
            ebp: ENTRY_EBP,
            ..zero_regs()
        };
        let want = ExtRegisters {
            ebp: captured_exit,
            ..entry
        };
        let got = ExtRegisters {
            ebp: port_exit,
            ..entry
        };
        (entry, want, got)
    }

    const LEAK_A: u32 = 0x4015_ff5c;
    const LEAK_B: u32 = 0x4015_fd48;

    /// A callee record whose captured EBP goes `entry_ebp -> exit_ebp`.
    fn callee(entry_ebp: u32, exit_ebp: u32) -> CalleeRecord {
        CalleeRecord {
            va: 0x5000,
            entry_regs: ExtRegisters {
                ebp: entry_ebp,
                ..zero_regs()
            },
            args: Vec::new(),
            exit_regs: ExtRegisters {
                ebp: exit_ebp,
                ..zero_regs()
            },
            write_runs: Vec::new(),
            exit_implicit: None,
        }
    }

    fn v7(
        callees: &[CalleeRecord],
        captured_exit: u32,
        port_exit: u32,
    ) -> Result<Option<String>, String> {
        let (entry, want, got) = ebp_case(captured_exit, port_exit);
        compare_regs(
            Side::Port,
            &entry,
            &want,
            &got,
            RetDead::default(),
            Some(SEH_EBP),
            leaked_ebp(callees),
        )
    }

    #[test]
    fn v7_transitive_leak_from_the_last_changed_callee_is_equal() {
        // Callee 0 leaked LEAK_A, callee 1 (itself fed a leak by its own callee) LEAK_B, callee 2
        // kept its EBP: the root's captured exit EBP is LEAK_B.
        let callees = [
            callee(ENTRY_EBP, LEAK_A),
            callee(LEAK_A, LEAK_B),
            callee(LEAK_B, LEAK_B),
        ];
        assert_eq!(leaked_ebp(&callees), Some(LEAK_B));
        assert_eq!(v7(&callees, LEAK_B, ENTRY_EBP), Ok(None));
        // An earlier leak is not the published one: strict.
        assert!(v7(&callees, LEAK_A, ENTRY_EBP).is_err());
    }

    #[test]
    fn v7_applies_to_a_non_fpo_leaf_root_on_the_port_side_only() {
        let callees = [callee(ENTRY_EBP, LEAK_A)];
        let (entry, want, got) = ebp_case(LEAK_A, ENTRY_EBP);
        let leaked = leaked_ebp(&callees);
        let port = compare_regs(
            Side::Port,
            &entry,
            &want,
            &got,
            RetDead::default(),
            None,
            leaked,
        );
        assert_eq!(port, Ok(None));
        let lifted = compare_regs(
            Side::Lifted,
            &entry,
            &want,
            &got,
            RetDead::default(),
            None,
            leaked,
        );
        assert!(lifted.is_err());
    }

    #[test]
    fn v7_port_ebp_of_neither_value_diverges() {
        let callees = [callee(ENTRY_EBP, LEAK_A)];
        let verdict = v7(&callees, LEAK_A, ENTRY_EBP + 4);
        assert!(verdict.is_err_and(|detail| detail.starts_with("ebp")));
        // Nor does the port get away with the leaked value when the capture kept the entry EBP.
        assert!(v7(&callees, ENTRY_EBP, LEAK_A).is_err());
    }

    #[test]
    fn v7_captured_exit_matching_no_callee_stays_strict() {
        // A callee changed EBP, but to another value than the captured exit.
        let callees = [callee(ENTRY_EBP, LEAK_A)];
        assert!(v7(&callees, LEAK_B, ENTRY_EBP).is_err());
        // No callee changed EBP (missing chain): strict even if a callee's exit equals it.
        let unchanged = [callee(LEAK_B, LEAK_B)];
        assert_eq!(leaked_ebp(&unchanged), None);
        assert!(v7(&unchanged, LEAK_B, ENTRY_EBP).is_err());
        assert!(v7(&[], LEAK_B, ENTRY_EBP).is_err());
    }

    #[test]
    fn fpo_leaf_port_keeping_entry_ebp_is_equal() {
        let (entry, want, got) = ebp_case(SEH_EBP, ENTRY_EBP);
        let verdict = compare_regs(
            Side::Port,
            &entry,
            &want,
            &got,
            FPO_LEAF,
            Some(SEH_EBP),
            None,
        );
        assert_eq!(verdict, Ok(None));
        // The lifted control is still exact.
        let lifted = compare_regs(
            Side::Lifted,
            &entry,
            &want,
            &got,
            FPO_LEAF,
            Some(SEH_EBP),
            None,
        );
        assert!(lifted.is_err());
    }

    #[test]
    fn non_fpo_leaf_ebp_difference_still_diverges() {
        let (entry, want, got) = ebp_case(SEH_EBP, ENTRY_EBP);
        let verdict = compare_regs(
            Side::Port,
            &entry,
            &want,
            &got,
            RetDead::default(),
            Some(SEH_EBP),
            None,
        );
        assert!(verdict.is_err());
        // fpo_leaf without the implicit seh_ebp (format 1 record): unknown, strict.
        assert!(compare_regs(Side::Port, &entry, &want, &got, FPO_LEAF, None, None).is_err());
    }

    #[test]
    fn fpo_leaf_port_ebp_of_neither_value_diverges() {
        let (entry, want, got) = ebp_case(SEH_EBP, 0x1234);
        let verdict = compare_regs(
            Side::Port,
            &entry,
            &want,
            &got,
            FPO_LEAF,
            Some(SEH_EBP),
            None,
        );
        assert!(verdict.is_err());
        // Captured exit EBP that is not seh_ebp: the quirk does not explain it, strict.
        let (entry, want, got) = ebp_case(0x5555, ENTRY_EBP);
        let verdict = compare_regs(
            Side::Port,
            &entry,
            &want,
            &got,
            FPO_LEAF,
            Some(SEH_EBP),
            None,
        );
        assert!(verdict.is_err());
    }

    #[test]
    fn dead_edx_difference_is_equal_on_the_port_side() {
        let (entry, want) = (zero_regs(), zero_regs());
        let got = ExtRegisters { edx: 0x202, ..want };
        assert_eq!(
            compare_regs(Side::Port, &entry, &want, &got, DEAD_EDX, None, None),
            Ok(None)
        );
        // Strict without the liveness fact (unknown = fail closed) and always for the control.
        assert!(
            compare_regs(
                Side::Port,
                &entry,
                &want,
                &got,
                RetDead::default(),
                None,
                None
            )
            .is_err()
        );
        assert!(compare_regs(Side::Lifted, &entry, &want, &got, DEAD_BOTH, None, None).is_err());
    }

    #[test]
    fn live_eax_difference_diverges() {
        let (entry, want) = (zero_regs(), zero_regs());
        let got = ExtRegisters { eax: 1, ..want };
        assert_eq!(
            compare_regs(Side::Port, &entry, &want, &got, DEAD_EDX, None, None),
            Err("eax want 0x0 got 0x1".to_string())
        );
    }

    #[test]
    fn callee_saved_difference_diverges_with_both_return_registers_dead() {
        let (entry, want) = (zero_regs(), zero_regs());
        for got in [
            ExtRegisters { ebx: 1, ..want },
            ExtRegisters { esi: 1, ..want },
            ExtRegisters { edi: 1, ..want },
            ExtRegisters { ebp: 1, ..want },
            ExtRegisters { esp: 1, ..want },
        ] {
            let both = ExtRegisters {
                eax: 7,
                edx: 7,
                ..got
            };
            let verdict = compare_regs(Side::Port, &entry, &want, &both, DEAD_BOTH, None, None);
            assert!(verdict.is_err(), "{got:?}");
        }
    }

    #[test]
    fn ret_dead_parses_the_flag_value() {
        assert!(RetDead::parse("ebp").unwrap().ebp);
        assert_eq!(RetDead::parse("eax,edx"), Ok(DEAD_BOTH));
        assert_eq!(RetDead::parse("edx"), Ok(DEAD_EDX));
        assert_eq!(RetDead::parse(""), Ok(RetDead::default()));
        assert!(RetDead::parse("ebx").is_err());
    }

    #[test]
    fn dead_ebp_only_masks_port_comparison() {
        let entry = zero_regs();
        let want = zero_regs();
        let got = ExtRegisters {
            ebp: 7,
            ..zero_regs()
        };
        let dead = RetDead::parse("ebp").unwrap();
        assert!(compare_regs(Side::Port, &entry, &want, &got, dead, None, None).is_ok());
        assert!(
            compare_regs(
                Side::Port,
                &entry,
                &want,
                &got,
                RetDead::default(),
                None,
                None
            )
            .is_err()
        );
        assert!(compare_regs(Side::Lifted, &entry, &want, &got, dead, None, None).is_err());
    }

    #[test]
    fn dead_stack_is_the_window_below_entry_esp() {
        assert!(in_dead_stack(0x4000_fffc, 0x4001_0000));
        assert!(in_dead_stack(0x4000_0000, 0x4001_0000));
        assert!(!in_dead_stack(0x3fff_ffff, 0x4001_0000));
        assert!(!in_dead_stack(0x4001_0000, 0x4001_0000));
        assert!(!in_dead_stack(0x0050_0000, 0x4001_0000));
    }

    #[test]
    fn ulp_closeness_is_f32_bits_same_sign_finite() {
        let one = 1.0f32.to_bits();
        assert!(ulp_close_f32(one, one + 1));
        assert!(ulp_close_f32(one, one + 2));
        assert!(!ulp_close_f32(one, one + 3));
        assert!(!ulp_close_f32(0x0000_0001, 0x8000_0001));
        assert!(
            !ulp_close_f32(42, 43),
            "integers are subnormal floats, never fp_ulp"
        );
        assert!(!ulp_close_f32(f32::NAN.to_bits(), f32::NAN.to_bits() + 1));
        assert!(ulp_close_st(1.0, 1.0 + f64::EPSILON));
        assert!(!ulp_close_st(1.0, 1.001));
    }
}
