//! Native keyboard input for the Rust-owned Map Description editor.

use tsfp_mapmaker::description::{DescriptionCommand, DescriptionTarget};
use tsfp_mapmaker::ui::BriefingTextTarget;

use crate::input::{IntentSet, MenuIntent};

/// A runtime-projected key in the Map Description on-screen keyboard.
/// Neighbor IDs come from the page layout provider; this reducer does not
/// assume the profile-name grid geometry applies to every text page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DescriptionGridKey {
    pub id: u32,
    pub up: Option<u32>,
    pub down: Option<u32>,
    pub left: Option<u32>,
    pub right: Option<u32>,
    pub command: DescriptionCommand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DescriptionGridError {
    Empty,
    DuplicateKey(u32),
    MissingKey(u32),
    DuplicateHitTarget(u32),
    MissingHitTarget(u32),
    InvalidHitTarget(u32),
    EmptyCaretMap,
    DuplicateCaretIndex(usize),
    MissingCaretIndex(usize),
    InvalidCaretHitTarget(usize),
    Inactive,
}

/// A design-space rectangle supplied by the live Map Description page.
/// Bounds use left/top inclusive and right/bottom exclusive edges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DescriptionKeyboardHitTarget {
    pub key: u32,
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// Rust-owned hit testing for the live page's projected key rectangles.
#[derive(Clone, Debug, PartialEq)]
pub struct DescriptionKeyboardHitMap {
    targets: Vec<DescriptionKeyboardHitTarget>,
}

/// A caller-projected hit region for one Unicode-scalar caret boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DescriptionCaretHitTarget {
    pub index: usize,
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// Rust-owned caret hit testing, tied to the exact text and generation it lays out.
#[derive(Clone, Debug, PartialEq)]
pub struct DescriptionCaretHitMap {
    target: DescriptionTarget,
    text_snapshot: String,
    targets: Vec<DescriptionCaretHitTarget>,
}

impl DescriptionCaretHitMap {
    pub fn new(
        target: DescriptionTarget,
        text: &str,
        targets: Vec<DescriptionCaretHitTarget>,
    ) -> Result<Self, DescriptionGridError> {
        if targets.is_empty() {
            return Err(DescriptionGridError::EmptyCaretMap);
        }
        let count = text.chars().count();
        for (index, hit) in targets.iter().enumerate() {
            if hit.index > count
                || !hit.left.is_finite()
                || !hit.top.is_finite()
                || !hit.right.is_finite()
                || !hit.bottom.is_finite()
                || hit.right <= hit.left
                || hit.bottom <= hit.top
            {
                return Err(DescriptionGridError::InvalidCaretHitTarget(hit.index));
            }
            if targets[..index]
                .iter()
                .any(|other| other.index == hit.index)
            {
                return Err(DescriptionGridError::DuplicateCaretIndex(hit.index));
            }
            if targets[..index].iter().any(|other| {
                hit.left < other.right
                    && other.left < hit.right
                    && hit.top < other.bottom
                    && other.top < hit.bottom
            }) {
                return Err(DescriptionGridError::InvalidCaretHitTarget(hit.index));
            }
        }
        for caret in 0..=count {
            if !targets.iter().any(|hit| hit.index == caret) {
                return Err(DescriptionGridError::MissingCaretIndex(caret));
            }
        }
        Ok(Self {
            target,
            text_snapshot: text.to_owned(),
            targets,
        })
    }

    pub fn target(&self) -> DescriptionTarget {
        self.target
    }

    pub fn matches_text(&self, text: &str) -> bool {
        self.text_snapshot == text
    }

    pub fn targets(&self) -> &[DescriptionCaretHitTarget] {
        &self.targets
    }

    /// Return a caret only for a finite point inside a half-open design-space box.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<usize> {
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        self.targets
            .iter()
            .find(|hit| x >= hit.left && x < hit.right && y >= hit.top && y < hit.bottom)
            .map(|hit| hit.index)
    }
}

