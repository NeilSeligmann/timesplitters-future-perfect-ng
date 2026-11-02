// SPDX-License-Identifier: GPL-3.0-or-later
//! T1900 slice B Tier 0 smoke tests for the team pickup ports: hand fixtures per branch plus a
//! seeded shape/invariant loop (not differential).
use std::collections::HashMap;
use tsfp_engine_object::closure::{
    callee::{Callee, Cpu},
    memory::GuestMemory,
};
use tsfp_port_gameplay_modes_multiplayer_1 as p;

#[derive(Default)]
struct Mem(HashMap<u32, u8>);
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        *self.0.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.insert(a, v);
    }
}
/// Records (target, esi, eax, ecx, args[4]); scripted EAX answers.
#[derive(Default)]
struct Trace {
    calls: Vec<(u32, u32, [u32; 4])>,
    answers: HashMap<u32, u32>,
}
impl Callee<Mem> for Trace {
    fn call(&mut self, m: &mut Mem, target: u32, cpu: &mut Cpu) {
        let a = |i: u32| m.read32(cpu.esp + 4 + 4 * i);
        self.calls.push((target, cpu.esi, [a(0), a(1), a(2), a(3)]));
        cpu.eax = *self.answers.get(&target).unwrap_or(&0);
    }
}
fn cpu() -> Cpu {
    Cpu {
        esp: 0x1000,
        ..Cpu::default()
    }
}
fn targets(t: &Trace) -> Vec<u32> {
    t.calls.iter().map(|c| c.0).collect()
}

const BASE: u32 = 0x2000;
const BASE_EXT: u32 = 0x2100;
const FLAG: u32 = 0x3000;
const FLAG_EXT: u32 = 0x3100;
const PLAYER: u32 = 0x4000;
const LIST: u32 = 0x8000;

/// base team 2, flag team `flag_team`, player team `player_team`, two list players on teams 2 and 1.
fn deliver_fixture(m: &mut Mem, player_team: u32, flag_team: u32) {
    m.write32(BASE + 0x7c, BASE_EXT);
    m.write32(BASE_EXT + 0x1c, 2);
    m.write32(FLAG + 0x7c, FLAG_EXT);
    m.write32(FLAG_EXT + 0x1c, flag_team);
    m.write32(FLAG_EXT + 0x20, 7);
    m.write32(FLAG_EXT + 0x3c, 9);
    m.write32(FLAG + 0x64, 5);
    m.write32(PLAYER + 0x84, player_team);
    m.write32(PLAYER + 8, 3);
    m.write32(p::PLAYER_COUNT, 2);
    m.write32(p::PLAYER_ENTRIES, LIST);
    m.write32(p::TICK_COUNT, 30);
    for (i, team) in [2u32, 1].into_iter().enumerate() {
        let entry = LIST + i as u32 * p::PLAYER_ENTRY_STRIDE;
        let object = 0x9000 + i as u32 * 0x100;
        let ext = 0x9800 + i as u32 * 0x100;
        m.write32(entry + 4, 0xa0 + i as u32);
        m.write32(entry + 0x14, object);
        m.write32(object + 0x7c, ext);
        m.write32(ext + 0x84, team);
    }
}
fn deliver(m: &mut Mem, t: &mut Trace) -> Cpu {
    let mut c = cpu();
    (c.eax, c.ecx, c.esi) = (BASE, PLAYER, FLAG);
    p::deliver_flag(t, m, &mut c);
    c
}

#[test]
fn deliver_capture_posts_messages_and_resets_home() {
    let (mut m, mut t) = (Mem::default(), Trace::default());
    deliver_fixture(&mut m, 2, 1);
    let c = deliver(&mut m, &mut t);
    let stat = t.calls.iter().find(|x| x.0 == p::CALL_STAT_ADD).unwrap();
    assert_eq!(stat.2[..3], [p::STAT_FLAG_CAPTURE, 3, 1.0f32.to_bits()]);
    // team 2 list player flashed + own text; team 1 list player gets the capture line
    assert_eq!(
        t.calls
            .iter()
            .filter(|x| x.0 == p::CALL_ENTRY_POST_TEXT)
            .count(),
        1
    );
    assert_eq!(
        t.calls
            .iter()
            .filter(|x| x.0 == p::CALL_ENTRY_MESSAGE)
            .count(),
        1
    );
    assert!(targets(&t).contains(&p::CALL_TEAM_SCORE));
    assert_eq!(t.calls.last().unwrap().0, p::CALL_RESET_HOME);
    assert_eq!(m.read32(FLAG + 0x64), 7);
    assert_eq!(m.read32(FLAG + 0xec), 5);
    assert_eq!(m.read32(FLAG_EXT + 0x38), 9);
    assert_eq!(m.read32(FLAG + 0x28) & 2, 2);
    assert_eq!(c.esp, 0x1000);
}

