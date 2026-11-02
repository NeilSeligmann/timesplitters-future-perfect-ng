// SPDX-License-Identifier: GPL-3.0-or-later
//! Per-sink WSOLA backend. Native DSP state is owned by Rust; C is an oracle.
use std::ffi::c_void;
use tsfp_audio::stretch::Stretch;

type Ptr = *mut c_void;
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ReferenceOps {
    pub create: Option<unsafe extern "C" fn(u16, usize) -> Ptr>,
    pub destroy: Option<unsafe extern "C" fn(Ptr)>,
    pub overlap: Option<unsafe extern "C" fn(Ptr) -> usize>,
    pub search: Option<unsafe extern "C" fn(Ptr) -> usize>,
    pub needed: Option<unsafe extern "C" fn(Ptr, usize) -> usize>,
    pub primed: Option<unsafe extern "C" fn(Ptr) -> bool>,
    pub prime: Option<unsafe extern "C" fn(Ptr, *const i16, usize, *mut i16) -> bool>,
    pub next:
        Option<unsafe extern "C" fn(Ptr, *const i16, usize, usize, *mut i16, *mut usize) -> bool>,
    pub hold: Option<unsafe extern "C" fn(Ptr, *mut i16) -> bool>,
    pub leave: Option<unsafe extern "C" fn(Ptr, *const i16, usize, *mut i16) -> bool>,
    pub fade: Option<unsafe extern "C" fn(Ptr, *mut i16) -> bool>,
}
impl ReferenceOps {
    fn complete(&self) -> bool {
        self.create.is_some()
            && self.destroy.is_some()
            && self.overlap.is_some()
            && self.search.is_some()
            && self.needed.is_some()
            && self.primed.is_some()
            && self.prime.is_some()
            && self.next.is_some()
            && self.hold.is_some()
            && self.leave.is_some()
            && self.fade.is_some()
    }
}
enum Backend {
    Native(Stretch),
    Reference { handle: Ptr, ops: ReferenceOps },
}

fn span(address: usize, bytes: usize) -> Option<(usize, usize)> {
    Some((address, address.checked_add(bytes)?))
}
fn overlaps(a: (usize, usize), b: (usize, usize)) -> bool {
    a.0 < a.1 && b.0 < b.1 && a.0 < b.1 && b.0 < a.1
}
pub struct Engine {
    backend: Backend,
    channels: usize,
    successful_calls: u64,
    successful_primes: u64,
}

#[repr(C)]
pub struct Activity {
    pub backend: u32,
    pub reserved: u32,
    pub calls: u64,
    pub primes: u64,
}

