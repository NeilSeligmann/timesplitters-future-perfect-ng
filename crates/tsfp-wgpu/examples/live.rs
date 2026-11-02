// SPDX-License-Identifier: GPL-3.0-or-later
//! The live renderer process (T2024 slice 7): reads the text stream lines the host writes (`TSFP_WGPU_LIVE=1` in
//! `src/host/rust_lane_call.c` pipes them to stdin) and renders them as they arrive, headless, to an offscreen target.
//!
//!   tsfp-wgpu-live [--png-base PREFIX] [--dump-at N]... [--dump-every] [--size WxH] [--from N] [--to N]
//!                  [--software | --adapter NAME|INDEX] [--opt name[=value]] [--quiet]
//!
//! Adapter policy (T2136d): the best real adapter by default (discrete GPU first), `--software` for the deterministic CPU adapter
//! the golden hashes are pinned to, `--adapter` or `TSFP_WGPU_ADAPTER` to pick one.
//!
//! A PNG `PREFIX.p<N>.png` is written for each chosen present number N (the host's own present counter, named by the
//! `# present N` line). With no `--png-base` nothing is written and only the per present hash is printed. Default off in the
//! host: this process only exists when the host was started with `TSFP_WGPU_LIVE=1`.
use std::io::BufRead;
use std::process::ExitCode;

use tsfp_wgpu::context::Preference;
use tsfp_wgpu::live::{DumpPlan, LiveConfig, LiveRenderer};

fn usage() -> ExitCode {
    eprintln!(
        "usage: live [--png-base PREFIX] [--dump-at N]... [--dump-every] [--size WxH] [--from N] [--to N] \
         [--software | --adapter NAME|INDEX] [--opt name[=value]] [--quiet]"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut config = LiveConfig::default();
    let mut quiet = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--png-base" => config.dump.base = args.next().map(Into::into),
            "--dump-at" => match args.next().and_then(|v| v.parse().ok()) {
                Some(number) => {
                    config.dump.at.insert(number);
                }
                None => return usage(),
            },
            "--dump-every" => config.dump.every = true,
            "--from" => config.from = args.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "--to" => config.to = args.next().and_then(|v| v.parse().ok()).unwrap_or(u64::MAX),
            "--size" => {
                let size = args.next().unwrap_or_default();
                let Some((w, h)) = size.split_once('x') else {
                    return usage();
                };
                let (Ok(w), Ok(h)) = (w.parse(), h.parse()) else {
                    return usage();
                };
                (config.width, config.height) = (w, h);
            }
            "--opt" => {
                let Some(option) = args.next() else {
                    return usage();
                };
                let (name, value) = option.split_once('=').unwrap_or((option.as_str(), "1"));
                config.knobs.push((name.to_string(), value.to_string()));
            }
            "--any-adapter" => config.preference = Preference::Any,
            "--software" => config.preference = Preference::Software,
            "--adapter" => {
                let Some(text) = args.next() else {
                    return usage();
                };
                config.preference = text
                    .parse()
                    .map_or(Preference::Named(text), Preference::Index);
            }
            "--quiet" => quiet = true,
            _ => return usage(),
        }
    }
    if config.dump.base.is_none() {
        config.dump = DumpPlan::default();
    }
    let mut live = match LiveRenderer::new(config) {
        Ok(live) => live,
        Err(error) => {
            eprintln!("no usable adapter: {error}");
            return ExitCode::FAILURE;
        }
    };
    if !quiet {
        eprintln!("live: adapter {}", live.replayer.adapter_name());
    }
    let reader = std::io::BufReader::with_capacity(1 << 20, std::io::stdin().lock());
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let before = live.rendered();
        if let Err(error) = live.feed_line(&line) {
            eprintln!("live: {error}");
            return ExitCode::FAILURE;
        }
        if live.rendered() != before && !quiet {
            if let Some(frame) = live.last_frame() {
                eprintln!(
                    "live: present {} hash {:016x}",
                    live.presents(),
                    frame.hash()
                );
            }
        }
        // Keep draining after `--to` so the host never blocks on a full pipe; frames past it are not rendered.
    }
    ExitCode::SUCCESS
}
