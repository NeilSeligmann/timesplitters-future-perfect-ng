// SPDX-License-Identifier: GPL-3.0-or-later
// T1917: local network slice state; derived from the existing T1815 register contract.
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
pub struct NetworkRegisters {
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
/// Network/live, session, CRT, XDK and virtual-call attachment boundary.
/// Arguments start at `regs.esp`: the adapter supplies the nested return slot,
/// executes the callee and consumes that slot. A cdecl callee leaves ESP unchanged;
/// stdcall advances it by its argument byte count. Preserve all register/memory effects.
/// This local state contract does not install a production trampoline or selector.
pub trait NetworkCall {
    fn call(&mut self, va: u32, regs: &mut NetworkRegisters);
}

impl Default for Fpu {
    fn default() -> Self {
        Self {
            stack: [0.0; 8],
            top: 0,
            control: 0x37f,
        }
    }
}
impl Default for NetworkRegisters {
    fn default() -> Self {
        Self {
            eax: 0,
            ecx: 0,
            edx: 0,
            esp: 0,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            eflags: 0,
            fpu: Fpu::default(),
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        }
    }
}
