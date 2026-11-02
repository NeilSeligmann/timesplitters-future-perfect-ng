// SPDX-License-Identifier: GPL-3.0-or-later
//! T2169: type into the game's on-screen keyboard pages with the host keyboard. The C frame hook (`host_text_entry_frame`)
//! calls [`tsfp_text_entry_frame`] at every frame boundary with a READ-ONLY guest accessor; while the top page slot is a
//! known text-entry page, its widget words (grid slot, flags, cursor, text length, limit) are published here. Typed keys
//! (characters, Backspace, Enter, Escape) are swallowed before the plain key path and the input router.
//! T2310 (native text field): the typed keys go straight into the Rust text-field model
//! (`tsfp_ui_input::native_text`, `tsfp_text_field::TextField`) at the frame boundary ([`frame_service`], called from
//! `ui_live::tsfp_ui_frame_service`); the page's field record is its write-through view, Enter commits through the
//! original Done path 0x7C4F0, Escape is the framework's native back. The T2169 pad-tap typing shim (TypingDriver) is
//! gone from this page. The pad and every other key keep working. `TSFP_TEXT_KEYS=0` turns typing off.
//! Contract and evidence: docs/t2169-keyboard-text-entry.md, docs/t-ui-native-framework.md.
use crate::shared::{SHARED, lock};
use crate::window_input::RawEvent;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::prelude::*;
use std::collections::{HashSet, VecDeque};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tsfp_ui_input::native_page::GuestUi;
use tsfp_ui_input::native_text::{self, NativeTextField, TextCommand, TextQueue};
use tsfp_ui_input::text_entry::{GridLayout, LiveField, TextKey, char_to_key};

/// Page slots of the original UI page table (C `host_ui_page_snapshots`).
const SLOT_BASE: u32 = 0x6F_EBE8;
const SLOT_SIZE: u32 = 0x23C;
const SLOTS: u32 = 20;
/// Builders of the pages whose slot record is an on-screen keyboard widget (T2197 layout), with their grid. MEASURED:
/// the Change Players / Create New Profile Enter Name page (docs/t2169-keyboard-text-entry.md).
const TEXT_PAGES: &[(u32, GridLayout)] = &[(PROFILE_NAME_BUILDER, GridLayout::LETTERS)];
pub const PROFILE_NAME_BUILDER: u32 = 0x19_4980;

static LIVE: Mutex<Option<LiveField>> = Mutex::new(None);
static FOOTER_PAGE: AtomicU32 = AtomicU32::new(0);
/// T2310: typed keys for the native field, drained on the guest thread.
static QUEUE: std::sync::OnceLock<TextQueue> = std::sync::OnceLock::new();
/// The Rust-owned field of the live text page (guest thread).
static FIELD: Mutex<Option<NativeTextField>> = Mutex::new(None);
/// Keys applied, keys rejected, pages refused (stop line).
static COUNTS: Mutex<[u32; 3]> = Mutex::new([0; 3]);

fn queue() -> &'static TextQueue {
    QUEUE.get_or_init(TextQueue::default)
}

fn push(command: TextCommand) {
    if let Ok(mut q) = queue().lock() {
        q.push_back(command);
    }
}

/// Queue the generic text page's native Back contract after a Map Description
/// commit or cancel has already been applied to Rust-owned document state.
pub fn queue_native_back() {
    push(TextCommand::Cancel);
}

/// Queue one recorded text operation only while the same Rust-owned text page is active.
/// Page identity prevents a recording made in one naming flow from changing another.
pub fn replay_recorded(page: u32, event: &tsfp_input::recording::UiEvent) -> bool {
    if !active() || TOP_BUILDER.load(Ordering::Acquire) != page {
        return false;
    }
    let command = match event {
        tsfp_input::recording::UiEvent::TextInput('\u{8}') => TextCommand::Backspace,
        tsfp_input::recording::UiEvent::TextInput(ch) => TextCommand::Char(*ch),
        tsfp_input::recording::UiEvent::TextCommit { .. } => TextCommand::Confirm,
        tsfp_input::recording::UiEvent::TextCancel(_) => TextCommand::Cancel,
        tsfp_input::recording::UiEvent::Start => TextCommand::Confirm,
        _ => return false,
    };
    push(command);
    true
}