#[test]
fn deliver_wrong_team_only_resets() {
    let (mut m, mut t) = (Mem::default(), Trace::default());
    deliver_fixture(&mut m, 1, 1);
    deliver(&mut m, &mut t);
    assert!(!targets(&t).contains(&p::CALL_STAT_ADD));
    assert_eq!(*targets(&t).last().unwrap(), p::CALL_RESET_HOME);
}

#[test]
fn deliver_own_flag_returned_skips_reset_home() {
    let (mut m, mut t) = (Mem::default(), Trace::default());
    deliver_fixture(&mut m, 2, 2);
    deliver(&mut m, &mut t);
    assert!(targets(&t).contains(&p::CALL_RETURNED_HOME));
    assert!(!targets(&t).contains(&p::CALL_RESET_HOME));
    assert!(!targets(&t).contains(&p::CALL_STAT_ADD));
    let event = t
        .calls
        .iter()
        .find(|x| x.0 == p::CALL_EVENT_POST)
        .unwrap()
        .2[0];
    assert_eq!(m.read32(event), p::SOUND_FLAG_DELIVER);
}

fn drop_fixture(m: &mut Mem, countdown: u32, mode: u32) {
    m.write32(FLAG + 0x7c, FLAG_EXT);
    m.write32(FLAG + 4, 0x5000);
    m.write32(0x5010, 1);
    m.write32(FLAG_EXT + p::ext::STATE, 2);
    m.write32(FLAG_EXT + p::ext::COUNTDOWN, countdown);
    m.write32(FLAG_EXT + p::ext::SPAWN_MODE, mode);
    m.write32(p::FRAME_TICKS, 1);
    m.write32(p::TICK_SCALE, 0.5f32.to_bits());
    m.write32(p::TICK_COUNT, 30);
}
fn drop_tick(m: &mut Mem, t: &mut Trace) -> u32 {
    let mut c = Cpu {
        esp: 0x1000,
        eax: FLAG,
        ebx: 0x1234,
        ..Cpu::default()
    };
    p::dropped_countdown_tick(t, m, &mut c);
    assert_eq!(c.esp, 0x1000);
    c.edx
}

#[test]
fn dropped_guards_skip() {
    for case in 0..4 {
        let (mut m, mut t) = (Mem::default(), Trace::default());
        drop_fixture(&mut m, 5, 1);
        match case {
            0 => m.write32(FLAG + 0x78, 1),
            1 => m.write32(FLAG + 0x128, 1),
            2 => m.write32(FLAG_EXT + p::ext::STATE, 1),
            _ => m.write32(0x5010, 0),
        }
        drop_tick(&mut m, &mut t);
        assert_eq!(m.read32(FLAG_EXT + p::ext::COUNTDOWN), 5);
        assert!(t.calls.is_empty());
    }
}

#[test]
fn dropped_positive_countdown_integrates_motion() {
    let (mut m, mut t) = (Mem::default(), Trace::default());
    drop_fixture(&mut m, 5, 1);
    m.write32(FLAG + 0xb4, 2.0f32.to_bits());
    m.write32(FLAG + 0x64, 1.0f32.to_bits());
    m.write32(FLAG_EXT + p::ext::DROP_VEL, 4.0f32.to_bits());
    assert_eq!(drop_tick(&mut m, &mut t), 4);
    assert_eq!(m.read32(FLAG_EXT + p::ext::COUNTDOWN), 4);
    assert_eq!(f32::from_bits(m.read32(FLAG + 0x64)), 2.0);
    assert_eq!(f32::from_bits(m.read32(FLAG + 0xc0)), 2.0);
    assert!(t.calls.is_empty());
}

