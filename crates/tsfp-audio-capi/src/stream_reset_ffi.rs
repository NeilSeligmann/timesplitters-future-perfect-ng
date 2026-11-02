//! Rust-owned ordering for the Rust audio host's stream reset sweep.
use crate::stream_lease_ffi::CDeviceLease;

unsafe extern "C" {
    fn tsfp_audio_stream_reset_release_host_c(address: u32, identity: *const CDeviceLease) -> u32;
    fn tsfp_audio_stream_reset_validate_host_c(address: u32, identity: *const CDeviceLease)
    -> bool;
    fn tsfp_audio_stream_reset_heap_destroy_host_c(heap: u32) -> bool;
    fn tsfp_audio_stream_reset_log_host_c(code: u32, address: u32, serial: u64);
}

fn destroy_then_retire(
    address: u32,
    identity: &CDeviceLease,
    heap: u32,
    destroy: impl FnOnce(u32) -> bool,
    restore: impl FnOnce(u32, &CDeviceLease),
    retire: impl FnOnce(u32, &CDeviceLease) -> bool,
    report: impl FnOnce(u32, u32, u64),
) -> bool {
    if !destroy(heap) {
        restore(address, identity);
        return false;
    }
    if !retire(address, identity) {
        report(1, address, identity.serial);
        return false;
    }
    true
}

/// Sweep active, detached, and failed-create stream allocations in legacy order.
/// Device lease callbacks and guest heap operations remain synchronous host services.
#[cfg(feature = "rust-audio-host")]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_stream_reset_rust() -> bool {
    let mutex = &crate::stream_mutex_ffi::tsfp_audio_stream_mutex_lock_rust;
    let unlock = &crate::stream_mutex_ffi::tsfp_audio_stream_mutex_unlock_rust;

    loop {
        let mut address = 0;
        mutex();
        // SAFETY: the output is a live stack slot for this synchronous Rust-owned query.
        let found = unsafe {
            crate::stream_lease_ffi::tsfp_audio_stream_lease_first_active_rust(&mut address)
        };
        if !found {
            unlock();
            break;
        }
        let mut identity = CDeviceLease {
            device_heap: 0,
            internal_address: 0,
            serial: 0,
        };
        // SAFETY: the output is live and the manager copies the canonical lease identity.
        let identified = unsafe {
            crate::stream_lease_ffi::tsfp_audio_stream_lease_identity_rust(address, &mut identity)
        };
        unlock();
        if address == 0 || !identified {
            return false;
        }
        // SAFETY: the C adapter keeps its reset callback request alive through lease release.
        if unsafe { tsfp_audio_stream_reset_release_host_c(address, &identity) } != 0 {
            return false;
        }

        mutex();
        // SAFETY: reset serialization prevents competing detach/retire transitions.
        let valid = unsafe {
            tsfp_audio_stream_reset_validate_host_c(address, &identity)
                && crate::stream_lease_ffi::tsfp_audio_stream_lease_begin_retire_detached_rust(
                    address, &identity,
                )
        };
        if !valid {
            unlock();
            return false;
        }
        let mut heap = 0;
        // SAFETY: the output is a live stack slot and this exact address is detached above.
        let has_heap = unsafe {
            crate::stream_lease_ffi::tsfp_audio_stream_lease_heap_rust(address, &mut heap)
        };
        let retired = has_heap
            && destroy_then_retire(
                address,
                &identity,
                heap,
                |heap| {
                    // SAFETY: host destruction is paired with the Rust lease heap record.
                    unsafe { tsfp_audio_stream_reset_heap_destroy_host_c(heap) }
                },
                |address, identity| {
                    // SAFETY: restore the exact identity after refused host heap cleanup.
                    unsafe {
                        crate::stream_lease_ffi::tsfp_audio_stream_lease_restore_detached_rust(
                            address, identity,
                        )
                    };
                },
                |address, identity| {
                    // SAFETY: retire only after the host heap destruction succeeded.
                    unsafe {
                        crate::stream_lease_ffi::tsfp_audio_stream_lease_retire_detached_rust(
                            address, identity,
                        )
                    }
                },
                |code, address, serial| {
                    // SAFETY: this host adapter only emits the diagnostic.
                    unsafe { tsfp_audio_stream_reset_log_host_c(code, address, serial) }
                },
            );
        if !retired {
            unlock();
            return false;
        }
        unlock();
    }

    loop {
        mutex();
        let mut address = 0;
        // SAFETY: the output is a live stack slot for this synchronous manager query.
        let found = unsafe {
            crate::stream_lease_ffi::tsfp_audio_stream_lease_first_detached_rust(&mut address)
        };
        if !found {
            unlock();
            break;
        }
        let mut identity = CDeviceLease {
            device_heap: 0,
            internal_address: 0,
            serial: 0,
        };
        // SAFETY: the output is live and the manager copies the canonical lease identity.
        let identified = unsafe {
            crate::stream_lease_ffi::tsfp_audio_stream_lease_identity_rust(address, &mut identity)
        };
        // SAFETY: the C adapter validates the public header and heap under the held stream lock.
        let valid = address != 0
            && identified
            && unsafe { tsfp_audio_stream_reset_validate_host_c(address, &identity) }
            // SAFETY: this exact detached identity was just validated.
            && unsafe {
                crate::stream_lease_ffi::tsfp_audio_stream_lease_begin_retire_detached_rust(
                    address,
                    &identity,
                )
            };
        if !valid {
            unlock();
            return false;
        }
        let mut heap = 0;
        // SAFETY: the output is a live stack slot and this exact address is retiring.
        let has_heap = unsafe {
            crate::stream_lease_ffi::tsfp_audio_stream_lease_heap_rust(address, &mut heap)
        };
        let retired = has_heap
            && destroy_then_retire(
                address,
                &identity,
                heap,
                |heap| {
                    // SAFETY: host destruction is paired with the Rust lease heap record.
                    unsafe { tsfp_audio_stream_reset_heap_destroy_host_c(heap) }
                },
                |address, identity| {
                    // SAFETY: restore the exact identity after refused host heap cleanup.
                    unsafe {
                        crate::stream_lease_ffi::tsfp_audio_stream_lease_restore_detached_rust(
                            address, identity,
                        )
                    };
                },
                |address, identity| {
                    // SAFETY: retire only after the host heap destruction succeeded.
                    unsafe {
                        crate::stream_lease_ffi::tsfp_audio_stream_lease_retire_detached_rust(
                            address, identity,
                        )
                    }
                },
                |code, address, serial| {
                    // SAFETY: this host adapter only emits the diagnostic.
                    unsafe { tsfp_audio_stream_reset_log_host_c(code, address, serial) }
                },
            );
        if !retired {
            unlock();
            return false;
        }
        unlock();
    }

    mutex();
    // SAFETY: the existing Rust manager owns both rollback stores and their heap adapters.
    let rollback_status = crate::stream_lease_ffi::tsfp_audio_stream_lease_cleanup_rollbacks_rust();
    if rollback_status != 0 {
        if rollback_status == 2 {
            // SAFETY: static service logs this manager invariant failure.
            unsafe { tsfp_audio_stream_reset_log_host_c(2, 0, 0) };
        }
        unlock();
        return false;
    }
    // SAFETY: this call drains Rust-owned emergency rollback nodes with matching heap cleanup.
    if crate::stream_create_hle_ffi::tsfp_audio_stream_create_cleanup_untracked_rust() != 0 {
        unlock();
        return false;
    }
    crate::stream_create_hle_ffi::tsfp_audio_stream_create_reset_announcement_rust();
    unlock();
    true
}