impl DescriptionKeyboardHitMap {
    pub fn new(
        keys: &[DescriptionGridKey],
        targets: Vec<DescriptionKeyboardHitTarget>,
    ) -> Result<Self, DescriptionGridError> {
        for (index, target) in targets.iter().enumerate() {
            if targets[..index].iter().any(|other| other.key == target.key) {
                return Err(DescriptionGridError::DuplicateHitTarget(target.key));
            }
            if !keys.iter().any(|key| key.id == target.key) {
                return Err(DescriptionGridError::MissingKey(target.key));
            }
            if !target.left.is_finite()
                || !target.top.is_finite()
                || !target.right.is_finite()
                || !target.bottom.is_finite()
                || target.left >= target.right
                || target.top >= target.bottom
            {
                return Err(DescriptionGridError::InvalidHitTarget(target.key));
            }
        }
        for key in keys {
            if !targets.iter().any(|target| target.key == key.id) {
                return Err(DescriptionGridError::MissingHitTarget(key.id));
            }
        }
        for (index, target) in targets.iter().enumerate() {
            if targets[index + 1..].iter().any(|other| {
                target.left < other.right
                    && other.left < target.right
                    && target.top < other.bottom
                    && other.top < target.bottom
            }) {
                return Err(DescriptionGridError::InvalidHitTarget(target.key));
            }
        }
        Ok(Self { targets })
    }

    pub fn targets(&self) -> &[DescriptionKeyboardHitTarget] {
        &self.targets
    }

    pub fn hit_test(&self, x: f32, y: f32) -> Option<u32> {
        self.targets
            .iter()
            .find(|target| {
                x >= target.left && x < target.right && y >= target.top && y < target.bottom
            })
            .map(|target| target.key)
    }
}

/// Typed controller and pointer state for a caller-supplied on-screen key grid.
/// The caller owns rendering and obtains key actions from runtime text tables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DescriptionKeyboardGrid {
    target: DescriptionTarget,
    keys: Vec<DescriptionGridKey>,
    focused: u32,
}

impl DescriptionKeyboardGrid {
    pub fn new(
        target: DescriptionTarget,
        keys: Vec<DescriptionGridKey>,
        focused: u32,
    ) -> Result<Self, DescriptionGridError> {
        if keys.is_empty() {
            return Err(DescriptionGridError::Empty);
        }
        for (index, key) in keys.iter().enumerate() {
            if keys[..index].iter().any(|other| other.id == key.id) {
                return Err(DescriptionGridError::DuplicateKey(key.id));
            }
        }
        for key in &keys {
            for neighbor in [key.up, key.down, key.left, key.right]
                .into_iter()
                .flatten()
            {
                if !keys.iter().any(|candidate| candidate.id == neighbor) {
                    return Err(DescriptionGridError::MissingKey(neighbor));
                }
            }
        }
        if !keys.iter().any(|key| key.id == focused) {
            return Err(DescriptionGridError::MissingKey(focused));
        }
        Ok(Self {
            target,
            keys,
            focused,
        })
    }

    pub fn focused(&self) -> u32 {
        self.focused
    }

    pub fn target(&self) -> DescriptionTarget {
        self.target
    }

    pub fn keys(&self) -> &[DescriptionGridKey] {
        &self.keys
    }

    /// A pointer hit test selects a typed key identity, never a guest address.
    pub fn focus(&mut self, key: u32) -> Result<(), DescriptionGridError> {
        if !self.keys.iter().any(|candidate| candidate.id == key) {
            return Err(DescriptionGridError::MissingKey(key));
        }
        self.focused = key;
        Ok(())
    }

