//! T2210: boundary callbacks; all completion state/decisions live in the safe crate.
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicU32, Ordering},
};
use tsfp_engine_fileio::async_io::{AsyncIo, CompletionHost, Read, Stats, Volume};

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Host {
    now: extern "C" fn() -> u64,
    advance: extern "C" fn(u64),
    identity: extern "C" fn(u32) -> u32,
    live: extern "C" fn(u32) -> bool,
    buffer: unsafe extern "C" fn(u32, *const u8, u32) -> bool,
    iosb: extern "C" fn(u32, u32, u32),
    signal: extern "C" fn(u32),
    signalled: extern "C" fn(u32) -> bool,
}
impl CompletionHost for Host {
    fn now(&self) -> u64 {
        (self.now)()
    }
    fn advance_to(&mut self, ticks: u64) {
        (self.advance)(ticks);
    }
    fn identity(&self, file: u32) -> u32 {
        (self.identity)(file)
    }
    fn identity_live(&self, identity: u32) -> bool {
        (self.live)(identity)
    }
    fn write_buffer(&mut self, address: u32, bytes: &[u8]) -> bool {
        unsafe { (self.buffer)(address, bytes.as_ptr(), bytes.len() as u32) }
    }
    fn write_iosb(&mut self, address: u32, status: u32, information: u32) {
        (self.iosb)(address, status, information);
    }
    fn signal_event(&mut self, event: u32) {
        (self.signal)(event);
    }
    fn event_signalled(&self, event: u32) -> bool {
        (self.signalled)(event)
    }
}
fn state() -> &'static Mutex<AsyncIo> {
    static STATE: OnceLock<Mutex<AsyncIo>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(AsyncIo::default()))
}
static PENDING: AtomicU32 = AtomicU32::new(0);
fn update<T>(f: impl FnOnce(&mut AsyncIo) -> T) -> T {
    let mut state = state().lock().unwrap();
    let result = f(&mut state);
    PENDING.store(state.stats().pending, Ordering::Relaxed);
    result
}
#[repr(C)]
pub struct CRead {
    pub(super) file: u32,
    pub(super) event: u32,
    pub(super) iosb: u32,
    pub(super) buffer: u32,
    pub(super) data: *mut u8,
    pub(super) length: u32,
    pub(super) requested: u32,
    pub(super) volume: u32,
}
extern "C" fn set_enabled(value: bool) {
    state().lock().unwrap().set_enabled(value);
}
extern "C" fn enabled() -> bool {
    state().lock().unwrap().enabled
}
extern "C" fn set_file_enabled(value: bool) {
    state().lock().unwrap().file_object_enabled = value;
}
extern "C" fn file_enabled() -> bool {
    state().lock().unwrap().file_object_enabled
}
extern "C" fn eligible(options: u32, event: u32) -> bool {
    state().lock().unwrap().eligible(options, event)
}
extern "C" fn apc(options: u32, routine: u32) -> bool {
    state().lock().unwrap().apc_refused(options, routine)
}
unsafe extern "C" fn submit(request: *mut CRead, host: *const Host) -> bool {
    let Some(request) = (unsafe { request.as_mut() }) else {
        return false;
    };
    if request.data.is_null() {
        return false;
    }
    let mut bytes = Vec::new();
    if bytes.try_reserve_exact(request.length as usize).is_err() {
        return false;
    }
    bytes.extend_from_slice(unsafe {
        std::slice::from_raw_parts(request.data, request.length as usize)
    });
    let read = Read {
        file: request.file,
        event: request.event,
        iosb: request.iosb,
        buffer: request.buffer,
        data: bytes,
        requested: request.requested,
        volume: if request.volume == 1 {
            Volume::Hdd
        } else {
            Volume::Disc
        },
    };
    if update(|s| s.submit(unsafe { &*host }, read)).is_err() {
        return false;
    }
    // Ownership transferred only on success. C allocated this buffer with malloc.
    unsafe { libc::free(request.data.cast()) };
    request.data = std::ptr::null_mut();
    true
}
unsafe extern "C" fn service(host: *const Host) -> u32 {
    update(|s| s.service(&mut unsafe { *host }))
}
unsafe extern "C" fn elapsed(wall: u64, host: *const Host) -> u32 {
    update(|s| s.service_elapsed(&mut unsafe { *host }, wall))
}
extern "C" fn idle() -> bool {
    PENDING.load(Ordering::Relaxed) == 0
}
unsafe extern "C" fn path(path: *const std::ffi::c_char) -> u32 {
    let bytes = if path.is_null() {
        &[][..]
    } else {
        unsafe { std::ffi::CStr::from_ptr(path) }.to_bytes()
    };
    if Volume::of_path(bytes) == Volume::Hdd {
        1
    } else {
        0
    }
}
extern "C" fn ticks(volume: u32, bytes: u32) -> u64 {
    (if volume == 1 {
        Volume::Hdd
    } else {
        Volume::Disc
    })
    .service_ticks(bytes)
}
unsafe extern "C" fn wait(
    event: bool,
    key: u32,
    finite: bool,
    timeout: u64,
    host: *const Host,
) -> u32 {
    let mut host = unsafe { *host };
    let key = if event { key } else { host.identity(key) };
    update(|s| s.wait(&mut host, event, key, finite.then_some(timeout))) as u32
}
extern "C" fn pending() -> u32 {
    state().lock().unwrap().stats().pending
}
unsafe extern "C" fn next(host: *const Host) -> bool {
    update(|s| s.complete_next(&mut unsafe { *host }))
}
extern "C" fn stats() -> Stats {
    state().lock().unwrap().stats()
}
extern "C" fn reset() {
    update(AsyncIo::reset);
}
#[repr(C)]
pub struct Backend {
    set_enabled: extern "C" fn(bool),
    enabled: extern "C" fn() -> bool,
    set_file_enabled: extern "C" fn(bool),
    file_enabled: extern "C" fn() -> bool,
    eligible: extern "C" fn(u32, u32) -> bool,
    apc: extern "C" fn(u32, u32) -> bool,
    submit: unsafe extern "C" fn(*mut CRead, *const Host) -> bool,
    service: unsafe extern "C" fn(*const Host) -> u32,
    elapsed: unsafe extern "C" fn(u64, *const Host) -> u32,
    idle: extern "C" fn() -> bool,
    path: unsafe extern "C" fn(*const std::ffi::c_char) -> u32,
    ticks: extern "C" fn(u32, u32) -> u64,
    wait: unsafe extern "C" fn(bool, u32, bool, u64, *const Host) -> u32,
    pending: extern "C" fn() -> u32,
    next: unsafe extern "C" fn(*const Host) -> bool,
    stats: extern "C" fn() -> Stats,
    reset: extern "C" fn(),
}
static BACKEND: Backend = Backend {
    set_enabled,
    enabled,
    set_file_enabled,
    file_enabled,
    eligible,
    apc,
    submit,
    service,
    elapsed,
    idle,
    path,
    ticks,
    wait,
    pending,
    next,
    stats,
    reset,
};
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_async_backend() -> *const Backend {
    &BACKEND
}
