//! T1668 tier 1: layer accessors against the ORIGINAL (`tools/t1668_layers_gold.py`; worlds regenerated from the seed with the same splitmix64 recipe).
use std::collections::BTreeMap;
use tsfp_engine_anim::layers::*;

mod support;
use support::Rng;

const FLOATS: [f32; 6] = [0.0, 0.5, 1.0, 2.5, -1.0, f32::NAN];
const DURATIONS: [f32; 5] = [2.5, 0.0, -1.5, 1.0, 10.0];
const MASKS: [u32; 5] = [0, 1, 4, 0x82, 0xFFFF_FFFF];
const BLENDS: [f32; 4] = [0.0, 0.5, -0.5, f32::NAN];
const OFFSETS: [usize; 26] = [0x0, 0x108, 0x124, 0x160, 0x17C, 0x198, 0xD4, 0xD8, 0xDC, 0xE0, 0x12C, 0x130, 0x134, 0x138, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

fn offsets() -> Vec<usize> {
    let mut v = vec![0x0, 0x108, 0x124, 0x160, 0x17C, 0x198];
    v.extend((0xD4..0xE4).step_by(4));
    v.extend((0x12C..0x13C).step_by(4));
    let _ = OFFSETS;
    v
}

struct World {
    clips: Vec<(Option<f32>, u32, u32)>,
    body: Body,
    model_type: u32,
    slow: bool,
    flag_blocks: bool,
    player_type: u32,
    player_null: bool,
    resident_odd: u32,
    args: Vec<u32>,
}

impl ClipWorld for World {
    fn duration(&self, slot: usize) -> Option<f32> {
        self.clips.get(slot).and_then(|c| c.0)
    }
    fn policy(&self, slot: usize) -> (u32, u32) {
        self.clips.get(slot).map_or((0, 0), |c| (c.1, c.2))
    }
}

impl SpeedWorld for World {
    fn slow_motion_allowed(&self) -> bool {
        self.slow && !self.flag_blocks
    }
    fn player_type(&self) -> Option<u32> {
        (!self.player_null).then_some(self.player_type)
    }
    fn slow_motion_scale(&self) -> f32 {
        0.25
    }
}

fn world(function: u32, seed: u64) -> World {
    let all_clips = matches!(function, 0x59D50 | 0x59EC0);
    let mut r = Rng(seed);
    let mut clips = Vec::new();
    for _ in 0..8 {
        let null = r.below(5) == 0 && !all_clips;
        let duration = DURATIONS[r.below(5) as usize];
        let (p0, p1) = (MASKS[r.below(5) as usize], MASKS[r.below(5) as usize]);
        clips.push(((!null).then_some(duration), p0, p1));
    }
    let mut body = Body([0; tsfp_engine_anim::layers::BODY_DWORDS]);
    for off in offsets() {
        let word = match off {
            0xD8 | 0x130 => r.below(8) as u32,
            0xD4 | 0x12C => r.below(4) as u32,
            0xDC | 0xE0 | 0x134 | 0x138 => FLOATS[r.below(6) as usize].to_bits(),
            0x198 => BLENDS[r.below(4) as usize].to_bits(),
            0x0 => 0x02D0_2000,
            _ => r.below(1 << 32) as u32,
        };
        body.0[off / 4] = word;
    }
    let model_type = if r.below(2) != 0 { 8 } else { 3 };
    let args: Vec<u32> = match function {
        0x59D70 | 0x59D90 | 0x5A1E0 => vec![r.below(4) as u32],
        0x59E00 | 0x59F10 | 0x59F50 | 0x59F90 => vec![MASKS[r.below(5) as usize], MASKS[r.below(5) as usize]],
        0x5A070 | 0x5A2F0 => vec![FLOATS[r.below(6) as usize].to_bits()],
        0x5A260 => {
            let layer = if r.below(2) != 0 { 0xD4 } else { 0x12C };
            vec![layer, FLOATS[r.below(6) as usize].to_bits()]
        }
        _ => vec![],
    };
    let slow = r.below(2) != 0;
    let flag_blocks = r.below(3) == 0;
    let player_type = [0x1B, 5][r.below(2) as usize];
    let player_null = r.below(4) == 0;
    let resident_odd = r.below(2) as u32;
    World { clips, body, model_type, slow, flag_blocks, player_type, player_null, resident_odd, args }
}

#[test]
fn layer_accessors_equal_the_original() {
    let text = std::fs::read_to_string(format!("{}/tests/data/t1668/layers.golden", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut per_function: BTreeMap<u32, usize> = BTreeMap::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split(' ').collect();
        let function = u32::from_str_radix(f[0], 16).unwrap();
        let seed: u64 = f[1].parse().unwrap();
        let mut w = world(function, seed);
        let mut body = w.body.clone();
        let masks = |a: &[u32]| (a[0], a[1]);
        let (value, expected_value): (String, &str) = match function {
            0x59CF0 => (format!("{:x}", body.active_layer()), f[2]),
            0x59D50 => (format!("{:x}", f64::from(current_duration(&body, &w).unwrap()).to_bits()), f[2]),
            0x59D70 => (format!("{:x}", u32::from(current_plays(&body, w.args[0]))), f[2]),
            0x59D90 => (format!("{:x}", u32::from(previous_plays(&body, w.args[0]))), f[2]),
            0x59DD0 => (format!("{:x}", secondary_word(Some(&body))), f[2]),
            0x59E00 => (format!("{:x}", u32::from(any_policy_hit(&body, &w, masks(&w.args)))), f[2]),
            0x59EC0 => (format!("{:x}", normalised_position(&body, &w).unwrap().to_bits()), f[2]),
            0x59F10 => (format!("{:x}", u32::from(current_policy_hit(&body, &w, masks(&w.args)))), f[2]),
            0x59F50 => (format!("{:x}", u32::from(previous_policy_hit(&body, &w, masks(&w.args)))), f[2]),
            0x59F90 => (format!("{:x}", u32::from(active_policy_hit(&body, &w, masks(&w.args)))), f[2]),
            0x5A1E0 => (format!("{:x}", u32::from(active_plays(&body, w.args[0]))), f[2]),
            0x5A2B0 => (format!("{:x}", f64::from(active_time(&body)).to_bits()), f[2]),
            0x61430 => (format!("{:x}", u32::from(active_clip_resident(&body, |id| (id + w.resident_odd) & 1 != 0))), f[2]),
            0x5A070 => {
                set_speed(&mut body, Some(w.model_type), &w, f32::from_bits(w.args[0]));
                ("-".into(), "-")
            }
            0x5A260 => {
                set_layer_time(&mut body, w.args[0] as usize, &w, f32::from_bits(w.args[1]));
                ("-".into(), "-")
            }
            0x5A2F0 => {
                set_active_time(&mut body, &w, f32::from_bits(w.args[0]));
                ("-".into(), "-")
            }
            0x5BA00 => {
                reset_current(&mut body, &w);
                ("-".into(), "-")
            }
            other => panic!("unknown function {other:x}"),
        };
        // the original returns eax as a full register for booleans (al only for sete): compare the low byte for 0/1 results
        if expected_value != "-" {
            let got = u64::from_str_radix(&value, 16).unwrap();
            let want = u64::from_str_radix(expected_value, 16).unwrap();
            let bool_like = matches!(function, 0x59D70 | 0x59D90 | 0x5A1E0 | 0x59E00 | 0x59F10 | 0x59F50 | 0x59F90 | 0x61430);
            if bool_like {
                assert_eq!(got & 1, want & 1, "{line}");
            } else if matches!(function, 0x59D50 | 0x59EC0 | 0x5A2B0) {
                let (g, wv) = (f64::from_bits(got), f64::from_bits(want));
                assert!(g.to_bits() == wv.to_bits() || (g.is_nan() && wv.is_nan()), "{line}: {g} vs {wv}");
            } else {
                assert_eq!(got, want, "{line}");
            }
        }
        let mut expected = w.body.clone();
        if f[3] != "-" {
            for pair in f[3].split(',') {
                let (off, word) = pair.split_once(':').unwrap();
                expected.0[usize::from_str_radix(off, 16).unwrap() / 4] = u32::from_str_radix(word, 16).unwrap();
            }
        }
        let nan_equal = |a: u32, b: u32| a == b || (f32::from_bits(a).is_nan() && f32::from_bits(b).is_nan());
        for (i, (a, b)) in body.0.iter().zip(&expected.0).enumerate() {
            assert!(nan_equal(*a, *b), "{line}: body word {:#x} {a:#x} vs {b:#x}", i * 4);
        }
        *per_function.entry(function).or_default() += 1;
        w.args.clear();
    }
    assert_eq!(per_function.len(), 17);
    assert!(per_function.values().all(|n| *n >= 250), "{per_function:?}");
}
