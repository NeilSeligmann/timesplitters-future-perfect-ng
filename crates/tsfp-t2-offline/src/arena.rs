// SPDX-License-Identifier: GPL-3.0-or-later
//! Fault-tracked guest arena (docs/t3738-capture-design.md section 3, "Guest arena").
//!
//! The arena covers the whole 32-bit guest window (`XBOX_PTR(a) = a + g_xbox_mem_offset`). Per
//! entry every page starts `PROT_NONE`. Pages holding read-set bytes are filled with the 0xCD
//! poison, the captured bytes are written in and the pages become read-only. During a run:
//! - a write fault records the page, makes it writable and continues, so the set of pages the
//!   code under test wrote is exact;
//! - a read fault on a page with no captured bytes poisons the page, makes it read-only and counts
//!   an `extra_read` (claude-1 amendment 2: a warning, the outputs decide).
//!
//! Signals: one process-wide SIGSEGV/SIGBUS/SIGFPE/SIGILL handler, installed while any arena run is
//! active (reference counted) and restored afterwards. Which arena a fault belongs to is
//! per-thread (`ACTIVE`), so concurrent test threads with their own arenas do not interfere. A
//! fault outside the active arena, or any other signal, escapes the guarded run through the
//! sigsetjmp guard in `c/t2o_glue.c` (the entry becomes `fault`); with no guard armed the previous
//! handler runs as before.
use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr::null_mut;
use std::sync::Mutex;

use crate::spool::Run;

pub const PAGE: usize = 4096;
pub const POISON: u8 = 0xCD;
/// The 32-bit guest window plus slack for a 16-byte access at the top.
pub const GUEST_WINDOW: usize = (1usize << 32) + 0x1_0000;

const UNTOUCHED: u8 = 0;
const CAPTURED: u8 = 1;
const POISONED: u8 = 2;
const WRITTEN: u8 = 3;

unsafe extern "C" {
    fn t2o_guarded(body: unsafe extern "C" fn(*mut c_void), context: *mut c_void) -> i32;
    fn t2o_escape(signal: i32, address: usize);
    fn t2o_last_fault_address() -> usize;
    fn t2o_last_fault_signal() -> i32;
}

/// Most pages one entry may touch (256 MiB); more is a harness fault, not a silent miss.
const TOUCHED_CAP: usize = 1 << 16;

/// What the signal handler may touch: plain words, the page-state table and the preallocated
/// list of touched pages (no allocation in the handler).
struct FaultState {
    base: usize,
    len: usize,
    states: *mut u8,
    touched: *mut u32,
    touched_len: usize,
    extra_reads: u32,
}

thread_local! {
    static ACTIVE: Cell<*mut FaultState> = const { Cell::new(null_mut()) };
}

