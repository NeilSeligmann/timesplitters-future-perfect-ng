// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-ports/c2r/engine_object
//! T1918: second wave of animation guest adapters (shadow mode, see `anim_lane.rs`), for the `tsfp-engine-anim` modules that are `core` only:
//! `tail` (0x5ADC0), `start` (0x5A360), `advance` (0x60670, 0x60920), `queries` (0x585E0), `keyrec` (0x59A40, 0x59B00). Samples come from the lifted
//! sampler 0x59320 through `GuestCall`, constants the original reads from memory are read live (they differ from the image: `[0x4D1994]`).
//! Roots whose Rust needs `std` float math (posemix, instance, prop, bounds, aim, heading, the slerp chain) cannot be built into this `no_std`
//! crate yet: recorded as the concrete blocker in `docs/t1668-anim-tiers.md`.
use crate::views::generated_views::TsfpAnimClipHeaderView;
use crate::{
    abi::{ExtRegisters, GuestCall},
    anim_lane::{Check, shadow_regions},
    memory::GuestMemory,
};
use crate::{advance, keyrec, queries, start, tail};

pub const ROOTS: [u32; 7] = [
    0x5adc0, 0x5a360, 0x60670, 0x60920, 0x585e0, 0x59a40, 0x59b00,
];

const CLIP_TABLE: u32 = 0x007e_3954;
const POLICY_TABLE: u32 = 0x007d_e660;
const ID_SLOT_MAP: u32 = 0x007e_03c0;

pub(crate) fn arg<M: GuestMemory>(m: &M, entry: &ExtRegisters, index: u32) -> u32 {
    m.read32(entry.esp.wrapping_add(4).wrapping_add(4 * index))
}

pub(crate) fn float<M: GuestMemory>(m: &M, address: u32) -> f32 {
    f32::from_bits(m.read32(address))
}

pub(crate) fn clip_pointer<M: GuestMemory>(m: &M, slot: u32) -> u32 {
    m.read32(m.read32(CLIP_TABLE).wrapping_add(slot.wrapping_mul(4)))
}

pub(crate) fn slot_of<M: GuestMemory>(m: &M, id: u32) -> u32 {
    i16::from_le_bytes([
        m.read8(ID_SLOT_MAP.wrapping_add(id.wrapping_mul(2))),
        m.read8(ID_SLOT_MAP.wrapping_add(id.wrapping_mul(2).wrapping_add(1))),
    ]) as i32 as u32
}

pub(crate) fn clip_key_count<M: GuestMemory>(m: &M, clip: u32) -> u32 {
    m.read32(clip.wrapping_add(TsfpAnimClipHeaderView::OFFSET_KEY_COUNT))
}

pub(crate) fn clip_frame_count<M: GuestMemory>(m: &M, clip: u32) -> u32 {
    m.read32(clip.wrapping_add(TsfpAnimClipHeaderView::OFFSET_FRAME_COUNT))
}

pub(crate) fn policy<M: GuestMemory>(m: &M, slot: u32) -> (u32, u32) {
    let base = POLICY_TABLE.wrapping_add(slot.wrapping_mul(8));
    (m.read32(base), m.read32(base.wrapping_add(4)))
}

/// Calls the lifted `va` with cdecl `args` on scratch stack below the root's frame; returns the register file after the call.
pub(crate) fn call<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    entry: &ExtRegisters,
    va: u32,
    args: &[u32],
) -> ExtRegisters {
    let mut cr = *entry;
    cr.esp = entry.esp.wrapping_sub(0x60); // args (at most 7 dwords) sit below the sampler output buffer so the callee frame never overlaps it
    for (i, value) in args.iter().enumerate() {
        m.write32(cr.esp.wrapping_add(4 * i as u32), *value);
    }
    guest.call(va, &mut cr);
    cr
}

/// `0x59320(clip, node, key, out, scale)`: the position (three floats) of the sample.
pub(crate) fn sample_position<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    entry: &ExtRegisters,
    clip: u32,
    node: u32,
    key: i32,
    scale: u32,
) -> [f32; 3] {
    let out = entry.esp.wrapping_sub(0x20);
    call(
        m,
        guest,
        entry,
        0x59320,
        &[clip, node, key as u32, out, scale],
    );
    [float(m, out), float(m, out + 4), float(m, out + 8)]
}

pub(crate) fn write_float<M: GuestMemory>(m: &mut M, address: u32, value: f32) {
    m.write32(address, value.to_bits());
}

