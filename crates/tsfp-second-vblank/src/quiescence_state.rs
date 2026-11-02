//! Host-wide quiescence-check configuration and getter accounting.
//!
//! The original host counters are sequentially consistent atomics. Preserve
//! that ordering because checks can arrive from different guest threads.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);
static GETTER_CHECKS: AtomicU32 = AtomicU32::new(0);

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_quiescence_configure(enabled: bool) {
    ENABLED.store(enabled, Ordering::SeqCst);
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_quiescence_enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_quiescence_note_getter() {
    GETTER_CHECKS.fetch_add(1, Ordering::SeqCst);
}

/// Account for a getter and decide whether its reader is permitted.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_quiescence_admit_getter(registered: bool) -> bool {
    if !tsfp_second_vblank_quiescence_enabled() {
        return true;
    }
    tsfp_second_vblank_quiescence_note_getter();
    registered
}

#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_quiescence_getter_checks() -> u32 {
    GETTER_CHECKS.load(Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configuration_does_not_reset_getter_count() {
        let before = tsfp_second_vblank_quiescence_getter_checks();
        tsfp_second_vblank_quiescence_configure(true);
        assert!(tsfp_second_vblank_quiescence_enabled());
        tsfp_second_vblank_quiescence_note_getter();
        tsfp_second_vblank_quiescence_configure(false);
        assert!(!tsfp_second_vblank_quiescence_enabled());
        assert_eq!(
            tsfp_second_vblank_quiescence_getter_checks(),
            before.wrapping_add(1)
        );
        let before = tsfp_second_vblank_quiescence_getter_checks();
        assert!(tsfp_second_vblank_quiescence_admit_getter(false));
        tsfp_second_vblank_quiescence_configure(true);
        assert!(!tsfp_second_vblank_quiescence_admit_getter(false));
        assert!(tsfp_second_vblank_quiescence_admit_getter(true));
        assert_eq!(
            tsfp_second_vblank_quiescence_getter_checks(),
            before.wrapping_add(2)
        );
        tsfp_second_vblank_quiescence_configure(false);
        extern "C" fn registered(handle: u32, address: u32) -> bool {
            handle == 7 && address == 9
        }
        let before = tsfp_second_vblank_quiescence_getter_checks();
        assert!(tsfp_second_vblank_quiescence_check_getter(0, 0, registered));
        assert_eq!(tsfp_second_vblank_quiescence_getter_checks(), before);
        tsfp_second_vblank_quiescence_configure(true);
        assert!(tsfp_second_vblank_quiescence_check_getter(7, 9, registered));
        assert!(!tsfp_second_vblank_quiescence_check_getter(
            7, 0, registered
        ));
        assert_eq!(
            tsfp_second_vblank_quiescence_getter_checks(),
            before.wrapping_add(2)
        );
        tsfp_second_vblank_quiescence_configure(false);
    }
}
/// Check a getter only while quiescence checking is enabled. The callback
/// performs the host thread lookup and reader registration check.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_second_vblank_quiescence_check_getter(
    handle: u32,
    return_address: u32,
    registered: extern "C" fn(u32, u32) -> bool,
) -> bool {
    if !tsfp_second_vblank_quiescence_enabled() {
        return true;
    }
    tsfp_second_vblank_quiescence_note_getter();
    registered(handle, return_address)
}
