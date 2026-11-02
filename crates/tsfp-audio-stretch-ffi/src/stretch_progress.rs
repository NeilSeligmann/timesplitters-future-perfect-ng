// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline bookkeeping after a successful stretch block.

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub position_q16: u64,
    pub tail: usize,
    pub head: usize,
    pub fill: usize,
    pub consumed: usize,
    pub hold_run: usize,
    pub blocks: u64,
}

/// Bookkeeping after a successful return from stretching to real-time audio.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Leave {
    pub head: usize,
    pub fill: usize,
    pub consumed: usize,
    pub fifo_pos: usize,
    pub fifo_len: usize,
    pub active: bool,
}

/// Sink bookkeeping around a successful engine prime.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prime {
    pub fill: usize,
    pub fifo_pos: usize,
    pub fifo_len: usize,
    pub tail: usize,
    pub position_q16: u64,
    pub hold_run: usize,
    pub gain_q16: u32,
    pub active: bool,
    pub fade_in_pending: bool,
}

/// Apply priming only when the sink would request it and the engine succeeds.
pub fn prime(state: &mut Prime, overlap: usize, ratio: u32, engine_ok: bool) -> bool {
    let Some(required) = overlap.checked_mul(2) else {
        return false;
    };
    let Some(position) = (ratio as u64).checked_mul(overlap as u64) else {
        return false;
    };
    if state.active || state.fifo_len != 0 || ratio >= 65536 || state.fill < required || !engine_ok
    {
        return false;
    }
    state.fifo_pos = 0;
    state.fifo_len = overlap;
    state.fade_in_pending = false;
    state.gain_q16 = 65536;
    state.tail = overlap;
    state.position_q16 = position;
    state.hold_run = 0;
    state.active = true;
    true
}

/// Apply the transition after the engine has successfully produced its leave block.
pub fn leave(state: &mut Leave, capacity: usize, tail: usize, overlap: usize) -> bool {
    let Some(requested) = tail.checked_add(overlap) else {
        return false;
    };
    if requested > state.fill {
        return false;
    }
    let Some(consumed) = state.consumed.checked_add(requested) else {
        return false;
    };
    let mut next = *state;
    if crate::stretch_ring::consume(capacity, &mut next.head, &mut next.fill, requested).is_none() {
        return false;
    }
    next.consumed = consumed;
    next.fifo_pos = 0;
    next.fifo_len = overlap;
    next.active = false;
    *state = next;
    true
}

/// Apply the successful-next transition. Invalid or overflowing inputs do not
/// mutate the state. The caller supplies the engine's selected frame.
pub fn next(
    state: &mut Progress,
    capacity: usize,
    chosen: usize,
    overlap: usize,
    search: usize,
    ratio: u32,
) -> bool {
    let Some(position) = (ratio as u64)
        .checked_mul(overlap as u64)
        .and_then(|step| state.position_q16.checked_add(step))
    else {
        return false;
    };
    let Some(tail) = chosen.checked_add(overlap) else {
        return false;
    };
    let Some(blocks) = state.blocks.checked_add(1) else {
        return false;
    };
    let requested = (position >> 16).saturating_sub(search as u64) as usize;
    let mut next = *state;
    let Some(dropped) =
        crate::stretch_ring::consume(capacity, &mut next.head, &mut next.fill, requested)
    else {
        return false;
    };
    let Some(consumed) = state.consumed.checked_add(dropped) else {
        return false;
    };
    next.position_q16 = position - ((dropped as u64) << 16);
    next.tail = tail.saturating_sub(dropped);
    next.consumed = consumed;
    next.hold_run = 0;
    next.blocks = blocks;
    *state = next;
    true
}
