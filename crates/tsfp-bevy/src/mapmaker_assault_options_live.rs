// SPDX-License-Identifier: GPL-3.0-or-later
//! T2408: route Assault Options page focus and activation through typed Rust state.
//!
//! T1714 measured the four row IDs and order. The original page still renders
//! and handles effects; this typed event capture is a temporary callback seam
//! pending the native renderer and owned launch handlers.

use std::sync::Mutex;

use tsfp_mapmaker::assault_options::{
    ASSAULT_OPTIONS_ORDER, AssaultOption, AssaultOptionsCommand, AssaultOptionsGeneration,
    AssaultOptionsState, AssaultOptionsUpdate, AssaultParticipantChoice,
    AssaultParticipantChoiceId, AssaultParticipantField, AssaultParticipantRuntimeOptionId,
};
use tsfp_ui_input::live_page::{LivePage, LiveText};
use tsfp_ui_input::native_page::NativeCommand;
use tsfp_ui_input::widgets::Step;
use tsfp_ui_model::mapmaker_assault_options::{
    AssaultOptionsLayout, AssaultOptionsWidgetLayout, build_assault_options_page,
};
use tsfp_ui_model::model::{Page, WidgetId};

pub const BUILDER: u32 = 0x31_3FE0;

#[derive(Default)]
struct Session {
    active_page: Option<(u32, u32)>,
    generation: u64,
    state: Option<AssaultOptionsState>,
    updates: Vec<AssaultOptionsUpdate>,
}

/// Ordered semantic outcomes from one opening of the Assault Options page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssaultOptionsUpdateBatch {
    pub generation: AssaultOptionsGeneration,
    pub updates: Vec<AssaultOptionsUpdate>,
}

static SESSION: Mutex<Session> = Mutex::new(Session {
    active_page: None,
    generation: 0,
    state: None,
    updates: Vec::new(),
});

pub fn observe_top(top: Option<(u32, u32)>) {
    let page = top.filter(|(builder, _)| *builder == BUILDER);
    let Ok(mut session) = SESSION.lock() else {
        return;
    };
    if session.active_page != page {
        session.active_page = page;
        session.state = None;
        session.updates.clear();
        if page.is_some() {
            session.generation = session.generation.wrapping_add(1).max(1);
        }
    }
}

fn option_for_label(label: u32) -> Option<AssaultOption> {
    Some(match label {
        0x0AF1 => AssaultOption::PreviewTeam,
        0x0AF3 => AssaultOption::Attackers,
        0x0AF4 => AssaultOption::Defenders,
        0x0AF9 => AssaultOption::StartAssault,
        _ => return None,
    })
}

fn option_at(live: &LivePage, cursor_nodes: &[u32], index: usize) -> Option<AssaultOption> {
    let node = *cursor_nodes.get(index)?;
    let row = live.rows.iter().find(|row| row.node == node)?;
    let LiveText::Id(label) = &row.text else {
        return None;
    };
    option_for_label(*label)
}

fn participant_field(option: AssaultOption) -> Option<AssaultParticipantField> {
    option.participant_field()
}

fn runtime_choices(
    live: &LivePage,
    field: AssaultParticipantField,
) -> Option<(Vec<AssaultParticipantChoice>, AssaultParticipantChoiceId)> {
    let label = match field {
        AssaultParticipantField::Attackers => 0x0AF3,
        AssaultParticipantField::Defenders => 0x0AF4,
    };
    let row = live
        .rows
        .iter()
        .find(|row| row.text == LiveText::Id(label))?;
    if row.options.is_empty() {
        return None;
    }
    let choices = row
        .options
        .iter()
        .enumerate()
        .map(|(index, option)| {
            u32::try_from(index)
                .ok()
                .map(|ordinal| AssaultParticipantChoice {
                    id: AssaultParticipantChoiceId::new(ordinal),
                    runtime_option_id: AssaultParticipantRuntimeOptionId::new(option.id),
                    enabled: option.enabled,
                })
        })
        .collect::<Option<Vec<_>>>()?;
    let selected = AssaultParticipantChoiceId::new(row.selected);
    choices
        .iter()
        .any(|choice| choice.id == selected)
        .then_some((choices, selected))
}

