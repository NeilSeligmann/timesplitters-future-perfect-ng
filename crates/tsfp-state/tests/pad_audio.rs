// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED synthetic fixtures (patterned bytes); no retail data.
use std::collections::BTreeMap;
use tsfp_state::GuestMemory;
use tsfp_state::pad_audio::*;

#[derive(Clone)]
struct Image(BTreeMap<u32, u8>);
impl GuestMemory for Image {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.insert(a, v);
    }
}

const PAD_END: u32 = PAD_BASE + 4 * PAD_WORDS as u32;
const AUDIO_END: u32 = AUDIO_BASE + 4 * AUDIO_WORDS as u32;

fn image() -> Image {
    let mut m = Image(BTreeMap::new());
    for i in 0..PAD_WORDS as u32 {
        m.write32(PAD_BASE + 4 * i, 0x1000 + i);
    }
    for i in 0..INPUT_WORDS as u32 {
        m.write32(INPUT_BASE + 4 * i, 0x2000 + i);
    }
    for i in 0..AUDIO_WORDS as u32 {
        m.write32(AUDIO_BASE + 4 * i, 0x3000 + i);
    }
    // neighbours that must stay untouched
    m.write32(PAD_BASE - 4, 0xaaaa_0001);
    m.write32(PAD_END, 0xaaaa_0002);
    m.write32(AUDIO_END, 0xaaaa_0004);
    m
}

#[test]
fn extents_are_adjacent_and_ordered() {
    assert_eq!(INPUT_BASE + 4 * INPUT_WORDS as u32, AUDIO_BASE);
    assert_eq!(PAD_END, 0x6b94f8);
    assert_eq!(AUDIO_END, 0x6b9934);
    assert_eq!(INPUT_BASE + 4 * IN_STATE as u32, 0x6b98a4);
    assert_eq!(INPUT_BASE + 4 * IN_COUNTDOWN as u32, 0x6b98b0);
    assert_eq!(INPUT_BASE + 4 * IN_DONE as u32, 0x6b98b4);
    assert_eq!(SCRATCH + 4 * SCRATCH_WORDS as u32, GROUPS[0].0);
    for (p, c, c2, _) in GROUPS {
        for a in [p, c, c2] {
            assert!((AUDIO_BASE..AUDIO_END).contains(&a));
        }
    }
}

#[test]
fn round_trip_touches_only_owned_extents() {
    let m = image();
    let (p, a) = (PadState::load(&m), AudioRamp::load(&m));
    let mut out = Image(BTreeMap::new());
    p.store(&mut out);
    a.store(&mut out);
    let owned = |x: &u32| (PAD_BASE..PAD_END).contains(x) || (INPUT_BASE..AUDIO_END).contains(x);
    let want: BTreeMap<u32, u8> =
        m.0.iter()
            .filter(|(k, _)| owned(k))
            .map(|(k, v)| (*k, *v))
            .collect();
    assert_eq!(out.0, want);
    assert!(p.diff(&m).is_none() && a.diff(&m).is_none());
}

#[test]
fn setters_write_through() {
    let mut m = image();
    let mut p = PadState::load(&m);
    let mut a = AudioRamp::load(&m);
    assert!(p.set_table(&mut m, 0x6b940c, 0x77));
    assert!(!p.set_table(&mut m, PAD_END, 0x77));
    assert!(!p.set_table(&mut m, 0x6b93c1, 0x77));
    assert_eq!(m.read32(0x6b940c), 0x77);
    assert_eq!(m.read32(PAD_END), 0xaaaa_0002);
    assert_eq!(p.exchange_gate(&mut m, 5), 0x2000);
    assert_eq!(m.read32(GATE), 5);
    assert_eq!(p.gate(), 5);
    p.enter_state_4(&mut m);
    assert_eq!((m.read32(0x6b98a4), m.read32(0x6b98b4)), (4, 1));
    assert_eq!((p.state(), p.done_flag()), (4, 1));
    p.set_countdown(&mut m, 1.5);
    assert_eq!(m.read32(0x6b98b0), 1.5f32.to_bits());
    assert_eq!(p.countdown(), 1.5);
    a.set_scale(&mut m, 0.25);
    assert_eq!(m.read32(SCALE), 0.25f32.to_bits());
    assert_eq!(a.scale(), 0.25);
    assert!(!a.set(&mut m, AUDIO_END, 1));
    assert_eq!(m.read32(AUDIO_END), 0xaaaa_0004);
    a.set_table_pointer(&mut m, 1, 0x4300_0000);
    assert_eq!(a.group(1).0, 0x4300_0000);
    assert_eq!(m.read32(0x6b98b8), 0x4300_0000);
    assert!(p.diff(&m).is_none() && a.diff(&m).is_none());
}

