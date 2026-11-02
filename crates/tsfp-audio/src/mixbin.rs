//! IDirectSound_SetMixBinHeadroom: per-device 32-bin submix shift table.
//! Source contract: `src/audio/dsound_mixbin_headroom.c`. Host-free: device ownership, IRQL, the
//! guest global word and the PCM renderer's verdict are inputs.
//!
//! Original state is one byte per bin. A freshly created device has bins 0..30 at 1 and bin 31 at
//! 0. The table follows the last device it was bound to or written for. The headroom argument is
//! stored truncated to a byte, exactly as the C keeps it.
use crate::adapter::Refusal;

pub const ENTRY: u32 = 0x0040_7A2C;
pub const BINS: usize = 32;
/// E_FAIL, returned when the guest global audio state is non-zero.
pub const E_FAIL: u32 = 0x8000_4005;

/// Device identity packed as `heap << 32 | internal address`.
pub fn pack_identity(device_heap: u32, internal_address: u32) -> u64 {
    (u64::from(device_heap) << 32) | u64::from(internal_address)
}

pub fn created_amounts() -> [u8; BINS] {
    let mut amounts = [1; BINS];
    amounts[BINS - 1] = 0;
    amounts
}

/// Host inputs. `pcm_policy` false means no renderer accepts headroom (the C `pcm_note` NULL).
#[derive(Clone, Copy, Debug, Default)]
pub struct MixbinEnv {
    pub pcm_policy: bool,
    pub irql: Option<u8>,
    pub global_state: Option<u32>,
}

#[derive(Clone, Debug, Default)]
pub struct MixbinHeadroom {
    identity: u64,
    amounts: [u8; BINS],
}

impl MixbinHeadroom {
    pub const fn new() -> Self {
        Self {
            identity: 0,
            amounts: [0; BINS],
        }
    }

    /// Reconfiguration clears the bound identity and every bin (C `configure`).
    pub fn configure(&mut self) {
        *self = Self::default();
    }

    /// Device create note. `renderer_bound` is the PCM renderer's bind verdict.
    pub fn bind(&mut self, identity: u64, renderer_bound: bool) -> bool {
        if renderer_bound {
            self.identity = identity;
            self.amounts = created_amounts();
        }
        renderer_bound
    }

    fn table_for(&self, identity: u64) -> [u8; BINS] {
        if identity == self.identity {
            self.amounts
        } else {
            created_amounts()
        }
    }

    /// `device` is `None` when ownership validation fails.
    pub fn snapshot(&self, device: Option<u64>) -> Option<[u8; BINS]> {
        device.map(|identity| self.table_for(identity))
    }

    /// `renderer` receives (identity, bin, headroom) and returns whether the PCM side accepted it.
    /// The table changes only when it does.
    pub fn set(
        &mut self,
        env: MixbinEnv,
        device: impl FnOnce() -> Option<u64>,
        bin: u32,
        headroom: u32,
        renderer: impl FnOnce(u64, u32, u32) -> bool,
    ) -> Result<u32, Refusal> {
        if !env.pcm_policy || env.irql != Some(0) {
            return Err(Refusal("requires explicit PCM policy and known IRQL0"));
        }
        match env.global_state {
            None => return Err(Refusal("global audio state inaccessible")),
            Some(0) => {}
            Some(_) => return Ok(E_FAIL),
        }
        let identity = device().ok_or(Refusal("device ownership/header invalid"))?;
        if bin as usize >= BINS {
            return Err(Refusal("bin outside original 32-bin submix range"));
        }
        let mut candidate = self.table_for(identity);
        candidate[bin as usize] = headroom as u8;
        if !renderer(identity, bin, headroom) {
            return Err(Refusal(
                "PCM rejected clock/history/device lifetime or inactive renderer",
            ));
        }
        self.amounts = candidate;
        self.identity = identity;
        Ok(0)
    }
}
