//! T1867: the hook ports record their effect spawns directly (`0xEFBD0(kind, ..)` as a kind, `0x111AE0(.., template, ..)`
//! as an emitter). This module turns each port's record into an [`EffectRequest`] and runs it through
//! [`HookServices`]: a kind goes through the kind table (rows loaded from the image by
//! [`super::svc_kind_table`]) and its ported handler (`0xEDD40` and `svc_particle_kinds`), an emitter goes through the
//! ported creation `0x111AE0`. The trace events the call recorded come back with the result.
//!
//! Argument shapes are the call sites' pushes (MEASURED for `0x220AF0` at `0x220D84..0x220D9C`: kind, 0, position pointer,
//! vector pointer, 0, 0, 2.0, 1; the other ports' shapes follow their documented calls and are INFERRED where noted).

use super::hooks_audio::StrikeRequest;
use super::hooks_fx::RoofEffect;
use super::hooks_l06_0b;
use super::hooks_l11;
use super::hooks_scene::{ParticleAnchor, SceneEffect};
use super::hooks_story::SparkEffect;
use super::svc_particle_efbd0::{SpawnArgs, SpawnResult};
use super::svc_wiring::{HookServices, TraceEvent};

/// One effect spawn a hook port asked for.
#[derive(Clone, Debug, PartialEq)]
pub enum EffectRequest {
    /// `0xEFBD0`: a kind table effect.
    Kind(SpawnArgs),
    /// `0x111AE0(0, template, flags, &position, &vector)`: an emitter from a template.
    Emitter {
        template: u32,
        flags: u32,
        position: [f32; 3],
        vector: [f32; 3],
    },
}

/// What running a request produced.
#[derive(Clone, Debug, PartialEq)]
pub enum RouteResult {
    Kind(SpawnResult),
    /// The result of `create_emitter` (1 = created, 0 = pool empty).
    Emitter(u32),
}

/// A request, the port that recorded it, the result and the engine calls recorded during it.
#[derive(Clone, Debug, PartialEq)]
pub struct RoutedEffect {
    pub origin: &'static str,
    pub request: EffectRequest,
    pub result: RouteResult,
    pub trace: Vec<TraceEvent>,
}

impl RoutedEffect {
    /// The effect kind started (kind requests only).
    pub fn kind(&self) -> Option<u32> {
        match &self.request {
            EffectRequest::Kind(args) => Some(args.kind),
            EffectRequest::Emitter { .. } => None,
        }
    }
}

/// Which ported code `0xEDD40` runs for a kind (`start_effect`): kind 15 is the ember pair, 6..=48 except 7, 8, 9, 19 and
/// 27 have a handler in `svc_particle_kinds`, everything else takes the default path over the kind row's effect ids.
pub fn handler_of(kind: u32) -> &'static str {
    match kind {
        15 => "ember (two emitters, scale)",
        7 | 8 | 9 | 19 | 27 => "default path (kind row effect ids)",
        6..=48 => "kind handler (svc_particle_kinds)",
        _ => "default path (kind row effect ids)",
    }
}

fn kind_args(
    kind: u32,
    extra: u32,
    position: [f32; 3],
    direction: Option<[f32; 3]>,
    seconds: f32,
) -> SpawnArgs {
    SpawnArgs {
        kind,
        extra,
        position,
        direction,
        source: None,
        owner: 0,
        seconds,
        sound: true,
    }
}

/// `0x220AF0` (level 0x30): `0xEFBD0(0xC, 0, point, vector, 0, 0, 2.0, 1)`. MEASURED shape; the real position is the ray
/// hit, the port only has the disc point, and the vector (a stack scratch) is taken as up (INFERRED).
pub fn from_strike(strike: &StrikeRequest) -> EffectRequest {
    EffectRequest::Kind(kind_args(0xC, 0, strike.point, Some([0.0, 1.0, 0.0]), 2.0))
}

/// Level 0x1D weather: a kind `0x25` table entry (`0x207830`, INFERRED seconds 0 and sound 1), or the rain particle
/// emitter `0x111AE0(handle, 0x78DBA0, 0, position, vector)`. Other effects are not spawns.
pub fn from_l11(effect: &hooks_l11::Effect) -> Option<EffectRequest> {
    match effect {
        hooks_l11::Effect::SpawnTableEntry { kind, position } => Some(EffectRequest::Kind(
            kind_args(*kind, 0, *position, None, 0.0),
        )),
        hooks_l11::Effect::SpawnParticle {
            template,
            position,
            vector,
            ..
        } => Some(EffectRequest::Emitter {
            template: *template,
            flags: 0,
            position: *position,
            vector: *vector,
        }),
        _ => None,
    }
}

/// Level 0xB barrier (`0x1EC300`): `0x111AE0(&[0x78E8E4], 0x78ED40, 0x10000, a, b)`.
pub fn from_barrier(effect: &hooks_l06_0b::Effect) -> Option<EffectRequest> {
    match effect {
        hooks_l06_0b::Effect::CreateEmitter { vector_a, vector_b } => {
            Some(EffectRequest::Emitter {
                template: 0x78_ED40,
                flags: 0x1_0000,
                position: *vector_a,
                vector: *vector_b,
            })
        }
        _ => None,
    }
}

/// `0x1F9DA0` roof: three `0xEFBD0(0x16, 0x17, position, [1, 0, 0], 0, 0, 0, 1)` bursts.
pub fn from_roof(effect: &RoofEffect) -> Vec<EffectRequest> {
    match effect {
        RoofEffect::SecondExpired { bursts } => bursts
            .iter()
            .map(|p| EffectRequest::Kind(kind_args(0x16, 0x17, *p, Some([1.0, 0.0, 0.0]), 0.0)))
            .collect(),
        RoofEffect::FirstExpired => Vec::new(),
    }
}

/// Spark pool ticks (`0x21C110`, `0x21D520`): `0x111AE0` with template `0x7B0A80` (flags and sizes are set by later calls).
pub fn from_spark(effect: &SparkEffect) -> Option<EffectRequest> {
    match effect {
        SparkEffect::Spark { position, velocity } => Some(EffectRequest::Emitter {
            template: 0x7B_0A80,
            flags: 0,
            position: *position,
            vector: *velocity,
        }),
        _ => None,
    }
}

/// Scene particle spawns at a fixed world point (anchors on pieces and creatures need the owner's position and are not
/// resolved here, `None`).
pub fn from_scene(effect: &SceneEffect) -> Option<EffectRequest> {
    match effect {
        SceneEffect::Particle {
            template,
            anchor: ParticleAnchor::Point(point),
            vector,
        } => Some(EffectRequest::Emitter {
            template: *template,
            flags: 0,
            position: *point,
            vector: *vector,
        }),
        _ => None,
    }
}

/// Run one request over the services and collect its trace.
pub fn route(
    services: &mut HookServices,
    origin: &'static str,
    request: EffectRequest,
) -> RoutedEffect {
    let start = services.trace().len();
    let result = match &request {
        EffectRequest::Kind(args) => RouteResult::Kind(services.spawn_effect(args)),
        EffectRequest::Emitter {
            template,
            flags,
            position,
            vector,
        } => RouteResult::Emitter(services.create_emitter(*template, *flags, *position, *vector)),
    };
    let trace = services.trace()[start..].to_vec();
    RoutedEffect {
        origin,
        request,
        result,
        trace,
    }
}
