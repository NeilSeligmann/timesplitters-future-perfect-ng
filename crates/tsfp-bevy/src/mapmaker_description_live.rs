// SPDX-License-Identifier: GPL-3.0-or-later
//! Attach the Rust-owned Map Description session to its measured generic text page.

use std::sync::Mutex;

use tsfp_mapmaker::description::{DescriptionCommand, DescriptionTarget};
use tsfp_mapmaker::ui::EditorSurfaceTarget;
use tsfp_ui_input::input::{IntentSet, MenuIntent};
use tsfp_ui_input::mapmaker_description::{
    DescriptionCaretHitMap, DescriptionCaretHitTarget, DescriptionGridError, DescriptionGridKey,
    DescriptionKeyboardGrid, DescriptionKeyboardHitMap, DescriptionKeyboardHitTarget,
};
use tsfp_ui_input::native_page::{GuestUi, PAGE_SLOTS, PageRef};

const TEXT_ENTRY_BUILDER: u32 = 0x7_E800;
/// T1714's text-entry widget stores 224 including its terminator, so the
/// editable UTF-8 payload is limited to 223 bytes.
const DESCRIPTION_BYTE_LIMIT: usize = 223;

#[derive(Default)]
struct Session {
    active_page: Option<((u32, u32), EditorSurfaceTarget)>,
    keyboard_grid: Option<DescriptionKeyboardGrid>,
    keyboard_hit_map: Option<DescriptionKeyboardHitMap>,
    caret_hit_map: Option<DescriptionCaretHitMap>,
}

static SESSION: Mutex<Session> = Mutex::new(Session {
    active_page: None,
    keyboard_grid: None,
    keyboard_hit_map: None,
    caret_hit_map: None,
});

fn is_native_text_builder(top: Option<(u32, u32)>) -> bool {
    top.is_some_and(|(builder, _)| builder == TEXT_ENTRY_BUILDER)
}

fn active_native_text_surface(
    state: &tsfp_mapmaker::ui::MapMakerUiState,
) -> Option<EditorSurfaceTarget> {
    state.active_map_description_target().or_else(|| {
        state
            .active_briefing_target()
            .filter(|_| state.briefing_text_view().is_some())
    })
}

/// Select the live text widget page independently of the highest occupied page slot.
/// T3497 measured Map Settings in slot 2 with its Briefing text field in slot 1.
fn current_text_page(g: &dyn GuestUi) -> Option<(u32, u32)> {
    let page = tsfp_ui_input::native_text::top_text_page(g, &[])?;
    let slot = (0..PAGE_SLOTS).find(|slot| PageRef::slot(*slot) == page)?;
    Some((page.builder(g), slot))
}

/// Observe the actual text widget even when a non-text parent remains alive in a
/// higher-numbered page slot. Map Description can create its session here because its
/// field contract is measured; Briefing is admitted only after its caller supplies a
/// typed field session.
pub fn observe_guest(g: &dyn GuestUi) {
    observe_top(current_text_page(g));
}

/// Start a Rust-owned description edit only for the generic text page opened
/// from Map Settings' Edit Map Description action. The same text builder is
/// used by Save Map, whose Rust surface does not match this route.
pub fn observe_top(top: Option<(u32, u32)>) {
    if !crate::storage_live::mapmaker_ui_enabled() {
        reset();
        return;
    }

    let page = if is_native_text_builder(top) {
        crate::storage_live::mapmaker_ui()
            .lock()
            .ok()
            .and_then(|state| active_native_text_surface(&state))
            .and_then(|target| top.map(|page| (page, target)))
    } else {
        None
    };

    let start = match SESSION.lock() {
        Ok(mut session) if session.active_page != page => {
            session.active_page = page;
            session.keyboard_grid = None;
            session.keyboard_hit_map = None;
            session.caret_hit_map = None;
            page.map(|(_, target)| target).filter(|target| {
                target.surface() == tsfp_mapmaker::ui::EditorSurface::MapDescription
            })
        }
        _ => None,
    };

    if let Some(target) = start {
        let _ =
            crate::storage_live::native_begin_mapmaker_description(target, DESCRIPTION_BYTE_LIMIT);
    }
}

/// Whether the observed top page currently owns native Map Maker text input.
/// The Rust edit session can outlive the generic text page, so input routing must
/// consult page ownership instead of only the still-active editor surface.
fn owns_text_input_for(session: &Session) -> bool {
    session.active_page.is_some()
}

