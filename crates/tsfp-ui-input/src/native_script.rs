// SPDX-License-Identifier: GPL-3.0-or-later
//! Native UI script diagnostics shared by the trace host and Bevy shell (T2319).
//! Opt-in with TSFP_UI_NATIVE_SCRIPT/TSFP_UI_NATIVE_LOG. Applies the existing native
//! page/text handlers; never forces device or storage state. Input is FABRICATED.
use std::sync::{Mutex, OnceLock};

use crate::menu_state::MenuState;
use crate::native_page::{self, CommandQueue, GuestUi, NativeCommand, PAGE_STRIDE, PAGE_TABLE};
use crate::native_text::{self, NativeTextField, TextCommand, TextQueue};
use crate::widgets::Step;

/// One scripted action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Step(Step),
    Accept,
    Back,
    /// T3817: pad X, the page's X shortcut.
    Shortcut,
    /// Diagnostic input for the existing native host Close command.
    Close,
    Focus(u32),
    Select(u32),
    Adjust {
        row: u32,
        right: bool,
    },
    Wait(u32),
    Page(u32),
    Text(TextCommand),
    /// T1916: move the Map Maker pointer (the shell's mouse path).
    Point {
        x: u32,
        y: u32,
    },
}

/// Parses a script; errors name the line.
pub fn parse(text: &str) -> Result<Vec<Action>, String> {
    let mut out = Vec::new();
    for (number, raw) in text.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let int = |at: usize| -> Result<u32, String> {
            words
                .get(at)
                .and_then(|w| w.parse().ok())
                .ok_or_else(|| format!("line {}: expected a number in {line:?}", number + 1))
        };
        if words[0].eq_ignore_ascii_case("type") {
            let rest = line[4..].trim_start();
            out.extend(rest.chars().map(|c| Action::Text(TextCommand::Char(c))));
            continue;
        }
        let action = match words[0].to_ascii_lowercase().as_str() {
            "backspace" => Action::Text(TextCommand::Backspace),
            "enter" => Action::Text(TextCommand::Confirm),
            "escape" => Action::Text(TextCommand::Cancel),
            "up" => Action::Step(Step::Up),
            "down" => Action::Step(Step::Down),
            "left" => Action::Step(Step::Left),
            "right" => Action::Step(Step::Right),
            "a" => Action::Accept,
            "b" => Action::Back,
            "x" => Action::Shortcut,
            "close" => Action::Close,
            "focus" => Action::Focus(int(1)?),
            "select" => Action::Select(int(1)?),
            "wait" => Action::Wait(int(1)?),
            "point" => Action::Point {
                x: int(1)?,
                y: int(2)?,
            },
            "page" => Action::Page(
                words
                    .get(1)
                    .and_then(|w| u32::from_str_radix(w.trim_start_matches("0x"), 16).ok())
                    .ok_or_else(|| format!("line {}: page HEX", number + 1))?,
            ),
            "adjust" => Action::Adjust {
                row: int(1)?,
                right: match words.get(2).copied() {
                    Some("right") => true,
                    Some("left") => false,
                    _ => return Err(format!("line {}: adjust N left|right", number + 1)),
                },
            },
            other => return Err(format!("line {}: unknown action {other:?}", number + 1)),
        };
        out.push(action);
    }
    Ok(out)
}

struct Runner {
    actions: Vec<Action>,
    next: usize,
    gap: u32,
    hold: u32,
    frame: u64,
    log: bool,
    before: MenuState,
    queue: CommandQueue,
    text_queue: TextQueue,
    text_field: Option<NativeTextField>,
    /// T1916 increment 3: the builder of the last `page` reached; the actions go to that page while it takes input
    /// (the Map Maker text entry can open in a lower slot than the page it was opened from).
    target: Option<u32>,
}

static RUNNER: OnceLock<Option<Mutex<Runner>>> = OnceLock::new();

fn runner() -> Option<&'static Mutex<Runner>> {
    RUNNER
        .get_or_init(|| {
            let log = std::env::var("TSFP_UI_NATIVE_LOG").is_ok_and(|v| v == "1");
            let actions = match std::env::var("TSFP_UI_NATIVE_SCRIPT") {
                Ok(path) => match std::fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|text| parse(&text))
                {
                    Ok(actions) => actions,
                    Err(why) => {
                        eprintln!("ui-native: script {path}: {why}; native script off");
                        Vec::new()
                    }
                },
                Err(_) => Vec::new(),
            };
            if actions.is_empty() && !log {
                return None;
            }
            let gap = std::env::var("TSFP_UI_NATIVE_GAP")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20);
            eprintln!(
                "ui-native: {} scripted actions, gap {gap} frames",
                actions.len()
            );
            Some(Mutex::new(Runner {
                log: log || !actions.is_empty(),
                actions,
                next: 0,
                gap,
                hold: 0,
                frame: 0,
                before: MenuState::default(),
                queue: CommandQueue::default(),
                text_queue: TextQueue::default(),
                text_field: None,
                target: None,
            }))
        })
        .as_ref()
}

