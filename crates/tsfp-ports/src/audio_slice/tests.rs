// SPDX-License-Identifier: GPL-3.0-or-later
// Standalone rustc harness: avoids the pre-existing full lib's w4_object closure import defect.
#![allow(dead_code, unused_imports)]
#[path = "../memory.rs"] mod memory;
#[path = "../abi.rs"] mod abi;
#[path = "../inventory.rs"] mod inventory;
#[path = "../pose.rs"] mod pose;
#[path = "../audio.rs"] mod audio;

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
use abi::{ExtRegisters,Fpu,GuestCall,Xmm};
use memory::GuestMemory;
use std::{collections::BTreeMap,cell::Cell};
#[derive(Default)] struct Mem { data:BTreeMap<u32,u8>, reads:Cell<usize> }
impl GuestMemory for Mem {
 fn read8(&self,a:u32)->u8 { let n=self.reads.get()+1; self.reads.set(n); assert!(n<100000,"bounded smoke memory budget"); *self.data.get(&a).unwrap_or(&0) }
 fn write8(&mut self,a:u32,v:u8){ self.data.insert(a,v); }
}
#[derive(Default)] struct Guest { seen:Vec<u32> }
impl GuestCall for Guest {
 fn call(&mut self,va:u32,r:&mut ExtRegisters) { assert!(self.seen.len()<1000,"bounded smoke call budget"); self.seen.push(va); r.eax=0; }
}
fn regs()->ExtRegisters {ExtRegisters {eax:0,ecx:0,edx:0,esp:0x10000,ebx:0,ebp:0,esi:0,edi:0,eflags:0,fpu:Fpu{stack:[0.0;8],top:0,control:0x27f},xmm:[Xmm::default();5],xmm_hi:[Xmm::default();3]}}
#[test] fn disabled_boundary_is_inert() {
 let mut m=Mem::default();let mut g=Guest::default();let mut r=regs();
 for &va in audio::ROOTS { assert!(!audio::run(va,false,&mut m,&mut g,&mut r)); }
 assert!(!audio::run(0,true,&mut m,&mut g,&mut r));
 assert!(m.data.is_empty());assert_eq!(m.reads.get(),0);assert!(g.seen.is_empty());assert_eq!(r.esp,0x10000);
}
#[test] fn bounded_all_root_smoke() {
 for &va in audio::ROOTS {
 let result=std::panic::catch_unwind(|| {let mut m=Mem::default();m.write32(0x5659f4,0x5659c4);m.write32(0x5836c0,0x583690);let mut g=Guest::default();let mut r=regs();assert!(audio::run(va,true,&mut m,&mut g,&mut r));});
 assert!(result.is_ok(), "root {va:08x} failed smoke");
 println!("SMOKE {va:08x} pass");
 }
}
#[test] fn request_wrapper_preserves_callee_fp_and_argument_packet() {
 struct Submit { calls:Vec<u32>, packet:u32 }
 impl GuestCall for Submit {
  fn call(&mut self,va:u32,r:&mut ExtRegisters) {
   self.calls.push(va);
   if va==0x45820 {self.packet=r.esp; r.eax=0x1234;}
   r.fpu.stack[0]=42.5; r.fpu.control=0x37f;
  }
 }
 let mut m=Mem::default();let mut g=Submit{calls:vec![],packet:0};let mut r=regs();
 m.write32(r.esp+4,0x5678);m.write32(r.esp+8,1.5f32.to_bits());
 audio::f218d0(&mut m,&mut g,&mut r);
 assert_eq!(g.calls,vec![0x45390,0x45820]);
 let packet=m.read32(g.packet);
 assert_eq!(m.read32(packet),0x5678);
 assert_eq!(m.read32(packet+0x10)&0x200,0x200);
 assert_eq!(m.read32(packet+0x18),1.5f32.to_bits());
 assert_eq!(r.eax,0x1234);assert_eq!(r.esp,0x10000);
 assert_eq!(r.fpu.stack[0],42.5);assert_eq!(r.fpu.control,0x37f);
}