pub fn owns_text_input() -> bool {
    SESSION
        .lock()
        .is_ok_and(|session| owns_text_input_for(&session))
}

fn reset() {
    if let Ok(mut session) = SESSION.lock() {
        session.active_page = None;
        session.keyboard_grid = None;
        session.keyboard_hit_map = None;
        session.caret_hit_map = None;
    }
}

/// Renderer-facing projection of the current caller-supplied on-screen keyboard.
/// Key IDs and neighbors stay owned by the live page projection.
#[derive(Clone, Debug, PartialEq)]
pub struct DescriptionKeyboardView {
    pub target: DescriptionTarget,
    pub keys: Vec<DescriptionGridKey>,
    pub hit_targets: Vec<DescriptionKeyboardHitTarget>,
    pub focused: u32,
}

fn current_description_targets() -> Option<(EditorSurfaceTarget, DescriptionTarget)> {
    let ui = crate::storage_live::mapmaker_ui().lock().ok()?;
    let surface = ui.active_map_description_target()?;
    let target = ui.map_description_session()?.target();
    Some((surface, target))
}

/// Attach the runtime's measured key IDs and navigation links to this page's
/// Rust-owned description field. Rebuilding the page clears this projection.
pub fn install_keyboard_grid(
    keys: Vec<DescriptionGridKey>,
    focused: u32,
) -> Result<(), DescriptionGridError> {
    install_keyboard_grid_inner(keys, None, focused)
}

/// Install the live key grid and its design-space pointer hit targets. The
/// caller still supplies the measured layout; Rust validates and owns hit test.
pub fn install_keyboard_grid_with_hit_targets(
    keys: Vec<DescriptionGridKey>,
    hit_targets: Vec<DescriptionKeyboardHitTarget>,
    focused: u32,
) -> Result<(), DescriptionGridError> {
    install_keyboard_grid_inner(keys, Some(hit_targets), focused)
}

/// Install caret regions over the current Rust-owned description snapshot.
/// A text edit invalidates the snapshot, so these bounds cannot move a caret
/// using stale glyph geometry.
pub fn install_caret_hit_map(
    hit_targets: Vec<DescriptionCaretHitTarget>,
) -> Result<(), DescriptionGridError> {
    if !crate::storage_live::mapmaker_ui_enabled() {
        return Err(DescriptionGridError::Inactive);
    }
    let Some((surface, target)) = current_description_targets() else {
        return Err(DescriptionGridError::Inactive);
    };
    let Some(view) = crate::storage_live::native_mapmaker_description_view() else {
        return Err(DescriptionGridError::Inactive);
    };
    let hit_map = DescriptionCaretHitMap::new(target, &view.field.text, hit_targets)?;
    let mut session = SESSION.lock().map_err(|_| DescriptionGridError::Inactive)?;
    if !session
        .active_page
        .is_some_and(|(_, active_surface)| active_surface == surface)
    {
        return Err(DescriptionGridError::Inactive);
    }
    session.caret_hit_map = Some(hit_map);
    Ok(())
}

fn install_keyboard_grid_inner(
    keys: Vec<DescriptionGridKey>,
    hit_targets: Option<Vec<DescriptionKeyboardHitTarget>>,
    focused: u32,
) -> Result<(), DescriptionGridError> {
    if !crate::storage_live::mapmaker_ui_enabled() {
        return Err(DescriptionGridError::Inactive);
    }
    let Some((surface, target)) = current_description_targets() else {
        return Err(DescriptionGridError::Inactive);
    };
    let grid = DescriptionKeyboardGrid::new(target, keys, focused)?;
    let hit_map = hit_targets
        .map(|targets| DescriptionKeyboardHitMap::new(grid.keys(), targets))
        .transpose()?;
    let mut session = SESSION.lock().map_err(|_| DescriptionGridError::Inactive)?;
    let owns_page = session
        .active_page
        .is_some_and(|(_, active_surface)| active_surface == surface);
    if !owns_page {
        return Err(DescriptionGridError::Inactive);
    }
    session.keyboard_grid = Some(grid);
    session.keyboard_hit_map = hit_map;
    Ok(())
}

