// SPDX-License-Identifier: GPL-3.0-or-later
//! T2023 stage 2 of the Bevy plan (docs/decisions/bevy-runnable-focus.md): Bevy 0.16.1 owns the window, the event loop, input
//! and audio; the game code (`tsfp_host_entry`: the lifted C host plus the Rust ports) runs on a dedicated game thread with the
//! C host's own arguments. Frames reach Bevy through a latest-wins mailbox (`src/host/bevy_sink.h`), input flows back through
//! the `tsfp_wi_*` queue (`wi.rs`, `events.rs`), the host audio ring is pulled by Bevy audio (`audio.rs`). The C host still
//! renders; Bevy only displays. Build, run, limits: docs/t-bevy-shell.md.
use std::ffi::{CString, c_char, c_int};
use std::process::ExitCode;
use std::sync::atomic::{AtomicI32, Ordering};
#[cfg(tsfp_no_host)]
use tsfp_bevy_nohost_link::tsfp_host_entry;
use tsfp_host::run_then;

mod app;
mod audio;
mod editor_navigation;
mod events;
mod fps;
mod frame_budget;
mod gpu_share;
mod hud_live;
mod hud_producer_live;
mod input_recording;
mod input_router;
mod mapmaker_action_picker_live;
mod mapmaker_assault_options_live;
mod mapmaker_assault_phase_assignment_live;
mod mapmaker_assault_start_point_live;
mod mapmaker_award_flags_live;
mod mapmaker_award_medal_live;
mod mapmaker_award_table_live;
mod mapmaker_awards_live;
mod mapmaker_controls_ui_live;
mod mapmaker_description_live;
mod mapmaker_disabled_modes_live;
mod mapmaker_document_close_live;
mod mapmaker_editor_live;
mod mapmaker_editor_settings_live;
mod mapmaker_kind_list_live;
mod mapmaker_kind_picker_live;
mod mapmaker_logic_summary_live;
mod mapmaker_object_kind_live;
mod mapmaker_palette_grid_live;
mod mapmaker_palette_properties_live;
mod mapmaker_preview_live;
mod mapmaker_sample_map_menu_live;
mod mapmaker_selected_entry_live;
mod mapmaker_set_lists_live;
mod mapmaker_settings_live;
mod mapmaker_tool_menu_live;
mod mapmaker_trigger_selection_live;
mod mapmaker_ui_live;
mod native_render_prep;
pub(crate) mod physics_contact_live;
mod present;
mod present_latency;
mod rea_ai_trace_live;
mod sample_maps;
mod share_bench;
mod shared;
mod storage_caller;
mod storage_live;
mod storage_presentation;
mod storage_session;
mod story_mission_runtime;
mod story_progress_bridge;
mod text_entry_live;
mod ui_back;
mod ui_extensions;
mod ui_live;
mod ui_overlay;
mod ui_pages;
mod wgpu_live;
mod wi;
#[path = "../../tsfp-game/src/window_input.rs"]
#[allow(dead_code)]
mod window_input;

#[cfg(not(tsfp_no_host))]
unsafe extern "C" {
    /// src/host/rust_main.c: the C host's former `main`, renamed.
    fn tsfp_host_entry(argc: c_int, argv: *mut *mut c_char) -> c_int;
    /// tsfp-ports callback registry for Story mode ticks on the game thread.
    fn tsfp_ports_register_story_mission_tick(callback: Option<unsafe extern "C" fn() -> u32>);
    /// tsfp-ports callback registry for completed typed Story sequencer outputs.
    fn tsfp_ports_register_story_sequencer_output(
        callback: Option<unsafe extern "C" fn(u32, *const u8) -> u32>,
    );
    /// Register the typed contact adapter before the C host parses its opt-in switch.
    fn tsfp_host_contact_delivery_register(
        callback: Option<physics_contact_live::DirectCallCallback>,
    );
}

