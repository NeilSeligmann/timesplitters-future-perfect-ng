//! Native keyboard/controller actions for the Rust-owned Map Maker mode page.
//!
//! The device adapters already produce semantic [`IntentSet`] values. This
//! module resolves those intents against the owned mode-choice identities and
//! returns typed commands for [`MapMakerUiState`]; it never projects input to
//! guest memory or manufactures pad events.

use crate::{input::IntentSet, owned_catalogue::CatalogueAction};
use tsfp_mapmaker::ui::{
    MapMakerPage, MapMakerUiCommand, MapMakerUiState, MapMakerUiUpdate, ModeChoiceId,
    PreviewWarningChoice,
};

/// Pointer events after the page hit-test resolves a row to its opaque runtime identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerEvent {
    Hover(ModeChoiceId),
    Click(ModeChoiceId),
    StartPreview,
    ResolvePreviewWarning(PreviewWarningChoice),
    Back,
}

/// Resolve one semantic input frame to commands for the owned mode page.
///
/// Up and Down share the Rust-owned catalogue's edge/repeat schedule. Movement
/// stays within the ordered runtime list; an absent focus starts at the first
/// row for Down or the last row for Up. Accept selects the focused stable ID or
/// starts Preview; while a warning is open, Accept means Continue. Back keeps
/// the catalogue reducer's precedence over other intents.
///
/// The endpoint behavior is native policy, **INFERRED** pending the original
/// mode-page replay. A frame with no choices produces no focus or selection.
pub fn commands(
    state: &MapMakerUiState,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<MapMakerUiCommand> {
    let page = state.page();
    if page == MapMakerPage::EditorMenu {
        return Vec::new();
    }

    let choices = state.mode_choices();
    if page == MapMakerPage::ModeSelection && choices.is_empty() {
        return Vec::new();
    }

    let mut cursor = state
        .focused_mode()
        .and_then(|focused| choices.iter().position(|choice| choice.id == focused));
    let mut result = Vec::new();
    for action in crate::owned_catalogue::actions(held, before, now_ms, repeat_at, false) {
        match action {
            CatalogueAction::Back => result.push(MapMakerUiCommand::Back),
            CatalogueAction::Step(delta) if page == MapMakerPage::ModeSelection => {
                let next = match cursor {
                    Some(index) => index.checked_add_signed(delta as isize),
                    None if delta < 0 => Some(choices.len() - 1),
                    None => Some(0),
                };
                let Some(next) = next.filter(|&index| index < choices.len()) else {
                    continue;
                };
                let id = choices[next].id;
                if cursor != Some(next) {
                    result.push(MapMakerUiCommand::FocusMode(id));
                    cursor = Some(next);
                }
            }
            CatalogueAction::Accept if page == MapMakerPage::ModeSelection => {
                if let Some(id) = cursor.map(|index| choices[index].id) {
                    result.push(MapMakerUiCommand::SelectMode(id));
                }
            }
            CatalogueAction::Accept if page == MapMakerPage::Preview => {
                if state.snapshot().pending_preview_warning.is_some() {
                    result.push(MapMakerUiCommand::ResolvePreviewWarning(
                        PreviewWarningChoice::Continue,
                    ));
                } else {
                    result.push(MapMakerUiCommand::StartPreview);
                }
            }
            CatalogueAction::Step(_) | CatalogueAction::Accept | CatalogueAction::Focus(_) => {}
        }
    }
    result
}

/// Apply one keyboard/controller frame to the Rust-owned mode page.
///
/// This keeps semantic device input, stable row identity, and page-state
/// reduction on the Rust side of the UI seam. The returned updates are useful
/// to a renderer or host coordinator; they do not call lifted handlers.
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

/// Convenience for pointer adapters to keep their stable-ID command typed in
/// the same input module as the keyboard/controller mapping.
pub const fn focus_mode(id: ModeChoiceId) -> MapMakerUiCommand {
    MapMakerUiCommand::FocusMode(id)
}

/// Resolve a native pointer event to the same typed identity commands as the
/// keyboard/controller path. Hit testing owns geometry; this seam only accepts
/// IDs installed in the current mode table. The policy is **INFERRED** pending
/// the owner-route replay.
pub fn pointer_commands(state: &MapMakerUiState, event: PointerEvent) -> Vec<MapMakerUiCommand> {
    if state.page() == MapMakerPage::EditorMenu {
        return Vec::new();
    }

    match event {
        PointerEvent::Back => vec![MapMakerUiCommand::Back],
        PointerEvent::StartPreview if state.page() == MapMakerPage::Preview => {
            vec![MapMakerUiCommand::StartPreview]
        }
        PointerEvent::StartPreview => Vec::new(),
        PointerEvent::ResolvePreviewWarning(choice)
            if state.page() == MapMakerPage::Preview
                && state.snapshot().pending_preview_warning.is_some() =>
        {
            vec![MapMakerUiCommand::ResolvePreviewWarning(choice)]
        }
        PointerEvent::ResolvePreviewWarning(_) => Vec::new(),
        PointerEvent::Hover(id) | PointerEvent::Click(id) => {
            if state.page() != MapMakerPage::ModeSelection {
                return Vec::new();
            }
            if !state.mode_choices().iter().any(|choice| choice.id == id) {
                return Vec::new();
            }

            let clicked = matches!(event, PointerEvent::Click(_));
            let mut commands = Vec::with_capacity(2);
            if state.focused_mode() != Some(id) {
                commands.push(MapMakerUiCommand::FocusMode(id));
            }
            if clicked {
                commands.push(MapMakerUiCommand::SelectMode(id));
            }
            commands
        }
    }
}

/// Apply one hit-tested pointer event through the same Rust-owned reducer used
/// by keyboard and controller input.
pub fn dispatch_pointer(state: &mut MapMakerUiState, event: PointerEvent) -> Vec<MapMakerUiUpdate> {
    pointer_commands(state, event)
        .into_iter()
        .map(|command| state.dispatch(command))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{PointerEvent, commands, dispatch_frame, dispatch_pointer, pointer_commands};
    use crate::input::{IntentSet, MenuIntent as I};
    use tsfp_mapmaker::{
        modes::{
            AssaultModeConfiguration, EditorKind, GameType, LaunchVariant, ModeConfiguration,
            ModeSettings, PreviewRow, PreviewSelection, ThemeId,
        },
        ui::{
            MapMakerPage, MapMakerUiCommand as C, MapMakerUiState, MapMakerUiUpdate, ModeChoice,
            ModeChoiceId, PreviewStartGate, PreviewTextId, PreviewWarningChoice,
        },
    };

    fn mode() -> ModeConfiguration {
        ModeConfiguration::Assault(AssaultModeConfiguration {
            settings: ModeSettings {
                editor_kind: EditorKind(7),
                theme: ThemeId(3),
                preview: PreviewSelection {
                    row: PreviewRow(4),
                    variant: LaunchVariant(0x15),
                    game_type: GameType(9),
                },
                row_options: 0x12,
                duration: 900,
            },
            phases: Default::default(),
        })
    }

    fn state() -> (MapMakerUiState, ModeChoiceId, ModeChoiceId) {
        let first = ModeChoiceId::new(0xfeed);
        let second = ModeChoiceId::new(0xbeef);
        let mut state = MapMakerUiState::default();
        state
            .install_mode_choices(vec![
                ModeChoice {
                    id: first,
                    configuration: mode(),
                },
                ModeChoice {
                    id: second,
                    configuration: mode(),
                },
            ])
            .unwrap();
        state.dispatch(C::OpenModeSelection);
        (state, first, second)
    }

    #[test]
    fn semantic_keyboard_navigation_and_accept_reach_owned_mode_state() {
        let (mut state, first, second) = state();
        let down = IntentSet::default().with(I::Down);
        let mut repeat = [0; 4];
        let next = commands(&state, down, IntentSet::default(), 10, &mut repeat);
        assert_eq!(next, [C::FocusMode(first)]);
        for command in next {
            state.dispatch(command);
        }

        let next = commands(&state, down, down, 409, &mut repeat);
        assert!(next.is_empty());
        let next = commands(&state, down, down, 410, &mut repeat);
        assert_eq!(next, [C::FocusMode(second)]);
        for command in next {
            state.dispatch(command);
        }

        let accept = IntentSet::default().with(I::Accept);
        assert_eq!(
            commands(&state, accept, IntentSet::default(), 411, &mut repeat),
            [C::SelectMode(second)]
        );
    }

    #[test]
    fn keyboard_frame_reducer_updates_owned_focus_and_selected_mode() {
        let (mut state, first, second) = state();
        let down = IntentSet::default().with(I::Down);
        let mut repeat = [0; 4];
        assert_eq!(
            dispatch_frame(&mut state, down, IntentSet::default(), 10, &mut repeat),
            [MapMakerUiUpdate::ModeFocused(first)]
        );
        assert_eq!(state.focused_mode(), Some(first));

        assert_eq!(
            dispatch_frame(&mut state, down, down, 410, &mut repeat),
            [MapMakerUiUpdate::ModeFocused(second)]
        );
        let accept = IntentSet::default().with(I::Accept);
        assert_eq!(
            dispatch_frame(&mut state, accept, IntentSet::default(), 411, &mut repeat),
            [MapMakerUiUpdate::PageChanged(MapMakerPage::ModeSettings)]
        );
        assert_eq!(state.selected_mode_id(), Some(second));
        assert_eq!(state.page(), MapMakerPage::ModeSettings);
    }

    #[test]
    fn back_wins_and_navigation_stays_inside_the_ordered_list() {
        let (mut state, first, _) = state();
        let up = IntentSet::default().with(I::Up);
        let mut repeat = [0; 4];
        assert_eq!(
            commands(&state, up, IntentSet::default(), 0, &mut repeat),
            [C::FocusMode(ModeChoiceId::new(0xbeef))]
        );
        state.dispatch(C::FocusMode(first));
        assert!(commands(&state, up, IntentSet::default(), 1, &mut repeat).is_empty());
        let mixed = up.with(I::Back).with(I::Accept);
        assert_eq!(
            commands(&state, mixed, IntentSet::default(), 1, &mut repeat),
            [C::Back]
        );
    }

    #[test]
    fn mode_input_is_ignored_after_leaving_the_mode_page() {
        let (mut state, _, _) = state();
        state.dispatch(C::Back);
        assert_eq!(state.page(), MapMakerPage::EditorMenu);
        let down = IntentSet::default().with(I::Down);
        assert!(commands(&state, down, IntentSet::default(), 0, &mut [0; 4]).is_empty());
    }

    #[test]
    fn pointer_hover_and_click_resolve_only_installed_mode_identities() {
        let (mut state, first, second) = state();
        assert_eq!(
            pointer_commands(&state, PointerEvent::Hover(second)),
            [C::FocusMode(second)]
        );
        for command in pointer_commands(&state, PointerEvent::Click(first)) {
            state.dispatch(command);
        }
        assert_eq!(state.page(), MapMakerPage::ModeSettings);
        assert_eq!(state.selected_mode_id(), Some(first));
        assert!(pointer_commands(&state, PointerEvent::Click(ModeChoiceId::new(9))).is_empty());
        let back = pointer_commands(&state, PointerEvent::Back);
        assert_eq!(back, [C::Back]);
        for command in back {
            state.dispatch(command);
        }
        assert_eq!(state.page(), MapMakerPage::ModeSelection);
    }

    #[test]
    fn pointer_back_returns_a_typed_page_command() {
        let (mut state, _, _) = state();
        assert_eq!(pointer_commands(&state, PointerEvent::Back), [C::Back]);
        state.dispatch(C::Back);
        assert_eq!(state.page(), MapMakerPage::EditorMenu);
        assert!(
            pointer_commands(&state, PointerEvent::Click(ModeChoiceId::new(0xbeef))).is_empty()
        );
    }

    #[test]
    fn pointer_dispatch_uses_owned_reducer_for_select_and_back() {
        let (mut state, _, second) = state();
        assert_eq!(
            dispatch_pointer(&mut state, PointerEvent::Click(second)),
            [
                MapMakerUiUpdate::ModeFocused(second),
                MapMakerUiUpdate::PageChanged(MapMakerPage::ModeSettings),
            ]
        );
        assert_eq!(state.selected_mode_id(), Some(second));
        assert_eq!(
            dispatch_pointer(&mut state, PointerEvent::Back),
            [MapMakerUiUpdate::PageChanged(MapMakerPage::ModeSelection)]
        );
        assert_eq!(
            dispatch_frame(
                &mut state,
                IntentSet::default().with(I::Back),
                IntentSet::default(),
                20,
                &mut [0; 4],
            ),
            [MapMakerUiUpdate::PageChanged(MapMakerPage::EditorMenu)]
        );
        assert_eq!(state.page(), MapMakerPage::EditorMenu);
    }

    #[test]
    fn preview_start_uses_capacity_then_overrideable_warning_gate() {
        let (mut state, first, _) = state();
        state.dispatch(C::SelectMode(first));
        state.dispatch(C::OpenPreview);
        let row = state.preview_row().unwrap();
        let accept = IntentSet::default().with(I::Accept);
        let updates = dispatch_frame(&mut state, accept, IntentSet::default(), 10, &mut [0; 4]);
        let request = match updates.as_slice() {
            [MapMakerUiUpdate::PreviewStartCheckRequested(request)] => *request,
            other => panic!("expected one typed preview gate request, got {other:?}"),
        };
        assert_eq!(request.row, row);
        assert_eq!(state.snapshot().pending_preview_gate, Some(request));
        assert_eq!(state.dispatch(C::StartPreview), MapMakerUiUpdate::Ignored);

        assert_eq!(
            state.dispatch(C::CompletePreviewStartGate {
                request: request.id,
                gate: PreviewStartGate::CapacityBlocked,
            }),
            MapMakerUiUpdate::PreviewCapacityBlocked {
                row,
                text: PreviewTextId::new(0x16ee),
            }
        );
        assert!(state.snapshot().preview_capacity_blocked);
        assert_eq!(
            dispatch_frame(
                &mut state,
                IntentSet::default().with(I::Back),
                IntentSet::default(),
                20,
                &mut [0; 4],
            ),
            [MapMakerUiUpdate::PreviewCapacityDismissed(row)]
        );
        assert_eq!(state.page(), MapMakerPage::Preview);

        let next_request = match dispatch_pointer(&mut state, PointerEvent::StartPreview).as_slice()
        {
            [MapMakerUiUpdate::PreviewStartCheckRequested(request)] => *request,
            other => panic!("expected one typed preview gate request, got {other:?}"),
        };
        let warning = PreviewTextId::new(0x0aec);
        assert_eq!(
            state.dispatch(C::CompletePreviewStartGate {
                request: next_request.id,
                gate: PreviewStartGate::ContinueAnyway(warning),
            }),
            MapMakerUiUpdate::PreviewWarningOpened { row, text: warning }
        );
        assert_eq!(state.snapshot().pending_preview_warning, Some(warning));
        assert_eq!(
            pointer_commands(
                &state,
                PointerEvent::ResolvePreviewWarning(PreviewWarningChoice::Cancel),
            ),
            [C::ResolvePreviewWarning(PreviewWarningChoice::Cancel)]
        );
        assert_eq!(
            dispatch_frame(
                &mut state,
                IntentSet::default().with(I::Back),
                IntentSet::default(),
                30,
                &mut [0; 4],
            ),
            [MapMakerUiUpdate::PreviewWarningDismissed(row)]
        );
        assert_eq!(state.page(), MapMakerPage::Preview);

        let next_request = match dispatch_pointer(&mut state, PointerEvent::StartPreview).as_slice()
        {
            [MapMakerUiUpdate::PreviewStartCheckRequested(request)] => *request,
            other => panic!("expected one typed preview gate request, got {other:?}"),
        };
        assert_eq!(
            state.dispatch(C::CompletePreviewStartGate {
                request: next_request.id,
                gate: PreviewStartGate::Allowed,
            }),
            MapMakerUiUpdate::PreviewLaunchRequested(row)
        );

        state.dispatch(C::Back);
        state.dispatch(C::OpenPreview);
        let newer_request =
            match dispatch_pointer(&mut state, PointerEvent::StartPreview).as_slice() {
                [MapMakerUiUpdate::PreviewStartCheckRequested(request)] => *request,
                other => panic!("expected one typed preview gate request, got {other:?}"),
            };
        assert_ne!(newer_request.id, next_request.id);
        assert_eq!(
            state.dispatch(C::CompletePreviewStartGate {
                request: next_request.id,
                gate: PreviewStartGate::Allowed,
            }),
            MapMakerUiUpdate::Ignored
        );
        assert_eq!(state.snapshot().pending_preview_gate, Some(newer_request));
        let warning = PreviewTextId::new(0x0aec);
        state.dispatch(C::CompletePreviewStartGate {
            request: newer_request.id,
            gate: PreviewStartGate::ContinueAnyway(warning),
        });
        assert_eq!(
            dispatch_frame(
                &mut state,
                IntentSet::default().with(I::Accept),
                IntentSet::default(),
                40,
                &mut [0; 4],
            ),
            [MapMakerUiUpdate::PreviewLaunchRequested(row)]
        );
    }
}
