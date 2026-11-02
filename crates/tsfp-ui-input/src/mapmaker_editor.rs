//! Semantic keyboard/controller navigation for the Rust-owned Map Maker editor.
//!
//! The native page registry supplies the ordered semantic page actions. This
//! adapter tracks focus and emits typed commands; descriptor and widget IDs do
//! not cross into MapMakerUiState.

use crate::{
    input::IntentSet,
    owned_catalogue::{self, CatalogueAction},
};
use tsfp_mapmaker::ui::{
    EditorConfirmationChoice, EditorConfirmationTarget, EditorMenuAction, EditorMenuTarget,
    EditorPreviewChoiceTarget, EditorSurface, EditorSurfaceTarget, MapMakerPage, MapMakerUiCommand,
    MapMakerUiState, MapMakerUiUpdate, PaletteGridChoiceTarget, PalettePropertyTarget,
};

/// Semantic pointer event after the active Rust page registry resolves a hit.
/// Geometry and widget identities remain owned by that registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorPointerEvent {
    Hover(Option<EditorSurfaceTarget>),
    Activate(Option<EditorSurfaceTarget>),
    Back,
}

/// Pointer event after hit testing one action on the loaded editor menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorMenuPointerEvent {
    Hover(Option<EditorMenuTarget>),
    Activate(Option<EditorMenuTarget>),
    Back,
}

/// Pointer input after hit testing one of the owned Clear Map dialog choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorConfirmationPointerEvent {
    Hover(Option<EditorConfirmationTarget>),
    Activate(Option<EditorConfirmationTarget>),
    Back,
}

/// Pointer event after hit testing an action on the Rust-owned palette
/// properties page. Page geometry and labels stay with its renderer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PalettePropertiesPointerEvent {
    Hover(Option<PalettePropertyTarget>),
    Activate(Option<PalettePropertyTarget>),
    Back,
}

/// Pointer input after hit testing one opaque choice on the owned palette grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteGridPointerEvent {
    Hover(Option<PaletteGridChoiceTarget>),
    Activate(Option<PaletteGridChoiceTarget>),
    Back,
}

/// Pointer input after hit testing one choice on the loaded editor Preview page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorPreviewPointerEvent {
    Hover(Option<EditorPreviewChoiceTarget>),
    Activate(Option<EditorPreviewChoiceTarget>),
    Back,
}

fn native_menu_surface(action: EditorMenuAction) -> Option<EditorSurface> {
    match action {
        EditorMenuAction::PreviewMap => Some(EditorSurface::MapPreview),
        EditorMenuAction::SaveMap => Some(EditorSurface::SaveMap),
        EditorMenuAction::MapSettings => Some(EditorSurface::MapSettings),
        EditorMenuAction::EditorSettings => Some(EditorSurface::EditorSettings),
        EditorMenuAction::Controls => Some(EditorSurface::Controls),
        _ => None,
    }
}

fn dispatch_menu_command(
    state: &mut MapMakerUiState,
    command: MapMakerUiCommand,
) -> MapMakerUiUpdate {
    if let MapMakerUiCommand::ActivateEditorMenuAction(target) = command {
        if !state.contains_editor_menu_target(target) {
            return MapMakerUiUpdate::Ignored;
        }
        if target.action() == EditorMenuAction::LoadMap {
            return state.dispatch(MapMakerUiCommand::OpenStorage);
        }
        if let Some(surface) = native_menu_surface(target.action())
            && let Some(surface_target) = state.editor_surface_target(surface)
        {
            return state.dispatch(MapMakerUiCommand::OpenEditorSurface(surface_target));
        }
    }
    state.dispatch(command)
}

