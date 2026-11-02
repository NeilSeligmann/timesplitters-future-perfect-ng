// SPDX-License-Identifier: GPL-3.0-or-later
//! Source-recovered damage/health behaviour awaiting original differential admission.
//! This module is intentionally absent from the runtime selector. Hooks must be
//! supplied by the caller; no fabricated success/no-op hook is built in.
use crate::memory::GuestMemory;
// T2058: typed views replace the ad hoc offsets of the object and character records.
#[path = "views.rs"]
mod views;
use views::generated_views::{TsfpCharacterView, TsfpObjectView};

pub trait DamageHooks<M: GuestMemory> {
    fn pending_effect(&mut self, memory: &mut M, record: u32);
    fn hit_record_enabled(&mut self, memory: &mut M, character: u32) -> bool;
    fn record_hit(&mut self, memory: &mut M, hit_state: u32, record: u32);
}
fn read_float<M: GuestMemory>(memory: &M, address: u32) -> f32 {
    f32::from_bits(memory.read32(address))
}
/// MULSS a,b with the destination operand first. x86 returns the first operand's payload
/// when both inputs are NaN and LLVM treats fmul as commutative, so a plain `a * b` can
/// pick the other payload (found by the BB490 differential, docs/t1815-ext-bridge.md).
#[cfg(target_arch = "x86_64")]
fn mulss(a: f32, b: f32) -> f32 {
    let mut result = a;
    unsafe { core::arch::asm!("mulss {r}, {b}", r = inout(xmm_reg) result, b = in(xmm_reg) b, options(pure, nomem, nostack)) };
    result
}
#[cfg(not(target_arch = "x86_64"))]
fn mulss(a: f32, b: f32) -> f32 { a * b }
fn write_float<M: GuestMemory>(memory: &mut M, address: u32, value: f32) {
    memory.write32(address, value.to_bits());
}

/// Semantic body of 0xBA230. Character is captured before hooks; victim is
/// reloaded afterward. Each store precedes later alias-sensitive reloads.
/// Does not reproduce hybrid GPR/SIMD/stack residue or SSE exception state.
pub fn apply_damage<M: GuestMemory, H: DamageHooks<M>>(memory: &mut M, hooks: &mut H, record: u32) {
    let cancel = memory.read32(0x7de45c) & 0x2000000 != 0;
    let victim = memory.read32(record);
    let character = TsfpObjectView(victim).character(memory).0;
    if cancel && victim != 0 && TsfpObjectView(victim).pending_effect(memory) != 0 {
        write_float(memory, record.wrapping_add(0x20), 0.0);
    }
    if TsfpObjectView(victim).pending_effect(memory) != 0 {
        hooks.pending_effect(memory, record);
    }
    if hooks.hit_record_enabled(memory, character) {
        hooks.record_hit(memory, character.wrapping_add(0xc68), record);
    }
    let damage = read_float(memory, record.wrapping_add(0x20));
    let armor = TsfpCharacterView(character).armor(memory);
    // Original COMISS/JB takes the damage operand on unordered comparison.
    let mut absorb = if damage < armor || damage.is_nan() || armor.is_nan() { damage } else { armor };
    if memory.read32(record.wrapping_add(0x3c)) & 0x200000 != 0 { absorb = 0.0; }
    let armor = TsfpCharacterView(character).armor(memory) - absorb;
    TsfpCharacterView(character).set_armor(memory, armor);
    let remainder = read_float(memory, record.wrapping_add(0x20)) - absorb;
    write_float(memory, record.wrapping_add(0x20), remainder);
    TsfpCharacterView(character).set_seconds_since_damage(memory, 0.0);
    let remainder_bits = memory.read32(record.wrapping_add(0x20));
    TsfpCharacterView(character).set_last_health_damage(memory, f32::from_bits(remainder_bits));
    let victim = memory.read32(record);
    let health = TsfpObjectView(victim).health(memory)
        - read_float(memory, record.wrapping_add(0x20));
    TsfpObjectView(victim).set_health(memory, health);
    let victim = memory.read32(record);
    if TsfpObjectView(victim).health(memory) < 0.0 {
        TsfpObjectView(victim).set_health(memory, 0.0);
    }
}

