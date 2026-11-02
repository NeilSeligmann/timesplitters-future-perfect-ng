//! Pause menu item list, `original` preset (T2191). Readable port of the decisions in
//! 0x2F5520 `game_ingame_menu_build_pause_item_list_text_ids_0x179d_to_0x17ad_with_session_gates`.
//! The model is pure: the session queries and globals the original reads are passed in as
//! [`PauseInputs`], the result is the ordered item list plus the page shell parameters. Text
//! ids are bank ids (strings stay on the disc), handlers are original code addresses used as
//! stable action keys until the handlers themselves are ported.

/// Levels above this are not missions (front end / Map Maker test levels).
pub const LAST_MISSION_LEVEL: i32 = 100;

/// Everything 0x2F5520 reads. Query results are assumed stable within one build call.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PauseInputs {
    /// `GameState::level` (signed compare in the original).
    pub level: i32,
    /// 0x1CDAE0 `game_thunk_net_effective_player_count`.
    pub player_count: i32,
    /// Local slot of the page (`page + 0x38`).
    pub local_slot: u32,
    /// `GameState::mode`.
    pub mode: u8,
    /// `GameState::flags`.
    pub flags: u32,
    /// 0x3533F0 `game_net_is_session_active`.
    pub session_active: bool,
    /// 0x352880 `game_net_is_host_side`.
    pub host_side: bool,
    /// 0x35E960 `game_net_is_live_mode_active`.
    pub live_mode: bool,
    /// 0x25EE60 `game_record_238_field4_or_null` on the current slot record is non-null.
    pub slot_record_present: bool,
    /// Global 0x7894FC is positive (gates the third item).
    pub third_item_enabled: bool,
    /// Global 0x789640 is positive (enable state applied to the marked items).
    pub items_enabled: bool,
}

/// Mode value whose resume/restart items differ (team / objective mode, INFERRED name).
pub const MODE_ALT: u8 = 0x0a;
/// `flags` bit that hides the session and restart items.
pub const FLAG_HIDE_SESSION_ITEMS: u32 = 0x1000_0000;
/// `flags` bit selecting the alternate restart text.
pub const FLAG_ALT_RESTART_TEXT: u32 = 0x0400_0000;

/// One added text item, in the argument order of 0x79D60.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PauseItem {
    /// Text bank id, `None` for the host-side value item that carries no text.
    pub text: Option<u32>,
    /// Activation handler (original code address).
    pub handler: u32,
    /// Third argument: initial state flag (only the third item sets it).
    pub state: bool,
    /// Fourth argument as f32 bits.
    pub metric_bits: u32,
    /// Sixth argument (value callback for the host-side item).
    pub value: u32,
    /// `Some(enabled)` when the original applies 0x71C70 to this item after adding it.
    pub enabled: Option<bool>,
}

impl PauseItem {
    fn text(text: u32, handler: u32) -> Self {
        Self {
            text: Some(text),
            handler,
            state: false,
            metric_bits: 0,
            value: 0,
            enabled: None,
        }
    }
}

/// What happens to the page after the items are added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseTail {
    /// Non-mission level: flush the current widget and apply its default float.
    FlushCurrent,
    /// Mission: viewport scale adjust and centre; `shift_up` subtracts the global step from
    /// `page + 0x60` (split-screen corner slots).
    Centre { shift_up: bool },
}

/// The built pause page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PauseMenu {
    /// Taller header layout (more than two players with this slot in the lower row).
    pub tall: bool,
    /// Header metric passed to 0x75B70, f32 bits (80.0 tall, 62.0 otherwise).
    pub header_metric_bits: u32,
    /// Widget flags passed to 0x71C50.
    pub widget_flags: u32,
    pub items: Vec<PauseItem>,
    pub tail: PauseTail,
}

const TEXT_RESUME: u32 = 0x179d;
const TEXT_OPTIONS: u32 = 0x17a4;
const TEXT_THIRD: u32 = 0x182f;
const TEXT_THIRD_TALL: u32 = 0x1830;
const TEXT_LIVE: u32 = 0x1af0;
const TEXT_QUIT: u32 = 0x17a5;
const TEXT_QUIT_ALT: u32 = 0x17a8;
const TEXT_ALT_EXTRA: u32 = 0x17a7;
const TEXT_SESSION: u32 = 0x17ad;
const TEXT_RESTART: u32 = 0x17ab;
const TEXT_RESTART_ALT: u32 = 0x17ac;

