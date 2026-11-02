//! Held menu intents to native page commands (moved from the Bevy shell's `ui_live`, T3770).
use crate::native_page::NativeCommand;
use crate::widgets::Step;
use crate::{IntentSet, MenuIntent};
use tsfp_input::player::{PlayerInput, bits};

/// First repeat of a held direction after this many milliseconds, then one every period.
pub const REPEAT_DELAY_MS: u64 = 400;
pub const REPEAT_PERIOD_MS: u64 = 120;

/// T3770: the recording has no delete tag; a `Scroll` on this list id is the page delete action
/// (the other reserved ids, `u32::MAX` and `u32::MAX - 1`, are value steps).
pub const RECORDED_DELETE_LIST: u32 = u32::MAX - 2;
/// T3772: a profile page join (START on controller port n, `NativeCommand::Join(n)`) is recorded as a `Scroll` on this
/// list id with delta n, in a UI row whose player is n.
pub const RECORDED_JOIN_LIST: u32 = u32::MAX - 3;
/// T3817: the page's X shortcut (`NativeCommand::Shortcut`) is recorded as a `Scroll` on this list id.
pub const RECORDED_SHORTCUT_LIST: u32 = u32::MAX - 4;

/// Reserved list ids (page actions, not scrolls) replay even with a zero delta.
pub fn recorded_action_list(list: u32) -> bool {
    matches!(
        list,
        RECORDED_DELETE_LIST | RECORDED_JOIN_LIST | RECORDED_SHORTCUT_LIST
    )
}

/// Pad buttons of a replayed or routed player input as the menu intents they hold.
pub fn replay_menu_intents(input: &PlayerInput) -> IntentSet {
    let mut held = IntentSet::default();
    for (button, intent) in [
        (bits::DPAD_UP, MenuIntent::Up),
        (bits::DPAD_DOWN, MenuIntent::Down),
        (bits::DPAD_LEFT, MenuIntent::Left),
        (bits::DPAD_RIGHT, MenuIntent::Right),
        (bits::A, MenuIntent::Accept),
        (bits::B, MenuIntent::Back),
        (bits::START, MenuIntent::Start),
        (bits::Y, MenuIntent::Delete),
        (bits::X, MenuIntent::Shortcut),
    ] {
        if input.held & button != 0 {
            held = held.with(intent);
        }
    }
    if input.menu.accept {
        held = held.with(MenuIntent::Accept);
    }
    if input.menu.back {
        held = held.with(MenuIntent::Back);
    }
    held
}

/// The press edges of a replayed player input as menu intents.
pub fn replay_menu_presses(input: PlayerInput) -> IntentSet {
    let mut pressed = IntentSet::default();
    for (button, intent) in [
        (bits::DPAD_UP, MenuIntent::Up),
        (bits::DPAD_DOWN, MenuIntent::Down),
        (bits::DPAD_LEFT, MenuIntent::Left),
        (bits::DPAD_RIGHT, MenuIntent::Right),
        (bits::A, MenuIntent::Accept),
        (bits::B, MenuIntent::Back),
        (bits::Y, MenuIntent::Delete),
        (bits::X, MenuIntent::Shortcut),
    ] {
        if input.pressed & button != 0 {
            pressed = pressed.with(intent);
        }
    }
    if input.menu.accept {
        pressed = pressed.with(MenuIntent::Accept);
    }
    if input.menu.back {
        pressed = pressed.with(MenuIntent::Back);
    }
    pressed
}

