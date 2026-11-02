//! Tick functions installed by callbacks 24, 72 and 73 and the callees of callbacks 3, 38, 41 and 51 to 56 (T1844).
//!
//! Static reading of `build/default.xbe` (SHA-256 `3cfd001a...1816bc`), no xemu run. Control flow and constants are
//! MEASURED; the game meaning of a field is INFERRED unless stated. `0xA5CE0(object, state, function)` stores a
//! per-state tick function at `object + 0x154 + 4*state` (and `[object+0x90] = [0x7DE318]`). The frame step is
//! `dt = (1 / [0x4E8CEC]) * [0x4E7944]` with `[0x4E8CEC] = 60` in the image and `[0x4E7944]` the frame multiplier.

/// How sure a role is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confidence {
    /// Control flow and constants read from the image.
    Measured,
    /// Control flow read, the game meaning guessed from argument shapes.
    Inferred,
}

/// One function of the catalogue.
#[derive(Clone, Copy, Debug)]
pub struct FunctionRole {
    pub address: u32,
    pub role: &'static str,
    pub confidence: Confidence,
}

/// Decoded functions (T1844). The model functions of this module reproduce the `Measured` numeric ones.
pub const CATALOGUE: &[FunctionRole] = &[
    FunctionRole {
        address: 0x1EBF90,
        role: "TRST entry state 3 tick: shutter open/close state machine (see shutter_step)",
        confidence: Confidence::Measured,
    },
    FunctionRole {
        address: 0x1EC250,
        role: "TRST entry state 2 tick: skeleton pose update with blend ext+0x40 (0x153240 per node, 0x1536E0 pose), flag 0x200000",
        confidence: Confidence::Inferred,
    },
    FunctionRole {
        address: 0x1EB240,
        role: "ramp tick of callbacks 72 and 73 (see ramp_step)",
        confidence: Confidence::Measured,
    },
    FunctionRole {
        address: 0x1F2B00,
        role: "NKLC entry state 2 tick: skeleton pose update with blend +0xD0, scale from +0x1CC, flag 0x200000",
        confidence: Confidence::Inferred,
    },
    FunctionRole {
        address: 0x1E0A70,
        role: "tick id 3 helper (register convention, esi = object): transforms two anchor points of the object model by node matrices (0x152090), adds the object offset +0x64..0x6C and a vector, then sets the endpoints of the two handles at [ctrl+0x100] and [ctrl+0x104] (0xFF490) with length 30.0 (0xFF420); the 'XPSC' (0x43535058) type at +0x60 takes the +0x274 matrix path",
        confidence: Confidence::Inferred,
    },
    FunctionRole {
        address: 0xA6EC0,
        role: "spawn object of a type (class table 0x791D4C): 0xA6880 creates it, +0x20 = flags argument, 0x9E330 unless level 0x66, 0x6B060, then the class init callback [class+0x2C] unless flags == 0x100000",
        confidence: Confidence::Measured,
    },
    FunctionRole {
        address: 0xAE240,
        role: "timed state: when the flags argument is non-zero, +0x1A0 = flags | 1, +0x1A4 = (int)(60 * seconds), tick 0xAC560 installed in state 3 (0xA5CE0); body ported as pose::timed_tick (T1845)",
        confidence: Confidence::Measured,
    },
    FunctionRole {
        address: 0xAC560,
        role: "timed state tick (state 3 of 0xAE240): see pose::timed_tick (T1845)",
        confidence: Confidence::Measured,
    },
    FunctionRole {
        address: 0xA5CE0,
        role: "tick registration: stores the function at object + 0x154 + 4*state, global list moves for states 0, 8, 9 (pose::tick_slot_offset and tick_list, T1845)",
        confidence: Confidence::Measured,
    },
    FunctionRole {
        address: 0x172760,
        role: "thin wrapper of 0x172180 with a zero offset vector (particle or sprite start)",
        confidence: Confidence::Measured,
    },
    FunctionRole {
        address: 0x16C170,
        role: "cursor step over an indexed list (head 0x74C45E, tail 0x74C460, list 0x74C46C): kind 0, 1 or 2 (2 = step back)",
        confidence: Confidence::Inferred,
    },
    FunctionRole {
        address: 0x249C00,
        role: "take a free entry of the 6 entry emitter pool at [0x78B718] (stride 0x140, count [0x78B6FC]; entry state 2 = free) and fill three random floats in [-1, 1) at +0x1C, +0x20, +0x24",
        confidence: Confidence::Measured,
    },
    FunctionRole {
        address: 0x111AE0,
        role: "create a particle emitter record (0x10DF90) with two vec3 and flags; used by tick 0x1EC300 with template 0x78ED40",
        confidence: Confidence::Inferred,
    },
    FunctionRole {
        address: 0x1D8660,
        role: "start a fixed particle effect: no arguments, builds the template with the 0x1115F0 / 0x111680 / 0x1117F0 / 0x111920 / 0x111860 / 0x1118A0 chain and fixed constants",
        confidence: Confidence::Inferred,
    },
    FunctionRole {
        address: 0xB86A0,
        role: "interaction entry start (entry, target): player 0x243060, aborts when the linked object's health +0x1CC is above 0, requires entry+0xC == 0; entry mode 0 with the object's state +0x114 not 0x10 resets (0xB80A0), stores 0x217B0(.., entry object + 0x64) in [0x4DACE4] and runs 0xB76B0; other modes run 0xB7830 and either 0xB76B0 or reset plus stop of the sound handle [0x4DACE4]",
        confidence: Confidence::Inferred,
    },
];

