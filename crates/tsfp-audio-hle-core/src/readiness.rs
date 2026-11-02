// SPDX-License-Identifier: GPL-3.0-or-later
//! AC97 codec readiness and the DSP command-word acknowledgement of `dsound_hle.h`.
use std::ffi::c_void;
use std::sync::Mutex;

use crate::table::ensure_initialised;
use crate::{lock, log};

pub const NULL_DEVICE_FAULT_VA: u32 = 0xFFFF_FFEC;
pub const AC97_PRIMARY_CODEC_READY: u32 = 0x100;
pub const DSP_ACK_UNSET: u32 = u32::MAX;
pub const DSP_ACK_UPSTREAM_OFFSET: u32 = 0x810;

/// `dsound_codec_state`.
pub const CODEC_NOT_READY: u32 = 0;
pub const CODEC_READY: u32 = 1;

/// `dsound_dsp_write_fn`.
pub type DspWriteFn = unsafe extern "C" fn(u32, u32, *mut c_void);

pub(crate) struct State {
    pub ac97_status: u32,
    pub codec_state: u32,
    announced: bool,
    pub not_ready_queries: u64,
    not_ready_reported: bool,
    writer: Option<DspWriteFn>,
    writer_user: usize,
    pub ack_base: u32,
    pub ack_offset: u32,
    pub ack_address: u32,
    pub ack_writes: u64,
    pub ack_refusals: u64,
    unconfigured_reported: bool,
}

const FRESH: State = State {
    ac97_status: 0,
    codec_state: CODEC_NOT_READY,
    announced: false,
    not_ready_queries: 0,
    not_ready_reported: false,
    writer: None,
    writer_user: 0,
    ack_base: DSP_ACK_UNSET,
    ack_offset: 0,
    ack_address: DSP_ACK_UNSET,
    ack_writes: 0,
    ack_refusals: 0,
    unconfigured_reported: false,
};

pub(crate) static STATE: Mutex<State> = Mutex::new(FRESH);

pub(crate) fn reset() {
    *lock(&STATE) = FRESH;
}

/// `dsound_hle_set_codec_state`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_set_codec_state(state: u32) {
    ensure_initialised();
    {
        let mut s = lock(&STATE);
        if state == s.codec_state && s.announced {
            return;
        }
        s.codec_state = state;
        s.announced = true;
        if state == CODEC_READY {
            s.ac97_status |= AC97_PRIMARY_CODEC_READY;
        } else {
            s.ac97_status &= !AC97_PRIMARY_CODEC_READY;
        }
    }
    if state == CODEC_READY {
        log(&format!(
            "dsound: codec readiness FORCED READY (FABRICATED -- there is no AC97 codec and no audio device; this \
             exists because two upstream titles fault at guest {NULL_DEVICE_FAULT_VA:#010x} on the NULL device a \
             not-ready codec produces)\n"
        ));
    } else {
        log("dsound: codec readiness set NOT READY (the honest default)\n");
    }
}

/// `dsound_hle_ac97_global_status`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_ac97_global_status() -> u32 {
    ensure_initialised();
    lock(&STATE).ac97_status
}

/// `dsound_hle_codec_state`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_codec_state() -> u32 {
    ensure_initialised();
    lock(&STATE).codec_state
}

/// `dsound_hle_codec_ready`: counts and signposts every not-ready query.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_codec_ready() -> bool {
    ensure_initialised();
    let first = {
        let mut s = lock(&STATE);
        if s.ac97_status & AC97_PRIMARY_CODEC_READY != 0 {
            return true;
        }
        s.not_ready_queries += 1;
        !std::mem::replace(&mut s.not_ready_reported, true)
    };
    if first {
        log(&format!(
            "dsound: codec is NOT READY -- DirectSoundCreate will fail, the NULL device will be carried forward. \
             Upstream titles then fault at guest {NULL_DEVICE_FAULT_VA:#010x}, but that address is AMBIGUOUS (a \
             missing fs:[4] produces it too), or run and never draw having skipped engine init. Enable the \
             fabricated ready state if you want past this.\n"
        ));
    }
    false
}

/// `dsound_hle_not_ready_query_count`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_not_ready_query_count() -> u64 {
    ensure_initialised();
    lock(&STATE).not_ready_queries
}

/// `dsound_hle_set_dsp_writer`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_set_dsp_writer(writer: Option<DspWriteFn>, user: *mut c_void) {
    ensure_initialised();
    let mut s = lock(&STATE);
    s.writer = writer;
    s.writer_user = user as usize;
}

/// `dsound_hle_set_dsp_ack`: refuses an unset base and a wrapping sum.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_set_dsp_ack(scratch_base: u32, offset: u32) -> bool {
    ensure_initialised();
    if scratch_base == DSP_ACK_UNSET || offset > u32::MAX - scratch_base {
        return false;
    }
    let address = scratch_base + offset;
    {
        let mut s = lock(&STATE);
        s.ack_base = scratch_base;
        s.ack_offset = offset;
        s.ack_address = address;
    }
    let note = if offset == DSP_ACK_UPSTREAM_OFFSET {
        " -- matches the +0x810 upstream measured in four titles"
    } else {
        " -- offset DIFFERS from upstream's +0x810"
    };
    log(&format!(
        "dsound: DSP command-word ack address DERIVED as {address:#010x} (base {scratch_base:#010x} + {offset:#x}){note}\n"
    ));
    true
}

/// `dsound_hle_dsp_ack_configured`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_dsp_ack_configured() -> bool {
    ensure_initialised();
    lock(&STATE).ack_address != DSP_ACK_UNSET
}

/// `dsound_hle_dsp_ack_address`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_dsp_ack_address() -> u32 {
    ensure_initialised();
    lock(&STATE).ack_address
}

/// `dsound_hle_ack_dsp_command`: never guesses a base; refuses unconfigured or writer-less acknowledgements.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_ack_dsp_command() -> bool {
    ensure_initialised();
    let (address, writer, user, first_unconfigured) = {
        let mut s = lock(&STATE);
        if s.ack_address == DSP_ACK_UNSET {
            s.ack_refusals += 1;
            let first = !std::mem::replace(&mut s.unconfigured_reported, true);
            (DSP_ACK_UNSET, None, 0, first)
        } else if s.writer.is_none() {
            s.ack_refusals += 1;
            (s.ack_address, None, 0, false)
        } else {
            s.ack_writes += 1;
            (s.ack_address, s.writer, s.writer_user, false)
        }
    };
    if address == DSP_ACK_UNSET {
        if first_unconfigured {
            log(&format!(
                "dsound: DSP command-word ack REFUSED -- no address derived for this title. Upstream acknowledges at \
                 its DirectSound scratch + {DSP_ACK_UPSTREAM_OFFSET:#x} in all four of its titles, but the BASE is \
                 per-title and is not guessed here. Derive it from the spin loop the guest is sitting in.\n"
            ));
        }
        return false;
    }
    let Some(writer) = writer else {
        log(&format!(
            "dsound: DSP command-word ack at {address:#010x} has no writer installed; nothing was written\n"
        ));
        return false;
    };
    // SAFETY: the installed writer with the user pointer it was installed with.
    unsafe { writer(address, 0, user as *mut c_void) };
    true
}

/// `dsound_hle_dsp_ack_count`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_dsp_ack_count() -> u64 {
    ensure_initialised();
    lock(&STATE).ack_writes
}

/// `dsound_hle_dsp_ack_refused_count`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_dsp_ack_refused_count() -> u64 {
    ensure_initialised();
    lock(&STATE).ack_refusals
}
