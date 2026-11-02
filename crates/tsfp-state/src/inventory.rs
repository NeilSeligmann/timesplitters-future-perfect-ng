// SPDX-License-Identifier: GPL-3.0-or-later
//! T2072 (S3-5 of T2051): Rust-owned weapon entries and per-player Inventory. Design and honest
//! limits: docs/t-s3-inventory.md. Stage is Own with write-through: the structs are authoritative
//! for the declared cells, every setter stores the changed bytes into the guest image in the same
//! call, so lifted readers still see the original layout. Field names are INFERRED from
//! docs/t-player-inventory.md (T1628, T1642 owner dumps).
use super::GuestMemory;

/// Weapon entry table (70 entries of 0x3C bytes).
pub const ENTRY_TABLE: u32 = 0x004f_a128;
pub const ENTRY_STRIDE: u32 = 0x3c;
pub const ENTRY_COUNT: usize = 70;
const ENTRY_WORDS: usize = 15;
/// Entry word holding the group number (computed at boot by 0x1B6B40).
pub const GROUP_WORD: usize = 14;
/// Highest group number (T1642: 48 on the retail table), written by 0x1B6B40.
pub const MAX_GROUP_CELL: u32 = 0x0075_a024;

/// Pointer to the player extension blocks (stride 0xEA0, slot 0 is the first local player).
pub const EXT_POINTER: u32 = 0x0073_56d8;
pub const EXT_STRIDE: u32 = 0xea0;
pub const EXT_FLAGS: u32 = 0x28;
pub const EXT_CURRENT: u32 = 0x94;
pub const EXT_PREVIOUS: u32 = 0x98;
pub const EXT_THROWABLE: u32 = 0xa0;
/// One owned byte per entry.
pub const EXT_OWNED: u32 = 0x54c;
/// One "equipped once" byte per entry (T1642 correction of the +0x594 reading).
pub const EXT_EQUIPPED: u32 = 0x592;
/// Reserve ammo words, 2 bytes per ammo type.
pub const EXT_AMMO: u32 = 0x61e;
pub const AMMO_TYPES: usize = 44;
/// Extension block bit set by every pickup (0x1BFCA0), only 0 and 8 were seen.
pub const FLAG_PICKUP: u32 = 0x100;

/// Entry flag bits (docs/t-player-inventory.md).
pub const FLAG_X2: u32 = 0x04;
pub const FLAG_LEFT: u32 = 0x08;
pub const FLAG_HIDDEN_BY_X2: u32 = 0x100;

/// One 0x3C-byte weapon entry. Words 0..4 are named (INFERRED), the rest stay raw.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WeaponEntry {
    pub words: [u32; ENTRY_WORDS],
}

impl WeaponEntry {
    pub fn flags(&self) -> u32 {
        self.words[0]
    }
    pub fn class_word(&self) -> u32 {
        self.words[1]
    }
    pub fn priority(&self) -> u32 {
        self.words[2]
    }
    /// Weapon record index of the left hand, `-1` for none.
    pub fn left_record(&self) -> i32 {
        self.words[3] as i32
    }
    /// Weapon record index (0x4EEF74 table), `-1` for none.
    pub fn record(&self) -> i32 {
        self.words[4] as i32
    }
    pub fn group(&self) -> u32 {
        self.words[GROUP_WORD]
    }
    pub fn is_x2(&self) -> bool {
        self.flags() & FLAG_X2 != 0
    }
}

/// The whole entry table (mirror, owner of the group numbers after boot).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponEntries {
    pub entries: [WeaponEntry; ENTRY_COUNT],
    pub max_group: u32,
}

impl WeaponEntries {
    fn word_address(entry: usize, word: usize) -> u32 {
        ENTRY_TABLE + entry as u32 * ENTRY_STRIDE + 4 * word as u32
    }

