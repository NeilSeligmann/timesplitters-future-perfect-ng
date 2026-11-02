// SPDX-License-Identifier: GPL-3.0-or-later
//! Smoke tests for the T1895 syslink batch 1 (Tier 0, fake guest memory).
use std::collections::HashMap;
use tsfp_network::w4n_syslink::*;
use tsfp_network::w4n_syslink_llm::*;

#[derive(Default)]
struct Fake {
    words: HashMap<u32, u32>,
    link: u32,
    calls: Vec<(u32, Vec<u32>)>,
    results: HashMap<u32, u32>,
}
impl SyslinkHost for Fake {
    fn read32(&mut self, a: u32) -> u32 {
        self.words.get(&a).copied().unwrap_or(0)
    }
    fn read8(&mut self, a: u32) -> u8 {
        let w = self.words.get(&(a & !3)).copied().unwrap_or(0);
        w.to_le_bytes()[(a & 3) as usize]
    }
    fn write32(&mut self, a: u32, v: u32) {
        self.words.insert(a, v);
    }
    fn xnet_link_status(&mut self) -> u32 {
        self.link
    }
    fn platform_call(&mut self, target: u32, args: &[u32]) -> u32 {
        self.calls.push((target, args.to_vec()));
        self.results.get(&target).copied().unwrap_or(0)
    }
    fn pool_call(&mut self, target: u32, args: &[u32]) {
        self.calls.push((target, args.to_vec()));
    }
    fn stack_scratch(&mut self) -> u32 {
        0x7000
    }
}

#[test]
fn flag_setters_and_link_status() {
    let mut h = Fake { link: 0x30, ..Fake::default() };
    set_global_665908_to_1(&mut h);
    assert_eq!(h.words[&0x0066_5908], 1);
    assert_eq!(set_global_665908_to_2_return_true(&mut h), 1);
    assert_eq!(h.words[&0x0066_5908], 2);
    set_global_66591c_to_1(&mut h);
    set_global_665920_to_1(&mut h);
    assert_eq!(set_global_665920_to_2_return_true(&mut h), 1);
    clear_globals_665920_and_66591c(&mut h);
    assert_eq!((h.words[&0x0066_5920], h.words[&0x0066_591C]), (0, 0));
    global_686644_set_one_wrapper(&mut h);
    assert_eq!(h.words[&0x0068_6644], 1);
    assert_eq!(return_constant_0x24(), 0x24);
    assert_eq!(ethernet_link_status_nonzero(&mut h), 1);
    h.link = 0;
    assert_eq!(ethernet_link_status_nonzero(&mut h), 0);
}

#[test]
fn peer_lookup() {
    let mut h = Fake::default();
    h.words.insert(PEER_SLOT_FLAGS + 4 * 3, 1);
    h.words.insert(PEER_SLOT_FLAGS + 4 * 5, 1);
    h.words.insert(PEER_ADDRESSES + 0x10 * 3, 0x0102_0304);
    h.words.insert(PEER_ADDRESSES + 0x10 * 5, 0x0A0B_0C0D);
    h.words.insert(0x5000, 0x0A0B_0C0D);
    h.words.insert(0x5100, 0xFFFF_FFFF);
    assert_eq!(peer_find_by_addr(&mut h, 0x5000), 5);
    assert_eq!(peer_find_by_addr(&mut h, 0x5100), u32::MAX);
    h.words.insert(CURRENT_SLOT_INDEX, 5);
    assert_eq!(current_peer_slot_flags(&mut h), 1);
}

