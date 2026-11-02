// SPDX-License-Identifier: GPL-3.0-or-later
//! T1960 host-free Tier 0 checks for menu leaf memory and flag contracts.
use tsfp_ui::memory::GuestMemory;
use tsfp_ui::menu_leaves::{hud_prompt_allowed, menu_flag_mask, menu_store_field_into_low_nibble};
use tsfp_ui::strmem::SparseMemory;

#[test]
fn prompt_gate_preserves_flag_and_player_precedence() {
    let mut m = SparseMemory::default();
    assert_eq!(hud_prompt_allowed(&m), 1);
    m.write32(0x007d_e458, 0x200);
    assert_eq!(hud_prompt_allowed(&m), 0);
    m.write32(0x007d_e458, 0);
    m.write32(0x007b_0c48, 0x1000);
    m.write32(0x1114, 0x100);
    assert_eq!(hud_prompt_allowed(&m), 0);
    m.write32(0x0079_094c, 101);
    assert_eq!(hud_prompt_allowed(&m), 1);
}

#[test]
fn menu_field_is_or_ed_without_masking_and_entry_flags_are_independent() {
    let mut m = SparseMemory::default();
    m.write32(0x0075_6e58, 0x1234_5678);
    m.write32(0x1184, 0x1001);
    let regs = menu_store_field_into_low_nibble(&mut m, 0x1000);
    assert_eq!(regs.eax, 0x1234_5671);
    assert_eq!(m.read32(0x0075_6e58), regs.eax);

    assert_eq!(menu_flag_mask(&m, 2).eax, 0x0202_0000);
    m.write32(0x004d_1f88 + 0x40, 7);
    assert_eq!(menu_flag_mask(&m, 2).eax, 0x0002_0000);
    m.write8(0x007d_e455, 0x0a);
    assert_eq!(menu_flag_mask(&m, 2).eax, 0);
}
