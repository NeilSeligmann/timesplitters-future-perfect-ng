// SPDX-License-Identifier: GPL-3.0-or-later
//! Hand ports of the four T2049 translator-gap functions (T2123): 0x1538C0 (frame timing step, rdtsc), 0x153B00
//! (rdtsc scaled to a float global), 0x25C30 (storage free space query, `shrd`) and 0x150230 (local time to BCD,
//! 8-bit `imul`). `tools.c2rust` cannot parse their rdtsc / shift / 8-bit imul statements (docs/t2049-batch-notes.md),
//! so they are written from the lifted C and the disassembly. Registers are kept as the original leaves them so the
//! callee records of the c2r differential match (observable mode, `C2R_OBSERVABLE=1`).
//!
//! rdtsc is `xbox_ReadTimeStampCounter` of the C host (`kernel_clock_read`); the c2r harness defines a seed-derived
//! stub of the same symbol, so the lifted original and this port read the identical timestamp.
//!
//! Verification tier 1 (weak, FABRICATED inputs): differential vs the lifted original, see docs/t2049-batch-notes.md.
#![allow(clippy::all)]
use super::rt::*;

#[cfg(not(test))]
unsafe extern "C" {
    fn xbox_ReadTimeStampCounter() -> u64;
}

/// The guest's one time base (`rdtsc`).
#[cfg(not(test))]
fn read_tsc() -> u64 {
    unsafe { xbox_ReadTimeStampCounter() }
}
/// Unit tests have no C host: a fixed timestamp.
#[cfg(test)]
fn read_tsc() -> u64 {
    0x0000_0001_8000_0000
}

const FP_BIAS: u32 = 0x0047_5ccc; // float added when the low timestamp word is negative as i32 (2^32)
const TSC_DIVIDEND: i64 = 0x2bb5_c755;
const TSC_DIVISOR: u32 = 0x004e_8cec;

/// Low timestamp word as a double, the way `fild dword` + the bias fix-up leaves it: unsigned value of the word.
fn tsc_low_as_double<M: GuestMemory>(m: &M, fpu: &Fpu, low: u32) -> f64 {
    let ticks = low as i32 as f64;
    if (low as i32) < 0 {
        fp_pc(fpu, ticks + f32::from_bits(m.read32(FP_BIAS)) as f64)
    } else {
        ticks
    }
}

/// `idiv` of the fixed dividend by the divisor global: (quotient, remainder). A zero divisor is the x86 divide fault.
fn tsc_divide<M: GuestMemory>(m: &M) -> (u32, u32) {
    let divisor = m.read32(TSC_DIVISOR) as i32 as i64;
    assert!(
        divisor != 0,
        "0x153b00/0x1538c0: divide by zero ([0x4e8cec] == 0), the original faults here"
    );
    (
        (TSC_DIVIDEND / divisor) as i32 as u32,
        (TSC_DIVIDEND % divisor) as i32 as u32,
    )
}

/// Seconds-like scale: low timestamp word over `0x2BB5C755 / [0x4E8CEC]`, narrowed to f32.
fn tsc_scaled<M: GuestMemory>(m: &M, fpu: &Fpu, low: u32, quotient: u32) -> f32 {
    fp_pc(
        fpu,
        tsc_low_as_double(m, fpu, low) / (quotient as i32 as f64),
    ) as f32
}

/// 0x153AC0: set pause to 1 for a nonzero request or -1 for resume.
/// Native frame selection routes this write to typed frame state.
pub fn game_frame_pause_set_from_flag<M: GuestMemory>(m: &mut M, r: &ExtRegisters) {
    let requested = m.read32(r.esp.wrapping_add(4)) != 0;
    if crate::frame_state_ffi::native_frame_state_selected() {
        crate::frame_state_ffi::native_frame_pause().set_from_flag(requested);
    } else {
        m.write32(0x7497b4, if requested { 1 } else { u32::MAX });
    }
}

