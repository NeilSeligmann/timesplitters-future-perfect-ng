//! The C ABI of mode 1 (`TSFP_RUST_AUDIO=1`): an address the twin does not model is answered by the C, nothing falls back.
use tsfp_audio_capi::{tsfp_audcmp_counter, tsfp_audcmp_init, tsfp_audcmp_observe, CConfig, COps};

unsafe extern "C" fn read(_: u32, _: *mut u8, _: u32) -> bool {
    false
}
unsafe extern "C" fn readable(_: u32, _: u32) -> bool {
    false
}
unsafe extern "C" fn flag() -> bool {
    false
}
unsafe extern "C" fn snapshot(_: u32, _: *mut u32, _: u32) -> i32 {
    -1
}

#[test]
fn mode1_forwards_the_c_answer_for_unmodelled_addresses() {
    let config = CConfig {
        streams_enabled: 1,
        buffers_enabled: 1,
        completion: 1,
        frequency_hook: 1,
        routing_hook: 1,
        format_hook: 1,
        movie_calls: 0,
        answer: 1,
        ops: COps {
            read,
            readable,
            irql0: flag,
            stream_snapshot: snapshot,
            buffer_snapshot: snapshot,
            codec_ready: flag,
        },
    };
    // SAFETY: the config and its callbacks outlive the test, the observe buffers are null with zero lengths.
    unsafe {
        assert_eq!(tsfp_audcmp_init(&config), 1);
        let r = tsfp_audcmp_observe(
            0x0012_3456,
            std::ptr::null(),
            0,
            0x1234,
            std::ptr::null(),
            0,
            0,
        );
        assert_eq!(r, 0x1234);
    }
    assert_eq!(tsfp_audcmp_counter(2), 0, "nothing was answered by Rust");
    assert_eq!(tsfp_audcmp_counter(3), 0, "no fallback");
}
