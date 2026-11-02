//! Exercise the live atomic host contract in its own process.
#![allow(unsafe_code)] // The test calls the exported C ABI; arguments are plain values.
#[repr(C)]
struct FrameResult {
    action: u32,
    producer_handle: u32,
    producer_fs: u32,
}

unsafe extern "C" {
    fn tsfp_second_vblank_host_clear_epoch(configure: bool, enabled: bool);
    fn tsfp_vblank_bound_set(value: bool);
    fn tsfp_vblank_registered_set(value: bool);
    fn tsfp_vblank_owner_handle_set(value: u32);
    fn tsfp_vblank_owner_fs_set(value: u32);
    fn tsfp_vblank_inflight_get() -> bool;
    fn tsfp_vblank_inflight_set(value: bool);
    fn tsfp_vblank_first_delivered_get() -> bool;
    fn tsfp_vblank_second_delivered_get() -> bool;
    fn tsfp_vblank_refused_set(value: bool);
    fn tsfp_vblank_credits_get() -> u32;
    fn tsfp_vblank_credits_set(value: u32);
    fn tsfp_second_vblank_host_complete_original(first: bool) -> bool;
    fn tsfp_second_vblank_host_await_credit(owner: bool) -> bool;
    fn tsfp_vblank_startup_clock_set(value: bool);
    fn tsfp_vblank_interactive_set(value: bool);
    fn tsfp_vblank_first_delivered_set(value: bool);
    fn tsfp_vblank_second_delivered_set(value: bool);
    fn tsfp_vblank_owner_budget_set(value: u32);
    fn tsfp_vblank_owner_delivered_set(value: u32);
    fn tsfp_vblank_worker_delivered_total_set(value: u32);
    fn tsfp_second_vblank_host_expected_counter(startup: bool) -> u32;
    fn tsfp_vblank_owner_location_get() -> u32;
    fn tsfp_vblank_owner_location_set(value: u32);
    fn tsfp_second_vblank_host_owner_call_begin(callee: u32) -> u32;
    fn tsfp_second_vblank_host_owner_call_finish(
        location: u32,
        callee: u32,
        budgeted: bool,
        active: bool,
    ) -> u32;
    fn tsfp_second_vblank_host_frame_needs_counters(handle: u32, fs: u32, owner: bool) -> bool;
    fn tsfp_second_vblank_host_frame(
        handle: u32,
        fs: u32,
        owner: bool,
        producer: bool,
        valid: bool,
        counter: u32,
        last: u32,
    ) -> FrameResult;
}

#[test]
fn frame_uses_and_reserves_live_host_state() {
    // Ensure the rlib is linked; the host contract is exported by its C ABI.
    assert!(tsfp_second_vblank::tsfp_second_vblank_gate_closed(
        0,
        1.0f32.to_bits()
    ));
    unsafe {
        tsfp_second_vblank_host_clear_epoch(true, true);
        tsfp_vblank_owner_location_set(1);
        let location = tsfp_second_vblank_host_owner_call_begin(0x3800bf);
        assert_eq!(location, 1);
        assert_eq!(tsfp_vblank_owner_location_get(), 1);
        assert_eq!(
            tsfp_second_vblank_host_owner_call_finish(location, 0x3800bf, true, true),
            1
        );
        assert_eq!(tsfp_vblank_owner_location_get(), 2);
        let location = tsfp_second_vblank_host_owner_call_begin(0x22030);
        assert_eq!(location, 2);
        assert_eq!(tsfp_vblank_owner_location_get(), 0);
        assert_eq!(
            tsfp_second_vblank_host_owner_call_finish(location, 0x22030, true, true),
            3
        );
        assert_eq!(tsfp_second_vblank_host_expected_counter(false), 2);
        tsfp_vblank_owner_delivered_set(u32::MAX);
        tsfp_vblank_worker_delivered_total_set(2);
        assert_eq!(tsfp_second_vblank_host_expected_counter(false), 3);
        assert_eq!(tsfp_second_vblank_host_expected_counter(true), 1);
        tsfp_vblank_owner_delivered_set(0);
        tsfp_vblank_worker_delivered_total_set(0);
        tsfp_vblank_bound_set(true);
        tsfp_vblank_registered_set(true);
        tsfp_vblank_owner_handle_set(7);
        tsfp_vblank_owner_fs_set(4096);
        assert!(!tsfp_second_vblank_host_frame_needs_counters(7, 4096, true));
        tsfp_vblank_first_delivered_set(true);
        tsfp_vblank_second_delivered_set(true);
        tsfp_vblank_owner_budget_set(1);
        assert!(tsfp_second_vblank_host_frame_needs_counters(7, 4096, true));
        assert!(!tsfp_second_vblank_host_frame_needs_counters(8, 4096, true));
        assert!(!tsfp_second_vblank_host_frame_needs_counters(
            7, 4096, false
        ));
        tsfp_vblank_first_delivered_set(false);
        tsfp_vblank_second_delivered_set(false);
        let result = tsfp_second_vblank_host_frame(7, 4096, true, false, false, 0, 0);
        assert_eq!(result.action, 1);
        assert!(tsfp_vblank_inflight_get());
        assert_eq!(
            tsfp_second_vblank_host_frame(7, 4096, true, false, false, 0, 0).action,
            5
        );
        tsfp_vblank_credits_set(1);
        assert!(tsfp_second_vblank_host_complete_original(true));
        assert!(tsfp_vblank_first_delivered_get());
        assert_eq!(tsfp_vblank_credits_get(), 0);
        assert!(!tsfp_vblank_inflight_get());
        assert!(tsfp_second_vblank_host_await_credit(true));
        assert!(!tsfp_second_vblank_host_await_credit(false));
        tsfp_vblank_credits_set(1);
        assert!(!tsfp_second_vblank_host_await_credit(true));
        tsfp_vblank_credits_set(0);
        tsfp_vblank_interactive_set(true);
        tsfp_vblank_startup_clock_set(true);
        assert!(!tsfp_second_vblank_host_await_credit(true));
        tsfp_vblank_startup_clock_set(false);
        tsfp_vblank_inflight_set(true);
        tsfp_vblank_refused_set(true);
        assert!(!tsfp_second_vblank_host_complete_original(false));
        assert!(!tsfp_vblank_second_delivered_get());
        assert!(tsfp_vblank_inflight_get());
        tsfp_vblank_refused_set(false);
        assert!(tsfp_second_vblank_host_complete_original(false));
        assert!(tsfp_vblank_second_delivered_get());
        assert!(!tsfp_vblank_inflight_get());
    }
}
