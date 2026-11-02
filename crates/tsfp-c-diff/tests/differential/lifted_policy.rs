// SPDX-License-Identifier: GPL-3.0-or-later
//! T2015: exit-register policy for the lifted reference (`TSFP_REF_LIFTED=1`, T2007).
//!
//! Against the C replacement every register is compared. Against the lifted original some
//! registers are not comparable for a documented reason, per root and never globally:
//!
//! * `EBP` of a frameless (`fpo_leaf`) lifted body is re-seeded from the `g_seh_ebp` model global at
//!   entry (the lifter's "inherit caller's frame"), so the harness's random EBP never reaches the
//!   body and the lifted exit EBP is that global, not the Rust port's untouched input.
//! * `EAX`/`ECX`/`EDX` scratch of a void or stdcall-style function is dead by the Xbox ABI; the
//!   original leaves compiler residue in it that the port does not model.
//!
//! The table below lists the register bits (order EAX ECX EDX EBX ESI EDI EBP ESP) excluded per
//! root, with the reason. Roots that are not in the table still compare everything.
//! `TSFP_DIAG_REGS=1` turns every comparator into a recorder that prints the union of differing
//! registers per root (`T2015-DIFF va mask`) instead of failing, which is how the table was built.
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// Entry ESP of every case (`STACK`). The lifted body pushes its saved registers below it, and
/// those spills are dead once it returns (the same rule as `w4.rs`: "dead stack contents below ESP
/// are not compared"), so lifted-mode memory comparison skips this window. The port also spills
/// but with the EBP/scratch values it was given, which the lifted body (EBP re-seeded) does not
/// share. Everything at or above `STACK + 4` (arguments, caller frame) is still compared.
const DEAD_STACK_LOW: u32 = 0x0030_0000 - 0x1000;
const DEAD_STACK_HIGH: u32 = 0x0030_0000 + 4;
/// Roots whose lifted spill region differs only because the port spills a different EBP/scratch
/// value (found with `TSFP_DIAG_REGS`: the only memory differences were at 0x2FFFF0..0x300003).
/// Not applied globally: the closure skew controls detect real faults through this very region.
const DEAD_STACK_ROOTS: &[u32] = &[
    0x074950, 0x074b80, 0x0a2c60, 0x17a200, 0x17a260, 0x165760, 0x20fa50, 0x0db480, 0x15c4e0,
    0x359910, 0x3ba270, 0x3c6730, 0x128e90, 0x129ec0, 0x347d70, 0x1b7d00, 0x1b7d70,
];

pub const EAX: u8 = 1;
pub const ECX: u8 = 2;
pub const EDX: u8 = 4;
pub const EBX: u8 = 8;
pub const ESI: u8 = 16;
pub const EDI: u8 = 32;
pub const EBP: u8 = 64;
pub const ESP: u8 = 128;

fn lifted() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("TSFP_REF_LIFTED").is_some_and(|v| v == "1"))
}

fn diag() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("TSFP_DIAG_REGS").is_some())
}

/// Registers excluded from the comparison of `va` against the lifted original.
pub fn ignored(va: u32) -> u8 {
    if !lifted() {
        return 0;
    }
    POLICY.iter().find(|(root, _, _)| *root == va).map_or(0, |(_, mask, _)| *mask)
}

/// `diff` has bit i set when register i differs. Returns whether the case is a mismatch.
pub fn mismatch(va: u32, diff: u8) -> bool {
    if diag() && lifted() {
        static SEEN: OnceLock<Mutex<HashMap<u32, u8>>> = OnceLock::new();
        let mut seen = SEEN.get_or_init(Default::default).lock().unwrap();
        let entry = seen.entry(va).or_insert(0);
        if *entry | diff != *entry {
            *entry |= diff;
            eprintln!("T2015-DIFF {va:08X} {:08b}", *entry);
        }
        return false;
    }
    diff & !ignored(va) != 0
}

