// SPDX-License-Identifier: GPL-3.0-or-later
//! T2062 (S3-4 of T2051): Rust-owned leaf state of the data/text and data/save systems.
//! Design: docs/t-s3-save-text.md. Stage is Own with write-through: the Rust structs are
//! authoritative, every Rust writer stores the changed bytes into the guest image in the same call,
//! so lifted readers of 0x7E7200.. and 0x7BEDC0.. still see the original layout (invariants 1 and 2
//! of docs/t-s3-state-design.md section 3.4).
use super::GuestMemory;
use super::generated_views::{TsfpVoiceBankOwned, TsfpVoiceBankView};
use super::values::GuestPtr;

/// First voice bank descriptor (`TsfpVoiceBank`, 0xC bytes) and the stride between the two banks.
pub const VOICE_BANK_BASE: u32 = 0x007e_7200;
pub const VOICE_BANK_STRIDE: u32 = 0x10;
pub const VOICE_BANK_COUNT: usize = 2;

/// Owner of both voice banks. Private to data/text: the lookup (0x461C0) is the only reader seen in
/// the static xref, the loader (0x46170 through 0x46030) the only writer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextBank {
    pub banks: [TsfpVoiceBankOwned; VOICE_BANK_COUNT],
}

fn bank_view(index: usize) -> TsfpVoiceBankView {
    TsfpVoiceBankView(VOICE_BANK_BASE + index as u32 * VOICE_BANK_STRIDE)
}

/// Borrowed sample cells addressed by a voice-bank descriptor. The descriptor
/// owns only the pointer; the cells remain live guest memory.
#[derive(Clone, Copy)]
struct SampleCells(GuestPtr<()>);

impl SampleCells {
    fn read<M: GuestMemory>(self, m: &M, index: u32) -> u32 {
        m.read32(self.0.0.wrapping_add(index.wrapping_mul(4)))
    }
}

impl TextBank {
    pub const EMPTY: Self = Self {
        banks: [TsfpVoiceBankOwned {
            samples: GuestPtr::new(0),
            count: 0,
            label_base: 0,
        }; VOICE_BANK_COUNT],
    };

    /// Mirror: copies the banks out of the guest image.
    pub fn load<M: GuestMemory>(m: &M) -> Self {
        Self {
            banks: core::array::from_fn(|i| bank_view(i).load(m)),
        }
    }

    /// Writes every bank back (declared fields only).
    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        for (i, bank) in self.banks.iter().enumerate() {
            bank_view(i).store(m, bank);
        }
    }

    /// First bank whose guest bytes differ from the owned value, with the field name.
    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<(usize, &'static str)> {
        (0..VOICE_BANK_COUNT).find_map(|i| bank_view(i).diff(m, &self.banks[i]).map(|f| (i, f)))
    }

    /// Rust writer with write-through: installs bank `index` (what 0x46030 does per call).
    pub fn set_bank<M: GuestMemory>(&mut self, m: &mut M, index: usize, bank: TsfpVoiceBankOwned) {
        self.banks[index] = bank;
        bank_view(index).store(m, &bank);
    }

    /// Port of 0x461C0 `lookup_by_id` over the owned banks. Bank 0 is consulted first and its
    /// sentinel (-1) falls through to bank 1; bank 1's value is returned as is. Ids with no entry
    /// (and id 0) give -1. Table cells are read from guest memory (`samples` points at them).
    pub fn lookup<M: GuestMemory>(&self, m: &M, id: u32) -> u32 {
        if id != 0 {
            let [first, second] = &self.banks;
            let index = id.wrapping_sub(first.label_base as u32) as i32;
            if index >= 0 && index < first.count {
                let value = SampleCells(first.samples).read(m, index as u32);
                if value != u32::MAX {
                    return value;
                }
            }
            let index = id.wrapping_sub(second.label_base as u32) as i32;
            if index >= 0 && index < second.count {
                return SampleCells(second.samples).read(m, index as u32);
            }
        }
        u32::MAX
    }
}

/// Local slot table: 16 entries of `SLOT_STRIDE` bytes, dword 0 of each is the pointer to its
/// allocation record (0 = inactive).
pub const SLOT_TABLE: u32 = 0x007b_edc0;
pub const SLOT_STRIDE: u32 = 0x1f54;
pub const SLOT_COUNT: usize = 16;
/// Allocation record fields touched by the writers (record stride 0x40, table at 0x7BC9C0).
pub const RECORD_IN_USE: u32 = 0x34;
pub const RECORD_BACK_LINK: u32 = 0x3c;
/// Record kind word (T2091, INFERRED from 0x69D90): `RECORD_KIND_FRESH` asks the attach path to reset the
/// slot profile, `RECORD_KIND_NO_CHANGE_FLAG` skips the changed flag.
pub const RECORD_KIND: u32 = 0x24;
pub const RECORD_KIND_FRESH: u32 = 0xffff_ffff;
pub const RECORD_KIND_NO_CHANGE_FLAG: u32 = 0xb;
/// Changed flag set when an existing record is re-attached to a slot (T2091, INFERRED).
pub const RECORD_CHANGED: u32 = 0x38;

