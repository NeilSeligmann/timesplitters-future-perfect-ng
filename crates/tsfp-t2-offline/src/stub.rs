// SPDX-License-Identifier: GPL-3.0-or-later
//! Callee stub (design section 3, "Driving the port"): every nested guest call of the code under
//! test is matched against the next captured callee (VA plus stack argument words) and replaced by
//! its captured outcome: the net write-set of the callee subtree is written into the arena and the
//! exit registers are applied. Unported callees, kernel imports and indirect targets are handled
//! alike, so a function is judged in isolation. The first unexpected or missing call is
//! `callee_mismatch@k` (k = 0-based index into the captured callee list).
use tsfp_port_runtime::abi::{ExtRegisters, GuestCall};

use crate::arena::Arena;
use crate::frames::{NestedFrames, Slot, expand};
use crate::spool::{Entry, Implicit};

/// Where the caller left ESP when it hands a nested call to the stub.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Convention {
    /// `GuestCall::call` and `tsfp_lane_x_call`: ESP points at the first argument; the bridge
    /// pushes the return slot itself (the callee entry ESP is ESP - 4).
    BridgePushesReturn,
    /// `tsfp_lane_guest_call`: the caller already pushed the return PC (callee entry ESP = ESP).
    ReturnPushed,
}

/// Which callee argument bytes are compared (T3767).
///
/// The capture records a fixed 16-word window per callee. That window also covers the function's
/// own frame and its caller's: locals it had not written yet when it made the call still hold
/// whatever an earlier call left on the stack (0x3F800000 in the capture, 0xCDCDCDCD poison in
/// the replay), and slots it writes only after the call held their old values. Such a byte cannot
/// be passed by the function, so it is only compared when the function had written it by the time
/// of the call, or the capture read it (then it is in the read-set and the arena holds the
/// captured value).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArgPolicy {
    /// `--rust-only` (no lifted control to learn from): the version 2 rule, every word below the
    /// entry ESP and the words of the read-set and write-set above it.
    Strict,
    /// The lifted control: a byte the run had written (changed from the arena's initial value)
    /// before the call. Records the mask of each call for the port side.
    Observe,
    /// The port: the bytes the lifted control had written before the same call (`k`); a call the
    /// control never reached falls back to `Strict`.
    FromLifted(Vec<u64>),
}

/// Most callee VAs one root may declare as inlined (`--inlined-callees`).
pub const MAX_INLINED: usize = 8;

/// v6 (port side only, INFERRED): callee VAs the port runs natively instead of through a guest
/// call (rule 20), declared per root in `tools/data/t2capture_inlined_callees.json`. A captured
/// call to one of them that the port does not make is dropped from the sequence check; a call it
/// does make is matched as usual. Anything undeclared stays strict.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Inlined {
    vas: [u32; MAX_INLINED],
    len: usize,
}

impl Inlined {
    /// `0x153bd0,0x47560` (the `--inlined-callees` value).
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut out = Self::default();
        for item in text.split(',').filter(|item| !item.is_empty()) {
            let digits = item.trim_start_matches("0x").trim_start_matches("0X");
            let va = u32::from_str_radix(digits, 16)
                .map_err(|_| format!("bad --inlined-callees VA {item}"))?;
            if out.len == MAX_INLINED {
                return Err(format!("more than {MAX_INLINED} --inlined-callees"));
            }
            out.vas[out.len] = va;
            out.len += 1;
        }
        Ok(out)
    }

    pub fn contains(&self, va: u32) -> bool {
        self.vas[..self.len].contains(&va)
    }
}

/// v8 (T3738, port side only): proven stack arity per callee VA (`--callee-arity <file>`, derived
/// from the lifted bodies by `tools.t2capture.callee_arity`). An argument word at or above the
/// callee's arity is the caller's spill or local, not an argument: it is not compared. A callee
/// absent from the table is compared on the whole captured window (strict).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CalleeArity(std::collections::BTreeMap<u32, u32>);

