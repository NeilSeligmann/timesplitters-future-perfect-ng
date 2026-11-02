//! Source-contract tests on synthetic guest state; no claim of original-oracle comparison.
use std::collections::BTreeMap;
use tsfp_gameplay::story_events::{
    eff_guest::{Guest, RNG_COUNTER, RNG_STATE},
    eff_late::flare_draw,
};
#[derive(Default)]
struct Host {
    words: BTreeMap<u32, u32>,
    calls: Vec<(u32, Vec<u32>)>,
}
impl Guest for Host {
    fn read(&mut self, a: u32) -> u32 {
        *self.words.get(&a).unwrap_or(&0)
    }
    fn write(&mut self, a: u32, v: u32) {
        self.words.insert(a, v);
    }
    fn call(&mut self, a: u32, args: &[u32]) -> u32 {
        self.calls.push((a, args.to_vec()));
        if a == 0x80F20 { 0x9000 } else { 0 }
    }
    fn call_f(&mut self, _: u32, _: &[u32]) -> f32 {
        panic!("unexpected float leaf")
    }
}
fn host(age: f32) -> Host {
    let mut h = Host::default();
    h.write(0x1004, 1);
    h.writef(0x1098, age);
    h.writef(0x109C, 1.0);
    h
}
#[test]
fn static_flare_keeps_size_colour_call_order_and_rng() {
    let mut h = Host::default();
    h.write(RNG_STATE, 123);
    flare_draw(&mut h, 0x1000);
    assert_eq!(
        h.calls,
        vec![
            (0x19D70, vec![2]),
            (0x19650, vec![2]),
            (0x80F20, vec![0x28D]),
            (
                0x107B60,
                vec![
                    0x9000,
                    0x101C,
                    200f32.to_bits(),
                    200f32.to_bits(),
                    0,
                    0x7F6FBAFF,
                    0
                ]
            )
        ]
    );
    assert_eq!(h.read(RNG_STATE), 123);
    assert_eq!(h.read(RNG_COUNTER), 0);
}
#[test]
fn live_flare_fades_sizes_and_advances_rng_once() {
    let mut h = host(0.5);
    flare_draw(&mut h, 0x1000);
    assert_eq!(h.read(RNG_COUNTER), 1);
    assert_eq!(h.read(RNG_STATE), 0x269EC3);
    // Period 2 and the first LCG output is odd: the draw still occurs with colour zero.
    assert_eq!(
        h.calls[3],
        (
            0x107B60,
            vec![0x9000, 0x101C, 1105f32.to_bits(), 850f32.to_bits(), 0, 0, 0]
        )
    );
}
#[test]
fn live_flare_flicker_packs_alpha_in_green_and_red() {
    let mut h = host(0.0);
    flare_draw(&mut h, 0x1000);
    assert_eq!(h.calls[3].1[5], 0x7FFFFFFF);
    assert_eq!(h.calls[3].1[3], 1700f32.to_bits());
    assert_eq!(h.calls[3].1[2], 1700f32.to_bits());
}
#[test]
#[should_panic]
fn zero_flicker_period_retains_division_fault() {
    flare_draw(&mut host(-0.5), 0x1000);
}
#[test]
fn late_loop_dispatches_flare_instead_of_recording_callback() {
    use tsfp_gameplay::story_events::{
        svc_pool_classes::{ClassSlot, ClassTable},
        svc_wiring::{HookServices, TraceEvent},
    };
    let mut s = HookServices::new(1);
    let mut records = vec![[0; 16]; 0x30];
    records[0x26][ClassSlot::Late.offset() as usize / 4] = 0xE3270;
    s.classes = ClassTable { records };
    s.ember.pool.entries[0].w[0] = 0x26;
    s.tick_effects(1);
    assert!(s.trace().iter().any(|e| matches!(
        e,
        TraceEvent::Ember {
            service: "effect_late",
            ..
        }
    )));
    assert!(!s.trace().iter().any(|e| matches!(
        e,
        TraceEvent::Ember {
            service: "class_callback",
            ..
        }
    )));
}