impl FaultState {
    /// Resolve a SIGSEGV inside the arena. Returns false when the fault is not a tracked one.
    unsafe fn resolve(&mut self, address: usize, write: bool) -> bool {
        if address < self.base || address >= self.base + self.len {
            return false;
        }
        let page = (address - self.base) / PAGE;
        let start = (self.base + page * PAGE) as *mut c_void;
        let state = unsafe { &mut *self.states.add(page) };
        match *state {
            UNTOUCHED => {
                if self.touched_len == TOUCHED_CAP {
                    return false;
                }
                unsafe { self.touched.add(self.touched_len).write(page as u32) };
                self.touched_len += 1;
                if unsafe { libc::mprotect(start, PAGE, libc::PROT_READ | libc::PROT_WRITE) } != 0 {
                    return false;
                }
                unsafe { std::ptr::write_bytes(start.cast::<u8>(), POISON, PAGE) };
                if write {
                    *state = WRITTEN;
                } else {
                    self.extra_reads += 1;
                    *state = POISONED;
                    if unsafe { libc::mprotect(start, PAGE, libc::PROT_READ) } != 0 {
                        return false;
                    }
                }
                true
            }
            CAPTURED | POISONED => {
                if unsafe { libc::mprotect(start, PAGE, libc::PROT_READ | libc::PROT_WRITE) } != 0 {
                    return false;
                }
                *state = WRITTEN;
                true
            }
            _ => false,
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
unsafe fn fault_is_write(context: *mut c_void) -> bool {
    // The page-fault error code: bit 1 set for a write access.
    let context = unsafe { &*context.cast::<libc::ucontext_t>() };
    context.uc_mcontext.gregs[libc::REG_ERR as usize] & 2 != 0
}

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
unsafe fn fault_is_write(_context: *mut c_void) -> bool {
    // Without the error code a write to an untouched page is first seen as a read (one spurious
    // extra_read), then faults again on the read-only page and is recorded as a write.
    false
}

const SIGNALS: [i32; 4] = [libc::SIGSEGV, libc::SIGBUS, libc::SIGFPE, libc::SIGILL];

struct Installed {
    users: usize,
    previous: [Option<libc::sigaction>; 4],
}

static INSTALLED: Mutex<Installed> = Mutex::new(Installed {
    users: 0,
    previous: [None; 4],
});
/// Copy of `previous` the handler reads without locking (written only while `users` goes 0 -> 1,
/// before the handler can run).
struct PreviousActions(std::cell::UnsafeCell<[Option<libc::sigaction>; 4]>);
unsafe impl Sync for PreviousActions {}
static PREVIOUS: PreviousActions = PreviousActions(std::cell::UnsafeCell::new([None; 4]));

unsafe extern "C" fn on_signal(signal: i32, info: *mut libc::siginfo_t, context: *mut c_void) {
    let address = unsafe { (*info).si_addr() } as usize;
    if signal == libc::SIGSEGV {
        let state = ACTIVE.with(Cell::get);
        if !state.is_null() && unsafe { (*state).resolve(address, fault_is_write(context)) } {
            return;
        }
    }
    unsafe { t2o_escape(signal, address) };
    // No guard armed on this thread: behave as if this handler were not installed.
    let slot = SIGNALS.iter().position(|&s| s == signal).unwrap_or(0);
    let previous = unsafe { (*PREVIOUS.0.get())[slot] };
    match previous {
        Some(action)
            if action.sa_sigaction != libc::SIG_DFL && action.sa_sigaction != libc::SIG_IGN =>
        {
            if action.sa_flags & libc::SA_SIGINFO != 0 {
                let handler: extern "C" fn(i32, *mut libc::siginfo_t, *mut c_void) =
                    unsafe { std::mem::transmute(action.sa_sigaction) };
                handler(signal, info, context);
            } else {
                let handler: extern "C" fn(i32) =
                    unsafe { std::mem::transmute(action.sa_sigaction) };
                handler(signal);
            }
        }
        _ => unsafe {
            // Default action: reinstall it and return, the faulting instruction runs again.
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = libc::SIG_DFL;
            libc::sigaction(signal, &action, null_mut());
        },
    }
}

/// RAII registration of the handler (reference counted, previous handlers restored at zero).
struct HandlerGuard;

impl HandlerGuard {
    fn install() -> Self {
        let mut installed = INSTALLED
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if installed.users == 0 {
            for (slot, &signal) in SIGNALS.iter().enumerate() {
                unsafe {
                    let mut action: libc::sigaction = std::mem::zeroed();
                    action.sa_sigaction = on_signal as *const () as usize;
                    action.sa_flags = libc::SA_SIGINFO;
                    libc::sigemptyset(&mut action.sa_mask);
                    let mut old: libc::sigaction = std::mem::zeroed();
                    libc::sigaction(signal, &action, &mut old);
                    installed.previous[slot] = Some(old);
                    (*PREVIOUS.0.get())[slot] = Some(old);
                }
            }
        }
        installed.users += 1;
        HandlerGuard
    }
}

impl Drop for HandlerGuard {
    fn drop(&mut self) {
        let mut installed = INSTALLED
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        installed.users -= 1;
        if installed.users == 0 {
            for (slot, &signal) in SIGNALS.iter().enumerate() {
                if let Some(old) = installed.previous[slot].take() {
                    unsafe { libc::sigaction(signal, &old, null_mut()) };
                }
            }
        }
    }
}

/// True while the calling thread runs guest code inside `Arena::run` (for nested helpers).
pub fn in_run() -> bool {
    !ACTIVE.with(Cell::get).is_null()
}

pub struct Arena {
    base: *mut u8,
    len: usize,
    /// Page states; mutated through `FaultState::states` by the handler, so read with `state()`.
    states: Vec<u8>,
    /// Backing store of `FaultState::touched` (capacity `TOUCHED_CAP`, never reallocated).
    touched: Vec<u32>,
    fault: Box<std::cell::UnsafeCell<FaultState>>,
    /// Initial contents of the pages that hold captured bytes (others start as all poison).
    initial: HashMap<usize, Box<[u8; PAGE]>>,
}

// The arena is used by one thread at a time; the raw pointers only name its own mapping.
unsafe impl Send for Arena {}

impl Drop for Arena {
    fn drop(&mut self) {
        unsafe { libc::munmap(self.base.cast(), self.len) };
    }
}

/// How a guarded run ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunEnd {
    Returned,
    /// A signal outside the tracked arena or a trap: signal number and address.
    Fault {
        signal: i32,
        address: usize,
    },
    Panic(String),
}

impl Arena {
    /// Reserve a fresh `PROT_NONE` window of `len` bytes (no memory committed).
    pub fn reserve(len: usize) -> Result<Self, String> {
        let len = len.div_ceil(PAGE) * PAGE;
        let base = unsafe {
            libc::mmap(
                null_mut(),
                len,
                libc::PROT_NONE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NORESERVE,
                -1,
                0,
            )
        };
        if base == libc::MAP_FAILED {
            return Err(format!("arena mmap of {len:#x} bytes failed"));
        }
        let mut states = vec![UNTOUCHED; len / PAGE];
        let mut touched = vec![0u32; TOUCHED_CAP];
        let fault = Box::new(std::cell::UnsafeCell::new(FaultState {
            base: base as usize,
            len,
            states: states.as_mut_ptr(),
            touched: touched.as_mut_ptr(),
            touched_len: 0,
            extra_reads: 0,
        }));
        Ok(Self {
            base: base.cast(),
            len,
            states,
            touched,
            fault,
            initial: HashMap::new(),
        })
    }

    fn state(&self, page: usize) -> u8 {
        unsafe { self.states.as_ptr().add(page).read_volatile() }
    }

    fn set_state(&mut self, page: usize, value: u8) {
        unsafe { self.states.as_mut_ptr().add(page).write_volatile(value) };
    }

    /// Pages touched this entry, in first-touch order.
    fn touched(&self) -> &[u32] {
        let len = unsafe { (*self.fault.get()).touched_len };
        &self.touched[..len]
    }

    fn touch(&mut self, page: usize) -> Result<(), String> {
        let fault = self.fault.get_mut();
        if fault.touched_len == TOUCHED_CAP {
            return Err(format!("entry touches more than {TOUCHED_CAP} pages"));
        }
        self.touched[fault.touched_len] = page as u32;
        fault.touched_len += 1;
        Ok(())
    }

    /// The host pointer of guest address 0 (the value for `g_xbox_mem_offset`).
    pub fn base(&self) -> usize {
        self.base as usize
    }

    fn protect(&self, first: usize, pages: usize, prot: i32) -> Result<(), String> {
        let start = unsafe { self.base.add(first * PAGE) };
        if unsafe { libc::mprotect(start.cast(), pages * PAGE, prot) } != 0 {
            return Err(format!("mprotect page {first:#x} failed"));
        }
        Ok(())
    }

    /// Every touched page back to `PROT_NONE`.
    fn reset(&mut self) -> Result<(), String> {
        let mut pages = self.touched().to_vec();
        pages.sort_unstable();
        let mut at = 0;
        while at < pages.len() {
            let first = pages[at] as usize;
            let mut end = first + 1;
            at += 1;
            while at < pages.len() && pages[at] as usize == end {
                end += 1;
                at += 1;
            }
            for page in first..end {
                self.set_state(page, UNTOUCHED);
            }
            self.protect(first, end - first, libc::PROT_NONE)?;
        }
        self.initial.clear();
        let fault = self.fault.get_mut();
        fault.touched_len = 0;
        fault.extra_reads = 0;
        Ok(())
    }

    /// Set the arena up for one entry: poison and fill the read-set pages, make them read-only.
    pub fn prepare(&mut self, read_runs: &[Run]) -> Result<(), String> {
        self.reset()?;
        for run in read_runs {
            let start = run.addr as usize;
            if run.data.is_empty() {
                continue;
            }
            if start + run.data.len() > self.len {
                return Err(format!(
                    "read run {:#010x} lies outside the arena",
                    run.addr
                ));
            }
            for page in start / PAGE..=(start + run.data.len() - 1) / PAGE {
                if self.state(page) == UNTOUCHED {
                    self.touch(page)?;
                    self.protect(page, 1, libc::PROT_READ | libc::PROT_WRITE)?;
                    unsafe { std::ptr::write_bytes(self.base.add(page * PAGE), POISON, PAGE) };
                    self.set_state(page, CAPTURED);
                }
            }
        }
        // A later run wins an overlap here, as the capture recorded the first read of each byte
        // and runs never overlap in a well-formed record.
        for run in read_runs {
            let target = unsafe { self.base.add(run.addr as usize) };
            unsafe { std::ptr::copy_nonoverlapping(run.data.as_ptr(), target, run.data.len()) };
        }
        for at in 0..self.touched().len() {
            let page = self.touched()[at] as usize;
            if self.state(page) == CAPTURED {
                let mut copy = Box::new([0u8; PAGE]);
                let source =
                    unsafe { std::slice::from_raw_parts(self.base.add(page * PAGE), PAGE) };
                copy.copy_from_slice(source);
                self.initial.insert(page, copy);
                self.protect(page, 1, libc::PROT_READ)?;
            }
        }
        Ok(())
    }

    /// Run guest code against the prepared arena under the fault handler and the escape guard.
    pub fn run(&self, body: &mut dyn FnMut()) -> RunEnd {
        struct Call<'a> {
            body: &'a mut dyn FnMut(),
            panic: Option<String>,
        }
        unsafe extern "C" fn trampoline(context: *mut c_void) {
            let call = unsafe { &mut *context.cast::<Call>() };
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| (call.body)())) {
                let text = payload
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| payload.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "panic".to_string());
                call.panic = Some(text);
            }
        }
        let _handler = HandlerGuard::install();
        let mut call = Call { body, panic: None };
        let outer = ACTIVE.with(|active| active.replace(self.fault.get()));
        let escaped = unsafe { t2o_guarded(trampoline, (&raw mut call).cast()) } != 0;
        ACTIVE.with(|active| active.set(outer));
        if escaped {
            return RunEnd::Fault {
                signal: unsafe { t2o_last_fault_signal() },
                address: unsafe { t2o_last_fault_address() },
            };
        }
        match call.panic {
            Some(text) => RunEnd::Panic(text),
            None => RunEnd::Returned,
        }
    }

    /// The guest address of a host address inside the arena.
    pub fn guest_of(&self, host: usize) -> Option<u32> {
        let offset = host.checked_sub(self.base as usize)?;
        (offset < self.len).then_some(offset as u32)
    }

    /// Pages written during the last run, as page indexes (guest address / PAGE).
    pub fn written_pages(&self) -> Vec<usize> {
        let mut pages: Vec<usize> = self
            .touched()
            .iter()
            .map(|&page| page as usize)
            .filter(|&page| self.state(page) == WRITTEN)
            .collect();
        pages.sort_unstable();
        pages
    }

    pub fn extra_reads(&self) -> u32 {
        unsafe { (*self.fault.get()).extra_reads }
    }

    /// The current byte at a guest address, None for a page that was never mapped this entry.
    pub fn byte(&self, addr: u64) -> Option<u8> {
        let addr = addr as usize;
        if addr >= self.len || self.state(addr / PAGE) == UNTOUCHED {
            return None;
        }
        Some(unsafe { self.base.add(addr).read_volatile() })
    }

    /// The byte the arena held at a guest address before the run.
    pub fn initial_byte(&self, addr: u64) -> u8 {
        let addr = addr as usize;
        self.initial
            .get(&(addr / PAGE))
            .map_or(POISON, |page| page[addr % PAGE])
    }

    /// Guest u32 at `addr` read during a run (may fault into the tracker, which is fine).
    ///
    /// # Safety
    /// Call only inside `run` (the page may be `PROT_NONE`) or on a readable page.
    pub unsafe fn read32(&self, addr: u32) -> u32 {
        unsafe { self.base.add(addr as usize).cast::<u32>().read_unaligned() }
    }

    /// # Safety
    /// Call only inside `run`: the write faults into the tracker.
    pub unsafe fn write_bytes(&self, addr: u32, data: &[u8]) {
        for (offset, byte) in data.iter().enumerate() {
            let target = unsafe { self.base.add(addr as usize + offset) };
            unsafe { target.write_volatile(*byte) };
        }
    }
}