pub(crate) fn read_layer<M: GuestMemory>(m: &M, address: u32) -> start::Layer {
    start::Layer(core::array::from_fn(|i| {
        m.read32(address.wrapping_add(4 * i as u32))
    }))
}

/// Precomputed world of one `0x5A360` call.
struct StartWorld {
    slot_id: u32,
    slot_zero: u32,
    duration: Option<f32>,
    policy0: u32,
    root: [f32; 3],
}

impl start::StartEnv for StartWorld {
    fn slot_of(&self, id: u32) -> u32 {
        if id == 0 {
            self.slot_zero
        } else {
            self.slot_id
        }
    }
    fn duration(&self, _slot: u32) -> Option<f32> {
        self.duration
    }
    fn policy0(&self, _slot: u32) -> u32 {
        self.policy0
    }
    fn root_at_start(&self, _slot: u32, _scale: f32) -> [f32; 3] {
        self.root
    }
}

/// `0x5A360(id, blend, speed, scale, request, current, previous)` on `m`: the typed `start::start_animation` and its three block stores.
/// A refused start changes nothing.
pub(crate) fn start_port<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    entry: &ExtRegisters,
    request: u32,
    current: u32,
    previous: u32,
) {
    let (id, blend, speed, scale_bits) = (
        arg(m, entry, 0),
        float(m, entry.esp + 8),
        float(m, entry.esp + 12),
        arg(m, entry, 3),
    );
    let slot_id = slot_of(m, id);
    let clip = clip_pointer(m, slot_id);
    let world = StartWorld {
        slot_id,
        slot_zero: slot_of(m, 0),
        duration: (clip != 0).then(|| float(m, clip.wrapping_add(0x18))),
        policy0: policy(m, slot_id).0,
        root: sample_position(m, guest, entry, clip, 0, 0, scale_bits),
    };
    let mut req = start::Request(core::array::from_fn(|i| {
        m.read32(request.wrapping_add(4 * i as u32))
    }));
    let (mut cur, mut prev) = (read_layer(m, current), read_layer(m, previous));
    if start::start_animation(
        &world,
        id,
        blend,
        speed,
        f32::from_bits(scale_bits),
        &mut req,
        &mut cur,
        &mut prev,
    ) {
        for (address, words) in [
            (request, &req.0[..]),
            (current, &cur.0[..]),
            (previous, &prev.0[..]),
        ] {
            for (i, word) in words.iter().enumerate() {
                m.write32(address.wrapping_add(4 * i as u32), *word);
            }
        }
    }
}

pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if !ROOTS.contains(&va) {
        return false;
    }
    crate::anim_lane::guarded(va, m, guest, regs, |view, guest, regs| {
        run_inner(va, view, guest, regs)
    })
}

