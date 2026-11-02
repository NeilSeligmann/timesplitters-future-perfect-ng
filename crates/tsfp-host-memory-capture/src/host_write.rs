// SPDX-License-Identifier: GPL-3.0-or-later
//! Replays host-helper write witnesses (T3906) onto Rust-owned bytes. It covers writes made through the C
//! `kernel_guest_write_*` helpers only: recompiled guest stores are not witnessed, so a clean replay never
//! proves whole-memory conformance and the stream stays unsealable in production.
use crate::events;
use std::collections::BTreeMap;

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    /// Wrapper call has the wrong arity, result, or not exactly one matching write.
    Wrapper(u64),
    /// A write or read fell outside every placed region, or a read disagreed with replayed bytes.
    Access(u64),
    /// Truncated or inconsistent event layout.
    Layout,
}

fn word(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}
fn wide(data: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_le_bytes(data.get(at..at + 8)?.try_into().ok()?))
}

/// Placed regions keyed by base; the replayed bytes of each.
pub type Regions = BTreeMap<u32, Vec<u8>>;

fn locate(regions: &mut Regions, addr: u32, len: usize) -> Option<(&mut Vec<u8>, usize)> {
    let (base, bytes) = regions.range_mut(..=addr).next_back()?;
    let offset = (addr - base) as usize;
    (offset.checked_add(len)? <= bytes.len()).then_some((bytes, offset))
}

/// Apply every placement and memory event in order. `write_op` is the host-write wrapper operation id
/// (`[addr, length]`, exactly one MEMORY_WRITE with that range, result 1). Returns the replayed regions.
pub fn replay(events: &[(u32, Vec<u8>)], write_op: u32) -> Result<Regions, Error> {
    replay_ops(events, &[write_op])
}

/// As [`replay`] for several writer classes that share the wrapper shape (host helper, lifted store).
pub fn replay_ops(events: &[(u32, Vec<u8>)], write_ops: &[u32]) -> Result<Regions, Error> {
    let mut regions = Regions::new();
    // (call id, Some((addr, len, writes seen)) for wrapper calls)
    let mut stack: Vec<(u64, Option<(u32, u32, u32)>)> = Vec::new();
    for (kind, data) in events.iter().skip(2) {
        match *kind {
            events::CALL_BEGIN => {
                let id = wide(data, 0).ok_or(Error::Layout)?;
                let (op, count) = (
                    word(data, 24).ok_or(Error::Layout)?,
                    word(data, 28).ok_or(Error::Layout)?,
                );
                let wrapper = if write_ops.contains(&op) {
                    if count != 2 {
                        return Err(Error::Wrapper(id));
                    }
                    Some((
                        word(data, 32).ok_or(Error::Layout)?,
                        word(data, 36).ok_or(Error::Layout)?,
                        0,
                    ))
                } else {
                    None
                };
                stack.push((id, wrapper));
            }
            events::PLACEMENT => {
                let (pkind, length, address, result) = (
                    word(data, 8).ok_or(Error::Layout)?,
                    wide(data, 12).ok_or(Error::Layout)?,
                    word(data, 20).ok_or(Error::Layout)?,
                    word(data, 24).ok_or(Error::Layout)?,
                );
                if pkind == 0 && result != 0 {
                    let size = usize::try_from(length).map_err(|_| Error::Layout)?;
                    if size > 1 << 24 || regions.insert(result, vec![0; size]).is_some() {
                        return Err(Error::Layout);
                    }
                } else if pkind == 1 && regions.remove(&address).is_none() {
                    return Err(Error::Layout);
                }
            }
            events::MEMORY_READ | events::MEMORY_WRITE => {
                let (id, addr) = (
                    wide(data, 0).ok_or(Error::Layout)?,
                    word(data, 8).ok_or(Error::Layout)?,
                );
                let len = word(data, 12).ok_or(Error::Layout)? as usize;
                let bytes = data
                    .get(16..)
                    .filter(|b| b.len() == len)
                    .ok_or(Error::Layout)?;
                let write = *kind == events::MEMORY_WRITE;
                if let Some((open, wrapper)) = stack.last_mut() {
                    if let Some((wa, wl, seen)) = wrapper {
                        if !write || *open != id || (*wa, *wl) != (addr, len as u32) || *seen != 0 {
                            return Err(Error::Wrapper(id));
                        }
                        *seen = 1;
                    }
                }
                let (region, at) = locate(&mut regions, addr, len).ok_or(Error::Access(id))?;
                if write {
                    region[at..at + len].copy_from_slice(bytes);
                } else if &region[at..at + len] != bytes {
                    return Err(Error::Access(id));
                }
            }
            events::CALL_END => {
                let id = wide(data, 0).ok_or(Error::Layout)?;
                let (open, wrapper) = stack.pop().ok_or(Error::Layout)?;
                if open != id {
                    return Err(Error::Layout);
                }
                if let Some((_, _, seen)) = wrapper {
                    if seen != 1 || word(data, 8) != Some(1) {
                        return Err(Error::Wrapper(id));
                    }
                }
            }
            _ => {}
        }
    }
    if stack.is_empty() {
        Ok(regions)
    } else {
        Err(Error::Layout)
    }
}
