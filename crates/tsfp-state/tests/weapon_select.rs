// SPDX-License-Identifier: GPL-3.0-or-later
// T2193: FABRICATED inputs, translated reference roots, scripted neighbour effects.
use std::{cell::RefCell, rc::Rc};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
};
use tsfp_state::{inventory::*, weapon_select::*};

#[derive(Clone)]
struct Image(Rc<RefCell<Vec<u8>>>);
impl GuestMemory for Image {
    fn read8(&self, a: u32) -> u8 {
        self.0.borrow()[a as usize]
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.borrow_mut()[a as usize] = v;
    }
}
impl tsfp_state::GuestMemory for Image {
    fn read8(&self, a: u32) -> u8 {
        GuestMemory::read8(self, a)
    }
    fn write8(&mut self, a: u32, v: u8) {
        GuestMemory::write8(self, a, v);
    }
}
const EXT: u32 = 0x200000;
const PLAYER: u32 = 0x210000;
const ACTOR: u32 = 0x220000;
const RECORDS: u32 = 0x230000;
const STACK: u32 = 0x240000;
fn regs() -> ExtRegisters {
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
            stack: [0.; 8],
            top: 0,
            control: 0x300,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}
fn image() -> Image {
    Image(Rc::new(RefCell::new(vec![0; 0x800000])))
}
fn word(m: &mut Image, a: u32, v: u32) {
    GuestMemory::write32(m, a, v);
}
fn half(m: &mut Image, a: u32, v: u16) {
    GuestMemory::write8(m, a, v as u8);
    GuestMemory::write8(m, a + 1, (v >> 8) as u8);
}
fn read(m: &Image, a: u32) -> u32 {
    GuestMemory::read32(m, a)
}
fn random(s: &mut u32) -> u32 {
    *s = s.wrapping_mul(1664525).wrapping_add(1013904223);
    *s
}

struct Gate {
    m: Image,
    hidden: bool,
    exception: bool,
    detonator: bool,
    seen: Vec<u32>,
}
impl Gate {
    fn metadata(&self, index: i32) -> Option<AmmoRecord> {
        (index != -1).then(|| AmmoRecord {
            primary: read(&self.m, RECORDS + index as u32 * 0x29c + 0x14) as usize,
            secondary: read(&self.m, RECORDS + index as u32 * 0x29c + 0x1c) as usize,
        })
    }
}
impl SelectabilityContext for Gate {
    fn local_player(&self) -> bool {
        (read(&self.m, EXT + 8) as i32) < read(&self.m, 0x790950) as i32
    }
    fn hands_disabled(&self) -> bool {
        read(&self.m, 0x7de458) & 0x2000 != 0
    }
    fn record(&self, index: i32) -> Option<AmmoRecord> {
        self.metadata(index)
    }
    fn loaded_hands(&self) -> [LoadedHand; 2] {
        let prev = read(&self.m, EXT + EXT_PREVIOUS);
        core::array::from_fn(|i| {
            let record = self.metadata(read(
                &self.m,
                ENTRY_TABLE + prev * ENTRY_STRIDE + 12 + 4 * i as u32,
            ) as i32);
            let rounds = core::array::from_fn(|t| {
                GuestMemory::read16(&self.m, EXT + [0xa4, 0x1d4][i] + 0x38 + 2 * t as u32)
            });
            LoadedHand { record, rounds }
        })
    }
    fn owned_x2(&mut self, _: &Inventory, _: usize) -> bool {
        self.seen.push(0x1b68f0);
        self.hidden
    }
    fn ammo_exception(&mut self, _: &Inventory, _: usize) -> bool {
        self.seen.push(0x1b6950);
        self.exception
    }
    fn detonator_available(&mut self, _: &Inventory) -> bool {
        self.seen.push(0xe02b0);
        self.detonator
    }
    fn hand_transition_active(&self) -> bool {
        read(&self.m, EXT + 0x80) != 0
    }
    fn left_secondary_required(&self) -> bool {
        let record = read(&self.m, ENTRY_TABLE + 0x31 * ENTRY_STRIDE + 12);
        read(&self.m, RECORDS + record * 0x29c + 12) & 0x1000 != 0
    }
}
impl GuestCall for Gate {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        self.seen.push(va);
        r.eax = match va {
            0x1b68f0 => self.hidden,
            0x1b6950 => self.exception,
            0xe02b0 => self.detonator,
            _ => panic!("unexpected {va:x}"),
        } as u32;
    }
}

