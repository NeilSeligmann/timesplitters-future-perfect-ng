// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_imports)]
use crate::{
    abi::{self, ExtRegisters, Fpu, GuestCall, Xmm},
    mapmaker_ui,
    memory::{self, GuestMemory},
};
use std::{cell::Cell, collections::BTreeMap};
#[derive(Default)]
struct Mem {
    bytes: BTreeMap<u32, u8>,
    accesses: Cell<u32>,
}
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        let n = self.accesses.get() + 1;
        self.accesses.set(n);
        assert!(n < 100000, "fixture needs bounded structured state");
        *self.bytes.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.bytes.insert(a, v);
    }
}
#[derive(Default)]
struct Host {
    calls: Vec<u32>,
}
impl GuestCall for Host {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        assert!(self.calls.len() < 256, "fixture needs scripted callee");
        self.calls.push(va);
        r.eax = 0;
    }
}
fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: 0x10000,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.; 8],
            top: 0,
            control: 0x27f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}
#[test]
fn smoke_70e30() {
    mapmaker_ui::f70e30(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_802c0() {
    mapmaker_ui::f802c0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_81480() {
    mapmaker_ui::f81480(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_81c10() {
    mapmaker_ui::f81c10(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_81c40() {
    mapmaker_ui::f81c40(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fa030() {
    mapmaker_ui::f2fa030(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fe450() {
    mapmaker_ui::f2fe450(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_308b10() {
    mapmaker_ui::f308b10(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_308c00() {
    mapmaker_ui::f308c00(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3091c0() {
    mapmaker_ui::f3091c0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3092e0() {
    mapmaker_ui::f3092e0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3096a0() {
    mapmaker_ui::f3096a0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_309a80() {
    mapmaker_ui::f309a80(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30aea0() {
    mapmaker_ui::f30aea0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30b150() {
    mapmaker_ui::f30b150(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30b280() {
    mapmaker_ui::f30b280(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30b3a0() {
    mapmaker_ui::f30b3a0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30b480() {
    mapmaker_ui::f30b480(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30b560() {
    mapmaker_ui::f30b560(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30b7c0() {
    mapmaker_ui::f30b7c0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30b9b0() {
    mapmaker_ui::f30b9b0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30ba90() {
    mapmaker_ui::f30ba90(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30bc50() {
    mapmaker_ui::f30bc50(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30be60() {
    mapmaker_ui::f30be60(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30c210() {
    mapmaker_ui::f30c210(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30c3c0() {
    mapmaker_ui::f30c3c0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30c440() {
    mapmaker_ui::f30c440(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30cb30() {
    mapmaker_ui::f30cb30(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30cb90() {
    mapmaker_ui::f30cb90(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30ccd0() {
    mapmaker_ui::f30ccd0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30cf60() {
    mapmaker_ui::f30cf60(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30d120() {
    mapmaker_ui::f30d120(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30d300() {
    mapmaker_ui::f30d300(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30d410() {
    mapmaker_ui::f30d410(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
#[ignore = "needs structured fixture; compile-only verification debt"]
fn smoke_30d4d0() {
    mapmaker_ui::f30d4d0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30d7b0() {
    mapmaker_ui::f30d7b0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30dd40() {
    mapmaker_ui::f30dd40(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30de40() {
    mapmaker_ui::f30de40(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_310100() {
    mapmaker_ui::f310100(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_310310() {
    mapmaker_ui::f310310(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3108d0() {
    mapmaker_ui::f3108d0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_310a20() {
    mapmaker_ui::f310a20(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_312270() {
    mapmaker_ui::f312270(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
#[ignore = "needs structured fixture; compile-only verification debt"]
fn smoke_3133e0() {
    mapmaker_ui::f3133e0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_314420() {
    mapmaker_ui::f314420(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_314f80() {
    mapmaker_ui::f314f80(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_314fa0() {
    mapmaker_ui::f314fa0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_315d20() {
    mapmaker_ui::f315d20(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_315d60() {
    mapmaker_ui::f315d60(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_315db0() {
    mapmaker_ui::f315db0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_315e10() {
    mapmaker_ui::f315e10(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_316110() {
    mapmaker_ui::f316110(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_316b50() {
    mapmaker_ui::f316b50(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3179d0() {
    mapmaker_ui::f3179d0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_317d70() {
    mapmaker_ui::f317d70(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_318350() {
    mapmaker_ui::f318350(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3188c0() {
    mapmaker_ui::f3188c0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_318be0() {
    mapmaker_ui::f318be0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_318ea0() {
    mapmaker_ui::f318ea0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3190c0() {
    mapmaker_ui::f3190c0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3191f0() {
    mapmaker_ui::f3191f0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3192e0() {
    mapmaker_ui::f3192e0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_319300() {
    mapmaker_ui::f319300(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_319bd0() {
    mapmaker_ui::f319bd0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_319da0() {
    mapmaker_ui::f319da0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_319e60() {
    mapmaker_ui::f319e60(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31ab80() {
    mapmaker_ui::f31ab80(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31ad90() {
    mapmaker_ui::f31ad90(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31b6b0() {
    mapmaker_ui::f31b6b0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31b7d0() {
    mapmaker_ui::f31b7d0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31b8b0() {
    mapmaker_ui::f31b8b0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31c7c0() {
    mapmaker_ui::f31c7c0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31cb90() {
    mapmaker_ui::f31cb90(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31d140() {
    mapmaker_ui::f31d140(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31d160() {
    mapmaker_ui::f31d160(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31d180() {
    mapmaker_ui::f31d180(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31d3e0() {
    mapmaker_ui::f31d3e0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31d580() {
    mapmaker_ui::f31d580(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31dc10() {
    mapmaker_ui::f31dc10(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31eab0() {
    mapmaker_ui::f31eab0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31ec90() {
    mapmaker_ui::f31ec90(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31ee30() {
    mapmaker_ui::f31ee30(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31fdc0() {
    mapmaker_ui::f31fdc0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_320250() {
    mapmaker_ui::f320250(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_320580() {
    mapmaker_ui::f320580(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_321f20() {
    mapmaker_ui::f321f20(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3223a0() {
    mapmaker_ui::f3223a0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_322bb0() {
    mapmaker_ui::f322bb0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3236c0() {
    mapmaker_ui::f3236c0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_323c30() {
    mapmaker_ui::f323c30(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3244e0() {
    mapmaker_ui::f3244e0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3246a0() {
    mapmaker_ui::f3246a0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_324c00() {
    mapmaker_ui::f324c00(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3266b0() {
    mapmaker_ui::f3266b0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3267c0() {
    mapmaker_ui::f3267c0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3268c0() {
    mapmaker_ui::f3268c0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3276f0() {
    mapmaker_ui::f3276f0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_327a70() {
    mapmaker_ui::f327a70(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_327f90() {
    mapmaker_ui::f327f90(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_328d20() {
    mapmaker_ui::f328d20(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_329de0() {
    mapmaker_ui::f329de0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_32ce00() {
    mapmaker_ui::f32ce00(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_32d9b0() {
    mapmaker_ui::f32d9b0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_32dba0() {
    mapmaker_ui::f32dba0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_331c70() {
    mapmaker_ui::f331c70(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_334730() {
    mapmaker_ui::f334730(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3368b0() {
    mapmaker_ui::f3368b0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3368e0() {
    mapmaker_ui::f3368e0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_336910() {
    mapmaker_ui::f336910(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_336950() {
    mapmaker_ui::f336950(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_336d30() {
    mapmaker_ui::f336d30(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_336d60() {
    mapmaker_ui::f336d60(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_336d90() {
    mapmaker_ui::f336d90(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_336dc0() {
    mapmaker_ui::f336dc0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_336e60() {
    mapmaker_ui::f336e60(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_336f60() {
    mapmaker_ui::f336f60(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_336fb0() {
    mapmaker_ui::f336fb0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_337ed0() {
    mapmaker_ui::f337ed0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn disabled_and_unknown_do_not_mutate() {
    let mut m = Mem::default();
    let mut g = Host::default();
    let mut r = regs();
    let original = r;
    for va in mapmaker_ui::ROOTS {
        assert!(!mapmaker_ui::run(*va, false, &mut m, &mut g, &mut r));
    }
    assert!(!mapmaker_ui::run(0, true, &mut m, &mut g, &mut r));
    assert_eq!(r.eax, original.eax);
    assert_eq!(r.esp, original.esp);
    assert!(g.calls.is_empty());
    assert!(m.bytes.is_empty());
    assert_eq!(m.accesses.get(), 0);
}