fn record_text_command(key: TextKey) {
    if !crate::input_recording::enabled() {
        return;
    }
    let page = TOP_BUILDER.load(Ordering::Acquire);
    let event = match key {
        TextKey::Char(byte) => tsfp_input::recording::UiEvent::TextInput(char::from(byte)),
        TextKey::Backspace => tsfp_input::recording::UiEvent::TextInput('\u{8}'),
        TextKey::Done => tsfp_input::recording::UiEvent::TextCommit {
            widget: 0,
            text: lock(&FIELD)
                .as_ref()
                .map(|field| field.field.text().to_vec())
                .unwrap_or_default(),
        },
        TextKey::Cancel => tsfp_input::recording::UiEvent::TextCancel(0),
    };
    if page != 0
        && let Err(error) = crate::input_recording::record_ui(page, 0, 0, event)
    {
        eprintln!("tsfp-bevy: text UI recording failed: {error}");
    }
}

/// Replay page-addressed text operations into the native field command queue.
pub fn replay_recorded_events(events: &[tsfp_input::recording::Ui]) {
    let page = TOP_BUILDER.load(Ordering::Acquire);
    if page == 0 || !active() {
        return;
    }
    for event in events.iter().filter(|event| event.page == page) {
        let _ = replay_recorded(page, &event.event);
    }
}

fn command_of(key: TextKey) -> TextCommand {
    match key {
        TextKey::Char(byte) => TextCommand::Char(char::from(byte)),
        TextKey::Backspace => TextCommand::Backspace,
        TextKey::Done => TextCommand::Confirm,
        TextKey::Cancel => TextCommand::Cancel,
    }
}

/// T2310: guest-thread frame boundary: apply the typed keys to the Rust field of the profile name page.
pub fn frame_service(guest: &mut dyn GuestUi) {
    let mut done = Vec::new();
    let mut field = lock(&FIELD);
    // T2310 increment 5: any text input widget page (string 0x7D090 or numeric 0x7D1B0, e.g. Map Maker names), not
    // only the profile name page.
    let page = native_text::top_text_page(&*guest, &[PROFILE_NAME_BUILDER]);
    FOOTER_PAGE.store(
        page.filter(|p| {
            matches!(p.state(guest), 1..=3)
                && (p.builder(guest) != PROFILE_NAME_BUILDER || field_row_attached(guest, *p))
        })
        .map_or(0, |p| p.0),
        Ordering::Release,
    );
    let refused = native_text::service_on(guest, page, queue(), &mut field, &mut done);
    let mut counts = lock(&COUNTS);
    if let Some(reason) = refused {
        counts[2] += 1;
        eprintln!("tsfp-bevy: native text field refused: {reason:?}");
    }
    for (command, outcome) in done {
        let rejected = matches!(outcome, native_text::TextOutcome::Rejected(_));
        counts[usize::from(rejected)] += 1;
        eprintln!("tsfp-bevy: native text {command:?} -> {outcome:?}");
    }
}
/// Key codes whose press was swallowed (their release is swallowed too).
static SWALLOWED: Mutex<Option<HashSet<KeyCode>>> = Mutex::new(None);
static DISABLED: AtomicBool = AtomicBool::new(false);
/// Builder of the top occupied page slot (0: none), for the diagnostic key script.
static TOP_BUILDER: AtomicU32 = AtomicU32::new(0);
static SCRIPT: Mutex<Option<KeyScript>> = Mutex::new(None);

type Read32 = unsafe extern "C" fn(u32, *mut u32) -> bool;

fn read32(read: Read32, address: u32) -> Option<u32> {
    let mut value = 0u32;
    // SAFETY: the C host's accessor checks the guest range and writes one u32.
    unsafe { read(address, &mut value) }.then_some(value)
}

fn read8(read: Read32, address: u32) -> Option<u8> {
    let word = read32(read, address & !3)?;
    Some((word >> ((address & 3) * 8)) as u8)
}

