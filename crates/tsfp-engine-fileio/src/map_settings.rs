//! Owned settings record for the Map Maker file boundary (T1717/T2319).
//! Widths/offsets are statically measured; option meanings remain INFERRED.
//! IDs are not indexes. Unresolved bytes are retained, never invented/reset.
use crate::vfs::{NtStatus, status};

pub const SETTINGS_BYTES: usize = 0x3dc;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptionId(pub i16);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetupEntry {
    pub option: OptionId,
    pub parameter: u8,
    pub secondary: i8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorOptions {
    pub first: OptionId,
    pub second: OptionId,
    pub parameter: u8,
    pub setup_preset: OptionId,
    pub weapon_preset: OptionId,
    pub extra: i8,
    pub flags: u32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapSettings {
    pub mode_availability: u32,
    pub node_count: u32,
    pub live_eligibility: u8,
    pub saved_layer: u8,
    pub map_selection: u16,
    pub mode: u8,
    pub options: EditorOptions,
    pub setups: Vec<SetupEntry>,
    pub weapons: Vec<OptionId>,
    title: [u8; 32],
    description: [u8; 224],
    briefing: [u8; 224],
    retained: [u8; SETTINGS_BYTES],
}
impl MapSettings {
    pub fn decode(bytes: &[u8]) -> Result<Self, NtStatus> {
        let retained: [u8; SETTINGS_BYTES] =
            bytes.try_into().map_err(|_| status::INVALID_PARAMETER)?;
        let setup_count = bytes[0x16] as usize;
        let weapon_count = bytes[0x42] as usize;
        if setup_count > 10 || weapon_count > 6 {
            return Err(status::UNRECOGNIZED_VOLUME);
        }
        let word = |at| u16::from_le_bytes([bytes[at], bytes[at + 1]]);
        let dword = |at| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        Ok(Self {
            mode_availability: dword(0),
            node_count: dword(4),
            live_eligibility: bytes[8],
            saved_layer: bytes[9],
            map_selection: word(0xa),
            mode: bytes[0xc],
            options: EditorOptions {
                first: OptionId(word(0xe) as i16),
                second: OptionId(word(0x10) as i16),
                parameter: bytes[0x12],
                setup_preset: OptionId(word(0x14) as i16),
                weapon_preset: OptionId(word(0x40) as i16),
                extra: bytes[0x242] as i8,
                flags: dword(0x244),
            },
            setups: (0..setup_count)
                .map(|index| {
                    let at = 0x18 + index * 4;
                    SetupEntry {
                        option: OptionId(word(at) as i16),
                        parameter: bytes[at + 2],
                        secondary: bytes[at + 3] as i8,
                    }
                })
                .collect(),
            weapons: (0..weapon_count)
                .map(|index| OptionId(word(0x44 + index * 2) as i16))
                .collect(),
            title: bytes[0x62..0x82].try_into().unwrap(),
            description: bytes[0x82..0x162].try_into().unwrap(),
            briefing: bytes[0x162..0x242].try_into().unwrap(),
            retained,
        })
    }
    pub fn encode(&self) -> Result<Vec<u8>, NtStatus> {
        if self.setups.len() > 10 || self.weapons.len() > 6 {
            return Err(status::INVALID_PARAMETER);
        }
        let mut bytes = self.retained.to_vec();
        bytes[..4].copy_from_slice(&self.mode_availability.to_le_bytes());
        bytes[4..8].copy_from_slice(&self.node_count.to_le_bytes());
        bytes[8] = self.live_eligibility;
        bytes[9] = self.saved_layer;
        bytes[0xa..0xc].copy_from_slice(&self.map_selection.to_le_bytes());
        bytes[0xc] = self.mode;
        for (at, value) in [
            (0xe, self.options.first),
            (0x10, self.options.second),
            (0x14, self.options.setup_preset),
            (0x40, self.options.weapon_preset),
        ] {
            bytes[at..at + 2].copy_from_slice(&value.0.to_le_bytes());
        }
        bytes[0x12] = self.options.parameter;
        bytes[0x242] = self.options.extra as u8;
        bytes[0x244..0x248].copy_from_slice(&self.options.flags.to_le_bytes());
        bytes[0x16] = self.setups.len() as u8;
        for (index, entry) in self.setups.iter().enumerate() {
            let at = 0x18 + index * 4;
            bytes[at..at + 2].copy_from_slice(&entry.option.0.to_le_bytes());
            bytes[at + 2] = entry.parameter;
            bytes[at + 3] = entry.secondary as u8;
        }
        bytes[0x42] = self.weapons.len() as u8;
        for (index, option) in self.weapons.iter().enumerate() {
            let at = 0x44 + index * 2;
            bytes[at..at + 2].copy_from_slice(&option.0.to_le_bytes());
        }
        bytes[0x62..0x82].copy_from_slice(&self.title);
        bytes[0x82..0x162].copy_from_slice(&self.description);
        bytes[0x162..0x242].copy_from_slice(&self.briefing);
        Ok(bytes)
    }
    pub fn title_bytes(&self) -> &[u8] {
        terminated(&self.title)
    }
    pub fn description_bytes(&self) -> &[u8] {
        terminated(&self.description)
    }
    pub fn briefing_bytes(&self) -> &[u8] {
        terminated(&self.briefing)
    }
    /// Original text bytes are retained until edited. Native edits use UTF-8;
    /// the file envelope reserves one of its 32 bytes for the terminator.
    pub fn with_title(&self, title: &str) -> Result<Self, NtStatus> {
        Self::validate_title(title)?;
        let mut changed = self.clone();
        changed.title.fill(0);
        changed.title[..title.len()].copy_from_slice(title.as_bytes());
        changed.retained[0x62..0x82].copy_from_slice(&changed.title);
        Ok(changed)
    }
    /// Apply UTF-8 text to the fixed 224-byte description field. The record
    /// reserves its final byte for NUL; this storage bound does not establish
    /// the original editor widget's input limit.
    pub fn with_description(&self, description: &str) -> Result<Self, NtStatus> {
        if description.len() > 223 || description.chars().any(char::is_control) {
            return Err(status::INVALID_PARAMETER);
        }
        let mut changed = self.clone();
        changed.description.fill(0);
        changed.description[..description.len()].copy_from_slice(description.as_bytes());
        changed.retained[0x82..0x162].copy_from_slice(&changed.description);
        Ok(changed)
    }
    pub fn validate_title(title: &str) -> Result<(), NtStatus> {
        if title.is_empty() || title.len() > 31 || title.chars().any(char::is_control) {
            return Err(status::INVALID_PARAMETER);
        }
        Ok(())
    }
}
fn terminated(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len())]
}

