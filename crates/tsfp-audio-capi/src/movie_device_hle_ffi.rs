//! Rust-owned DirectSound HLE registration/dispatch and XMV movie-device Release adapter.

use std::ffi::c_void;
use std::sync::Mutex;

use crate::hle_surface::LOOKUP_UNINITIALIZED;
use crate::movie_policy_ffi::tsfp_audio_movie_device_release_frame_rust;

const MOVIE_DEVICE_RELEASE_ENTRY: u32 = 0x0040_6A8A;
const REFUSE_CALLER: u32 = 0;
const REFUSE_ARGUMENT: u32 = 1;

type DsoundHandler = unsafe extern "C" fn(*mut c_void) -> u32;

#[derive(Clone, Copy)]
struct HleRegistration {
    address: u32,
    handler: Option<DsoundHandler>,
    implemented: bool,
    default_return: u32,
}

static RUST_HLE_REGISTRY: Mutex<Vec<HleRegistration>> = Mutex::new(Vec::new());

/// Reset per-host registration state during DirectSound subsystem initialization.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_hle_registry_reset_rust() {
    RUST_HLE_REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
}

/// `(implemented, has a callback)` of a registered measured entry, or `None` when nothing was registered for it.
pub(crate) fn registration(address: u32) -> Option<(bool, bool)> {
    RUST_HLE_REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .find(|entry| entry.address == address)
        .map(|entry| (entry.implemented, entry.handler.is_some()))
}

fn with_registration(address: u32, update: impl FnOnce(&mut HleRegistration)) -> bool {
    if !crate::hle_surface::contains_address(address) {
        return false;
    }
    let mut registry = RUST_HLE_REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let entry = registry.iter_mut().find(|entry| entry.address == address);
    if let Some(entry) = entry {
        update(entry);
    } else {
        let mut entry = HleRegistration {
            address,
            handler: None,
            implemented: false,
            default_return: 0,
        };
        update(&mut entry);
        registry.push(entry);
    }
    true
}

unsafe extern "C" {
    fn dsound_hle_register_rust(address: u32) -> bool;
    fn tsfp_audio_movie_release_frame_snapshot(
        context: *mut c_void,
        host_pid_out: *mut i32,
        stack_address_out: *mut u32,
        stack_limit_out: *mut u32,
    ) -> bool;
    fn tsfp_audio_movie_release_refuse(reason: u32) -> !;
    fn dsound_device_release_movie(interface: u32) -> u32;
}

/// Register the measured movie-owned device Release entry with the C telemetry table while
/// retaining the callback in the Rust-owned registry.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_movie_device_register_release_rust() -> usize {
    if !tsfp_audio_hle_register_handler_rust(
        MOVIE_DEVICE_RELEASE_ENTRY,
        movie_device_release_handler,
    ) {
        return 0;
    }
    // SAFETY: the C table is initialized before audio registration and retains only the
    // address/call-count row; Rust owns the callback selected for this entry.
    if unsafe { dsound_hle_register_rust(MOVIE_DEVICE_RELEASE_ENTRY) } {
        1
    } else {
        0
    }
}

/// Register a C HLE callback in the Rust-host measured registry. Unknown addresses are rejected
/// here, before the C compatibility projection is updated.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_hle_register_handler_rust(
    address: u32,
    handler: DsoundHandler,
) -> bool {
    with_registration(address, |entry| {
        entry.handler = Some(handler);
        entry.implemented = true;
    })
}

/// Register an address-specific default return for an unimplemented measured entry.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_hle_set_default_return_rust(address: u32, value: u32) -> bool {
    with_registration(address, |entry| entry.default_return = value)
}

/// Mark a measured entry implemented when its callback is registered by Rust itself.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_hle_mark_implemented_rust(address: u32) -> bool {
    with_registration(address, |entry| entry.implemented = true)
}

