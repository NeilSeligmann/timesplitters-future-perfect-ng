//! Streaming PCM comparison of captured commands. No guest or device is started.
use crate::{
    capture_parse::{Record, parse},
    capture_wav::PcmWav,
    compare_samples,
    oracle::COracle,
};
use std::{
    fs,
    io::{self, BufRead, BufReader, Read, Write},
    path::Path,
};
use tsfp_audio::{contract::SoundHost, mixer::Mixer, replay::fnv1a};

/// Bounded before allocation, including a maximum-sized PB encoded payload.
pub const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Default)]
pub struct CaptureReport {
    /// Modern recorder completed and its full prefix matches the END seal.
    /// Legacy C logs have no seal and cannot establish capture completeness.
    pub sealed: bool,
    pub lines: usize,
    pub commands: usize,
    pub refused_buffers: usize,
    pub completion_observations: usize,
    pub renders: usize,
    pub samples: usize,
    pub nonsilent_samples: usize,
    pub input_bytes: u64,
    /// Noncryptographic identity for diagnostics; evidence runs also retain SHA256.
    pub input_fnv1a: u64,
}

/// Reject partial/unknown records and verify captured verdicts/hashes as well as
/// C-versus-Rust PCM. Completion records are counted, not compared.
pub fn compare_log(
    mut input: impl BufRead,
    mut observe: impl FnMut(u32, &[i16], &[i16]) -> io::Result<()>,
) -> Result<CaptureReport, String> {
    let mut report = CaptureReport {
        input_fnv1a: 1469598103934665603,
        ..Default::default()
    };
    let mut hosts: Option<(u32, COracle, Mixer)> = None;
    let mut bytes = Vec::new();
    let mut envelope = false;
    loop {
        bytes.clear();
        let read = input
            .by_ref()
            .take((MAX_LINE_BYTES + 1) as u64)
            .read_until(b'\n', &mut bytes)
            .map_err(|e| format!("input read: {e}"))?;
        if read == 0 {
            break;
        }
        if report.sealed {
            return Err("records after capture seal".into());
        }
        report.lines += 1;
        let fail = |s: &str| format!("line {}: {s}", report.lines);
        if read > MAX_LINE_BYTES {
            return Err(fail("record exceeds bound"));
        }
        if bytes.last() != Some(&b'\n') {
            return Err(fail("unterminated record"));
        }
        let prefix_hash = report.input_fnv1a;
        report.input_bytes += read as u64;
        for &b in &bytes {
            report.input_fnv1a = (report.input_fnv1a ^ u64::from(b)).wrapping_mul(1099511628211);
        }
        let line = std::str::from_utf8(&bytes[..read - 1]).map_err(|_| fail("invalid UTF-8"))?;
        // Error messages intentionally never echo payload bytes or owner audio.
        match parse(line).map_err(|e| fail(&e))? {
            Record::Envelope => {
                if report.lines != 1 {
                    return Err(fail("capture header must be first"));
                }
                envelope = true;
            }
            Record::End(records, hash) => {
                if !envelope || records != (report.lines - 1) as u64 || hash != prefix_hash {
                    return Err(fail("capture seal differs from prefix"));
                }
                report.sealed = true;
            }
            Record::Start(rate, tps) => {
                if hosts.is_some() {
                    return Err(fail("duplicate START"));
                }
                hosts = Some((rate, COracle::new(rate, tps), Mixer::new(rate, tps)));
            }
            Record::Completion => report.completion_observations += 1,
            record => {
                let (rate, c, rust) = hosts
                    .as_mut()
                    .ok_or_else(|| fail("mixer record before START"))?;
                match record {
                    Record::Apply(command, expected) => {
                        let a = c.apply(&command);
                        let b = rust.apply(&command);
                        if a != expected || b != expected {
                            return Err(fail("command acceptance differs from capture"));
                        }
                        report.commands += 1;
                    }
                    Record::RefusedBuffer => {
                        report.commands += 1;
                        report.refused_buffers += 1;
                    }
                    Record::Render {
                        ticks,
                        capacity,
                        frames,
                        hash,
                    } => {
                        let a = c.render_until(ticks, capacity);
                        let b = rust.render_until(ticks, capacity);
                        compare_samples(report.lines, &a, &b)
                            .map_err(|_| fail("C/Rust PCM differs"))?;
                        if a.len() / 2 != frames || fnv1a(&a) != hash {
                            return Err(fail("C PCM differs from recorded frames/hash"));
                        }
                        observe(*rate, &a, &b).map_err(|e| fail(&format!("PCM output: {e}")))?;
                        report.renders += 1;
                        report.samples += a.len();
                        report.nonsilent_samples += a.iter().filter(|&&s| s != 0).count();
                    }
                    _ => unreachable!(),
                }
            }
        }
    }
    if envelope && !report.sealed {
        return Err("missing capture seal".into());
    }
    if report.commands == 0 || report.nonsilent_samples == 0 {
        return Err("vacuous command/PCM capture".into());
    }
    Ok(report)
}

/// Stream paired WAVs to a fresh directory, publishing a receipt only after both
/// files finalize successfully. Source provenance must be established by the caller.
pub fn write_evidence(input: &Path, out: &Path) -> Result<String, String> {
    let input = fs::File::open(input).map_err(|e| format!("open capture: {e}"))?;
    fs::create_dir(out).map_err(|e| format!("fresh output directory: {e}"))?;
    let mut wavs: Option<(PcmWav, PcmWav)> = None;
    let report = compare_log(BufReader::new(input), |rate, c, rust| {
        if wavs.is_none() {
            wavs = Some((
                PcmWav::new(&out.join("capture-c.wav"), rate)?,
                PcmWav::new(&out.join("capture-rust.wav"), rate)?,
            ));
        }
        let (a, b) = wavs.as_mut().unwrap();
        a.write(c)?;
        b.write(rust)
    })?;
    let (a, b) = wavs.ok_or("missing WAV output")?;
    a.finish()
        .and_then(|_| b.finish())
        .map_err(|e| format!("finalize WAV: {e}"))?;
    let receipt = format!(
        "{{\"verdict\":\"EQUAL\",\"source\":\"command-log\",\"boundary\":\"offline-mixer\",\"tolerance\":0,\"sealed\":{},\"lines\":{},\"commands\":{},\"refused_buffers\":{},\"completion_observations_excluded\":{},\"renders\":{},\"samples\":{},\"nonsilent_samples\":{},\"input_bytes\":{},\"input_fnv1a\":\"{:016x}\"}}\n",
        report.sealed,
        report.lines,
        report.commands,
        report.refused_buffers,
        report.completion_observations,
        report.renders,
        report.samples,
        report.nonsilent_samples,
        report.input_bytes,
        report.input_fnv1a,
    );
    let pending = out.join("receipt.pending");
    let mut file = fs::File::create(&pending).map_err(|e| e.to_string())?;
    file.write_all(receipt.as_bytes())
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())?;
    fs::rename(pending, out.join("receipt.json")).map_err(|e| e.to_string())?;
    Ok(receipt)
}
