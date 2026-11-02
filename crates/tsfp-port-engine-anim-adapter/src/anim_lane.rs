// SPDX-License-Identifier: GPL-3.0-or-later
//! T1905: guest memory adapters for the T1668 animation ports, so they can be selected as lane roots (`--rust-ports`) and traced with
//! `tools.rust_trace` (Tier 2). The pure ports live in `tsfp-engine-anim` (`layers.rs`, `queries.rs`); this file only marshals the cdecl
//! arguments, copies the animation body out of guest memory, runs the port, writes changed body words back and sets EAX / ST0. Same
//! `run` contract as `pose_leaf.rs` (ExtRegisters, GuestCall), registered through `lane_inv.rs`.
//!
//! Roots: 0x59F90 0x61570 0x5A070 0x5A2B0 0x59D50 0x59EC0 0x5A260 0x5A2F0 0x614B0 0x61450. Registers other than EAX / ST0 are left as
//! entered (the originals clobber ECX / EDX / XMM scratch, no caller relies on it; the trace compares memory only).
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};
use core::cell::RefCell;
use core::sync::atomic::{AtomicU32, Ordering};

use crate::{layers, queries};

use layers::{Body, ClipWorld, SpeedWorld};
use queries::Event;

const CLIP_TABLE: u32 = 0x007e_3954;
const POLICY_TABLE: u32 = 0x007d_e660;
const ID_SLOT_MAP: u32 = 0x007e_03c0;
const RESIDENT_TABLE: u32 = 0x007e_1be0;

fn arg<M: GuestMemory>(m: &M, regs: &ExtRegisters, index: u32) -> u32 {
    m.read32(regs.esp.wrapping_add(4).wrapping_add(4 * index))
}

/// Clip and policy tables read straight from guest memory.
struct GuestWorld<'a, M: GuestMemory, C: GuestCall> {
    m: &'a RefCell<&'a mut M>,
    guest: &'a RefCell<&'a mut C>,
    regs: &'a RefCell<ExtRegisters>,
}

impl<M: GuestMemory, C: GuestCall> ClipWorld for GuestWorld<'_, M, C> {
    fn duration(&self, slot: usize) -> Option<f32> {
        let m = self.m.borrow();
        let clip = m.read32(
            m.read32(CLIP_TABLE)
                .wrapping_add((slot as u32).wrapping_mul(4)),
        );
        (clip != 0).then(|| f32::from_bits(m.read32(clip.wrapping_add(0x18))))
    }
    fn policy(&self, slot: usize) -> (u32, u32) {
        let m = self.m.borrow();
        let base = POLICY_TABLE.wrapping_add((slot as u32).wrapping_mul(8));
        (m.read32(base), m.read32(base.wrapping_add(4)))
    }
}

impl<M: GuestMemory, C: GuestCall> SpeedWorld for GuestWorld<'_, M, C> {
    fn slow_motion_allowed(&self) -> bool {
        let m = self.m.borrow();
        m.read32(0x007d_e458) & 0x800 != 0 && m.read32(0x007d_e45c) & 0x0200_0000 == 0
    }
    fn player_type(&self) -> Option<u32> {
        // 0xC6140: the player object; the original calls it only when the flags above allow, same order here.
        let mut cr = *self.regs.borrow();
        self.guest.borrow_mut().call(0xc6140, &mut cr);
        (cr.eax != 0).then(|| self.m.borrow().read32(cr.eax.wrapping_add(0x218)))
    }
    fn slow_motion_scale(&self) -> f32 {
        f32::from_bits(self.m.borrow().read32(0x007d_e4a4))
    }
}

fn load_body<M: GuestMemory>(m: &M, address: u32) -> Body {
    Body(core::array::from_fn(|i| {
        m.read32(address.wrapping_add(4 * i as u32))
    }))
}

fn store_body<M: GuestMemory>(m: &mut M, address: u32, before: &Body, after: &Body) {
    for (i, (b, a)) in before.0.iter().zip(&after.0).enumerate() {
        if b != a {
            m.write32(address.wrapping_add(4 * i as u32), *a);
        }
    }
}

/// `0x56FC0(id)`: the resident event record of an animation id, 0 when out of range.
fn resident_record<M: GuestMemory>(m: &M, id: u32) -> u32 {
    if (id as i32) <= 0 || !(0x25..0xc03).contains(&(id as i32)) {
        return 0;
    }
    let slot = i16::from_le_bytes([
        m.read8(ID_SLOT_MAP.wrapping_add(id.wrapping_mul(2))),
        m.read8(ID_SLOT_MAP.wrapping_add(id.wrapping_mul(2).wrapping_add(1))),
    ]) as i32;
    m.read32(RESIDENT_TABLE.wrapping_add((slot as u32).wrapping_mul(4)))
}

const MAX_EVENTS: usize = 256;

/// Event list copied out of guest memory (fixed buffer, the crate has no allocator); the count is capped at `MAX_EVENTS`.
struct EventList {
    items: [Event; MAX_EVENTS],
    len: usize,
}

impl EventList {
    fn as_slice(&self) -> &[Event] {
        &self.items[..self.len]
    }
}