#[test]
fn lifted_clears_match_measured_stores() {
    let mut m = image();
    let mut a = AudioRamp::load(&m);
    a.clear_counts(&mut m);
    for g in 0..4 {
        let (ptr, c, c2) = a.group(g);
        assert_ne!(ptr, 0);
        assert_eq!((c, c2), (0, 0));
    }
    assert_eq!(m.read32(0x6b9930), 0x3000 + 30, "scale untouched");
    a.release_tables(&mut m);
    for g in 0..4 {
        assert_eq!(a.group(g).0, 0);
    }
    assert!(a.diff(&m).is_none());
}

#[test]
fn entry_lookup_stays_inside_extent() {
    let mut m = image();
    let mut p = PadState::load(&m);
    p.set_table(&mut m, PAD_BASE + 4, 0x3c); // slot 1 -> second entry
    assert_eq!(p.entry_addr(1), 0x6b940c);
    assert_eq!(
        p.entry_word(1, 8),
        Some(p.table[(0x6b9414 - PAD_BASE) as usize / 4])
    );
    p.set_table(&mut m, PAD_BASE + 8, 0x1_0000);
    assert_eq!(p.entry_word(2, 0), None);
}

#[test]
fn mutation_of_every_owned_word_is_reported() {
    let m = image();
    for i in 0..PAD_WORDS {
        let mut p = PadState::load(&m);
        p.table[i] ^= 1;
        assert_eq!(p.diff(&m).unwrap().addr, PAD_BASE + 4 * i as u32);
    }
    for i in 0..INPUT_WORDS {
        let mut p = PadState::load(&m);
        p.input[i] ^= 1;
        assert_eq!(p.diff(&m).unwrap().addr, INPUT_BASE + 4 * i as u32);
    }
    for i in 0..AUDIO_WORDS {
        let mut a = AudioRamp::load(&m);
        a.words[i] ^= 1;
        assert_eq!(a.diff(&m).unwrap().addr, AUDIO_BASE + 4 * i as u32);
    }
}

/// Drift against the translations: every `0x6B98xx/0x6B99xx` address in the input and audio ports
/// that falls in 0x6B9888..0x6B9938 is owned, except the list at 0x6B9938 (not owned, debt).
#[test]
fn translation_addresses_are_owned() {
    let pad = std::fs::read_to_string("../tsfp-port-engine-input/src/c2r/engine_input.rs").unwrap();
    let audio = std::fs::read_to_string("../tsfp-ports/src/audio.rs").unwrap();
    let remainder = std::fs::read_to_string("../tsfp-ports/src/audio_remainder.rs").unwrap();
    let mut seen = 0;
    for src in [&pad, &audio, &remainder] {
        for tok in src.split("0x6b").skip(1) {
            let hex: String = tok.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
            if hex.len() != 4 {
                continue;
            }
            let a = 0x6b_0000 + u32::from_str_radix(&hex, 16).unwrap();
            if !(0x6b9888..0x6b9938).contains(&a) {
                continue;
            }
            seen += 1;
            assert!((INPUT_BASE..AUDIO_END).contains(&a), "{a:#x} not owned");
        }
    }
    assert!(seen > 50, "only {seen} references scanned");
}
