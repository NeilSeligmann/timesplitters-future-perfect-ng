// SPDX-License-Identifier: GPL-3.0-or-later
//! Owned C ABI for the Rust XInput dispatch model. Live use is opt-in; C remains the
//! authoritative default and owns the measured table and diagnostics.

use std::ffi::c_void;
use std::sync::{Mutex, MutexGuard};
use tsfp_input::dispatch::{Registry, SURFACE};
use tsfp_input::dispatch_capture::{HandlerId, Lifecycle, Route};

type Handler = unsafe extern "C" fn(*mut c_void) -> u32;
const CAPTURE_MAX_ENTRIES: usize = 64;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct CaptureEntry {
    pub(crate) address: u32,
    pub(crate) handler: u32,
    pub(crate) default_return: u32,
}

#[repr(C)]
pub struct CaptureSelection {
    pub(crate) generation: u64,
    pub(crate) lifecycle_sequence: u64,
    pub(crate) selection_sequence: u64,
    pub(crate) address: u32,
    pub(crate) route: u32,
    pub(crate) handler: u32,
    pub(crate) default_return: u32,
    pub(crate) complete: u32,
    pub(crate) entry_count: u32,
    pub(crate) entries: [CaptureEntry; CAPTURE_MAX_ENTRIES],
}

fn registered(_: &mut ()) -> u32 {
    0
}

struct DispatchState {
    registry: Registry<()>,
    handlers: [Option<Handler>; SURFACE.len()],
    lifecycle: Lifecycle,
}

impl Default for DispatchState {
    fn default() -> Self {
        let mut lifecycle = Lifecycle::new(SURFACE.len());
        lifecycle.reset();
        Self {
            registry: Registry::default(),
            handlers: [None; SURFACE.len()],
            lifecycle,
        }
    }
}

pub struct Dispatch {
    state: Mutex<DispatchState>,
}

