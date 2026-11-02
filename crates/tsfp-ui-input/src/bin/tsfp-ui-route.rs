// SPDX-License-Identifier: GPL-3.0-or-later
//! T2188/T2168: replay a `--ui=rust` device script through the Rust UI input path (UiInput on
//! the page's `ModelLayout`, `LiveMenu` dispatch over the tree-generated `PageMap`,
//! `pad_route` tap driver, `LiveMenu::pad_route`) and print the pad route tail the original handlers receive, in the
//! `tools/t2116_route_build.py` macro format. The trace host has no shell input loop, so the
//! guest cursor is simulated open loop (moves skip non-navigable rows and clamp, select leaves
//! the page, each page remembers its cursor); the live shell runs the same dispatch closed loop
//! against the real page slot.
//!
//! usage: tsfp-ui-route --tree tmp/ui-tree/tree.json --script FILE [--builder 0x2D0BE0]
//! Script lines (`#` comments): `page 0xBUILDER [CURSOR]` (this page is now live: after a select
//! or back the script names the page the original opened), `wait MS`,
//! `move X Y` (pointer to design coordinates, e.g. off every row), `press up|down|left|right|accept|back`, `hold INTENT MS`, `point ROW`, `click ROW`,
//! `rclick [ROW]`, `wheel NOTCHES [ROW]`, `key up|down|left|right|a|b` (one plain tap, the
//! keyboard on a page whose navigation the original keeps), `legacy MACRO...` (input while no
//! Rust page is live passes straight through).
use std::collections::BTreeMap;
use std::process::ExitCode;

use tsfp_ui_input::live_menu::{
    Goal, LiveMenu, LiveSlot, PadButton, TapTiming, goals_to_macros_on,
};
use tsfp_ui_input::{
    ContextId, FrontendPages, IntentSet, MenuIntent, ModelLayout, Ownership, PageMap, PlayerFrame,
    PointerButtons, PointerFrame, Routed, UiInput, UiLayout,
};
use tsfp_ui_model::frontend::{self, MAIN_MENU_BUILDER};
use tsfp_ui_model::{Page, UiModel};

const FRAME_MS: u64 = 16;

fn intent(name: &str) -> Option<MenuIntent> {
    Some(match name {
        "up" => MenuIntent::Up,
        "down" => MenuIntent::Down,
        "left" => MenuIntent::Left,
        "right" => MenuIntent::Right,
        "accept" => MenuIntent::Accept,
        "back" => MenuIntent::Back,
        _ => return None,
    })
}

fn button(name: &str) -> Option<PadButton> {
    Some(match name {
        "up" => PadButton::Up,
        "down" => PadButton::Down,
        "left" => PadButton::Left,
        "right" => PadButton::Right,
        "a" => PadButton::A,
        "b" => PadButton::B,
        _ => return None,
    })
}

struct Live<'a> {
    page: &'a Page,
    map: PageMap,
    layout: ModelLayout,
    menu: LiveMenu,
    input: UiInput,
    live: LiveSlot,
}

impl Live<'_> {
    fn centre(&self, row: usize) -> Result<(f32, f32), String> {
        let target = *self.menu.rows.get(row).ok_or(format!("no row {row}"))?;
        let mut centre = None;
        self.layout.visit(&mut |t| {
            if t.id().0 == target.0 {
                let r = t.rect();
                centre = Some((r.x + r.w * 0.5, r.y + r.h * 0.5));
            }
        });
        centre.ok_or(format!("row {row} has no rect"))
    }
}

struct Sim<'a> {
    model: &'a UiModel,
    pages: FrontendPages,
    current: Option<Live<'a>>,
    cursors: BTreeMap<u32, u32>,
    now: u64,
    pointer: PointerFrame,
    tail: Vec<String>,
}

impl<'a> Sim<'a> {
    fn open(&mut self, builder: u32, cursor: Option<u32>) -> Result<(), String> {
        let map = self
            .pages
            .get(builder)
            .cloned()
            .ok_or(format!("no page for builder {builder:#x} in the tree"))?;
        let page = self
            .model
            .page(map.page)
            .ok_or("page missing in the model")?;
        let layout = ModelLayout::from_page(page, 640.0, 480.0).ok_or("no layout")?;
        let menu = LiveMenu::pad_route(page, &map, TapTiming::default());
        let rows = menu.rows.len() as u32;
        let cursor = cursor
            .or_else(|| self.cursors.get(&builder).copied())
            .unwrap_or(0);
        eprintln!(
            "page {:#x} builder {builder:#x}: {rows} cursor rows, pointer rows {}",
            map.page.0, menu.pointer_rows
        );
        let mut input = UiInput::new();
        input.open_context(ContextId(0), Ownership::Any);
        input.set_focus(
            ContextId(0),
            menu.rows
                .get(cursor as usize)
                .map(|w| tsfp_ui_input::WidgetId(w.0)),
        );
        self.current = Some(Live {
            page,
            map,
            layout,
            menu,
            input,
            live: LiveSlot {
                builder,
                state: 0,
                cursor,
                rows,
            },
        });
        Ok(())
    }

