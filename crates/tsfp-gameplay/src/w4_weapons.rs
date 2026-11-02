// SPDX-License-Identifier: GPL-3.0-or-later
//! T1895 wave 4 gameplay/weapons ports (Tier 0): translated from the lifted original, nested guest
//! calls over the T1815 closure model. Callee contracts are read from the call sites.
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu, guest_call, stack_arg},
    memory::GuestMemory,
};

const TICK_SCALE: u32 = 0x004e_794c;
const TICK_COUNT: u32 = 0x004e_8cec;
const FRAME_UNIT: u32 = 0x0047_5c78;

fn f32_at<M: GuestMemory>(m: &M, address: u32) -> f32 {
    f32::from_bits(m.read32(address))
}

/// Seconds-per-frame step `[4E794C] / (float)[4E8CEC]`.
/// x86 cvttss2si: out of range and NaN give the integer indefinite 0x80000000 (Rust `as` saturates).
fn cvttss2si(value: f32) -> u32 {
    if value.is_nan() || value >= 2147483648.0 || value < -2147483648.0 {
        0x8000_0000
    } else {
        value as i32 as u32
    }
}

fn tick_step<M: GuestMemory>(m: &M) -> f32 {
    f32_at(m, TICK_SCALE) / (m.read32(TICK_COUNT) as i32 as f32)
}

/// Effect strength `([475C78] / (float)[4E8CEC]) * [4E794C]`, passed to the effect starters.
fn effect_strength<M: GuestMemory>(m: &M) -> u32 {
    ((f32_at(m, FRAME_UNIT) / (m.read32(TICK_COUNT) as i32 as f32)) * f32_at(m, TICK_SCALE))
        .to_bits()
}

// portreg: va=0x000d9030
/// 000D9030 game_player_record_decay_list_timers_at_offset_0x304_tail_call. cdecl(1): replaces its
/// stack argument (a player) with `player.record + 0x304` and tail-jumps to DB4C0.
pub fn record_decay_list_timers<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let player = stack_arg(m, cpu, 0);
    let record = m.read32(player.wrapping_add(0x7c));
    m.write32(cpu.esp.wrapping_add(4), record.wrapping_add(0x304));
    // Tail jump: the callee returns straight to our caller, so the call is made in place.
    c.call(m, 0x000d_b4c0, cpu);
}

// portreg: va=0x000d8e30
/// 000D8E30 game_player_frame_update_decay_timers_add_stats_and_run_weapon_timers. cdecl(1).
pub fn player_frame_update<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let player = stack_arg(m, cpu, 0);
    let record = m.read32(player.wrapping_add(0x7c));
    guest_call(c, m, cpu, 0x000c_63d0, 0x000d_8e3e, 8, &[]);
    if cpu.eax == 9 {
        return;
    }
    guest_call(c, m, cpu, 0x001c_eba0, 0x000d_8e4c, 8, &[]);
    if cpu.eax != 0 {
        return;
    }
    let slot = m.read32(record.wrapping_add(8));
    guest_call(c, m, cpu, 0x0035_68c0, 0x000d_8e5d, 8, &[slot]);
    if cpu.eax == 0 {
        return;
    }
    for offset in [0xad4u32, 0xb40] {
        let timer = f32_at(m, record.wrapping_add(offset));
        if timer > 0.0 {
            m.write32(
                record.wrapping_add(offset),
                (timer - tick_step(m)).to_bits(),
            );
        }
    }
    guest_call(c, m, cpu, 0x000d_8c80, 0x000d_8eda, 8, &[]);
    guest_call(c, m, cpu, 0x000d_c110, 0x000d_8ee0, 8, &[player]);
    let target = m.read32(record.wrapping_add(0x94));
    let mut start_effect_3 = target != 0;
    if !start_effect_3 {
        guest_call(c, m, cpu, 0x001b_6a80, 0x000d_8ef3, 8, &[record]);
        start_effect_3 = cpu.eax != 0;
    }
    if start_effect_3 {
        let target = m.read32(record.wrapping_add(0x94));
        let slot = m.read32(record.wrapping_add(8));
        let strength = effect_strength(m);
        guest_call(
            c,
            m,
            cpu,
            0x0023_85b0,
            0x000d_8f2e,
            8,
            &[3, slot, target, strength],
        );
    }
    if m.read32(record.wrapping_add(0x80)) != 0 {
        let slot = m.read32(record.wrapping_add(8));
        let strength = effect_strength(m);
        guest_call(
            c,
            m,
            cpu,
            0x0023_8510,
            0x000d_8f68,
            8,
            &[0x25, slot, strength],
        );
    }
    if m.read32(0x007d_e30c) == 7 {
        let edi = m.read32(record.wrapping_add(0xa68));
        let effect = if edi == 1 {
            Some(0x2a)
        } else {
            guest_call(c, m, cpu, 0x0035_6940, 0x000d_8faf, 12, &[]);
            let expected = cpu.eax.wrapping_add(m.read32(0x007a_2958));
            (edi == expected).then_some(0x2b)
        };
        if let Some(code) = effect {
            let slot = m.read32(record.wrapping_add(8));
            let strength = effect_strength(m);
            guest_call(
                c,
                m,
                cpu,
                0x0023_8510,
                0x000d_8fe6,
                12,
                &[code, slot, strength],
            );
        }
    }
    guest_call(c, m, cpu, 0x001b_de40, 0x000d_8ff0, 8, &[record]);
    let slot = m.read32(record.wrapping_add(8));
    if (slot as i32) < (m.read32(0x0079_0950) as i32) {
        return;
    }
    guest_call(c, m, cpu, 0x0035_33f0, 0x000d_9005, 8, &[]);
    if cpu.eax != 0 {
        guest_call(c, m, cpu, 0x0035_8560, 0x000d_900f, 8, &[player]);
        if cpu.eax == 0 {
            return;
        }
    }
    if m.read8(record.wrapping_add(0x2c)) & 4 != 0 {
        return;
    }
    guest_call(c, m, cpu, 0x0015_eae0, 0x000d_9022, 8, &[player]);
}

