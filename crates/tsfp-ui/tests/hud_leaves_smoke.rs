// SPDX-License-Identifier: GPL-3.0-or-later
//! T1960 Tier 0 checks for the three call-free HUD/campaign leaf contracts.
use tsfp_ui::gprs::Gprs;
use tsfp_ui::hud_leaves::{campaign_status_write_result, hud_queue_overlay_request, hud_transition_set_two_params};
use tsfp_ui::memory::GuestMemory;
use tsfp_ui::strmem::SparseMemory;

#[test]
fn campaign_status_routes_to_the_matching_output() {
    let mut m = SparseMemory::default();
    let regs = Gprs { eax: 7, ecx: 8, edx: 9 };
    m.write32(0x1004, 0x2000);
    m.write32(0x1008, 0x3000);
    m.write32(0x0068_9178, u32::MAX);
    m.write32(0x0068_91ac, 42);
    assert_eq!(campaign_status_write_result(&mut m, 0x1000, regs).eax, u32::MAX);
    assert_eq!(m.read32(0x3000), 42);
    m.write32(0x0068_9178, 2);
    m.write32(0x0068_9160, 77);
    campaign_status_write_result(&mut m, 0x1000, regs);
    assert_eq!(m.read32(0x2000), 77);
}

#[test]
fn overlay_queue_masks_second_word_and_honours_capacity() {
    let mut m = SparseMemory::default();
    let regs = Gprs { eax: 7, ecx: 8, edx: 9 };
    m.write32(0x1004, 11);
    m.write32(0x1008, 0x1234_5678);
    m.write32(0x100c, 33);
    assert_eq!(hud_queue_overlay_request(&mut m, 0x1000, regs).eax, 0);
    assert_eq!(m.read32(0x0076_6f60), 11);
    assert_eq!(m.read32(0x0076_6f64), 0x1234_5600);
    assert_eq!(m.read32(0x0076_6f68), 33);
    assert_eq!(m.read32(0x0076_70e0), 1);
    m.write32(0x0076_70e0, 32);
    assert_eq!(hud_queue_overlay_request(&mut m, 0x1000, regs).eax, 7);
    assert_eq!(m.read32(0x0076_70e0), 32);
}

#[test]
fn transition_arguments_keep_their_original_order() {
    let mut m = SparseMemory::default();
    let regs = Gprs { eax: 7, ecx: 8, edx: 9 };
    m.write32(0x1004, 12);
    m.write32(0x1008, 34);
    let out = hud_transition_set_two_params(&mut m, 0x1000, regs);
    assert_eq!((out.eax, out.ecx, out.edx), (34, 12, 9));
    assert_eq!(m.read32(0x0076_76c8), 12);
    assert_eq!(m.read32(0x0076_76dc), 34);
}
