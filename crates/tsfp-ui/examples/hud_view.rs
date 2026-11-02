// SPDX-License-Identifier: GPL-3.0-or-later
//! Render the Rust HUD from a recorded guest snapshot.
//!
//!     cargo run -p tsfp-ui --example hud_view -- SNAPSHOT.hudsnap [--slot N] [--json OUT.jsonl] [--text ID=STRING ...]
//!
//! Prints the [`PlayerHudState`] the HUD read and writes one draw call per line as JSON
//! (`tools/hud_render.py` rasterises it next to the host screenshot).

use std::io::Write;
use tsfp_ui::hud::draw::RecordingDraw;
use tsfp_ui::hud::frame::draw_frame;
use tsfp_ui::hud::snapshot::Snapshot;
use tsfp_ui::hud::state::PlayerHudState;
use tsfp_ui::hud::text::MapText;

fn main() {
    let mut args = std::env::args().skip(1);
    let mut snapshot_path = None;
    let mut json_path = None;
    let mut slot = 0u32;
    let mut text = MapText::default();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--slot" => slot = args.next().and_then(|v| v.parse().ok()).expect("--slot N"),
            "--json" => json_path = args.next(),
            "--text" => {
                let pair = args.next().expect("--text ID=STRING");
                let (id, string) = pair.split_once('=').expect("ID=STRING");
                let id = u32::from_str_radix(id.trim_start_matches("0x"), 16).expect("hex id");
                text.0.insert(id, string.to_owned());
            }
            other => snapshot_path = Some(other.to_owned()),
        }
    }
    let path = snapshot_path.expect("snapshot path");
    let bytes = std::fs::read(&path).expect("read snapshot");
    let mut memory = Snapshot::parse(&bytes).expect("parse snapshot");
    let state = PlayerHudState::read(&memory, slot);
    println!("{state:#?}");
    let mut draw = RecordingDraw::default();
    draw_frame(&memory, &mut draw, &text, &state);
    println!(
        "draw calls: {}, snapshot reads outside the dumped ranges: {}",
        draw.calls.len(),
        memory.missing_reads.get()
    );
    if let Some(json_path) = json_path {
        let mut file = std::fs::File::create(json_path).expect("create json");
        let (l, r, t, b) = (
            state.viewport[0],
            state.viewport[1],
            state.viewport[2],
            state.viewport[3],
        );
        writeln!(file, "{{\"op\":\"viewport\",\"l\":{l},\"r\":{r},\"t\":{t},\"b\":{b}}}").unwrap();
        for call in &draw.calls {
            writeln!(file, "{}", call.to_json()).unwrap();
        }
    }
    let _ = &mut memory;
}
