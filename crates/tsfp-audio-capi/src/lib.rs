// SPDX-License-Identifier: GPL-3.0-or-later
//! T2046 audio compare mode: the Rust DirectSound lifecycle, setters and stream frames of `tsfp-audio` as a read-only twin of the
//! C host. The C glue (`src/audio/dsound_audio_cmp.c`, built with `-DTSFP_RUST_AUDIO_CMP=ON`, active with
//! `TSFP_RUST_AUDIO=compare`) calls [`tsfp_audcmp_init`] once and [`tsfp_audcmp_observe`] after every implemented DirectSound handler.
//! The C always answers. Design, measurements and the path to answering from Rust: docs/t-audio-replace.md, "Live compare mode".
// T3906: shared direct-pointer writer witness (host symbols in guest_mem.c).
pub mod adpcm_ffi;
pub use tsfp_audio_stream_snapshot::tsfp_audio_stream_snapshot_project_rust;
pub use tsfp_audio_stream_status::tsfp_audio_stream_status_admit_rust;
mod audio_capture;
pub mod buffer_frame_ffi;
#[cfg(all(feature = "rust-audio-host", not(test)))]
pub mod buffer_hle_ffi;
pub mod buffer_publish_ffi;
#[cfg(feature = "rust-audio-host")]
pub mod buffer_scope_ffi;
pub mod compare;
pub mod completion_layout;
#[cfg(feature = "rust-audio-host")]
pub mod device_ffi;
#[cfg(all(feature = "rust-audio-host", not(test)))]
pub mod device_host_ffi;
#[allow(dead_code)]
#[path = "../../tsfp-direct-witness/src/lib.rs"]
mod direct_witness;
pub mod effects_allocation_ffi;
pub mod effects_binding_ffi;
pub mod effects_cipher_ffi;
pub mod effects_guest_copy_ffi;
#[cfg(all(feature = "rust-audio-host", not(test)))]
pub mod effects_host_ffi;
pub mod effects_metadata_ffi;
pub mod effects_publish_ffi;
pub(crate) mod frame_guest_memory;
pub mod frequency_clock_ffi;
#[cfg(feature = "rust-audio-host")]
pub mod hle_host_entry;
pub mod hle_surface;
#[cfg(feature = "rust-audio-host")]
pub use tsfp_audio_hle_core as hle_core;
#[cfg(feature = "rust-audio-host")]
pub use tsfp_audio_host_wiring as host_wiring;
#[cfg(feature = "rust-audio-host")]
pub use tsfp_audio_movie_host as movie_host;
#[cfg(all(feature = "rust-audio-host", not(test)))]
pub mod completion_hle_ffi;
pub mod gpdsp_ffi;
pub mod host_runtime;
#[cfg(feature = "rust-audio-host")]
pub mod hrtf_hle_ffi;
#[cfg(all(feature = "rust-audio-host", not(test)))]
pub mod listener_hle_ffi;
#[cfg(all(feature = "rust-audio-host", not(test)))]
pub mod mixbin_hle_ffi;
pub mod movie_completion_ffi;
#[cfg(feature = "rust-audio-host")]
pub mod movie_device_hle_ffi;
pub mod movie_guest_call_ffi;
pub mod movie_policy_ffi;
pub mod movie_schedule_ffi;
pub mod refill_worker;
pub mod routing_ffi;
#[cfg(feature = "rust-audio-host")]
pub mod stream_control_hle_ffi;
pub(crate) mod stream_create_announcement;
#[cfg(feature = "rust-audio-host")]
pub mod stream_create_hle_ffi;
pub mod stream_format_ffi;
pub mod stream_frame_ffi;
#[cfg(feature = "rust-audio-host")]
pub mod stream_frequency_hle_ffi;
pub mod stream_lease_ffi;
pub mod stream_mutex_ffi;
pub mod stream_packet_ffi;
pub mod stream_pending_ffi;
pub mod stream_reset_ffi;
#[cfg(feature = "rust-audio-host")]
pub mod stream_routing_hle_ffi;
#[cfg(feature = "rust-audio-host")]
pub mod stream_scope_ffi;
#[cfg(feature = "rust-audio-host")]
pub mod stream_spatial_hle_ffi;
#[cfg(feature = "rust-audio-host")]
pub mod stream_volume_hle_ffi;
#[cfg(test)]
mod test_guest_memory;
pub mod work_lock;
pub mod world;

use std::sync::Mutex;
use std::time::{Duration, Instant};

use compare::{Config, Rec, Twin};
use world::Host;

/// Mirrors `audcmp_rec` of dsound_audio_cmp.c.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CRec {
    pub kind: u32,
    pub a: [u32; 5],
    pub serial: u64,
    pub result: u32,
    pub blob: [u32; 40],
}

