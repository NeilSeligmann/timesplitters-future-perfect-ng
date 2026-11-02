// SPDX-License-Identifier: GPL-3.0-or-later
//! Readable pad-axis deadzone reader, original 0x00042970 (T1960).
//! The guest tables and stack remain the ABI boundary; input ownership is separate.
use crate::{
    abi::ExtRegisters,
    c2r::rt::{RegsExt, XmmExt},
    memory::GuestMemory,
    views::generated_views::TsfpPadAxisRecordView,
};

const DISABLED: u32 = 0x6b9888;
const DIRECT_LAYOUT: u32 = 0x76b0fc;
const PLAYER_TO_PAD: u32 = 0x6b93c0;
const MAPPED_PAD_BASE: u32 = 0x6b93d0;
const DIRECT_PAD_BASE: u32 = 0x6b94c8;
const AXIS_CENTER: u32 = 0x80;
const POSITIVE_SCALE: u32 = 0x476038;
const NEGATIVE_SCALE: u32 = 0x4783a8;
const FULL_RANGE: u32 = 0x475c78;
const DISABLED_VALUE: u32 = 0x475cac;

/// Read one pad axis, apply its signed scale and deadzone, and return through x87.
/// The saved ECX stack word is observable and restored even on the disabled path.
pub fn read<M: GuestMemory>(memory: &mut M, regs: &mut ExtRegisters) {
    regs.push(memory, regs.ecx);
    regs.eax = memory.read32(DISABLED);
    if regs.eax != 0 {
        regs.fpu
            .push(f32::from_bits(memory.read32(DISABLED_VALUE)) as f64);
        regs.ecx = memory.read32(regs.esp);
        regs.esp = regs.esp.wrapping_add(4);
        return;
    }

    regs.eax = memory.read32(DIRECT_LAYOUT);
    let direct_layout = regs.eax != 0;
    regs.eax = memory.read32(regs.esp.wrapping_add(8));
    let pad_index = if direct_layout {
        regs.eax
    } else {
        regs.eax = memory.read32(regs.eax.wrapping_mul(4).wrapping_add(PLAYER_TO_PAD));
        regs.eax
    };
    let pad = TsfpPadAxisRecordView(
        pad_index
            .wrapping_mul(TsfpPadAxisRecordView::SIZE)
            .wrapping_add(if direct_layout {
                DIRECT_PAD_BASE
            } else {
                MAPPED_PAD_BASE
            }),
    );
    regs.eax = pad.0;
    regs.ecx = memory.read32(regs.esp.wrapping_add(12));
    regs.xmm[1] = crate::abi::Xmm::scalar(memory.read32(
        pad.0.wrapping_add(TsfpPadAxisRecordView::OFFSET_DEADZONE_BITS),
    ));
    regs.eax = memory.read32(
        pad.0
            .wrapping_add(regs.ecx.wrapping_mul(4))
            .wrapping_add(TsfpPadAxisRecordView::OFFSET_AXIS_ZERO),
    );
    let scale = if (regs.eax as i32) >= AXIS_CENTER as i32 {
        POSITIVE_SCALE
    } else {
        NEGATIVE_SCALE
    };
    regs.eax = regs.eax.wrapping_sub(AXIS_CENTER);
    regs.xmm[0].set_f(0, (regs.eax as i32) as f32);
    regs.xmm[0].set_f(0, regs.xmm[0].f(0) * f32::from_bits(memory.read32(scale)));
    regs.xmm[2] = crate::abi::Xmm::default();
    let deadzone = regs.xmm[1].f(0);
    let value = regs.xmm[0].f(0);
    if deadzone > 0.0 {
        regs.xmm[3] = regs.xmm[2];
        regs.xmm[3].set_f(0, regs.xmm[3].f(0) - deadzone);
        if regs.xmm[3].f(0) <= value || regs.xmm[3].f(0).is_nan() || value.is_nan() {
            if value <= deadzone || value.is_nan() || deadzone.is_nan() {
                memory.write32(regs.esp, regs.xmm[2].f(0).to_bits());
                regs.fpu
                    .push(f32::from_bits(memory.read32(regs.esp)) as f64);
                regs.ecx = memory.read32(regs.esp);
                regs.esp = regs.esp.wrapping_add(4);
                return;
            }
            regs.xmm[0].set_f(0, value - deadzone);
        } else {
            regs.xmm[0].set_f(0, value + deadzone);
        }
        regs.xmm[2] = crate::abi::Xmm::scalar(memory.read32(FULL_RANGE));
        regs.xmm[2].set_f(0, regs.xmm[2].f(0) - deadzone);
        regs.xmm[0].set_f(0, regs.xmm[0].f(0) / regs.xmm[2].f(0));
    }
    memory.write32(regs.esp, regs.xmm[0].f(0).to_bits());
    regs.fpu
        .push(f32::from_bits(memory.read32(regs.esp)) as f64);
    regs.ecx = memory.read32(regs.esp);
    regs.esp = regs.esp.wrapping_add(4);
}
