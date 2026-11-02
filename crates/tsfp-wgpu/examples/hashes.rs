// SPDX-License-Identifier: GPL-3.0-or-later
//! Per present frame hashes of a text stream (T2136f pixel identity checks between two builds):
//!
//!   hashes STREAM.txt [--software]
use std::io::BufRead;
use std::process::ExitCode;

use tsfp_wgpu::context::Preference;
use tsfp_wgpu::live::{LiveConfig, LiveRenderer};

fn main() -> ExitCode {
    let mut config = LiveConfig::default();
    let mut path = None;
    for arg in std::env::args().skip(1) {
        if arg == "--software" {
            config.preference = Preference::Software;
        } else {
            path = Some(arg);
        }
    }
    let Some(path) = path else {
        eprintln!("usage: hashes STREAM.txt [--software]");
        return ExitCode::from(2);
    };
    let mut live = LiveRenderer::new(config).expect("adapter");
    let file = std::fs::File::open(path).expect("open");
    for line in std::io::BufReader::with_capacity(1 << 20, file).lines() {
        let before = live.rendered();
        live.feed_line(&line.expect("read")).expect("feed");
        if live.rendered() != before {
            if let Some(frame) = live.take_last_frame() {
                println!("{:016x}", frame.hash());
            }
        }
    }
    ExitCode::SUCCESS
}
