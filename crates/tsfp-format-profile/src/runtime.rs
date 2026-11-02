//! Runtime producer of the settings tables (T2196): the `OriginalTables` the model consumes,
//! read from the running original at run time instead of embedded retail data.
//!
//! The default head is the slot's profile settings head as the original initialised or loaded it
//! in guest memory (record R at `0x7BEDC0 + slot * 0x1F54`, docs/t1709-settings-dump.md). Row
//! bindings and choice domains are code facts MEASURED on the hybrid host (T1709 walk, T2196 trace):
//! no label text, asset or disc byte is embedded. `resolve_labels` reads text references from
//! the original page builders. Profile-only snapshots retain `UNRESOLVED_LABEL` until resolved.
use crate::settings::{Binding, HEAD_SIZE, OptionRow, OriginalTables, SettingsPages};
use tsfp_format_core::{Result, word};

/// Guest address of profile record R of slot 0; P = R + 4 holds the settings head.
pub const PROFILE_SLOT0: u32 = 0x7b_edc0;
pub const PROFILE_RECORD_SIZE: u32 = 0x1f54;
/// Page ids follow the Settings list cursor order (MEASURED): Audio/Video row 0, Controls row 1.
pub const PAGE_AUDIO_VIDEO: u32 = 0;
pub const PAGE_CONTROLS: u32 = 1;
/// Marker for a row whose original text id has not been read from the builder code yet.
pub const UNRESOLVED_LABEL: u32 = u32::MAX;

/// Read-only access to the running original's memory (hybrid host, trace dump or Rust-owned state).
pub trait GuestRead {
    fn read(&self, address: u32, out: &mut [u8]) -> Result<()>;
}

/// Profile-relative bindings of the original Controls and Audio/Video pages. Row is the cursor row.
/// Flag bits sit in head byte 2, which is bits 16..23 of the payload word at offset 4.
const CONTROL_ROWS: &[(u32, Binding, &[u32])] = &[
    (1, Binding::ProfileWord { offset: 4 + 0x74 }, &[0, 1, 2]),
    (
        2,
        Binding::ProfileBits {
            offset: 4,
            mask: 0x01_0000,
        },
        &[0, 0x01_0000],
    ),
    (
        3,
        Binding::ProfileBits {
            offset: 4,
            mask: 0x80_0000,
        },
        &[0, 0x80_0000],
    ),
    (
        4,
        Binding::ProfileBits {
            offset: 4,
            mask: 0x08_0000,
        },
        &[0, 0x08_0000],
    ),
    (
        5,
        Binding::ProfileBits {
            offset: 4,
            mask: 0x02_0000,
        },
        &[0, 0x02_0000],
    ),
    (
        6,
        Binding::ProfileBits {
            offset: 4,
            mask: 0x04_0000,
        },
        &[0, 0x04_0000],
    ),
    (
        7,
        Binding::ProfileWord { offset: 4 + 0x68 },
        &[0, 1, 2, 3, 4],
    ),
    // Off, Fire, Hit, Fire and Hit: Off clears the kind bits (MEASURED 0x8F).
    (
        8,
        Binding::ProfileBits {
            offset: 4,
            mask: 0x70_0000,
        },
        &[0, 0x50_0000, 0x30_0000, 0x70_0000],
    ),
];
const SLIDER: &[u32] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
const AUDIO_VIDEO_ROWS: &[(u32, Binding, &[u32])] = &[
    (0, Binding::ProfileWord { offset: 0x198 }, &[0, 1]),
    (1, Binding::ProfileWord { offset: 0x1a0 }, SLIDER),
    (2, Binding::ProfileWord { offset: 0x19c }, &[0, 1]),
    (3, Binding::ProfileWord { offset: 0x1a4 }, SLIDER),
    (4, Binding::ProfileWord { offset: 0x1b0 }, &[0, 1]),
];