impl CalleeArity {
    /// One `<hex va> <words>` pair per line (blank lines ignored).
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut out = std::collections::BTreeMap::new();
        for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
            let mut fields = line.split_whitespace();
            let (Some(va), Some(words), None) = (fields.next(), fields.next(), fields.next())
            else {
                return Err(format!("bad --callee-arity line {line:?}"));
            };
            let va = u32::from_str_radix(va.trim_start_matches("0x"), 16)
                .map_err(|_| format!("bad --callee-arity VA {va}"))?;
            let words: u32 = words
                .parse()
                .map_err(|_| format!("bad --callee-arity words {words}"))?;
            if out.insert(va, words).is_some() {
                return Err(format!("duplicate --callee-arity VA {va:#x}"));
            }
        }
        Ok(Self(out))
    }

    /// Proven argument words of `va`; None = unknown (strict).
    pub fn get(&self, va: u32) -> Option<u32> {
        self.0.get(&va).copied()
    }
}

pub struct CalleeStub<'a> {
    entry: &'a Entry,
    clock: Option<crate::clock::ClockReplay<'a>>,
    clock_error: Option<String>,
    /// The captured calls in order; v10 (`recursion`) inserts the inner frames of resolved
    /// recursive self calls after their slot (`frames::expand`).
    items: Vec<Slot<'a>>,
    /// Guest calls the code under test made.
    made: usize,
    arena: &'a Arena,
    seh_ebp: Option<u32>,
    next: usize,
    mismatch: Option<String>,
    /// Entry ESP of the function under test and its captured read runs (addr, len): an argument
    /// word at or above the entry ESP that the function never read or wrote lies in its caller's frame,
    /// so the capture's fixed 16-word window may hold values the run cannot know (not compared).
    root_esp: u32,
    root_reads: Vec<(u64, u64)>,
    /// Read runs only (a captured write says nothing about the call time).
    read_only: Vec<(u64, u64)>,
    policy: ArgPolicy,
    /// `Observe`: per call, bit `i` = byte `i` of the window had been written by the run when the
    /// call came.
    observed: Vec<u64>,
    /// v6 port rules (`port_rules`): the root is `fpo_leaf`, its entry EBP, its inlined callees.
    fpo_leaf: bool,
    root_ebp: u32,
    inlined: Inlined,
    /// v7 port rule: accept the g_ebp an earlier callee leaked (`compare::leaked_ebp`).
    leak_rule: bool,
    /// v8 port rule: compare only the argument words below each callee's proven arity.
    arity: Option<&'a CalleeArity>,
}

fn span(run: &crate::spool::Run) -> (u64, u64) {
    (
        u64::from(run.addr),
        u64::from(run.addr) + run.data.len() as u64,
    )
}

impl<'a> CalleeStub<'a> {
    /// A stub with the version 2 argument rule (`ArgPolicy::Strict`).
    pub fn new(entry: &'a Entry, arena: &'a Arena) -> Self {
        Self::with_policy(entry, arena, ArgPolicy::Strict)
    }

    pub fn with_policy(entry: &'a Entry, arena: &'a Arena, policy: ArgPolicy) -> Self {
        Self {
            entry,
            clock: entry.clock_events.as_deref().map(crate::clock::ClockReplay::new),
            clock_error: None,
            items: expand(entry, None, &NestedFrames::default()),
            made: 0,
            arena,
            seh_ebp: entry.entry_implicit.map(|implicit| implicit[0]),
            next: 0,
            mismatch: None,
            root_esp: entry.entry_regs.esp,
            root_reads: entry
                .read_runs
                .iter()
                .chain(&entry.write_runs)
                .map(span)
                .collect(),
            read_only: entry.read_runs.iter().map(span).collect(),
            policy,
            observed: Vec::new(),
            fpo_leaf: false,
            root_ebp: entry.entry_regs.ebp,
            inlined: Inlined::default(),
            leak_rule: false,
            arity: None,
        }
    }

    /// Replay a direct timestamp read while the arena is fault-tracked.
    /// Captures with inlined or expanded callees require a separate ordering model.
    ///
    /// # Safety
    /// Must be called inside `Arena::run` on this stub's arena.
    pub unsafe fn clock_read(&mut self) -> Result<u64, String> {
        if let Some(error) = &self.clock_error {
            return Err(error.clone());
        }
        let clock = self.clock.as_mut().ok_or("clock events absent")?;
        if self.items.len() != self.entry.callees.len() || self.inlined != Inlined::default() {
            let error = "clock ordering with expanded or inlined callees unsupported".to_string();
            self.clock_error = Some(error.clone());
            return Err(error);
        }
        let arena = self.arena;
        clock.read(self.next, |run| {
            if u64::from(run.addr) + run.data.len() as u64 > 0x1_0000_0000 {
                return Err("clock write exceeds guest address space".into());
            }
            unsafe { arena.write_bytes(run.addr, &run.data) };
            Ok(())
        })
    }

