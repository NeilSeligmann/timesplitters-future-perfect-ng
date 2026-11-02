// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 differential for the typed Rust source-alert loop vs lifted 0x25FAF0.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use tsfp_ai::awareness::{
    AlertSource, AwarenessAlertWorld, AwarenessSourceRecord, GroupMask, OwnedAwarenessAlertEffects,
    ReaAwarenessSourceStore, process_awareness_sources, process_owned_awareness_sources,
};
use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::memory::GuestMemory;

const TABLE: u32 = 0x78b2e0;
const COUNTER: u32 = 0x7a2b80;
const STACK: u32 = 0x70000;

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<HashMap<u32, u8>>>);

impl Memory {
    fn put8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
    }
    fn put32(&mut self, address: u32, value: u32) {
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.put8(address + offset as u32, byte);
        }
    }
    fn get32(&self, address: u32) -> u32 {
        u32::from_le_bytes(std::array::from_fn(|i| self.read8(address + i as u32)))
    }
}

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.borrow().get(&address).copied().unwrap_or(0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.put8(address, value);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Reset(u32),
    NestedAlert { mask: u32, target: u32 },
    Register(usize),
}

#[derive(Clone)]
struct OriginalCalls(Memory, Rc<RefCell<Vec<Event>>>);

impl GuestCall for OriginalCalls {
    fn call(&mut self, address: u32, regs: &mut ExtRegisters) {
        let mut events = self.1.borrow_mut();
        match address {
            0x349e20 => events.push(Event::Reset(self.0.get32(regs.esp))),
            0x267ff0 => {
                let selector = self.0.get32(regs.esp);
                assert_eq!(selector, 1);
                let record = self.0.get32(regs.esp + 4);
                events.push(Event::NestedAlert {
                    mask: self.0.get32(record + 4),
                    target: self.0.get32(record + 0x10),
                });
            }
            0xa0640 => events.push(Event::Register(self.0.get32(regs.esp) as usize)),
            _ => panic!("unexpected lifted source-alert delegate {address:#x}"),
        }
    }
}

struct NativeWorld {
    records: Vec<AwarenessSourceRecord>,
    events: Vec<Event>,
    alert_count: u32,
}

#[derive(Default)]
struct OwnedEffects {
    events: Vec<Event>,
    alert_count: u32,
}

impl OwnedAwarenessAlertEffects for OwnedEffects {
    fn set_group_alert_value(&mut self, value: u32) {
        self.events.push(Event::Reset(value));
    }

    fn dispatch_alert(&mut self, sources: &mut ReaAwarenessSourceStore, source: AlertSource) {
        let AlertSource::RecordOne { mask, target } = source else {
            panic!("source loop must dispatch selector 1")
        };
        assert!(sources.records().iter().any(|record| {
            record.state == 1 && record.target.is_none() && record.elapsed == 0.0
        }));
        self.events.push(Event::NestedAlert {
            mask: mask.0,
            target: target.map_or(0, |target| target.0),
        });
    }

    fn register_spawn_entry(&mut self, index: usize) {
        self.events.push(Event::Register(index));
    }

    fn increment_alert_count(&mut self) {
        self.alert_count += 1;
    }
}

impl AwarenessAlertWorld for NativeWorld {
    fn source_count(&self) -> usize {
        self.records.len()
    }
    fn source_record(&self, index: usize) -> Option<AwarenessSourceRecord> {
        self.records.get(index).copied()
    }
    fn write_source_record(&mut self, index: usize, record: AwarenessSourceRecord) {
        self.records[index] = record;
    }
    fn set_group_alert_value(&mut self, value: u32) {
        self.events.push(Event::Reset(value));
    }
    fn dispatch_alert(&mut self, source: AlertSource) {
        let AlertSource::RecordOne { mask, target } = source else {
            panic!("source loop must dispatch selector 1")
        };
        self.events.push(Event::NestedAlert {
            mask: mask.0,
            target: target.map_or(0, |target| target.0),
        });
    }
    fn register_spawn_entry(&mut self, index: usize) {
        self.events.push(Event::Register(index));
    }
    fn increment_alert_count(&mut self) {
        self.alert_count += 1;
    }
}

struct Seed(u32);
impl Seed {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0
    }
}

