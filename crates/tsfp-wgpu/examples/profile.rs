// SPDX-License-Identifier: GPL-3.0-or-later
//! Render thread profile (T2136f): feeds a text stream through `LiveRenderer` exactly like the runtime's render thread and prints
//! the per phase cost and the frame time spike analysis.
//!
//!   profile STREAM.txt [--software | --adapter NAME|INDEX] [--no-readback] [--spike-ms 8]
use std::io::BufRead;
use std::process::ExitCode;

use tsfp_wgpu::context::Preference;
use tsfp_wgpu::live::{LiveConfig, LiveRenderer};
use tsfp_wgpu::replay::FrameSample;

fn pct(sorted: &[u64], p: usize) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    sorted[(sorted.len() * p / 100).min(sorted.len() - 1)]
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut path = None;
    let mut config = LiveConfig::default();
    let mut readback = true;
    let mut repeat = 1usize;
    let mut spike_ms = 8u64;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--software" => config.preference = Preference::Software,
            "--no-readback" => readback = false,
            "--repeat" => repeat = args.next().and_then(|v| v.parse().ok()).unwrap_or(1),
            "--spike-ms" => spike_ms = args.next().and_then(|v| v.parse().ok()).unwrap_or(8),
            "--adapter" => {
                let text = args.next().unwrap_or_default();
                config.preference = text
                    .parse()
                    .map_or(Preference::Named(text), Preference::Index);
            }
            _ => path = Some(arg),
        }
    }
    let Some(path) = path else {
        eprintln!(
            "usage: profile STREAM.txt [--software|--adapter X] [--no-readback] [--spike-ms N]"
        );
        return ExitCode::from(2);
    };
    // TSFP_WGPU_OUTPUT=WxH internal size, TSFP_WGPU_WINDOW=WxH present pass target (T2095)
    let env_size = |name: &str| {
        std::env::var(name).ok().and_then(|text| {
            let (w, h) = text.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
    };
    config.output = env_size("TSFP_WGPU_OUTPUT");
    config.window = env_size("TSFP_WGPU_WINDOW");
    let mut live = match LiveRenderer::new(config) {
        Ok(live) => live,
        Err(error) => {
            eprintln!("no adapter: {error}");
            return ExitCode::FAILURE;
        }
    };
    live.replayer.readback = readback;
    eprintln!("profile: adapter {}", live.replayer.adapter_name());
    let file = std::fs::File::open(&path).expect("open stream");
    let started = std::time::Instant::now();
    // Per event kind cost of the parse and of the apply (mem, texture, present, everything else), nanoseconds and counts.
    let mut parse_ns = [0u64; 4];
    let mut apply_ns = [0u64; 4];
    let mut counts = [0u64; 4];
    let mut line_number = 0usize;
    let mut hashes: Vec<u64> = Vec::new();
    let mut by_name: std::collections::HashMap<String, (u64, u64)> = Default::default();
    let all_lines: Vec<String> = std::io::BufReader::with_capacity(1 << 20, file)
        .lines()
        .map(|line| line.expect("read"))
        .collect();
    // `--repeat N` feeds the stream N times (state persists: the later rounds are the steady state of the same 7 frames).
    let repeated = (0..repeat).flat_map(|_| all_lines.iter());
    for line in repeated {
        line_number += 1;
        if let Some(rest) = line.strip_prefix("# present ") {
            live.name_next_present(rest.trim().parse().unwrap_or(0));
            continue;
        }
        let began = std::time::Instant::now();
        let parsed = tsfp_wgpu::stream::Stream::parse_line(line_number - 1, &line);
        let parse = began.elapsed().as_nanos() as u64;
        let event = match parsed {
            Ok(Some(event)) => event,
            Ok(None) => continue,
            Err(error) => {
                eprintln!("profile: {error}");
                return ExitCode::FAILURE;
            }
        };
        let kind = match &event {
            tsfp_wgpu::stream::Event::Memory { .. } => 0,
            tsfp_wgpu::stream::Event::Texture { .. } => 1,
            tsfp_wgpu::stream::Event::Present => 2,
            _ => 3,
        };
        let began = std::time::Instant::now();
        if let Err(error) = live.feed(&event) {
            eprintln!("profile: {error}");
            return ExitCode::FAILURE;
        }
        if kind == 3 {
            let key: String = line
                .split_whitespace()
                .take(2)
                .collect::<Vec<_>>()
                .join(" ");
            let entry = by_name.entry(key).or_insert((0u64, 0u64));
            entry.0 += 1;
            entry.1 += began.elapsed().as_nanos() as u64;
        }
        parse_ns[kind] += parse;
        apply_ns[kind] += began.elapsed().as_nanos() as u64;
        counts[kind] += 1;
        if matches!(event, tsfp_wgpu::stream::Event::Present) {
            if let Some(frame) = live.last_frame() {
                hashes.push(frame.hash());
            }
        }
        live.take_last_frame();
    }
    let wall = started.elapsed();
    let mut names: Vec<_> = by_name.iter().collect();
    names.sort_by_key(|(_, v)| std::cmp::Reverse(v.1));
    for (name, (count, ns)) in names.iter().take(8) {
        println!(
            "  other {name}: {count} events, apply {:.1} ms ({} ns each)",
            *ns as f64 / 1e6,
            ns / count.max(&1)
        );
    }
    println!(
        "frame hashes: {}",
        hashes
            .iter()
            .map(|h| format!("{h:016x}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    for (kind, name) in ["mem", "texture", "present", "other"].iter().enumerate() {
        println!(
            "event {name}: {} events, parse {:.1} ms, apply {:.1} ms",
            counts[kind],
            parse_ns[kind] as f64 / 1e6,
            apply_ns[kind] as f64 / 1e6
        );
    }
    let samples: Vec<FrameSample> = live.replayer.samples.clone();
    let t = live.replayer.timings;
    let ms = |ns: u64| ns as f64 / 1e6;
    println!(
        "frames {} (wall {:.1} s), draws {}, textures: {} uploads / {} cache hits",
        t.frames,
        wall.as_secs_f64(),
        t.draws,
        t.texture_uploads,
        t.texture_cache_hits
    );
    println!(
        "phase totals ms: textures {:.1}, bind groups+uniforms {:.1} (of which pipelines {:.1}, {} created), encode {:.1}, submit {:.1} (of which readback {:.1}), total {:.1}",
        ms(t.textures_ns),
        ms(t.bind_groups_ns),
        ms(t.pipelines_ns),
        t.pipelines_created,
        ms(t.encode_ns),
        ms(t.submit_ns),
        ms(t.readback_ns),
        ms(t.total_ns)
    );
    println!(
        "draw records ms: {:.1} (vertex program {:.1}, {} vertices shaded)\n gpu side ms: queue.submit {:.1}, waits {:.1} ({}), bind cache hits {} / created {}, decodes {} ({:.1} ms)",
        ms(t.draw_ns),
        ms(t.shade_ns),
        t.shaded_vertices,
        ms(t.queue_submit_ns),
        ms(t.wait_ns),
        t.waits,
        t.bind_cache_hits,
        t.bind_groups_created,
        t.decodes,
        ms(t.decode_ns)
    );
    let mut totals: Vec<u64> = samples.iter().map(|s| s.total_ns).collect();
    totals.sort_unstable();
    println!(
        "frame ms: p50 {:.2} p90 {:.2} p99 {:.2} max {:.2}",
        ms(pct(&totals, 50)),
        ms(pct(&totals, 90)),
        ms(pct(&totals, 99)),
        ms(*totals.last().unwrap_or(&0))
    );
    let new: Vec<&FrameSample> = samples.iter().filter(|s| s.pipelines_created > 0).collect();
    let mut old: Vec<u64> = samples
        .iter()
        .filter(|s| s.pipelines_created == 0)
        .map(|s| s.total_ns)
        .collect();
    old.sort_unstable();
    let mut with: Vec<u64> = new.iter().map(|s| s.total_ns).collect();
    with.sort_unstable();
    println!(
        "frames with a new pipeline key: {} (p50 {:.2} max {:.2} ms), without: {} (p50 {:.2} p99 {:.2} max {:.2} ms)",
        new.len(),
        ms(pct(&with, 50)),
        ms(*with.last().unwrap_or(&0)),
        old.len(),
        ms(pct(&old, 50)),
        ms(pct(&old, 99)),
        ms(*old.last().unwrap_or(&0))
    );
    let spikes: Vec<(usize, &FrameSample)> = samples
        .iter()
        .enumerate()
        .filter(|(_, s)| s.total_ns > spike_ms * 1_000_000)
        .collect();
    println!("spikes over {spike_ms} ms: {}", spikes.len());
    let mut cause = [0usize; 5];
    for (_, s) in &spikes {
        let parts = [
            s.pipelines_ns,
            s.textures_ns,
            s.bind_groups_ns.saturating_sub(s.pipelines_ns),
            s.encode_ns,
            s.submit_ns,
        ];
        let top = (0..5).max_by_key(|&i| parts[i]).unwrap();
        cause[top] += 1;
    }
    println!(
        "spike dominant phase: pipelines {}, textures {}, bind groups {}, encode {}, submit/readback {}",
        cause[0], cause[1], cause[2], cause[3], cause[4]
    );
    for (index, s) in spikes.iter().take(25) {
        println!(
            "  frame {index}: total {:.1} ms = textures {:.1} ({} uploads) + pipelines {:.1} ({} new) + bind {:.1} + encode {:.1} + submit {:.1} (readback {:.1}), {} draws",
            ms(s.total_ns),
            ms(s.textures_ns),
            s.texture_uploads,
            ms(s.pipelines_ns),
            s.pipelines_created,
            ms(s.bind_groups_ns.saturating_sub(s.pipelines_ns)),
            ms(s.encode_ns),
            ms(s.submit_ns),
            ms(s.readback_ns),
            s.draws
        );
    }
    ExitCode::SUCCESS
}
