//! Typed game-to-audio boundary for the REA contract in `docs/t1669-rea-audio.md`.
//!
//! This is the owned Rust edge after the guest adapter has decoded the title's calls. It keeps
//! the manager's request/result ABI separate from DirectSound HRESULTs, carries object lease
//! identity into the replacement host, and maps admitted voice/buffer/stream effects to the
//! existing [`crate::contract::Command`] stream. The legacy guest adapter still owns raw addresses
//! and call admission; this module does not claim that live guest wiring is complete.

use crate::contract::{Command, Routing, SoundHost};
use crate::setters::{BUFFER_CURVE, STREAM_CURVE, STREAM_CURVE_FAR, STREAM_CURVE_NEAR, Setters};

/// Calling conventions retained by the two distinct game/audio layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CallingConvention {
    Cdecl,
    Stdcall,
    Thiscall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReturnRegister {
    Eax,
    Al,
}

/// Compact ABI facts useful to bindings and conformance tests. `stack_bytes` excludes the return
/// address; `this_in_ecx` is true only for thiscall entry points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AbiContract {
    pub convention: CallingConvention,
    /// `None` means the byte count varies by method.
    pub stack_bytes: Option<u8>,
    pub this_in_ecx: bool,
    pub result_register: ReturnRegister,
    pub callee_pops_stack: bool,
}

/// `0x45820(request)`: cdecl, one pointer argument, signed manager result in EAX.
pub const SOUND_SUBMIT_ABI: AbiContract = AbiContract {
    convention: CallingConvention::Cdecl,
    stack_bytes: Some(4),
    this_in_ecx: false,
    result_register: ReturnRegister::Eax,
    callee_pops_stack: false,
};

/// DirectSound virtual methods: stdcall, interface pointer first, HRESULT in EAX, callee cleanup.
pub const DIRECTSOUND_METHOD_ABI: AbiContract = AbiContract {
    convention: CallingConvention::Stdcall,
    stack_bytes: None, // Method-specific arguments follow `this`; see the method table in the contract doc.
    this_in_ecx: false,
    result_register: ReturnRegister::Eax,
    callee_pops_stack: true,
};

/// Internal stream predicates `0x29CC0`/`0x29D10`: thiscall and callers consume AL only.
pub const STREAM_PREDICATE_ABI: AbiContract = AbiContract {
    convention: CallingConvention::Thiscall,
    stack_bytes: Some(0),
    this_in_ecx: true,
    result_register: ReturnRegister::Al,
    callee_pops_stack: false,
};

/// A guest address at the compatibility boundary. Native game state should use owned handles;
/// this wrapper exists only where the current guest adapter still needs an address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GuestAddress(pub u32);

/// A game-visible sound token. Keep the original bits: class bits are part of the token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SoundToken(pub u32);

/// The manager's `0x45820` return values. They are not HRESULTs or booleans.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubmitResult {
    Failed,
    QueueFull,
    Duplicate,
    Started(SoundToken),
    Unclassified(i32),
}

impl SubmitResult {
    pub const fn from_abi(value: i32) -> Self {
        match value {
            -1 => Self::Failed,
            -2 => Self::QueueFull,
            -3 => Self::Duplicate,
            other if other >= 0 => Self::Started(SoundToken(other as u32)),
            other => Self::Unclassified(other),
        }
    }

    pub const fn to_abi(self) -> i32 {
        match self {
            Self::Failed => -1,
            Self::QueueFull => -2,
            Self::Duplicate => -3,
            Self::Started(token) => token.0 as i32,
            Self::Unclassified(value) => value,
        }
    }
}

/// Owned, bit-preserving view of the manager's 0x30-byte request (`0x45390`). Float fields stay
/// as IEEE-754 bits so NaNs and signed zero survive the ABI round trip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SoundRequest {
    pub sound_id: u32,
    pub owner: GuestAddress,
    pub gain_bits: u32,
    pub reserved_0c: u32,
    pub flags: u32,
    pub ramp_bits: u32,
    pub scalar_bits: u32,
    pub position_bits: [u32; 3],
    pub positional: u8,
    pub reserved_29_2b: [u8; 3],
    pub priority_bits: u32,
}