#[test]
fn dropped_expiry_effect_mode_returns_home() {
    let (mut m, mut t) = (Mem::default(), Trace::default());
    drop_fixture(&mut m, 1, 2);
    m.write32(FLAG + 0x14, 0x7b25_98);
    t.answers.insert(p::CALL_SPAWN_LOOKUP, 0x6000);
    m.write32(0x6000 + 0x28, 11);
    m.write32(FLAG_EXT + p::ext::CARRIER_INDEX, 4);
    m.write32(FLAG_EXT + p::ext::DROP_POS, 77);
    drop_tick(&mut m, &mut t);
    assert_eq!(
        targets(&t),
        vec![p::CALL_SPAWN_LOOKUP, p::CALL_EFFECT_AT, p::CALL_DROP_NOTIFY]
    );
    assert_eq!(t.calls[0].2[2], 0x7b25_98);
    assert_eq!(m.read32(FLAG + 0x1a0), 1);
    assert_eq!(m.read32(FLAG + 0x1a4), 360);
    assert_eq!(m.read32(FLAG + 0xc0), 11);
    assert_eq!(m.read32(FLAG + 0x64), 77);
    assert_eq!(m.read32(FLAG_EXT + p::ext::STATE), 1);
    assert_eq!(m.read32(FLAG_EXT + p::ext::CARRIER_SLOT), 4);
}

#[test]
fn dropped_expiry_flag_mode_delivers_to_carrier_base() {
    let (mut m, mut t) = (Mem::default(), Trace::default());
    drop_fixture(&mut m, 1, 1);
    m.write32(FLAG_EXT + p::ext::CARRIER_INDEX, 3);
    m.write32(FLAG_EXT + p::ext::RECORD_INDEX, 2);
    m.write32(p::PICKUP_TABLE, 0xa000);
    m.write32(0xa000 + (3 << 5) + 4, 0xffff_ffff);
    m.write32(p::PLAYER_RECORD_BASE, 0x10000);
    t.answers.insert(p::CALL_PLAYER_BY_SLOT, 0xb000);
    m.write32(0xb004, 0x2000);
    drop_tick(&mut m, &mut t);
    assert_eq!(m.read32(0xa000 + (3 << 5) + 4), 0xffdf_ffff);
    assert_eq!(m.read32(FLAG_EXT + p::ext::CARRIER_INDEX), u32::MAX);
    assert_eq!(*targets(&t).last().unwrap(), p::CALL_DELIVER);
    // slot lookup that finds nobody stops before the delivery
    let (mut m2, mut t2) = (Mem::default(), Trace::default());
    drop_fixture(&mut m2, 1, 1);
    drop_tick(&mut m2, &mut t2);
    assert_eq!(*targets(&t2).last().unwrap(), p::CALL_PLAYER_BY_SLOT);
}

fn touch(m: &mut Mem, t: &mut Trace) -> (Cpu, u32) {
    let mut c = Cpu {
        esp: 0x1000,
        eax: BASE,
        ..Cpu::default()
    };
    m.write32(0x1004, 0x4000 - 0x100); // toucher object
    m.write32(0x1008, 0x7000); // out pointer
    p::touch_assign_team_or_deliver(t, m, &mut c);
    (c, m.read32(0x7000))
}
fn touch_fixture(m: &mut Mem, mode: u8, team: u32) {
    m.write8(p::GAME_MODE, mode);
    m.write32(0x4000 - 0x100 + 0x7c, PLAYER);
    m.write32(PLAYER + 0x84, team);
    m.write32(BASE + 0x7c, BASE_EXT);
    m.write32(BASE + 4, 0xc000);
    m.write32(BASE + 0x1fc, 0xd000);
    m.write32(0xd000, 0x111);
    m.write32(0xd004, 0x222);
    m.write32(p::DEFAULT_TEAM_COLOR, 0x333);
}

#[test]
fn touch_assign_team_mode_sets_team_mask_and_color() {
    for (team, count, want) in [(1, 1, 0x111), (3, 2, 0x222), (3, 1, 0x333), (1, 0, 0x333)] {
        let (mut m, mut t) = (Mem::default(), Trace::default());
        touch_fixture(&mut m, p::MODE_ASSIGN_TEAM, team);
        m.write32(BASE + 0x1e8, count);
        m.write32(BASE_EXT + 0x1c, 99);
        let (c, out) = touch(&mut m, &mut t);
        assert_eq!((c.eax, out), (1, 1));
        assert_eq!(m.read32(BASE_EXT + 0x1c), team);
        assert_eq!(m.read32(0xc030), 1 << team);
        assert_eq!(targets(&t), vec![p::CALL_TEAM_REFRESH, p::CALL_TEAM_COLOR]);
        assert_eq!(t.calls[1].2[0], want);
    }
    // unchanged team: mask written, no refresh
    let (mut m, mut t) = (Mem::default(), Trace::default());
    touch_fixture(&mut m, p::MODE_ASSIGN_TEAM, 2);
    m.write32(BASE_EXT + 0x1c, 2);
    let (c, _) = touch(&mut m, &mut t);
    assert_eq!(c.eax, 1);
    assert!(t.calls.is_empty());
}