fn exact_page_rows(live: &LivePage, cursor_nodes: &[u32]) -> bool {
    live.builder == BUILDER
        && cursor_nodes.len() == ASSAULT_OPTIONS_ORDER.len()
        && ASSAULT_OPTIONS_ORDER
            .iter()
            .enumerate()
            .all(|(index, expected)| option_at(live, cursor_nodes, index) == Some(*expected))
}

fn ensure_state(live: &LivePage, cursor_nodes: &[u32]) -> bool {
    if !crate::storage_live::mapmaker_ui_enabled() || !exact_page_rows(live, cursor_nodes) {
        return false;
    }
    let Ok(mut session) = SESSION.lock() else {
        return false;
    };
    if session.active_page != Some((live.builder, live.slot)) || session.generation == 0 {
        return false;
    }
    if session.state.is_none() {
        let mut state = AssaultOptionsState::new(AssaultOptionsGeneration::new(session.generation));
        for field in [
            AssaultParticipantField::Attackers,
            AssaultParticipantField::Defenders,
        ] {
            let Some((choices, selected)) = runtime_choices(live, field) else {
                return false;
            };
            if !state.install_participant_choices(field, choices, selected) {
                return false;
            }
        }
        session.state = Some(state);
    }
    true
}

fn command_for_native(
    live: &LivePage,
    cursor_nodes: &[u32],
    state: &AssaultOptionsState,
    command: NativeCommand,
) -> Option<AssaultOptionsCommand> {
    if let NativeCommand::Step(direction) = command {
        let field = participant_field(state.focused()?)?;
        let forward = match direction {
            Step::Left => false,
            Step::Right => true,
            Step::Up | Step::Down => return None,
        };
        return state
            .next_participant_choice(field, forward)
            .map(AssaultOptionsCommand::SelectParticipantChoice);
    }
    if let NativeCommand::Adjust { row, right } = command {
        let field = participant_field(option_at(live, cursor_nodes, row as usize)?)?;
        return state
            .next_participant_choice(field, right)
            .map(AssaultOptionsCommand::SelectParticipantChoice);
    }
    let option = match command {
        NativeCommand::Focus(index) | NativeCommand::Select(index) => {
            option_at(live, cursor_nodes, index as usize)?
        }
        NativeCommand::Accept => state.focused()?,
        _ => return None,
    };
    let target = state.target(option);
    Some(match command {
        NativeCommand::Focus(_) => AssaultOptionsCommand::Focus(target),
        NativeCommand::Select(_) | NativeCommand::Accept
            if option.participant_field().is_some() =>
        {
            AssaultOptionsCommand::Focus(target)
        }
        NativeCommand::Select(_) | NativeCommand::Accept => AssaultOptionsCommand::Activate(target),
        _ => return None,
    })
}

fn dispatch_native_commands(
    live: &LivePage,
    cursor_nodes: &[u32],
    state: &mut AssaultOptionsState,
    commands: &[NativeCommand],
) -> Vec<AssaultOptionsUpdate> {
    commands
        .iter()
        .filter_map(|command| {
            let typed = command_for_native(live, cursor_nodes, state, *command)?;
            match state.dispatch(typed) {
                AssaultOptionsUpdate::Ignored => None,
                update => Some(update),
            }
        })
        .collect()
}