    /// Verify that every captured timestamp was consumed.
    pub fn clock_finish(&self) -> Result<(), String> {
        if let Some(error) = &self.clock_error {
            return Err(error.clone());
        }
        self.clock.as_ref().ok_or("clock events absent")?.finish()
    }

    /// v8, port side only: argument words at or above a callee's proven arity are not compared.
    pub fn callee_arity(mut self, arity: Option<&'a CalleeArity>) -> Self {
        self.arity = arity;
        self
    }

    /// v6, port side only: accept the lifted `seh_ebp` in an argument word of an `fpo_leaf` root
    /// and drop declared inlined callees from the sequence. The lifted control never gets them.
    pub fn port_rules(mut self, fpo_leaf: bool, inlined: Inlined) -> Self {
        self.fpo_leaf = fpo_leaf;
        self.inlined = inlined;
        self.leak_rule = true;
        self
    }

    /// v10 (T3839), port side only, after `port_rules`: when the root's own VA is a declared
    /// inlined callee (the port runs its recursion natively), a recursive self slot whose inner
    /// activation is among `frames` is followed by that frame's captured calls, so the inner
    /// frame's guest calls are matched inside their own frame instead of against the root's.
    pub fn recursion(mut self, frames: &NestedFrames<'a>) -> Self {
        if self.inlined.contains(self.entry.va) && !frames.is_empty() {
            self.items = expand(self.entry, Some(self.entry.va), frames);
        }
        self
    }

    /// First item from `k` the call `va` is matched against: a declared inlined callee other
    /// than `va` is dropped, a resolved self slot other than `va` is entered (its frame follows).
    fn skip_inlined(&self, mut k: usize, va: Option<u32>) -> usize {
        while let Some(item) = self.items.get(k) {
            let callee = item.record.va;
            if Some(callee) == va || (item.frame_end.is_none() && !self.inlined.contains(callee)) {
                break;
            }
            k += 1;
        }
        k
    }

    /// `@k` of a detail (the root's callee index) and the nested-frame suffix.
    fn label(&self, k: usize) -> (usize, String) {
        match self.items.get(k) {
            Some(item) if item.depth > 0 => (
                item.root_k,
                format!(
                    " (nested {:#x} frame depth {}, call {})",
                    self.entry.va, item.depth, item.local
                ),
            ),
            Some(item) => (item.root_k, String::new()),
            None => (self.entry.callees.len(), String::new()),
        }
    }

    /// `Observe`: the per-call masks of window bytes the run had written.
    pub fn observed(&self) -> &[u64] {
        &self.observed
    }

    fn in_read_set(&self, addr: u64) -> bool {
        self.read_only
            .iter()
            .any(|&(lo, hi)| lo <= addr && addr < hi)
    }

    /// Bytes of the window the run has written (changed from the arena's value before the run).
    /// Bit `i` = window byte `i` (`entry_esp + 4 + i`).
    fn written_mask(&self, entry_esp: u32, words: usize) -> u64 {
        let mut mask = 0u64;
        for i in 0..(words * 4).min(64) {
            let at = u64::from(entry_esp) + 4 + i as u64;
            if self
                .arena
                .byte(at)
                .is_some_and(|now| now != self.arena.initial_byte(at))
            {
                mask |= 1 << i;
            }
        }
        mask
    }

    /// Bit `i` set: window byte `i` (outside the read-set) is compared for call `k`.
    fn written_policy_mask(&self, k: usize, written: u64) -> u64 {
        match &self.policy {
            ArgPolicy::Strict => u64::MAX,
            ArgPolicy::Observe => written,
            ArgPolicy::FromLifted(masks) => masks.get(k).copied().unwrap_or(u64::MAX),
        }
    }

    /// Whether an argument word at `addr` is one the function under test could have passed.
    fn knowable(&self, addr: u32) -> bool {
        let (start, end) = (u64::from(addr), u64::from(addr) + 4);
        addr < self.root_esp
            || self
                .root_reads
                .iter()
                .any(|&(lo, hi)| lo <= start && end <= hi)
    }

