// SPDX-License-Identifier: GPL-3.0-or-later
// T2376: typed hand transfer and unload compared to the lifted original entrypoints.
use std::collections::BTreeMap;
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, Xmm},
    c2r::objchar_wired3,
    memory::GuestMemory,
};
use tsfp_state::{
    GuestMemory as StateMemory,
    inventory::{AMMO_TYPES, Inventory},
    weapon_hands::{
        AmmoType, HandAmmoState, HandEventEffects, HandPresentation, HandRealiseOutcome,
        HandRealisePublication, HandRealiseRequest, HandSide, OwnedObject, ReloadSession,
        ReloadSessionOutcome, ReserveGrant, WeaponAmmoRecord, WeaponHandSystem, WeaponHands,
        WeaponRecordId,
    },
};

const EXT: u32 = 0x200000;
const HAND: u32 = EXT + 0xa4;
const STACK: u32 = 0x300000;
const RECORDS: u32 = 0x400000;
const RECORD: u32 = RECORDS + 0x29c;

#[derive(Clone, Default)]
struct Image(BTreeMap<u32, u8>);
impl GuestMemory for Image {
    fn read8(&self, address: u32) -> u8 {
        *self.0.get(&address).unwrap_or(&0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}
impl StateMemory for Image {
    fn read8(&self, address: u32) -> u8 {
        GuestMemory::read8(self, address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        GuestMemory::write8(self, address, value);
    }
}
impl Image {
    fn word(&mut self, at: u32, value: u32) {
        GuestMemory::write32(self, at, value);
    }
    fn half(&mut self, at: u32, value: u16) {
        GuestMemory::write8(self, at, value as u8);
        GuestMemory::write8(self, at + 1, (value >> 8) as u8);
    }
    fn read_half(&self, at: u32) -> u16 {
        GuestMemory::read16(self, at)
    }
}

fn registers() -> ExtRegisters {
    ExtRegisters {
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
            control: 0x27f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

fn fixture() -> (Image, WeaponAmmoRecord) {
    let mut memory = Image::default();
    memory.word(0x4f9bac, RECORDS);
    memory.word(HAND + 0x30, 1);
    memory.word(RECORD + 0x14, 3);
    memory.word(RECORD + 0x1c, 4);
    memory.word(RECORD + 0x18, 12);
    memory.word(RECORD + 0x20, 9);
    (
        memory,
        WeaponAmmoRecord {
            id: WeaponRecordId(1),
            primary: AmmoType::new(3),
            secondary: AmmoType::new(4),
            primary_capacity: 12,
            secondary_capacity: 9,
            primary_single_round: false,
        },
    )
}

fn typed_state(memory: &Image) -> (Inventory, WeaponHands) {
    let inventory = Inventory::load(memory, EXT);
    let mut hands = WeaponHands::default();
    let mut hand = HandAmmoState::empty();
    hand.record = (GuestMemory::read32(memory, HAND + 0x30) != u32::MAX).then_some(WeaponRecordId(
        GuestMemory::read32(memory, HAND + 0x30) as u16,
    ));
    for ammo in 0..AMMO_TYPES {
        hand.loaded[ammo] = GuestMemory::read16(memory, HAND + 0x38 + ammo as u32 * 2) as i16;
        hand.saved_rounds[ammo] = GuestMemory::read16(memory, HAND + 0x90 + ammo as u32 * 2) as i16;
    }
    hands.hands[0] = hand;
    (inventory, hands)
}

fn random(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    *seed
}

#[test]
fn reloadable_count_matches_mechanical_reference_for_2048_seeded_states() {
    let mut seed = 0x2376_6e20;
    for case in 0..2048 {
        let (mut memory, record) = fixture();
        let capacity_primary = [0i32, 1, 2, 12, -2][(random(&mut seed) as usize) % 5];
        let capacity_secondary = [0i32, 1, 2, 9, -1][(random(&mut seed) as usize) % 5];
        memory.word(RECORD + 0x18, capacity_primary as u32);
        memory.word(RECORD + 0x20, capacity_secondary as u32);
        for ammo in [3u32, 4] {
            memory.half(EXT + 0x61e + ammo * 2, random(&mut seed) as u16);
            memory.half(HAND + 0x38 + ammo * 2, random(&mut seed) as u16);
        }
        memory.word(STACK, 0xfeedface);
        memory.word(STACK + 4, HAND);
        let (inventory, hands) = typed_state(&memory);
        let mut expected = registers();
        expected.esi = EXT;
        tsfp_ports::inventory_ammo::ammo_compute_reloadable_amount_for_both_ammo_t(
            &mut memory,
            &mut NoCallsForPure,
            &mut expected,
        );
        let actual = hands.reloadable_count(
            &inventory,
            HandSide::Right,
            WeaponAmmoRecord {
                primary_capacity: capacity_primary,
                secondary_capacity: capacity_secondary,
                ..record
            },
        );
        assert_eq!(
            expected.eax as i32,
            actual,
            "case {case}, capacities=({capacity_primary},{capacity_secondary}), reserves=({},{}), loaded=({}, {})",
            GuestMemory::read16(&memory, EXT + 0x61e + 6) as i16,
            GuestMemory::read16(&memory, EXT + 0x61e + 8) as i16,
            GuestMemory::read16(&memory, HAND + 0x38 + 6) as i16,
            GuestMemory::read16(&memory, HAND + 0x38 + 8) as i16
        );
        assert_eq!(expected.esp, STACK, "stack case {case}");
    }
}

struct NoCallsForPure;
impl tsfp_ports::abi::GuestCall for NoCallsForPure {
    fn call(&mut self, address: u32, _: &mut ExtRegisters) {
        panic!("unexpected reference call to {address:#x}");
    }
}

#[test]
fn transfer_matches_lifted_original_for_2048_seeded_states() {
    let mut seed = 0x2376_6ec0;
    for case in 0..2048 {
        let (mut memory, record) = fixture();
        let capacity_primary = [0i32, 1, 2, 12, -2][(random(&mut seed) as usize) % 5];
        let capacity_secondary = [0i32, 1, 2, 9, -1][(random(&mut seed) as usize) % 5];
        let reserve_primary = random(&mut seed) as i16;
        let reserve_secondary = random(&mut seed) as i16;
        let loaded_primary = random(&mut seed) as i16;
        let loaded_secondary = random(&mut seed) as i16;
        let saved_primary = random(&mut seed) as i16;
        let saved_secondary = random(&mut seed) as i16;
        let limited = case & 1 != 0;
        memory.half(EXT + 0x61e + 3 * 2, reserve_primary as u16);
        memory.half(EXT + 0x61e + 4 * 2, reserve_secondary as u16);
        memory.half(HAND + 0x38 + 3 * 2, loaded_primary as u16);
        memory.half(HAND + 0x38 + 4 * 2, loaded_secondary as u16);
        memory.half(HAND + 0x90 + 3 * 2, saved_primary as u16);
        memory.half(HAND + 0x90 + 4 * 2, saved_secondary as u16);
        memory.word(RECORD + 0x18, capacity_primary as u32);
        memory.word(RECORD + 0x20, capacity_secondary as u32);
        memory.word(STACK, 0xfeedface);
        memory.word(STACK + 4, EXT);
        memory.word(STACK + 8, 0);
        memory.word(STACK + 12, u32::from(limited));

        let (inventory, hands) = typed_state(&memory);
        let mut actual_regs = registers();
        actual_regs.eax = HAND;
        objchar_wired3::game_ammo_reload_transfer_reserve_to_clip_for_both_ammo_types(
            &mut memory,
            &mut actual_regs,
        );
        let mut expected_inventory = inventory;
        let mut expected_hands = hands.clone();
        let mut session = ReloadSession::new(HandSide::Right, limited, false);
        let ReloadSessionOutcome::Committed(moved) = session.apply(
            &mut expected_hands,
            &mut expected_inventory,
            WeaponAmmoRecord {
                primary_capacity: capacity_primary,
                secondary_capacity: capacity_secondary,
                ..record
            },
        ) else {
            panic!("fresh session must commit");
        };

        assert_eq!(
            actual_regs.eax as i32, moved,
            "case {case}, capacities=({capacity_primary},{capacity_secondary}), reserves=({reserve_primary},{reserve_secondary}), loaded=({loaded_primary},{loaded_secondary}), saved=({saved_primary},{saved_secondary}), limited={limited}"
        );
        assert_eq!(actual_regs.esp, STACK, "stack case {case}");
        assert_eq!(
            memory.read_half(EXT + 0x61e + 3 * 2),
            expected_inventory.ammo[3],
            "primary reserve case {case}"
        );
        assert_eq!(
            memory.read_half(EXT + 0x61e + 4 * 2),
            expected_inventory.ammo[4],
            "secondary reserve case {case}"
        );
        assert_eq!(
            memory.read_half(HAND + 0x38 + 3 * 2) as i16,
            expected_hands.hands[0].loaded[3],
            "primary loaded case {case}"
        );
        assert_eq!(
            memory.read_half(HAND + 0x38 + 4 * 2) as i16,
            expected_hands.hands[0].loaded[4],
            "secondary loaded case {case}"
        );
        assert_eq!(
            GuestMemory::read32(&memory, HAND + 0xf8),
            0,
            "reload timer case {case}"
        );
    }
}

#[test]
fn reload_session_cancel_is_a_mutation_guard_and_never_reverses_commit() {
    let (mut memory, record) = fixture();
    memory.half(EXT + 0x61e + 3 * 2, 7);
    let (mut reserve, mut hands) = typed_state(&memory);
    let mut cancelled = ReloadSession::new(HandSide::Right, false, false);
    cancelled.cancel();
    assert_eq!(
        cancelled.apply(&mut hands, &mut reserve, record),
        ReloadSessionOutcome::Cancelled
    );
    assert_eq!(reserve.ammo[3], 7);
    assert_eq!(hands.hands[0].loaded[3], 0);

    let mut committed = ReloadSession::new(HandSide::Right, false, false);
    let outcome = committed.apply(&mut hands, &mut reserve, record);
    assert!(matches!(outcome, ReloadSessionOutcome::Committed(moved) if moved > 0));
    let reserve_after_commit = reserve.ammo[3];
    let loaded_after_commit = hands.hands[0].loaded[3];
    committed.cancel();
    assert_eq!(committed.apply(&mut hands, &mut reserve, record), outcome);
    assert_eq!(reserve.ammo[3], reserve_after_commit);
    assert_eq!(hands.hands[0].loaded[3], loaded_after_commit);
}

#[test]
fn unload_matches_lifted_original_for_2048_seeded_states() {
    let mut seed = 0x2376_9760;
    for case in 0..2048 {
        let (mut memory, record) = fixture();
        let primary = if case % 7 == 0 { 3 } else { 5 };
        let secondary = if case % 5 == 0 { primary } else { 4 };
        memory.word(RECORD + 0x14, primary);
        memory.word(RECORD + 0x1c, secondary);
        memory.word(HAND + 0x30, if case % 11 == 0 { u32::MAX } else { 1 });
        for ammo in [primary, secondary] {
            memory.half(HAND + 0x38 + ammo * 2, random(&mut seed) as u16);
            memory.half(EXT + 0x61e + ammo * 2, random(&mut seed) as u16);
        }
        memory.word(STACK, 0xfeedface);
        memory.word(STACK + 4, EXT);
        memory.word(STACK + 8, HAND);

        let (mut inventory, mut hands) = typed_state(&memory);
        let matching = GuestMemory::read32(&memory, HAND + 0x30) == 1;
        let mut actual_regs = registers();
        objchar_wired3::game_ammo_unload_clip_back_into_reserve_both_types(
            &mut memory,
            &mut actual_regs,
        );
        let typed_record = WeaponAmmoRecord {
            id: record.id,
            primary: AmmoType::new(primary as u8),
            secondary: AmmoType::new(secondary as u8),
            ..record
        };
        hands.unload_without_clamp(&mut inventory, HandSide::Right, typed_record);
        assert_eq!(actual_regs.esp, STACK, "stack case {case}");
        for ammo in [primary, secondary] {
            assert_eq!(
                memory.read_half(HAND + 0x38 + ammo * 2) as i16,
                hands.hands[0].loaded[ammo as usize],
                "loaded case {case}"
            );
            assert_eq!(
                memory.read_half(EXT + 0x61e + ammo * 2),
                inventory.ammo[ammo as usize],
                "reserve case {case}"
            );
        }
        assert_eq!(hands.hands[0].record.is_some(), matching);
    }
}

#[test]
fn holster_saved_restore_and_realised_handoff_keep_state_separate() {
    let (memory, record) = fixture();
    let (inventory, hands) = typed_state(&memory);
    let mut system = WeaponHandSystem::new(inventory);
    system.hands = hands;
    system.inventory.ammo[3] = 4;
    system.inventory.ammo[4] = 2;
    system.hands.hands[0].loaded[3] = 8;
    system.hands.hands[0].loaded[4] = 5;
    system.hands.hands[0].loaded[2] = 7;
    let mut maxima = [0i16; AMMO_TYPES];
    maxima[2] = 3;
    maxima[3] = 10;
    maxima[4] = 6;
    system.hands.controllers[0].state = 3;
    system.hands.controllers[0].hand = Some(HandSide::Right.index());

    tsfp_state::weapon_hands::complete_holster_transition(
        &mut system.hands,
        &mut system.inventory,
        HandSide::Right,
        record,
        &maxima,
        7,
        0,
    );
    assert_eq!(system.hands.hands[0].saved_rounds[3], 8);
    assert_eq!(system.hands.hands[0].saved_rounds[4], 5);
    assert_eq!(system.inventory.ammo[2], 3);
    assert_eq!(system.inventory.ammo[3], 10);
    assert_eq!(system.inventory.ammo[4], 6);
    assert_eq!(system.hands.realised_entry, Some(7));
    assert_eq!(system.hands.controllers[0].state, 0);
    assert_eq!(system.hands.controllers[0].timer, 1000);

    let restored =
        system
            .hands
            .restore_saved(&mut system.inventory, HandSide::Right, record, false);
    assert_eq!(restored, 13);
    assert_eq!(system.hands.hands[0].loaded[3], 8);
    assert_eq!(system.hands.hands[0].loaded[4], 5);
    assert_eq!(system.inventory.ammo[3], 2);
    assert_eq!(system.inventory.ammo[4], 1);

    system.inventory.current = 5;
    assert!(system.request_handoff(8, 9));
    assert_eq!(system.inventory.current, 9);
    assert_eq!(system.hands.realised_entry, Some(5));
    assert!(
        system
            .hands
            .controllers
            .iter()
            .all(|slot| slot.state == 3 && slot.timer == 0)
    );
    system.commit_realised(9);
    assert_eq!(system.hands.realised_entry, Some(9));
    assert_eq!(system.inventory.previous, 9);
    assert_eq!(system.inventory.equipped[9], 1);
}

struct Presentation(
    Vec<String>,
    Vec<HandAmmoState>,
    bool,
    Vec<(HandSide, HandRealiseOutcome)>,
);
impl Default for Presentation {
    fn default() -> Self {
        Self(Vec::new(), Vec::new(), true, Vec::new())
    }
}
impl HandPresentation for Presentation {
    fn release_child(&mut self, child: u32) {
        self.0.push(format!("release:{child}"));
    }
    fn create_child(&mut self, record: WeaponRecordId, hand: HandAmmoState) -> Option<u32> {
        self.1.push(hand);
        self.0.push(format!("create:{}", record.0));
        self.2.then_some(0x1234)
    }
    fn attach_child(&mut self, side: HandSide, child: Option<u32>) {
        self.0.push(format!("attach:{side:?}:{child:?}"));
    }
    fn hand_committed(&mut self, side: HandSide, outcome: HandRealiseOutcome) {
        self.3.push((side, outcome));
    }
}

#[test]
fn availability_and_native_hand_attachment_use_typed_neighbors() {
    let (memory, record) = fixture();
    let (mut inventory, mut hands) = typed_state(&memory);
    let mut entries = tsfp_state::inventory::WeaponEntries {
        entries: [Default::default(); tsfp_state::inventory::ENTRY_COUNT],
        max_group: 0,
    };
    entries.entries[4].words[14] = 3;
    entries.entries[5].words[14] = 3;
    entries.entries[5].words[0] = 4;
    inventory.owned[5] = 1;
    assert!(hands.has_owned_x2(&inventory, &entries, 4));
    assert!(!hands.has_owned_x2(&inventory, &entries, 70));
    assert!(!WeaponHands::record_is_ammo_free(record));
    let detonators = WeaponHands::count_owned_detonators(
        EXT,
        &[
            OwnedObject {
                kind: 16,
                state: 16,
                owner: EXT,
            },
            OwnedObject {
                kind: 16,
                state: 0,
                owner: EXT,
            },
            OwnedObject {
                kind: 16,
                state: 16,
                owner: EXT + 1,
            },
        ],
    );
    assert_eq!(detonators, 1);
    assert!(WeaponHands::special_entry_available(
        0x2d, record, detonators
    ));
    assert!(!hands.reload_request_allowed(&inventory, HandSide::Right, record));

    let mut presentation = Presentation::default();
    hands.hands[0].child = Some(0x77);
    hands.hands[0].loaded[3] = 2;
    inventory.ammo[3] = 0;
    let next = WeaponAmmoRecord {
        id: WeaponRecordId(2),
        ..record
    };
    let moved = hands.realise_hand(
        &mut inventory,
        HandSide::Right,
        Some(record),
        next,
        &mut presentation,
        false,
    );
    let moved = moved.expect("hand replacement completed");
    assert_eq!(moved.transferred_rounds, 2);
    assert_eq!(
        moved.publication,
        HandRealisePublication::Replaced {
            record: next.id,
            retired_child: Some(0x77),
            created_child: Some(0x1234),
        }
    );
    assert_eq!(hands.hands[0].record, Some(WeaponRecordId(2)));
    assert_eq!(inventory.ammo[3], 0);
    assert_eq!(hands.hands[0].loaded[3], 2);
    assert_eq!(
        presentation.0,
        ["release:119", "create:2", "attach:Right:Some(4660)"]
    );
    let create_snapshot = presentation.1[0];
    assert_eq!(create_snapshot.record, Some(next.id));
    assert_eq!(create_snapshot.child, None);
    assert_eq!(create_snapshot.loaded[3], 0);
}

#[test]
fn hand_replacement_reports_no_child_after_old_state_is_committed() {
    let (memory, previous_record) = fixture();
    let (mut inventory, mut hands) = typed_state(&memory);
    hands.hand_mut(HandSide::Right).record = Some(previous_record.id);
    hands.hand_mut(HandSide::Right).child = Some(0x77);
    hands.hand_mut(HandSide::Right).loaded[3] = 3;
    inventory.ammo[3] = 1;
    let next_record = WeaponAmmoRecord {
        id: WeaponRecordId(12),
        ..WeaponAmmoRecord::default()
    };
    let mut presentation = Presentation::default();
    presentation.2 = false;

    let outcome = hands
        .realise_hand(
            &mut inventory,
            HandSide::Right,
            Some(previous_record),
            next_record,
            &mut presentation,
            false,
        )
        .expect("previous definition resolves");

    assert_eq!(
        outcome.publication,
        HandRealisePublication::Replaced {
            record: next_record.id,
            retired_child: Some(0x77),
            created_child: None,
        }
    );
    assert_eq!(outcome.transferred_rounds, 0);
    assert_eq!(hands.hand(HandSide::Right).record, Some(next_record.id));
    assert_eq!(hands.hand(HandSide::Right).child, None);
    assert_eq!(hands.hand(HandSide::Right).loaded[3], 0);
    assert_eq!(inventory.ammo[3], 4);
    assert_eq!(
        presentation.0,
        ["release:119", "create:12", "attach:Right:None"]
    );
}

#[test]
fn actor_hand_realisation_commits_right_then_left_independently() {
    let (memory, _) = fixture();
    let (mut inventory, mut hands) = typed_state(&memory);
    let ammo = AmmoType::new(3).unwrap();
    let previous_right = WeaponAmmoRecord {
        id: WeaponRecordId(1),
        primary: Some(ammo),
        primary_capacity: 9,
        ..WeaponAmmoRecord::default()
    };
    let previous_left = WeaponAmmoRecord {
        id: WeaponRecordId(2),
        ..previous_right
    };
    let next_right = WeaponAmmoRecord {
        id: WeaponRecordId(3),
        primary_capacity: 4,
        ..previous_right
    };
    let next_left = WeaponAmmoRecord {
        id: WeaponRecordId(4),
        primary_capacity: 2,
        ..previous_left
    };
    hands.realised_entry = Some(9);
    let right = hands.hand_mut(HandSide::Right);
    right.record = Some(previous_right.id);
    right.child = Some(17);
    right.loaded[ammo.index()] = 5;
    let left = hands.hand_mut(HandSide::Left);
    left.record = Some(previous_left.id);
    left.child = Some(34);
    left.loaded[ammo.index()] = 3;
    inventory.ammo[ammo.index()] = 1;

    let mut presentation = Presentation::default();
    let outcome = hands.realise_actor_update(
        &mut inventory,
        HandRealiseRequest {
            previous: Some(previous_right),
            next: next_right,
            restrict_primary_to_one: false,
        },
        HandRealiseRequest {
            previous: Some(previous_left),
            next: next_left,
            restrict_primary_to_one: false,
        },
        &mut presentation,
    );

    assert_eq!(
        outcome.right.map(|outcome| outcome.transferred_rounds),
        Ok(4)
    );
    assert_eq!(
        outcome.left.map(|outcome| outcome.transferred_rounds),
        Ok(2)
    );
    assert_eq!(hands.hand(HandSide::Right).record, Some(next_right.id));
    assert_eq!(hands.hand(HandSide::Right).loaded[ammo.index()], 4);
    assert_eq!(hands.hand(HandSide::Left).record, Some(next_left.id));
    assert_eq!(hands.hand(HandSide::Left).loaded[ammo.index()], 2);
    assert_eq!(inventory.ammo[ammo.index()], 3);
    assert_eq!(hands.realised_entry, Some(9));
    assert_eq!(
        presentation.0,
        [
            "release:17",
            "create:3",
            "attach:Right:Some(4660)",
            "release:34",
            "create:4",
            "attach:Left:Some(4660)",
        ]
    );
    assert_eq!(
        presentation
            .3
            .iter()
            .map(|(side, _)| *side)
            .collect::<Vec<_>>(),
        [HandSide::Right, HandSide::Left]
    );
    assert_eq!(presentation.3[0].1.transferred_rounds, 4);
    assert_eq!(presentation.3[1].1.transferred_rounds, 2);
}

#[test]
fn actor_hand_realisation_attempts_left_after_right_definition_error() {
    let (memory, _) = fixture();
    let (mut inventory, mut hands) = typed_state(&memory);
    let previous_right = WeaponAmmoRecord {
        id: WeaponRecordId(1),
        ..WeaponAmmoRecord::default()
    };
    let previous_left = WeaponAmmoRecord {
        id: WeaponRecordId(2),
        ..WeaponAmmoRecord::default()
    };
    let next_right = WeaponAmmoRecord {
        id: WeaponRecordId(3),
        ..WeaponAmmoRecord::default()
    };
    let next_left = WeaponAmmoRecord {
        id: WeaponRecordId(4),
        ..WeaponAmmoRecord::default()
    };
    hands.hand_mut(HandSide::Right).record = Some(previous_right.id);
    hands.hand_mut(HandSide::Right).child = Some(17);
    hands.hand_mut(HandSide::Left).record = Some(previous_left.id);
    hands.hand_mut(HandSide::Left).child = Some(34);

    let mut presentation = Presentation::default();
    let outcome = hands.realise_actor_update(
        &mut inventory,
        HandRealiseRequest {
            previous: None,
            next: next_right,
            restrict_primary_to_one: false,
        },
        HandRealiseRequest {
            previous: Some(previous_left),
            next: next_left,
            restrict_primary_to_one: false,
        },
        &mut presentation,
    );

    assert_eq!(
        outcome.right,
        Err(tsfp_state::weapon_hands::HandRealiseError::MissingPreviousRecord(previous_right.id))
    );
    assert_eq!(
        outcome.left.map(|outcome| outcome.transferred_rounds),
        Ok(0)
    );
    assert_eq!(hands.hand(HandSide::Right).record, Some(previous_right.id));
    assert_eq!(hands.hand(HandSide::Left).record, Some(next_left.id));
    assert_eq!(
        presentation.0,
        ["release:34", "create:4", "attach:Left:Some(4660)",]
    );
}

#[derive(Default)]
struct Events(Vec<(usize, u32, u32)>);
impl HandEventEffects for Events {
    fn enter_transition(&mut self, controller: usize, selector: u32, phase: u32) {
        self.0.push((controller, selector, phase));
    }
}

#[test]
fn wrapper_initialization_event_and_refill_seams_smoke() {
    let (memory, record) = fixture();
    let (inventory, mut hands) = typed_state(&memory);
    hands.hands[0].loaded[3] = 1;
    hands.hands[0].reload_timer = 7;
    hands.hands[0].hand_counters = [8, 9];
    hands.controllers[1].flags = 0x400;
    hands.controllers[1].attachment_target = Some(0x55);
    hands.controllers[1].timer = 10;

    let mut events = Events::default();
    hands.enter_realised_state(&mut inventory.clone(), 6, 1, 0x21, 3, &mut events);
    assert_eq!(events.0, [(1, 0x21, 3)]);
    assert!(hands.equipped_once[6]);

    let mut initialized = Vec::new();
    hands.initialize_hand_with(HandSide::Right, |side| initialized.push(side.index()));
    assert_eq!(initialized, [0]);
    assert_eq!(hands.hands[0], HandAmmoState::empty());

    let moved = hands.reload_then_reset(
        &mut inventory.clone(),
        HandSide::Right,
        1,
        record,
        false,
        false,
    );
    assert_eq!(moved, 0);
    assert_eq!(hands.controllers[1].flags, 0);
    assert_eq!(hands.controllers[1].attachment_target, None);
    assert_eq!(hands.controllers[1].timer, 1000);
    assert_eq!(hands.hands[0].reload_timer, 0);
    assert_eq!(hands.hands[0].hand_counters, [1000, 1000]);

    let mut inventory = inventory;
    inventory.current = 1;
    let result = hands.refill_and_reload(
        &mut inventory,
        &[ReserveGrant {
            ammo: AmmoType::new(3).unwrap(),
            amount: 2,
            maximum: 10,
        }],
        [Some(record), None],
        false,
    );
    assert_eq!(result.reserve_delta, 2);
    assert_eq!(result.rounds_loaded, 2);
    assert_eq!(inventory.ammo[3], 0);
    assert_eq!(hands.hands[0].loaded[3], 2);
}

#[test]
fn hand_reset_returns_prior_state_and_preserves_other_hand() {
    let mut hands = WeaponHands::default();
    let mut previous = HandAmmoState::empty();
    previous.record = Some(WeaponRecordId(7));
    previous.loaded[3] = 6;
    previous.saved_rounds[3] = 4;
    previous.reload_timer = -2;
    previous.hand_counters = [13, 17];
    previous.child = Some(0x1234);
    let mut other = HandAmmoState::empty();
    other.record = Some(WeaponRecordId(9));
    other.loaded[5] = 2;
    other.child = Some(0x5678);
    hands.hands[HandSide::Left.index()] = previous;
    hands.hands[HandSide::Right.index()] = other;

    let receipt = hands.reset_hand(HandSide::Left);

    assert_eq!(receipt.side, HandSide::Left);
    assert_eq!(receipt.previous, previous);
    assert_eq!(hands.hand(HandSide::Left), &HandAmmoState::empty());
    assert_eq!(hands.hand(HandSide::Right), &other);
}

#[test]
fn measured_hand_counter_branch_decrements_pair_only_for_zero_owner_gate() {
    let mut hands = WeaponHands::default();
    hands.hands[HandSide::Right.index()].hand_counters = [-1, i32::MIN];
    hands.hands[HandSide::Left.index()].hand_counters = [8, 9];

    assert!(hands.decrement_hand_counters_if_owner_gate_zero(HandSide::Right, 0));
    assert_eq!(hands.hand(HandSide::Right).hand_counters, [-2, i32::MAX]);
    assert_eq!(hands.hand(HandSide::Left).hand_counters, [8, 9]);

    assert!(!hands.decrement_hand_counters_if_owner_gate_zero(HandSide::Right, 1));
    assert_eq!(hands.hand(HandSide::Right).hand_counters, [-2, i32::MAX]);
}
