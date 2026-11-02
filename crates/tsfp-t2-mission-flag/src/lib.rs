// SPDX-License-Identifier: GPL-3.0-or-later
//! Capture-only adapter for the existing readable C2R at 0x0006C890.

use tsfp_port_runtime::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};

/// Execute the existing translated function through the capture ABI.
///
/// This wrapper is registered only in the offline t2capture lane. Production
/// dispatch remains unchanged and continues to use the existing C2R fallback.
pub fn run<M: GuestMemory, C: GuestCall>(memory: &mut M, guest: &mut C, regs: &mut ExtRegisters) {
    tsfp_ports::c2r::t2049_exec_001::game_flag_mask_query_with_record_238_aux_filter_then_6b100(
        memory, guest, regs,
    );
}

/// Execute the direct 0x6B100 flag-mask query through the capture ABI.
///
/// This wrapper is registered only in the offline t2capture lane. Production
/// dispatch remains unchanged and continues to use the existing C2R fallback.
pub fn run_6b100<M: GuestMemory, C: GuestCall>(
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) {
    tsfp_ports::c2r::t2049_exec_001::game_flag_mask_query_by_byte_7de455_jump_table_or_state_descriptor_dword_0x14(
        memory, guest, regs,
    );
}
