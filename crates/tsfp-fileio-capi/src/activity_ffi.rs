//! Attempt records own Rust bytes; C receives stable compatibility snapshots.
use std::ffi::{CStr, c_char};
use std::sync::Mutex;
use tsfp_engine_fileio::activity::{
    AttemptIndex, Attempts, Event, EventKind, MissingPolicy, Observer, Openables,
};
static ATTEMPTS: Mutex<Attempts> = Mutex::new(Attempts::new(64));
type Callback = unsafe extern "C" fn(u32, *const c_char, u64);
static OBSERVER: Mutex<Observer<Callback>> = Mutex::new(Observer::new());

/// Route the host probe's observer registration through Rust while preserving
/// the selected fileio backend's existing observer fanout.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_set_observer(callback: Option<Callback>) {
    unsafe { set_observer(callback) };
}
#[repr(C)]
pub struct Snapshot {
    path: [u8; 261],
    access: u32,
    share: u32,
    options: u32,
    attempts: u32,
    opened: bool,
}
pub unsafe extern "C" fn reset() {
    ATTEMPTS.lock().unwrap().reset();
}
pub unsafe extern "C" fn count() -> u32 {
    ATTEMPTS.lock().unwrap().count() as u32
}
pub unsafe extern "C" fn record(path: *const c_char, access: u32, share: u32, options: u32) -> i32 {
    if path.is_null() {
        return -1;
    }
    crate::counters::bump(crate::counters::ACTIVITY);
    let path = unsafe { CStr::from_ptr(path) }.to_bytes();
    if path.len() >= 261 {
        return -1;
    }
    ATTEMPTS
        .lock()
        .unwrap()
        .record(path, access, share, options)
        .map_or(-1, |index| index.0 as i32)
}
pub unsafe extern "C" fn get(index: u32, out: *mut Snapshot) -> bool {
    if out.is_null() {
        return false;
    }
    let records = ATTEMPTS.lock().unwrap();
    let Some(record) = records.get(AttemptIndex(index as usize)) else {
        return false;
    };
    let mut snapshot = Snapshot {
        path: [0; 261],
        access: record.access,
        share: record.share,
        options: record.options,
        attempts: record.attempts,
        opened: record.opened,
    };
    snapshot.path[..record.path.len()].copy_from_slice(&record.path);
    unsafe { *out = snapshot };
    true
}
pub unsafe extern "C" fn opened(index: u32) {
    ATTEMPTS
        .lock()
        .unwrap()
        .opened(AttemptIndex(index as usize));
}
pub unsafe extern "C" fn set_observer(callback: Option<Callback>) {
    OBSERVER.lock().unwrap().set(callback);
}
pub unsafe extern "C" fn event(kind: u32, path: *const c_char, bytes: u64) {
    if path.is_null() {
        return;
    }
    let kind = match kind {
        1 => EventKind::Open,
        2 => EventKind::Read,
        3 => EventKind::Write,
        _ => return,
    };
    let callback = OBSERVER.lock().unwrap().callback();
    if let Some(callback) = callback {
        crate::counters::bump(crate::counters::OBSERVER);
        let event = Event {
            kind,
            path: unsafe { CStr::from_ptr(path) }.to_bytes(),
            bytes,
        };
        // Path is borrowed only for the synchronous callback; no state mutex held.
        let mut terminated = event.path.to_vec();
        terminated.push(0);
        unsafe { callback(event.kind as u32, terminated.as_ptr().cast(), event.bytes) };
    }
}

pub unsafe extern "C" fn observer_enabled() -> bool {
    OBSERVER.lock().unwrap().callback().is_some()
}

static OPENABLES: Mutex<Openables> = Mutex::new(Openables::new(32, 261));
pub unsafe extern "C" fn openable_reset() {
    OPENABLES.lock().unwrap().reset();
}
pub unsafe extern "C" fn openable_add(path: *const c_char) -> bool {
    if path.is_null() {
        return false;
    }
    crate::counters::bump(crate::counters::OPENABLE);
    let path = unsafe { CStr::from_ptr(path) }.to_bytes();
    OPENABLES.lock().unwrap().add(path)
}
pub unsafe extern "C" fn openable_count() -> u32 {
    OPENABLES.lock().unwrap().count() as u32
}
pub unsafe extern "C" fn openable_contains(path: *const c_char) -> bool {
    if path.is_null() {
        return false;
    }
    let path = unsafe { CStr::from_ptr(path) }.to_bytes();
    OPENABLES.lock().unwrap().contains(path)
}
pub unsafe extern "C" fn missing_policy_set(raw: u32) {
    OPENABLES
        .lock()
        .unwrap()
        .set_policy(MissingPolicy::from_raw(raw));
}
pub unsafe extern "C" fn missing_policy_get() -> u32 {
    OPENABLES.lock().unwrap().policy() as u32
}
