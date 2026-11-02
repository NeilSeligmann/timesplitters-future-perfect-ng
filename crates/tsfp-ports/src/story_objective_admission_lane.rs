// SPDX-License-Identifier: GPL-3.0-or-later
//! T2456: typed Story objective admission at `0x227E90`.
//!
//! Typed admission is kept local to this no_std lane. This bridge decodes SETU rows, applies the
//! measured Story predicates and serializes typed slots into the hybrid guest image. The selector
//! is default-off.

use crate::{
    abi::{ExtRegisters, GuestCall, call_cdecl},
    memory::GuestMemory,
};
use alloc::vec::Vec;

const STATUS_PENDING: u8 = 1;
const STATUS_ACTIVE: u8 = 2;
const NO_KIND: u32 = u32::MAX;

#[derive(Clone, Copy)]
struct ModeContext {
    game_flags: u32,
    mode_id: u8,
    submode: u8,
    difficulty: u8,
    players: u32,
    network: bool,
    descriptor_word: u32,
    level_id: u32,
}

impl ModeContext {
    fn story(difficulty: u8, submode: u8) -> Self {
        Self {
            game_flags: 0,
            mode_id: 10,
            submode,
            difficulty,
            players: 1,
            network: false,
            descriptor_word: 0,
            level_id: 0,
        }
    }

    fn base_predicate(&self, mask: u32) -> bool {
        if self.game_flags & (0x40 | 0x0800_0000) != 0 {
            return self.descriptor_word & mask != 0;
        }
        if mask & 0x8000_0000 != 0 && self.network {
            return false;
        }
        if mask & 0x0060_0000 != 0 {
            let one = mask & 0x0020_0000 != 0 && self.players == 1;
            let two = mask & 0x0040_0000 != 0 && self.players == 2;
            if !(one || two) {
                return false;
            }
        }
        let mode_bit = match self.mode_id {
            1 => 2,
            2 => 4,
            3 => 1,
            4 | 9 => 8,
            5 => 0x40,
            6 => 0x20,
            7 => 0x10,
            10 => {
                let difficulty = mask & 0x3800;
                if difficulty != 0
                    && [0x800, 0x1000, 0x2000]
                        .get(self.difficulty as usize)
                        .is_none_or(|bit| bit & difficulty == 0)
                {
                    return false;
                }
                match self.submode {
                    0 => 0x100,
                    1 => 0x200,
                    2 => 0x400,
                    _ => 0,
                }
            }
            _ => 0,
        };
        mode_bit != 0 && mask & mode_bit != 0
    }
}

#[derive(Clone)]
struct ObjectiveDefinitionKey(u32);
#[derive(Clone)]
struct SlotDef {
    id: u32,
    key: ObjectiveDefinitionKey,
    mask: u32,
    label: u32,
    kinds: [u32; 3],
    callback_id: u32,
    dependencies: [u32; 5],
    initially_active: bool,
}
#[derive(Clone, Copy)]
enum SlotCallback {
    None,
    World(u32),
}
struct Slot {
    label: u32,
    kind: u8,
    status: u8,
    callback: SlotCallback,
    dependencies: [u32; 5],
    _key: ObjectiveDefinitionKey,
}
#[derive(Default)]
struct SlotManager {
    slots: Vec<Slot>,
    order: Vec<u32>,
    counters: [i32; 3],
    current: Option<u32>,
}