/// Bit mask of the positions where two register arrays differ.
pub fn diff8(c: &[u32], r: &[u32]) -> u8 {
    (0..8).filter(|&i| c[i] != r[i]).fold(0, |m, i| m | 1 << i)
}

fn is_nan(word: u32) -> bool {
    word & 0x7f80_0000 == 0x7f80_0000 && word & 0x007f_ffff != 0
}

/// Single-precision roots whose NaN payload is not comparable against the lifted original. The
/// lifted body is C compiled by gcc, which may commute the operands of `mulss`/`addss`; SSE returns
/// the first NaN operand, so with two NaN inputs the payload (and sign) of the result depends on
/// operand order, not on the original machine code. A word where both sides are NaN is equal.
const NAN_PAYLOAD_FREE: &[u32] = &[
    0x150580, 0x151190, 0x151f20, 0x152120, 0x1543a0, 0x154470, 0x166060, 0x166140, 0x36ec40,
];

/// True when the 4-byte word holding `address` is a NaN on both sides (see [`NAN_PAYLOAD_FREE`]).
pub fn nan_equal(va: u32, c_word: u32, r_word: u32) -> bool {
    lifted() && NAN_PAYLOAD_FREE.contains(&va) && is_nan(c_word) && is_nan(r_word)
}

/// Bit mask of the XMM registers that differ (lanes that are NaN on both sides count as equal for
/// the roots in [`NAN_PAYLOAD_FREE`]).
pub fn xmm_diff(va: u32, c: &[[u32; 4]; 8], r: &[[u32; 4]; 8]) -> u8 {
    (0..8)
        .filter(|&i| (0..4).any(|l| c[i][l] != r[i][l] && !nan_equal(va, c[i][l], r[i][l])))
        .fold(0, |m, i| m | 1 << i)
}

/// XMM comparison, with the same per-root exclusion and diagnostic recording as [`mismatch`].
pub fn xmm_mismatch(va: u32, diff: u8) -> bool {
    if diag() && lifted() {
        if diff != 0 {
            eprintln!("T2015-XMM {va:08X} {diff:08b}");
        }
        return false;
    }
    diff != 0
}

/// Memory comparison result; recorded as `T2015-MEM va addr` (first few distinct addresses) in
/// diagnostic mode, where it never fails.
pub fn mem(va: u32, addr: u32, differs: bool) -> bool {
    if differs && lifted() && DEAD_STACK_ROOTS.contains(&va) && (DEAD_STACK_LOW..DEAD_STACK_HIGH).contains(&addr) {
        return false;
    }
    if differs && diag() && lifted() {
        static SEEN: OnceLock<Mutex<HashMap<u32, Vec<u32>>>> = OnceLock::new();
        let mut seen = SEEN.get_or_init(Default::default).lock().unwrap();
        let list = seen.entry(va).or_default();
        if list.len() < 6 && !list.contains(&addr) {
            list.push(addr);
            eprintln!("T2015-MEM {va:08X} {addr:08x}");
        }
        return false;
    }
    differs
}

/// Any other comparison result (`what`: trace, extra slot, ...), recorded like [`mem`].
pub fn other(va: u32, what: &str, differs: bool) -> bool {
    if differs && diag() && lifted() {
        eprintln!("T2015-OTHER {va:08X} {what}");
        return false;
    }
    differs
}

/// Call-trace comparison. For the roots in [`TRACE_REGS_DEAD`] the register snapshot taken at each
/// nested call entry (`e[6..13]`: EAX ECX EDX EBX ESI EDI EBP) is not compared against the lifted
/// body; target, ESP, return address and the stack arguments (`e[0..6]`) always are. Every callee
/// of those roots takes only stack arguments (`callee_argc_or_neg` in c/shim.c), so the registers
/// at entry are residue the callee never reads, and which residue it is depends on how the compiler
/// scheduled the caller, not on what the port must do.
pub fn trace(va: u32, c: &[[u32; 16]], r: &[[u32; 16]]) -> bool {
    let masked = lifted() && TRACE_REGS_DEAD.contains(&va);
    let differs = c.len() != r.len()
        || c.iter().zip(r).any(|(x, y)| if masked { x[..6] != y[..6] } else { x != y });
    other(va, "trace", differs)
}

