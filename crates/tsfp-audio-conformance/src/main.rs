//! Write deterministic paired WAVs and a compact machine-readable PCM verdict.
use std::{
    fs::{self, File},
    io::{self, Write},
    path::PathBuf,
};
use tsfp_audio::{contract::AudioSink, mixer::Mixer, sink::WavSink};
use tsfp_audio_conformance::{compare, oracle::COracle, scenarios::*};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args
        .first()
        .is_some_and(|s| s == "--completion-capture" || s == "--completion-capture-v2")
    {
        if args.len() != 2 {
            return Err("usage: tsfp-audio-conformance --completion-capture CAPTURE".into());
        }
        let file = File::open(&args[1])?;
        let count = if args[0] == "--completion-capture-v2" {
            tsfp_audio_conformance::completion_stream::compare_v2(file)?
        } else {
            tsfp_audio_conformance::completion_stream::compare(file)?
        };
        println!("EQUAL: {count} modeled completion deliveries (captured host calls only)");
        return Ok(());
    }
    if args.first().is_some_and(|s| s == "--capture") {
        if args.len() != 3 {
            return Err("usage: tsfp-audio-conformance --capture COMMAND_LOG OUTPUT_DIR".into());
        }
        let receipt = tsfp_audio_conformance::capture::write_evidence(
            std::path::Path::new(&args[1]),
            std::path::Path::new(&args[2]),
        )?;
        print!("{receipt}");
        return Ok(());
    }
    if args.len() != 1 {
        return Err(
            "usage: tsfp-audio-conformance OUTPUT_DIR | --capture COMMAND_LOG OUTPUT_DIR".into(),
        );
    }
    let out = PathBuf::from(&args[0]);
    // Fresh directory: a failed rerun must never leave an old EQUAL receipt behind.
    fs::create_dir(&out)?;
    let mut total = 0;
    let cases = fixed_inputs();
    for (name, steps) in &cases {
        let mut c = COracle::new(OUTPUT_HZ, TICKS_PER_SECOND);
        let mut rust = Mixer::new(OUTPUT_HZ, TICKS_PER_SECOND);
        // WavSink currently swallows write errors. Render into bounded memory first,
        // then use fallible filesystem writes so evidence cannot silently truncate.
        let mut a = WavSink::new(io::Cursor::new(Vec::new()), OUTPUT_HZ, 2)?;
        let mut b = WavSink::new(io::Cursor::new(Vec::new()), OUTPUT_HZ, 2)?;
        let result = compare(&mut c, &mut rust, steps, |x, y| {
            a.write(x);
            b.write(y);
        });
        fs::write(out.join(format!("{name}-c.wav")), a.finish()?.into_inner())?;
        fs::write(
            out.join(format!("{name}-rust.wav")),
            b.finish()?.into_inner(),
        )?;
        let report = result.map_err(|e| format!("{name}: {e:?}"))?;
        total += report.samples;
    }
    let verdict = format!(
        "{{\"verdict\":\"EQUAL\",\"source\":\"fabricated-fixed-inputs\",\"tier\":1,\"cases\":{},\"samples\":{total},\"tolerance\":0}}\n",
        cases.len()
    );
    File::create(out.join("receipt.json"))?.write_all(verdict.as_bytes())?;
    print!("{verdict}");
    Ok(())
}