impl PauseMenu {
    /// The `original` preset: the same items, order and gates as 0x2F5520.
    pub fn original(inp: &PauseInputs) -> Self {
        let mission = inp.level <= LAST_MISSION_LEVEL;
        let lower_row = matches!(inp.local_slot, 1 | 2);
        let tall = mission && inp.player_count > 2 && (inp.player_count == 4 || lower_row);
        let split = mission && inp.player_count > 1;
        let widget_flags = 0x32000 | if split { 0 } else { 0x200 };
        let header_metric_bits = if tall { 80.0f32 } else { 62.0f32 }.to_bits();

        let mut items = vec![
            PauseItem::text(TEXT_RESUME, 0x2e_bb80),
            PauseItem::text(TEXT_OPTIONS, 0x2e_c1a0),
            PauseItem {
                state: inp.third_item_enabled,
                ..PauseItem::text(if tall { TEXT_THIRD_TALL } else { TEXT_THIRD }, 0x2e_c1f0)
            },
        ];
        let marked = Some(inp.items_enabled);

        if inp.session_active && inp.host_side {
            if inp.local_slot == 0 {
                items.push(PauseItem {
                    text: None,
                    handler: 0x2e_c2a0,
                    state: false,
                    metric_bits: 7.0f32.to_bits(),
                    value: 0x1a_0d80,
                    enabled: None,
                });
            }
        } else if inp.live_mode {
            items.push(PauseItem::text(TEXT_LIVE, 0x2e_c2a0));
        }

        if !inp.session_active {
            let alt = inp.mode == MODE_ALT;
            items.push(PauseItem {
                enabled: marked,
                ..PauseItem::text(if alt { TEXT_QUIT_ALT } else { TEXT_QUIT }, 0x2e_bc30)
            });
            if alt && inp.slot_record_present {
                items.push(PauseItem {
                    enabled: marked,
                    ..PauseItem::text(TEXT_ALT_EXTRA, 0x2e_bd50)
                });
            }
        }

        if inp.flags & FLAG_HIDE_SESSION_ITEMS == 0 {
            if inp.session_active && inp.local_slot == 0 {
                items.push(PauseItem::text(TEXT_SESSION, 0x2e_c060));
            }
            if !inp.session_active {
                let text = if inp.flags & FLAG_ALT_RESTART_TEXT == 0 {
                    TEXT_RESTART
                } else {
                    TEXT_RESTART_ALT
                };
                items.push(PauseItem {
                    enabled: marked,
                    ..PauseItem::text(text, 0x2e_c0f0)
                });
            }
        }

        let tail = if mission {
            let shift_up = match inp.player_count {
                3 => inp.local_slot == 2,
                4 => matches!(inp.local_slot, 1 | 3),
                _ => false,
            };
            PauseTail::Centre { shift_up }
        } else {
            PauseTail::FlushCurrent
        };
        Self {
            tall,
            header_metric_bits,
            widget_flags,
            items,
            tail,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_player_mission_has_quit_and_restart() {
        let menu = PauseMenu::original(&PauseInputs {
            level: 3,
            player_count: 1,
            items_enabled: true,
            ..Default::default()
        });
        let texts: Vec<_> = menu.items.iter().map(|i| i.text).collect();
        assert_eq!(
            texts,
            [
                Some(TEXT_RESUME),
                Some(TEXT_OPTIONS),
                Some(TEXT_THIRD),
                Some(TEXT_QUIT),
                Some(TEXT_RESTART)
            ]
        );
        assert_eq!(menu.widget_flags, 0x32200);
        assert!(!menu.tall);
        assert_eq!(menu.tail, PauseTail::Centre { shift_up: false });
    }

    #[test]
    fn host_slot_zero_gets_value_item_and_session_item() {
        let menu = PauseMenu::original(&PauseInputs {
            level: 3,
            player_count: 4,
            local_slot: 0,
            session_active: true,
            host_side: true,
            ..Default::default()
        });
        assert!(menu.tall);
        assert_eq!(menu.items.len(), 5);
        assert_eq!(menu.items[3].text, None);
        assert_eq!(menu.items[4].text, Some(TEXT_SESSION));
    }

    #[test]
    fn non_mission_level_flushes() {
        let menu = PauseMenu::original(&PauseInputs {
            level: 101,
            player_count: 4,
            ..Default::default()
        });
        assert_eq!(menu.tail, PauseTail::FlushCurrent);
        assert_eq!(menu.widget_flags, 0x32200);
    }
}
