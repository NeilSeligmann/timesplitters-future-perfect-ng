// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: wraps=tsfp-ports/c2r/t2123_manual
// portreg: wraps=tsfp-ports/c2r/t2049_exec_003
// portreg: wraps=tsfp-ports/c2r/engine_anim
//! T1815: entry points for call-bearing roots over the extended `ExtRegisters` ABI.
//! A root's own logic runs in Rust. Every callee runs as the retained lifted original
//! through the host's `GuestCall` trampoline (docs/t1815-ext-bridge.md).
use crate::c2r::t2049_exec_003::game_frame_update_and_render_all_players as frame_update_all;
use crate::{
    abi::{ExtRegisters, GuestCall, call_cdecl},
    combat::{
        CombatHooks, DamageHooks, DamageResolveHooks, DeathHooks, apply_damage, coordinate_damage,
        handle_death, resolve_damage,
    },
    memory::{GuestMemory, MappedGuest},
};
use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Host trampoline: loads `regs` into the guest register globals, pushes a return slot,
/// runs the lifted function for `va`, checks it returned (`esp == entry + 4`) and stores
/// every register back. Implemented in C (src/host/rust_ext_bridge.c).
pub type CallFn = unsafe extern "C" fn(u32, *mut ExtRegisters);
pub struct CGuest(pub CallFn);
impl GuestCall for CGuest {
    #[cfg(tsfp_mmx_harness)]
    fn mmx_read(&self, index: usize) -> u64 {
        unsafe extern "C" {
            fn c2r_mmx_read(index: usize) -> u64;
        }
        unsafe { c2r_mmx_read(index) }
    }
    #[cfg(tsfp_mmx_harness)]
    fn mmx_write(&mut self, index: usize, value: u64) {
        unsafe extern "C" {
            fn c2r_mmx_write(index: usize, value: u64);
        }
        unsafe { c2r_mmx_write(index, value) }
    }
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        unsafe { (self.0)(va, regs) }
    }
    fn frame_wait_preflight(&mut self, getter: u32, return_address: u32) {
        #[cfg(not(test))]
        {
            unsafe extern "C" {
                fn host_rust_frame_wait_preflight(getter: u32, return_address: u32);
            }
            // SAFETY: called by the host-backed guest context on the guest owner thread.
            unsafe { host_rust_frame_wait_preflight(getter, return_address) }
        }
    }
}

/// `CombatHooks` over a `GuestCall`: EAX/ESI are the implicit register inputs of the
/// original's register-convention helpers, `args` are pushed right to left (cdecl).
pub struct GuestHooks<'a, C: GuestCall> {
    pub guest: &'a mut C,
    pub regs: ExtRegisters,
}
impl<M: GuestMemory, C: GuestCall> CombatHooks<M> for GuestHooks<'_, C> {
    fn call(&mut self, memory: &mut M, target: u32, eax: u32, esi: u32, args: &[u32]) -> u32 {
        self.regs.eax = eax;
        self.regs.esi = esi;
        call_cdecl(self.guest, memory, &mut self.regs, target, args);
        self.regs.eax
    }
}

impl<M: GuestMemory, C: GuestCall> DeathHooks<M> for GuestHooks<'_, C> {
    fn esp(&self) -> u32 {
        self.regs.esp
    }
    fn set_esp(&mut self, value: u32) {
        self.regs.esp = value;
    }
}

fn push<M: GuestMemory>(m: &mut M, regs: &mut ExtRegisters, value: u32) {
    regs.esp = regs.esp.wrapping_sub(4);
    m.write32(regs.esp, value);
}

/// 0xBB490. EAX = hit record at entry, cdecl RET owned by the caller of this function.
/// Mirrors the original's frame (EBX, EBP, ESI, EDI saved on the guest stack) so that
/// callee-visible stack addresses match, restores the four callee-saved registers and
/// ESP. EAX/ECX/EDX/XMM/x87 are volatile for this void root: not claimed.
pub fn bb490<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let record = regs.eax;
    let mut frame = *regs;
    for value in [entry.ebx, entry.ebp, entry.esi, entry.edi] {
        push(m, &mut frame, value);
    }
    let mut hooks = GuestHooks { guest, regs: frame };
    coordinate_damage(m, &mut hooks, record);
    let after = hooks.regs;
    *regs = ExtRegisters {
        ebx: entry.ebx,
        ebp: entry.ebp,
        esi: entry.esi,
        edi: entry.edi,
        esp: entry.esp,
        ..after
    };
}

