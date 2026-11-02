// SPDX-License-Identifier: GPL-3.0-or-later
//! T1835 w12: Rust port of region 0 of the entity tick 0x2784D0 (prologue up to the first
//! jump-table dispatch at 0x27881B, original bytes 0x2784D0..0x27881B). Not admitted, not
//! registered. The remaining regions run as the retained lifted original: the port hands its exact
//! exit state to a resumable copy of the lifted body through a pseudo call (`RESUME_BASE | label`).
//! Region map and evidence: docs/t1835-entity-tick.md.
use crate::state_wire as gs;
use crate::{
    abi::{ExtRegisters, GuestCall, Xmm},
    memory::GuestMemory,
};

/// Pseudo call target the C trampoline maps to "resume the lifted tick at this label".
pub const RESUME_BASE: u32 = 0xFD00_0000;
pub const TICK_VA: u32 = 0x2784D0;
/// Region 0 exits: early epilogue (flag bit 2), state out of range, jump-table dispatch.
pub const EXIT_EPILOGUE: u32 = 0x27B798;
pub const EXIT_LEAVE: u32 = 0x27B716;
pub const EXIT_DISPATCH: u32 = 0x27881B;

#[cfg(target_arch = "x86_64")]
mod sse {
    macro_rules! op { ($name:ident, $mn:literal) => {
        pub fn $name(a: f32, b: f32) -> f32 {
            let mut r = a;
            unsafe { core::arch::asm!(concat!($mn, " {r}, {b}"), r = inout(xmm_reg) r, b = in(xmm_reg) b, options(pure, nomem, nostack)) };
            r
        }
    } }
    op!(addss, "addss"); op!(subss, "subss"); op!(mulss, "mulss"); op!(divss, "divss");
}
#[cfg(not(target_arch = "x86_64"))]
mod sse {
    pub fn addss(a: f32, b: f32) -> f32 { a + b }
    pub fn subss(a: f32, b: f32) -> f32 { a - b }
    pub fn mulss(a: f32, b: f32) -> f32 { a * b }
    pub fn divss(a: f32, b: f32) -> f32 { a / b }
}

fn f(bits: u32) -> f32 { f32::from_bits(bits) }
fn lane0(x: &Xmm) -> f32 { f(x.lanes[0]) }
fn set0(x: &mut Xmm, v: f32) { x.lanes[0] = v.to_bits(); }
/// MOVSS xmm, m32 and XORPS reg,reg: whole register written (lanes 1..3 cleared).
fn load_ss<M: GuestMemory>(m: &M, address: u32) -> Xmm { Xmm::scalar(m.read32(address)) }
/// COMISS a,b: JBE taken when a <= b or unordered.
fn below_or_equal(a: f32, b: f32) -> bool { !(a > b) }

struct Tick<'a, M: GuestMemory, C: GuestCall> {
    m: &'a mut M,
    guest: &'a mut C,
    regs: ExtRegisters,
    /// The lifted function's local EBP (the character object). `regs.ebp` stays the caller's value.
    ebp: u32,
}
impl<M: GuestMemory, C: GuestCall> Tick<'_, M, C> {
    fn push(&mut self, value: u32) {
        self.regs.esp = self.regs.esp.wrapping_sub(4);
        self.m.write32(self.regs.esp, value);
    }
    fn call(&mut self, va: u32) { self.guest.call(va, &mut self.regs); }
    fn at(&self, offset: u32) -> u32 { self.ebp.wrapping_add(offset) }
    fn enter_state(&mut self, previous: u32) {
        // mov [ebp+0x9d4],prev ; mov [ebp+0x20],1 ; mov [ebp+0x9d8],xmm ; mov [ebp+0x9d0],1
        let (a, b, c) = (self.at(0x9d4), self.at(0x20), self.at(0x9d0));
        self.m.write32(a, previous);
        self.m.write32(b, 1);
        self.m.write32(c, 1);
    }
}

/// Region 0 of 0x2784D0 followed by the lifted remainder. cdecl, EAX unused, the entity is the
/// first stack argument. `regs.ebp` must equal the caller's frame register (the lifted body
/// pushes `g_seh_ebp`, which the host keeps equal to EBP). ESP is left at entry like every ext root.
pub fn entity_tick<M: GuestMemory, C: GuestCall>(m: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    let entry = *regs;
    let mut t = Tick { m, guest, regs: entry, ebp: entry.ebp };
    let exit = t.region0();
    let mut resume = t.regs;
    resume.ebp = t.ebp;
    t.guest.call(RESUME_BASE | exit, &mut resume);
    // The lifted epilogue RET left ESP at entry + 4, like the original. Ext roots leave it at entry.
    resume.esp = entry.esp;
    *regs = resume; // EBP is whatever the lifted epilogue popped (the frame the port pushed)
}

