// SPDX-License-Identifier: GPL-3.0-or-later
//! Replay a C host `TSFP_AUDIO_CMD_LOG` through the in-process audio thread: `audio_replay LOG [--chunk N] [runtime args...]`
//! (runtime args as `parse_audio_args`, e.g. `--sink wav=out.wav`). Exit code 1 when any render hash or verdict differs.
use tsfp_rt_host::audio::{AudioRuntime, parse_audio_args};

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" {
        eprintln!(
            "usage: audio_replay LOG [--chunk BYTES] [--sink null|wav=PATH|ring=N] [--queue N] [--batch-bytes N] ..."
        );
        std::process::exit(2);
    }
    let log = args.remove(0);
    let mut chunk = 8192usize;
    if let Some(at) = args.iter().position(|a| a == "--chunk") {
        chunk = args
            .get(at + 1)
            .and_then(|v| v.parse().ok())
            .expect("--chunk needs a number");
        args.drain(at..at + 2);
    }
    let options = parse_audio_args(&args).expect("arguments");
    let data = std::fs::read(&log).expect("log");
    let runtime = AudioRuntime::start(options).expect("start");
    let began = std::time::Instant::now();
    for part in data.chunks(chunk) {
        runtime.write(part);
    }
    let stats = runtime.finish();
    println!("{stats:#?}\nwall {:?}", began.elapsed());
    if stats.render_diff != 0 || stats.verdict_mismatch != 0 || stats.gave_up {
        std::process::exit(1);
    }
}
