// SPDX-License-Identifier: GPL-3.0-or-later
//! T2377 Tier 1 differentials against the retained lifted Rust bodies. Inputs are fabricated
//! owned/table fixtures; network and mission delegates are stubbed at their documented boundary.

use std::{cell::RefCell, collections::HashMap, rc::Rc};
use tsfp_gameplay::rea_interaction_inventory as native;
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    c2r::{engine_object, gameplay_player},
    memory::GuestMemory,
};

const STACK: u32 = 0x0030_0000;
const RECORDS: u32 = 0x0050_0000;
const TARGET: u32 = 0x0060_0000;

#[test]
fn t2377_ammo_subrecord_stays_distinct_from_record_ammo_and_row_limit() {
    let entries = [0i32; native::ENTRY_COUNT];
    let mut records = [None; 66];
    records[0] = Some(native::WeaponAmmoRecord {
        primary: native::AmmoPair {
            ammo_type: 20,
            amount: 5,
        },
        secondary: native::AmmoPair {
            ammo_type: 21,
            amount: 7,
        },
    });
    let mut words = [0u32; native::GameObjectDefinition::WORD_COUNT];
    words[0] = 0x1111;
    words[1] = 0x2222;
    words[10] = 0x3333;
    words[15] = 0x4444;
    words[18] = 0x5555;
    let definition = native::GameObjectDefinition::from_words(words);
    let mut ammo_rows = [None; native::AMMO_TYPE_COUNT];
    ammo_rows[20] = Some(native::AmmoDefinitionRow {
        reserve_limit: 99,
        definition,
    });
    ammo_rows[21] = Some(native::AmmoDefinitionRow {
        reserve_limit: 77,
        definition,
    });
    let inventory = native::InventoryDefinitionView::new(&entries, &records, &ammo_rows);
    let type_tables = [];
    let catalog = native::DefinitionCatalog {
        ammo_rows: &ammo_rows,
        type_tables: &type_tables,
        fixed_type_800: None,
    };

    assert_eq!(
        inventory.pickup_pairs(0),
        Some([
            native::AmmoPair {
                ammo_type: 20,
                amount: 5
            },
            native::AmmoPair {
                ammo_type: 21,
                amount: 7
            },
        ])
    );
    assert_eq!(
        catalog.ammo_definition_for_entry(&inventory, 0, native::AmmoHand::Primary),
        Some(&definition)
    );
    assert_eq!(
        catalog.ammo_definition_for_entry(&inventory, 0, native::AmmoHand::Secondary),
        Some(&definition)
    );
    let resolved = catalog.lookup(0x40, 20).unwrap();
    assert_eq!(resolved.label_id(), 0x1111);
    assert_eq!(resolved.stored_resource_id(), 0x2222);
    assert_eq!(resolved.creation_payload_a(), 0x3333);
    assert_eq!(resolved.creation_payload_b(), 0x4444);
    assert_eq!(resolved.resolver_id(), Some(0x5555));
    assert_eq!(ammo_rows[20].unwrap().reserve_limit, 99);
    assert_eq!(ammo_rows[21].unwrap().reserve_limit, 77);
    assert!(catalog.lookup(0x40, 0).is_none());
    assert!(catalog.lookup(0x40, 44).is_none());
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    Send(u32, u32),
    Lookup,
    Callback(u32, u32),
    Flag(u32),
    Mission(u32, u32),
}

#[derive(Default)]
struct MemoryState {
    bytes: HashMap<u32, u8>,
    events: Vec<Event>,
    target: u32,
    record: u32,
    set_callback: bool,
    callback_address: u32,
    new_actor: u32,
    new_mission_key: u32,
}

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<MemoryState>>);