impl SlotManager {
    fn new() -> Self {
        Self::default()
    }
    fn load_level(&mut self, context: &ModeContext, defs: &[SlotDef]) {
        for def in defs {
            if !context.base_predicate(def.mask) {
                continue;
            }
            let Some(&kind) = def.kinds.get(context.submode as usize) else {
                continue;
            };
            if kind == NO_KIND || !context.base_predicate(0x700) {
                continue;
            }
            let kind = (kind as u8).min(2);
            // The lifted 0x2278E0 helper appends dependencies uniquely and stops at the
            // first 0xFFFFFFFF sentinel. Store that resulting list in typed state.
            let mut dependencies = [u32::MAX; 5];
            let mut dependency_count = 0;
            for dependency in def.dependencies {
                if dependency == u32::MAX {
                    break;
                }
                if !dependencies[..dependency_count].contains(&dependency) {
                    dependencies[dependency_count] = dependency;
                    dependency_count += 1;
                }
            }
            while self.slots.len() <= def.id as usize {
                let next = self.slots.len() as u32;
                self.slots.push(Slot {
                    label: 0,
                    kind: 0,
                    status: 0,
                    callback: SlotCallback::None,
                    dependencies: [u32::MAX; 5],
                    _key: ObjectiveDefinitionKey(0),
                });
                if next == u32::MAX {
                    break;
                }
            }
            self.counters[kind as usize] += 1;
            self.current = Some(def.id);
            self.order.push(def.id);
            self.slots[def.id as usize] = Slot {
                label: def.label,
                kind,
                status: STATUS_ACTIVE,
                callback: if def.callback_id == 0 {
                    SlotCallback::None
                } else {
                    SlotCallback::World(def.callback_id)
                },
                dependencies,
                _key: def.key.clone(),
            };
            if !def.initially_active {
                self.slots[def.id as usize].status = STATUS_PENDING;
                self.counters[kind as usize] -= 1;
            }
        }
    }
    fn ids(&self) -> &[u32] {
        &self.order
    }
    fn get(&self, id: u32) -> Option<&Slot> {
        self.order.contains(&id).then(|| &self.slots[id as usize])
    }
    fn current(&self) -> Option<u32> {
        self.current
    }
    fn active_count(&self, kind: usize) -> i32 {
        self.counters.get(kind).copied().unwrap_or(0)
    }
}

const ROOT: u32 = 0x227e90;
/// The A/B switch of these roots (selector and T3738 `--t2-offline` opt-in registry).
pub const OPT_IN: crate::runtime::OptIn = crate::runtime::OptIn {
    env: c"TSFP_STORY_OBJECTIVE_ADMISSION",
    on: c"rust",
    roots: &[ROOT],
};
const MAX_ROWS: u32 = 0x1000;
const SLOT_TABLE: u32 = 0x78cecc;
const SLOT_COUNT: u32 = 0x78cec8;
const SLOT_CURRENT: u32 = 0x78ced0;
const SLOT_TRACKERS: u32 = 0x78ced4;
const SLOT_ORDER: u32 = 0x75c14c;
const SLOT_COUNTERS: u32 = 0x75c134;
const SLOT_STRIDE: u32 = 0x4c;

#[cfg(not(test))]
unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}

fn enabled_from(value: Option<&[u8]>) -> bool {
    value == Some(OPT_IN.on.to_bytes())
}

#[cfg(not(test))]
fn enabled() -> bool {
    unsafe {
        let value = getenv(OPT_IN.env.as_ptr());
        !value.is_null() && enabled_from(Some(core::ffi::CStr::from_ptr(value).to_bytes()))
    }
}
#[cfg(test)]
fn enabled() -> bool {
    true
}

fn put16<M: GuestMemory>(memory: &mut M, address: u32, value: u16) {
    memory.write8(address, value as u8);
    memory.write8(address.wrapping_add(1), (value >> 8) as u8);
}

fn mode_context<M: GuestMemory>(memory: &M) -> Option<ModeContext> {
    let game_flags = memory.read32(0x7de458);
    // The descriptor-backed branch also depends on 0x230510 state. Keep that separate route on
    // the lifted fallback until its state contract is ported.
    if game_flags & 0x0800_0040 != 0 {
        return None;
    }
    Some(ModeContext {
        game_flags,
        mode_id: memory.read8(0x7de455),
        submode: memory.read8(0x7de457),
        difficulty: memory.read8(0x7de49a),
        players: memory.read32(0x790950),
        network: memory.read8(0x76b108) != 0,
        descriptor_word: 0,
        level_id: memory.read32(0x79094c),
    })
}

fn read_defs<M: GuestMemory>(memory: &M, base: u32, count: u32) -> Option<Vec<SlotDef>> {
    if base == 0 || count > MAX_ROWS {
        return None;
    }
    let mut defs = Vec::with_capacity(count as usize);
    for index in 0..count {
        let row = base.wrapping_add(index.wrapping_mul(0x38));
        defs.push(SlotDef {
            id: memory.read32(row),
            key: ObjectiveDefinitionKey(memory.read32(row.wrapping_add(4))),
            mask: memory.read32(row.wrapping_add(8)),
            label: memory.read32(row.wrapping_add(0x0c)),
            kinds: [
                memory.read32(row.wrapping_add(0x10)),
                memory.read32(row.wrapping_add(0x14)),
                memory.read32(row.wrapping_add(0x18)),
            ],
            callback_id: memory.read32(row.wrapping_add(0x1c)),
            dependencies: [
                memory.read32(row.wrapping_add(0x20)),
                memory.read32(row.wrapping_add(0x24)),
                memory.read32(row.wrapping_add(0x28)),
                memory.read32(row.wrapping_add(0x2c)),
                memory.read32(row.wrapping_add(0x30)),
            ],
            initially_active: memory.read32(row.wrapping_add(0x34)) != 0,
        });
    }
    Some(defs)
}

