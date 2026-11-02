// SPDX-License-Identifier: GPL-3.0-or-later
//! compare v10 (T3839): recursive self-calls with nested callees.
//!
//! The capture is flat per frame: a root's callee list holds its direct calls only, and a
//! recursive call to the root's own VA is one opaque slot (its net write-set and exit
//! registers). The callees of that inner activation are in the inner activation's own record,
//! which the corpus keeps as another entry of the same VA. A port that runs the recursion
//! natively (the self VA declared in `--inlined-callees`, e.g. 0x8F5E0 flattened by
//! `render_prep::model_commands`) makes the inner frame's guest calls (0x18EA90) itself, so
//! the old flat sequence check saw them as unexpected calls of the root.
//!
//! `expand` turns the root's callee list into one sequence in call order: a self slot whose
//! inner activation is found among the spool's entries (`NestedFrames::find`, keyed on the full
//! entry registers, exit registers and net write-set of the slot) is followed by the inner
//! frame's own callees, recursively, each tagged with its frame (depth, frame-local index).
//! The k-th activation of the port pairs with the k-th captured one because both are walked in
//! call order. A self slot whose inner record is not in the corpus stays opaque (old rule), so
//! an unresolved frame can only make a port diverge, never pass.
use crate::spool::{CalleeRecord, Entry};

/// Deepest recursion expanded; deeper slots stay opaque (fail closed: their callees are then
/// unexpected). The 0x8F5E0 corpus reaches depth 27 (MEASURED 2026-10-27).
pub const MAX_DEPTH: u32 = 256;

/// The entries of one spool that can stand for an inner activation of the root VA.
#[derive(Clone, Debug, Default)]
pub struct NestedFrames<'a> {
    entries: Vec<&'a Entry>,
}

impl<'a> NestedFrames<'a> {
    /// Unflagged records of `va` (flagged ones never get here after export, checked anyway).
    pub fn new(va: u32, entries: impl IntoIterator<Item = &'a Entry>) -> Self {
        Self {
            entries: entries
                .into_iter()
                .filter(|entry| entry.va == va && entry.flags == 0)
                .collect(),
        }
    }

    /// The record of the activation `slot` called: same VA, entry registers, exit registers and
    /// net write-set. Entry registers alone are not enough (MEASURED: 29 of 254 register matches
    /// in the 0x8F5E0 corpus wrote other heap objects).
    pub fn find(&self, slot: &CalleeRecord) -> Option<&'a Entry> {
        self.entries.iter().copied().find(|entry| {
            entry.va == slot.va
                && entry.entry_regs == slot.entry_regs
                && entry.exit_regs == slot.exit_regs
                && entry.write_runs == slot.write_runs
        })
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// One captured call in the expanded sequence.
#[derive(Clone, Copy, Debug)]
pub struct Slot<'a> {
    pub record: &'a CalleeRecord,
    /// The callee list of the frame that made the call (the root's at depth 0).
    pub frame: &'a [CalleeRecord],
    /// Index of `record` in `frame`.
    pub local: usize,
    /// 0 = a call of the root, d = a call of the d-th nested activation.
    pub depth: u32,
    /// Index of the depth-0 slot this call belongs to (itself at depth 0): the `@k` of details.
    pub root_k: usize,
    /// A self slot with a resolved inner frame: the items before `frame_end` are its frame.
    pub frame_end: Option<usize>,
}

/// The callees of `entry`, with every resolvable self slot of `self_va` followed by its inner
/// frame (`self_va` None: no expansion, the plain captured list).
pub fn expand<'a>(
    entry: &'a Entry,
    self_va: Option<u32>,
    frames: &NestedFrames<'a>,
) -> Vec<Slot<'a>> {
    let mut out = Vec::with_capacity(entry.callees.len());
    push_frame(&entry.callees, 0, None, self_va, frames, &mut out);
    out
}

fn push_frame<'a>(
    callees: &'a [CalleeRecord],
    depth: u32,
    root_k: Option<usize>,
    self_va: Option<u32>,
    frames: &NestedFrames<'a>,
    out: &mut Vec<Slot<'a>>,
) {
    for (local, record) in callees.iter().enumerate() {
        let at = out.len();
        let root_k = root_k.unwrap_or(local);
        out.push(Slot {
            record,
            frame: callees,
            local,
            depth,
            root_k,
            frame_end: None,
        });
        if Some(record.va) != self_va || depth + 1 > MAX_DEPTH {
            continue;
        }
        if let Some(inner) = frames.find(record) {
            push_frame(
                &inner.callees,
                depth + 1,
                Some(root_k),
                self_va,
                frames,
                out,
            );
            out[at].frame_end = Some(out.len());
        }
    }
}