/// Text page of the top occupied slot, as a live field.
fn observe(read: Read32) -> Option<(LiveField, GridLayout)> {
    // The slot index is not the stack order (Map Maker Briefing: text entry in slot 1 below Map Settings in
    // slot 2, T3497): pick the highest live slot that is a text page, not just the highest occupied one.
    let mut top_builder = None;
    let mut found = None;
    for slot in (0..SLOTS).rev() {
        let base = SLOT_BASE + slot * SLOT_SIZE;
        if !read32(read, base).is_some_and(|d| d != 0) {
            continue;
        }
        let builder = read32(read, base + 8)?;
        top_builder.get_or_insert(builder);
        let id = read32(read, base + 4);
        let text = id.is_some_and(|id| native_text::TEXT_WIDGET_IDS.contains(&id))
            // Builder-listed pages (profile name keeps a retained buffer) count only as the top page.
            || (top_builder == Some(builder) && TEXT_PAGES.iter().any(|(b, _)| *b == builder));
        if text && read32(read, base + 0x220).is_some_and(|b| b != 0) {
            found = Some((base, builder));
            break;
        }
    }
    TOP_BUILDER.store(top_builder?, Ordering::Release);
    let (base, builder) = found?;
    let word = |offset: u32| read32(read, base + offset);
    // T2310 increment 5: a text input widget page (its id is the widget handler) is a text page too.
    let widget = word(4).is_some_and(|id| native_text::TEXT_WIDGET_IDS.contains(&id));
    let layout = TEXT_PAGES
        .iter()
        .find(|(b, _)| *b == builder)
        .map(|(_, layout)| *layout);
    let layout = if widget {
        layout.or(Some(GridLayout::LETTERS))
    } else {
        layout
    }?;
    // The slot holds the page before its keyboard widget exists (buffer 0): not a text field yet.
    let buffer = word(0x220).filter(|b| *b != 0)?;
    let (mut len, mut hash) = (0u32, 7u32);
    while len < 256 {
        let byte = read8(read, buffer + len)?;
        if byte == 0 {
            break;
        }
        hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
        len += 1;
    }
    Some((
        LiveField {
            builder,
            cell: layout.live_cell(word(0x184)?, word(0x188)?),
            flags: word(0x228)?,
            cursor: word(0x22C)?,
            len,
            limit: word(0x230)?,
            text_hash: hash,
        },
        layout,
    ))
}

/// Frame-boundary observation from the C host (guest thread). Read-only.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_text_entry_frame(read: Option<Read32>) {
    let Some(read) = read else { return };
    TOP_BUILDER.store(0, Ordering::Release);
    let field = observe(read).map(|(field, _layout)| field);
    let mut live = lock(&LIVE);
    if live.map(|l| l.builder) != field.map(|f| f.builder) {
        match field {
            Some(f) => eprintln!(
                "tsfp-bevy: text entry page 0x{:x} up (limit {}): type with the keyboard, Backspace deletes, Enter is Done, Esc backs",
                f.builder, f.limit
            ),
            None => eprintln!("tsfp-bevy: text entry page closed"),
        }
    }
    *live = field;
}

/// Whether a text page is live and typing is on.
pub fn active() -> bool {
    !DISABLED.load(Ordering::Acquire) && lock(&LIVE).is_some()
}

/// The active text-field page, if the native text handler owns a field now.
pub fn active_page() -> Option<u32> {
    if DISABLED.load(Ordering::Acquire) {
        return None;
    }
    lock(&LIVE).map(|field| field.builder)
}

/// The profile page reuses its record and retains BUFFER after leaving name mode.
/// The original width contract resolves field node id -2 (0x71940(widget,-2));
/// require that attached node rather than treating the retained buffer as active.
fn field_row_attached(g: &dyn GuestUi, page: tsfp_ui_input::menu_state::PageRef) -> bool {
    use tsfp_ui_input::menu_state::layout::{page as p, row};
    let mut node = g.read32(page.0 + p::FIRST_NODE).unwrap_or(0);
    let mut seen = Vec::new();
    let mut found = false;
    while node != 0 && seen.len() < 256 && !seen.contains(&node) {
        found |= g.read32(node + row::ID) == Some(u32::MAX - 1);
        seen.push(node);
        node = g.read32(node + row::NEXT).unwrap_or(0);
    }
    node == 0 && found
}

/// T2181: native string/numeric fields expose their existing Cancel handler to the footer.
pub fn footer_page() -> Option<u32> {
    let page = FOOTER_PAGE.load(Ordering::Acquire);
    (active() && page != 0).then_some(page)
}
pub fn pointer_cancel() {
    push(TextCommand::Cancel);
}

pub fn init() {
    if std::env::var("TSFP_TEXT_KEYS").is_ok_and(|v| v == "0") {
        DISABLED.store(true, Ordering::Release);
        eprintln!("tsfp-bevy: TSFP_TEXT_KEYS=0: keyboard typing into text pages is off");
    }
    if let Ok(path) = std::env::var("TSFP_TEXT_KEYS_SCRIPT") {
        match std::fs::read_to_string(&path)
            .map_err(|e| e.to_string())
            .and_then(|text| KeyScript::parse(&text))
        {
            Ok(script) => {
                eprintln!(
                    "tsfp-bevy: T2169 diagnostic key script {path}: {} steps",
                    script.steps.len()
                );
                *lock(&SCRIPT) = Some(script);
            }
            Err(message) => eprintln!("tsfp-bevy: T2169 key script {path} ignored: {message}"),
        }
    }
}