/// `0x56FC0(id)` by calling the lifted function (exact, no re-implementation of its table walk).
fn resident_via_guest<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    entry: &ExtRegisters,
    id: u32,
) -> u32 {
    let mut cr = *entry;
    cr.esp = entry.esp.wrapping_sub(8);
    m.write32(cr.esp, id);
    guest.call(0x56fc0, &mut cr);
    cr.eax
}

fn events<M: GuestMemory>(m: &M, record: u32) -> Option<EventList> {
    if record == 0 {
        return None;
    }
    let count = (i16::from_le_bytes([
        m.read8(record.wrapping_add(6)),
        m.read8(record.wrapping_add(7)),
    ])
    .max(0) as usize)
        .min(MAX_EVENTS);
    let mut list = EventList {
        items: [Event { frame: 0, flags: 0 }; MAX_EVENTS],
        len: count,
    };
    for i in 0..count {
        let at = record.wrapping_add(8 + 4 * i as u32);
        list.items[i] = Event {
            frame: u16::from_le_bytes([m.read8(at), m.read8(at + 1)]),
            flags: u16::from_le_bytes([m.read8(at + 2), m.read8(at + 3)]),
        };
    }
    Some(list)
}

/// The frame rate float `[0x4D1994]` as it is in guest memory now (not 60.0 at run time, see `queries::key_numbers_at`).
fn frames_per_second<M: GuestMemory>(m: &M) -> f32 {
    f32::from_bits(m.read32(0x004d_1994))
}

fn keys<M: GuestMemory>(m: &M, body: &Body) -> (i32, i32) {
    let layer = body.active_layer();
    queries::key_numbers_at(
        body.layer_time(layer),
        body.float(layer + 0xC),
        frames_per_second(m),
    )
}

/// T1918 shadow mode: every call runs the Rust port AND the retained lifted original on identical guest memory, compares the guest memory the
/// root may write plus EAX / ST0, aborts on the first difference and leaves the ORIGINAL's result in place. A route that completes is therefore a
/// live differential over every real call (not only over sampled regions). Turn off to run the port alone.
pub const SHADOW: bool = true;

/// The root whose retained original is running inside `shadow` (0 none): the selector declines it so `tsfp_lane_x_call` reaches the lifted body.
static SHADOWING: AtomicU32 = AtomicU32::new(0);
/// Last values the 0x61570 / 0x614B0 port looked at (cur, prev, count, first two events as frame << 16 | flags, mask), printed on a mismatch.
static DEBUG: [AtomicU32; 6] = [const { AtomicU32::new(0) }; 6];

/// T2031: a guest address the animation ports may dereference. The first 64 KB are never mapped and nothing the game owns lives at or
/// above the kernel window, so a null `this`, a small integer used as a pointer or a wild value is refused instead of faulting the host.
pub fn guest_ok(address: u32) -> bool {
    (0x1_0000..0x8000_0000).contains(&address)
}

/// Number of byte writes the shared journal can undo (a guarded call nested in another shares it, see [`Guarded`]).
const JOURNAL: usize = 262_144;
static mut JOURNAL_BUF: [(u32, u8); JOURNAL] = [(0, 0); JOURNAL];
static JOURNAL_TOP: AtomicU32 = AtomicU32::new(0);
static JOURNAL_NEST: AtomicU32 = AtomicU32::new(0);
/// Counts every refused guest access of any [`Guarded`] view; the shadow mode compares the count before and after the port (T2043).
static TAINT: AtomicU32 = AtomicU32::new(0);
/// Address and kind of the latest refused access (evidence for the `GUARD` lines, T2177).
static LAST_BAD: AtomicU32 = AtomicU32::new(0);
static LAST_BAD_WRITE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
/// T2177: guarded calls that hit an invalid access AFTER a callee already ran (no rollback possible, degraded), and callees skipped
/// because the view was already tainted. Both are counted and each event prints a `GUARD` line.
static DEGRADED: AtomicU32 = AtomicU32::new(0);
static SKIPPED_CALLS: AtomicU32 = AtomicU32::new(0);
static OVERFLOWS: AtomicU32 = AtomicU32::new(0);
static ROLLED_BACK: AtomicU32 = AtomicU32::new(0);
static ESP_NOTED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

/// Writes that did not fit the journal (they landed unjournalled).
pub fn journal_overflows() -> u32 {
    OVERFLOWS.load(Ordering::Relaxed)
}

/// `(degraded, skipped callees, rollbacks)` since start, for tests and the host report.
pub fn guard_counters() -> (u32, u32, u32) {
    (
        DEGRADED.load(Ordering::Relaxed),
        SKIPPED_CALLS.load(Ordering::Relaxed),
        ROLLED_BACK.load(Ordering::Relaxed),
    )
}

/// Guest-call wrapper handed to a guarded port (T2177). The retained original cannot be re-run safely once a callee has run (its guest
/// writes, allocations and counters are not journalled), so the wrapper records that a callee ran, and refuses to start a callee on a view
/// that already refused an access (the port is computing from zeros and the result will be discarded).
pub struct Calls<'a, C: GuestCall> {
    inner: &'a mut C,
    ran: bool,
    view_bad: &'a core::cell::Cell<bool>,
}