impl Memory {
    fn put32(&self, address: u32, value: u32) {
        let mut state = self.0.borrow_mut();
        for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
            state.bytes.insert(address.wrapping_add(i as u32), byte);
        }
    }

    fn get32(&self, address: u32) -> u32 {
        self.read32(address)
    }
}

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.borrow().bytes.get(&address).copied().unwrap_or(0)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().bytes.insert(address, value);
    }

    fn write32(&mut self, address: u32, value: u32) {
        let mut state = self.0.borrow_mut();
        for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
            state.bytes.insert(address.wrapping_add(i as u32), byte);
        }
        if address == state.record.wrapping_add(0x1570) && value == 1 {
            // Compare the owned record identity, not its guest field address.
            state.events.push(Event::Flag(address.wrapping_sub(0x1570)));
        }
    }
}

fn registers(esp: u32) -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp,
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
    }
}

struct InteractionCallees(Memory);

impl GuestCall for InteractionCallees {
    fn call(&mut self, address: u32, regs: &mut ExtRegisters) {
        // The lifted `RegsExt::call` trampoline supplies the nested return slot in production.
        // This test GuestCall does not, so its callee-visible first argument is at ESP itself.
        let arg0 = self.0.read32(regs.esp);
        let arg1 = self.0.read32(regs.esp.wrapping_add(4));
        let mission_key = self.0.read32(arg0.wrapping_add(0x60));
        let mut state = self.0.0.borrow_mut();
        match address {
            0x35aa70 => {
                state.events.push(Event::Send(arg0, arg1));
                let target = state.target;
                let callback = if state.set_callback {
                    state.callback_address
                } else {
                    0
                };
                let new_actor = state.new_actor;
                let record = state.record;
                drop(state);
                self.0.put32(target.wrapping_add(0x154), callback);
                self.0.put32(record.wrapping_add(0x14), new_actor);
                regs.eax = 0;
            }
            0xcafe => {
                state.events.push(Event::Callback(arg0, arg1));
                let target = state.target;
                let key = state.new_mission_key;
                drop(state);
                self.0.put32(target.wrapping_add(0x60), key);
                regs.eax = key;
            }
            0x2345d0 => {
                state.events.push(Event::Mission(arg0, mission_key));
                regs.eax = 0;
            }
            other => panic!("unexpected lifted callee {other:#x}"),
        }
    }
}

#[derive(Clone, Copy)]
struct NativeHost {
    player: u32,
    record: u32,
    actor: u32,
    network_reference: u32,
    set_callback: bool,
    callback: u32,
    mission_key: u32,
    callback_mission_key: u32,
}

#[derive(Default)]
struct NativeRuntime {
    host: Option<NativeHost>,
    events: Vec<Event>,
}

impl native::InteractionRuntime for NativeRuntime {
    fn capture_player(
        &self,
        player: native::PlayerId,
    ) -> Option<native::CapturedInteractionPlayer> {
        let host = self.host?;
        (host.player == player.0 as u32).then_some(native::CapturedInteractionPlayer {
            record: native::PlayerRecordId(host.record),
            actor: native::ActorId(host.actor),
        })
    }

    fn publish_target_reference(
        &mut self,
        player: native::PlayerId,
        _target: native::InteractionTargetId,
    ) {
        let host = self.host.unwrap();
        self.events
            .push(Event::Send(player.0 as u32, host.network_reference));
        // A sender-side mutation must be visible to the subsequent callback lookup.
        if !host.set_callback {
            self.host.as_mut().unwrap().callback = 0;
        }
    }

    fn callback_for_target(
        &mut self,
        _target: native::InteractionTargetId,
    ) -> Option<native::InteractionCallbackId> {
        self.events.push(Event::Lookup);
        let callback = self.host.unwrap().callback;
        (callback != 0).then_some(native::InteractionCallbackId(callback))
    }

    fn invoke_callback(
        &mut self,
        callback: native::InteractionCallbackId,
        target: native::InteractionTargetId,
        captured_actor: native::ActorId,
    ) {
        self.events
            .push(Event::Callback(target.0, captured_actor.0));
        assert_eq!(callback.0, self.host.unwrap().callback);
        let host = self.host.as_mut().unwrap();
        host.mission_key = host.callback_mission_key;
    }