#[test]
fn send_helpers_issue_the_expected_calls() {
    let mut h = Fake::default();
    h.words.insert(0x0066_5904, 2);
    h.words.insert(0x0066_57F0, 0x55);
    assert_eq!(send_to_peer(&mut h, 9, 7), 0);
    assert_eq!(h.calls[0], (0x0043_188F, vec![0x55, 9, 7, 0, 0x0066_58A4, 0x10]));
    h.calls.clear();
    send_packet_init_node_pool(&mut h);
    assert_eq!(h.calls[0].0, 0x0015_6DF0);
    assert_eq!(h.calls[1], (0x0015_6DB0, vec![0x0066_5894, 0x54, 0x0066_5538, 8]));
    assert_eq!(h.calls[2], (0x0043_188F, vec![0x55, 0x7000, 4, 0, 0x0066_58A4, 0x10]));
    assert_eq!(h.words[&0x7000], 0xFFFF_FFFB);
    assert_eq!(h.words[&0x0066_5910], 1);
    h.calls.clear();
    h.words.insert(0x0066_5538 + 0x54 * 2, 0xAAAA);
    h.words.insert(0x0066_5538 + 0x54 * 2 + 4, 0xBBBB);
    sendto_peer_helper(&mut h, 2);
    assert_eq!(h.words[&0x0066_552C], 0xAAAA);
    assert_eq!(h.words[&0x0066_5530], 0xBBBB);
    assert_eq!(h.calls[0], (0x0043_188F, vec![0x55, 0x0066_5520, 0x18, 0, 0x0066_58A4, 0x10]));
}

#[test]
fn llm_group_close_and_retry() {
    let mut h = Fake::default();
    assert_eq!(net_socket_close_unregister_wrapper(&mut h), 1);
    assert_eq!(h.calls, vec![(0x2E280, vec![])]);
    h.calls.clear();
    h.words.insert(0x0066_5920, 1);
    assert_eq!(net_socket_close_unregister_wrapper(&mut h), 0);
    assert!(h.calls.is_empty());
    net_recv_discovery_wrapper(&mut h);
    assert_eq!(h.calls.iter().map(|c| c.0).collect::<Vec<_>>(), vec![0x2DFC0, 0x2EBC0]);
    h.calls.clear();
    h.words.insert(0x0066_5904, 2);
    h.words.insert(0x0066_57F0, 0x77);
    assert_eq!(net_socket_cleanup_xnet_unregister_close(&mut h), 1);
    assert_eq!(h.calls[0], (0x0043_1721, vec![0x0066_57D8]));
    assert_eq!(h.calls[1], (0x0043_17BC, vec![0x77]));
    assert_eq!(h.calls.last().unwrap().0, 0x2E280);
    assert_eq!((h.words[&0x0066_5904], h.words[&0x0066_5908]), (0, 0));
}

#[test]
fn llm_group_error_assert_inaddr_and_qos() {
    let mut h = Fake::default();
    assert_eq!(net_error_assert(&mut h, 3), 0); // last error is not 0x2751
    h.results.insert(0x0043_1D53, 0x2751);
    h.words.insert(0x0077_4484 + 0x30C * 2, 0x32);
    assert_eq!(net_error_assert(&mut h, 2), 1);
    assert!(h.calls.iter().all(|c| c.0 != 0x35BEF0)); // counter 0x33 triggers the allocation probe
    h.calls.clear();
    h.words.insert(0x0077_4484 + 0x30C * 2, 0x32);
    h.results.insert(0x0035_5EC0, 1);
    net_error_assert(&mut h, 2);
    assert!(h.calls.iter().any(|c| c.0 == 0x35BEF0 && c.1 == vec![0x0077_4180 + 0x30C * 2, 0]));
    // inaddr wrapper: unused slot returns the fixed address, used slot passes the peer value
    let mut h = Fake::default();
    assert_eq!(net_inaddr_to_xnaddr_wrapper(&mut h, 4), 0x0068_6658);
    h.words.insert(PEER_SLOT_FLAGS + 16, 1);
    h.words.insert(PEER_ADDRESSES + 4 * 16, 0xABCD);
    assert_eq!(net_inaddr_to_xnaddr_wrapper(&mut h, 4), 0x0066_5930 + 36 * 4);
    assert_eq!(h.calls[0], (0x0043_1968, vec![0xABCD, 0x0066_5930 + 36 * 4, 0]));
    // qos start with no flags set records a zero count
    let mut h = Fake::default();
    net_peer_qos_lookup_start(&mut h);
    assert_eq!(h.words[&0x0068_667C], 0);
    assert_eq!(h.calls[0].0, 0x0043_177F);
    assert_eq!(h.calls[0].1[0], 0);
    assert_eq!(h.calls[0].1[1], 0x7000 + 0x8C);
}
