// SPDX-License-Identifier: GPL-3.0-or-later
//! T2195: readable presentation producer at the legacy decoder/renderer ABI edge.
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../../tsfp-gameplay/src/cutscene_movie_setup.rs"]
mod movie_setup;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../../tsfp-gameplay/src/cutscene_overlays.rs"]
mod overlay_model;
use movie_setup::{Extent, Rect};
use overlay_model::*;

fn call<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    va: u32,
    args: &[u32],
) {
    for &value in args.iter().rev() {
        regs.esp = regs.esp.wrapping_sub(4);
        m.write32(regs.esp, value);
    }
    let frame_ebp = regs.ebp;
    guest.call(va, regs);
    regs.ebp = frame_ebp;
    regs.esp = regs.esp.wrapping_add(args.len() as u32 * 4);
}
fn float<M: GuestMemory>(m: &M, address: u32) -> f32 {
    f32::from_bits(m.read32(address))
}

/// Snapshot owns the evaluated overlay values; no policy advances playback.
/// The animation sampler, subtitle callback, and rasterizer remain neighbouring contracts.
pub fn draw_original<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) {
    let entry = *regs;
    regs.esp = regs.esp.wrapping_sub(20);
    let scratch = regs.esp;
    call(m, guest, regs, 0x98da0, &[]);
    let record = regs.eax;
    let clock = if record == 0 {
        None
    } else {
        Some(PlaybackClock {
            time: float(m, record + 8),
            ending: m.read32(record + 0x1c) != 0,
            end_elapsed: float(m, record + 0x18),
            end_duration: float(m, record + 0x10),
        })
    };
    let mut sample = OverlaySample {
        strip_height: 0.0,
        gray: 0.0,
        color: 0,
        additive_color: 0,
    };
    if record != 0 {
        call(
            m,
            guest,
            regs,
            0x607a0,
            &[
                record,
                m.read32(record + 12),
                scratch,
                scratch + 4,
                scratch + 8,
                scratch + 12,
                scratch + 16,
            ],
        );
        sample = OverlaySample {
            strip_height: float(m, scratch),
            gray: float(m, scratch + 4),
            color: m.read32(scratch + 8),
            additive_color: m.read32(scratch + 12),
        };
    }
    let threshold = float(m, 0x475cac);
    let viewport = if sample.strip_height > threshold {
        let p = m.read32(0x7e71c8);
        Rect {
            left: m.read32(p) as i32,
            top: m.read32(p + 0x14) as i32,
            right: m.read32(p + 0x18) as i32,
            bottom: m.read32(p + 4) as i32,
        }
    } else {
        Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        }
    };
    let wide_display = if record == 0 {
        false
    } else {
        call(m, guest, regs, 0x3c420, &[]);
        regs.eax as u8 != 0
    };
    let flags = m.read32(0x7de458);
    let input = OverlayInput {
        clock,
        sample,
        viewport,
        interval: Extent {
            width: m.read32(0x4e8cf0) as i32,
            height: m.read32(0x4e8cf4) as i32,
        },
        wide_display,
        suppress_fallback: flags & 0x200 != 0,
        force_black: flags & 0x20000000 != 0,
        record_allows_scene: record != 0 && m.read32(record + 0x20) != 0,
        constants: OverlayConstants {
            threshold,
            opening_end: float(m, 0x475cd4),
            opening_rate: float(m, 0x475d10),
            alpha_scale: float(m, 0x475cc0),
            fixed_bar_fraction: float(m, 0x478920),
        },
    };
    let sampled_fills = usize::from(sample.color != 0) + usize::from(sample.additive_color != 0);
    let mut fills = 0;
    let mut normalised = false;
    original_overlays(&input, |command| {
        // The original resets once after sampled colour layers, before fade/bars.
        // In particular the record-less fallback inherits its caller's render state.
        if clock.is_some()
            && !normalised
            && fills >= sampled_fills
            && !matches!(
                command,
                OverlayCommand::Strips { .. } | OverlayCommand::Gray { .. }
            )
        {
            call(m, guest, regs, 0x19d70, &[1]);
            call(m, guest, regs, 0x19650, &[0]);
            normalised = true;
        }
        match command {
            OverlayCommand::Strips { height, viewport } => {
                let height = height.to_bits();
                call(
                    m,
                    guest,
                    regs,
                    0xf8bb0,
                    &[
                        1,
                        height,
                        height,
                        height,
                        height,
                        1.0f32.to_bits(),
                        0x7f7f7f3f,
                        (-6i32) as u32,
                        viewport.left as u32,
                        viewport.top as u32,
                        viewport.bottom as u32,
                        viewport.right as u32,
                    ],
                );
            }
            OverlayCommand::Gray { alpha } => call(m, guest, regs, 0xf8ed0, &[alpha as u32, 1]),
            OverlayCommand::Fill { rect, color, blend } => {
                if fills < sampled_fills && clock.is_some() {
                    call(
                        m,
                        guest,
                        regs,
                        0x19d70,
                        &[match blend {
                            Blend::Normal => 1,
                            Blend::Additive => 2,
                        }],
                    );
                    call(m, guest, regs, 0x19650, &[0]);
                }
                fills += 1;
                call(
                    m,
                    guest,
                    regs,
                    0x1efb0,
                    &[
                        rect.left as u32,
                        rect.top as u32,
                        rect.right as u32,
                        rect.bottom as u32,
                        color,
                    ],
                );
            }
            OverlayCommand::Subtitles => {
                call(m, guest, regs, 0x6eea0, &[]);
            }
        }
    });
    regs.esp = entry.esp;
    regs.ebx = entry.ebx;
    regs.ebp = entry.ebp;
    regs.esi = entry.esi;
    regs.edi = entry.edi;
}