/// 001C6AA0 game_weapon_fpistol_record_14_event_callback. cdecl(4): (code, owner, ?, out).
/// Event codes above 0x11 do nothing; the others go through the byte index table at 0x1C6BE4 and
/// the dword jump table at 0x1C6BC8 (read from guest memory). Returns EAX 0 or 1.
// portreg: va=0x001c6aa0
pub fn fpistol_event_callback<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let code = stack_arg(m, cpu, 0);
    let owner = stack_arg(m, cpu, 1);
    let out = stack_arg(m, cpu, 3);
    cpu.eax = 0;
    if code > 0x11 {
        return;
    }
    let index = u32::from(m.read8(code.wrapping_add(0x001c_6be4)));
    match m.read32(index.wrapping_mul(4).wrapping_add(0x001c_6bc8)) {
        0x001c_6ab7 => {
            guest_call(c, m, cpu, 0x0004_5590, 0x001c_6ac1, 0, &[0xc87]);
            cpu.eax = 0;
        }
        0x001c_6ac7 => {
            let effect = m.read32(owner.wrapping_add(0x54));
            if effect != 0 {
                cpu.eax = 0xff;
                guest_call(c, m, cpu, 0x001c_4a50, 0x001c_6ae0, 0, &[owner, effect, 3]);
            }
            cpu.eax = 1;
        }
        0x001c_6ae9 => {
            guest_call(c, m, cpu, 0x001c_4c60, 0x001c_6aee, 0, &[]);
            cpu.eax = 1;
        }
        0x001c_6af4 => {
            guest_call(c, m, cpu, 0x000f_8ae0, 0x001c_6af9, 0, &[]);
            let field = cpu.eax.wrapping_add(0x34);
            cpu.ecx = cvttss2si((m.read32(field) as i32 as f32) + f32_at(m, 0x0047_cff4));
            m.write32(field, cpu.ecx); // the original converts into ECX, then EDX below
            guest_call(c, m, cpu, 0x000f_8ae0, 0x001c_6b14, 0, &[]);
            let field = cpu.eax.wrapping_add(0x2c);
            cpu.edx = cvttss2si((m.read32(field) as i32 as f32) + f32_at(m, 0x0047_82c4));
            m.write32(field, cpu.edx);
            cpu.eax = 1;
        }
        0x001c_6b30 => {
            m.write32(out, 5);
            cpu.eax = 1;
        }
        0x001c_6b40 => {
            let state = m.read32(owner.wrapping_add(0xe8));
            let sound_owner = stack_arg(m, cpu, 2);
            let play = if state == 9 {
                Some((0xc87, 0x3f19_999au32, 0x001c_6bbf))
            } else if state == 0x15 {
                if m.read32(owner.wrapping_add(0x40)) == 0 {
                    Some((0xcbb, 0x3e19_999a, 0x001c_6ba0))
                } else if m.read32(sound_owner.wrapping_add(0xec)).wrapping_add(0xa4) == sound_owner
                {
                    Some((0xcbb, 0x3e4c_cccd, 0x001c_6b81))
                } else {
                    None
                }
            } else {
                None
            };
            if let Some((sound, fade, return_pc)) = play {
                guest_call(
                    c,
                    m,
                    cpu,
                    0x001b_8710,
                    return_pc,
                    0,
                    &[sound, sound_owner, fade, 1],
                );
            }
            cpu.eax = 1;
        }
        _ => cpu.eax = 0,
    }
}
