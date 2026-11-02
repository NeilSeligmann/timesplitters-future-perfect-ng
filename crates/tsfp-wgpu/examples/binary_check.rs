// SPDX-License-Identifier: GPL-3.0-or-later
//! T2136d: check that the binary form of a text stream means the same thing, on real captured streams.
//!
//!   binary_check [--from N] [--to N] [--write-binary DIR] <stream.txt>...
//!   binary_check --against <guest.bin> <stream.txt>      (a binary dump written by the host, TSFP_RT_HOST_BINARY_DUMP)
//!
//! For every stream: (1) text -> binary -> events must equal text -> events, (2) the frames rendered from the text lines and from
//! the binary records (1x, same adapter) must have equal hashes, present by present, (3) the sizes of both forms are printed.
//! Exit code 1 on any difference. See docs/t-rust-hosted-dispatcher.md section 11.
use std::process::ExitCode;

use tsfp_wgpu::binary::{event_of, text_to_binary};
use tsfp_wgpu::live::{LiveConfig, LiveRenderer};
use tsfp_wgpu::stream::Stream;
use tsfp_wgpu::trace_wire::{Reader, Record};

fn frame_hashes_text(text: &str, config: LiveConfig) -> Result<Vec<(u64, u64)>, String> {
    let mut live = LiveRenderer::new(config)?;
    let mut hashes = Vec::new();
    for line in text.lines() {
        let before = live.rendered();
        live.feed_line(line)?;
        if live.rendered() != before {
            hashes.push((
                live.presents(),
                live.last_frame().map_or(0, |frame| frame.hash()),
            ));
        }
    }
    Ok(hashes)
}