    pub fn load<M: GuestMemory>(m: &M) -> Self {
        let mut entries = [WeaponEntry::default(); ENTRY_COUNT];
        for (i, e) in entries.iter_mut().enumerate() {
            for w in 0..ENTRY_WORDS {
                e.words[w] = m.read32(Self::word_address(i, w));
            }
        }
        Self {
            entries,
            max_group: m.read32(MAX_GROUP_CELL),
        }
    }

    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        for (i, e) in self.entries.iter().enumerate() {
            for w in 0..ENTRY_WORDS {
                m.write32(Self::word_address(i, w), e.words[w]);
            }
        }
        m.write32(MAX_GROUP_CELL, self.max_group);
    }

    /// First drifted cell as `(entry, word)`, `(usize::MAX, 0)` for the group count.
    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<(usize, usize)> {
        for (i, e) in self.entries.iter().enumerate() {
            for w in 0..ENTRY_WORDS {
                if m.read32(Self::word_address(i, w)) != e.words[w] {
                    return Some((i, w));
                }
            }
        }
        (m.read32(MAX_GROUP_CELL) != self.max_group).then_some((usize::MAX, 0))
    }

    /// Write-through store of one entry word (the 10 lifted entry writers are all inventory).
    pub fn set_word<M: GuestMemory>(&mut self, m: &mut M, entry: usize, word: usize, v: u32) {
        self.entries[entry].words[word] = v;
        m.write32(Self::word_address(entry, word), v);
    }

    pub fn set_group<M: GuestMemory>(&mut self, m: &mut M, entry: usize, v: u32) {
        self.set_word(m, entry, GROUP_WORD, v);
    }

    pub fn set_max_group<M: GuestMemory>(&mut self, m: &mut M, v: u32) {
        self.max_group = v;
        m.write32(MAX_GROUP_CELL, v);
    }

    /// Port of the 0x1B6B40 numbering (INFERRED from tools.t1628_inventory_entries, which decodes it
    /// from the disassembly): a running counter, incremented once per entry whose flags & 0xC is
    /// clear, becomes the group of every entry. Returns the highest group number written (also stored in the max-group cell).
    pub fn assign_groups<M: GuestMemory>(&mut self, m: &mut M) -> u32 {
        let mut counter: i32 = -1;
        for i in 0..ENTRY_COUNT {
            if self.entries[i].flags() & (FLAG_X2 | FLAG_LEFT) == 0 {
                counter += 1;
            }
            self.set_group(m, i, counter as u32);
        }
        let groups = counter as u32;
        self.set_max_group(m, groups);
        groups
    }
}

/// Per-player inventory (one extension block). `base` is the guest address of the block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inventory {
    pub base: u32,
    pub flags: u32,
    pub current: u32,
    pub previous: u32,
    pub throwable: u32,
    pub owned: [u8; ENTRY_COUNT],
    pub equipped: [u8; ENTRY_COUNT],
    pub ammo: [u16; AMMO_TYPES],
}

impl Inventory {
    /// Guest address of the extension block of `slot`, `None` while the pointer is null.
    pub fn slot_base<M: GuestMemory>(m: &M, slot: u32) -> Option<u32> {
        let p = m.read32(EXT_POINTER);
        (p != 0).then(|| p.wrapping_add(slot.wrapping_mul(EXT_STRIDE)))
    }

    /// Mirror: copies the declared cells out of the block at `base`.
    pub fn load<M: GuestMemory + ?Sized>(m: &M, base: u32) -> Self {
        let mut owned = [0u8; ENTRY_COUNT];
        let mut equipped = [0u8; ENTRY_COUNT];
        for i in 0..ENTRY_COUNT {
            owned[i] = m.read8(base + EXT_OWNED + i as u32);
            equipped[i] = m.read8(base + EXT_EQUIPPED + i as u32);
        }
        let mut ammo = [0u16; AMMO_TYPES];
        for (t, a) in ammo.iter_mut().enumerate() {
            *a = m.read16(base + EXT_AMMO + 2 * t as u32);
        }
        Self {
            base,
            flags: m.read32(base + EXT_FLAGS),
            current: m.read32(base + EXT_CURRENT),
            previous: m.read32(base + EXT_PREVIOUS),
            throwable: m.read32(base + EXT_THROWABLE),
            owned,
            equipped,
            ammo,
        }
    }

    /// Mirror of every local slot whose block exists (`slots` of them).
    pub fn load_slot<M: GuestMemory>(m: &M, slot: u32) -> Option<Self> {
        Self::slot_base(m, slot).map(|b| Self::load(m, b))
    }