    fn set_interacted_flag(&mut self, record: native::PlayerRecordId) {
        self.events.push(Event::Flag(record.0));
    }

    fn mark_mission_entries(&mut self, target: native::InteractionTargetId) {
        // This stand-in represents the typed mission sink's post-callback target-key read.
        let host = self.host.unwrap();
        self.events.push(Event::Mission(target.0, host.mission_key));
    }
}

/// T2377: 2,500 varied slot/object cases compare the native continuation with lifted A0D30.
/// The sender mutates the slot actor and callback field; the callback changes the mission key.
#[test]
fn t2377_interaction_continuation_matches_lifted_a0d30() {
    let mut seed = 0x7a39_4211u32;
    for case in 0..2500u32 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let player = (seed % 4) + 1;
        let record = RECORDS.wrapping_add(player.wrapping_mul(0x1584));
        let actor = 0x0080_0000u32.wrapping_add(seed & 0xffff);
        let new_actor = actor ^ 0x0100_0000;
        let target = TARGET.wrapping_add(case.wrapping_mul(0x200));
        let network_ref = seed.rotate_left(7) | 1;
        let callback = case % 3 != 0;
        let callback_address = 0xcafe;
        let new_key = seed.rotate_right(5) | 1;
        let initial_key = seed;

        let memory = Memory::default();
        {
            let mut state = memory.0.borrow_mut();
            state.target = target;
            state.record = record;
            state.set_callback = callback;
            state.callback_address = callback_address;
            state.new_actor = new_actor;
            state.new_mission_key = new_key;
        }
        memory.put32(0x7b0c48, RECORDS);
        memory.put32(record + 0x14, actor);
        memory.put32(target + 0x2c, network_ref);
        memory.put32(target + 0x60, initial_key);
        memory.put32(target + 0x154, if callback { 0xdead } else { 0 });
        memory.put32(STACK + 4, player);
        memory.put32(STACK + 8, target);
        let mut regs = registers(STACK);
        let mut callees = InteractionCallees(memory.clone());
        gameplay_player::game_player_object_ref_broadcast_net_0x36_callback_0x154_and_flag_0x1570(
            &mut memory.clone(),
            &mut callees,
            &mut regs,
        );
        let lifted_events = memory.0.borrow().events.clone();
        assert_eq!(memory.get32(record + 0x1570), 1, "case {case}");

        let mut native = NativeRuntime {
            host: Some(NativeHost {
                player,
                record,
                actor,
                network_reference: network_ref,
                set_callback: callback,
                callback: if callback { callback_address } else { 0 },
                mission_key: initial_key,
                callback_mission_key: new_key,
            }),
            events: Vec::new(),
        };
        native::submit_interaction(
            &mut native,
            native::PlayerId(player),
            native::InteractionTargetId(target),
        )
        .unwrap();
        let native_events: Vec<_> = native
            .events
            .into_iter()
            .filter(|event| *event != Event::Lookup)
            .collect();
        assert_eq!(native_events, lifted_events, "case {case}");
        if callback {
            assert_eq!(
                lifted_events[1],
                Event::Callback(target, actor),
                "case {case}"
            );
        }
    }
}

