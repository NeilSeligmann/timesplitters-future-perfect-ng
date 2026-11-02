// SPDX-License-Identifier: GPL-3.0-or-later
//! T2408: attach the loaded editor menu's live input to Rust-owned semantic
//! actions. Original handlers remain the typed effect boundary until the full
//! Map Maker subsystem passes the Rule 24 replay gate.

use std::sync::Mutex;

use tsfp_mapmaker::ui::{
    EDITOR_CHILD_SURFACES, EditorConfirmation, EditorConfirmationChoice, EditorMenuAction,
    EditorMenuGeneration, EditorSurfaceGeneration, MapMakerUiCommand, ORIGINAL_EDITOR_MENU_ACTIONS,
};
use tsfp_ui_input::live_page::{LivePage, LiveText};
use tsfp_ui_input::mapmaker_editor::{
    EditorMenuPointerEvent, dispatch_editor_menu_frame, dispatch_editor_menu_pointer,
};
use tsfp_ui_input::native_page::NativeCommand;
use tsfp_ui_input::{IntentSet, MenuIntent};
use tsfp_ui_model::mapmaker_editor_menu::{
    EditorMenuLayout, EditorMenuWidgetLayout, build_editor_menu_page,
};
use tsfp_ui_model::model::{Page, WidgetId};

const EDITOR_MENU_BUILDER: u32 = 0x31_3010;
const MAPMAKER_CONFIRM_BUILDER: u32 = 0x7_A080;
const DIRECT_CLEAR_MAP_YES_HANDLER: u32 = 0x31_1AA0;

#[derive(Default)]
struct Session {
    active_page: Option<(u32, u32)>,
    generation: u64,
    repeat_at: [u64; 4],
}

static SESSION: Mutex<Session> = Mutex::new(Session {
    active_page: None,
    generation: 0,
    repeat_at: [0; 4],
});

/// Track entry/exit so reopening the same page slot invalidates old typed IDs.
pub fn observe_top(top: Option<(u32, u32)>) {
    let page = top.filter(|(builder, _)| *builder == EDITOR_MENU_BUILDER);
    let Ok(mut session) = SESSION.lock() else {
        return;
    };
    if session.active_page != page {
        session.active_page = page;
        if page.is_some() {
            session.generation = session.generation.wrapping_add(1).max(1);
            session.repeat_at = [0; 4];
        }
    }
}

fn action_for_label(id: u32) -> Option<EditorMenuAction> {
    use EditorMenuAction as A;
    Some(match id {
        0x0AD6 => A::PreviewMap,
        0x0AD7 => A::Undo,
        0x0AD8 => A::Redo,
        0x0AD5 => A::MapSettings,
        0x0ADB => A::EditorSettings,
        0x0AD9 => A::CenterMap,
        0x0ADA => A::ClearMap,
        0x0ADC => A::LoadMap,
        0x0ADD => A::SaveMap,
        0x17A4 => A::Controls,
        0x0ADF => A::QuitMapMaker,
        _ => return None,
    })
}

fn live_action_order(live: &LivePage, cursor_nodes: &[u32]) -> Option<Vec<EditorMenuAction>> {
    cursor_nodes
        .iter()
        .map(|node| {
            let row = live.rows.iter().find(|row| row.node == *node)?;
            let LiveText::Id(label) = &row.text else {
                return None;
            };
            // The live cursor list includes Edit/Resume (row 0); it is not
            // one of the owned editor actions. Keep its original row index.
            Some((*label != 0x0B07).then(|| action_for_label(*label)))
        })
        .collect::<Option<Vec<_>>>()
        .and_then(|rows| rows.into_iter().flatten().collect::<Option<Vec<_>>>())
        .filter(|actions| actions == ORIGINAL_EDITOR_MENU_ACTIONS)
}

fn action_at(live: &LivePage, cursor_nodes: &[u32], index: u32) -> Option<EditorMenuAction> {
    let node = *cursor_nodes.get(index as usize)?;
    let row = live.rows.iter().find(|row| row.node == node)?;
    let LiveText::Id(label) = &row.text else {
        return None;
    };
    action_for_label(*label)
}

