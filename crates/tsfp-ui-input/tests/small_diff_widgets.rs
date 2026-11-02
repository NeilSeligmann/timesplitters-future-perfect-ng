// SPDX-License-Identifier: GPL-3.0-or-later
//! T2310 UI-12 Tier 1: the native widget classes replayed against goldens recorded from the
//! original XBE under Unicorn (fabricated row and page records, rule 12a).
//! `python -m tools.small_diff --system ui_widgets --xbe /workspace/build/default.xbe && cargo test -p tsfp-ui-input --test small_diff_widgets -- --ignored`
#[path = "../../tsfp-engine-memory/src/golden.rs"]
mod golden;

use golden::{Call, GoldenWorld, STACK_LOW};
use tsfp_ui_input::menu_state::PageRef;
use tsfp_ui_input::native_page::{GuestUi, original};
use tsfp_ui_input::page_stack::{self, ContextStack};
use tsfp_text_field::keyboard::{FieldFlags, select_table};
use tsfp_ui_input::native_text;
use tsfp_ui_input::widgets;

/// The page record the golden stub of the original open 0x79860 returns (specs_ui_widgets.py `OPENED`).
const OPENED: u32 = 0x0B00_B000;
/// The text input widget record of the ring cases (specs_ui_widgets.py `FIELD`).
const RING_FIELD: u32 = 0x0B00_C000;
const ROW: u32 = 0x0B00_4000;
const PAGE_AFTER_VALUE: u32 = 0x0C00_5800;
const CB_VALUE: u32 = 0x0C80_0000;
const CB_CHANGE_AFTER_VALUE: u32 = 0x0C80_0020;

struct World(GoldenWorld, bool);

impl GuestUi for World {
    fn read32(&self, va: u32) -> Option<u32> {
        Some(self.0.read32(va))
    }
    fn write32(&mut self, va: u32, value: u32) -> bool {
        if va == RING_FIELD + native_text::field::DIRTY_FLAGS {
            // The harness stubs the original dirty mark 0x746D0(widget) (a logged call, no write); the port writes
            // the same bit natively, so its write is the call here.
            self.0.calls.push(Call { va: 0x746D0, ret: 0, args: vec![RING_FIELD] });
            return true;
        }
        for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.0.write8(va.wrapping_add(i as u32), byte);
        }
        true
    }
    fn call(&mut self, va: u32, args: &[u32]) -> Option<u32> {
        if va == CB_VALUE && self.1 {
            for (i, byte) in PAGE_AFTER_VALUE.to_le_bytes().into_iter().enumerate() {
                self.0.write8(ROW + 0x88 + i as u32, byte);
            }
            for (i, byte) in CB_CHANGE_AFTER_VALUE.to_le_bytes().into_iter().enumerate() {
                self.0.write8(ROW + 0x4C + i as u32, byte);
            }
        }
        if va == original::PLAY_SOUND {
            // The original's inline pair: 0x45390(block) then 0x45820(block) with block+0 = handle;
            // the golden stub of 0x45820 returns that handle. The block is on the stack (wildcard).
            self.0.calls.push(Call { va: 0x45390, ret: 0, args: vec![STACK_LOW] });
            self.0.calls.push(Call { va: 0x45820, ret: args[0], args: vec![STACK_LOW] });
            return Some(0);
        }
        let ret = if va == page_stack::original::OPEN_PAGE { OPENED } else { 0 };
        self.0.calls.push(Call { va, ret, args: args.to_vec() });
        Some(ret)
    }
}

const NAMES: &[&str] = &[
    "cursor_row_at",
    "scroll_to_row",
    "option_select",
    "option_next",
    "option_prev",
    "stack_append",
    "stack_push",
    "stack_pop",
    "ring_step_back",
    "ring_step_forward",
    "ring_delete",
    "ring_key",
];

#[test]
#[ignore = "needs goldens from the owner's XBE (tmp/small-diff/ui_widgets)"]
fn widgets_match_the_original() {
    golden::replay_all(
        "ui_widgets",
        NAMES,
        |golden| World(golden, false),
        |w| &mut w.0,
        |case, w| match case.name.as_str() {
            "cursor_row_at" => Some(widgets::cursor_row_at(w, PageRef(case.arg(0)), case.arg(1), case.arg(2) as i32)),
            "scroll_to_row" => {
                widgets::scroll_to_row(w, case.arg(0), case.arg(1));
                None
            }
            "option_select" => {
                widgets::option_select(w, case.arg(0), case.arg(1) as i32);
                None
            }
            "option_next" => {
                widgets::option_next(w, case.arg(0));
                None
            }
            "option_prev" => {
                widgets::option_prev(w, case.arg(0));
                None
            }
            "stack_append" => {
                page_stack::append(w, ContextStack(case.arg(0)), PageRef(case.arg(1)));
                None
            }
            "stack_push" => Some(page_stack::push(w, PageRef(case.arg(0)), case.arg(1))),
            "stack_pop" => Some(page_stack::pop(w, PageRef(case.arg(0)))),
            "ring_step_back" => {
                native_text::ring::step(w, PageRef(case.reg("esi")), false);
                None
            }
            "ring_step_forward" => {
                native_text::ring::step(w, PageRef(case.reg("esi")), true);
                None
            }
            "ring_delete" => {
                native_text::ring::delete(w, PageRef(case.reg("ebx")));
                None
            }
            "ring_key" => {
                // The key byte of 0x7DB20: the pad grid slot +0x18C in the table the field flags select.
                let field = case.arg(0);
                let flags = w.0.read32(field + 0x228);
                let table = select_table(FieldFlags(flags & 0xFFFF));
                let key = w.0.read8(table.bytes + w.0.read32(field + 0x18C));
                w.call(native_text::original::KEY_SOUND, &[]);
                native_text::ring::key(w, PageRef(field), key);
                None
            }
            other => panic!("unknown case {other}"),
        },
    );
}

#[test]
fn option_select_reloads_callback_and_page_after_value_effect() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/goldens/ui_widgets/option_select_page_reload.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let cases = golden::parse(&text);
    assert_eq!(cases.len(), 1, "expected one callback reload case");
    let case = &cases[0];
    assert_eq!(case.name, "option_select_page_reload");

    let mut world = World(GoldenWorld::new(case), true);
    widgets::option_select(&mut world, case.arg(0), case.arg(1) as i32);
    golden::compare(case, &world.0, None).expect("option selection differs from lifted original");
}