pub fn keyboard_view() -> Option<DescriptionKeyboardView> {
    if !crate::storage_live::mapmaker_ui_enabled() {
        return None;
    }
    let (surface, target) = current_description_targets()?;
    let session = SESSION.lock().ok()?;
    if !session
        .active_page
        .is_some_and(|(_, active_surface)| active_surface == surface)
    {
        return None;
    }
    let grid = session.keyboard_grid.as_ref()?;
    (grid.target() == target).then(|| DescriptionKeyboardView {
        target,
        keys: grid.keys().to_vec(),
        hit_targets: session
            .keyboard_hit_map
            .as_ref()
            .map(|hit_map| hit_map.targets().to_vec())
            .unwrap_or_default(),
        focused: grid.focused(),
    })
}

/// Whether the current Map Description text page has an installed native key
/// grid and should route menu intents through it.
pub fn keyboard_grid_active() -> bool {
    keyboard_view().is_some()
}

/// Whether the current grid also owns live mouse hit testing.
pub fn keyboard_pointer_active() -> bool {
    keyboard_view().is_some_and(|view| !view.hit_targets.is_empty())
}

/// Whether an installed caret map still describes the current text snapshot.
pub fn caret_pointer_active() -> bool {
    let Some((_, target)) = current_description_targets() else {
        return false;
    };
    let Some(view) = crate::storage_live::native_mapmaker_description_view() else {
        return false;
    };
    SESSION.lock().is_ok_and(|session| {
        session.caret_hit_map.as_ref().is_some_and(|hit_map| {
            hit_map.target() == target && hit_map.matches_text(&view.field.text)
        })
    })
}

pub fn pointer_active() -> bool {
    keyboard_pointer_active() || caret_pointer_active()
}

/// Route pointer motion and left-button activation through Rust-owned bounds.
/// An active grid consumes the pointer even when it lands outside every key.
pub fn dispatch_pointer(x: f32, y: f32, activate: bool) -> bool {
    let keyboard_active = keyboard_pointer_active();
    let caret_state = current_description_targets()
        .zip(crate::storage_live::native_mapmaker_description_view())
        .and_then(|((surface, target), view)| {
            SESSION.lock().ok().and_then(|session| {
                session.caret_hit_map.as_ref().and_then(|hit_map| {
                    (hit_map.target() == target && hit_map.matches_text(&view.field.text))
                        .then_some((surface, target, hit_map.clone(), view.field.text.clone()))
                })
            })
        });
    if !keyboard_active && caret_state.is_none() {
        return false;
    }
    let action = {
        let Ok(mut session) = SESSION.lock() else {
            return false;
        };
        let Session {
            active_page,
            keyboard_grid,
            keyboard_hit_map,
            caret_hit_map: _,
        } = &mut *session;
        let Some((_, surface)) = *active_page else {
            return false;
        };
        let key = keyboard_hit_map
            .as_ref()
            .and_then(|hit_map| hit_map.hit_test(x, y));
        if let (Some(key), Some(grid)) = (key, keyboard_grid.as_mut()) {
            let _ = grid.focus(key);
            dispatch_key_pointer(grid, surface, activate)
        } else if key.is_some() {
            None
        } else {
            caret_state.and_then(|(caret_surface, target, hit_map, text)| {
                if caret_surface != surface {
                    return None;
                }
                caret_pointer_command(&hit_map, target, &text, x, y, activate)
                    .map(|command| (surface, target, command))
            })
        }
    };
    if let Some((surface, target, command)) = action {
        let _ = apply_keyboard_command(surface, target, command);
    }
    true
}

fn dispatch_key_pointer(
    grid: &DescriptionKeyboardGrid,
    surface: EditorSurfaceTarget,
    activate: bool,
) -> Option<(EditorSurfaceTarget, DescriptionTarget, DescriptionCommand)> {
    activate.then(|| {
        let (target, command) = grid.activate();
        (surface, target, command)
    })
}

fn caret_pointer_command(
    hit_map: &DescriptionCaretHitMap,
    target: DescriptionTarget,
    text: &str,
    x: f32,
    y: f32,
    activate: bool,
) -> Option<DescriptionCommand> {
    if !activate || hit_map.target() != target || !hit_map.matches_text(text) {
        return None;
    }
    hit_map.hit_test(x, y).map(DescriptionCommand::PlaceCaret)
}

/// Pointer hit testing returns a typed key ID from the published view.
pub fn focus_keyboard_key(key: u32) -> Result<(), DescriptionGridError> {
    if keyboard_view().is_none() {
        return Err(DescriptionGridError::Inactive);
    }
    let mut session = SESSION.lock().map_err(|_| DescriptionGridError::Inactive)?;
    session
        .keyboard_grid
        .as_mut()
        .ok_or(DescriptionGridError::Inactive)?
        .focus(key)
}