fn ensure_owned_menu_state(
    actions: Vec<EditorMenuAction>,
    generation: EditorMenuGeneration,
    focused: Option<EditorMenuAction>,
) -> bool {
    let Ok(mut state) = crate::storage_live::mapmaker_ui().lock() else {
        return false;
    };
    if state.editor_menu_generation() != Some(generation) {
        state.dispatch(MapMakerUiCommand::BeginEditor);
        if state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(generation.get()),
                EDITOR_CHILD_SURFACES.to_vec(),
            )
            .is_err()
        {
            return false;
        }
        if state
            .install_editor_menu_catalog(generation, actions)
            .is_err()
        {
            return false;
        }
        if let Some(action) = focused
            && let Some(target) = state.editor_menu_target(action)
        {
            dispatch_editor_menu_pointer(&mut state, EditorMenuPointerEvent::Hover(Some(target)));
        }
    }
    true
}

fn ensure_owned_menu(current: &crate::ui_pages::Current, generation: EditorMenuGeneration) -> bool {
    let Some(actions) = live_action_order(&current.live, &current.cursor_nodes) else {
        return false;
    };
    if !crate::storage_live::mapmaker_ui_enabled() {
        return false;
    }
    let focused = current
        .focused
        .and_then(|widget| {
            current
                .live
                .cursor_index(widget.0 as usize, &current.cursor_nodes)
        })
        .and_then(|index| action_at(&current.live, &current.cursor_nodes, index));
    ensure_owned_menu_state(actions, generation, focused)
}

/// Recognize only the direct Clear Map modal: two cursor rows, measured Yes
/// handler, and No's measured zero-handler contract. Mode-change confirmations
/// have a different callback pair and remain owned by Editor Settings.
fn clear_map_confirmation_widgets(
    live: &LivePage,
    cursor_nodes: &[u32],
) -> Option<(WidgetId, WidgetId)> {
    if live.state != 2 || cursor_nodes.len() != 2 {
        return None;
    }
    let mut cancel = None;
    let mut accept = None;
    for node in cursor_nodes {
        let row = live.rows.iter().find(|row| row.node == *node)?;
        let index = live.rows.iter().position(|row| row.node == *node)?;
        match row.activate {
            0 if cancel.is_none() => {
                cancel = Some(tsfp_ui_input::live_page::widget_id(index));
            }
            DIRECT_CLEAR_MAP_YES_HANDLER if accept.is_none() => {
                accept = Some(tsfp_ui_input::live_page::widget_id(index));
            }
            _ => return None,
        }
    }
    Some((cancel?, accept?))
}

/// 0x7A080 stores the caller's callbacks on the page, not its rows. Match the
/// dirty-Quit prompt key and generic Yes/No wrappers; the title may also be in
/// the cursor list. Both opening (2) and input-ready (3) snapshots are valid.
fn quit_map_confirmation_widgets(
    live: &LivePage,
    cursor_nodes: &[u32],
) -> Option<(WidgetId, WidgetId)> {
    if !matches!(live.state, 2 | 3) || !live.rows.iter().any(|row| row.text == LiveText::Id(0x0AFA))
    {
        return None;
    }
    let mut cancel = None;
    let mut accept = None;
    for node in cursor_nodes {
        let row = live.rows.iter().find(|row| row.node == *node)?;
        let index = live.rows.iter().position(|row| row.node == *node)?;
        let choice = match row.activate {
            0x7_9730 => &mut accept,
            0x7_9790 => &mut cancel,
            0 if row.kind == 0 => continue,
            _ => return None,
        };
        if choice
            .replace(tsfp_ui_input::live_page::widget_id(index))
            .is_some()
        {
            return None;
        }
    }
    Some((cancel?, accept?))
}

fn clear_map_confirmation_page(
    mut page: Page,
    cancel_widget: WidgetId,
    accept_widget: WidgetId,
    focused: Option<EditorConfirmationChoice>,
) -> Page {
    page.initial_focus = Some(match focused {
        Some(EditorConfirmationChoice::Accept) => accept_widget,
        Some(EditorConfirmationChoice::Cancel) | None => cancel_widget,
    });
    page
}

fn clear_map_confirmation_choice(
    widget: WidgetId,
    cancel_widget: WidgetId,
    accept_widget: WidgetId,
) -> Option<EditorConfirmationChoice> {
    if widget == cancel_widget {
        Some(EditorConfirmationChoice::Cancel)
    } else if widget == accept_widget {
        Some(EditorConfirmationChoice::Accept)
    } else {
        None
    }
}