impl<C: GuestCall> GuestCall for Calls<'_, C> {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        if !shadowing_now() && self.view_bad.get() {
            SKIPPED_CALLS.fetch_add(1, Ordering::Relaxed);
            regs.eax = 0;
            return;
        }
        self.ran = true;
        self.inner.call(va, regs);
    }
}

/// Guest memory view that never faults: an unmapped read returns 0 and an unmapped write is dropped, both set `bad`. Every write
/// that did land is journalled so `rollback` can restore it, and the caller then runs the retained original instead (T2031).
/// The journal is one shared stack (a single guest thread runs the animation roots, the same assumption as `DEPTH`): a view
/// records where its entries start, an inner view's entries stay for the outer one and the stack empties when the outermost view ends (T2043).
pub struct Guarded<'a, M: GuestMemory> {
    inner: &'a mut M,
    own: core::cell::Cell<bool>,
    overflow: core::cell::Cell<bool>,
    shared: Option<&'a core::cell::Cell<bool>>,
    start: u32,
}

impl<'a, M: GuestMemory> Guarded<'a, M> {
    pub fn new(inner: &'a mut M) -> Self {
        JOURNAL_NEST.fetch_add(1, Ordering::Relaxed);
        Self {
            inner,
            own: core::cell::Cell::new(false),
            overflow: core::cell::Cell::new(false),
            shared: None,
            start: JOURNAL_TOP.load(Ordering::Relaxed),
        }
    }
    /// A view that also reports its refusals through `flag` (the guest-call wrapper reads it, T2177).
    fn with_flag(inner: &'a mut M, flag: &'a core::cell::Cell<bool>) -> Self {
        JOURNAL_NEST.fetch_add(1, Ordering::Relaxed);
        Self {
            inner,
            own: core::cell::Cell::new(false),
            overflow: core::cell::Cell::new(false),
            shared: Some(flag),
            start: JOURNAL_TOP.load(Ordering::Relaxed),
        }
    }
    pub fn is_bad(&self) -> bool {
        self.own.get()
    }
    /// True when a write could not be journalled (journal full): a rollback would be partial.
    pub fn is_overflowed(&self) -> bool {
        self.overflow.get()
    }
    fn refuse(&self, address: u32, write: bool) {
        if !self.own.get() {
            LAST_BAD.store(address, Ordering::Relaxed);
            LAST_BAD_WRITE.store(write, Ordering::Relaxed);
        }
        self.own.set(true);
        if let Some(flag) = self.shared {
            flag.set(true);
        }
        TAINT.fetch_add(1, Ordering::Relaxed);
    }
    pub fn rollback(&mut self) {
        let mut top = JOURNAL_TOP.load(Ordering::Relaxed);
        while top > self.start {
            top -= 1;
            let (address, old) = unsafe { (*core::ptr::addr_of!(JOURNAL_BUF))[top as usize] };
            self.inner.write8(address, old);
        }
        JOURNAL_TOP.store(top, Ordering::Relaxed);
    }
}

impl<M: GuestMemory> Drop for Guarded<'_, M> {
    fn drop(&mut self) {
        if JOURNAL_NEST.fetch_sub(1, Ordering::Relaxed) == 1 {
            JOURNAL_TOP.store(0, Ordering::Relaxed);
        }
    }
}

impl<M: GuestMemory> GuestMemory for Guarded<'_, M> {
    fn read8(&self, address: u32) -> u8 {
        if guest_ok(address) {
            self.inner.read8(address)
        } else {
            self.refuse(address, false);
            0
        }
    }
    fn read32(&self, address: u32) -> u32 {
        // Keep the bytewise path for unaligned or partially refused reads: it
        // preserves the refusal address and per-byte behavior at both bounds.
        if address & 3 == 0 && guest_ok(address) && guest_ok(address + 3) {
            self.inner.read32(address)
        } else {
            u32::from_le_bytes(core::array::from_fn(|i| {
                self.read8(address.wrapping_add(i as u32))
            }))
        }
    }
    fn write8(&mut self, address: u32, value: u8) {
        if !guest_ok(address) {
            self.refuse(address, true);
            return;
        }
        let top = JOURNAL_TOP.load(Ordering::Relaxed) as usize;
        if top < JOURNAL {
            unsafe {
                (*core::ptr::addr_of_mut!(JOURNAL_BUF))[top] = (address, self.inner.read8(address))
            };
            JOURNAL_TOP.store(top as u32 + 1, Ordering::Relaxed);
        } else {
            // T2177: the journal is full. The write still lands (refusing it, as T2031 did, corrupted the result of a root that
            // legitimately writes a large buffer, e.g. the text bank relocation of 0x3DD20: 16 KB of relocations plus the bank copy) and
            // the view only records that it can no longer undo everything, which matters only if an invalid access follows.
            self.overflow.set(true);
            OVERFLOWS.fetch_add(1, Ordering::Relaxed);
        }
        self.inner.write8(address, value);
    }
}