mod common;
#[test]
fn all_callbacks_match_original_state_and_renderer_trace_on_synthetic_worlds() {
    use tsfp_gameplay::story_events::eff_dispatch::native;
    let cases = common::parse(include_str!("data/t1870/late-readable.golden"));
    for (index, case) in cases.iter().enumerate() {
        struct OracleHost {
            inner: Host,
        }
        impl Guest for OracleHost {
            fn read(&mut self, a: u32) -> u32 {
                self.inner.read(a)
            }
            fn write(&mut self, a: u32, v: u32) {
                self.inner.write(a, v)
            }
            fn call(&mut self, va: u32, args: &[u32]) -> u32 {
                let mut flat = vec![];
                for (i, &word) in args.iter().enumerate() {
                    let count = match va {
                        0x107B60 if i == 1 => Some(3),
                        0x12BDC0 if [1, 2, 3, 5, 6].contains(&i) => Some(3),
                        0x167B0 if i >= 3 => Some(3),
                        0x151FC0 if i == 1 => Some(3),
                        0x151FC0 if i == 0 || i == 2 => Some(0),
                        0x152770 if i == 0 => Some(0),
                        0x152770 if i == 1 => Some(3),
                        0x152090 | 0x151E80 if i == 0 => Some(0),
                        0x152090 | 0x151E80 if i == 1 => Some(3),
                        0x35DE70 if i < 2 => Some(3),
                        _ => None,
                    };
                    if let Some(count) = count {
                        for k in 0..count {
                            flat.push(self.read(word + k * 4));
                        }
                    } else {
                        flat.push(word);
                    }
                }
                self.inner.calls.push((va, flat));
                if va == 0x151FC0 {
                    self.set_vec3(args[2], [0.0, 0.0, 0.5]);
                    self.writef(args[2] + 12, 1.0);
                }
                match va {
                    0x65250 => 1,
                    0x102880 => 0x1800000,
                    0x154FE0 => 0x12345678,
                    _ => 0,
                }
            }
            fn call_f(&mut self, _: u32, _: &[u32]) -> f32 {
                panic!("float leaf")
            }
        }
        let mut h = OracleHost {
            inner: Host {
                words: case.ins.clone(),
                calls: vec![],
            },
        };
        let va = case.input(0xF0000000);
        let entry = case.input(0xF0000004);
        native(&mut h, va, &[entry]).unwrap();
        let got: Vec<_> = h.inner.calls.iter().map(|c| c.0).collect();
        let expected: Vec<_> = case
            .calls
            .iter()
            .filter(|c| c.callee != 0x389B0)
            .map(|c| c.callee)
            .collect();
        assert_eq!(
            got.len(),
            expected.len(),
            "case {index} root {va:x} call count"
        );
        for (i, (a, b)) in got.iter().zip(&expected).enumerate() {
            assert_eq!(a, b, "case {index} root {va:x} call {i}");
        }
        let expected_calls: Vec<_> = case.calls.iter().filter(|c| c.callee != 0x389B0).collect();
        for (i, (actual, expected)) in h.inner.calls.iter().zip(expected_calls).enumerate() {
            assert_eq!(
                actual.1.len(),
                expected.args.len(),
                "case {index} call {i} argument length"
            );
            for (j, (&a, &b)) in actual.1.iter().zip(&expected.args).enumerate() {
                if a == b {
                    continue;
                }
                let af = f32::from_bits(a);
                let bf = f32::from_bits(b);
                if (af.is_nan() && bf.is_nan())
                    || (af.abs() > 1e-20
                        && bf.abs() > 1e-20
                        && (af - bf).abs() <= 0.002_f32.max(bf.abs() * 0.002))
                {
                    continue;
                }
                panic!("case {index} root {va:x} call {i} arg {j}: {a:x} != {b:x}");
            }
        }
        for (&address, &expected) in &case.outs {
            let got = h.read(address);
            if got == expected {
                continue;
            }
            let stream = (0x1400000..0x1410000).contains(&address);
            let float = stream && (address - 0x1400000) % 24 != 12
                || (entry + 0x1C..=entry + 0x84).contains(&address)
                || address == entry + 0x1E8
                || address == entry + 0x1EC;
            if float {
                let a = f32::from_bits(got);
                let b = f32::from_bits(expected);
                if a.is_nan() && b.is_nan() {
                    continue;
                }
                if a == b || (a - b).abs() <= 0.002_f32.max(b.abs() * 0.002) {
                    continue;
                }
            }
            panic!(
                "case {index} root {va:x} memory {address:x}: {got:x} != {expected:x} ({} vs {})",
                f32::from_bits(got),
                f32::from_bits(expected)
            );
        }
    }
}
