use tsfp_audio::{
    contract::{Command, SoundHost},
    mixer::Mixer,
};
use tsfp_audio_conformance::{Difference, compare, oracle::COracle, scenarios::*};

#[test]
fn fixed_inputs_match_c_sample_by_sample() {
    for (name, steps) in fixed_inputs() {
        let mut c = COracle::new(OUTPUT_HZ, TICKS_PER_SECOND);
        let mut rust = Mixer::new(OUTPUT_HZ, TICKS_PER_SECOND);
        let report = compare(&mut c, &mut rust, &steps, |_, _| {})
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        assert!(report.nonsilent_samples > 0, "{name}");
    }
}

struct Mutant {
    mixer: Mixer,
    kind: u8,
}
impl SoundHost for Mutant {
    fn apply(&mut self, command: &Command) -> bool {
        let verdict = self.mixer.apply(command);
        if self.kind == 4 { !verdict } else { verdict }
    }
    fn render_until(&mut self, ticks: u64, cap: usize) -> Vec<i16> {
        let mut pcm = self.mixer.render_until(ticks, cap);
        match self.kind {
            0 => {
                if let Some(sample) = pcm.iter_mut().find(|s| **s != 0) {
                    *sample ^= 1;
                }
            }
            1 => {
                for frame in pcm.chunks_exact_mut(2) {
                    frame.swap(0, 1);
                }
            }
            2 => {
                pcm.truncate(pcm.len().saturating_sub(2));
            }
            3 => {
                pcm.fill(0);
            }
            _ => {}
        }
        pcm
    }
}

#[test]
fn guard_detects_pcm_and_verdict_mutations() {
    let (_, steps) = fixed_inputs().remove(0);
    for kind in 0..5 {
        let mut c = COracle::new(OUTPUT_HZ, TICKS_PER_SECOND);
        let mut mutant = Mutant {
            mixer: Mixer::new(OUTPUT_HZ, TICKS_PER_SECOND),
            kind,
        };
        let result = compare(&mut c, &mut mutant, &steps, |_, _| {});
        match kind {
            0 | 1 | 3 => assert!(
                matches!(result, Err(Difference::Sample { .. })),
                "mutation {kind}: {result:?}"
            ),
            2 => assert!(
                matches!(result, Err(Difference::Length { .. })),
                "{result:?}"
            ),
            4 => assert!(
                matches!(result, Err(Difference::Verdict { .. })),
                "{result:?}"
            ),
            _ => unreachable!(),
        }
    }
}

#[test]
fn no_output_cannot_pass() {
    let mut c = COracle::new(OUTPUT_HZ, TICKS_PER_SECOND);
    let mut rust = Mixer::new(OUTPUT_HZ, TICKS_PER_SECOND);
    assert!(matches!(
        compare(&mut c, &mut rust, &[], |_, _| {}),
        Err(Difference::Vacuous)
    ));
}