    /// Render and apply the goals queued on the live page (open loop).
    fn flush(&mut self) {
        let Some(cur) = self.current.as_mut() else {
            return;
        };
        let goals = cur
            .menu
            .pad_route_driver()
            .map(|d| d.drain())
            .unwrap_or_default();
        if goals.is_empty() {
            return;
        }
        let navigable = cur.map.navigable();
        let mut cursor = cur.live.cursor;
        let mut leaves = false;
        let mut taken = Vec::new();
        for goal in goals {
            taken.push(goal);
            if matches!(
                goal,
                Goal::Select(_) | Goal::Back | Goal::Tap(PadButton::A | PadButton::B)
            ) {
                leaves = true;
                break;
            }
        }
        let (macros, after) = goals_to_macros_on(&taken, cursor, &navigable);
        eprintln!("  goals {taken:?} -> {macros:?}");
        self.tail.extend(macros.iter().map(|m| m.to_string()));
        cursor = after;
        cur.live.cursor = cursor;
        let builder = cur.live.builder;
        // The page keeps its cursor while a child page is on top (MEASURED on the settings
        // routes); select resets it in the simulation only when the page is rebuilt.
        let keep = taken
            .iter()
            .rev()
            .find_map(|g| match g {
                Goal::Select(row) | Goal::Focus(row) => Some(*row),
                Goal::Adjust { row, .. } => Some(*row),
                _ => None,
            })
            .unwrap_or(cursor);
        self.cursors.insert(builder, keep);
        if leaves {
            // The original handler opens the next page (or backs out): the script names it.
            self.current = None;
        }
    }

    fn frame(&mut self, held: IntentSet) {
        self.now += FRAME_MS;
        let Some(cur) = self.current.as_mut() else {
            return;
        };
        cur.menu.observe(Some(cur.live));
        let mut routed: Vec<Routed> = Vec::new();
        cur.input.update(
            ContextId(0),
            self.now,
            &cur.layout as &dyn UiLayout,
            &[PlayerFrame {
                held,
                chars: Vec::new(),
            }],
            Some(&self.pointer),
            &mut routed,
        );
        for r in &routed {
            cur.menu.dispatch(cur.page, &r.event);
        }
        self.flush();
    }

    fn hovered(&self) -> Option<tsfp_ui_model::WidgetId> {
        let cur = self.current.as_ref()?;
        let (x, y) = self.pointer.pos?;
        tsfp_ui_input::hit_test(&cur.layout, x, y).map(|w| tsfp_ui_model::WidgetId(w.0))
    }
}

