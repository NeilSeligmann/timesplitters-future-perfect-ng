//! T2191 Tier 1: typed HUD message ring vs `tsfp_ui::hud::messages` (itself Tier 1 against the
//! original bytes, docs/t-hud.md). Seeded FABRICATED call sequences; constants are stand-ins.
use std::collections::HashMap;

use tsfp_ui::hud::addr::*;
use tsfp_ui::hud::messages as guest;
use tsfp_ui::hud::x86::{read_f32, write_f32};
use tsfp_ui::memory::GuestMemory;
use tsfp_ui_model::messages::{
    HudMessages, MessageTuning, PLAYERS, RING_LEN as MODEL_RING_LEN, TEXT_BYTES,
};

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

const SOURCE: u32 = 0x0100_0000;

fn block(player: usize) -> u32 {
    HUD_BLOCK_BASE + player as u32 * HUD_BLOCK_STRIDE
}

fn assert_same(m: &Mem, hud: &HudMessages, context: &str) {
    for (player, ring) in hud.players.iter().enumerate() {
        assert_eq!(
            m.read32(block(player) + BLOCK_RING_CURSOR),
            ring.cursor as u32,
            "{context}"
        );
        for (index, entry) in ring.entries.iter().enumerate() {
            let at = guest::entry_address(block(player), index as u32);
            assert_eq!(
                m.read32(at + MSG_KIND),
                entry.kind,
                "{context} kind {player}/{index}"
            );
            if !entry.is_used() && entry.text.is_empty() {
                continue;
            }
            assert_eq!(
                guest::entry_text(m, at),
                entry.text_string(),
                "{context} text"
            );
            for (offset, value) in [
                (MSG_AGE, entry.age),
                (MSG_FADE_START, entry.fade_start),
                (MSG_END, entry.end),
                (MSG_EVICT_AFTER, entry.evict_after),
            ] {
                assert_eq!(
                    read_f32(m, at + offset).to_bits(),
                    value.to_bits(),
                    "{context} {offset:#x}"
                );
            }
            assert_eq!(m.read32(at + MSG_WEIGHT), entry.weight, "{context} weight");
            assert_eq!(m.read32(at + MSG_OPAQUE), entry.opaque, "{context} opaque");
            let alpha = guest::fade_alpha(entry.age, entry.fade_start, entry.end, 1.0);
            assert_eq!(
                alpha.to_bits(),
                entry.alpha(1.0).to_bits(),
                "{context} alpha"
            );
        }
    }
}

#[test]
fn ring_matches_guest_port_over_seeded_sequences() {
    let delays = [0.0f32, -0.0, 0.5, 1.25, -2.0];
    let kinds = [-1i32, 0, 4, 5, 0x0a, 0x0c, 0x0d, 7];
    let mut comparisons = 0;
    for seed in 1..=2_000u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
        let tuning = MessageTuning {
            fade_start: 1.0 + (rng.next() % 4) as f32,
            end: 6.0 + (rng.next() % 4) as f32,
            one: 1.0,
        };
        let mut m = Mem::default();
        write_f32(&mut m, CONST_FADE, tuning.fade_start);
        write_f32(&mut m, CONST_MSG_END, tuning.end);
        let mut hud = HudMessages::default();
        for step in 0..40 {
            let player = rng.pick(&[-1i32, 0, 1, 2, 3, 4]);
            match rng.next() % 3 {
                0 | 1 => {
                    let length =
                        rng.pick(&[0usize, 1, 7, TEXT_BYTES - 1, TEXT_BYTES, TEXT_BYTES + 5]);
                    let text: Vec<u8> = (0..length)
                        .map(|_| b'a' + (rng.next() % 26) as u8)
                        .collect();
                    for (offset, byte) in text.iter().chain([0u8].iter()).enumerate() {
                        m.write8(SOURCE + offset as u32, *byte);
                    }
                    let opaque = rng.next();
                    let delay = rng.pick(&delays);
                    if rng.next() % 2 == 0 {
                        guest::push_notice(&mut m, player, SOURCE, opaque, delay);
                        hud.push_notice(player, &text, opaque, delay, &tuning);
                    } else {
                        guest::push_notice_c(&mut m, player, SOURCE, opaque, delay);
                        hud.push_notice_c(player, &text, opaque, delay, &tuning);
                    }
                }
                _ => {
                    let kind = rng.pick(&kinds);
                    guest::clear_kind(&mut m, player, kind);
                    hud.clear_kind(player, kind);
                }
            }
            // Age entries so the alpha rule is compared off its plateau as well.
            let age = (rng.next() % 900) as f32 / 100.0;
            let player_index = rng.next() as usize % PLAYERS;
            let index = rng.next() as usize % MODEL_RING_LEN;
            hud.players[player_index].entries[index].age = age;
            write_f32(
                &mut m,
                guest::entry_address(block(player_index), index as u32) + MSG_AGE,
                age,
            );
            assert_same(&m, &hud, &format!("seed {seed} step {step}"));
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 80_000);
}
