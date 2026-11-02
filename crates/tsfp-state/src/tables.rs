// SPDX-License-Identifier: GPL-3.0-or-later
//! Immutable XBE data tables as Rust-owned values (T2059, S3-2 of T2051). The bytes come from the
//! original XBE at run time (never committed); the pinned SHA-256 of every table (generated from
//! tools/data/immutable_tables.json) rejects a different build. Mirror stage: `encode_into` writes
//! the tables at their guest addresses for lifted readers; Rust readers use the typed views over
//! `Tables` itself, which implements read-only `GuestMemory`.
use super::GuestMemory;
use super::generated_tables::TABLES;
use super::generated_views::{TsfpModeRowView, TsfpWeaponRecordView};
use super::sha256;
use super::xbe::{XbeError, XbeImage};

#[derive(Clone, Copy, Debug)]
pub struct TableSpec {
    pub name: &'static str,
    pub va: u32,
    pub size: u32,
    pub stride: u32,
    pub count: u32,
    pub sha256: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TableError {
    Xbe(XbeError),
    InvalidSpec,
    /// The table is not inside a file-backed section range (it would be runtime-filled).
    NotFileBacked(&'static str),
    /// The bytes differ from the pinned hash (a different build, or a modified file).
    HashMismatch(&'static str),
}

pub const MAX_TABLES: usize = 16;

pub struct Tables<'a> {
    specs: &'static [TableSpec],
    bodies: [&'a [u8]; MAX_TABLES],
}

impl<'a> Tables<'a> {
    /// Loads the pinned table set from an XBE image, verifying every hash.
    pub fn from_xbe(xbe: &'a [u8]) -> Result<Self, TableError> {
        Self::from_xbe_with(xbe, TABLES, true)
    }

    pub fn from_xbe_with(
        xbe: &'a [u8],
        specs: &'static [TableSpec],
        verify: bool,
    ) -> Result<Self, TableError> {
        if specs.len() > MAX_TABLES
            || specs.iter().any(|s| {
                s.size == 0
                    || s.stride == 0
                    || s.stride.checked_mul(s.count) != Some(s.size)
                    || s.va.checked_add(s.size).is_none()
            })
            || specs.iter().enumerate().any(|(i, a)| {
                specs[..i].iter().any(|b| {
                    a.name == b.name
                        || (u64::from(a.va) < u64::from(b.va) + u64::from(b.size)
                            && u64::from(b.va) < u64::from(a.va) + u64::from(a.size))
                })
            })
        {
            return Err(TableError::InvalidSpec);
        }
        let image = XbeImage::parse(xbe).map_err(TableError::Xbe)?;
        let mut bodies: [&[u8]; MAX_TABLES] = [&[]; MAX_TABLES];
        for (slot, spec) in bodies.iter_mut().zip(specs) {
            let body = image
                .file_bytes(spec.va, spec.size)
                .ok_or(TableError::NotFileBacked(spec.name))?;
            if verify && !sha256::matches(body, spec.sha256) {
                return Err(TableError::HashMismatch(spec.name));
            }
            *slot = body;
        }
        Ok(Self { specs, bodies })
    }

    pub fn specs(&self) -> &'static [TableSpec] {
        self.specs
    }

    pub fn body(&self, name: &str) -> Option<&'a [u8]> {
        self.specs
            .iter()
            .position(|s| s.name == name)
            .map(|i| self.bodies[i])
    }

    fn spec(&self, name: &str) -> Option<&'static TableSpec> {
        self.specs.iter().find(|s| s.name == name)
    }

    /// Weapon record `index` (66 records, `TsfpWeaponRecord`), read through `Tables` as memory.
    pub fn weapon_record(&self, index: u32) -> Option<TsfpWeaponRecordView> {
        let spec = self.spec("weapon_records")?;
        (index < spec.count).then(|| TsfpWeaponRecordView(spec.va + index * spec.stride))
    }

    /// Mode row `index` (87 rows, `TsfpModeRow`).
    pub fn mode_row(&self, index: u32) -> Option<TsfpModeRowView> {
        let spec = self.spec("mode_rows")?;
        (index < spec.count).then(|| TsfpModeRowView(spec.va + index * spec.stride))
    }

