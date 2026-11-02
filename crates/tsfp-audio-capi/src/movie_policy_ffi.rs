//! Rust-owned admission policy for the live XMV movie stream facade.

use tsfp_audio::adapter::GuestMemory;
use tsfp_audio::movie;
#[cfg(feature = "rust-audio-host")]
use tsfp_audio::movie::MoviePolicyError;

use crate::frame_guest_memory::FrameGuestMemory;

#[cfg(feature = "rust-audio-host")]
type CurrentIrql = unsafe extern "C" fn(*mut u8) -> bool;
#[cfg(feature = "rust-audio-host")]
type ReadGuestU32 = unsafe extern "C" fn(u32, *mut u32) -> bool;

/// Store the Rust-host movie-audio policy flag behind the C facade's stable setter ABI.
#[cfg(feature = "rust-audio-host")]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_set_enabled_rust(enabled: bool) {
    crate::movie_host::dsound_movie_stream_set_enabled(enabled);
}

/// Read the Rust-host movie-audio policy flag behind the C facade's stable getter ABI.
#[cfg(feature = "rust-audio-host")]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_enabled_rust() -> bool {
    crate::movie_host::dsound_movie_stream_enabled()
}

/// Check the measured XMV caller for the movie-owned DirectSound device Release entry.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_device_release_caller_ok_rust(address: u32) -> bool {
    movie::device_release_caller_ok(address)
}

/// Decode the movie device Release caller and interface argument through Rust guest memory.
/// Returns 0 for a missing frame or rejected caller, 1 for a decoded argument, and 2 when the
/// caller is admitted but the bounded argument read fails. The C host keeps its guarded reads.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_movie_device_release_frame_rust(
    host_pid: i32,
    stack_address: u32,
    stack_limit: u32,
    interface_out: *mut u32,
) -> u32 {
    if host_pid <= 0 || stack_address == 0 || interface_out.is_null() {
        return 0;
    }
    let memory = FrameGuestMemory::new(host_pid);
    let Some(caller) = memory.read_u32(stack_address) else {
        return 0;
    };
    if !movie::device_release_caller_ok(caller) {
        return 0;
    }
    let argument_address = u64::from(stack_address) + 4;
    let argument_end = argument_address + 4;
    if argument_end > (1u64 << 32) || (stack_limit != 0 && argument_end > u64::from(stack_limit)) {
        return 2;
    }
    let Some(interface) = memory.read_u32(argument_address as u32) else {
        return 2;
    };
    // SAFETY: the C adapter passes writable stack-local storage and checked it is non-null.
    unsafe { interface_out.write(interface) };
    1
}

