//! Profile envelope only: the retained 20-byte XDK signature is not authenticated or recomputed.
pub mod runtime;
pub mod settings;

use tsfp_format_core::*;

use settings::Settings;

pub const PLAYER_RECORD_SIZE: usize = 0x1f54;
const SETTINGS_OFFSET: usize = 0x04;
const RETAINED_OFFSET: usize = 0x80;

/// Rust-owned view of one active profile record R. Only the runtime word and
/// the settings head have typed meaning here; all remaining record bytes are
/// retained verbatim until their contracts are established.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerRecord {
    runtime_pointer: u32,
    settings: Settings,
    retained: [u8; PLAYER_RECORD_SIZE - RETAINED_OFFSET],
}

impl PlayerRecord {
    /// Parse the fixed-size in-memory record R. The disk signature is outside R.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != PLAYER_RECORD_SIZE {
            return Err("player record length");
        }
        if word(bytes, 0x1ee0)? != 2 {
            return Err("profile version");
        }
        Ok(Self {
            runtime_pointer: word(bytes, 0)?,
            settings: Settings::from_head(&bytes[SETTINGS_OFFSET..RETAINED_OFFSET])?,
            retained: bytes[RETAINED_OFFSET..].try_into().unwrap(),
        })
    }

    pub fn runtime_pointer(&self) -> u32 {
        self.runtime_pointer
    }

    /// This is a runtime identity/presence word, not disk-owned profile data.
    pub fn set_runtime_pointer(&mut self, pointer: u32) {
        self.runtime_pointer = pointer;
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut Settings {
        &mut self.settings
    }

    /// Bytes from R+0x80 onward, including progress and unresolved profile fields.
    pub fn retained_bytes(&self) -> &[u8] {
        &self.retained
    }

    /// Reconstruct the exact 0x1F54-byte record, retaining every opaque byte.
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(PLAYER_RECORD_SIZE);
        bytes.extend(self.runtime_pointer.to_le_bytes());
        bytes.extend_from_slice(self.settings.head());
        bytes.extend_from_slice(&self.retained);
        bytes
    }
}

/// An inspection codec for the version-2, fixed-length profile envelope.
///
/// `encode` retains the signature unchanged. Editing `payload` therefore does not
/// create an authenticated save file.
///
/// ```
/// use tsfp_format_profile::Profile;
/// let mut bytes = vec![0; 0x1f68];
/// bytes[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
/// let profile = Profile::parse(&bytes).unwrap();
/// assert_eq!(profile.version().unwrap(), 2);
/// assert_eq!(profile.encode().unwrap(), bytes);
/// ```
#[derive(Debug, Clone)]
pub struct Profile {
    pub payload: Vec<u8>,
    pub signature: [u8; 20],
}

