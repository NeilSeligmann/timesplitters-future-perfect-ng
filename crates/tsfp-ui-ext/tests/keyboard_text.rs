//! T2169 keyboard text entry layer: synthetic key streams on a Rust text field.
use tsfp_text_field::{EditError, Event, Outcome, TextField, TextWidth};
use tsfp_ui_ext::*;
use tsfp_ui_input::PlayerId;
use tsfp_ui_model::UiModel;

/// Fabricated test charset (uppercase, digits, space); real charsets come from disc tables.
const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 ";

struct Any;
impl TextWidth for Any {
    fn fits(&mut self, _: &[u8]) -> bool {
        true
    }
}
struct MaxBytes(usize);
impl TextWidth for MaxBytes {
    fn fits(&mut self, bytes: &[u8]) -> bool {
        bytes.len() <= self.0
    }
}

const P0: PlayerId = PlayerId(0);

fn field(initial: &[u8]) -> TextField {
    TextField::new(initial, 8, CHARSET).unwrap()
}

fn typed(
    keys: KeyboardText,
    field: &mut TextField,
    stream: &[Key],
) -> Vec<Result<(), KeyRejected>> {
    stream
        .iter()
        .map(|key| keys.apply(field, P0, KeyEvent::press(*key), &mut Any))
        .collect()
}

fn chars(text: &str) -> Vec<Key> {
    text.chars().map(Key::Character).collect()
}

#[test]
fn original_preset_has_no_keyboard_text() {
    let base = UiModel::new();
    assert_eq!(
        ExtensionPreset::original()
            .apply(&base)
            .unwrap()
            .keyboard_text,
        None
    );
    let preset = ExtensionPreset {
        name: "kbd".into(),
        layers: vec![keyboard_text(&base, KeyboardText::default())],
    };
    let extended = preset.apply(&base).unwrap();
    assert_eq!(extended.keyboard_text, Some(KeyboardText::default()));
    assert_eq!(extended.model.pages, base.pages);
}

#[test]
fn conflicting_layers_are_rejected() {
    let base = UiModel::new();
    let other = KeyboardText {
        fold_case: false,
        ..KeyboardText::default()
    };
    let preset = ExtensionPreset {
        name: "kbd".into(),
        layers: vec![
            keyboard_text(&base, KeyboardText::default()),
            keyboard_text(&base, other),
        ],
    };
    assert_eq!(
        preset.apply(&base).unwrap_err(),
        ExtensionError::KeyboardTextConflict
    );
}

#[test]
fn typing_folds_case_and_commits() {
    let mut f = field(b"");
    let mut stream = chars("ab1");
    stream.push(Key::Enter);
    let results = typed(KeyboardText::default(), &mut f, &stream);
    assert!(results.iter().all(Result::is_ok), "{results:?}");
    assert_eq!(f.text(), b"AB1");
    assert_eq!(f.outcome(), Outcome::Confirmed);
}

#[test]
fn without_folding_lowercase_is_invalid() {
    let keys = KeyboardText {
        fold_case: false,
        ..KeyboardText::default()
    };
    let mut f = field(b"");
    assert_eq!(
        typed(keys, &mut f, &chars("a")),
        vec![Err(KeyRejected::Edit(EditError::InvalidCharacter))]
    );
    assert!(f.text().is_empty());
}

#[test]
fn charset_limit_and_width_rules_hold() {
    let keys = KeyboardText::default();
    let mut f = field(b"");
    // Non-charset, non-byte (IME output the font cannot draw) characters are refused.
    let results = typed(keys, &mut f, &chars("!\u{e9}\u{4e2d}"));
    assert!(
        results
            .iter()
            .all(|r| *r == Err(KeyRejected::Edit(EditError::InvalidCharacter)))
    );
    let results = typed(keys, &mut f, &chars("ABCDEFGHI"));
    assert_eq!(results[8], Err(KeyRejected::Edit(EditError::TooLong)));
    assert_eq!(f.text(), b"ABCDEFGH");
    let mut narrow = field(b"AB");
    assert_eq!(
        keys.apply(
            &mut narrow,
            P0,
            KeyEvent::press(Key::Character('C')),
            &mut MaxBytes(2)
        ),
        Err(KeyRejected::Edit(EditError::TooWide))
    );
    assert_eq!(narrow.text(), b"AB");
}