/// `GuestMemory` over the arena window (what a port's `MappedGuest` is in the host): plain
/// volatile accesses at `base + address`, so every access goes through the fault tracker.
#[derive(Clone, Copy, Debug)]
pub struct GuestView {
    base: usize,
}

impl GuestView {
    /// # Safety
    /// `base` is the base of a live arena, used only inside `Arena::run`.
    pub unsafe fn new(base: usize) -> Self {
        Self { base }
    }
}

impl tsfp_port_runtime::memory::GuestMemory for GuestView {
    fn read8(&self, address: u32) -> u8 {
        unsafe { ((self.base + address as usize) as *const u8).read_volatile() }
    }
    fn write8(&mut self, address: u32, value: u8) {
        unsafe { ((self.base + address as usize) as *mut u8).write_volatile(value) };
    }
    fn read32(&self, address: u32) -> u32 {
        unsafe { ((self.base + address as usize) as *const u32).read_unaligned() }
    }
    fn write32(&mut self, address: u32, value: u32) {
        unsafe { ((self.base + address as usize) as *mut u32).write_unaligned(value) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arena() -> Arena {
        Arena::reserve(GUEST_WINDOW).expect("reserve")
    }

    #[test]
    fn prepare_poisons_fills_and_protects_the_read_set() {
        let mut arena = arena();
        let runs = [Run {
            addr: 0x1_0ffe,
            data: vec![1, 2, 3, 4],
        }];
        arena.prepare(&runs).unwrap();
        assert_eq!(arena.byte(0x1_0ffe), Some(1));
        assert_eq!(arena.byte(0x1_1001), Some(4));
        assert_eq!(arena.byte(0x1_0000), Some(POISON));
        assert_eq!(arena.byte(0x1_2000), None);
        assert_eq!(arena.initial_byte(0x1_0fff), 2);
        assert_eq!(arena.initial_byte(0x5_0000), POISON);
        assert!(arena.written_pages().is_empty());
    }

    #[test]
    fn writes_are_tracked_per_page_and_reads_of_uncaptured_pages_counted() {
        let mut arena = arena();
        arena
            .prepare(&[Run {
                addr: 0x2000,
                data: vec![7; 8],
            }])
            .unwrap();
        let base = arena.base();
        let end = arena.run(&mut || unsafe {
            let captured = (base + 0x2000) as *mut u32;
            let value = captured.read_volatile();
            captured.add(1).write_volatile(value + 1); // captured page: write fault
            ((base + 0x9_0000) as *mut u8).write_volatile(5); // untouched page: write fault
            let _ = ((base + 0x7_0000) as *const u8).read_volatile(); // untouched: extra read
        });
        assert_eq!(end, RunEnd::Returned);
        assert_eq!(arena.written_pages(), vec![0x2, 0x90]);
        assert_eq!(arena.extra_reads(), 1);
        assert_eq!(arena.byte(0x2004), Some(8));
        assert_eq!(arena.byte(0x9_0000), Some(5));
        assert_eq!(arena.byte(0x9_0001), Some(POISON));
        assert_eq!(arena.byte(0x7_0000), Some(POISON));
        // The next entry starts from scratch.
        arena.prepare(&[]).unwrap();
        assert!(arena.written_pages().is_empty());
        assert_eq!(arena.byte(0x9_0000), None);
        assert_eq!(arena.extra_reads(), 0);
    }

    #[test]
    fn a_fault_outside_the_arena_and_a_panic_end_the_run_not_the_process() {
        let mut arena = arena();
        arena.prepare(&[]).unwrap();
        let end = arena.run(&mut || unsafe {
            (16usize as *mut u32).write_volatile(1);
        });
        assert_eq!(
            end,
            RunEnd::Fault {
                signal: libc::SIGSEGV,
                address: 16
            }
        );
        let end = arena.run(&mut || panic!("port panicked"));
        assert_eq!(end, RunEnd::Panic("port panicked".into()));
        // The handler is gone again once no run is active on any thread of this test.
        let end = arena.run(&mut || {});
        assert_eq!(end, RunEnd::Returned);
    }
}
