// SPDX-License-Identifier: GPL-3.0-or-later
// T1510: the Rust closure comparer must agree with the Python comparer on every shared vector.
#[path = "../src/fpclosure.rs"] mod fpclosure;
use fpclosure::*;
const VECTORS: &str = include_str!("../../../docs/evidence/t1510/fpclosure_vectors.txt");
fn hex(t: &str) -> u128 { u128::from_str_radix(t, 16).unwrap() }
fn lifted(f: &[&str]) -> ClosureFp {
    assert_eq!(&f[..2], ["FPCLOSURE", "1"]);
    let mut slots = [0u64; 8];
    for i in 0..8 { slots[i] = hex(f[5 + i]) as u64; }
    ClosureFp { slots, top: hex(f[2]) as u8, control: hex(f[3]) as u16, depth: hex(f[4]) as u8 }
}
fn kind(name: &str) -> Kind {
    match name { "MATCH" => Kind::Match, "DIFFER" => Kind::Differ, "UNREPRESENTED" => Kind::Unrepresented, _ => panic!("{name}") }
}
#[test] fn conversions_and_cases_agree_with_python() {
    let (mut conv, mut cases, mut seen) = (0, 0, [0u32; 3]);
    for line in VECTORS.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f[0] == "C" {
            let bits = hex(f[1]) as u64;
            assert_eq!(f64_to_raw80(bits), hex(f[2]), "widen {}", f[1]);
            assert_eq!(raw80_to_f64(hex(f[2])), Some(bits), "narrow {}", f[2]);
            conv += 1;
        } else {
            let mut physical = [0u128; 8];
            for i in 0..8 { physical[i] = hex(f[4 + i]); }
            let raw = RawX87 { physical, tags: hex(f[12]) as u16, control: hex(f[13]) as u16, status: hex(f[14]) as u16 };
            let l = lifted(&f[15..]);
            let v = compare_raw(Some(&raw), Some(&l), hex(f[3]) as u16);
            assert_eq!(v.kind, kind(f[2]), "{line}");
            assert_eq!(kind(f[1]), kind(f[2]), "intent expectation {line}");
            seen[match v.kind { Kind::Match => 0, Kind::Differ => 1, _ => 2 }] += 1;
            cases += 1;
        }
    }
    assert_eq!((conv, cases), (215, 1200));
    assert!(seen.iter().all(|n| *n > 0));
}
#[test] fn missing_fails_closed_even_when_both_missing() {
    assert_eq!(compare_raw(None, None, 0).kind, Kind::Missing);
    assert_eq!(differences(None, None), None);
}
#[test] fn narrowing_never_rounds() {
    assert_eq!(raw80_to_f64(f64_to_raw80(0x3FF0000000000000) | 1), None);
    assert_eq!(raw80_to_f64(0x3FFF_0000_0000_0000_0000), None);
    assert_eq!(raw80_to_f64(0x7FFF_0000_0000_0000_0000), None);
}
#[test] fn top_can_never_be_ignored() {
    let raw = RawX87 { physical: [0; 8], tags: 0xFFFF, control: 0x27F, status: 0 };
    assert!(from_raw(&raw, STATUS_TOP).is_err());
    assert!(from_raw(&raw, 0).is_ok());
}

#[test] fn conversion_errors_identify_the_failed_boundary() {
    let mut raw = RawX87 { physical: [0; 8], tags: 0xFFFF, control: 0x27F, status: 0 };
    assert_eq!(from_raw(&raw, STATUS_TOP), Err(RawConversionError::IgnoredTop));
    raw.status = 1;
    assert_eq!(from_raw(&raw, 0), Err(RawConversionError::UnmodelledStatus));
    raw.status = 0;
    raw.tags = 0xFFF7; // physical slot 1 occupied, slot 0 empty
    assert_eq!(from_raw(&raw, 0), Err(RawConversionError::SparseStack));
    raw.tags = 0xFFFC; // physical slot 0 occupied with a valid tag
    raw.physical[0] = f64_to_raw80(1.0f64.to_bits()) | 1;
    assert_eq!(from_raw(&raw, 0), Err(RawConversionError::UnrepresentableSlot { physical: 0 }));
    raw.physical[0] = 0;
    assert_eq!(from_raw(&raw, 0), Err(RawConversionError::TagMismatch { physical: 0 }));
    assert_eq!(compare_raw(Some(&raw), None, 0).kind, Kind::Missing);
}