/// Mirrors `audcmp_ops`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct COps {
    pub read: unsafe extern "C" fn(u32, *mut u8, u32) -> bool,
    pub readable: unsafe extern "C" fn(u32, u32) -> bool,
    pub irql0: unsafe extern "C" fn() -> bool,
    pub stream_snapshot: unsafe extern "C" fn(u32, *mut u32, u32) -> i32,
    pub buffer_snapshot: unsafe extern "C" fn(u32, *mut u32, u32) -> i32,
    pub codec_ready: unsafe extern "C" fn() -> bool,
}

/// Mirrors `audcmp_config`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CConfig {
    pub streams_enabled: u32,
    pub buffers_enabled: u32,
    pub completion: u32,
    pub frequency_hook: u32,
    pub routing_hook: u32,
    pub format_hook: u32,
    pub movie_calls: u32,
    /// 1: mode 1 (TSFP_RUST_AUDIO=1), forward the verified Rust answer.
    pub answer: u32,
    pub ops: COps,
}

/// The C host seen through its function pointers.
#[derive(Clone, Copy)]
struct CHost(COps);

const SNAPSHOT_CAPACITY: usize = 128;

impl Host for CHost {
    fn read(&self, address: u32, out: &mut [u8]) -> bool {
        // SAFETY: the C callback fills at most out.len() bytes of the buffer we hand it.
        unsafe { (self.0.read)(address, out.as_mut_ptr(), out.len() as u32) }
    }
    fn readable(&self, address: u32, len: u32) -> bool {
        // SAFETY: plain value call.
        unsafe { (self.0.readable)(address, len) }
    }
    fn irql0(&self) -> bool {
        // SAFETY: plain value call.
        unsafe { (self.0.irql0)() }
    }
    fn codec_ready(&self) -> bool {
        // SAFETY: plain value call.
        unsafe { (self.0.codec_ready)() }
    }
    fn stream_snapshot(&self, stream: u32) -> Option<Vec<u32>> {
        let mut buf = [0u32; SNAPSHOT_CAPACITY];
        // SAFETY: the callback writes at most SNAPSHOT_CAPACITY words and returns the full count.
        let n =
            unsafe { (self.0.stream_snapshot)(stream, buf.as_mut_ptr(), SNAPSHOT_CAPACITY as u32) };
        (n > 0 && n as usize <= SNAPSHOT_CAPACITY).then(|| buf[..n as usize].to_vec())
    }
    fn buffer_snapshot(&self, buffer: u32) -> Option<Vec<u32>> {
        let mut buf = [0u32; SNAPSHOT_CAPACITY];
        // SAFETY: as above.
        let n =
            unsafe { (self.0.buffer_snapshot)(buffer, buf.as_mut_ptr(), SNAPSHOT_CAPACITY as u32) };
        (n > 0 && n as usize <= SNAPSHOT_CAPACITY).then(|| buf[..n as usize].to_vec())
    }
}

/// Mode 1 bookkeeping. The C handler still ran (it owns the guest objects and the mixer state), the Rust answer is forwarded
/// only while this call produced no mismatch of any kind. The first one switches to C for good.
#[derive(Default)]
struct Mode1 {
    on: bool,
    fell_back: bool,
    /// EAX values taken from the Rust twin (equal to the C ones by construction of the check).
    answered: u64,
    /// Calls answered by the C because the twin did not model them (other handlers, movie streams, refusals).
    c_unmodelled: u64,
    c_foreign: u64,
    c_refused: u64,
    /// Calls answered by the C after the permanent fall back.
    c_after_fallback: u64,
    fallback_call: u64,
    calls: u64,
}

struct Global {
    twin: Twin<CHost>,
    last_summary: Instant,
    mode1: Mode1,
}

static GLOBAL: Mutex<Option<Global>> = Mutex::new(None);

/// Start the twin. Returns 1 on success. The fault injection variant reads `TSFP_AUDIO_FAULT=ret|state|refcount|header|hook`.
///
/// # Safety
/// `config` must point to a valid [`CConfig`] whose callbacks stay valid for the process lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audcmp_init(config: *const CConfig) -> u32 {
    if config.is_null() {
        return 0;
    }
    // SAFETY: checked non-null, the caller guarantees validity.
    let c = unsafe { *config };
    let cfg = Config {
        streams_enabled: c.streams_enabled != 0,
        buffers_enabled: c.buffers_enabled != 0,
        completion: c.completion != 0,
        frequency_hook: c.frequency_hook != 0,
        routing_hook: c.routing_hook != 0,
        format_hook: c.format_hook != 0,
        movie_calls: c.movie_calls != 0,
    };
    let answer = c.answer != 0;
    #[allow(unused_mut)]
    let mut twin = Twin::new(CHost(c.ops), cfg);
    #[cfg(feature = "fault-inject")]
    {
        use compare::Fault;
        twin.fault = match std::env::var("TSFP_AUDIO_FAULT").as_deref() {
            Ok("ret") => Some(Fault::Ret),
            Ok("state") => Some(Fault::State),
            Ok("refcount") => Some(Fault::Refcount),
            Ok("header") => Some(Fault::Header),
            Ok("hook") => Some(Fault::Hook),
            _ => None,
        };
        eprintln!("audio-cmp: FAULT INJECTION build, fault {:?}", twin.fault);
    }
    *GLOBAL.lock().unwrap_or_else(|e| e.into_inner()) = Some(Global {
        twin,
        last_summary: Instant::now(),
        mode1: Mode1 {
            on: answer,
            ..Mode1::default()
        },
    });
    1
}

