// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned measured DirectSound surface lookup and per-run call telemetry.
//!
//! The immutable measured rows are supplied by the C-host compatibility table at
//! initialization. The Rust-host call path performs all exact-target lookup and
//! counter updates here; C retains only the state/handler projection needed by the
//! existing facade and report format.

use std::{ffi::c_char, sync::Mutex};

const SECTION_BEGIN: u32 = 0x0040_67C0;
const SECTION_END: u32 = 0x0041_2D14;

pub const LOOKUP_KNOWN: u32 = 0;
pub const LOOKUP_UNKNOWN_INSIDE: u32 = 1;
pub const LOOKUP_UNKNOWN_OUTSIDE: u32 = 2;
pub const LOOKUP_UNINITIALIZED: u32 = 3;

#[derive(Clone, Copy)]
struct MeasuredRow {
    address: u32,
    name: Option<&'static [u8]>,
    sites: u32,
}

/// Whether an address is present in the immutable measured DirectSound surface.
pub fn contains_address(address: u32) -> bool {
    MEASURED_ROWS.iter().any(|row| row.address == address)
}

// The Rust audio host owns the measured lookup rows. The C host keeps its own
// compatibility table behind TSFP_RUST_AUDIO_HOST=0.
const MEASURED_ROWS: &[MeasuredRow] = &[
    MeasuredRow {
        address: 0x0040_7abc,
        name: Some(b"IDirectSoundBuffer_Pause\0"),
        sites: 6,
    },
    MeasuredRow {
        address: 0x0040_7b40,
        name: Some(b"DirectSoundDoWork\0"),
        sites: 5,
    },
    MeasuredRow {
        address: 0x0040_7b23,
        name: Some(b"IDirectSoundStream_Pause\0"),
        sites: 4,
    },
    MeasuredRow {
        address: 0x0040_7b28,
        name: Some(b"IDirectSoundStream_FlushEx\0"),
        sites: 4,
    },
    MeasuredRow {
        address: 0x0040_7aa4,
        name: Some(b"IDirectSoundBuffer_Stop\0"),
        sites: 3,
    },
    MeasuredRow {
        address: 0x0040_7af8,
        name: Some(b"IDirectSoundBuffer_GetStatus\0"),
        sites: 3,
    },
    MeasuredRow {
        address: 0x0040_7b14,
        name: Some(b"IDirectSoundStream_SetVolume\0"),
        sites: 3,
    },
    MeasuredRow {
        address: 0x0040_7b1e,
        name: Some(b"IDirectSoundStream_SetMixBinVolumes\0"),
        sites: 3,
    },
    MeasuredRow {
        address: 0x0040_8556,
        name: Some(b"IDirectSoundBuffer_SetPosition\0"),
        sites: 3,
    },
    MeasuredRow {
        address: 0x0040_85d4,
        name: Some(b"IDirectSoundStream_SetMixBins\0"),
        sites: 3,
    },
    MeasuredRow {
        address: 0x0040_8609,
        name: Some(b"IDirectSoundStream_SetPosition\0"),
        sites: 3,
    },
    MeasuredRow {
        address: 0x0040_8632,
        name: Some(b"IDirectSoundStream_SetRolloffCurve\0"),
        sites: 3,
    },
    MeasuredRow {
        address: 0x0040_9635,
        name: Some(b"DirectSoundCreate\0"),
        sites: 3,
    },
    MeasuredRow {
        address: 0x0040_6a8a,
        name: Some(b"IDirectSound_Release\0"),
        sites: 2,
    },
    MeasuredRow {
        address: 0x0040_7a64,
        name: Some(b"IDirectSoundBuffer_SetVolume\0"),
        sites: 2,
    },
    MeasuredRow {
        address: 0x0040_7ad8,
        name: Some(b"IDirectSoundBuffer_SetLoopRegion\0"),
        sites: 2,
    },
    MeasuredRow {
        address: 0x0040_7b19,
        name: Some(b"IDirectSoundStream_SetHeadroom\0"),
        sites: 2,
    },
    MeasuredRow {
        address: 0x0040_8532,
        name: Some(b"IDirectSoundBuffer_SetMinDistance\0"),
        sites: 2,
    },
    MeasuredRow {
        address: 0x0040_858b,
        name: Some(b"IDirectSoundBuffer_SetRolloffCurve\0"),
        sites: 2,
    },
    MeasuredRow {
        address: 0x0040_85d9,
        name: Some(b"IDirectSoundStream_SetMaxDistance\0"),
        sites: 2,
    },
    MeasuredRow {
        address: 0x0040_93c8,
        name: Some(b"IDirectSound_CreateSoundBuffer\0"),
        sites: 2,
    },
    MeasuredRow {
        address: 0x0040_967c,
        name: Some(b"DirectSoundCreateStream\0"),
        sites: 2,
    },
    MeasuredRow {
        address: 0x0040_6ab6,
        name: Some(b"DirectSoundUseLightHRTF\0"),
        sites: 1,
    },
    MeasuredRow {
        address: 0x0040_79db,
        name: Some(b"IDirectSound_DownloadEffectsImage\0"),
        sites: 1,
    },
    MeasuredRow {
        address: 0x0040_7a02,
        name: Some(b"IDirectSound_SetEffectData\0"),
        sites: 1,
    },
    MeasuredRow {
        address: 0x0040_7a2c,
        name: Some(b"IDirectSound_SetMixBinHeadroom\0"),
        sites: 1,
    },
    MeasuredRow {
        address: 0x0040_7a80,
        name: Some(b"IDirectSoundBuffer_Play\0"),
        sites: 1,
    },
    MeasuredRow {
        address: 0x0040_805b,
        name: None,
        sites: 1,
    },
    MeasuredRow {
        address: 0x0040_84f2,
        name: Some(b"IDirectSoundBuffer_SetFrequency\0"),
        sites: 1,
    },
    MeasuredRow {
        address: 0x0040_850e,
        name: Some(b"IDirectSoundBuffer_SetMaxDistance\0"),
        sites: 1,
    },
    MeasuredRow {
        address: 0x0040_85af,
        name: Some(b"IDirectSoundBuffer_SetI3DL2Source\0"),
        sites: 1,
    },
    MeasuredRow {
        address: 0x0040_85cf,
        name: Some(b"IDirectSoundStream_SetFrequency\0"),
        sites: 1,
    },
    MeasuredRow {
        address: 0x0040_85f1,
        name: Some(b"IDirectSoundStream_SetMinDistance\0"),
        sites: 1,
    },
    MeasuredRow {
        address: 0x0040_8637,
        name: Some(b"IDirectSoundStream_SetI3DL2Source\0"),
        sites: 1,
    },
    MeasuredRow {
        address: 0x0040_8c0d,
        name: Some(b"IDirectSoundBuffer_SetBufferData\0"),
        sites: 1,
    },
    MeasuredRow {
        address: 0x0040_8c2d,
        name: Some(b"IDirectSoundStream_SetFormat\0"),
        sites: 1,
    },
    MeasuredRow {
        address: 0x0040_9109,
        name: Some(b"IDirectSound_CommitDeferredSettings\0"),
        sites: 1,
    },
    MeasuredRow {
        address: 0x0040_93ec,
        name: Some(b"IDirectSound_SetDopplerFactor\0"),
        sites: 1,
    },
    MeasuredRow {
        address: 0x0040_9410,
        name: Some(b"IDirectSound_SetOrientation\0"),
        sites: 1,
    },
    MeasuredRow {
        address: 0x0040_945a,
        name: Some(b"IDirectSound_SetPosition\0"),
        sites: 1,
    },
    MeasuredRow {
        address: 0x0040_af54,
        name: None,
        sites: 1,
    },
];