/// Result of one [`shutter_step`].
#[derive(Clone, Debug, PartialEq)]
pub struct ShutterStep {
    pub state: u32,
    pub progress: f32,
    pub effects: Vec<ShutterEffect>,
}

/// Side effects of the shutter tick (`0x1EBF90`).
#[derive(Clone, Debug, PartialEq)]
pub enum ShutterEffect {
    /// The "SHUT" object's extension word `+0xAC` is set (3 while opening, 2 while closing) and its flag `0x4000000` set.
    ShutterDriving { value: u32 },
    /// The linked object's flag word `+0x18` gets bit 2.
    LinkedObjectFlag,
    /// Opening finished: "SHUT" flag `0x4000000` cleared, `[0x75BF0C] = 0`, `[0x75BFF4] = 1`, slot 0x27 started.
    OpenedStartSlot { slot: u32 },
    /// Closing finished: `[0x75BF0C] = 1`, `[0x75BFF4] = 0`, slots 2 and 0x27 finished.
    ClosedFinishSlots { slots: [u32; 2] },
}

/// Shutter speed (`[0x49E2B8]`) and limit (`[0x49E2C0]`, a double).
pub const SHUTTER_SPEED: f32 = 2.94;
pub const SHUTTER_LIMIT: f64 = 0.823;

/// `0x1EBF90` for the machine state of the TRST extension (`+0x3C`): 2 opens (progress falls by speed * dt and
/// is clamped to `-0.823` when its magnitude passes the limit, then state 1), 3 closes (progress rises by speed * dt
/// and is clamped to 0 when positive, then state 0). Other states do nothing. The timer `+0x1A4` of the linked
/// object also falls by `dt` every call (not modelled here).
pub fn shutter_step(state: u32, progress: f32, dt: f32) -> ShutterStep {
    let mut effects = Vec::new();
    match state {
        2 => {
            effects.push(ShutterEffect::ShutterDriving { value: 3 });
            let mut next = progress - dt * SHUTTER_SPEED;
            effects.push(ShutterEffect::LinkedObjectFlag);
            if (next.abs() as f64) > SHUTTER_LIMIT {
                next = -(SHUTTER_LIMIT as f32);
                effects.push(ShutterEffect::OpenedStartSlot { slot: 0x27 });
                return ShutterStep {
                    state: 1,
                    progress: next,
                    effects,
                };
            }
            ShutterStep {
                state,
                progress: next,
                effects,
            }
        }
        3 => {
            effects.push(ShutterEffect::ShutterDriving { value: 2 });
            let mut next = progress + dt * SHUTTER_SPEED;
            effects.push(ShutterEffect::LinkedObjectFlag);
            if next > 0.0 {
                next = 0.0;
                effects.push(ShutterEffect::ClosedFinishSlots { slots: [2, 0x27] });
                return ShutterStep {
                    state: 0,
                    progress: next,
                    effects,
                };
            }
            ShutterStep {
                state,
                progress: next,
                effects,
            }
        }
        _ => ShutterStep {
            state,
            progress,
            effects,
        },
    }
}

/// `0x1EB240`: `(blend, scale)` is `(+0xD0, +0x1CC)`. The blend rises by `dt * pi/2` and is capped at `pi/2`; the
/// scale is left alone when it is exactly 1.0, else it rises by `dt * 0.02` and is capped at `0.01`.
pub fn ramp_step(blend: f32, scale: f32, dt: f32) -> (f32, f32) {
    const BLEND_CAP: f32 = 1.570_796_4;
    const SCALE_RATE: f32 = 0.02;
    const SCALE_CAP: f32 = 0.01;
    let blend = (blend + dt * BLEND_CAP).min(BLEND_CAP);
    let scale = if scale == 1.0 {
        scale
    } else {
        (scale + dt * SCALE_RATE).min(SCALE_CAP)
    };
    (blend, scale)
}

/// The frame step `(1 / [0x4E8CEC]) * [0x4E7944]` with the image's rate of 60.
pub fn frame_dt(multiplier: i32) -> f32 {
    (1.0 / 60.0) * multiplier as f32
}
