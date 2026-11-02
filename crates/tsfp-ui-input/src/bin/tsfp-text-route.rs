// SPDX-License-Identifier: GPL-3.0-or-later
//! T2169: render typed host keys as the pad tail the keyboard driver taps on the profile name page (open loop, for
//! `tools/t2116_route_build.py` and `tools.rust_trace`). The key tables are read from the owner's `default.xbe`.
//!
//! usage: tsfp-text-route --xbe build/default.xbe --type TEXT [--backspace N] [--done|--cancel]
//!        [--flags 0x190] [--len 8] [--limit 29]
//! The page opens on Done with flags 0x190 and "Player 1" selected (MEASURED, docs/t2169-keyboard-text-entry.md).
use std::path::PathBuf;
use std::process::ExitCode;
use tsfp_text_field::keyboard::{KeyGrid, KeyTable};
use tsfp_ui_input::text_entry::{Cell, GridLayout, KeyTables, LiveField, TextKey, char_to_key, keys_to_macros};
use tsfp_ui_model::xbe::XbeImage;

struct Image(XbeImage);
impl KeyTables for Image {
    fn grid(&mut self, table: KeyTable) -> Option<KeyGrid> {
        KeyGrid::load(table, |a| self.0.byte(a))
    }
}

fn number(text: &str) -> Option<u32> {
    match text.strip_prefix("0x") {
        Some(hex) => u32::from_str_radix(hex, 16).ok(),
        None => text.parse().ok(),
    }
}

fn main() -> ExitCode {
    let mut xbe = PathBuf::from("build/default.xbe");
    let mut keys: Vec<TextKey> = Vec::new();
    let mut live = LiveField {
        builder: 0x19_4980,
        cell: Some(Cell { row: 0, col: 0 }),
        flags: 0x190,
        cursor: 8,
        len: 8,
        limit: 29,
        text_hash: 0,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().unwrap_or_default();
        match arg.as_str() {
            "--xbe" => xbe = PathBuf::from(value()),
            "--type" => keys.extend(value().chars().filter_map(char_to_key)),
            "--backspace" => keys.extend(std::iter::repeat_n(TextKey::Backspace, number(&value()).unwrap_or(1) as usize)),
            "--done" => keys.push(TextKey::Done),
            "--cancel" => keys.push(TextKey::Cancel),
            "--flags" => live.flags = number(&value()).unwrap_or(live.flags),
            "--len" => live.len = number(&value()).unwrap_or(live.len),
            "--limit" => live.limit = number(&value()).unwrap_or(live.limit),
            other => {
                eprintln!("tsfp-text-route: unknown argument {other}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(image) = XbeImage::load(&xbe) else {
        eprintln!("tsfp-text-route: cannot read {}", xbe.display());
        return ExitCode::from(4);
    };
    println!("# T2169 keyboard route: typed keys rendered by tsfp_ui_input::text_entry (open loop)");
    for name in keys_to_macros(&GridLayout::LETTERS, live, &keys, &mut Image(image)) {
        println!("{name}");
    }
    ExitCode::SUCCESS
}