/// 0x153AF0: return whether the owned pause word is nonzero.
pub fn game_frame_pause_is_nonzero<M: GuestMemory>(m: &mut M, r: &mut ExtRegisters) {
    let pause_word = if crate::frame_state_ffi::native_frame_state_selected() {
        crate::frame_state_ffi::native_frame_pause().word()
    } else {
        m.read32(0x7497b4)
    };
    r.eax = u32::from(pause_word != 0);
}

// portreg: va=0x00153b00 tier=1
/// 0x153B00 `game_rdtsc_store_as_float_scaled_to_global`: [0x7A58D0] = f32(low32(rdtsc) / (0x2BB5C755 / [0x4E8CEC])).
/// cdecl, no arguments. EAX = quotient, EDX = remainder, ECX = quotient (the original's `pop ecx` reloads the scratch
/// slot it overwrote).
pub fn game_rdtsc_store_as_float_scaled_to_global<M: GuestMemory>(m: &mut M, r: &mut ExtRegisters) {
    r.push(m, r.ecx);
    let tsc = read_tsc();
    r.eax = tsc as u32;
    r.edx = (tsc >> 32) as u32;
    let low = r.eax;
    let (quotient, remainder) = tsc_divide(m);
    r.eax = quotient;
    r.edx = remainder;
    m.write32(r.esp, quotient);
    let scaled_sample = tsc_scaled(m, &r.fpu, low, quotient).to_bits();
    if crate::frame_state_ffi::native_frame_state_selected() {
        crate::frame_state_ffi::native_frame_timing_history().record_scaled_sample(scaled_sample);
    } else {
        m.write32(0x7a58d0, scaled_sample);
    }
    r.ecx = m.read32(r.esp);
    r.esp = r.esp.wrapping_add(4);
}

// portreg: va=0x001538c0 tier=1
/// 0x1538C0 `game_frame_timing_update_wait_vblank_step_clamped_1_to_3_and_pause_state`. Shifts the timing history
/// (floats at 0x7A58C0..0x7A58EC, ints at 0x7A58A0..0x7A58B0), waits until the vblank counter from 0x22030 passed
/// the last one, samples rdtsc into [0x7A58F0], clamps the frame step [0x4E7940] to 1..3, publishes it as the float
/// frame time [0x4E7948], runs the pause state machine on [0x7497B4] (0x455B0 / 0x455C0) and tail-jumps to 0x150190.
fn native_vblank_sample<C: GuestCall>(
    guest: &mut C,
    counter: &crate::frame_state::VblankCounter,
) -> u32 {
    // T1737/T371 measure the owner getter at 0x22030, returning to 0x153975.
    guest.frame_wait_preflight(0x0002_2030, 0x0015_3975);
    counter.delivered()
}

/// Preserve the host's measured safepoint at entry to the original frame wait.
/// The native counter loop reads its Rust owner directly, so it cannot rely on
/// the lifted 0x1538C0 entry to trigger the host's vblank delivery model.
fn frame_wait_preflight<C: GuestCall>(guest: &mut C) {
    // T1737: 0x3D370 calls 0x1538C0 at 0x3D3E6; the return site is 0x3D3EB.
    guest.frame_wait_preflight(0x0015_38c0, 0x0003_d3eb);
}