/// 0xBABA0 death handler. cdecl, record is the first stack argument. Builds the original frame
/// (`sub esp,0x104`, push EBP, push ESI; the body pushes EDI) so callee-visible stack addresses
/// and words match, and restores EBP, ESI, EDI, ESP. EAX/ECX/EDX/XMM exit residue not claimed.
pub fn baba0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let record = m.read32(entry.esp.wrapping_add(4));
    let mut frame = entry;
    frame.esp = frame.esp.wrapping_sub(0x104);
    for value in [entry.ebp, entry.esi] {
        push(m, &mut frame, value);
    }
    frame.esi = record;
    let mut hooks = GuestHooks { guest, regs: frame };
    handle_death(m, &mut hooks, record, entry.edi);
    *regs = ExtRegisters {
        ebp: entry.ebp,
        esi: entry.esi,
        edi: entry.edi,
        esp: entry.esp,
        ..hooks.regs
    };
}

/// `DamageHooks` over a `GuestCall`: B9510(record), 124C70(character) -> EAX, 124D00(hit, record).
pub struct GuestDamageHooks<'a, C: GuestCall> {
    pub guest: &'a mut C,
    pub regs: ExtRegisters,
}
impl<M: GuestMemory, C: GuestCall> DamageHooks<M> for GuestDamageHooks<'_, C> {
    fn pending_effect(&mut self, memory: &mut M, record: u32) {
        call_cdecl(self.guest, memory, &mut self.regs, 0xb9510, &[record]);
    }
    fn hit_record_enabled(&mut self, memory: &mut M, character: u32) -> bool {
        call_cdecl(self.guest, memory, &mut self.regs, 0x124c70, &[character]);
        self.regs.eax != 0
    }
    fn record_hit(&mut self, memory: &mut M, hit_state: u32, record: u32) {
        call_cdecl(
            self.guest,
            memory,
            &mut self.regs,
            0x124d00,
            &[hit_state, record],
        );
    }
}

/// 0xBA230 damage pipeline. cdecl, the hit record is the first stack argument. The original
/// saves ESI and EDI on the guest stack, which this mirrors; both are restored.
pub fn ba230<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let record = m.read32(entry.esp.wrapping_add(4));
    let mut frame = entry;
    for value in [entry.esi, entry.edi] {
        push(m, &mut frame, value);
    }
    let mut hooks = GuestDamageHooks { guest, regs: frame };
    apply_damage(m, &mut hooks, record);
    *regs = ExtRegisters {
        esi: entry.esi,
        edi: entry.edi,
        esp: entry.esp,
        ..hooks.regs
    };
}

/// `DamageResolveHooks` over a `GuestCall`. B9790/B9960 return the amount in ST0, consumed
/// exactly like the original's `FSTP dword [record+0x20]`: round to f32, then pop.
pub struct GuestResolveHooks<'a, C: GuestCall> {
    pub guest: &'a mut C,
    pub regs: ExtRegisters,
}
impl<C: GuestCall> GuestResolveHooks<'_, C> {
    fn amount<M: GuestMemory>(&mut self, memory: &mut M, va: u32, args: &[u32]) -> u32 {
        call_cdecl(self.guest, memory, &mut self.regs, va, args);
        let bits = (self.regs.fpu.st(0) as f32).to_bits();
        self.regs.fpu.pop();
        bits
    }
}
impl<M: GuestMemory, C: GuestCall> DamageResolveHooks<M> for GuestResolveHooks<'_, C> {
    fn character_amount(
        &mut self,
        memory: &mut M,
        character: u32,
        kind: u32,
        hit: u32,
        scale: u32,
        record: u32,
    ) -> u32 {
        self.amount(memory, 0xb9790, &[character, hit, kind, scale, record])
    }
    fn generic_amount(&mut self, memory: &mut M, kind: u32, hit: u32, scale: u32) -> u32 {
        self.amount(memory, 0xb9960, &[hit, kind, scale])
    }
    fn finish(&mut self, memory: &mut M, record: u32) {
        call_cdecl(self.guest, memory, &mut self.regs, 0xbb6c0, &[record]);
    }
}

