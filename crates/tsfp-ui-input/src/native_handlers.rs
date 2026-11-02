//! T2316 (playtest blocker 2026-10-13): page handlers that read the activating controller, ported so a native
//! activation names its controller instead of the guest's pad state.
//!
//! The Players page (builder 0x2D2350, the 1/2/3/4 player count) accepts through 0x2D2260. The original finds the
//! controller that pressed A by scanning the connected pads (0x42720 fills the table `[0x6B93C0]` of connected ports
//! and returns their count) from the highest index down for one whose pressed byte (0x427B0) has bit 7 set; that pad
//! becomes player 1 (0x42D00(0, port)). A native activation (keyboard, mouse, or a pad the Rust page captured) leaves no
//! pad pressed in the guest, so the scan ends at index -1, 0x426D0(-1) reads the word before the table and player 1 got
//! a garbage port: the first rumble write (0x42B60) then faulted in the lifted code (owner run
//! `tmp/play/default-20261013-100201`, reproduced on the trace host: native Accept on Players, STOPPED host fault in
//! `tsfp_original_00042b60`). MEASURED from code (static reading of 0x2D2260, 0x42720, 0x427B0, 0x426D0).
//!
//! [`players_accept`] is 0x2D2260 in readable Rust: the same calls in the same order, with the scan kept and, when no
//! pad is pressed (every native activation), the first connected pad (table index 0, the input router's player 1) as
//! the activating controller instead of index -1. Everything it calls stays original (UiBindings boundary).

use crate::native_page::{GuestUi, PageRef};

/// Original functions and globals of the Players accept.
pub mod players {
    /// The Players page's accept handler (`page`), replaced natively by [`super::players_accept`].
    pub const ACCEPT: u32 = 0x2D_2260;
    /// Play a sound with default parameters (`id`), and the id 0x2D2260 plays first.
    pub const PLAY_SOUND: u32 = 0x2_1780;
    pub const SOUND: u32 = 0xD1E;
    /// `[0x78AE00] = -1` (first statement after the sound).
    pub const RESET_WORD: u32 = 0x78_AE00;
    /// Page word `+0x18C`: the cursor row's id (the player count) -> `[0x5236C4]`, then 0x3567F0(count).
    pub const CURSOR_ID: u32 = 0x18C;
    pub const PLAYER_COUNT: u32 = 0x52_36C4;
    pub const SET_PLAYER_COUNT: u32 = 0x35_67F0;
    /// `[0x7613E0] = 1`, `[0x78AD34] = 0`.
    pub const FLAG_ON: u32 = 0x76_13E0;
    pub const FLAG_OFF: u32 = 0x78_AD34;
    /// Connected pad table: 0x42720() fills `[0x6B93C0 + 4 i]` with the ports and returns the count; 0x427B0(i) is
    /// the pressed byte of entry i (bit 7: A); 0x426D0(i) = `[0x6B93C0 + 4 i]`.
    pub const CONNECTED_PADS: u32 = 0x4_2720;
    pub const PAD_PRESSED: u32 = 0x4_27B0;
    pub const PAD_TABLE: u32 = 0x6B_93C0;
    /// 0x42D00(player, port): the controller of a player.
    pub const ASSIGN_PAD: u32 = 0x4_2D00;
    /// Per player i below the count: 0x696D0(i) (nonzero: has a profile); without one the default name
    /// sprintf(0x49F628, text 0x39C, i + 1) (0x151030, text 0x3D7B0) goes to 0x69D90(i, name, 0).
    pub const HAS_PROFILE: u32 = 0x6_96D0;
    pub const TEXT: u32 = 0x3_D7B0;
    pub const DEFAULT_NAME_TEXT: u32 = 0x39C;
    pub const NAME_BUFFER: u32 = 0x49_F628;
    pub const SPRINTF: u32 = 0x15_1030;
    pub const SET_NAME: u32 = 0x6_9D90;
    /// Then 0x69A60(1) and 0x191BA0([0x7613E4]) (the next page through the context of `[0x7613E4]`).
    pub const PROFILES_DONE: u32 = 0x6_9A60;
    pub const NEXT_PAGE: u32 = 0x19_1BA0;
    pub const CONTEXT: u32 = 0x76_13E4;
}

/// The activating controller's index in the connected pad table: the original scan (highest index whose pressed byte
/// has bit 7), else index 0 when at least one pad is connected (a native activation), else None.
pub fn activating_pad(g: &mut dyn GuestUi, connected: u32) -> Option<u32> {
    (0..connected.min(4))
        .rev()
        .find(|&index| {
            g.call(players::PAD_PRESSED, &[index])
                .is_some_and(|byte| byte & 0x80 != 0)
        })
        .or((connected > 0).then_some(0))
}