impl SoundRequest {
    pub const SIZE: usize = 0x30;

    /// The measured default initializer values from `0x45390` / `0x21780`.
    pub const fn defaults(sound_id: u32) -> Self {
        Self {
            sound_id,
            owner: GuestAddress(0),
            gain_bits: 1.0f32.to_bits(),
            reserved_0c: 0,
            flags: 0x14,
            ramp_bits: 1.0f32.to_bits(),
            scalar_bits: 1.0f32.to_bits(),
            position_bits: [0; 3],
            positional: 0,
            reserved_29_2b: [0; 3],
            priority_bits: 50.0f32.to_bits(),
        }
    }

    pub fn from_bytes(bytes: [u8; Self::SIZE]) -> Self {
        let word = |offset: usize| {
            u32::from_le_bytes(
                bytes[offset..offset + 4]
                    .try_into()
                    .expect("fixed request word"),
            )
        };
        Self {
            sound_id: word(0),
            owner: GuestAddress(word(4)),
            gain_bits: word(8),
            reserved_0c: word(0x0c),
            flags: word(0x10),
            ramp_bits: word(0x14),
            scalar_bits: word(0x18),
            position_bits: [word(0x1c), word(0x20), word(0x24)],
            positional: bytes[0x28],
            reserved_29_2b: [bytes[0x29], bytes[0x2a], bytes[0x2b]],
            priority_bits: word(0x2c),
        }
    }

    pub fn to_bytes(self) -> [u8; Self::SIZE] {
        let mut out = [0; Self::SIZE];
        for (offset, value) in [
            (0, self.sound_id),
            (4, self.owner.0),
            (8, self.gain_bits),
            (0x0c, self.reserved_0c),
            (0x10, self.flags),
            (0x14, self.ramp_bits),
            (0x18, self.scalar_bits),
            (0x1c, self.position_bits[0]),
            (0x20, self.position_bits[1]),
            (0x24, self.position_bits[2]),
            (0x2c, self.priority_bits),
        ] {
            out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        out[0x28] = self.positional;
        out[0x29..0x2c].copy_from_slice(&self.reserved_29_2b);
        out
    }
}

/// One live object identity in the Rust adapter. Address alone is insufficient after address
/// reuse; serial is the replacement's lease generation and is not an original game token.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AudioObject {
    pub address: GuestAddress,
    pub serial: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioObjectKind {
    Buffer,
    Stream,
}

/// Position is kept as three source float words, with no coordinate transform or NaN cleanup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position3 {
    pub xyz_bits: [u32; 3],
}

/// The observed curve family selector. These identify source tables; they do not copy their
/// points and must not be treated as native pointers by new Rust-owned systems.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RolloffCurve {
    BufferDefault,
    StreamDefault,
    StreamNear,
    StreamFar,
}

/// Owned projection of cached 3D parameters. `None` means the setter has not published that field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpatialParameters {
    pub position: Option<Position3>,
    pub min_distance_bits: Option<u32>,
    pub max_distance_bits: Option<u32>,
    pub rolloff: Option<RolloffCurve>,
    pub i3dl2_source_words: Option<[u32; 9]>,
}

/// Read-only projection of the spatial state currently accepted by the Rust setter replacement.
pub trait SpatialStateSource {
    fn spatial_state(
        &self,
        kind: AudioObjectKind,
        object: AudioObject,
    ) -> Option<SpatialParameters>;
}

fn stream_curve(address: u32) -> Option<RolloffCurve> {
    match address {
        STREAM_CURVE => Some(RolloffCurve::StreamDefault),
        STREAM_CURVE_NEAR => Some(RolloffCurve::StreamNear),
        STREAM_CURVE_FAR => Some(RolloffCurve::StreamFar),
        _ => None,
    }
}