const INFO: &str = concat!(
    "tsfp-bevy ",
    env!("CARGO_PKG_VERSION"),
    " (T2023, Bevy 0.16.1 shell over the C host)\n",
    "host build dir: ",
    env!("TSFP_BEVY_HOST_BUILD_DIR"),
    "\n",
    "build stamp: git ",
    env!("TSFP_BUILD_GIT_SHA"),
    " ports.a sha256 ",
    env!("TSFP_BUILD_PORTS_ARCHIVE"),
    " ports-src sha256 ",
    env!("TSFP_BUILD_PORTS_SRC"),
    "\n",
);

static GAME_STATUS: AtomicI32 = AtomicI32::new(0);

/// `--present bevy` and `--present window` both mean the Bevy window here (the C sink variant behind it is the Bevy one).
/// Returns the host arguments and whether a Bevy window is wanted (no `--present` given means yes, `--present window` is added).
fn host_arguments(mut arguments: Vec<String>) -> (Vec<String>, bool) {
    let mut at = 0;
    let mut windowed = None;
    while at < arguments.len() {
        if arguments[at] == "--present" && at + 1 < arguments.len() {
            let sink = arguments[at + 1].as_str();
            windowed = Some(sink == "window" || sink == "bevy");
            if windowed == Some(true) {
                arguments[at + 1] = "window".into();
            }
            at += 1;
        }
        at += 1;
    }
    match windowed {
        Some(wanted) => (arguments, wanted),
        None => {
            arguments.extend(["--present".to_string(), "window".to_string()]);
            (arguments, true)
        }
    }
}

fn run_host(program: &str, arguments: &[String]) -> c_int {
    let mut owned: Vec<CString> = Vec::with_capacity(arguments.len() + 1);
    for text in std::iter::once(program).chain(arguments.iter().map(String::as_str)) {
        match CString::new(text) {
            Ok(value) => owned.push(value),
            Err(_) => {
                eprintln!("argument contains a NUL byte");
                return 2;
            }
        }
    }
    let mut pointers: Vec<*mut c_char> = owned
        .iter()
        .map(|value| value.as_ptr().cast_mut())
        .collect();
    pointers.push(std::ptr::null_mut());
    #[cfg(not(tsfp_no_host))]
    // SAFETY: process-lifetime callback, registered on the game thread that enters the C2R mode tick; 0 until a Story owner exists.
    unsafe {
        tsfp_ports_register_story_mission_tick(Some(
            story_mission_runtime::tsfp_bevy_story_mission_host_tick,
        ));
        tsfp_ports_register_story_sequencer_output(Some(
            story_mission_runtime::tsfp_bevy_story_sequencer_output,
        ));
        tsfp_host_contact_delivery_register(Some(physics_contact_live::direct_call));
    }
    // SAFETY: argv is NUL-terminated, `owned` outlives the call, the host does not retain it after return.
    unsafe { tsfp_host_entry(owned.len() as c_int, pointers.as_mut_ptr()) }
}