/// `OriginalTables` over guest memory. Defaults are the slot's current original values.
pub struct GuestTables<'a, G: GuestRead> {
    pub guest: &'a G,
    pub slot: u32,
}
// Instruction addresses, not original instruction bytes or embedded text references.
// Controls builder 0x2EA3F0 uses widget ids in a different order than cursor order.
const LABEL_SITES: &[(u32, u32, u32)] = &[
    (PAGE_CONTROLS, 1, 0x2ea55a),
    (PAGE_CONTROLS, 2, 0x2ea5f6),
    (PAGE_CONTROLS, 3, 0x2ea5bf),
    (PAGE_CONTROLS, 4, 0x2ea62d),
    (PAGE_CONTROLS, 5, 0x2ea664),
    (PAGE_CONTROLS, 6, 0x2ea6b3),
    (PAGE_CONTROLS, 7, 0x2ea708),
    (PAGE_CONTROLS, 8, 0x2ea7b3),
    (PAGE_AUDIO_VIDEO, 0, 0x2c3f54),
    (PAGE_AUDIO_VIDEO, 1, 0x2c3f9a),
    (PAGE_AUDIO_VIDEO, 2, 0x2c3fc3),
    (PAGE_AUDIO_VIDEO, 3, 0x2c4004),
    (PAGE_AUDIO_VIDEO, 4, 0x2c402a),
];

impl<G: GuestRead> GuestTables<'_, G> {
    /// Load original values and labels together from a full guest memory reader.
    pub fn load_resolved(&mut self) -> Result<SettingsPages> {
        let mut pages = SettingsPages::load(self)?;
        self.resolve_labels(&mut pages)?;
        Ok(pages)
    }

    /// Original labels for scalar rows and the layout/turn-speed presentation rows.
    /// Layout and Turn Speed use their typed APIs rather than integer OptionRows.
    pub fn label(&self, page: u32, row: u32) -> Result<u32> {
        let address = match (page, row) {
            (PAGE_CONTROLS, 0) => 0x2ea492,
            (PAGE_CONTROLS, 9) => 0x2ea822,
            _ => LABEL_SITES
                .iter()
                .find(|s| s.0 == page && s.1 == row)
                .map(|s| s.2)
                .ok_or("settings label row")?,
        };
        let mut instruction = [0; 5];
        self.guest.read(address, &mut instruction)?;
        if instruction[0] != 0x68 {
            return Err("settings label instruction");
        }
        let label = u32::from_le_bytes(instruction[1..].try_into().unwrap());
        if label == UNRESOLVED_LABEL {
            return Err("settings label reference");
        }
        Ok(label)
    }

    /// Resolve original labels from the owner's loaded executable at runtime.
    /// Profile-only trace snapshots cannot supply code: the caller must provide a full
    /// guest reader (or a reader joining the profile and executable regions).
    /// Failure leaves all rows unchanged. Extension rows are retained.
    pub fn resolve_labels(&self, pages: &mut SettingsPages) -> Result<()> {
        let mut resolved = Vec::with_capacity(LABEL_SITES.len());
        for &(page, row, _) in LABEL_SITES {
            let label = self.label(page, row)?;
            let index = pages
                .rows
                .iter()
                .position(|r| r.page == page && r.row == row)
                .ok_or("settings label row")?;
            resolved.push((index, label));
        }
        for (index, label) in resolved {
            pages.rows[index].label_id = label;
        }
        Ok(())
    }

    fn record(&self) -> Result<Vec<u8>> {
        let base = self
            .slot
            .checked_mul(PROFILE_RECORD_SIZE)
            .and_then(|o| o.checked_add(PROFILE_SLOT0))
            .ok_or("profile slot")?;
        let mut record = vec![0; PROFILE_RECORD_SIZE as usize];
        self.guest.read(base, &mut record)?;
        Ok(record)
    }
}
impl<G: GuestRead> OriginalTables for GuestTables<'_, G> {
    fn default_head(&mut self) -> Result<[u8; HEAD_SIZE]> {
        Ok(self.record()?[4..4 + HEAD_SIZE].try_into().unwrap())
    }
    fn rows(&mut self) -> Result<Vec<OptionRow>> {
        let record = self.record()?;
        let pages = [
            (PAGE_CONTROLS, CONTROL_ROWS),
            (PAGE_AUDIO_VIDEO, AUDIO_VIDEO_ROWS),
        ];
        let mut rows = vec![];
        for (page, table) in pages {
            for &(row, binding, choices) in table {
                let default = match binding {
                    Binding::ProfileWord { offset } => word(&record, offset)?,
                    Binding::ProfileBits { offset, mask } => word(&record, offset)? & mask,
                    Binding::Host { .. } => return Err("original host row"),
                };
                if !choices.contains(&default) {
                    return Err("guest value outside the original row domain");
                }
                rows.push(OptionRow {
                    page,
                    row,
                    label_id: UNRESOLVED_LABEL,
                    binding,
                    choices: choices.to_vec(),
                    default,
                });
            }
        }
        Ok(rows)
    }
}

