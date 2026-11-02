// SPDX-License-Identifier: GPL-3.0-or-later
//! Standalone Tier 0 semantic tests. FABRICATED fixtures; no oracle goldens.
#![allow(dead_code, unused_imports)]
use crate::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use crate::c2r::{
    t1892_adapters as adapters, t1892_input as input, t1892_pathfinding as pathfinding,
    t1892_settings as settings,
};
use crate::memory::GuestMemory;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

#[derive(Clone, Default)]
struct Mem(Rc<RefCell<BTreeMap<u32, u8>>>);
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        self.0.borrow().get(&a).copied().unwrap_or(0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.borrow_mut().insert(a, v);
    }
}
struct Host {
    memory: Mem,
    calls: Vec<(u32, Vec<u32>)>,
    returns: BTreeMap<u32, u32>,
    removed: u32,
    added: u32,
    new_viewport: Option<u32>,
}
impl Host {
    fn new(memory: &Mem) -> Self {
        Self {
            memory: memory.clone(),
            calls: vec![],
            returns: BTreeMap::new(),
            removed: 0,
            added: 0,
            new_viewport: None,
        }
    }
}
impl GuestCall for Host {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        assert!(self.calls.len() < 100, "unbounded callee calls");
        let arity = match va {
            0x46dbcd | 0x42d00 | 0x15e120 | 0x2735a0 => 2,
            0x46dbf4 | 0x17570 | 0x74910 | 0x74950 => 3,
            0xcf2d0 | 0x79c00 | 0x46e189 | 0x426d0 | 0x15d470 => 1,
            0x46e133 => 4,
            0x46e195 => 2,
            _ => 0,
        };
        self.calls.push((
            va,
            (0..arity)
                .map(|i| self.memory.read32(r.esp + i * 4))
                .collect(),
        ));
        if va == 0x46dbf4 {
            let removed = self.memory.read32(r.esp + 8);
            let added = self.memory.read32(r.esp + 4);
            self.memory.write32(removed, self.removed);
            self.memory.write32(added, self.added);
        }
        if va == 0xcf2d0 {
            if let Some(viewport) = self.new_viewport {
                self.memory.write32(0x7ba9e0, viewport);
            }
        }
        r.eax = self.returns.get(&va).copied().unwrap_or(0);
        // stdcall device routines consume arguments; cdecl guest calls do not.
        r.esp = r.esp.wrapping_add(match va {
            0x46dbcd => 8,
            0x46dbf4 => 12,
            0x46e189 => 4,
            0x46e133 => 16,
            0x46e195 => 8,
            _ => 0,
        });
    }
}
fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: 0x10000,
        ebx: 0x123,
        ebp: 0x456,
        esi: 0x789,
        edi: 0xabc,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0x27f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}
fn saved(r: &ExtRegisters) -> [u32; 5] {
    [r.esp, r.ebx, r.ebp, r.esi, r.edi]
}

#[test]
fn settings_chain_clears_matching_other_kinds_until_sentinel() {
    let mut m = Mem::default();
    let mut r = regs();
    let before = saved(&r);
    m.write32(r.esp + 4, 0x20000);
    m.write32(r.esp + 8, 7);
    m.write32(r.esp + 12, 3);
    m.write32(0x20178, 0x30000);
    for (i, kind, key) in [(0, 2, 7), (1, 3, 7), (2, 2, 8), (3, 17, 7)] {
        let node = 0x30000 + i * 0x1000;
        m.write32(node + 4, node + 0x1000);
        m.write32(node + 0x14, kind);
        m.write32(node + 0xcc, key);
        m.write32(node + 0xc8, 9);
        m.write32(node + 0xb8, 0xffff);
    }
    let mut host = Host::new(&m);
    assert!(settings::run(0x2e7e50, &mut m, &mut host, &mut r));
    assert_eq!(m.read32(0x300c8), 0);
    assert_eq!(m.read32(0x300cc), 0);
    assert_eq!(m.read32(0x300b8), 0xffef);
    for node in [0x31000, 0x32000, 0x33000] {
        assert_eq!(m.read32(node + 0xc8), 9);
    }
    assert_eq!(saved(&r), before);
    assert!(host.calls.is_empty());
}