fn run_inner<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    let entry = *regs;
    let (a0, a1, a2) = (arg(m, &entry, 0), arg(m, &entry, 1), arg(m, &entry, 2));
    match va {
        // 0x5ADC0(state, scale, event_object): clip time step with loop / clamp and root offset across a wrap
        0x5adc0 => {
            let regions = [(a0, 22)];
            shadow_regions(
                va,
                m,
                guest,
                regs,
                &regions,
                Check::None,
                |m, guest, regs| {
                    let entry = *regs;
                    let (state, scale, event) = (a0, a1, a2);
                    let slot = m.read32(state.wrapping_add(4));
                    let clip = clip_pointer(m, slot);
                    if clip == 0 {
                        return;
                    }
                    let duration = float(m, clip.wrapping_add(0x18));
                    if !(duration > 0.0) {
                        return;
                    }
                    let (time, speed) = (
                        float(m, state.wrapping_add(8)),
                        float(m, state.wrapping_add(0x34)),
                    );
                    let (p0, p1) = policy(m, slot);
                    let rate = float(m, 0x004e_794c);
                    let per = m.read32(0x004e_8cec) as i32 as f32;
                    // T3818: the policy flag first, like the original: callers pass an uninitialised event
                    // pointer (0xCDCDCDCD) when the flag is clear, and reading it made the guard roll the port back.
                    let fires = p1 & 0x0100_0000 != 0
                        && event != 0
                        && m.read32(event.wrapping_add(0x20)) == 8;
                    let Some(step) =
                        tail::step_time_at(duration, time, speed, p0, fires, rate, per)
                    else {
                        return;
                    };
                    m.write32(state.wrapping_add(0xc), time.to_bits());
                    m.write32(state.wrapping_add(0x50), step.wrapped.unwrap_or(0));
                    m.write32(state.wrapping_add(8), step.time.to_bits());
                    // The loop events (0xDB5E0, one per forward wrap) are NOT replayed in shadow mode: the original does them and they have side effects.
                    if step.wrapped == Some(1) && p0 & 1 != 0 {
                        let last_key = clip_key_count(m, clip) as i32 - 1;
                        let (first, last) = if step.direction > 0 {
                            let last = sample_position(m, guest, &entry, clip, 0, last_key, scale);
                            (sample_position(m, guest, &entry, clip, 0, 0, scale), last)
                        } else {
                            let first = sample_position(m, guest, &entry, clip, 0, 0, scale);
                            (
                                first,
                                sample_position(m, guest, &entry, clip, 0, last_key, scale),
                            )
                        };
                        let offset = [
                            float(m, state + 0x1c),
                            float(m, state + 0x20),
                            float(m, state + 0x24),
                        ];
                        let updated =
                            tail::apply_loop_root_offset(offset, step.direction, first, last);
                        for (i, value) in updated.into_iter().enumerate() {
                            write_float(m, state + 0x1c + 4 * i as u32, value);
                        }
                    }
                },
            );
        }
        // 0x5A360(id, blend, speed, scale, request, current, previous)
        0x5a360 => {
            let (request, current, previous) =
                (arg(m, &entry, 4), arg(m, &entry, 5), arg(m, &entry, 6));
            if crate::player_wire::anim_start_authoritative(m) {
                // T2118 pass 4: active PlayerSet mode, the readable port owns the stores (record words through the owner and mirror)
                start_port(
                    &mut crate::player_wire::Wired(m),
                    guest,
                    &entry,
                    request,
                    current,
                    previous,
                );
                return true;
            }
            let regions = [(request, 6), (current, 22), (previous, 22)];
            shadow_regions(
                va,
                m,
                guest,
                regs,
                &regions,
                Check::None,
                |m, guest, regs| {
                    let entry = *regs;
                    start_port(m, guest, &entry, request, current, previous);
                },
            );
        }
        // 0x60670(owner, index, state): wrap the layer time of a clip track into the clip's range
        0x60670 => {
            let regions = [(a2.wrapping_add(8), 1)];
            shadow_regions(
                va,
                m,
                guest,
                regs,
                &regions,
                Check::None,
                |m, _guest, _regs| {
                    let slot = m.read32(a2.wrapping_add(4));
                    let clip = clip_pointer(m, slot);
                    if clip == 0 {
                        return;
                    }
                    let duration = float(m, clip.wrapping_add(0x18));
                    if !(duration > 0.0) {
                        return;
                    }
                    let owner_time = float(m, a0.wrapping_add(8));
                    let offset =
                        float(m, a0.wrapping_add(a1.wrapping_mul(0x2c)).wrapping_add(0x34));
                    let old = float(m, a2.wrapping_add(8));
                    if let Some(time) = advance::advance_time(duration, owner_time, offset, old) {
                        write_float(m, a2.wrapping_add(8), time);
                    }
                },
            );
        }
        // 0x60920(track): does the tracked position jump between the keys around its time
        0x60920 => {
            shadow_regions(va, m, guest, regs, &[], Check::Bool, |m, guest, regs| {
                let entry = *regs;
                if m.read32(0x006d_780c) == 0 || a0 == 0 {
                    regs.eax = 0;
                    return;
                }
                let frame_count = m.read32(a0) as i32;
                let time = float(m, a0.wrapping_add(8));
                let fps = float(m, 0x004d_1994);
                let jump = float(m, 0x0047_85c8);
                let tick = float(m, 0x0047_5c78) / (m.read32(0x004e_8cec) as i32 as f32);
                // the key records come from the lifted 0x606E0 (store or stream cache)
                let mut positions = [None::<advance::KeyPosition>; 3];
                let frame0 = queries::cvttss2si(time * fps);
                for (i, slot) in positions.iter_mut().enumerate() {
                    let cr = call(m, guest, &entry, 0x606e0, &[(frame0 + i as i32) as u32]);
                    *slot = (cr.eax != 0).then(|| {
                        [
                            float(m, cr.eax + 0x10),
                            float(m, cr.eax + 0x14),
                            float(m, cr.eax + 0x18),
                        ]
                    });
                }
                let result =
                    advance::position_jump_at(frame_count, time, fps, jump, tick, |frame| {
                        positions[(frame - frame0) as usize]
                    });
                regs.eax = u32::from(result.unwrap_or(false));
            });
        }
        // 0x585E0(layer, out_current, out_previous)
        0x585e0 => {
            let regions = [(a1, usize::from(a1 != 0)), (a2, usize::from(a2 != 0))];
            shadow_regions(
                va,
                m,
                guest,
                regs,
                &regions,
                Check::None,
                |m, _guest, _regs| {
                    let fps = float(m, 0x004d_1994);
                    if a1 != 0 {
                        m.write32(
                            a1,
                            queries::cvttss2si(float(m, a0.wrapping_add(8)) * fps) as u32,
                        );
                    }
                    if a2 != 0 {
                        m.write32(
                            a2,
                            queries::cvttss2si(float(m, a0.wrapping_add(0xc)) * fps) as u32,
                        );
                    }
                },
            );
        }
        // 0x59A40(object, id, out3): root displacement of a whole clip
        0x59a40 => {
            let regions = [(a2, 3)];
            shadow_regions(
                va,
                m,
                guest,
                regs,
                &regions,
                Check::None,
                |m, guest, regs| {
                    let entry = *regs;
                    let scale = if a0 != 0 && m.read32(a0.wrapping_add(0x20)) == 8 {
                        m.read32(m.read32(a0.wrapping_add(0x7c)).wrapping_add(0xb08))
                    } else {
                        m.read32(0x0047_5c78)
                    };
                    let clip = clip_pointer(m, slot_of(m, a1));
                    let first = sample_position(m, guest, &entry, clip, 0, 0, scale);
                    let last_key = clip_key_count(m, clip) as i32 - 1;
                    let last = sample_position(m, guest, &entry, clip, 0, last_key, scale);
                    let d = keyrec::root_displacement(first, last);
                    for (i, value) in d.into_iter().enumerate() {
                        write_float(m, a2.wrapping_add(4 * i as u32), value);
                    }
                },
            );
        }
        // 0x59B00(object, out_first, out_last, out_frames, axis)
        0x59b00 => {
            let (a3, a4) = (arg(m, &entry, 3), arg(m, &entry, 4));
            let regions = [(a1, 1), (a2, 1), (a3, 1)];
            shadow_regions(
                va,
                m,
                guest,
                regs,
                &regions,
                Check::None,
                |m, guest, regs| {
                    let entry = *regs;
                    let scale = if a0 != 0 && m.read32(a0.wrapping_add(0x20)) == 8 {
                        m.read32(m.read32(a0.wrapping_add(0x7c)).wrapping_add(0xb08))
                    } else {
                        m.read32(0x0047_5c78)
                    };
                    let body = m.read32(a0.wrapping_add(4));
                    let clip = clip_pointer(m, m.read32(body.wrapping_add(0xd8)));
                    let first = sample_position(m, guest, &entry, clip, 0, 0, scale);
                    let last_key = clip_key_count(m, clip) as i32 - 1;
                    let last = sample_position(m, guest, &entry, clip, 0, last_key, scale);
                    let (range, frames) =
                        keyrec::axis_range(first, last, a4, clip_frame_count(m, clip) as i32);
                    if let Some((start_value, end_value)) = range {
                        write_float(m, a1, start_value);
                        write_float(m, a2, end_value);
                    }
                    write_float(m, a3, frames);
                },
            );
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod clip_header_tests {
    use super::*;
    use core::cell::RefCell;

    struct Memory {
        reads: RefCell<alloc::vec::Vec<u32>>,
        key: u32,
        frames: u32,
    }
    impl GuestMemory for Memory {
        fn read8(&self, _: u32) -> u8 {
            panic!("unexpected byte read")
        }
        fn write8(&mut self, _: u32, _: u8) {
            panic!("unexpected byte write")
        }
        fn read32(&self, address: u32) -> u32 {
            self.reads.borrow_mut().push(address);
            match address {
                0x0000_0007 => self.key,
                0x0000_0003 => self.frames,
                _ => panic!("unexpected dword read {address:x}"),
            }
        }
    }
    #[test]
    fn clip_header_fields_read_live_with_wrapping_addresses() {
        let memory = Memory {
            reads: RefCell::new(alloc::vec::Vec::new()),
            key: 17,
            frames: 31,
        };
        let clip = 0xffff_fff7;
        assert_eq!(clip_key_count(&memory, clip), 17);
        assert_eq!(clip_frame_count(&memory, clip), 31);
        assert_eq!(&*memory.reads.borrow(), &[7, 3]);
    }
}