#[test]
fn selectable_matches_translated_root_4096_seeds() {
    let mut m = image();
    let mut seed = 2193;
    for case in 0..4096 {
        let entry = case % ENTRY_COUNT;
        word(&mut m, EXT + 8, random(&mut seed) % 4);
        word(&mut m, 0x790950, 2);
        word(&mut m, 0x7de458, random(&mut seed) & 0x2000);
        word(&mut m, 0x4f9bac, RECORDS);
        word(&mut m, EXT + EXT_CURRENT, random(&mut seed) % 70);
        word(&mut m, EXT + EXT_PREVIOUS, random(&mut seed) % 70);
        word(&mut m, EXT + 0x80, random(&mut seed) % 2);
        for i in 0..ENTRY_COUNT {
            word(
                &mut m,
                ENTRY_TABLE + i as u32 * ENTRY_STRIDE,
                random(&mut seed) & 0x112,
            );
            for offset in [12, 16] {
                let record = random(&mut seed) % 5;
                word(
                    &mut m,
                    ENTRY_TABLE + i as u32 * ENTRY_STRIDE + offset,
                    if record == 4 { u32::MAX } else { record },
                );
            }
            GuestMemory::write8(
                &mut m,
                EXT + EXT_OWNED + i as u32,
                (random(&mut seed) % 3 != 0) as u8,
            );
        }
        // Entry 0x31's left record must exist for its conditional metadata read.
        word(&mut m, ENTRY_TABLE + 0x31 * ENTRY_STRIDE + 12, 0);
        for i in 0..4 {
            word(&mut m, RECORDS + i * 0x29c + 0x14, random(&mut seed) % 4);
            word(&mut m, RECORDS + i * 0x29c + 0x1c, random(&mut seed) % 4);
            word(&mut m, RECORDS + i * 0x29c + 12, random(&mut seed) & 0x1000);
        }
        for t in 0..AMMO_TYPES as u32 {
            for offset in [EXT_AMMO, 0xdc, 0x20c] {
                // Include signed values, zero and nonzero reserves/loaded words.
                let value = [0, 1, 0xffff, 0x8000][(random(&mut seed) % 4) as usize];
                half(&mut m, EXT + offset + 2 * t, value);
            }
        }
        let hidden = random(&mut seed) & 0x100 != 0;
        let exception = random(&mut seed) & 0x100 != 0;
        let detonator = random(&mut seed) & 0x100 != 0;
        let ignore = case & 1 != 0;
        let mut reference = Gate {
            m: m.clone(),
            hidden,
            exception,
            detonator,
            seen: vec![],
        };
        let mut readable = Gate {
            m: m.clone(),
            hidden,
            exception,
            detonator,
            seen: vec![],
        };
        let inventory = Inventory::load(&m, EXT);
        let table = WeaponEntries::load(&m);
        let expected = selectable(&inventory, &table, entry, ignore, &mut readable);
        let mut r = regs();
        word(&mut m, STACK + 4, EXT);
        word(&mut m, STACK + 8, entry as u32);
        word(&mut m, STACK + 12, ignore as u32);
        tsfp_ports::inventory_select::weapon_entry_selectable(&mut m, &mut reference, &mut r);
        assert_eq!(expected, r.eax != 0, "case {case} entry {entry}");
        assert_eq!(readable.seen, reference.seen, "callee order case {case}");
    }
}