/// Runs `port` on a checked view of guest memory. If it touched an invalid address, its writes are undone and the retained lifted
/// original runs on the untouched state instead (the original is the reference for null / wild objects). Returns `port`'s result.
pub fn guarded<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    port: impl FnOnce(&mut Guarded<'_, M>, &mut Calls<'_, C>, &mut ExtRegisters) -> bool,
) -> bool {
    let entry = *regs;
    if !ESP_NOTED.swap(true, Ordering::Relaxed) {
        note("guard-esp", &[va, entry.esp]);
    }
    // T2061: the slerp inside the ports rounds like the guest's x87 precision control (24 bit in the live game)
    crate::blend::set_single_precision(regs.fpu.control as u16);
    let view_bad = core::cell::Cell::new(false);
    let mut view = Guarded::with_flag(&mut *m, &view_bad);
    let mut calls = Calls {
        inner: &mut *guest,
        ran: false,
        view_bad: &view_bad,
    };
    let handled = port(&mut view, &mut calls, regs);
    let callee_ran = calls.ran;
    if !view.is_bad() || shadowing_now() {
        // shadow mode already restored the footprint and ran the original after the port (`shadow_regions` skips its comparison for a refused access)
        return handled;
    }
    if callee_ran {
        // T2177: a callee already ran, so its side effects (guest writes, allocations, counters) cannot be undone and re-running the
        // original would repeat them. The invalid access was dropped (read as 0, write skipped): keep the port's result, count it.
        DEGRADED.fetch_add(1, Ordering::Relaxed);
        note(
            "guard-degraded",
            &[
                va,
                LAST_BAD.load(Ordering::Relaxed),
                LAST_BAD_WRITE.load(Ordering::Relaxed) as u32,
                entry.esp,
                view.is_overflowed() as u32,
            ],
        );
        return handled;
    }
    ROLLED_BACK.fetch_add(1, Ordering::Relaxed);
    note(
        "guard-rollback",
        &[
            va,
            LAST_BAD.load(Ordering::Relaxed),
            LAST_BAD_WRITE.load(Ordering::Relaxed) as u32,
            entry.esp,
            view.is_overflowed() as u32,
        ],
    );
    view.rollback();
    drop(view);
    let mut original = entry;
    original.esp = entry.esp.wrapping_add(4);
    let slot = m.read32(entry.esp);
    let outer = SHADOWING.swap(va, Ordering::Relaxed);
    guest.call(va, &mut original);
    SHADOWING.store(outer, Ordering::Relaxed);
    m.write32(entry.esp, slot);
    original.esp = entry.esp;
    *regs = original;
    true
}

/// T3818: runs the retained original of root `va` in place of the port (the same delegation the guard uses after a refused
/// access). For inputs a port does not model: it must never return having done nothing where the original does work.
pub fn run_original<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    let entry = *regs;
    let mut original = entry;
    original.esp = entry.esp.wrapping_add(4);
    let slot = m.read32(entry.esp);
    let outer = SHADOWING.swap(va, Ordering::Relaxed);
    guest.call(va, &mut original);
    SHADOWING.store(outer, Ordering::Relaxed);
    m.write32(entry.esp, slot);
    original.esp = entry.esp;
    *regs = original;
    true
}

pub fn declined(va: u32) -> bool {
    SHADOWING.load(Ordering::Relaxed) == va
}

/// Guest bytes a root may write: `(address, dwords)`.
fn footprint<M: GuestMemory>(va: u32, m: &M, regs: &ExtRegisters) -> (u32, usize) {
    let object = arg(m, regs, 0);
    match va {
        0x5a070 => (object, layers::BODY_DWORDS),
        0x5a2f0 => (m.read32(object.wrapping_add(4)), layers::BODY_DWORDS),
        0x5a260 => (object, 22),
        _ => (0, 0),
    }
}

fn snapshot<M: GuestMemory>(
    m: &M,
    (address, dwords): (u32, usize),
    out: &mut [u32; layers::BODY_DWORDS],
) {
    for (i, word) in out.iter_mut().take(dwords).enumerate() {
        *word = m.read32(address.wrapping_add(4 * i as u32));
    }
}

unsafe extern "C" {
    fn write(fd: i32, buffer: *const u8, count: usize) -> isize;
}

/// Writes `ANIM-SHADOW <tag> v0 v1 ...` (hex dwords) to stderr, then aborts: the evidence for a mismatch found in a live run.
fn mismatch(tag: &str, values: &[u32]) -> ! {
    emit(tag, values);
    panic!("anim lane shadow mismatch");
}
fn emit(tag: &str, values: &[u32]) {
    emit_prefixed("ANIM-SHADOW ", tag, values);
}
/// `GUARD <tag> v0 v1 ...` line for the guarded-view events (T2177).
fn note(tag: &str, values: &[u32]) {
    emit_prefixed("GUARD ", tag, values);
}
fn emit_prefixed(prefix: &str, tag: &str, values: &[u32]) {
    let mut line = [0u8; 400];
    let mut at = 0;
    for byte in prefix.as_bytes().iter().chain(tag.as_bytes()) {
        line[at] = *byte;
        at += 1;
    }
    for value in values.iter().take(16) {
        line[at] = b' ';
        at += 1;
        for shift in (0..8).rev() {
            line[at] = b"0123456789abcdef"[((value >> (4 * shift)) & 0xf) as usize];
            at += 1;
        }
    }
    line[at] = b'\n';
    unsafe { write(2, line.as_ptr(), at + 1) };
}

