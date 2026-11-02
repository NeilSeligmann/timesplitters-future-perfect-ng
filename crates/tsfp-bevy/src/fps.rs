// SPDX-License-Identifier: GPL-3.0-or-later
//! Opt-in native presented-frame diagnostics. No guest state or synthetic input.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tsfp_ui_model::hud_native::{HudDraw, HudElement, HudText};
use tsfp_ui_renderer::{Color, Rect, Vec2};
use tsfp_wgpu::frame_layer::{FrameLayer, LayerTarget};

#[derive(Default)]
struct Rate {
    start: Option<Instant>,
    game_frames: u64,
    presents: u64,
    last_game: Option<u64>,
    fps: Option<(f64, f64)>,
}
impl Rate {
    fn game_frame(&mut self, frame: u64) {
        if self.last_game != Some(frame) {
            self.last_game = Some(frame);
            self.game_frames += 1;
        }
    }
    fn tick(&mut self, now: Instant) {
        let Some(start) = self.start else {
            self.start = Some(now);
            self.game_frames = 0;
            return;
        };
        self.presents += 1;
        let elapsed = now.duration_since(start);
        if elapsed >= Duration::from_millis(500) {
            let seconds = elapsed.as_secs_f64();
            let game_fps = self.game_frames as f64 / seconds;
            let present_fps = self.presents as f64 / seconds;
            self.fps = Some((game_fps, present_fps));
            eprintln!("tsfp-bevy: fps_sample game={game_fps:.3} present={present_fps:.3}");
            self.game_frames = 0;
            self.presents = 0;
            self.start = Some(now);
        }
    }
    fn label(&self) -> String {
        self.fps.map_or_else(
            || "game -- / present -- fps".into(),
            |(game, present)| format!("game {game:.1} / present {present:.1} fps"),
        )
    }
}
static RATE: Mutex<Rate> = Mutex::new(Rate {
    start: None,
    game_frames: 0,
    presents: 0,
    last_game: None,
    fps: None,
});
/// One new scene image submitted by the shell to its display texture. Repeated
/// window presentations (including display-rate UI updates) do not count here.
pub fn game_frame(frame: u64) {
    if ENABLED.get() == Some(&true) {
        RATE.lock().unwrap().game_frame(frame);
    }
}
static ENABLED: OnceLock<bool> = OnceLock::new();
static WINDOW_FRAME: AtomicBool = AtomicBool::new(false);
pub fn acquired(windows: bevy::prelude::Res<bevy::render::view::window::ExtractedWindows>) {
    if ENABLED.get() != Some(&true) {
        return;
    }
    WINDOW_FRAME.store(
        windows
            .windows
            .values()
            .any(|window| window.swap_chain_texture.is_some()),
        Ordering::Relaxed,
    );
}
/// Called after Bevy submits its window frame, rather than on guest ticks or Update.
/// This counts submitted presentations, not physical monitor scanout (Mailbox may discard).
pub fn presented() {
    if WINDOW_FRAME.swap(false, Ordering::Relaxed) && ENABLED.get() == Some(&true) {
        RATE.lock().unwrap().tick(Instant::now());
    }
}
pub fn extract(args: &mut Vec<String>, env: Option<&str>) -> bool {
    let flag = args.iter().any(|s| s == "--show-fps");
    args.retain(|s| s != "--show-fps");
    let enabled = flag || env == Some("1");
    let _ = ENABLED.set(enabled);
    enabled
}
fn draw(width: u32, height: u32, label: String) -> HudDraw {
    let scale = (height as f32 / 480.0).max(0.01);
    let margin = 8.0 * scale;
    HudDraw {
        texts: vec![HudText {
            clip_px: None,
            element: HudElement::Whole,
            bytes: label.into_bytes(),
            font: 1,
            rect_px: Rect {
                origin: Vec2::new((width as f32 - 300.0 * scale - margin).max(0.0), margin),
                size: Vec2::new(300.0 * scale, 20.0 * scale),
            },
            align: tsfp_ui_model::Align::End,
            vertical_align: tsfp_ui_model::Align::Start,
            color: Color([1.0, 1.0, 1.0, 1.0]),
            scale: [1.0, 1.0],
            additive: false,
        }],
        text_scale: [0.4, 0.4],
        ..HudDraw::default()
    }
}
type Source = fn(u32, u32) -> Option<HudDraw>;
struct Layer {
    hud: tsfp_ui_wgpu::hud::HudLayer<Source>,
    display: bool,
}
impl Layer {
    fn record(&mut self, target: LayerTarget<'_>) -> bool {
        let label = RATE.lock().unwrap().label();
        let frame = draw(target.target.width(), target.target.height(), label);
        self.hud.record(target, &frame, false)
    }
}
impl FrameLayer for Layer {
    fn encode(&mut self, target: LayerTarget<'_>) -> bool {
        if self.display {
            false
        } else {
            self.record(target)
        }
    }
    fn wants_display(&self) -> bool {
        self.display
    }
    fn encode_display(&mut self, target: LayerTarget<'_>) -> bool {
        self.record(target)
    }
}
fn disc_argument(arguments: &[String]) -> Option<&str> {
    arguments.iter().enumerate().find_map(|(i, arg)| {
        if arg == "--disc" {
            arguments.get(i + 1).map(String::as_str)
        } else {
            arg.strip_prefix("--disc=")
        }
    })
}
pub fn install(arguments: &[String]) {
    // Separate runtime original-font load: FPS does not require --ui=rust or a menu tree.
    let disc = std::env::var("TSFP_UI_DISC")
        .ok()
        .or_else(|| disc_argument(arguments).map(str::to_owned))
        .or_else(|| std::env::var("TSFP_DISC").ok());
    let Some(disc) = disc else {
        eprintln!("tsfp-bevy: FPS unavailable: --disc/TSFP_UI_DISC/TSFP_DISC unset");
        return;
    };
    let text = match tsfp_ui_wgpu::overlay::TextAssets::from_disc(std::path::Path::new(&disc), 'e')
    {
        Ok(text) => text,
        Err(e) => {
            eprintln!("tsfp-bevy: FPS original font unavailable: {e}");
            return;
        }
    };
    let assets = Box::leak(Box::new(tsfp_ui_wgpu::hud::HudAssets {
        text: Some(text),
        ..Default::default()
    }));
    let hud = tsfp_ui_wgpu::hud::HudLayer::new_original(assets, (|_, _| None) as Source);
    tsfp_wgpu::frame_layer::install(
        "fps",
        100,
        Box::new(Layer {
            hud,
            display: crate::gpu_share::gpu().is_some(),
        }),
    );
    eprintln!(
        "tsfp-bevy: FPS enabled: native top-right frame text, original disc font; game fps / present fps: distinct guest images submitted to display / window submissions per second (not simulation ticks or monitor scanout)"
    );
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_disc_is_available_from_direct_launcher_arguments() {
        assert_eq!(
            disc_argument(&["--disc".into(), "owner.iso".into()]),
            Some("owner.iso")
        );
        assert_eq!(
            disc_argument(&["--disc=owner.iso".into()]),
            Some("owner.iso")
        );
        assert_eq!(disc_argument(&["--disc".into()]), None);
    }
    #[test]
    fn rate_counts_window_frames_and_updates_twice_per_second() {
        let now = Instant::now();
        let mut rate = Rate::default();
        assert_eq!(rate.label(), "game -- / present -- fps");
        rate.tick(now);
        for n in 1..=60 {
            // Every guest scene appears twice at a 100 Hz presentation rate.
            rate.game_frame(n / 2);
            rate.game_frame(n / 2);
            rate.tick(now + Duration::from_millis(n * 10));
        }
        assert_eq!(rate.label(), "game 52.0 / present 100.0 fps");
        // A frozen scene still presents, and the scene rate becomes zero.
        for n in 61..=110 {
            rate.game_frame(30);
            rate.tick(now + Duration::from_millis(n * 10));
        }
        assert_eq!(rate.label(), "game 10.0 / present 100.0 fps");
        for n in 111..=160 {
            rate.game_frame(30);
            rate.tick(now + Duration::from_millis(n * 10));
        }
        assert_eq!(rate.label(), "game 0.0 / present 100.0 fps");
    }
    #[test]
    fn top_right_original_font_and_flag_removed_from_host() {
        let frame = draw(1920, 1080, "FPS 60.0".into());
        let text = &frame.texts[0];
        assert_eq!(text.font, 1);
        assert_eq!(text.align, tsfp_ui_model::Align::End);
        assert_eq!(text.rect_px.origin.y, 18.0);
        assert_eq!(text.rect_px.origin.x + text.rect_px.size.x, 1902.0);
        let mut args = vec!["--show-fps".into(), "--mute".into()];
        assert!(extract(&mut args, None));
        assert_eq!(args, ["--mute"]);
        assert!(!extract(&mut Vec::new(), None));
        assert!(extract(&mut Vec::new(), Some("1")));
        assert!(!extract(&mut Vec::new(), Some("0")));
    }
}