/// Snapshot after caller has serialized the engine; 0 means no live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_wsola_activity(e: *const Engine) -> Activity {
    let Some(e) = (unsafe { e.as_ref() }) else {
        return Activity {
            backend: 0,
            reserved: 0,
            calls: 0,
            primes: 0,
        };
    };
    Activity {
        backend: match &e.backend {
            Backend::Native(_) => 2,
            Backend::Reference { .. } => 1,
        },
        reserved: 0,
        calls: e.successful_calls,
        primes: e.successful_primes,
    }
}
impl Drop for Engine {
    fn drop(&mut self) {
        if let Backend::Reference { handle, ops } = &self.backend {
            unsafe { (ops.destroy.unwrap())(*handle) }
        }
    }
}
impl Engine {
    fn overlap(&self) -> usize {
        match &self.backend {
            Backend::Native(s) => s.overlap(),
            Backend::Reference { handle, ops } => unsafe { (ops.overlap.unwrap())(*handle) },
        }
    }
    fn search(&self) -> usize {
        match &self.backend {
            Backend::Native(s) => s.search(),
            Backend::Reference { handle, ops } => unsafe { (ops.search.unwrap())(*handle) },
        }
    }
}
/// The caller serializes handles and supplies valid non-overlapping pointers.
/// kind 0 is reference (complete callbacks required); kind 1 is native (callbacks ignored).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_wsola_create(
    kind: u32,
    channels: u16,
    overlap: usize,
    callbacks: *const ReferenceOps,
) -> *mut Engine {
    if channels == 0
        || overlap < 4
        || overlap
            .checked_mul(usize::from(channels))
            .and_then(|n| n.checked_mul(2))
            .filter(|n| *n <= isize::MAX as usize / 2)
            .is_none()
    {
        return std::ptr::null_mut();
    }
    let backend = match kind {
        1 => match Stretch::new(channels, overlap) {
            Some(s) => Backend::Native(s),
            None => return std::ptr::null_mut(),
        },
        0 => {
            let Some(ops) = (unsafe { callbacks.as_ref() }).copied() else {
                return std::ptr::null_mut();
            };
            if !ops.complete() {
                return std::ptr::null_mut();
            }
            let handle = unsafe { (ops.create.unwrap())(channels, overlap) };
            if handle.is_null() {
                return std::ptr::null_mut();
            }
            Backend::Reference { handle, ops }
        }
        _ => return std::ptr::null_mut(),
    };
    // A failed wrapper allocation must release a reference engine and return
    // failure rather than aborting or silently switching the selected backend.
    let engine = Engine {
        backend,
        channels: channels.into(),
        successful_calls: 0,
        successful_primes: 0,
    };
    let raw = unsafe { std::alloc::alloc(std::alloc::Layout::new::<Engine>()) }.cast::<Engine>();
    if raw.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        raw.write(engine);
    }
    raw
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_wsola_destroy(e: *mut Engine) {
    if !e.is_null() {
        drop(unsafe { Box::from_raw(e) });
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_wsola_overlap(e: *const Engine) -> usize {
    unsafe { e.as_ref() }.map_or(0, Engine::overlap)
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_wsola_search(e: *const Engine) -> usize {
    unsafe { e.as_ref() }.map_or(0, Engine::search)
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_wsola_primed(e: *const Engine) -> bool {
    match unsafe { e.as_ref() }.map(|x| &x.backend) {
        Some(Backend::Native(s)) => s.primed(),
        Some(Backend::Reference { handle, ops }) => unsafe { (ops.primed.unwrap())(*handle) },
        None => false,
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_wsola_needed(
    e: *const Engine,
    nominal: usize,
    needed: *mut usize,
) -> bool {
    let Some(e) = (unsafe { e.as_ref() }) else {
        return false;
    };
    if needed.is_null() || !(needed as usize).is_multiple_of(std::mem::align_of::<usize>()) {
        return false;
    }
    let (Some(engine_span), Some(needed_span)) = (
        span(e as *const Engine as usize, std::mem::size_of::<Engine>()),
        span(needed as usize, std::mem::size_of::<usize>()),
    ) else {
        return false;
    };
    if overlaps(engine_span, needed_span) {
        return false;
    }
    let Some(value) = e
        .overlap()
        .checked_mul(2)
        .and_then(|x| x.checked_add(e.search()))
        .and_then(|x| x.checked_add(nominal))
    else {
        return false;
    };
    let result = match &e.backend {
        Backend::Native(_) => value,
        Backend::Reference { handle, ops } => unsafe { (ops.needed.unwrap())(*handle, nominal) },
    };
    unsafe {
        *needed = result;
    }
    true
}
/// Operation 0 prime, 1 next, 2 hold, 3 leave, 4 fade; frames/nominal are frames.
/// Input and output buffers and chosen pointer must be valid, disjoint, serialized.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_wsola_process(
    e: *mut Engine,
    op: u32,
    input: *const i16,
    frames: usize,
    nominal: usize,
    output: *mut i16,
    output_samples: usize,
    chosen: *mut usize,
) -> bool {
    let Some(engine) = (unsafe { e.as_ref() }) else {
        return false;
    };
    let Some(block) = engine
        .overlap()
        .checked_mul(engine.channels)
        .filter(|n| *n <= isize::MAX as usize / 2)
    else {
        return false;
    };
    let Some(input_samples) = frames
        .checked_mul(engine.channels)
        .filter(|n| *n <= isize::MAX as usize / 2)
    else {
        return false;
    };
    if output.is_null()
        || output_samples < block
        || op > 4
        || (matches!(op, 0 | 1 | 3) && input.is_null())
        || !(output as usize).is_multiple_of(std::mem::align_of::<i16>())
        || (matches!(op, 0 | 1 | 3)
            && !(input as usize).is_multiple_of(std::mem::align_of::<i16>()))
        || (op == 1
            && !chosen.is_null()
            && !(chosen as usize).is_multiple_of(std::mem::align_of::<usize>()))
    {
        return false;
    }
    if op == 1
        && engine
            .overlap()
            .checked_mul(2)
            .and_then(|n| nominal.checked_add(n))
            .is_none()
    {
        return false;
    }
    let (Some(engine_span), Some(output_span)) = (
        span(e as usize, std::mem::size_of::<Engine>()),
        span(output as usize, block * std::mem::size_of::<i16>()),
    ) else {
        return false;
    };
    if overlaps(engine_span, output_span) {
        return false;
    }
    let input_span = if matches!(op, 0 | 1 | 3) {
        let Some(s) = span(input as usize, input_samples * std::mem::size_of::<i16>()) else {
            return false;
        };
        if overlaps(s, output_span) || overlaps(s, engine_span) {
            return false;
        }
        Some(s)
    } else {
        None
    };
    if op == 1 && !chosen.is_null() {
        let Some(s) = span(chosen as usize, std::mem::size_of::<usize>()) else {
            return false;
        };
        if overlaps(s, output_span)
            || overlaps(s, engine_span)
            || input_span.is_some_and(|i| overlaps(s, i))
        {
            return false;
        }
    }
    let e = unsafe { &mut *e };
    let success = match &mut e.backend {
        Backend::Native(s) => {
            let out = unsafe { std::slice::from_raw_parts_mut(output, block) };
            let data = if matches!(op, 0 | 1 | 3) {
                unsafe { std::slice::from_raw_parts(input, input_samples) }
            } else {
                &[]
            };
            match op {
                0 => s.prime(data, out),
                1 => match s.next(data, nominal, out) {
                    Some(n) => {
                        if !chosen.is_null() {
                            unsafe {
                                *chosen = n;
                            }
                        }
                        true
                    }
                    None => false,
                },
                2 => s.hold(out),
                3 => s.leave(data, out),
                4 => s.fade_out(out),
                _ => false,
            }
        }
        Backend::Reference { handle, ops } => unsafe {
            match op {
                0 => (ops.prime.unwrap())(*handle, input, frames, output),
                1 => (ops.next.unwrap())(*handle, input, frames, nominal, output, chosen),
                2 => (ops.hold.unwrap())(*handle, output),
                3 => (ops.leave.unwrap())(*handle, input, frames, output),
                4 => (ops.fade.unwrap())(*handle, output),
                _ => false,
            }
        },
    };
    if success {
        e.successful_calls = e.successful_calls.saturating_add(1);
        if op == 0 {
            e.successful_primes = e.successful_primes.saturating_add(1);
        }
    }
    success
}