/// Float tolerance in units in the last place for the memory comparison of the matrix roots (T1986).
/// Roots whose float matrices are compared with 64 ulps (or 1e-6 absolute) instead of bit exactly. A function, not a flag, because the roots nest.
fn ulps_of(va: u32) -> u32 {
    if matches!(va, 0x60540 | 0x605d0 | 0x604d0 | 0x60b30) {
        64
    } else {
        0
    }
}

fn within_ulps(va: u32, a: u32, b: u32) -> bool {
    let limit = ulps_of(va);
    // same sign and finite: distance of the bit patterns is the distance in ulps
    if limit == 0 {
        return false;
    }
    // rounding noise around zero (a matrix cell that is 0 in one evaluation order and 1.8e-7 in the other) passes on an absolute bound, and uninitialised matrices of 1e15..1e18 differ by up to 1e-5 relative between the two evaluation orders
    let (x, y) = (f32::from_bits(a), f32::from_bits(b));
    (x - y).abs() <= 1e-6
        || (x - y).abs() <= 1e-4 * x.abs().max(y.abs())
        || ((a ^ b) >> 31 == 0 && a.abs_diff(b) <= limit)
}

/// Shadow mode (the retained original is run on the same memory and compared) is a measurement, off unless `TSFP_ANIM_SHADOW` is set.
/// T2177: inside the shadow comparison of an animation root (`shadow_regions` sets `DEPTH`), which restores the footprint and runs
/// the original itself. A guarded lane root outside it (every non-animation lane, also with `TSFP_ANIM_SHADOW` set globally) has no one
/// else to recover a refused access, so it keeps the guarded semantics.
fn shadowing_now() -> bool {
    shadow_enabled() && DEPTH.load(Ordering::Relaxed) > 0
}

pub fn shadow_enabled() -> bool {
    static STATE: AtomicU32 = AtomicU32::new(0); // 0 unknown, 1 off, 2 on
    match STATE.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => {
            let on = unsafe { !getenv(c"TSFP_ANIM_SHADOW".as_ptr()).is_null() };
            STATE.store(if on { 2 } else { 1 }, Ordering::Relaxed);
            on
        }
    }
}

static REACHED: [AtomicU32; 48] = [const { AtomicU32::new(0) }; 48];

/// Writes `ANIM-REACH <va> <footprint dwords>` to stderr the first time a root is shadowed (T1986 reach evidence).
fn note_reach(va: u32, total: usize) {
    // A root may first run before its object exists. Record its first nonempty
    // footprint separately; the initial zero-word reach is not coverage.
    let key = va | if total > 0 { 0x8000_0000 } else { 0 };
    for slot in &REACHED {
        let seen = slot.load(Ordering::Relaxed);
        if seen == key {
            return;
        }
        if seen == 0
            && slot
                .compare_exchange(0, key, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
        {
            let mut line = [0u8; 40];
            let mut at = 0;
            for byte in b"ANIM-REACH ".iter() {
                line[at] = *byte;
                at += 1;
            }
            for shift in (0..8).rev() {
                line[at] = b"0123456789abcdef"[((va >> (4 * shift)) & 0xf) as usize];
                at += 1;
            }
            line[at] = b' ';
            line[at + 1] = b'0' + (total.min(9999) / 1000 % 10) as u8;
            line[at + 2] = b'0' + (total.min(9999) / 100 % 10) as u8;
            line[at + 3] = b'0' + (total.min(9999) / 10 % 10) as u8;
            line[at + 4] = b'0' + (total.min(9999) % 10) as u8;
            line[at + 5] = b'\n';
            unsafe { write(2, line.as_ptr(), at + 6) };
            return;
        }
    }
}

unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}

/// `TSFP_ANIM_MUTATE=<hex va>`: the root whose port result is corrupted by one bit (0 when unset).
fn mutation_target() -> u32 {
    let text = unsafe { getenv(c"TSFP_ANIM_MUTATE".as_ptr()) };
    if text.is_null() {
        return 0;
    }
    let mut value = 0u32;
    let mut i = 0;
    loop {
        let c = unsafe { *text.add(i) } as u8;
        let digit = match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => break,
        };
        value = (value << 4) | u32::from(digit);
        i += 1;
    }
    value
}

