// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED fixtures. Tier 0 smoke, not original differential evidence.
extern crate std;
use self::std::{prelude::v1::*, println};
use std::{
    cell::Cell,
    collections::BTreeMap,
    panic::{AssertUnwindSafe, catch_unwind},
};
use tsfp_port_runtime::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
};
use tsfp_ports::assets::{self, AssetGuest};
#[derive(Default)]
struct Memory {
    bytes: BTreeMap<u32, u8>,
    budget: Cell<u32>,
}
impl Memory {
    fn tick(&self) {
        let n = self.budget.get();
        assert!(n > 0, "fixture access budget exhausted");
        self.budget.set(n - 1);
    }
}
impl GuestMemory for Memory {
    fn read8(&self, a: u32) -> u8 {
        self.tick();
        *self.bytes.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.tick();
        self.bytes.insert(a, v);
    }
}
#[derive(Default)]
struct Guest {
    calls: Vec<u32>,
    cache: Vec<u32>,
}
impl GuestCall for Guest {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        assert!(self.calls.len() < 1000, "fixture call budget exhausted");
        self.calls.push(va);
        r.eax = 0;
    }
}
impl AssetGuest for Guest {
    fn writeback_invalidate(&mut self, va: u32) {
        self.cache.push(va);
    }
}
fn registers() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: 0x9000,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
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
#[test]
fn selection_preserves_disabled_and_unknown_state() {
    let mut m = Memory {
        budget: Cell::new(10000),
        ..Default::default()
    };
    let mut g = Guest::default();
    let mut r = registers();
    let before = r;
    for va in assets::FUNCTIONS
        .iter()
        .map(|f| f.address)
        .chain([0xffffffff])
    {
        assert!(!assets::dispatch(va, &[], &mut m, &mut g, &mut r));
        assert_eq!(r, before);
    }
    assert!(!assets::dispatch(
        0xffffffff,
        &[0xffffffff],
        &mut m,
        &mut g,
        &mut r
    ));
    assert!(g.calls.is_empty());
    assert!(g.cache.is_empty());
    assert!(m.bytes.is_empty());
}
#[test]
fn smoke_census() {
    let mut passed = Vec::new();
    let mut exhausted = Vec::new();
    for f in assets::FUNCTIONS {
        let mut m = Memory {
            budget: Cell::new(20000),
            ..Default::default()
        };
        let mut g = Guest::default();
        let mut r = registers();
        m.write32(r.esp, 0x1234);
        m.write32(0x6648d0, 0x664890); // circular-list sentinel
        if f.address == 0x29650 || f.address == 0x57db0 {
            m.write32(r.esp + 4, 0x4000);
            let bytes: &[u8] = b"a/\0";
            for (i, b) in bytes.iter().enumerate() {
                m.write8(0x4000 + i as u32, *b);
            }
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            assets::dispatch(f.address, &[f.address], &mut m, &mut g, &mut r)
        }));
        if result.is_ok() {
            passed.push(f.address)
        } else {
            exhausted.push(f.address)
        }
    }
    println!(
        "SMOKE passed={} addresses={passed:x?}; no-verdict={} addresses={exhausted:x?}",
        passed.len(),
        exhausted.len()
    );
    assert_eq!(
        passed.len(),
        94,
        "every registered Tier 0 function must terminate its fixture"
    );
    assert!(exhausted.is_empty());
}

fn invoke(va: u32, m: &mut Memory, g: &mut Guest, r: &mut ExtRegisters) {
    assert!(assets::dispatch(va, &[va], m, g, r));
}
#[test]
fn cache_policy_and_async_request_record_are_explicit() {
    let mut m = Memory {
        budget: Cell::new(20000),
        ..Default::default()
    };
    let mut g = Guest::default();
    let mut r = registers();
    let args = [3, 0x5000, 0x81, 0x801, 0x13];
    for (i, v) in args.iter().enumerate() {
        m.write32(r.esp + 4 + 4 * i as u32, *v);
    }
    let record = 0x661370 + 3 * 0x44;
    m.write32(record + 0x18, 0x1000);
    m.write32(0x7497a8, 0x30);
    m.write32(0x6648d0, 0x664890);
    invoke(0x29370, &mut m, &mut g, &mut r);
    assert_eq!(g.cache, [0x29394]);
    assert!(g.calls.is_empty());
    assert_eq!((r.eax, r.esp), (1, 0x9000));
    assert_eq!(m.read32(record + 8), 0x1081);
    assert_eq!(m.read32(record + 0x2c), 0x1000);
    assert_eq!(m.read32(record + 0x30), 0x1d);
    assert_eq!(m.read32(record + 0x38), 0x5000);
    assert_eq!(m.read32(record + 0x34), 1);
    assert_eq!(m.read32(0x6648d0), record);
}
#[test]
fn file_table_reset_and_stride_lookup_contracts() {
    let mut m = Memory {
        budget: Cell::new(20000),
        ..Default::default()
    };
    let mut g = Guest::default();
    let mut r = registers();
    m.write32(0x6d7880, 27);
    m.write32(0x4d19ec, 123);
    invoke(0x626e0, &mut m, &mut g, &mut r);
    assert_eq!(m.read32(0x6d7880), 0);
    assert_eq!(m.read32(0x4d19ec), u32::MAX);
    for index in [0u32, 1, 17, 0x8000_0000, u32::MAX] {
        let address = index.wrapping_mul(0x34).wrapping_add(0x5149ac);
        m.write32(r.esp + 4, index);
        m.write32(address, index ^ 0xa55a);
        invoke(0x6a660, &mut m, &mut g, &mut r);
        assert_eq!(r.eax, index ^ 0xa55a);
        assert_eq!(r.esp, 0x9000);
    }
    assert!(g.calls.is_empty());
}
#[test]
fn float_getter_preserves_payload_and_moves_x87_top() {
    let mut m = Memory {
        budget: Cell::new(20000),
        ..Default::default()
    };
    let mut g = Guest::default();
    let mut r = registers();
    for bits in [0u32, 0x80000000, 0x3f800000, 0x7f800000, 0xff800000] {
        m.write32(0x75f5b0, bits);
        let top = r.fpu.top;
        invoke(0x257b20, &mut m, &mut g, &mut r);
        assert_eq!(r.fpu.top, (top + 7) & 7);
        assert_eq!((r.fpu.st(0) as f32).to_bits(), bits);
    }
}