fn parse_hex(v: &str) -> Option<u32> {
    u32::from_str_radix(v.trim_start_matches("0x"), 16).ok()
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (mut tree, mut script, mut builder) = (None, None, MAIN_MENU_BUILDER as u32);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--tree" => tree = args.next(),
            "--script" => script = args.next(),
            "--builder" => builder = args.next().as_deref().and_then(parse_hex).unwrap_or(0),
            _ => {
                eprintln!("usage: tsfp-ui-route --tree FILE --script FILE [--builder 0xADDR]");
                return ExitCode::from(2);
            }
        }
    }
    let (Some(tree), Some(script)) = (tree, script) else {
        eprintln!("usage: tsfp-ui-route --tree FILE --script FILE [--builder 0xADDR]");
        return ExitCode::from(2);
    };
    let text = match std::fs::read_to_string(&tree) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("tsfp-ui-route: {tree}: {e}");
            return ExitCode::from(1);
        }
    };
    let model = match UiModel::from_tree_json(&text, &frontend::dump_load_options()) {
        Ok(model) => model,
        Err(e) => {
            eprintln!("tsfp-ui-route: {tree}: {e:?}");
            return ExitCode::from(1);
        }
    };
    let run = || -> Result<Vec<String>, String> {
        let script_text = std::fs::read_to_string(&script).map_err(|e| format!("{script}: {e}"))?;
        let mut sim = Sim {
            model: &model,
            pages: FrontendPages::from_tree(&text, &model),
            current: None,
            cursors: BTreeMap::new(),
            now: 0,
            pointer: PointerFrame::default(),
            tail: vec!["WAIT 40".into()],
        };
        sim.open(builder, Some(0))?;
        let bad = |line: &str| format!("bad line: {line}");
        for line in script_text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let words: Vec<&str> = line.split_whitespace().collect();
            let live = sim.current.is_some();
            let need_live = || -> Result<(), String> {
                if live {
                    Ok(())
                } else {
                    Err(format!("no Rust page is live (add a `page` line): {line}"))
                }
            };
            match words.as_slice() {
                [] => {}
                ["page", b, rest @ ..] => {
                    let b = parse_hex(b).ok_or(bad(line))?;
                    let cursor = match rest {
                        [] => None,
                        [c] => Some(c.parse().map_err(|_| bad(line))?),
                        _ => return Err(bad(line)),
                    };
                    sim.open(b, cursor)?;
                }
                ["wait", ms] => {
                    let ms: u64 = ms.parse().map_err(|_| bad(line))?;
                    for _ in 0..ms / FRAME_MS {
                        sim.frame(IntentSet::default());
                    }
                }
                ["press", name] => {
                    need_live()?;
                    let i = intent(name).ok_or(bad(line))?;
                    sim.frame(IntentSet::default().with(i));
                    sim.frame(IntentSet::default());
                }
                ["hold", name, ms] => {
                    need_live()?;
                    let i = intent(name).ok_or(bad(line))?;
                    let ms: u64 = ms.parse().map_err(|_| bad(line))?;
                    for _ in 0..ms / FRAME_MS {
                        sim.frame(IntentSet::default().with(i));
                    }
                    sim.frame(IntentSet::default());
                }
                ["key", name] => {
                    need_live()?;
                    let b = button(name).ok_or(bad(line))?;
                    if let Some(cur) = sim.current.as_mut() {
                        cur.menu.observe(Some(cur.live));
                        cur.menu.tap(b);
                    }
                    sim.flush();
                }
                ["move", x, y] => {
                    let x: f32 = x.parse().map_err(|_| bad(line))?;
                    let y: f32 = y.parse().map_err(|_| bad(line))?;
                    sim.pointer.pos = Some((x, y));
                    sim.frame(IntentSet::default());
                }
                ["point", row] | ["click", row] => {
                    need_live()?;
                    let row: usize = row.parse().map_err(|_| bad(line))?;
                    let centre = sim.current.as_ref().map(|c| c.centre(row)).transpose()?;
                    sim.pointer.pos = centre;
                    sim.frame(IntentSet::default());
                    if words[0] == "click" && sim.current.is_some() {
                        sim.pointer.buttons = PointerButtons {
                            left: true,
                            ..Default::default()
                        };
                        sim.frame(IntentSet::default());
                        sim.pointer.buttons = PointerButtons::default();
                        sim.frame(IntentSet::default());
                    }
                }
                ["rclick", rest @ ..] => {
                    need_live()?;
                    if let [row] = rest {
                        let row: usize = row.parse().map_err(|_| bad(line))?;
                        let centre = sim.current.as_ref().map(|c| c.centre(row)).transpose()?;
                        sim.pointer.pos = centre;
                        sim.frame(IntentSet::default());
                    }
                    // Same order as the shell: an option row under the pointer takes the right
                    // click as "previous value", anywhere else it is Back.
                    let hovered = sim.hovered();
                    let adjusted = sim
                        .current
                        .as_mut()
                        .is_some_and(|c| c.menu.value_prev_at(hovered));
                    if adjusted {
                        sim.flush();
                    } else {
                        sim.pointer.buttons = PointerButtons {
                            right: true,
                            ..Default::default()
                        };
                        sim.frame(IntentSet::default());
                        sim.pointer.buttons = PointerButtons::default();
                        sim.frame(IntentSet::default());
                    }
                }
                ["wheel", notches, rest @ ..] => {
                    need_live()?;
                    let notches: i32 = notches.parse().map_err(|_| bad(line))?;
                    if let [row] = rest {
                        let row: usize = row.parse().map_err(|_| bad(line))?;
                        let centre = sim.current.as_ref().map(|c| c.centre(row)).transpose()?;
                        sim.pointer.pos = centre;
                        sim.frame(IntentSet::default());
                    }
                    let hovered = sim.hovered();
                    if let Some(cur) = sim.current.as_mut() {
                        cur.menu.wheel(hovered, notches);
                    }
                    sim.flush();
                }
                ["legacy", rest @ ..] if !live => {
                    sim.tail.push(rest.join(" "));
                }
                ["legacy", ..] => {
                    return Err(format!("legacy input while the Rust page is live: {line}"));
                }
                _ => return Err(bad(line)),
            }
        }
        Ok(sim.tail)
    };
    match run() {
        Ok(tail) => {
            println!(
                "# T2188/T2168 tsfp-ui-route output: pad tail produced by the Rust UI dispatch (FABRICATED device script, open-loop cursor)."
            );
            for line in tail {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("tsfp-ui-route: {message}");
            ExitCode::from(1)
        }
    }
}