impl SpatialStateSource for Setters {
    fn spatial_state(
        &self,
        kind: AudioObjectKind,
        object: AudioObject,
    ) -> Option<SpatialParameters> {
        let address = object.address.0;
        match kind {
            AudioObjectKind::Stream => {
                let state = self.stream(address)?;
                if state.serial != object.serial {
                    return None;
                }
                Some(SpatialParameters {
                    position: (state.position_sets != 0).then_some(Position3 {
                        xyz_bits: state.position_bits,
                    }),
                    min_distance_bits: (state.cache_mask & 2 != 0)
                        .then_some(state.min_distance_bits),
                    max_distance_bits: state.max_distance_seen.then_some(state.max_distance_bits),
                    rolloff: (state.cache_mask & 4 != 0)
                        .then(|| stream_curve(state.curve_address))
                        .flatten(),
                    i3dl2_source_words: (state.cache_mask & 1 != 0).then_some(state.i3dl2),
                })
            }
            AudioObjectKind::Buffer => {
                let state = self.buffer(address)?;
                if state.serial != object.serial {
                    return None;
                }
                let rolloff = if state.cache_mask & 4 != 0 && state.curve_address == BUFFER_CURVE {
                    Some(RolloffCurve::BufferDefault)
                } else {
                    None
                };
                Some(SpatialParameters {
                    position: (state.position_sets != 0).then_some(Position3 {
                        xyz_bits: state.position_bits,
                    }),
                    min_distance_bits: (state.cache_mask & 2 != 0)
                        .then_some(state.min_distance_bits),
                    max_distance_bits: (state.max_distance_sets != 0)
                        .then_some(state.max_distance_bits),
                    rolloff,
                    i3dl2_source_words: (state.cache_mask & 1 != 0).then_some(state.i3dl2),
                })
            }
        }
    }
}

