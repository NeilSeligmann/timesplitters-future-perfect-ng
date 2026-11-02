//! Native input routing for the measured Map Maker Preview/tool menu.

use crate::{live_page::LivePage, native_page::NativeCommand, widgets::Step};
use tsfp_mapmaker::tool_menu::{
    ToolMenuChoice, ToolMenuGeneration, ToolMenuPageState, ToolMenuUpdate,
};

pub const BUILDER: u32 = 0x31_4420;
pub const ENABLED_ROW_HANDLER: u32 = 0x31_3FA0;
pub const HELP_HANDLER: u32 = 0x31_26D0;
pub const HELP_DRAWER: u32 = 0x31_2610;
pub const HELP_LABEL: u32 = 0x0AE2;

/// T1714's exact game-type/label table in display order. The help row is a
/// separate, non-game-type row and is checked after this sequence.
pub const TOOL_ROWS: [(u32, ToolMenuChoice); 15] = [
    (0x16F6, ToolMenuChoice::MapmakerPreview),
    (0x16C9, ToolMenuChoice::Story),
    (0x16C5, ToolMenuChoice::Deathmatch),
    (0x16CF, ToolMenuChoice::TeamDeathmatch),
    (0x16C3, ToolMenuChoice::CaptureTheBag),
    (0x16C1, ToolMenuChoice::BagTag),
    (0x16D1, ToolMenuChoice::Elimination),
    (0x16DF, ToolMenuChoice::Shrink),
    (0x16E1, ToolMenuChoice::Vampire),
    (0x16D8, ToolMenuChoice::Thief),
    (0x16D3, ToolMenuChoice::Virus),
    (0x16C7, ToolMenuChoice::Zones),
    (0x16CB, ToolMenuChoice::Assault),
    (0x16DB, ToolMenuChoice::Gladiator),
    (0x16E3, ToolMenuChoice::MonkeyAssistant),
];

/// Renderer projection for a caller-ordered tool menu. `None` preserves a
/// disabled or non-action row while keeping focus tied to Rust-owned state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderRow {
    choice: Option<ToolMenuChoice>,
    focused: bool,
}

impl RenderRow {
    pub const fn choice(self) -> Option<ToolMenuChoice> {
        self.choice
    }

    pub const fn enabled(self) -> bool {
        self.choice.is_some()
    }

    pub const fn focused(self) -> bool {
        self.focused
    }
}

/// Project the live row sequence without moving focus or changing page state.
pub fn render_rows(
    state: &ToolMenuPageState,
    actions: &[Option<ToolMenuChoice>],
) -> Vec<RenderRow> {
    actions
        .iter()
        .map(|choice| RenderRow {
            choice: *choice,
            focused: choice.is_some() && *choice == state.focused(),
        })
        .collect()
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use tsfp_mapmaker::tool_menu::ToolMenuGeneration;

    #[test]
    fn renderer_projection_preserves_disabled_slots_and_rust_focus() {
        let actions = [
            Some(ToolMenuChoice::Story),
            None,
            Some(ToolMenuChoice::Assault),
        ];
        let mut state = ToolMenuPageState::new(ToolMenuGeneration::new(9), &actions);
        state.focus(state.target(ToolMenuChoice::Assault).unwrap());
        let rows = render_rows(&state, &actions);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].choice(), Some(ToolMenuChoice::Story));
        assert!(!rows[0].focused());
        assert_eq!(rows[1].choice(), None);
        assert!(!rows[1].enabled());
        assert_eq!(rows[2].choice(), Some(ToolMenuChoice::Assault));
        assert!(rows[2].focused());
    }
}

/// Bind the enabled measured rows. A disabled known row is retained in display
/// order but receives no Rust action target; unknown or reordered rows fail
/// closed. Original values and callback fields stay on the live page.
pub fn exact_actions(live: &LivePage, cursor_nodes: &[u32]) -> Option<Vec<Option<ToolMenuChoice>>> {
    if live.builder != BUILDER || cursor_nodes.is_empty() || cursor_nodes.len() > 16 {
        return None;
    }
    let mut actions = Vec::with_capacity(cursor_nodes.len());
    let mut seen = [false; TOOL_ROWS.len()];
    let mut last_tool_index = None;
    let mut saw_help = false;
    let mut enabled_count = 0usize;
    for node in cursor_nodes {
        let row = live.rows.iter().find(|row| row.node == *node)?;
        let label = match &row.text {
            crate::live_page::LiveText::Id(label) => label,
            _ => return None,
        };
        if *label == HELP_LABEL {
            if saw_help || row.activate != HELP_HANDLER || row.draw != HELP_DRAWER {
                return None;
            }
            saw_help = true;
            actions.push(Some(ToolMenuChoice::Help));
            continue;
        }
        let index = TOOL_ROWS
            .iter()
            .position(|(candidate, _)| *candidate == *label)?;
        if seen[index] || last_tool_index.is_some_and(|last| index <= last) {
            return None;
        }
        seen[index] = true;
        last_tool_index = Some(index);
        let action = (row.activate == ENABLED_ROW_HANDLER).then_some(TOOL_ROWS[index].1);
        if action.is_some() {
            enabled_count += 1;
        }
        actions.push(action);
    }
    (enabled_count > 0).then_some(actions)
}

