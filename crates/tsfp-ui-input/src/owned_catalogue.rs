//! Typed input boundary for Rust-owned storage catalogues, including Map Maker.
//!
//! Device adapters reduce keyboard and pad state to [`IntentSet`] before this
//! module runs. The result is a small command vocabulary consumed by the
//! authenticated `StorageSession`; no guest page offsets or synthetic pad
//! events cross this boundary.

use crate::input::{IntentSet, MenuIntent};

/// Commands accepted by an owned storage catalogue/session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CatalogueAction {
    Step(i32),
    Focus(usize),
    Accept,
    Back,
}

/// Map the current semantic input frame to owned catalogue commands.
///
/// Up and Down repeat after 400 ms, then every 120 ms, matching the current
/// native UI host policy. Accept and Back are edge triggered; Back (including
/// pointer right-click) wins over every other command in that frame.
pub fn actions(
    held: IntentSet,
    before: IntentSet,
    now_ms: u64,
    repeat_at: &mut [u64; 4],
    pointer_back: bool,
) -> Vec<CatalogueAction> {
    let mut commands = Vec::new();
    if pointer_back || (held.contains(MenuIntent::Back) && !before.contains(MenuIntent::Back)) {
        commands.push(CatalogueAction::Back);
        return commands;
    }

    for (index, intent, delta) in [(0, MenuIntent::Up, -1), (1, MenuIntent::Down, 1)] {
        if !held.contains(intent) {
            continue;
        }
        if !before.contains(intent) {
            repeat_at[index] = now_ms.saturating_add(400);
            commands.push(CatalogueAction::Step(delta));
        } else if now_ms >= repeat_at[index] {
            repeat_at[index] = now_ms.saturating_add(120);
            commands.push(CatalogueAction::Step(delta));
        }
    }

    if held.contains(MenuIntent::Accept) && !before.contains(MenuIntent::Accept) {
        commands.push(CatalogueAction::Accept);
    }
    commands
}

#[cfg(test)]
mod tests {
    use super::{CatalogueAction as A, actions};
    use crate::input::{IntentSet, MenuIntent as I};

    #[test]
    fn keyboard_and_pad_semantic_rows_step_once_then_repeat_on_schedule() {
        let mut repeat = [0; 4];
        let down = IntentSet::default().with(I::Down);
        assert_eq!(
            actions(down, IntentSet::default(), 10, &mut repeat, false),
            [A::Step(1)]
        );
        assert!(actions(down, down, 409, &mut repeat, false).is_empty());
        assert_eq!(actions(down, down, 410, &mut repeat, false), [A::Step(1)]);
        assert!(actions(down, down, 529, &mut repeat, false).is_empty());
        assert_eq!(actions(down, down, 530, &mut repeat, false), [A::Step(1)]);
    }

    #[test]
    fn accept_is_edge_triggered_and_back_wins_over_navigation() {
        let mut repeat = [0; 4];
        let accept = IntentSet::default().with(I::Accept);
        assert_eq!(
            actions(accept, IntentSet::default(), 0, &mut repeat, false),
            [A::Accept]
        );
        assert!(actions(accept, accept, 1000, &mut repeat, false).is_empty());

        let mixed = IntentSet::default()
            .with(I::Up)
            .with(I::Accept)
            .with(I::Back);
        assert_eq!(
            actions(mixed, IntentSet::default(), 1000, &mut repeat, false),
            [A::Back]
        );
        assert_eq!(
            actions(accept, IntentSet::default(), 1000, &mut repeat, true),
            [A::Back]
        );
    }

    #[test]
    fn simultaneous_directions_keep_each_ordered_step() {
        let mut repeat = [0; 4];
        let both = IntentSet::default().with(I::Up).with(I::Down);
        assert_eq!(
            actions(both, IntentSet::default(), 0, &mut repeat, false),
            [A::Step(-1), A::Step(1)]
        );
    }
}
