//! Checkpoint snapshot of the event instances (spec 5.3, capture `0x234320`, restore `0x237500`).
//! One 16-byte entry per instance: dword 0 packs the instance bytes (+0x14 word, +0x08 word,
//! +0x34, +0x2C from the low byte up), dword 1 the word of the restore function `R+0xBC`, dwords
//! 2 and 3 the extension words `+0xDC` and `+0x108` of a verb 3 object. The capture and restore
//! logic lives in `Engine::capture` and `Engine::restore`.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EventSnapshot {
    pub entries: Vec<[u32; 4]>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotError(pub String);

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bad event snapshot: {}", self.0)
    }
}

impl std::error::Error for SnapshotError {}

impl EventSnapshot {
    /// Serialize as `16 * n` little-endian bytes (the layout stored in the checkpoint block).
    pub fn to_bytes(&self) -> Vec<u8> {
        self.entries
            .iter()
            .flat_map(|words| words.iter().flat_map(|w| w.to_le_bytes()))
            .collect()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SnapshotError> {
        if bytes.len() % 16 != 0 {
            return Err(SnapshotError(format!(
                "length {} is not a multiple of 16",
                bytes.len()
            )));
        }
        let entries = bytes
            .chunks_exact(16)
            .map(|chunk| {
                let mut words = [0u32; 4];
                for (word, raw) in words.iter_mut().zip(chunk.chunks_exact(4)) {
                    *word = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
                }
                words
            })
            .collect();
        Ok(Self { entries })
    }
}
