use std::sync::atomic::{AtomicU32, Ordering};
use tsfp_second_vblank::tsfp_second_vblank_check_flip_queue_host;

static STEP: AtomicU32 = AtomicU32::new(0);
static FAIL_AT: AtomicU32 = AtomicU32::new(0);

#[allow(unsafe_code)]
extern "C" fn read(address: u32, value: *mut u32) -> bool {
    let step = STEP.fetch_add(1, Ordering::SeqCst) + 1;
    assert_eq!(address, if step == 1 { 0x3e5d54 } else { 0x3e5d4c });
    if FAIL_AT.load(Ordering::SeqCst) == step {
        return false;
    }
    // The host supplies valid output storage for each read.
    unsafe {
        *value = if step == 1 { 3 } else { 0 };
    }
    true
}

extern "C" fn record(_: *mut core::ffi::c_void, count: u32, producer: u32, threshold: u32) -> i32 {
    assert_eq!(STEP.fetch_add(1, Ordering::SeqCst), 2);
    assert_eq!((count, producer, threshold), (7, 3, 0));
    -1
}

#[test]
fn reads_in_order_and_only_checks_record_after_preflight() {
    for (fail_at, queued, expected, steps) in
        [(1, 3, 0, 1), (2, 3, 0, 2), (0, 2, 1, 2), (0, 3, -1, 3)]
    {
        STEP.store(0, Ordering::SeqCst);
        FAIL_AT.store(fail_at, Ordering::SeqCst);
        assert_eq!(
            tsfp_second_vblank_check_flip_queue_host(
                queued,
                3,
                7,
                core::ptr::null_mut(),
                read,
                record
            ),
            expected
        );
        assert_eq!(STEP.load(Ordering::SeqCst), steps);
    }
}