fn cpu_seconds() -> f64 {
    let Ok(text) = std::fs::read_to_string("/proc/self/stat") else {
        return 0.0;
    };
    let tail = text.rsplit_once(')').map_or("", |(_, tail)| tail);
    let fields: Vec<&str> = tail.split_whitespace().collect();
    let ticks = |at: usize| {
        fields
            .get(at)
            .and_then(|field| field.parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    (ticks(11) + ticks(12)) / 100.0 // utime, stime (fields 14 and 15 of the line), 100 ticks per second
}

fn print_stats(wall: f64) {
    use shared::SHARED;
    let counters = &SHARED.counters;
    let received = counters.received.load(Ordering::Relaxed);
    let first = SHARED.first_picture_ms.load(Ordering::Relaxed);
    let last = SHARED.last_picture_ms.load(Ordering::Relaxed);
    let span = last.saturating_sub(first) as f64 / 1000.0;
    let rate = |count: u64| if span > 0.0 { count as f64 / span } else { 0.0 };
    let legacy_shown = counters.shown.load(Ordering::Relaxed);
    let frames = counters.bevy_frames.load(Ordering::Relaxed);
    eprintln!(
        "tsfp-bevy: pictures from the host {received} ({:.1}/s over {span:.1} s), replaced in the mailbox {}, \
         frames taken into Bevy {frames} ({:.1}/s of {wall:.1} s wall), legacy mailbox frames shown {legacy_shown}, \
         process cpu {:.1} s ({:.0}% of one core), \
         audio frames pulled by Bevy {} / by the fallback clock {}, Rust audio ring {} (overruns {}, underruns {})",
        rate(received),
        counters.replaced.load(Ordering::Relaxed),
        frames as f64 / wall.max(0.001),
        cpu_seconds(),
        cpu_seconds() / wall.max(0.001) * 100.0,
        audio::PULLED_BY_BEVY.load(Ordering::Relaxed),
        audio::PULLED_BY_FALLBACK.load(Ordering::Relaxed),
        audio::rust_ring_enabled(),
        audio::RUST_RING_OVERRUNS.load(Ordering::Relaxed),
        audio::RUST_RING_UNDERRUNS.load(Ordering::Relaxed),
    );
}

/// T2143: with `--perf-log` one stderr line per second from the Bevy side: pictures received from the host, replaced in the mailbox,
/// frames taken into Bevy, legacy mailbox frames shown, audio frames pulled by Bevy (48000 per second when the device keeps up)
/// and by the fallback clock, and process CPU. The host prints its own `perf t=` lines (frame gaps) next to these.
fn spawn_perf_ticker() {
    std::thread::Builder::new()
        .name("tsfp-perf".into())
        .spawn(|| {
            use shared::SHARED;
            let counters = &SHARED.counters;
            let read = || {
                [
                    counters.received.load(Ordering::Relaxed),
                    counters.replaced.load(Ordering::Relaxed),
                    counters.shown.load(Ordering::Relaxed),
                    counters.bevy_frames.load(Ordering::Relaxed),
                    audio::PULLED_BY_BEVY.load(Ordering::Relaxed),
                    audio::PULLED_BY_FALLBACK.load(Ordering::Relaxed),
                    audio::RUST_RING_OVERRUNS.load(Ordering::Relaxed),
                    audio::RUST_RING_UNDERRUNS.load(Ordering::Relaxed),
                ]
            };
            let (mut before, mut cpu_before, mut at) = (read(), cpu_seconds(), std::time::Instant::now());
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                let (now, cpu_now, when) = (read(), cpu_seconds(), std::time::Instant::now());
                let span = when.duration_since(at).as_secs_f64().max(0.001);
                let delta = |index: usize| now[index].saturating_sub(before[index]);
                eprintln!(
                    "perf bevy t={:.0}s pictures_from_host={} replaced={} mailbox_shown={} taken_into_bevy={} audio_by_bevy={} audio_by_fallback={} rust_audio_ring={} ring_overruns={} ring_underruns={} process_cpu={:.0}%",
                    shared::START.elapsed().as_secs_f64(),
                    delta(0),
                    delta(1),
                    delta(2),
                    delta(3),
                    delta(4),
                    delta(5),
                    audio::rust_ring_enabled(),
                    delta(6),
                    delta(7),
                    (cpu_now - cpu_before) / span * 100.0
                );
                (before, cpu_before, at) = (now, cpu_now, when);
            }
        })
        .expect("spawn the perf ticker");
}

fn print_wgpu_stats() {
    if let Some(line) = gpu_share::stats_line().or_else(wgpu_live::stats_line) {
        eprintln!("{line}");
    }
}

fn main() -> ExitCode {
    tsfp_memory_abi::link_anchor(wi::keep_exports());
    native_render_prep::enable_from_env();
    let mut arguments: Vec<String> = std::env::args().collect();
    let program = arguments.remove(0);
    let story_progress_enabled = story_progress_bridge::extract_switch(&mut arguments);
    storage_live::set_story_progress_enabled(story_progress_enabled);
    let unlock_all = arguments.iter().any(|argument| argument == "--unlock-all");
    match arguments.first().map(String::as_str) {
        Some("--t2-offline" | "--t2-construct") => return tsfp_t2_construct::dispatch(&arguments), // T3742, T3796
        Some("--bevy-info") => {
            print!("{INFO}");
            return ExitCode::SUCCESS;
        }
        Some("--bevy-help") => {
            println!(
                "usage: {program} [XBE] [host options...]\n\
                 Every option is the C host's. `--present window` and `--present bevy` mean the Bevy window (added when absent),\n\
                 Shell options (not forwarded): --input-router (opt-in, EXPERIMENTAL, T2160: per slot input router with the pad compatibility shim), --input-record PATH (T2348: capture fixed-tick native gameplay input; F12 adds marks), --input-replay PATH (T2348: replay a native input recording), --mouse-keyboard (needs --input-router: player 1 on mouse and keyboard, pointer captured, Ctrl+Alt or F10 releases it, Escape is the pad's START (pause toggle, T2178) and never releases it, Backspace is B, Alt+F4 quits), --input-bindings PATH (TOML subset, default tmp/input-bindings.toml), --input-preset NAME[:slot] (needs --input-router, T2167: original (default) | modern | mouse-raw | a user preset from the bindings file, repeatable), --aspect auto|4:3|16:9|21:9|<w>:<h> (default auto = what the game renders, 4:3; 16:9 or wider\n\
                 also adds the host's --eeprom-widescreen so the game renders its own 16:9, shown pillarboxed at 16:9 in wider windows),\n\
                 --filter nearest|linear (default linear), --wgpu-share (opt-in, default off, EXPERIMENTAL, T2229: share the wgpu device with Bevy and show the renderer frame as a GPU texture, no CPU readback, one startup line says path=gpu-texture or path=cpu-readback and why; needs a render runtime with the shared frame ABI, docs/t-wgpu-gpu-share.md), --wgpu-live (opt-in, default off, also --wgpu-live=cpu; --wgpu-live-solo also drops --gpu-live, experimental: the picture comes from the wgpu renderer of the\n\
                 render runtime at the window's pixel size, resized with the window, instead of the C renderer's 640x480; drops --gpu-live;\n\
                 needs a host built with -DTSFP_RUST_RT_HOST=ON; adapter args via TSFP_WGPU_LIVE_ARGS, default --publish, adapter by the rt-host policy or TSFP_WGPU_ADAPTER). The window is resizable; the picture keeps its aspect (bars), never stretched.\n\
                 --story-progress-rust opts into the typed Rust MissionOutcome -> authenticated Story profile bridge (default off).\n\
                 any other sink runs the game without a Bevy window. Extra: --bevy-info, --bevy-help (first argument only).\n\
                 Needs Vulkan (WGPU_BACKEND=vulkan is the default set here). Under Xvfb force lavapipe: \
                 VK_DRIVER_FILES=/usr/share/vulkan/icd.d/lvp_icd.json."
            );
            return ExitCode::SUCCESS;
        }
        _ => {}
    }
    // T2246: windowless consumer of the shared ring for measurement (docs/t-wgpu-renderer.md "T2227/T2229 A/B"). Implies --wgpu-share.
    let bench = match share_bench::extract(&mut arguments) {
        Ok(bench) => bench,
        Err(message) => {
            eprintln!("tsfp-bevy: {message}");
            return ExitCode::from(2);
        }
    };
    if bench.is_some() {
        arguments.push("--wgpu-share".to_string());
    }
    // T2136e: opt-in wgpu live picture at window size (docs/t-wgpu-default-presentation.md).
    let wgpu_live = wgpu_live::extract(&mut arguments);
    let options = match present::extract(&mut arguments) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("tsfp-bevy: {message}");
            return ExitCode::from(2);
        }
    };
    present::set_options(options);
    // T2160: opt-in input router (docs/t-input-rework-design.md).
    match input_router::extract(&mut arguments) {
        Ok(Some(options)) => {
            eprintln!("{}", input_router::banner(&options));
            input_router::set_options(options);
            tsfp_input_slots::ports::host_arguments(&mut arguments); // T3771: pads 2 to 4 are guest ports 1 to 3
        }
        Ok(None) => {}
        Err(message) => {
            eprintln!("tsfp-bevy: {message}");
            return ExitCode::from(2);
        }
    }
    // T2189 slice 15: opt-in Rust UI overlay (--ui=rust or TSFP_UI=rust); the --ui flags never reach the C host.
    let show_fps = fps::extract(
        &mut arguments,
        std::env::var("TSFP_SHOW_FPS").ok().as_deref(),
    );
    text_entry_live::init();
    hud_live::init(); // T2312: TSFP_HUD_NATIVE
    let ui_request = match ui_overlay::extract(&mut arguments, |name| std::env::var(name).ok()) {
        Ok(request) => request,
        Err(message) => {
            eprintln!("tsfp-bevy: {message}");
            return ExitCode::from(2);
        }
    };
    // Slice 16: only the inputs load here; the render world draws the page on the shell's own device.
    if let Some(request) = &ui_request {
        ui_overlay::prepare(request);
        ui_live::init();
    }
    let (mut arguments, windowed) = host_arguments(arguments);
    story_mission_runtime::init(&arguments);
    if let Err(error) = storage_live::init(&mut arguments) {
        eprintln!("tsfp-bevy: local storage: {error}");
        return ExitCode::from(2);
    }
    // Slice 1 of T2036: a 16:9 or wider request asks the game for its own 16:9 mode (the host's console video setting).
    // T2094: wider than 16:9 (and 16:10) is rendered by the game itself through the host's --aspect-override.
    if windowed && !arguments.iter().any(|a| a == "--aspect-override") {
        if let Some((num, den)) = present::plan(options.aspect).aspect_override {
            arguments.push("--aspect-override".to_string());
            arguments.push(format!("{num}:{den}"));
        }
    }
    if windowed
        && present::plan(options.aspect).game_widescreen
        && !arguments.iter().any(|a| a == "--eeprom-widescreen")
    {
        arguments.push("--eeprom-widescreen".to_string());
    }
    if wgpu_live.on && windowed && !cfg!(tsfp_no_host) {
        wgpu_live::set_enabled(true);
        let removed = if wgpu_live.solo {
            wgpu_live::strip_c_renderer(&mut arguments)
        } else {
            Vec::new()
        };
        // T2229: decide the path before the game thread starts (the runtime must get the shared device before it is created).
        let shared = if wgpu_live.share {
            Some(gpu_share::prepare())
        } else {
            None
        };
        let gpu_path = matches!(shared, Some(Ok(_)));
        // SAFETY: single threaded here, before the game thread and Bevy start.
        unsafe {
            std::env::set_var("TSFP_WGPU_LIVE", "1");
            // The GPU path never reads pixels back; the CPU path takes published frames.
            if std::env::var_os("TSFP_WGPU_LIVE_ARGS").is_none() && !gpu_path {
                std::env::set_var("TSFP_WGPU_LIVE_ARGS", "--publish");
            }
        }
        let dropped = if removed.is_empty() {
            String::new()
        } else {
            format!(
                " (dropped host option {}: the C Vulkan renderer is not used)",
                removed.join(" ")
            )
        };
        // ONE line says which path is active and why.
        match shared {
            Some(Ok(adapter)) => eprintln!(
                "tsfp-bevy: --wgpu-share: path=gpu-texture, picture from the wgpu renderer at window size shared with Bevy on {adapter}, no CPU readback{dropped}"
            ),
            Some(Err(reason)) => eprintln!(
                "tsfp-bevy: --wgpu-share: path=cpu-readback because {reason}; picture from the wgpu renderer at window size{dropped}"
            ),
            None => eprintln!(
                "tsfp-bevy: --wgpu-live: path=cpu-readback (requested), picture from the wgpu renderer at window size{dropped}"
            ),
        }
        // T2309: the Rust UI page is drawn by this renderer inside the frame (no overlay sprite).
        if ui_request.is_some() {
            let installed = ui_overlay::install_in_frame();
            if std::env::var_os("TSFP_UI_DISPLAY_RATE").is_some_and(|v| v == "1") {
                let enabled = installed && gpu_path;
                tsfp_wgpu::frame_layer::set_display_rate(enabled);
                eprintln!(
                    "tsfp-bevy: UI display-rate pages={} (requires native in-frame UI and shared GPU); HUD view-pass stays guest-rate",
                    enabled
                );
            }
        }
        // T2312: the native HUD is drawn by this renderer inside the frame too (under the page).
        hud_live::install_in_frame();
        if show_fps {
            fps::install(&arguments);
        }
    }
    if cfg!(tsfp_no_host) || !windowed {
        if input_router::options()
            .is_some_and(|options| options.record_path.is_some() || options.replay_path.is_some())
        {
            eprintln!("tsfp-bevy: native input recording and replay need a windowed host build");
            return ExitCode::from(2);
        }
        let status = run_then(|| run_host(&program, &arguments), hud_live::report);
        return ExitCode::from(status as u8);
    }
    if let Some(options) = input_router::options()
        && let Some(path) = options.replay_path.as_deref()
        && let Err(message) =
            input_recording::start_replay(path, unlock_all, options.replay_handover)
    {
        eprintln!("tsfp-bevy: input replay: {message}");
        return ExitCode::from(2);
    }
    if let Some(path) = input_router::options().and_then(|options| options.record_path.as_deref())
        && let Err(message) = input_recording::start(path, unlock_all)
    {
        eprintln!("tsfp-bevy: input recording: {message}");
        return ExitCode::from(2);
    }
    // SAFETY: single threaded here, before the game thread and Bevy start.
    unsafe {
        if std::env::var_os("WGPU_BACKEND").is_none() {
            std::env::set_var("WGPU_BACKEND", "vulkan");
        }
        // The C host's SDL is only used for the live renderer's Vulkan surface (no window of ours): the offscreen video
        // driver gives it a headless surface, so no second display connection and no hidden X window.
        if std::env::var_os("SDL_VIDEODRIVER").is_none() {
            std::env::set_var("SDL_VIDEODRIVER", "offscreen");
        }
        std::env::set_var("SDL_NO_SIGNAL_HANDLERS", "1");
    }
    std::sync::LazyLock::force(&shared::START);
    input_router::set_diag(arguments.iter().any(|argument| argument == "--perf-log"));
    if arguments.iter().any(|argument| argument == "--perf-log") {
        spawn_perf_ticker();
    }
    events::install_signal_handlers();
    let game = std::thread::Builder::new()
        .name("tsfp-game".into())
        .stack_size(256 << 20)
        .spawn(move || {
            let status = run_then(|| run_host(&program, &arguments), hud_live::report);
            GAME_STATUS.store(status, Ordering::Release);
            shared::SHARED.game_done.store(true, Ordering::Release);
        })
        .expect("spawn the game thread");
    if let Some(bench) = bench {
        share_bench::run(bench);
    } else {
        let _ = app::run();
    }
    if !shared::SHARED.game_done.load(Ordering::Acquire) {
        // Bevy ended first (window system failure): stop the guest the way a window close does.
        wi::request_quit(window_input::RawEvent::Quit);
    }
    let _ = game.join();
    if let Err(message) = input_recording::finish() {
        eprintln!("tsfp-bevy: input recording finish failed: {message}");
    }
    print_stats(shared::START.elapsed().as_secs_f64());
    ui_live::report();
    text_entry_live::report();
    storage_live::report();
    print_wgpu_stats();
    present_latency::report();
    frame_budget::report();
    ExitCode::from(GAME_STATUS.load(Ordering::Acquire) as u8)
}

#[cfg(test)]
mod tests {
    use super::host_arguments;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn present_bevy_becomes_the_window_sink() {
        let (arguments, windowed) =
            host_arguments(strings(&["a.xbe", "--present", "bevy", "--interactive"]));
        assert_eq!(
            (arguments, windowed),
            (
                strings(&["a.xbe", "--present", "window", "--interactive"]),
                true
            )
        );
    }

    #[test]
    fn absent_present_adds_the_window() {
        let (arguments, windowed) = host_arguments(strings(&["a.xbe"]));
        assert_eq!(
            (arguments, windowed),
            (strings(&["a.xbe", "--present", "window"]), true)
        );
    }

    #[test]
    fn other_sinks_run_without_a_bevy_window() {
        let (arguments, windowed) = host_arguments(strings(&["--present", "null"]));
        assert_eq!(
            (arguments, windowed),
            (strings(&["--present", "null"]), false)
        );
    }
}
