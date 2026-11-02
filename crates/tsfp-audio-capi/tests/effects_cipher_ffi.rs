use tsfp_audio::effects_cipher::{self, Decoder};
use tsfp_audio_capi::effects_cipher_ffi::{
    dsound_effects_cipher_decode, dsound_effects_cipher_setup,
};

#[test]
fn setup_and_decode_ffi_match_the_rust_contract() {
    let expected_key = effects_cipher::setup_key();
    let mut key = [0; 8];
    // SAFETY: `key` is an eight-byte writable buffer.
    unsafe { dsound_effects_cipher_setup(key.as_mut_ptr()) };
    assert_eq!(key, expected_key);

    let source: Vec<u8> = (0..128)
        .map(|value| (value as u8).wrapping_mul(37))
        .collect();
    let mut expected = vec![0; source.len()];
    assert!(effects_cipher::decode(key, &source, &mut expected));
    let mut actual = vec![0; source.len()];
    // SAFETY: pointers refer to readable/writable spans of the specified length.
    assert!(unsafe {
        dsound_effects_cipher_decode(
            key.as_ptr(),
            source.as_ptr(),
            actual.as_mut_ptr(),
            source.len(),
        )
    });
    assert_eq!(actual, expected);
}

#[test]
fn ffi_preserves_serial_partial_overlap_behavior() {
    let key = [0x91, 0x22, 0x3f, 0x44, 0xa5, 0x66, 0x17, 0x88];
    let mut expected = [0x31; 65];
    let mut decoder = Decoder::new(key);
    for index in 0..64 {
        expected[index + 1] = decoder.decode_byte(expected[index]);
    }

    let mut actual = [0x31; 65];
    // SAFETY: both pointers cover the same valid allocation; writes intentionally overlap future reads.
    assert!(unsafe {
        dsound_effects_cipher_decode(
            key.as_ptr(),
            actual.as_ptr(),
            actual.as_mut_ptr().add(1),
            64,
        )
    });
    assert_eq!(actual, expected);
}

#[test]
fn ffi_rejects_invalid_spans_and_accepts_empty_output() {
    let mut key = [0; 8];
    // SAFETY: an empty output does not dereference the data pointers.
    assert!(unsafe {
        dsound_effects_cipher_decode(key.as_ptr(), std::ptr::null(), std::ptr::null_mut(), 0)
    });
    // SAFETY: validation rejects these arguments before dereferencing them.
    assert!(!unsafe {
        dsound_effects_cipher_decode(std::ptr::null(), std::ptr::null(), std::ptr::null_mut(), 0)
    });
    // SAFETY: validation rejects null spans before dereferencing them.
    assert!(!unsafe {
        dsound_effects_cipher_decode(key.as_ptr(), std::ptr::null(), std::ptr::null_mut(), 1)
    });
    // SAFETY: the oversized length is rejected before pointer arithmetic or dereference.
    assert!(!unsafe {
        dsound_effects_cipher_decode(key.as_ptr(), key.as_ptr(), key.as_mut_ptr(), usize::MAX)
    });
}
