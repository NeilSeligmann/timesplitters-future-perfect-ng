//! Native keyboard, controller, and pointer input for Map Maker's disable-mode
//! checklist. The renderer supplies ordered opaque IDs; Rust owns each flag.

use crate::{
    input::IntentSet,
    owned_catalogue::{self, CatalogueAction},
};
use tsfp_mapmaker::disabled_modes::{
    DisabledModeId, DisabledModeTarget, DisabledModesPage, DisabledModesUpdate,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerEvent {
    Hover(DisabledModeId),
    Click(DisabledModeId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Focus(DisabledModeTarget),
    Toggle(DisabledModeTarget),
}

pub fn commands(
    page: &DisabledModesPage,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<Command> {
    let rows = page.rows();
    if rows.is_empty() {
        return Vec::new();
    }
    let mut cursor = page
        .focused()
        .and_then(|focused| rows.iter().position(|row| row.id == focused));
    let mut result = Vec::new();
    for action in owned_catalogue::actions(held, before, now_ms, repeat_at, false) {
        match action {
            CatalogueAction::Step(delta) => {
                let next = match cursor {
                    Some(index) => index.checked_add_signed(delta as isize),
                    None if delta > 0 => Some(0),
                    None => rows.len().checked_sub(1),
                }
                .filter(|&index| index < rows.len());
                if let Some(next) = next
                    && cursor != Some(next)
                    && let Some(target) = page.target(rows[next].id)
                {
                    result.push(Command::Focus(target));
                    cursor = Some(next);
                }
            }
            CatalogueAction::Accept => {
                if let Some(target) = cursor
                    .and_then(|index| rows.get(index))
                    .and_then(|row| page.target(row.id))
                {
                    result.push(Command::Toggle(target));
                }
            }
            CatalogueAction::Back | CatalogueAction::Focus(_) => {}
        }
    }
    result
}

pub fn pointer_commands(page: &DisabledModesPage, event: PointerEvent) -> Vec<Command> {
    let id = match event {
        PointerEvent::Hover(id) | PointerEvent::Click(id) => id,
    };
    let Some(target) = page.target(id) else {
        return Vec::new();
    };
    let mut result = Vec::with_capacity(2);
    if page.focused() != Some(id) {
        result.push(Command::Focus(target));
    }
    if matches!(event, PointerEvent::Click(_)) {
        result.push(Command::Toggle(target));
    }
    result
}

pub fn dispatch_frame(
    page: &mut DisabledModesPage,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<DisabledModesUpdate> {
    commands(page, held, before, now_ms, repeat_at)
        .into_iter()
        .map(|command| match command {
            Command::Focus(target) => page.focus(target),
            Command::Toggle(target) => page.toggle(target),
        })
        .collect()
}

pub fn dispatch_pointer(
    page: &mut DisabledModesPage,
    event: PointerEvent,
) -> Vec<DisabledModesUpdate> {
    pointer_commands(page, event)
        .into_iter()
        .map(|command| match command {
            Command::Focus(target) => page.focus(target),
            Command::Toggle(target) => page.toggle(target),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::MenuIntent as I;
    use tsfp_mapmaker::disabled_modes::{DisabledModeRow, DisabledModesGeneration};

    fn page() -> DisabledModesPage {
        let mut page = DisabledModesPage::default();
        page.install(
            DisabledModesGeneration::new(8),
            vec![
                DisabledModeRow {
                    id: DisabledModeId::new(1),
                    disabled: false,
                },
                DisabledModeRow {
                    id: DisabledModeId::new(2),
                    disabled: true,
                },
            ],
        )
        .unwrap();
        page
    }

    #[test]
    fn keyboard_and_controller_intents_focus_and_toggle_rust_owned_flags() {
        let mut page = page();
        let mut repeat = [0; 4];
        let down = IntentSet::default().with(I::Down);
        assert_eq!(
            dispatch_frame(&mut page, down, IntentSet::default(), 1, &mut repeat),
            vec![DisabledModesUpdate::Focused(DisabledModeId::new(1))]
        );
        let accept = IntentSet::default().with(I::Accept);
        assert_eq!(
            dispatch_frame(&mut page, accept, IntentSet::default(), 2, &mut repeat),
            vec![DisabledModesUpdate::Toggled {
                id: DisabledModeId::new(1),
                disabled: true,
            }]
        );
        assert!(page.rows()[0].disabled);
        assert!(page.rows()[1].disabled);
    }

    #[test]
    fn pointer_click_focuses_then_toggles_only_the_hit_row() {
        let mut page = page();
        assert_eq!(
            dispatch_pointer(&mut page, PointerEvent::Click(DisabledModeId::new(2))),
            vec![
                DisabledModesUpdate::Focused(DisabledModeId::new(2)),
                DisabledModesUpdate::Toggled {
                    id: DisabledModeId::new(2),
                    disabled: false,
                },
            ]
        );
        assert_eq!(page.rows()[0].disabled, false);
        assert_eq!(page.rows()[1].disabled, false);
        assert!(
            dispatch_pointer(&mut page, PointerEvent::Click(DisabledModeId::new(99))).is_empty()
        );
    }
}