pub fn dispatch(
    state: &mut ToolMenuPageState,
    actions: &[Option<ToolMenuChoice>],
    focused_index: Option<usize>,
    command: NativeCommand,
    wrap: bool,
) -> Option<ToolMenuUpdate> {
    let action_at = |index: usize| actions.get(index).copied().flatten();
    match command {
        NativeCommand::Focus(index) => action_at(index as usize)
            .and_then(|choice| state.target(choice))
            .and_then(|target| state.focus(target)),
        NativeCommand::Select(index) => action_at(index as usize)
            .and_then(|choice| state.target(choice))
            .and_then(|target| state.activate(target)),
        NativeCommand::Accept => focused_index
            .and_then(action_at)
            .and_then(|choice| state.target(choice))
            .and_then(|target| state.activate(target)),
        NativeCommand::Back => Some(state.back()),
        NativeCommand::Step(Step::Up | Step::Down) => {
            if actions.is_empty() {
                return None;
            }
            let index = focused_index?;
            let next = next_enabled_index(
                actions,
                index,
                matches!(command, NativeCommand::Step(Step::Down)),
                wrap,
            )?;
            action_at(next)
                .and_then(|choice| state.target(choice))
                .and_then(|target| state.focus(target))
        }
        NativeCommand::Step(Step::Left | Step::Right)
        | NativeCommand::Adjust { .. }
        | NativeCommand::Scroll(_)
        | NativeCommand::Delete
        | NativeCommand::Shortcut
        | NativeCommand::Join(_)
        | NativeCommand::ResumeGame => None,
    }
}

fn next_enabled_index(
    actions: &[Option<ToolMenuChoice>],
    current: usize,
    forward: bool,
    wrap: bool,
) -> Option<usize> {
    if actions.is_empty() || current >= actions.len() {
        return None;
    }
    for distance in 1..=actions.len() {
        let candidate = if forward {
            current
                .checked_add(distance)
                .filter(|index| *index < actions.len())
                .or_else(|| wrap.then_some((current + distance) % actions.len()))?
        } else {
            current.checked_sub(distance).or_else(|| {
                wrap.then_some((current + actions.len() - distance % actions.len()) % actions.len())
            })?
        };
        if actions[candidate].is_some() {
            return Some(candidate);
        }
    }
    None
}

pub fn dispatch_commands(
    state: &mut ToolMenuPageState,
    actions: &[Option<ToolMenuChoice>],
    focused_index: Option<usize>,
    commands: &[NativeCommand],
    wrap: bool,
) -> Vec<ToolMenuUpdate> {
    let mut focus = focused_index;
    let mut updates = Vec::new();
    for command in commands {
        let next_focus = match command {
            NativeCommand::Step(Step::Up | Step::Down) => focus.and_then(|index| {
                next_enabled_index(
                    actions,
                    index,
                    matches!(command, NativeCommand::Step(Step::Down)),
                    wrap,
                )
            }),
            NativeCommand::Focus(index) | NativeCommand::Select(index) => Some(*index as usize),
            _ => None,
        };
        if let Some(update) = dispatch(state, actions, focus, *command, wrap) {
            if matches!(update, ToolMenuUpdate::Focused(_)) {
                focus = next_focus;
            } else if matches!(command, NativeCommand::Select(_)) {
                focus = next_focus;
            }
            updates.push(update);
        }
    }
    updates
}

/// Keep the generation attached while updates cross to the caller-owned
/// original-effect adapter.
pub fn qualify_updates(
    state: &ToolMenuPageState,
    updates: impl IntoIterator<Item = ToolMenuUpdate>,
) -> Vec<tsfp_mapmaker::tool_menu::ToolMenuEffect> {
    updates
        .into_iter()
        .filter_map(|update| {
            let effect = state.effect(update);
            state.accepts_effect(effect).then_some(effect)
        })
        .collect()
}

#[cfg(test)]
mod qualification_tests {
    use super::*;