/// Build the editor menu's renderer model from its Rust-owned action state and
/// the live page's localized text/layout. Unknown or reordered live rows fail
/// closed, leaving the existing page model untouched.
pub fn project_owned_editor_menu_page(
    live: &LivePage,
    slot: u32,
    cursor_nodes: &[u32],
    focused: Option<WidgetId>,
    page: Page,
) -> Option<Page> {
    if live.builder != EDITOR_MENU_BUILDER || !crate::storage_live::mapmaker_ui_enabled() {
        return None;
    }
    let actions = live_action_order(live, cursor_nodes)?;
    let layout = editor_menu_layout(live, cursor_nodes, &page)?;
    let (generation, active_page) = SESSION
        .lock()
        .ok()
        .map(|session| (session.generation, session.active_page))?;
    if active_page != Some((EDITOR_MENU_BUILDER, slot)) || generation == 0 {
        return None;
    }
    let generation = EditorMenuGeneration::new(generation);
    let focused_action = focused
        .and_then(|widget| live.cursor_index(widget.0 as usize, cursor_nodes))
        .and_then(|index| action_at(live, cursor_nodes, index));
    if !ensure_owned_menu_state(actions, generation, focused_action) {
        return None;
    }
    let history_availability = crate::storage_live::mapmaker_editor_history_availability();
    let mut state = crate::storage_live::mapmaker_ui().lock().ok()?;
    state.set_editor_history_availability(history_availability);
    let projection = build_editor_menu_page(&state.snapshot(), page.id, layout).ok()?;
    let mut projected_page = projection.page;
    // Keep all framework metadata not owned by this page projection.
    projected_page.highlight_edges = page.highlight_edges;
    projected_page.handlers = page.handlers;
    projected_page.links = page.links;
    projected_page.enter = page.enter;
    projected_page.exit = page.exit;
    Some(projected_page)
}

/// Keep the generic prompt's runtime text and geometry while projecting the
/// Rust-owned Clear Map choice and safe initial focus.
pub fn project_owned_clear_map_confirmation_page(
    live: &LivePage,
    cursor_nodes: &[u32],
    page: Page,
) -> Option<Page> {
    if !crate::storage_live::mapmaker_ui_enabled() {
        return None;
    }
    let (cancel_widget, accept_widget) = clear_map_confirmation_widgets(live, cursor_nodes)?;
    let state = crate::storage_live::mapmaker_ui().lock().ok()?;
    if state.editor_confirmation() != Some(EditorConfirmation::ClearMap)
        || state
            .editor_confirmation_target(EditorConfirmationChoice::Cancel)
            .is_none()
        || state
            .editor_confirmation_target(EditorConfirmationChoice::Accept)
            .is_none()
    {
        return None;
    }
    Some(clear_map_confirmation_page(
        page,
        cancel_widget,
        accept_widget,
        state.focused_editor_confirmation_choice(),
    ))
}

/// Project the dirty Quit Map Maker prompt from Rust-owned confirmation focus.
/// Its prompt string and row labels/layout remain runtime supplied.
pub fn project_owned_quit_map_confirmation_page(
    live: &LivePage,
    cursor_nodes: &[u32],
    page: Page,
) -> Option<Page> {
    if !crate::storage_live::mapmaker_ui_enabled() {
        return None;
    }
    let (cancel_widget, accept_widget) = quit_map_confirmation_widgets(live, cursor_nodes)?;
    let mut state = crate::storage_live::mapmaker_ui().lock().ok()?;
    match state.editor_confirmation() {
        Some(EditorConfirmation::QuitMapMaker) => {}
        Some(_) => return None,
        None if state.focused_editor_menu_action() == Some(EditorMenuAction::QuitMapMaker) => {
            state.dispatch(MapMakerUiCommand::ObserveQuitMapMakerConfirmation);
        }
        None => return None,
    }
    if state
        .editor_confirmation_target(EditorConfirmationChoice::Cancel)
        .is_none()
    {
        return None;
    }
    Some(clear_map_confirmation_page(
        page,
        cancel_widget,
        accept_widget,
        state.focused_editor_confirmation_choice(),
    ))
}

