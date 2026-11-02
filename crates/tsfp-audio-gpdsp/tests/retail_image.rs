//! Retail-image smoke and timing probe. Skipped unless TSFP_GPDSP_FIRMWARE and TSFP_GPDSP_IMAGE name
//! the owner-extracted firmware and effects image (never committed).

use std::time::Instant;
use tsfp_audio_gpdsp::gp::{FRAME_WORDS, GpDsp};
use tsfp_audio_gpdsp::prepare;

fn load() -> Option<(Vec<u8>, Vec<u8>)> {
    let f = std::fs::read(std::env::var("TSFP_GPDSP_FIRMWARE").ok()?).ok()?;
    let i = std::fs::read(std::env::var("TSFP_GPDSP_IMAGE").ok()?).ok()?;
    Some((f, i))
}

#[test]
fn retail_frames_are_deterministic_and_timed() {
    let Some((firmware, image)) = load() else {
        eprintln!("skipped: no retail image");
        return;
    };
    let run = || {
        let mut gp = GpDsp::new(prepare(&firmware, &image).expect("prepare"), true).expect("boot");
        let mut input = [0u32; FRAME_WORDS];
        input[0] = 1000;
        input[32] = 1000;
        let start = Instant::now();
        let mut out = [0u32; FRAME_WORDS];
        let frames = 200;
        for _ in 0..frames {
            out = gp.frame(&input).expect("frame");
        }
        let per_frame = start.elapsed() / frames;
        (out, per_frame)
    };
    let (a, per_frame) = run();
    let (b, _) = run();
    assert_eq!(a, b);
    eprintln!("GPDSP frame cost {per_frame:?}");
    if let Ok(path) = std::env::var("TSFP_GPDSP_DUMP") {
        let words: Vec<String> = a.iter().map(|w| format!("{w:x}")).collect();
        std::fs::write(path, format!("out: {}\n", words.join(" "))).unwrap();
    }
    let mut gp = GpDsp::new(prepare(&firmware, &image).unwrap(), true).unwrap();
    let input = [0u32; FRAME_WORDS];
    for _ in 0..200 {
        gp.frame(&input).unwrap();
    }
    let start = Instant::now();
    for _ in 0..2000 {
        gp.frame(&input).unwrap();
    }
    eprintln!("GPDSP warm frame cost {:?}", start.elapsed() / 2000);
    if let Ok(path) = std::env::var("TSFP_GPDSP_SCENARIO") {
        std::fs::write(path, scenario(&firmware, &image)).unwrap();
    }
}

/// Deferred/immediate writes across maps, commit, frames and state readback, printed like the C driver.
fn scenario(firmware: &[u8], image: &[u8]) -> String {
    let mut gp = GpDsp::new(prepare(firmware, image).unwrap(), true).unwrap();
    let mut text = String::new();
    let mut input = [0u32; FRAME_WORDS];
    input[0] = 1000;
    input[32] = 1000;
    for _ in 0..200 {
        gp.frame(&input).unwrap();
    }
    for map in 0..9usize {
        let value = [0, 0, 0x40, 0, map as u8, 0x11, 0x22, 0];
        if gp.write(map, 0, &value, map % 2 == 0).is_err() {
            text += &format!("w{map} fail\n");
        }
    }
    let (start, bytes) = gp.pending();
    text += &format!(
        "pending {start:x} {bytes:x}\ncommit {}\n",
        u8::from(gp.commit().is_ok())
    );
    for i in 0..5u32 {
        input[0] = 1000 + i;
        input[64] = 5000;
        let ok = gp.frame(&input);
        text += &format!("frame{i} {}:", u8::from(ok.is_ok()));
        for (k, w) in ok.unwrap_or([0; FRAME_WORDS]).iter().enumerate() {
            if *w != 0 {
                text += &format!(" {k}={w:x}");
            }
        }
        text += "\n";
    }
    for map in 0..9usize {
        let mut b = [0u8; 16];
        let ok = gp.read(map, 0, &mut b).is_ok();
        text += &format!("map{map} {}:", u8::from(ok));
        for x in b.iter().filter(|_| ok) {
            text += &format!(" {x:02x}");
        }
        text += "\n";
    }
    text
}

