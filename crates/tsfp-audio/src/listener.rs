//! Passive listener startup cache (DoppLer, Position, Orientation, CommitDeferredSettings, DoWork).
//! Source contract: `src/audio/dsound_listener.c`. Host-free: the device ownership check, IRQL and
//! the guest global word are inputs, so no guest memory or C is touched here.
//!
//! The original game issues exactly one startup sequence (Doppler 0, Position origin, Orientation
//! front +Z / top +Y, all deferred), then CommitDeferredSettings and DoWork. Only that exact
//! sequence is accepted; anything else is a `Refusal` the caller degrades or stops on. No 3D math,
//! APU or voice propagation is modelled (the C claims none either).
use crate::adapter::Refusal;

pub const WORK: u32 = 0x0040_7B40;
pub const COMMIT: u32 = 0x0040_9109;
pub const DOPPLER: u32 = 0x0040_93EC;
pub const POSITION: u32 = 0x0040_945A;
pub const ORIENTATION: u32 = 0x0040_9410;
/// Guest global audio state word, must read zero.
pub const GLOBAL_STATE: u32 = 0x0041_24A8;
/// Guest device singleton, DoWork targets its internal address + 8.
pub const DEVICE_SINGLETON: u32 = 0x0041_2B30;

const ONE: u32 = 0x3F80_0000; // 1.0f32 bits
/// Front (0,0,1) and top (0,1,0): the only orientation the startup sequence sets.
pub const STARTUP_ORIENTATION: [u32; 6] = [0, 0, ONE, 0, ONE, 0];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeviceIdentity {
    pub device_heap: u32,
    pub internal_address: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListenerCall {
    Doppler { factor: u32, apply: u32 },
    Position { xyz: [u32; 3], apply: u32 },
    Orientation { front_top: [u32; 6], apply: u32 },
    Commit,
    Work,
}

impl ListenerCall {
    pub fn entry(self) -> u32 {
        match self {
            Self::Doppler { .. } => DOPPLER,
            Self::Position { .. } => POSITION,
            Self::Orientation { .. } => ORIENTATION,
            Self::Commit => COMMIT,
            Self::Work => WORK,
        }
    }
}

/// Host inputs read at call time. `None` means the provider is absent or the read failed.
#[derive(Clone, Copy, Debug, Default)]
pub struct ListenerEnv {
    pub enabled: bool,
    pub irql: Option<u8>,
    pub global_state: Option<u32>,
}

/// Mirrors `dsound_listener_snapshot`. `cache_mask` bit 0 Doppler, 1 Position, 2 Orientation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListenerSnapshot {
    pub identity: DeviceIdentity,
    pub cache_mask: u32,
    pub doppler_bits: u32,
    pub position: [u32; 3],
    pub orientation: [u32; 6],
    pub commit_seen: bool,
    pub work_seen: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accepted {
    /// HRESULT (S_OK) or, for DoWork, the observed EAX (zero).
    pub result: u32,
    /// First accepted setter since reset: the host logs its one-time banner.
    pub announce: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ListenerCache {
    state: ListenerSnapshot,
    announced: bool,
}

impl ListenerCache {
    pub const fn new() -> Self {
        Self {
            state: ListenerSnapshot {
                identity: DeviceIdentity {
                    device_heap: 0,
                    internal_address: 0,
                },
                cache_mask: 0,
                doppler_bits: 0,
                position: [0; 3],
                orientation: [0; 6],
                commit_seen: false,
                work_seen: false,
            },
            announced: false,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// DoWork reads the device singleton itself; returns the interface the call acts on.
    pub fn work_interface(singleton: Option<u32>) -> Result<u32, Refusal> {
        match singleton {
            Some(internal) if internal != 0 && internal <= u32::MAX - 8 => Ok(internal + 8),
            _ => Err(Refusal("unavailable or invalid device singleton")),
        }
    }

    /// Read-only observation. `device` is `None` when ownership validation fails.
    pub fn snapshot(&self, device: Option<DeviceIdentity>) -> Option<ListenerSnapshot> {
        let identity = device?;
        if self.changed_incarnation(identity) {
            return None;
        }
        Some(ListenerSnapshot {
            identity,
            ..self.state
        })
    }

    fn changed_incarnation(&self, identity: DeviceIdentity) -> bool {
        self.state.cache_mask != 0 && self.state.identity != identity
    }

    /// One listener call on an owned device. Refusals leave the cache unchanged.
    pub fn call(
        &mut self,
        device: Option<DeviceIdentity>,
        call: ListenerCall,
        env: ListenerEnv,
    ) -> Result<Accepted, Refusal> {
        let identity = device.ok_or(Refusal("unowned or changed device"))?;
        if self.changed_incarnation(identity) {
            return Err(Refusal("cached device incarnation changed"));
        }
        if !env.enabled {
            return Err(Refusal("explicit headless-listener policy is disabled"));
        }
        if env.irql != Some(0) {
            return Err(Refusal("only known IRQL0 supported"));
        }
        if env.global_state != Some(0) {
            return Err(Refusal("original global audio state must be zero"));
        }
        let (stage, values, apply) = match call {
            ListenerCall::Commit | ListenerCall::Work => return self.commit_or_work(call),
            ListenerCall::Doppler { factor, apply } => (0, [factor, 0, 0, 0, 0, 0], apply),
            ListenerCall::Position {
                xyz: [x, y, z],
                apply,
            } => (1, [x, y, z, 0, 0, 0], apply),
            ListenerCall::Orientation { front_top, apply } => (2, front_top, apply),
        };
        let expected = if stage == 2 {
            STARTUP_ORIENTATION
        } else {
            [0; 6]
        };
        // Setters must arrive in order, deferred, with the startup values.
        if apply != 0 || self.state.cache_mask != (1 << stage) - 1 || values != expected {
            return Err(Refusal("unsupported startup scalar bits, apply or order"));
        }
        self.state.identity = identity;
        match stage {
            0 => self.state.doppler_bits = values[0],
            1 => self.state.position.copy_from_slice(&values[..3]),
            _ => self.state.orientation = values,
        }
        self.state.cache_mask |= 1 << stage;
        let announce = !self.announced;
        self.announced = true;
        Ok(Accepted {
            result: 0,
            announce,
        })
    }

    fn commit_or_work(&mut self, call: ListenerCall) -> Result<Accepted, Refusal> {
        let s = &self.state;
        if s.cache_mask != 7
            || s.doppler_bits != 0
            || s.position != [0; 3]
            || s.orientation != STARTUP_ORIENTATION
        {
            return Err(Refusal(
                "only completed exact startup listener commit request is supported",
            ));
        }
        if call == ListenerCall::Work {
            if !s.commit_seen {
                return Err(Refusal("DoWork requires recorded listener commit"));
            }
            self.state.work_seen = true;
        } else {
            self.state.commit_seen = true;
        }
        Ok(Accepted {
            result: 0,
            announce: false,
        })
    }
}