#[test]
fn touch_carried_flag_delivers_and_resets() {
    let (mut m, mut t) = (Mem::default(), Trace::default());
    touch_fixture(&mut m, 1, 2);
    m.write32(BASE_EXT + 0x1c, 2);
    m.write32(PLAYER + 0x80, FLAG);
    m.write32(FLAG + 0x7c, FLAG_EXT);
    m.write32(FLAG + 4, 0xe000);
    m.write32(0xe004, 0xe100);
    m.write32(0xe104, 2); // two parts
    m.write32(0xe010, 0xf000);
    m.write32(FLAG + 0x28, 0xffff_ffff);
    t.answers.insert(p::CALL_CARRIER_VALID, 1);
    let (c, out) = touch(&mut m, &mut t);
    assert_eq!((c.eax, out), (0, 1));
    let parts: Vec<u32> = t
        .calls
        .iter()
        .filter(|x| x.0 == p::CALL_RESET_PART)
        .map(|x| x.2[0])
        .collect();
    assert_eq!(parts, vec![0xf000, 0xf040]);
    assert_eq!(m.read32(PLAYER + 0x80), 0);
    assert_eq!(m.read32(FLAG_EXT + p::ext::STATE), 1);
    assert_eq!(m.read32(FLAG + 0x12c), u32::MAX);
    assert_eq!(m.read8(FLAG + 0x32), 2);
    assert_eq!(m.read32(FLAG + 0x28) & 0x4000_0000, 0);
    assert_eq!(t.calls.last().unwrap().0, p::CALL_DROP_CARRIED);
}

#[test]
fn touch_not_carrying_or_wrong_team_or_invalid_is_noop() {
    for case in 0..3 {
        let (mut m, mut t) = (Mem::default(), Trace::default());
        touch_fixture(&mut m, 1, 2);
        m.write32(BASE_EXT + 0x1c, if case == 1 { 3 } else { 2 });
        m.write32(PLAYER + 0x80, if case == 0 { 0 } else { FLAG });
        let (c, out) = touch(&mut m, &mut t);
        assert_eq!((c.eax, out), (0, 0));
        assert!(!targets(&t).contains(&p::CALL_DELIVER));
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
}

#[test]
fn seeded_invariants() {
    for seed in 0..3000u64 {
        let mut r = Rng(seed);
        let (player_team, flag_team) = (1 + r.next() % 4, 1 + r.next() % 4);
        let (mut m, mut t) = (Mem::default(), Trace::default());
        deliver_fixture(&mut m, player_team, flag_team);
        m.write32(BASE_EXT + 0x1c, 1 + r.next() % 4);
        deliver(&mut m, &mut t);
        let n = targets(&t);
        // the flag always snaps home, and the sound event is always posted
        assert_eq!(m.read32(FLAG + 0x64), 7);
        assert_eq!(n.iter().filter(|x| **x == p::CALL_EVENT_POST).count(), 1);
        // reset-home runs exactly when the return path did not
        assert_eq!(
            n.contains(&p::CALL_RESET_HOME),
            !n.contains(&p::CALL_RETURNED_HOME)
        );
        let capture = n.contains(&p::CALL_STAT_ADD);
        assert_eq!(capture, n.contains(&p::CALL_TEAM_SCORE));
        assert!(n.len() < 40);
        // dropped tick: countdown never increases, state stays 1 or 2
        let mut m = Mem::default();
        let mut t = Trace::default();
        let count = r.next() % 6;
        drop_fixture(&mut m, count, 1 + r.next() % 2);
        t.answers.insert(p::CALL_SPAWN_LOOKUP, 0x6000);
        drop_tick(&mut m, &mut t);
        let after = m.read32(FLAG_EXT + p::ext::COUNTDOWN);
        assert_eq!(after, count.wrapping_sub(1));
        let state = m.read32(FLAG_EXT + p::ext::STATE);
        assert_eq!(state, if (after as i32) > 0 { 2 } else { 1 });
    }
}