/// Resolve keyboard/controller navigation over the Rust-owned editor menu
/// action order. Activation emits a semantic action for the native handler.
pub fn editor_menu_commands(
    state: &MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiCommand> {
    if state.editor_confirmation().is_some() {
        return editor_confirmation_commands(state, held, before, now_ms, repeat_at);
    }
    if state.page() != MapMakerPage::Editor || state.editor_surface() != EditorSurface::MainMenu {
        return Vec::new();
    }
    let actions = state.editor_menu_actions();
    let mut cursor = state
        .focused_editor_menu_action()
        .and_then(|focused| actions.iter().position(|action| *action == focused));
    let mut result = Vec::new();

    for action in owned_catalogue::actions(held, before, now_ms, repeat_at, false) {
        match action {
            CatalogueAction::Back => result.push(MapMakerUiCommand::Back),
            CatalogueAction::Step(delta) if !actions.is_empty() => {
                let next = match cursor {
                    Some(index) => index.checked_add_signed(delta as isize),
                    None if delta > 0 => Some(0),
                    None => Some(actions.len() - 1),
                }
                .filter(|&index| index < actions.len());
                if let Some(action) = next.and_then(|index| actions.get(index)).copied()
                    && cursor != next
                    && let Some(target) = state.editor_menu_target(action)
                {
                    result.push(MapMakerUiCommand::FocusEditorMenuAction(target));
                    cursor = next;
                }
            }
            CatalogueAction::Accept => {
                if let Some(action) = cursor.and_then(|index| actions.get(index)).copied()
                    && let Some(target) = state.editor_menu_target(action)
                {
                    result.push(MapMakerUiCommand::ActivateEditorMenuAction(target));
                }
            }
            CatalogueAction::Step(_) | CatalogueAction::Focus(_) => {}
        }
    }
    result
}

/// Resolve native confirmation focus and decisions. The runtime supplies labels
/// and geometry; this semantic order is Cancel then Accept, with Cancel focused
/// by default so an accidental activation cannot clear the map.
pub fn editor_confirmation_commands(
    state: &MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiCommand> {
    if state.editor_confirmation().is_none() || state.page() != MapMakerPage::Editor {
        return Vec::new();
    }
    let choices = [
        EditorConfirmationChoice::Cancel,
        EditorConfirmationChoice::Accept,
    ];
    let mut cursor = state
        .focused_editor_confirmation_choice()
        .and_then(|choice| choices.iter().position(|&item| item == choice));
    let mut result = Vec::new();

    for action in owned_catalogue::actions(held, before, now_ms, repeat_at, false) {
        match action {
            CatalogueAction::Back => {
                if let Some(target) =
                    state.editor_confirmation_target(EditorConfirmationChoice::Cancel)
                {
                    result.push(MapMakerUiCommand::ResolveEditorConfirmationChoice(target));
                }
            }
            CatalogueAction::Step(delta) => {
                let next = match cursor {
                    Some(index) => index.checked_add_signed(delta as isize),
                    None if delta > 0 => Some(0),
                    None => Some(choices.len() - 1),
                }
                .filter(|&index| index < choices.len());
                if let Some(choice) = next.and_then(|index| choices.get(index)).copied()
                    && cursor != next
                    && let Some(target) = state.editor_confirmation_target(choice)
                {
                    result.push(MapMakerUiCommand::FocusEditorConfirmationChoice(target));
                    cursor = next;
                }
            }
            CatalogueAction::Accept => {
                if let Some(choice) = cursor.and_then(|index| choices.get(index)).copied()
                    && let Some(target) = state.editor_confirmation_target(choice)
                {
                    result.push(MapMakerUiCommand::ResolveEditorConfirmationChoice(target));
                }
            }
            CatalogueAction::Focus(_) => {}
        }
    }
    result
}

pub fn dispatch_editor_confirmation_frame(
    state: &mut MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiUpdate> {
    editor_confirmation_commands(state, held, before, now_ms, repeat_at)
        .into_iter()
        .map(|command| state.dispatch(command))
        .collect()
}

pub fn editor_confirmation_pointer_commands(
    state: &MapMakerUiState,
    event: EditorConfirmationPointerEvent,
) -> Vec<MapMakerUiCommand> {
    if state.editor_confirmation().is_none() {
        return Vec::new();
    }
    let cancel = EditorConfirmationChoice::Cancel;
    match event {
        EditorConfirmationPointerEvent::Back => state
            .editor_confirmation_target(cancel)
            .map(MapMakerUiCommand::ResolveEditorConfirmationChoice)
            .into_iter()
            .collect(),
        EditorConfirmationPointerEvent::Hover(Some(target))
            if state.contains_editor_confirmation_target(target)
                && state.focused_editor_confirmation_choice() != Some(target.choice()) =>
        {
            vec![MapMakerUiCommand::FocusEditorConfirmationChoice(target)]
        }
        EditorConfirmationPointerEvent::Activate(Some(target))
            if state.contains_editor_confirmation_target(target) =>
        {
            let mut commands = Vec::with_capacity(2);
            if state.focused_editor_confirmation_choice() != Some(target.choice()) {
                commands.push(MapMakerUiCommand::FocusEditorConfirmationChoice(target));
            }
            commands.push(MapMakerUiCommand::ResolveEditorConfirmationChoice(target));
            commands
        }
        EditorConfirmationPointerEvent::Hover(_) | EditorConfirmationPointerEvent::Activate(_) => {
            Vec::new()
        }
    }
}

pub fn dispatch_editor_confirmation_pointer(
    state: &mut MapMakerUiState,
    event: EditorConfirmationPointerEvent,
) -> Vec<MapMakerUiUpdate> {
    editor_confirmation_pointer_commands(state, event)
        .into_iter()
        .map(|command| state.dispatch(command))
        .collect()
}

pub fn dispatch_editor_menu_frame(
    state: &mut MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiUpdate> {
    editor_menu_commands(state, held, before, now_ms, repeat_at)
        .into_iter()
        .map(|command| dispatch_menu_command(state, command))
        .collect()
}

/// Resolve a hit-tested menu action through the same target generation as
/// keyboard focus. Stale or out-of-menu targets are ignored.
pub fn editor_menu_pointer_commands(
    state: &MapMakerUiState,
    event: EditorMenuPointerEvent,
) -> Vec<MapMakerUiCommand> {
    if state.page() != MapMakerPage::Editor
        || state.editor_surface() != EditorSurface::MainMenu
        || state.editor_confirmation().is_some()
    {
        return Vec::new();
    }
    match event {
        EditorMenuPointerEvent::Back => vec![MapMakerUiCommand::Back],
        EditorMenuPointerEvent::Hover(Some(target))
            if state.contains_editor_menu_target(target)
                && state.focused_editor_menu_action() != Some(target.action()) =>
        {
            vec![MapMakerUiCommand::FocusEditorMenuAction(target)]
        }
        EditorMenuPointerEvent::Activate(Some(target))
            if state.contains_editor_menu_target(target) =>
        {
            let mut commands = Vec::with_capacity(2);
            if state.focused_editor_menu_action() != Some(target.action()) {
                commands.push(MapMakerUiCommand::FocusEditorMenuAction(target));
            }
            commands.push(MapMakerUiCommand::ActivateEditorMenuAction(target));
            commands
        }
        EditorMenuPointerEvent::Hover(_) | EditorMenuPointerEvent::Activate(_) => Vec::new(),
    }
}

pub fn dispatch_editor_menu_pointer(
    state: &mut MapMakerUiState,
    event: EditorMenuPointerEvent,
) -> Vec<MapMakerUiUpdate> {
    editor_menu_pointer_commands(state, event)
        .into_iter()
        .map(|command| dispatch_menu_command(state, command))
        .collect()
}

/// Resolve one semantic input frame against the runtime-ordered editor actions.
/// The list contains only focusable child surfaces for the current page, in
/// page-table row order. Up/Down share the catalogue repeat timing.
pub fn commands(
    state: &MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiCommand> {
    if state.page() != MapMakerPage::Editor {
        return Vec::new();
    }

    let ordered_surfaces = state.editor_surfaces();
    let mut cursor = state.focused_editor_surface().and_then(|focused| {
        ordered_surfaces
            .iter()
            .position(|surface| *surface == focused)
    });
    let mut result = Vec::new();

    for action in owned_catalogue::actions(held, before, now_ms, repeat_at, false) {
        match action {
            CatalogueAction::Back => result.push(MapMakerUiCommand::Back),
            CatalogueAction::Step(delta) if !ordered_surfaces.is_empty() => {
                let next = match cursor {
                    Some(index) => index.checked_add_signed(delta as isize),
                    None if delta > 0 => Some(0),
                    None => Some(ordered_surfaces.len() - 1),
                }
                .filter(|&index| index < ordered_surfaces.len());
                if let Some(surface) = next.and_then(|index| ordered_surfaces.get(index)).copied()
                    && surface != EditorSurface::MainMenu
                    && cursor != next
                {
                    if let Some(target) = state.editor_surface_target(surface) {
                        result.push(MapMakerUiCommand::FocusEditorSurface(target));
                        cursor = next;
                    }
                }
            }
            CatalogueAction::Accept => {
                if let Some(surface) = cursor
                    .and_then(|index| ordered_surfaces.get(index))
                    .copied()
                    && surface != EditorSurface::MainMenu
                {
                    if let Some(target) = state.editor_surface_target(surface) {
                        result.push(MapMakerUiCommand::OpenEditorSurface(target));
                    }
                }
            }
            CatalogueAction::Step(_) | CatalogueAction::Focus(_) => {}
        }
    }
    result
}

/// Apply one semantic keyboard/controller frame to owned editor page state.
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

/// Resolve one hit-tested pointer event against the runtime-ordered editor
/// actions. Unknown or stale targets are ignored; valid activation uses the
/// same typed focus/open commands as keyboard and controller input.
pub fn pointer_commands(
    state: &MapMakerUiState,
    event: EditorPointerEvent,
) -> Vec<MapMakerUiCommand> {
    if state.page() != MapMakerPage::Editor {
        return Vec::new();
    }

    match event {
        EditorPointerEvent::Back => vec![MapMakerUiCommand::Back],
        EditorPointerEvent::Hover(Some(target))
            if state.contains_editor_surface_target(target)
                && target.surface() != EditorSurface::MainMenu
                && state.focused_editor_surface() != Some(target.surface()) =>
        {
            vec![MapMakerUiCommand::FocusEditorSurface(target)]
        }
        EditorPointerEvent::Activate(Some(target))
            if state.contains_editor_surface_target(target)
                && target.surface() != EditorSurface::MainMenu =>
        {
            let mut commands = Vec::with_capacity(2);
            if state.focused_editor_surface() != Some(target.surface()) {
                commands.push(MapMakerUiCommand::FocusEditorSurface(target));
            }
            commands.push(MapMakerUiCommand::OpenEditorSurface(target));
            commands
        }
        EditorPointerEvent::Hover(_) | EditorPointerEvent::Activate(_) => Vec::new(),
    }
}

/// Apply one hit-tested pointer event to the Rust-owned editor page state.
pub fn dispatch_pointer(
    state: &mut MapMakerUiState,
    event: EditorPointerEvent,
) -> Vec<MapMakerUiUpdate> {
    pointer_commands(state, event)
        .into_iter()
        .map(|command| state.dispatch(command))
        .collect()
}

/// Resolve keyboard/controller input over the installed palette-properties
/// actions. Back is page navigation; it does not claim cancel or rollback.
pub fn palette_properties_commands(
    state: &MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiCommand> {
    if state.page() != MapMakerPage::Editor
        || state.editor_surface() != EditorSurface::PaletteProperties
    {
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
            CatalogueAction::Step(delta) if !actions.is_empty() => {
                let next = match cursor {
                    Some(index) => index.checked_add_signed(delta as isize),
                    None if delta > 0 => Some(0),
                    None => Some(actions.len() - 1),
                }
                .filter(|&index| index < actions.len());
                if let Some(action) = next.and_then(|index| actions.get(index)).copied()
                    && cursor != next
                {
                    if let Some(target) = state.palette_property_target(action) {
                        result.push(MapMakerUiCommand::FocusPaletteProperty(target));
                        cursor = next;
                    }
                }
            }
            CatalogueAction::Accept => {
                if let Some(action) = cursor.and_then(|index| actions.get(index)).copied()
                    && let Some(target) = state.palette_property_target(action)
                {
                    result.push(MapMakerUiCommand::ActivatePaletteProperty(target));
                }
            }
            CatalogueAction::Step(_) | CatalogueAction::Focus(_) => {}
        }
    }
    result
}

pub fn dispatch_palette_properties_frame(
    state: &mut MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiUpdate> {
    palette_properties_commands(state, held, before, now_ms, repeat_at)
        .into_iter()
        .map(|command| state.dispatch(command))
        .collect()
}

pub fn palette_properties_pointer_commands(
    state: &MapMakerUiState,
    event: PalettePropertiesPointerEvent,
) -> Vec<MapMakerUiCommand> {
    if state.page() != MapMakerPage::Editor
        || state.editor_surface() != EditorSurface::PaletteProperties
    {
        return Vec::new();
    }
    match event {
        PalettePropertiesPointerEvent::Back => vec![MapMakerUiCommand::Back],
        PalettePropertiesPointerEvent::Hover(Some(target))
            if state.contains_palette_property_target(target)
                && state.focused_palette_property() != Some(target.action()) =>
        {
            vec![MapMakerUiCommand::FocusPaletteProperty(target)]
        }
        PalettePropertiesPointerEvent::Activate(Some(target))
            if state.contains_palette_property_target(target) =>
        {
            let mut commands = Vec::with_capacity(2);
            if state.focused_palette_property() != Some(target.action()) {
                commands.push(MapMakerUiCommand::FocusPaletteProperty(target));
            }
            commands.push(MapMakerUiCommand::ActivatePaletteProperty(target));
            commands
        }
        PalettePropertiesPointerEvent::Hover(_) | PalettePropertiesPointerEvent::Activate(_) => {
            Vec::new()
        }
    }
}

pub fn dispatch_palette_properties_pointer(
    state: &mut MapMakerUiState,
    event: PalettePropertiesPointerEvent,
) -> Vec<MapMakerUiUpdate> {
    palette_properties_pointer_commands(state, event)
        .into_iter()
        .map(|command| state.dispatch(command))
        .collect()
}

/// Resolve keyboard/controller input over the runtime-provided opaque choices.
/// Accept emits a typed selection request; it does not infer document effects.
pub fn palette_grid_commands(
    state: &MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiCommand> {
    if state.page() != MapMakerPage::Editor || state.editor_surface() != EditorSurface::PaletteGrid
    {
        return Vec::new();
    }
    let choices = state.palette_grid_choices();
    let mut cursor = state
        .focused_palette_grid_choice()
        .and_then(|focused| choices.iter().position(|choice| *choice == focused));
    let mut result = Vec::new();
    for action in owned_catalogue::actions(held, before, now_ms, repeat_at, false) {
        match action {
            CatalogueAction::Back => result.push(MapMakerUiCommand::Back),
            CatalogueAction::Step(delta) if !choices.is_empty() => {
                let next = match cursor {
                    Some(index) => index.checked_add_signed(delta as isize),
                    None if delta > 0 => Some(0),
                    None => Some(choices.len() - 1),
                }
                .filter(|&index| index < choices.len());
                if let Some(choice) = next.and_then(|index| choices.get(index)).copied()
                    && cursor != next
                    && let Some(target) = state.palette_grid_choice_target(choice)
                {
                    result.push(MapMakerUiCommand::FocusPaletteGridChoice(target));
                    cursor = next;
                }
            }
            CatalogueAction::Accept => {
                if let Some(choice) = cursor.and_then(|index| choices.get(index)).copied()
                    && let Some(target) = state.palette_grid_choice_target(choice)
                {
                    result.push(MapMakerUiCommand::ActivatePaletteGridChoice(target));
                }
            }
            CatalogueAction::Step(_) | CatalogueAction::Focus(_) => {}
        }
    }
    result
}

pub fn dispatch_palette_grid_frame(
    state: &mut MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiUpdate> {
    palette_grid_commands(state, held, before, now_ms, repeat_at)
        .into_iter()
        .map(|command| state.dispatch(command))
        .collect()
}

pub fn palette_grid_pointer_commands(
    state: &MapMakerUiState,
    event: PaletteGridPointerEvent,
) -> Vec<MapMakerUiCommand> {
    if state.page() != MapMakerPage::Editor || state.editor_surface() != EditorSurface::PaletteGrid
    {
        return Vec::new();
    }
    match event {
        PaletteGridPointerEvent::Back => vec![MapMakerUiCommand::Back],
        PaletteGridPointerEvent::Hover(Some(target))
            if state.contains_palette_grid_choice_target(target)
                && state.focused_palette_grid_choice() != Some(target.choice()) =>
        {
            vec![MapMakerUiCommand::FocusPaletteGridChoice(target)]
        }
        PaletteGridPointerEvent::Activate(Some(target))
            if state.contains_palette_grid_choice_target(target) =>
        {
            let mut commands = Vec::with_capacity(2);
            if state.focused_palette_grid_choice() != Some(target.choice()) {
                commands.push(MapMakerUiCommand::FocusPaletteGridChoice(target));
            }
            commands.push(MapMakerUiCommand::ActivatePaletteGridChoice(target));
            commands
        }
        PaletteGridPointerEvent::Hover(_) | PaletteGridPointerEvent::Activate(_) => Vec::new(),
    }
}

pub fn dispatch_palette_grid_pointer(
    state: &mut MapMakerUiState,
    event: PaletteGridPointerEvent,
) -> Vec<MapMakerUiUpdate> {
    palette_grid_pointer_commands(state, event)
        .into_iter()
        .map(|command| state.dispatch(command))
        .collect()
}

/// Resolve keyboard/controller navigation over the runtime-supplied editor
/// Preview choices. Activation stays a typed request for the owning handler.
pub fn editor_preview_commands(
    state: &MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiCommand> {
    if state.page() != MapMakerPage::Editor || state.editor_surface() != EditorSurface::MapPreview {
        return Vec::new();
    }
    let choices = state.editor_preview_choices();
    let mut cursor = state
        .focused_editor_preview_choice()
        .and_then(|focused| choices.iter().position(|choice| *choice == focused));
    let mut result = Vec::new();
    for action in owned_catalogue::actions(held, before, now_ms, repeat_at, false) {
        match action {
            CatalogueAction::Back => result.push(MapMakerUiCommand::Back),
            CatalogueAction::Step(delta) if !choices.is_empty() => {
                let next = match cursor {
                    Some(index) => index.checked_add_signed(delta as isize),
                    None if delta > 0 => Some(0),
                    None => Some(choices.len() - 1),
                }
                .filter(|&index| index < choices.len());
                if let Some(choice) = next.and_then(|index| choices.get(index)).copied()
                    && cursor != next
                    && let Some(target) = state.editor_preview_choice_target(choice)
                {
                    result.push(MapMakerUiCommand::FocusEditorPreviewChoice(target));
                    cursor = next;
                }
            }
            CatalogueAction::Accept => {
                if let Some(choice) = cursor.and_then(|index| choices.get(index)).copied()
                    && let Some(target) = state.editor_preview_choice_target(choice)
                {
                    result.push(MapMakerUiCommand::ActivateEditorPreviewChoice(target));
                }
            }
            CatalogueAction::Step(_) | CatalogueAction::Focus(_) => {}
        }
    }
    result
}

pub fn dispatch_editor_preview_frame(
    state: &mut MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiUpdate> {
    editor_preview_commands(state, held, before, now_ms, repeat_at)
        .into_iter()
        .map(|command| state.dispatch(command))
        .collect()
}

pub fn editor_preview_pointer_commands(
    state: &MapMakerUiState,
    event: EditorPreviewPointerEvent,
) -> Vec<MapMakerUiCommand> {
    if state.page() != MapMakerPage::Editor || state.editor_surface() != EditorSurface::MapPreview {
        return Vec::new();
    }
    match event {
        EditorPreviewPointerEvent::Back => vec![MapMakerUiCommand::Back],
        EditorPreviewPointerEvent::Hover(Some(target))
            if state.contains_editor_preview_choice_target(target)
                && state.focused_editor_preview_choice() != Some(target.choice()) =>
        {
            vec![MapMakerUiCommand::FocusEditorPreviewChoice(target)]
        }
        EditorPreviewPointerEvent::Activate(Some(target))
            if state.contains_editor_preview_choice_target(target) =>
        {
            let mut commands = Vec::with_capacity(2);
            if state.focused_editor_preview_choice() != Some(target.choice()) {
                commands.push(MapMakerUiCommand::FocusEditorPreviewChoice(target));
            }
            commands.push(MapMakerUiCommand::ActivateEditorPreviewChoice(target));
            commands
        }
        EditorPreviewPointerEvent::Hover(_) | EditorPreviewPointerEvent::Activate(_) => Vec::new(),
    }
}

pub fn dispatch_editor_preview_pointer(
    state: &mut MapMakerUiState,
    event: EditorPreviewPointerEvent,
) -> Vec<MapMakerUiUpdate> {
    editor_preview_pointer_commands(state, event)
        .into_iter()
        .map(|command| state.dispatch(command))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        EditorConfirmationPointerEvent, EditorMenuPointerEvent, EditorPointerEvent,
        EditorPreviewPointerEvent, PaletteGridPointerEvent, PalettePropertiesPointerEvent,
        dispatch_editor_confirmation_frame, dispatch_editor_confirmation_pointer,
        dispatch_editor_menu_frame, dispatch_editor_menu_pointer, dispatch_editor_preview_frame,
        dispatch_editor_preview_pointer, dispatch_frame, dispatch_palette_grid_frame,
        dispatch_palette_grid_pointer, dispatch_palette_properties_frame,
        dispatch_palette_properties_pointer, dispatch_pointer,
    };
    use crate::input::{IntentSet, MenuIntent};
    use tsfp_mapmaker::modes::{PreviewRow, PreviewSelection};
    use tsfp_mapmaker::ui::{
        EditorConfirmation, EditorConfirmationChoice, EditorHistoryOperation, EditorMenuAction,
        EditorMenuGeneration, EditorPreviewChoiceId, EditorPreviewGeneration, EditorSurface,
        EditorSurfaceGeneration, EditorSurfaceTarget, EditorViewportOperation, MapListGeneration,
        MapListRow, MapListRowId, MapLoadOutcome, MapMakerPage, MapMakerUiCommand, MapMakerUiState,
        MapMakerUiUpdate, PaletteGridCatalogError, PaletteGridChoiceId, PaletteGridGeneration,
        PalettePropertiesGeneration, PalettePropertyAction,
    };

    fn loaded_editor() -> MapMakerUiState {
        let mut state = MapMakerUiState::default();
        let id = MapListRowId::new(MapListGeneration::new(3), 8);
        state
            .install_local_map_rows(MapListGeneration::new(3), vec![MapListRow { id }])
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenStorage);
        state.dispatch(MapMakerUiCommand::SelectLocalStorage);
        let MapMakerUiUpdate::LoadRequested(request) =
            state.dispatch(MapMakerUiCommand::SelectMap(id))
        else {
            panic!("row selection should request a map load");
        };
        state.dispatch(MapMakerUiCommand::CompleteMapLoad {
            request: request.id,
            outcome: MapLoadOutcome::Loaded,
        });
        state
            .install_original_editor_menu(EditorMenuGeneration::new(4))
            .unwrap();
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(4),
                vec![
                    EditorSurface::MapPreview,
                    EditorSurface::MapSettings,
                    EditorSurface::EditorSettings,
                    EditorSurface::Controls,
                    EditorSurface::PaletteProperties,
                ],
            )
            .unwrap();
        state
    }

    fn palette_properties_page() -> MapMakerUiState {
        let mut state = loaded_editor();
        let target = state
            .editor_surface_target(EditorSurface::PaletteProperties)
            .unwrap();
        assert_eq!(
            dispatch_pointer(&mut state, EditorPointerEvent::Activate(Some(target))),
            [
                MapMakerUiUpdate::EditorSurfaceFocused(EditorSurface::PaletteProperties),
                MapMakerUiUpdate::EditorSurfaceChanged(EditorSurface::PaletteProperties),
            ]
        );
        state
            .install_palette_properties_catalog(
                PalettePropertiesGeneration::new(8),
                vec![
                    PalettePropertyAction::LightRow,
                    PalettePropertyAction::ChangePalette,
                ],
            )
            .unwrap();
        state
    }

    #[test]
    fn original_editor_menu_keyboard_and_pointer_use_owned_action_state() {
        let mut state = loaded_editor();
        let mut repeat_at = [0; 4];
        let down = IntentSet::default().with(MenuIntent::Down);
        assert_eq!(
            dispatch_editor_menu_frame(&mut state, down, IntentSet::default(), 10, &mut repeat_at),
            [MapMakerUiUpdate::EditorMenuActionFocused(
                EditorMenuAction::PreviewMap
            )]
        );
        let accept = IntentSet::default().with(MenuIntent::Accept);
        assert_eq!(
            dispatch_editor_menu_frame(&mut state, accept, down, 20, &mut repeat_at),
            [MapMakerUiUpdate::EditorSurfaceChanged(
                EditorSurface::MapPreview
            )]
        );
        assert_eq!(state.editor_surface(), EditorSurface::MapPreview);
        assert_eq!(
            state.dispatch(MapMakerUiCommand::Back),
            MapMakerUiUpdate::EditorSurfaceChanged(EditorSurface::MainMenu)
        );

        // Child-page entry invalidates the caller page's targets. The runtime
        // installs a fresh generation when the menu is shown again.
        state
            .install_original_editor_menu(EditorMenuGeneration::new(5))
            .unwrap();
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(5),
                vec![
                    EditorSurface::MapPreview,
                    EditorSurface::MapSettings,
                    EditorSurface::EditorSettings,
                    EditorSurface::Controls,
                ],
            )
            .unwrap();

        let undo = state.editor_menu_target(EditorMenuAction::Undo).unwrap();
        assert_eq!(
            dispatch_editor_menu_pointer(&mut state, EditorMenuPointerEvent::Hover(Some(undo))),
            [MapMakerUiUpdate::EditorMenuActionFocused(
                EditorMenuAction::Undo
            )]
        );
        assert_eq!(
            dispatch_editor_menu_pointer(&mut state, EditorMenuPointerEvent::Activate(Some(undo)),),
            [MapMakerUiUpdate::EditorHistoryRequested(
                EditorHistoryOperation::Undo
            )]
        );

        let redo = state.editor_menu_target(EditorMenuAction::Redo).unwrap();
        assert_eq!(
            dispatch_editor_menu_pointer(&mut state, EditorMenuPointerEvent::Activate(Some(redo)),),
            [
                MapMakerUiUpdate::EditorMenuActionFocused(EditorMenuAction::Redo),
                MapMakerUiUpdate::EditorHistoryRequested(EditorHistoryOperation::Redo),
            ]
        );
    }

    #[test]
    fn keyboard_editor_menu_accept_emits_owned_history_request() {
        let mut state = loaded_editor();
        let mut repeat_at = [0; 4];
        let down = IntentSet::default().with(MenuIntent::Down);
        let idle = IntentSet::default();
        assert_eq!(
            dispatch_editor_menu_frame(&mut state, down, idle, 10, &mut repeat_at),
            [MapMakerUiUpdate::EditorMenuActionFocused(
                EditorMenuAction::PreviewMap
            )]
        );
        assert!(dispatch_editor_menu_frame(&mut state, idle, down, 20, &mut repeat_at).is_empty());
        assert_eq!(
            dispatch_editor_menu_frame(&mut state, down, idle, 30, &mut repeat_at),
            [MapMakerUiUpdate::EditorMenuActionFocused(
                EditorMenuAction::Undo
            )]
        );
        let accept = IntentSet::default().with(MenuIntent::Accept);
        assert_eq!(
            dispatch_editor_menu_frame(&mut state, accept, idle, 40, &mut repeat_at),
            [MapMakerUiUpdate::EditorHistoryRequested(
                EditorHistoryOperation::Undo
            )]
        );
    }

    #[test]
    fn preview_map_mouse_activation_enters_owned_editor_preview_surface() {
        let mut state = loaded_editor();
        let preview = state
            .editor_menu_target(EditorMenuAction::PreviewMap)
            .unwrap();
        assert_eq!(
            dispatch_editor_menu_pointer(
                &mut state,
                EditorMenuPointerEvent::Activate(Some(preview)),
            ),
            [
                MapMakerUiUpdate::EditorMenuActionFocused(EditorMenuAction::PreviewMap),
                MapMakerUiUpdate::EditorSurfaceChanged(EditorSurface::MapPreview),
            ]
        );
        assert_eq!(state.editor_surface(), EditorSurface::MapPreview);
        assert_eq!(
            state.dispatch(MapMakerUiCommand::Back),
            MapMakerUiUpdate::EditorSurfaceChanged(EditorSurface::MainMenu)
        );
    }

    #[test]
    fn save_map_keyboard_and_pointer_enter_owned_save_route() {
        let mut state = loaded_editor();
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(5),
                vec![EditorSurface::MapPreview, EditorSurface::SaveMap],
            )
            .unwrap();
        let save = state.editor_menu_target(EditorMenuAction::SaveMap).unwrap();
        state.dispatch(MapMakerUiCommand::FocusEditorMenuAction(save));
        let accept = IntentSet::default().with(MenuIntent::Accept);
        let mut repeat_at = [0; 4];
        assert_eq!(
            dispatch_editor_menu_frame(
                &mut state,
                accept,
                IntentSet::default(),
                10,
                &mut repeat_at
            ),
            [MapMakerUiUpdate::EditorSaveMapRouteOpened]
        );
        assert_eq!(state.editor_surface(), EditorSurface::SaveMap);
        assert_eq!(
            state.dispatch(MapMakerUiCommand::Back),
            MapMakerUiUpdate::EditorSurfaceChanged(EditorSurface::MainMenu)
        );

        state
            .install_original_editor_menu(EditorMenuGeneration::new(5))
            .unwrap();
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(6),
                vec![EditorSurface::SaveMap],
            )
            .unwrap();
        let save = state.editor_menu_target(EditorMenuAction::SaveMap).unwrap();
        assert_eq!(
            dispatch_editor_menu_pointer(&mut state, EditorMenuPointerEvent::Activate(Some(save)),),
            [
                MapMakerUiUpdate::EditorMenuActionFocused(EditorMenuAction::SaveMap),
                MapMakerUiUpdate::EditorSaveMapRouteOpened,
            ]
        );
        assert_eq!(state.editor_surface(), EditorSurface::SaveMap);
    }

    #[test]
    fn settings_and_controls_menu_rows_open_registered_owned_surfaces() {
        let mut state = loaded_editor();
        let mut repeat_at = [0; 4];
        let editor_settings = state
            .editor_menu_target(EditorMenuAction::EditorSettings)
            .unwrap();
        state.dispatch(MapMakerUiCommand::FocusEditorMenuAction(editor_settings));
        let accept = IntentSet::default().with(MenuIntent::Accept);
        assert_eq!(
            dispatch_editor_menu_frame(
                &mut state,
                accept,
                IntentSet::default(),
                10,
                &mut repeat_at,
            ),
            [MapMakerUiUpdate::EditorSurfaceChanged(
                EditorSurface::EditorSettings
            )]
        );
        assert_eq!(state.editor_surface(), EditorSurface::EditorSettings);

        state.dispatch(MapMakerUiCommand::Back);
        state
            .install_original_editor_menu(EditorMenuGeneration::new(5))
            .unwrap();
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(5),
                vec![
                    EditorSurface::MapPreview,
                    EditorSurface::MapSettings,
                    EditorSurface::EditorSettings,
                    EditorSurface::Controls,
                ],
            )
            .unwrap();
        let map_settings = state
            .editor_menu_target(EditorMenuAction::MapSettings)
            .unwrap();
        assert_eq!(
            dispatch_editor_menu_pointer(
                &mut state,
                EditorMenuPointerEvent::Activate(Some(map_settings)),
            ),
            [
                MapMakerUiUpdate::EditorMenuActionFocused(EditorMenuAction::MapSettings),
                MapMakerUiUpdate::EditorSurfaceChanged(EditorSurface::MapSettings),
            ]
        );
        assert_eq!(state.editor_surface(), EditorSurface::MapSettings);

        state.dispatch(MapMakerUiCommand::Back);
        state
            .install_original_editor_menu(EditorMenuGeneration::new(6))
            .unwrap();
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(6),
                vec![
                    EditorSurface::MapPreview,
                    EditorSurface::MapSettings,
                    EditorSurface::EditorSettings,
                    EditorSurface::Controls,
                ],
            )
            .unwrap();
        let controls = state
            .editor_menu_target(EditorMenuAction::Controls)
            .unwrap();
        assert_eq!(
            dispatch_editor_menu_pointer(
                &mut state,
                EditorMenuPointerEvent::Activate(Some(controls)),
            ),
            [
                MapMakerUiUpdate::EditorMenuActionFocused(EditorMenuAction::Controls),
                MapMakerUiUpdate::EditorSurfaceChanged(EditorSurface::Controls),
            ]
        );
        assert_eq!(state.editor_surface(), EditorSurface::Controls);
    }

    #[test]
    fn load_map_keyboard_action_enters_owned_storage_and_back_returns_to_editor() {
        let mut state = loaded_editor();
        let load_map = state.editor_menu_target(EditorMenuAction::LoadMap).unwrap();
        state.dispatch(MapMakerUiCommand::FocusEditorMenuAction(load_map));
        let mut repeat_at = [0; 4];
        let accept = IntentSet::default().with(MenuIntent::Accept);

        assert_eq!(
            dispatch_editor_menu_frame(
                &mut state,
                accept,
                IntentSet::default(),
                10,
                &mut repeat_at,
            ),
            [MapMakerUiUpdate::PageChanged(MapMakerPage::StorageSource)]
        );
        assert_eq!(
            state.dispatch(MapMakerUiCommand::SelectLocalStorage),
            MapMakerUiUpdate::PageChanged(MapMakerPage::MapList)
        );
        assert_eq!(
            state.dispatch(MapMakerUiCommand::Back),
            MapMakerUiUpdate::PageChanged(MapMakerPage::StorageSource)
        );
        assert_eq!(
            state.dispatch(MapMakerUiCommand::Back),
            MapMakerUiUpdate::PageChanged(MapMakerPage::Editor)
        );
        assert_eq!(state.editor_surface(), EditorSurface::MainMenu);
        assert!(
            state
                .editor_menu_target(EditorMenuAction::LoadMap)
                .is_some()
        );

        let load_map = state.editor_menu_target(EditorMenuAction::LoadMap).unwrap();
        assert_eq!(
            dispatch_editor_menu_pointer(
                &mut state,
                EditorMenuPointerEvent::Activate(Some(load_map)),
            ),
            [MapMakerUiUpdate::PageChanged(MapMakerPage::StorageSource)]
        );
    }

    #[test]
    fn clear_map_dialog_is_owned_and_keyboard_accept_emits_typed_action() {
        let mut state = loaded_editor();
        let clear = state
            .editor_menu_target(EditorMenuAction::ClearMap)
            .unwrap();
        assert_eq!(
            dispatch_editor_menu_pointer(&mut state, EditorMenuPointerEvent::Activate(Some(clear)),),
            [
                MapMakerUiUpdate::EditorMenuActionFocused(EditorMenuAction::ClearMap),
                MapMakerUiUpdate::EditorConfirmationOpened(EditorConfirmation::ClearMap),
            ]
        );
        assert_eq!(
            state.snapshot().editor_confirmation,
            Some(EditorConfirmation::ClearMap)
        );
        assert_eq!(
            state.focused_editor_confirmation_choice(),
            Some(EditorConfirmationChoice::Cancel)
        );

        let mut repeat_at = [0; 4];
        let down = IntentSet::default().with(MenuIntent::Down);
        assert_eq!(
            dispatch_editor_confirmation_frame(
                &mut state,
                down,
                IntentSet::default(),
                10,
                &mut repeat_at,
            ),
            [MapMakerUiUpdate::EditorConfirmationFocused(
                EditorConfirmationChoice::Accept
            )]
        );
        let accept = IntentSet::default().with(MenuIntent::Accept);
        assert_eq!(
            dispatch_editor_menu_frame(&mut state, accept, down, 20, &mut repeat_at),
            [MapMakerUiUpdate::EditorMenuActionActivated(
                EditorMenuAction::ClearMap
            )]
        );
        assert_eq!(state.editor_confirmation(), None);
    }

    #[test]
    fn clear_map_confirmation_pointer_cancel_and_back_never_emit_clear() {
        let mut state = loaded_editor();
        let clear = state
            .editor_menu_target(EditorMenuAction::ClearMap)
            .unwrap();
        dispatch_editor_menu_pointer(&mut state, EditorMenuPointerEvent::Activate(Some(clear)));
        let cancel = state
            .editor_confirmation_target(EditorConfirmationChoice::Cancel)
            .unwrap();
        assert_eq!(
            dispatch_editor_confirmation_pointer(
                &mut state,
                EditorConfirmationPointerEvent::Activate(Some(cancel)),
            ),
            [MapMakerUiUpdate::EditorConfirmationDismissed(
                EditorConfirmation::ClearMap
            )]
        );
        assert_eq!(state.editor_confirmation(), None);

        dispatch_editor_menu_pointer(&mut state, EditorMenuPointerEvent::Activate(Some(clear)));
        assert_eq!(
            dispatch_editor_confirmation_pointer(&mut state, EditorConfirmationPointerEvent::Back,),
            [MapMakerUiUpdate::EditorConfirmationDismissed(
                EditorConfirmation::ClearMap
            )]
        );
        assert_eq!(state.editor_confirmation(), None);

        dispatch_editor_menu_pointer(&mut state, EditorMenuPointerEvent::Activate(Some(clear)));
        let mut repeat_at = [0; 4];
        let back = IntentSet::default().with(MenuIntent::Back);
        assert_eq!(
            dispatch_editor_menu_frame(&mut state, back, IntentSet::default(), 30, &mut repeat_at,),
            [MapMakerUiUpdate::EditorConfirmationDismissed(
                EditorConfirmation::ClearMap
            )]
        );
        assert_eq!(state.editor_confirmation(), None);
    }

    #[test]
    fn quit_map_confirmation_keyboard_cancel_and_accept_use_owned_action() {
        let mut state = loaded_editor();
        let quit = state
            .editor_menu_target(EditorMenuAction::QuitMapMaker)
            .unwrap();
        assert_eq!(
            dispatch_editor_menu_pointer(&mut state, EditorMenuPointerEvent::Activate(Some(quit))),
            [
                MapMakerUiUpdate::EditorMenuActionFocused(EditorMenuAction::QuitMapMaker),
                MapMakerUiUpdate::EditorMenuActionActivated(EditorMenuAction::QuitMapMaker),
            ]
        );
        assert_eq!(
            state.dispatch(MapMakerUiCommand::ObserveQuitMapMakerConfirmation),
            MapMakerUiUpdate::EditorConfirmationOpened(EditorConfirmation::QuitMapMaker)
        );
        assert_eq!(
            state.focused_editor_confirmation_choice(),
            Some(EditorConfirmationChoice::Cancel)
        );

        let mut repeat_at = [0; 4];
        let down = IntentSet::default().with(MenuIntent::Down);
        assert_eq!(
            dispatch_editor_confirmation_frame(
                &mut state,
                down,
                IntentSet::default(),
                10,
                &mut repeat_at,
            ),
            [MapMakerUiUpdate::EditorConfirmationFocused(
                EditorConfirmationChoice::Accept
            )]
        );
        let accept = IntentSet::default().with(MenuIntent::Accept);
        assert_eq!(
            dispatch_editor_confirmation_frame(&mut state, accept, down, 20, &mut repeat_at,),
            [MapMakerUiUpdate::EditorMenuActionActivated(
                EditorMenuAction::QuitMapMaker
            )]
        );
        assert_eq!(state.editor_confirmation(), None);
    }

    #[test]
    fn center_map_keyboard_and_pointer_emit_typed_viewport_request() {
        let mut state = loaded_editor();
        let target = state
            .editor_menu_target(EditorMenuAction::CenterMap)
            .unwrap();
        state.dispatch(MapMakerUiCommand::FocusEditorMenuAction(target));

        let accept = IntentSet::default().with(MenuIntent::Accept);
        let mut repeat_at = [0; 4];
        assert_eq!(
            dispatch_editor_menu_frame(
                &mut state,
                accept,
                IntentSet::default(),
                10,
                &mut repeat_at,
            ),
            [MapMakerUiUpdate::EditorViewportOperationRequested(
                EditorViewportOperation::CenterMap,
            )]
        );

        let mut state = loaded_editor();
        let target = state
            .editor_menu_target(EditorMenuAction::CenterMap)
            .unwrap();
        assert_eq!(
            dispatch_editor_menu_pointer(
                &mut state,
                EditorMenuPointerEvent::Activate(Some(target)),
            ),
            [
                MapMakerUiUpdate::EditorMenuActionFocused(EditorMenuAction::CenterMap),
                MapMakerUiUpdate::EditorViewportOperationRequested(
                    EditorViewportOperation::CenterMap,
                ),
            ]
        );
    }

    #[test]
    fn editor_menu_pointer_rejects_stale_generation_and_child_pages() {
        let mut state = loaded_editor();
        let stale = state.editor_menu_target(EditorMenuAction::SaveMap).unwrap();
        state
            .install_original_editor_menu(EditorMenuGeneration::new(5))
            .unwrap();
        assert!(dispatch_editor_menu_pointer(
            &mut state,
            EditorMenuPointerEvent::Activate(Some(stale)),
        )
        .is_empty());

        let map_settings = state
            .editor_surface_target(EditorSurface::MapSettings)
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenEditorSurface(map_settings));
        assert!(dispatch_editor_menu_pointer(
            &mut state,
            EditorMenuPointerEvent::Activate(Some(stale)),
        )
        .is_empty());
    }

    #[test]
    fn keyboard_focus_accept_and_back_drive_owned_editor_surface_history() {
        let mut state = loaded_editor();
        let mut repeat_at = [0; 4];
        let down = IntentSet::default().with(MenuIntent::Down);
        assert_eq!(
            dispatch_frame(&mut state, down, IntentSet::default(), 10, &mut repeat_at,),
            [MapMakerUiUpdate::EditorSurfaceFocused(
                EditorSurface::MapPreview
            )]
        );
        let accept = IntentSet::default().with(MenuIntent::Accept);
        assert_eq!(
            dispatch_frame(&mut state, accept, down, 20, &mut repeat_at),
            [MapMakerUiUpdate::EditorSurfaceChanged(
                EditorSurface::MapPreview
            )]
        );
        assert_eq!(state.page(), MapMakerPage::Editor);
        assert_eq!(state.editor_surface(), EditorSurface::MapPreview);
        let back = IntentSet::default().with(MenuIntent::Back);
        assert_eq!(
            dispatch_frame(&mut state, back, accept, 30, &mut repeat_at),
            [MapMakerUiUpdate::EditorSurfaceChanged(
                EditorSurface::MainMenu
            )]
        );
    }

    #[test]
    fn pointer_hover_activate_and_back_drive_owned_editor_surface_history() {
        let mut state = loaded_editor();
        let target = state
            .editor_surface_target(EditorSurface::EditorSettings)
            .unwrap();
        assert_eq!(
            dispatch_pointer(&mut state, EditorPointerEvent::Hover(Some(target)),),
            [MapMakerUiUpdate::EditorSurfaceFocused(
                EditorSurface::EditorSettings
            )]
        );
        assert_eq!(
            dispatch_pointer(&mut state, EditorPointerEvent::Activate(Some(target)),),
            [MapMakerUiUpdate::EditorSurfaceChanged(
                EditorSurface::EditorSettings
            )]
        );
        assert_eq!(state.editor_surface(), EditorSurface::EditorSettings);
        assert_eq!(
            dispatch_pointer(&mut state, EditorPointerEvent::Back),
            [MapMakerUiUpdate::EditorSurfaceChanged(
                EditorSurface::MainMenu
            )]
        );
    }

    #[test]
    fn pointer_ignores_stale_targets_and_events_outside_loaded_editor() {
        let mut state = loaded_editor();
        let stale_target = state
            .editor_surface_target(EditorSurface::MapSettings)
            .unwrap();
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(5),
                vec![EditorSurface::MapSettings],
            )
            .unwrap();
        assert!(
            dispatch_pointer(
                &mut state,
                EditorPointerEvent::Activate(Some(EditorSurfaceTarget::new(
                    EditorSurfaceGeneration::new(5),
                    EditorSurface::PaletteProperties,
                ))),
            )
            .is_empty()
        );
        assert!(
            dispatch_pointer(&mut state, EditorPointerEvent::Activate(Some(stale_target)))
                .is_empty()
        );
        assert_eq!(state.editor_surface(), EditorSurface::MainMenu);
        let mut map_list = MapMakerUiState::default();
        map_list.dispatch(MapMakerUiCommand::OpenStorage);
        map_list.dispatch(MapMakerUiCommand::SelectLocalStorage);
        assert_eq!(map_list.page(), MapMakerPage::MapList);
        assert!(
            dispatch_pointer(
                &mut map_list,
                EditorPointerEvent::Activate(Some(EditorSurfaceTarget::new(
                    EditorSurfaceGeneration::new(5),
                    EditorSurface::MapSettings,
                ))),
            )
            .is_empty()
        );
    }

    #[test]
    fn palette_properties_keyboard_and_pointer_use_typed_actions() {
        let mut state = palette_properties_page();
        let mut repeat_at = [0; 4];
        let down = IntentSet::default().with(MenuIntent::Down);
        assert_eq!(
            dispatch_palette_properties_frame(
                &mut state,
                down,
                IntentSet::default(),
                10,
                &mut repeat_at,
            ),
            [MapMakerUiUpdate::PalettePropertyFocused(
                PalettePropertyAction::ChangePalette
            )]
        );
        let accept = IntentSet::default().with(MenuIntent::Accept);
        assert_eq!(
            dispatch_palette_properties_frame(&mut state, accept, down, 20, &mut repeat_at),
            [MapMakerUiUpdate::PalettePageReentered]
        );
        assert_eq!(state.editor_surface(), EditorSurface::PaletteGrid);
        assert_eq!(
            state.dispatch(MapMakerUiCommand::Back),
            MapMakerUiUpdate::EditorSurfaceChanged(EditorSurface::PaletteProperties)
        );

        let mut state = palette_properties_page();
        let change_palette = state
            .palette_property_target(PalettePropertyAction::ChangePalette)
            .unwrap();
        let target = state
            .palette_property_target(PalettePropertyAction::LightRow)
            .unwrap();
        assert_eq!(
            dispatch_palette_properties_pointer(
                &mut state,
                PalettePropertiesPointerEvent::Hover(Some(change_palette)),
            ),
            [MapMakerUiUpdate::PalettePropertyFocused(
                PalettePropertyAction::ChangePalette
            )]
        );
        assert_eq!(
            dispatch_palette_properties_pointer(
                &mut state,
                PalettePropertiesPointerEvent::Hover(Some(target)),
            ),
            [MapMakerUiUpdate::PalettePropertyFocused(
                PalettePropertyAction::LightRow
            )]
        );
        assert_eq!(
            dispatch_palette_properties_pointer(
                &mut state,
                PalettePropertiesPointerEvent::Activate(Some(target)),
            ),
            [MapMakerUiUpdate::PalettePropertyActivated(
                PalettePropertyAction::LightRow
            )]
        );

        let stale = state
            .palette_property_target(PalettePropertyAction::ChangePalette)
            .unwrap();
        state
            .install_palette_properties_catalog(
                PalettePropertiesGeneration::new(9),
                vec![PalettePropertyAction::LightRow],
            )
            .unwrap();
        assert!(
            dispatch_palette_properties_pointer(
                &mut state,
                PalettePropertiesPointerEvent::Activate(Some(stale)),
            )
            .is_empty()
        );

        assert_eq!(
            dispatch_palette_properties_pointer(&mut state, PalettePropertiesPointerEvent::Back,),
            [MapMakerUiUpdate::EditorSurfaceChanged(
                EditorSurface::MainMenu
            )]
        );
    }

    #[test]
    fn palette_grid_keyboard_and_pointer_use_typed_choice_ids() {
        let generation = PaletteGridGeneration::new(12);
        let mut properties = palette_properties_page();
        assert_eq!(
            properties.install_palette_grid_catalog(
                generation,
                vec![PaletteGridChoiceId::new(40), PaletteGridChoiceId::new(41)],
            ),
            Err(PaletteGridCatalogError::NotOnPaletteGrid)
        );

        let mut state = palette_properties_page();
        let mut repeat_at = [0; 4];
        let mut grid_repeat_at = [0; 4];
        let down = IntentSet::default().with(MenuIntent::Down);
        dispatch_palette_properties_frame(
            &mut state,
            down,
            IntentSet::default(),
            10,
            &mut repeat_at,
        );
        let accept = IntentSet::default().with(MenuIntent::Accept);
        assert_eq!(
            dispatch_palette_properties_frame(&mut state, accept, down, 20, &mut repeat_at),
            [MapMakerUiUpdate::PalettePageReentered]
        );
        assert_eq!(state.editor_surface(), EditorSurface::PaletteGrid);

        let first = PaletteGridChoiceId::new(40);
        let second = PaletteGridChoiceId::new(41);
        state
            .install_palette_grid_catalog(generation, vec![first, second])
            .unwrap();
        assert_eq!(state.snapshot().palette_grid_choices, &[first, second]);
        assert_eq!(state.focused_palette_grid_choice(), Some(first));
        let second_target = state.palette_grid_choice_target(second).unwrap();
        assert_eq!(
            state.install_palette_grid_catalog(generation, vec![first, first]),
            Err(PaletteGridCatalogError::DuplicateChoice)
        );
        assert_eq!(
            state.palette_grid_choice_target(second),
            Some(second_target)
        );

        assert_eq!(
            dispatch_palette_grid_frame(
                &mut state,
                down,
                IntentSet::default(),
                30,
                &mut grid_repeat_at,
            ),
            [MapMakerUiUpdate::PaletteGridChoiceFocused(second)]
        );
        assert_eq!(
            dispatch_palette_grid_frame(&mut state, accept, down, 40, &mut grid_repeat_at),
            [MapMakerUiUpdate::PaletteGridChoiceActivated(second)]
        );
        let first_target = state.palette_grid_choice_target(first).unwrap();
        assert_eq!(
            dispatch_palette_grid_pointer(
                &mut state,
                PaletteGridPointerEvent::Hover(Some(first_target)),
            ),
            [MapMakerUiUpdate::PaletteGridChoiceFocused(first)]
        );
        assert_eq!(
            dispatch_palette_grid_pointer(
                &mut state,
                PaletteGridPointerEvent::Activate(Some(second_target)),
            ),
            [
                MapMakerUiUpdate::PaletteGridChoiceFocused(second),
                MapMakerUiUpdate::PaletteGridChoiceActivated(second),
            ]
        );

        state
            .install_palette_grid_catalog(PaletteGridGeneration::new(13), vec![first, second])
            .unwrap();
        assert!(
            dispatch_palette_grid_pointer(
                &mut state,
                PaletteGridPointerEvent::Activate(Some(second_target)),
            )
            .is_empty()
        );
        assert_eq!(
            dispatch_palette_grid_pointer(&mut state, PaletteGridPointerEvent::Back),
            [MapMakerUiUpdate::EditorSurfaceChanged(
                EditorSurface::PaletteProperties
            )]
        );
        assert!(!state.contains_palette_grid_choice_target(second_target));
    }

    #[test]
    fn loaded_editor_preview_keyboard_and_pointer_use_owned_choice_ids() {
        let mut state = loaded_editor();
        let preview = state
            .editor_surface_target(EditorSurface::MapPreview)
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenEditorSurface(preview));
        let choices = [EditorPreviewChoiceId::new(0), EditorPreviewChoiceId::new(1)];
        let generation = EditorPreviewGeneration::new(7);
        state
            .install_editor_preview_catalog(generation, choices.to_vec())
            .unwrap();
        let first_target = state.editor_preview_choice_target(choices[0]).unwrap();
        let second_target = state.editor_preview_choice_target(choices[1]).unwrap();
        let mut repeat_at = [0; 4];
        let down = IntentSet::default().with(MenuIntent::Down);
        let accept = IntentSet::default().with(MenuIntent::Accept);
        assert_eq!(
            dispatch_editor_preview_frame(
                &mut state,
                down,
                IntentSet::default(),
                10,
                &mut repeat_at,
            ),
            [MapMakerUiUpdate::EditorPreviewChoiceFocused(choices[1])]
        );
        assert_eq!(
            dispatch_editor_preview_frame(&mut state, accept, down, 20, &mut repeat_at),
            [MapMakerUiUpdate::EditorPreviewSelectionRequested(
                PreviewSelection::from_row(PreviewRow(1)).unwrap()
            )]
        );
        assert_eq!(
            dispatch_editor_preview_pointer(
                &mut state,
                EditorPreviewPointerEvent::Hover(Some(first_target)),
            ),
            [MapMakerUiUpdate::EditorPreviewChoiceFocused(choices[0])]
        );
        assert_eq!(
            dispatch_editor_preview_pointer(
                &mut state,
                EditorPreviewPointerEvent::Activate(Some(second_target)),
            ),
            [
                MapMakerUiUpdate::EditorPreviewChoiceFocused(choices[1]),
                MapMakerUiUpdate::EditorPreviewSelectionRequested(
                    PreviewSelection::from_row(PreviewRow(1)).unwrap()
                ),
            ]
        );
        state.dispatch(MapMakerUiCommand::Back);
        assert!(!state.contains_editor_preview_choice_target(second_target));
        assert!(
            dispatch_editor_preview_pointer(
                &mut state,
                EditorPreviewPointerEvent::Activate(Some(second_target)),
            )
            .is_empty()
        );
    }
}