    /// Convert controller edges into focus movement or a generation-bound edit.
    /// Back wins; otherwise the first edge in the documented order is handled.
    pub fn input(
        &mut self,
        held: IntentSet,
        before: IntentSet,
    ) -> Option<(DescriptionTarget, DescriptionCommand)> {
        for (intent, neighbor) in [
            (MenuIntent::Back, None),
            (MenuIntent::Up, Some(Direction::Up)),
            (MenuIntent::Down, Some(Direction::Down)),
            (MenuIntent::Left, Some(Direction::Left)),
            (MenuIntent::Right, Some(Direction::Right)),
        ] {
            if !held.contains(intent) || before.contains(intent) {
                continue;
            }
            if intent == MenuIntent::Back {
                return Some((self.target, DescriptionCommand::Cancel));
            }
            let key = self.key(self.focused)?;
            let next = match neighbor? {
                Direction::Up => key.up,
                Direction::Down => key.down,
                Direction::Left => key.left,
                Direction::Right => key.right,
            }?;
            self.focused = next;
            return None;
        }
        for (intent, command) in [
            (MenuIntent::Backspace, DescriptionCommand::Backspace),
            (MenuIntent::Accept, self.key(self.focused)?.command),
        ] {
            if held.contains(intent) && !before.contains(intent) {
                return Some((self.target, command));
            }
        }
        None
    }

    /// Pointer activation follows the same typed key action as controller A.
    pub fn activate(&self) -> (DescriptionTarget, DescriptionCommand) {
        let command = self
            .key(self.focused)
            .expect("focused key is validated at construction")
            .command;
        (self.target, command)
    }

    /// Move focus from a pointer hit and optionally activate that key.
    pub fn pointer(
        &mut self,
        hit_map: &DescriptionKeyboardHitMap,
        x: f32,
        y: f32,
        activate: bool,
    ) -> Option<(DescriptionTarget, DescriptionCommand)> {
        self.focus(hit_map.hit_test(x, y)?).ok()?;
        activate.then(|| self.activate())
    }

    fn key(&self, id: u32) -> Option<&DescriptionGridKey> {
        self.keys.iter().find(|key| key.id == id)
    }
}

#[derive(Clone, Copy)]
enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Translate text characters and keyboard navigation into page-level native
/// commands. The runtime still owns text sourcing and accepting the commit.
pub fn commands(
    target: DescriptionTarget,
    chars: &[char],
    held: IntentSet,
    before: IntentSet,
) -> Vec<(DescriptionTarget, DescriptionCommand)> {
    commands_with_delete(target, chars, false, held, before)
}

/// Translate one native text-input frame, including the host Delete key, into
/// generation-bound description commands. Device-specific key decoding stays
/// at the host edge; navigation, editing and commit intents share this reducer.
pub fn commands_with_delete(
    target: DescriptionTarget,
    chars: &[char],
    delete: bool,
    held: IntentSet,
    before: IntentSet,
) -> Vec<(DescriptionTarget, DescriptionCommand)> {
    commands_for_text_input(chars, delete, held, before)
        .into_iter()
        .map(|command| (target, command))
        .collect()
}

/// Translate native keyboard edits for the Rust-owned Briefing field while
/// preserving its page-specific token at the input boundary.
pub fn briefing_text_commands_with_delete(
    target: BriefingTextTarget,
    chars: &[char],
    delete: bool,
    held: IntentSet,
    before: IntentSet,
) -> Vec<(BriefingTextTarget, DescriptionCommand)> {
    commands_for_text_input(chars, delete, held, before)
        .into_iter()
        .map(|command| (target, command))
        .collect()
}

