// SPDX-License-Identifier: GPL-3.0-or-later
//! Guest VA to lifted function, the Rust twin of the generated `recomp_dispatch.c`.
//!
//! The C file (writer: `_write_dispatch_table` in `third_party/xboxrecomp/tools/recomp/translator.py`)
//! holds a table sorted by VA and answers `recomp_lookup` two ways: a binary search until
//! `recomp_dispatch_init` runs, then a flat array indexed by `va - base` (the host always calls
//! init, `src/host/main.c`). [`DispatchTable::lookup`] is the flat mode, [`DispatchTable::lookup_search`]
//! the search mode, each reproducing the C answer including for a duplicated VA (flat: the last
//! row wins because init fills in table order; search: whichever row the bisection meets).
//!
//! Memory: one `u32` slot per byte of guest code span (C uses one pointer), zero-allocated so
//! untouched pages are never committed.

/// Largest accepted span (base to last VA inclusive). The title's span is about 4 MiB; a larger
/// one means a corrupt table, and refusing it keeps a bad install from allocating gigabytes.
pub const MAX_SPAN: u64 = 64 * 1024 * 1024;

/// Why a table was refused. The C host then keeps its own dispatch (the switch fails closed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableError {
    /// Rows are not in ascending VA order; the C binary search would answer wrongly past `index`.
    Unsorted {
        index: usize,
        previous: u32,
        va: u32,
    },
    /// The span from the first to the last VA exceeds [`MAX_SPAN`].
    SpanTooLarge { base: u32, last: u32 },
}

impl core::fmt::Display for TableError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Unsorted {
                index,
                previous,
                va,
            } => write!(
                formatter,
                "dispatch row {index} (0x{va:08X}) is below the previous row (0x{previous:08X})"
            ),
            Self::SpanTooLarge { base, last } => write!(
                formatter,
                "dispatch span 0x{base:08X}..=0x{last:08X} exceeds {MAX_SPAN} bytes"
            ),
        }
    }
}

/// The dispatch table. `F` is the function handle: a C function pointer in the host binding, a
/// plain id in tests.
#[derive(Clone, Debug)]
pub struct DispatchTable<F> {
    entries: Vec<(u32, F)>,
    base: u32,
    /// Slot per VA in `base..base + span`: 0 empty, otherwise 1 + the index of the winning row.
    slots: Vec<u32>,
    duplicates: usize,
}

impl<F: Copy> DispatchTable<F> {
    /// Builds the table from rows in ascending VA order (the generator's order). Equal VAs are
    /// accepted and counted, as the C table accepts them.
    pub fn new(entries: Vec<(u32, F)>) -> Result<Self, TableError> {
        let mut duplicates = 0usize;
        for index in 1..entries.len() {
            let (previous, va) = (entries[index - 1].0, entries[index].0);
            if va < previous {
                return Err(TableError::Unsorted {
                    index,
                    previous,
                    va,
                });
            }
            duplicates += usize::from(va == previous);
        }
        let (base, span) = match (entries.first(), entries.last()) {
            (Some(&(first, _)), Some(&(last, _))) => {
                let span = u64::from(last) - u64::from(first) + 1;
                if span > MAX_SPAN {
                    return Err(TableError::SpanTooLarge { base: first, last });
                }
                (first, span as usize)
            }
            _ => (0, 0),
        };
        let mut slots = vec![0u32; span];
        for (index, &(va, _)) in entries.iter().enumerate() {
            // Table order, so a duplicated VA ends on its last row, as the C init loop does.
            slots[(va - base) as usize] = index as u32 + 1;
        }
        Ok(Self {
            entries,
            base,
            slots,
            duplicates,
        })
    }

    /// Flat-mode lookup, the answer of C `recomp_lookup` after `recomp_dispatch_init`.
    #[inline]
    pub fn lookup(&self, va: u32) -> Option<F> {
        // Wrapping like the C unsigned offset: a VA below the base becomes huge and fails the bound.
        let slot = *self.slots.get(va.wrapping_sub(self.base) as usize)?;
        let index = slot.checked_sub(1)?;
        Some(self.entries[index as usize].1)
    }

    /// Search-mode lookup, the answer of C `recomp_lookup` before `recomp_dispatch_init`
    /// (same bisection, so equal VAs resolve to the same row as in C).
    pub fn lookup_search(&self, va: u32) -> Option<F> {
        let (mut low, mut high) = (0usize, self.entries.len());
        while low < high {
            let middle = low + (high - low) / 2;
            let row = self.entries[middle];
            match row.0.cmp(&va) {
                core::cmp::Ordering::Less => low = middle + 1,
                core::cmp::Ordering::Greater => high = middle,
                core::cmp::Ordering::Equal => return Some(row.1),
            }
        }
        None
    }

    /// Rows in table order.
    pub fn entries(&self) -> &[(u32, F)] {
        &self.entries
    }

    /// Row count, C `recomp_get_count`.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// First VA of the flat span (C `g_flat_base`).
    pub fn base(&self) -> u32 {
        self.base
    }

    /// Flat span in VAs (C `g_flat_span`).
    pub fn span(&self) -> usize {
        self.slots.len()
    }

    /// Rows whose VA equals the previous row's.
    pub fn duplicate_count(&self) -> usize {
        self.duplicates
    }

    /// The deterministic probe list of [`verify_against`]: every row VA and both neighbours, then
    /// the span edges and the address-space ends.
    pub fn probe_addresses(&self) -> impl Iterator<Item = u32> + '_ {
        let edges = [
            0u32,
            self.base.wrapping_sub(1),
            self.base.wrapping_add(self.slots.len() as u32),
            u32::MAX,
        ];
        self.entries
            .iter()
            .flat_map(|&(va, _)| [va, va.wrapping_sub(1), va.wrapping_add(1)])
            .chain(edges)
    }
}