/// create-at-download (frame unfinished) then one live gain write, as the binding does; printed like
/// tmp/gpdsp/c/sig.c.
#[test]
fn at_download_signal_phase() {
    let Some((firmware, image)) = load() else {
        return;
    };
    let mut gp = GpDsp::new(prepare(&firmware, &image).unwrap(), false).unwrap();
    gp.live_aligned(5, 0x20, &[0, 0, 0x40, 0]).unwrap();
    let mut input = [0u32; FRAME_WORDS];
    input[27 * 32] = 0x1000;
    let mut text = String::new();
    for i in 0..8 {
        let out = gp.frame(&input).unwrap();
        text += &format!("signal {i} {:x}\n", out[27 * 32]);
    }
    // The C core (and the original binding test) shows the gain signal on frames 2 and 5.
    let expected = "signal 0 0\nsignal 1 0\nsignal 2 4800\nsignal 3 0\nsignal 4 0\nsignal 5 4800\nsignal 6 0\nsignal 7 0\n";
    assert_eq!(text, expected);
    if let Ok(path) = std::env::var("TSFP_GPDSP_SIGNAL") {
        std::fs::write(path, text).unwrap();
    }
}

fn fnv(mut h: u64, w: u32) -> u64 {
    for i in 0..4 {
        h ^= u64::from((w >> (8 * i)) & 0xff);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Retained effect command log (T3949): the LCG command stream of `tests/ref/log_driver.c` replayed on the Rust
/// core. With `TSFP_GPDSP_COMMANDLOG_REF` (the C core's output, untracked) the text must be identical.
#[test]
fn command_log() {
    let Some((firmware, image)) = load() else {
        return;
    };
    let mut gp = GpDsp::new(prepare(&firmware, &image).unwrap(), true).unwrap();
    let mut seed: u64 = 0x2545F4914F6CDD1D;
    let mut rnd = move || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (seed >> 32) as u32
    };
    let mut input = [0u32; FRAME_WORDS];
    let mut text = String::new();
    for i in 0..600 {
        for k in (0..1024).step_by(32) {
            let on = rnd() & 3 != 0;
            let r = rnd();
            input[k] = if !on {
                0
            } else if std::env::var_os("FULLSCALE").is_some() {
                r & 0xff_ffff
            } else {
                ((r % 16384) as i32 - 8192) as u32 & 0xff_ffff
            };
        }
        if i % 7 == 3 {
            let map = rnd() % 9;
            let b = [rnd() as u8, rnd() as u8, 0x40, 0, map as u8, 0x11, 0x22, 0];
            let deferred = rnd() & 1 != 0;
            let ok = std::env::var_os("NOWRITE").is_some()
                || gp.write(map as usize, 0, &b, deferred).is_ok();
            text += &format!("w{i} m{map} {}\n", u8::from(ok));
        }
        if i == 5 {
            text += &format!("c{i} {}\n", u8::from(gp.commit().is_ok()));
        }
        let out = gp.frame(&input);
        let ok = out.is_ok();
        if std::env::var("DUMPF")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            == Some(i)
        {
            for (k, w) in out.iter().flatten().enumerate().filter(|(_, w)| **w != 0) {
                eprintln!("{k}={w:x}");
            }
        }
        let h = out
            .unwrap_or([0; FRAME_WORDS])
            .iter()
            .fold(0xcbf29ce484222325u64, |h, w| fnv(h, *w));
        text += &format!("f{i} {} {h:016x}\n", u8::from(ok));
    }
    for map in 0..9usize {
        let mut b = [0u8; 16];
        let ok = gp.read(map, 0, &mut b).is_ok();
        let h = b
            .iter()
            .filter(|_| ok)
            .fold(0xcbf29ce484222325u64, |h, x| fnv(h, u32::from(*x)));
        text += &format!("r{map} {} {h:016x}\n", u8::from(ok));
    }
    if let Ok(path) = std::env::var("TSFP_GPDSP_COMMANDLOG") {
        std::fs::write(path, &text).unwrap();
    }
    if let Ok(path) = std::env::var("TSFP_GPDSP_COMMANDLOG_REF") {
        assert_eq!(text, std::fs::read_to_string(path).unwrap());
    }
}