/// Admitted one-shot/loop voice start. `pcm` is decoded mono i16, matching the mixer voice command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoiceStart {
    pub object: AudioObject,
    pub ticks: u64,
    pub frequency: u32,
    pub pcm: Vec<i16>,
    pub paused: bool,
    pub looping: bool,
    pub loop_start: usize,
    pub loop_end: usize,
    pub millibels: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferCall {
    Pause {
        object: AudioObject,
        ticks: u64,
        paused: bool,
    },
    Stop {
        object: AudioObject,
        ticks: u64,
    },
    Frequency {
        object: AudioObject,
        ticks: u64,
        hertz: u32,
    },
    Volume {
        object: AudioObject,
        ticks: u64,
        millibels: i32,
    },
    Loop {
        object: AudioObject,
        ticks: u64,
        start: usize,
        end: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StreamCall {
    SubmitAdpcm {
        stream: GuestAddress,
        ticks: u64,
        source_hz: u32,
        mono: bool,
        data: Vec<u8>,
    },
    SubmitPcm16Stereo {
        stream: GuestAddress,
        ticks: u64,
        source_hz: u32,
        pcm: Vec<i16>,
    },
    Cut {
        stream: GuestAddress,
        ticks: u64,
    },
    Running {
        stream: GuestAddress,
        ticks: u64,
        running: bool,
    },
    Frequency {
        stream: GuestAddress,
        ticks: u64,
        source_hz: u32,
    },
    Volume {
        stream: GuestAddress,
        millibels: i32,
    },
}

/// The current mixer-facing subset of 3D state: stream routing and device headroom. Positional
/// source values remain in [`SpatialParameters`] until the renderer contract consumes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpatialCall {
    RouteStream {
        object: AudioObject,
        ticks: u64,
        routing: Routing,
    },
    EnableHeadroom {
        enabled: bool,
    },
    BindHeadroom {
        identity: u64,
    },
    SetHeadroom {
        identity: u64,
        ticks: u64,
        bin: u32,
        headroom: u32,
    },
}

/// Game-side audio effects admitted by the DirectSound compatibility adapter and delivered to
/// the Rust replacement. The bool is the mixer refusal contract: false means unchanged state.
pub trait GameAudioBoundary {
    fn start_voice(&mut self, start: VoiceStart) -> bool;
    fn buffer_call(&mut self, call: BufferCall) -> bool;
    fn spatial_call(&mut self, call: SpatialCall) -> bool;
    fn stream_call(&mut self, call: StreamCall) -> bool;
}

impl<H: SoundHost> GameAudioBoundary for H {
    fn start_voice(&mut self, start: VoiceStart) -> bool {
        self.apply(&Command::PlayBuffer {
            key: start.object.address.0,
            serial: start.object.serial,
            ticks: start.ticks,
            frequency: start.frequency,
            pcm: start.pcm,
            paused: start.paused,
            looping: start.looping,
            loop_start: start.loop_start,
            loop_end: start.loop_end,
            volume: start.millibels,
        })
    }

    fn buffer_call(&mut self, call: BufferCall) -> bool {
        let command = match call {
            BufferCall::Pause {
                object,
                ticks,
                paused,
            } => Command::PauseBuffer {
                key: object.address.0,
                serial: object.serial,
                ticks,
                paused,
            },
            BufferCall::Stop { object, ticks } => Command::StopBuffer {
                key: object.address.0,
                serial: object.serial,
                ticks,
            },
            BufferCall::Frequency {
                object,
                ticks,
                hertz,
            } => Command::FrequencyBuffer {
                key: object.address.0,
                serial: object.serial,
                ticks,
                frequency: hertz,
            },
            BufferCall::Volume {
                object,
                ticks,
                millibels,
            } => Command::VolumeBuffer {
                key: object.address.0,
                serial: object.serial,
                ticks,
                volume: millibels,
            },
            BufferCall::Loop {
                object,
                ticks,
                start,
                end,
            } => Command::LoopBuffer {
                key: object.address.0,
                serial: object.serial,
                ticks,
                loop_start: start,
                loop_end: end,
            },
        };
        self.apply(&command)
    }

    fn spatial_call(&mut self, call: SpatialCall) -> bool {
        let command = match call {
            SpatialCall::RouteStream {
                object,
                ticks,
                routing,
            } => Command::RouteStream {
                key: object.address.0,
                serial: object.serial,
                ticks,
                routing,
            },
            SpatialCall::EnableHeadroom { enabled } => Command::EnableHeadroom { enabled },
            SpatialCall::BindHeadroom { identity } => Command::BindHeadroom { identity },
            SpatialCall::SetHeadroom {
                identity,
                ticks,
                bin,
                headroom,
            } => Command::SetHeadroom {
                identity,
                ticks,
                bin,
                headroom,
            },
        };
        self.apply(&command)
    }

    fn stream_call(&mut self, call: StreamCall) -> bool {
        let command = match call {
            StreamCall::SubmitAdpcm {
                stream,
                ticks,
                source_hz,
                mono,
                data,
            } => Command::SubmitAdpcm {
                key: stream.0,
                ticks,
                source_hz,
                mono,
                data,
            },
            StreamCall::SubmitPcm16Stereo {
                stream,
                ticks,
                source_hz,
                pcm,
            } => Command::SubmitPcm16Stereo {
                key: stream.0,
                ticks,
                source_hz,
                pcm,
            },
            StreamCall::Cut { stream, ticks } => Command::CutStream {
                key: stream.0,
                ticks,
            },
            StreamCall::Running {
                stream,
                ticks,
                running,
            } => Command::SetStreamRunning {
                key: stream.0,
                ticks,
                running,
            },
            StreamCall::Frequency {
                stream,
                ticks,
                source_hz,
            } => Command::SetStreamFrequency {
                key: stream.0,
                ticks,
                source_hz,
            },
            StreamCall::Volume { stream, millibels } => Command::SetStreamVolume {
                key: stream.0,
                millibels,
            },
        };
        self.apply(&command)
    }
}
