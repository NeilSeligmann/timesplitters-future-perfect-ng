// SPDX-License-Identifier: GPL-3.0-or-later
//! T2181: opt-in host-authored navigation affordance over sealed native pages.
use std::sync::{Arc, Mutex};
use tsfp_ui_input::native_page::NativeCommand;
use tsfp_ui_model::{Align, TextRef, Widget, WidgetId, WidgetKind, menu_rect};
use tsfp_ui_renderer::Vec2;
use tsfp_ui_wgpu::in_frame::LiveContent;

const ID: WidgetId = WidgetId::patch(0x2181);
const RECT: [f32; 4] = [20.0, 432.0, 100.0, 464.0];
static HOVER: Mutex<bool> = Mutex::new(false);

pub fn layer(value: &str) -> bool {
    value
        .split(',')
        .any(|v| matches!(v.trim(), "back-button" | "all"))
}
pub fn enabled() -> bool {
    std::env::var("TSFP_UI_EXTENSIONS").is_ok_and(|v| layer(&v))
}
fn contains(pos: Option<(f32, f32)>) -> bool {
    pos.is_some_and(|(x, y)| x >= RECT[0] && x < RECT[2] && y >= RECT[1] && y < RECT[3])
}
/// Patch identities never become guest row indices. Back runs the native framework's
/// veto/cancel/pop contract; pause selects its measured Resume handler instead.
pub fn command(current: &crate::ui_pages::Current) -> Option<NativeCommand> {
    if current.descriptor.builder == 0x2F_5520 {
        let index = current
            .live
            .rows
            .iter()
            .position(|r| r.activate == 0x2E_BB80)?;
        current
            .live
            .cursor_index(index, &current.cursor_nodes)
            .map(|i| NativeCommand::Select(i))
    } else if matches!(current.descriptor.builder, 0x2F_5920 | 0x2F_5E60) {
        Some(NativeCommand::ResumeGame)
    } else {
        Some(NativeCommand::Back)
    }
}
fn pointer_frame(pos: Option<(f32, f32)>, left_pressed: bool) -> (bool, bool) {
    let over = contains(pos);
    (over, over && left_pressed)
}
pub fn pointer(pos: Option<(f32, f32)>, left: bool) -> bool {
    if !enabled() {
        return false;
    }
    let (over, activate) = pointer_frame(pos, left);
    if let Ok(mut hover) = HOVER.lock() {
        *hover = over;
    }
    activate
}
pub fn compose(
    builder: u32,
    base: Arc<LiveContent>,
    focused: Option<WidgetId>,
) -> (Arc<LiveContent>, Option<WidgetId>) {
    compose_layer(enabled(), builder, base, focused)
}

fn compose_layer(
    on: bool,
    builder: u32,
    base: Arc<LiveContent>,
    focused: Option<WidgetId>,
) -> (Arc<LiveContent>, Option<WidgetId>) {
    if !on {
        return (base, focused);
    }
    let mut out = (*base).clone();
    let Some(page) = out.model.pages.get_mut(&out.page) else {
        return (base, focused);
    };
    let mut button = Widget::new(ID, WidgetKind::Button);
    button.rect = menu_rect(
        Vec2::new(RECT[0], RECT[1]),
        Vec2::new(RECT[2] - RECT[0], RECT[3] - RECT[1]),
    );
    button.text = TextRef::Dynamic;
    button.font_index = 19;
    button.align = Align::Center;
    // Authored extension labels: no proprietary strings or bytes committed.
    let label = if crate::text_entry_live::footer_page().is_some() {
        "Cancel"
    } else {
        match builder {
            0x2F_5520 => "Resume",
            0x2F_5920 | 0x2F_5E60 => "Close",
            0x2E_B9E0 | 0x2E_BE90 | 0x2E_9EC0 | 0x7_A080 | 0x7_A420 => "Cancel",
            0x7_A610 | 0x7_FA00 => "Close",
            _ => "Back",
        }
    };
    out.dynamic.insert(ID, label.as_bytes().to_vec());
    // Authored footer chrome shares its hit bounds; automatic dump-page margins
    // are calibrated for full menus and can run beyond this small edge affordance.
    page.panel_bounds = Some(button.rect);
    page.widgets.push(button);
    let hover = HOVER.lock().is_ok_and(|h| *h);
    (Arc::new(out), if hover { Some(ID) } else { focused })
}
/// Registered native-input pages may retain their original custom drawer. Decorate
/// those with a footer-only model, so the extension cannot duplicate their whole page.
pub fn fallback(
    builder: u32,
    page_id: tsfp_ui_model::PageId,
) -> (Arc<LiveContent>, Option<WidgetId>) {
    let mut model = tsfp_ui_model::UiModel::new();
    model
        .pages
        .insert(page_id, tsfp_ui_model::Page::new(page_id, 1));
    compose(
        builder,
        Arc::new(LiveContent {
            model,
            page: page_id,
            dynamic: Default::default(),
            list: None,
            default_page: None,
        }),
        None,
    )
}

