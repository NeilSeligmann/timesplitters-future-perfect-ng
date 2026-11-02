// SPDX-License-Identifier: GPL-3.0-or-later
//! `cargo run -p tsfp-wgpu --example replay -- <stream|--scene title> [--out frame.png] [--write-stream f]`
//! Renders a recorded call stream (text form, docs/t-wgpu-renderer.md) or a built-in synthetic
//! scene to a PNG, and prints the adapter, the frame hash and the unsupported-call report.
use std::io::BufRead;
use std::process::ExitCode;

use tsfp_wgpu::context::{GpuContext, Preference, list_adapters};
use tsfp_wgpu::live::{DumpPlan, LiveConfig, LiveRenderer};
use tsfp_wgpu::replay::{FbSnapshot, Replayer};
use tsfp_wgpu::scenes;
use tsfp_wgpu::stream::Stream;

fn usage() -> ExitCode {
    eprintln!(
        "usage: replay (<stream-file> | --scene title) [--out frame.png] [--write-stream file] \
         [--size WxH] [--from N] [--to N] [--opt name[=value]] [--software | --any-adapter] [--list-adapters]"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (mut input, mut scene, mut out, mut write_stream) = (None, None, None, None);
    let (mut width, mut height, mut preference) = (640u32, 480u32, Preference::Software);
    let (mut from, mut to, mut debug_vertices) = (0u64, u64::MAX, 0usize);
    let mut textures_enabled = true;
    let mut skip_multitexture = false;
    let mut skip_framebuffer_samples = false;
    let mut clamp = false;
    let mut fb_copy = FbSnapshot::PerDraw;
    let mut max_draws = usize::MAX;
    let mut knobs: Vec<(String, String)> = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--scene" => scene = args.next(),
            "--out" => out = args.next(),
            "--from" => from = args.next().and_then(|v| v.parse().ok()).unwrap_or(0),
            "--to" => to = args.next().and_then(|v| v.parse().ok()).unwrap_or(u64::MAX),
            "--write-stream" => write_stream = args.next(),
            "--no-textures" => textures_enabled = false,
            "--skip-multitexture" => skip_multitexture = true,
            "--skip-framebuffer-samples" => skip_framebuffer_samples = true,
            "--clamp" => clamp = true,
            // Experiment switches (`name` or `name=value`, see `Replayer::knobs`), repeatable.
            "--opt" => {
                let Some(option) = args.next() else {
                    return usage();
                };
                let (name, value) = option.split_once('=').unwrap_or((option.as_str(), "1"));
                knobs.push((name.to_string(), value.to_string()));
            }
            "--fb-copy" => {
                fb_copy = match args.next().as_deref() {
                    Some("draw") => FbSnapshot::PerDraw,
                    Some("once") => FbSnapshot::Once,
                    Some("record") => FbSnapshot::PerRecord,
                    _ => return usage(),
                }
            }
            "--max-draws" => {
                max_draws = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(usize::MAX)
            }
            "--debug-vertices" => {
                debug_vertices = args.next().and_then(|v| v.parse().ok()).unwrap_or(0)
            }
            "--any-adapter" => preference = Preference::Any,
            // explicit form of the default of this offline tool: the software adapter the golden hashes are pinned to
            "--software" => preference = Preference::Software,
            "--list-adapters" => {
                for adapter in list_adapters() {
                    println!("{adapter:?}");
                }
                return ExitCode::SUCCESS;
            }
            "--size" => {
                let Some(size) = args.next() else {
                    return usage();
                };
                let Some((w, h)) = size.split_once('x') else {
                    return usage();
                };
                let (Ok(w), Ok(h)) = (w.parse(), h.parse()) else {
                    return usage();
                };
                (width, height) = (w, h);
            }
            "-h" | "--help" => return usage(),
            other if input.is_none() && !other.starts_with("--") => input = Some(other.to_string()),
            _ => return usage(),
        }
    }
    if let (Some(path), None, None) = (&input, scene.as_deref(), &write_stream) {
        return stream_file(
            path,
            out.as_deref(),
            (width, height),
            &preference,
            (from, to),
            debug_vertices,
            textures_enabled,
            max_draws,
            (skip_multitexture, skip_framebuffer_samples, clamp, fb_copy),
            knobs,
        );
    }
    let stream = match (&input, scene.as_deref()) {
        (Some(path), None) => match std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|text| Stream::from_text(&text))
        {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("{path}: {error}");
                return ExitCode::FAILURE;
            }
        },
        (None, Some("title")) => scenes::title_scene(),
        _ => return usage(),
    };
    if let Some(path) = write_stream {
        if let Err(error) = std::fs::write(&path, stream.to_text()) {
            eprintln!("{path}: {error}");
            return ExitCode::FAILURE;
        }
    }
    let gpu = match GpuContext::new(&preference) {
        Ok(gpu) => gpu,
        Err(error) => {
            eprintln!("no usable adapter: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut replayer = Replayer::new(gpu, width, height);
    println!("adapter: {}", replayer.adapter_name());
    replayer.run(&stream);
    for (number, frame) in replayer.frames.iter().enumerate() {
        println!(
            "frame {number}: {}x{} hash {:016x}",
            frame.width,
            frame.height,
            frame.hash()
        );
        if let Some(path) = &out {
            let name = if replayer.frames.len() == 1 {
                path.clone()
            } else {
                format!("{path}.{number}.png")
            };
            if let Err(error) = std::fs::write(&name, frame.to_png()) {
                eprintln!("{name}: {error}");
                return ExitCode::FAILURE;
            }
            println!("wrote {name}");
        }
    }
    println!("calls: {:?}", stream.call_histogram());
    println!("report: {:#?}", replayer.report);
    println!(
        "grades: native {} replayed {}",
        replayer.native_grades, replayer.replayed_grades
    );
    ExitCode::SUCCESS
}

/// Replays a text stream file line by line (the real host streams are gigabytes), writing one PNG per presented frame whose
/// host present number (the `# present N` comment the traced host writes before each `present`) is in `from..=to`.
/// Frames outside the range are fast forwarded: state, memory, constants and programs still apply, nothing is drawn.
fn stream_file(
    path: &str,
    out: Option<&str>,
    size: (u32, u32),
    preference: &Preference,
    range: (u64, u64),
    debug_vertices: usize,
    textures_enabled: bool,
    max_draws: usize,
    skips: (bool, bool, bool, FbSnapshot),
    knobs: Vec<(String, String)>,
) -> ExitCode {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) => {
            eprintln!("{path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let config = LiveConfig {
        width: size.0,
        height: size.1,
        // TSFP_WGPU_OUTPUT=WxH renders the stream at that pixel size (T2136c render scale)
        output: std::env::var("TSFP_WGPU_OUTPUT").ok().and_then(|text| {
            let (w, h) = text.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        }),
        // TSFP_WGPU_WINDOW=WxH adds the T2095 present pass (sharp bilinear, aspect kept) to that window size
        window: std::env::var("TSFP_WGPU_WINDOW").ok().and_then(|text| {
            let (w, h) = text.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        }),
        preference: preference.clone(),
        dump: DumpPlan {
            base: out.map(Into::into),
            at: Default::default(),
            every: out.is_some(),
        },
        knobs,
        from: range.0,
        to: range.1,
    };
    // The offline tool is the live renderer fed from a file: one state machine, one present path.
    let mut live = match LiveRenderer::new(config) {
        Ok(live) => live,
        Err(error) => {
            eprintln!("no usable adapter: {error}");
            return ExitCode::FAILURE;
        }
    };
    let replayer = &mut live.replayer;
    replayer.debug_vertices = debug_vertices;
    replayer.textures_enabled = textures_enabled;
    replayer.max_draws = max_draws;
    replayer.skip_multitexture = skips.0;
    replayer.skip_framebuffer_samples = skips.1;
    replayer.wrap_textures = !skips.2;
    replayer.fb_snapshot = skips.3;
    // TSFP_REPLAY_NO_READBACK=1: do not copy frames to the CPU (measures the render cost alone, no PNG)
    replayer.readback = std::env::var_os("TSFP_REPLAY_NO_READBACK").is_none();
    println!("adapter: {}", live.replayer.adapter_name());
    let reader = std::io::BufReader::with_capacity(1 << 22, file);
    for (line_number, line) in reader.lines().enumerate() {
        let Ok(line) = line else {
            eprintln!("{path}: read error at line {}", line_number + 1);
            return ExitCode::FAILURE;
        };
        let before = live.rendered();
        if let Err(error) = live.feed_line(&line) {
            eprintln!("{path}: {error}");
            return ExitCode::FAILURE;
        }
        if live.rendered() != before {
            if let Some(frame) = live.last_frame() {
                println!(
                    "present {}: {}x{} hash {:016x}",
                    live.presents(),
                    frame.width,
                    frame.height,
                    frame.hash()
                );
            }
        }
        if live.finished() && line.trim() == "present" {
            break;
        }
    }
    let t = live.replayer.timings;
    let ms = |ns: u64| ns as f64 / 1e6;
    println!(
        "timings: {} frames {} draws, textures {:.1} ms ({} uploads, {} cache hits), bind groups {:.1} ms, encode {:.1} ms, submit+readback {:.1} ms, total {:.1} ms",
        t.frames,
        t.draws,
        ms(t.textures_ns),
        t.texture_uploads,
        t.texture_cache_hits,
        ms(t.bind_groups_ns),
        ms(t.encode_ns),
        ms(t.submit_ns),
        ms(t.total_ns)
    );
    println!(
        "stream side: draw() {:.1} ms (shade_vertices {:.1} ms: interpreter {:.1} ms for {} vertices, fetch {:.1} ms), present() total {:.1} ms",
        ms(t.draw_ns),
        ms(t.shade_ns),
        ms(t.interp_ns),
        t.interp_vertices,
        ms(t.fetch_ns),
        ms(t.total_ns)
    );
    println!(
        "grades: native {} replayed {}",
        live.replayer.native_grades, live.replayer.replayed_grades
    );
    let g = live.replayer.gpu_vsh_stats();
    println!(
        "gpu vsh: job recording {:.1} ms, {} draws {} vertices on the GPU, {} draws waited for a pipeline, {} programs ready, {} failed, compile {:.1} ms",
        ms(t.gpu_prep_ns),
        g.gpu_draws,
        g.gpu_vertices,
        g.pending_draws,
        g.programs_ready,
        g.programs_failed,
        ms(g.compile_ns)
    );
    println!(
        "stall experiments: {} upload batches, {} input bytes / {} reuses, {} constant bytes / {} reuses, {} small draws / {} vertices, upload encode {:.1} ms, cold compute max {:.1} ms, texture frame max {:.1} ms, present {} bindings / {} hits, readback wait {:.1} ms / {} waits, callback {:.1} ms, rows {:.1} ms",
        g.upload_batches,
        g.input_bytes,
        g.input_reuses,
        g.constant_bytes,
        g.constant_reuses,
        g.small_draws,
        g.small_vertices,
        ms(g.upload_encode_ns),
        ms(g.compile_max_ns),
        ms(t.textures_frame_max_ns),
        t.present_bindings_created,
        t.present_bind_cache_hits,
        ms(t.readback_wait_ns),
        t.readback_waits,
        ms(t.map_callback_ns),
        ms(t.row_copy_ns)
    );
    println!("report: {}", live.report());
    ExitCode::SUCCESS
}
