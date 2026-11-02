// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline ordered DrawList comparison. No GPU, replay, guest memory or live routing.
//!
//! Adapters must decode bounded frames and bind resources to the pinned manifest.
//! This library checks supplied typed frames; it does not decode NV2A or certify
//! adapter truthfulness. Equality alone is not a Tier 2 evidence declaration.
mod codec;
mod fields;
pub mod wire;

use fields::{Budget, Field};
use tsfp_scene_model::{CONTRACT_VERSION, ContentId, Draw, DrawList, Sampling};

/// Shared provenance: native inputs must originate from this same oracle capture.
/// The candidate's own revision is recorded separately in `Comparison`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capture {
    pub contract_version: u32,
    pub decoder_version: u32,
    pub original_revision: [u8; 20],
    pub original_prebuilt: ContentId,
    pub route: ContentId,
    pub input: ContentId,
    pub stream: ContentId,
    pub resources: ContentId,
    pub first_present: u64,
    pub frame_count: u64,
}
impl Capture {
    fn valid(&self) -> bool {
        self.contract_version == CONTRACT_VERSION
            && self.decoder_version != 0
            && self.original_revision != [0; 20]
            && [
                self.original_prebuilt,
                self.route,
                self.input,
                self.stream,
                self.resources,
            ]
            .iter()
            .all(|id| id.0 != [0; 32])
            && self.frame_count > 0
            && self
                .first_present
                .checked_add(self.frame_count - 1)
                .is_some()
    }
}

/// Decoder/accounting facts, never inferred from native draw counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Gaps {
    pub dropped_records: u64,
    pub missing_resources: u64,
    pub unsupported_states: u64,
    pub unmatched_views: u64,
}
#[derive(Clone, Debug)]
pub struct Frame {
    pub present: u64,
    pub gaps: Gaps,
    pub draws: DrawList,
}

