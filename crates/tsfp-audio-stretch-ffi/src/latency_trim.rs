// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline model of the legacy sink's latency-trimming hysteresis.

pub fn excess(fill: usize, target: usize, hysteresis: usize, trimming: &mut bool) -> usize {
    if target == 0 {
        return 0;
    }
    if fill > target.wrapping_add(hysteresis) {
        *trimming = true;
    } else if fill <= target {
        *trimming = false;
    }
    if *trimming {
        fill.wrapping_sub(target)
    } else {
        0
    }
}

/// Model the production sink's silence removal on a valid interleaved ring.
pub fn skip_silence(ring: &[i16], channels: usize, head: &mut usize, fill: &mut usize, excess: usize) -> usize {
    assert!(channels > 0 && ring.len() % channels == 0);
    let frames = ring.len() / channels;
    assert!(frames > 0 && *head < frames && *fill <= frames);
    let mut skipped = 0;
    while skipped < excess && skipped < *fill {
        let mut run = 0;
        while skipped + run < *fill {
            let index = (*head + run) % frames * channels;
            if ring[index..index + channels].iter().any(|sample| *sample != 0) {
                break;
            }
            run += 1;
        }
        if run < 128 {
            break;
        }
        run = run.min(excess - skipped);
        *head = (*head + run) % frames;
        *fill -= run;
        skipped += run;
    }
    skipped
}
