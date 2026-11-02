//! AUD-1 sample-exact comparison at the production C/Rust SoundHost boundary.
//! Fixed generated inputs are FABRICATED Tier 1, not a live host adoption gate.
pub mod capture;
mod capture_parse;
mod capture_wav;
pub mod completion_capture;
pub mod completion_stream;
pub mod oracle;
pub mod scenarios;

use tsfp_audio::contract::{Command, SoundHost};

#[derive(Debug, PartialEq, Eq)]
pub enum Difference {
    Verdict {
        step: usize,
        c: bool,
        rust: bool,
    },
    Length {
        step: usize,
        c: usize,
        rust: usize,
    },
    Sample {
        step: usize,
        frame: usize,
        channel: usize,
        c: i16,
        rust: i16,
    },
    Vacuous,
}

#[derive(Debug, Clone)]
pub enum Step {
    Command(Command),
    Render { ticks: u64, capacity: usize },
}

#[derive(Debug, Default)]
pub struct Report {
    pub commands: usize,
    pub renders: usize,
    pub samples: usize,
    pub nonsilent_samples: usize,
}

/// Zero tolerance, including the resampler. Never treats missing output as equality.
pub fn compare_samples(step: usize, c: &[i16], rust: &[i16]) -> Result<(), Difference> {
    if c.len() != rust.len() {
        return Err(Difference::Length {
            step,
            c: c.len(),
            rust: rust.len(),
        });
    }
    for (i, (&c, &rust)) in c.iter().zip(rust).enumerate() {
        if c != rust {
            return Err(Difference::Sample {
                step,
                frame: i / 2,
                channel: i % 2,
                c,
                rust,
            });
        }
    }
    Ok(())
}

/// Feed one immutable sequence to independent hosts; observer can write paired WAV evidence.
pub fn compare(
    c: &mut impl SoundHost,
    rust: &mut impl SoundHost,
    steps: &[Step],
    mut observe: impl FnMut(&[i16], &[i16]),
) -> Result<Report, Difference> {
    let mut report = Report::default();
    for (step, event) in steps.iter().enumerate() {
        match event {
            Step::Command(command) => {
                let a = c.apply(command);
                let b = rust.apply(command);
                if a != b {
                    return Err(Difference::Verdict {
                        step,
                        c: a,
                        rust: b,
                    });
                }
                report.commands += 1;
            }
            Step::Render { ticks, capacity } => {
                let a = c.render_until(*ticks, *capacity);
                let b = rust.render_until(*ticks, *capacity);
                observe(&a, &b);
                compare_samples(step, &a, &b)?;
                report.renders += 1;
                report.samples += a.len();
                report.nonsilent_samples += a.iter().filter(|&&s| s != 0).count();
            }
        }
    }
    if report.commands == 0 || report.nonsilent_samples == 0 {
        return Err(Difference::Vacuous);
    }
    Ok(report)
}