/// Observe one handler call (after the C handler ran).
///
/// # Safety
/// `stack` must point to `stack_words` readable words, `records` to `record_count` readable [`CRec`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_audcmp_observe(
    address: u32,
    stack: *const u32,
    stack_words: u32,
    result: u32,
    records: *const CRec,
    record_count: u32,
    records_dropped: u32,
) -> u32 {
    // SAFETY: the C glue passes buffers of the stated lengths.
    let stack = if stack.is_null() {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(stack, stack_words as usize) }
    };
    let crecs = if records.is_null() {
        &[][..]
    } else {
        unsafe { std::slice::from_raw_parts(records, record_count as usize) }
    };
    let recs: Vec<Rec> = crecs
        .iter()
        .map(|r| Rec {
            kind: r.kind,
            a: r.a,
            serial: r.serial,
            result: r.result != 0,
            blob: r.blob,
        })
        .collect();
    let mut guard = GLOBAL.lock().unwrap_or_else(|e| e.into_inner());
    let Some(g) = guard.as_mut() else {
        return result;
    };
    if g.mode1.on && g.mode1.fell_back {
        g.mode1.c_after_fallback += 1;
        return result;
    }
    let sum =
        |g: &Global, f: fn(&compare::VaStat) -> u64| g.twin.stats.values().map(f).sum::<u64>();
    let (m0, h0, r0, f0) = (
        g.twin.total_mismatches,
        g.twin.hook_problems,
        sum(g, |s| s.refusals),
        sum(g, |s| s.foreign),
    );
    g.twin
        .observe(address, stack, result, &recs, records_dropped);
    let mut answer = result;
    if g.mode1.on {
        g.mode1.calls += 1;
        let diverged = g.twin.total_mismatches != m0 || g.twin.hook_problems != h0;
        if diverged {
            g.mode1.fell_back = true;
            g.mode1.fallback_call = g.mode1.calls;
            eprintln!(
                "audio-rust: SELF-CHECK FAILED at call #{} ({address:#010x}), the C answers from now on (permanent)",
                g.mode1.calls
            );
        } else if let Some(r) = g.twin.last_answer {
            // The twin compared r with the C result above, so forwarding r is value-identical to the C answer today.
            answer = r;
            g.mode1.answered += 1;
        } else if sum(g, |s| s.refusals) != r0 {
            g.mode1.c_refused += 1;
        } else if sum(g, |s| s.foreign) != f0 {
            g.mode1.c_foreign += 1;
        } else {
            g.mode1.c_unmodelled += 1;
        }
    }
    if g.last_summary.elapsed() >= Duration::from_secs(15) {
        g.last_summary = Instant::now();
        for line in g.twin.summary() {
            eprintln!("{line}");
        }
        mode1_line(g);
    }
    answer
}

fn mode1_line(g: &Global) {
    let m = &g.mode1;
    if m.on {
        eprintln!(
            "audio-rust: mode 1 calls {} rust-answered {} c-fallbacks (unmodelled {} foreign {} refused {}) self-check {} (c after fallback {}, fell back at call {})",
            m.calls,
            m.answered,
            m.c_unmodelled,
            m.c_foreign,
            m.c_refused,
            if m.fell_back { "FAILED" } else { "ok" },
            m.c_after_fallback,
            m.fallback_call
        );
    }
}

/// Print the per-VA table and totals to stderr.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audcmp_summary() {
    let guard = GLOBAL.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(g) = guard.as_ref() {
        for line in g.twin.summary() {
            eprintln!("{line}");
        }
        mode1_line(g);
    }
}

/// 0 mismatches, 1 modelled-call checks total, 2 mode 1 Rust-answered calls, 3 fell back to C (0/1).
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_audcmp_counter(which: u32) -> u64 {
    let guard = GLOBAL.lock().unwrap_or_else(|e| e.into_inner());
    guard.as_ref().map_or(0, |g| match which {
        0 => g.twin.total_mismatches,
        1 => g.twin.stats.values().map(|s| s.checks).sum(),
        2 => g.mode1.answered,
        3 => u64::from(g.mode1.fell_back),
        _ => 0,
    })
}