/// 0x2777E0 semantic term; mode table/constants remain guest data. Reads the
/// runtime signed frame count/scale rather than baking in file-image values.
/// Not a replacement for the entire 0x2784D0 entity update or its gates.
pub fn health_term<M: GuestMemory>(memory: &M, object: u32) -> f32 {
    let index = memory.read32(0x79094c).wrapping_sub(1);
    let one = read_float(memory, 0x475c78);
    let divisor = if index <= 0x50 && memory.read8(0x277840 + index) == 0 {
        read_float(memory, 0x475d18)
    } else { one };
    let frame_count = memory.read32(0x4e8cec) as i32 as f32;
    let per_frame = one / frame_count;
    let maximum = TsfpObjectView(object).max_health(memory) / divisor;
    mulss(mulss(per_frame, maximum), read_float(memory, 0x4e794c))
}

/// Closure for the 0xBD0F0 integer dispatch. The provider must execute the
/// indicated original callee and its x87 FSTP-to-f32 boundary, returning the
/// actual stored bits. No Rust f64 conversion substitutes for guest x87 state.
pub trait DamageResolveHooks<M: GuestMemory> {
    fn character_amount(&mut self, memory: &mut M, character: u32, kind: u32,
        hit: u32, scale: u32, record: u32) -> u32;
    fn generic_amount(&mut self, memory: &mut M, kind: u32, hit: u32, scale: u32) -> u32;
    fn finish(&mut self, memory: &mut M, record: u32);
}

/// Source-recovered integer routing/stores of 0xBD0F0, not a runtime admission.
/// hit is captured before the initial amount store (including alias cases).
/// Callback arguments are loaded in the original order; callback mutations
/// remain visible to subsequent stores and the final BB6C0 call.
pub fn resolve_damage<M: GuestMemory, H: DamageResolveHooks<M>>(
    memory: &mut M, hooks: &mut H, record: u32,
) {
    let hit = memory.read32(record.wrapping_add(0x24));
    memory.write32(record.wrapping_add(0x20), 0);
    if hit != u32::MAX {
        let entity = memory.read32(record);
        let class = memory.read32(entity.wrapping_add(0x20));
        let character = if class == 8 { Some(entity) }
            else if class == 0x20 {
                let character = TsfpObjectView(entity).character(memory).0;
                if character == 0 { None } else {
                    let flags = memory.read32(record.wrapping_add(0x40));
                    let owner = memory.read32(character.wrapping_add(0xec));
                    memory.write32(record.wrapping_add(0x40), flags | 8);
                    let scale = memory.read32(record.wrapping_add(0x54));
                    let owner = memory.read32(owner);
                    let kind = memory.read32(record.wrapping_add(0x18));
                    let bits = hooks.character_amount(memory, owner, kind, hit, scale, record);
                    memory.write32(record.wrapping_add(0x20), bits);
                    hooks.finish(memory, record);
                    return;
                }
            } else { None };
        let scale = memory.read32(record.wrapping_add(0x54));
        let kind = memory.read32(record.wrapping_add(0x18));
        let bits = if let Some(character) = character {
            hooks.character_amount(memory, character, kind, hit, scale, record)
        } else { hooks.generic_amount(memory, kind, hit, scale) };
        memory.write32(record.wrapping_add(0x20), bits);
    }
    hooks.finish(memory, record);
}

/// Required closure for BB490. EAX/ESI are implicit inputs for the original
/// register-convention helpers; args are in guest argument order. Implementors
/// must preserve the caller's callee-saved registers and all memory effects.
/// This semantic interface does not claim exact volatile-register residue.
pub trait CombatHooks<M: GuestMemory> {
    fn call(&mut self, memory: &mut M, target: u32, eax: u32, esi: u32, args: &[u32]) -> u32;
}
fn event<M: GuestMemory,H: CombatHooks<M>>(m:&mut M,h:&mut H,id:u32,subject:u32) {
    h.call(m,0x238510,0,0,&[id,subject,1.0f32.to_bits()]);
}