/// 0x2D2260(page) with the activating controller resolved natively (module doc).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayersSelection {
    pub count: u32,
    pub first_controller: Option<u32>,
}

/// The typed Players handler owns the selection. Profiles and page construction
/// remain explicit services on the other side of the frontier.
pub trait PlayersServices {
    fn begin_selection(&mut self, count: u32) -> u32;
    fn activating_controller(&mut self) -> Option<u32>;
    fn assign_controller(&mut self, player: u32, controller: u32);
    fn has_profile(&mut self, player: u32) -> bool;
    fn default_name(&mut self, player: u32);
    fn finish_selection(&mut self);
}

pub fn select_players(state: &mut PlayersSelection, services: &mut dyn PlayersServices) {
    state.count = services.begin_selection(state.count);
    state.first_controller = services.activating_controller();
    services.assign_controller(0, state.first_controller.unwrap_or(0));
    for player in 0..(state.count as i32).max(0) as u32 {
        if !services.has_profile(player) {
            services.default_name(player);
        }
    }
    services.finish_selection();
}

struct GuestPlayers<'a>(&'a mut dyn GuestUi);
impl PlayersServices for GuestPlayers<'_> {
    fn begin_selection(&mut self, count: u32) -> u32 {
        use players::*;
        self.0.call(PLAY_SOUND, &[SOUND]);
        self.0.write32(RESET_WORD, u32::MAX);
        self.0.write32(PLAYER_COUNT, count);
        self.0.call(SET_PLAYER_COUNT, &[count]);
        self.0.write32(FLAG_ON, 1);
        self.0.write32(FLAG_OFF, 0);
        self.0.read32(PLAYER_COUNT).unwrap_or(0)
    }
    fn activating_controller(&mut self) -> Option<u32> {
        let connected = self.0.call(players::CONNECTED_PADS, &[]).unwrap_or(0);
        activating_pad(self.0, connected)
            .and_then(|index| self.0.read32(players::PAD_TABLE + 4 * index))
            .or_else(|| self.0.read32(players::PAD_TABLE))
    }
    fn assign_controller(&mut self, player: u32, controller: u32) {
        self.0.call(players::ASSIGN_PAD, &[player, controller]);
    }
    fn has_profile(&mut self, player: u32) -> bool {
        self.0.call(players::HAS_PROFILE, &[player]).unwrap_or(1) != 0
    }
    fn default_name(&mut self, player: u32) {
        use players::*;
        let text = self.0.call(TEXT, &[DEFAULT_NAME_TEXT]).unwrap_or(0);
        let name = self
            .0
            .call(SPRINTF, &[NAME_BUFFER, text, player + 1])
            .unwrap_or(NAME_BUFFER);
        self.0.call(SET_NAME, &[player, name, 0]);
    }
    fn finish_selection(&mut self) {
        self.0.call(players::PROFILES_DONE, &[1]);
        let context = self.0.read32(players::CONTEXT).unwrap_or(0);
        self.0.call(players::NEXT_PAGE, &[context]);
    }
}

pub fn players_accept(g: &mut dyn GuestUi, page: PageRef) {
    let mut state = PlayersSelection {
        count: g.read32(page.0 + players::CURSOR_ID).unwrap_or(1),
        first_controller: None,
    };
    select_players(&mut state, &mut GuestPlayers(g));
}