impl<M: GuestMemory, C: GuestCall> Tick<'_, M, C> {
    fn pop_args(&mut self, count: u32) { self.regs.esp = self.regs.esp.wrapping_add(4 * count); }
    fn set_state_one(&mut self, previous: u32, xmm_bits: u32) {
        let (a, b, c, d) = (self.at(0x9d4), self.at(0x20), self.at(0x9d8), self.at(0x9d0));
        self.m.write32(a, previous);
        self.m.write32(b, 1);
        self.m.write32(c, xmm_bits);
        self.m.write32(d, 1);
    }
    /// Returns the label at which the lifted body must resume.
    fn region0(&mut self) -> u32 {
        let entry_esp = self.regs.esp;
        let caller_ebp = self.regs.ebp;
        // 0x2784D0 sub esp,0x50 ; push ebp ; push esi
        self.regs.esp = entry_esp.wrapping_sub(0x50);
        self.push(caller_ebp);
        let caller_esi = self.regs.esi;
        self.push(caller_esi);
        // mov esi,[esp+0x5c] ; mov ebp,[esi+0x7c]
        let esi = self.m.read32(self.regs.esp.wrapping_add(0x5c));
        self.regs.esi = esi;
        self.ebp = self.m.read32(esi.wrapping_add(0x7c));
        // push edi ; mov edi,1 ; push edi ; push ebp ; call 0x15E120
        let caller_edi = self.regs.edi;
        self.push(caller_edi);
        self.regs.edi = 1;
        self.push(1);
        self.push(self.ebp);
        self.call(0x15e120);
        // mov [esp+0x14],eax ; mov eax,[ebp+0x9d0] ; add esp,8 ; or edx,-1
        let slot = self.regs.esp.wrapping_add(0x14);
        self.m.write32(slot, self.regs.eax);
        self.regs.eax = self.m.read32(self.at(0x9d0));
        self.pop_args(2);
        self.regs.edx = 0xffff_ffff;
        if (self.regs.eax as i32) >= 0 {
            self.regs.ecx = self.m.read32(self.at(0x20));
            if self.regs.eax != self.regs.ecx {
                self.regs.xmm[0] = Xmm::default();
                let (a, b, c) = (self.at(0x9d8), self.at(0x9d4), self.at(0x20));
                self.m.write32(a, 0);
                self.m.write32(b, self.regs.ecx);
                self.m.write32(c, self.regs.eax);
            }
            let a = self.at(0x9d0);
            self.m.write32(a, self.regs.edx);
        }
        // 0x27851E
        self.regs.eax = self.m.read32(self.at(0x2c));
        self.regs.xmm[3] = Xmm::default();
        let a90 = self.at(0xa90);
        if (self.regs.eax as u8 as i8) < 0 {
            let count = self.m.read32(0x4e8cec) as i32;
            set0(&mut self.regs.xmm[0], count as f32);
            self.regs.xmm[1] = load_ss(self.m, 0x475c78);
            let q = sse::divss(lane0(&self.regs.xmm[1]), lane0(&self.regs.xmm[0]));
            let q = sse::mulss(q, f(self.m.read32(0x4e794c)));
            let q = sse::addss(q, f(self.m.read32(a90)));
            set0(&mut self.regs.xmm[1], q);
            self.m.write32(a90, q.to_bits());
        } else {
            let bits = self.regs.xmm[3].lanes[0];
            self.m.write32(a90, bits);
        }
        // 0x27855E test al,4 ; jne 0x27B798
        if self.regs.eax & 4 != 0 { return EXIT_EPILOGUE; }
        // squared distance between this entity and the object at [[[0x7b0c48]+0x14]+0x7c]
        let world = self.m.read32(0x7b0c48);
        self.regs.ecx = self.m.read32(world.wrapping_add(0x14));
        self.regs.eax = self.m.read32(self.regs.ecx.wrapping_add(0x7c));
        self.m.write32(0x78b2c8, self.regs.eax);
        self.regs.eax = self.m.read32(self.regs.eax);
        let other = self.regs.eax;
        let delta = |m: &M, off: u32| sse::subss(f(m.read32(esi.wrapping_add(off))), f(m.read32(other.wrapping_add(off))));
        let d68 = delta(self.m, 0x68);
        let d6c = delta(self.m, 0x6c);
        let d64 = delta(self.m, 0x64);
        let cur_25c = self.m.read32(0x78b25c);
        self.regs.ecx = esi.wrapping_add(0x64);
        let x4 = sse::mulss(d68, d68);
        let sq64 = sse::mulss(d64, d64);
        let partial = sse::addss(sq64, x4);
        let x1 = sse::mulss(d6c, d6c);
        let x2 = sse::addss(partial, x1);
        self.regs.xmm[0] = Xmm::scalar(d6c.to_bits());
        self.regs.xmm[1] = Xmm::scalar(x1.to_bits());
        self.regs.xmm[2] = Xmm::scalar(x2.to_bits());
        self.regs.xmm[4] = Xmm::scalar(x4.to_bits());
        // push ebx ; mov [esp+0x14],ecx ; movss [esp+0x30],xmm2
        let caller_ebx = self.regs.ebx;
        self.push(caller_ebx);
        self.m.write32(self.regs.esp.wrapping_add(0x14), self.regs.ecx);
        self.m.write32(self.regs.esp.wrapping_add(0x30), x2.to_bits());
        // jne 0x2785d2 ; cmp [0x78b2d4],edx ; je 0x2786d2
        if cur_25c != self.regs.edx || self.m.read32(0x78b2d4) != self.regs.edx { self.scan_slots(); }
        self.drain_pending();
        self.tail_of_region0()
    }
    /// 0x2785D2..0x2786D2: look for the slot whose id [obj+0x678] matches one of the pending ids.
    fn scan_slots(&mut self) {
        self.regs.eax = gs::get_players(self.m);
        self.regs.ecx = 0;
        if (self.regs.eax as i32) <= 0 { return; }
        self.regs.edx = self.m.read32(0x78b260);
        self.regs.ebx = self.m.read32(0x78b2c4);
        loop {
            let object = self.m.read32(self.regs.ecx.wrapping_mul(4).wrapping_add(0x7a3580));
            self.regs.eax = self.m.read32(object.wrapping_add(0x678));
            let id = self.regs.eax;
            if id == self.regs.edx || id == self.regs.ebx { return self.matched_primary(); }
            if id == self.m.read32(0x78b2d0) { self.regs.edi = 1; return self.matched_primary(); }
            if id == self.m.read32(0x78b2c0) || id == self.m.read32(0x78b254) || id == self.m.read32(0x78b264) {
                return self.matched_secondary();
            }
            self.regs.eax = gs::get_players(self.m);
            self.regs.ecx = self.regs.ecx.wrapping_add(1);
            self.regs.edi = 1;
            if (self.regs.ecx as i32) >= (self.regs.eax as i32) { return; }
        }
    }
    /// 0x27868E
    fn matched_primary(&mut self) {
        self.regs.eax = self.m.read32(0x78b25c);
        let a = self.at(0x9ec);
        self.m.write32(a, self.regs.eax);
        self.regs.eax = self.m.read32(self.at(0x20));
        self.regs.ecx = 0xffff_ffff;
        if self.regs.eax != self.regs.edi {
            let bits = self.regs.xmm[3].lanes[0];
            self.set_state_one(self.regs.eax, bits);
        }
        for address in [0x78b25c, 0x78b2d0, 0x78b2c4, 0x78b260] { self.m.write32(address, self.regs.ecx); }
    }
    /// 0x278641
    fn matched_secondary(&mut self) {
        self.regs.eax = self.m.read32(self.at(0x20));
        self.regs.edx = self.m.read32(0x78b2d4);
        self.regs.edi = 1;
        let a = self.at(0x9ec);
        self.m.write32(a, self.regs.edx);
        if self.regs.eax != self.regs.edi {
            let bits = self.regs.xmm[3].lanes[0];
            self.set_state_one(self.regs.eax, bits);
        }
        self.regs.eax = 0xffff_ffff;
        for address in [0x78b2d4, 0x78b264, 0x78b254, 0x78b2c0] { self.m.write32(address, self.regs.eax); }
    }
    /// 0x2786D2..0x27872C: eight (id, value) pending-event pairs at 0x78b280.
    fn drain_pending(&mut self) {
        self.regs.ebx = 0x78b280;
        loop {
            self.regs.eax = self.m.read32(self.regs.ebx);
            if self.regs.eax != 0 {
                self.push(self.regs.eax);
                self.call(0x234270);
                self.pop_args(1);
                if self.regs.eax != 0 {
                    self.regs.eax = self.m.read32(self.at(0x20));
                    self.regs.ecx = self.m.read32(self.regs.ebx.wrapping_add(4));
                    let a = self.at(0x9ec);
                    self.m.write32(a, self.regs.ecx);
                    if self.regs.eax != self.regs.edi {
                        self.regs.xmm[0] = Xmm::default();
                        self.set_state_one(self.regs.eax, 0);
                    }
                    self.m.write32(self.regs.ebx, 0);
                    self.m.write32(self.regs.ebx.wrapping_add(4), 0xffff_ffff);
                }
            }
            self.regs.ebx = self.regs.ebx.wrapping_add(8);
            if (self.regs.ebx as i32) >= 0x78b2c0 { return; }
        }
    }
    /// 0x27872C..0x27881B
    fn tail_of_region0(&mut self) -> u32 {
        let esi = self.regs.esi;
        self.regs.xmm[0] = Xmm::default();
        self.push(esi);
        let a = self.at(0xb3c);
        self.m.write32(a, 0);
        self.call(0x276b60);
        self.regs.eax = self.m.read32(0x78b250);
        self.pop_args(1);
        if self.regs.eax == self.regs.edi {
            self.regs.eax = self.m.read32(0x78b230);
            if self.regs.eax != 0xffff_ffff {
                self.push(self.regs.eax);
                self.call(0x160ec0);
                self.pop_args(1);
                if self.regs.eax != 0 {
                    self.m.write32(0x78b250, 0);
                    self.m.write32(0x78b230, 0xffff_ffff);
                }
            }
        }
        self.push(esi);
        self.call(0x276660);
        self.regs.eax = self.m.read32(self.at(0x9ec));
        self.regs.xmm[0] = load_ss(self.m, self.at(0xb44));
        self.regs.edx = self.regs.eax.wrapping_add(self.regs.eax.wrapping_mul(8));
        self.regs.eax = self.m.read32(0x74c36c);
        self.regs.ecx = self.m.read32(self.regs.eax.wrapping_add(0x3c));
        self.pop_args(1);
        let limit = f(self.m.read32(0x4760cc));
        self.regs.ebx = self.regs.ecx.wrapping_add(self.regs.edx.wrapping_mul(8));
        self.m.write32(self.regs.esp.wrapping_add(0x18), self.regs.ebx);
        let mut applied = false;
        if lane0(&self.regs.xmm[0]) > limit {
            self.regs.edx = self.m.read32(0x7b0c7c);
            if self.m.read32(self.regs.edx.wrapping_add(0xff4)) != esi && gs::get_level(self.m) != 0x1a {
                self.regs.xmm[3] = load_ss(self.m, esi.wrapping_add(0x1d8));
                self.push(esi);
                self.call(0x2777e0);
                let sum = sse::addss(lane0(&self.regs.xmm[0]), f(self.m.read32(esi.wrapping_add(0x1cc))));
                set0(&mut self.regs.xmm[0], sum);
                self.pop_args(1);
                let upper = lane0(&self.regs.xmm[3]);
                self.regs.xmm[2] = self.regs.xmm[3];
                if !(lane0(&self.regs.xmm[0]) > upper) { self.regs.xmm[2] = self.regs.xmm[0]; }
                self.regs.xmm[1] = Xmm::default();
                if lane0(&self.regs.xmm[1]) > lane0(&self.regs.xmm[2]) {
                    self.regs.xmm[0] = self.regs.xmm[1];
                } else if lane0(&self.regs.xmm[0]) > upper {
                    self.regs.xmm[0] = self.regs.xmm[3];
                }
                let bits = self.regs.xmm[0].lanes[0];
                self.m.write32(esi.wrapping_add(0x1cc), bits);
                applied = true;
            }
        }
        if !applied { self.regs.xmm[1] = Xmm::default(); }
        self.regs.eax = self.m.read32(self.at(0x20));
        self.regs.ecx = 0x15;
        if self.regs.eax > 0x15 { EXIT_LEAVE } else { EXIT_DISPATCH }
    }
}