pub fn game_frame_timing_update<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    frame_wait_preflight(guest);
    r.push(m, r.ecx);
    let native_frame_state = crate::frame_state_ffi::native_frame_state_selected();
    if native_frame_state {
        let history = crate::frame_state_ffi::native_frame_timing_history();
        let prior = history.snapshot();
        r.eax = prior[1];
        r.ecx = prior[2];
        r.edx = prior[3];
        history.shift_for_frame();
        r.eax = prior[4];
        r.xmm[0] = Xmm::scalar(prior[9]);
        r.ecx = prior[3];
        r.edx = prior[4];
    } else {
        // Timing history shift (every source is read before it is overwritten).
        r.eax = m.read32(0x7a58a4);
        r.ecx = m.read32(0x7a58a8);
        r.edx = m.read32(0x7a58ac);
        m.write32(0x7a58e0, m.read32(0x7a58e4));
        m.write32(0x7a58c0, m.read32(0x7a58c4));
        m.write32(0x7a58e4, m.read32(0x7a58e8));
        m.write32(0x7a58c4, m.read32(0x7a58c8));
        m.write32(0x7a58e8, m.read32(0x7a58ec));
        m.write32(0x7a58c8, m.read32(0x7a58cc));
        let newest_float = m.read32(0x7a58f0);
        m.write32(0x7a58a0, r.eax);
        r.eax = m.read32(0x7a58b0);
        m.write32(0x7a58ec, newest_float);
        let scaled_float = m.read32(0x7a58d0);
        r.xmm[0] = Xmm::scalar(scaled_float);
        m.write32(0x7a58a4, r.ecx);
        m.write32(0x7a58a8, r.edx);
        m.write32(0x7a58ac, r.eax);
        m.write32(0x7a58cc, scaled_float);
    }
    // wait for the next vblank: frame step = vblank counter - last counter, loop while < 1
    loop {
        if crate::frame_state_ffi::native_frame_state_selected() {
            r.eax = native_vblank_sample(guest, crate::frame_state_ffi::native_vblank_counter());
        } else {
            r.call(guest, 0x22030);
        }
        r.ecx = if native_frame_state {
            crate::frame_state_ffi::native_frame_timing_history().previous_vblank()
        } else {
            m.read32(0x7a58ac)
        };
        let sampled_vblank = r.eax;
        if native_frame_state {
            crate::frame_state_ffi::native_frame_clock_state()
                .record_sampled_vblank(sampled_vblank);
        } else {
            m.write32(0x7497a8, sampled_vblank);
        }
        r.eax = sampled_vblank.wrapping_sub(r.ecx);
        m.write32(0x4e7940, r.eax);
        if (r.eax as i32) >= 1 {
            break;
        }
    }
    r.edx = if native_frame_state {
        crate::frame_state_ffi::native_frame_clock_state().sampled_vblank()
    } else {
        m.read32(0x7497a8)
    };
    r.eax = r.edx;
    r.edx = r.edx.wrapping_sub(r.ecx);
    m.write32(0x4e7940, r.edx);
    if native_frame_state {
        crate::frame_state_ffi::native_frame_timing_history().record_vblank_sample(r.eax);
    } else {
        m.write32(0x7a58b0, r.eax);
    }
    // rdtsc sample
    let tsc = read_tsc();
    r.eax = tsc as u32;
    r.edx = (tsc >> 32) as u32;
    m.write32(r.esp, r.eax);
    r.ecx = r.eax;
    let low = r.eax;
    let (quotient, remainder) = tsc_divide(m);
    r.eax = quotient;
    r.edx = remainder;
    r.edx = if native_frame_state {
        crate::frame_state_ffi::native_frame_timing_history().latest_vblank_sample()
    } else {
        m.read32(0x7a58b0)
    };
    if native_frame_state {
        crate::frame_state_ffi::native_frame_clock_state().record_raw_vblank_step(r.edx);
    } else {
        m.write32(0x7497ac, r.edx);
    }
    m.write32(r.esp, quotient);
    r.eax = m.read32(0x4e7940);
    let tsc_sample = tsc_scaled(m, &r.fpu, low, quotient).to_bits();
    if native_frame_state {
        crate::frame_state_ffi::native_frame_timing_history().record_tsc_sample(tsc_sample);
    } else {
        m.write32(0x7a58f0, tsc_sample);
    }
    if native_frame_state {
        let vblank_difference = r.eax;
        let transition = crate::frame_state_ffi::native_apply_frame_step(
            vblank_difference,
            m.read32(0x475c78),
            |sound, eax, ecx, xmm, frames| {
                r.eax = eax;
                r.ecx = ecx;
                r.edx = frames;
                r.xmm[0] = Xmm::scalar(xmm);
                let target = match sound {
                    crate::frame_state::FrameSoundTransition::Pause => 0x455b0,
                    crate::frame_state::FrameSoundTransition::Resume => 0x455c0,
                };
                r.call(guest, target);
            },
        )
        .expect("native frame state must remain selected during its timing root");
        r.eax = transition.elapsed_step;
        if transition.sound.is_none() {
            r.edx = transition.frames;
            r.ecx = transition.pause_branch_value;
            r.xmm[0] = Xmm::scalar(transition.effective_float_bits);
        } else if transition.sound == Some(crate::frame_state::FrameSoundTransition::Pause) {
            // The original pause branch clears the scalar lane after its sound call.
            r.xmm[0] = Xmm::scalar(transition.effective_float_bits);
        }
    } else {
        // Lifted-ABI fallback retained as the A/B reference path.
        if (r.eax as i32) > 3 {
            r.eax = 3;
            m.write32(0x4e7940, 3);
        } else if r.eax == 0 {
            r.eax = 1;
            m.write32(0x4e7940, 1);
        }
        r.edx = m.read32(0x7497b8).wrapping_add(1);
        let step_float = (r.eax as i32 as f32).to_bits();
        r.xmm[0].lanes[0] = step_float;
        m.write32(0x4e7948, step_float);
        m.write32(0x7497b8, r.edx);
        if (r.eax as i32) > 1 {
            m.write32(0x7497bc, m.read32(0x7497bc).wrapping_add(1));
        }
        // Pause word: -1 resumes, 0 runs, any other value pauses.
        r.ecx = m.read32(0x7497b4);
        if r.ecx == 0xffff_ffff {
            let one_scale = m.read32(0x475c78);
            r.xmm[0] = Xmm::scalar(one_scale);
            m.write32(0x4e7940, 1);
            m.write32(0x4e7944, 1);
            m.write32(0x4e7948, one_scale);
            m.write32(0x4e794c, one_scale);
            m.write32(0x7497b4, 0);
            r.call(guest, 0x455c0);
            r.eax = m.read32(0x4e7940);
        } else if r.ecx == 0 {
            m.write32(0x4e7944, r.eax);
            m.write32(0x4e794c, r.xmm[0].lanes[0]);
        } else {
            r.ecx = m.read32(0x4e7944);
            if r.ecx != 0 {
                r.call(guest, 0x455b0);
                r.eax = m.read32(0x4e7940);
            }
            r.xmm[0] = Xmm::scalar(0);
            m.write32(0x4e7944, 0);
            m.write32(0x4e794c, 0);
        }
        m.write32(0x7497b0, m.read32(0x7497b0).wrapping_add(r.eax));
    }
    r.esp = r.esp.wrapping_add(4);
    r.tail_call(guest, 0x150190);
}