impl Profile {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 0x1f68 {
            return Err("profile length");
        }
        if word(bytes, 0x1ee0)? != 2 {
            return Err("profile version");
        }
        Ok(Self {
            payload: bytes[..0x1f54].to_vec(),
            signature: bytes[0x1f54..].try_into().unwrap(),
        })
    }

    pub fn version(&self) -> Result<u32> {
        word(&self.payload, 0x1ee0)
    }

    /// Decode the owned record R while keeping its XDK signature envelope separate.
    pub fn player_record(&self) -> Result<PlayerRecord> {
        PlayerRecord::parse(&self.payload)
    }

    /// Replace R from typed state. The envelope signature is retained unchanged;
    /// callers still need the native save backend to authenticate edited data.
    pub fn replace_player_record(&mut self, record: &PlayerRecord) -> Result<()> {
        if self.payload.len() != PLAYER_RECORD_SIZE {
            return Err("profile length");
        }
        let bytes = record.encode();
        if word(&bytes, 0x1ee0)? != 2 {
            return Err("profile version");
        }
        self.payload.copy_from_slice(&bytes);
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        // Reject malformed public payloads before cloning them into an output buffer.
        if self.payload.len() != 0x1f54 {
            return Err("profile length");
        }
        if self.version()? != 2 {
            return Err("profile version");
        }
        let mut bytes = self.payload.clone();
        bytes.extend(self.signature);
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_record_owns_settings_and_bindings_without_losing_opaque_bytes() {
        let mut bytes: Vec<u8> = (0..PLAYER_RECORD_SIZE)
            .map(|index| (index.wrapping_mul(37) ^ (index >> 3)) as u8)
            .collect();
        bytes[0..4].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        bytes[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
        let mut record = PlayerRecord::parse(&bytes).unwrap();
        assert_eq!(record.runtime_pointer(), 0x1234_5678);
        assert_eq!(record.encode(), bytes);

        let old_retained = record.retained_bytes().to_vec();
        let bindings: [u32; 22] = std::array::from_fn(|i| 0x1000 + i as u32 * 0x101);
        record.settings_mut().set_custom_mapping(bindings);
        record.set_runtime_pointer(0x8765_4321);
        let encoded = record.encode();
        assert_eq!(&encoded[..4], &0x8765_4321u32.to_le_bytes());
        assert_eq!(record.settings().mapping(), bindings);
        assert_eq!(&encoded[RETAINED_OFFSET..], old_retained);
        assert_eq!(PlayerRecord::parse(&encoded).unwrap().encode(), encoded);
    }

    #[test]
    fn player_record_rejects_wrong_extent_and_version() {
        let bytes = vec![0; PLAYER_RECORD_SIZE];
        assert_eq!(PlayerRecord::parse(&bytes).unwrap_err(), "profile version");
        assert_eq!(
            PlayerRecord::parse(&bytes[..PLAYER_RECORD_SIZE - 1]).unwrap_err(),
            "player record length"
        );
    }

    #[test]
    fn profile_settings_bridge_keeps_record_and_signature_identity() {
        let mut bytes = vec![0x5a; 0x1f68];
        bytes[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
        let mut profile = Profile::parse(&bytes).unwrap();
        let opaque_before = profile.payload[0x80..].to_vec();
        let signature_before = profile.signature;
        let mut record = profile.player_record().unwrap();
        let bindings: [u32; 22] = std::array::from_fn(|i| i as u32 * 13 + 7);
        record.settings_mut().set_custom_mapping(bindings);
        profile.replace_player_record(&record).unwrap();

        assert_eq!(profile.settings().unwrap().mapping(), bindings);
        assert_eq!(&profile.payload[0x80..], opaque_before);
        assert_eq!(profile.signature, signature_before);
    }

    #[test]
    fn envelope_retains_payload_and_signature_across_patterns() {
        for seed in 0..256u32 {
            let mut bytes = vec![0; 0x1f68];
            for (index, byte) in bytes.iter_mut().enumerate() {
                *byte = (seed as usize ^ index.wrapping_mul(31)) as u8;
            }
            bytes[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
            let mut profile = Profile::parse(&bytes).unwrap();
            assert_eq!(profile.version().unwrap(), 2);
            assert_eq!(profile.encode().unwrap(), bytes);
            profile.payload[seed as usize % 0x1ee0] ^= 0xff;
            bytes[seed as usize % 0x1ee0] ^= 0xff;
            assert_eq!(profile.encode().unwrap(), bytes);
        }
    }

    #[test]
    fn envelope_rejects_wrong_lengths_and_versions() {
        let mut bytes = vec![0; 0x1f68];
        bytes[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
        for length in [0, 0x1ee0, 0x1f53, 0x1f54, 0x1f67, 0x1f69, 0x2000] {
            let mut candidate = bytes.clone();
            candidate.resize(length, 0);
            assert!(Profile::parse(&candidate).is_err(), "length {length}");
        }
        for version in [0, 1, 3, u32::MAX] {
            bytes[0x1ee0..0x1ee4].copy_from_slice(&version.to_le_bytes());
            assert!(Profile::parse(&bytes).is_err(), "version {version}");
        }
    }

    #[test]
    fn encode_validates_mutated_public_payload_before_copying() {
        let mut profile = Profile {
            payload: vec![0; 0x1f54],
            signature: [0x5a; 20],
        };
        assert_eq!(profile.encode().unwrap_err(), "profile version");
        profile.payload.truncate(0x1ee0);
        assert_eq!(profile.encode().unwrap_err(), "profile length");
        profile.payload.resize(0x1f54, 0);
        profile.payload[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
        let encoded = profile.encode().unwrap();
        assert_eq!(&encoded[0x1f54..], &profile.signature);
        assert_eq!(encoded.len(), 0x1f68);
    }
}

pub mod mapping;