struct Cycle {
    m: Image,
    polls: [u32; 5],
    poll: usize,
    seen: Vec<(u32, u32)>,
    unchanged: bool,
}
impl Cycle {
    fn log(&mut self, va: u32, arg: u32) {
        self.seen.push((va, arg));
    }
    fn next_poll(&mut self) -> u32 {
        let v = self.polls[self.poll];
        self.poll += 1;
        v
    }
    fn result(&self, va: u32, current: u32) -> u32 {
        if self.unchanged {
            current
        } else {
            (current
                + match va {
                    0x1baaf0 => 1,
                    0x1baba0 => 2,
                    0x1ba8f0 => 3,
                    0x1ba9b0 => 4,
                    _ => 5,
                })
                % 70
        }
    }
}
impl GuestCall for Cycle {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        match va {
            0x427b0 => {
                self.log(va, read(&self.m, r.esp));
                r.eax = self.next_poll();
            }
            0x238510 => {
                assert_eq!(read(&self.m, r.esp), 0x3e);
                assert_eq!(read(&self.m, r.esp + 8), 1f32.to_bits());
                self.log(va, read(&self.m, r.esp + 4));
            }
            0x1baaf0 | 0x1baba0 | 0x1ba8f0 | 0x1ba9b0 => {
                let current = read(&self.m, EXT + EXT_CURRENT);
                assert_eq!(read(&self.m, r.esp), EXT);
                assert_eq!(read(&self.m, r.esp + 8), 1);
                if matches!(va, 0x1baaf0 | 0x1baba0) {
                    assert_eq!(read(&self.m, r.esp + 4), 1);
                } else {
                    assert_eq!(read(&self.m, r.esp + 4), current);
                }
                self.log(va, current);
                r.eax = self.result(va, current);
            }
            0x1ba130 => {
                let entry = read(&self.m, r.esp + 4);
                self.log(va, entry);
                word(&mut self.m, EXT + EXT_CURRENT, entry);
            }
            0x1427d0 => self.log(va, read(&self.m, r.esp)),
            0x1b6bf0 => {
                let entry = read(&self.m, r.esp + 4);
                self.log(va, entry);
                r.eax = (entry + 1) % 70;
            }
            0x142820 | 0x142880 | 0x142900 | 0x389b0 => self.log(va, 0),
            _ => panic!("unexpected {va:x}"),
        }
    }
}
impl SelectionEffects for Cycle {
    fn buttons(&mut self) -> u32 {
        self.log(0x427b0, 0);
        self.next_poll()
    }
    fn candidate(&mut self, inv: &Inventory, request: Request) -> u32 {
        let va = match request {
            Request::Cycle(Direction::Next) => 0x1baaf0,
            Request::Cycle(Direction::Previous) => 0x1baba0,
            Request::AltMode(Direction::Next) => 0x1ba8f0,
            Request::AltMode(Direction::Previous) => 0x1ba9b0,
            Request::CycleThrowable => {
                self.log(0x1b6bf0, inv.throwable);
                return (inv.throwable + 1) % 70;
            }
            _ => unreachable!(),
        };
        self.log(va, inv.current);
        self.result(va, inv.current)
    }
    fn equip(&mut self, inv: &mut Inventory, entry: u32) {
        self.log(0x1ba130, entry);
        inv.set_current(&mut self.m, entry);
    }
    fn count_change(&mut self) {
        self.log(0x238510, 0);
    }
    fn group_changed(&mut self) {
        self.log(0x142820, 0);
    }
    fn variant_changed(&mut self, direction: Direction) {
        self.log(
            if direction == Direction::Next {
                0x142880
            } else {
                0x142900
            },
            0,
        );
    }
    fn variant_unchanged(&mut self) {
        self.log(0x1427d0, PLAYER);
    }
    fn throwable_changed(&mut self, inv: &mut Inventory, entry: u32) {
        inv.set_throwable(&mut self.m, entry);
        self.log(0x389b0, 0);
    }
}

#[test]
fn cycle_matches_translated_root_4096_seeds() {
    let mut seed = 2193;
    for case in 0..4096 {
        let mut m = image();
        word(&mut m, 0x7b0c7c, PLAYER);
        word(&mut m, PLAYER + 0x14, ACTOR);
        word(&mut m, ACTOR + 0x7c, EXT);
        word(&mut m, EXT + EXT_CURRENT, random(&mut seed) % 70);
        word(&mut m, EXT + EXT_THROWABLE, random(&mut seed) % 70);
        let bindings = ButtonBindings {
            next: 1,
            previous: 2,
            alt_next: 4,
            alt_previous: 8,
            throwable: 16,
        };
        for (offset, mask) in [(0x3c, 1), (0x38, 2), (0x44, 4), (0x40, 8), (0x48, 16)] {
            word(&mut m, PLAYER + offset, mask);
        }
        let locked = case & 1 != 0;
        word(&mut m, PLAYER + 0xc, if locked { 0x20 } else { 0 });
        word(&mut m, PLAYER + 0x58c, 0);
        word(&mut m, 0x4f9bac, RECORDS);
        word(&mut m, RECORDS + 12, if locked { 0x100000 } else { 0 });
        let polls = core::array::from_fn(|_| random(&mut seed) >> 16);
        let unchanged = case & 2 != 0;
        let mut inv = Inventory::load(&m, EXT);
        let mut reference = Cycle {
            m: m.clone(),
            polls,
            poll: 0,
            seen: vec![],
            unchanged,
        };
        let readable_image = Image(Rc::new(RefCell::new(m.0.borrow().clone())));
        let mut readable = Cycle {
            m: readable_image,
            polls,
            poll: 0,
            seen: vec![],
            unchanged,
        };
        cycle_buttons(&mut inv, bindings, locked, &mut readable);
        let mut r = regs();
        tsfp_ports::c2r::t1925_weapons::game_player_weapon_cycle_buttons_next_previous_and_slot_order_toggle_handler(&mut m, &mut reference, &mut r);
        assert_eq!(
            inv.current,
            read(&m, EXT + EXT_CURRENT),
            "current case {case}"
        );
        assert_eq!(
            inv.throwable,
            read(&m, EXT + EXT_THROWABLE),
            "throwable case {case}"
        );
        assert_eq!(readable.seen, reference.seen, "effects case {case}");
        assert_eq!(readable.poll, reference.poll);
    }
}

