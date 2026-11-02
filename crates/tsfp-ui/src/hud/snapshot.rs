// SPDX-License-Identifier: GPL-3.0-or-later
//! Guest memory snapshots for the HUD view: a sparse set of dumped regions (`tools/hud_snapshot.py`
//! writes the `.hudsnap` container). Implements [`GuestMemory`], so every ported HUD function can
//! run on a recorded frame.

use crate::memory::GuestMemory;
use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};

/// Sparse guest memory built from dumped regions.
#[derive(Default)]
pub struct Snapshot {
    regions: BTreeMap<u32, Vec<u8>>,
    /// Bytes written after loading (the functions under test write guest records).
    overlay: HashMap<u32, u8>,
    /// Reads that hit no region (they return 0). A nonzero count means the dump missed a range.
    pub missing_reads: Cell<usize>,
}

impl Snapshot {
    pub fn from_regions(regions: BTreeMap<u32, Vec<u8>>) -> Self {
        Snapshot {
            regions,
            ..Snapshot::default()
        }
    }

    /// Parse the `HUDSNAP1` container: magic, u32 count, then (u32 address, u32 length, bytes)*.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 12 || &bytes[..8] != b"HUDSNAP1" {
            return Err("not a HUDSNAP1 file".to_owned());
        }
        let count = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        let mut at = 12;
        let mut regions = BTreeMap::new();
        for _ in 0..count {
            let header = bytes.get(at..at + 8).ok_or("truncated region header")?;
            let start = u32::from_le_bytes(header[..4].try_into().unwrap());
            let length = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
            at += 8;
            let data = bytes.get(at..at + length).ok_or("truncated region data")?;
            at += length;
            regions.insert(start, data.to_vec());
        }
        Ok(Snapshot::from_regions(regions))
    }

    fn lookup(&self, address: u32) -> Option<u8> {
        let (start, data) = self.regions.range(..=address).next_back()?;
        data.get(address.wrapping_sub(*start) as usize).copied()
    }
}

impl GuestMemory for Snapshot {
    fn read8(&self, address: u32) -> u8 {
        if let Some(byte) = self.overlay.get(&address) {
            return *byte;
        }
        self.lookup(address).unwrap_or_else(|| {
            self.missing_reads.set(self.missing_reads.get() + 1);
            0
        })
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.overlay.insert(address, value);
    }
}