/// Source-recovered BB490 damage coordinator, EAX=record at entry. Required
/// hooks retain pending-effect, indirect damage, reaction and death closures.
/// SSE comparisons preserve unordered branch decisions, but FP exception state
/// and exact guest machine residue are pending admission.
pub fn coordinate_damage<M: GuestMemory,H: CombatHooks<M>>(m:&mut M,h:&mut H,r:u32) {
    let victim=m.read32(r);
    let character=TsfpObjectView(victim).character(m).0;
    if m.read32(0x7de30c)==8 {return;}
    if h.call(m,0xc6140,0,8,&[])!=0 {
        let current=h.call(m,0xc6140,0,8,&[]);
        if m.read32(current.wrapping_add(0x218))==0x1b {return;}
    }
    let source=m.read32(r.wrapping_add(4));
    let attacker=if source!=0 && m.read32(source.wrapping_add(0x20))==8 {
        TsfpObjectView(source).character(m).0
    } else {0};
    event(m,h,0x1b,m.read32(character.wrapping_add(8)));
    h.call(m,0xb9340,0,attacker,&[]);
    if read_float(m,r.wrapping_add(0x4c))>0.0 && read_float(m,r.wrapping_add(0x50))>0.0 {
        let second=m.read32(r.wrapping_add(0x50));
        let first=m.read32(r.wrapping_add(0x4c));
        h.call(m,0x120220,first,attacker,&[character.wrapping_add(0xc68),first,second]);
    }
    if h.call(m,0xb9430,0,r,&[])!=0 {return;}
    let source=m.read32(r.wrapping_add(4));
    let identity=if source!=0 {m.read32(source.wrapping_add(0x2c))} else {0};
    m.write32(character.wrapping_add(0x90),identity);
    let damage=read_float(m,r.wrapping_add(0x20));
    if !(damage>0.0 || damage.is_nan()) && m.read32(r.wrapping_add(0x18))==0x19 {
        let model=m.read32(character.wrapping_add(0xbf0));
        if model==0 || m.read32(model.wrapping_add(0x58))&0x40000==0 {return;}
    }
    if m.read32(character.wrapping_add(0x14))&1==0 {h.call(m,0xbb090,r,1,&[]);}
    if m.read8(character.wrapping_add(0x24))&8!=0 && m.read32(character.wrapping_add(0x44))!=1 {
        if m.read32(r.wrapping_add(0x18)) as i32>0 && attacker!=0 {
            event(m,h,0xc,m.read32(attacker.wrapping_add(8)));
        }
        return;
    }
    let mode=m.read8(0x7de455);
    if mode==0xa {
        if attacker!=0 {
            let source=m.read32(r.wrapping_add(4));
            if TsfpObjectView(source).pending_effect(m)==0 {
                let victim=m.read32(r);
                if TsfpObjectView(victim).pending_effect(m)==0 && m.read32(r.wrapping_add(0x18))!=0x15 {
                    let value=mulss(read_float(m,r.wrapping_add(0x20)),read_float(m,0x475cdc));
                    write_float(m,r.wrapping_add(0x20),value);
                }
            }
        }
        if m.read32(0x6d6134)==2 {
            let value=mulss(read_float(m,r.wrapping_add(0x20)),read_float(m,0x475db8));
            write_float(m,r.wrapping_add(0x20),value);
        }
    }
    let victim=m.read32(r);
    let callback=m.read32(victim.wrapping_add(0x19c));
    if callback!=0 {h.call(m,callback,callback,1,&[r]);}
    else if mode==0xa {h.call(m,0xba230,0,1,&[r]);}
    else {h.call(m,0xba310,r,1,&[]);}
    if m.read32(character.wrapping_add(0x14))&1==0 || m.read32(character.wrapping_add(0x44))!=1 {
        h.call(m,0xba8a0,r,1,&[]);
    }
    let victim=m.read32(r);
    let health=TsfpObjectView(victim).health(m);
    let zero=read_float(m,0x475cac);
    // UCOMISS/LAHF/TEST AH,0x44/JP: PF is set (jump, skip death) when ZF and the unordered PF
    // bit agree, i.e. unordered or ordered-unequal. Only ordered *equal* (ZF alone) runs
    // BABA0. Corrected by the original-vs-Rust differential (an earlier static reading had it
    // inverted), see docs/t1815-ext-bridge.md.
    if health==zero {
        h.call(m,0xbaba0,0,1,&[r]);return;
    }
    if m.read32(character.wrapping_add(8)) as i32>=m.read32(0x790950) as i32 {
        let flags=m.read32(r.wrapping_add(0x48));
        h.call(m,0x160020,flags,1,&[victim,flags,r]);
    } else if m.read32(r.wrapping_add(4))!=0 && TsfpObjectView(victim).pending_effect(m)!=0 {
        h.call(m,0x25b350,0,1,&[r]);
    }
}

