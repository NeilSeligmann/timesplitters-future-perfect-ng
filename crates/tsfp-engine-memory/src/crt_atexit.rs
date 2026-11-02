// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded CRT atexit-table growth at `0x3C8A84`.
//!
//! This owns the table bookkeeping only. The host supplies the existing heap-size and realloc
//! operations; their internal allocation metadata and failure behavior remain outside this
//! caller-specific contract.

use crate::MemoryFault;

const TABLE_END: u32 = 0x7EB844;
const TABLE_BASE: u32 = 0x7EB848;
const FIRST_GROWTH_LIMIT: u32 = 0x800;
const RETRY_CAPACITY_OFFSET: u32 = 0x10;

/// Guest memory and heap operations needed by the retained CRT registration caller.
pub trait AtexitHost {
    fn read32(&mut self, address: u32) -> Result<u32, MemoryFault>;
    fn write32(&mut self, address: u32, value: u32) -> Result<(), MemoryFault>;

    /// Query the usable size for this process-heap handle and pointer.
    fn heap_size(&mut self, heap_handle: u32, pointer: u32) -> Result<u32, MemoryFault>;

    /// Reallocate one specific CRT table block. A zero result leaves the caller's old block live.
    fn reallocate(&mut self, pointer: u32, requested_size: u32) -> Result<u32, MemoryFault>;
}

/// Append one CRT exit callback to the table rooted at `0x7EB848`.
///
/// If the current capacity is insufficient, the caller requests `capacity + min(capacity, 0x800)`
/// bytes, then retries with `capacity + 0x10` if realloc returns null. Both failed attempts leave
/// the published base/end and the old table contents untouched. After success, the old used dword
/// count is preserved at the returned base before publishing the new entry and end pointer.
///
/// The `heap_handle` and heap implementation are supplied by the C-runtime boundary; this is not
/// a generic game-heap realloc contract.
pub fn register_atexit<H: AtexitHost>(
    host: &mut H,
    heap_handle: u32,
    entry: u32,
) -> Result<u32, MemoryFault> {
    // Preserve the original order: query capacity using the base first, then reload end and base
    // after the heap call in case that boundary changed guest state.
    let size_pointer = host.read32(TABLE_BASE)?;
    let capacity = host.heap_size(heap_handle, size_pointer)?;
    let mut end = host.read32(TABLE_END)?;
    let old_base = host.read32(TABLE_BASE)?;
    let used_bytes = end.wrapping_sub(old_base);

    if capacity < used_bytes.wrapping_add(4) {
        let requested = capacity.wrapping_add(capacity.min(FIRST_GROWTH_LIMIT));
        let new_base = match host.reallocate(old_base, requested)? {
            0 => host.reallocate(old_base, capacity.wrapping_add(RETRY_CAPACITY_OFFSET))?,
            address => address,
        };

        // REA revision 2 establishes this failure-preservation behavior only for this atexit
        // caller. In particular, do not publish either global or write the new entry on failure.
        if new_base == 0 {
            return Ok(0);
        }

        end = new_base.wrapping_add(used_bytes & !3);
        host.write32(TABLE_BASE, new_base)?;
        host.write32(TABLE_END, end)?;
    }

    host.write32(end, entry)?;
    host.write32(TABLE_END, end.wrapping_add(4))?;
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, VecDeque};

    #[derive(Default)]
    struct Host {
        words: BTreeMap<u32, u32>,
        capacity: u32,
        results: VecDeque<u32>,
        reallocations: Vec<(u32, u32)>,
        writes: Vec<(u32, u32)>,
    }

    impl AtexitHost for Host {
        fn read32(&mut self, address: u32) -> Result<u32, MemoryFault> {
            Ok(self.words.get(&address).copied().unwrap_or(0))
        }

        fn write32(&mut self, address: u32, value: u32) -> Result<(), MemoryFault> {
            self.words.insert(address, value);
            self.writes.push((address, value));
            Ok(())
        }

        fn heap_size(&mut self, _heap_handle: u32, _pointer: u32) -> Result<u32, MemoryFault> {
            Ok(self.capacity)
        }

        fn reallocate(&mut self, pointer: u32, requested_size: u32) -> Result<u32, MemoryFault> {
            self.reallocations.push((pointer, requested_size));
            Ok(self.results.pop_front().unwrap_or(0))
        }
    }

    fn initialized(capacity: u32, base: u32, used: u32) -> Host {
        let mut host = Host {
            capacity,
            ..Host::default()
        };
        host.words.insert(TABLE_BASE, base);
        host.words.insert(TABLE_END, base.wrapping_add(used));
        host
    }

    #[test]
    fn atexit_growth_failure_keeps_old_table_and_does_not_publish_entry() {
        let base = 0x1000;
        let used = 12;
        let mut host = initialized(8, base, used);
        host.words.insert(base + used, 0xAABB_CCDD);
        host.results.extend([0, 0]);

        assert_eq!(register_atexit(&mut host, 7, 0x1234).unwrap(), 0);
        assert_eq!(host.words[&TABLE_BASE], base);
        assert_eq!(host.words[&TABLE_END], base + used);
        assert_eq!(host.words[&(base + used)], 0xAABB_CCDD);
        assert_eq!(host.reallocations, [(base, 16), (base, 24)]);
        assert!(host.writes.is_empty());
    }

    #[test]
    fn atexit_growth_publishes_relocated_table_after_success() {
        let base = 0x1000;
        let relocated = 0x4000;
        let used = 12;
        let mut host = initialized(8, base, used);
        host.results.extend([relocated]);

        assert_eq!(register_atexit(&mut host, 7, 0x1234).unwrap(), 0x1234);
        assert_eq!(host.words[&TABLE_BASE], relocated);
        assert_eq!(host.words[&TABLE_END], relocated + used + 4);
        assert_eq!(host.words[&(relocated + used)], 0x1234);
        assert_eq!(
            host.writes,
            [
                (TABLE_BASE, relocated),
                (TABLE_END, relocated + used),
                (relocated + used, 0x1234),
                (TABLE_END, relocated + used + 4)
            ]
        );
    }
}
