// SPDX-License-Identifier: GPL-3.0-or-later
//! Native typed replacements for the small D3D8 render-state handlers.

use core::sync::atomic::{AtomicU32, Ordering};
use tsfp_port_runtime::memory::GuestMemory;

/// Allow state-handler replacement only for traced, explicitly selected handlers.
pub fn handler_bypass(trace_enabled: bool, native_enabled: bool, index: u32) -> bool {
    trace_enabled
        && native_enabled
        && (matches!(index, 0x89..=0x8E)
            || index == 0x94
            || index == 0x92
            || index == 0x93
            || matches!(index, 0x90 | 0x91)
            || index == 0x96
            || matches!(
                index,
                0x95 | 0x97 | 0x99 | 0x9C | 0x9D | 0x9E | 0xA0 | 0xA1 | 0xA2 | 0xA3 | 0xA4 | 0xA5
            ))
}

static SELECTORS: [AtomicU32; 24] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];

/// Cache each default-off A/B selector once per process; `probe` reads its host setting lazily.
pub fn selector_enabled(index: u32, probe: impl FnOnce(u32) -> bool) -> bool {
    let slot = match index {
        0x89 => 0,
        0x8A => 1,
        0x8B => 2,
        0x8C => 3,
        0x8D => 4,
        0x8E => 5,
        0x99 => 6,
        0x9C => 7,
        0x9D => 8,
        0x9E => 9,
        0x97 => 10,
        0x95 => 11,
        0x96 => 12,
        0x91 => 13,
        0x90 => 14,
        0x93 => 15,
        0x92 => 16,
        0x94 => 17,
        0xA3 => 18,
        0xA4 => 19,
        0xA2 => 20,
        0xA1 => 21,
        0xA0 => 22,
        0xA5 => 23,
        _ => return false,
    };
    let state = &SELECTORS[slot];
    match state.load(Ordering::Relaxed) {
        1 => false,
        2 => true,
        _ => {
            let enabled = probe(index);
            let selected = if enabled { 2 } else { 1 };
            match state.compare_exchange(0, selected, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => enabled,
                Err(value) => value == 2,
            }
        }
    }
}

/// Method writes produced by one state handler. Entries after `len` are ignored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeStateMethods {
    pub writes: [(u32, u32); 3],
    pub len: usize,
}

/// Five ordered RenderDevice calls made by the D3D8 library's ZBIAS handler.
pub fn depth_bias_methods(value: u32) -> [(u32, u32); 5] {
    // The handler treats the argument as unsigned, converts it to extended precision, negates
    // it, and rounds each stored result to f32. f64 exactly represents every u32 and preserves
    // the negative zero produced by an input of zero.
    let negated = -(value as f64);
    let bias = (negated * 0.25) as f32;
    let units = negated as f32;
    let enabled = u32::from(value != 0);
    [
        (0x0004_0384, bias.to_bits()),
        (0x0004_0388, units.to_bits()),
        (0x0004_0338, enabled),
        (0x0004_0330, enabled),
        (0x0004_0334, enabled),
    ]
}

/// Translate supported D3D8 state handlers into typed single-value RenderDevice methods.
///
/// `back_fill` and `two_sided` are the guest shadows for states `0x8C` and `0x8D`.
pub fn native_state_methods(
    index: u32,
    value: u32,
    front_fill: u32,
    back_fill: u32,
    two_sided: u32,
) -> Option<NativeStateMethods> {
    match index {
        0x89 => Some(NativeStateMethods {
            // C marks group 0x200 before emitting this method and publishing the shadow.
            writes: [(0x0004_0328, value), (0, 0), (0, 0)],
            len: 1,
        }),
        0x8A => {
            let packed = ((value >> 16) & 0xFF) | ((value & 0xFF) << 16) | (value & 0xFF00_FF00);
            Some(NativeStateMethods {
                writes: [(0x0004_02A8, packed), (0, 0), (0, 0)],
                len: 1,
            })
        }
        0x8B => Some(NativeStateMethods {
            // The C handler emits 0x0008038C (two auto-incrementing values).
            // RenderDevice represents those as count-one writes at 0x38C and 0x390.
            writes: [
                (0x0004_038C, value),
                (0x0004_0390, if two_sided != 0 { back_fill } else { value }),
                (0, 0),
            ],
            len: 2,
        }),
        0x8C => Some(NativeStateMethods {
            // This handler changes the back-fill shadow, then re-emits current front fill.
            writes: [
                (0x0004_038C, front_fill),
                (0x0004_0390, if two_sided != 0 { value } else { front_fill }),
                (0, 0),
            ],
            len: 2,
        }),
        0x8D => Some(NativeStateMethods {
            // C emits the two-sided light control, publishes state 0x8D, then re-emits
            // the current front/back fill pair. The adapter preserves that order.
            writes: [
                (0x0004_17C4, value),
                (0x0004_038C, front_fill),
                (0x0004_0390, if value != 0 { back_fill } else { front_fill }),
            ],
            len: 3,
        }),
        0x8E => Some(NativeStateMethods {
            // C emits the alpha-compare word, marks group 0x200 dirty, then stores the shadow.
            writes: [(0x0004_03A4, value), (0, 0), (0, 0)],
            len: 1,
        }),
        0x96 => Some(if value == 0 {
            NativeStateMethods {
                // The disabled path writes a single logic-op-off method.
                writes: [(0x0004_17BC, 0), (0, 0), (0, 0)],
                len: 1,
            }
        } else {
            NativeStateMethods {
                // The enabled path auto-increments from 0x17BC to 0x17C0.
                writes: [(0x0004_17BC, 1), (0x0004_17C0, value), (0, 0)],
                len: 2,
            }
        }),
        0x97 => Some(NativeStateMethods {
            // C emits the count-two shade method at 0x320 with the same value in both slots.
            // RenderDevice expresses the auto-increment as two typed single-value writes.
            writes: [(0x0004_0320, value), (0x0004_0324, value), (0, 0)],
            len: 2,
        }),
        0x9C => Some(NativeStateMethods {
            // C scales the multisample word by subtracting 0x200 before emitting method 0x1E6C.
            writes: [(0x0004_1E6C, value.wrapping_sub(0x200)), (0, 0), (0, 0)],
            len: 1,
        }),
        _ => None,
    }
}

