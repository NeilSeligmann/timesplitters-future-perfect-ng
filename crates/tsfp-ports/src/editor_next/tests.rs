// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(dead_code, unused_imports)]
use crate::{
    abi::{self, ExtRegisters, Fpu, GuestCall, Xmm},
    editor_next_reference as editor_next,
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
fn smoke_2fbf70() {
    editor_next::f2fbf70(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fc0d0() {
    editor_next::f2fc0d0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fc130() {
    editor_next::f2fc130(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fc190() {
    editor_next::f2fc190(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fc200() {
    editor_next::f2fc200(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fc2a0() {
    editor_next::f2fc2a0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fc340() {
    editor_next::f2fc340(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fc3d0() {
    editor_next::f2fc3d0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fc420() {
    editor_next::f2fc420(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fc470() {
    editor_next::f2fc470(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fd9a0() {
    editor_next::f2fd9a0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fda20() {
    editor_next::f2fda20(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fddf0() {
    editor_next::f2fddf0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fde80() {
    editor_next::f2fde80(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fdf10() {
    editor_next::f2fdf10(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fdf20() {
    editor_next::f2fdf20(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fe0c0() {
    editor_next::f2fe0c0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fe0f0() {
    editor_next::f2fe0f0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fe160() {
    editor_next::f2fe160(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fe420() {
    editor_next::f2fe420(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fe430() {
    editor_next::f2fe430(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fe480() {
    editor_next::f2fe480(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fe520() {
    editor_next::f2fe520(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fe570() {
    editor_next::f2fe570(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fe610() {
    editor_next::f2fe610(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fe700() {
    editor_next::f2fe700(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fe730() {
    editor_next::f2fe730(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fe780() {
    editor_next::f2fe780(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fe7a0() {
    editor_next::f2fe7a0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fe940() {
    editor_next::f2fe940(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fe9a0() {
    editor_next::f2fe9a0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fea00() {
    editor_next::f2fea00(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2feb50() {
    editor_next::f2feb50(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2febb0() {
    editor_next::f2febb0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fee60() {
    editor_next::f2fee60(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2ff9d0() {
    editor_next::f2ff9d0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2ffa50() {
    editor_next::f2ffa50(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2ffac0() {
    editor_next::f2ffac0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2ffc40() {
    editor_next::f2ffc40(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2ffd50() {
    editor_next::f2ffd50(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2ffe10() {
    editor_next::f2ffe10(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_2fff70() {
    editor_next::f2fff70(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3000a0() {
    editor_next::f3000a0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3005f0() {
    editor_next::f3005f0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_300630() {
    editor_next::f300630(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_300670() {
    editor_next::f300670(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3031b0() {
    editor_next::f3031b0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_303350() {
    editor_next::f303350(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_303430() {
    editor_next::f303430(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_303460() {
    editor_next::f303460(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3037a0() {
    editor_next::f3037a0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_303a30() {
    editor_next::f303a30(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_303b10() {
    editor_next::f303b10(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_303c50() {
    editor_next::f303c50(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_303d00() {
    editor_next::f303d00(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_303e10() {
    editor_next::f303e10(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_304330() {
    editor_next::f304330(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_304810() {
    editor_next::f304810(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_304c30() {
    editor_next::f304c30(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3051f0() {
    editor_next::f3051f0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3058b0() {
    editor_next::f3058b0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3063c0() {
    editor_next::f3063c0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_307cf0() {
    editor_next::f307cf0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_307da0() {
    editor_next::f307da0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_307ec0() {
    editor_next::f307ec0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_307ef0() {
    editor_next::f307ef0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_307fd0() {
    editor_next::f307fd0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_308070() {
    editor_next::f308070(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3080b0() {
    editor_next::f3080b0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_308120() {
    editor_next::f308120(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3081e0() {
    editor_next::f3081e0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_308260() {
    editor_next::f308260(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_308340() {
    editor_next::f308340(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3083c0() {
    editor_next::f3083c0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_308420() {
    editor_next::f308420(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_308460() {
    editor_next::f308460(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_308480() {
    editor_next::f308480(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_308560() {
    editor_next::f308560(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_308aa0() {
    editor_next::f308aa0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_308ac0() {
    editor_next::f308ac0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_308af0() {
    editor_next::f308af0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_308eb0() {
    editor_next::f308eb0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30ad90() {
    editor_next::f30ad90(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30adf0() {
    editor_next::f30adf0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_30ae70() {
    editor_next::f30ae70(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_311900() {
    editor_next::f311900(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3119b0() {
    editor_next::f3119b0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_311fb0() {
    editor_next::f311fb0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_312570() {
    editor_next::f312570(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_312810() {
    editor_next::f312810(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_312880() {
    editor_next::f312880(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_312950() {
    editor_next::f312950(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3129d0() {
    editor_next::f3129d0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_312d20() {
    editor_next::f312d20(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_312e50() {
    editor_next::f312e50(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_314210() {
    editor_next::f314210(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3143c0() {
    editor_next::f3143c0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_314b20() {
    editor_next::f314b20(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_314c90() {
    editor_next::f314c90(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_314d60() {
    editor_next::f314d60(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_314d70() {
    editor_next::f314d70(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_314dd0() {
    editor_next::f314dd0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_314e00() {
    editor_next::f314e00(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3157a0() {
    editor_next::f3157a0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3158c0() {
    editor_next::f3158c0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_315960() {
    editor_next::f315960(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3159c0() {
    editor_next::f3159c0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_315d10() {
    editor_next::f315d10(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_316200() {
    editor_next::f316200(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3162b0() {
    editor_next::f3162b0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3162e0() {
    editor_next::f3162e0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3163d0() {
    editor_next::f3163d0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_316920() {
    editor_next::f316920(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_316d10() {
    editor_next::f316d10(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_316d50() {
    editor_next::f316d50(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_316d80() {
    editor_next::f316d80(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_316fe0() {
    editor_next::f316fe0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3170a0() {
    editor_next::f3170a0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_317750() {
    editor_next::f317750(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_317780() {
    editor_next::f317780(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3177f0() {
    editor_next::f3177f0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_317900() {
    editor_next::f317900(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_317970() {
    editor_next::f317970(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_317b40() {
    editor_next::f317b40(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_317b70() {
    editor_next::f317b70(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_317b80() {
    editor_next::f317b80(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_317bd0() {
    editor_next::f317bd0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_317c00() {
    editor_next::f317c00(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_317c30() {
    editor_next::f317c30(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_317c40() {
    editor_next::f317c40(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_317d60() {
    editor_next::f317d60(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_318280() {
    editor_next::f318280(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3182b0() {
    editor_next::f3182b0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_318af0() {
    editor_next::f318af0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_318b60() {
    editor_next::f318b60(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_318e00() {
    editor_next::f318e00(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3194c0() {
    editor_next::f3194c0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_3194e0() {
    editor_next::f3194e0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_319510() {
    editor_next::f319510(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_319b50() {
    editor_next::f319b50(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_319f40() {
    editor_next::f319f40(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31a130() {
    editor_next::f31a130(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31a1b0() {
    editor_next::f31a1b0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31a300() {
    editor_next::f31a300(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31a330() {
    editor_next::f31a330(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31a4e0() {
    editor_next::f31a4e0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31a500() {
    editor_next::f31a500(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31a710() {
    editor_next::f31a710(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31a780() {
    editor_next::f31a780(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31acd0() {
    editor_next::f31acd0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31af80() {
    editor_next::f31af80(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31b490() {
    editor_next::f31b490(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31b560() {
    editor_next::f31b560(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31b5f0() {
    editor_next::f31b5f0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31ba80() {
    editor_next::f31ba80(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31ba90() {
    editor_next::f31ba90(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31baa0() {
    editor_next::f31baa0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31bac0() {
    editor_next::f31bac0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31bb00() {
    editor_next::f31bb00(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31bb30() {
    editor_next::f31bb30(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31bb60() {
    editor_next::f31bb60(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31bdb0() {
    editor_next::f31bdb0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31bdf0() {
    editor_next::f31bdf0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31be40() {
    editor_next::f31be40(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31c480() {
    editor_next::f31c480(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31c8b0() {
    let mut m = Mem::default();
    m.write32(0x280, 1);
    let mut r = regs();
    editor_next::f31c8b0(&mut m, &mut Host::default(), &mut r);
    assert_eq!(m.read32(0x280), 0);
    assert_eq!(r.esp, 0x10000);
}
#[test]
fn smoke_31c9e0() {
    editor_next::f31c9e0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31d2c0() {
    editor_next::f31d2c0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31d2d0() {
    editor_next::f31d2d0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31d360() {
    editor_next::f31d360(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31d3a0() {
    editor_next::f31d3a0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31d4c0() {
    editor_next::f31d4c0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31d9f0() {
    editor_next::f31d9f0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31da30() {
    editor_next::f31da30(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31da50() {
    editor_next::f31da50(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31da80() {
    editor_next::f31da80(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31dab0() {
    editor_next::f31dab0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31dac0() {
    editor_next::f31dac0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31db70() {
    editor_next::f31db70(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31dda0() {
    editor_next::f31dda0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31de40() {
    editor_next::f31de40(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31de90() {
    editor_next::f31de90(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31deb0() {
    editor_next::f31deb0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31dee0() {
    editor_next::f31dee0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31df00() {
    editor_next::f31df00(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31df20() {
    editor_next::f31df20(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31df50() {
    editor_next::f31df50(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31df80() {
    editor_next::f31df80(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31e060() {
    editor_next::f31e060(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31e0f0() {
    editor_next::f31e0f0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31e160() {
    editor_next::f31e160(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31e250() {
    editor_next::f31e250(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31e2a0() {
    editor_next::f31e2a0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31e2f0() {
    editor_next::f31e2f0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31e380() {
    editor_next::f31e380(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31e3b0() {
    editor_next::f31e3b0(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31e400() {
    editor_next::f31e400(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31e490() {
    editor_next::f31e490(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31e510() {
    editor_next::f31e510(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn smoke_31e590() {
    editor_next::f31e590(&mut Mem::default(), &mut Host::default(), &mut regs());
}
#[test]
fn disabled_unknown_preserve_state() {
    let mut m = Mem::default();
    let mut h = Host::default();
    let mut r = regs();
    let old = r;
    for &va in editor_next::ROOTS {
        assert!(!editor_next::run(va, false, &mut m, &mut h, &mut r));
    }
    assert!(!editor_next::run(0, true, &mut m, &mut h, &mut r));
    assert_eq!(r, old);
    assert!(m.bytes.is_empty());
    assert_eq!(m.accesses.get(), 0);
    assert!(h.calls.is_empty());
}