/// Evaluate movie policy with host callbacks for the current-thread IRQL and guest state read.
///
/// Return codes: 0 = allowed, 1 = disabled, 2 = unknown/nonzero IRQL, 3 = unavailable/nonzero
/// global audio state. The callbacks are invoked lazily in the same short-circuit order as the C
/// oracle: disabled policy reads neither IRQL nor guest memory; invalid IRQL reads no guest state.
#[cfg(feature = "rust-audio-host")]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_policy_rust(
    current_irql: Option<CurrentIrql>,
    read_guest_u32: Option<ReadGuestU32>,
) -> u32 {
    let result = movie::check_policy(
        crate::movie_host::dsound_movie_stream_enabled(),
        || {
            let callback = current_irql?;
            let mut value = 0;
            // SAFETY: caller supplies a live synchronous host callback and writable output slot.
            unsafe { callback(&mut value) }.then_some(value)
        },
        || {
            let callback = read_guest_u32?;
            let mut value = 0;
            // SAFETY: caller supplies a live synchronous host callback and writable output slot.
            unsafe { callback(movie::GLOBAL_STATE, &mut value) }.then_some(value)
        },
    );
    match result {
        Ok(()) => 0,
        Err(MoviePolicyError::Disabled) => 1,
        Err(MoviePolicyError::IrqlUnknownOrNonzero) => 2,
        Err(MoviePolicyError::GlobalStateUnavailableOrNonzero) => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "rust-audio-host")]
    use std::cell::Cell;

    #[cfg(feature = "rust-audio-host")]
    thread_local! {
        static STATE: Cell<(Option<u8>, Option<u32>, u32)> = const { Cell::new((Some(0), Some(0), 0)) };
    }

    #[cfg(feature = "rust-audio-host")]
    unsafe extern "C" fn irql(out: *mut u8) -> bool {
        if out.is_null() {
            return false;
        }
        let Some(value) = STATE.with(|state| state.get().0) else {
            return false;
        };
        // SAFETY: the exported policy supplies one writable byte.
        unsafe { out.write(value) };
        true
    }

    #[cfg(feature = "rust-audio-host")]
    unsafe extern "C" fn guest_read(address: u32, out: *mut u32) -> bool {
        if address != movie::GLOBAL_STATE || out.is_null() {
            return false;
        }
        let Some(value) = STATE.with(|state| state.get().1) else {
            return false;
        };
        STATE.with(|state| {
            let (_, _, reads) = state.get();
            state.set((state.get().0, state.get().1, reads + 1));
        });
        // SAFETY: the exported policy supplies one writable dword.
        unsafe { out.write(value) };
        true
    }

    #[test]
    #[cfg(feature = "rust-audio-host")]
    fn movie_policy_ffi_preserves_order_and_host_oracle_refusals() {
        tsfp_audio_movie_set_enabled_rust(false);
        assert!(!tsfp_audio_movie_enabled_rust());
        STATE.with(|state| state.set((Some(0), Some(0), 0)));
        assert_eq!(
            tsfp_audio_movie_policy_rust(Some(irql), Some(guest_read)),
            1
        );
        assert_eq!(STATE.with(|state| state.get().2), 0);

        tsfp_audio_movie_set_enabled_rust(true);
        assert!(tsfp_audio_movie_enabled_rust());
        STATE.with(|state| state.set((None, Some(0), 0)));
        assert_eq!(
            tsfp_audio_movie_policy_rust(Some(irql), Some(guest_read)),
            2
        );
        assert_eq!(STATE.with(|state| state.get().2), 0);

        STATE.with(|state| state.set((Some(0), Some(1), 0)));
        assert_eq!(
            tsfp_audio_movie_policy_rust(Some(irql), Some(guest_read)),
            3
        );
        assert_eq!(STATE.with(|state| state.get().2), 1);

        STATE.with(|state| state.set((Some(0), Some(0), 0)));
        assert_eq!(
            tsfp_audio_movie_policy_rust(Some(irql), Some(guest_read)),
            0
        );
        assert_eq!(STATE.with(|state| state.get().2), 1);
        tsfp_audio_movie_set_enabled_rust(false);
    }

    #[test]
    fn movie_device_release_ffi_admits_only_the_measured_return_address() {
        let caller = movie::DEVICE_RELEASE_CALLER;
        assert!(tsfp_audio_movie_device_release_caller_ok_rust(caller));
        assert!(!tsfp_audio_movie_device_release_caller_ok_rust(caller - 1));
        assert!(!tsfp_audio_movie_device_release_caller_ok_rust(caller + 1));
    }

    #[test]
    fn movie_device_release_decodes_caller_and_interface_with_guest_stack_bounds() {
        let (page, address) = crate::test_guest_memory::map_guest_page();
        let caller = movie::DEVICE_RELEASE_CALLER;
        // SAFETY: the test owns this writable identity-mapped guest page.
        unsafe {
            page.cast::<u32>().write(caller);
            page.cast::<u32>().add(1).write(0x1234_5678);
        }
        let pid = std::process::id() as i32;
        let mut interface = 0;
        assert_eq!(
            unsafe { tsfp_audio_movie_device_release_frame_rust(pid, address, 0, &mut interface) },
            1
        );
        assert_eq!(interface, 0x1234_5678);
        assert_eq!(
            unsafe {
                tsfp_audio_movie_device_release_frame_rust(
                    pid,
                    address,
                    address + 8,
                    &mut interface,
                )
            },
            1
        );
        assert_eq!(
            unsafe {
                tsfp_audio_movie_device_release_frame_rust(
                    pid,
                    address,
                    address + 7,
                    &mut interface,
                )
            },
            2
        );

        // SAFETY: the same mapped page remains writable for the negative caller case.
        unsafe { page.cast::<u32>().write(caller + 1) };
        assert_eq!(
            unsafe { tsfp_audio_movie_device_release_frame_rust(pid, address, 0, &mut interface) },
            0
        );
        assert_eq!(
            unsafe { tsfp_audio_movie_device_release_frame_rust(pid, 0, 0, &mut interface) },
            0
        );
        assert_eq!(
            unsafe {
                tsfp_audio_movie_device_release_frame_rust(pid, u32::MAX - 1, 0, &mut interface)
            },
            0
        );
        assert_eq!(
            unsafe {
                tsfp_audio_movie_device_release_frame_rust(pid, address, 0, std::ptr::null_mut())
            },
            0
        );

        // A valid caller at the last mapped word cannot read an interface argument beyond
        // the supplied exclusive stack bound.
        let argumentless_stack = address + 4092;
        // SAFETY: the test owns the last aligned word of this mapping.
        unsafe { page.cast::<u32>().add(1023).write(caller) };
        assert_eq!(
            unsafe {
                tsfp_audio_movie_device_release_frame_rust(
                    pid,
                    argumentless_stack,
                    address + 4096,
                    &mut interface,
                )
            },
            2
        );
        crate::test_guest_memory::unmap_guest_page(page);
        assert_eq!(
            unsafe { tsfp_audio_movie_device_release_frame_rust(pid, address, 0, &mut interface) },
            0
        );
    }
}
