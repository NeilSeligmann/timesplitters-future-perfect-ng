//! T1668: `0x5B720` against the ORIGINAL (`tools/t1668_probe_gold.py`).
#[path = "support/bits.rs"]
mod bits_support;
use bits_support::f32_bits as f;
use tsfp_engine_anim::aim::{MoveQuery, move_probe};

#[test]
fn move_probe_equals_the_original() {
    let text = std::fs::read_to_string(format!("{}/tests/data/t1668/probe.golden", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let (mut total, mut refused, mut probed) = (0, 0, 0);
    for line in text.lines() {
        let (head, tail) = line.split_once(" : ").unwrap();
        let h: Vec<&str> = head.split(' ').collect();
        let keys: i32 = h[8].parse().unwrap();
        let t = f(h[7]);
        // the generator's sampler: pos[i] = key * 0.75 + node(0) + i * 0.5 - 3
        let sample = |key: i32| [0, 1, 2].map(|i| (key as f32 * 0.75 + i as f32 * 0.5 - 3.0));
        let end_key = ((keys - 1) as f32 * t) as i32;
        let query = MoveQuery {
            yaw_degrees: f(h[1]),
            position: [f(h[2]), f(h[3]), f(h[4])],
            level: h[5].parse().unwrap(),
            level_threshold: h[6].parse().unwrap(),
            start: sample(0),
            end: sample(end_key),
            target: [f(h[10]), f(h[11])],
            radius: f(h[9]),
        };
        let probe_flags = u32::from_str_radix(h[12], 16).unwrap();
        let probe_vec = [f(h[13]), f(h[14]), f(h[15])];
        let mut probe_called = false;
        let (flags, out) = move_probe(&query, |_| {
            probe_called = true;
            (probe_flags, probe_vec)
        });
        let w: Vec<&str> = tail.split(' ').collect();
        assert_eq!(format!("{flags:x}"), w[0], "{line}");
        if h[16] == "1" {
            let got: Vec<String> = out.iter().map(|v| format!("{:08x}", v.to_bits())).collect();
            assert_eq!(got, w[1..4], "{line}");
        }
        let probes = w[w.len() - 2];
        assert_eq!(probe_called, probes == "1", "{line}");
        total += 1;
        refused += usize::from(!probe_called);
        probed += usize::from(probe_called);
    }
    assert!(total == 800 && refused > 20 && probed > 100, "{total} {refused} {probed}");
}
