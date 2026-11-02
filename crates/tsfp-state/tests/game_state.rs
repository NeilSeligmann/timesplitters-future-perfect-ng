// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED synthetic fixtures (patterned bytes); no retail data.
use tsfp_state::GuestMemory;
use tsfp_state::game_state::*;

#[path = "support/patterned_image.rs"]
mod patterned_image;
use patterned_image::image;
fn sample() -> GameState {
    GameState {
        level: 0x65,
        players: 2,
        extra_players: 1,
        mode: 10,
        mode_sub: 0xfe,
        flags: 0xd840a,
        match_state: 0xc,
        tick_divisor: 3,
        tick_scale_bits: 0x3f80_0000,
    }
}

#[test]
fn round_trip_and_untouched_bytes() {
    let mut m = image();
    let before = m.0.clone();
    let s = sample();
    s.store(&mut m);
    assert_eq!(GameState::load(&m), s);
    assert_eq!(s.diff(&m), None);
    // exactly the declared cells changed
    let declared = [
        (LEVEL, 4),
        (PLAYERS, 4),
        (EXTRA_PLAYERS, 4),
        (MODE, 1),
        (MODE_SUB, 1),
        (FLAGS, 4),
        (MATCH, 4),
        (TICK_DIVISOR, 4),
        (TICK_SCALE_BITS, 4),
    ];
    for (i, (b, a)) in before.iter().zip(&m.0).enumerate() {
        let inside = declared
            .iter()
            .any(|(base, len)| (i as u32) >= *base && (i as u32) < base + len);
        assert!(
            inside || b == a,
            "byte {i:#x} outside the declared cells changed"
        );
    }
}

#[test]
fn writers_write_through() {
    let mut m = image();
    let mut s = GameState::load(&m);
    s.set_level(&mut m, 0x67);
    s.set_players(&mut m, 4);
    s.set_extra_players(&mut m, 2);
    s.set_mode(&mut m, 9);
    s.set_mode_sub(&mut m, 1);
    s.set_flags(&mut m, 0x4000_0040);
    s.set_match_state(&mut m, 9);
    s.set_tick_words(&mut m, 5, 0x4000_0000);
    assert_eq!(s.diff(&m), None);
    assert_eq!(
        (
            m.read32(LEVEL),
            m.read32(PLAYERS),
            m.read8(MODE),
            m.read32(MATCH)
        ),
        (0x67, 4, 9, 9)
    );
    assert_eq!(m.read32(TICK_SCALE_BITS), 0x4000_0000);
    // the byte flag store keeps the upper bytes, as the lifted write8 does
    s.set_flags_low8(&mut m, 0x01);
    assert_eq!(m.read32(FLAGS), 0x4000_0001);
    assert_eq!(s.flags, 0x4000_0001);
    assert_eq!(s.diff(&m), None);
}

#[test]
fn drift_is_detected_per_field() {
    let cells: [(u32, &str); 9] = [
        (LEVEL, "level"),
        (PLAYERS, "players"),
        (EXTRA_PLAYERS, "extra_players"),
        (MODE, "mode"),
        (MODE_SUB, "mode_sub"),
        (FLAGS, "flags"),
        (MATCH, "match_state"),
        (TICK_DIVISOR, "tick_divisor"),
        (TICK_SCALE_BITS, "tick_scale_bits"),
    ];
    for (addr, name) in cells {
        let mut m = image();
        let s = sample();
        s.store(&mut m);
        let v = m.read8(addr);
        m.write8(addr, v ^ 0x10);
        assert_eq!(s.diff(&m), Some(name));
    }
}

#[test]
fn readers_match_lifted_predicates() {
    let s = sample();
    // 0x2382b0 offline limit from player_system.rs: local plus extra players
    assert_eq!(s.slot_limit_offline(), 3);
    assert!(s.is_party());
    assert!(!s.in_level());
}
