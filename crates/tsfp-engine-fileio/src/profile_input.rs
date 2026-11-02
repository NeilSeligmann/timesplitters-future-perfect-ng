//! T2211 native profile extension. Original 0x1f68 envelope remains intact;
//! a versioned, authenticated trailer carries owned input settings in the same file.
use crate::storage_flow::{SaveAuthentication, SaveKind};
use crate::vfs::{NtStatus, status};
use tsfp_format_profile::Profile;
use tsfp_input::settings::InputSettings;
const BASE: usize = 0x1f68;
const MAGIC: &[u8; 8] = b"TSFPIN01";
const LIMIT: usize = 65536;

pub fn encode(
    profile: &Profile,
    input: &InputSettings,
    auth: &mut impl SaveAuthentication,
) -> Result<Vec<u8>, NtStatus> {
    let mut bytes = profile.encode().map_err(|_| status::INVALID_PARAMETER)?;
    let text = input.to_text();
    // Round-trip validation also rejects malformed public maps before disk mutation.
    InputSettings::parse(&text).map_err(|_| status::INVALID_PARAMETER)?;
    if text.len() > LIMIT {
        return Err(status::INVALID_PARAMETER);
    }
    bytes.extend(MAGIC);
    bytes.extend((text.len() as u32).to_le_bytes());
    bytes.extend(text.as_bytes());
    let signature = auth.sign(SaveKind::Profile, &bytes)?;
    bytes.extend(signature);
    Ok(bytes)
}

pub fn decode(
    bytes: &[u8],
    auth: &mut impl SaveAuthentication,
) -> Result<(Profile, InputSettings), NtStatus> {
    if bytes.len() < BASE {
        return Err(status::UNRECOGNIZED_VOLUME);
    }
    let profile = Profile::parse(&bytes[..BASE]).map_err(|_| status::UNRECOGNIZED_VOLUME)?;
    auth.verify(SaveKind::Profile, &profile.payload, &profile.signature)?;
    if bytes.len() == BASE {
        return Ok((profile, InputSettings::default()));
    }
    if bytes.len() < BASE + 12 + 20 || &bytes[BASE..BASE + 8] != MAGIC {
        return Err(status::UNRECOGNIZED_VOLUME);
    }
    let length = u32::from_le_bytes(bytes[BASE + 8..BASE + 12].try_into().unwrap()) as usize;
    if length > LIMIT || bytes.len() != BASE + 12 + length + 20 {
        return Err(status::UNRECOGNIZED_VOLUME);
    }
    let signature = bytes[bytes.len() - 20..].try_into().unwrap();
    auth.verify(SaveKind::Profile, &bytes[..bytes.len() - 20], &signature)?;
    let text = std::str::from_utf8(&bytes[BASE + 12..BASE + 12 + length])
        .map_err(|_| status::UNRECOGNIZED_VOLUME)?;
    let input = InputSettings::parse(text).map_err(|_| status::UNRECOGNIZED_VOLUME)?;
    Ok((profile, input))
}

pub const MAX_ENVELOPE: usize = BASE + 12 + LIMIT + 20;