fn page_layout(live: &LivePage, cursor_nodes: &[u32], page: &Page) -> Option<AssaultOptionsLayout> {
    if !exact_page_rows(live, cursor_nodes) || live.rows.len() != page.widgets.len() {
        return None;
    }
    let widgets = page
        .widgets
        .iter()
        .zip(&live.rows)
        .map(|(widget, row)| AssaultOptionsWidgetLayout {
            action: match &row.text {
                LiveText::Id(label) => option_for_label(*label),
                LiveText::None | LiveText::Bytes(_) => None,
            },
            widget: widget.clone(),
        })
        .collect::<Vec<_>>();
    let actions = widgets
        .iter()
        .filter_map(|item| item.action)
        .collect::<Vec<_>>();
    if actions != ASSAULT_OPTIONS_ORDER {
        return None;
    }
    Some(AssaultOptionsLayout {
        widget_type: page.widget_type,
        panel_bounds: page.panel_bounds,
        nav: page.nav,
        wrap: page.wrap,
        widgets,
    })
}

/// Capture typed focus/action requests while the original callback remains the
/// temporary effect seam. Updates are available through [`take_updates`]; no
/// guest widget address or synthesized device event is stored in the model.
pub fn apply_commands(current: &crate::ui_pages::Current, commands: &[NativeCommand]) {
    if !ensure_state(&current.live, &current.cursor_nodes) {
        return;
    }
    let Ok(mut session) = SESSION.lock() else {
        return;
    };
    let Some(state) = session.state.as_mut() else {
        return;
    };
    let updates = dispatch_native_commands(&current.live, &current.cursor_nodes, state, commands);
    session.updates.extend(updates);
}

fn take_session_updates(session: &mut Session) -> Option<AssaultOptionsUpdateBatch> {
    if session.active_page.is_none() || session.updates.is_empty() {
        return None;
    }
    Some(AssaultOptionsUpdateBatch {
        generation: AssaultOptionsGeneration::new(session.generation),
        updates: std::mem::take(&mut session.updates),
    })
}

/// Drain Rust-owned page outcomes for the typed caller/handler seam. The batch
/// carries its generation and is discarded when that page exits.
pub fn take_updates() -> Option<AssaultOptionsUpdateBatch> {
    SESSION
        .lock()
        .ok()
        .and_then(|mut session| take_session_updates(&mut session))
}