/// BCD byte of the low two decimal digits of a 16-bit value, as the machine code builds it: t = v % 100,
/// q = t / 10 (multiply by 0x66666667 and shift), byte = q * 6 + t (8-bit wrap).
fn bcd_of(value: u16) -> (u8, u32, u32) {
    let t = (value % 100) as u32;
    let q = (((0x6666_6667i64 * t as i64) >> 32) as i32 >> 2) as u32;
    let byte = (q.wrapping_mul(6).wrapping_add(t)) as u8;
    (byte, t, q)
}

// portreg: va=0x00150230 tier=1
/// 0x150230 `game_local_time_encode_31day_timestamp`: reads the six time words of the 0x37F057 local time record
/// (year, month, day, hour, minute, second at +0, +2, +6, +8, +0xA, +0xC), stores each as a BCD byte in an 8-byte
/// buffer on the stack (bytes 1, 2, 3, 5, 6, 7; bytes 0 and 4 stay untouched) and calls 0x14FF30 with its address.
pub fn game_local_time_encode_bcd_timestamp<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) {
    r.esp = r.esp.wrapping_sub(0x18);
    r.eax = r.esp.wrapping_add(8);
    r.push(m, r.eax);
    r.call(guest, 0x37f057);
    // (source offset, destination byte) in the order of the machine code
    for (source, destination) in [
        (0x14u32, 1u32),
        (0x12, 2),
        (0x10, 3),
        (0xe, 5),
        (0xa, 6),
        (0x8, 7),
    ] {
        let word = m.read16(r.esp.wrapping_add(source));
        let (byte, t, q) = bcd_of(word);
        m.write8(r.esp.wrapping_add(destination), byte);
        r.ecx = t;
        r.edx = set_lo8(q, 6);
        r.eax = set_lo16(q, (q as u8 as i8 as i32).wrapping_mul(6) as u32);
        r.eax = set_lo8(r.eax, byte as u32);
    }
    r.eax = r.esp;
    r.call(guest, 0x14ff30);
    r.esp = r.esp.wrapping_add(0x18);
}