    /// v11: `pointer - call_esp` when `pointer` lies in the root's stack (within the dead-stack
    /// window of the root's entry ESP, either side), else None. Equal offsets at the captured and
    /// the run call point to the same frame slot whatever the frame size.
    fn stack_pointer_offset(&self, pointer: u32, call_esp: u32) -> Option<u32> {
        let window = crate::compare::DEAD_STACK_WINDOW;
        let near = pointer.wrapping_sub(self.root_esp.wrapping_sub(window)) < 2 * window;
        near.then(|| pointer.wrapping_sub(call_esp))
    }

    /// Calls seen so far.
    pub fn calls(&self) -> usize {
        self.made
    }

    fn word(&self, addr: u32) -> Option<u32> {
        let mut bytes = [0u8; 4];
        for (offset, byte) in bytes.iter_mut().enumerate() {
            *byte = self.arena.byte(u64::from(addr) + offset as u64)?;
        }
        Some(u32::from_le_bytes(bytes))
    }

    /// One nested call. After a mismatch every later call only pops its return slot and
    /// returns EAX 0, so the code under test can run to its end; the verdict is already fixed.
    pub fn dispatch(&mut self, va: u32, regs: &mut ExtRegisters, convention: Convention) {
        self.apply(va, regs, convention, None);
    }

    /// A declared native descendant that is absent from the next captured direct-call slot runs
    /// through the host's Rust port dispatcher. Matching direct calls still use their captured
    /// outcomes; undeclared calls stay strict.
    pub fn should_dispatch_native(&self, va: u32) -> bool {
        if !self.inlined.contains(va) {
            return false;
        }
        let k = self.skip_inlined(self.next, Some(va));
        self.items.get(k).is_none_or(|item| item.record.va != va)
    }

    /// A nested call of the LIFTED original (T3740 fidelity: the host's RECOMP_ABI_CALL hook, so
    /// lifted callees, kernel imports and HLE thunks are replayed from the capture instead of
    /// dispatched). The return PC is already pushed. Unlike a port bridge, the lifted caller reads
    /// the globals after the call: `g_ebp` (EBP here) and the implicit state take the captured
    /// values after the callee returned.
    pub fn dispatch_lifted(&mut self, va: u32, regs: &mut ExtRegisters, implicit: &mut Implicit) {
        self.apply(va, regs, Convention::ReturnPushed, Some(implicit));
    }

    fn apply(
        &mut self,
        va: u32,
        regs: &mut ExtRegisters,
        convention: Convention,
        lifted: Option<&mut Implicit>,
    ) {
        let k = if self.mismatch.is_none() {
            self.skip_inlined(self.next, Some(va))
        } else {
            self.next
        };
        // A resolved self slot the code calls itself is replayed whole: its frame is skipped.
        self.next = match self.items.get(k) {
            Some(item) if self.mismatch.is_none() && item.record.va == va => {
                item.frame_end.unwrap_or(k + 1)
            }
            _ => k + 1,
        };
        self.made += 1;
        let entry_esp = match convention {
            Convention::BridgePushesReturn => regs.esp.wrapping_sub(4),
            Convention::ReturnPushed => regs.esp,
        };
        let words = self.items.get(k).map_or(0, |item| item.record.args.len());
        let written = self.written_mask(entry_esp, words);
        if self.mismatch.is_none() {
            self.mismatch = self.check(k, va, entry_esp, written);
        }
        if self.policy == ArgPolicy::Observe && self.mismatch.is_none() {
            self.observed.push(written);
        }
        if self.mismatch.is_some() {
            regs.eax = 0;
            regs.esp = entry_esp.wrapping_add(4);
            return;
        }
        let expected = self.items[k].record;
        for run in &expected.write_runs {
            // SAFETY: the stub runs inside `Arena::run`, the write faults into the tracker.
            unsafe { self.arena.write_bytes(run.addr, &run.data) };
        }
        let frame_ebp = regs.ebp;
        *regs = expected.exit_regs;
        match lifted {
            Some(implicit) => {
                if let Some(after) = expected.exit_implicit {
                    *implicit = after;
                }
            }
            // EBP: the lifted callee keeps it in a C local and the bridges hand the caller's back.
            None => regs.ebp = frame_ebp,
        }
        let pops = expected.exit_regs.esp.wrapping_sub(expected.entry_regs.esp);
        regs.esp = entry_esp.wrapping_add(pops);
    }