/// A flat memory snapshot (trace dump region) as `GuestRead`.
pub struct Snapshot<'a> {
    pub base: u32,
    pub bytes: &'a [u8],
}
impl GuestRead for Snapshot<'_> {
    fn read(&self, address: u32, out: &mut [u8]) -> Result<()> {
        let start = address.checked_sub(self.base).ok_or("snapshot range")? as usize;
        let src = self
            .bytes
            .get(start..start.checked_add(out.len()).ok_or("snapshot range")?)
            .ok_or("snapshot range")?;
        out.copy_from_slice(src);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::SettingsPages;

    #[test]
    fn labels_are_runtime_read_and_resolution_is_atomic() {
        struct Memory {
            bad: bool,
        }
        impl GuestRead for Memory {
            fn read(&self, address: u32, out: &mut [u8]) -> Result<()> {
                if address == PROFILE_SLOT0 {
                    out.fill(0);
                    return Ok(());
                }
                let index = LABEL_SITES
                    .iter()
                    .position(|s| s.2 == address)
                    .ok_or("test address")?;
                if self.bad && index == LABEL_SITES.len() - 1 {
                    return Err("missing code region");
                }
                out[0] = 0x68;
                out[1..].copy_from_slice(&(100 + index as u32).to_le_bytes());
                Ok(())
            }
        }
        let guest = Memory { bad: false };
        let tables = GuestTables {
            guest: &guest,
            slot: 0,
        };
        let mut pages = SettingsPages::load(&mut GuestTables {
            guest: &guest,
            slot: 0,
        })
        .unwrap();
        tables.resolve_labels(&mut pages).unwrap();
        for (i, &(page, row, _)) in LABEL_SITES.iter().enumerate() {
            assert_eq!(
                pages
                    .rows
                    .iter()
                    .find(|r| r.page == page && r.row == row)
                    .unwrap()
                    .label_id,
                100 + i as u32
            );
        }
        let labels: Vec<_> = pages.rows.iter().map(|r| r.label_id).collect();
        assert!(
            GuestTables {
                guest: &Memory { bad: true },
                slot: 0
            }
            .resolve_labels(&mut pages)
            .is_err()
        );
        assert_eq!(
            labels,
            pages.rows.iter().map(|r| r.label_id).collect::<Vec<_>>()
        );
    }

    #[test]
    fn guest_tables_read_defaults_and_validate_bindings() {
        let mut record = vec![0u8; PROFILE_RECORD_SIZE as usize];
        record[6] = 0xff; // all flags set, vibration Fire and Hit
        record[4 + 0x68] = 4;
        record[4 + 0x74] = 2;
        record[0x198] = 1;
        record[0x1a0] = 11;
        let snapshot = Snapshot {
            base: PROFILE_SLOT0,
            bytes: &record,
        };
        let pages = SettingsPages::load(&mut GuestTables {
            guest: &snapshot,
            slot: 0,
        })
        .unwrap();
        assert_eq!(pages.defaults.head()[..], record[4..0x80]);
        assert_eq!(
            pages.rows.len(),
            CONTROL_ROWS.len() + AUDIO_VIDEO_ROWS.len()
        );
        let default = |page, row| {
            pages
                .rows
                .iter()
                .find(|r| r.page == page && r.row == row)
                .unwrap()
                .default
        };
        assert_eq!(default(PAGE_CONTROLS, 1), 2);
        assert_eq!(default(PAGE_CONTROLS, 8), 0x70_0000);
        assert_eq!(default(PAGE_CONTROLS, 7), 4);
        assert_eq!(default(PAGE_AUDIO_VIDEO, 1), 11);
        // A slot outside the snapshot and an out-of-domain guest value are rejected, not invented.
        assert!(
            SettingsPages::load(&mut GuestTables {
                guest: &snapshot,
                slot: 1
            })
            .is_err()
        );
        record[4 + 0x68] = 9;
        let snapshot = Snapshot {
            base: PROFILE_SLOT0,
            bytes: &record,
        };
        assert!(
            SettingsPages::load(&mut GuestTables {
                guest: &snapshot,
                slot: 0
            })
            .is_err()
        );
    }
}