fn frame_hashes_binary(bytes: &[u8], config: LiveConfig) -> Result<Vec<(u64, u64)>, String> {
    let mut live = LiveRenderer::new(config)?;
    let mut hashes = Vec::new();
    for record in Reader::new(bytes) {
        let record = record.map_err(|offset| format!("malformed record at {offset}"))?;
        let before = live.rendered();
        if let Record::Present { number } = record {
            if number != 0 {
                live.name_next_present(number);
            }
        }
        if let Some(event) = event_of(&record).map_err(|error| error.0)? {
            live.feed(&event)?;
        }
        if live.rendered() != before {
            hashes.push((
                live.presents(),
                live.last_frame().map_or(0, |frame| frame.hash()),
            ));
        }
    }
    Ok(hashes)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (mut from, mut to) = (0u64, u64::MAX);
    let (mut write_dir, mut files) = (None, Vec::new());
    let mut against: Option<String> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--from" => from = args.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "--to" => to = args.next().and_then(|v| v.parse().ok()).unwrap_or(u64::MAX),
            "--write-binary" => write_dir = args.next(),
            "--against" => against = args.next(),
            _ => files.push(arg),
        }
    }
    if files.is_empty() {
        eprintln!("usage: binary_check [--from N] [--to N] [--write-binary DIR] <stream.txt>...");
        return ExitCode::from(2);
    }
    let mut failed = false;
    for file in files {
        let text = match std::fs::read_to_string(&file) {
            Ok(text) => text,
            Err(error) => {
                eprintln!("{file}: {error}");
                return ExitCode::from(2);
            }
        };
        let started = std::time::Instant::now();
        let converted = match &against {
            Some(dump) => std::fs::read(dump).map_err(|error| format!("{dump}: {error}")),
            None => text_to_binary(&text),
        };
        let binary = match converted {
            Ok(binary) => binary,
            Err(error) => {
                eprintln!("{file}: text to binary: {error}");
                failed = true;
                continue;
            }
        };
        let convert = started.elapsed();
        if let Some(dir) = &write_dir {
            let name = std::path::Path::new(&file)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let _ = std::fs::write(format!("{dir}/{name}.bin"), &binary);
        }
        // (1) events
        let text_events = Stream::from_text(&text).map(|s| s.events);
        let mut events = Vec::new();
        let mut broken = None;
        for record in Reader::new(&binary) {
            match record {
                Ok(record) => match event_of(&record) {
                    Ok(Some(event)) => events.push(event),
                    Ok(None) => {}
                    Err(error) => broken = Some(error.0),
                },
                Err(offset) => broken = Some(format!("malformed record at {offset}")),
            }
        }
        // Walk both lists: equal events advance together, a text `Memory` event the binary list does not have is a dedup drop (the
        // binary form skips a snapshot whose bytes equal the last one at that address). Anything else is a difference.
        let mut dropped = 0usize;
        let mut extra = 0usize;
        let mut first_difference = None;
        if let Ok(expected) = &text_events {
            let is_memory = |event: &tsfp_wgpu::stream::Event| {
                matches!(event, tsfp_wgpu::stream::Event::Memory { .. })
            };
            let mut next = 0usize;
            'text: for (index, event) in expected.iter().enumerate() {
                let wanted = format!("{event:?}");
                // binary snapshots the text form does not have (a collision in the binary dedup table evicted an entry the text
                // form still held, so the binary form sends the same bytes again) are skipped, they change nothing in the image
                loop {
                    if next < events.len() && wanted == format!("{:?}", events[next]) {
                        next += 1;
                        continue 'text;
                    }
                    if next < events.len() && is_memory(&events[next]) && !is_memory(event) {
                        extra += 1;
                        next += 1;
                        continue;
                    }
                    break;
                }
                if is_memory(event) {
                    dropped += 1;
                } else {
                    first_difference = Some(index);
                    break;
                }
            }
            while first_difference.is_none() && next < events.len() && is_memory(&events[next]) {
                extra += 1;
                next += 1;
            }
            if first_difference.is_none() && next != events.len() {
                first_difference = Some(expected.len());
            }
        }
        let events_equal = broken.is_none() && text_events.is_ok() && first_difference.is_none();
        println!(
            "{file}: text {} bytes, binary {} bytes ({:.1} percent), {} events, conversion {:?}, events {}",
            text.len(),
            binary.len(),
            100.0 * binary.len() as f64 / text.len().max(1) as f64,
            events.len(),
            convert,
            if events_equal {
                format!(
                    "EQUAL ({dropped} text snapshots dropped by the binary dedup, {extra} binary snapshots resent)"
                )
            } else {
                "DIFFER".to_string()
            }
        );
        if !events_equal {
            failed = true;
            if let Some(message) = broken {
                eprintln!("  {message}");
            }
            if let Ok(expected) = &text_events {
                eprintln!(
                    "  text has {} events, binary has {}",
                    expected.len(),
                    events.len()
                );
                if let Some(index) = first_difference.filter(|&index| index < expected.len()) {
                    let cut = |event: &dyn std::fmt::Debug| {
                        format!("{event:?}").chars().take(240).collect::<String>()
                    };
                    let other = events.get(index.saturating_sub(dropped)).map(|e| cut(e));
                    eprintln!(
                        "  first difference at text event {index}:\n    text   {}\n    binary {:?}",
                        cut(&expected[index]),
                        other
                    );
                }
            }
            continue;
        }
        // (2) frames
        let config = || LiveConfig {
            from,
            to,
            ..LiveConfig::default()
        };
        let by_text = frame_hashes_text(&text, config());
        let by_binary = frame_hashes_binary(&binary, config());
        match (by_text, by_binary) {
            (Ok(a), Ok(b)) => {
                let same = !a.is_empty() && a == b;
                println!(
                    "  frames {} (text) {} (binary): {}{}",
                    a.len(),
                    b.len(),
                    if same { "IDENTICAL" } else { "DIFFER" },
                    a.last()
                        .map(|(present, hash)| format!(", last present {present} hash {hash:016x}"))
                        .unwrap_or_default()
                );
                failed |= !same;
            }
            (a, b) => {
                eprintln!("  render error: text {:?} binary {:?}", a.err(), b.err());
                failed = true;
            }
        }
    }
    if failed {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}