/// Page accept handlers run natively instead of the original call (by the handler address in page `+0x1BC`).
pub fn native_accept(handler: u32) -> Option<fn(&mut dyn GuestUi, PageRef)> {
    match handler {
        players::ACCEPT => Some(players_accept),
        0x2D_0A60 if crate::frontend_handlers::enabled_for(0x2D_0A60) => {
            Some(crate::frontend_handlers::main_accept)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    //! Tier 0, FABRICATED guest (recorded calls); the call order and arguments are MEASURED from 0x2D2260.
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Guest {
        memory: HashMap<u32, u32>,
        calls: Vec<(u32, Vec<u32>)>,
        pressed: Vec<u32>,
        connected: u32,
    }

    impl GuestUi for Guest {
        fn read32(&self, va: u32) -> Option<u32> {
            Some(*self.memory.get(&va).unwrap_or(&0))
        }
        fn write32(&mut self, va: u32, value: u32) -> bool {
            self.memory.insert(va, value);
            true
        }
        fn call(&mut self, va: u32, args: &[u32]) -> Option<u32> {
            self.calls.push((va, args.to_vec()));
            Some(match va {
                players::CONNECTED_PADS => self.connected,
                players::PAD_PRESSED => self.pressed.get(args[0] as usize).copied().unwrap_or(0),
                players::HAS_PROFILE => 0,
                players::TEXT => 0x9000,
                players::SPRINTF => args[0],
                _ => 0,
            })
        }
    }

    fn guest(connected: u32, pressed: Vec<u32>) -> Guest {
        let mut g = Guest {
            connected,
            pressed,
            ..Guest::default()
        };
        g.memory.insert(0x1000 + players::CURSOR_ID, 2);
        for (index, port) in [3u32, 1, 2, 0].iter().enumerate() {
            g.memory
                .insert(players::PAD_TABLE + 4 * index as u32, *port);
        }
        g.memory.insert(players::CONTEXT, 0x7777);
        g
    }

    fn assigned(g: &Guest) -> Vec<u32> {
        g.calls
            .iter()
            .find(|(va, _)| *va == players::ASSIGN_PAD)
            .map(|(_, a)| a.clone())
            .unwrap_or_default()
    }

    #[test]
    fn a_native_accept_gives_player_one_the_first_connected_pad() {
        // The original scan finds no pressed pad: index -1 in the original (the fault), index 0 here.
        let mut g = guest(2, vec![0, 0]);
        players_accept(&mut g, PageRef(0x1000));
        assert_eq!(
            assigned(&g),
            [0, 3],
            "player 0 gets the port of table entry 0"
        );
        assert_eq!(g.memory[&players::PLAYER_COUNT], 2);
        assert_eq!(g.memory[&players::RESET_WORD], u32::MAX);
        let order: Vec<u32> = g.calls.iter().map(|(va, _)| *va).collect();
        assert_eq!(order.first(), Some(&players::PLAY_SOUND));
        assert_eq!(order.last(), Some(&players::NEXT_PAGE));
        assert_eq!(g.calls.last().unwrap().1, [0x7777]);
        // Two players without a profile get the default name for players 1 and 2.
        let names: Vec<Vec<u32>> = g
            .calls
            .iter()
            .filter(|(va, _)| *va == players::SPRINTF)
            .map(|(_, a)| a.clone())
            .collect();
        assert_eq!(
            names,
            [
                vec![players::NAME_BUFFER, 0x9000, 1],
                vec![players::NAME_BUFFER, 0x9000, 2]
            ]
        );
    }

    #[test]
    fn a_pressed_pad_still_wins_as_in_the_original_scan() {
        // Entries 0 and 2 pressed: the scan runs from the highest index down.
        let mut g = guest(3, vec![0x80, 0, 0x80]);
        players_accept(&mut g, PageRef(0x1000));
        assert_eq!(assigned(&g), [0, 2]);
        let mut none = guest(0, vec![]);
        assert_eq!(activating_pad(&mut none, 0), None);
    }

    #[test]
    fn players_and_qualified_main_handlers_are_native() {
        assert!(native_accept(players::ACCEPT).is_some());
        assert_eq!(
            native_accept(0x2D_0A60).is_some(),
            crate::frontend_handlers::enabled_for(0x2D_0A60),
            "main follows the per-handler A/B selector"
        );
        assert!(native_accept(0xDEAD).is_none());
    }
    #[test]
    fn typed_players_selection_supports_explicit_native_controller_and_profiles() {
        #[derive(Default)]
        struct Services {
            calls: Vec<(String, u32)>,
        }
        impl PlayersServices for Services {
            fn begin_selection(&mut self, count: u32) -> u32 {
                self.calls.push(("begin".into(), count));
                count
            }
            fn activating_controller(&mut self) -> Option<u32> {
                Some(2)
            }
            fn assign_controller(&mut self, player: u32, controller: u32) {
                assert_eq!(player, 0);
                self.calls.push(("controller".into(), controller));
            }
            fn has_profile(&mut self, player: u32) -> bool {
                player == 1
            }
            fn default_name(&mut self, player: u32) {
                self.calls.push(("name".into(), player));
            }
            fn finish_selection(&mut self) {
                self.calls.push(("finish".into(), 0));
            }
        }
        let mut state = PlayersSelection {
            count: 4,
            first_controller: None,
        };
        let mut services = Services::default();
        select_players(&mut state, &mut services);
        assert_eq!(state.first_controller, Some(2));
        assert_eq!(
            services.calls,
            [
                ("begin".into(), 4),
                ("controller".into(), 2),
                ("name".into(), 0),
                ("name".into(), 2),
                ("name".into(), 3),
                ("finish".into(), 0)
            ]
        );
    }
}
