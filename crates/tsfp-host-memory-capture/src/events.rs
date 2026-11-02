// SPDX-License-Identifier: GPL-3.0-or-later
//! Strict host-memory event envelope. A valid envelope is not a conformance
//! verdict: the producer must still establish complete boundary coverage.

pub const IDENTITY: u32 = 0;
pub const EMPTY_REGISTRY: u32 = 1;
pub const CALL_BEGIN: u32 = 2;
pub const MEMORY_READ: u32 = 3;
pub const MEMORY_WRITE: u32 = 4;
pub const PLACEMENT: u32 = 5;
pub const CALL_END: u32 = 6;
const MAX_DEPTH: usize = 64;

#[derive(Debug, PartialEq, Eq)]
pub enum Invalid {
    Identity,
    InitialState,
    UnknownEvent,
    UnsupportedCall,
    InvalidPayload,
    InterleavedCalls,
    EventOutsideCall,
    IncompleteCall,
    CountMismatch,
    NoCalls,
}

fn u32_at(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}
fn u64_at(data: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap())
}
struct Call {
    id: u64,
    thread: u64,
    reads: u32,
    writes: u32,
    placements: u32,
    outputs: usize,
}

/// Require exact identity and a producer-verified empty boot registry. Calls
/// may nest on the same thread; cross-thread overlap, unknown kinds, missing
/// events and incomplete calls are refused. This checks structure only.
pub fn validate(
    events: &[(u32, Vec<u8>)],
    expected_identity: &[u8],
    supported_ops: &[(u32, usize, usize)],
) -> Result<usize, Invalid> {
    if expected_identity.is_empty()
        || events
            .first()
            .is_none_or(|(k, d)| *k != IDENTITY || d != expected_identity)
    {
        return Err(Invalid::Identity);
    }
    if events
        .get(1)
        .is_none_or(|(k, d)| *k != EMPTY_REGISTRY || !d.is_empty())
    {
        return Err(Invalid::InitialState);
    }
    let mut stack: Vec<Call> = Vec::new();
    let mut last_id = 0u64;
    let mut calls = 0usize;
    for (kind, data) in &events[2..] {
        match *kind {
            CALL_BEGIN => {
                // id, parent id, thread id (u64), op and arg count (u32), args (u32).
                if data.len() < 32 {
                    return Err(Invalid::InvalidPayload);
                }
                let id = u64_at(data, 0);
                let parent = u64_at(data, 8);
                let thread = u64_at(data, 16);
                if id <= last_id || thread == 0 || stack.len() >= MAX_DEPTH {
                    return Err(Invalid::InvalidPayload);
                }
                match stack.last() {
                    Some(call) if call.id == parent && call.thread == thread => {}
                    None if parent == 0 => {}
                    _ => return Err(Invalid::InterleavedCalls),
                }
                let op = u32_at(data, 24);
                let count = u32_at(data, 28) as usize;
                let Some((_, arity, outputs)) = supported_ops
                    .iter()
                    .find(|(candidate, _, _)| *candidate == op)
                else {
                    return Err(Invalid::UnsupportedCall);
                };
                if count != *arity || count > 16 || *outputs > 16 || data.len() != 32 + count * 4 {
                    return Err(Invalid::InvalidPayload);
                }
                last_id = id;
                stack.push(Call {
                    id,
                    thread,
                    reads: 0,
                    writes: 0,
                    placements: 0,
                    outputs: *outputs,
                });
            }
            MEMORY_READ | MEMORY_WRITE => {
                let Some(call) = stack.last_mut() else {
                    return Err(Invalid::EventOutsideCall);
                };
                // call id, guest address, length, then the observed bytes.
                if data.len() < 16 || u64_at(data, 0) != call.id {
                    return Err(Invalid::InvalidPayload);
                }
                let length = u32_at(data, 12) as usize;
                if length > super::MAX_RECORD - 16
                    || u64::from(u32_at(data, 8)) + length as u64 > (1u64 << 32)
                    || data.len() != 16 + length
                {
                    return Err(Invalid::InvalidPayload);
                }
                let counter = if *kind == MEMORY_READ {
                    &mut call.reads
                } else {
                    &mut call.writes
                };
                *counter = counter.checked_add(1).ok_or(Invalid::CountMismatch)?;
            }
            PLACEMENT => {
                let Some(call) = stack.last_mut() else {
                    return Err(Invalid::EventOutsideCall);
                };
                // call id, kind (u32), length (u64), requested address/result (u32).
                if data.len() != 28 || u64_at(data, 0) != call.id || u32_at(data, 8) > 1 {
                    return Err(Invalid::InvalidPayload);
                }
                call.placements = call
                    .placements
                    .checked_add(1)
                    .ok_or(Invalid::CountMismatch)?;
            }
            CALL_END => {
                let Some(call) = stack.pop() else {
                    return Err(Invalid::EventOutsideCall);
                };
                // call id, result, read/write/placement counts, output count and words.
                if data.len() < 28
                    || u64_at(data, 0) != call.id
                    || u32_at(data, 24) as usize != call.outputs
                    || data.len() != 28 + call.outputs * 4
                {
                    return Err(Invalid::InvalidPayload);
                }
                if [call.reads, call.writes, call.placements]
                    != [u32_at(data, 12), u32_at(data, 16), u32_at(data, 20)]
                {
                    return Err(Invalid::CountMismatch);
                }
                calls += 1;
            }
            _ => return Err(Invalid::UnknownEvent),
        }
    }
    if !stack.is_empty() {
        return Err(Invalid::IncompleteCall);
    }
    if calls == 0 {
        return Err(Invalid::NoCalls);
    }
    Ok(calls)
}
