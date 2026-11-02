// SPDX-License-Identifier: GPL-3.0-or-later
//! The measured DirectSound entry table (`dsound_entry` of `dsound_hle.h`) and its registration and counting API.
use std::cell::UnsafeCell;
use std::env;
use std::ffi::{CStr, c_char, c_void};
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::ffi::{self, Handler};
use crate::readiness;
use crate::{lock, log};

pub const FUNCTION_COUNT: usize = 41;
pub const SITE_COUNT: u32 = 83;
pub const SECTION_VA_BEGIN: u32 = 0x0040_67C0;
pub const SECTION_VA_END: u32 = 0x0041_2D14;

pub const ENTRY_STUB: u32 = 1;
pub const ENTRY_IMPLEMENTED: u32 = 2;

/// `dsound_entry`.
#[repr(C)]
pub struct Entry {
    pub address: u32,
    pub name: *const c_char,
    pub sites: u32,
    pub handler: Option<Handler>,
    pub state: u32,
    pub default_return: u32,
    pub call_count: u64,
    pub reported: bool,
}

/// `dsound_surface_ref`.
#[repr(C)]
pub struct SurfaceRef {
    pub address: u32,
    pub name: *const c_char,
    pub sites: u32,
}

const ZERO: Entry = Entry {
    address: 0,
    name: ptr::null(),
    sites: 0,
    handler: None,
    state: 0,
    default_return: 0,
    call_count: 0,
    reported: false,
};

struct Table(UnsafeCell<[Entry; FUNCTION_COUNT]>);
// SAFETY: writes happen only under `GUARD`; readers tolerate a concurrent registration like the C table did.
unsafe impl Sync for Table {}

static TABLE: Table = Table(UnsafeCell::new([ZERO; FUNCTION_COUNT]));
static GUARD: Mutex<bool> = Mutex::new(false);

fn entries() -> &'static mut [Entry; FUNCTION_COUNT] {
    // SAFETY: see `Table`.
    unsafe { &mut *TABLE.0.get() }
}

pub(crate) fn label(entry: &Entry) -> String {
    if entry.name.is_null() {
        return "<unnamed in .XTLID>".to_owned();
    }
    // SAFETY: names are static NUL-terminated strings of the measured surface or caller-owned rows.
    unsafe { CStr::from_ptr(entry.name) }
        .to_string_lossy()
        .into_owned()
}

pub(crate) fn calls(entry: &Entry) -> u64 {
    // SAFETY: Rust-owned call counter of the audio host.
    unsafe { ffi::tsfp_audio_host_hle_call_count(entry.address) }
}

fn init_locked(initialised: &mut bool) {
    // SAFETY: Rust-owned measured surface of the audio host.
    unsafe {
        if ffi::tsfp_audio_hle_surface_count() != FUNCTION_COUNT
            || !ffi::tsfp_audio_hle_surface_init()
        {
            log("dsound: Rust measured-surface initialization failed; refusing audio dispatch\n");
            std::process::abort();
        }
        ffi::tsfp_audio_hle_registry_reset_rust();
    }
    for (index, entry) in entries().iter_mut().enumerate() {
        *entry = ZERO;
        // SAFETY: valid out pointers into the entry just reset.
        let ok = unsafe {
            ffi::tsfp_audio_hle_surface_row(
                index,
                &mut entry.address,
                &mut entry.name,
                &mut entry.sites,
            )
        };
        if !ok {
            log(&format!(
                "dsound: Rust measured-surface row {index} unavailable; refusing audio dispatch\n"
            ));
            std::process::abort();
        }
        entry.state = ENTRY_STUB;
    }
    *initialised = true;
    readiness::reset();
}

/// `dsound_hle_init`: fresh run state.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_init() {
    init_locked(&mut lock(&GUARD));
}

pub(crate) fn ensure_initialised() {
    let mut initialised = lock(&GUARD);
    if !*initialised {
        init_locked(&mut initialised);
    }
}