fn editor_menu_layout(
    live: &LivePage,
    cursor_nodes: &[u32],
    page: &Page,
) -> Option<EditorMenuLayout> {
    if live.rows.len() != page.widgets.len() {
        return None;
    }
    let widgets: Vec<_> = page
        .widgets
        .iter()
        .zip(&live.rows)
        .map(|(widget, row)| {
            let action = match &row.text {
                LiveText::Id(label) => action_for_label(*label),
                LiveText::None | LiveText::Bytes(_) => None,
            };
            EditorMenuWidgetLayout {
                action,
                widget: widget.clone(),
            }
        })
        .collect();
    let projected_actions: Vec<_> = widgets.iter().filter_map(|item| item.action).collect();
    let cursor_actions: Vec<_> = live_action_order(live, cursor_nodes)?;
    if projected_actions != cursor_actions {
        return None;
    }
    Some(EditorMenuLayout {
        widget_type: page.widget_type,
        panel_bounds: page.panel_bounds,
        nav: page.nav,
        wrap: page.wrap,
        widgets,
    })
}

/// Update the Rust-owned menu state from the same typed commands that will be
/// sent through the existing caller seam. No guest page fields are written.
pub fn apply_commands(current: &crate::ui_pages::Current, commands: &[NativeCommand]) {
    if current.descriptor.builder == MAPMAKER_CONFIRM_BUILDER {
        let confirmation = crate::storage_live::mapmaker_ui()
            .lock()
            .ok()
            .and_then(|state| state.editor_confirmation());
        match confirmation {
            Some(EditorConfirmation::ClearMap) => {
                apply_clear_map_confirmation_commands(current, commands)
            }
            Some(EditorConfirmation::QuitMapMaker) => {
                apply_quit_map_confirmation_commands(current, commands)
            }
            None => {}
        }
        return;
    }
    if current.descriptor.builder != EDITOR_MENU_BUILDER
        || !crate::storage_live::mapmaker_ui_enabled()
    {
        return;
    }
    let (generation, active_page) = match SESSION.lock() {
        Ok(session) => (session.generation, session.active_page),
        Err(_) => return,
    };
    if active_page != Some((EDITOR_MENU_BUILDER, current.slot)) || generation == 0 {
        return;
    }
    let generation = EditorMenuGeneration::new(generation);
    if !ensure_owned_menu(current, generation) {
        return;
    }

    let Ok(mut state) = crate::storage_live::mapmaker_ui().lock() else {
        return;
    };
    let mut session = match SESSION.lock() {
        Ok(session) => session,
        Err(_) => return,
    };
    for command in commands {
        match *command {
            NativeCommand::Focus(index) => {
                if let Some(action) = action_at(&current.live, &current.cursor_nodes, index)
                    && let Some(target) = state.editor_menu_target(action)
                {
                    dispatch_editor_menu_pointer(
                        &mut state,
                        EditorMenuPointerEvent::Hover(Some(target)),
                    );
                }
            }
            NativeCommand::Select(index) => {
                if let Some(action) = action_at(&current.live, &current.cursor_nodes, index)
                    && let Some(target) = state.editor_menu_target(action)
                {
                    dispatch_editor_menu_pointer(
                        &mut state,
                        EditorMenuPointerEvent::Activate(Some(target)),
                    );
                }
            }
            NativeCommand::Accept => {
                if let Some(action) = state.focused_editor_menu_action()
                    && let Some(target) = state.editor_menu_target(action)
                {
                    dispatch_editor_menu_pointer(
                        &mut state,
                        EditorMenuPointerEvent::Activate(Some(target)),
                    );
                }
            }
            NativeCommand::Step(step) => {
                let intent = match step {
                    tsfp_ui_input::widgets::Step::Up => Some(MenuIntent::Up),
                    tsfp_ui_input::widgets::Step::Down => Some(MenuIntent::Down),
                    tsfp_ui_input::widgets::Step::Left | tsfp_ui_input::widgets::Step::Right => {
                        None
                    }
                };
                if let Some(intent) = intent {
                    dispatch_editor_menu_frame(
                        &mut state,
                        IntentSet::default().with(intent),
                        IntentSet::default(),
                        0,
                        &mut session.repeat_at,
                    );
                }
            }
            NativeCommand::Back => {
                state.dispatch(MapMakerUiCommand::Back);
            }
            // Adjust, Scroll, ResumeGame and the page buttons (Delete, Shortcut, Join) do nothing here.
            _ => {}
        }
    }
}