impl Default for Dispatch {
    fn default() -> Self {
        Self {
            state: Mutex::new(DispatchState::default()),
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_input_dispatch_new() -> *mut Dispatch {
    Box::into_raw(Box::new(Dispatch::default()))
}

/// The caller must keep the handle alive through each operation. State access is
/// synchronized. The selected callback is returned to C and is never invoked here.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_dispatch_free(handle: *mut Dispatch) {
    if !handle.is_null() {
        // SAFETY: the caller transfers the unique allocation back exactly once.
        drop(unsafe { Box::from_raw(handle) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_dispatch_register(
    handle: *mut Dispatch,
    address: u32,
    handler: Option<Handler>,
) -> bool {
    unsafe { tsfp_input_dispatch_register_identified(handle, address, handler, 0) }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_dispatch_register_identified(
    handle: *mut Dispatch,
    address: u32,
    handler: Option<Handler>,
    identity: u32,
) -> bool {
    let (Some(dispatch), Some(handler)) = (unsafe { handle.as_ref() }, handler) else {
        return false;
    };
    let Some(index) = SURFACE.iter().position(|entry| entry.address == address) else {
        return false;
    };
    let mut state = lock(&dispatch.state);
    state.handlers[index] = Some(handler);
    let registered = state.registry.register(address, registered);
    if registered {
        state
            .lifecycle
            .register(address, HandlerId::from_wire(identity));
    }
    registered
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_dispatch_default(
    handle: *mut Dispatch,
    address: u32,
    value: u32,
) -> bool {
    let Some(dispatch) = (unsafe { handle.as_ref() }) else {
        return false;
    };
    let mut state = lock(&dispatch.state);
    let updated = state.registry.set_default_return(address, value);
    if updated {
        state.lifecycle.set_default(address, value);
    }
    updated
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_dispatch_reset(handle: *mut Dispatch) -> bool {
    let Some(dispatch) = (unsafe { handle.as_ref() }) else {
        return false;
    };
    let mut state = lock(&dispatch.state);
    state.registry = Registry::default();
    state.handlers = [None; SURFACE.len()];
    state.lifecycle.reset();
    true
}

/// Select the registered C handler without invoking it. Live callers use this to
/// return to C before executing callbacks that may use C non-local exits.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_dispatch_select(
    handle: *mut Dispatch,
    address: u32,
) -> Option<Handler> {
    let Some(dispatch) = (unsafe { handle.as_ref() }) else {
        return None;
    };
    let mut state = lock(&dispatch.state);
    let prepared = state.registry.prepare_call(address);
    let supported = SURFACE.iter().any(|entry| entry.address == address);
    state.lifecycle.select(
        address,
        if prepared.has_handler() {
            Route::RustSelected
        } else {
            Route::CStub
        },
        supported,
    );
    if !prepared.has_handler() {
        return None;
    }
    SURFACE
        .iter()
        .position(|entry| entry.address == address)
        .and_then(|index| state.handlers[index])
}

/// Return the callback and bounded registry pre-state from the same lock.
/// The caller invokes the callback after this function returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_dispatch_select_capture(
    handle: *mut Dispatch,
    address: u32,
    output: *mut CaptureSelection,
) -> Option<Handler> {
    let (Some(dispatch), Some(output)) = (unsafe { handle.as_ref() }, unsafe { output.as_mut() })
    else {
        return None;
    };
    let mut state = lock(&dispatch.state);
    let prepared = state.registry.prepare_call(address);
    let handler = if prepared.has_handler() {
        SURFACE
            .iter()
            .position(|entry| entry.address == address)
            .and_then(|i| state.handlers[i])
    } else {
        None
    };
    let route = if handler.is_some() {
        Route::RustSelected
    } else {
        Route::CStub
    };
    *output = capture_selection(&mut state, address, route);
    handler
}

fn capture_selection(state: &mut DispatchState, address: u32, route: Route) -> CaptureSelection {
    let surface: Vec<_> = SURFACE.iter().map(|entry| entry.address).collect();
    let (selection, snapshot) = state
        .lifecycle
        .select_with_snapshot(address, route, &surface);
    let mut result = CaptureSelection {
        generation: selection.generation,
        lifecycle_sequence: selection.lifecycle_sequence,
        selection_sequence: selection.selection_sequence,
        address,
        route: match route {
            Route::RustSelected => 1,
            Route::CHandler => 2,
            Route::CStub => 3,
        },
        handler: selection.handler as u32,
        default_return: selection.default_return,
        complete: u32::from(
            selection.identity_complete()
                && snapshot.complete
                && surface.len() <= CAPTURE_MAX_ENTRIES,
        ),
        entry_count: 0,
        entries: [CaptureEntry::default(); CAPTURE_MAX_ENTRIES],
    };
    if snapshot.entries.len() <= CAPTURE_MAX_ENTRIES {
        result.entry_count = snapshot.entries.len() as u32;
        for (out, entry) in result.entries.iter_mut().zip(snapshot.entries) {
            *out = CaptureEntry {
                address: entry.address,
                handler: entry.handler.unwrap_or(HandlerId::Unknown) as u32,
                default_return: entry.default_return,
            };
        }
    }
    result
}

/// Observe C's selected callback without issuing a Rust registry selection.
/// Caller holds the C registry lock until after this returns and passes the
/// actual callback and default from that locked table. No callback is invoked.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_input_dispatch_observe_c(
    handle: *mut Dispatch,
    address: u32,
    actual: Option<Handler>,
    c_default: u32,
    output: *mut CaptureSelection,
) -> bool {
    let (Some(dispatch), Some(output)) = (unsafe { handle.as_ref() }, unsafe { output.as_mut() })
    else {
        return false;
    };
    let mut state = lock(&dispatch.state);
    let expected = SURFACE
        .iter()
        .position(|entry| entry.address == address)
        .and_then(|i| state.handlers[i]);
    let matches = match (actual, expected) {
        (Some(a), Some(b)) => std::ptr::fn_addr_eq(a, b),
        (None, None) => true,
        _ => false,
    };
    let route = if actual.is_some() {
        Route::CHandler
    } else {
        Route::CStub
    };
    let mut result = capture_selection(&mut state, address, route);
    if !matches {
        result.handler = 0;
    }
    if !matches || result.default_return != c_default {
        result.complete = 0;
    }
    *output = result;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    unsafe extern "C" fn increment(user: *mut c_void) -> u32 {
        let value = unsafe { &mut *user.cast::<u32>() };
        *value += 1;
        *value
    }

    unsafe extern "C" fn atomic_increment(user: *mut c_void) -> u32 {
        unsafe { &*user.cast::<AtomicU32>() }.fetch_add(1, Ordering::Relaxed) + 1
    }

    #[test]
    fn independent_handles_preserve_registration_and_selection() {
        let a = tsfp_input_dispatch_new();
        let b = tsfp_input_dispatch_new();
        let address = SURFACE[0].address;
        let mut value = 7u32;
        unsafe {
            assert!(!tsfp_input_dispatch_register(a, address, None));
            assert!(!tsfp_input_dispatch_register(a, 0, Some(increment)));
            assert!(tsfp_input_dispatch_default(b, address, 42));
            assert!(tsfp_input_dispatch_select(b, address).is_none());
            assert!(tsfp_input_dispatch_select(a, 0).is_none());
            assert!(tsfp_input_dispatch_register(a, address, Some(increment)));
            let selected = tsfp_input_dispatch_select(a, address).expect("registered handler");
            assert_eq!(selected((&mut value as *mut u32).cast()), 8);
            tsfp_input_dispatch_free(a);
            tsfp_input_dispatch_free(b);
            tsfp_input_dispatch_free(std::ptr::null_mut());
        }
    }

    #[test]
    fn registration_can_be_replaced_after_selection_returns() {
        let handle = tsfp_input_dispatch_new();
        unsafe {
            assert!(tsfp_input_dispatch_register(
                handle,
                SURFACE[0].address,
                Some(increment),
            ));
            let selected =
                tsfp_input_dispatch_select(handle, SURFACE[0].address).expect("first handler");
            assert!(tsfp_input_dispatch_register(
                handle,
                SURFACE[0].address,
                Some(atomic_increment),
            ));
            assert!(tsfp_input_dispatch_select(handle, SURFACE[0].address).is_some());
            let mut value = 6u32;
            assert_eq!(selected((&mut value as *mut u32).cast()), 7);
            tsfp_input_dispatch_free(handle);
        }
    }

    #[test]
    fn capture_select_pairs_registry_state_with_callback_and_reset() {
        let handle = tsfp_input_dispatch_new();
        let address = SURFACE[0].address;
        let mut capture = std::mem::MaybeUninit::<CaptureSelection>::uninit();
        unsafe {
            assert!(tsfp_input_dispatch_register_identified(
                handle,
                address,
                Some(increment),
                3
            ));
            assert!(tsfp_input_dispatch_default(handle, address, 42));
            assert!(
                tsfp_input_dispatch_select_capture(handle, address, capture.as_mut_ptr()).is_some()
            );
            let selected = capture.assume_init();
            assert_eq!(
                (selected.complete, selected.handler, selected.default_return),
                (1, 3, 42)
            );
            assert_eq!(selected.entries[0].handler, 3);
            assert!(tsfp_input_dispatch_reset(handle));
            let mut capture = std::mem::MaybeUninit::<CaptureSelection>::uninit();
            assert!(
                tsfp_input_dispatch_select_capture(handle, address, capture.as_mut_ptr()).is_none()
            );
            let stub = capture.assume_init();
            assert_eq!(
                (stub.generation, stub.route, stub.default_return),
                (2, 3, 0)
            );
            assert!(stub.selection_sequence > selected.selection_sequence);
            assert!(tsfp_input_dispatch_register(
                handle,
                address,
                Some(increment)
            ));
            let mut capture = std::mem::MaybeUninit::<CaptureSelection>::uninit();
            assert!(
                tsfp_input_dispatch_select_capture(handle, address, capture.as_mut_ptr()).is_some()
            );
            assert_eq!(capture.assume_init().complete, 0);
            tsfp_input_dispatch_free(handle);
        }
    }

    #[test]
    fn c_observation_does_not_select_and_checks_c_state() {
        let handle = tsfp_input_dispatch_new();
        let address = SURFACE[0].address;
        unsafe {
            assert!(tsfp_input_dispatch_register_identified(
                handle,
                address,
                Some(increment),
                3
            ));
            let before = lock(&(*handle).state).registry.call_count(address);
            let mut capture = std::mem::MaybeUninit::<CaptureSelection>::uninit();
            assert!(tsfp_input_dispatch_observe_c(
                handle,
                address,
                Some(increment),
                0,
                capture.as_mut_ptr()
            ));
            let snapshot = capture.assume_init();
            assert_eq!(
                (snapshot.route, snapshot.handler, snapshot.complete),
                (2, 3, 1)
            );
            assert_eq!(lock(&(*handle).state).registry.call_count(address), before);
            let mut mismatch = std::mem::MaybeUninit::<CaptureSelection>::uninit();
            assert!(tsfp_input_dispatch_observe_c(
                handle,
                address,
                Some(atomic_increment),
                0,
                mismatch.as_mut_ptr()
            ));
            assert_eq!(mismatch.assume_init().handler, 0);
            let mut stub = std::mem::MaybeUninit::<CaptureSelection>::uninit();
            assert!(tsfp_input_dispatch_observe_c(
                handle,
                address,
                None,
                0,
                stub.as_mut_ptr()
            ));
            let stub = stub.assume_init();
            assert_eq!((stub.route, stub.handler, stub.complete), (3, 0, 0));
            let mut default_mismatch = std::mem::MaybeUninit::<CaptureSelection>::uninit();
            assert!(tsfp_input_dispatch_observe_c(
                handle,
                address,
                Some(increment),
                1,
                default_mismatch.as_mut_ptr()
            ));
            assert_eq!(default_mismatch.assume_init().complete, 0);
            assert_eq!(lock(&(*handle).state).registry.call_count(address), before);
            tsfp_input_dispatch_free(handle);
        }
    }

    #[test]
    fn concurrent_selection_is_synchronized() {
        let handle = tsfp_input_dispatch_new();
        let address = SURFACE[0].address;
        let total = Arc::new(AtomicU32::new(0));
        unsafe {
            assert!(tsfp_input_dispatch_register(
                handle,
                address,
                Some(atomic_increment)
            ))
        };
        let handle_addr = handle as usize;
        let workers: Vec<_> = (0..4)
            .map(|_| {
                let total = Arc::clone(&total);
                std::thread::spawn(move || {
                    for _ in 0..64 {
                        unsafe {
                            let handler =
                                tsfp_input_dispatch_select(handle_addr as *mut Dispatch, address)
                                    .expect("registered handler");
                            handler(Arc::as_ptr(&total).cast_mut().cast());
                        }
                    }
                })
            })
            .collect();
        for worker in workers {
            worker.join().expect("dispatch worker");
        }
        assert_eq!(total.load(Ordering::Relaxed), 256);
        unsafe { tsfp_input_dispatch_free(handle) };
    }

    #[test]
    fn concurrent_capture_selections_have_contiguous_unique_sequences() {
        let handle = tsfp_input_dispatch_new();
        let address = SURFACE[0].address;
        unsafe {
            assert!(tsfp_input_dispatch_register_identified(
                handle,
                address,
                Some(increment),
                3
            ));
        }
        let address_of_handle = handle as usize;
        let workers: Vec<_> = (0..4)
            .map(|_| {
                std::thread::spawn(move || {
                    (0..64)
                        .map(|_| {
                            let mut capture = std::mem::MaybeUninit::<CaptureSelection>::uninit();
                            unsafe {
                                assert!(
                                    tsfp_input_dispatch_select_capture(
                                        address_of_handle as *mut Dispatch,
                                        address,
                                        capture.as_mut_ptr()
                                    )
                                    .is_some()
                                );
                                let capture = capture.assume_init();
                                assert_eq!(capture.complete, 1);
                                capture.selection_sequence
                            }
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        let mut sequences: Vec<_> = workers
            .into_iter()
            .flat_map(|worker| worker.join().unwrap())
            .collect();
        sequences.sort_unstable();
        assert_eq!(sequences, (1..=256).collect::<Vec<_>>());
        unsafe { tsfp_input_dispatch_free(handle) };
    }
}
