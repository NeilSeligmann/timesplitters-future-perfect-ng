//! T1877: spawn class mask initialisation `0x15F9BA..0x15FA9E` against the ORIGINAL (golden
//! `tests/data/t1877/spawn_mask.golden` from `tools/t1877_spawn_mask_gold.py`, numbers only).
use tsfp_gameplay::char_class::{SpawnMaskState, init_spawn_mask};

fn hex(token: &str) -> u32 {
    u32::from_str_radix(token, 16).unwrap()
}

#[test]
fn spawn_mask_init_matches_the_original() {
    const SENT: u32 = 0x5A5A_5A5A;
    let text = include_str!("data/t1877/spawn_mask.golden");
    let (mut cases, mut dormant, mut active, mut deferred) = (0, 0, 0, 0);
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        assert_eq!((t[0], t[3]), ("M", "="));
        let mut state = SpawnMaskState {
            mask: u64::from(hex(t[1])) | u64::from(hex(t[2])) << 32,
            aa8: SENT,
            aac: SENT,
            ab8: SENT,
            abc: SENT,
            ac0: SENT,
            ac4: SENT,
        };
        init_spawn_mask(&mut state);
        let want = SpawnMaskState {
            mask: u64::from(hex(t[4])) | u64::from(hex(t[5])) << 32,
            aa8: hex(t[6]),
            aac: hex(t[7]),
            ab8: hex(t[8]),
            abc: hex(t[9]),
            ac0: hex(t[10]),
            ac4: hex(t[11]),
        };
        assert_eq!(state, want, "{line}");
        cases += 1;
        dormant += usize::from(want.mask == 0x400_0000);
        active += usize::from(want.ab8 == 0 && want.abc == 0 && want.ac0 == SENT);
        deferred += usize::from(want.ac0 != SENT);
    }
    eprintln!(
        "MEASURED spawn mask: {cases} cases, {dormant} dormant, {active} direct, {deferred} deferred"
    );
    assert!(cases >= 1500 && dormant > 100 && active > 20 && deferred > 50);
}
