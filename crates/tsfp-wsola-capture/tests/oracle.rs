// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(feature = "c-oracle")]
use std::{ffi::c_void, fs};
unsafe extern "C" {
    fn audio_stretch_create(channels: u16, overlap: usize) -> *mut c_void;
    fn audio_stretch_destroy(e: *mut c_void);
    fn audio_stretch_prime(e: *mut c_void, input: *const i16, frames: usize, out: *mut i16)
    -> bool;
    fn audio_stretch_next(
        e: *mut c_void,
        input: *const i16,
        frames: usize,
        nominal: usize,
        out: *mut i16,
        chosen: *mut usize,
    ) -> bool;
    fn audio_stretch_hold(e: *mut c_void, out: *mut i16) -> bool;
    fn audio_stretch_leave(e: *mut c_void, input: *const i16, frames: usize, out: *mut i16)
    -> bool;
    fn audio_stretch_fade_out(e: *mut c_void, out: *mut i16) -> bool;
}
#[test]
fn c_observer_roundtrip_and_fail_closed() {
    let dir = (0..1000)
        .find_map(|n| {
            let path = std::env::temp_dir().join(format!("tsfp-wsola-{}-{n}", std::process::id()));
            fs::create_dir(&path).ok().map(|_| path)
        })
        .expect("create isolated capture directory");
    let prefix = dir.join("capture");
    // With no opt-in, engine creation and destruction do not create a file.
    unsafe {
        std::env::remove_var("TSFP_WSOLA_CAPTURE");
        let e = audio_stretch_create(2, 4);
        assert!(!e.is_null());
        audio_stretch_destroy(e);
    }
    assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
    // This integration test is the only test in this process using this variable.
    unsafe {
        std::env::set_var("TSFP_WSOLA_CAPTURE", &prefix);
    }
    let mut out = [123i16; 8];
    let input: Vec<i16> = (0..32).map(|n| n * 71 - 900).collect();
    unsafe {
        let e = audio_stretch_create(2, 4);
        assert!(!e.is_null());
        assert!(!audio_stretch_hold(e, out.as_mut_ptr()));
        assert_eq!(out, [123; 8]);
        assert!(audio_stretch_prime(e, input.as_ptr(), 16, out.as_mut_ptr()));
        assert!(audio_stretch_next(
            e,
            input.as_ptr(),
            16,
            3,
            out.as_mut_ptr(),
            std::ptr::null_mut()
        ));
        assert!(audio_stretch_hold(e, out.as_mut_ptr()));
        assert!(audio_stretch_leave(e, input.as_ptr(), 16, out.as_mut_ptr()));
        let before_failed_fade = out;
        assert!(!audio_stretch_fade_out(e, out.as_mut_ptr()));
        assert_eq!(out, before_failed_fade);
        audio_stretch_destroy(e);
        // The C call still executes, but an aliased input/output makes the
        // post-call observation incomplete rather than recording altered input.
        let aliased = audio_stretch_create(2, 4);
        let mut same = [12i16; 16];
        assert!(audio_stretch_prime(
            aliased,
            same.as_ptr(),
            8,
            same.as_mut_ptr()
        ));
        audio_stretch_destroy(aliased);
        let aliased_choice = audio_stretch_create(2, 4);
        assert!(audio_stretch_prime(
            aliased_choice,
            input.as_ptr(),
            16,
            out.as_mut_ptr()
        ));
        let mut aligned = [0usize; 8];
        assert!(audio_stretch_next(
            aliased_choice,
            input.as_ptr(),
            16,
            3,
            aligned.as_mut_ptr().cast(),
            aligned.as_mut_ptr()
        ));
        audio_stretch_destroy(aliased_choice);
        // Exactly one complete prime fits with room for a seal; the next
        // event exceeds the configured limit and must not write any prefix.
        std::env::set_var("TSFP_WSOLA_CAPTURE_MAX_BYTES", "116");
        let capped = audio_stretch_create(2, 4);
        assert!(audio_stretch_prime(
            capped,
            input.as_ptr(),
            8,
            out.as_mut_ptr()
        ));
        assert!(audio_stretch_hold(capped, out.as_mut_ptr()));
        audio_stretch_destroy(capped);
        std::env::set_var("TSFP_WSOLA_CAPTURE_MAX_BYTES", "64garbage");
        let invalid_limit = audio_stretch_create(2, 4);
        assert!(!invalid_limit.is_null());
        audio_stretch_destroy(invalid_limit);
        std::env::remove_var("TSFP_WSOLA_CAPTURE_MAX_BYTES");
        std::env::remove_var("TSFP_WSOLA_CAPTURE");
    }
    let data = fs::read(dir.join("capture.0.trace")).unwrap();
    let report = tsfp_wsola_capture::compare(&data[..]).unwrap();
    assert_eq!((report.calls, report.primes), (6, 1));
    let abi_report = tsfp_wsola_capture::compare_native_abi(&data[..]).unwrap();
    assert_eq!((abi_report.calls, abi_report.primes), (6, 1));
    assert!(tsfp_wsola_capture::compare_native_abi(&data[..data.len() - 1]).is_err());
    assert!(tsfp_wsola_capture::compare(&data[..data.len() - 1]).is_err());
    let prefix = tsfp_wsola_capture::audit_prefix(&data[..data.len() - 1]).unwrap();
    assert_eq!((prefix.calls, prefix.primes), (6, 1));
    let mut corrupt = data.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    assert!(tsfp_wsola_capture::compare(&corrupt[..]).is_err());
    // Corrupt a prime output and re-seal: comparing samples, not just the
    // integrity check, must detect the altered original observation.
    let mut altered_output = data.clone();
    let prime_output = 24 + 27 + 27 + 16 * 2 * 2;
    altered_output[prime_output] ^= 1;
    let seal = altered_output.len() - 17;
    let hash = altered_output[..seal]
        .iter()
        .fold(1469598103934665603u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(1099511628211)
        });
    altered_output[seal + 9..].copy_from_slice(&hash.to_le_bytes());
    assert!(tsfp_wsola_capture::compare(&altered_output[..]).is_err());
    assert!(tsfp_wsola_capture::compare_native_abi(&altered_output[..]).is_err());
    assert!(tsfp_wsola_capture::audit_prefix(&altered_output[..]).is_err());
    // Stop at the first partial event without replaying it.
    let partial_prime = &data[..prime_output + 1];
    let prefix = tsfp_wsola_capture::audit_prefix(partial_prime).unwrap();
    assert_eq!((prefix.calls, prefix.primes), (1, 0));
    assert!(tsfp_wsola_capture::compare(partial_prime).is_err());
    let mut trailing = data.clone();
    trailing.push(0);
    assert!(tsfp_wsola_capture::compare(&trailing[..]).is_err());
    let mut empty = data[..24].to_vec();
    let hash = empty.iter().fold(1469598103934665603u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(1099511628211)
    });
    empty.push(255);
    empty.extend_from_slice(&0u64.to_le_bytes());
    empty.extend_from_slice(&hash.to_le_bytes());
    assert!(tsfp_wsola_capture::compare(&empty[..]).is_err());
    let aliased = fs::read(dir.join("capture.1.trace")).unwrap();
    assert!(tsfp_wsola_capture::compare(&aliased[..]).is_err());
    let aliased_choice = fs::read(dir.join("capture.2.trace")).unwrap();
    assert!(tsfp_wsola_capture::compare(&aliased_choice[..]).is_err());
    let capped = fs::read(dir.join("capture.3.trace")).unwrap();
    assert_eq!(capped.len(), 99);
    assert!(tsfp_wsola_capture::compare(&capped[..]).is_err());
    let prefix = tsfp_wsola_capture::audit_prefix(&capped[..]).unwrap();
    assert_eq!((prefix.calls, prefix.primes), (1, 1));
    assert!(!dir.join("capture.4.trace").exists());
    fs::remove_dir_all(dir).unwrap();
}