fn compare_seed(seed_value: u32) {
    let mut seed = Seed(seed_value);
    let count = (seed.next() % 9 + 1) as usize;
    let incoming = seed.next() | 1;
    let mut memory = Memory::default();
    memory.put32(0x78b520, count as u32);
    memory.put32(COUNTER, seed.next() % 8);
    let initial_count = memory.get32(COUNTER);
    let mut records = Vec::new();
    for index in 0..count {
        let base = TABLE + index as u32 * 0x20;
        let alert_mask = seed.next();
        let filter_mask = match seed.next() % 4 {
            0 => incoming & seed.next(),
            1 => incoming,
            _ => seed.next() & !incoming,
        };
        let state = match seed.next() % 5 {
            0 => 1,
            1 => 2,
            _ => (seed.next() % 6) as i32 + 3,
        };
        let target = (seed.next() & 1 != 0).then_some(0x40_000 + index as u32 * 4);
        let elapsed = f32::from_bits(0x3f00_0000 + (seed.next() & 0x007f_ffff));
        memory.put32(base + 4, alert_mask);
        memory.put32(base + 8, filter_mask);
        memory.put32(base + 0x10, target.map_or(0, |v| v));
        memory.put32(base + 0x14, state as u32);
        memory.put32(base + 0x18, elapsed.to_bits());
        records.push(AwarenessSourceRecord {
            alert_mask: GroupMask(alert_mask),
            filter_mask: GroupMask(filter_mask),
            target: target.map(tsfp_ai::navigation_scoring::ObjectId),
            state,
            elapsed,
        });
    }

    let mut regs = ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: STACK,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    };
    // The lifted body saves EDI and EBP before reading its single cdecl argument.
    memory.put32(STACK + 4, incoming);
    let original_events = Rc::new(RefCell::new(Vec::new()));
    let mut original_calls = OriginalCalls(memory.clone(), original_events.clone());
    tsfp_ports::c2r::t2049_nx_005::game_set_four_globals_77883c_group_then_267ff0_a0640_25faf0(
        &mut memory,
        &mut original_calls,
        &mut regs,
    );

    let mut native = NativeWorld {
        records: records.clone(),
        events: Vec::new(),
        alert_count: initial_count,
    };
    let native_admitted = process_awareness_sources(GroupMask(incoming), &mut native);
    let mut owned_sources = ReaAwarenessSourceStore::new(records);
    let mut owned_effects = OwnedEffects {
        alert_count: initial_count,
        ..OwnedEffects::default()
    };
    let owned_admitted = process_owned_awareness_sources(
        GroupMask(incoming),
        &mut owned_sources,
        &mut owned_effects,
    );
    let mut original_records = Vec::new();
    for index in 0..count {
        let base = TABLE + index as u32 * 0x20;
        original_records.push((
            memory.get32(base + 4),
            memory.get32(base + 8),
            memory.get32(base + 0x10),
            memory.get32(base + 0x14) as i32,
            memory.get32(base + 0x18),
        ));
    }
    let native_records: Vec<_> = native
        .records
        .iter()
        .map(|r| {
            (
                r.alert_mask.0,
                r.filter_mask.0,
                r.target.map_or(0, |target| target.0),
                r.state,
                r.elapsed.to_bits(),
            )
        })
        .collect();
    assert_eq!(
        native_records, original_records,
        "record state seed={seed_value}, incoming={incoming:#x}"
    );
    assert_eq!(
        native.events,
        *original_events.borrow(),
        "event order seed={seed_value}"
    );
    assert_eq!(
        native.alert_count,
        memory.get32(COUNTER),
        "counter seed={seed_value}"
    );
    assert_eq!(
        native_admitted as u32,
        memory.get32(COUNTER) - initial_count
    );
    assert_eq!(
        owned_sources.records(),
        original_records
            .iter()
            .map(|record| AwarenessSourceRecord {
                alert_mask: GroupMask(record.0),
                filter_mask: GroupMask(record.1),
                target: (record.2 != 0).then_some(tsfp_ai::navigation_scoring::ObjectId(record.2)),
                state: record.3,
                elapsed: f32::from_bits(record.4),
            })
            .collect::<Vec<_>>(),
        "Rust-owned source records seed={seed_value}, incoming={incoming:#x}"
    );
    assert_eq!(
        owned_effects.events,
        *original_events.borrow(),
        "Rust-owned effect order seed={seed_value}"
    );
    assert_eq!(
        owned_effects.alert_count,
        memory.get32(COUNTER),
        "Rust-owned counter seed={seed_value}"
    );
    assert_eq!(owned_admitted as u32, memory.get32(COUNTER) - initial_count);
}

#[test]
fn tier1_2048_seeded_source_tables_match_lifted_original_and_owned_model() {
    for seed in 0..2048 {
        compare_seed(seed);
    }
}
