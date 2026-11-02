//! Native keyboard, controller, and pointer input for the Rust-owned Palette
//! Properties action catalogue. Runtime code supplies labels and hit geometry.

use crate::{
    input::IntentSet,
    owned_catalogue::{self, CatalogueAction},
};
use tsfp_mapmaker::ui::{
    EditorSurface, MapMakerPage, MapMakerUiCommand, MapMakerUiState, MapMakerUiUpdate,
    PalettePropertyTarget,
};

use crate::input::MenuIntent as I;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerEvent {
    Hover(Option<PalettePropertyTarget>),
    Activate(Option<PalettePropertyTarget>),
    Back,
}

fn is_palette_properties(state: &MapMakerUiState) -> bool {
    state.page() == MapMakerPage::Editor
        && state.editor_surface() == EditorSurface::PaletteProperties
}

/// Resolve semantic keyboard/controller input into generation-qualified page
/// commands. Focus order follows the installed runtime action catalogue.
pub fn commands(
    state: &MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiCommand> {
    if !is_palette_properties(state) {
        return Vec::new();
    }

    let actions = state.palette_property_actions();
    let mut cursor = state
        .focused_palette_property()
        .and_then(|focused| actions.iter().position(|action| *action == focused));
    let mut result = Vec::new();
    for action in owned_catalogue::actions(held, before, now_ms, repeat_at, false) {
        match action {
            CatalogueAction::Back => result.push(MapMakerUiCommand::Back),
            CatalogueAction::Step(delta) => {
                let next = match cursor {
                    Some(index) => index.checked_add_signed(delta as isize),
                    None if delta > 0 => Some(0),
                    None => actions.len().checked_sub(1),
                }
                .filter(|&index| index < actions.len());
                if let Some(next) = next
                    && cursor != Some(next)
                    && let Some(target) = state.palette_property_target(actions[next])
                {
                    result.push(MapMakerUiCommand::FocusPaletteProperty(target));
                    cursor = Some(next);
                }
            }
            CatalogueAction::Accept => {
                if let Some(target) = cursor
                    .and_then(|index| actions.get(index))
                    .and_then(|action| state.palette_property_target(*action))
                {
                    result.push(MapMakerUiCommand::ActivatePaletteProperty(target));
                }
            }
            CatalogueAction::Focus(_) => {}
        }
    }
    if result
        .iter()
        .any(|command| matches!(command, MapMakerUiCommand::Back))
    {
        return result;
    }
    result
}

pub fn dispatch_frame(
    state: &mut MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiUpdate> {
    commands(state, held, before, now_ms, repeat_at)
        .into_iter()
        .map(|command| state.dispatch(command))
        .collect()
}

pub fn pointer_commands(state: &MapMakerUiState, event: PointerEvent) -> Vec<MapMakerUiCommand> {
    if !is_palette_properties(state) {
        return Vec::new();
    }
    match event {
        PointerEvent::Back => vec![MapMakerUiCommand::Back],
        PointerEvent::Hover(Some(target)) if state.contains_palette_property_target(target) => {
            vec![MapMakerUiCommand::FocusPaletteProperty(target)]
        }
        PointerEvent::Activate(Some(target)) if state.contains_palette_property_target(target) => {
            vec![MapMakerUiCommand::ActivatePaletteProperty(target)]
        }
        PointerEvent::Hover(None)
        | PointerEvent::Hover(Some(_))
        | PointerEvent::Activate(None)
        | PointerEvent::Activate(Some(_)) => Vec::new(),
    }
}

pub fn dispatch_pointer(state: &mut MapMakerUiState, event: PointerEvent) -> Vec<MapMakerUiUpdate> {
    pointer_commands(state, event)
        .into_iter()
        .map(|command| state.dispatch(command))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_mapmaker::ui::{
        EditorSurfaceGeneration, MapListGeneration, MapListRow, MapListRowId, MapLoadOutcome,
        MapMakerUiCommand as C, PalettePropertiesGeneration, PalettePropertyAction,
    };

    fn palette_state() -> MapMakerUiState {
        let mut state = MapMakerUiState::default();
        state.dispatch(C::OpenStorage);
        state.dispatch(C::SelectLocalStorage);
        let list = MapListGeneration::new(1);
        state
            .install_local_map_rows(
                list,
                vec![MapListRow {
                    id: MapListRowId::new(list, 7),
                }],
            )
            .unwrap();
        let request = match state.dispatch(C::SelectMap(MapListRowId::new(list, 7))) {
            MapMakerUiUpdate::LoadRequested(request) => request,
            update => panic!("unexpected map selection: {update:?}"),
        };
        state.dispatch(C::CompleteMapLoad {
            request: request.id,
            outcome: MapLoadOutcome::Loaded,
        });
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(2),
                vec![EditorSurface::PaletteProperties],
            )
            .unwrap();
        let target = state
            .editor_surface_target(EditorSurface::PaletteProperties)
            .unwrap();
        state.dispatch(C::OpenEditorSurface(target));
        state
            .install_palette_properties_catalog(
                PalettePropertiesGeneration::new(3),
                vec![
                    PalettePropertyAction::LightRow,
                    PalettePropertyAction::ChangePalette,
                ],
            )
            .unwrap();
        state
    }

    #[test]
    fn keyboard_focus_and_accept_open_the_owned_palette_grid() {
        let mut state = palette_state();
        let mut repeat = [0; 4];
        let down = IntentSet::default().with(I::Down);
        dispatch_frame(&mut state, down, IntentSet::default(), 1, &mut repeat);
        assert_eq!(
            state.focused_palette_property(),
            Some(PalettePropertyAction::ChangePalette)
        );

        let accept = IntentSet::default().with(I::Accept);
        let updates = dispatch_frame(&mut state, accept, IntentSet::default(), 2, &mut repeat);
        assert_eq!(updates, vec![MapMakerUiUpdate::PalettePageReentered]);
        assert_eq!(state.editor_surface(), EditorSurface::PaletteGrid);
    }

    #[test]
    fn pointer_activation_is_generation_checked_and_light_row_does_not_commit() {
        let mut state = palette_state();
        let light = state
            .palette_property_target(PalettePropertyAction::LightRow)
            .unwrap();
        assert_eq!(
            dispatch_pointer(&mut state, PointerEvent::Activate(Some(light))),
            vec![MapMakerUiUpdate::PalettePropertyActivated(
                PalettePropertyAction::LightRow
            )]
        );
        assert_eq!(state.editor_surface(), EditorSurface::PaletteProperties);

        state
            .install_palette_properties_catalog(
                PalettePropertiesGeneration::new(4),
                vec![
                    PalettePropertyAction::LightRow,
                    PalettePropertyAction::ChangePalette,
                ],
            )
            .unwrap();
        assert!(pointer_commands(&state, PointerEvent::Activate(Some(light))).is_empty());
        assert_eq!(
            pointer_commands(&state, PointerEvent::Hover(Some(light))),
            Vec::new()
        );
    }
}