pub fn activate_keyboard_key() -> bool {
    let action = {
        let Ok(session) = SESSION.lock() else {
            return false;
        };
        let Some((_, surface)) = session.active_page else {
            return false;
        };
        let Some(grid) = session.keyboard_grid.as_ref() else {
            return false;
        };
        let (target, command) = grid.activate();
        Some((surface, target, command))
    };
    action
        .is_some_and(|(surface, target, command)| apply_keyboard_command(surface, target, command))
}

/// Route one controller edge into the installed grid. Navigation is consumed
/// even at an edge with no neighbor so it cannot leak into the guest page.
pub fn dispatch_keyboard_intents(held: IntentSet, before: IntentSet) -> bool {
    if !has_fresh_grid_intent(held, before) {
        return false;
    }

    let action = {
        let Ok(mut session) = SESSION.lock() else {
            return false;
        };
        let Some((_, surface)) = session.active_page else {
            return false;
        };
        let Some(grid) = session.keyboard_grid.as_mut() else {
            return false;
        };
        let result = grid.input(held, before);
        let command = result.map(|(target, command)| (surface, target, command));
        Some(command)
    };
    let Some(action) = action else {
        return false;
    };
    if let Some((surface, target, command)) = action {
        let _ = apply_keyboard_command(surface, target, command);
    }
    true
}

fn has_fresh_grid_intent(held: IntentSet, before: IntentSet) -> bool {
    [
        MenuIntent::Back,
        MenuIntent::Up,
        MenuIntent::Down,
        MenuIntent::Left,
        MenuIntent::Right,
        MenuIntent::Backspace,
        MenuIntent::Accept,
    ]
    .into_iter()
    .any(|intent| held.contains(intent) && !before.contains(intent))
}