/// 0xBD0F0 hit-amount resolver. cdecl, record is the first stack argument, ESI is saved on
/// the guest stack. The x87 stack is balanced (callee pushes ST0, FSTP pops).
pub fn bd0f0<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let record = m.read32(entry.esp.wrapping_add(4));
    let mut frame = entry;
    push(m, &mut frame, entry.esi);
    let mut hooks = GuestResolveHooks { guest, regs: frame };
    resolve_damage(m, &mut hooks, record);
    *regs = ExtRegisters {
        esi: entry.esi,
        esp: entry.esp,
        ..hooks.regs
    };
}

pub const EXT_ROOTS: [u32; 8] = [
    0xbb490, 0xba230, 0xbd0f0, 0xbaba0, 0x1538c0, 0x3d370, 0x3c440, 0x1c8540,
];
static ENABLED: AtomicU32 = AtomicU32::new(0);
static RUST_CALLS: [AtomicU64; EXT_ROOTS.len()] = [const { AtomicU64::new(0) }; EXT_ROOTS.len()];
static C_CALLS: [AtomicU64; EXT_ROOTS.len()] = [const { AtomicU64::new(0) }; EXT_ROOTS.len()];

/// Enable the frame body, timing root, and outer loop when the native frame-loop
/// switch is selected. Other explicitly selected extended roots keep their bits.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rust_enable_frame_timing_port() {
    ENABLED.fetch_or((1 << 4) | (1 << 5) | (1 << 6), Ordering::AcqRel);
}

/// `off` or a comma list of ext roots (hex). `all` is deliberately not accepted: these
/// roots are opt-in until an original-vs-Rust proof admits them.
pub fn parse_ext_selection(spec: &[u8]) -> Option<u32> {
    if spec == b"off" {
        return Some(0);
    }
    let mut mask = 0;
    for token in spec.split(|&b| b == b',') {
        let digits = token
            .strip_prefix(b"0x")
            .or_else(|| token.strip_prefix(b"0X"))
            .unwrap_or(token);
        if digits.is_empty() || digits.len() > 8 {
            return None;
        }
        let mut va = 0u32;
        for &b in digits {
            let digit = match b {
                b'0'..=b'9' => b - b'0',
                b'a'..=b'f' => b - b'a' + 10,
                b'A'..=b'F' => b - b'A' + 10,
                _ => return None,
            };
            va = (va << 4) | u32::from(digit);
        }
        let bit = 1 << EXT_ROOTS.iter().position(|&root| root == va)?;
        if mask & bit != 0 {
            return None;
        }
        mask |= bit;
    }
    Some(mask)
}

/// # Safety
/// `spec` points to `len` initialized bytes. Call only before guest threads start.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rust_configure_ext(spec: *const u8, len: usize) -> bool {
    let Some(mask) = parse_ext_selection(unsafe { core::slice::from_raw_parts(spec, len) }) else {
        return false;
    };
    for count in RUST_CALLS.iter().chain(C_CALLS.iter()) {
        count.store(0, Ordering::Relaxed);
    }
    ENABLED.store(mask, Ordering::Release);
    true
}

/// Returns true when the Rust body ran (registers updated), false when the caller must run
/// the retained lifted original.
///
/// # Safety
/// `regs` is a live `ExtRegisters`, `offset` maps the guest window, `call` is the trampoline.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rust_try_ext(
    va: u32,
    regs: *mut ExtRegisters,
    offset: usize,
    call: CallFn,
) -> bool {
    let Some(index) = EXT_ROOTS.iter().position(|&root| root == va) else {
        return false;
    };
    if ENABLED.load(Ordering::Acquire) & (1 << index) == 0 {
        C_CALLS[index].fetch_add(1, Ordering::Relaxed);
        return false;
    }
    RUST_CALLS[index].fetch_add(1, Ordering::Relaxed);
    let mut memory = unsafe { MappedGuest::new(offset) };
    let mut guest = CGuest(call);
    let regs = unsafe { &mut *regs };
    match va {
        0xbb490 => bb490(&mut memory, &mut guest, regs),
        0xba230 => ba230(&mut memory, &mut guest, regs),
        0xbd0f0 => bd0f0(&mut memory, &mut guest, regs),
        0xbaba0 => baba0(&mut memory, &mut guest, regs),
        0x1538c0 => regs.run_tail_jump_root(|regs| {
            crate::c2r::t2123_manual::game_frame_timing_update(&mut memory, &mut guest, regs)
        }),
        0x3d370 => frame_loop(&mut memory, &mut guest, regs),
        // TEMPORARY-SHIM owned by T2409: route the full translated frame body
        // through MappedGuest so its measured event words use Rust storage. Replace
        // this mechanical root with readable Rust before the subsystem is adopted.
        0x3c440 => regs.run_tail_jump_root(|r| frame_update_all(&mut memory, &mut guest, r)),
        // T3402: opt-in extended root for the native dependency-prefix seam. The
        // C2R body retains the measured selector tail while the prefix is modelled
        // by the typed animation service when TSFP_ANIM_RESOURCE_PREFIX=rust.
        0x1c8540 => crate::c2r::engine_anim::game_anim_pak_list_open(&mut memory, &mut guest, regs),
        _ => return false,
    }
    true
}

