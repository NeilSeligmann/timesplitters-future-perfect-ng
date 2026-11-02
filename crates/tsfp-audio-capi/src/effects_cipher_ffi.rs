// SPDX-License-Identifier: GPL-3.0-or-later
//! C ABI for the private effects-image byte transform.

use std::ptr;

use tsfp_audio::effects_cipher::{self, Decoder};

/// Produce the initial eight-byte effects-image key.
///
/// A null output pointer is ignored to keep this boundary fail-safe.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_effects_cipher_setup(key: *mut u8) {
    if key.is_null() {
        return;
    }
    let bytes = effects_cipher::setup_key();
    // SAFETY: the C contract requires an eight-byte writable output span.
    unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), key, bytes.len()) };
}

/// Decode a byte span using the original serial read-then-write overlap behavior.
///
/// The caller must provide an eight-byte readable key and valid source/destination spans for the
/// full duration of the call. The spans may overlap; each source byte is read immediately before
/// its corresponding destination byte is written, matching the C loader's behavior.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_effects_cipher_decode(
    key: *const u8,
    source: *const u8,
    destination: *mut u8,
    bytes: usize,
) -> bool {
    if key.is_null()
        || (bytes != 0 && (source.is_null() || destination.is_null()))
        || bytes > isize::MAX as usize
    {
        return false;
    }

    let mut copied_key = [0; 8];
    // SAFETY: the C contract requires an eight-byte readable key span. Copying first also
    // preserves behavior if the key itself aliases either data span.
    unsafe { ptr::copy_nonoverlapping(key, copied_key.as_mut_ptr(), copied_key.len()) };
    let mut decoder = Decoder::new(copied_key);
    for index in 0..bytes {
        // SAFETY: callers provide valid spans; using raw pointer operations keeps overlapping
        // source and destination legal while preserving the C loop's order.
        let input = unsafe { source.add(index).read() };
        let output = decoder.decode_byte(input);
        // SAFETY: destination span validity is part of the C contract above.
        unsafe { destination.add(index).write(output) };
    }
    true
}