    #[test]
    fn effect_requests_keep_the_page_generation() {
        let choices = [Some(ToolMenuChoice::Story)];
        let mut state = ToolMenuPageState::new(ToolMenuGeneration::new(17), &choices);
        let updates = dispatch_commands(
            &mut state,
            &choices,
            Some(0),
            &[NativeCommand::Accept],
            false,
        );
        let effects = qualify_updates(&state, updates);
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].generation.get(), 17);
        assert_eq!(
            effects[0].update,
            ToolMenuUpdate::ActionRequested(ToolMenuChoice::Story)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::live_page::{LiveRow, LiveText};
    use tsfp_mapmaker::tool_menu::{ToolMenuGeneration, ToolMenuUpdate};

    fn row(node: u32, label: u32, activate: u32, draw: u32) -> LiveRow {
        LiveRow {
            node,
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
            activate,
            draw,
            kind: 0,
            selected: 0,
            options: Vec::new(),
            brightness: 0.0,
            height: 0.0,
            text_align: 0,
            text_x: 0.0,
            text_y: 0.0,
        }
    }

    fn live(rows: Vec<LiveRow>) -> (LivePage, Vec<u32>) {
        let nodes = rows.iter().map(|row| row.node).collect();
        (
            LivePage {
                slot: 5,
                builder: BUILDER,
                state: 2,
                cursor_node: 0,
                rows,
                units: (6.4, 4.8),
            },
            nodes,
        )
    }

    #[test]
    fn measured_enabled_rows_and_help_row_bind_only_typed_choices() {
        let (page, nodes) = live(vec![
            row(1, 0x16F6, ENABLED_ROW_HANDLER, 0),
            row(2, 0x16C9, 0, 0),
            row(3, 0x16C5, ENABLED_ROW_HANDLER, 0),
            row(4, HELP_LABEL, HELP_HANDLER, HELP_DRAWER),
        ]);
        let actions = exact_actions(&page, &nodes).unwrap();
        assert_eq!(
            actions,
            [
                Some(ToolMenuChoice::MapmakerPreview),
                None,
                Some(ToolMenuChoice::Deathmatch),
                Some(ToolMenuChoice::Help),
            ]
        );
        let mut state = ToolMenuPageState::new(ToolMenuGeneration::new(3), &actions);
        assert_eq!(
            dispatch_commands(
                &mut state,
                &actions,
                Some(0),
                &[NativeCommand::Step(Step::Down), NativeCommand::Accept,],
                false,
            ),
            [
                ToolMenuUpdate::Focused(ToolMenuChoice::Deathmatch),
                ToolMenuUpdate::ActionRequested(ToolMenuChoice::Deathmatch)
            ]
        );
    }

    #[test]
    fn navigation_skips_disabled_rows_and_wraps_to_an_enabled_row() {
        let actions = [
            Some(ToolMenuChoice::Story),
            None,
            Some(ToolMenuChoice::Deathmatch),
            None,
        ];
        let mut state = ToolMenuPageState::new(ToolMenuGeneration::new(5), &actions);
        assert_eq!(
            dispatch_commands(
                &mut state,
                &actions,
                Some(0),
                &[
                    NativeCommand::Step(Step::Down),
                    NativeCommand::Step(Step::Down),
                    NativeCommand::Accept,
                ],
                true,
            ),
            [
                ToolMenuUpdate::Focused(ToolMenuChoice::Deathmatch),
                ToolMenuUpdate::Focused(ToolMenuChoice::Story),
                ToolMenuUpdate::ActionRequested(ToolMenuChoice::Story),
            ]
        );
    }

    #[test]
    fn unknown_reordered_or_miswired_rows_fail_closed() {
        let (reordered, nodes) = live(vec![
            row(1, TOOL_ROWS[1].0, ENABLED_ROW_HANDLER, 0),
            row(2, TOOL_ROWS[0].0, ENABLED_ROW_HANDLER, 0),
        ]);
        assert_eq!(exact_actions(&reordered, &nodes), None);

        let (unknown, nodes) = live(vec![row(1, 0xFFFF, ENABLED_ROW_HANDLER, 0)]);
        assert_eq!(exact_actions(&unknown, &nodes), None);

        let (bad_help, nodes) = live(vec![
            row(1, TOOL_ROWS[0].0, ENABLED_ROW_HANDLER, 0),
            row(2, HELP_LABEL, HELP_HANDLER, 0),
        ]);
        assert_eq!(exact_actions(&bad_help, &nodes), None);
    }

    #[test]
    fn native_commands_cover_wrapped_keyboard_back_and_pointer_selection() {
        let actions = [
            Some(ToolMenuChoice::Story),
            Some(ToolMenuChoice::Deathmatch),
        ];
        let mut state = ToolMenuPageState::new(ToolMenuGeneration::new(4), &actions);
        assert_eq!(
            dispatch_commands(
                &mut state,
                &actions,
                Some(1),
                &[
                    NativeCommand::Step(Step::Down),
                    NativeCommand::Accept,
                    NativeCommand::Back,
                    NativeCommand::Select(1),
                ],
                true,
            ),
            [
                ToolMenuUpdate::Focused(ToolMenuChoice::Story),
                ToolMenuUpdate::ActionRequested(ToolMenuChoice::Story),
                ToolMenuUpdate::BackRequested,
                ToolMenuUpdate::ActionRequested(ToolMenuChoice::Deathmatch),
            ]
        );
    }
}