/// 0x25C30 `game_storage_free_space_query` moved to tsfp-port-data-save (T3785, rule 37); the path stays.
pub use tsfp_port_data_save::c2r::t2123_storage_query::game_storage_free_space_query;

/// Hybrid entry: true when `va` is one of the four hand-ported roots.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    match va {
        0x0015_3ac0 => game_frame_pause_set_from_flag(m, regs),
        0x0015_3af0 => game_frame_pause_is_nonzero(m, regs),
        0x0015_38c0 => game_frame_timing_update(m, guest, regs),
        0x0015_3b00 => game_rdtsc_store_as_float_scaled_to_global(m, regs),
        0x0002_5c30 => game_storage_free_space_query(m, guest, regs),
        0x0015_0230 => game_local_time_encode_bcd_timestamp(m, guest, regs),
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    struct PreflightGuest<'a> {
        counter: &'a crate::frame_state::VblankCounter,
        getters: Vec<(u32, u32)>,
    }

    impl GuestCall for PreflightGuest<'_> {
        fn frame_wait_preflight(&mut self, getter: u32, return_address: u32) {
            self.getters.push((getter, return_address));
            self.counter.record_delivery();
        }

        fn call(&mut self, _va: u32, _regs: &mut ExtRegisters) {
            unreachable!()
        }
    }

    #[test]
    fn native_vblank_sample_runs_host_preflight_before_each_owner_read() {
        let counter = crate::frame_state::VblankCounter::new();
        counter.initialize(40);
        let mut guest = PreflightGuest {
            counter: &counter,
            getters: Vec::new(),
        };

        assert_eq!(native_vblank_sample(&mut guest, &counter), 41);
        assert_eq!(native_vblank_sample(&mut guest, &counter), 42);
        assert_eq!(
            guest.getters,
            [(0x0002_2030, 0x0015_3975), (0x0002_2030, 0x0015_3975)]
        );
    }

    #[test]
    fn native_timing_root_preflights_the_measured_frame_wait_entry() {
        let counter = crate::frame_state::VblankCounter::new();
        let mut guest = PreflightGuest {
            counter: &counter,
            getters: Vec::new(),
        };

        frame_wait_preflight(&mut guest);

        assert_eq!(guest.getters, [(0x0015_38c0, 0x0003_d3eb)]);
    }

    #[test]
    fn bcd_matches_decimal_digits() {
        for value in 0..=u16::MAX {
            let (byte, t, _) = bcd_of(value);
            let t10 = (value % 100) as u8;
            assert_eq!(t, t10 as u32);
            assert_eq!(byte, ((t10 / 10) << 4) | (t10 % 10), "value {value}");
        }
    }
}
