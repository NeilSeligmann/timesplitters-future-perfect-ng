// SPDX-License-Identifier: GPL-3.0-or-later
//! Mouse capture state (pure; the Bevy shell applies `wants_grab` to the window cursor options).
//!
//! The pointer is grabbed while a mouse and keyboard slot is active, the window is focused and the user has not released it.
//! Escape never releases the pointer or closes the window (it is the virtual pad START, T2178). The documented release
//! chord is Ctrl+Alt (both held, completing the chord toggles) or F10; a click in the window takes the capture back.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capture {
    released: bool,
    ctrl: bool,
    alt: bool,
}

pub const RELEASE_HELP: &str =
    "Ctrl+Alt or F10 releases or retakes the mouse, a click in the window retakes it";

/// Tracks the user's explicit pointer release across window focus changes.
///
/// A focus change clears a partly held release chord, but does not silently
/// recapture a pointer the user released. The shell decides whether to grab
/// from [`Capture::wants_grab`].
///
/// ```
/// use tsfp_input::capture::Capture;
/// let mut capture = Capture::default();
/// assert!(capture.wants_grab(true, true));
/// assert!(capture.key("F10", true));
/// capture.focus_lost();
/// assert!(!capture.wants_grab(true, true));
/// assert!(capture.click());
/// assert!(capture.wants_grab(true, true));
/// ```
impl Capture {
    pub fn is_released(&self) -> bool {
        self.released
    }
    /// Feed physical key names (`KeyCode` Debug names). Returns true when the capture was toggled.
    pub fn key(&mut self, name: &str, down: bool) -> bool {
        match name {
            "ControlLeft" | "ControlRight" => self.ctrl = down,
            "AltLeft" | "AltRight" => self.alt = down,
            "F10" if down => {
                self.released = !self.released;
                return true;
            }
            _ => return false,
        }
        if down && self.ctrl && self.alt {
            self.released = !self.released;
            return true;
        }
        false
    }
    /// A mouse click while released takes the capture back. Returns true when it did.
    pub fn click(&mut self) -> bool {
        std::mem::replace(&mut self.released, false)
    }
    pub fn focus_lost(&mut self) {
        self.ctrl = false;
        self.alt = false;
    }
    pub fn wants_grab(&self, mouse_keyboard_active: bool, focused: bool) -> bool {
        mouse_keyboard_active && focused && !self.released
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chord_toggles_once_per_completion() {
        let mut capture = Capture::default();
        assert!(capture.wants_grab(true, true));
        assert!(!capture.key("ControlLeft", true));
        assert!(capture.key("AltLeft", true));
        assert!(capture.is_released());
        assert!(!capture.wants_grab(true, true));
        capture.key("AltLeft", false);
        assert!(
            capture.key("AltLeft", true),
            "pressing the chord again retakes"
        );
        assert!(!capture.is_released());
    }

    #[test]
    fn f10_click_and_focus() {
        let mut capture = Capture::default();
        assert!(capture.key("F10", true));
        assert!(!capture.key("F10", false));
        assert!(capture.is_released());
        assert!(capture.click());
        assert!(!capture.click());
        assert!(!capture.wants_grab(true, false));
        assert!(!capture.wants_grab(false, true));
    }

    #[test]
    fn escape_is_not_a_chord() {
        let mut capture = Capture::default();
        assert!(!capture.key("Escape", true));
        assert!(!capture.is_released());
    }

    #[test]
    fn grab_truth_table_and_focus_preserves_release() {
        for released in [false, true] {
            for active in [false, true] {
                for focused in [false, true] {
                    let mut capture = Capture::default();
                    if released {
                        assert!(capture.key("F10", true));
                    }
                    assert_eq!(
                        capture.wants_grab(active, focused),
                        active && focused && !released
                    );
                    capture.focus_lost();
                    assert_eq!(capture.is_released(), released);
                    assert_eq!(
                        capture.wants_grab(active, focused),
                        active && focused && !released
                    );
                }
            }
        }
    }

    #[test]
    fn interrupted_chord_needs_fresh_modifier_press() {
        for first in ["ControlLeft", "ControlRight"] {
            for second in ["AltLeft", "AltRight"] {
                let mut capture = Capture::default();
                assert!(!capture.key(first, true));
                capture.focus_lost();
                assert!(!capture.key(second, true));
                assert!(!capture.is_released());
                assert!(capture.key(first, true));
                assert!(capture.is_released());
                assert!(capture.click());
                assert!(!capture.click());
            }
        }
    }
}