/// Build the Assault Options renderer page from the typed Rust state. Localized
/// text, widget geometry and non-action metadata remain borrowed from the live
/// registry until their own native data sources are ported.
pub fn project_owned_page(
    live: &LivePage,
    slot: u32,
    cursor_nodes: &[u32],
    focused: Option<WidgetId>,
    page: Page,
) -> Option<Page> {
    let layout = page_layout(live, cursor_nodes, &page)?;
    if !crate::storage_live::mapmaker_ui_enabled() || !ensure_state(live, cursor_nodes) {
        return None;
    }
    let (generation, active_page) = SESSION
        .lock()
        .ok()
        .map(|session| (session.generation, session.active_page))?;
    if active_page != Some((BUILDER, slot)) || generation == 0 {
        return None;
    }

    let mut session = SESSION.lock().ok()?;
    if let Some(state) = session.state.as_mut()
        && let Some(option) = focused
            .and_then(|widget| live.cursor_index(widget.0 as usize, cursor_nodes))
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| option_at(live, cursor_nodes, index))
    {
        let _ = state.dispatch(AssaultOptionsCommand::Focus(state.target(option)));
    }
    let state = session.state.as_ref()?;
    let projection = build_assault_options_page(&state, page.id, layout).ok()?;
    let mut projected_page = projection.page;
    projected_page.highlight_edges = page.highlight_edges;
    projected_page.handlers = page.handlers;
    projected_page.links = page.links;
    projected_page.enter = page.enter;
    projected_page.exit = page.exit;
    Some(projected_page)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tsfp_mapmaker::assault_options::{AssaultParticipantChoiceId, AssaultParticipantField};
    use tsfp_ui_model::model::{TextId, TextRef, UiOption, Widget, WidgetKind};

    fn live_with_rows(labels: [u32; 4]) -> (LivePage, Vec<u32>) {
        let nodes: Vec<_> = (0..4).map(|index| 0x100 + index).collect();
        let rows = nodes
            .iter()
            .zip(labels)
            .map(|(node, label)| tsfp_ui_input::live_page::LiveRow {
                node: *node,
                node_type: 0,
                id: 0,
                flags: 0,
                text: LiveText::Id(label),
                font: 0,
                value_font: 0,
                align: 0,
                rect: None,
                clip: None,
                value: None,
                activate: 0,
                draw: 0,
                kind: 0,
                selected: 0,
                options: if matches!(label, 0x0AF3 | 0x0AF4) {
                    [10, 11]
                        .into_iter()
                        .map(|id| tsfp_ui_input::live_page::LiveOption {
                            id,
                            text: LiveText::Id(id),
                            enabled: true,
                        })
                        .collect()
                } else {
                    Vec::new()
                },
                brightness: 0.0,
                height: 0.0,
                text_align: 0,
                text_x: 0.0,
                text_y: 0.0,
            })
            .collect();
        (
            LivePage {
                slot: 2,
                builder: BUILDER,
                state: 0,
                cursor_node: nodes[0],
                rows,
                units: (1.0, 1.0),
            },
            nodes,
        )
    }

    #[test]
    fn renderer_layout_preserves_widgets_and_rejects_unmeasured_order() {
        let (live, nodes) = live_with_rows([0x0AF1, 0x0AF3, 0x0AF4, 0x0AF9]);
        let mut page = Page::new(tsfp_ui_model::model::PageId(0x313FE0), 1);
        page.widgets = [0x0AF1, 0x0AF3, 0x0AF4, 0x0AF9]
            .into_iter()
            .enumerate()
            .map(|(index, label)| {
                let kind = if matches!(label, 0x0AF3 | 0x0AF4) {
                    WidgetKind::Cycler
                } else {
                    WidgetKind::Button
                };
                let mut widget = Widget::new(WidgetId(index as u64 + 1), kind);
                if kind == WidgetKind::Cycler {
                    widget.options = [10, 11]
                        .into_iter()
                        .map(|id| UiOption {
                            id,
                            text: TextRef::Id(TextId(id)),
                            enabled: true,
                        })
                        .collect();
                }
                widget
            })
            .collect();
        let layout = page_layout(&live, &nodes, &page).expect("measured page order");
        assert_eq!(layout.widgets.len(), 4);
        assert_eq!(layout.widgets[1].action, Some(AssaultOption::Attackers));
        assert_eq!(layout.widgets[1].widget.id, WidgetId(2));

        let (reordered, reordered_nodes) = live_with_rows([0x0AF3, 0x0AF1, 0x0AF4, 0x0AF9]);
        assert!(page_layout(&reordered, &reordered_nodes, &page).is_none());
        let (unknown, unknown_nodes) = live_with_rows([0x0AF1, 0x0AF3, 0x0AF4, 0x0AF8]);
        assert!(page_layout(&unknown, &unknown_nodes, &page).is_none());
    }

    #[test]
    fn measured_assault_rows_are_exact_and_ordered() {
        let (live, nodes) = live_with_rows([0x0AF1, 0x0AF3, 0x0AF4, 0x0AF9]);
        assert!(exact_page_rows(&live, &nodes));
        let (reordered, reordered_nodes) = live_with_rows([0x0AF3, 0x0AF1, 0x0AF4, 0x0AF9]);
        assert!(!exact_page_rows(&reordered, &reordered_nodes));
        let (unknown, unknown_nodes) = live_with_rows([0x0AF1, 0x0AF3, 0x0AF4, 0x0AF8]);
        assert!(!exact_page_rows(&unknown, &unknown_nodes));
    }

    #[test]
    fn native_keyboard_and_pointer_commands_resolve_to_typed_actions() {
        let (live, nodes) = live_with_rows([0x0AF1, 0x0AF3, 0x0AF4, 0x0AF9]);
        let mut state = AssaultOptionsState::new(AssaultOptionsGeneration::new(4));
        for field in [
            AssaultParticipantField::Attackers,
            AssaultParticipantField::Defenders,
        ] {
            let (choices, selected) = runtime_choices(&live, field).expect("live choices");
            assert!(state.install_participant_choices(field, choices, selected));
        }
        assert_eq!(
            command_for_native(&live, &nodes, &state, NativeCommand::Focus(2)),
            Some(AssaultOptionsCommand::Focus(
                state.target(AssaultOption::Defenders)
            ))
        );
        assert_eq!(
            command_for_native(&live, &nodes, &state, NativeCommand::Select(3)),
            Some(AssaultOptionsCommand::Activate(
                state.target(AssaultOption::StartAssault)
            ))
        );
        assert_eq!(
            command_for_native(&live, &nodes, &state, NativeCommand::Select(8)),
            None
        );
        let focus = command_for_native(&live, &nodes, &state, NativeCommand::Focus(1)).unwrap();
        assert_eq!(
            state.dispatch(focus),
            AssaultOptionsUpdate::Focused(AssaultOption::Attackers)
        );
        let step =
            command_for_native(&live, &nodes, &state, NativeCommand::Step(Step::Right)).unwrap();
        assert_eq!(
            step,
            AssaultOptionsCommand::SelectParticipantChoice(
                state
                    .participant_choice_target(
                        AssaultParticipantField::Attackers,
                        AssaultParticipantChoiceId::new(1),
                    )
                    .unwrap()
            )
        );
    }

    #[test]
    fn dispatch_returns_ordered_typed_updates_for_page_caller() {
        let (live, nodes) = live_with_rows([0x0AF1, 0x0AF3, 0x0AF4, 0x0AF9]);
        let mut state = AssaultOptionsState::new(AssaultOptionsGeneration::new(5));
        for field in [
            AssaultParticipantField::Attackers,
            AssaultParticipantField::Defenders,
        ] {
            let (choices, selected) = runtime_choices(&live, field).expect("live choices");
            assert!(state.install_participant_choices(field, choices, selected));
        }

        let updates = dispatch_native_commands(
            &live,
            &nodes,
            &mut state,
            &[
                NativeCommand::Focus(1),
                NativeCommand::Step(Step::Right),
                NativeCommand::Select(3),
            ],
        );
        assert_eq!(
            updates,
            [
                AssaultOptionsUpdate::Focused(AssaultOption::Attackers),
                AssaultOptionsUpdate::ParticipantChoiceSelected {
                    field: AssaultParticipantField::Attackers,
                    choice: AssaultParticipantChoiceId::new(1),
                    runtime_option_id: AssaultParticipantRuntimeOptionId::new(11),
                },
                AssaultOptionsUpdate::ActionRequested(AssaultOption::StartAssault),
            ]
        );
        assert_eq!(
            state.selected_participant_choice(AssaultParticipantField::Attackers),
            Some(AssaultParticipantChoiceId::new(1))
        );
        assert_eq!(
            state.selected_runtime_option_id(AssaultParticipantField::Attackers),
            Some(AssaultParticipantRuntimeOptionId::new(11))
        );
    }

    #[test]
    fn update_batch_carries_generation_and_drains_once() {
        let mut session = Session {
            active_page: Some((BUILDER, 3)),
            generation: 8,
            state: None,
            updates: vec![AssaultOptionsUpdate::ActionRequested(
                AssaultOption::PreviewTeam,
            )],
        };
        assert_eq!(
            take_session_updates(&mut session),
            Some(AssaultOptionsUpdateBatch {
                generation: AssaultOptionsGeneration::new(8),
                updates: vec![AssaultOptionsUpdate::ActionRequested(
                    AssaultOption::PreviewTeam,
                )],
            })
        );
        assert_eq!(take_session_updates(&mut session), None);
    }
}