/// Exact-match lookup, linear on purpose (a neighbour five bytes away must never answer).
fn find(address: u32) -> Option<&'static mut Entry> {
    ensure_initialised();
    entries().iter_mut().find(|entry| entry.address == address)
}

pub(crate) fn snapshot() -> &'static [Entry; FUNCTION_COUNT] {
    ensure_initialised();
    entries()
}

/// `dsound_hle_register`: the Rust registry selects the callback from its exact-address table.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_hle_register(address: u32, handler: Option<Handler>) -> bool {
    let (Some(entry), Some(handler)) = (find(address), handler) else {
        return false;
    };
    // SAFETY: Rust-owned registry.
    if !unsafe { ffi::tsfp_audio_hle_register_handler_rust(address, handler) } {
        return false;
    }
    entry.handler = None;
    entry.state = ENTRY_IMPLEMENTED;
    true
}

/// `dsound_hle_register_rust`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_register_rust(address: u32) -> bool {
    let Some(entry) = find(address) else {
        return false;
    };
    // SAFETY: Rust-owned registry.
    if !unsafe { ffi::tsfp_audio_hle_mark_implemented_rust(address) } {
        return false;
    }
    entry.handler = None;
    entry.state = ENTRY_IMPLEMENTED;
    true
}

/// `dsound_hle_set_default_return`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_set_default_return(address: u32, value: u32) -> bool {
    let Some(entry) = find(address) else {
        return false;
    };
    // SAFETY: Rust-owned registry.
    if !unsafe { ffi::tsfp_audio_hle_set_default_return_rust(address, value) } {
        return false;
    }
    entry.default_return = value;
    true
}

/// `dsound_hle_entry`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_entry(address: u32) -> *const Entry {
    find(address).map_or(ptr::null(), |entry| entry as *const Entry)
}

/// `dsound_hle_table`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_hle_table(count: *mut usize) -> *const Entry {
    ensure_initialised();
    if !count.is_null() {
        // SAFETY: caller-provided out pointer.
        unsafe { *count = FUNCTION_COUNT };
    }
    TABLE.0.get().cast()
}

/// `dsound_hle_call`: lookup, counts and report-once flags are Rust-owned by the audio host dispatcher.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_hle_call(address: u32, context: *mut c_void) -> u32 {
    // SAFETY: forwarded unchanged.
    unsafe { ffi::tsfp_audio_host_hle_call(address, context) }
}

/// `dsound_hle_set_observer`: the Rust host has no compare observer in its live path.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_set_observer(_observer: *const c_void) {}

/// `dsound_hle_implemented_count`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_implemented_count() -> usize {
    snapshot()
        .iter()
        .filter(|entry| entry.state == ENTRY_IMPLEMENTED)
        .count()
}

/// `dsound_hle_touched_count`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_touched_count() -> usize {
    snapshot().iter().filter(|entry| calls(entry) > 0).count()
}

/// `dsound_hle_unknown_call_count`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_unknown_call_count() -> u64 {
    ensure_initialised();
    // SAFETY: Rust-owned counter.
    unsafe { ffi::tsfp_audio_hle_surface_unknown_calls() }
}

static DEGRADED: AtomicU64 = AtomicU64::new(0);

extern "C" fn degraded_summary() {
    let count = DEGRADED.load(Ordering::Relaxed);
    if count > 0 {
        log(&format!(
            "dsound: {count} stream/listener/mixbin/effects calls ran as degraded S_OK no-ops (unmodelled state, T2173)\n"
        ));
    }
}

