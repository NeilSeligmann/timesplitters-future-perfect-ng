//! Exercise startup admission and reservation against live host state in isolation.
#![allow(unsafe_code)]

unsafe extern "C" {
    fn tsfp_second_vblank_host_clear_epoch(configure: bool, enabled: bool);
    fn tsfp_vblank_bound_set(value: bool);
    fn tsfp_vblank_registered_set(value: bool);
    fn tsfp_vblank_first_delivered_set(value: bool);
    fn tsfp_vblank_interactive_set(value: bool);
    fn tsfp_vblank_startup_clock_set(value: bool);
    fn tsfp_vblank_owner_handle_set(value: u32);
    fn tsfp_vblank_owner_fs_set(value: u32);
    fn tsfp_vblank_credits_set(value: u32);
    fn tsfp_vblank_credits_get() -> u32;
    fn tsfp_vblank_inflight_get() -> bool;
    fn tsfp_vblank_second_attempted_get() -> bool;
    fn tsfp_second_vblank_host_startup_candidate(handle: u32, producer_matches: bool) -> bool;
    fn tsfp_second_vblank_host_startup_reserve(
        handle: u32,
        producer_matches: bool,
        had_credit: bool,
        owner_blocked: bool,
    ) -> bool;
    fn tsfp_second_vblank_host_owner_clock_candidate(handle: u32, fs: u32) -> u32;
    fn tsfp_second_vblank_host_owner_clock_reserve(handle: u32, fs: u32, startup: bool) -> bool;
}

#[test]
fn startup_reservation_rechecks_live_state_before_consuming_credit() {
    assert!(tsfp_second_vblank::tsfp_second_vblank_gate_closed(
        0,
        1.0f32.to_bits()
    ));
    unsafe {
        tsfp_second_vblank_host_clear_epoch(true, true);
        tsfp_vblank_bound_set(true);
        tsfp_vblank_registered_set(true);
        tsfp_vblank_first_delivered_set(true);
        tsfp_vblank_interactive_set(true);
        tsfp_vblank_startup_clock_set(true);
        tsfp_vblank_owner_handle_set(7);
        tsfp_vblank_credits_set(1);
        assert!(!tsfp_second_vblank_host_startup_candidate(7, true));
        assert!(!tsfp_second_vblank_host_startup_candidate(8, false));
        assert!(tsfp_second_vblank_host_startup_candidate(8, true));
        assert!(!tsfp_second_vblank_host_startup_reserve(
            8, true, true, false
        ));
        assert_eq!(tsfp_vblank_credits_get(), 1);
        assert!(!tsfp_vblank_inflight_get());
        assert!(!tsfp_vblank_second_attempted_get());
        assert!(tsfp_second_vblank_host_startup_reserve(8, true, true, true));
        assert_eq!(tsfp_vblank_credits_get(), 0);
        assert!(tsfp_vblank_inflight_get());
        assert!(tsfp_vblank_second_attempted_get());
        assert!(!tsfp_second_vblank_host_startup_reserve(
            8, true, true, true
        ));
        tsfp_second_vblank_host_clear_epoch(true, true);
        tsfp_vblank_bound_set(true);
        tsfp_vblank_registered_set(true);
        tsfp_vblank_first_delivered_set(true);
        tsfp_vblank_interactive_set(true);
        tsfp_vblank_startup_clock_set(true);
        tsfp_vblank_owner_handle_set(7);
        tsfp_vblank_owner_fs_set(9);
        assert_eq!(tsfp_second_vblank_host_owner_clock_candidate(7, 9), 1);
        assert!(!tsfp_second_vblank_host_owner_clock_reserve(7, 8, true));
        assert!(!tsfp_second_vblank_host_owner_clock_reserve(7, 9, false));
        assert!(tsfp_second_vblank_host_owner_clock_reserve(7, 9, true));
        assert!(tsfp_vblank_inflight_get());
        assert!(tsfp_vblank_second_attempted_get());
        assert_eq!(tsfp_second_vblank_host_owner_clock_candidate(7, 9), 0);
    }
}