fn apply_clear_map_confirmation_commands(
    current: &crate::ui_pages::Current,
    commands: &[NativeCommand],
) {
    apply_confirmation_commands(
        current,
        commands,
        EditorConfirmation::ClearMap,
        clear_map_confirmation_widgets,
    );
}

fn apply_quit_map_confirmation_commands(
    current: &crate::ui_pages::Current,
    commands: &[NativeCommand],
) {
    apply_confirmation_commands(
        current,
        commands,
        EditorConfirmation::QuitMapMaker,
        quit_map_confirmation_widgets,
    );
}

fn apply_confirmation_commands(
    current: &crate::ui_pages::Current,
    commands: &[NativeCommand],
    confirmation: EditorConfirmation,
    identify_widgets: fn(&LivePage, &[u32]) -> Option<(WidgetId, WidgetId)>,
) {
    if !crate::storage_live::mapmaker_ui_enabled() {
        return;
    }
    let Some((cancel_widget, accept_widget)) =
        identify_widgets(&current.live, &current.cursor_nodes)
    else {
        return;
    };
    let Ok(mut state) = crate::storage_live::mapmaker_ui().lock() else {
        return;
    };
    if state.editor_confirmation() != Some(confirmation)
        || state
            .editor_confirmation_target(EditorConfirmationChoice::Cancel)
            .is_none()
    {
        return;
    }
    let mut repeat_at = [0; 4];
    for command in commands {
        let pointer_choice = match *command {
            NativeCommand::Focus(index) | NativeCommand::Select(index) => {
                let widget = current
                    .cursor_nodes
                    .get(index as usize)
                    .and_then(|node| current.live.rows.iter().position(|row| row.node == *node))
                    .map(tsfp_ui_input::live_page::widget_id);
                widget.and_then(|widget| {
                    clear_map_confirmation_choice(widget, cancel_widget, accept_widget)
                })
            }
            NativeCommand::Accept => current
                .focused
                .and_then(|widget| {
                    clear_map_confirmation_choice(widget, cancel_widget, accept_widget)
                })
                .or_else(|| state.focused_editor_confirmation_choice())
                .or(Some(EditorConfirmationChoice::Cancel)),
            _ => None,
        };
        match *command {
            NativeCommand::Focus(_) => {
                if let Some(choice) = pointer_choice
                    && let Some(target) = state.editor_confirmation_target(choice)
                {
                    tsfp_ui_input::mapmaker_editor::dispatch_editor_confirmation_pointer(
                        &mut state,
                        tsfp_ui_input::mapmaker_editor::EditorConfirmationPointerEvent::Hover(
                            Some(target),
                        ),
                    );
                }
            }
            NativeCommand::Select(_) | NativeCommand::Accept => {
                if let Some(choice) = pointer_choice
                    && let Some(target) = state.editor_confirmation_target(choice)
                {
                    tsfp_ui_input::mapmaker_editor::dispatch_editor_confirmation_pointer(
                        &mut state,
                        tsfp_ui_input::mapmaker_editor::EditorConfirmationPointerEvent::Activate(
                            Some(target),
                        ),
                    );
                }
            }
            NativeCommand::Back => {
                tsfp_ui_input::mapmaker_editor::dispatch_editor_confirmation_pointer(
                    &mut state,
                    tsfp_ui_input::mapmaker_editor::EditorConfirmationPointerEvent::Back,
                );
            }
            NativeCommand::Step(step) => {
                let intent = match step {
                    tsfp_ui_input::widgets::Step::Up => Some(tsfp_ui_input::MenuIntent::Up),
                    tsfp_ui_input::widgets::Step::Down => Some(tsfp_ui_input::MenuIntent::Down),
                    tsfp_ui_input::widgets::Step::Left | tsfp_ui_input::widgets::Step::Right => {
                        None
                    }
                };
                if let Some(intent) = intent {
                    tsfp_ui_input::mapmaker_editor::dispatch_editor_confirmation_frame(
                        &mut state,
                        tsfp_ui_input::IntentSet::default().with(intent),
                        tsfp_ui_input::IntentSet::default(),
                        0,
                        &mut repeat_at,
                    );
                }
            }
            // Adjust, Scroll, ResumeGame and the page buttons (Delete, Shortcut, Join) do nothing here.
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        action_for_label, clear_map_confirmation_choice, clear_map_confirmation_page,
        clear_map_confirmation_widgets, editor_menu_layout, quit_map_confirmation_widgets,
    };
    use tsfp_mapmaker::ui::{
        EditorConfirmationChoice, EditorMenuAction as A, ORIGINAL_EDITOR_MENU_ACTIONS,
    };
    use tsfp_ui_input::live_page::{LivePage, LiveRow, LiveText};
    use tsfp_ui_model::model::{Page, PageId, TextId, TextRef, Widget, WidgetId, WidgetKind};

    #[test]
    fn measured_t1714_labels_map_to_the_original_typed_action_order() {
        let labels = [
            0x0AD6, 0x0AD7, 0x0AD8, 0x0AD5, 0x0ADB, 0x0AD9, 0x0ADA, 0x0ADC, 0x0ADD, 0x17A4, 0x0ADF,
        ];
        let actions: Vec<_> = labels
            .into_iter()
            .map(action_for_label)
            .collect::<Option<_>>()
            .unwrap();
        assert_eq!(actions, ORIGINAL_EDITOR_MENU_ACTIONS);
        assert_eq!(action_for_label(0x0B07), None); // title/header, not an action
        assert_eq!(action_for_label(0xFFFF), None);
        assert_eq!(actions[9], A::Controls);
    }

    #[test]
    fn t1714_menu_projection_binds_only_the_measured_action_rows() {
        let labels = [
            0x0B07, 0x0AD6, 0x0AD7, 0x0AD8, 0x0AD5, 0x0ADB, 0x0AD9, 0x0ADA, 0x0ADC, 0x0ADD, 0x17A4,
            0x0ADF,
        ];
        let live = LivePage {
            slot: 3,
            builder: 0x31_3010,
            state: 2,
            cursor_node: 0,
            rows: labels
                .iter()
                .enumerate()
                .map(|(index, label)| LiveRow {
                    node: 100 + index as u32,
                    node_type: 7,
                    id: index as i32,
                    flags: 0,
                    text: LiveText::Id(*label),
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
                    options: Vec::new(),
                    brightness: 0.0,
                    height: 0.0,
                    text_align: 0,
                    text_x: 0.0,
                    text_y: 0.0,
                })
                .collect(),
            units: (1.0, 1.0),
        };
        // The original row 0 resumes editing and remains in the cursor list.
        let cursor_nodes: Vec<_> = live.rows.iter().map(|row| row.node).collect();
        let mut page = Page::new(PageId(0x52D464), 1);
        for (index, label) in labels.iter().enumerate() {
            let mut widget = Widget::new(WidgetId(index as u64), WidgetKind::Button);
            widget.text = TextRef::Id(TextId(*label));
            page.widgets.push(widget);
        }
        let layout = editor_menu_layout(&live, &cursor_nodes, &page).unwrap();
        assert_eq!(super::action_at(&live, &cursor_nodes, 0), None);
        assert_eq!(
            super::action_at(&live, &cursor_nodes, 11),
            Some(A::QuitMapMaker)
        );
        let mut unknown = live.clone();
        unknown.rows[0].text = LiveText::Id(0xFFFF);
        assert!(super::live_action_order(&unknown, &cursor_nodes).is_none());
        let mut reordered = cursor_nodes.clone();
        reordered.swap(1, 2);
        assert!(super::live_action_order(&live, &reordered).is_none());
        assert_eq!(layout.widgets.len(), labels.len());
        assert_eq!(layout.widgets[0].action, None); // Resume uses the existing framework action.
        assert_eq!(
            layout
                .widgets
                .iter()
                .filter_map(|item| item.action)
                .collect::<Vec<_>>(),
            ORIGINAL_EDITOR_MENU_ACTIONS
        );
    }

    fn confirmation_row(node: u32, activate: u32) -> LiveRow {
        LiveRow {
            node,
            node_type: 7,
            id: node as i32,
            flags: 0,
            text: LiveText::Id(0),
            font: 0,
            value_font: 0,
            align: 0,
            rect: None,
            clip: None,
            value: None,
            activate,
            draw: 0,
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

    #[test]
    fn direct_clear_map_rows_bind_yes_and_zero_no_without_assuming_order() {
        let live = LivePage {
            slot: 1,
            builder: 0,
            state: 2,
            cursor_node: 20,
            rows: vec![confirmation_row(10, 0x31_1AA0), confirmation_row(20, 0)],
            units: (1.0, 1.0),
        };
        assert_eq!(
            clear_map_confirmation_widgets(&live, &[20, 10]),
            Some((WidgetId(1), WidgetId(0)))
        );
        let reversed = LivePage {
            rows: vec![confirmation_row(10, 0), confirmation_row(20, 0x31_1AA0)],
            ..live.clone()
        };
        assert_eq!(
            clear_map_confirmation_widgets(&reversed, &[10, 20]),
            Some((WidgetId(0), WidgetId(1)))
        );

        let mode_change_pair = LivePage {
            rows: vec![
                confirmation_row(10, 0x31_2120),
                confirmation_row(20, 0x36_1710),
            ],
            ..live.clone()
        };
        assert_eq!(
            clear_map_confirmation_widgets(&mode_change_pair, &[10, 20]),
            None
        );
        let no_accept = LivePage {
            rows: vec![confirmation_row(10, 0), confirmation_row(20, 0)],
            ..live.clone()
        };
        assert_eq!(clear_map_confirmation_widgets(&no_accept, &[10, 20]), None);
        assert_eq!(clear_map_confirmation_widgets(&live, &[10]), None);
    }

    #[test]
    fn dirty_quit_confirmation_uses_wrappers_and_preserves_live_indices() {
        let mut title = confirmation_row(5, 0);
        title.text = LiveText::Id(0x0AFA);
        let live = LivePage {
            slot: 4,
            builder: 0,
            state: 2,
            cursor_node: 20,
            rows: vec![
                title.clone(),
                confirmation_row(10, 0x7_9730),
                confirmation_row(20, 0x7_9790),
            ],
            units: (1.0, 1.0),
        };
        assert_eq!(
            quit_map_confirmation_widgets(&live, &[5, 10, 20]),
            Some((WidgetId(2), WidgetId(1)))
        );
        let reordered = LivePage {
            rows: vec![
                title,
                confirmation_row(10, 0x7_9790),
                confirmation_row(20, 0x7_9730),
            ],
            state: 3,
            ..live.clone()
        };
        assert_eq!(
            quit_map_confirmation_widgets(&reordered, &[20, 10]),
            Some((WidgetId(1), WidgetId(2)))
        );
        let no_yes = LivePage {
            rows: vec![confirmation_row(10, 0), confirmation_row(20, 0x7_9790)],
            ..live.clone()
        };
        assert_eq!(quit_map_confirmation_widgets(&no_yes, &[10, 20]), None);
        assert_eq!(quit_map_confirmation_widgets(&live, &[10]), None);
        let mut wrong_prompt = live.clone();
        wrong_prompt.rows[0].text = LiveText::Id(0x0AE1);
        assert_eq!(
            quit_map_confirmation_widgets(&wrong_prompt, &[5, 10, 20]),
            None
        );
        let mut duplicate = live.clone();
        duplicate.rows.push(confirmation_row(30, 0x7_9730));
        assert_eq!(
            quit_map_confirmation_widgets(&duplicate, &[5, 10, 20, 30]),
            None
        );
        let mut closed = live;
        closed.state = 4;
        assert_eq!(quit_map_confirmation_widgets(&closed, &[5, 10, 20]), None);
    }

    #[test]
    fn clear_map_confirmation_page_uses_owned_cancel_or_accept_focus() {
        let page = Page::new(PageId(1), 2);
        assert_eq!(
            clear_map_confirmation_page(page.clone(), WidgetId(3), WidgetId(4), None).initial_focus,
            Some(WidgetId(3))
        );
        assert_eq!(
            clear_map_confirmation_page(
                page.clone(),
                WidgetId(3),
                WidgetId(4),
                Some(EditorConfirmationChoice::Accept)
            )
            .initial_focus,
            Some(WidgetId(4))
        );
    }

    #[test]
    fn clear_map_native_widget_targets_bind_only_cancel_and_yes() {
        assert_eq!(
            clear_map_confirmation_choice(WidgetId(3), WidgetId(3), WidgetId(4)),
            Some(EditorConfirmationChoice::Cancel)
        );
        assert_eq!(
            clear_map_confirmation_choice(WidgetId(4), WidgetId(3), WidgetId(4)),
            Some(EditorConfirmationChoice::Accept)
        );
        assert_eq!(
            clear_map_confirmation_choice(WidgetId(5), WidgetId(3), WidgetId(4)),
            None
        );
    }
}