#[derive(Default)]
struct SurfaceTelemetry {
    rows: Vec<MeasuredRow>,
    calls: Vec<u64>,
    reported: Vec<bool>,
    unknown_calls: u64,
}

static SURFACE: Mutex<Option<SurfaceTelemetry>> = Mutex::new(None);

/// Number of rows in the Rust-owned measured host surface.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_hle_surface_count() -> usize {
    MEASURED_ROWS.len()
}

/// Copy one measured row into the C compatibility projection. Names point at
/// static, NUL-terminated Rust data and remain valid for the process.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_hle_surface_row(
    index: usize,
    address_out: *mut u32,
    name_out: *mut *const c_char,
    sites_out: *mut u32,
) -> bool {
    if address_out.is_null() || name_out.is_null() || sites_out.is_null() {
        return false;
    }
    let Some(row) = MEASURED_ROWS.get(index) else {
        return false;
    };
    // SAFETY: the C facade supplies writable output slots for this synchronous call.
    unsafe {
        address_out.write(row.address);
        name_out.write(
            row.name
                .map_or(std::ptr::null(), |name| name.as_ptr().cast()),
        );
        sites_out.write(row.sites);
    }
    true
}

/// Initialize Rust lookup from its measured rows, rejecting malformed or duplicate
/// entries instead of dispatching against a partial surface.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_hle_surface_init() -> bool {
    for (index, row) in MEASURED_ROWS.iter().enumerate() {
        if !(SECTION_BEGIN..SECTION_END).contains(&row.address)
            || row.sites == 0
            || MEASURED_ROWS[..index]
                .iter()
                .any(|prior| prior.address == row.address)
            || row.name.is_some_and(|name| name.last() != Some(&0))
        {
            return false;
        }
    }
    let rows = MEASURED_ROWS.to_vec();
    let calls = vec![0; rows.len()];
    let reported = vec![false; rows.len()];
    *SURFACE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(SurfaceTelemetry {
        rows,
        calls,
        reported,
        unknown_calls: 0,
    });
    true
}

/// Mark a known measured stub as reported; true only for the first report in this host run.
pub fn mark_reported(index: usize) -> Option<bool> {
    let mut guard = SURFACE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let surface = guard.as_mut()?;
    let reported = surface.reported.get_mut(index)?;
    if *reported {
        Some(false)
    } else {
        *reported = true;
        Some(true)
    }
}

