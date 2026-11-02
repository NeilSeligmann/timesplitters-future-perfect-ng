// SPDX-License-Identifier: GPL-3.0-or-later
//! C-facing nested-event producer. No host is wired to it by this crate alone.
use crate::{Writer, file::AtomicFile, producer::Producer};
use std::cell::Cell;
use std::ffi::{CStr, c_char};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

#[repr(C)]
pub struct Operation {
    pub id: u32,
    pub inputs: u32,
    pub outputs: u32,
}
static ACTIVE: Mutex<Option<Producer<AtomicFile>>> = Mutex::new(None);
static NEXT_THREAD: AtomicU64 = AtomicU64::new(1);
thread_local! { static THREAD: Cell<u64> = const { Cell::new(0) }; }
fn thread_id() -> u64 {
    THREAD.with(|id| {
        if id.get() == 0 {
            let next = NEXT_THREAD
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
                .unwrap_or(0);
            id.set(next);
        }
        id.get()
    })
}

/// Start after the caller has verified the complete initial registry state.
///
/// # Safety
/// `path` is NUL-terminated UTF-8; identity and ops point to their stated readable extents.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_hostmem_recording_start(
    path: *const c_char,
    identity: *const u8,
    identity_len: usize,
    boot_state_verified: bool,
    ops: *const Operation,
    ops_len: usize,
    limit: u64,
) -> bool {
    if path.is_null()
        || identity.is_null()
        || identity_len == 0
        || identity_len > 4096
        || ops.is_null()
        || ops_len == 0
        || ops_len > 128
        || !boot_state_verified
    {
        return false;
    }
    let Ok(path) = (unsafe { CStr::from_ptr(path) }).to_str() else {
        return false;
    };
    let identity = unsafe { std::slice::from_raw_parts(identity, identity_len) };
    let ops = unsafe { std::slice::from_raw_parts(ops, ops_len) };
    let mut table = Vec::with_capacity(ops.len());
    for op in ops {
        if op.inputs > 16 || op.outputs > 16 || table.iter().any(|(id, _, _)| *id == op.id) {
            return false;
        }
        table.push((op.id, op.inputs as usize, op.outputs as usize));
    }
    let Ok(mut active) = ACTIVE.lock() else {
        return false;
    };
    if let Some(producer) = active.as_mut() {
        producer.incomplete();
        return false;
    }
    let Ok(file) = AtomicFile::create(Path::new(path)) else {
        return false;
    };
    let Ok(writer) = Writer::new(file, limit) else {
        return false;
    };
    let Ok(producer) = Producer::new(writer, identity, boot_state_verified, &table) else {
        return false;
    };
    *active = Some(producer);
    true
}

/// # Safety
/// `args` points to `argc` readable words, or is null when argc is zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_hostmem_recording_begin(
    op: u32,
    args: *const u32,
    argc: usize,
) -> u64 {
    unsafe { begin_checked(None, op, args, argc) }
}

/// Stream position to take immediately before a host-helper mutation; `u64::MAX` when no capture is active.
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hostmem_recording_epoch() -> u64 {
    ACTIVE
        .lock()
        .ok()
        .and_then(|active| active.as_ref().map(Producer::epoch))
        .unwrap_or(u64::MAX)
}

/// Like `tsfp_hostmem_recording_begin` for a mutation already made: fails and poisons the capture unless no
/// event was recorded since `epoch` was taken (the write is then adjacent to its record).
///
/// # Safety
/// `args` points to `argc` readable words, or is null when argc is zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_hostmem_recording_begin_after(
    epoch: u64,
    op: u32,
    args: *const u32,
    argc: usize,
) -> u64 {
    unsafe { begin_checked(Some(epoch), op, args, argc) }
}

unsafe fn begin_checked(epoch: Option<u64>, op: u32, args: *const u32, argc: usize) -> u64 {
    let Ok(mut active) = ACTIVE.lock() else {
        return 0;
    };
    let Some(producer) = active.as_mut() else {
        return 0;
    };
    if argc > 16 || (args.is_null() && argc != 0) {
        producer.stop();
        return 0;
    }
    let args = if argc == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(args, argc) }
    };
    match epoch {
        Some(epoch) => producer.begin_after(thread_id(), epoch, op, args),
        None => producer.begin(thread_id(), op, args),
    }
    .unwrap_or(0)
}

/// # Safety
/// `bytes` points to `len` readable bytes, or is null when len is zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_hostmem_recording_memory(
    write: bool,
    address: u32,
    bytes: *const u8,
    len: usize,
) -> bool {
    let Ok(mut active) = ACTIVE.lock() else {
        return false;
    };
    let Some(producer) = active.as_mut() else {
        return false;
    };
    if len > crate::MAX_RECORD - 16 || (bytes.is_null() && len != 0) {
        producer.stop();
        return false;
    }
    let bytes = if len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(bytes, len) }
    };
    producer.memory(thread_id(), write, address, bytes).is_ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hostmem_recording_placement(
    kind: u32,
    length: u64,
    address: u32,
    result: u32,
) -> bool {
    let Ok(mut active) = ACTIVE.lock() else {
        return false;
    };
    active.as_mut().is_some_and(|p| {
        p.placement(thread_id(), kind, length, address, result)
            .is_ok()
    })
}

/// # Safety
/// `outputs` points to `count` readable words, or is null when count is zero.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_hostmem_recording_end(
    id: u64,
    result: u32,
    outputs: *const u32,
    count: usize,
) -> bool {
    let Ok(mut active) = ACTIVE.lock() else {
        return false;
    };
    let Some(producer) = active.as_mut() else {
        return false;
    };
    if count > 16 || (outputs.is_null() && count != 0) {
        producer.stop();
        return false;
    }
    let outputs = if count == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(outputs, count) }
    };
    producer.end(thread_id(), id, result, outputs).is_ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hostmem_recording_incomplete() {
    debug_first_incomplete();
    if let Ok(mut active) = ACTIVE.lock()
        && let Some(producer) = active.as_mut()
    {
        producer.incomplete();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_hostmem_recording_seal() -> bool {
    let Ok(mut active) = ACTIVE.lock() else {
        return false;
    };
    let Some(producer) = active.take() else {
        return false;
    };
    let result = producer.seal().and_then(AtomicFile::publish);
    if std::env::var_os("TSFP_HOSTMEM_CAPTURE_DEBUG").is_some()
        && let Err(error) = &result
    {
        eprintln!("hostmem-capture: seal failed: {error}");
    }
    result.is_ok()
}

/// Opt-in (TSFP_HOSTMEM_CAPTURE_DEBUG): print the stack of the first incomplete mark so a refusal can be diagnosed.
fn debug_first_incomplete() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static SEEN: AtomicBool = AtomicBool::new(false);
    if std::env::var_os("TSFP_HOSTMEM_CAPTURE_DEBUG").is_some()
        && !SEEN.swap(true, Ordering::Relaxed)
    {
        eprintln!(
            "hostmem-capture: first incomplete mark:
{}",
            std::backtrace::Backtrace::force_capture()
        );
    }
}
