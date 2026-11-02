//! Read-only hybrid runtime import for ordinary local-save authentication.
//! The imported key is owned by Rust; it is never logged or used as game state.
use crate::local_storage::TITLE_ID;
use crate::vfs::{NtStatus, status};

pub trait RuntimeRead {
    fn word(&self, address: u32) -> Option<u32>;
}

/// 0x37E742 selects this pointer for the current certificate title (class 0).
/// A missing/null pointer is pending boot, not permission to invent a key.
/// Caller must run at a frame boundary where the source runtime is quiescent.
pub fn title_key(runtime: &impl RuntimeRead) -> Result<Option<[u8; 16]>, NtStatus> {
    let Some(certificate) = runtime.word(0x10118).filter(|p| *p != 0) else {
        return Ok(None);
    };
    let title_address = certificate
        .checked_add(8)
        .ok_or(status::INVALID_PARAMETER)?;
    let title = runtime
        .word(title_address)
        .ok_or(status::INVALID_PARAMETER)?;
    if title != u32::from_str_radix(TITLE_ID, 16).map_err(|_| status::INVALID_PARAMETER)? {
        return Err(status::INVALID_PARAMETER);
    }
    let Some(pointer) = runtime.word(0x47583c).filter(|p| *p != 0) else {
        return Ok(None);
    };
    let mut key = [0; 16];
    for (index, out) in key.chunks_exact_mut(4).enumerate() {
        let address = pointer
            .checked_add(index as u32 * 4)
            .ok_or(status::INVALID_PARAMETER)?;
        let word = runtime.word(address).ok_or(status::INVALID_PARAMETER)?;
        out.copy_from_slice(&word.to_le_bytes());
    }
    // A source that changed during import is not a valid activation receipt.
    if runtime.word(0x47583c) != Some(pointer) || runtime.word(0x10118) != Some(certificate) {
        return Err(status::INVALID_PARAMETER);
    }
    Ok(Some(key))
}

/// Import a caller-selected profile slot as retained constructor input only.
/// A slot is not a participant id: the native caller must select it explicitly.
/// No pointer, signature or progress is activated by this read-only snapshot.
fn retained_slot_bytes(runtime: &impl RuntimeRead, slot: usize) -> Result<Vec<u8>, NtStatus> {
    if slot >= 16 {
        return Err(status::INVALID_PARAMETER);
    }
    let base = 0x7bedc0 + slot as u32 * 0x1f54;
    let mut payload = Vec::with_capacity(0x1f54);
    for offset in (0..0x1f54).step_by(4) {
        payload.extend_from_slice(
            &runtime
                .word(base + offset)
                .ok_or(status::INVALID_PARAMETER)?
                .to_le_bytes(),
        );
    }
    for (index, bytes) in payload.chunks_exact(4).enumerate() {
        if runtime.word(base + index as u32 * 4)
            != Some(u32::from_le_bytes(bytes.try_into().unwrap()))
        {
            return Err(status::INVALID_PARAMETER);
        }
    }
    Ok(payload)
}

/// A retained slot that already has the saved-file version. Unlike a create
/// candidate, an invalid version is rejected and never silently repaired.
pub fn retained_profile(
    runtime: &impl RuntimeRead,
    slot: usize,
) -> Result<tsfp_format_profile::Profile, NtStatus> {
    let profile = tsfp_format_profile::Profile {
        payload: retained_slot_bytes(runtime, slot)?,
        signature: [0; 20],
    };
    profile.encode().map_err(|_| status::INVALID_PARAMETER)?;
    Ok(profile)
}

/// Constructor input is not yet a saved envelope. Original 0x192580 stamps
/// version 2 at 0x1925B3 after 0x69D90; its constructors do not touch that word.
/// Prepare the same independent file-schema word in an owned copy, without
/// altering retained counters/default bytes or the source runtime slot. Native
/// settings/progress constructors still run before CreateProfile can commit.
pub fn new_profile_candidate(
    runtime: &impl RuntimeRead,
    slot: usize,
) -> Result<tsfp_format_profile::Profile, NtStatus> {
    let mut payload = retained_slot_bytes(runtime, slot)?;
    payload[0x1ee0..0x1ee4].copy_from_slice(&2u32.to_le_bytes());
    let profile = tsfp_format_profile::Profile {
        payload,
        signature: [0; 20],
    };
    profile.encode().map_err(|_| status::INVALID_PARAMETER)?;
    Ok(profile)
}

/// Native save identity is independent of display text and guest pointers.
/// Directory creation remains exclusive; a collision fails without overwrite.
pub fn profile_identity() -> Result<String, NtStatus> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| status::INVALID_PARAMETER)?
        .as_nanos();
    let serial = SERIAL
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .map_err(|_| status::INVALID_PARAMETER)?;
    Ok(format!(
        "profile-{time:x}-{:x}-{serial:x}",
        std::process::id()
    ))
}