#[test]
fn settings_chain_high_kind_returns_before_traversal() {
    let mut m = Mem::default();
    let mut r = regs();
    let mut host = Host::new(&m);
    m.write32(r.esp + 12, 17);
    assert!(settings::run(0x2e7e50, &mut m, &mut host, &mut r));
    assert_eq!(r.esp, 0x10000);
}

#[test]
fn settings_slider_join_clamps_both_directions_and_keeps_nan() {
    for (value, flag, expected) in [
        (0.05f32, 0x40000, 0.0f32),
        (0.95, 0x80000, 1.0),
        (0.5, 0x40000, 0.4),
        (0.0, 0x40000, 0.0),
        (1.0, 0x80000, 1.0),
        (f32::NAN, 0x40000, f32::NAN),
    ] {
        let mut m = Mem::default();
        let mut r = regs();
        let before = saved(&r);
        m.write32(r.esp + 4, 0x20000);
        m.write32(0x2018c, 9);
        m.write32(0x200c4, flag);
        m.write32(0x7ba3dc, value.to_bits());
        m.write32(0x475ce8, 0.1f32.to_bits());
        m.write32(0x475c78, 1.0f32.to_bits());
        m.write32(0x47d674, 1.0f32.to_bits());
        let mut host = Host::new(&m);
        assert!(settings::run(0x2eab30, &mut m, &mut host, &mut r));
        let actual = f32::from_bits(m.read32(0x7ba3dc));
        if expected.is_nan() {
            assert!(actual.is_nan());
        } else {
            assert_eq!(actual, expected);
        }
        assert_eq!(saved(&r), before);
        assert_eq!(
            host.calls.iter().map(|c| c.0).collect::<Vec<_>>(),
            [0x2e9bc0, 0x74910]
        );
    }
}

#[test]
fn settings_accept_copies_31_words_reloads_and_uses_postcall_viewport() {
    let mut m = Mem::default();
    let mut r = regs();
    let before = saved(&r);
    m.write32(0x7b0c7c, 0x20000);
    m.write32(0x21500, 0x30000);
    m.write32(0x7ba9e0, 1);
    for i in 0..31 {
        m.write32(0x7ba544 + i * 4, i + 100);
    }
    m.write32(0x761bc4, 0x55555);
    m.write32(0x3007c, 0xdeadbeef);
    let mut host = Host::new(&m);
    host.new_viewport = Some(2);
    assert!(adapters::run(0x2e9e50, &mut m, &mut host, &mut r));
    for i in 0..31 {
        assert_eq!(m.read32(0x30000 + i * 4), i + 100);
    }
    assert_eq!(m.read32(0x3007c), 0xdeadbeef);
    assert_eq!(saved(&r), before);
    assert_eq!(
        host.calls,
        [(0xcf2d0, vec![0x20000]), (0x79c00, vec![0x55555])]
    );
    assert_eq!(m.read32(r.esp + 4), 0x55555);
}

#[test]
fn input_suffixes_ignore_gate_and_select_mapped_or_alternate_records() {
    for alternate in [0, 1] {
        let mut m = Mem::default();
        let mut r = regs();
        let mut host = Host::new(&m);
        m.write32(0x6b9888, 1);
        m.write32(0x76b0fc, alternate);
        m.write32(r.esp + 4, 1);
        m.write32(0x6b93c4, 3);
        let base = if alternate == 0 {
            0x6b93d0 + 3 * 0x3c
        } else {
            0x6b94c8 + 0x3c
        };
        m.write32(base + 0xc, 0b1111);
        m.write32(base + 0x38, 0b0101);
        m.write32(base + 0x34, 77);
        assert!(adapters::run(0x4276c, &mut m, &mut host, &mut r));
        assert_eq!(r.eax, 0b1010);
        assert!(adapters::run(0x42bfc, &mut m, &mut host, &mut r));
        assert_eq!(r.eax, 77);
        assert_eq!(r.esp, 0x10000);
        assert!(host.calls.is_empty());
    }
}