struct Controller {
    blocked: bool,
    seen_cycle: bool,
}
impl GuestCall for Controller {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        if va == 0x56e90 {
            r.eax = self.blocked as u32;
        }
        if va == 0x1bf140 {
            self.seen_cycle = true;
        }
    }
}
#[test]
fn controller_selection_branch_matches_both_translations_4096_seeds() {
    let mut m = image();
    let mut seed = 2193;
    word(&mut m, 0x7b0c7c, PLAYER);
    word(&mut m, PLAYER + 0x14, ACTOR);
    word(&mut m, ACTOR + 0x7c, EXT);
    word(&mut m, EXT + EXT_PREVIOUS, 0);
    word(&mut m, ENTRY_TABLE + 12, u32::MAX);
    // Safe finite synthetic constants, no owner data; remaining controller effects are outside this comparison.
    for a in [0x475d0c, 0x475ce0, 0x4765e0, 0x4e8cec, 0x4785d0] {
        word(&mut m, a, 1f32.to_bits());
    }
    word(&mut m, PLAYER + 0x58c, u32::MAX);
    for case in 0..4096 {
        let state = ControllerState {
            player_index: if case & 1 != 0 { -1 } else { 0 },
            match_state: if case & 2 != 0 { 8 } else { 1 },
            blocked_game_state: case & 4 != 0,
            global_input_blocked: case & 8 != 0,
            player_flags: random(&mut seed),
            actor_flags: random(&mut seed),
        };
        word(&mut m, PLAYER + 4, state.player_index as u32);
        word(&mut m, 0x7de30c, state.match_state);
        word(&mut m, 0x6d60a8, state.global_input_blocked as u32);
        word(&mut m, PLAYER + 0xc, state.player_flags);
        word(&mut m, PLAYER + 0x114, state.actor_flags);
        for wired in [false, true] {
            word(&mut m, EXT + EXT_CURRENT, 5);
            let mut guest = Controller {
                blocked: state.blocked_game_state,
                seen_cycle: false,
            };
            let mut r = regs();
            if wired {
                tsfp_ports::c2r::player_wired::game_player_weapon_controller_per_frame_cycle_buttons_view_sway_and_dispatch_slot_updates(&mut m,&mut guest,&mut r);
            } else {
                tsfp_ports::c2r::t1925_weapons::game_player_weapon_controller_per_frame_cycle_buttons_view_sway_and_dispatch_slot_updates(&mut m,&mut guest,&mut r);
            }
            let expected = controller_action(state);
            assert_eq!(
                guest.seen_cycle,
                expected == ControllerAction::ReadButtons,
                "case {case} wired {wired}"
            );
            assert_eq!(
                read(&m, EXT + EXT_CURRENT) == 0,
                expected == ControllerAction::ClearCurrent,
                "clear case {case} wired {wired}"
            );
        }
    }
}