/// Select and invoke the Rust-owned registered callback or measured default return.
/// Status 0 means the measured registry is unavailable, 1 is a stub, 2 is dispatched,
/// and 3 is an inconsistent implemented row without a callback.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_hle_invoke_rust(
    address: u32,
    context: *mut c_void,
    result_out: *mut u32,
) -> u32 {
    if result_out.is_null() || !crate::hle_surface::contains_address(address) {
        return 0;
    }
    let (implemented, handler, default_return) = {
        let registry = RUST_HLE_REGISTRY
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        registry
            .iter()
            .find(|entry| entry.address == address)
            .map_or((false, None, 0), |entry| {
                (entry.implemented, entry.handler, entry.default_return)
            })
    };
    if !implemented {
        // SAFETY: the caller supplies the checked synchronous result slot.
        unsafe { result_out.write(default_return) };
        return 1;
    }
    let Some(handler) = handler else {
        // SAFETY: the caller supplies the checked synchronous result slot.
        unsafe { result_out.write(default_return) };
        return 3;
    };
    // SAFETY: registrations retain process-lifetime callbacks with the dsound_fn ABI. The
    // registry lock is dropped before the callback runs, and the context is the live thunk frame.
    let result = unsafe { handler(context) };
    // SAFETY: the caller supplies the checked synchronous result slot.
    unsafe { result_out.write(result) };
    2
}

/// Perform the Rust-host measured-address lookup, telemetry update, registration selection,
/// and synchronous handler invocation as one dispatch transaction. C receives only the
/// compatibility projection needed by its existing reporting surface.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audio_hle_call_rust(
    address: u32,
    context: *mut c_void,
    index_out: *mut u32,
    calls_out: *mut u64,
    sites_out: *mut u32,
    result_out: *mut u32,
) -> u32 {
    if index_out.is_null() || calls_out.is_null() || sites_out.is_null() || result_out.is_null() {
        return LOOKUP_UNINITIALIZED;
    }
    // SAFETY: all four output pointers were checked; their synchronous caller owns them.
    let mut index = 0;
    let mut calls = 0;
    let mut sites = 0;
    let lookup = unsafe {
        crate::hle_surface::tsfp_audio_hle_surface_record_call(
            address, &mut index, &mut calls, &mut sites,
        )
    };
    if lookup != crate::hle_surface::LOOKUP_KNOWN {
        return lookup;
    }
    unsafe {
        index_out.write(index);
        calls_out.write(calls);
        sites_out.write(sites);
    }

    let (implemented, handler, default_return) = {
        let registry = RUST_HLE_REGISTRY
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        registry
            .iter()
            .find(|entry| entry.address == address)
            .map_or((false, None, 0), |entry| {
                (entry.implemented, entry.handler, entry.default_return)
            })
    };
    if !implemented {
        unsafe { result_out.write(default_return) };
        return match crate::hle_surface::mark_reported(index as usize) {
            Some(true) => 3,  // first measured-stub report
            Some(false) => 6, // already reported this run
            None => LOOKUP_UNINITIALIZED,
        };
    }
    let Some(handler) = handler else {
        unsafe { result_out.write(default_return) };
        return 4; // implementation flag without a callable registration
    };
    // SAFETY: handlers are process-lifetime registrations. The registry lock is released
    // before guest code runs; context remains live for this synchronous HLE call.
    let result = unsafe { handler(context) };
    unsafe { result_out.write(result) };
    5 // dispatched
}

/// Dispatch an address registered in the Rust audio HLE table.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audio_hle_dispatch_rust(
    address: u32,
    context: *mut c_void,
    result_out: *mut u32,
) -> bool {
    if result_out.is_null() {
        return false;
    }
    let handler = {
        let registry = RUST_HLE_REGISTRY
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        registry
            .iter()
            .find(|entry| entry.address == address && entry.implemented)
            .and_then(|entry| entry.handler)
    };
    let Some(handler) = handler else {
        return false;
    };
    // SAFETY: registration stores process-lifetime handlers with the `dsound_fn` ABI. The
    // registry lock is dropped before invocation, and the C dispatcher passes the live
    // synchronous call frame through unchanged.
    let result = unsafe { handler(context) };
    // SAFETY: the dispatcher checked this synchronous output slot for null above.
    unsafe { result_out.write(result) };
    true
}