/// One step of the diagnostic key script (TSFP_TEXT_KEYS_SCRIPT, docs/t2169-keyboard-text-entry.md): it stands in for a
/// person at the keyboard where no input can be injected (headless weston). Typed keys take the same path as real ones.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Step {
    /// `until 0xBUILDER BUTTON`: tap BUTTON once a second until that page is the top slot (60 s at most).
    Until(u32, u8),
    /// `tap BUTTON`: one pad tap.
    Tap(u8),
    /// `wait-text`: wait until a text page is live.
    WaitText,
    /// `type TEXT`, `backspace`, `enter`, `escape`: keys as the keyboard would send them.
    Key(TextKey),
    /// `sleep MS`.
    Sleep(u64),
}

struct KeyScript {
    steps: VecDeque<Step>,
    next_at: u64,
    deadline: u64,
}

fn button_index(name: &str) -> Option<u8> {
    Some(match name {
        "A" => 0,
        "B" => 1,
        "X" => 2,
        "Y" => 3,
        "BACK" => 4,
        "START" => 6,
        "UP" => 11,
        "DOWN" => 12,
        "LEFT" => 13,
        "RIGHT" => 14,
        _ => return None,
    })
}

impl KeyScript {
    fn parse(text: &str) -> Result<Self, String> {
        let mut steps = VecDeque::new();
        for line in text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
        {
            let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
            let bad = || format!("bad line '{line}'");
            match word {
                "until" => {
                    let (page, button) = rest.split_once(' ').ok_or_else(bad)?;
                    let page = u32::from_str_radix(page.trim_start_matches("0x"), 16)
                        .map_err(|_| bad())?;
                    steps.push_back(Step::Until(
                        page,
                        button_index(button.trim()).ok_or_else(bad)?,
                    ));
                }
                "tap" => steps.push_back(Step::Tap(button_index(rest.trim()).ok_or_else(bad)?)),
                "wait-text" => steps.push_back(Step::WaitText),
                "type" => steps.extend(rest.chars().filter_map(char_to_key).map(Step::Key)),
                "backspace" => steps.push_back(Step::Key(TextKey::Backspace)),
                "enter" => steps.push_back(Step::Key(TextKey::Done)),
                "escape" => steps.push_back(Step::Key(TextKey::Cancel)),
                "sleep" => steps.push_back(Step::Sleep(rest.trim().parse().map_err(|_| bad())?)),
                _ => return Err(bad()),
            }
        }
        Ok(Self {
            steps,
            next_at: 0,
            deadline: 0,
        })
    }

    /// Runs due steps; pad taps go to `taps` as (button, press) pairs.
    fn tick(&mut self, now: u64, taps: &mut Vec<(u8, bool)>) {
        while now >= self.next_at {
            let Some(step) = self.steps.front().cloned() else {
                return;
            };
            match step {
                Step::Until(page, button) => {
                    if self.deadline == 0 {
                        self.deadline = now + 60_000;
                    }
                    if TOP_BUILDER.load(Ordering::Acquire) == page || now > self.deadline {
                        eprintln!(
                            "tsfp-bevy: T2169 script: page 0x{page:x} {}",
                            if now > self.deadline {
                                "NOT reached"
                            } else {
                                "reached"
                            }
                        );
                        self.deadline = 0;
                        self.steps.pop_front();
                        self.next_at = now + 500;
                        continue;
                    }
                    taps.push((button, true));
                    taps.push((button, false));
                    self.next_at = now + 1_000;
                    return;
                }
                Step::Tap(button) => {
                    taps.push((button, true));
                    taps.push((button, false));
                    self.steps.pop_front();
                    self.next_at = now + 1_500;
                    return;
                }
                Step::WaitText => {
                    if !active() {
                        return;
                    }
                    self.steps.pop_front();
                    self.next_at = now + 500;
                }
                Step::Key(key) => {
                    eprintln!("tsfp-bevy: T2169 script: key {key:?}");
                    push(command_of(key));
                    self.steps.pop_front();
                    self.next_at = now + 30;
                }
                Step::Sleep(ms) => {
                    self.steps.pop_front();
                    self.next_at = now + ms;
                }
            }
        }
    }
}