impl Runner {
    fn tick(&mut self, g: &mut dyn GuestUi) {
        self.frame += 1;
        let now = MenuState::load(g);
        if self.log {
            for event in self.before.events(&now) {
                eprintln!("ui-native: frame {} event {event:?}", self.frame);
            }
        }
        self.before = now;
        if self.hold > 0 {
            self.hold -= 1;
            return;
        }
        let Some(&action) = self.actions.get(self.next) else {
            return;
        };
        if let Action::Wait(frames) = action {
            self.hold = frames;
            self.next += 1;
            return;
        }
        let accepting = |builder: Option<u32>| {
            self.before
                .pages
                .iter()
                .rev()
                .find(|p| p.accepts_input() && builder.is_none_or(|b| p.builder == b))
        };
        let wanted = match action {
            Action::Page(want) => Some(want),
            _ => self.target,
        };
        let Some(top) = wanted
            .and_then(|b| accepting(Some(b)))
            .or_else(|| accepting(None))
        else {
            return;
        };
        let builder = top.builder;
        if let Action::Page(want) = action {
            if builder == want {
                eprintln!("ui-native: frame {} page {want:#x} reached", self.frame);
                self.target = Some(want);
                self.next += 1;
            }
            return;
        }
        if let Action::Point { x, y } = action {
            if crate::editor_pointer::write(g, x as f32, y as f32) {
                eprintln!(
                    "ui-native: frame {} page {builder:#x} pointer -> ({x}, {y})",
                    self.frame
                );
                self.next += 1;
                self.hold = self.gap;
            }
            return;
        }
        if let Action::Text(command) = action {
            if let Ok(mut queue) = self.text_queue.lock()
                && queue.is_empty()
            {
                queue.push_back(command);
            }
            let mut done = Vec::new();
            if let Some(refused) = native_text::service(
                g,
                builder,
                &self.text_queue,
                &mut self.text_field,
                &mut done,
            ) {
                // The page is up before its keyboard widget fills the field record: wait for it.
                if refused != native_text::OpenError::NoField {
                    eprintln!(
                        "ui-native: frame {} page {builder:#x} text field refused: {refused:?}",
                        self.frame
                    );
                    self.next += 1;
                }
            }
            for (command, outcome) in done {
                // T1916 increment 3: the field's limit (`+0x230`) and text as the guest holds them after the command.
                let field = self
                    .text_field
                    .as_ref()
                    .map(|f| f.page.0)
                    .unwrap_or(PAGE_TABLE + top.slot * PAGE_STRIDE);
                let limit = g.read32(field + native_text::field::LIMIT).unwrap_or(0);
                eprintln!(
                    "ui-native: frame {} page {builder:#x} native text {command:?} -> {outcome:?} (field limit {limit})",
                    self.frame
                );
                self.next += 1;
                // Characters follow each other at typing pace; other keys settle like page actions.
                self.hold = if matches!(command, TextCommand::Char(_)) {
                    2
                } else {
                    self.gap
                };
            }
            return;
        }
        let command = match action {
            Action::Step(dir) => NativeCommand::Step(dir),
            Action::Accept => NativeCommand::Accept,
            Action::Back => NativeCommand::Back,
            Action::Shortcut => NativeCommand::Shortcut,
            Action::Close => NativeCommand::ResumeGame,
            Action::Focus(row) => NativeCommand::Focus(row),
            Action::Select(row) => NativeCommand::Select(row),
            Action::Adjust { row, right } => NativeCommand::Adjust { row, right },
            Action::Wait(_) | Action::Page(_) | Action::Text(_) | Action::Point { .. } => return,
        };
        if let Ok(mut queue) = self.queue.lock() {
            if queue.is_empty() {
                queue.push_back(command);
            }
        }
        if std::env::var_os("TSFP_UI_NATIVE_WORDS").is_some() {
            // T2316 diagnostic: the page words the handlers read (viewport +0x38, cursor +0x184..+0x198).
            let base = top.record.0;
            let words: Vec<String> = [0x38u32, 0x184, 0x188, 0x18C, 0x190, 0x198, 0x1BC]
                .iter()
                .map(|o| format!("+{o:#x}={:#x}", g.read32(base + o).unwrap_or(0xDEAD)))
                .collect();
            eprintln!(
                "ui-native: frame {} page {builder:#x} words {}",
                self.frame,
                words.join(" ")
            );
        }
        let mut done = Vec::new();
        native_page::service(g, builder, &self.queue, &mut done);
        for (command, outcome) in done {
            eprintln!(
                "ui-native: frame {} page {builder:#x} native {command:?} -> {outcome:?}",
                self.frame
            );
            self.next += 1;
            self.hold = self.gap;
        }
    }
}

/// Run at the guest-thread frame boundary. An absent diagnostics request does nothing.
pub fn service(g: &mut dyn GuestUi) {
    let Some(runner) = runner() else { return };
    if let Ok(mut runner) = runner.lock() {
        runner.tick(g);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_route_vocabulary() {
        let script = "# arcade\npage 0x2d0be0\nwait 40\ndown\na\nadjust 2 right\nselect 1 # trailing\nb\ntype A b\nenter\npoint 490 194\n";
        assert_eq!(
            parse(script).unwrap(),
            vec![
                Action::Page(0x2D_0BE0),
                Action::Wait(40),
                Action::Step(Step::Down),
                Action::Accept,
                Action::Adjust {
                    row: 2,
                    right: true
                },
                Action::Select(1),
                Action::Back,
                Action::Text(TextCommand::Char('A')),
                Action::Text(TextCommand::Char(' ')),
                Action::Text(TextCommand::Char('b')),
                Action::Text(TextCommand::Confirm),
                Action::Point { x: 490, y: 194 },
            ]
        );
        assert!(parse("jump").is_err());
        assert!(parse("adjust 1 up").is_err());
    }
}