    fn check(&self, k: usize, va: u32, entry_esp: u32, written: u64) -> Option<String> {
        let (at_k, frame) = self.label(k);
        let Some(item) = self.items.get(k) else {
            return Some(format!(
                "callee_mismatch@{at_k}: unexpected call to {va:#x} (captured {})",
                self.entry.callees.len()
            ));
        };
        let expected = item.record;
        if expected.va != va {
            return Some(format!(
                "callee_mismatch@{at_k}: expected {:#x}, got {va:#x}{frame}",
                expected.va
            ));
        }
        // v10: a call of a nested frame has no lifted mask (the control replays the self slot
        // whole) and none of the root's frame rules: every byte below the arity is compared.
        let nested = item.depth > 0;
        let policy_mask = if nested {
            u64::MAX
        } else {
            self.written_policy_mask(item.root_k, written)
        };
        // v8: words at or above the callee's proven arity are the caller's spills (port side).
        let words = self
            .arity
            .and_then(|table| table.get(va))
            .map_or(expected.args.len(), |arity| {
                (arity as usize).min(expected.args.len())
            });
        for (index, &want) in expected.args.iter().enumerate().take(words) {
            let at = entry_esp.wrapping_add(4 + 4 * index as u32);
            // T3767: per byte, only what the function could have passed at this call: a byte of
            // the read-set (captured input), else one the function had written by then. Its
            // captured write-set says nothing about the call time (0x26D20 writes its own argument
            // slots above the entry ESP only after the call).
            let mut compare = [false; 4];
            for (byte, flag) in compare.iter_mut().enumerate() {
                let addr = u64::from(at) + byte as u64;
                let bit = index * 4 + byte;
                *flag = if self.in_read_set(addr) || nested {
                    true
                } else if self.policy == ArgPolicy::Strict {
                    self.knowable(at)
                } else {
                    bit < 64 && policy_mask & (1 << bit) != 0
                };
            }
            if !compare.contains(&true) {
                continue;
            }
            let got = self.word(at);
            let differs = match got {
                None => true,
                Some(value) => (0..4).any(|byte| {
                    compare[byte] && (value >> (8 * byte)) as u8 != (want >> (8 * byte)) as u8
                }),
            };
            // v6 (INFERRED): an fpo_leaf root pushes EBP before a call. The lifted body holds the
            // entry seh_ebp there, hardware and the port the real entry EBP.
            // v7 (INFERRED): an earlier callee leaked its g_ebp into the lifted body, which pushes
            // it where hardware and the port push the real entry EBP.
            let leaked = self.leak_rule
                && crate::compare::leaked_ebp(&item.frame[..item.local]) == Some(want);
            let lifted_seh_ebp = !nested
                && got == Some(self.root_ebp)
                && ((self.fpo_leaf && self.seh_ebp == Some(want)) || leaked);
            // v11 (INFERRED): a pointer into the root's own stack frame is a frame layout, not a
            // value: the port may reserve a different frame than the original.
            let frame_relative = compare == [true; 4]
                && got.is_some_and(|value| {
                    self.stack_pointer_offset(want, expected.entry_regs.esp)
                        .is_some_and(|offset| {
                            self.stack_pointer_offset(value, entry_esp) == Some(offset)
                        })
                });
            if differs && !lifted_seh_ebp && !frame_relative {
                let got = got.map_or("unmapped".to_string(), |value| format!("{value:#x}"));
                return Some(format!(
                    "callee_mismatch@{at_k}: {va:#x} arg {index} expected {want:#x}, got {got}{frame}"
                ));
            }
        }
        None
    }

    /// The final callee verdict: the first mismatch, else a captured call that never came.
    pub fn finish(&self) -> Option<String> {
        if let Some(text) = &self.mismatch {
            return Some(text.clone());
        }
        let k = self.skip_inlined(self.next, None);
        let (at_k, frame) = self.label(k);
        self.items.get(k).map(|missing| {
            format!(
                "callee_mismatch@{at_k}: missing call to {:#x} ({} of {} made){frame}",
                missing.record.va,
                at_k,
                self.entry.callees.len()
            )
        })
    }
}

impl GuestCall for CalleeStub<'_> {
    fn seh_ebp(&self) -> Option<u32> {
        self.seh_ebp
    }

    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        self.dispatch(va, regs, Convention::BridgePushesReturn);
    }
}