/// The typed key of one keyboard event, `None` for keys that keep their normal meaning.
fn text_key(event: &KeyboardInput) -> Option<TextKey> {
    match tsfp_ui_input::keyboard::menu_intent(crate::events::sdl_keycode(event.key_code)) {
        Some(tsfp_ui_input::MenuIntent::Accept) => return Some(TextKey::Done),
        Some(tsfp_ui_input::MenuIntent::Back) if event.key_code == KeyCode::Escape => {
            return Some(TextKey::Cancel);
        }
        _ => {}
    }
    match &event.logical_key {
        Key::Backspace | Key::Delete => Some(TextKey::Backspace),
        Key::Space => Some(TextKey::Char(b' ')),
        Key::Character(text) => {
            let mut chars = text.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            char_to_key(c)
        }
        _ => None,
    }
}

/// Called by the plain key path for every keyboard event: `true` when the event belongs to the text page and must not
/// reach the guest's keyboard mapping or the input router.
pub fn swallow(event: &KeyboardInput) -> bool {
    let mut swallowed = lock(&SWALLOWED);
    let set = swallowed.get_or_insert_with(HashSet::new);
    match event.state {
        ButtonState::Pressed => {
            if !active() {
                return false;
            }
            let Some(key) = text_key(event) else {
                return false;
            };
            set.insert(event.key_code);
            record_text_command(key);
            push(command_of(key));
            true
        }
        ButtonState::Released => set.remove(&event.key_code),
    }
}

/// PreUpdate: the diagnostic key script (typed keys themselves are applied natively at the frame boundary).
pub fn text_entry_tick() {
    crate::ui_back::diagnostic_text(crate::shared::now_ms());
    script_tick();
}

/// The script's own taps: pressed now, released 100 ms later.
static SCRIPT_RELEASE: Mutex<Vec<(u8, u64)>> = Mutex::new(Vec::new());

fn script_tick() {
    let now = crate::shared::ui_now_ms();
    let mut taps = Vec::new();
    if let Some(script) = lock(&SCRIPT).as_mut() {
        script.tick(now, &mut taps);
    }
    let mut release = lock(&SCRIPT_RELEASE);
    let mut wi = lock(&SHARED.wi);
    for (button, press) in taps {
        if press {
            wi.feed(RawEvent::button_down(None, button));
            release.push((button, now + 100));
        }
    }
    release.retain(|(button, at)| {
        if now >= *at {
            wi.feed(RawEvent::button_up(None, *button));
            return false;
        }
        true
    });
}

/// Stop line.
pub fn report() {
    let [applied, rejected, refused] = *lock(&COUNTS);
    eprintln!(
        "tsfp-bevy: text entry: native keys applied={applied} rejected={rejected} pages_refused={refused} taps=0"
    );
}

#[cfg(test)]
mod keyboard_standard_tests {
    use super::*;
    #[test]
    fn text_fields_commit_cancel_and_type_spaces() {
        for (key_code, logical_key, expected) in [
            (KeyCode::Enter, Key::Enter, TextKey::Done),
            (KeyCode::NumpadEnter, Key::Enter, TextKey::Done),
            (KeyCode::Escape, Key::Escape, TextKey::Cancel),
            (KeyCode::Space, Key::Space, TextKey::Char(b' ')),
            (KeyCode::Backspace, Key::Backspace, TextKey::Backspace),
        ] {
            let event = KeyboardInput {
                key_code,
                logical_key,
                state: ButtonState::Pressed,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            };
            assert_eq!(text_key(&event), Some(expected));
        }
    }
}

#[cfg(test)]
mod footer_field_tests {
    use super::*;
    use std::collections::HashMap;
    struct Memory(HashMap<u32, u32>);
    impl GuestUi for Memory {
        fn read32(&self, a: u32) -> Option<u32> {
            self.0.get(&a).copied()
        }
        fn write32(&mut self, _: u32, _: u32) -> bool {
            panic!("classifier must be read-only")
        }
        fn call(&mut self, _: u32, _: &[u32]) -> Option<u32> {
            panic!("classifier must not call a handler")
        }
    }
    #[test]
    fn retained_buffer_does_not_classify_profile_list_as_field_and_cycles_fail_closed() {
        use tsfp_ui_input::menu_state::layout::{page as p, row};
        let page = tsfp_ui_input::menu_state::PageRef::slot(3);
        let node = 0x1000;
        let mut g = Memory(HashMap::from([
            (page.0 + p::FIRST_NODE, node),
            (page.0 + native_text::field::BUFFER, 0x8000),
            (node + row::ID, 1),
            (node + row::NEXT, 0),
        ]));
        assert!(!field_row_attached(&g, page));
        g.0.insert(node + row::ID, u32::MAX - 1);
        assert!(field_row_attached(&g, page));
        g.0.insert(node + row::NEXT, node);
        assert!(!field_row_attached(&g, page));
    }
}