fn commands_for_text_input(
    chars: &[char],
    delete: bool,
    held: IntentSet,
    before: IntentSet,
) -> Vec<DescriptionCommand> {
    let mut result = Vec::new();
    if held.contains(MenuIntent::Back) && !before.contains(MenuIntent::Back) {
        result.push(DescriptionCommand::Cancel);
        return result;
    }

    result.extend(chars.iter().copied().map(DescriptionCommand::Insert));
    if delete {
        result.push(DescriptionCommand::Delete);
    }

    for (intent, command) in [
        (MenuIntent::Left, DescriptionCommand::MoveLeft),
        (MenuIntent::Right, DescriptionCommand::MoveRight),
        (MenuIntent::Backspace, DescriptionCommand::Backspace),
        (MenuIntent::Accept, DescriptionCommand::Commit),
    ] {
        if held.contains(intent) && !before.contains(intent) {
            result.push(command);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{IntentSet, MenuIntent};

    fn target() -> DescriptionTarget {
        tsfp_mapmaker::description::MapDescriptionSession::new(
            tsfp_mapmaker::description::DescriptionGeneration::new(4),
            "",
            16,
        )
        .unwrap()
        .target()
    }

    fn briefing_target() -> BriefingTextTarget {
        use tsfp_mapmaker::ui::{
            EditorSurface, EditorSurfaceGeneration, MapMakerUiCommand, MapMakerUiState,
        };

        let mut state = MapMakerUiState::default();
        state.dispatch(MapMakerUiCommand::BeginEditor);
        state
            .install_editor_surface_catalog(
                EditorSurfaceGeneration::new(4),
                vec![EditorSurface::Briefing],
            )
            .unwrap();
        let surface = state
            .editor_surface_target(EditorSurface::Briefing)
            .unwrap();
        state.dispatch(MapMakerUiCommand::OpenEditorSurface(surface));
        state.install_briefing_text_session(surface, "", 8).unwrap()
    }

    #[test]
    fn briefing_keyboard_edits_keep_the_briefing_field_token() {
        let target = briefing_target();
        let held = IntentSet::default()
            .with(MenuIntent::Left)
            .with(MenuIntent::Accept);

        assert_eq!(
            briefing_text_commands_with_delete(target, &['B'], true, held, IntentSet::default()),
            vec![
                (target, DescriptionCommand::Insert('B')),
                (target, DescriptionCommand::Delete),
                (target, DescriptionCommand::MoveLeft),
                (target, DescriptionCommand::Commit),
            ]
        );
        assert_eq!(
            briefing_text_commands_with_delete(
                target,
                &['x'],
                true,
                IntentSet::default().with(MenuIntent::Back),
                IntentSet::default(),
            ),
            vec![(target, DescriptionCommand::Cancel)]
        );
        assert!(
            briefing_text_commands_with_delete(
                target,
                &[],
                false,
                IntentSet::default().with(MenuIntent::Back),
                IntentSet::default().with(MenuIntent::Back),
            )
            .is_empty()
        );
    }

    fn keyboard(target: DescriptionTarget) -> DescriptionKeyboardGrid {
        DescriptionKeyboardGrid::new(
            target,
            vec![
                DescriptionGridKey {
                    id: 10,
                    up: None,
                    down: None,
                    left: None,
                    right: Some(20),
                    command: DescriptionCommand::Insert('m'),
                },
                DescriptionGridKey {
                    id: 20,
                    up: None,
                    down: None,
                    left: Some(10),
                    right: None,
                    command: DescriptionCommand::Delete,
                },
            ],
            10,
        )
        .unwrap()
    }

    #[test]
    fn controller_grid_moves_focus_then_emits_typed_key_action() {
        let target = target();
        let mut grid = keyboard(target);
        let right = IntentSet::default().with(MenuIntent::Right);

        assert_eq!(grid.input(right, IntentSet::default()), None);
        assert_eq!(grid.focused(), 20);
        assert_eq!(
            grid.input(
                IntentSet::default().with(MenuIntent::Accept),
                IntentSet::default(),
            ),
            Some((target, DescriptionCommand::Delete))
        );
        assert_eq!(
            grid.input(
                IntentSet::default().with(MenuIntent::Right),
                IntentSet::default().with(MenuIntent::Right),
            ),
            None
        );
        assert_eq!(grid.focused(), 20);
    }

    #[test]
    fn pointer_focus_and_controller_back_share_the_typed_page_target() {
        let target = target();
        let mut grid = keyboard(target);
        grid.focus(20).unwrap();
        assert_eq!(grid.activate(), (target, DescriptionCommand::Delete));
        assert_eq!(
            grid.input(
                IntentSet::default().with(MenuIntent::Back),
                IntentSet::default(),
            ),
            Some((target, DescriptionCommand::Cancel))
        );
        assert_eq!(grid.focus(99), Err(DescriptionGridError::MissingKey(99)));
    }

    #[test]
    fn pointer_hit_map_uses_half_open_design_bounds_and_typed_key_ids() {
        let grid = keyboard(target());
        let hit_map = DescriptionKeyboardHitMap::new(
            grid.keys(),
            vec![
                DescriptionKeyboardHitTarget {
                    key: 10,
                    left: 0.0,
                    top: 0.0,
                    right: 20.0,
                    bottom: 10.0,
                },
                DescriptionKeyboardHitTarget {
                    key: 20,
                    left: 20.0,
                    top: 0.0,
                    right: 40.0,
                    bottom: 10.0,
                },
            ],
        )
        .unwrap();

        assert_eq!(hit_map.hit_test(0.0, 0.0), Some(10));
        assert_eq!(hit_map.hit_test(19.999, 9.999), Some(10));
        assert_eq!(hit_map.hit_test(20.0, 5.0), Some(20));
        assert_eq!(hit_map.hit_test(40.0, 5.0), None);
        assert_eq!(hit_map.hit_test(5.0, 10.0), None);
    }

    #[test]
    fn pointer_motion_focuses_and_left_press_activates_the_same_typed_key() {
        let target = target();
        let mut grid = keyboard(target);
        let hit_map = DescriptionKeyboardHitMap::new(
            grid.keys(),
            vec![
                DescriptionKeyboardHitTarget {
                    key: 10,
                    left: 0.0,
                    top: 0.0,
                    right: 20.0,
                    bottom: 10.0,
                },
                DescriptionKeyboardHitTarget {
                    key: 20,
                    left: 20.0,
                    top: 0.0,
                    right: 40.0,
                    bottom: 10.0,
                },
            ],
        )
        .unwrap();

        assert_eq!(grid.pointer(&hit_map, 22.0, 5.0, false), None);
        assert_eq!(grid.focused(), 20);
        assert_eq!(
            grid.pointer(&hit_map, 22.0, 5.0, true),
            Some((target, DescriptionCommand::Delete))
        );
    }

    #[test]
    fn pointer_hit_map_rejects_incomplete_ambiguous_and_invalid_geometry() {
        let grid = keyboard(target());
        let left = DescriptionKeyboardHitTarget {
            key: 10,
            left: 0.0,
            top: 0.0,
            right: 20.0,
            bottom: 10.0,
        };
        let right = DescriptionKeyboardHitTarget {
            key: 20,
            left: 19.0,
            top: 0.0,
            right: 40.0,
            bottom: 10.0,
        };

        assert_eq!(
            DescriptionKeyboardHitMap::new(grid.keys(), vec![left]),
            Err(DescriptionGridError::MissingHitTarget(20))
        );
        assert_eq!(
            DescriptionKeyboardHitMap::new(grid.keys(), vec![left, left]),
            Err(DescriptionGridError::DuplicateHitTarget(10))
        );
        assert_eq!(
            DescriptionKeyboardHitMap::new(grid.keys(), vec![left, right]),
            Err(DescriptionGridError::InvalidHitTarget(10))
        );
        assert_eq!(
            DescriptionKeyboardHitMap::new(
                grid.keys(),
                vec![
                    left,
                    DescriptionKeyboardHitTarget {
                        key: 20,
                        left: f32::NAN,
                        ..right
                    },
                ],
            ),
            Err(DescriptionGridError::InvalidHitTarget(20))
        );
    }

    #[test]
    fn caret_hit_map_binds_unicode_scalar_boundaries_to_text_snapshot() {
        let text = "A😀";
        let hit_map = DescriptionCaretHitMap::new(
            target(),
            text,
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

        assert_eq!(hit_map.target(), target());
        assert_eq!(hit_map.hit_test(0.0, 5.0), Some(0));
        assert_eq!(hit_map.hit_test(10.0, 5.0), Some(1));
        assert_eq!(hit_map.hit_test(29.9, 5.0), Some(2));
        assert_eq!(hit_map.hit_test(30.0, 5.0), None);
        assert_eq!(hit_map.hit_test(f32::NAN, 5.0), None);
        assert!(hit_map.matches_text(text));
        assert!(!hit_map.matches_text("A😀!"));
    }

    #[test]
    fn caret_hit_map_rejects_missing_duplicate_and_overlapping_boundaries() {
        let boundary = |index, left, right| DescriptionCaretHitTarget {
            index,
            left,
            top: 0.0,
            right,
            bottom: 10.0,
        };
        assert_eq!(
            DescriptionCaretHitMap::new(target(), "a", vec![boundary(0, 0.0, 10.0)]),
            Err(DescriptionGridError::MissingCaretIndex(1))
        );
        assert_eq!(
            DescriptionCaretHitMap::new(
                target(),
                "a",
                vec![
                    boundary(0, 0.0, 10.0),
                    boundary(0, 10.0, 20.0),
                    boundary(1, 20.0, 30.0)
                ],
            ),
            Err(DescriptionGridError::DuplicateCaretIndex(0))
        );
        assert_eq!(
            DescriptionCaretHitMap::new(
                target(),
                "a",
                vec![boundary(0, 0.0, 11.0), boundary(1, 10.0, 20.0)],
            ),
            Err(DescriptionGridError::InvalidCaretHitTarget(1))
        );
    }

    #[test]
    fn malformed_runtime_grid_is_rejected_before_input() {
        let target = target();
        assert_eq!(
            DescriptionKeyboardGrid::new(target, vec![], 1),
            Err(DescriptionGridError::Empty)
        );
        assert_eq!(
            DescriptionKeyboardGrid::new(
                target,
                vec![
                    DescriptionGridKey {
                        id: 1,
                        up: None,
                        down: None,
                        left: None,
                        right: None,
                        command: DescriptionCommand::Insert('a'),
                    },
                    DescriptionGridKey {
                        id: 1,
                        up: None,
                        down: None,
                        left: None,
                        right: None,
                        command: DescriptionCommand::Insert('b'),
                    },
                ],
                1,
            ),
            Err(DescriptionGridError::DuplicateKey(1))
        );
    }

    #[test]
    fn keyboard_characters_navigation_backspace_and_enter_are_native_commands() {
        let target = target();
        let emitted = commands(
            target,
            &['m', 'a', 'p'],
            IntentSet::default()
                .with(MenuIntent::Left)
                .with(MenuIntent::Backspace),
            IntentSet::default(),
        );
        assert_eq!(
            emitted,
            vec![
                (target, DescriptionCommand::Insert('m')),
                (target, DescriptionCommand::Insert('a')),
                (target, DescriptionCommand::Insert('p')),
                (target, DescriptionCommand::MoveLeft),
                (target, DescriptionCommand::Backspace),
            ]
        );

        assert_eq!(
            commands(
                target,
                &[],
                IntentSet::default().with(MenuIntent::Accept),
                IntentSet::default()
            ),
            vec![(target, DescriptionCommand::Commit)]
        );
    }

    #[test]
    fn back_cancels_without_forwarding_same_frame_text() {
        let target = target();
        assert_eq!(
            commands(
                target,
                &['x'],
                IntentSet::default().with(MenuIntent::Back),
                IntentSet::default(),
            ),
            vec![(target, DescriptionCommand::Cancel)]
        );
    }

    #[test]
    fn delete_and_navigation_are_generation_bound_commands() {
        let target = target();
        assert_eq!(
            commands_with_delete(
                target,
                &[],
                true,
                IntentSet::default().with(MenuIntent::Left),
                IntentSet::default(),
            ),
            vec![
                (target, DescriptionCommand::Delete),
                (target, DescriptionCommand::MoveLeft),
            ]
        );
    }

    #[test]
    fn back_wins_over_same_frame_text_and_delete() {
        let target = target();
        assert_eq!(
            commands_with_delete(
                target,
                &['x'],
                true,
                IntentSet::default().with(MenuIntent::Back),
                IntentSet::default(),
            ),
            vec![(target, DescriptionCommand::Cancel)]
        );
    }
}
