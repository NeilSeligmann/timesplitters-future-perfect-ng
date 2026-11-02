// SPDX-License-Identifier: GPL-3.0-or-later
// T1815: extended FP/XMM/closure ABI for call-bearing, float-bearing roots (BA230, BD0F0,
// B9960, ...). Mirrors the lifted-C model (docs/t1815-fp-closure-abi.md): x87 is a
// 8-slot f64 stack plus top and control word, XMM is five 128-bit registers (xmm0-xmm4).
// This is the contract only. No root uses it until admitted by an original-vs-Rust proof.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fpu {
    pub stack: [f64; 8],
    pub top: u32,
    pub control: u16,
}
impl Fpu {
    pub fn push(&mut self, v: f64) {
        self.top = (self.top + 7) & 7;
        self.stack[self.top as usize] = v;
    }
    pub fn pop(&mut self) {
        self.top = (self.top + 1) & 7;
    }
    pub fn st(&self, i: u32) -> f64 {
        self.stack[((self.top + i) & 7) as usize]
    }
    /// FLD m32 / FSTP m32 round to 24 bits unless precision control (bits 8-9) selects wider.
    pub fn round_single(&self, v: f64) -> f64 {
        if self.control & 0x300 != 0 {
            v
        } else {
            v as f32 as f64
        }
    }
    /// A float-returning cdecl root leaves its result in ST0, so top moves down by one.
    pub fn return_st0(&mut self, v: f32) {
        self.push(v as f64);
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Xmm {
    pub lanes: [u32; 4],
}
impl Xmm {
    /// XORPS reg,reg and MOVSS from memory clear lanes 1..3 as well (whole register).
    pub fn scalar(bits: u32) -> Self {
        Xmm {
            lanes: [bits, 0, 0, 0],
        }
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtRegisters {
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
    pub esp: u32,
    /// Callee-saved registers the lifted body pushes and pops itself (EBX, EBP, ESI, EDI).
    pub ebx: u32,
    pub ebp: u32,
    pub esi: u32,
    pub edi: u32,
    /// Arithmetic flags as the flag bridge models them: CF, PF, ZF, SF in EFLAGS positions.
    pub eflags: u32,
    pub fpu: Fpu,
    pub xmm: [Xmm; 5],
    /// XMM5-XMM7 (T1815 w7, additive: appended after `xmm` so every earlier offset is unchanged).
    pub xmm_hi: [Xmm; 3],
}
impl ExtRegisters {
    /// Run a root whose body ends in `RegsExt::tail_call`. That idiom leaves ESP on the root's own
    /// return slot, but the bridge contract is that a returned root has popped it (entry + 4), as the
    /// lifted `RET` does. A body that already returned is untouched. (I-T3879: the unpopped slot
    /// stopped the Rust frame loop at its first 0x3C440 call.)
    pub fn run_tail_jump_root(&mut self, body: impl FnOnce(&mut Self)) {
        let entry_esp = self.esp;
        body(self);
        if self.esp == entry_esp {
            self.esp = entry_esp.wrapping_add(4);
        }
    }
}
/// Closure boundary: Rust calls an unported callee by running the retained lifted original.
/// The callee follows cdecl: arguments already pushed at esp+4.., it RETs (esp += 4), the
/// caller pops arguments. Float results come back in `fpu.st(0)` and must be consumed with
/// `pop` exactly as the original's FSTP does. The c2r `RegsExt::call` helper preserves
/// the caller's frame EBP because lifted callees use a C-local EBP while the bridge
/// returns its thread-global EBP. Every call is counted for the A/B report.
pub trait GuestCall {
    /// Current Rust-owned SEH frame EBP when the execution context exposes it.
    /// Lifted capture replay supplies this separately from `ExtRegisters::ebp`,
    /// which can be the host frame value at the capture boundary.
    fn seh_ebp(&self) -> Option<u32> {
        None
    }
    /// MMX is separate from x87 in the lifted reference. Implement this per guest context;
    /// the legacy 240-byte register ABI deliberately stays unchanged. Unwired contexts reject
    /// these ports instead of silently initialising MMX state or aliasing the x87 stack.
    fn mmx_read(&self, _index: usize) -> u64 {
        panic!("MMX guest state is not configured")
    }
    fn mmx_write(&mut self, _index: usize, _value: u64) {
        panic!("MMX guest state is not configured")
    }
    /// Run host work at the original guest getter boundary immediately before a
    /// native vblank-counter sample. Pure Rust callers may leave this empty.
    fn frame_wait_preflight(&mut self, _getter: u32, _return_address: u32) {}
    fn call(&mut self, va: u32, regs: &mut ExtRegisters);
}
/// cdecl helper: push `args` right to left, call, pop them again, and return the pointer
/// of the pushed block so alias-sensitive readers can reload after the callee.
pub fn call_cdecl<C: GuestCall, M: crate::memory::GuestMemory>(
    c: &mut C,
    m: &mut M,
    regs: &mut ExtRegisters,
    va: u32,
    args: &[u32],
) {
    let entry = regs.esp;
    for a in args.iter().rev() {
        regs.esp = regs.esp.wrapping_sub(4);
        m.write32(regs.esp, *a);
    }
    c.call(va, regs);
    regs.esp = entry; // the callee RET popped its own return slot, the caller pops arguments
}
// The C bridge (src/host/rust_ext_bridge.c) declares the identical layout and asserts it too.
/// Lifted-code range of the host (`g_xbox_code_lo/hi`). Mirrors `RECOMP_ICALL_IS_CODE`
/// (third_party/xboxrecomp/templates/runtime/recomp_types.h): a target at or above the synthetic
/// kernel window 0xFE000000, any target while no span is configured (`hi == 0`), or one inside
/// `[lo, hi)` is code. Anything else is the lifted `not-code` path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CodeRange {
    pub lo: u32,
    pub hi: u32,
}
impl CodeRange {
    pub fn is_code(&self, va: u32) -> bool {
        va >= 0xFE00_0000 || self.hi == 0 || (va >= self.lo && va < self.hi)
    }
}
/// Indirect cdecl call (for example the definition+0x294 hooks): like `call_cdecl`, but a target
/// outside the lifted-code range takes the lifted `RECOMP_ICALL` not-code path. The callee never
/// runs, EAX becomes 0 and the pushed arguments are popped. Returns whether the callee ran.
pub fn call_indirect_cdecl<C: GuestCall, M: crate::memory::GuestMemory>(
    c: &mut C,
    m: &mut M,
    regs: &mut ExtRegisters,
    va: u32,
    args: &[u32],
    code: CodeRange,
) -> bool {
    if !code.is_code(va) {
        regs.eax = 0;
        return false;
    }
    call_cdecl(c, m, regs, va, args);
    true
}
const _: () = {
    assert!(core::mem::size_of::<ExtRegisters>() == 240);
    assert!(core::mem::offset_of!(ExtRegisters, xmm_hi) == 192);
    assert!(core::mem::offset_of!(ExtRegisters, fpu) == 40);
    assert!(core::mem::offset_of!(ExtRegisters, xmm) == 112);
    assert!(core::mem::offset_of!(Fpu, top) == 64 && core::mem::offset_of!(Fpu, control) == 68);
};