/// Apply one native state handler in the source-ordered state/method sequence.
pub fn apply_native_state_handler<M, F>(
    index: u32,
    value: u32,
    front_fill: u32,
    back_fill: u32,
    two_sided: u32,
    memory: &mut M,
    dirty_mask: u32,
    state_shadow: u32,
    front_fill_shadow: u32,
    mut emit: F,
) -> bool
where
    M: GuestMemory,
    F: FnMut(u32, u32),
{
    if index == 0xA0 {
        // C loads the device pointer before publishing the new YUV state, then
        // assembles surface-control from the updated shadow and current device state.
        let device = memory.read32(0x003E_3F58);
        memory.write32(state_shadow, value);
        let mut word = if memory.read32(0x003E_3F40) != 0 {
            0x1010_0001
        } else {
            0x0010_0001
        };
        if memory.read32(0x003E_3F00) == 2 {
            word |= 0x1_0000;
        }
        let depth = memory.read32(device.wrapping_add(0x1A08));
        if depth != 0 {
            let format = memory.read8(depth.wrapping_add(0xD));
            if matches!(format, 0x2D | 0x2B | 0x31 | 0x2F) {
                word |= 0x1000;
            }
        }
        emit(0x0004_0290, word);
        return true;
    }
    if index == 0xA1 || index == 0xA2 {
        // Both C helpers store their shadow before rebuilding the shared cull word.
        memory.write32(state_shadow, value);
        let mut cull = u32::from(memory.read32(0x003E_3F48) != 0) << 1;
        if memory.read32(0x003E_3F44) != 0
            && (memory.read32(0x003E_3F00) == 0 || memory.read32(0x003E_3F04) == 0x1E00)
        {
            cull |= 1;
        }
        emit(0x0004_1D84, cull);
        return true;
    }
    if index == 0xA3 || index == 0xA4 || index == 0xA5 {
        // The C helpers store the selected state before recomputing two device control words.
        // Their source masks and conditional bits are local C evidence, not a REA contract.
        if index == 0xA5 {
            memory.write32(0x003E_3F54, value);
        } else {
            memory.write32(state_shadow, value);
        }
        let device = memory.read32(0x003E_3F58);
        let control_a = device.wrapping_add(0x2428);
        let mut word_a = memory.read32(control_a) & !0x8;
        if memory.read32(0x003E_3F54) != 0 {
            word_a |= 0x8;
        }
        memory.write32(control_a, word_a);

        let control_b = device.wrapping_add(0x242C);
        let mut word_b = memory.read32(control_b) & 0xE7EF_FFFF;
        if memory.read32(0x003E_3F4C) != 0 {
            word_b |= 0x0010_0000;
        }
        if memory.read32(0x003E_3F50) != 0 {
            word_b |= 0x0800_0000;
        }
        memory.write32(control_b, word_b);
        return true;
    }
    if index == 0x94 {
        // C emits one count-16 method when device +0x784 is clear. RenderDevice
        // carries those writes as ordered single-register method calls.
        if memory.read32(0x003E_3F60 + 0x784) == 0 {
            for slot in 0..16u32 {
                emit(0x0004_0A60 + slot * 4, value);
            }
        }
        memory.write32(state_shadow, value);
        return true;
    }
    if index == 0x95 {
        let render_state_shadow = 0x003E_3CC0u32 + 0x4Du32 * 4;
        for (slot, (header, method_value)) in depth_bias_methods(value).iter().copied().enumerate()
        {
            emit(header, method_value);
            // The SetRenderState dispatcher updates each shadow after its method call.
            memory.write32(render_state_shadow + slot as u32 * 4, method_value);
        }
        // State 0x95 itself is published after all five method calls.
        memory.write32(state_shadow, value);
        return true;
    }
    if index == 0x9D {
        // C reads the device's 0x964 scale plus the title's 8.0 and 0.5 constants.
        let device = memory.read32(0x003E_3F58);
        let word = point_size_word(
            value,
            memory.read32(device.wrapping_add(0x964)),
            memory.read32(0x0047_D1E4),
            memory.read32(0x0047_5CD4),
        );
        emit(0x0004_0380, word);
        memory.write32(state_shadow, value);
        return true;
    }
    if index == 0x9E {
        // C stores state 0x9E before emitting the combined 0x99/0x9E mask.
        memory.write32(state_shadow, value);
        let state_99 = memory.read32(0x003E_3F24);
        let state_98 = memory.read32(0x003E_3F20);
        let flags = memory.read32(0x003E_3F68);
        let word = multisample_mask(state_99, value, flags, state_98);
        emit(0x0004_1D7C, word);
        return true;
    }
    if index == 0x90 {
        // C publishes state 0x90 before reading the dependent cull and depth-surface state.
        memory.write32(state_shadow, value);
        let state_a2 = memory.read32(0x003E_3F48);
        let state_a1 = memory.read32(0x003E_3F44);
        let state_91 = memory.read32(0x003E_3F04);
        let cull = u32::from(state_a2 != 0) << 1
            | u32::from(state_a1 != 0 && (value == 0 || state_91 == 0x1E00));
        let device = memory.read32(0x003E_3F58);
        let depth_surface = memory.read32(device.wrapping_add(0x1A08));
        emit(0x0004_1D84, cull);
        emit(0x0004_032C, u32::from(value != 0 && depth_surface != 0));
        return true;
    }
    if index == 0x91 {
        // C publishes state 0x91 before deriving the combined cull word.
        memory.write32(state_shadow, value);
        let state_a2 = memory.read32(0x003E_3F48);
        let state_a1 = memory.read32(0x003E_3F44);
        let state_90 = memory.read32(0x003E_3F00);
        let cull = u32::from(state_a2 != 0) << 1
            | u32::from(state_a1 != 0 && (state_90 == 0 || value == 0x1E00));
        emit(0x0004_1D84, cull);
        emit(0x0004_0370, value);
        return true;
    }
    if index == 0x93 {
        // C emits the enable word first, and when enabled emits the face selector
        // chosen against the previous state 0x92 before publishing state 0x93.
        let previous_92 = memory.read32(state_shadow.wrapping_sub(4));
        emit(0x0004_0308, u32::from(value != 0));
        if value != 0 {
            emit(0x0004_039C, 0x404 + u32::from(value != previous_92));
        }
        memory.write32(state_shadow, value);
        return true;
    }
    if index == 0x92 {
        // C emits the requested state-0x92 method, publishes its shadow, then tail-calls
        // handler 0x93 with the current 0x93 value. Preserve that nested call's method order.
        let current_93 = memory.read32(state_shadow.wrapping_add(4));
        emit(0x0004_03A0, value);
        memory.write32(state_shadow, value);
        emit(0x0004_0308, u32::from(current_93 != 0));
        if current_93 != 0 {
            emit(0x0004_039C, 0x404 + u32::from(current_93 != value));
        }
        memory.write32(state_shadow.wrapping_add(4), current_93);
        return true;
    }
    if index == 0x99 {
        let state_9e = memory.read32(0x003E_3F44);
        let device_flags = memory.read32(0x003E_3F68);
        let antialias = memory.read32(0x003E_3F20);
        let word = multisample_mask(value, state_9e, device_flags, antialias);
        memory.write32(state_shadow, value);
        emit(0x0004_1D7C, word);
        return true;
    }
    let Some(methods) = native_state_methods(index, value, front_fill, back_fill, two_sided) else {
        return false;
    };
    if index == 0x89 {
        dirty_or(memory, dirty_mask, 0x200);
    }
    if index == 0x8D {
        let (header, method_value) = methods.writes[0];
        emit(header, method_value);
        if value != 0 {
            dirty_or(memory, dirty_mask, 0x9000);
        }
        memory.write32(state_shadow, value);
        for (header, method_value) in methods.writes.into_iter().take(methods.len).skip(1) {
            emit(header, method_value);
        }
        memory.write32(front_fill_shadow, front_fill);
        return true;
    }
    for (header, method_value) in methods.writes.into_iter().take(methods.len) {
        emit(header, method_value);
    }
    if index == 0x8E {
        dirty_or(memory, dirty_mask, 0x200);
    }
    memory.write32(state_shadow, value);
    true
}

