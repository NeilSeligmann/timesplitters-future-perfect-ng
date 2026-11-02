// SPDX-License-Identifier: GPL-3.0-or-later
//! Smoke tests for the T1895 arena ports (Tier 0, fake guest memory).
use std::collections::HashMap;
use tsfp_engine_memory::w4m_arena::{Arena, globals as g};
use tsfp_engine_memory::{GuestMemory, MemoryFault};
use tsfp_types::GuestAddr;

#[derive(Default)]
struct Fake(HashMap<u32, u32>);
impl GuestMemory for Fake {
    fn read(&mut self, a: GuestAddr, out: &mut [u8]) -> Result<(), MemoryFault> {
        out.copy_from_slice(&self.0.get(&a.0).copied().unwrap_or(0).to_le_bytes()[..out.len()]);
        Ok(())
    }
    fn write(&mut self, a: GuestAddr, bytes: &[u8]) -> Result<(), MemoryFault> {
        let mut word = [0; 4];
        word[..bytes.len()].copy_from_slice(bytes);
        self.0.insert(a.0, u32::from_le_bytes(word));
        Ok(())
    }
}
#[path = "support/arena_hooks.rs"]
mod arena_hooks;
use arena_hooks::Hooks;

fn setup(m: &mut Fake) {
    for (a, v) in [
        (g::TOP_CURSOR, 0x2000_0000),
        (g::MAIN_REMAINING, 0x1000),
        (g::REGION_B_CURSOR, 0x1000_0000),
        (g::REGION_A_CURSOR, 0x1800_0000),
        (g::REGION_A_REMAINING, 0x800),
        (g::KIND4_CURSOR, 0x3000_0000),
        (g::BOTTOM_LIMIT, 0x2000_0000),
        (g::BUMP_CURSOR, 0x4000_0000),
    ] {
        m.0.insert(a, v);
    }
}

#[test]
fn bump_rounds_to_sixteen() {
    let (mut m, mut h) = (Fake::default(), Hooks::default());
    setup(&mut m);
    let mut a = Arena {
        memory: &mut m,
        hooks: &mut h,
    };
    assert_eq!(a.bump_allocate(0x11).unwrap(), 0x4000_0000);
    assert_eq!(a.bump_allocate(1).unwrap(), 0x4000_0020);
    assert_eq!(m.0[&g::BUMP_TOTAL], 0x30);
}

#[test]
fn tagged_block_alloc_then_release_restores_state() {
    let (mut m, mut h) = (Fake::default(), Hooks::default());
    setup(&mut m);
    let mut a = Arena {
        memory: &mut m,
        hooks: &mut h,
    };
    let block = a.alloc_round16(0, 0x25).unwrap();
    assert_eq!(block, 0x2000_0000 - 0x30); // header at top-0x40, payload 0x30
    assert_eq!(a.memory.0[&g::MAIN_REMAINING], 0x1000 - 0x40);
    a.release_tagged_block(block).unwrap();
    assert_eq!(a.memory.0[&g::TOP_CURSOR], 0x2000_0000);
    assert_eq!(a.memory.0[&g::MAIN_REMAINING], 0x1000);
    assert!(h.0.is_empty());
}

#[test]
fn tagged_release_rereads_size_after_publishing_top_and_keeps_register_results() {
    let (mut m, mut h) = (Fake::default(), Hooks::default());
    setup(&mut m);
    let top = 0x10;
    m.0.insert(g::TOP_CURSOR, top);
    m.0.insert(g::MAIN_REMAINING, 0x4D45_4D4C);
    m.0.insert(g::BOTTOM_LIMIT, 0x4143_4F4C);
    m.0.insert(0x006B_838C, top);
    let mut a = Arena {
        memory: &mut m,
        hooks: &mut h,
    };

    let registers = a.release_tagged_block_with_registers(0x006B_8390).unwrap();

    assert_eq!(registers.eax, 0x4D45_4D8C);
    assert_eq!(registers.ecx, 0x30);
    assert_eq!(registers.edx, 0x4D45_4D4C);
    assert_eq!(a.memory.0[&g::TOP_CURSOR], 0x30);
    assert_eq!(a.memory.0[&g::MAIN_REMAINING], registers.eax);
    assert!(h.0.is_empty());
}

#[test]
fn kinds_and_overflow() {
    let (mut m, mut h) = (Fake::default(), Hooks::default());
    setup(&mut m);
    let mut a = Arena {
        memory: &mut m,
        hooks: &mut h,
    };
    assert_eq!(a.alloc_round16(1, 0x20).unwrap(), 0x1000_0000);
    assert_eq!(a.alloc_round16(2, 0x20).unwrap(), 0x1800_0000);
    assert_eq!(a.alloc_round16(4, 0x20).unwrap(), 0x3000_0000);
    assert_eq!(a.alloc_round16(3, 0x20).unwrap(), 0);
    a.alloc_round16(1, 0x2000).unwrap();
    assert_eq!(h.0, vec![u32::MAX, 1]);
}

#[test]
fn bad_release_reports_and_rewind_credits() {
    let (mut m, mut h) = (Fake::default(), Hooks::default());
    setup(&mut m);
    let mut a = Arena {
        memory: &mut m,
        hooks: &mut h,
    };
    a.release_tagged_block(0x5000_0010).unwrap();
    assert_eq!(h.0, vec![1, 1]);
    let mut a = Arena {
        memory: &mut m,
        hooks: &mut h,
    };
    a.rewind_top().unwrap();
    a.set_ledger_flag(7).unwrap();
    assert_eq!(a.ledger_flag().unwrap(), 7);
}
