//! Pause menu item handlers, `original` preset (T2191). Readable ports of the eight activation
//! handlers the pause list of [`crate::pause`] attaches to its items:
//!
//! | handler | item (text id) | action |
//! |---|---|---|
//! | 0x2EBB80 | 0x179D | [`PauseAction::Resume`] |
//! | 0x2EC1A0 | 0x17A4 | [`PauseAction::Controls`] |
//! | 0x2EC1F0 | 0x182F/0x1830 | [`PauseAction::AudioVideo`] |
//! | 0x2EC2A0 | 0x1AF0 / value item | [`PauseAction::Online`] |
//! | 0x2EBC30 | 0x17A5/0x17A8 | [`PauseAction::Quit`] |
//! | 0x2EBD50 | 0x17A7 | [`PauseAction::AltExtra`] |
//! | 0x2EC060 | 0x17AD | [`PauseAction::LeaveSession`] |
//! | 0x2EC0F0 | 0x17AB/0x17AC | [`PauseAction::Restart`] |
//!
//! The handlers are pure here: the guest globals they read and write are the typed
//! [`PauseActionState`], the queries are [`PauseActionInputs`], and the calls they make into
//! other systems come back as ordered [`PauseEffect`]s for the host to apply. Page
//! descriptors, callbacks and text ids stay original addresses and bank ids (stable keys, no
//! strings or bytes). Names of the confirm callbacks and of the two masks are INFERRED.

use crate::pause::MODE_ALT;

/// Page descriptors the handlers open (original addresses, used as page keys).
pub mod page {
    /// Two-line yes/no confirmation (builder 0x2EB9E0).
    pub const CONFIRM: u32 = 0x52_4a70;
    /// Controls page (builder 0x2EA8E0).
    pub const CONTROLS: u32 = 0x52_4a40;
    /// Audio and video settings (builder 0x2C3E20).
    pub const AUDIO_VIDEO: u32 = 0x52_377c;
    /// Leave-match warning (builder 0x2EBE90).
    pub const LEAVE_MATCH: u32 = 0x52_4a80;
    /// Online options with the players list (builder 0x2F00C0).
    pub const ONLINE_PLAYERS: u32 = 0x52_4b5c;
    /// Online page without a session (builder 0x2F0340).
    pub const ONLINE_IDLE: u32 = 0x52_4b6c;
}

/// Confirm callbacks stored into the yes/no globals of the confirmation page.
pub mod callback {
    /// "No" of every pause confirmation.
    pub const BACK: u32 = 0x2e_bbb0;
    /// "Yes" of the quit confirmation.
    pub const QUIT_YES: u32 = 0x2e_bbc0;
    /// "Yes" of the alternate-mode extra confirmation.
    pub const ALT_EXTRA_YES: u32 = 0x2e_bcd0;
    /// "Yes" of the restart confirmation.
    pub const RESTART_YES: u32 = 0x2e_c090;
}

/// Confirmation texts written into the line globals (bank ids).
pub mod text {
    pub const QUIT: u32 = 0x17a5;
    pub const QUIT_ALT: u32 = 0x17a8;
    pub const ALT_EXTRA: u32 = 0x17a7;
    pub const RESTART: u32 = 0x17ab;
    pub const RESTART_ALT: u32 = 0x17ac;
    /// Second line shared by the alternate-mode confirmations.
    pub const ALT_SECOND_LINE: u32 = 0x17ae;
}

/// `GameState::flags` bit selecting the alternate restart confirmation (same bit as
/// [`crate::pause::FLAG_ALT_RESTART_TEXT`]).
pub const FLAG_ALT_RESTART: u32 = 0x0400_0000;
/// `GameState::flags` byte-0 bit that makes the online item leave the live lobby first.
pub const FLAG_LIVE_LOBBY: u32 = 0x40;

/// Which pause item was activated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PauseAction {
    Resume,
    Controls,
    AudioVideo,
    Online,
    Quit,
    AltExtra,
    LeaveSession,
    Restart,
}

impl PauseAction {
    pub const ALL: [Self; 8] = [
        Self::Resume,
        Self::Controls,
        Self::AudioVideo,
        Self::Online,
        Self::Quit,
        Self::AltExtra,
        Self::LeaveSession,
        Self::Restart,
    ];

    /// Original handler address (the key in [`crate::pause::PauseItem::handler`]).
    pub const fn handler(self) -> u32 {
        match self {
            Self::Resume => 0x2e_bb80,
            Self::Controls => 0x2e_c1a0,
            Self::AudioVideo => 0x2e_c1f0,
            Self::Online => 0x2e_c2a0,
            Self::Quit => 0x2e_bc30,
            Self::AltExtra => 0x2e_bd50,
            Self::LeaveSession => 0x2e_c060,
            Self::Restart => 0x2e_c0f0,
        }
    }

    pub fn from_handler(handler: u32) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.handler() == handler)
    }
}

