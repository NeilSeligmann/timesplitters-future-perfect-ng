//! Profile and save file I/O on a writable volume. Layout (T1762, static): the profile file is
//! the `0x1F54` payload followed by one 20-byte XDK signature = `0x1F68` bytes. The signature is
//! retained, never recomputed or authenticated here. Writes go to a temporary name and are
//! renamed over the target so an interrupted write never leaves a truncated profile.
use crate::vfs::{Access, Disposition, NtStatus, Vfs, status};
use tsfp_format_profile::Profile;

pub const PROFILE_FILE_BYTES: usize = 0x1F68;

pub fn load_profile(vfs: &mut Vfs, path: &str) -> Result<Profile, NtStatus> {
    let h = vfs.open(path, Access::READ, Disposition::Open)?;
    let size = vfs.size(h)? as usize;
    let mut b = vec![0u8; size.min(1 << 20)];
    let n = vfs.read_at(h, 0, &mut b);
    vfs.close(h)?;
    b.truncate(n?);
    Profile::parse(&b).map_err(|_| status::UNRECOGNIZED_VOLUME)
}

/// Write `profile` to `path` (full envelope, validated before and after encoding).
pub fn save_profile(vfs: &mut Vfs, path: &str, profile: &Profile) -> Result<(), NtStatus> {
    let bytes = profile.encode().map_err(|_| status::INVALID_PARAMETER)?;
    if bytes.len() != PROFILE_FILE_BYTES { return Err(status::INVALID_PARAMETER); }
    let tmp = format!("{path}.tmp");
    let h = vfs.open(&tmp, Access::READ_WRITE, Disposition::OverwriteIf)?;
    let r = vfs.write(h, &bytes).and_then(|n| if n == bytes.len() { vfs.set_end_of_file(h) } else { Err(status::ACCESS_DENIED) });
    vfs.close(h)?;
    r?;
    vfs.rename(&tmp, path)
}

/// Settings page whose owned working state is accepted by a persistence operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsPage { Controls, AudioVideo }

/// Accept and persist an owned settings edit, publishing the in-memory profile only on success.
/// Uses the existing native save backend; like `save_profile`, this retains the signature and
/// does not authenticate edited data. A signing backend is required for original-game acceptance.
pub fn save_settings(
    vfs: &mut Vfs,
    path: &str,
    profile: &mut Profile,
    edit: tsfp_format_profile::settings::SettingsEdit,
    page: SettingsPage,
) -> Result<(), NtStatus> {
    let mut candidate = profile.clone();
    match page {
        SettingsPage::Controls => edit.accept_controls(&mut candidate),
        SettingsPage::AudioVideo => edit.accept_audio_video(&mut candidate),
    }.map_err(|_| status::INVALID_PARAMETER)?;
    // Original save serializes a zero runtime pointer while retaining it in memory.
    let mut serialized = candidate.clone();
    serialized.payload[..4].fill(0);
    save_profile(vfs, path, &serialized)?;
    *profile = candidate;
    Ok(())
}
