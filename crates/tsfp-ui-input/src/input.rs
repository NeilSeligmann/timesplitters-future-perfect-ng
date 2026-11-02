//! UiInput: intents, pointer and text to UiEvents, with per-context player ownership.

use crate::bindings::ListId;
use crate::layout::{Dir, Snap, UiLayout, WidgetId, hit_snap, navigate, snapshot};
use crate::pointer::{PointerFrame, PointerPolicy, UiPointerState};
use crate::repeat::{Hold, RepeatPolicy};
use tsfp_text_field::{Event as TfEvent, Outcome, TextField, TextWidth};

struct UnconstrainedWidth;
impl TextWidth for UnconstrainedWidth {
    fn fits(&mut self, _: &[u8]) -> bool {
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlayerId(pub u8);
/// A page or split-screen viewport context.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContextId(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuIntent {
    Up,
    Down,
    Left,
    Right,
    Accept,
    Back,
    Start,
    PageUp,
    PageDown,
    Tab,
    BackTab,
    /// Keyboard Backspace: Back outside text entry, delete inside.
    Backspace,
    /// T3770: the page's delete action (pad Y, keyboard Delete), e.g. delete the profile under the cursor.
    Delete,
    /// T3817: the page's X shortcut (pad X, keyboard X), the front-end "Player Progress" prompt.
    Shortcut,
}

impl MenuIntent {
    const ALL: [MenuIntent; 14] = [
        Self::Up,
        Self::Down,
        Self::Left,
        Self::Right,
        Self::Accept,
        Self::Back,
        Self::Start,
        Self::PageUp,
        Self::PageDown,
        Self::Tab,
        Self::BackTab,
        Self::Backspace,
        Self::Delete,
        Self::Shortcut,
    ];
    fn bit(self) -> u16 {
        1 << (self as u16)
    }
    fn repeats(self) -> bool {
        matches!(
            self,
            Self::Up
                | Self::Down
                | Self::Left
                | Self::Right
                | Self::PageUp
                | Self::PageDown
                | Self::Tab
                | Self::BackTab
                | Self::Backspace
        )
    }
    /// Adapter from the tsfp-input action layer (additive, no change there).
    pub fn from_input(i: tsfp_input::actions::Intent) -> Option<Self> {
        use tsfp_input::actions::Intent as I;
        Some(match i {
            I::MenuAccept => Self::Accept,
            I::MenuBack => Self::Back,
            I::MenuUp => Self::Up,
            I::MenuDown => Self::Down,
            I::MenuLeft => Self::Left,
            I::MenuRight => Self::Right,
            I::Pause => Self::Start,
            _ => return None,
        })
    }
}

/// Held intents as a bitset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IntentSet(u16);
impl IntentSet {
    pub fn with(mut self, i: MenuIntent) -> Self {
        self.0 |= i.bit();
        self
    }
    pub fn contains(&self, i: MenuIntent) -> bool {
        self.0 & i.bit() != 0
    }
}

/// T3414: one press edge acts on exactly one page. Edge detection compares held intents with the
/// previous frame, but each page (main menu input context, native builder pages) kept its own
/// "previous" and the native one was reset to empty while no native page was live. A key still
/// held when a new page took input therefore looked like a fresh press on that page: Accept opened
/// a child and then its grandchild, Back popped two levels. The gate is fed every frame with the
/// page that owns input; when that owner changes, every intent held at that moment is swallowed
/// until it is released, so the new page sees only presses that start on it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PageEdgeGate {
    owner: Option<u32>,
    swallowed: IntentSet,
}

impl PageEdgeGate {
    /// Held intents the current `owner` page may see this frame.
    pub fn filter(&mut self, owner: Option<u32>, held: IntentSet) -> IntentSet {
        if owner != self.owner {
            self.owner = owner;
            self.swallowed = IntentSet(self.swallowed.0 | held.0);
        }
        self.swallowed = IntentSet(self.swallowed.0 & held.0);
        IntentSet(held.0 & !self.swallowed.0)
    }
}

