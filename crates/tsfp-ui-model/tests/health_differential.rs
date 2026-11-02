//! T2191 Tier 1: typed health segment model vs `tsfp_ui::hud::health` (itself Tier 1 against
//! the original bytes, docs/t-hud-slice.md). Seeded FABRICATED states; constants are stand-ins.
use std::collections::HashMap;

use tsfp_ui::hud::addr::*;
use tsfp_ui::hud::health as guest;
use tsfp_ui::hud::x86::{read_f32, read_i32, write_f32};
use tsfp_ui::memory::GuestMemory;
use tsfp_ui_model::health::*;

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

struct FirstSlot;
impl guest::PlayerRemap for FirstSlot {
    fn remap_player(&mut self, _: i32) -> u32 {
        0
    }
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

const RECORD: u32 = 0x0010_0000;
const OBJECT: u32 = 0x0011_0000;
const CHARACTER: u32 = 0x0012_0000;
const BLOCK: u32 = 0x0013_0000;

const VALUES: [f32; 10] = [
    0.0,
    1.0,
    17.5,
    35.0,
    50.0,
    99.9,
    100.0,
    140.0,
    -5.0,
    f32::NAN,
];
const MAXIMA: [f32; 5] = [100.0, 35.0, 0.0, 1.0, f32::NAN];

fn tuning(r: &mut Rng) -> HealthTuning {
    let mut health = [0.0; SEGMENTS];
    let mut armor = [0.0; SEGMENTS];
    for index in 0..SEGMENTS {
        let even = (index as f32 + 1.0) / SEGMENTS as f32;
        health[index] = r.pick(&[even, even, 0.0, 1.0, 0.33]);
        armor[index] = r.pick(&[even, even, 0.5, f32::NAN]);
    }
    HealthTuning {
        health_thresholds: health,
        armor_thresholds: armor,
        flash: r.pick(&[1.0, 0.75, 2.5]),
        segment_fade: r.pick(&[0.5, 1.0, 0.0]),
        one: r.pick(&[1.0, 2.0]),
        half: r.pick(&[0.5, 0.25]),
        slot_start_scale: r.pick(&[0.01, 0.5, 1.0]),
    }
}

fn vitals(r: &mut Rng, character: bool) -> Vitals {
    Vitals {
        health: r.pick(&VALUES),
        max_health: r.pick(&MAXIMA),
        armor: character.then(|| ArmorPool {
            value: r.pick(&VALUES),
            max: r.pick(&MAXIMA),
        }),
    }
}

fn store_constants(m: &mut Mem, t: &HealthTuning, rate_hz: i32, delta: f32) {
    for index in 0..SEGMENTS as u32 {
        write_f32(
            m,
            HEALTH_THRESHOLDS + index * THRESHOLD_STRIDE,
            t.health_thresholds[index as usize],
        );
        write_f32(
            m,
            ARMOR_THRESHOLDS + index * THRESHOLD_STRIDE,
            t.armor_thresholds[index as usize],
        );
    }
    write_f32(m, CONST_FADE, t.flash);
    write_f32(m, CONST_FLASH, t.segment_fade);
    write_f32(m, CONST_ONE, t.one);
    write_f32(m, CONST_HALF, t.half);
    write_f32(m, CONST_LETTERBOX_MIN, t.slot_start_scale);
    m.write32(FRAME_RATE_INT, rate_hz as u32);
    write_f32(m, FRAME_DELTA_F32, delta);
}

fn store_vitals(m: &mut Mem, v: &Vitals, character_class: bool) {
    m.write32(
        OBJECT + OBJ_CLASS,
        if character_class { CLASS_CHARACTER } else { 3 },
    );
    m.write32(OBJECT + OBJ_CHARACTER, CHARACTER);
    write_f32(m, OBJECT + OBJ_HEALTH, v.health);
    write_f32(m, OBJECT + OBJ_MAX_HEALTH, v.max_health);
    // A non-character still has some bytes there: the guest must ignore them.
    let pool = v.armor.unwrap_or(ArmorPool {
        value: 100.0,
        max: 1.0,
    });
    write_f32(m, CHARACTER + CHAR_ARMOR, pool.value);
    write_f32(m, CHARACTER + CHAR_ARMOR_MAX, pool.max);
}

fn assert_same(m: &Mem, bars: &HealthBars, timer: SlotTimer, player: u32, seed: u64, step: &str) {
    for index in 0..SEGMENTS {
        let slot = BLOCK + 4 * index as u32;
        let context = format!("seed {seed} {step} segment {index}");
        assert_eq!(
            m.read32(slot + FLAGS_HEALTH),
            u32::from(bars.shown.health[index]),
            "{context} health"
        );
        assert_eq!(
            m.read32(slot + FLAGS_ARMOR),
            u32::from(bars.shown.armor[index]),
            "{context} armor"
        );
        assert_eq!(
            read_f32(m, slot + TIMERS_HEALTH).to_bits(),
            bars.health_fade[index].to_bits(),
            "{context} health fade"
        );
        assert_eq!(
            read_f32(m, slot + TIMERS_ARMOR).to_bits(),
            bars.armor_fade[index].to_bits(),
            "{context} armor fade"
        );
    }
    assert_eq!(
        read_f32(m, BLOCK + FLAGS_FLASH).to_bits(),
        bars.flash.to_bits(),
        "seed {seed} {step} flash"
    );
    let timer_at = HUD_BLOCK_BASE + player * HUD_BLOCK_STRIDE + BLOCK_TIMER_20;
    assert_eq!(
        read_i32(m, timer_at),
        timer.0,
        "seed {seed} {step} slot timer"
    );
}

#[test]
fn health_model_matches_guest_port() {
    let mut checked = 0;
    for seed in 0..4_000u64 {
        let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let t = tuning(&mut r);
        let rate_hz = r.pick(&[60, 50, 30, 0, -2]);
        let delta = r.pick(&[1.0, 0.5, 2.0, 0.0, f32::NAN]);
        let character = r.next() % 4 != 0;
        let player = r.next() % 4;
        let mut m = Mem::default();
        store_constants(&mut m, &t, rate_hz, delta);
        m.write32(CURRENT_PLAYER_PTR, RECORD);
        m.write32(RECORD + 4, player);
        m.write32(RECORD + PLAYER_OBJECT, OBJECT);
        m.write32(OBJECT + OBJ_HUD_FLAGS, BLOCK);

        let mut v = vitals(&mut r, character);
        store_vitals(&mut m, &v, character);
        guest::slot_init(&mut m, RECORD);
        let mut bars = HealthBars::new(&v, &t);
        let mut timer = SlotTimer::start(rate_hz, &t);
        assert_same(&m, &bars, timer, player, seed, "init");

        for frame in 0..6 {
            if r.next() % 2 == 0 {
                v = vitals(&mut r, character);
                store_vitals(&mut m, &v, character);
            }
            assert_eq!(
                guest::flags_differ(&m, BLOCK, OBJECT),
                bars.changed(&v, &t),
                "seed {seed} frame {frame} differ"
            );
            if r.next() % 3 == 0 {
                let scale = r.pick(&[1.0, 0.5, 4.0, -1.0, f32::NAN]);
                let first = r.next() % 2 == 0;
                let second = r.next() % 2 == 0;
                m.write32(0x006d_612c, u32::from(first));
                m.write32(0x006d_6130, u32::from(second));
                guest::slot_update(&mut m, &mut FirstSlot, player as i32, scale);
                timer.update(&mut bars, &v, &t, rate_hz, scale, first && second);
            } else {
                guest::flags_tick(&mut m, BLOCK, OBJECT);
                bars.tick(&v, &t, rate_hz, delta);
            }
            assert_same(&m, &bars, timer, player, seed, &format!("frame {frame}"));
            checked += 1;
        }
    }
    assert_eq!(checked, 24_000);
}