/// Rust-owned outer frame loop from the measured `0x3D370` contract. Its simulation body,
/// timing sample, vblank update, visibility work, and presentation remain explicit lifted
/// callees at this boundary; the frame count itself routes through `MappedGuest`'s typed
/// `FrameClockState` owner. Capture uses the separate adapter; REA specifies no queue growth.
pub fn frame_loop<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    const QUIT: u32 = 0x004c_0a2c;
    const DRAW_COUNTERS: u32 = 0x006b_7ab0;
    const OUTER_FRAMES: u32 = 0x006b_7af0;
    const FRAME_PHASE: u32 = 0x006b_7ad4;
    const SAMPLED_FRAME: u32 = 0x0074_97b0;

    let saved = *regs;
    let mut frame = saved;
    let mut delayed_stamp = u32::MAX;
    let mut previous_stamp = u32::MAX;
    call_cdecl(guest, m, &mut frame, 0x0003_89b0, &[]);

    while m.read32(QUIT) == 0 {
        call_cdecl(guest, m, &mut frame, 0x0003_89b0, &[]);
        for index in 0..7 {
            m.write32(DRAW_COUNTERS + index * 4, 0);
        }
        // Route the update through the memory owner: MappedGuest redirects this word to
        // FrameClockState when native ownership is selected, while test/backing memories can
        // provide their own typed clock without consulting the process-global FFI selection.
        m.write32(OUTER_FRAMES, m.read32(OUTER_FRAMES).wrapping_add(1));

        call_cdecl(guest, m, &mut frame, 0x0003_c440, &[]);
        call_cdecl(guest, m, &mut frame, 0x0015_3b00, &[]);
        m.write32(FRAME_PHASE, 1u32.wrapping_sub(m.read32(FRAME_PHASE)));
        call_cdecl(guest, m, &mut frame, 0x0015_38c0, &[]);
        call_cdecl(guest, m, &mut frame, 0x0001_7d20, &[]);
        call_cdecl(guest, m, &mut frame, 0x0002_32a0, &[]);
        call_cdecl(guest, m, &mut frame, 0x0002_3300, &[]);
        call_cdecl(guest, m, &mut frame, 0x0001_7e90, &[delayed_stamp]);

        delayed_stamp = previous_stamp;
        previous_stamp = m.read32(SAMPLED_FRAME);
    }

    for _ in 0..5 {
        call_cdecl(guest, m, &mut frame, 0x0003_89b0, &[]);
        call_cdecl(guest, m, &mut frame, 0x0002_32a0, &[]);
        call_cdecl(guest, m, &mut frame, 0x0002_3300, &[]);
    }

    // 0x3D370 tail-jumps here after restoring its callee-saved registers. Preserve the
    // original return slot so the host wrapper can return through the guest ABI.
    frame.ebx = saved.ebx;
    frame.esi = saved.esi;
    frame.edi = saved.edi;
    frame.esp = saved.esp;
    guest.call(0x0002_3230, &mut frame);
    *regs = frame;
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rust_count_ext(va: u32, rust: bool) -> u64 {
    EXT_ROOTS
        .iter()
        .position(|&root| root == va)
        .map_or(0, |index| {
            if rust {
                &RUST_CALLS[index]
            } else {
                &C_CALLS[index]
            }
            .load(Ordering::Relaxed)
        })
}

#[cfg(test)]
mod frame_timing_tests {
    use super::{EXT_ROOTS, ExtRegisters, GuestCall, GuestMemory, frame_loop, parse_ext_selection};
    use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

    type Bytes = Rc<RefCell<BTreeMap<u32, u8>>>;

    struct TestMemory(Bytes);
    impl GuestMemory for TestMemory {
        fn read8(&self, address: u32) -> u8 {
            self.0.borrow().get(&address).copied().unwrap_or(0)
        }

