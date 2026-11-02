//! Native keyboard, controller, and pointer input for Assault phase toggles.
//!
//! Phase IDs and availability arrive from the caller because REA has not
//! established a portable descriptor ordering or caller-side phase identity.

use std::vec::{self, Vec};

use crate::{input::IntentSet, owned_catalogue::CatalogueAction};
use tsfp_mapmaker::assault_phase_toggle::{
    AssaultPhaseId, AssaultPhaseToggleOutcome, AssaultPhaseTogglePage, AssaultPhaseToggleRow,
    AssaultPhaseToggleTarget,
};

/// Resolve ordinary menu navigation into a target supplied by the rendered
/// checklist. Disabled rows remain harmless because the state model checks
/// availability when the target is applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Focus(AssaultPhaseId),
    Toggle(AssaultPhaseToggleTarget),
}

pub fn commands(
    page: &AssaultPhaseTogglePage,
    focused: Option<AssaultPhaseId>,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<Action> {
    let mut result = Vec::new();
    let rows = page
        .rows()
        .iter()
        .filter(|row| row.available())
        .collect::<Vec<_>>();
    let actions = crate::owned_catalogue::actions(held, before, now_ms, repeat_at, false);
    for action in actions {
        match action {
            CatalogueAction::Accept => {
                if let Some(id) = focused
                    && rows.iter().any(|row| row.id() == id)
                {
                    result.push(Action::Toggle(page.target(id)));
                }
            }
            CatalogueAction::Step(delta) if !rows.is_empty() => {
                let current = focused.and_then(|id| rows.iter().position(|row| row.id() == id));
                let index = match (current, delta.signum()) {
                    (Some(index), direction) => {
                        (index as i64 + i64::from(direction)).rem_euclid(rows.len() as i64) as usize
                    }
                    (None, direction) if direction < 0 => rows.len() - 1,
                    _ => 0,
                };
                result.push(Action::Focus(rows[index].id()));
            }
            _ => {}
        }
    }
    result
}

pub fn dispatch_frame(
    page: &mut AssaultPhaseTogglePage,
    focused: Option<AssaultPhaseId>,
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
) -> Vec<Action> {
    commands(page, focused, held, before, now_ms, repeat_at)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerEvent {
    Activate(AssaultPhaseToggleTarget),
}

pub fn dispatch_pointer(
    page: &mut AssaultPhaseTogglePage,
    event: PointerEvent,
) -> AssaultPhaseToggleOutcome {
    match event {
        PointerEvent::Activate(target) => page.toggle(target),
    }
}

/// Owns one checklist opening's row focus and input repeat state alongside its
/// typed page data. The caller supplies the initial focus because original
/// focus policy and row ordering are not established by REA.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssaultPhaseToggleSession {
    page: AssaultPhaseTogglePage,
    focused: Option<AssaultPhaseId>,
    repeat_at: [u64; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionUpdate {
    Focused(AssaultPhaseId),
    Toggled(AssaultPhaseToggleOutcome),
}

/// Renderer-facing checklist row projected from the same Rust session that
/// accepts input. Labels and geometry remain caller supplied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionRow {
    id: AssaultPhaseId,
    available: bool,
    value: u32,
    focused: bool,
}

impl SessionRow {
    pub const fn id(self) -> AssaultPhaseId {
        self.id
    }

    pub const fn available(self) -> bool {
        self.available
    }

    pub const fn value(self) -> u32 {
        self.value
    }

    pub const fn focused(self) -> bool {
        self.focused
    }
}

impl AssaultPhaseToggleSession {
    pub fn new(page: AssaultPhaseTogglePage, focused: Option<AssaultPhaseId>) -> Self {
        let focused = focused.filter(|id| {
            page.rows()
                .iter()
                .any(|row| row.id() == *id && row.available())
        });
        Self {
            page,
            focused,
            repeat_at: [0; 4],
        }
    }

    pub const fn focused(&self) -> Option<AssaultPhaseId> {
        self.focused
    }

    pub fn page(&self) -> &AssaultPhaseTogglePage {
        &self.page
    }

    /// Return ordered display state with focus qualified by this session.
    pub fn render_rows(&self) -> Vec<SessionRow> {
        self.page
            .rows()
            .iter()
            .map(|row: &AssaultPhaseToggleRow| SessionRow {
                id: row.id(),
                available: row.available(),
                value: row.value(),
                focused: self.focused == Some(row.id()),
            })
            .collect()
    }

    /// Replace the caller's checklist snapshot and reset input state.
    /// Focus is caller supplied and retained only when it names an available
    /// row in the replacement page.
    pub fn replace_page(
        &mut self,
        page: AssaultPhaseTogglePage,
        focused: Option<AssaultPhaseId>,
    ) -> Option<AssaultPhaseId> {
        let focused = focused.filter(|id| {
            page.rows()
                .iter()
                .any(|row| row.id() == *id && row.available())
        });
        self.page = page;
        self.focused = focused;
        self.repeat_at = [0; 4];
        focused
    }

    pub fn dispatch_frame(
        &mut self,
        held: IntentSet,
        before: IntentSet,
        now_ms: u64,
    ) -> Vec<SessionUpdate> {
        let actions = commands(
            &self.page,
            self.focused,
            held,
            before,
            now_ms,
            &mut self.repeat_at,
        );
        let mut updates = Vec::with_capacity(actions.len());
        for action in actions {
            match action {
                Action::Focus(id) => {
                    self.focused = Some(id);
                    updates.push(SessionUpdate::Focused(id));
                }
                Action::Toggle(target) => {
                    updates.push(SessionUpdate::Toggled(self.page.toggle(target)))
                }
            }
        }
        updates
    }

    pub fn dispatch_pointer(&mut self, event: PointerEvent) -> AssaultPhaseToggleOutcome {
        dispatch_pointer(&mut self.page, event)
    }

    /// Apply a row click as one session operation: validate the rendered
    /// target against the current page, move focus to it, then toggle it.
    /// Stale and unavailable targets leave both focus and row values intact.
    pub fn activate_pointer(
        &mut self,
        target: AssaultPhaseToggleTarget,
    ) -> Option<AssaultPhaseToggleOutcome> {
        self.focus_pointer(target)?;
        Some(self.dispatch_pointer(PointerEvent::Activate(target)))
    }

    /// Apply pointer focus from a renderer-resolved target. This is caller
    /// supplied native behavior; it does not claim the original cursor policy.
    pub fn focus_pointer(&mut self, target: AssaultPhaseToggleTarget) -> Option<AssaultPhaseId> {
        let phase = target.phase();
        if self.page.target(phase) != target {
            return None;
        }
        let row = self
            .page
            .rows()
            .iter()
            .find(|row| row.id() == phase && row.available())?;
        let id = row.id();
        if self.focused != Some(id) {
            self.repeat_at = [0; 4];
        }
        self.focused = Some(id);
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec;
    use tsfp_mapmaker::assault_phase_toggle::{
        AssaultPhaseToggleGeneration, AssaultPhaseToggleRow,
    };

    fn page() -> AssaultPhaseTogglePage {
        AssaultPhaseTogglePage::new(
            AssaultPhaseToggleGeneration::new(4),
            vec![
                AssaultPhaseToggleRow::new(AssaultPhaseId::new(8), true, 0),
                AssaultPhaseToggleRow::new(AssaultPhaseId::new(9), false, 1),
                AssaultPhaseToggleRow::new(AssaultPhaseId::new(10), true, 0),
            ],
        )
        .unwrap()
    }

    #[test]
    fn session_owns_keyboard_focus_and_applies_toggle_to_its_page() {
        let enabled = AssaultPhaseId::new(8);
        let next = AssaultPhaseId::new(10);
        let mut session = AssaultPhaseToggleSession::new(page(), Some(enabled));
        let accept = IntentSet::default().with(crate::input::MenuIntent::Accept);
        assert_eq!(
            session.dispatch_frame(accept, IntentSet::default(), 10),
            [SessionUpdate::Toggled(AssaultPhaseToggleOutcome::Toggled {
                phase: enabled,
                value: 1,
            })]
        );
        let down = IntentSet::default().with(crate::input::MenuIntent::Down);
        assert_eq!(
            session.dispatch_frame(down, IntentSet::default(), 20),
            [SessionUpdate::Focused(next)]
        );
        assert_eq!(session.focused(), Some(next));
        assert_eq!(session.page().rows()[0].value(), 1);
    }

    #[test]
    fn session_pointer_activation_mutates_only_the_generation_qualified_row() {
        let mut session = AssaultPhaseToggleSession::new(page(), None);
        let id = AssaultPhaseId::new(10);
        let outcome = session.dispatch_pointer(PointerEvent::Activate(session.page().target(id)));
        assert_eq!(
            outcome,
            AssaultPhaseToggleOutcome::Toggled {
                phase: id,
                value: 1
            }
        );
        assert_eq!(session.page().rows()[2].value(), 1);
    }

    #[test]
    fn session_pointer_focus_updates_only_an_available_current_row() {
        let mut session = AssaultPhaseToggleSession::new(page(), None);
        let enabled = AssaultPhaseId::new(10);
        session.repeat_at = [40, 50, 60, 70];
        assert_eq!(
            session.focus_pointer(session.page().target(enabled)),
            Some(enabled)
        );
        assert_eq!(session.focused(), Some(enabled));
        assert_eq!(session.repeat_at, [0; 4]);
        session.repeat_at = [80, 90, 100, 110];
        assert_eq!(
            session.focus_pointer(session.page().target(AssaultPhaseId::new(9))),
            None
        );
        assert_eq!(session.focused(), Some(enabled));
        assert_eq!(session.repeat_at, [80, 90, 100, 110]);
        let stale_page = AssaultPhaseTogglePage::new(
            AssaultPhaseToggleGeneration::new(3),
            vec![AssaultPhaseToggleRow::new(enabled, true, 0)],
        )
        .unwrap();
        let stale = stale_page.target(enabled);
        assert_eq!(session.focus_pointer(stale), None);
        assert_eq!(session.focused(), Some(enabled));
    }

    #[test]
    fn pointer_activation_focuses_and_toggles_only_a_current_available_row() {
        let enabled = AssaultPhaseId::new(10);
        let mut session = AssaultPhaseToggleSession::new(page(), None);
        assert_eq!(
            session.activate_pointer(session.page().target(enabled)),
            Some(AssaultPhaseToggleOutcome::Toggled {
                phase: enabled,
                value: 1,
            })
        );
        assert_eq!(session.focused(), Some(enabled));

        let repeat_at = session.repeat_at;
        let disabled = AssaultPhaseId::new(9);
        assert_eq!(
            session.activate_pointer(session.page().target(disabled)),
            None
        );
        assert_eq!(session.focused(), Some(enabled));
        assert_eq!(session.page().rows()[1].value(), 1);
        assert_eq!(session.repeat_at, repeat_at);

        let stale_page = AssaultPhaseTogglePage::new(
            AssaultPhaseToggleGeneration::new(3),
            vec![AssaultPhaseToggleRow::new(enabled, true, 0)],
        )
        .unwrap();
        assert_eq!(session.activate_pointer(stale_page.target(enabled)), None);
        assert_eq!(session.focused(), Some(enabled));
        assert_eq!(session.page().rows()[2].value(), 1);
    }

    #[test]
    fn replacing_page_uses_only_caller_focus_and_resets_repeat_state() {
        let mut session = AssaultPhaseToggleSession::new(page(), Some(AssaultPhaseId::new(8)));
        session.repeat_at = [40, 50, 60, 70];
        let replacement = AssaultPhaseTogglePage::new(
            AssaultPhaseToggleGeneration::new(5),
            vec![
                AssaultPhaseToggleRow::new(AssaultPhaseId::new(8), false, 1),
                AssaultPhaseToggleRow::new(AssaultPhaseId::new(12), true, 0),
            ],
        )
        .unwrap();

        assert_eq!(
            session.replace_page(replacement, Some(AssaultPhaseId::new(8))),
            None
        );
        assert_eq!(session.focused(), None);
        assert_eq!(session.repeat_at, [0; 4]);
        assert_eq!(session.page().rows()[1].id(), AssaultPhaseId::new(12));

        assert_eq!(
            session.replace_page(page(), Some(AssaultPhaseId::new(10))),
            Some(AssaultPhaseId::new(10))
        );
        assert_eq!(session.focused(), Some(AssaultPhaseId::new(10)));
    }

    #[test]
    fn new_session_accepts_only_focus_for_an_available_row() {
        let available = AssaultPhaseId::new(8);
        assert_eq!(
            AssaultPhaseToggleSession::new(page(), Some(available)).focused(),
            Some(available)
        );
        assert_eq!(
            AssaultPhaseToggleSession::new(page(), Some(AssaultPhaseId::new(9))).focused(),
            None
        );
        assert_eq!(
            AssaultPhaseToggleSession::new(page(), Some(AssaultPhaseId::new(77))).focused(),
            None
        );
    }

    #[test]
    fn render_rows_share_the_session_focus_and_keep_caller_row_order() {
        let mut session = AssaultPhaseToggleSession::new(page(), Some(AssaultPhaseId::new(8)));
        let rows = session.render_rows();
        assert_eq!(
            rows.iter().map(|row| row.id()).collect::<Vec<_>>(),
            [
                AssaultPhaseId::new(8),
                AssaultPhaseId::new(9),
                AssaultPhaseId::new(10),
            ]
        );
        assert_eq!(
            rows.iter().map(|row| row.focused()).collect::<Vec<_>>(),
            [true, false, false]
        );
        assert_eq!(
            rows.iter().map(|row| row.available()).collect::<Vec<_>>(),
            [true, false, true]
        );
        assert_eq!(
            rows.iter().map(|row| row.value()).collect::<Vec<_>>(),
            [0, 1, 0]
        );

        session.focus_pointer(session.page().target(AssaultPhaseId::new(10)));
        assert_eq!(
            session
                .render_rows()
                .iter()
                .map(|row| row.focused())
                .collect::<Vec<_>>(),
            [false, false, true]
        );
    }

    #[test]
    fn keyboard_accept_toggles_only_the_focused_available_phase() {
        let mut page = page();
        let accept = IntentSet::default().with(crate::input::MenuIntent::Accept);
        let mut repeat_at = [0; 4];
        let expected = Action::Toggle(page.target(AssaultPhaseId::new(8)));
        assert_eq!(
            dispatch_frame(
                &mut page,
                Some(AssaultPhaseId::new(8)),
                accept,
                IntentSet::default(),
                10,
                &mut repeat_at,
            ),
            [expected]
        );
        assert_eq!(
            dispatch_frame(
                &mut page,
                Some(AssaultPhaseId::new(9)),
                accept,
                IntentSet::default(),
                11,
                &mut repeat_at,
            ),
            []
        );
    }

    #[test]
    fn keyboard_navigation_skips_unavailable_rows_and_wraps() {
        let page = page();
        let down = IntentSet::default().with(crate::input::MenuIntent::Down);
        let up = IntentSet::default().with(crate::input::MenuIntent::Up);
        let mut repeat_at = [0; 4];
        assert_eq!(
            commands(
                &page,
                Some(AssaultPhaseId::new(8)),
                down,
                IntentSet::default(),
                10,
                &mut repeat_at
            ),
            [Action::Focus(AssaultPhaseId::new(10))]
        );
        assert_eq!(
            commands(
                &page,
                Some(AssaultPhaseId::new(10)),
                up,
                IntentSet::default(),
                11,
                &mut repeat_at
            ),
            [Action::Focus(AssaultPhaseId::new(8))]
        );
        assert_eq!(
            commands(&page, None, up, IntentSet::default(), 12, &mut repeat_at),
            [Action::Focus(AssaultPhaseId::new(10))]
        );
    }

    #[test]
    fn pointer_activation_uses_generation_qualified_target() {
        let mut page = page();
        let target = page.target(AssaultPhaseId::new(8));
        assert_eq!(
            dispatch_pointer(&mut page, PointerEvent::Activate(target)),
            AssaultPhaseToggleOutcome::Toggled {
                phase: AssaultPhaseId::new(8),
                value: 1,
            }
        );
    }
}