#[cfg(test)]
mod tests {
    use super::destroy_then_retire;
    use crate::stream_lease_ffi::CDeviceLease;
    use std::cell::RefCell;

    fn identity() -> CDeviceLease {
        CDeviceLease {
            device_heap: 3,
            internal_address: 0x4000,
            serial: 9,
        }
    }

    #[test]
    fn reset_heap_refusal_restores_without_retiring() {
        let events = RefCell::new(Vec::new());
        let complete = destroy_then_retire(
            0x8000,
            &identity(),
            0xA000,
            |heap| {
                events.borrow_mut().push(("destroy", heap));
                false
            },
            |address, _| events.borrow_mut().push(("restore", address)),
            |_, _| {
                events.borrow_mut().push(("retire", 0));
                true
            },
            |_, _, _| events.borrow_mut().push(("report", 0)),
        );
        assert!(!complete);
        assert_eq!(
            events.into_inner(),
            [("destroy", 0xA000), ("restore", 0x8000)]
        );
    }

    #[test]
    fn reset_success_destroys_before_retiring() {
        let events = RefCell::new(Vec::new());
        let complete = destroy_then_retire(
            0x8000,
            &identity(),
            0xA000,
            |heap| {
                events.borrow_mut().push(("destroy", heap));
                true
            },
            |address, _| events.borrow_mut().push(("restore", address)),
            |address, _| {
                events.borrow_mut().push(("retire", address));
                true
            },
            |_, _, _| events.borrow_mut().push(("report", 0)),
        );
        assert!(complete);
        assert_eq!(
            events.into_inner(),
            [("destroy", 0xA000), ("retire", 0x8000)]
        );
    }
}