    /// Mirror stage: writes every table body at its guest address, so lifted readers of a
    /// Rust-owned image see the same bytes the original loader produced.
    pub fn encode_into<M: GuestMemory>(&self, memory: &mut M) {
        for (spec, body) in self.specs.iter().zip(&self.bodies) {
            for (i, byte) in body.iter().enumerate() {
                memory.write8(spec.va + i as u32, *byte);
            }
        }
    }
}

/// Read-only guest memory over the tables. Reads outside every table and any write are bugs in
/// the caller (a table was never mapped, or a writer exists for an "immutable" table) and panic.
impl GuestMemory for Tables<'_> {
    fn read8(&self, address: u32) -> u8 {
        for (spec, body) in self.specs.iter().zip(&self.bodies) {
            if address >= spec.va && address - spec.va < spec.size {
                return body[(address - spec.va) as usize];
            }
        }
        panic!("read of {address:#x} outside the immutable tables");
    }
    fn write8(&mut self, address: u32, _value: u8) {
        panic!("write to immutable table memory at {address:#x}");
    }
}

/// Own-stage storage: independent of the source XBE buffer and immutable after loading.
/// Unknown fields and padding are retained, including guest pointers (never relocated).
/// Host integration is explicit: no default build loads or mirrors these tables.
pub struct OwnedTables {
    specs: &'static [TableSpec],
    bodies: alloc::vec::Vec<alloc::boxed::Box<[u8]>>,
}

impl OwnedTables {
    pub fn from_xbe(xbe: &[u8]) -> Result<Self, TableError> {
        Self::from_xbe_with(xbe, TABLES, true)
    }

    pub fn from_xbe_with(
        xbe: &[u8],
        specs: &'static [TableSpec],
        verify: bool,
    ) -> Result<Self, TableError> {
        let borrowed = Tables::from_xbe_with(xbe, specs, verify)?;
        Ok(Self {
            specs,
            bodies: borrowed.bodies[..specs.len()]
                .iter()
                .map(|b| alloc::boxed::Box::from(*b))
                .collect(),
        })
    }

    /// A temporary read-only address map; it borrows only Rust-owned table storage.
    pub fn as_tables(&self) -> Tables<'_> {
        let mut bodies = [&[][..]; MAX_TABLES];
        for (slot, body) in bodies.iter_mut().zip(&self.bodies) {
            *slot = body;
        }
        Tables {
            specs: self.specs,
            bodies,
        }
    }

    pub fn body(&self, name: &str) -> Option<&[u8]> {
        self.as_tables().body(name)
    }

    pub fn weapon_record(
        &self,
        index: u32,
    ) -> Option<super::generated_views::TsfpWeaponRecordOwned> {
        let tables = self.as_tables();
        let spec = tables.spec("weapon_records")?;
        if spec.stride != TsfpWeaponRecordView::SIZE {
            return None;
        }
        Some(tables.weapon_record(index)?.load(&tables))
    }

    pub fn mode_row(&self, index: u32) -> Option<super::generated_views::TsfpModeRowOwned> {
        let tables = self.as_tables();
        let spec = tables.spec("mode_rows")?;
        if spec.stride != TsfpModeRowView::SIZE {
            return None;
        }
        Some(tables.mode_row(index)?.load(&tables))
    }

    /// Call once after the reference loader and before any lifted reader; never per frame.
    pub fn mirror_into<M: GuestMemory>(&self, memory: &mut M) {
        self.as_tables().encode_into(memory);
    }
}

impl GuestMemory for OwnedTables {
    fn read8(&self, address: u32) -> u8 {
        for (spec, body) in self.specs.iter().zip(&self.bodies) {
            if address >= spec.va && address - spec.va < spec.size {
                return body[(address - spec.va) as usize];
            }
        }
        panic!("read of {address:#x} outside the immutable tables");
    }
    fn write8(&mut self, address: u32, _value: u8) {
        panic!("write to immutable table memory at {address:#x}");
    }
}
