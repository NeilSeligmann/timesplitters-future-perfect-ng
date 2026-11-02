//! Synthetic conformance probe, paired with tools/audio_stretch_check.py.
use std::io::{self, BufRead};
use tsfp_audio::stretch::Stretch;
fn main() {
    for line in io::stdin().lock().lines() {
        let values: Vec<i64> = line
            .unwrap()
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        let channels = values[0] as u16;
        let overlap = values[1] as usize;
        let nominal = values[2] as usize;
        let input: Vec<i16> = values[3..].iter().map(|v| *v as i16).collect();
        let mut engine = Stretch::new(channels, overlap).unwrap();
        let mut out = vec![0; channels as usize * overlap];
        let ok = engine.prime(&input, &mut out);
        println!("{} {:?}", u8::from(ok), out);
        let start = engine.next(&input, nominal, &mut out);
        println!(
            "{} {} {:?}",
            u8::from(start.is_some()),
            start.unwrap_or(0),
            out
        );
        let ok = engine.hold(&mut out);
        println!("{} {:?}", u8::from(ok), out);
        let ok = engine.leave(&input, &mut out);
        println!("{} {:?}", u8::from(ok), out);
        engine.prime(&input, &mut out);
        let ok = engine.fade_out(&mut out);
        println!("{} {:?}", u8::from(ok), out);
    }
}
