#![cfg(feature = "oracle-test")]
use std::ffi::c_void;
use tsfp_audio_stretch_ffi::sink_adapter::process_frames;
use tsfp_audio_stretch_ffi::{
    tsfp_stretch_create, tsfp_stretch_destroy, tsfp_stretch_overlap, tsfp_stretch_primed,
    tsfp_stretch_process, tsfp_stretch_query, tsfp_stretch_search,
};

unsafe extern "C" {
    fn audio_stretch_create(channels: u16, overlap: usize) -> *mut c_void;
    fn audio_stretch_destroy(e: *mut c_void);
    fn audio_stretch_input_needed(e: *const c_void, nominal: usize) -> usize;
    fn audio_stretch_primed(e: *const c_void) -> bool;
    fn audio_stretch_overlap(e: *const c_void) -> usize;
    fn audio_stretch_search(e: *const c_void) -> usize;
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
fn c_abi_matches_c_stretch_samples_and_state() {
    let mut seed = 2046u32;
    for channels in [1u16, 2, 4] {
        for overlap in [4usize, 5, 8, 17, 32] {
            for case in 0..40 {
                let frames = overlap * 4;
                let input: Vec<i16> = (0..frames * channels as usize)
                    .map(|_| {
                        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                        (seed >> 16) as i16
                    })
                    .collect();
                let mut c = vec![0; overlap * channels as usize];
                let mut rust = c.clone();
                unsafe {
                    let ce = audio_stretch_create(channels, overlap);
                    let re = tsfp_stretch_create(channels, overlap);
                    assert!(!ce.is_null() && !re.is_null());
                    assert_eq!(tsfp_stretch_overlap(re), audio_stretch_overlap(ce));
                    assert_eq!(tsfp_stretch_search(re), audio_stretch_search(ce));
                    let mut needed = 0;
                    assert!(tsfp_stretch_query(re, case, &mut needed));
                    assert_eq!(needed, audio_stretch_input_needed(ce, case));
                    assert!(!tsfp_stretch_query(re, usize::MAX, &mut needed));
                    assert_eq!(needed, audio_stretch_input_needed(ce, case));
                    for op in [1, 2, 0, 1, 2, 3, 4, 0, 4] {
                        let nominal = case % frames;
                        let (mut cs, mut rs) = (usize::MAX, usize::MAX);
                        let expected = match op {
                            0 => audio_stretch_prime(ce, input.as_ptr(), frames, c.as_mut_ptr()),
                            1 => audio_stretch_next(
                                ce,
                                input.as_ptr(),
                                frames,
                                nominal,
                                c.as_mut_ptr(),
                                &mut cs,
                            ),
                            2 => audio_stretch_hold(ce, c.as_mut_ptr()),
                            3 => audio_stretch_leave(ce, input.as_ptr(), frames, c.as_mut_ptr()),
                            _ => audio_stretch_fade_out(ce, c.as_mut_ptr()),
                        };
                        let actual = tsfp_stretch_process(
                            re,
                            op,
                            input.as_ptr(),
                            input.len(),
                            nominal,
                            rust.as_mut_ptr(),
                            rust.len(),
                            &mut rs,
                        );
                        assert_eq!(
                            (actual, rs, &rust),
                            (expected, cs, &c),
                            "channels={channels} overlap={overlap} case={case} op={op}"
                        );
                        assert_eq!(tsfp_stretch_primed(re), audio_stretch_primed(ce));
                    }
                    audio_stretch_destroy(ce);
                    tsfp_stretch_destroy(re);
                }
            }
        }
    }
}

#[test]
fn sink_frame_adapter_matches_c_and_rejects_overflow() {
    unsafe {
        let c = audio_stretch_create(2, 4);
        let r = tsfp_stretch_create(2, 4);
        assert!(!c.is_null() && !r.is_null());
        let input: Vec<i16> = (0..32).map(|v| v * 101 - 1000).collect();
        let (mut expected, mut actual) = ([0i16; 8], [0i16; 8]);
        assert!(!process_frames(
            r,
            0,
            input.as_ptr(),
            usize::MAX,
            0,
            actual.as_mut_ptr(),
            std::ptr::null_mut()
        ));
        assert_eq!(actual, [0; 8]);
        for op in [0, 1, 2, 3, 4] {
            let (mut cs, mut rs) = (usize::MAX, usize::MAX);
            let result = match op {
                0 => audio_stretch_prime(c, input.as_ptr(), 16, expected.as_mut_ptr()),
                1 => audio_stretch_next(c, input.as_ptr(), 16, 2, expected.as_mut_ptr(), &mut cs),
                2 => audio_stretch_hold(c, expected.as_mut_ptr()),
                3 => audio_stretch_leave(c, input.as_ptr(), 16, expected.as_mut_ptr()),
                _ => audio_stretch_fade_out(c, expected.as_mut_ptr()),
            };
            assert_eq!(
                process_frames(r, op, input.as_ptr(), 16, 2, actual.as_mut_ptr(), &mut rs),
                result
            );
            assert_eq!((actual, rs), (expected, cs));
        }
        audio_stretch_destroy(c);
        tsfp_stretch_destroy(r);
    }
}