/// Commands of one frame: direction steps with key repeat, Accept and Delete on their press edge,
/// Back only from a replayed press (live Back is handled by the shell).
pub fn native_page_commands(
    held: IntentSet,
    before: IntentSet,
    replay_pressed: Option<IntentSet>,
    now: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<NativeCommand> {
    let mut commands = Vec::new();
    for (i, (intent, step)) in [
        (MenuIntent::Up, Step::Up),
        (MenuIntent::Down, Step::Down),
        (MenuIntent::Left, Step::Left),
        (MenuIntent::Right, Step::Right),
    ]
    .into_iter()
    .enumerate()
    {
        let replay_edge = replay_pressed.is_some_and(|pressed| pressed.contains(intent));
        if replay_edge {
            repeat_at[i] = now + REPEAT_DELAY_MS;
            commands.push(NativeCommand::Step(step));
        } else if held.contains(intent) {
            if !before.contains(intent) {
                repeat_at[i] = now + REPEAT_DELAY_MS;
                commands.push(NativeCommand::Step(step));
            } else if now >= repeat_at[i] {
                repeat_at[i] = now + REPEAT_PERIOD_MS;
                commands.push(NativeCommand::Step(step));
            }
        }
    }
    let edge = |intent| {
        replay_pressed.map_or_else(
            || held.contains(intent) && !before.contains(intent),
            |pressed| pressed.contains(intent),
        )
    };
    if edge(MenuIntent::Accept) {
        commands.push(NativeCommand::Accept);
    }
    if edge(MenuIntent::Delete) {
        commands.push(NativeCommand::Delete);
    }
    if edge(MenuIntent::Shortcut) {
        commands.push(NativeCommand::Shortcut);
    }
    let back = replay_pressed.is_some_and(|pressed| pressed.contains(MenuIntent::Back));
    if back {
        commands.push(NativeCommand::Back);
    }
    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T3817: pad X, keyboard X and a replayed X press reach the page as the X shortcut (an edge, no repeat).
    #[test]
    fn x_is_the_page_shortcut_from_keys_pads_and_replays() {
        let mut repeat_at = [0; 4];
        let x = IntentSet::default().with(MenuIntent::Shortcut);
        let none = IntentSet::default();
        assert_eq!(
            native_page_commands(x, none, None, 5, &mut repeat_at),
            [NativeCommand::Shortcut]
        );
        assert!(native_page_commands(x, x, None, 900, &mut repeat_at).is_empty());
        assert_eq!(
            crate::keyboard::menu_intent(0x78),
            Some(MenuIntent::Shortcut)
        );
        assert_eq!(crate::keyboard::fallback_button(0x78), Some(2));
        let input = pad_input(bits::X);
        assert!(replay_menu_intents(&input).contains(MenuIntent::Shortcut));
        assert!(replay_menu_presses(input).contains(MenuIntent::Shortcut));
        assert!(recorded_action_list(RECORDED_SHORTCUT_LIST));
    }

    #[test]
    fn delete_is_a_press_edge_from_keys_pads_and_replays() {
        let mut repeat_at = [0; 4];
        let delete = IntentSet::default().with(MenuIntent::Delete);
        let none = IntentSet::default();
        assert_eq!(
            native_page_commands(delete, none, None, 5, &mut repeat_at),
            [NativeCommand::Delete]
        );
        assert!(native_page_commands(delete, delete, None, 5, &mut repeat_at).is_empty());
        assert_eq!(
            native_page_commands(none, none, Some(delete), 5, &mut repeat_at),
            [NativeCommand::Delete]
        );
    }

    fn pad_input(button: tsfp_input::player::ButtonBits) -> PlayerInput {
        use tsfp_input::player::{AssistMask, Look, MenuIntent as Menu, SourceKind, Vec2};
        PlayerInput {
            step: 1,
            slot: 0,
            source: SourceKind::Replay,
            held: button,
            pressed: button,
            released: 0,
            move_axis: Vec2::new(0.0, 0.0),
            look: Look::Stick(Vec2::new(0.0, 0.0)),
            triggers: [0.0; 2],
            menu: Menu {
                accept: false,
                back: false,
            },
            assists: AssistMask::NONE,
        }
    }

    #[test]
    fn pad_y_holds_and_presses_delete() {
        let input = pad_input(bits::Y);
        assert!(replay_menu_intents(&input).contains(MenuIntent::Delete));
        assert!(replay_menu_presses(input).contains(MenuIntent::Delete));
        assert!(!replay_menu_intents(&input).contains(MenuIntent::Accept));
    }
}