/// Handles only the Story setup root; all other paths retain the lifted implementation.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if va != ROOT || !enabled() || memory.read8(0x7de455) != 10 {
        return false;
    }
    let Some(context) = mode_context(memory) else {
        return false;
    };
    let mut table_regs = *regs;
    call_cdecl(guest, memory, &mut table_regs, 0x6a850, &[context.level_id]);
    let entry = table_regs.eax;
    if entry == 0 {
        return true;
    }
    let count = memory.read32(entry.wrapping_add(0xd8));
    if (count as i32) <= 0 {
        return true;
    }
    let Some(defs) = read_defs(memory, memory.read32(entry.wrapping_add(0xd4)), count) else {
        return false;
    };
    let mut manager = SlotManager::new();
    manager.load_level(&context, &defs);
    serialize(memory, guest, regs, &manager);
    true
}

fn serialize<M: GuestMemory, C: GuestCall>(
    memory: &mut M,
    guest: &mut C,
    regs: &ExtRegisters,
    manager: &SlotManager,
) {
    let table = memory.read32(SLOT_TABLE);
    let order = memory.read32(SLOT_ORDER);
    let tracker_base = memory.read32(SLOT_TRACKERS);
    let start_count = memory.read32(SLOT_COUNT);
    for (offset, &id) in manager.ids().iter().enumerate() {
        let slot = manager
            .get(id)
            .expect("SlotManager order contains a live slot");
        let index = start_count.wrapping_add(offset as u32);
        let address = table.wrapping_add(id.wrapping_mul(SLOT_STRIDE));
        memory.write32(order.wrapping_add(index.wrapping_mul(4)), id);
        put16(memory, address, id as u16);
        put16(memory, address.wrapping_add(2), slot.label as u16);
        put16(memory, address.wrapping_add(4), 0);
        put16(memory, address.wrapping_add(6), 0);
        memory.write8(address.wrapping_add(8), slot.kind);
        memory.write8(address.wrapping_add(0xb), slot.status);
        if id < 0x2d {
            let tracker = tracker_base.wrapping_add(index.wrapping_mul(0x7c));
            memory.write32(address.wrapping_add(0xc), tracker);
            let mut helper_regs = *regs;
            call_cdecl(
                guest,
                memory,
                &mut helper_regs,
                0x227100,
                &[tracker, u32::MAX],
            );
        } else {
            memory.write32(address.wrapping_add(0xc), 0);
        }
        for (dep_index, dependency) in slot.dependencies.iter().copied().enumerate() {
            memory.write32(
                address.wrapping_add(0x10 + dep_index as u32 * 4),
                dependency,
            );
        }
        if let SlotCallback::World(callback_id) = slot.callback {
            if callback_id != 0 {
                memory.write32(address.wrapping_add(0x3c), callback_id);
            }
        }
    }
    memory.write32(
        SLOT_COUNT,
        start_count.wrapping_add(manager.ids().len() as u32),
    );
    memory.write32(SLOT_CURRENT, manager.current().unwrap_or(u32::MAX));
    for kind in 0..3 {
        memory.write32(
            SLOT_COUNTERS + kind * 4,
            manager.active_count(kind as usize) as u32,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::enabled_from;
    use super::*;
    use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
    #[derive(Clone, Default)]
    struct Ram(Rc<RefCell<BTreeMap<u32, u8>>>);
    impl GuestMemory for Ram {
        fn read8(&self, address: u32) -> u8 {
            self.0.borrow().get(&address).copied().unwrap_or(0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.borrow_mut().insert(address, value);
        }
    }
    fn put32(memory: &mut Ram, address: u32, value: u32) {
        for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
            memory.write8(address.wrapping_add(i as u32), byte);
        }
    }
    fn get32(memory: &Ram, address: u32) -> u32 {
        u32::from_le_bytes([
            memory.read8(address),
            memory.read8(address + 1),
            memory.read8(address + 2),
            memory.read8(address + 3),
        ])
    }
    struct Calls {
        memory: Ram,
        context: ModeContext,
    }
    impl GuestCall for Calls {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            // The production GuestCall bridge pushes the callee return slot before entering a
            // lifted body. Reproduce that frame here; without it both the generated bodies and
            // their cdecl argument reads are shifted by one word.
            let caller_esp = regs.esp;
            regs.esp = caller_esp.wrapping_sub(4);
            put32(&mut self.memory, regs.esp, 0xfeed_cafe);
            match va {
                0x6a850 => regs.eax = 0x10000,
                0x6b100 => {
                    regs.eax = u32::from(
                        self.context
                            .base_predicate(get32(&self.memory, regs.esp.wrapping_add(4))),
                    )
                }
                0x227750 => {
                    let mut memory = self.memory.clone();
                    crate::c2r::data_missions::game_slot_4c_register_new_by_state_index_and_init_fields(&mut memory, self, regs);
                }
                0x227100 => {
                    let pointer = get32(&self.memory, regs.esp.wrapping_add(4));
                    for offset in (0..0x7cu32).step_by(4) {
                        put32(&mut self.memory.clone(), pointer.wrapping_add(offset), 0);
                    }
                }
                0x2278e0 => {
                    let dependency = get32(&self.memory, regs.esp.wrapping_add(4));
                    let id = get32(&self.memory, SLOT_CURRENT);
                    let record =
                        get32(&self.memory, SLOT_TABLE).wrapping_add(id.wrapping_mul(SLOT_STRIDE));
                    for index in 0..5u32 {
                        let field = record.wrapping_add(0x10 + index * 4);
                        let old = get32(&self.memory, field);
                        if old == dependency {
                            break;
                        }
                        if old == u32::MAX {
                            put32(&mut self.memory.clone(), field, dependency);
                            break;
                        }
                    }
                }
                other => panic!("unexpected guest call {other:#x}"),
            }
            // Model the guest RET consumed by the production bridge. The caller owns its args.
            regs.esp = caller_esp;
        }
    }

    fn fixture(seed: &mut u32, submode: u8) -> (Ram, Calls, Vec<SlotDef>) {
        let mut memory = Ram::default();
        // Match the lifted fixture's guest globals: the Story mode id is 10 and one player is
        // active. `ModeContext::story` leaves mode_id at its default, which would make the lifted
        // 0x6B100 oracle reject every row while the lane reads mode id 10 from guest memory.
        let context = ModeContext::story(0, submode);
        for (at, value) in [
            (0x78cecc, 0x30000),
            (0x78cec8, 0),
            (0x78ced0, u32::MAX),
            (0x78ced4, 0x40000),
            (0x75c14c, 0x50000),
            (0x100d4, 0x20000),
            (0x100d8, 6),
            (0x79094c, 0x38),
            (0x790950, 1),
            // The lifted root increments the three existing counters in place. Seed them
            // explicitly so the byte-map comparison treats untouched zero bytes as state.
            (0x75c134, 0),
            (0x75c138, 0),
            (0x75c13c, 0),
        ] {
            put32(&mut memory, at, value);
        }
        memory.write8(0x7de455, 10);
        memory.write8(0x7de457, submode);
        memory.write8(0x7de49a, 0);
        memory.write8(0x76b108, 0);
        for id in 0..0x2du32 {
            let record = 0x30000 + id * SLOT_STRIDE;
            for i in 0..5 {
                put32(&mut memory, record + 0x10 + i * 4, u32::MAX);
            }
        }
        let mut rows = Vec::new();
        for id in 0..6u32 {
            *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let mask = match (*seed >> 4) & 3 {
                0 => 0x100,
                1 => 0x200,
                2 => 0x400,
                _ => 0x800,
            };
            let kind = (*seed >> 9) % 3;
            let active = (*seed >> 14) & 1 != 0;
            let deps = [
                (*seed >> 16) & 3,
                (*seed >> 18) & 3,
                u32::MAX,
                u32::MAX,
                u32::MAX,
            ];
            let row = 0x20000 + id * 0x38;
            let def = SlotDef {
                id,
                key: ObjectiveDefinitionKey(id ^ 0x53455455),
                mask,
                label: id + 100,
                kinds: [kind; 3],
                callback_id: if id & 1 == 0 { id + 7 } else { 0 },
                dependencies: deps,
                initially_active: active,
            };
            for (off, value) in [
                (0, id),
                (4, def.key.0),
                (8, mask),
                (0x0c, def.label),
                (0x10, kind),
                (0x14, kind),
                (0x18, kind),
                (0x1c, def.callback_id),
                (0x20, deps[0]),
                (0x24, deps[1]),
                (0x28, u32::MAX),
                (0x2c, u32::MAX),
                (0x30, u32::MAX),
                (0x34, u32::from(active)),
            ] {
                put32(&mut memory, row + off, value);
            }
            rows.push(def);
        }
        (memory.clone(), Calls { memory, context }, rows)
    }
    fn registers() -> ExtRegisters {
        let mut r: ExtRegisters = unsafe { core::mem::zeroed() };
        r.esp = 0x8000;
        r
    }
    fn gameplay_state(memory: &Ram) -> Vec<(u32, u8)> {
        memory
            .0
            .borrow()
            .iter()
            .filter_map(|(&address, &value)| {
                let shared = matches!(address, 0x78cec8..=0x78ced0 | 0x75c134..=0x75c13f)
                    || (0x30000..0x30000 + 0x2d * SLOT_STRIDE).contains(&address)
                    || (0x50000..0x50000 + 0x2d * 4).contains(&address)
                    || (0x40000..0x40000 + 0x2d * 0x7c).contains(&address);
                shared.then_some((address, value))
            })
            .collect()
    }

    #[test]
    fn switch_is_explicit_and_default_off() {
        assert!(!enabled_from(None));
        assert!(!enabled_from(Some(b"1")));
        assert!(enabled_from(Some(b"rust")));
    }

    #[test]
    fn typed_slot_manager_matches_lifted_registration_over_3000_fabricated_rows() {
        let mut seed = 0x2456_17u32;
        for case in 0..3000 {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let submode = ((seed >> 8) % 3) as u8;
            let fixture_seed = seed;
            let (mut rust_memory, mut rust_calls, rows) = fixture(&mut seed, submode);
            let mut lifted_seed = fixture_seed;
            let (mut lifted_memory, mut lifted_calls, _) = fixture(&mut lifted_seed, submode);
            let mut rust_regs = registers();
            let mut lifted_regs = registers();
            assert!(run(ROOT, &mut rust_memory, &mut rust_calls, &mut rust_regs));
            crate::c2r::data_missions::game_slot_4c_register_all_from_mode_table_entries(
                &mut lifted_memory,
                &mut lifted_calls,
                &mut lifted_regs,
            );
            let rust_state = gameplay_state(&rust_memory);
            let lifted_state = gameplay_state(&lifted_memory);
            if rust_state != lifted_state {
                let rust_map: std::collections::BTreeMap<_, _> =
                    rust_state.iter().copied().collect();
                let lifted_map: std::collections::BTreeMap<_, _> =
                    lifted_state.iter().copied().collect();
                let differences: Vec<_> = rust_map
                    .keys()
                    .chain(lifted_map.keys())
                    .copied()
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .filter_map(|address| {
                        let values = (rust_map.get(&address), lifted_map.get(&address));
                        (values.0 != values.1).then_some((address, values.0, values.1))
                    })
                    .take(16)
                    .collect();
                let first = differences
                    .first()
                    .map(|(address, _, _)| *address)
                    .unwrap_or(0);
                let nearby: Vec<_> = (first.saturating_sub(8)..=first.saturating_add(12))
                    .map(|address| (address, rust_map.get(&address), lifted_map.get(&address)))
                    .collect();
                panic!(
                    "case {case}, submode {submode}: rust bytes {}, lifted bytes {}, first differing pairs {differences:?}; nearby {nearby:?}; defs {:?}; rust count/current {}/{} lifted {}/{}",
                    rust_state.len(),
                    lifted_state.len(),
                    rows.iter()
                        .map(|d| (d.id, d.mask, d.dependencies))
                        .collect::<Vec<_>>(),
                    get32(&rust_memory, SLOT_COUNT),
                    get32(&rust_memory, SLOT_CURRENT),
                    get32(&lifted_memory, SLOT_COUNT),
                    get32(&lifted_memory, SLOT_CURRENT)
                );
            }
        }
    }

    #[test]
    fn changing_the_setup_mask_changes_the_typed_admission_result() {
        let mut seed = 1;
        let (_, _, defs) = fixture(&mut seed, 0);
        let mut manager = SlotManager::new();
        manager.load_level(&ModeContext::story(0, 0), &defs);
        assert!(!manager.ids().is_empty());
        let mut changed = defs.clone();
        for row in &mut changed {
            row.mask = 0x800;
        }
        let mut mutated = SlotManager::new();
        mutated.load_level(&ModeContext::story(0, 0), &changed);
        assert_ne!(manager.ids(), mutated.ids());
    }
}