fn same_float(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits()
        || (a.is_nan() && b.is_nan())
        || (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

/// Which result registers a shadowed root must reproduce.
#[derive(Clone, Copy)]
pub enum Check {
    /// void
    None,
    /// boolean in EAX (bit 0)
    Bool,
    /// f32 in ST0
    Float,
    /// f64 in ST0 (x87 extended precision tolerance)
    Double,
}

pub(crate) const MAX_REGIONS: usize = 6;
// T3818: room for the largest skeleton footprint (0x60B30: three regions of up to NODE_LIMIT matrices)
const SHADOW_WORDS: usize = 1 << 16;
/// 1 while a root is being shadowed (single guest thread runs the animation roots).
static DEPTH: AtomicU32 = AtomicU32::new(0);
static mut BEFORE: [u32; SHADOW_WORDS] = [0; SHADOW_WORDS];
static mut PORT_AFTER: [u32; SHADOW_WORDS] = [0; SHADOW_WORDS];
static mut ORIGINAL_AFTER: [u32; SHADOW_WORDS] = [0; SHADOW_WORDS];

/// Runs `port` and the retained original on identical memory over the guest `regions` (`(address, dwords)`), aborts on a difference and leaves the
/// ORIGINAL's result. Shared by the T1918 animation adapters (`anim_lane2.rs`).
pub fn shadow_regions<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    regions: &[(u32, usize)],
    check: Check,
    port: impl FnOnce(&mut M, &mut C, &mut ExtRegisters),
) {
    shadow_regions_diag(va, m, guest, regs, regions, check, port, |_, _| [0; 14]);
}

/// [`shadow_regions`] with `describe(memory, word index)` giving up to 14 extra words printed with a memory mismatch.
#[allow(clippy::too_many_arguments)]
pub fn shadow_regions_diag<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    regions: &[(u32, usize)],
    check: Check,
    port: impl FnOnce(&mut M, &mut C, &mut ExtRegisters),
    describe: impl Fn(&M, usize) -> [u32; 14],
) {
    if !shadow_enabled() {
        // default: the port alone is the implementation (T1986: shadow mode is a Tier 2 measurement, opt in with TSFP_ANIM_SHADOW=1)
        port(m, guest, regs);
        return;
    }
    let entry = *regs;
    if DEPTH.load(Ordering::Relaxed) > 0 {
        // A root called by another root's port or original: three footprint arrays per level exhaust the guest thread's host stack
        // (T1986: fault in `shadow_regions` with the whole of wave B selected), so only the outermost root is compared and the
        // nested one runs its retained original; its effect is part of the outer footprint.
        let mut original = entry;
        original.esp = entry.esp.wrapping_add(4);
        let slot = m.read32(entry.esp);
        let outer = SHADOWING.swap(va, Ordering::Relaxed);
        guest.call(va, &mut original);
        SHADOWING.store(outer, Ordering::Relaxed);
        m.write32(entry.esp, slot);
        original.esp = entry.esp;
        *regs = original;
        return;
    }
    DEPTH.store(1, Ordering::Relaxed);
    // a region at a null or low guest address is skipped (size 0): the host maps no page there and a read faults the process
    let mut guarded = [(0u32, 0usize); 8];
    let regions = if regions.len() <= guarded.len() {
        for (slot, &(address, dwords)) in guarded.iter_mut().zip(regions) {
            *slot = if address >= 0x1_0000 {
                (address, dwords)
            } else {
                (0, 0)
            };
        }
        &guarded[..regions.len()]
    } else {
        regions
    };
    let total: usize = regions.iter().map(|r| r.1).sum();
    if total > SHADOW_WORDS {
        panic!("anim lane shadow: footprint too large");
    }
    // statics, not locals: three 8 KB arrays per call exhausted the guest thread's host stack (T1986); DEPTH makes this non-reentrant
    let (before, port_after, original_after) = unsafe {
        (
            &mut *core::ptr::addr_of_mut!(BEFORE),
            &mut *core::ptr::addr_of_mut!(PORT_AFTER),
            &mut *core::ptr::addr_of_mut!(ORIGINAL_AFTER),
        )
    };
    let read = |m: &M, out: &mut [u32; SHADOW_WORDS]| {
        let mut at = 0;
        for &(address, dwords) in regions {
            for i in 0..dwords {
                out[at] = m.read32(address.wrapping_add(4 * i as u32));
                at += 1;
            }
        }
    };
    read(m, before);
    let taint = TAINT.load(Ordering::Relaxed);
    port(m, guest, regs);
    // T2043: a port that touched a refused address ran on invalid state, its result is not comparable; the original still runs below
    let refused = TAINT.load(Ordering::Relaxed) != taint;
    let port_regs = *regs;
    read(m, port_after);
    note_reach(va, total);
    if total > 0 && mutation_target() == va {
        port_after[0] ^= 0x3f80_0000; // T1986 mutation control: an exponent-sized corruption (a low bit would hide in the matrix tolerance) of the port's first footprint word must be reported below
    }
    let mut at = 0;
    for &(address, dwords) in regions {
        for i in 0..dwords {
            m.write32(address.wrapping_add(4 * i as u32), before[at]);
            at += 1;
        }
    }
    let mut original = entry;
    original.esp = entry.esp.wrapping_add(4);
    let slot = m.read32(entry.esp);
    SHADOWING.store(va, Ordering::Relaxed);
    guest.call(va, &mut original);
    SHADOWING.store(0, Ordering::Relaxed);
    m.write32(entry.esp, slot);
    original.esp = entry.esp;
    read(m, original_after);
    // T3835 (compare v9): a port that changed nothing where the original changed the footprint is
    // DIVERGED, even when the adapter-sized footprint would otherwise compare equal word by word.
    if !refused
        && no_work(
            &before[..total],
            &port_after[..total],
            &original_after[..total],
        )
    {
        mismatch("no_work", &[va, total as u32]);
    }
    for i in 0..if refused { 0 } else { total } {
        let (a, b) = (port_after[i], original_after[i]);
        if a != b
            && !(f32::from_bits(a).is_nan() && f32::from_bits(b).is_nan())
            && !within_ulps(va, a, b)
        {
            let extra = describe(m, i);
            let mut all = [0u32; 16];
            all[0] = i as u32;
            all[1] = a;
            all[2..16].copy_from_slice(&extra);
            all[15] = b; // the original's value (last word)
            mismatch("memory", &all);
        }
    }
    match if refused { Check::None } else { check } {
        Check::Bool => {
            if (port_regs.eax & 1) != (original.eax & 1) {
                mismatch(
                    "result",
                    &[
                        va,
                        port_regs.eax,
                        original.eax,
                        arg(m, &entry, 0),
                        arg(m, &entry, 1),
                        arg(m, &entry, 2),
                    ],
                );
            }
        }
        Check::Float => {
            let (a, b) = (port_regs.fpu.st(0) as f32, original.fpu.st(0) as f32);
            if !same_float(f64::from(a), f64::from(b)) {
                mismatch("float", &[va, a.to_bits(), b.to_bits()]);
            }
        }
        Check::Double => {
            if !same_float(port_regs.fpu.st(0), original.fpu.st(0)) {
                mismatch(
                    "double",
                    &[
                        va,
                        port_regs.fpu.st(0).to_bits() as u32,
                        original.fpu.st(0).to_bits() as u32,
                    ],
                );
            }
        }
        Check::None => {}
    }
    DEPTH.store(0, Ordering::Relaxed);
    *regs = original;
}