/// Guest globals the handlers own or share, as typed fields (raw addresses in the docs).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PauseActionState {
    /// 0x789640: per-slot bit set when a confirmation was opened from the pause menu
    /// (also the `items_enabled` gate of the list builder).
    pub confirm_open_mask: u32,
    /// 0x7894FC: per-slot bit set when the audio/video page was opened from the pause menu
    /// (the `third_item_enabled` gate of the list builder).
    pub audio_video_open_mask: u32,
    /// 0x762A18 / 0x762A1C: first and second confirmation line (0 = no second line).
    pub confirm_lines: [u32; 2],
    /// 0x761C5C / 0x761C60: yes and no callbacks of the confirmation page.
    pub confirm_yes: u32,
    pub confirm_no: u32,
    /// 0x762A20: set when the leave-match warning was opened.
    pub leave_match_pending: bool,
    /// 0x78963C: cleared when the controls page opens.
    pub controls_dirty: u32,
}

/// Queries the handlers read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PauseActionInputs {
    /// 0x7BA9E0: slot of the player driving the menu.
    pub current_slot: u32,
    /// `page + 0x38`: local slot of the activated page (Controls and Online read this instead).
    pub page_slot: u32,
    /// `GameState::mode`.
    pub mode: u8,
    /// `GameState::flags`.
    pub flags: u32,
    /// 0x35E960 `game_net_is_live_mode_active`.
    pub live_mode: bool,
    /// 0x3533F0 `game_net_is_session_active`.
    pub session_active: bool,
}

/// Calls into neighbouring systems, in the order the original makes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseEffect {
    /// 0x79B70: open `page` (0 = close back to the game) on the widget stack of `slot`
    /// (`[0x761BBC + slot * 4]`).
    PushPage { slot: u32, page: u32 },
    /// Resume tail: set bit 8 on the slot's pause widget (`[0x761C48 + slot * 4]`) and zero its
    /// float at +0xC.
    MarkPauseWidgetClosed { slot: u32 },
    /// 0x1F dwords from the profile block (`[0x7B0C7C] + 0x1500`) into the slot's controls
    /// scratch (`0x7BA364 + slot * 0x1E0`).
    SnapshotControls { slot: u32 },
    /// 0x2C3300 `game_load_audio_settings_from_profile_into_menu_globals`.
    LoadAudioSettings,
    /// 0x42640 `game_state_6b98a4_enter_4_from_3_or_5_and_flag`.
    LeaveLiveLobby,
}

fn slot_bit(slot: u32) -> u32 {
    1u32 << (slot & 31)
}

impl PauseActionState {
    /// Run the `original` handler of `action`; returns the effects in call order.
    pub fn activate(&mut self, action: PauseAction, inp: &PauseActionInputs) -> Vec<PauseEffect> {
        let slot = inp.current_slot;
        match action {
            PauseAction::Resume => vec![
                PauseEffect::PushPage { slot, page: 0 },
                PauseEffect::MarkPauseWidgetClosed { slot },
            ],
            PauseAction::Controls => {
                self.controls_dirty = 0;
                vec![
                    PauseEffect::SnapshotControls { slot },
                    PauseEffect::PushPage {
                        slot: inp.page_slot,
                        page: page::CONTROLS,
                    },
                ]
            }
            PauseAction::AudioVideo => {
                self.audio_video_open_mask |= slot_bit(slot);
                vec![
                    PauseEffect::LoadAudioSettings,
                    PauseEffect::PushPage {
                        slot,
                        page: page::AUDIO_VIDEO,
                    },
                ]
            }
            PauseAction::Online => {
                let mut effects = Vec::new();
                if inp.live_mode && inp.flags & FLAG_LIVE_LOBBY != 0 {
                    effects.push(PauseEffect::LeaveLiveLobby);
                }
                let page = if inp.session_active {
                    page::ONLINE_PLAYERS
                } else {
                    page::ONLINE_IDLE
                };
                effects.push(PauseEffect::PushPage {
                    slot: inp.page_slot,
                    page,
                });
                effects
            }
            PauseAction::Quit => {
                let lines = if inp.mode == MODE_ALT {
                    [text::QUIT_ALT, text::ALT_SECOND_LINE]
                } else {
                    [text::QUIT, 0]
                };
                self.confirm(slot, lines, callback::QUIT_YES)
            }
            PauseAction::AltExtra => self.confirm(
                slot,
                [text::ALT_EXTRA, text::ALT_SECOND_LINE],
                callback::ALT_EXTRA_YES,
            ),
            PauseAction::LeaveSession => {
                self.leave_match_pending = true;
                vec![PauseEffect::PushPage {
                    slot,
                    page: page::LEAVE_MATCH,
                }]
            }
            PauseAction::Restart => {
                let lines = if inp.flags & FLAG_ALT_RESTART != 0 {
                    [text::RESTART_ALT, 0]
                } else if inp.mode == MODE_ALT {
                    [text::RESTART, text::ALT_SECOND_LINE]
                } else {
                    [text::RESTART, 0]
                };
                self.confirm(slot, lines, callback::RESTART_YES)
            }
        }
    }