/// Extra guest-stack control for roots with their own frame (BABA0). `esp` is the guest stack
/// pointer the next callee is entered with. Pushes are plain stores below it.
pub trait DeathHooks<M: GuestMemory>: CombatHooks<M> {
    fn esp(&self) -> u32;
    fn set_esp(&mut self, value: u32);
}
fn push_word<M: GuestMemory, H: DeathHooks<M>>(m: &mut M, h: &mut H, value: u32) {
    let esp = h.esp().wrapping_sub(4);
    m.write32(esp, value);
    h.set_esp(esp);
}
/// ADDSS a,b with the destination operand first (NaN payload rule, see `mulss`).
#[cfg(target_arch = "x86_64")]
fn addss(a: f32, b: f32) -> f32 {
    let mut result = a;
    unsafe { core::arch::asm!("addss {r}, {b}", r = inout(xmm_reg) result, b = in(xmm_reg) b, options(pure, nomem, nostack)) };
    result
}
#[cfg(not(target_arch = "x86_64"))]
fn addss(a: f32, b: f32) -> f32 { a + b }
const ONE_BITS: u32 = 0x3f800000;

/// Source-recovered 0xBABA0 death handler (cdecl, record at the first stack argument, ESI =
/// record inside). The wrapper has already built the original frame (`sub esp,0x104`, push
/// EBP, push ESI). EDI is pushed here exactly where the original does, so every callee sees
/// the original ESP and stack words. Callees run as the retained originals. Does not model
/// callee-visible EAX/ECX/EDX/EBP/EDI residue (only ESI = record is passed) or XMM exit state.
/// Branch conditions follow the lift (cmp/test/comiss with unordered operands), and fields are
/// re-read wherever the original re-reads them, because callees can mutate them.
pub fn handle_death<M: GuestMemory, H: DeathHooks<M>>(m: &mut M, h: &mut H, r: u32, entry_edi: u32) {
    fn call<M: GuestMemory, H: DeathHooks<M>>(m: &mut M, h: &mut H, r: u32, target: u32, args: &[u32]) -> u32 {
        h.call(m, target, 0, r, args)
    }
    fn event<M: GuestMemory, H: DeathHooks<M>>(m: &mut M, h: &mut H, r: u32, id: u32, subject: u32) {
        call(m, h, r, 0x238510, &[id, subject, ONE_BITS]);
    }
    // [[record+4]+0x150], the killer's player slot, re-read at each use as the original does.
    fn killer_player<M: GuestMemory>(m: &M, r: u32) -> u32 {
        TsfpObjectView(m.read32(r.wrapping_add(4))).pending_effect(m)
    }
    let victim = m.read32(r);
    let character = TsfpObjectView(victim).character(m).0;
    if m.read8(character.wrapping_add(0x24)) & 8 != 0 {
        let controller = TsfpObjectView(victim).pending_effect(m);
        if controller == 0 || m.read32(controller.wrapping_add(0x110)) != 4
            || m.read32(controller.wrapping_add(0xc)) & 0x800 != 0 { return; }
        call(m, h, r, 0xd5480, &[r]);
        return;
    }
    let counter = m.read32(victim.wrapping_add(0x258));
    if call(m, h, r, 0x35cc10, &[r, counter]) == 0 { return; }
    if call(m, h, r, 0x3533f0, &[]) != 0 { call(m, h, r, 0x17bf70, &[character]); }
    if m.read8(0x7de458) & 0x40 != 0 {
        let owner = m.read32(character);
        if owner != 0 && TsfpObjectView(owner).pending_effect(m) == 0 { call(m, h, r, 0x22dd10, &[r]); }
    }
    TsfpCharacterView(character).set_armor(m, 0.0);
    let health_owner = m.read32(r);
    push_word(m, h, entry_edi);
    TsfpObjectView(health_owner).set_health(m, 0.0);
    let frame = h.esp();
    let local = frame.wrapping_add(0xc);
    if call(m, h, r, 0x278040, &[character]) == 0 {
        let model = m.read32(character.wrapping_add(0xbf0));
        if m.read32(model.wrapping_add(0x28)) == 0x34574159 { call(m, h, r, 0xb9d00, &[character]); }
    } else {
        let source = m.read32(r.wrapping_add(4));
        let id = if source == 0 || TsfpObjectView(source).pending_effect(m) == 0 { 0x1646 } else { 0x1645 };
        let name = m.read32(character.wrapping_add(0x70));
        push_word(m, h, name);
        let format = call(m, h, r, 0x3d7b0, &[id]);
        h.set_esp(frame);
        let buffer = frame.wrapping_add(0x10);
        call(m, h, r, 0x3c85d4, &[buffer, format, name]);
        let mut index = 0u32;
        let mut count = call(m, h, r, 0x1cdae0, &[]) as i32;
        if count > 0 {
            loop {
                call(m, h, r, 0x12d350, &[index, 4, buffer, 0x3fcccccd]);
                index = index.wrapping_add(1);
                count = call(m, h, r, 0x1cdae0, &[]) as i32;
                if !((index as i32) < count) { break; }
            }
        }
        let flags = m.read32(0x7de45c) | 0x10000;
        m.write32(0x7de45c, flags);
        call(m, h, r, 0x67450, &[0]);
    }
    if m.read8(0x7de455) != 0xa { call(m, h, r, 0x165010, &[r]); }
    let source = m.read32(r.wrapping_add(4));
    if source != 0 && m.read32(source.wrapping_add(0x20)) == 8 && source != m.read32(r) {
        let killer = TsfpObjectView(source).character(m).0;
        let ammo = m.read32(character.wrapping_add(8));
        call(m, h, r, 0x644d0, &[killer, ammo, 1]);
        if killer_player(m, r) != 0 {
            let player = killer_player(m, r);
            if m.read32(r.wrapping_add(0x18)) == 0 { event(m, h, r, 0x5f, m.read32(player)); }
            if m.read8(0x7de455) == 9 && m.read8(0x7de456) != 0xa {
                let frames = m.read32(0x4e8cec);
                let slot = m.read32(killer_player(m, r).wrapping_add(4));
                call(m, h, r, 0x12e3b0, &[slot, frames.wrapping_mul(2).wrapping_add(0x40)]);
            }
            if m.read8(0x7de458) & 0x40 != 0 && m.read32(0x7de470) == 0x13 {
                call(m, h, r, 0x22c3a0, &[m.read32(r.wrapping_add(0xc)), character]);
            } else {
                call(m, h, r, 0xcf440, &[killer_player(m, r), character]);
            }
            let player = killer_player(m, r);
            if m.read8(player.wrapping_add(0xc)) & 0x20 != 0 { event(m, h, r, 0x48, m.read32(player)); }
            let target = m.read32(r.wrapping_add(4));
            if 0.0 >= TsfpObjectView(target).health(m) {
                let slot = TsfpObjectView(target).pending_effect(m);
                event(m, h, r, 0x5e, m.read32(slot));
            } else {
                let from = m.read32(r);
                let to = m.read32(r.wrapping_add(4));
                let diff = |offset: u32| read_float(m, from.wrapping_add(offset)) - read_float(m, to.wrapping_add(offset));
                let (dx, dy, dz) = (diff(0x68), diff(0x64), diff(0x6c));
                let squared = addss(addss(mulss(dz, dz), mulss(dx, dx)), mulss(dy, dy));
                write_float(m, local, squared);
                if read_float(m, 0x47d580) > squared {
                    let slot = TsfpObjectView(target).pending_effect(m);
                    event(m, h, r, 0x5b, m.read32(slot));
                }
                let squared = read_float(m, local);
                if squared > read_float(m, 0x47d5d4) {
                    let slot = killer_player(m, r);
                    let id = if m.read8(slot.wrapping_add(0xc)) & 0x20 != 0 { 0x5d } else { 0x5c };
                    event(m, h, r, id, m.read32(slot));
                }
            }
        }
        call(m, h, r, 0x12dfb0, &[killer]);
        if m.read32(r.wrapping_add(0x18)) == 0 {
            let slot = TsfpObjectView(m.read32(r)).pending_effect(m);
            if slot != 0 { event(m, h, r, 0x60, m.read32(slot)); }
        }
        let source = m.read32(r.wrapping_add(4));
        let weapon = m.read32(r.wrapping_add(8));
        if source != 0 && weapon != 0 && m.read32(weapon.wrapping_add(0x20)) == 0x400000
            && m.read32(source.wrapping_add(0x20)) == 8 {
            let slot = TsfpObjectView(source).pending_effect(m);
            if slot != 0 {
                event(m, h, r, 0x57, m.read32(slot));
                if m.read8(0x7de458) & 1 != 0
                    && m.read32(killer.wrapping_add(0x84)) == m.read32(character.wrapping_add(0x84)) {
                    event(m, h, r, 0x59, m.read32(killer_player(m, r)));
                }
            }
            let slot = TsfpObjectView(m.read32(r)).pending_effect(m);
            if slot != 0 { event(m, h, r, 0x58, m.read32(slot)); }
        }
        if m.read8(0x7de456) == 9 {
            let object = m.read32(m.read32(r.wrapping_add(4)).wrapping_add(0x7c));
            let mut value = addss(read_float(m, object.wrapping_add(0xa88)), read_float(m, 0x475cd4));
            let one = read_float(m, 0x475c78);
            write_float(m, object.wrapping_add(0xa88), value);
            if value > one { value = one; }
            write_float(m, object.wrapping_add(0xa88), value);
        }
    } else {
        if m.read8(0x7de456) == 0xc {
            call(m, h, r, 0x1252c0, &[character.wrapping_add(0xc68), ONE_BITS, source]);
        }
        let source = m.read32(r.wrapping_add(4));
        let first = if source == 0 || m.read32(source.wrapping_add(0x20)) == 8 { character } else { 0 };
        call(m, h, r, 0x644d0, &[first, m.read32(character.wrapping_add(8)), 1]);
        if m.read8(0x7de455) == 9 && m.read8(0x7de456) != 0xa {
            let frames = m.read32(0x4e8cec);
            let source = m.read32(r.wrapping_add(4));
            if source != 0 {
                let slot = TsfpObjectView(source).pending_effect(m);
                if slot != 0 {
                    call(m, h, r, 0x12e3b0, &[m.read32(slot.wrapping_add(4)), frames.wrapping_mul(2).wrapping_add(0x40)]);
                }
            }
        }
    }
    h.set_esp(frame.wrapping_add(4)); // pop EDI
    let slot = TsfpObjectView(m.read32(r)).pending_effect(m);
    call(m, h, r, if slot == 0 { 0x15db20 } else { 0xd5480 }, &[r]);
    if m.read8(0x7de456) == 0xa { call(m, h, r, 0x2b24e0, &[m.read32(character)]); }
    let owner = m.read32(r);
    let deaths = m.read32(owner.wrapping_add(0x258));
    m.write32(owner.wrapping_add(0x258), deaths.wrapping_add(1));
}