#[test]
fn input_init_preserves_stack_and_initializes_all_four_port_mappings() {
    let mut m = Mem::default();
    let mut r = regs();
    let before = saved(&r);
    m.write32(0x74d0fc, u32::MAX);
    m.write32(0x7e7240, 0xabcdffff);
    for a in [
        0x74cf80, 0x74cfc6, 0x74d00c, 0x74d052, 0x74d100, 0x74d104, 0x74d108, 0x74d10c,
    ] {
        m.write32(a, 123);
    }
    let mut host = Host::new(&m);
    assert!(input::run(0x18feb0, &mut m, &mut host, &mut r));
    assert_eq!(saved(&r), before);
    assert_eq!(m.read32(0x74d0fc), 0);
    assert_eq!(m.read32(0x7e7240), 0xabcd0000);
    let maps: Vec<_> = host
        .calls
        .iter()
        .filter(|c| c.0 == 0x42d00)
        .map(|c| c.1.clone())
        .collect();
    assert_eq!(maps, [vec![0, 0], vec![1, 1], vec![2, 2], vec![3, 3]]);
    assert_eq!(host.calls.first().unwrap().0, 0x46dbcd);
    assert_eq!(host.calls.last().unwrap().0, 0x25860);
}

#[test]
fn input_hotplug_removes_handles_and_rejects_failed_open() {
    let mut m = Mem::default();
    let mut r = regs();
    let before = saved(&r);
    m.write32(0x74d0fc, 1);
    m.write32(0x74d100, 0x9999);
    m.write32(0x74d0ad, 0xffffffff);
    let mut host = Host::new(&m);
    host.removed = 1;
    host.added = 2;
    assert!(input::run(0x190340, &mut m, &mut host, &mut r));
    assert_eq!(saved(&r), before);
    assert_eq!(m.read32(0x74d100), 0);
    assert_eq!(m.read32(0x74d104), 0);
    assert_eq!(m.read32(0x74d0fc), 0);
    assert_eq!(m.read32(0x74d0ad), 0);
    assert!(host.calls.contains(&(0x46e189, vec![0x9999])));
    assert!(host.calls.iter().any(|c| c.0 == 0x46e133));
}

#[test]
fn path_route_mode_handles_helper_success_and_no_route_flag() {
    for success in [0, 1] {
        let mut m = Mem::default();
        let mut r = regs();
        let before = saved(&r);
        m.write32(r.esp + 4, 0x20000);
        m.write32(r.esp + 8, 0x40000);
        m.write32(0x2007c, 0x30000);
        let mut host = Host::new(&m);
        host.returns.insert(0x2735a0, success);
        assert!(pathfinding::run(0x2737c0, &mut m, &mut host, &mut r));
        assert_eq!(saved(&r), before);
        assert_eq!(host.calls.len(), if success == 0 { 1 } else { 2 });
        if success != 0 {
            assert_eq!(host.calls[1], (0x17570, vec![0x30000, 0x200000, 0]));
        }
    }
}

#[test]
fn path_state_machine_applies_pending_state_and_clears_timer() {
    let mut m = Mem::default();
    let mut r = regs();
    let before = saved(&r);
    m.write32(r.esp + 4, 0x20000);
    m.write32(0x2007c, 0x30000);
    m.write32(0x30020, 3);
    m.write32(0x309d0, 17);
    m.write32(0x309d8, 4.0f32.to_bits());
    m.write32(0x30a90, 9.0f32.to_bits());
    let mut host = Host::new(&m);
    assert!(pathfinding::run(0x2a0500, &mut m, &mut host, &mut r));
    assert_eq!(m.read32(0x30020), 17);
    assert_eq!(m.read32(0x309d4), 3);
    assert_eq!(m.read32(0x309d0), u32::MAX);
    assert_eq!(m.read32(0x309d8), 0);
    assert_eq!(m.read32(0x30a90), 0);
    assert_eq!(saved(&r), before);
    assert_eq!(
        host.calls,
        [(0x15e120, vec![0x30000, 1]), (0x15d470, vec![0x20000])]
    );
}