/// Native-field companion to the bounded footer diagnostic (no key or pad emulation).
pub fn diagnostic_text(now: u64) {
    if std::env::var("TSFP_UI_BACK_CHECK").as_deref() != Ok("text") {
        return;
    }
    static TEXT: Mutex<(u64, bool)> = Mutex::new((0, false));
    let Ok(mut route) = TEXT.lock() else {
        return;
    };
    if route.1 {
        return;
    }
    if crate::text_entry_live::footer_page().is_none() {
        route.0 = 0;
        return;
    }
    if route.0 == 0 {
        route.0 = now + 4000;
    }
    if now >= route.0 && pointer(Some((88.0, 448.0)), true) {
        route.1 = true;
        eprintln!("tsfp-bevy: back-button diagnostic: footer click on native text field");
        crate::text_entry_live::pointer_cancel();
    }
}

/// Bounded readback diagnostic: select Controls from Settings, then click this footer
/// through its pointer hit test and native command contract. No controller replay adapter.
pub fn diagnostic(builder: u32, now: u64) -> Vec<NativeCommand> {
    let check = std::env::var("TSFP_UI_BACK_CHECK").unwrap_or_default();
    if check != "1" && check != "ingame" {
        return Vec::new();
    }
    static ROUTE: Mutex<(u32, u64, u8)> = Mutex::new((0, 0, 0));
    let Ok(mut route) = ROUTE.lock() else {
        return Vec::new();
    };
    if route.0 != builder {
        route.0 = builder;
        route.1 = now + 4000;
    }
    if now < route.1 {
        return Vec::new();
    }
    if check == "ingame" {
        if (route.2 == 0 && matches!(builder, 0x2F_5920 | 0x2F_5E60))
            || (route.2 <= 1 && builder == 0x2F_5520)
        {
            let Some(current) = crate::ui_pages::current() else {
                return Vec::new();
            };
            if pointer(Some((88.0, 448.0)), true) {
                route.2 = if builder == 0x2F_5520 { 2 } else { 1 };
                eprintln!("tsfp-bevy: back-button diagnostic: footer click on {builder:#x}");
                return command(&current).into_iter().collect();
            }
        }
        return Vec::new();
    }
    if route.2 == 0 && builder == 0x2C_BFC0 {
        route.2 = 1;
        return vec![NativeCommand::Select(1)];
    }
    if route.2 == 1 && builder == 0x2E_A8E0 {
        let Some(current) = crate::ui_pages::current() else {
            return Vec::new();
        };
        if pointer(Some((88.0, 448.0)), true) {
            route.2 = 2;
            eprintln!("tsfp-bevy: back-button diagnostic: footer click on Controls");
            return command(&current).into_iter().collect();
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_is_arc_identity_and_every_native_page_gets_one_owned_button() {
        use tsfp_ui_model::{Page, PageId, UiModel};
        let page_id = PageId(1);
        let mut model = UiModel::new();
        model.pages.insert(page_id, Page::new(page_id, 1));
        let base = Arc::new(LiveContent {
            model,
            page: page_id,
            dynamic: Default::default(),
            list: None,
            default_page: None,
        });
        let (original, _) = compose_layer(false, 0, base.clone(), None);
        assert!(Arc::ptr_eq(&base, &original));
        for descriptor in crate::ui_pages::PAGES {
            let (patched, _) = compose_layer(true, descriptor.builder, base.clone(), None);
            assert_eq!(patched.model.pages[&page_id].widgets.len(), 1);
            assert_eq!(patched.model.pages[&page_id].widgets[0].id, ID);
            assert!(patched.dynamic.contains_key(&ID));
            assert!(base.model.pages[&page_id].widgets.is_empty());
        }
    }

    #[test]
    fn synthetic_pointer_stream_activates_only_the_pressed_footer_and_not_holds_or_letterbox() {
        let stream = [
            (None, true),
            (Some((-10.0, 448.0)), true),
            (Some((88.0, 448.0)), false),
            (Some((88.0, 448.0)), true),
            (Some((88.0, 448.0)), false),
            (Some((300.0, 240.0)), true),
        ];
        let events: Vec<_> = stream
            .into_iter()
            .map(|(p, pressed)| pointer_frame(p, pressed))
            .collect();
        assert_eq!(
            events,
            vec![
                (false, false),
                (false, false),
                (true, false),
                (true, true),
                (true, false),
                (false, false)
            ]
        );
    }

    #[test]
    fn opt_in_and_safe_area_edges() {
        for v in ["", "original", "mk-settings", "not-back-button"] {
            assert!(!layer(v));
        }
        for v in ["back-button", "all", "original, back-button"] {
            assert!(layer(v));
        }
        assert!(contains(Some((20.0, 432.0))));
        assert!(contains(Some((99.9, 463.9))));
        assert!(!contains(Some((100.0, 440.0))));
        assert!(!contains(Some((30.0, 464.0))));
        assert!(!contains(None));
    }
}