#[cfg(test)]
mod tests {
    use super::*;
    // FABRICATED codec fixture, no original bytes or game text.
    fn record() -> Vec<u8> {
        let mut bytes: Vec<u8> = (0..SETTINGS_BYTES).map(|i| (i * 37) as u8).collect();
        bytes[0x16] = 10;
        bytes[0x42] = 6;
        bytes[0xe..0x10].copy_from_slice(&(-1i16).to_le_bytes());
        bytes[0x1b] = 0x80;
        bytes[0x242] = 0xfe;
        bytes
    }
    #[test]
    fn typed_signed_fields_and_retained_bytes_roundtrip() {
        let bytes = record();
        let settings = MapSettings::decode(&bytes).unwrap();
        assert_eq!(settings.options.first, OptionId(-1));
        assert_eq!(settings.setups[0].secondary, -128);
        assert_eq!(settings.options.extra, -2);
        assert_eq!(settings.setups.len(), 10);
        assert_eq!(settings.weapons.len(), 6);
        assert_eq!(settings.encode().unwrap(), bytes);
        let changed = settings.with_title("Native title").unwrap();
        let encoded = changed.encode().unwrap();
        assert_eq!(changed.title_bytes(), b"Native title");
        assert_eq!(MapSettings::decode(&encoded).unwrap(), changed);
        assert_eq!(&encoded[..0x62], &bytes[..0x62]);
        assert_eq!(&encoded[0x82..], &bytes[0x82..]);
    }
    #[test]
    fn malformed_counts_and_oversized_native_names_fail() {
        let mut bytes = record();
        bytes[0x16] = 11;
        assert!(MapSettings::decode(&bytes).is_err());
        bytes[0x16] = 10;
        bytes[0x42] = 7;
        assert!(MapSettings::decode(&bytes).is_err());
        assert!(MapSettings::decode(&bytes[..40]).is_err());
        let mut settings = MapSettings::decode(&record()).unwrap();
        assert!(settings.with_title(&"x".repeat(32)).is_err());
        settings.weapons.push(OptionId(-1));
        assert!(settings.encode().is_err());
    }
}
