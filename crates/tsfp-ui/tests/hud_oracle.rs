// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 differential of the ported HUD functions against the ORIGINAL retail bytes.
//!
//! `python3 -m tools.hud_oracle` runs the original function under Unicorn on seeded random guest
//! state and writes case files (stack args, every byte read with its initial value, every byte
//! written, EAX, the renderer/session calls made). This test replays each case through the Rust
//! port and compares writes, calls and EAX, and fails if the port reads a byte the original did
//! not. The case files are untracked (they contain bytes of the retail image); without them the
//! test prints SKIPPED and passes. Set `HUD_ORACLE_REQUIRED=1` to turn a missing directory into a
//! failure. Generate: `.venv/bin/python -m tools.hud_oracle --seeds 3000`.

use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};
use tsfp_ui::hud::draw::{DrawCall, RecordingDraw};
use tsfp_ui::hud::health::{self, PlayerRemap};
use tsfp_ui::hud::{glyph, letterbox, messages, primitives, prompt, widgets};
use tsfp_ui::memory::GuestMemory;

#[path = "support/hex.rs"]
mod hex_support;
use hex_support::{hex, word};

#[path = "support/oracle_path.rs"]
mod oracle_path;
use oracle_path::oracle_dir;

struct CaseMemory {
    init: HashMap<u32, u8>,
    writes: BTreeMap<u32, u8>,
    missing: Cell<usize>,
}

impl GuestMemory for CaseMemory {
    fn read8(&self, address: u32) -> u8 {
        if let Some(byte) = self.writes.get(&address) {
            return *byte;
        }
        match self.init.get(&address) {
            Some(byte) => *byte,
            None => {
                self.missing.set(self.missing.get() + 1);
                0
            }
        }
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.writes.insert(address, value);
    }
}

struct Case {
    args: Vec<u32>,
    init: HashMap<u32, u8>,
    writes: BTreeMap<u32, u8>,
    calls: Vec<(u32, u32, Vec<u32>)>,
    eax: Option<u32>,
}

fn parse(text: &str) -> Vec<Case> {
    let mut cases = Vec::new();
    let mut current: Option<Case> = None;
    for line in text.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts.as_slice() {
            ["case", _] => {
                current = Some(Case {
                    args: vec![],
                    init: HashMap::new(),
                    writes: BTreeMap::new(),
                    calls: vec![],
                    eax: None,
                })
            }
            ["end"] => cases.push(current.take().unwrap()),
            [tag, rest @ ..] => {
                let case = current.as_mut().unwrap();
                match *tag {
                    "args" => case.args = rest.iter().map(|w| word(w)).collect(),
                    "init" | "write" => {
                        let address = word(rest[0]);
                        for (i, byte) in hex(rest[1]).into_iter().enumerate() {
                            if *tag == "init" {
                                case.init.insert(address + i as u32, byte);
                            } else {
                                case.writes.insert(address + i as u32, byte);
                            }
                        }
                    }
                    "call" => case.calls.push((
                        u32::from_str_radix(rest[0], 16).unwrap(),
                        word(rest[1]),
                        rest[2..].iter().map(|w| word(w)).collect(),
                    )),
                    "eax" => case.eax = Some(word(rest[0])),
                    _ => panic!("bad line {line}"),
                }
            }
            [] => {}
        }
    }
    cases
}

struct ReplayRemap {
    returns: Vec<u32>,
    next: usize,
}

impl PlayerRemap for ReplayRemap {
    fn remap_player(&mut self, _player: i32) -> u32 {
        let value = self.returns[self.next];
        self.next += 1;
        value
    }
}

/// Run one spec file. Returns the number of cases checked.
fn check(
    name: &str,
    run: impl Fn(&mut CaseMemory, &Case, &mut RecordingDraw) -> Option<u32>,
) -> usize {
    let path = oracle_dir().join(format!("{name}.cases"));
    let Ok(text) = std::fs::read_to_string(&path) else {
        assert!(
            std::env::var("HUD_ORACLE_REQUIRED").is_err(),
            "{name}: oracle cases missing at {path:?}"
        );
        eprintln!("SKIPPED {name}: no case file (run tools.hud_oracle)");
        return 0;
    };
    let cases = parse(&text);
    assert!(!cases.is_empty(), "{name}: empty case file");
    for (index, case) in cases.iter().enumerate() {
        let mut memory = CaseMemory {
            init: case.init.clone(),
            writes: BTreeMap::new(),
            missing: Cell::new(0),
        };
        let mut draw = RecordingDraw::default();
        let eax = run(&mut memory, case, &mut draw);
        assert_eq!(
            memory.missing.get(),
            0,
            "{name} case {index}: port read bytes the original did not"
        );
        assert_eq!(
            memory.writes, case.writes,
            "{name} case {index}: writes differ"
        );
        let calls: Vec<(u32, Vec<u32>)> =
            draw.calls.iter().map(|c| (c.callee(), c.words())).collect();
        let expected: Vec<(u32, Vec<u32>)> = case
            .calls
            .iter()
            .filter(|(callee, _, _)| *callee != 0x356800)
            .map(|(callee, _, args)| (*callee, args.clone()))
            .collect();
        assert_eq!(
            calls, expected,
            "{name} case {index}: renderer calls differ"
        );
        if let Some(expected_eax) = case.eax {
            assert_eq!(eax, Some(expected_eax), "{name} case {index}: EAX differs");
        }
    }
    println!("{name}: {} cases agree with the original", cases.len());
    cases.len()
}

