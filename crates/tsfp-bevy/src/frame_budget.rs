// SPDX-License-Identifier: GPL-3.0-or-later
//! Opt-in CPU presenter budget. Durations include host scheduling, never imply GPU execution or scanout.
use bevy::prelude::*;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

static ENABLED: LazyLock<bool> =
    LazyLock::new(|| std::env::var("TSFP_FRAME_BUDGET").as_deref() == Ok("1"));
static STATE: Mutex<Option<Samples>> = Mutex::new(None);
const LIMIT: usize = 65536;

struct Samples {
    start: Instant,
    last: Option<Instant>,
    periods: Vec<u64>,
    render: Vec<u64>,
    handoff: Vec<u64>,
}
impl Samples {
    fn new(now: Instant) -> Self {
        Self {
            start: now,
            last: None,
            periods: Vec::new(),
            render: Vec::new(),
            handoff: Vec::new(),
        }
    }
    fn warm(&self, now: Instant) -> bool {
        now.duration_since(self.start) >= Duration::from_secs(5)
    }
}
fn push(values: &mut Vec<u64>, ns: u64) {
    if values.len() < LIMIT {
        values.push(ns);
    }
}
fn percentiles(values: &[u64]) -> [f64; 3] {
    let mut values = values.to_vec();
    values.sort_unstable();
    [50, 99, 100].map(|p| {
        values
            .get((values.len() * p).div_ceil(100).saturating_sub(1))
            .copied()
            .unwrap_or(0) as f64
            / 1e6
    })
}
pub fn handoff(elapsed: Duration) {
    if !*ENABLED {
        return;
    }
    let now = Instant::now();
    let mut state = STATE.lock().unwrap();
    let samples = state.get_or_insert_with(|| Samples::new(now));
    if samples.warm(now) {
        push(&mut samples.handoff, elapsed.as_nanos() as u64);
    }
}
fn before(windows: Res<bevy::render::view::ExtractedWindows>, mut started: ResMut<Started>) {
    started.0 = windows
        .windows
        .values()
        .any(|w| w.swap_chain_texture.is_some())
        .then(Instant::now);
}
#[derive(Resource, Default)]
struct Started(Option<Instant>);
fn after(mut started: ResMut<Started>) {
    let Some(before) = started.0.take() else {
        return;
    };
    let now = Instant::now();
    let mut state = STATE.lock().unwrap();
    let samples = state.get_or_insert_with(|| Samples::new(now));
    if samples.warm(now) {
        if let Some(last) = samples.last {
            push(
                &mut samples.periods,
                now.duration_since(last).as_nanos() as u64,
            );
        }
        push(
            &mut samples.render,
            now.duration_since(before).as_nanos() as u64,
        );
        samples.last = Some(now);
    }
}
fn focus(windows: Query<&bevy::window::Window>, mut previous: Local<Option<bool>>) {
    let focused = windows.iter().any(|window| window.focused);
    if *previous != Some(focused) {
        eprintln!(
            "tsfp-bevy: frame-budget focused={focused} (Bevy game policy: focused continuous, unfocused 60Hz)"
        );
        *previous = Some(focused);
    }
}
pub fn install(app: &mut App) {
    if !*ENABLED {
        return;
    }
    app.add_systems(Update, focus);
    if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
        use bevy::render::{Render, RenderSet};
        render
            .init_resource::<Started>()
            .add_systems(
                Render,
                before.after(RenderSet::Prepare).before(RenderSet::Render),
            )
            .add_systems(
                Render,
                after.after(RenderSet::Render).before(RenderSet::Cleanup),
            );
    }
}
pub fn report() {
    if !*ENABLED {
        return;
    }
    let state = STATE.lock().unwrap();
    let Some(samples) = state.as_ref() else {
        return;
    };
    let seconds = samples
        .periods
        .iter()
        .map(|&ns| ns as f64 / 1e9)
        .sum::<f64>();
    eprintln!(
        "tsfp-bevy: frame-budget warmup=5s submissions={} rate={:.1}/s period_ms={:?} render_cpu_ms={:?} handoff_cpu_ms={:?} handoffs={} (p50/p99/max; CPU timings include scheduling; no GPU/scanout claim)",
        samples.render.len(),
        samples.periods.len() as f64 / seconds.max(0.001),
        percentiles(&samples.periods),
        percentiles(&samples.render),
        percentiles(&samples.handoff),
        samples.handoff.len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn budget_percentiles_keep_tail_and_empty_is_zero() {
        assert_eq!(percentiles(&[]), [0.0; 3]);
        assert_eq!(
            percentiles(&[9_000_000, 1_000_000, 2_000_000]),
            [2.0, 9.0, 9.0]
        );
        let mut samples = Vec::new();
        for n in 0..LIMIT + 1 {
            push(&mut samples, n as u64);
        }
        assert_eq!(samples.len(), LIMIT);
    }
}
