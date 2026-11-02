// SPDX-License-Identifier: GPL-3.0-or-later
//! T2169 keyboard text entry: host keys edit the active Rust text field.
//! Opt-in extension layer, contract and evidence in docs/t2169-keyboard-text-entry.md.
use tsfp_text_field::{EditError, Event, Outcome, TextField, TextWidth};
use tsfp_ui_input::PlayerId;

/// Which keyboard gestures the layer enables. Absent (the original preset) means the host
/// keyboard never edits text fields, only the pad on-screen keyboard does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyboardText {
    /// The player whose fields the keyboard edits (the keyboard is one device, split-screen
    /// pages owned by other players keep pad-only entry).
    pub player: PlayerId,
    /// Map a character missing from the charset to its other ASCII case when that one exists
    /// (an uppercase-only charset still accepts lowercase typing).
    pub fold_case: bool,
    /// Delete, Home and End keys, which the original pad editor has no events for.
    pub extended_keys: bool,
    /// Accept host auto-repeat events. Off means only the first press of a held key counts.
    pub key_repeat: bool,
}

impl Default for KeyboardText {
    fn default() -> Self {
        Self {
            player: PlayerId(0),
            fold_case: true,
            extended_keys: true,
            key_repeat: true,
        }
    }
}

/// One host keyboard event, already decoded by the host (IME and dead-key composition end
/// as committed `Character` events, so the model sees only final characters).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Character(char),
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    Enter,
    Escape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: Key,
    /// Host auto-repeat of a held key.
    pub repeat: bool,
}

impl KeyEvent {
    pub fn press(key: Key) -> Self {
        Self { key, repeat: false }
    }
}

/// Why a key did nothing. Edit errors come from the field's own rules (charset, limit, width).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyRejected {
    /// The event came from a player that is not the keyboard's player.
    NotKeyboardPlayer,
    /// Repeat event while repeat is off, or an extended key while extended keys are off.
    Disabled,
    Edit(EditError),
}

impl KeyboardText {
    /// Apply one key to `field` for `player`. Pad events go to the same field through
    /// `TextField::apply` and `pad_character`, so mixed pad and keyboard use shares one
    /// cursor, one buffer and one outcome. Keys after Enter or Escape return `Closed`.
    pub fn apply(
        &self,
        field: &mut TextField,
        player: PlayerId,
        event: KeyEvent,
        width: &mut (impl TextWidth + ?Sized),
    ) -> Result<(), KeyRejected> {
        if player != self.player {
            return Err(KeyRejected::NotKeyboardPlayer);
        }
        if event.repeat && !self.key_repeat {
            return Err(KeyRejected::Disabled);
        }
        if field.outcome() != Outcome::Editing {
            return Err(KeyRejected::Edit(EditError::Closed));
        }
        let edit = |field: &mut TextField, event| field.apply(event).map_err(KeyRejected::Edit);
        match event.key {
            Key::Character(character) => {
                let character = self.fold(field, character);
                field
                    .character_with_width(character, width)
                    .map_err(KeyRejected::Edit)
            }
            Key::Backspace => edit(field, Event::Backspace),
            Key::Left => edit(field, Event::Left),
            Key::Right => edit(field, Event::Right),
            Key::Enter => edit(field, Event::Confirm),
            Key::Escape => edit(field, Event::Cancel),
            Key::Delete | Key::Home | Key::End if !self.extended_keys => Err(KeyRejected::Disabled),
            // Delete is Right then Backspace, a no-op at the end of the text.
            Key::Delete => {
                if field.cursor() < field.text().len() {
                    edit(field, Event::Right)?;
                    edit(field, Event::Backspace)?;
                }
                Ok(())
            }
            Key::Home => {
                while field.cursor() > 0 {
                    edit(field, Event::Left)?;
                }
                Ok(())
            }
            Key::End => {
                while field.cursor() < field.text().len() {
                    edit(field, Event::Right)?;
                }
                Ok(())
            }
        }
    }

    fn fold(&self, field: &TextField, character: char) -> char {
        if !self.fold_case || !character.is_ascii_alphabetic() || field.accepts(character) {
            return character;
        }
        let other = if character.is_ascii_lowercase() {
            character.to_ascii_uppercase()
        } else {
            character.to_ascii_lowercase()
        };
        if field.accepts(other) {
            other
        } else {
            character
        }
    }
}