/// Sources retain only their bounded decoder state plus the current frame.
/// Enforce this byte limit BEFORE allocating a record or reading a payload.
/// A truncated record, unknown opcode or unresolved manifest entry is an error
/// or a counted gap, never an EOF or an omitted draw.
pub trait FrameSource {
    fn next_frame(&mut self, max_frame_bytes: usize) -> Result<Option<Frame>, String>;
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub max_frame_bytes: usize,
    pub max_field_visits: usize,
    pub max_frames: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_frame_bytes: 64 * 1024 * 1024,
            max_field_visits: 4_000_000,
            max_frames: 1_000_000,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Oracle,
    Native,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    Provenance,
    Decode {
        side: Side,
        message: String,
    },
    Incomplete {
        side: Side,
        gaps: Gaps,
    },
    FrameCount {
        side: Side,
        expected: u64,
        actual: u64,
    },
    Present {
        side: Side,
        expected: u64,
        actual: u64,
    },
    TickRegression {
        side: Side,
    },
    Invalid {
        side: Side,
        message: String,
    },
    NotOriginal {
        side: Side,
    },
    Limit {
        field: String,
    },
    Different {
        field: String,
        oracle: String,
        native: String,
    },
    NoDraws,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComparisonError {
    pub frame_index: u64,
    pub present: Option<u64>,
    pub compared_frames: u64,
    pub failure: Failure,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Comparison {
    pub capture: Capture,
    pub native_revision: [u8; 20],
    pub compared_frames: u64,
    /// Geometry/particle commands, excluding clears; zero is never equality evidence.
    pub compared_draws: u64,
}

fn read(
    source: &mut impl FrameSource,
    side: Side,
    limits: Limits,
) -> Result<Option<Frame>, Failure> {
    source
        .next_frame(limits.max_frame_bytes)
        .map_err(|message| Failure::Decode { side, message })
}
fn admission(
    frame: &Frame,
    side: Side,
    present: u64,
    last_tick: Option<u64>,
) -> Result<(), Failure> {
    if frame.gaps != Gaps::default() {
        return Err(Failure::Incomplete {
            side,
            gaps: frame.gaps,
        });
    }
    if frame.present != present {
        return Err(Failure::Present {
            side,
            expected: present,
            actual: frame.present,
        });
    }
    if !matches!(frame.draws.sample.sampling, Sampling::Original) {
        return Err(Failure::NotOriginal { side });
    }
    if last_tick.is_some_and(|tick| frame.draws.sample.current_tick < tick) {
        return Err(Failure::TickRegression { side });
    }
    Ok(())
}

/// Compare in capture order, holding at most one decoded frame from each source.
/// Exact bits include signed zero. No tolerance, sorting, pointer masking or
/// omission is applied. On a difference, the field path includes pass/draw indices.
pub fn compare(
    oracle_capture: &Capture,
    native_capture: &Capture,
    native_revision: [u8; 20],
    oracle: &mut impl FrameSource,
    native: &mut impl FrameSource,
    limits: Limits,
) -> Result<Comparison, ComparisonError> {
    let error = |index, present, failure| ComparisonError {
        frame_index: index,
        present,
        compared_frames: index,
        failure,
    };
    if !oracle_capture.valid() || oracle_capture != native_capture || native_revision == [0; 20] {
        return Err(error(0, None, Failure::Provenance));
    }
    if limits.max_frame_bytes == 0
        || limits.max_field_visits == 0
        || oracle_capture.frame_count > limits.max_frames
    {
        return Err(error(
            0,
            None,
            Failure::Limit {
                field: "capture".into(),
            },
        ));
    }
    let mut draws = 0u64;
    let mut last_tick = None;
    for index in 0..oracle_capture.frame_count {
        let present = oracle_capture.first_present + index;
        let pair = (|| {
            let a = read(oracle, Side::Oracle, limits)?.ok_or(Failure::FrameCount {
                side: Side::Oracle,
                expected: oracle_capture.frame_count,
                actual: index,
            })?;
            let b = read(native, Side::Native, limits)?.ok_or(Failure::FrameCount {
                side: Side::Native,
                expected: oracle_capture.frame_count,
                actual: index,
            })?;
            admission(&a, Side::Oracle, present, last_tick)?;
            admission(&b, Side::Native, present, last_tick)?;
            // Budget the entire schema walk before structural validation, which
            // builds binding maps. No route-sized vector or canonical blob is built.
            a.draws
                .compare(&b.draws, "draw_list", &mut Budget(limits.max_field_visits))?;
            for (side, frame) in [(Side::Oracle, &a), (Side::Native, &b)] {
                frame.draws.validate().map_err(|e| Failure::Invalid {
                    side,
                    message: format!("{e:?}"),
                })?;
            }
            let count = a
                .draws
                .passes
                .iter()
                .flat_map(|pass| &pass.draws)
                .filter(|draw| !matches!(draw, Draw::Clear { .. }))
                .count() as u64;
            draws = draws.checked_add(count).ok_or(Failure::Limit {
                field: "total_draws".into(),
            })?;
            last_tick = Some(a.draws.sample.current_tick);
            Ok(())
        })();
        pair.map_err(|failure| error(index, Some(present), failure))?;
    }
    // An explicit declared count catches equal truncation and trailing frames.
    let count = oracle_capture.frame_count;
    for (side, extra) in [
        (Side::Oracle, read(oracle, Side::Oracle, limits)),
        (Side::Native, read(native, Side::Native, limits)),
    ] {
        if extra
            .map_err(|failure| error(count, None, failure))?
            .is_some()
        {
            return Err(error(
                count,
                None,
                Failure::FrameCount {
                    side,
                    expected: count,
                    actual: count.saturating_add(1),
                },
            ));
        }
    }
    if draws == 0 {
        return Err(error(count, None, Failure::NoDraws));
    }
    Ok(Comparison {
        capture: oracle_capture.clone(),
        native_revision,
        compared_frames: count,
        compared_draws: draws,
    })
}