/// `dsound_hle_degrade`: false under `TSFP_DSOUND_STRICT`, otherwise the call runs as a logged S_OK no-op.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_hle_degrade(
    kind: *const c_char,
    entry: u32,
    reason: *const c_char,
) -> bool {
    if env::var_os("TSFP_DSOUND_STRICT").is_some() {
        return false;
    }
    let count = DEGRADED.fetch_add(1, Ordering::Relaxed) + 1;
    if count == 1 {
        // SAFETY: a plain extern "C" callback.
        unsafe { ffi::atexit(degraded_summary) };
    }
    if count <= 20 || count.is_power_of_two() {
        let text = |value: *const c_char| {
            if value.is_null() {
                String::new()
            } else {
                // SAFETY: NUL-terminated strings from the caller.
                unsafe { CStr::from_ptr(value) }
                    .to_string_lossy()
                    .into_owned()
            }
        };
        log(&format!(
            "dsound passive {} {entry:#x} DEGRADED (#{count}, S_OK no-op): {}\n",
            text(kind),
            text(reason)
        ));
    }
    true
}

/// `dsound_hle_degraded_count`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_degraded_count() -> u64 {
    DEGRADED.load(Ordering::Relaxed)
}

/// `dsound_hle_crosscheck`: both directions against a generated surface table.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_hle_crosscheck(refs: *const SurfaceRef, count: usize) -> u32 {
    ensure_initialised();
    if refs.is_null() && count > 0 {
        log(&format!(
            "dsound: crosscheck given {count} rows and a NULL pointer\n"
        ));
        return 1;
    }
    let rows: &[SurfaceRef] = if count == 0 {
        &[]
    } else {
        // SAFETY: `count` rows at a non-null pointer.
        unsafe { std::slice::from_raw_parts(refs, count) }
    };
    let name = |pointer: *const c_char| {
        // SAFETY: NUL-terminated or null.
        (!pointer.is_null()).then(|| {
            unsafe { CStr::from_ptr(pointer) }
                .to_string_lossy()
                .into_owned()
        })
    };
    let mut disagreements = 0;
    for row in rows {
        let row_name = name(row.name);
        let shown = row_name.clone().unwrap_or_else(|| "unnamed".to_owned());
        let Some(mine) = find(row.address) else {
            disagreements += 1;
            log(&format!(
                "dsound: crosscheck -- generated table has {:#010x} ({shown}, {} sites) and we do not\n",
                row.address, row.sites
            ));
            continue;
        };
        if mine.sites != row.sites {
            disagreements += 1;
            log(&format!(
                "dsound: crosscheck -- {:#010x} site count {} here, {} in the generated table\n",
                row.address, mine.sites, row.sites
            ));
        }
        let mine_name = name(mine.name);
        if mine_name != row_name {
            disagreements += 1;
            log(&format!(
                "dsound: crosscheck -- {:#010x} named \"{}\" here, \"{}\" in the generated table\n",
                row.address,
                mine_name.as_deref().unwrap_or("(null)"),
                row_name.as_deref().unwrap_or("(null)")
            ));
        }
    }
    for entry in entries().iter() {
        if !rows.iter().any(|row| row.address == entry.address) {
            disagreements += 1;
            log(&format!(
                "dsound: crosscheck -- we have {:#010x} ({}) and the generated table does not\n",
                entry.address,
                label(entry)
            ));
        }
    }
    disagreements
}

/// `dsound_hle_requires_implementation`: the explicit measured stream, buffer and listener startup surface.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_hle_requires_implementation(address: u32) -> bool {
    matches!(
        address,
        0x0040_7B14
            | 0x0040_7B19
            | 0x0040_7B1E
            | 0x0040_7B23
            | 0x0040_7B28
            | 0x0040_85CF
            | 0x0040_85D4
            | 0x0040_85D9
            | 0x0040_85F1
            | 0x0040_8609
            | 0x0040_8632
            | 0x0040_8637
            | 0x0040_8C2D
            | 0x0040_967C
            | 0x0040_7ABC
            | 0x0040_7AA4
            | 0x0040_7AF8
            | 0x0040_8556
            | 0x0040_7A64
            | 0x0040_7AD8
            | 0x0040_8532
            | 0x0040_858B
            | 0x0040_7A80
            | 0x0040_84F2
            | 0x0040_850E
            | 0x0040_85AF
            | 0x0040_8C0D
            | 0x0040_93C8
            | 0x0040_93EC
            | 0x0040_945A
            | 0x0040_9410
    )
}