fn multisample_mask(state_99: u32, sample_alpha: u32, device_flags: u32, antialias: u32) -> u32 {
    ((state_99 << 16) | sample_alpha) | u32::from(device_flags & 0x8000 != 0 && antialias != 0)
}

fn point_size_word(value: u32, scale: u32, eight: u32, half: u32) -> u32 {
    let scaled =
        f32::from_bits(value) as f64 * f32::from_bits(scale) as f64 * f32::from_bits(eight) as f64
            + f32::from_bits(half) as f64;
    let converted = scaled as f32;
    let level =
        if !converted.is_finite() || converted >= 2_147_483_648.0 || converted < -2_147_483_648.0 {
            0x8000_0000
        } else {
            // Float-to-int casts truncate toward zero, matching the guest's
            // integer conversion here and remaining available in no_std.
            (converted as i32) as u32
        };
    level.min(0x1FF)
}

fn dirty_or<M: GuestMemory>(memory: &mut M, dirty_mask: u32, bits: u32) {
    let dirty = memory.read32(dirty_mask);
    memory.write32(dirty_mask, dirty | bits);
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::vec::Vec;

    use super::{
        NativeStateMethods, apply_native_state_handler, depth_bias_methods, handler_bypass,
        multisample_mask, native_state_methods, point_size_word,
    };
    use tsfp_port_runtime::memory::GuestMemory;

    struct TestMemory([u8; 16]);

    impl GuestMemory for TestMemory {
        fn read8(&self, address: u32) -> u8 {
            self.0[address as usize]
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0[address as usize] = value;
        }
    }

    struct SparseMemory(BTreeMap<u32, u8>);

    impl GuestMemory for SparseMemory {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    #[test]
    fn handler_bypass_requires_trace_opt_in_and_supported_index() {
        assert!(!handler_bypass(false, false, 0x8A));
        assert!(!handler_bypass(false, true, 0x8B));
        assert!(!handler_bypass(true, false, 0x8A));
        assert!(!handler_bypass(true, true, 0x88));
        assert!(handler_bypass(true, true, 0x89));
        assert!(handler_bypass(true, true, 0x91));
        assert!(handler_bypass(true, true, 0x92));
        assert!(handler_bypass(true, true, 0x94));
        assert!(handler_bypass(true, true, 0x90));
        assert!(handler_bypass(true, true, 0x97));
        assert!(handler_bypass(true, true, 0x95));
        assert!(handler_bypass(true, true, 0x96));
        assert!(handler_bypass(true, true, 0x8A));
        assert!(handler_bypass(true, true, 0x99));
        assert!(handler_bypass(true, true, 0x8E));
        assert!(handler_bypass(true, true, 0x9C));
        assert!(handler_bypass(true, true, 0x9E));
        assert!(handler_bypass(true, true, 0xA3));
        assert!(handler_bypass(true, true, 0xA4));
        assert!(handler_bypass(true, true, 0xA1));
        assert!(handler_bypass(true, true, 0xA0));
        assert!(handler_bypass(true, true, 0xA2));
        assert!(!handler_bypass(true, true, 0x9B));
    }

    #[test]
    fn library_state_a2_publishes_shadow_then_emits_recomputed_cull_word() {
        let mut memory = SparseMemory(BTreeMap::new());
        memory.write32(0x003E_3F44, 1);
        memory.write32(0x003E_3F48, 0);
        memory.write32(0x003E_3F00, 0);
        memory.write32(0x003E_3F04, 0);
        let mut emitted = Vec::new();
        assert!(apply_native_state_handler(
            0xA2,
            1,
            0,
            0,
            0,
            &mut memory,
            0,
            0x003E_3F48,
            0,
            |header, value| emitted.push((header, value))
        ));
        assert_eq!(memory.read32(0x003E_3F48), 1);
        assert_eq!(emitted, [(0x0004_1D84, 3)]);

        memory.write32(0x003E_3F00, 1);
        emitted.clear();
        assert!(apply_native_state_handler(
            0xA2,
            0,
            0,
            0,
            0,
            &mut memory,
            0,
            0x003E_3F48,
            0,
            |header, value| emitted.push((header, value))
        ));
        assert_eq!(memory.read32(0x003E_3F48), 0);
        assert_eq!(emitted, [(0x0004_1D84, 0)]);
    }

    #[test]
    fn library_state_a1_publishes_shadow_then_emits_recomputed_cull_word() {
        let mut memory = SparseMemory(BTreeMap::new());
        memory.write32(0x003E_3F48, 1);
        memory.write32(0x003E_3F00, 0);
        memory.write32(0x003E_3F04, 0);
        let mut emitted = Vec::new();
        assert!(apply_native_state_handler(
            0xA1,
            1,
            0,
            0,
            0,
            &mut memory,
            0,
            0x003E_3F44,
            0,
            |header, value| emitted.push((header, value))
        ));
        assert_eq!(memory.read32(0x003E_3F44), 1);
        assert_eq!(emitted, [(0x0004_1D84, 3)]);

        memory.write32(0x003E_3F00, 1);
        emitted.clear();
        assert!(apply_native_state_handler(
            0xA1,
            0,
            0,
            0,
            0,
            &mut memory,
            0,
            0x003E_3F44,
            0,
            |header, value| emitted.push((header, value))
        ));
        assert_eq!(memory.read32(0x003E_3F44), 0);
        assert_eq!(emitted, [(0x0004_1D84, 2)]);
    }

    #[test]
    fn yuv_state_rebuilds_surface_control_from_updated_shadow_and_depth_format() {
        let mut memory = SparseMemory(BTreeMap::new());
        memory.write32(0x003E_3F58, 0x1000);
        memory.write32(0x1000 + 0x1A08, 0x2000);
        memory.write8(0x2000 + 0xD, 0x2D);
        memory.write32(0x003E_3F00, 2);
        let mut emitted = Vec::new();
        assert!(apply_native_state_handler(
            0xA0,
            1,
            0,
            0,
            0,
            &mut memory,
            0,
            0x003E_3F40,
            0,
            |header, value| emitted.push((header, value))
        ));
        assert_eq!(memory.read32(0x003E_3F40), 1);
        assert_eq!(emitted, [(0x0004_0290, 0x1011_1001)]);
    }

    #[test]
    fn yuv_state_without_depth_surface_emits_default_surface_control() {
        let mut memory = SparseMemory(BTreeMap::new());
        memory.write32(0x003E_3F58, 0x1000);
        let mut emitted = Vec::new();
        assert!(apply_native_state_handler(
            0xA0,
            0,
            0,
            0,
            0,
            &mut memory,
            0,
            0x003E_3F40,
            0,
            |header, value| emitted.push((header, value))
        ));
        assert_eq!(emitted, [(0x0004_0290, 0x0010_0001)]);
    }

    #[test]
    fn state_a3_a4_update_shadows_before_recomputed_device_control_bits() {
        let device = 0x10000;
        let mut memory = SparseMemory(BTreeMap::new());
        memory.write32(0x003E_3F58, device);
        memory.write32(0x003E_3F54, 0);
        memory.write32(device + 0x2428, u32::MAX);
        memory.write32(device + 0x242C, u32::MAX);
        let mut writes = Vec::new();

        assert!(apply_native_state_handler(
            0xA3,
            1,
            0,
            0,
            0,
            &mut memory,
            0,
            0x003E_3F4C,
            0,
            |h, v| writes.push((h, v))
        ));
        assert_eq!(memory.read32(0x003E_3F4C), 1);
        assert_eq!(memory.read32(device + 0x2428), u32::MAX & !0x8);
        assert_eq!(
            memory.read32(device + 0x242C),
            (u32::MAX & 0xE7EF_FFFF) | 0x0010_0000
        );

        assert!(apply_native_state_handler(
            0xA4,
            2,
            0,
            0,
            0,
            &mut memory,
            0,
            0x003E_3F50,
            0,
            |h, v| writes.push((h, v))
        ));
        assert_eq!(memory.read32(0x003E_3F50), 2);
        assert_eq!(
            memory.read32(device + 0x242C),
            (u32::MAX & 0xE7EF_FFFF) | 0x0010_0000 | 0x0800_0000
        );

        memory.write32(0x003E_3F54, 1);
        assert!(apply_native_state_handler(
            0xA3,
            0,
            0,
            0,
            0,
            &mut memory,
            0,
            0x003E_3F4C,
            0,
            |h, v| writes.push((h, v))
        ));
        // The control-word update clears only bit 3; all other bits from the
        // previous value remain set when the source flag turns on.
        assert_eq!(memory.read32(device + 0x2428), u32::MAX);
        assert_eq!(
            memory.read32(device + 0x242C),
            (u32::MAX & 0xE7EF_FFFF) | 0x0800_0000
        );

        assert!(apply_native_state_handler(
            0xA5,
            0,
            0,
            0,
            0,
            &mut memory,
            0,
            0,
            0,
            |h, v| writes.push((h, v))
        ));
        assert_eq!(memory.read32(0x003E_3F54), 0);
        assert_eq!(memory.read32(device + 0x2428), u32::MAX & !0x8);
        assert!(writes.is_empty());
    }

    #[test]
    fn library_state_94_writes_sixteen_ordered_constants_or_only_updates_shadow() {
        let mut memory = SparseMemory(BTreeMap::new());
        let state_shadow = 0x003E_3F10;
        let mut emitted = Vec::new();
        assert!(apply_native_state_handler(
            0x94,
            0x1234_5678,
            0,
            0,
            0,
            &mut memory,
            0,
            state_shadow,
            0,
            |header, value| emitted.push((header, value))
        ));
        assert_eq!(
            emitted,
            (0..16u32)
                .map(|slot| (0x0004_0A60 + slot * 4, 0x1234_5678))
                .collect::<Vec<_>>()
        );
        assert_eq!(memory.read32(state_shadow), 0x1234_5678);

        memory.write32(0x003E_3F60 + 0x784, 1);
        emitted.clear();
        assert!(apply_native_state_handler(
            0x94,
            0x8765_4321,
            0,
            0,
            0,
            &mut memory,
            0,
            state_shadow,
            0,
            |header, value| emitted.push((header, value))
        ));
        assert!(emitted.is_empty());
        assert_eq!(memory.read32(state_shadow), 0x8765_4321);
    }

    #[test]
    fn depth_bias_emits_five_ordered_states_and_preserves_negative_zero() {
        assert_eq!(
            depth_bias_methods(0),
            [
                (0x0004_0384, 0x8000_0000),
                (0x0004_0388, 0x8000_0000),
                (0x0004_0338, 0),
                (0x0004_0330, 0),
                (0x0004_0334, 0),
            ]
        );
        assert_eq!(depth_bias_methods(4)[0], (0x0004_0384, (-1.0f32).to_bits()));
        assert_eq!(depth_bias_methods(4)[1], (0x0004_0388, (-4.0f32).to_bits()));
        assert!(
            depth_bias_methods(4)[2..]
                .iter()
                .all(|(_, enabled)| *enabled == 1)
        );
    }

    #[test]
    fn z_bias_handler_emits_methods_then_updates_render_state_shadows() {
        let mut memory = SparseMemory(BTreeMap::new());
        let mut emitted = Vec::new();
        let state_shadow = 0x003E_3F94;
        assert!(apply_native_state_handler(
            0x95,
            4,
            0,
            0,
            0,
            &mut memory,
            0,
            state_shadow,
            0,
            |header, value| emitted.push((header, value))
        ));
        assert_eq!(emitted, depth_bias_methods(4));
        let render_state_shadow = 0x003E_3CC0u32 + 0x4Du32 * 4;
        for (slot, (_, value)) in emitted.iter().enumerate() {
            assert_eq!(memory.read32(render_state_shadow + slot as u32 * 4), *value);
        }
        assert_eq!(memory.read32(state_shadow), 4);
    }

    #[test]
    fn logic_op_state_emits_enabled_pair_or_disabled_method_before_shadow() {
        let state_shadow = 0x003E_3F18;
        let mut memory = SparseMemory(BTreeMap::new());
        let mut emitted = Vec::new();
        assert!(apply_native_state_handler(
            0x96,
            0x1234,
            0,
            0,
            0,
            &mut memory,
            0,
            state_shadow,
            0,
            |header, value| emitted.push((header, value))
        ));
        assert_eq!(emitted, [(0x0004_17BC, 1), (0x0004_17C0, 0x1234)]);
        assert_eq!(memory.read32(state_shadow), 0x1234);

        emitted.clear();
        assert!(apply_native_state_handler(
            0x96,
            0,
            0,
            0,
            0,
            &mut memory,
            0,
            state_shadow,
            0,
            |header, value| emitted.push((header, value))
        ));
        assert_eq!(emitted, [(0x0004_17BC, 0)]);
        assert_eq!(memory.read32(state_shadow), 0);
    }

    #[test]
    fn multisample_mask_combines_states_and_conditional_aa_bit() {
        assert_eq!(multisample_mask(0x1234, 0x5678, 0, 1), 0x1234_5678);
        assert_eq!(multisample_mask(0x1234, 0x5678, 0x8000, 0), 0x1234_5678);
        assert_eq!(multisample_mask(0x1234, 0x5678, 0x8000, 1), 0x1234_5679);
    }

    #[test]
    fn point_size_conversion_scales_rounds_and_clamps() {
        let eight = 8.0f32.to_bits();
        let half = 0.5f32.to_bits();
        assert_eq!(
            point_size_word(0.25f32.to_bits(), 0.5f32.to_bits(), eight, half),
            1
        );
        assert_eq!(
            point_size_word(100.0f32.to_bits(), 1.0f32.to_bits(), eight, half),
            0x1FF
        );
        assert_eq!(
            point_size_word((-1.0f32).to_bits(), 1.0f32.to_bits(), eight, half),
            0x1FF
        );
        assert_eq!(
            point_size_word(f32::NAN.to_bits(), 1.0f32.to_bits(), eight, half),
            0x1FF
        );
    }

    #[test]
    fn alpha_compare_marks_only_its_dirty_group() {
        let mut memory = TestMemory([0; 16]);
        memory.write32(0, 0x10);
        let mut emitted = Vec::new();
        assert!(apply_native_state_handler(
            0x8E,
            7,
            0,
            0,
            0,
            &mut memory,
            0,
            4,
            8,
            |h, v| {
                emitted.push((h, v));
            }
        ));
        assert_eq!(emitted, [(0x0004_03A4, 7)]);
        assert_eq!(memory.read32(0), 0x210);
        assert_eq!(memory.read32(4), 7);
    }

    #[test]
    fn stencil_enable_marks_dirty_then_emits_method_and_publishes_state() {
        let mut memory = TestMemory([0; 16]);
        memory.write32(0, 0x10);
        let mut emitted = Vec::new();
        assert!(apply_native_state_handler(
            0x89,
            9,
            0,
            0,
            0,
            &mut memory,
            0,
            4,
            8,
            |h, v| {
                emitted.push((h, v));
            }
        ));
        assert_eq!(emitted, [(0x0004_0328, 9)]);
        assert_eq!(memory.read32(0), 0x210);
        assert_eq!(memory.read32(4), 9);
    }

    #[test]
    fn two_sided_enable_emits_control_before_fill_pair_and_republishes_front() {
        let mut memory = TestMemory([0; 16]);
        let mut emitted = Vec::new();
        assert!(apply_native_state_handler(
            0x8D,
            1,
            3,
            4,
            0,
            &mut memory,
            0,
            4,
            8,
            |h, v| emitted.push((h, v)),
        ));
        assert_eq!(
            emitted,
            [(0x0004_17C4, 1), (0x0004_038C, 3), (0x0004_0390, 4)]
        );
        assert_eq!(memory.read32(0), 0x9000);
        assert_eq!(memory.read32(4), 1);
        assert_eq!(memory.read32(8), 3);
    }

    #[test]
    fn fog_colour_matches_the_original_swizzled_word() {
        assert_eq!(
            native_state_methods(0x8A, 0xAABB_CCDD, 0, 0, 0),
            Some(NativeStateMethods {
                writes: [(0x0004_02A8, 0xAADD_CCBB), (0, 0), (0, 0)],
                len: 1,
            })
        );
    }

    #[test]
    fn fill_mode_keeps_the_back_value_only_when_two_sided_is_enabled() {
        assert_eq!(
            native_state_methods(0x8B, 1, 0, 2, 0),
            Some(NativeStateMethods {
                writes: [(0x0004_038C, 1), (0x0004_0390, 1), (0, 0)],
                len: 2,
            })
        );
        assert_eq!(
            native_state_methods(0x8B, 1, 0, 2, 1),
            Some(NativeStateMethods {
                writes: [(0x0004_038C, 1), (0x0004_0390, 2), (0, 0)],
                len: 2,
            })
        );
    }

    #[test]
    fn shade_mode_emits_both_incremented_methods_then_publishes_state() {
        let mut memory = TestMemory([0; 16]);
        let mut emitted = Vec::new();
        assert!(apply_native_state_handler(
            0x97,
            0x1234,
            0,
            0,
            0,
            &mut memory,
            0,
            4,
            8,
            |header, value| emitted.push((header, value))
        ));
        assert_eq!(emitted, [(0x0004_0320, 0x1234), (0x0004_0324, 0x1234)]);
        assert_eq!(memory.read32(4), 0x1234);
    }

    #[test]
    fn back_fill_handler_reemits_front_and_selects_new_back_fill() {
        assert_eq!(
            native_state_methods(0x8C, 3, 1, 2, 0),
            Some(NativeStateMethods {
                writes: [(0x0004_038C, 1), (0x0004_0390, 1), (0, 0)],
                len: 2,
            })
        );
        assert_eq!(
            native_state_methods(0x8C, 3, 1, 2, 1),
            Some(NativeStateMethods {
                writes: [(0x0004_038C, 1), (0x0004_0390, 3), (0, 0)],
                len: 2,
            })
        );
    }

    #[test]
    fn two_sided_lighting_emits_control_then_current_fill_pair() {
        assert_eq!(
            native_state_methods(0x8D, 1, 3, 4, 0),
            Some(NativeStateMethods {
                writes: [(0x0004_17C4, 1), (0x0004_038C, 3), (0x0004_0390, 4)],
                len: 3,
            })
        );
        assert_eq!(
            native_state_methods(0x8D, 0, 3, 4, 1),
            Some(NativeStateMethods {
                writes: [(0x0004_17C4, 0), (0x0004_038C, 3), (0x0004_0390, 3)],
                len: 3,
            })
        );
    }

    #[test]
    fn alpha_compare_emits_its_typed_method() {
        assert_eq!(
            native_state_methods(0x8E, 0x1234, 0, 0, 0),
            Some(NativeStateMethods {
                writes: [(0x0004_03A4, 0x1234), (0, 0), (0, 0)],
                len: 1,
            })
        );
    }

    #[test]
    fn stencil_reference_preserves_nested_93_call_order() {
        let mut memory = SparseMemory(BTreeMap::new());
        memory.write32(0x003E_3F04, 1);
        let mut emitted = Vec::new();
        assert!(apply_native_state_handler(
            0x92,
            1,
            0,
            0,
            0,
            &mut memory,
            0,
            0x003E_3F00,
            0,
            |header, value| emitted.push((header, value))
        ));
        assert_eq!(
            emitted,
            [(0x0004_03A0, 1), (0x0004_0308, 1), (0x0004_039C, 0x404)]
        );
        assert_eq!(memory.read32(0x003E_3F00), 1);
        assert_eq!(memory.read32(0x003E_3F04), 1);
    }

    #[test]
    fn cull_state_emits_combined_cull_and_value_methods_then_publishes_state() {
        let mut memory = SparseMemory(BTreeMap::new());
        memory.write32(0x003E_3F48, 1);
        memory.write32(0x003E_3F44, 1);
        memory.write32(0x003E_3F00, 0);
        let mut emitted = Vec::new();
        assert!(apply_native_state_handler(
            0x91,
            0x1E00,
            0,
            0,
            0,
            &mut memory,
            0,
            0x003E_3F04,
            0,
            |header, value| emitted.push((header, value))
        ));
        assert_eq!(emitted, [(0x0004_1D84, 3), (0x0004_0370, 0x1E00)]);
        assert_eq!(memory.read32(0x003E_3F04), 0x1E00);
    }

    #[test]
    fn depth_enable_state_publishes_then_emits_cull_and_surface_gated_stencil() {
        let mut memory = SparseMemory(BTreeMap::new());
        memory.write32(0x003E_3F48, 1);
        memory.write32(0x003E_3F44, 1);
        memory.write32(0x003E_3F04, 0x1E00);
        memory.write32(0x003E_3F58, 0x1000);
        memory.write32(0x1000 + 0x1A08, 0x2000);
        let mut emitted = Vec::new();
        assert!(apply_native_state_handler(
            0x90,
            1,
            0,
            0,
            0,
            &mut memory,
            0,
            0x003E_3F00,
            0,
            |header, value| emitted.push((header, value))
        ));
        assert_eq!(emitted, [(0x0004_1D84, 3), (0x0004_032C, 1)]);
        assert_eq!(memory.read32(0x003E_3F00), 1);
    }

    #[test]
    fn stencil_enable_state_uses_previous_face_then_publishes_shadow() {
        let mut memory = SparseMemory(BTreeMap::new());
        memory.write32(0x003E_3F08, 7);
        let mut emitted = Vec::new();
        assert!(apply_native_state_handler(
            0x93,
            7,
            0,
            0,
            0,
            &mut memory,
            0,
            0x003E_3F0C,
            0,
            |header, value| emitted.push((header, value))
        ));
        assert_eq!(emitted, [(0x0004_0308, 1), (0x0004_039C, 0x404)]);
        assert_eq!(memory.read32(0x003E_3F0C), 7);

        emitted.clear();
        assert!(apply_native_state_handler(
            0x93,
            9,
            0,
            0,
            0,
            &mut memory,
            0,
            0x003E_3F0C,
            0,
            |header, value| emitted.push((header, value))
        ));
        assert_eq!(emitted, [(0x0004_0308, 1), (0x0004_039C, 0x405)]);
        assert_eq!(memory.read32(0x003E_3F0C), 9);

        emitted.clear();
        assert!(apply_native_state_handler(
            0x93,
            0,
            0,
            0,
            0,
            &mut memory,
            0,
            0x003E_3F0C,
            0,
            |header, value| emitted.push((header, value))
        ));
        assert_eq!(emitted, [(0x0004_0308, 0)]);
        assert_eq!(memory.read32(0x003E_3F0C), 0);
    }

    #[test]
    fn multisample_state_emits_scaled_word_before_publishing_shadow() {
        let mut memory = TestMemory([0; 16]);
        let mut emitted = Vec::new();
        assert!(apply_native_state_handler(
            0x9C,
            0x234,
            0,
            0,
            0,
            &mut memory,
            0,
            4,
            8,
            |header, value| emitted.push((header, value))
        ));
        assert_eq!(emitted, [(0x0004_1E6C, 0x34)]);
        assert_eq!(memory.read32(4), 0x234);
    }

    #[test]
    fn multisample_alpha_state_publishes_shadow_then_emits_combined_mask() {
        let mut memory = SparseMemory(BTreeMap::new());
        memory.write32(0x003E_3F44, 0x5678);
        memory.write32(0x003E_3F68, 0x8000);
        memory.write32(0x003E_3F20, 1);
        let mut emitted = Vec::new();
        assert!(apply_native_state_handler(
            0x99,
            0x1234,
            0,
            0,
            0,
            &mut memory,
            0,
            4,
            8,
            |header, value| emitted.push((header, value))
        ));
        assert_eq!(memory.read32(4), 0x1234);
        assert_eq!(emitted, [(0x0004_1D7C, 0x1234_5679)]);
    }
}