/// T3835: the port left every footprint word as it was, the original changed at least one.
pub(crate) fn no_work(before: &[u32], port_after: &[u32], original_after: &[u32]) -> bool {
    port_after == before && original_after != before
}

#[cfg(test)]
mod no_work_tests {
    use super::{Guarded, GuestMemory, no_work};
    use core::cell::Cell;

    struct Memory {
        words: Cell<u32>,
        bytes: Cell<u32>,
    }

    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.bytes.set(self.bytes.get() + 1);
            address as u8
        }
        fn read32(&self, address: u32) -> u32 {
            self.words.set(self.words.get() + 1);
            u32::from_le_bytes(core::array::from_fn(|i| {
                address.wrapping_add(i as u32) as u8
            }))
        }
        fn write8(&mut self, _: u32, _: u8) {}
    }

    #[test]
    fn guarded_word_reads_preserve_boundary_and_unaligned_behavior() {
        let mut mem = Memory {
            words: Cell::new(0),
            bytes: Cell::new(0),
        };
        let view = Guarded::new(&mut mem);
        assert_eq!(view.read32(0x10000), 0x03020100);
        assert_eq!(view.read32(0x10001), 0x04030201);
        assert_eq!(view.read32(0x7ffffffe), 0x0000fffe);
        assert!(view.is_bad());
        drop(view);
        assert_eq!(mem.words.get(), 1);
        assert_eq!(mem.bytes.get(), 6);
    }

    #[test]
    fn guarded_word_reads_at_mapped_region_end() {
        struct Mapped([u8; 16]);
        impl GuestMemory for Mapped {
            fn read8(&self, address: u32) -> u8 {
                self.0[(address - 0x10000) as usize]
            }
            fn read32(&self, address: u32) -> u32 {
                let offset = (address - 0x10000) as usize;
                u32::from_le_bytes(self.0[offset..offset + 4].try_into().unwrap())
            }
            fn write8(&mut self, _: u32, _: u8) {}
        }
        let mut mem = Mapped(core::array::from_fn(|i| i as u8));
        let view = Guarded::new(&mut mem);
        for offset in 8..=12 {
            assert_eq!(
                view.read32(0x10000 + offset),
                u32::from_le_bytes(core::array::from_fn(|i| offset as u8 + i as u8))
            );
        }
        assert!(!view.is_bad());
    }

    #[test]
    fn a_port_that_writes_nothing_where_the_original_wrote_is_no_work() {
        assert!(no_work(&[1, 2], &[1, 2], &[1, 3]));
    }

    #[test]
    fn a_port_that_worked_or_an_original_that_did_nothing_is_not_no_work() {
        assert!(!no_work(&[1, 2], &[1, 3], &[1, 3]));
        assert!(!no_work(&[1, 2], &[1, 2], &[1, 2]));
    }
}

fn shadow<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) {
    let foot = footprint(va, m, regs);
    let check = match va {
        0x59f90 | 0x61570 | 0x614b0 | 0x61450 => Check::Bool,
        0x5a2b0 | 0x59d50 => Check::Float,
        0x59ec0 => Check::Double,
        _ => Check::None,
    };
    let regions = [foot];
    shadow_regions(va, m, guest, regs, &regions, check, |m, guest, regs| {
        port(va, m, guest, regs);
    });
}

/// Dispatches a root; `false` when `va` is not one of the animation adapters.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !ROOTS.contains(&va) {
        return false;
    }
    guarded(va, m, guest, regs, |view, guest, regs| {
        run_inner(va, view, guest, regs)
    })
}