    /// Shared tail of the three confirmation handlers.
    fn confirm(&mut self, slot: u32, lines: [u32; 2], yes: u32) -> Vec<PauseEffect> {
        self.confirm_open_mask |= slot_bit(slot);
        self.confirm_lines = lines;
        self.confirm_yes = yes;
        self.confirm_no = callback::BACK;
        vec![PauseEffect::PushPage {
            slot,
            page: page::CONFIRM,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(action: PauseAction, inp: PauseActionInputs) -> (PauseActionState, Vec<PauseEffect>) {
        let mut state = PauseActionState::default();
        let effects = state.activate(action, &inp);
        (state, effects)
    }

    #[test]
    fn handlers_round_trip() {
        for a in PauseAction::ALL {
            assert_eq!(PauseAction::from_handler(a.handler()), Some(a));
        }
        assert_eq!(PauseAction::from_handler(0x2e_bbb0), None);
    }

    #[test]
    fn quit_alt_mode_sets_two_lines() {
        let inp = PauseActionInputs {
            current_slot: 2,
            mode: MODE_ALT,
            ..Default::default()
        };
        let (s, e) = run(PauseAction::Quit, inp);
        assert_eq!(s.confirm_open_mask, 0b100);
        assert_eq!(s.confirm_lines, [text::QUIT_ALT, text::ALT_SECOND_LINE]);
        assert_eq!(
            (s.confirm_yes, s.confirm_no),
            (callback::QUIT_YES, callback::BACK)
        );
        assert_eq!(
            e,
            [PauseEffect::PushPage {
                slot: 2,
                page: page::CONFIRM
            }]
        );
        let (s, _) = run(PauseAction::Quit, PauseActionInputs::default());
        assert_eq!(s.confirm_lines, [text::QUIT, 0]);
    }

    #[test]
    fn restart_flag_wins_over_mode() {
        let inp = PauseActionInputs {
            mode: MODE_ALT,
            flags: FLAG_ALT_RESTART,
            ..Default::default()
        };
        assert_eq!(
            run(PauseAction::Restart, inp).0.confirm_lines,
            [text::RESTART_ALT, 0]
        );
        let inp = PauseActionInputs {
            mode: MODE_ALT,
            ..Default::default()
        };
        assert_eq!(
            run(PauseAction::Restart, inp).0.confirm_lines,
            [text::RESTART, text::ALT_SECOND_LINE]
        );
        assert_eq!(
            run(PauseAction::Restart, Default::default()).0.confirm_yes,
            callback::RESTART_YES
        );
    }

    #[test]
    fn controls_and_online_use_page_slot() {
        let inp = PauseActionInputs {
            current_slot: 1,
            page_slot: 3,
            ..Default::default()
        };
        let (s, e) = run(PauseAction::Controls, inp);
        assert_eq!(s.controls_dirty, 0);
        assert_eq!(
            e,
            [
                PauseEffect::SnapshotControls { slot: 1 },
                PauseEffect::PushPage {
                    slot: 3,
                    page: page::CONTROLS
                }
            ]
        );
        let (_, e) = run(PauseAction::Online, inp);
        assert_eq!(
            e,
            [PauseEffect::PushPage {
                slot: 3,
                page: page::ONLINE_IDLE
            }]
        );
    }

    #[test]
    fn online_leaves_lobby_only_with_live_and_flag() {
        let both = PauseActionInputs {
            live_mode: true,
            flags: FLAG_LIVE_LOBBY,
            session_active: true,
            ..Default::default()
        };
        assert_eq!(
            run(PauseAction::Online, both).1,
            [
                PauseEffect::LeaveLiveLobby,
                PauseEffect::PushPage {
                    slot: 0,
                    page: page::ONLINE_PLAYERS
                }
            ]
        );
        let flag_only = PauseActionInputs {
            flags: FLAG_LIVE_LOBBY,
            ..Default::default()
        };
        assert_eq!(run(PauseAction::Online, flag_only).1.len(), 1);
    }

    #[test]
    fn audio_video_and_leave_session() {
        let inp = PauseActionInputs {
            current_slot: 3,
            ..Default::default()
        };
        let (s, e) = run(PauseAction::AudioVideo, inp);
        assert_eq!(s.audio_video_open_mask, 0b1000);
        assert_eq!(e[0], PauseEffect::LoadAudioSettings);
        let (s, e) = run(PauseAction::LeaveSession, inp);
        assert!(s.leave_match_pending);
        assert_eq!(
            e,
            [PauseEffect::PushPage {
                slot: 3,
                page: page::LEAVE_MATCH
            }]
        );
    }
}