pub const HOOK_FLAGS_A: u32 = 0x278822;
pub const HOOK_FLAGS_B: u32 = 0x27889B;
pub const LABEL_FINISH: u32 = 0x278B6A;
pub const LABEL_DISPATCH2: u32 = 0x2788B8;

impl<M: GuestMemory, C: GuestCall> Tick<'_, M, C> {
    /// Region 1a, 0x278822..0x27889B (a jump-table target for state 0): optionally runs the
    /// pending-reset callees, clears the vertical offset fields and goes to 0x278B6A with EAX = 1.
    fn region1_reset(&mut self) -> u32 {
        self.regs.eax = self.m.read32(0x78b234);
        self.regs.edi = 0;
        if self.regs.eax != self.regs.edi {
            self.call(0x2774d0);
            self.regs.eax = self.m.read32(0x78b2c8);
            self.regs.ecx = self.m.read32(self.regs.eax.wrapping_add(0x84));
            self.push(1);
            self.push(self.regs.esi);
            let a = self.at(0x84);
            self.m.write32(a, self.regs.ecx);
            self.call(0x276f80);
            self.regs.xmm[0] = Xmm::default();
            let (a, b) = (self.at(4), self.at(0xb30));
            self.m.write32(a, self.regs.eax);
            self.m.write32(b, 0);
            self.pop_args(2);
            self.m.write32(0x78b234, self.regs.edi);
            self.m.write32(0x78b268, 0xffff_ffff);
            self.regs.xmm[1] = self.regs.xmm[0];
        }
        self.regs.eax = self.m.read32(self.at(0x20));
        let (a, b) = (self.at(0xab8), self.at(0xabc));
        self.m.write32(a, 0x2000);
        self.m.write32(b, self.regs.edi);
        if self.regs.eax == 1 { return EXIT_LEAVE; }
        let a = self.at(0x9d4);
        self.m.write32(a, self.regs.eax);
        self.regs.eax = 1;
        LABEL_FINISH
    }
    /// Region 1b, 0x27889B (a jump-table target for state 1): clears flag bit 1, stores XMM1 and
    /// bounds-checks the second selector before the 0x2788B8 jump table.
    fn region1_flags(&mut self) -> u32 {
        self.regs.edi = self.m.read32(self.at(0x28)) & 0xffff_fffd;
        let (a, b) = (self.at(0xb30), self.at(0x28));
        self.m.write32(a, self.regs.xmm[1].lanes[0]);
        self.m.write32(b, self.regs.edi);
        self.regs.eax = self.m.read32(self.regs.ebx.wrapping_add(8));
        if self.regs.eax > 0x2d { EXIT_LEAVE } else { LABEL_DISPATCH2 }
    }
}