fn run_inner<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if SHADOW && shadow_enabled() {
        shadow(va, m, guest, regs);
    } else {
        port(va, m, guest, regs);
    }
    true
}

fn port<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    let object = arg(m, regs, 0);
    let (a1, a2) = (arg(m, regs, 1), arg(m, regs, 2));
    let entry = *regs;
    let memory = RefCell::new(&mut *m);
    let guest_cell = RefCell::new(&mut *guest);
    let regs_cell = RefCell::new(entry);
    let world = GuestWorld {
        m: &memory,
        guest: &guest_cell,
        regs: &regs_cell,
    };
    let body_address = |m: &M| m.read32(object.wrapping_add(4));
    match va {
        0x59f90 | 0x5a2b0 | 0x59d50 | 0x59ec0 | 0x5a2f0 | 0x61570 | 0x614b0 => {
            let address = body_address(&memory.borrow());
            let mut body = load_body(&**memory.borrow(), address);
            let before = body.clone();
            match va {
                0x59f90 => regs.eax = u32::from(layers::active_policy_hit(&body, &world, (a1, a2))),
                0x5a2b0 => regs.fpu.return_st0(layers::active_time(&body)),
                0x59d50 => regs
                    .fpu
                    .return_st0(layers::current_duration(&body, &world).unwrap_or(0.0)),
                0x59ec0 => regs
                    .fpu
                    .push(layers::normalised_position(&body, &world).unwrap_or(0.0)),
                0x5a2f0 => layers::set_active_time(&mut body, &world, f32::from_bits(a1)),
                _ => {
                    let layer = body.active_layer();
                    let (current, previous) = keys(&**memory.borrow(), &body);
                    let record = resident_via_guest(
                        *memory.borrow_mut(),
                        *guest_cell.borrow_mut(),
                        &entry,
                        body.layer_id(layer),
                    );
                    let list = events(&**memory.borrow(), record);
                    let first = list.as_ref().map_or([0, 0], |l| {
                        let s = l.as_slice();
                        let pack = |i: usize| {
                            s.get(i).map_or(0xdead_dead, |e| {
                                u32::from(e.frame) << 16 | u32::from(e.flags)
                            })
                        };
                        [pack(0), pack(1)]
                    });
                    let count = list.as_ref().map_or(0xffff, |l| l.len as u32);
                    for (i, v) in [
                        current as u32,
                        previous as u32,
                        count,
                        first[0],
                        first[1],
                        a1,
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        DEBUG[i].store(v, Ordering::Relaxed);
                    }
                    regs.eax = u32::from(if va == 0x61570 {
                        queries::event_in_window(
                            list.as_ref().map(EventList::as_slice),
                            a1,
                            current,
                            previous,
                        )
                    } else {
                        queries::passed_event(
                            list.as_ref().map(EventList::as_slice),
                            a1,
                            a2 as i32,
                            body.float(layer + 0x34),
                            current,
                            previous,
                        )
                    });
                }
            }
            if body != before {
                store_body(*memory.borrow_mut(), address, &before, &body);
            }
        }
        0x5a070 => {
            // (body, speed): the argument is the body itself
            let address = object;
            let mut body = load_body(&**memory.borrow(), address);
            let before = body.clone();
            let model = body.word(0);
            let model_type = (model != 0).then(|| memory.borrow().read32(model.wrapping_add(0x20)));
            layers::set_speed(&mut body, model_type, &world, f32::from_bits(a1));
            store_body(*memory.borrow_mut(), address, &before, &body);
        }
        0x5a260 => {
            // (layer, time): clamp rule on a bare layer state
            let mut body = {
                let mut words = [0u32; layers::BODY_DWORDS];
                for (i, word) in words.iter_mut().take(22).enumerate() {
                    *word = memory.borrow().read32(object.wrapping_add(4 * i as u32));
                }
                Body(words)
            };
            let before = body.clone();
            layers::set_layer_time(&mut body, 0, &world, f32::from_bits(a1));
            for (i, (b, a)) in before.0.iter().zip(&body.0).enumerate() {
                if b != a {
                    memory
                        .borrow_mut()
                        .write32(object.wrapping_add(4 * i as u32), *a);
                }
            }
        }
        0x61450 => {
            // (id, mask): any event flag overlaps the mask, no time window
            // the original returns 0 for an id of 0 or less without looking the animation up
            let list = if (object as i32) > 0 {
                let record = resident_via_guest(
                    *memory.borrow_mut(),
                    *guest_cell.borrow_mut(),
                    &entry,
                    object,
                );
                events(&**memory.borrow(), record)
            } else {
                None
            };
            regs.eax = u32::from(queries::event_flags_overlap(
                object as i32,
                list.as_ref().map(EventList::as_slice),
                a1,
            ));
        }
        _ => return false,
    }
    true
}

/// The animation lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 10] = [
    0x59f90, 0x61570, 0x5a070, 0x5a2b0, 0x59d50, 0x59ec0, 0x5a260, 0x5a2f0, 0x614b0, 0x61450,
];
