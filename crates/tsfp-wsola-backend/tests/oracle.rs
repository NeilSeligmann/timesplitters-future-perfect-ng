// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(feature = "c-oracle")]
use std::ffi::c_void;
use tsfp_wsola_backend::{
    Engine, ReferenceOps, tsfp_wsola_activity, tsfp_wsola_create, tsfp_wsola_destroy,
    tsfp_wsola_needed, tsfp_wsola_overlap, tsfp_wsola_primed, tsfp_wsola_process,
};
unsafe extern "C" {
    fn fixture_create(c: u16, overlap: usize) -> *mut c_void;
    fn fixture_destroy(e: *mut c_void);
    fn fixture_prime(e: *mut c_void, i: *const i16, n: usize, o: *mut i16, c: usize) -> bool;
    fn fixture_next(
        e: *mut c_void,
        i: *const i16,
        n: usize,
        nominal: usize,
        o: *mut i16,
        c: usize,
        chosen: *mut usize,
    ) -> bool;
    fn fixture_hold(e: *mut c_void, o: *mut i16, c: usize) -> bool;
    fn fixture_leave(e: *mut c_void, i: *const i16, n: usize, o: *mut i16, c: usize) -> bool;
    fn fixture_fade(e: *mut c_void, o: *mut i16, c: usize) -> bool;
    fn fixture_failed_reference() -> *mut c_void;
}
#[test]
fn c_facade_native_and_reference_match_and_pin_selection() {
    // Only this integration test changes the process setting.
    unsafe {
        std::env::remove_var("TSFP_WSOLA_BACKEND");
        let reference = fixture_create(2, 4);
        std::env::set_var("TSFP_WSOLA_BACKEND", "rust");
        let native = fixture_create(2, 4);
        assert!(!reference.is_null() && !native.is_null());
        // Changing the selector cannot change the existing handles.
        std::env::set_var("TSFP_WSOLA_BACKEND", "invalid");
        assert!(fixture_create(2, 4).is_null());
        let input: Vec<i16> = (0..32).map(|n| n * 59 - 1000).collect();
        let (mut c, mut r) = ([9i16; 8], [9i16; 8]);
        assert!(!fixture_hold(reference, c.as_mut_ptr(), 2));
        assert!(!fixture_hold(native, r.as_mut_ptr(), 2));
        assert_eq!((c, r), ([9; 8], [9; 8]));
        assert!(fixture_prime(
            reference,
            input.as_ptr(),
            16,
            c.as_mut_ptr(),
            2
        ));
        assert!(fixture_prime(native, input.as_ptr(), 16, r.as_mut_ptr(), 2));
        assert_eq!(c, r);
        let (mut cp, mut rp) = (0, 0);
        assert!(fixture_next(
            reference,
            input.as_ptr(),
            16,
            2,
            c.as_mut_ptr(),
            2,
            &mut cp
        ));
        assert!(fixture_next(
            native,
            input.as_ptr(),
            16,
            2,
            r.as_mut_ptr(),
            2,
            &mut rp
        ));
        assert_eq!((c, cp), (r, rp));
        assert!(fixture_next(
            reference,
            input.as_ptr(),
            16,
            2,
            c.as_mut_ptr(),
            2,
            std::ptr::null_mut()
        ));
        assert!(fixture_next(
            native,
            input.as_ptr(),
            16,
            2,
            r.as_mut_ptr(),
            2,
            std::ptr::null_mut()
        ));
        assert_eq!(c, r);
        assert!(fixture_hold(reference, c.as_mut_ptr(), 2));
        assert!(fixture_hold(native, r.as_mut_ptr(), 2));
        assert_eq!(c, r);
        assert!(fixture_leave(
            reference,
            input.as_ptr(),
            16,
            c.as_mut_ptr(),
            2
        ));
        assert!(fixture_leave(native, input.as_ptr(), 16, r.as_mut_ptr(), 2));
        assert_eq!(c, r);
        assert!(!fixture_fade(reference, c.as_mut_ptr(), 2));
        assert!(!fixture_fade(native, r.as_mut_ptr(), 2));
        let a = tsfp_wsola_activity(reference.cast::<Engine>());
        let b = tsfp_wsola_activity(native.cast::<Engine>());
        assert_eq!((a.backend, a.calls, a.primes), (1, 5, 1));
        assert_eq!((b.backend, b.calls, b.primes), (2, 5, 1));
        fixture_destroy(reference);
        fixture_destroy(native);
        std::env::remove_var("TSFP_WSOLA_BACKEND");
    }
}
#[test]
fn native_requires_no_reference_and_invalid_calls_preserve_output() {
    unsafe {
        assert!(tsfp_wsola_create(0, 2, 4, std::ptr::null()).is_null());
        assert!(fixture_failed_reference().is_null());
        let incomplete = ReferenceOps {
            create: None,
            destroy: None,
            overlap: None,
            search: None,
            needed: None,
            primed: None,
            prime: None,
            next: None,
            hold: None,
            leave: None,
            fade: None,
        };
        assert!(tsfp_wsola_create(0, 2, 4, &incomplete).is_null());
        assert!(tsfp_wsola_create(1, 0, 4, std::ptr::null()).is_null());
        assert!(tsfp_wsola_create(1, 2, usize::MAX, std::ptr::null()).is_null());
        let e = tsfp_wsola_create(1, 2, 4, std::ptr::null());
        assert!(!e.is_null());
        assert_eq!(tsfp_wsola_overlap(e), 4);
        let mut needed = 123;
        assert!(!tsfp_wsola_needed(e, usize::MAX, &mut needed));
        assert_eq!(needed, 123);
        assert!(!tsfp_wsola_needed(e, 0, e.cast::<usize>()));
        assert_eq!(tsfp_wsola_overlap(e), 4);
        let input = [42i16; 16];
        let mut out = [7i16; 8];
        assert!(!tsfp_wsola_process(
            e,
            0,
            input.as_ptr(),
            usize::MAX,
            0,
            out.as_mut_ptr(),
            8,
            std::ptr::null_mut()
        ));
        assert!(!tsfp_wsola_process(
            e,
            0,
            input.as_ptr(),
            8,
            0,
            out.as_mut_ptr(),
            7,
            std::ptr::null_mut()
        ));
        assert_eq!(out, [7; 8]);
        assert!(!tsfp_wsola_primed(e));
        assert!(tsfp_wsola_process(
            e,
            0,
            input.as_ptr(),
            8,
            0,
            out.as_mut_ptr(),
            8,
            std::ptr::null_mut()
        ));
        assert!(tsfp_wsola_primed(e));
        let mut chosen = 99;
        let before = out;
        assert!(!tsfp_wsola_process(
            e,
            1,
            out.as_ptr(),
            4,
            0,
            out.as_mut_ptr(),
            8,
            &mut chosen
        ));
        let mut aligned = [0usize; 8];
        assert!(!tsfp_wsola_process(
            e,
            1,
            input.as_ptr(),
            8,
            0,
            aligned.as_mut_ptr().cast(),
            8,
            aligned.as_mut_ptr()
        ));
        let invalid_span = (usize::MAX - 1) as *const i16;
        assert!(!tsfp_wsola_process(
            e,
            1,
            invalid_span,
            8,
            0,
            out.as_mut_ptr(),
            8,
            &mut chosen
        ));
        assert_eq!((out, chosen), (before, 99));
        assert!(!tsfp_wsola_process(
            e,
            1,
            input.as_ptr(),
            8,
            usize::MAX,
            out.as_mut_ptr(),
            8,
            &mut chosen
        ));
        assert_eq!((out, chosen), (before, 99));
        assert!(tsfp_wsola_primed(e));
        tsfp_wsola_destroy(e);
    }
}