/// One player's input for a frame: held intents plus typed characters.
#[derive(Clone, Debug, Default)]
pub struct PlayerFrame {
    pub held: IntentSet,
    pub chars: Vec<char>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ownership {
    /// Any player drives the context (single-player pages, shared menus).
    Any,
    /// Only this player (split-screen viewport page).
    Player(PlayerId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UiEvent {
    Focus(WidgetId),
    Activate(WidgetId),
    Back,
    Start,
    Scroll(ListId, i32),
    Hover(WidgetId),
    HoverLeave,
    TextInput(char),
    TextCommit { widget: WidgetId, text: Vec<u8> },
    TextCancel(WidgetId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Routed {
    pub context: ContextId,
    pub player: PlayerId,
    pub event: UiEvent,
}

#[derive(Clone, Debug)]
pub struct TextEntrySpec {
    pub initial: Vec<u8>,
    pub limit: usize,
    pub charset: Vec<u8>,
}

struct ActiveText {
    widget: WidgetId,
    field: TextField,
}

struct Ctx {
    owner: Ownership,
    focus: Option<WidgetId>,
    holds: [[Hold; MenuIntent::ALL.len()]; 4],
    prev: [IntentSet; 4],
    pointer: UiPointerState,
    text: Option<ActiveText>,
}

pub const MAX_PLAYERS: usize = 4;

/// The device-to-UI router. One instance per UI host, contexts added per page/viewport.
pub struct UiInput {
    pub repeat: RepeatPolicy,
    pub pointer_policy: PointerPolicy,
    pub wrap_focus: bool,
    pub page_size: i32,
    /// Player whose pointer device drives pointer events.
    pub pointer_player: PlayerId,
    ctxs: Vec<(ContextId, Ctx)>,
}

impl Default for UiInput {
    fn default() -> Self {
        Self {
            repeat: RepeatPolicy::default(),
            pointer_policy: PointerPolicy::default(),
            wrap_focus: true,
            page_size: 5,
            pointer_player: PlayerId(0),
            ctxs: Vec::new(),
        }
    }
}

impl UiInput {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_context(&mut self, id: ContextId, owner: Ownership) {
        self.ctxs.retain(|(c, _)| *c != id);
        self.ctxs.push((
            id,
            Ctx {
                owner,
                focus: None,
                holds: [[Hold::default(); MenuIntent::ALL.len()]; 4],
                prev: [IntentSet::default(); 4],
                pointer: UiPointerState::default(),
                text: None,
            },
        ));
    }
    pub fn close_context(&mut self, id: ContextId) {
        self.ctxs.retain(|(c, _)| *c != id);
    }
    fn ctx(&mut self, id: ContextId) -> Option<&mut Ctx> {
        self.ctxs.iter_mut().find(|(c, _)| *c == id).map(|(_, x)| x)
    }
    pub fn focus(&self, id: ContextId) -> Option<WidgetId> {
        self.ctxs
            .iter()
            .find(|(c, _)| *c == id)
            .and_then(|(_, x)| x.focus)
    }
    pub fn set_focus(&mut self, id: ContextId, w: Option<WidgetId>) {
        if let Some(c) = self.ctx(id) {
            c.focus = w;
        }
    }
    pub fn pointer_state(&self, id: ContextId) -> Option<UiPointerState> {
        self.ctxs
            .iter()
            .find(|(c, _)| *c == id)
            .map(|(_, x)| x.pointer)
    }
    /// Output for pointer capture/release (T2180).
    pub fn wants_pointer(&self, id: ContextId) -> bool {
        self.pointer_state(id).is_some_and(|p| p.visible)
    }
    pub fn text_active(&self, id: ContextId) -> bool {
        self.ctxs
            .iter()
            .find(|(c, _)| *c == id)
            .is_some_and(|(_, x)| x.text.is_some())
    }
    /// The model starts a text edit for `widget` (usually after `Activate` on a text field).
    pub fn begin_text(&mut self, id: ContextId, widget: WidgetId, spec: &TextEntrySpec) -> bool {
        let Ok(field) = TextField::new(&spec.initial, spec.limit, &spec.charset) else {
            return false;
        };
        self.begin_text_field(id, widget, field)
    }
    /// Start a text edit with a field the page already configured (runtime charset, limit,
    /// select-all), e.g. `NameEntry::field` built from the original builder's spec.
    pub fn begin_text_field(&mut self, id: ContextId, widget: WidgetId, field: TextField) -> bool {
        match self.ctx(id) {
            Some(c) => {
                c.text = Some(ActiveText { widget, field });
                true
            }
            None => false,
        }
    }

    /// Run one frame for a context. `players` is indexed by player id; players that do not own the
    /// context are ignored. Events are appended to `out`.
    pub fn update(
        &mut self,
        id: ContextId,
        now_ms: u64,
        layout: &dyn UiLayout,
        players: &[PlayerFrame],
        pointer: Option<&PointerFrame>,
        out: &mut Vec<Routed>,
    ) {
        self.update_with_text_width(
            id,
            now_ms,
            layout,
            players,
            pointer,
            &mut UnconstrainedWidth,
            out,
        );
    }

    /// Route text through the field's font-width contract. The caller supplies runtime
    /// font metrics; this crate never guesses widths from byte length.
    #[allow(clippy::too_many_arguments)]
    pub fn update_with_text_width(
        &mut self,
        id: ContextId,
        now_ms: u64,
        layout: &dyn UiLayout,
        players: &[PlayerFrame],
        pointer: Option<&PointerFrame>,
        width: &mut dyn TextWidth,
        out: &mut Vec<Routed>,
    ) {
        let (repeat, ppol, wrap, page, ptr_player) = (
            self.repeat.clone(),
            self.pointer_policy.clone(),
            self.wrap_focus,
            self.page_size,
            self.pointer_player,
        );
        let Some(c) = self.ctx(id) else { return };
        let snaps = snapshot(layout);
        let mut emit = |player: PlayerId, event: UiEvent| {
            out.push(Routed {
                context: id,
                player,
                event,
            })
        };
        let owns = |o: Ownership, p: PlayerId| match o {
            Ownership::Any => true,
            Ownership::Player(q) => q == p,
        };

        for (i, frame) in players.iter().enumerate().take(MAX_PLAYERS) {
            let player = PlayerId(i as u8);
            if !owns(c.owner, player) {
                continue;
            }
            for chars_ch in &frame.chars {
                if let Some(t) = c.text.as_mut() {
                    if t.field.character_with_width(*chars_ch, width).is_ok() {
                        emit(player, UiEvent::TextInput(*chars_ch));
                    }
                }
            }
            for intent in MenuIntent::ALL {
                let k = intent as usize;
                let held = frame.held.contains(intent);
                let was = c.prev[i].contains(intent);
                let mut fires = 0;
                if held && !was {
                    fires = 1;
                    if intent.repeats() {
                        c.holds[i][k].press(now_ms, &repeat);
                    }
                    if c.pointer.visible {
                        c.pointer.visible = false;
                    }
                } else if held && intent.repeats() {
                    fires = c.holds[i][k].due(now_ms, &repeat);
                } else if !held {
                    c.holds[i][k].active = false;
                }
                for _ in 0..fires {
                    step(c, &snaps, layout, wrap, page, player, intent, &mut emit);
                }
            }
            c.prev[i] = frame.held;
        }

        if let Some(p) = pointer {
            if owns(c.owner, ptr_player) {
                pointer_step(c, &snaps, &ppol, now_ms, p, ptr_player, &mut emit);
            }
        }
        if c.pointer.visible
            && now_ms.saturating_sub(c.pointer.last_activity_ms) > ppol.idle_hide_ms
        {
            c.pointer.visible = false;
        }
    }
}

fn set_focus(c: &mut Ctx, player: PlayerId, w: WidgetId, emit: &mut dyn FnMut(PlayerId, UiEvent)) {
    if c.focus != Some(w) {
        c.focus = Some(w);
        emit(player, UiEvent::Focus(w));
    }
}

fn first_usable(snaps: &[Snap]) -> Option<WidgetId> {
    snaps.iter().find(|s| s.usable).map(|s| s.id)
}

fn cycle(snaps: &[Snap], from: Option<WidgetId>, forward: bool) -> Option<WidgetId> {
    let order: Vec<WidgetId> = snaps.iter().filter(|s| s.usable).map(|s| s.id).collect();
    if order.is_empty() {
        return None;
    }
    let pos = from.and_then(|f| order.iter().position(|w| *w == f));
    let n = order.len();
    Some(match (pos, forward) {
        (None, true) => order[0],
        (None, false) => order[n - 1],
        (Some(p), true) => order[(p + 1) % n],
        (Some(p), false) => order[(p + n - 1) % n],
    })
}

#[allow(clippy::too_many_arguments)]
fn step(
    c: &mut Ctx,
    snaps: &[Snap],
    layout: &dyn UiLayout,
    wrap: bool,
    page: i32,
    player: PlayerId,
    intent: MenuIntent,
    emit: &mut dyn FnMut(PlayerId, UiEvent),
) {
    if let Some(t) = c.text.as_mut() {
        let widget = t.widget;
        let ev = match intent {
            MenuIntent::Left => Some(TfEvent::Left),
            MenuIntent::Right => Some(TfEvent::Right),
            MenuIntent::Backspace => Some(TfEvent::Backspace),
            MenuIntent::Accept => Some(TfEvent::Confirm),
            MenuIntent::Back => Some(TfEvent::Cancel),
            _ => None,
        };
        if let Some(e) = ev {
            let _ = t.field.apply(e);
        }
        match t.field.outcome() {
            Outcome::Confirmed => {
                let text = t.field.text().to_vec();
                c.text = None;
                emit(player, UiEvent::TextCommit { widget, text });
            }
            Outcome::Cancelled => {
                c.text = None;
                emit(player, UiEvent::TextCancel(widget));
            }
            Outcome::Editing => {}
        }
        return;
    }
    let dir = match intent {
        MenuIntent::Up => Some(Dir::Up),
        MenuIntent::Down => Some(Dir::Down),
        MenuIntent::Left => Some(Dir::Left),
        MenuIntent::Right => Some(Dir::Right),
        _ => None,
    };
    if let Some(d) = dir {
        match c.focus {
            None => {
                if let Some(w) = first_usable(snaps) {
                    set_focus(c, player, w, emit);
                }
            }
            Some(f) => {
                if let Some(w) = navigate(layout, snaps, f, d, wrap) {
                    set_focus(c, player, w, emit);
                }
            }
        }
        return;
    }
    match intent {
        MenuIntent::Accept => {
            if let Some(f) = c.focus {
                if snaps.iter().any(|s| s.id == f && s.enabled) {
                    emit(player, UiEvent::Activate(f));
                }
            }
        }
        MenuIntent::Back | MenuIntent::Backspace => emit(player, UiEvent::Back),
        MenuIntent::Start => emit(player, UiEvent::Start),
        MenuIntent::Tab | MenuIntent::BackTab => {
            if let Some(w) = cycle(snaps, c.focus, intent == MenuIntent::Tab) {
                set_focus(c, player, w, emit);
            }
        }
        MenuIntent::PageUp | MenuIntent::PageDown => {
            let list = c
                .focus
                .and_then(|f| snaps.iter().find(|s| s.id == f))
                .and_then(|s| s.list)
                .or_else(|| snaps.iter().find_map(|s| s.list));
            if let Some(l) = list {
                let d = if intent == MenuIntent::PageUp {
                    -page
                } else {
                    page
                };
                emit(player, UiEvent::Scroll(l, d));
            }
        }
        _ => {}
    }
}

fn pointer_step(
    c: &mut Ctx,
    snaps: &[Snap],
    pol: &PointerPolicy,
    now: u64,
    p: &PointerFrame,
    player: PlayerId,
    emit: &mut dyn FnMut(PlayerId, UiEvent),
) {
    let ptr = &mut c.pointer;
    let moved = p.pos.is_some() && p.pos != ptr.last_pos;
    let pressed = p.buttons != ptr.prev || p.wheel != 0;
    if moved || pressed {
        ptr.visible = true;
        ptr.last_activity_ms = now;
    }
    if let Some(t) = c.text.as_mut() {
        // Text entry owns input: only right click leaves it.
        if p.buttons.right && !ptr.prev.right {
            let w = t.widget;
            c.text = None;
            emit(player, UiEvent::TextCancel(w));
        }
        ptr.prev = p.buttons;
        ptr.last_pos = p.pos.or(ptr.last_pos);
        return;
    }
    let under = p.pos.and_then(|(x, y)| hit_snap(snaps, x, y));
    if moved {
        let new = under.map(|s| s.id);
        if new != ptr.hovered {
            match new {
                Some(w) => {
                    emit(player, UiEvent::Hover(w));
                    if pol.hover_focus && under.is_some_and(|s| s.usable) {
                        set_focus(c, player, w, emit);
                    }
                }
                None => emit(player, UiEvent::HoverLeave),
            }
            c.pointer.hovered = new;
        }
    }
    let ptr = &mut c.pointer;
    if p.buttons.left && !ptr.prev.left {
        ptr.pressed_left = under.filter(|s| s.usable).map(|s| s.id);
    }
    if !p.buttons.left && ptr.prev.left {
        if let (Some(down), Some(u)) = (ptr.pressed_left.take(), under) {
            if down == u.id {
                emit(player, UiEvent::Activate(down));
            }
        }
    }
    if p.buttons.right && !ptr.prev.right {
        emit(player, UiEvent::Back);
    }
    if p.wheel != 0 {
        let list = under.and_then(|s| s.list).or_else(|| {
            c.focus
                .and_then(|f| snaps.iter().find(|s| s.id == f))
                .and_then(|s| s.list)
        });
        if let Some(l) = list {
            emit(player, UiEvent::Scroll(l, -p.wheel * pol.wheel_step));
        }
    }
    c.pointer.prev = p.buttons;
    if p.pos.is_some() {
        c.pointer.last_pos = p.pos;
    }
}

#[cfg(test)]
mod t3414_page_edge_gate {
    use super::*;

    fn edge(held: IntentSet, before: IntentSet, i: MenuIntent) -> bool {
        held.contains(i) && !before.contains(i)
    }

    /// Simulates pages keyed by owner id, each with its own edge memory reset on entry (as the
    /// host does), and counts how many pages act on one Accept held over a page change.
    fn accepts_over_page_change(gated: bool) -> usize {
        let accept = IntentSet::default().with(MenuIntent::Accept);
        let mut gate = PageEdgeGate::default();
        let mut before = IntentSet::default();
        let mut owner_before = None;
        let mut acted = 0;
        // frames: main menu live, Accept pressed and held 6 frames, gap frame, child live.
        let owners = [Some(1), Some(1), Some(1), None, Some(2), Some(2), Some(2)];
        let helds = [
            IntentSet::default(),
            accept,
            accept,
            accept,
            accept,
            accept,
            IntentSet::default(),
        ];
        for (owner, raw) in owners.into_iter().zip(helds) {
            if owner != owner_before {
                before = IntentSet::default();
                owner_before = owner;
            }
            let held = if gated { gate.filter(owner, raw) } else { raw };
            if owner.is_some() && edge(held, before, MenuIntent::Accept) {
                acted += 1;
            }
            before = held;
        }
        acted
    }

    #[test]
    fn held_accept_acts_on_one_page_only() {
        assert_eq!(
            accepts_over_page_change(false),
            2,
            "control: ungated double action"
        );
        assert_eq!(accepts_over_page_change(true), 1);
    }

    #[test]
    fn held_back_pops_one_level() {
        let back = IntentSet::default().with(MenuIntent::Back);
        let mut gate = PageEdgeGate::default();
        gate.filter(Some(3), IntentSet::default());
        assert!(gate.filter(Some(3), back).contains(MenuIntent::Back));
        // parent page takes over while Back is still held: swallowed, also on later frames.
        assert!(!gate.filter(Some(2), back).contains(MenuIntent::Back));
        assert!(!gate.filter(Some(2), back).contains(MenuIntent::Back));
        // release then press again: a real new press reaches the parent.
        assert!(
            !gate
                .filter(Some(2), IntentSet::default())
                .contains(MenuIntent::Back)
        );
        assert!(gate.filter(Some(2), back).contains(MenuIntent::Back));
    }

    #[test]
    fn other_intents_and_new_presses_pass() {
        let accept = IntentSet::default().with(MenuIntent::Accept);
        let both = accept.with(MenuIntent::Down);
        let mut gate = PageEdgeGate::default();
        gate.filter(Some(1), accept);
        // Down pressed on the frame the child goes live together with a held Accept.
        let seen = gate.filter(Some(2), accept);
        assert!(!seen.contains(MenuIntent::Accept));
        let seen = gate.filter(Some(2), both);
        assert!(seen.contains(MenuIntent::Down));
        assert!(!seen.contains(MenuIntent::Accept));
    }
}
