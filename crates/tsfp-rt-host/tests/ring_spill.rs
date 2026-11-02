// SPDX-License-Identifier: GPL-3.0-or-later
//! T2292: on the nonblocking ring (`TSFP_RT_HOST_FAST_HOOKS=1`) a renderer slower than the guest no longer switches the live
//! renderer off when the ring is full: the batches wait in an ordered spill, every frame is replayed, and `frames_behind`
//! reports the backlog the guest uses to shed replay frames. Own test binary because the ring is chosen by an environment
//! variable. T2295: a live display drops the backlog at close. Needs a wgpu adapter (llvmpipe is enough).
use tsfp_wgpu::binary::text_to_binary;

const TITLE: &str = include_str!("../../tsfp-wgpu/tests/data/title.stream");

mod support;
use support::start;

fn frame_binary(number: u64) -> Vec<u8> {
    let body: String = TITLE
        .lines()
        .filter(|line| line.trim() != "present")
        .map(|line| format!("{line}\n"))
        .collect();
    text_to_binary(&format!("{body}# present {number}\npresent\n")).expect("convert")
}

#[test]
fn a_full_ring_spills_in_order_and_never_gives_up() {
    // SAFETY: the only test in this binary, set before any runtime starts.
    unsafe { std::env::set_var("TSFP_RT_HOST_FAST_HOOKS", "1") };
    let rt = start("--any-adapter --quiet --render-delay-ms 30 --queue 2");
    for number in 1..=40 {
        rt.write_binary(&frame_binary(number), 1);
    }
    assert!(rt.alive(), "{:?}", rt.stats());
    assert!(
        rt.spilled_batches() > 0,
        "the 8 slot ring should have filled"
    );
    assert!(rt.frames_behind() > 8, "backlog {}", rt.frames_behind());
    let stats = rt.finish();
    assert!(!stats.gave_up, "{stats:?}");
    assert!(!stats.render_error, "{stats:?}");
    assert_eq!(
        stats.presents, 40,
        "every spilled frame reaches the renderer: {stats:?}"
    );
    assert_eq!(rt.frames_behind(), 0);

    // the memory cap still ends a hopeless backlog
    let rt = start("--any-adapter --quiet --render-delay-ms 200 --queue 2 --queue-mb 1");
    for number in 1..=200 {
        rt.write_binary(&frame_binary(number), 1);
    }
    let stats = rt.finish();
    assert!(stats.gave_up, "{stats:?}");
    // T2293: with producer omission configured the valve opens at half the cap, before the cap ends the renderer, and closes
    // again once the renderer is back within `drop_behind` frames
    let rt =
        start("--any-adapter --quiet --render-delay-ms 100 --queue 2 --queue-mb 1 --drop-behind 2");
    assert!(!rt.pressure(), "no backlog yet");
    let mut opened = false;
    for number in 1..=200 {
        rt.write_binary(&frame_binary(number), 1);
        if rt.pressure() {
            opened = true;
            break;
        }
    }
    assert!(opened, "valve never opened: {:?}", rt.stats());
    assert!(
        rt.alive(),
        "the valve opens before the cap: {:?}",
        rt.stats()
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    // the guest keeps presenting while the valve is open, with its draws omitted (an empty frame); writes also drain the spill
    let mut number = 1000;
    while rt.pressure() && std::time::Instant::now() < deadline {
        number += 1;
        let empty = text_to_binary(&format!("# present {number}\npresent\n")).expect("convert");
        rt.write_binary(&empty, 1);
        // slower than the 100 ms renderer, as a caught-up renderer is
        std::thread::sleep(std::time::Duration::from_millis(150));
    }
    assert!(
        !rt.pressure(),
        "valve closes once caught up, behind {}",
        rt.frames_behind()
    );
    assert_eq!(rt.valve_episodes(), 1);
    let stats = rt.finish();
    assert!(!stats.gave_up, "{stats:?}");

    // T2295: a live display (drop_behind set) drops the backlog at close instead of drawing 30 frames nobody sees (3 s here)
    let rt = start("--any-adapter --quiet --render-delay-ms 100 --queue 2 --drop-behind 2");
    for number in 1..=30 {
        rt.write_binary(&frame_binary(number), 1);
    }
    let closing = std::time::Instant::now();
    let stats = rt.finish();
    assert!(
        closing.elapsed() < std::time::Duration::from_millis(1500),
        "close took {:?}",
        closing.elapsed()
    );
    assert!(stats.presents > 0 && stats.presents < 30, "{stats:?}");
    assert!(!stats.render_error, "{stats:?}");
}