        fn write8(&mut self, address: u32, value: u8) {
            self.0.borrow_mut().insert(address, value);
        }
    }

    struct OwnedClockMemory {
        raw: TestMemory,
        clock: std::rc::Rc<crate::frame_state::FrameClockState>,
    }

    impl GuestMemory for OwnedClockMemory {
        fn read8(&self, address: u32) -> u8 {
            self.clock
                .read_access(address, 1)
                .map_or_else(|| self.raw.read8(address), |value| value as u8)
        }

        fn write8(&mut self, address: u32, value: u8) {
            if !self.clock.write_access(address, 1, u32::from(value)) {
                self.raw.write8(address, value);
            }
        }
    }

    struct TestGuest {
        memory: Bytes,
        calls: Vec<u32>,
    }
    impl GuestCall for TestGuest {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            self.calls.push(va);
            regs.esp = regs.esp.wrapping_add(4);
            if va == 0x23300 {
                TestMemory(self.memory.clone()).write32(0x004c_0a2c, 1);
            }
        }
    }

    #[test]
    fn tail_jump_roots_return_with_their_slot_popped() {
        // SAFETY: ExtRegisters is plain data; zeroed is a valid value.
        let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = 0x4000;
        regs.run_tail_jump_root(|_| {});
        assert_eq!(regs.esp, 0x4004);
        regs.run_tail_jump_root(|r| r.esp += 4);
        assert_eq!(
            regs.esp, 0x4008,
            "a body that already returned is untouched"
        );
    }

    #[test]
    fn frame_timing_root_is_selectable_alongside_existing_extended_roots() {
        assert_eq!(EXT_ROOTS[4], 0x1538c0);
        assert_eq!(EXT_ROOTS[5], 0x3d370);
        assert_eq!(EXT_ROOTS[6], 0x3c440);
        assert_eq!(parse_ext_selection(b"1538c0"), Some(1 << 4));
        assert_eq!(parse_ext_selection(b"3d370"), Some(1 << 5));
        assert_eq!(parse_ext_selection(b"3c440"), Some(1 << 6));
        assert_eq!(parse_ext_selection(b"bb490,1538c0"), Some(1 | (1 << 4)));
    }

    #[test]
    fn anim_dependency_prefix_is_an_opt_in_extended_root() {
        assert_eq!(EXT_ROOTS[7], 0x1c8540);
        assert_eq!(parse_ext_selection(b"off"), Some(0));
        assert_eq!(parse_ext_selection(b"1c8540"), Some(1 << 7));
    }

    #[test]
    fn rust_outer_loop_owns_counter_and_preserves_measured_call_order() {
        let bytes = Rc::new(RefCell::new(BTreeMap::new()));
        let mut raw = TestMemory(bytes.clone());
        raw.write32(0x006b_7af0, 0xa5a5_5a5a);
        raw.write32(0x0074_97b0, 17);
        raw.write32(0x006b_7ab0, 1);
        let clock = std::rc::Rc::new(crate::frame_state::FrameClockState::new());
        clock.initialize(0, 0, 9, 0);
        let mut m = OwnedClockMemory {
            raw,
            clock: clock.clone(),
        };
        let mut guest = TestGuest {
            memory: bytes,
            calls: Vec::new(),
        };
        // SAFETY: all-zero integer, floating-point, and vector register state is valid here.
        let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
        regs.esp = 0x1000;
        regs.ebx = 11;
        regs.esi = 12;
        regs.edi = 13;

        frame_loop(&mut m, &mut guest, &mut regs);

        assert_eq!(m.read32(0x006b_7af0), 10);
        assert_eq!(clock.snapshot(), [0, 0, 10, 1]);
        assert_eq!(m.raw.read32(0x006b_7af0), 0xa5a5_5a5a);
        assert_eq!(m.read32(0x006b_7ab0), 0);
        assert_eq!(m.read32(0x006b_7ad4), 1);
        assert_eq!(regs.esp, 0x1004);
        assert_eq!((regs.ebx, regs.esi, regs.edi), (11, 12, 13));
        assert_eq!(
            &guest.calls[..9],
            &[
                0x389b0, 0x389b0, 0x3c440, 0x153b00, 0x1538c0, 0x17d20, 0x232a0, 0x23300, 0x17e90,
            ]
        );
        assert_eq!(&guest.calls[guest.calls.len() - 1..], &[0x23230]);
    }
}