pub const fn slot_address(index: usize) -> u32 {
    SLOT_TABLE + index as u32 * SLOT_STRIDE
}

/// Borrowed head cell. The release callback can receive a slot outside the
/// owned table, so this view keeps the supplied address and reads on demand.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotHeadCell(pub u32);

impl SlotHeadCell {
    pub fn read<M: GuestMemory>(self, m: &M) -> u32 {
        m.read32(self.0)
    }
}

/// Owner of the 16 slot heads. The remaining bytes of each 0x1F54 slot (profile stats, 62 reads in
/// the xref) have no recovered layout and stay guest-owned; `SaveSlots` never writes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SaveSlots {
    pub heads: [u32; SLOT_COUNT],
}

impl SaveSlots {
    pub const EMPTY: Self = Self {
        heads: [0; SLOT_COUNT],
    };

    pub fn load<M: GuestMemory>(m: &M) -> Self {
        Self {
            heads: core::array::from_fn(|i| m.read32(slot_address(i))),
        }
    }

    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        for (i, head) in self.heads.iter().enumerate() {
            m.write32(slot_address(i), *head);
        }
    }

    /// First slot whose guest head differs from the owned one.
    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<usize> {
        (0..SLOT_COUNT).find(|&i| m.read32(slot_address(i)) != self.heads[i])
    }

    pub fn is_active(&self, index: usize) -> bool {
        self.heads[index] != 0
    }

    /// Head write only (T2091): the single `[slot] = record` store of 0x69D90 and 0x699D0, write-through.
    /// The record's own back link is a separate store of the caller.
    pub fn set_head<M: GuestMemory>(&mut self, m: &mut M, index: usize, record: u32) {
        self.heads[index] = record;
        m.write32(slot_address(index), record);
    }

    /// Write 1 of 2 (0x69D90, else branch): link allocation record `record` to slot `index`. The old
    /// owner link of the record is cleared first, as the original does.
    pub fn attach<M: GuestMemory>(&mut self, m: &mut M, index: usize, record: u32) {
        let slot = slot_address(index);
        let back = m.read32(record.wrapping_add(RECORD_BACK_LINK));
        if back != 0 {
            m.write32(back, 0);
            // The old slot cell is cleared by the write above; keep the owned copy in step.
            if let Some(old) = (0..SLOT_COUNT).find(|&i| slot_address(i) == back) {
                self.heads[old] = 0;
            }
        }
        self.heads[index] = record;
        m.write32(slot, record);
        m.write32(record.wrapping_add(RECORD_BACK_LINK), slot);
    }

    /// Write 2 of 2 (0x699D0): release slot `index`. An in-use record is zeroed (0x10 dwords).
    pub fn release<M: GuestMemory>(&mut self, m: &mut M, index: usize) {
        let slot = slot_address(index);
        let record = self.heads[index];
        if record != 0 {
            m.write32(record.wrapping_add(RECORD_BACK_LINK), 0);
            if m.read32(record.wrapping_add(RECORD_IN_USE)) != 0 && m.read32(record) != 0 {
                // The original re-reads the back link after clearing it: always 0, so no cell clear.
                for i in 0..0x10 {
                    m.write32(record.wrapping_add(i * 4), 0);
                }
            }
        }
        self.heads[index] = 0;
        m.write32(slot, 0);
    }
}

/// Borrowed allocation-record handle for the SaveSlots writer boundary (T2091).
/// Reads individual fields on demand: release may receive aliased or partial records.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AllocationRecord(pub GuestPtr<AllocationRecordTag>);

/// Distinguishes allocation-record pointers from other guest pointers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AllocationRecordTag;

impl AllocationRecord {
    pub fn new(address: u32) -> Self {
        Self(GuestPtr::new(address))
    }
    pub fn address(self) -> u32 {
        self.0.0
    }
    pub fn in_use<M: GuestMemory>(self, m: &M) -> u32 {
        m.read32(self.address().wrapping_add(RECORD_IN_USE))
    }
    pub fn first_word<M: GuestMemory>(self, m: &M) -> u32 {
        m.read32(self.address())
    }
    pub fn back_link<M: GuestMemory>(self, m: &M) -> u32 {
        m.read32(self.address().wrapping_add(RECORD_BACK_LINK))
    }
    pub fn set_back_link<M: GuestMemory>(self, m: &mut M, slot: u32) {
        m.write32(self.address().wrapping_add(RECORD_BACK_LINK), slot);
    }
    pub fn kind<M: GuestMemory>(self, m: &M) -> u32 {
        m.read32(self.address().wrapping_add(RECORD_KIND))
    }
    pub fn mark_in_use<M: GuestMemory>(self, m: &mut M) {
        m.write32(self.address().wrapping_add(RECORD_IN_USE), 1);
    }
    pub fn mark_changed<M: GuestMemory>(self, m: &mut M) {
        m.write32(self.address().wrapping_add(RECORD_CHANGED), 1);
    }
    pub fn clear<M: GuestMemory>(self, m: &mut M) {
        for word in 0..16u32 {
            m.write32(self.address().wrapping_add(word * 4), 0);
        }
    }
}
