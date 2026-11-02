//! T1875: `0x584A0` key bracket and the `0x5AAA0` interpolation core against numbers recorded from the ORIGINAL code
//! under Unicorn (`tools/t1875_interp_gold.py`). `F` lines use fabricated key tables (inline), `R` and `I` lines name a
//! real clip of the owner disc (looked up through the private ISO reader, skipped with a SKIP line when it is absent).
#[path = "support/bits.rs"]
mod bits_support;
use bits_support::f32_bits as bits;
use std::collections::BTreeMap;
use tsfp_engine_anim::interp::{clip_duration, interpolate_node, key_bracket};
use tsfp_format_anim::Clip;

const SENTINEL: u32 = 0xA5A5_A5A5;

fn golden(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/data/t1875/{name}", env!("CARGO_MANIFEST_DIR"))).expect("golden file")
}

fn expected(low: &str, high: &str) -> Option<(i32, i32)> {
    (low != "X").then(|| (low.parse().unwrap(), high.parse().unwrap()))
}

fn real_clips() -> Option<BTreeMap<(String, u32), Clip>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let python = std::env::var("TSFP_TEST_PYTHON").unwrap_or("python3".into());
    let output = std::process::Command::new(python)
        .current_dir(root)
        .args(["-m", "tools.rust_anim_sample"])
        .output()
        .expect("Python reader");
    if output.status.code() == Some(77) {
        eprintln!("SKIP interp: private ISO absent");
        return None;
    }
    assert!(output.status.success(), "reader failed: {}", String::from_utf8_lossy(&output.stderr));
    let b = output.stdout;
    let (mut at, mut clips) = (0usize, BTreeMap::new());
    let u32_at = |o: usize| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    while at < b.len() {
        let n = u16::from_le_bytes([b[at], b[at + 1]]) as usize;
        let path = String::from_utf8(b[at + 2..at + 2 + n].to_vec()).unwrap();
        at += 2 + n;
        let (ordinal, len) = (u32_at(at), u32_at(at + 8) as usize);
        clips.insert((path, ordinal), Clip::parse(&b[at + 12..at + 12 + len]).unwrap());
        at += 12 + len;
    }
    Some(clips)
}

#[test]
fn key_bracket_on_fabricated_key_tables_equals_the_original() {
    let (mut compared, mut stuck) = (0, 0);
    for line in golden("bracket.golden").lines().filter(|l| l.starts_with("F ")) {
        let f: Vec<&str> = line.split(' ').collect();
        let keys: Vec<u32> = f[6..6 + f[5].parse::<usize>().unwrap()].iter().map(|k| k.parse().unwrap()).collect();
        let (duration, time) = (bits(f[1]), bits(f[2]));
        let want = expected(f[3], f[4]);
        assert_eq!(key_bracket(&keys, duration, time), want, "line {line}");
        stuck += usize::from(want.is_none());
        compared += 1;
    }
    eprintln!("MEASURED bracket: {compared} fabricated cases identical to 0x584A0 ({stuck} where the original never returns)");
    assert!(compared > 4000 && stuck > 0);
}

#[test]
fn key_bracket_and_interpolation_on_real_clips_equal_the_original() {
    let Some(clips) = real_clips() else { return };
    let (mut brackets, mut interps, mut unwritten) = (0, 0, BTreeMap::new());
    for line in golden("bracket.golden").lines().filter(|l| l.starts_with("R ")) {
        let f: Vec<&str> = line.split(' ').collect();
        let clip = &clips[&(f[1].to_string(), f[2].parse().unwrap())];
        let got = key_bracket(&clip.key_frames, clip_duration(clip), bits(f[3]));
        assert_eq!(got, expected(f[4], f[5]), "line {line}");
        brackets += 1;
    }
    for line in golden("interp.golden").lines().filter(|l| l.starts_with("I ")) {
        let f: Vec<&str> = line.split(' ').collect();
        let clip = &clips[&(f[1].to_string(), f[2].parse().unwrap())];
        let want: Vec<u32> = f[6..22].iter().map(|w| u32::from_str_radix(w, 16).unwrap()).collect();
        let got = interpolate_node(clip, 0, bits(f[3]), bits(f[4]), f[5] == "1").unwrap().expect("bracket");
        for (cell, (a, b)) in got.matrix.iter().zip(&want).enumerate() {
            if *b == SENTINEL {
                *unwritten.entry(cell).or_insert(0usize) += 1;
            } else {
                assert_eq!(a.to_bits(), *b, "cell {cell} of {line}");
            }
        }
        interps += 1;
    }
    eprintln!("MEASURED interp: {brackets} real-clip brackets and {interps} whole 0x5AAA0 interpolations identical, cells the original leaves untouched {unwritten:?}");
    assert!(brackets > 5000 && interps > 1000);
}