#[test]
fn direct_requests_validate_and_ab_defaults_to_reference() {
    let m = image();
    let mut inv = Inventory::load(&m, EXT);
    let mut effects = Cycle {
        m: m.clone(),
        polls: [0; 5],
        poll: 0,
        seen: vec![],
        unchanged: false,
    };
    assert!(!apply_request(
        &mut inv,
        Request::SelectSlot(70),
        &mut effects,
        |_, _| panic!("out of bounds gate")
    ));
    assert!(!apply_request(
        &mut inv,
        Request::SelectSlot(2),
        &mut effects,
        |_, _| false
    ));
    assert!(effects.seen.is_empty());
    assert!(!dispatch_request(
        SelectionPreset::default(),
        &mut inv,
        Request::SelectSlot(2),
        &mut effects,
        |_, _| panic!("reference must not call readable gate"),
        |_, request, _| {
            assert_eq!(request, Request::SelectSlot(2));
            false
        }
    ));
    assert!(dispatch_request(
        SelectionPreset::ReadableOriginal,
        &mut inv,
        Request::SelectSlot(2),
        &mut effects,
        |_, slot| slot == 2,
        |_, _, _| panic!("readable must not call reference")
    ));
    assert_eq!(inv.current, 2);
    assert_eq!(read(&m, EXT + EXT_CURRENT), 2);
    assert_eq!(effects.seen, [(0x1ba130, 2), (0x142820, 0)]);
}

#[test]
fn blocked_controller_clears_owned_current_with_write_through() {
    let mut m = image();
    word(&mut m, EXT + EXT_CURRENT, 5);
    let mut inv = Inventory::load(&m, EXT);
    let mut effects = Cycle {
        m: m.clone(),
        polls: [0; 5],
        poll: 0,
        seen: vec![],
        unchanged: false,
    };
    controller_selection(
        &mut inv,
        &mut m,
        ControllerState {
            blocked_game_state: true,
            ..ControllerState::default()
        },
        ButtonBindings::default(),
        false,
        &mut effects,
    );
    assert_eq!(inv.current, 0);
    assert_eq!(read(&m, EXT + EXT_CURRENT), 0);
    assert!(effects.seen.is_empty());
    assert_eq!(inv.diff(&m), None);
}

#[test]
fn direct_slot_numbering_and_rejections() {
    use tsfp_state::weapon_slot::*;
    let m = image();
    let inv = Inventory::load(&m, EXT);
    let mut table = WeaponEntries::load(&m);
    table.entries[3].words[0] = FLAG_LEFT;
    let order = [0, 1, 70, u32::MAX, 2, 2, 3, 0x28, 0x29, 0x2a, 4, 5];
    assert_eq!(WeaponSlot::new(0), None);
    assert_eq!(WeaponSlot::new(10), None);
    for (number, expected) in [(1, Some(2)), (2, Some(4)), (3, Some(5)), (9, None)] {
        assert_eq!(
            resolve_slot(
                &inv,
                &table,
                &order,
                WeaponSlot::new(number).unwrap(),
                |_, _| true
            ),
            expected
        );
    }
    assert_eq!(
        resolve_slot(&inv, &table, &order, WeaponSlot::new(1).unwrap(), |_, e| e
            == 5),
        Some(5)
    );
    assert_eq!(
        resolve_slot(
            &inv,
            &table,
            &[],
            WeaponSlot::new(1).unwrap(),
            |_, _| panic!("empty order")
        ),
        None
    );
}

struct DirectCycle {
    cycle: Cycle,
    target: u32,
}
impl GuestCall for DirectCycle {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        if va == 0x1baaf0 {
            assert_eq!(read(&self.cycle.m, r.esp), EXT);
            assert_eq!(read(&self.cycle.m, r.esp + 4), 1);
            assert_eq!(read(&self.cycle.m, r.esp + 8), 1);
            r.eax = self.target;
        } else {
            self.cycle.call(va, r);
        }
    }
}