fn apply_keyboard_command(
    surface: EditorSurfaceTarget,
    target: DescriptionTarget,
    command: DescriptionCommand,
) -> bool {
    match crate::storage_live::native_mapmaker_description_command(surface, target, command) {
        Ok(
            crate::storage_live::NativeMapDescriptionOutcome::Committed { .. }
            | crate::storage_live::NativeMapDescriptionOutcome::Cancelled,
        ) => {
            crate::text_entry_live::queue_native_back();
            true
        }
        Ok(_) => true,
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_mapmaker::ui::{EditorSurface, EditorSurfaceGeneration};

    #[derive(Default)]
    struct Memory(std::collections::HashMap<u32, u32>);

    impl GuestUi for Memory {
        fn read32(&self, address: u32) -> Option<u32> {
            self.0.get(&address).copied()
        }

        fn write32(&mut self, _address: u32, _value: u32) -> bool {
            false
        }

        fn call(&mut self, _address: u32, _args: &[u32]) -> Option<u32> {
            None
        }
    }

    #[test]
    fn description_text_page_is_found_below_map_settings_slot() {
        use tsfp_ui_input::menu_state::{PageRef, layout::page as fields};

        let description = PageRef::slot(1);
        let map_settings = PageRef::slot(2);
        let mut guest = Memory::default();
        guest.0.extend([
            (description.0 + fields::VTABLE, 1),
            (
                description.0 + fields::ID,
                tsfp_ui_input::native_text::TEXT_WIDGET_IDS[0],
            ),
            (description.0 + fields::BUILDER, TEXT_ENTRY_BUILDER),
            (
                description.0 + tsfp_ui_input::native_text::field::BUFFER,
                0x1000,
            ),
            (map_settings.0 + fields::VTABLE, 1),
            (map_settings.0 + fields::ID, 0),
            (map_settings.0 + fields::BUILDER, 0x31_33E0),
        ]);

        let page = current_text_page(&guest);
        assert_eq!(page, Some((TEXT_ENTRY_BUILDER, 1)));
        let target = EditorSurfaceTarget::new(
            EditorSurfaceGeneration::new(8),
            EditorSurface::MapDescription,
        );
        assert!(is_native_text_builder(page));
        assert_eq!(target.surface(), EditorSurface::MapDescription);
    }

    #[test]
    fn native_text_route_requires_the_measured_builder_and_typed_surface() {
        let description = EditorSurfaceTarget::new(
            EditorSurfaceGeneration::new(8),
            EditorSurface::MapDescription,
        );
        let briefing =
            EditorSurfaceTarget::new(EditorSurfaceGeneration::new(8), EditorSurface::Briefing);
        let save_map =
            EditorSurfaceTarget::new(EditorSurfaceGeneration::new(8), EditorSurface::SaveMap);

        assert!(is_native_text_builder(Some((TEXT_ENTRY_BUILDER, 1))));
        assert_eq!(description.surface(), EditorSurface::MapDescription);
        assert_eq!(briefing.surface(), EditorSurface::Briefing);
        assert_eq!(save_map.surface(), EditorSurface::SaveMap);
        assert!(!is_native_text_builder(Some((0x32_B6A0, 1))));
        assert!(!is_native_text_builder(None));
        assert_eq!(DESCRIPTION_BYTE_LIMIT, 223);
    }

    #[test]
    fn native_text_observer_requires_a_started_description_or_briefing_session() {
        use tsfp_mapmaker::ui::MapMakerUiCommand;

        for surface_kind in [EditorSurface::MapDescription, EditorSurface::Briefing] {
            let mut state = tsfp_mapmaker::ui::MapMakerUiState::default();
            state.dispatch(MapMakerUiCommand::BeginEditor);
            state
                .install_editor_surface_catalog(EditorSurfaceGeneration::new(8), vec![surface_kind])
                .unwrap();
            let surface = state.editor_surface_target(surface_kind).unwrap();
            state.dispatch(MapMakerUiCommand::OpenEditorSurface(surface));
            let expected_before_session =
                (surface_kind == EditorSurface::MapDescription).then_some(surface);
            assert_eq!(active_native_text_surface(&state), expected_before_session);

            match surface_kind {
                EditorSurface::MapDescription => {
                    state
                        .install_map_description_session(surface, "description", 32)
                        .unwrap();
                }
                EditorSurface::Briefing => {
                    state
                        .install_briefing_text_session(surface, "briefing", 32)
                        .unwrap();
                }
                _ => unreachable!(),
            }
            assert_eq!(active_native_text_surface(&state), Some(surface));
        }
    }

    #[test]
    fn text_input_ownership_tracks_the_observed_top_page() {
        let mut session = Session::default();
        assert!(!owns_text_input_for(&session));

        session.active_page = Some((
            (TEXT_ENTRY_BUILDER, 1),
            EditorSurfaceTarget::new(
                EditorSurfaceGeneration::new(8),
                EditorSurface::MapDescription,
            ),
        ));
        assert!(owns_text_input_for(&session));

        session.active_page = None;
        assert!(!owns_text_input_for(&session));
    }

    #[test]
    fn caller_grid_consumes_only_fresh_controller_edges() {
        let up = IntentSet::default().with(MenuIntent::Up);
        assert!(has_fresh_grid_intent(up, IntentSet::default()));
        assert!(!has_fresh_grid_intent(up, up));
        assert!(!has_fresh_grid_intent(
            IntentSet::default(),
            IntentSet::default()
        ));
    }

    #[test]
    fn pointer_caret_dispatch_requires_activation_and_current_snapshot() {
        use tsfp_mapmaker::description::{DescriptionGeneration, MapDescriptionSession};

        let field = MapDescriptionSession::new(DescriptionGeneration::new(4), "A😀", 223).unwrap();
        let target = field.target();
        let hit_map = DescriptionCaretHitMap::new(
            target,
            "A😀",
            vec![
                DescriptionCaretHitTarget {
                    index: 0,
                    left: 0.0,
                    top: 0.0,
                    right: 10.0,
                    bottom: 10.0,
                },
                DescriptionCaretHitTarget {
                    index: 1,
                    left: 10.0,
                    top: 0.0,
                    right: 20.0,
                    bottom: 10.0,
                },
                DescriptionCaretHitTarget {
                    index: 2,
                    left: 20.0,
                    top: 0.0,
                    right: 30.0,
                    bottom: 10.0,
                },
            ],
        )
        .unwrap();

        assert_eq!(
            caret_pointer_command(&hit_map, target, "A😀", 15.0, 5.0, true),
            Some(DescriptionCommand::PlaceCaret(1))
        );
        assert_eq!(
            caret_pointer_command(&hit_map, target, "A😀!", 15.0, 5.0, true),
            None
        );
        assert_eq!(
            caret_pointer_command(&hit_map, target, "A😀", 15.0, 5.0, false),
            None
        );
        let stale_target = MapDescriptionSession::new(DescriptionGeneration::new(5), "A😀", 223)
            .unwrap()
            .target();
        assert_eq!(
            caret_pointer_command(&hit_map, stale_target, "A😀", 15.0, 5.0, true),
            None
        );
    }
}
