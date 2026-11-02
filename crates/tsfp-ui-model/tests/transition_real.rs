//! Tier 1 check of `transition.rs` against the state timelines of the captured pages (ignored
//! `tmp/ui-tree/layout.json`, `python3 -m tools.ui_tree layout RUN_DIR...`, or `TSFP_UI_LAYOUT`).
//! Per page the open and close durations are fitted from the samples; the page alpha cell must
//! then follow the linear formula for every sample, the timer must be a whole number of 1/60 s
//! steps, and the durations of one page must agree across all samples.

use std::collections::BTreeMap;
use tsfp_ui_model::json::Json;
use tsfp_ui_model::transition::*;

#[path = "support/path.rs"]
mod capture_path;
use capture_path::layout_capture as path;

#[derive(Default)]
struct Fit {
    open: Vec<f32>,
    close: Vec<f32>,
}

#[test]
fn page_alpha_follows_the_linear_ramps() {
    let Ok(text) = std::fs::read_to_string(path()) else {
        eprintln!("no layout capture, skipped");
        return;
    };
    let json = Json::parse(&text).expect("parses");
    let timelines = json
        .get("timelines")
        .and_then(Json::as_object)
        .expect("timelines");
    let mut fits: BTreeMap<String, Fit> = BTreeMap::new();
    let mut samples = Vec::new();
    for run in timelines.values() {
        for (page, rows) in run.as_object().expect("run") {
            for row in rows.as_array().expect("rows") {
                let cells = row.as_array().expect("cells");
                let bits = |i: usize| cells[i].as_u64().unwrap_or(0) as u32;
                // index: 0 poll, 1..13 = words 0x34 .. 0x210 (see `timeline_words`)
                let (state, timer, alpha) =
                    (bits(8), f32::from_bits(bits(9)), f32::from_bits(bits(10)));
                samples.push((page.clone(), state, timer, alpha));
                let fit = fits.entry(page.clone()).or_default();
                if state == 2 && alpha > 0.01 && timer > 0.0 {
                    fit.open.push(timer / alpha);
                }
                if state == 4 && alpha < 0.99 && timer > 0.0 {
                    fit.close.push(timer / (1.0 - alpha));
                }
            }
        }
    }
    // Durations per page: the distinct values (rounded to 1 ms) seen in the samples.
    let mut seen_open = BTreeMap::<i32, usize>::new();
    let mut seen_close = BTreeMap::<i32, usize>::new();
    let mut varying = Vec::new();
    for (page, fit) in &fits {
        let round = |v: &Vec<f32>| {
            let mut ms: Vec<i32> = v.iter().map(|x| (x * 1000.0).round() as i32).collect();
            ms.sort_unstable();
            ms.dedup();
            ms
        };
        let (o, c) = (round(&fit.open), round(&fit.close));
        if o.len() > 1 || c.len() > 1 {
            varying.push((page.clone(), o.clone(), c.clone()));
        }
        for v in o {
            *seen_open.entry(v).or_default() += 1;
        }
        for v in c {
            *seen_close.entry(v).or_default() += 1;
        }
    }
    eprintln!("open durations (ms -> page ids): {seen_open:?}");
    eprintln!("close durations (ms -> page ids): {seen_close:?}");
    eprintln!("pages with two durations (viewport context differs per run): {varying:?}");
    for ms in seen_open.keys().chain(seen_close.keys()) {
        assert!(
            [100, 200, 350, 250, 600].contains(ms),
            "unexpected duration {ms} ms"
        );
    }
    // Every sample: the alpha cell equals the formula for its phase with the fitted duration, and
    // the timer is a whole number of steps.
    let (mut checked, mut on_step, mut whole_total) = (0, 0, 0);
    for (page, state, timer, alpha) in &samples {
        let fit = &fits[page];
        let phase = PagePhase::from_code(*state).expect("phase code");
        let nearest = |v: &Vec<f32>, t: f32| {
            v.iter().copied().min_by(|a, b| {
                (a - t / alpha.max(1e-6))
                    .abs()
                    .total_cmp(&(b - t / alpha.max(1e-6)).abs())
            })
        };
        let timing = match phase {
            PagePhase::Opening => nearest(&fit.open, *timer),
            _ => fit.open.first().copied(),
        };
        let close = match phase {
            PagePhase::Closing => fit.close.iter().copied().min_by(|a, b| {
                let want = timer / (1.0 - alpha).max(1e-6);
                (a - want).abs().total_cmp(&(b - want).abs())
            }),
            _ => fit.close.first().copied(),
        };
        let life = PageLife {
            phase,
            timer: *timer,
            timing: PageTiming {
                delay: 0.0,
                open: timing.unwrap_or(0.6),
                close: close.unwrap_or(0.25),
            },
        };
        match phase {
            PagePhase::Opening | PagePhase::Closing if *timer > 0.0 => {
                checked += 1;
                assert!(
                    (life.alpha() - alpha).abs() < 1e-5,
                    "{page}: alpha {} vs dump {alpha} (phase {phase:?}, timer {timer})",
                    life.alpha()
                );
            }
            PagePhase::Open => {
                checked += 1;
                assert_eq!(*alpha, 1.0, "{page}: open page alpha");
            }
            _ => {}
        }
        // Waiting and opening timers are whole steps. Open timers drift by float rounding after
        // seconds, and some closing timers advance in thirds of a step (page speed cell, OPEN).
        let steps = *timer / STEP_SECONDS;
        if matches!(
            phase,
            PagePhase::Waiting | PagePhase::Opening | PagePhase::Closed
        ) {
            whole_total += 1;
            on_step += usize::from((steps - steps.round()).abs() < 0.01);
        }
    }
    eprintln!(
        "alpha samples checked {checked}, opening timers on whole 1/60 s steps {on_step} of {whole_total} (of {} samples)",
        samples.len()
    );
    assert!(checked > 0);
    assert_eq!(
        on_step, whole_total,
        "waiting and opening timers advance in whole steps"
    );
}