unsafe extern "C" fn movie_device_release_handler(context: *mut c_void) -> u32 {
    let mut host_pid = 0;
    let mut stack_address = 0;
    let mut stack_limit = 0;
    // SAFETY: the host helper reads the call-frame fields synchronously while the dispatcher
    // owns the frame. The output pointers refer to live local storage.
    let has_frame = unsafe {
        tsfp_audio_movie_release_frame_snapshot(
            context,
            &mut host_pid,
            &mut stack_address,
            &mut stack_limit,
        )
    };
    if !has_frame {
        // SAFETY: the host refusal adapter logs the measured entry and terminates the call.
        unsafe { tsfp_audio_movie_release_refuse(REFUSE_CALLER) };
    }

    let mut interface = 0;
    // SAFETY: the snapshot contains a live host PID and guest stack bounds. Rust performs the
    // fault-safe reads and checks the exact caller and bounded interface argument.
    let decoded = unsafe {
        tsfp_audio_movie_device_release_frame_rust(
            host_pid,
            stack_address,
            stack_limit,
            &mut interface,
        )
    };
    match decoded {
        1 => {
            // SAFETY: lifecycle/reference application remains the measured C host operation.
            unsafe { dsound_device_release_movie(interface) }
        }
        0 => {
            // SAFETY: preserves the existing caller refusal diagnostic and fatal behavior.
            unsafe { tsfp_audio_movie_release_refuse(REFUSE_CALLER) }
        }
        _ => {
            // SAFETY: preserves the existing bounded argument-read refusal diagnostic.
            unsafe { tsfp_audio_movie_release_refuse(REFUSE_ARGUMENT) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static REGISTERED: Mutex<Option<u32>> = Mutex::new(None);
    static RELEASED_INTERFACE: Mutex<Option<u32>> = Mutex::new(None);

    #[repr(C)]
    struct TestFrame {
        host_pid: i32,
        stack_address: u32,
        stack_limit: u32,
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn dsound_hle_register_rust(address: u32) -> bool {
        *REGISTERED.lock().unwrap() = Some(address);
        true
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn tsfp_audio_movie_release_frame_snapshot(
        context: *mut c_void,
        host_pid_out: *mut i32,
        stack_address_out: *mut u32,
        stack_limit_out: *mut u32,
    ) -> bool {
        if context.is_null()
            || host_pid_out.is_null()
            || stack_address_out.is_null()
            || stack_limit_out.is_null()
        {
            return false;
        }
        // SAFETY: the test passes a pointer to a live TestFrame and valid output storage.
        let frame = unsafe { &*context.cast::<TestFrame>() };
        unsafe {
            host_pid_out.write(frame.host_pid);
            stack_address_out.write(frame.stack_address);
            stack_limit_out.write(frame.stack_limit);
        }
        true
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn tsfp_audio_movie_release_refuse(_reason: u32) -> ! {
        std::process::abort()
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn dsound_device_release_movie(interface: u32) -> u32 {
        *RELEASED_INTERFACE.lock().unwrap() = Some(interface);
        0xC0DE
    }

    #[test]
    fn rust_registers_and_dispatches_movie_release_through_the_hle_contract() {
        *REGISTERED.lock().unwrap() = None;
        *RELEASED_INTERFACE.lock().unwrap() = None;
        tsfp_audio_hle_registry_reset_rust();
        assert!(crate::hle_surface::tsfp_audio_hle_surface_init());
        assert_eq!(tsfp_audio_movie_device_register_release_rust(), 1);
        let entry = REGISTERED.lock().unwrap().expect("HLE entry registered");
        assert_eq!(entry, MOVIE_DEVICE_RELEASE_ENTRY);

        let (page, address) = crate::test_guest_memory::map_guest_page();
        // SAFETY: the test owns the page and writes the measured caller plus interface argument.
        unsafe {
            page.cast::<u32>().write(0x0044_5726);
            page.add(4).cast::<u32>().write(0x1234_5678);
        }
        let frame = TestFrame {
            host_pid: std::process::id() as i32,
            stack_address: address,
            stack_limit: address + 8,
        };
        // SAFETY: the frame and its guest mapping remain alive for the synchronous callback.
        let mut result = 0;
        let mut row_index = u32::MAX;
        let mut calls = 0;
        let mut sites = 0;
        // SAFETY: all output pointers and the test-owned call frame remain live for dispatch.
        let status = unsafe {
            tsfp_audio_hle_call_rust(
                entry,
                (&frame as *const TestFrame).cast_mut().cast(),
                &mut row_index,
                &mut calls,
                &mut sites,
                &mut result,
            )
        };
        assert_eq!(status, 5);
        assert_eq!(sites, 2);
        assert_eq!(calls, 1);
        assert_ne!(row_index, u32::MAX);
        assert_eq!(result, 0xC0DE);
        assert_eq!(*RELEASED_INTERFACE.lock().unwrap(), Some(0x1234_5678));
        let mut ignored_index = 0;
        let mut ignored_calls = 0;
        let mut ignored_sites = 0;
        let mut stub_calls = 0;
        let mut stub_sites = 0;
        let mut stub_result = u32::MAX;
        // SAFETY: the measured stub and all output slots are valid for these synchronous calls.
        let first_stub = unsafe {
            tsfp_audio_hle_call_rust(
                0x0040_7AA4,
                std::ptr::null_mut(),
                &mut ignored_index,
                &mut stub_calls,
                &mut stub_sites,
                &mut stub_result,
            )
        };
        assert_eq!(first_stub, 3);
        assert_eq!(stub_result, 0);
        let second_stub = unsafe {
            tsfp_audio_hle_call_rust(
                0x0040_7AA4,
                std::ptr::null_mut(),
                &mut ignored_index,
                &mut stub_calls,
                &mut stub_sites,
                &mut stub_result,
            )
        };
        assert_eq!(second_stub, 6);
        assert_eq!(stub_calls, 2);
        let mut unchanged = 0xCAFE_BABEu32;
        // SAFETY: outputs are writable; unknown-address dispatch never reads context.
        let unknown = unsafe {
            tsfp_audio_hle_call_rust(
                MOVIE_DEVICE_RELEASE_ENTRY + 1,
                std::ptr::null_mut(),
                &mut ignored_index,
                &mut ignored_calls,
                &mut ignored_sites,
                &mut unchanged,
            )
        };
        assert_eq!(unknown, crate::hle_surface::LOOKUP_UNKNOWN_INSIDE);
        assert_eq!(unchanged, 0xCAFE_BABE);

        // T3918: the host contract owns counts, the report-once flag and the label; a handled entry logs nothing.
        use crate::hle_host_entry::{
            TEST_LOG, tsfp_audio_host_hle_call, tsfp_audio_host_hle_call_count,
        };
        TEST_LOG.lock().unwrap().clear();
        let stub = 0x0040_7ABC;
        // SAFETY: stubs and unknown targets never read the context.
        let results = unsafe {
            [
                tsfp_audio_host_hle_call(stub, std::ptr::null_mut()),
                tsfp_audio_host_hle_call(stub, std::ptr::null_mut()),
                tsfp_audio_host_hle_call(0x0040_67C4 + 1, std::ptr::null_mut()),
                tsfp_audio_host_hle_call(0x0040_67C4 + 1, std::ptr::null_mut()),
                tsfp_audio_host_hle_call(0x1000, std::ptr::null_mut()),
                tsfp_audio_host_hle_call(
                    MOVIE_DEVICE_RELEASE_ENTRY,
                    (&frame as *const TestFrame).cast_mut().cast(),
                ),
            ]
        };
        assert_eq!(results, [0, 0, 0, 0, 0, 0xC0DE]);
        assert_eq!(tsfp_audio_host_hle_call_count(stub), 2);
        assert_eq!(
            tsfp_audio_host_hle_call_count(MOVIE_DEVICE_RELEASE_ENTRY),
            2
        );
        assert_eq!(tsfp_audio_host_hle_call_count(0x1000), 0);
        let log = TEST_LOG.lock().unwrap().clone();
        assert_eq!(log.len(), 4, "{log:?}");
        assert!(
            log[0].starts_with("dsound: 0x00407abc ") && log[0].contains("is not implemented ("),
            "{log:?}"
        );
        assert!(log[0].ends_with(", returning 0x0\n"), "{log:?}");
        assert!(
            log[1].contains("inside the DSOUND section") && log[2] == log[1],
            "{log:?}"
        );
        assert!(log[3].contains("OUTSIDE the DSOUND section"), "{log:?}");
        crate::test_guest_memory::unmap_guest_page(page);
    }

    unsafe extern "C" fn test_c_hle_handler(context: *mut c_void) -> u32 {
        if context.is_null() {
            return 0;
        }
        // SAFETY: the test passes a live pointer to its local counter for this synchronous call.
        let calls = unsafe { &mut *context.cast::<u32>() };
        *calls += 1;
        *calls + 40
    }

    unsafe extern "C" fn replacement_c_hle_handler(context: *mut c_void) -> u32 {
        if context.is_null() {
            return 0;
        }
        // SAFETY: the test passes a live pointer to its local counter for this synchronous call.
        let calls = unsafe { &mut *context.cast::<u32>() };
        *calls += 1;
        *calls + 70
    }

    #[test]
    fn rust_registry_selects_c_hle_handlers_by_exact_address() {
        const ENTRY: u32 = 0x0040_7abc;
        assert!(!tsfp_audio_hle_register_handler_rust(
            0x0040_1234,
            test_c_hle_handler,
        ));
        assert!(tsfp_audio_hle_register_handler_rust(
            ENTRY,
            test_c_hle_handler
        ));

        let mut calls = 0u32;
        let mut result = 0xCAFE_BABE;
        // SAFETY: the test passes a live call counter and result slot for the synchronous callback.
        assert_eq!(
            unsafe {
                tsfp_audio_hle_invoke_rust(ENTRY, (&mut calls as *mut u32).cast(), &mut result)
            },
            2
        );
        assert_eq!(calls, 1);
        assert_eq!(result, 41);

        assert!(tsfp_audio_hle_register_handler_rust(
            ENTRY,
            replacement_c_hle_handler,
        ));
        // SAFETY: the test passes a live call counter and result slot for the synchronous callback.
        assert_eq!(
            unsafe {
                tsfp_audio_hle_invoke_rust(ENTRY, (&mut calls as *mut u32).cast(), &mut result)
            },
            2
        );
        assert_eq!(calls, 2);
        assert_eq!(result, 72);

        let mut untouched = 0xDEAD_BEEF;
        // SAFETY: the output and context are valid; an unregistered address cannot call the context.
        assert_eq!(
            unsafe {
                tsfp_audio_hle_invoke_rust(
                    0x0040_1234,
                    (&mut calls as *mut u32).cast(),
                    &mut untouched,
                )
            },
            0
        );
        assert_eq!(untouched, 0xDEAD_BEEF);
        assert_eq!(calls, 2);
    }

    #[test]
    fn rust_dispatch_uses_the_measured_default_for_unimplemented_entries() {
        const ENTRY: u32 = 0x0040_7b40;
        const DEFAULT: u32 = 0x8000_4005;
        assert!(tsfp_audio_hle_set_default_return_rust(ENTRY, DEFAULT));

        let mut result = 0xDEAD_BEEF;
        // SAFETY: `result` is a live writable output for this synchronous call.
        let status =
            unsafe { tsfp_audio_hle_invoke_rust(ENTRY, std::ptr::null_mut(), &mut result) };
        assert_eq!(status, 1);
        assert_eq!(result, DEFAULT);

        assert!(tsfp_audio_hle_mark_implemented_rust(ENTRY));
        // SAFETY: `result` remains a live writable output for the synchronous call.
        let inconsistent =
            unsafe { tsfp_audio_hle_invoke_rust(ENTRY, std::ptr::null_mut(), &mut result) };
        assert_eq!(inconsistent, 3);
        assert_eq!(result, DEFAULT);

        // SAFETY: null output is rejected before any write or callback invocation.
        assert_eq!(
            unsafe {
                tsfp_audio_hle_invoke_rust(ENTRY, std::ptr::null_mut(), std::ptr::null_mut())
            },
            0
        );
    }
}
