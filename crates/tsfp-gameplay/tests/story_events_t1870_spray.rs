//! T1870: the spray services `0x345B60`, `0x345B10`/`0xD2D20` and `0x345C60` against emulated-original golden vectors (MEASURED).
//! Golden: `tests/data/t1870/{spray_burst,spray_streak}.golden` (`tools/t1846_gold/svc_spray.py`).
mod common;
use common::{Case, parse};
use tsfp_gameplay::story_events::svc_spray::*;

struct Host<'a> {
    case: &'a Case,
    low: u32,
    high: u32,
    draws: u32,
    log: Vec<(u32, Vec<u32>)>,
}

fn bits(v: [f32; 3]) -> Vec<u32> {
    v.iter().map(|c| c.to_bits()).collect()
}

impl SprayHost for Host<'_> {
    fn rng(&mut self) -> u32 {
        self.draws += 1;
        let state = (u64::from(self.high) << 32) | u64::from(self.low);
        let next = state.wrapping_mul(0x343FD).wrapping_add(0x26_9EC3);
        self.low = next as u32;
        self.high = (next >> 32) as u32;
        next as u32
    }
    fn word(&mut self, address: u32) -> u32 {
        self.case.input(address)
    }
    fn create_emitter(
        &mut self,
        handle: u32,
        template: u32,
        flags: u32,
        position: [f32; 3],
        vector: [f32; 3],
    ) {
        let mut words = vec![handle, template, flags];
        words.extend(bits(position));
        words.extend(bits(vector));
        self.log.push((0x111AE0, words));
    }
    fn emitter_mask(&mut self, mask: u32) {
        self.log.push((0x10E680, vec![mask]));
    }
    fn sprite_value(&mut self, index: u32) -> u32 {
        self.log.push((0x80F20, vec![index]));
        (0x1000 + index.wrapping_mul(7)) & 0xFFFF_FFFF
    }
    fn streak_sprite(&mut self, call: StreakSprite) {
        let mut words = vec![
            call.sprite,
            1,
            call.color,
            call.kind as u32,
            call.length.to_bits(),
            1,
            call.cell,
        ];
        words.extend(bits(call.position));
        words.extend(bits(call.vector));
        self.log.push((0x1128C0, words));
    }
}

fn expected(case: &Case) -> Vec<(u32, Vec<u32>)> {
    case.calls
        .iter()
        .map(|call| {
            let a = &call.args;
            let words = match call.callee {
                0x111AE0 => [&a[0..3], &a[5..11]].concat(),
                0x1128C0 => [&a[0..7], &a[9..15]].concat(),
                _ => a.clone(),
            };
            (call.callee, words)
        })
        .collect()
}

fn vec3(case: &Case, address: u32) -> [f32; 3] {
    case.vec3(address)
}

#[test]
fn burst_matches_the_original() {
    let cases = parse(include_str!("data/t1870/spray_burst.golden"));
    assert!(cases.len() >= 90);
    let (mut emitters, mut masks, mut refused) = (0, 0, 0);
    for (index, case) in cases.iter().enumerate() {
        let mut host = Host {
            case,
            low: 0,
            high: 0,
            draws: 0,
            log: vec![],
        };
        let (mode, owner, handle) = (
            case.input(0xF000_0000),
            case.input(0xF000_0004),
            case.input(0xF000_0008),
        );
        burst(
            &mut host,
            mode,
            owner,
            handle,
            vec3(case, 0x154_0000),
            vec3(case, 0x154_0020),
        );
        assert_eq!(host.log, expected(case), "case {index}");
        emitters += host.log.iter().filter(|(c, _)| *c == 0x111AE0).count();
        masks += host.log.iter().filter(|(c, _)| *c == 0x10E680).count();
        refused += usize::from(host.log.is_empty());
    }
    assert!(
        emitters > 40 && masks > 10 && refused > 3,
        "{emitters} {masks} {refused}"
    );
}

#[test]
fn streak_matches_the_original() {
    let cases = parse(include_str!("data/t1870/spray_streak.golden"));
    assert!(cases.len() >= 70);
    let mut sprites = std::collections::BTreeSet::new();
    for (index, case) in cases.iter().enumerate() {
        let mut host = Host {
            case,
            low: case.input(0x4E_7950),
            high: case.input(0x4E_7954),
            draws: 0,
            log: vec![],
        };
        streak(
            &mut host,
            vec3(case, 0x154_0000),
            vec3(case, 0x154_0020),
            case.input(0xF000_0000),
            case.input_f(0xF000_0004),
            case.input(0xF000_0008) as i32,
        );
        assert_eq!(host.log, expected(case), "case {index}");
        assert_eq!(
            (host.low, host.high),
            (case.output(0x4E_7950), case.output(0x4E_7954)),
            "case {index} rng"
        );
        sprites.insert(host.log[0].1[0]);
    }
    assert!(sprites.len() >= 4, "{sprites:?}");
}

#[test]
fn player_slot_divides_by_0xffc() {
    for slot in 0..8i32 {
        for extra in [0u32, 1, 0xFFB] {
            assert_eq!(
                player_slot(0x1000 + slot as u32 * 0xFFC + extra, 0x1000),
                slot
            );
        }
    }
    assert_eq!(player_slot(0, 0x1000), -1);
    assert_eq!(player_slot(0x1000 - 0xFFC, 0x1000), -1);
}