/// T2377: 2,500 fabricated rows compare secondary-ammo lookup and bounds to lifted 0x1B99F0.
#[test]
fn t2377_secondary_ammo_lookup_matches_lifted_getter() {
    let mut seed = 0x194e_a992u32;
    let mut entries = [0i32; native::ENTRY_COUNT];
    let mut records = [None; 66];
    let ammo_rows = [None; native::AMMO_TYPE_COUNT];
    let memory = Memory::default();
    let record_base = 0x0090_0000;
    memory.put32(0x4f9bac, record_base);

    for entry in 0..native::ENTRY_COUNT {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let record_index = (seed % 70) as i32 - 2;
        entries[entry] = record_index;
        memory.put32(0x4fa138 + (entry as u32) * 0x3c, record_index as u32);
        if (0..66).contains(&record_index) {
            seed = seed.rotate_left(9).wrapping_add(entry as u32);
            let ammo_type = seed & 0xff;
            records[record_index as usize] = Some(native::WeaponAmmoRecord {
                primary: native::AmmoPair {
                    ammo_type: ammo_type ^ 0x55,
                    amount: seed,
                },
                secondary: native::AmmoPair {
                    ammo_type,
                    amount: seed.rotate_right(3),
                },
            });
            memory.put32(
                record_base + (record_index as u32) * 0x29c + 0x1c,
                ammo_type,
            );
        }
    }
    let view = native::InventoryDefinitionView::new(&entries, &records, &ammo_rows);
    for case in 0..2500u32 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let entry_index = (seed % 80) as i32 - 5;
        memory.put32(STACK + 4, entry_index as u32);
        let mut regs = registers(STACK);
        engine_object::game_slot_4fa128_record_29c_dword_0x1c_by_slot_index(
            &mut memory.clone(),
            &mut regs,
        );
        assert_eq!(
            view.secondary_ammo_type(entry_index),
            regs.eax,
            "case {case}"
        );
    }
}

/// T2377: loaded ordinary/fixed definition tables preserve the original 0x54 stride and alias.
#[test]
fn t2377_definition_table_selection_matches_lifted_be860() {
    struct NoCalls;
    impl GuestCall for NoCalls {
        fn call(&mut self, address: u32, _regs: &mut ExtRegisters) {
            panic!("unexpected lifted callee {address:#x}");
        }
    }

    struct PrimaryAmmoCallee(u32);
    impl GuestCall for PrimaryAmmoCallee {
        fn call(&mut self, address: u32, regs: &mut ExtRegisters) {
            assert_eq!(address, 0x1b99b0);
            regs.eax = self.0;
        }
    }

    let ordinary_80: [Option<native::GameObjectDefinition>; 64] =
        std::array::from_fn(|_| Some(native::GameObjectDefinition::default()));
    let ordinary_100: [Option<native::GameObjectDefinition>; 64] =
        std::array::from_fn(|_| Some(native::GameObjectDefinition::default()));
    let ammo_rows: [Option<native::AmmoDefinitionRow>; native::AMMO_TYPE_COUNT] =
        std::array::from_fn(|index| {
            (index != 0).then_some(native::AmmoDefinitionRow {
                reserve_limit: 0,
                definition: native::GameObjectDefinition::default(),
            })
        });
    let fixed = native::GameObjectDefinition::default();
    let tables = [
        native::DefinitionTypeTable {
            type_code: 0x80,
            rows: &ordinary_80,
        },
        native::DefinitionTypeTable {
            type_code: 0x100,
            rows: &ordinary_100,
        },
    ];
    let catalog = native::DefinitionCatalog {
        ammo_rows: &ammo_rows,
        type_tables: &tables,
        fixed_type_800: Some(&fixed),
    };
    let memory = Memory::default();
    // The switch entry for type 0x40 is the primary-ammo table branch. Its getter is stubbed to
    // a valid ammo type so this differential isolates the original definition-address dispatch.
    memory.put32(0xbe934, 0xbe8ad);
    memory.put32(0xbe950 + 0x3f, 0); // selector for table entry 0xbe934
    let mut seed = 0x3a57_0bd1u32;
    for case in 0..2500u32 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let type_code = match seed % 4 {
            0 => 0x80,
            1 => 0x100,
            2 => 0x1000,
            _ => 0x800,
        };
        let index = ((seed >> 9) % 64) as i32;

        let ammo_type = (seed % 43) + 1;
        memory.put32(STACK + 4, 0x40);
        memory.put32(STACK + 8, index as u32);
        let mut ammo_regs = registers(STACK);
        engine_object::game_object_type_entry_address_by_type_mask_and_index(
            &mut memory.clone(),
            &mut PrimaryAmmoCallee(ammo_type),
            &mut ammo_regs,
        );
        assert!(
            catalog.lookup(0x40, ammo_type as i32).is_some(),
            "ammo case {case}"
        );
        assert_eq!(
            ammo_regs.eax,
            0x4ea484 + ammo_type * 0xe0,
            "ammo case {case}"
        );

        memory.put32(STACK + 4, type_code);
        memory.put32(STACK + 8, index as u32);
        let mut regs = registers(STACK);
        engine_object::game_object_type_entry_address_by_type_mask_and_index(
            &mut memory.clone(),
            &mut NoCalls,
            &mut regs,
        );
        let definition = catalog.lookup(type_code, index);
        assert!(definition.is_some(), "case {case}");
        let expected_address = match type_code {
            0x80 => {
                let selected = definition.unwrap();
                assert!(std::ptr::eq(
                    selected,
                    ordinary_80[index as usize].as_ref().unwrap()
                ));
                0x4dc3c0 + (index as u32) * 0x54
            }
            0x100 | 0x1000 => {
                let selected = definition.unwrap();
                assert!(std::ptr::eq(
                    selected,
                    ordinary_100[index as usize].as_ref().unwrap()
                ));
                0x4dc768 + (index as u32) * 0x54
            }
            0x800 => {
                assert!(std::ptr::eq(definition.unwrap(), &fixed));
                0x4dc568
            }
            _ => unreachable!(),
        };
        assert_eq!(regs.eax, expected_address, "case {case}");
    }
}

