// SPDX-License-Identifier: GPL-3.0-or-later
//! T2137: immutable mission/mode row catalogue, separate from mutable Missions cells.
//! Source layout: 87 x 0x80 at 0x515FFC (docs/t1824-spawn-record-layout.md).
//! Owns all bytes, exposes measured typed prefix fields; unknown fields remain preserved.
use super::{
    GuestMemory,
    generated_views::{TsfpModeRowOwned, TsfpModeRowView},
    tables::OwnedTables,
};

pub const MODE_ROWS: u32 = 0x0051_5ffc;
pub const MODE_ROW_COUNT: usize = 87;
pub struct MissionCatalog {
    rows: [[u32; 32]; MODE_ROW_COUNT],
}
impl MissionCatalog {
    /// Snapshot the original table after the reference loader. No pointer dereference/relocation.
    pub fn from_guest<M: GuestMemory>(m: &M) -> Self {
        Self {
            rows: core::array::from_fn(|row| {
                core::array::from_fn(|word| {
                    m.read32(MODE_ROWS + row as u32 * 0x80 + word as u32 * 4)
                })
            }),
        }
    }
    /// Load from the existing hash-checked Rust-owned immutable XBE tables.
    pub fn from_tables(tables: &OwnedTables) -> Option<Self> {
        let body = tables.body("mode_rows")?;
        if body.len() != MODE_ROW_COUNT * 0x80 {
            return None;
        }
        Some(Self {
            rows: core::array::from_fn(|row| {
                core::array::from_fn(|word| {
                    let offset = row * 0x80 + word * 4;
                    u32::from_le_bytes(
                        body[offset..offset + 4]
                            .try_into()
                            .expect("checked row extent"),
                    )
                })
            }),
        })
    }
    pub fn row(&self, index: usize) -> Option<TsfpModeRowOwned> {
        (index < MODE_ROW_COUNT)
            .then(|| TsfpModeRowView(MODE_ROWS + index as u32 * 0x80).load(self))
    }
    /// Original lookup stops at the zero marker. Duplicate ids retain their first row.
    pub fn find_mode(&self, mode: u32) -> Option<TsfpModeRowOwned> {
        for index in 0..MODE_ROW_COUNT {
            let row = self.row(index)?;
            if row.marker == 0 {
                return None;
            }
            if row.mode_id == mode {
                return Some(row);
            }
        }
        None
    }
    pub fn raw_row(&self, index: usize) -> Option<&[u32; 32]> {
        self.rows.get(index)
    }
}
impl GuestMemory for MissionCatalog {
    fn read8(&self, address: u32) -> u8 {
        let offset = address.wrapping_sub(MODE_ROWS) as usize;
        self.rows
            .get(offset / 0x80)
            .map_or(0, |row| (row[offset % 0x80 / 4] >> (offset % 4 * 8)) as u8)
    }
    fn write8(&mut self, _: u32, _: u8) {
        panic!("immutable mission catalogue");
    }
}

/// Current selection is a value passed across the GameState edge, never a second global owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissionSelection {
    pub level: u32,
    pub mode: u8,
}
impl From<&super::game_state::GameState> for MissionSelection {
    fn from(state: &super::game_state::GameState) -> Self {
        Self {
            level: state.level,
            mode: state.mode,
        }
    }
}
