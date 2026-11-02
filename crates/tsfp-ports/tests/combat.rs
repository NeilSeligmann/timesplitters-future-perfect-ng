// SPDX-License-Identifier: GPL-3.0-or-later
// FABRICATED synthetic fixtures; not original-vs-Rust equivalence receipts.
use std::collections::BTreeMap;
use tsfp_port_runtime::memory::GuestMemory;
use tsfp_ports::combat;
#[derive(Default)]
struct Memory {
    bytes: BTreeMap<u32, u8>,
    stores: Vec<u32>,
}
impl GuestMemory for Memory {
    fn read8(&self, a: u32) -> u8 {
        *self.bytes.get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.bytes.insert(a, v);
        self.stores.push(a);
    }
}
#[derive(Default)]
struct Hooks {
    events: Vec<(u32, u32)>,
    effect: bool,
    record: bool,
}
impl combat::DamageHooks<Memory> for Hooks {
    fn pending_effect(&mut self, m: &mut Memory, r: u32) {
        self.events.push((0xb9510, r));
        if self.effect {
            m.write32(r + 0x20, 3.0f32.to_bits());
            m.write32(r, 0x4000);
        }
    }
    fn hit_record_enabled(&mut self, _: &mut Memory, c: u32) -> bool {
        self.events.push((0x124c70, c));
        self.record
    }
    fn record_hit(&mut self, _: &mut Memory, c: u32, r: u32) {
        self.events.push((0x124d00, c));
        assert_eq!(r, 0x3000);
    }
}
fn fixture(health: f32, armor: f32, damage: f32) -> Memory {
    let mut m = Memory::default();
    m.write32(0x3000, 0x1000);
    m.write32(0x107c, 0x2000);
    m.write32(0x11cc, health.to_bits());
    m.write32(0x2078, armor.to_bits());
    m.write32(0x3020, damage.to_bits());
    m
}
fn float(m: &Memory, a: u32) -> f32 {
    f32::from_bits(m.read32(a))
}
#[test]
fn armor_first_then_health_and_strict_clamp() {
    for (armor, damage, health, expected) in [
        (4., 10., 20., 14.),
        (20., 10., 20., 20.),
        (0., 30., 20., 0.),
        (0., -1., 20., 20.),
    ] {
        let mut m = fixture(health, armor, damage);
        let mut h = Hooks::default();
        combat::apply_damage(&mut m, &mut h, 0x3000);
        assert_eq!(float(&m, 0x11cc), expected);
        assert_eq!(float(&m, 0x2078), armor - armor.min(damage));
        assert_eq!(m.read32(0x2b44), 0);
        assert_eq!(m.read32(0x2b48), m.read32(0x3020));
        assert_eq!(h.events, [(0x124c70, 0x2000)]);
    }
}
#[test]
fn bypass_and_cancel_with_effect_hook() {
    let mut m = fixture(20., 4., 10.);
    let mut h = Hooks::default();
    m.write32(0x303c, 0x200000);
    combat::apply_damage(&mut m, &mut h, 0x3000);
    assert_eq!(float(&m, 0x2078), 4.);
    assert_eq!(float(&m, 0x11cc), 10.);
    m = fixture(20., 4., 10.);
    m.write32(0x7de45c, 0x2000000);
    m.write32(0x1150, 1);
    combat::apply_damage(&mut m, &mut h, 0x3000);
    assert_eq!(float(&m, 0x11cc), 20.);
    assert_eq!(float(&m, 0x3020), 0.);
}
#[test]
fn hooks_can_retarget_victim_but_character_is_captured_before_hook() {
    let mut m = fixture(20., 0., 10.);
    m.write32(0x1150, 1);
    m.write32(0x41cc, 12.0f32.to_bits());
    let mut h = Hooks {
        effect: true,
        record: true,
        ..Hooks::default()
    };
    combat::apply_damage(&mut m, &mut h, 0x3000);
    assert_eq!(
        h.events,
        [(0xb9510, 0x3000), (0x124c70, 0x2000), (0x124d00, 0x2c68)]
    );
    assert_eq!(float(&m, 0x11cc), 20.);
    assert_eq!(float(&m, 0x41cc), 9.);
    assert_eq!(float(&m, 0x2b48), 3.);
}
#[test]
fn unordered_nan_and_signed_zero_are_not_numeric_min_or_max() {
    let mut m = fixture(20., f32::NAN, 10.);
    let mut h = Hooks::default();
    combat::apply_damage(&mut m, &mut h, 0x3000);
    assert!(float(&m, 0x2078).is_nan());
    assert_eq!(float(&m, 0x11cc), 20.);
    m = fixture(20., 4., f32::NAN);
    combat::apply_damage(&mut m, &mut h, 0x3000);
    assert!(float(&m, 0x11cc).is_nan());
    m = fixture(-0.0, 0., 0.);
    combat::apply_damage(&mut m, &mut h, 0x3000);
    assert_eq!(m.read32(0x11cc), (-0.0f32).to_bits());
}
#[test]
fn mode_bound_and_runtime_scale() {
    let mut m = Memory::default();
    m.write32(0x475c78, 1.0f32.to_bits());
    m.write32(0x475d18, 3.0f32.to_bits());
    m.write32(0x4e8cec, 60);
    m.write32(0x4e794c, (-1.0f32).to_bits());
    m.write32(0x11d8, 30.0f32.to_bits());
    m.write32(0x79094c, 2);
    m.write8(0x277841, 1);
    assert_eq!(combat::health_term(&m, 0x1000), -0.5);
    m.write32(0x79094c, 1);
    assert_eq!(combat::health_term(&m, 0x1000), -(1.0f32 / 60.0 * 10.0));
    m.write32(0x79094c, 0);
    assert_eq!(combat::health_term(&m, 0x1000), -0.5);
    m.write32(0x4e794c, 2.0f32.to_bits());
    assert_eq!(combat::health_term(&m, 0x1000), 1.0);
}
#[derive(Default)]
struct ResolveHooks {
    calls: Vec<Vec<u32>>,
}
impl combat::DamageResolveHooks<Memory> for ResolveHooks {
    fn character_amount(&mut self, m: &mut Memory, c: u32, k: u32, h: u32, s: u32, r: u32) -> u32 {
        self.calls.push(vec![0xb9790, c, h, k, s, r]);
        m.write32(r + 0x20, 0xdeadbeef);
        0x7fc01234
    }
    fn generic_amount(&mut self, _: &mut Memory, k: u32, h: u32, s: u32) -> u32 {
        self.calls.push(vec![0xb9960, h, k, s]);
        0x80000000
    }
    fn finish(&mut self, m: &mut Memory, r: u32) {
        self.calls.push(vec![0xbb6c0, r, m.read32(r + 0x20)]);
    }
}
#[test]
fn resolve_routes_character_proxy_generic_and_sentinel() {
    for (class, proxy, hit) in [
        (8, 0, 7),
        (0x20, 0x2000, 7),
        (0x20, 0, 7),
        (9, 0, 7),
        (8, 0, u32::MAX),
    ] {
        let mut m = fixture(20., 4., 10.);
        m.write32(0x1020, class);
        m.write32(0x107c, proxy);
        m.write32(0x20ec, 0x4000);
        m.write32(0x4000, 0x5000);
        m.write32(0x3024, hit);
        m.write32(0x3018, 11);
        m.write32(0x3054, 13);
        m.write32(0x3040, 0x40);
        let mut h = ResolveHooks::default();
        combat::resolve_damage(&mut m, &mut h, 0x3000);
        if hit == u32::MAX {
            assert_eq!(h.calls, [vec![0xbb6c0, 0x3000, 0]]);
        } else if class == 8 || proxy != 0 {
            assert_eq!(
                h.calls,
                [
                    vec![
                        0xb9790,
                        if class == 8 { 0x1000 } else { 0x5000 },
                        7,
                        11,
                        13,
                        0x3000
                    ],
                    vec![0xbb6c0, 0x3000, 0x7fc01234]
                ]
            );
        } else {
            assert_eq!(
                h.calls,
                [vec![0xb9960, 7, 11, 13], vec![0xbb6c0, 0x3000, 0x80000000]]
            );
        }
        assert_eq!(
            m.read32(0x3040),
            if class == 0x20 && proxy != 0 && hit != u32::MAX {
                0x48
            } else {
                0x40
            }
        );
    }
}
#[test]
fn resolve_amount_zero_is_visible_to_aliased_entity_class() {
    let mut m = Memory::default();
    m.write32(0x3000, 0x3000);
    m.write32(0x3020, 8);
    m.write32(0x3024, 7);
    m.write32(0x3018, 11);
    m.write32(0x3054, 13);
    let mut h = ResolveHooks::default();
    combat::resolve_damage(&mut m, &mut h, 0x3000);
    assert_eq!(
        h.calls,
        [vec![0xb9960, 7, 11, 13], vec![0xbb6c0, 0x3000, 0x80000000]]
    );
}
#[derive(Default)]
struct Coordinator {
    calls: Vec<(u32, Vec<u32>)>,
    cancel: bool,
    current: u32,
    retarget: bool,
}
impl combat::CombatHooks<Memory> for Coordinator {
    fn call(&mut self, m: &mut Memory, t: u32, eax: u32, esi: u32, args: &[u32]) -> u32 {
        self.calls.push((t, args.to_vec()));
        match t {
            0xc6140 => self.current,
            0xb9430 => {
                assert_eq!(esi, 0x3000);
                u32::from(self.cancel)
            }
            0xbb090 => {
                assert_eq!(eax, 0x3000);
                assert_eq!(esi, 1);
                0
            }
            0xba310 => {
                if self.retarget {
                    m.write32(0x3000, 0x4000);
                }
                0
            }
            _ => 0,
        }
    }
}
#[test]
fn coordinator_mode_and_cancel_early_exits_preserve_identity() {
    let mut m = fixture(20., 4., 10.);
    m.write32(0x2090, 0xaabbccdd);
    m.write32(0x7de30c, 8);
    let mut h = Coordinator::default();
    combat::coordinate_damage(&mut m, &mut h, 0x3000);
    assert!(h.calls.is_empty());
    assert_eq!(m.read32(0x2090), 0xaabbccdd);
    m.write32(0x7de30c, 0);
    h.current = 0x5000;
    m.write32(0x5218, 0x1b);
    combat::coordinate_damage(&mut m, &mut h, 0x3000);
    assert_eq!(h.calls, [(0xc6140, vec![]), (0xc6140, vec![])]);
    h = Coordinator {
        cancel: true,
        ..Coordinator::default()
    };
    combat::coordinate_damage(&mut m, &mut h, 0x3000);
    assert_eq!(m.read32(0x2090), 0xaabbccdd);
    assert_eq!(h.calls.last().unwrap().0, 0xb9430);
}
#[test]
fn coordinator_unordered_equal_and_retargeted_health_closure() {
    for (health, target) in [
        (0.0, 0xbaba0),
        (f32::NAN, 0x160020),
        (-1.0, 0x160020),
        (1.0, 0x160020),
    ] {
        let mut m = fixture(20., 4., 10.);
        m.write32(0x41cc, health.to_bits());
        m.write32(0x2008, 4);
        m.write32(0x790950, 4);
        m.write32(0x3018, 1);
        m.write32(0x3048, 0x1234);
        let mut h = Coordinator {
            retarget: true,
            ..Coordinator::default()
        };
        combat::coordinate_damage(&mut m, &mut h, 0x3000);
        assert_eq!(h.calls.last().unwrap().0, target);
        if target == 0x160020 {
            assert_eq!(h.calls.last().unwrap().1, [0x4000, 0x1234, 0x3000]);
        }
    }
}
#[test]
fn coordinator_mode_scaling_indirect_callback_and_positive_hit_pair() {
    let mut m = fixture(20., 4., 10.);
    m.write32(0x3004, 0x5000);
    m.write32(0x5020, 8);
    m.write32(0x507c, 0x6000);
    m.write32(0x7de455, 0xa);
    m.write32(0x6d6134, 2);
    m.write32(0x475cdc, 0.5f32.to_bits());
    m.write32(0x475db8, 0.25f32.to_bits());
    m.write32(0x3018, 1);
    m.write32(0x119c, 0xabcdef);
    m.write32(0x304c, 2.0f32.to_bits());
    m.write32(0x3050, 3.0f32.to_bits());
    m.write32(0x502c, 0x789);
    let mut h = Coordinator::default();
    combat::coordinate_damage(&mut m, &mut h, 0x3000);
    assert_eq!(float(&m, 0x3020), 1.25);
    assert_eq!(m.read32(0x2090), 0x789);
    assert!(h.calls.contains(&(0xabcdef, vec![0x3000])));
    assert!(
        h.calls
            .contains(&(0x120220, vec![0x2c68, 2.0f32.to_bits(), 3.0f32.to_bits()]))
    );
    assert!(!h.calls.iter().any(|c| c.0 == 0xba230 || c.0 == 0xba310));
}
// BABA0 (FABRICATED fixtures, scripted callee returns; the original differential is tests/c/test_rust_ext.c).
struct Death {
    calls: Vec<(u32, Vec<u32>)>,
    returns: BTreeMap<u32, u32>,
    esp: u32,
}
impl combat::CombatHooks<Memory> for Death {
    fn call(&mut self, _: &mut Memory, target: u32, _: u32, esi: u32, args: &[u32]) -> u32 {
        assert_eq!(esi, 0x3000);
        self.calls.push((target, args.to_vec()));
        *self.returns.get(&target).unwrap_or(&0)
    }
}
impl combat::DeathHooks<Memory> for Death {
    fn esp(&self) -> u32 {
        self.esp
    }
    fn set_esp(&mut self, value: u32) {
        self.esp = value;
    }
}
fn death_fixture(flags: u8) -> Memory {
    let mut m = Memory::default();
    m.write32(0x3000, 0x1000);
    m.write32(0x107c, 0x2000);
    m.write8(0x2024, flags);
    m
}
#[test]
fn death_controlled_victim_only_checks_controller_then_cleans_up() {
    let mut m = death_fixture(8);
    let mut h = Death {
        calls: vec![],
        returns: BTreeMap::new(),
        esp: 0x70000,
    };
    combat::handle_death(&mut m, &mut h, 0x3000, 0);
    assert!(h.calls.is_empty()); // no controller
    m.write32(0x1150, 0x5000);
    m.write32(0x5110, 4);
    combat::handle_death(&mut m, &mut h, 0x3000, 0);
    assert_eq!(h.calls, [(0xd5480, vec![0x3000])]);
    m.write32(0x500c, 0x800);
    h.calls.clear();
    combat::handle_death(&mut m, &mut h, 0x3000, 0);
    assert!(h.calls.is_empty()); // flag 0x800 blocks
}
#[test]
fn death_rejected_by_35cc10_leaves_state_untouched() {
    let mut m = death_fixture(0);
    let mut h = Death {
        calls: vec![],
        returns: BTreeMap::new(),
        esp: 0x70000,
    };
    m.write32(0x1258, 7);
    combat::handle_death(&mut m, &mut h, 0x3000, 0);
    assert_eq!(h.calls, [(0x35cc10, vec![0x3000, 7])]);
    assert_eq!(m.read32(0x1258), 7);
    assert_eq!(h.esp, 0x70000);
}
#[test]
fn death_without_killer_runs_cleanup_and_counts() {
    let mut m = death_fixture(0);
    let mut h = Death {
        calls: vec![],
        returns: BTreeMap::new(),
        esp: 0x70000,
    };
    h.returns.insert(0x35cc10, 1);
    m.write32(0x1258, 7);
    m.write32(0x2078, 5.0f32.to_bits());
    m.write32(0x11cc, 9.0f32.to_bits());
    combat::handle_death(&mut m, &mut h, 0x3000, 0xabcd);
    assert_eq!(float(&m, 0x2078), 0.0);
    assert_eq!(float(&m, 0x11cc), 0.0);
    assert_eq!(m.read32(0x1258), 8);
    assert_eq!(h.esp, 0x70000 - 4 + 4);
    assert_eq!(m.read32(0x70000 - 4), 0xabcd); // EDI saved where the original saves it
    let targets: Vec<u32> = h.calls.iter().map(|c| c.0).collect();
    assert_eq!(targets.first(), Some(&0x35cc10));
    assert_eq!(targets.last(), Some(&0x15db20));
}
