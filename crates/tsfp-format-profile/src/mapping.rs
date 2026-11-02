//! Owned layout editor reconstruction (0x2E8840), with original tables supplied at runtime.
//! This module owns selections and working mappings; page collision callbacks remain a UI contract.
use crate::{runtime::GuestRead, settings::Settings};
use tsfp_format_core::Result;

#[derive(Clone, Debug)]
pub struct MappingTables {
    pub digital: [u32; 14],
    pub axes: [[u32; 3]; 8],
    pub pairs: [[u32; 3]; 14],
}
impl MappingTables {
    pub fn read(guest: &impl GuestRead) -> Result<Self> {
        fn words<const N: usize>(g: &impl GuestRead, base: u32) -> Result<[u32; N]> {
            let mut words = [0; N];
            for (i, word) in words.iter_mut().enumerate() {
                let mut bytes = [0; 4];
                g.read(base + i as u32 * 4, &mut bytes)?;
                *word = u32::from_le_bytes(bytes);
            }
            Ok(words)
        }
        let axes = words::<24>(guest, 0x5249a0)?;
        let pairs = words::<42>(guest, 0x5248f8)?;
        Ok(Self {
            digital: words(guest, 0x761bf0)?,
            axes: core::array::from_fn(|i| axes[i * 3..i * 3 + 3].try_into().unwrap()),
            pairs: core::array::from_fn(|i| pairs[i * 3..i * 3 + 3].try_into().unwrap()),
        })
    }
}

#[derive(Clone, Debug)]
pub struct MappingEditor {
    /// Original editor row ids, including axis pair rows 17 and 18.
    pub selections: [u32; 19],
    pub bank: usize,
}
impl MappingEditor {
    /// Reconstruct the original working map. Unmatched selections write nothing.
    /// Repeated actions retain original last-row-wins semantics and pair scan order.
    pub fn reconstruct(&self, tables: &MappingTables, mut mapping: [u32; 22]) -> Result<[u32; 22]> {
        if self.bank > 1 {
            return Err("mapping bank");
        }
        // 0x2E86A0 clears offered digital actions, retaining pair/other slots.
        mapping[..6].fill(0);
        mapping[13] = 0;
        for (i, axis) in tables.axes.iter().enumerate() {
            if axis[0] != u32::MAX {
                mapping[14 + i] = 0;
            }
        }
        for (row, mask) in [
            (5, 0x8000),
            (16, 0x1000),
            (13, 0x2000),
            (3, 0x400),
            (6, 0x4000),
            (4, 0x800),
            (7, 0x10),
            (8, 0x40),
            (9, 0x20),
            (10, 0x80),
        ] {
            if let Some(index) = tables
                .digital
                .iter()
                .position(|v| *v == self.selections[row])
            {
                mapping[index] = mask;
            }
        }
        for (row, selector) in [(11, 4), (12, 3), (14, 2), (15, 1)] {
            if let Some(index) = tables
                .axes
                .iter()
                .position(|v| v[self.bank] == self.selections[row])
            {
                mapping[14 + index] = selector;
            }
        }
        for (row, first, second) in [(17, 4, 8), (18, 1, 2)] {
            let mut matched = false;
            for (index, pair) in tables.pairs.iter().enumerate() {
                if pair[self.bank] == self.selections[row] {
                    mapping[index] = if matched { second } else { first };
                    matched = true;
                }
            }
        }
        Ok(mapping)
    }
    /// Accept custom bindings, selecting layout zero and deriving the gameplay axis quartet.
    pub fn accept_custom(
        &self,
        settings: &mut Settings,
        tables: &MappingTables,
        working: [u32; 22],
    ) -> Result<()> {
        let mut mapping = self.reconstruct(tables, working)?;
        derive_axes(&mut mapping);
        settings.set_custom_mapping(mapping);
        Ok(())
    }
}

/// Original 0x258870: preserve primary selectors and derive the alternate quartet.
pub fn derive_axes(mapping: &mut [u32; 22]) {
    let mut derived: [u32; 4] = mapping[14..18].try_into().unwrap();
    if matches!((mapping[17], mapping[16]), (3, 2) | (1, 4)) {
        derived.swap(0, 2);
    }
    mapping[18..22].copy_from_slice(&derived);
}

/// Preset selection retains saved custom bytes; its generated mapping is supplied by the
/// runtime preset producer. Selecting Custom derives and uses the saved mapping instead.
#[derive(Clone, Debug)]
pub struct LayoutSelection {
    pub settings: Settings,
    pub working: [u32; 22],
}
impl LayoutSelection {
    pub fn choose(&mut self, layout: u16, preset: Option<[u32; 22]>) -> Result<()> {
        let mut working = if layout == 0 {
            self.settings.mapping()
        } else {
            preset.ok_or("runtime preset mapping required")?
        };
        derive_axes(&mut working);
        self.settings.set_layout(layout);
        self.working = working;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preset_custom_selection_and_axis_derivation() {
        let mut settings = Settings::from_head(&[0; 0x7c]).unwrap();
        let mut custom = [9; 22];
        custom[16] = 2;
        custom[17] = 3;
        settings.set_custom_mapping(custom);
        let mut selection = LayoutSelection {
            settings,
            working: custom,
        };
        assert!(selection.choose(2, None).is_err());
        assert_eq!(selection.settings.layout(), 0);
        selection.choose(2, Some([4; 22])).unwrap();
        assert_eq!(selection.settings.mapping(), custom);
        selection.choose(0, None).unwrap();
        assert_eq!(selection.working[18], custom[16]);
        assert_eq!(selection.working[20], custom[14]);
        assert_eq!(selection.settings.mapping(), custom);
    }
}