#[test]
fn direct_slot_matches_translated_gate_and_cycle_effects_4096_seeds() {
    use tsfp_state::weapon_slot::*;
    let mut m = image();
    let mut seed = 2182;
    word(&mut m, EXT + 8, 0);
    word(&mut m, 0x790950, 1);
    word(&mut m, 0x7b0c7c, PLAYER);
    word(&mut m, PLAYER + 0x14, ACTOR);
    word(&mut m, ACTOR + 0x7c, EXT);
    word(&mut m, PLAYER + 0x58c, u32::MAX);
    word(&mut m, PLAYER + 0x3c, 1);
    let state = ControllerState {
        actor_flags: 2,
        ..ControllerState::default()
    };
    for case in 0..4096 {
        for entry in 0..ENTRY_COUNT {
            word(
                &mut m,
                ENTRY_TABLE + entry as u32 * ENTRY_STRIDE,
                random(&mut seed) >> 24 & 0x11a,
            );
            word(
                &mut m,
                ENTRY_TABLE + entry as u32 * ENTRY_STRIDE + 12,
                u32::MAX,
            );
            word(
                &mut m,
                ENTRY_TABLE + entry as u32 * ENTRY_STRIDE + 16,
                u32::MAX,
            );
            GuestMemory::write8(
                &mut m,
                EXT + EXT_OWNED + entry as u32,
                (random(&mut seed) >> 31) as u8,
            );
        }
        // Entry 0x31 inspects left-record metadata even when the extra gate is off.
        word(&mut m, 0x4f9bac, RECORDS);
        word(&mut m, ENTRY_TABLE + 0x31 * ENTRY_STRIDE + 12, 0);
        word(&mut m, EXT + EXT_CURRENT, 2);
        let table = WeaponEntries::load(&m);
        let mut inv = Inventory::load(&m, EXT);
        // Synthetic runtime order, not a committed copy of the retail table.
        let order: [u32; ENTRY_COUNT] = core::array::from_fn(|i| ((i + case) % ENTRY_COUNT) as u32);
        let number = (case % 11) as u8;
        let hidden = random(&mut seed) >> 31 != 0;
        let gate = |m: Image| Gate {
            m,
            hidden,
            exception: false,
            detonator: false,
            seen: vec![],
        };
        let mut reference = gate(m.clone());
        let mut expected = None;
        if (1..=9).contains(&number) {
            let mut remaining = number;
            for &raw in &order {
                let entry = raw as usize;
                if entry <= 1
                    || (0x28..=0x2a).contains(&entry)
                    || table.entries[entry].flags() & FLAG_LEFT != 0
                {
                    continue;
                }
                word(&mut m, STACK + 4, EXT);
                word(&mut m, STACK + 8, raw);
                word(&mut m, STACK + 12, 1);
                let mut r = regs();
                tsfp_ports::inventory_select::weapon_entry_selectable(
                    &mut m,
                    &mut reference,
                    &mut r,
                );
                if r.eax != 0 {
                    remaining -= 1;
                    if remaining == 0 {
                        expected = Some(raw);
                        break;
                    }
                }
            }
        }
        let readable_image = Image(Rc::new(RefCell::new(m.0.borrow().clone())));
        let mut readable_gate = gate(readable_image.clone());
        let mut effects = Cycle {
            m: readable_image,
            polls: [0; 5],
            poll: 0,
            seen: vec![],
            unchanged: false,
        };
        assert_eq!(
            request_slot(
                DirectSlotPreset::SelectableOrder,
                state,
                &mut inv,
                &table,
                &order,
                number,
                &mut readable_gate,
                &mut effects
            ),
            expected.is_some(),
            "case {case}"
        );
        assert_eq!(readable_gate.seen, reference.seen, "gate order case {case}");
        if let Some(target) = expected {
            let mut oracle = DirectCycle {
                target,
                cycle: Cycle {
                    m: m.clone(),
                    polls: [1, 0, 0, 0, 0],
                    poll: 0,
                    seen: vec![],
                    unchanged: false,
                },
            };
            tsfp_ports::c2r::t1925_weapons::game_player_weapon_cycle_buttons_next_previous_and_slot_order_toggle_handler(&mut m, &mut oracle, &mut regs());
            let equip_hud: Vec<_> = oracle
                .cycle
                .seen
                .into_iter()
                .filter(|(va, _)| matches!(va, 0x1ba130 | 0x142820))
                .collect();
            assert_eq!(effects.seen, equip_hud, "equip/HUD case {case}");
            assert_eq!(inv.current, read(&m, EXT + EXT_CURRENT));
        } else {
            assert!(effects.seen.is_empty());
            assert_eq!(inv.current, 2);
        }
        // Off and ineligible paths never even call the gate or effects.
        for (preset, controller) in [
            (DirectSlotPreset::default(), state),
            (
                DirectSlotPreset::SelectableOrder,
                ControllerState::default(),
            ),
            (
                DirectSlotPreset::SelectableOrder,
                ControllerState {
                    blocked_game_state: true,
                    ..state
                },
            ),
        ] {
            let before = effects.seen.clone();
            let calls = readable_gate.seen.clone();
            assert!(!request_slot(
                preset,
                controller,
                &mut inv,
                &table,
                &order,
                1,
                &mut readable_gate,
                &mut effects
            ));
            assert_eq!(effects.seen, before);
            assert_eq!(readable_gate.seen, calls);
        }
    }
}