    /// Writes the declared cells back; no other guest byte is touched.
    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        let b = self.base;
        m.write32(b + EXT_FLAGS, self.flags);
        m.write32(b + EXT_CURRENT, self.current);
        m.write32(b + EXT_PREVIOUS, self.previous);
        m.write32(b + EXT_THROWABLE, self.throwable);
        for i in 0..ENTRY_COUNT {
            m.write8(b + EXT_OWNED + i as u32, self.owned[i]);
            m.write8(b + EXT_EQUIPPED + i as u32, self.equipped[i]);
        }
        for (t, a) in self.ammo.iter().enumerate() {
            let at = b + EXT_AMMO + 2 * t as u32;
            m.write8(at, *a as u8);
            m.write8(at + 1, (*a >> 8) as u8);
        }
    }

    /// Name of the first cell whose guest value differs from the owned one (drift check).
    pub fn diff<M: GuestMemory>(&self, m: &M) -> Option<&'static str> {
        let g = Self::load(m, self.base);
        if g.flags != self.flags {
            return Some("flags");
        }
        if g.current != self.current {
            return Some("current");
        }
        if g.previous != self.previous {
            return Some("previous");
        }
        if g.throwable != self.throwable {
            return Some("throwable");
        }
        if g.owned != self.owned {
            return Some("owned");
        }
        if g.equipped != self.equipped {
            return Some("equipped");
        }
        if g.ammo != self.ammo {
            return Some("ammo");
        }
        None
    }

    // Writers with write-through.
    pub fn set_owned<M: GuestMemory>(&mut self, m: &mut M, entry: usize, v: u8) {
        self.owned[entry] = v;
        m.write8(self.base + EXT_OWNED + entry as u32, v);
    }
    pub fn set_equipped<M: GuestMemory>(&mut self, m: &mut M, entry: usize, v: u8) {
        self.equipped[entry] = v;
        m.write8(self.base + EXT_EQUIPPED + entry as u32, v);
    }
    pub fn set_ammo<M: GuestMemory>(&mut self, m: &mut M, ammo_type: usize, v: u16) {
        self.ammo[ammo_type] = v;
        let at = self.base + EXT_AMMO + 2 * ammo_type as u32;
        m.write8(at, v as u8);
        m.write8(at + 1, (v >> 8) as u8);
    }
    /// Adds `amount` to the reserve of `ammo_type`, clamped to `max` (the type maximum minus the
    /// loaded rounds, see T1642). Returns the amount actually added. A lowered cap never removes
    /// reserve already held. The guest image is updated in the same call.
    ///
    /// ```
    /// use tsfp_state::inventory::Inventory;
    /// # struct Memory(std::collections::HashMap<u32, u8>);
    /// # impl tsfp_state::GuestMemory for Memory {
    /// #     fn read8(&self, a: u32) -> u8 { *self.0.get(&a).unwrap_or(&0) }
    /// #     fn write8(&mut self, a: u32, v: u8) { self.0.insert(a, v); }
    /// # }
    /// let mut guest = Memory(Default::default());
    /// let mut inventory = Inventory::load(&guest, 0x2000);
    /// assert_eq!(inventory.add_ammo(&mut guest, 0, 20, 12), 12);
    /// assert_eq!(inventory.add_ammo(&mut guest, 0, 5, 8), 0);
    /// assert_eq!(Inventory::load(&guest, 0x2000).ammo[0], 12);
    /// ```
    pub fn add_ammo<M: GuestMemory>(
        &mut self,
        m: &mut M,
        ammo_type: usize,
        amount: u16,
        max: u16,
    ) -> u16 {
        let old = self.ammo[ammo_type];
        let new = (old as u32 + amount as u32).min(max as u32).max(old as u32) as u16;
        self.set_ammo(m, ammo_type, new);
        new - old
    }
    pub fn set_current<M: GuestMemory>(&mut self, m: &mut M, v: u32) {
        self.current = v;
        m.write32(self.base + EXT_CURRENT, v);
    }
    pub fn set_previous<M: GuestMemory>(&mut self, m: &mut M, v: u32) {
        self.previous = v;
        m.write32(self.base + EXT_PREVIOUS, v);
    }
    pub fn set_throwable<M: GuestMemory>(&mut self, m: &mut M, v: u32) {
        self.throwable = v;
        m.write32(self.base + EXT_THROWABLE, v);
    }
    pub fn set_flags<M: GuestMemory>(&mut self, m: &mut M, v: u32) {
        self.flags = v;
        m.write32(self.base + EXT_FLAGS, v);
    }

    // Readers.
    pub fn owns(&self, entry: usize) -> bool {
        self.owned[entry] != 0
    }

    /// Grant rule of 0x1BFAA0 (T1642 CONFIRMED on owner dumps): an x1 entry owns itself plus its
    /// companions (left-hand entries of the same group), an x2 entry owns the whole group. Sets the
    /// pickup flag bit like 0x1BFCA0. Returns the number of owned bytes set.
    pub fn grant<M: GuestMemory>(&mut self, m: &mut M, table: &WeaponEntries, entry: usize) -> u32 {
        let group = table.entries[entry].group();
        let x2 = table.entries[entry].is_x2();
        let mut set = 0;
        for i in 0..ENTRY_COUNT {
            let e = &table.entries[i];
            let member = i == entry || (e.group() == group && (x2 || e.flags() & FLAG_LEFT != 0));
            if member && self.owned[i] == 0 {
                self.set_owned(m, i, 1);
                set += 1;
            }
        }
        let f = self.flags | FLAG_PICKUP;
        self.set_flags(m, f);
        set
    }
}