#[test]
fn editing_keys_move_and_delete() {
    let keys = KeyboardText::default();
    let mut f = field(b"ABCD");
    typed(keys, &mut f, &[Key::Home, Key::Delete]);
    assert_eq!((f.text(), f.cursor()), (&b"BCD"[..], 0));
    typed(keys, &mut f, &[Key::Right, Key::Backspace]);
    assert_eq!((f.text(), f.cursor()), (&b"CD"[..], 0));
    typed(keys, &mut f, &[Key::End, Key::Delete, Key::Left]);
    assert_eq!((f.text(), f.cursor()), (&b"CD"[..], 1));
    typed(keys, &mut f, &[Key::Delete]);
    assert_eq!((f.text(), f.cursor()), (&b"C"[..], 1));
}

#[test]
fn extended_keys_can_be_disabled() {
    let keys = KeyboardText {
        extended_keys: false,
        ..KeyboardText::default()
    };
    let mut f = field(b"AB");
    for key in [Key::Delete, Key::Home, Key::End] {
        assert_eq!(
            keys.apply(&mut f, P0, KeyEvent::press(key), &mut Any),
            Err(KeyRejected::Disabled)
        );
    }
    assert_eq!((f.text(), f.cursor()), (&b"AB"[..], 2));
}

#[test]
fn escape_restores_and_closes() {
    let keys = KeyboardText::default();
    let mut f = field(b"AB");
    typed(keys, &mut f, &chars("C"));
    typed(keys, &mut f, &[Key::Escape]);
    assert_eq!((f.text(), f.outcome()), (&b"AB"[..], Outcome::Cancelled));
    assert_eq!(
        typed(keys, &mut f, &[Key::Character('D'), Key::End, Key::Delete]),
        vec![Err(KeyRejected::Edit(EditError::Closed)); 3]
    );
}

#[test]
fn blank_name_cannot_be_confirmed() {
    let mut f = field(b"");
    typed(
        KeyboardText::default(),
        &mut f,
        &[Key::Character(' '), Key::Enter],
    );
    assert_eq!(f.outcome(), Outcome::Editing);
}

#[test]
fn repeat_follows_policy() {
    let held = KeyEvent {
        key: Key::Character('A'),
        repeat: true,
    };
    let mut f = field(b"");
    let off = KeyboardText {
        key_repeat: false,
        ..KeyboardText::default()
    };
    assert_eq!(
        off.apply(&mut f, P0, held, &mut Any),
        Err(KeyRejected::Disabled)
    );
    assert_eq!(
        KeyboardText::default().apply(&mut f, P0, held, &mut Any),
        Ok(())
    );
    assert_eq!(f.text(), b"A");
}

#[test]
fn split_screen_keyboard_belongs_to_one_player() {
    let keys = KeyboardText {
        player: PlayerId(1),
        ..KeyboardText::default()
    };
    let mut f = field(b"");
    let press = KeyEvent::press(Key::Character('A'));
    assert_eq!(
        keys.apply(&mut f, P0, press, &mut Any),
        Err(KeyRejected::NotKeyboardPlayer)
    );
    assert_eq!(keys.apply(&mut f, PlayerId(1), press, &mut Any), Ok(()));
}

#[test]
fn pad_and_keyboard_share_one_field() {
    let keys = KeyboardText::default();
    let mut f = field(b"");
    f.pad_character(b'X', &mut Any).unwrap();
    typed(keys, &mut f, &chars("y"));
    f.apply(Event::Left).unwrap();
    typed(keys, &mut f, &chars("z"));
    f.apply(Event::Backspace).unwrap();
    typed(keys, &mut f, &[Key::End]);
    f.pad_character(b'1', &mut Any).unwrap();
    assert_eq!(f.text(), b"XY1");
    f.apply(Event::Confirm).unwrap();
    assert_eq!(
        typed(keys, &mut f, &chars("A")),
        vec![Err(KeyRejected::Edit(EditError::Closed))]
    );
}

#[test]
fn select_all_is_replaced_by_typing() {
    let mut f = field(b"OLD");
    f.select_all();
    typed(KeyboardText::default(), &mut f, &chars("n"));
    assert_eq!(f.text(), b"N");
}
