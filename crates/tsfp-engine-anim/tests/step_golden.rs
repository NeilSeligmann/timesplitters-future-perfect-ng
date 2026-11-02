//! T1668: `0x5ADC0` time step against the ORIGINAL (`tools/t1668_step_gold.py`).
#[path = "support/bits.rs"]
mod bits_support;
use bits_support::f32_bits as f;
use tsfp_engine_anim::tail::{apply_loop_root_offset, step_time};

#[test]
fn step_time_and_loop_offset_equal_the_original() {
    let text = std::fs::read_to_string(format!("{}/tests/data/t1668/step.golden", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let (mut total, mut wrapped, mut events) = (0, 0, 0);
    for line in text.lines() {
        let (head, tail) = line.split_once(" : ").unwrap();
        let h: Vec<&str> = head.split(' ').collect();
        let t: Vec<&str> = tail.split(' ').collect();
        let (duration, key_count, time, speed) = (f(h[1]), h[2].parse::<u32>().unwrap(), f(h[3]), f(h[4]));
        let (p0, p1) = (u32::from_str_radix(h[5], 16).unwrap(), u32::from_str_radix(h[6], 16).unwrap());
        let fires = h[7] == "1" && p1 & 0x0100_0000 != 0;
        let step = step_time(duration, time, speed, p0, fires).unwrap();
        assert_eq!(step.time.to_bits(), u32::from_str_radix(t[0], 16).unwrap(), "{line}");
        let want_previous = u32::from_str_radix(t[1], 16).unwrap();
        // untouched memory keeps its sentinel when the clip has no duration
        if duration > 0.0 {
            assert_eq!(step.previous_time.to_bits(), want_previous, "{line}");
            assert_eq!(step.wrapped.map(|w| format!("{w:x}")), Some(t[2].to_string()), "{line}");
        } else {
            assert_eq!(step.wrapped, None, "{line}");
        }
        assert_eq!(step.events.to_string(), t[6], "{line}");
        // root offset on a looping wrap: sample positions are f(key, i) = key * 1.5 + 0.25 * i + 1 for the stub
        let offs_in = [f(h[8]), f(h[9]), f(h[10])];
        let mut offs = offs_in;
        let _ = &mut offs;
        if step.wrapped == Some(1) && p0 & 1 != 0 {
            let pos = |key: u32, i: usize| key as f32 * 1.5 + 0.25 * i as f32 + 1.0;
            let first = [0, 1, 2].map(|i| pos(0, i));
            let last = [0, 1, 2].map(|i| pos(key_count - 1, i));
            offs = apply_loop_root_offset(offs_in, step.direction, first, last);
            let keys: Vec<u32> = if step.direction > 0 { vec![key_count - 1, 0] } else { vec![0, key_count - 1] };
            assert_eq!(keys.iter().map(u32::to_string).collect::<Vec<_>>().join(","), t[7], "{line}");
        } else {
            assert_eq!(t.get(7).copied().unwrap_or(""), "", "{line}");
        }
        for axis in 0..3 {
            assert_eq!(offs[axis].to_bits(), u32::from_str_radix(t[3 + axis], 16).unwrap(), "{line} axis {axis}");
        }
        total += 1;
        wrapped += usize::from(step.wrapped == Some(1));
        events += usize::from(step.events > 0);
    }
    assert!(total == 800 && wrapped > 100 && events > 10, "{total} {wrapped} {events}");
}
