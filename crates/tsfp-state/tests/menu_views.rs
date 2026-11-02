// SPDX-License-Identifier: GPL-3.0-or-later
//! FABRICATED contract fixtures; no lifted or whole-system evidence.
use std::cell::RefCell;
use std::collections::BTreeMap;
use tsfp_state::GuestMemory;
use tsfp_state::menu::{
    CLICK_SOUND_ENABLED, CLICK_SOUND_REQUEST, MenuState, MenuStateView, OPEN_TRANSITION_FLAG,
    PLAYER_SLOT, UI_STATE_OBJECT,
};

#[derive(Default)]
struct Cells {
    words: BTreeMap<u32, u32>,
    accesses: RefCell<Vec<(bool, u32)>>,
}

impl GuestMemory for Cells {
    fn read8(&self, _: u32) -> u8 {
        panic!("view must preserve dword access")
    }
    fn write8(&mut self, _: u32, _: u8) {
        panic!("view must preserve dword access")
    }
    fn read32(&self, address: u32) -> u32 {
        self.accesses.borrow_mut().push((false, address));
        *self.words.get(&address).expect("unexpected cell read")
    }
    fn write32(&mut self, address: u32, value: u32) {
        self.accesses.borrow_mut().push((true, address));
        self.words.insert(address, value);
    }
}

#[test]
fn transition_flag_is_one_live_dword_write() {
    let mut m = Cells::default();
    MenuStateView.set_open_transition_flag(&mut m, 1);
    assert_eq!(*m.accesses.borrow(), [(true, OPEN_TRANSITION_FLAG)]);
    assert_eq!(m.words[&OPEN_TRANSITION_FLAG], 1);
}

#[test]
fn live_view_reads_only_requested_field_and_observes_intervening_writers() {
    let mut m = Cells::default();
    // Deliberately leave unrelated fields unmapped: a snapshot load would fault.
    m.words.insert(CLICK_SOUND_ENABLED, 2);
    assert_eq!(MenuStateView.click_sound_enabled(&m), 2);
    m.words.insert(CLICK_SOUND_ENABLED, u32::MAX);
    assert_eq!(MenuStateView.click_sound_enabled(&m), u32::MAX);
    assert_eq!(*m.accesses.borrow(), [(false, CLICK_SOUND_ENABLED); 2]);
}

#[test]
fn view_and_snapshot_round_trip_preserve_all_bits_and_neighbours_2000_seeds() {
    let mut seed = 2060u32;
    for _ in 0..2000 {
        let mut next = || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        let state = MenuState {
            click_sound_enabled: next(),
            click_sound_request: next(),
            player_slot: next(),
            ui_state_object: next(),
        };
        let mut m = Cells::default();
        m.words.insert(CLICK_SOUND_ENABLED + 4, 0x12345678);
        MenuStateView.set_click_sound_enabled(&mut m, state.click_sound_enabled);
        MenuStateView.set_click_sound_request(&mut m, state.click_sound_request);
        MenuStateView.set_player_slot(&mut m, state.player_slot);
        MenuStateView.set_ui_state_object(&mut m, state.ui_state_object);
        assert_eq!(MenuState::load(&m), state);
        assert_eq!(
            MenuStateView.click_sound_enabled(&m),
            state.click_sound_enabled
        );
        assert_eq!(
            MenuStateView.click_sound_request(&m),
            state.click_sound_request
        );
        assert_eq!(MenuStateView.player_slot(&m), state.player_slot);
        assert_eq!(MenuStateView.ui_state_object(&m), state.ui_state_object);
        let expected = [
            CLICK_SOUND_ENABLED,
            CLICK_SOUND_REQUEST,
            PLAYER_SLOT,
            UI_STATE_OBJECT,
        ];
        assert_eq!(m.accesses.borrow().len(), 12);
        for (i, (write, address)) in m.accesses.borrow().iter().enumerate() {
            assert_eq!(*write, i < 4);
            assert_eq!(*address, expected[i % 4]);
        }
        assert_eq!(m.words[&(CLICK_SOUND_ENABLED + 4)], 0x12345678);
        m.accesses.borrow_mut().clear();
        state.store(&mut m);
        assert_eq!(*m.accesses.borrow(), expected.map(|a| (true, a)));
    }
}