/// T2377: stored `+4` value and optional callback dispatch match lifted 0xDB2C0.
#[test]
fn t2377_resource_dispatch_returns_stored_value_or_callback_result() {
    struct Resolver;
    impl native::ResourceResolver for Resolver {
        fn resolve(&mut self, callback: u32, stored_resource_id: u32) -> u32 {
            stored_resource_id.rotate_left(callback & 31) ^ callback.wrapping_mul(17)
        }
    }
    struct Callee {
        memory: Memory,
        calls: Vec<(u32, u32)>,
    }
    impl GuestCall for Callee {
        fn call(&mut self, address: u32, regs: &mut ExtRegisters) {
            // `tail_call` advances past the root return slot before handing off to GuestCall.
            // The test callee omits the trampoline's synthetic return slot.
            let arg = self.memory.read32(regs.esp);
            self.calls.push((address, arg));
            regs.eax = arg.rotate_left(address & 31) ^ address.wrapping_mul(17);
        }
    }

    let definition_address = 0x0080_0000;
    let memory = Memory::default();
    let mut seed = 0x61c8_a24bu32;
    for case in 0..2500u32 {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let stored = seed.rotate_left(11);
        let callback = if case % 4 == 0 { 0 } else { seed | 1 };
        memory.put32(definition_address + 4, stored);
        memory.put32(definition_address + 0x48, callback);
        memory.put32(STACK + 4, definition_address);
        let mut regs = registers(STACK);
        let mut callee = Callee {
            memory: memory.clone(),
            calls: Vec::new(),
        };
        engine_object::game_object_invoke_callback_pointer_field_0x48_passing_field_0x4(
            &mut memory.clone(),
            &mut callee,
            &mut regs,
        );

        let mut words = [0u32; native::GameObjectDefinition::WORD_COUNT];
        words[1] = stored;
        words[18] = callback;
        let definition = native::GameObjectDefinition::from_words(words);
        let mut resolver = Resolver;
        let resolved = native::resolve_resource_id(&definition, &mut resolver);
        assert_eq!(resolved, regs.eax, "case {case}");
        if callback == 0 {
            assert!(callee.calls.is_empty(), "case {case}");
            assert_eq!(resolved, stored, "case {case}");
        } else {
            assert_eq!(callee.calls, vec![(callback, stored)], "case {case}");
        }
    }
}
