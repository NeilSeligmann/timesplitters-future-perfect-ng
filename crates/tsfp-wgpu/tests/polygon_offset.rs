// SPDX-License-Identifier: GPL-3.0-or-later
//! T502 polygon-offset contract conformance on fabricated geometry.

#[path = "support/replay.rs"]
mod replay_support;
mod support;

use replay_support::replay_with;
use support::vertex;

fn triangle(colour: u32) -> String {
    [
        vertex(100.0, 100.0, colour, 0.0, 0.0),
        vertex(500.0, 100.0, colour, 0.0, 0.0),
        vertex(100.0, 400.0, colour, 0.0, 0.0),
    ]
    .concat()
}

fn overlapping_triangles(offset_second: bool) -> String {
    let offset = if offset_second {
        "call PushMethod 338 1\ncall PushMethod 384 0\ncall PushMethod 388 c9742400\n"
    } else {
        ""
    };
    format!(
        "mem 40002000 {}\nmem 40003000 {}\n\
         call PushMethod 30c 1\ncall PushMethod 354 201\ncall PushMethod 35c 1\n\
         host 19240 40002000 18\ncall DrawVertices 5 0 3\n\
         {offset}host 19240 40003000 18\ncall DrawVertices 5 0 3\npresent\n",
        triangle(0xFFFF_0000),
        triangle(0xFF00_00FF)
    )
}

#[test]
fn polygon_offset_pixel_conformance_remains_unresolved() {
    let Some(unbiased) = replay_with(&overlapping_triangles(false), |_| {}) else {
        return;
    };
    let Some(biased) = replay_with(&overlapping_triangles(true), |replay| {
        replay.knobs.insert("polygon_offset".into(), "1".into());
    }) else {
        return;
    };

    assert_eq!(unbiased.frames.len(), biased.frames.len());
    assert!(!unbiased.frames.is_empty());
    let center = (200 * 640 + 200) * 4;
    assert_ne!(
        &unbiased.frames[0].rgba[center..center + 4],
        &biased.frames[0].rgba[center..center + 4],
        "offset enabled for only the second overlapping triangle must change depth ordering"
    );
}