const TRACE_REGS_DEAD: &[u32] =
    &[0x15c4e0, 0x2d74b0, 0x2b2390, 0x33acb0, 0x356870, 0x359910, 0x60740, 0xdb480];

const SCRATCH_ECX_EDX: &str = "ECX/EDX are caller-saved scratch of a stack-argument function: the lifted body leaves compiler residue in them that the port does not model, and no call site of the root reads them before overwriting (heuristic scan of the lifted callers)";

/// (root, ignored registers, reason). Built from `TSFP_DIAG_REGS` runs, see
/// docs/t-t2015-lifted-triage.md for the per-root triage.
const POLICY: &[(u32, u8, &str)] = &[
    // wave 10 / 12 roots: only ECX and/or EDX differ (EBP is no longer a difference since the
    // harness seeds the lifted body's g_seh_ebp from the case EBP, c/shim.c t20_seed_ebp).
    (0x070d10, ECX | EDX, SCRATCH_ECX_EDX),
    (0x0718e0, ECX, SCRATCH_ECX_EDX),
    (0x071cd0, ECX | EDX, SCRATCH_ECX_EDX),
    (0x074670, ECX, SCRATCH_ECX_EDX),
    (0x074840, ECX, SCRATCH_ECX_EDX),
    (0x074950, ECX | EDX, SCRATCH_ECX_EDX),
    (0x074b80, ECX | EDX, SCRATCH_ECX_EDX),
    (0x094290, ECX, SCRATCH_ECX_EDX),
    (0x09e730, ECX | EDX, SCRATCH_ECX_EDX),
    (0x0a2c60, ECX | EDX, SCRATCH_ECX_EDX),
    (0x0b2d40, ECX | EDX, SCRATCH_ECX_EDX),
    (0x10e780, ECX, SCRATCH_ECX_EDX),
    (0x11d990, ECX, SCRATCH_ECX_EDX),
    (0x160850, ECX | EDX, SCRATCH_ECX_EDX),
    (0x177ec0, ECX | EDX, SCRATCH_ECX_EDX),
    (0x1b6310, ECX | EDX, SCRATCH_ECX_EDX),
    (0x1b75b0, ECX, SCRATCH_ECX_EDX),
    (0x073e10, ECX | EDX, SCRATCH_ECX_EDX),
    (0x07c1b0, EDX, SCRATCH_ECX_EDX),
    (0x07c4d0, ECX, SCRATCH_ECX_EDX),
    (0x09e750, ECX | EDX, SCRATCH_ECX_EDX),
    (0x25b770, ECX | EDX, SCRATCH_ECX_EDX),
    (0x2aefb0, ECX | EDX, SCRATCH_ECX_EDX),
    (0x349710, ECX, SCRATCH_ECX_EDX),
    // closure roots whose fixture lets the stubbed callee overwrite the caller's saved-ESI slot
    // (STACK - 4): the lifted body pops that slot, the port keeps the saved ESI in a local.
    (0x0606e0, ESI, "fixture: seeded callee write lands on the pushed ESI slot, the lifted POP reads it back"),
    (0x0db480, EAX | ECX | EDX | ESI, "found path: the port models 000DB3B0's register outputs (EAX=object, ECX/EDX reloaded) that the opaque callee stub cannot reproduce; ESI as for 0606E0"),
    // base64 encoder: void, EAX/ECX/EDX at exit are the last table byte and loop counters
    (0x3ba270, EAX | ECX | EDX, "void function; EAX/ECX/EDX are the compiler's loop residue"),
];