fn arg(case: &Case, index: usize) -> u32 {
    case.args[index]
}

#[test]
fn textured_rect_40260() {
    let n = check("textured_rect_40260", |m, c, d| {
        primitives::draw_textured_rect(m, d, arg(c, 0));
        None
    });
    let _ = n;
}

#[test]
fn glyph_metric_3faa0() {
    check("glyph_metric_3faa0", |m, c, _| {
        Some(glyph::glyph_metric(m, arg(c, 0)) as u32)
    });
}

#[test]
fn letterbox_functions() {
    check("letterbox_hold_139520", |m, c, _| {
        letterbox::start_hold(m, arg(c, 0), arg(c, 1));
        None
    });
    check("letterbox_fade_in_139570", |m, c, _| {
        letterbox::start_fade_in(m, arg(c, 0), arg(c, 1));
        None
    });
    check("letterbox_update_1395c0", |m, _, _| {
        letterbox::update(m);
        None
    });
    check("letterbox_draw_1396b0", |m, _, d| {
        letterbox::draw(m, d);
        None
    });
}

#[test]
fn health_flag_functions() {
    check("flags_init_12c830", |m, c, _| {
        health::flags_init(m, arg(c, 0), arg(c, 1));
        None
    });
    check("flags_differ_13a140", |m, c, _| {
        Some(u32::from(health::flags_differ(m, arg(c, 0), arg(c, 1))))
    });
    check("flags_tick_13a2c0", |m, c, _| {
        health::flags_tick(m, arg(c, 0), arg(c, 1));
        None
    });
    check("slot_init_13a270", |m, c, _| {
        health::slot_init(m, arg(c, 0));
        None
    });
    check("slot_update_13a1d0", |m, c, _| {
        let mut remap = ReplayRemap {
            returns: c
                .calls
                .iter()
                .filter(|x| x.0 == 0x356800)
                .map(|x| x.1)
                .collect(),
            next: 0,
        };
        health::slot_update(m, &mut remap, arg(c, 0) as i32, f32::from_bits(arg(c, 1)));
        assert_eq!(remap.next, remap.returns.len(), "remap call count");
        None
    });
}

#[test]
fn message_ring_functions() {
    check("push_notice_12d6b0", |m, c, _| {
        messages::push_notice(
            m,
            arg(c, 0) as i32,
            arg(c, 1),
            arg(c, 2),
            f32::from_bits(arg(c, 3)),
        );
        None
    });
    check("push_notice_c_12d760", |m, c, _| {
        messages::push_notice_c(
            m,
            arg(c, 0) as i32,
            arg(c, 1),
            arg(c, 2),
            f32::from_bits(arg(c, 3)),
        );
        None
    });
    check("clear_kind_12d810", |m, c, _| {
        messages::clear_kind(m, arg(c, 0) as i32, arg(c, 1) as i32);
        None
    });
}

#[test]
fn predicates() {
    check("prompt_allowed_42f90", |m, _, _| {
        Some(prompt::prompt_allowed(m))
    });
    check("slot_visible_139160", |m, _, _| {
        Some(prompt::local_slot_visible(m))
    });
}

#[test]
fn widget_functions() {
    check("colour_scale_154fe0", |_, c, _| {
        Some(widgets::scale_colour_low_byte(
            arg(c, 0),
            f32::from_bits(arg(c, 1)),
        ))
    });
    check("dividers_73110", |m, c, d| {
        widgets::draw_divider_lines(m, d, arg(c, 0), arg(c, 2));
        None
    });
    check("corner_frame_2d7ae0", |m, c, d| {
        widgets::draw_corner_frame(m, d, arg(c, 0));
        None
    });
    check("loadbar_2d8060", |m, c, d| {
        widgets::draw_loadbar(m, d, arg(c, 0));
        None
    });
}

#[test]
fn recorded_calls_use_the_original_callees() {
    let call = DrawCall::FillRect {
        left: 1,
        top: 2,
        right: 3,
        bottom: 4,
        alpha: 5,
    };
    assert_eq!(call.callee(), 0x1efb0);
    assert_eq!(call.words(), vec![1, 2, 3, 4, 5]);
}
