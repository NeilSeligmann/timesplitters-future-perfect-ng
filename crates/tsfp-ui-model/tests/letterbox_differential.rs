//! T2191 Tier 1: typed letterbox model vs `tsfp_ui::hud::letterbox` (itself Tier 1 against the
//! original bytes, docs/t-hud-slice.md). Seeded FABRICATED states; constants are stand-ins.
use std::collections::HashMap;

use tsfp_ui::hud::addr::*;
use tsfp_ui::hud::draw::{Draw, TexturedRect};
use tsfp_ui::hud::letterbox as guest;
use tsfp_ui::hud::x86::{read_f32, read_i32, write_f32};
use tsfp_ui::memory::GuestMemory;
use tsfp_ui_model::letterbox::*;

#[derive(Default)]
struct Mem(HashMap<u32, u8>);
impl GuestMemory for Mem {
    fn read8(&self, address: u32) -> u8 {
        self.0.get(&address).copied().unwrap_or(0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}

#[derive(Default)]
struct Rects(Vec<[i32; 4]>);
impl Draw for Rects {
    fn state_group(&mut self, _: u32) {}
    fn blend_mode(&mut self, _: u32) {}
    fn colour_preset(&mut self, _: u32) {}
    fn fill_rect(&mut self, l: i32, t: i32, r: i32, b: i32, alpha: u32) {
        assert_eq!(alpha, u32::from(BAR_ALPHA));
        self.0.push([l, t, r, b]);
    }
    fn textured_rect(&mut self, _: TexturedRect) {}
    fn text(&mut self, _: i32, _: i32, _: &str, _: u32) {}
    fn rect_line(&mut self, _: i32, _: i32, _: i32, _: i32, _: u32) {}
    fn corner_frame(&mut self, _: i32, _: i32, _: i32, _: i32, _: i32, _: u32) {}
    fn loadbar(&mut self, _: i32, _: i32, _: i32, _: i32, _: i32, _: u32) {}
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[self.next() as usize % items.len()]
    }
}

const VIEW: u32 = 0x0010_0000;

fn seeded(
    seed: u64,
) -> (
    Mem,
    Letterbox,
    LetterboxTuning,
    FrameTiming,
    SessionGate,
    ViewportEdges,
) {
    let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut m = Mem::default();
    let tuning = LetterboxTuning {
        full: r.pick(&[1.0, 0.75, 2.0]),
        rate: r.pick(&[0.5, 1.5, 3.0, 0.0]),
        draw_min: r.pick(&[0.01, 0.0, 0.5]),
        bar_scale: r.pick(&[60.0, 48.5, 120.0]),
    };
    let timing = FrameTiming {
        delta_frames: r.pick(&[0, 1, 2, 5, -1]),
        step: r.pick(&[1.0, 0.5, 2.0, f32::NAN]),
        rate_hz: r.pick(&[60, 50, 30, 0]),
    };
    let gate = SessionGate {
        split_screen_mode: r.next() % 2 == 0,
        local_players: r.pick(&[0, 1, 2, 4]),
        allow_in_split: r.next() % 3 == 0,
    };
    let view = ViewportEdges {
        x0: r.pick(&[0, 10]),
        x1: r.pick(&[639, 319]),
        y0: r.pick(&[0, 240]),
        y1: r.pick(&[479, 239]),
    };
    let state = Letterbox {
        amount: r.pick(&[0.0, 0.005, 0.3, 0.99, 1.0, 1.7, -0.2, f32::NAN]),
        phase: LetterboxPhase::from_raw(r.pick(&[1, 0, -1, 2])),
        hold_frames: r.pick(&[-1, 0, 1, 3, 100]),
        suppress_draw: r.pick(&[0, 0, 1]),
    };
    write_f32(&mut m, CONST_ONE, tuning.full);
    write_f32(&mut m, CONST_LETTERBOX_RATE, tuning.rate);
    write_f32(&mut m, CONST_LETTERBOX_MIN, tuning.draw_min);
    write_f32(&mut m, CONST_BAR_SCALE, tuning.bar_scale);
    m.write32(FRAME_DELTA_INT, timing.delta_frames as u32);
    write_f32(&mut m, FRAME_STEP_F32, timing.step);
    m.write32(FRAME_RATE_INT, timing.rate_hz as u32);
    m.write8(MODE_BYTE, if gate.split_screen_mode { 0x0a } else { 0x01 });
    m.write32(LOCAL_PLAYER_COUNT, gate.local_players as u32);
    m.write32(MODE_FLAGS, if gate.allow_in_split { 0x200 } else { 0 });
    m.write32(VIEWPORT_PTR, VIEW);
    m.write32(VIEW, view.x0 as u32);
    m.write32(VIEW + 4, view.x1 as u32);
    m.write32(VIEW + 0x14, view.y0 as u32);
    m.write32(VIEW + 0x18, view.y1 as u32);
    store(&mut m, &state);
    (m, state, tuning, timing, gate, view)
}

fn store(m: &mut Mem, s: &Letterbox) {
    write_f32(m, LETTERBOX_AMOUNT, s.amount);
    m.write32(LETTERBOX_STATE, s.phase.to_raw() as u32);
    m.write32(LETTERBOX_HOLD, s.hold_frames as u32);
    m.write32(LETTERBOX_NO_DRAW, s.suppress_draw);
}

fn assert_same(m: &Mem, s: &Letterbox, seed: u64, step: &str) {
    let amount = read_f32(m, LETTERBOX_AMOUNT);
    assert_eq!(
        amount.to_bits(),
        s.amount.to_bits(),
        "seed {seed} {step} amount"
    );
    assert_eq!(
        read_i32(m, LETTERBOX_STATE),
        s.phase.to_raw(),
        "seed {seed} {step} phase"
    );
    assert_eq!(
        read_i32(m, LETTERBOX_HOLD),
        s.hold_frames,
        "seed {seed} {step} hold"
    );
    assert_eq!(
        m.read32(LETTERBOX_NO_DRAW),
        s.suppress_draw,
        "seed {seed} {step} suppress"
    );
}

fn draw_both(m: &Mem, s: &Letterbox, t: &LetterboxTuning, v: ViewportEdges, seed: u64) {
    let mut rects = Rects::default();
    guest::draw(m, &mut rects);
    let typed: Vec<[i32; 4]> = s
        .bars(t, v)
        .map(|b| {
            [b.bottom, b.top]
                .map(|r| [r.left, r.top, r.right, r.bottom])
                .to_vec()
        })
        .unwrap_or_default();
    assert_eq!(rects.0, typed, "seed {seed} draw");
}

#[test]
fn letterbox_matches_guest_port_over_seeded_states() {
    let mut drawn = 0;
    for seed in 0..4000u64 {
        let (mut m, mut s, tuning, timing, gate, view) = seeded(seed);
        let hold = seed as u32 % 7;
        let force = (seed / 7) as u32 % 2;
        match seed % 3 {
            0 => {
                guest::start_hold(&mut m, hold, force);
                s.start_hold(gate, &tuning, hold, force);
            }
            1 => {
                guest::start_fade_in(&mut m, hold, force);
                s.start_fade_in(gate, hold, force);
            }
            _ => {}
        }
        assert_same(&m, &s, seed, "start");
        for frame in 0..6 {
            guest::update(&mut m);
            s.update(&tuning, timing);
            assert_same(&m, &s, seed, &format!("update {frame}"));
            draw_both(&m, &s, &tuning, view, seed);
            drawn += usize::from(s.bars(&tuning, view).is_some());
        }
    }
    assert!(drawn > 1000, "too few drawn frames: {drawn}");
}