/// Result of comparing the Rust table with another dispatcher.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Agreement {
    /// Table rows covered (each is probed with both neighbours).
    pub rows: u64,
    /// Probes compared.
    pub checked: u64,
    /// Probes where the two answers differ.
    pub mismatches: u64,
    /// The first differing VA in probe order.
    pub first_mismatch: Option<u32>,
}

impl Agreement {
    /// True when at least one table row was covered and no probe differed (an empty table
    /// proves nothing, so it never agrees).
    pub fn agrees(&self) -> bool {
        self.rows != 0 && self.mismatches == 0
    }
}

/// Compares [`DispatchTable::lookup`] with `oracle` (the C `recomp_lookup` in the host) over
/// [`DispatchTable::probe_addresses`]: every table row, its neighbours and the span edges.
pub fn verify_against<F, O>(table: &DispatchTable<F>, mut oracle: O) -> Agreement
where
    F: Copy + PartialEq,
    O: FnMut(u32) -> Option<F>,
{
    let mut agreement = Agreement {
        rows: table.len() as u64,
        ..Agreement::default()
    };
    for va in table.probe_addresses() {
        agreement.checked += 1;
        if table.lookup(va) != oracle(va) {
            agreement.mismatches += 1;
            agreement.first_mismatch.get_or_insert(va);
        }
    }
    agreement
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The awkward table of `third_party/xboxrecomp/tools/recomp/test_dispatch_flat.py`.
    fn awkward() -> DispatchTable<u16> {
        DispatchTable::new(vec![(0x11000, 1), (0x11004, 2), (0x12000, 3), (0x203FC, 4)]).unwrap()
    }

    #[test]
    fn known_addresses_resolve_in_both_modes() {
        let table = awkward();
        assert_eq!((table.base(), table.span()), (0x11000, 0xF3FD));
        for (va, id) in [(0x11000, 1), (0x11004, 2), (0x12000, 3), (0x203FC, 4)] {
            assert_eq!(table.lookup(va), Some(id), "flat {va:#x}");
            assert_eq!(table.lookup_search(va), Some(id), "search {va:#x}");
        }
    }

    #[test]
    fn absent_addresses_miss_in_both_modes() {
        let table = awkward();
        let absent = [
            0x0,
            0x4,
            0x10FFF,
            0x11001,
            0x11008,
            0x11FFF,
            0x203FB,
            0x203FD,
            0x20400,
            0xFFFF_FFFF,
            0xFE00_0000,
            0x7FFF_FFFF,
            0x8000_0000,
        ];
        for va in absent {
            assert_eq!(table.lookup(va), None, "flat {va:#x}");
            assert_eq!(table.lookup_search(va), None, "search {va:#x}");
        }
    }

    #[test]
    fn empty_table_answers_nothing() {
        let table = DispatchTable::<u16>::new(Vec::new()).unwrap();
        assert!(table.is_empty());
        assert_eq!(
            (table.span(), table.lookup(0), table.lookup_search(0)),
            (0, None, None)
        );
        assert!(
            !verify_against(&table, |_| None).agrees(),
            "an empty comparison must not pass"
        );
    }

    #[test]
    fn unsorted_rows_are_refused() {
        let error = DispatchTable::new(vec![(0x2000, 1u8), (0x1000, 2)]).unwrap_err();
        assert_eq!(
            error,
            TableError::Unsorted {
                index: 1,
                previous: 0x2000,
                va: 0x1000
            }
        );
    }

    #[test]
    fn oversized_span_is_refused() {
        let error = DispatchTable::new(vec![(0, 1u8), (0x1000_0000, 2)]).unwrap_err();
        assert!(matches!(error, TableError::SpanTooLarge { .. }));
    }

    #[test]
    fn duplicate_rows_follow_the_c_modes() {
        // Flat: last row wins (the C init loop). Search: the bisection of 3 rows meets index 1.
        let table = DispatchTable::new(vec![(0x10, 1u8), (0x10, 2), (0x10, 3)]).unwrap();
        assert_eq!(table.duplicate_count(), 2);
        assert_eq!(table.lookup(0x10), Some(3));
        assert_eq!(table.lookup_search(0x10), Some(2));
    }

    #[test]
    fn verify_counts_every_probe_and_reports_the_first_difference() {
        let table = awkward();
        let same = verify_against(&table, |va| table.lookup_search(va));
        assert_eq!(
            same,
            Agreement {
                rows: 4,
                checked: 4 * 3 + 4,
                mismatches: 0,
                first_mismatch: None
            }
        );
        assert!(same.agrees());
        let broken = verify_against(&table, |va| {
            if va == 0x12000 {
                None
            } else {
                table.lookup(va)
            }
        });
        assert_eq!(
            (broken.mismatches, broken.first_mismatch),
            (1, Some(0x12000))
        );
        assert!(!broken.agrees());
    }

    #[test]
    fn flat_and_search_agree_on_a_dense_pseudo_random_table() {
        // Deterministic LCG, no clock or RNG crate: 2,000 rows over a 1 MiB span.
        let mut state = 0x1234_5678u32;
        let mut vas: Vec<u32> = (0..2_000)
            .map(|_| {
                state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                0x12000 + (state >> 12)
            })
            .collect();
        vas.sort_unstable();
        vas.dedup();
        let table = DispatchTable::new(vas.iter().map(|&va| (va, va ^ 0x5A5A)).collect()).unwrap();
        let agreement = verify_against(&table, |va| table.lookup_search(va));
        assert!(agreement.agrees(), "{agreement:?}");
        assert_eq!(agreement.checked, 3 * vas.len() as u64 + 4);
    }
}
