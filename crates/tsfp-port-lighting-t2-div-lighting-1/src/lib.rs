// SPDX-License-Identifier: GPL-3.0-or-later
#![no_std]

//! Caller-visible EDX contract for the native light-slot adapter (T2 item t2_div-lighting_1).

/// Adapt the native light-slot result to the lifted caller-visible register contract.
/// Enabled calls leave the copied green color word in EDX; disabled calls preserve incoming EDX.
#[macro_export]
macro_rules! fill_light_slot {
    ($renderer:expr, $regs:expr, $args:expr, $incoming_edx:expr) => {{
        let slot = $args[0];
        let enabled = $args[1];
        let result = $renderer.fill_light_slot(slot, enabled, $args[2], $args[3]);
        $regs.edx = if enabled == 0 {
            $incoming_edx
        } else {
            $renderer.mem.read32(slot.wrapping_add(0x14))
        };
        result
    }};
}
