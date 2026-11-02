// SPDX-License-Identifier: GPL-3.0-or-later
// Standalone rustc harness: avoids the pre-existing full lib's w4_object closure import defect.
#![allow(dead_code, unused_imports)]
#[path = "../memory.rs"] mod memory;
#[path = "../abi.rs"] mod abi;
#[path = "../inventory.rs"] mod inventory;
#[path = "../pose.rs"] mod pose;
#[path = "../video.rs"] mod video;
#[path = "../vehicles.rs"] mod vehicles;
#[path = "../system_slice/mod.rs"] mod system_slice;
#[path = "../combat.rs"] mod combat;
#[path = "../ext.rs"] mod ext;
#[path = "../inventory_ammo.rs"] mod inventory_ammo;
#[path = "../inventory_grant.rs"] mod inventory_grant;
#[path = "../inventory_pickup.rs"] mod inventory_pickup;
#[path = "../inventory_select.rs"] mod inventory_select;
#[path = "../inventory_slots.rs"] mod inventory_slots;
#[path = "../pose_anim.rs"] mod pose_anim;
#[path = "../pose_leaf.rs"] mod pose_leaf;
#[path = "../editor.rs"] mod editor;
use abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use memory::GuestMemory;
use std::{collections::BTreeMap, cell::Cell};
#[derive(Default)] struct Mem { bytes: BTreeMap<u32,u8>, accesses: Cell<u32> }
impl GuestMemory for Mem {
fn read8(&self,a:u32)->u8 { let n=self.accesses.get()+1; self.accesses.set(n); assert!(n<100000,"fixture needs bounded structured state"); *self.bytes.get(&a).unwrap_or(&0) }
fn write8(&mut self,a:u32,v:u8) { self.bytes.insert(a,v); }
}
#[derive(Default)] struct Host { calls: Vec<u32> }
impl GuestCall for Host { fn call(&mut self,va:u32,r:&mut ExtRegisters) { assert!(self.calls.len()<256,"fixture needs scripted callee");self.calls.push(va);r.eax=0; } }
fn regs()->ExtRegisters { ExtRegisters {eax:0,ecx:0,edx:0,esp:0x10000,ebx:0,ebp:0,esi:0,edi:0,eflags:0,fpu:Fpu{stack:[0.;8],top:0,control:0x27f},xmm:[Xmm::default();5],xmm_hi:[Xmm::default();3]} }
#[test] fn smoke_2fb4c0() { editor::f2fb4c0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_306460() { editor::f306460(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_3111f0() { editor::f3111f0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_3356a0() { editor::f3356a0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_335790() { editor::f335790(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fc050() { editor::f2fc050(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_32f8e0() { editor::f32f8e0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f7ba0() { editor::f2f7ba0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_335840() { editor::f335840(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_335500() { editor::f335500(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_336840() { editor::f336840(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_336850() { editor::f336850(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_311ae0() { editor::f311ae0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_30ad30() { editor::f30ad30(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_30acb0() { editor::f30acb0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_3004e0() { editor::f3004e0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_3086b0() { editor::f3086b0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fe840() { editor::f2fe840(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_311c00() { editor::f311c00(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_308730() { editor::f308730(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_311e90() { editor::f311e90(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_804c0() { editor::f804c0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_308300() { editor::f308300(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fe800() { editor::f2fe800(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_805b0() { editor::f805b0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_321290() { editor::f321290(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_30cd40() { editor::f30cd40(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_3118f0() { editor::f3118f0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_311920() { editor::f311920(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_80500() { editor::f80500(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_80a30() { editor::f80a30(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fe2b0() { editor::f2fe2b0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_302a20() { editor::f302a20(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_311890() { editor::f311890(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_3118a0() { editor::f3118a0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_311910() { editor::f311910(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_32a5b0() { editor::f32a5b0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_336600() { editor::f336600(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_62490() { editor::f62490(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_70c50() { editor::f70c50(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_71010() { editor::f71010(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_71590() { editor::f71590(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_73c80() { editor::f73c80(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_80780() { editor::f80780(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f9e90() { struct Unlocked;impl GuestCall for Unlocked {fn call(&mut self,_:u32,r:&mut ExtRegisters){r.eax=1;}}let mut m=Mem::default();m.write32(0x53e564,1);editor::f2f9e90(&mut m,&mut Unlocked,&mut regs()); }
#[test] fn smoke_2fdb40() { editor::f2fdb40(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fe360() { editor::f2fe360(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2febf0() { editor::f2febf0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_300580() { editor::f300580(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_302a60() { editor::f302a60(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_307cc0() { editor::f307cc0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_3082b0() { editor::f3082b0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_308590() { editor::f308590(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_308690() { editor::f308690(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_3089e0() { editor::f3089e0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_308dd0() { editor::f308dd0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_308f40() { editor::f308f40(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_309050() { editor::f309050(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_311930() { editor::f311930(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_311bc0() { editor::f311bc0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_311fc0() { editor::f311fc0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_312030() { editor::f312030(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_3120e0() { editor::f3120e0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_312bc0() { editor::f312bc0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_312ca0() { editor::f312ca0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_312d60() { editor::f312d60(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_3132d0() { editor::f3132d0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_319480() { editor::f319480(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_31a280() { editor::f31a280(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_31da60() { editor::f31da60(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_31da90() { editor::f31da90(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_320b20() { editor::f320b20(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_3215b0() { let mut m=Mem::default();let mut r=regs();r.esi=0x81a8;editor::f3215b0(&mut m,&mut Host::default(),&mut r);assert_eq!(r.eax & 255,0); }
#[test] fn smoke_3218b0() { editor::f3218b0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_321c00() { editor::f321c00(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_321d10() { editor::f321d10(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_32ccb0() { editor::f32ccb0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_334eb0() { editor::f334eb0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_3356f0() { editor::f3356f0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_3366a0() { editor::f3366a0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_247a0() { editor::f247a0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_427f0() { editor::f427f0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_70d30() { editor::f70d30(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_71030() { editor::f71030(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_74a30() { editor::f74a30(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_75580() { editor::f75580(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_7a270() { editor::f7a270(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f79e0() { editor::f2f79e0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f7da0() { editor::f2f7da0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f7dc0() { editor::f2f7dc0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f7de0() { let mut m=Mem::default();let mut r=regs();r.ecx=0x20000;pose::write16(&mut m,r.ecx,1);pose::write16(&mut m,r.ecx+6,1);editor::f2f7de0(&mut m,&mut Host::default(),&mut r);assert_eq!(m.read16(0x20000),0);assert_eq!(m.read16(0x20006),0);assert_eq!(r.esp,0x10000); }
#[test] fn smoke_2f7ef0() { editor::f2f7ef0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f7f30() { editor::f2f7f30(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f8020() { editor::f2f8020(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f8040() { editor::f2f8040(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f8140() { editor::f2f8140(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f82e0() { editor::f2f82e0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f8330() { editor::f2f8330(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f8450() { editor::f2f8450(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f8610() { editor::f2f8610(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f8780() { editor::f2f8780(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f8a20() { editor::f2f8a20(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f8b40() { editor::f2f8b40(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f8bd0() { editor::f2f8bd0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f8cc0() { editor::f2f8cc0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f8d50() { editor::f2f8d50(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f8de0() { editor::f2f8de0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f9040() { editor::f2f9040(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f92e0() { editor::f2f92e0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f9320() { editor::f2f9320(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f9440() { editor::f2f9440(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f9590() { editor::f2f9590(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f95e0() { editor::f2f95e0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f9650() { editor::f2f9650(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f96b0() { editor::f2f96b0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f9920() { editor::f2f9920(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f9de0() { editor::f2f9de0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2f9f70() { editor::f2f9f70(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fa060() { editor::f2fa060(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fa6f0() { editor::f2fa6f0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fa720() { editor::f2fa720(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fafa0() { editor::f2fafa0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fb020() { editor::f2fb020(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fb090() { editor::f2fb090(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fb0d0() { editor::f2fb0d0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fb230() { editor::f2fb230(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fb3c0() { editor::f2fb3c0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fb5e0() { editor::f2fb5e0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fb6b0() { editor::f2fb6b0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fb750() { editor::f2fb750(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fb7b0() { editor::f2fb7b0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fb830() { editor::f2fb830(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fb8d0() { editor::f2fb8d0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fb930() { editor::f2fb930(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fba30() { editor::f2fba30(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fbab0() { editor::f2fbab0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fbb10() { editor::f2fbb10(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fbb40() { editor::f2fbb40(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fbbc0() { editor::f2fbbc0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fbc60() { editor::f2fbc60(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fbd00() { editor::f2fbd00(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fbd60() { editor::f2fbd60(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fbde0() { editor::f2fbde0(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fbe70() { editor::f2fbe70(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn smoke_2fbf10() { editor::f2fbf10(&mut Mem::default(), &mut Host::default(), &mut regs()); }
#[test] fn overlay_decay_clamps_and_preserves_unordered() { for (alpha,expected) in [(0.1f32,0f32),(1f32,0.75f32),(f32::NAN,f32::NAN)] {let mut m=Mem::default();m.write32(0x4e8cec,1);m.write32(0x475c78,1f32.to_bits());m.write32(0x475d10,0.25f32.to_bits());m.write32(0x7844ac,alpha.to_bits());editor::f2fb4c0(&mut m,&mut Host::default(),&mut regs());let got=f32::from_bits(m.read32(0x7844ac));if expected.is_nan(){assert!(got.is_nan());}else{assert_eq!(got,expected);}} }
#[test] fn disabled_unknown_preserve_state() { let mut m=Mem::default();let mut h=Host::default();let mut r=regs();let old=r;for &va in editor::ROOTS {assert!(!editor::run(va,false,&mut m,&mut h,&mut r));}assert!(!editor::run(0,true,&mut m,&mut h,&mut r));assert_eq!(r,old);assert!(m.bytes.is_empty());assert_eq!(m.accesses.get(),0);assert!(h.calls.is_empty());}
