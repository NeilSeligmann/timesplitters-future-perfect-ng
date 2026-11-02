//! Owner menu keyboard standard, shared by native pages and the fallback adapter.
//! Text fields consume printable characters before this menu table.
use crate::MenuIntent;

/// SDL key code, menu intent. Space deliberately has no menu action.
pub const MENU_KEYS: &[(u32, MenuIntent)] = &[
    (0x4000_0052, MenuIntent::Up),
    (0x4000_0051, MenuIntent::Down),
    (0x4000_0050, MenuIntent::Left),
    (0x4000_004f, MenuIntent::Right),
    (13, MenuIntent::Accept),
    (0x4000_0058, MenuIntent::Accept),
    (27, MenuIntent::Back),
    (8, MenuIntent::Back),
    // T3770: SDLK_DELETE, the page delete action (the pad's Y on the profile page).
    (0x7f, MenuIntent::Delete),
    // T3817: SDLK_x, the page's X shortcut (the "Player Progress" prompt of the front-end pages).
    (0x78, MenuIntent::Shortcut),
];

pub fn menu_intent(key: u32) -> Option<MenuIntent> {
    MENU_KEYS
        .iter()
        .find(|(code, _)| *code == key)
        .map(|(_, intent)| *intent)
}

/// Pause-root Escape toggles pause; all subpages retain ordinary Back.
pub fn fallback_button_on_page(key: u32, builder: u32) -> Option<u8> {
    if key == 27 && builder == 0x2F_5520 {
        Some(6)
    } else {
        fallback_button(key)
    }
}

/// TEMPORARY-SHIM: unregistered original pages still consume pad events (removal T2308).
/// This adapter never changes gameplay bindings outside a live UI page.
pub fn fallback_button(key: u32) -> Option<u8> {
    match menu_intent(key)? {
        MenuIntent::Accept => Some(0),   // SDL South / Xbox A
        MenuIntent::Back => Some(1),     // SDL East / Xbox B
        MenuIntent::Delete => Some(3),   // SDL North / Xbox Y
        MenuIntent::Shortcut => Some(2), // SDL West / Xbox X
        MenuIntent::Up => Some(11),
        MenuIntent::Down => Some(12),
        MenuIntent::Left => Some(13),
        MenuIntent::Right => Some(14),
        _ => None,
    }
}

/// T3723: mouse clicks on an original page without a native pointer: left is South / Xbox A (accept), right is East /
/// Xbox B (back). The route chosen at press is kept until release, so a page change cannot strand a held button.
pub struct MouseFallback([Option<u8>; 2]);

impl MouseFallback {
    pub const fn new() -> Self {
        Self([None; 2])
    }
    /// One click edge of button id 0 (left) or 1 (right); other ids have no menu action. `original_page` is false on
    /// native pointer pages. Returns the pad button to press (`down`) or release.
    pub fn step(&mut self, id: u8, down: bool, original_page: bool) -> Option<u8> {
        let slot = self.0.get_mut(usize::from(id))?;
        if down {
            *slot = original_page.then_some(id);
            *slot
        } else {
            slot.take()
        }
    }
}

/// T3723: a list-selection Scroll on any list id other than the horizontal and adjust markers steps the list vertically.
pub fn list_scroll_step(list: u32, delta: i32) -> Option<crate::widgets::Step> {
    use crate::widgets::Step;
    (!matches!(list, 1 | u32::MAX | 0xFFFF_FFFE | 0xFFFF_FFFC) && delta != 0)
        .then_some(if delta < 0 { Step::Up } else { Step::Down })
}

/// The pad D-pad button (SDL index 11 up, 12 down) of [`list_scroll_step`], for original pages.
pub fn list_scroll_button(list: u32, delta: i32) -> Option<u8> {
    list_scroll_step(list, delta).map(|step| {
        if step == crate::widgets::Step::Up {
            11
        } else {
            12
        }
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn mouse_clicks_accept_and_back_on_original_pages() {
        let mut mouse = super::MouseFallback::new();
        assert_eq!(mouse.step(0, true, true), Some(0));
        assert_eq!(
            mouse.step(0, false, false),
            Some(0),
            "release follows the press route"
        );
        assert_eq!(mouse.step(1, true, true), Some(1));
        assert_eq!(mouse.step(2, true, true), None);
        assert_eq!(
            mouse.step(0, true, false),
            None,
            "native pointer pages keep their own click"
        );
        assert_eq!(super::list_scroll_button(0x2CA960, -1), Some(11));
        assert_eq!(super::list_scroll_button(7, 2), Some(12));
        assert_eq!(super::list_scroll_button(1, 1), None);
    }
    use super::*;
    #[test]
    fn pause_root_escape_is_start_and_subpage_escape_is_back() {
        assert_eq!(fallback_button_on_page(27, 0x2F_5520), Some(6));
        assert_eq!(fallback_button_on_page(8, 0x2F_5520), Some(1));
        for builder in [0x2E_B9E0, 0x2E_BE90, 0x2E_A8E0, 0x2C_3E20, 0x2D_62F0, 0] {
            assert_eq!(fallback_button_on_page(27, builder), Some(1));
        }
    }
    #[test]
    fn every_page_class_uses_owner_standard() {
        for class in [
            "main",
            "native-list",
            "registry",
            "map-maker",
            "popup",
            "pause",
            "legacy",
        ] {
            assert_eq!(menu_intent(13), Some(MenuIntent::Accept), "{class}");
            assert_eq!(
                menu_intent(0x4000_0058),
                Some(MenuIntent::Accept),
                "{class}"
            );
            assert_eq!(menu_intent(27), Some(MenuIntent::Back), "{class}");
            assert_eq!(menu_intent(32), None, "{class}");
            assert_eq!(fallback_button(13), Some(0), "{class}");
            assert_eq!(fallback_button(27), Some(1), "{class}");
            assert_eq!(fallback_button(32), None, "{class}");
            assert_eq!(
                fallback_button(0x7f),
                Some(3),
                "{class}: Delete is the pad Y"
            );
        }
    }
}
