// SPDX-License-Identifier: GPL-3.0-or-later
//! Exact extracted-frame association for handoff -> window-submit diagnostics.
//! This measures CPU submission, never GPU completion or physical scanout.
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use std::sync::{LazyLock, Mutex};
use std::time::Instant;

#[derive(Clone, Default, Resource, ExtractResource)]
struct Stamp {
    frame: u64,
    at: Option<Instant>,
}
static LATEST: Mutex<Stamp> = Mutex::new(Stamp { frame: 0, at: None });
static ENABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("TSFP_PRESENT_LATENCY").as_deref() == Ok("1"));
static SAMPLES: Mutex<Vec<u64>> = Mutex::new(Vec::new());
pub fn handoff(frame: u64) {
    if *ENABLED {
        *LATEST.lock().unwrap() = Stamp {
            frame,
            at: Some(Instant::now()),
        };
    }
}
fn sync_stamp(mut stamp: ResMut<Stamp>) {
    *stamp = LATEST.lock().unwrap().clone();
}
fn submitted(stamp: Res<Stamp>, mut last: Local<Option<u64>>) {
    // swap_chain_texture is consumed by Bevy's render_system. Use the acquired
    // flag from a separate pre-render system, not availability after submission.
    if !ACQUIRED.swap(false, std::sync::atomic::Ordering::Relaxed) || *last == Some(stamp.frame) {
        return;
    }
    if let Some(at) = stamp.at {
        *last = Some(stamp.frame);
        let mut samples = SAMPLES.lock().unwrap();
        if samples.len() < 65536 {
            samples.push(at.elapsed().as_nanos() as u64);
        }
        let periodic = samples.len() % 120 == 0;
        drop(samples);
        if periodic {
            report();
        }
    }
}
static ACQUIRED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
fn acquired(windows: Res<bevy::render::view::window::ExtractedWindows>) {
    ACQUIRED.store(
        windows
            .windows
            .values()
            .any(|w| w.swap_chain_texture.is_some()),
        std::sync::atomic::Ordering::Relaxed,
    );
}
pub fn install(app: &mut App) {
    if !*ENABLED {
        return;
    }
    app.init_resource::<Stamp>()
        .add_plugins(ExtractResourcePlugin::<Stamp>::default())
        .add_systems(PostUpdate, sync_stamp);
    if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
        use bevy::render::{Render, RenderSet};
        render.add_systems(
            Render,
            acquired.after(RenderSet::Prepare).before(RenderSet::Render),
        );
        render.add_systems(
            Render,
            submitted
                .after(RenderSet::Render)
                .before(RenderSet::Cleanup),
        );
    }
}
pub fn report() {
    if !*ENABLED {
        return;
    }
    let mut samples = SAMPLES.lock().unwrap().clone();
    samples.sort_unstable();
    let percentile = |p: usize| {
        samples
            .get(samples.len().saturating_sub(1) * p / 100)
            .copied()
            .unwrap_or(0) as f64
            / 1e6
    };
    eprintln!(
        "tsfp-bevy: presentation handoff-to-window-submit samples={} p50={:.3}ms p99={:.3}ms max={:.3}ms (CPU submission; excludes input, guest/render queue, GPU completion and scanout)",
        samples.len(),
        percentile(50),
        percentile(99),
        percentile(100)
    );
}