/// Exact-match lookup plus the matching telemetry update. Return values classify
/// unknown targets so the C facade can retain its established diagnostic wording.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_hle_surface_record_call(
    address: u32,
    index_out: *mut u32,
    calls_out: *mut u64,
    sites_out: *mut u32,
) -> u32 {
    if index_out.is_null() || calls_out.is_null() || sites_out.is_null() {
        return LOOKUP_UNINITIALIZED;
    }
    let mut guard = SURFACE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(surface) = guard.as_mut() else {
        return LOOKUP_UNINITIALIZED;
    };
    if let Some(index) = surface.rows.iter().position(|row| row.address == address) {
        surface.calls[index] = surface.calls[index].wrapping_add(1);
        // SAFETY: both outputs were checked non-null and point to caller-owned slots.
        unsafe {
            index_out.write(index as u32);
            calls_out.write(surface.calls[index]);
            sites_out.write(surface.rows[index].sites);
        }
        return LOOKUP_KNOWN;
    }
    surface.unknown_calls = surface.unknown_calls.wrapping_add(1);
    if (SECTION_BEGIN..SECTION_END).contains(&address) {
        LOOKUP_UNKNOWN_INSIDE
    } else {
        LOOKUP_UNKNOWN_OUTSIDE
    }
}

/// Row index of a measured address.
pub fn index_of(address: u32) -> Option<usize> {
    MEASURED_ROWS.iter().position(|row| row.address == address)
}

/// Measured `(name, sites)` of a row, for report labels. Unnamed rows have no name.
pub fn row_label(index: usize) -> Option<(Option<&'static str>, u32)> {
    let row = MEASURED_ROWS.get(index)?;
    let name = row
        .name
        .and_then(|bytes| std::str::from_utf8(&bytes[..bytes.len() - 1]).ok());
    Some((name, row.sites))
}

/// Runtime call count for an exact row, or zero for an invalid row index.
pub fn call_count(index: usize) -> u64 {
    SURFACE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .and_then(|surface| surface.calls.get(index).copied())
        .unwrap_or(0)
}

/// Number of unknown targets observed in this host run.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_hle_surface_unknown_calls() -> u64 {
    SURFACE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
        .map_or(0, |surface| surface.unknown_calls)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_owned_surface_exports_rows_and_classifies_calls() {
        assert_eq!(tsfp_audio_hle_surface_count(), 41);
        assert!(tsfp_audio_hle_surface_init());
        let mut exported = Vec::new();
        for row_index in 0..tsfp_audio_hle_surface_count() {
            let mut address = 0;
            let mut name = std::ptr::null();
            let mut sites = 0;
            // SAFETY: these output pointers refer to live local variables.
            assert!(unsafe {
                tsfp_audio_hle_surface_row(row_index, &mut address, &mut name, &mut sites)
            });
            assert!((SECTION_BEGIN..SECTION_END).contains(&address));
            assert!(sites > 0);
            if !name.is_null() {
                // SAFETY: non-null names are static, NUL-terminated entries above.
                assert!(
                    !unsafe { std::ffi::CStr::from_ptr(name) }
                        .to_bytes()
                        .is_empty()
                );
            }
            exported.push((address, sites));
        }
        assert_eq!(exported[0], (0x0040_7abc, 6));
        assert_eq!(exported[40], (0x0040_af54, 1));
        let mut unique = exported.iter().map(|row| row.0).collect::<Vec<_>>();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), exported.len());

        let mut index = u32::MAX;
        let mut calls = u64::MAX;
        let mut measured_sites = u32::MAX;
        let known = exported[1].0;
        // SAFETY: output pointers refer to the live local variables above.
        assert_eq!(
            unsafe {
                tsfp_audio_hle_surface_record_call(
                    known,
                    &mut index,
                    &mut calls,
                    &mut measured_sites,
                )
            },
            LOOKUP_KNOWN
        );
        assert_eq!((index, calls, measured_sites), (1, 1, exported[1].1));
        assert_eq!(call_count(0), 0);
        // SAFETY: output pointers still refer to the same live local variables.
        assert_eq!(
            unsafe {
                tsfp_audio_hle_surface_record_call(
                    0x0040_6801,
                    &mut index,
                    &mut calls,
                    &mut measured_sites,
                )
            },
            LOOKUP_UNKNOWN_INSIDE
        );
        // SAFETY: output pointers still refer to the same live local variables.
        assert_eq!(
            unsafe {
                tsfp_audio_hle_surface_record_call(
                    0x1234_5678,
                    &mut index,
                    &mut calls,
                    &mut measured_sites,
                )
            },
            LOOKUP_UNKNOWN_OUTSIDE
        );
        assert_eq!(tsfp_audio_hle_surface_unknown_calls(), 2);
        let mut address = 0;
        let mut name = std::ptr::null();
        let mut sites = 0;
        // SAFETY: these output pointers refer to live local variables.
        assert!(!unsafe { tsfp_audio_hle_surface_row(41, &mut address, &mut name, &mut sites) });
    }
}