/// Hook-out entry used by the resumable lifted body at the ported labels. Returns the label the
/// lifted body continues at, or 0 when `label` is not ported (the lifted code then runs itself).
///
/// # Safety
/// `local_ebp` and `regs` are live, `offset` maps the guest window, `call` is the trampoline.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rust_tick_hook(
    label: u32, local_ebp: *mut u32, regs: *mut ExtRegisters, offset: usize, call: crate::ext::CallFn,
) -> u32 {
    let mut memory = unsafe { crate::memory::MappedGuest::new(offset) };
    let mut guest = crate::ext::CGuest(call);
    let mut tick = Tick { m: &mut memory, guest: &mut guest, regs: unsafe { *regs }, ebp: unsafe { *local_ebp } };
    let next = match label {
        HOOK_FLAGS_A => tick.region1_reset(),
        HOOK_FLAGS_B => tick.region1_flags(),
        _ => return 0,
    };
    unsafe { *regs = tick.regs; *local_ebp = tick.ebp; }
    next
}

/// Test-only entry (the tick is not an ext root: nothing selects it in the production host, and
/// the resume pseudo call aborts when no resumable lifted body is installed).
///
/// # Safety
/// `regs` is a live `ExtRegisters`, `offset` maps the guest window, `call` is the trampoline.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rust_tick_region0(regs: *mut ExtRegisters, offset: usize, call: crate::ext::CallFn) {
    let mut memory = unsafe { crate::memory::MappedGuest::new(offset) };
    let mut guest = crate::ext::CGuest(call);
    entity_tick(&mut memory, &mut guest, unsafe { &mut *regs });
}

/// T1510 `ClosureFp` comparison of two register records, all eight physical slots occupied
/// (the lifted f64 stack has no tag word, stale slots must match too). Returns the DIFF_* bits.
///
/// # Safety
/// Both pointers refer to live `ExtRegisters`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_tick_fp_differences(expected: *const ExtRegisters, actual: *const ExtRegisters) -> u32 {
    use crate::fpclosure::{differences, ClosureFp};
    let lift = |r: &ExtRegisters| ClosureFp {
        slots: core::array::from_fn(|i| r.fpu.stack[i].to_bits()),
        top: (r.fpu.top & 7) as u8,
        control: r.fpu.control,
        depth: 8,
    };
    let (e, a) = unsafe { (lift(&*expected), lift(&*actual)) };
    differences(Some(&e), Some(&a)).unwrap_or(u32::MAX)
}
