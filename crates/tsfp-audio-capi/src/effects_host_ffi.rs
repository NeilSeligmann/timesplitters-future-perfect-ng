// SPDX-License-Identifier: GPL-3.0-or-later
//! T3959: the DirectSound effects binding of the Rust audio host (`IDirectSound_DownloadEffectsImage` and
//! `IDirectSound_SetEffectData`), replacing `crates/tsfp-audio-conformance/oracle/dsound_effects_binding.c`. Passive CPU views of the measured
//! image are always available; with the GP policy on, the genuine firmware runs on the Rust DSP core
//! (`tsfp-audio-gpdsp`, dsp56300-emu) behind the same owned device. Guest memory, descriptor construction and
//! allocation are the Rust models of `effects_*_ffi`; this module is the state, the policy gates and the HLE handlers.

use std::ffi::{CString, c_char, c_void};
use std::sync::Mutex;

use tsfp_audio::effects_metadata::Metadata;
use tsfp_audio_gpdsp::gp::{FRAME_WORDS, GpDsp};
use tsfp_audio_gpdsp::prepare;

use crate::effects_allocation_ffi::{
    EffectsAllocation, GuestMemoryOps, tsfp_effects_binding_allocate_rust,
    tsfp_effects_binding_destroy_rust, tsfp_effects_binding_intact_rust,
};
use crate::effects_binding_ffi::tsfp_effects_binding_build_descriptor_rust;
use crate::effects_guest_copy_ffi::{
    tsfp_effects_guest_copy_read_rust, tsfp_effects_guest_copy_write_rust,
};
use crate::effects_metadata_ffi::dsound_effects_snapshot_known_guest_rust;
use crate::effects_publish_ffi::tsfp_effects_binding_publish_rust;
use crate::movie_device_hle_ffi::tsfp_audio_hle_register_handler_rust;

const ENTRY: u32 = 0x0040_79DB;
const APPLY_ENTRY: u32 = 0x0040_7A02;
const CALLER: u32 = 0x0002_70AF;
const APPLY_CALLER: u32 = 0x0038_669F;
const OOM: u32 = 0x8007_000E;
const IMAGE_ADDRESS: u32 = 0x007F_78C0;
const IMAGE_BYTES: u32 = 18_608;
const STATE_BYTES: usize = 8088;
const CODE_BYTES: usize = 8080;
const DESCRIPTOR_BYTES: usize = 296;
const GP_FIRMWARE_ADDRESS: u32 = 0x0041_24D0;
const GP_FIRMWARE_BYTES: usize = 0x5CC;
const BLOCKED_GLOBAL: u32 = 0x0041_24A8;
const DEVICE_GLOBAL: u32 = 0x0041_2B30;
const DEVICE_VTABLE: u32 = 0x004A_1CB0;

type FatalFn = unsafe extern "C" fn(u32, *const c_char);
type Handler = unsafe extern "C" fn(*mut c_void) -> u32;
type OwnedCallback = unsafe extern "C" fn(*const u32, *mut c_void, *mut u32);

unsafe extern "C" {
    fn dsound_hle_register_rust(address: u32) -> bool;
    fn dsound_hle_log_line(line: *const c_char);
    fn dsound_hle_degrade(kind: *const c_char, entry: u32, reason: *const c_char) -> bool;
    fn kernel_guest_read_u32(address: u32, out: *mut u32) -> bool;
    fn kernel_frame_arg(frame: *const c_void, index: u32, out: *mut u32) -> bool;
    fn tsfp_audio_device_with_owned_identity(
        interface: u32,
        callback: Option<OwnedCallback>,
        userdata: *mut c_void,
        result: *mut u32,
    ) -> bool;
    #[cfg(feature = "rust-kmem")]
    fn tsfp_kmem_audio_heap_active() -> bool;
    #[cfg(feature = "rust-kmem")]
    fn tsfp_kmem_audio_heap_create(flags: u32, initial: u32, maximum: u32) -> u32;
    #[cfg(feature = "rust-kmem")]
    fn tsfp_kmem_audio_heap_alloc(heap: u32, bytes: u32) -> u32;
    #[cfg(feature = "rust-kmem")]
    fn tsfp_kmem_audio_heap_valid(heap: u32) -> bool;
    #[cfg(feature = "rust-kmem")]
    fn tsfp_kmem_audio_heap_block_size(heap: u32, address: u32, size_out: *mut u32) -> bool;
    #[cfg(feature = "rust-kmem")]
    fn tsfp_kmem_audio_heap_destroy(heap: u32) -> bool;
    fn kmem_heap_create(flags: u32, initial: u32, maximum: u32) -> u32;
    fn kmem_heap_alloc(heap: u32, bytes: u32) -> u32;
    fn kmem_heap_valid(heap: u32) -> bool;
    fn kmem_heap_block_size(heap: u32, address: u32, size_out: *mut u32) -> bool;
    fn kmem_heap_destroy(heap: u32) -> bool;
}

/// The guest heap the effects views live in: Rust kmem when active, the kernel memory ABI otherwise.
fn memory_ops() -> GuestMemoryOps {
    #[cfg(feature = "rust-kmem")]
    // SAFETY: scalar selector.
    if unsafe { tsfp_kmem_audio_heap_active() } {
        return GuestMemoryOps {
            heap_create: Some(tsfp_kmem_audio_heap_create),
            heap_alloc: Some(tsfp_kmem_audio_heap_alloc),
            heap_valid: Some(tsfp_kmem_audio_heap_valid),
            heap_block_size: Some(tsfp_kmem_audio_heap_block_size),
            heap_destroy: Some(tsfp_kmem_audio_heap_destroy),
        };
    }
    GuestMemoryOps {
        heap_create: Some(kmem_heap_create),
        heap_alloc: Some(kmem_heap_alloc),
        heap_valid: Some(kmem_heap_valid),
        heap_block_size: Some(kmem_heap_block_size),
        heap_destroy: Some(kmem_heap_destroy),
    }
}

fn pid() -> i32 {
    i32::try_from(std::process::id()).unwrap_or(0)
}

fn copy_read(address: u32, buffer: &mut [u8]) -> bool {
    // SAFETY: `buffer` is a live writable span; the helper checks the guest span.
    unsafe { tsfp_effects_guest_copy_read_rust(pid(), address, buffer.as_mut_ptr(), buffer.len()) }
}

fn copy_write(address: u32, buffer: &[u8]) -> bool {
    // SAFETY: `buffer` is a live readable span; the helper checks the guest span.
    unsafe { tsfp_effects_guest_copy_write_rust(pid(), buffer.as_ptr(), address, buffer.len()) }
}

fn log_line(line: &str) {
    if let Ok(line) = CString::new(line) {
        // SAFETY: a live NUL-terminated string for this synchronous call.
        unsafe { dsound_hle_log_line(line.as_ptr()) };
    }
}

fn degrade(entry: u32, reason: &str) -> bool {
    let (Ok(kind), Ok(reason)) = (CString::new("effects"), CString::new(reason)) else {
        return false;
    };
    // SAFETY: live NUL-terminated strings for this synchronous call.
    unsafe { dsound_hle_degrade(kind.as_ptr(), entry, reason.as_ptr()) }
}

#[derive(Default)]
struct Binding {
    enabled: bool,
    announced: bool,
    gp_enabled: bool,
    heap: u32,
    device: u32,
    device_heap: u32,
    descriptor: u32,
    code: u32,
    state: u32,
    workspace: u32,
    y: u32,
    staging: u32,
    gp: Option<GpDsp>,
    gp_metadata: Option<Metadata>,
    gp_mirror: Vec<u8>,
    immutable_descriptor: Vec<u8>,
    immutable_code: Vec<u8>,
}

// SAFETY: the GP DSP bus is only ever touched while holding the BINDING mutex.
unsafe impl Send for Binding {}

static BINDING: Mutex<Option<Binding>> = Mutex::new(None);
static FATAL: Mutex<Option<FatalFn>> = Mutex::new(None);

fn with_binding<T>(action: impl FnOnce(&mut Binding) -> T) -> T {
    let mut guard = BINDING.lock().unwrap();
    action(guard.get_or_insert_with(Binding::default))
}

fn overlaps(a: u32, n: u32, b: u32, m: u32) -> bool {
    u64::from(a) + u64::from(n) > u64::from(b) && u64::from(b) + u64::from(m) > u64::from(a)
}

fn refuse_at(entry: u32, message: &str) -> ! {
    log_line(&format!("dsound effects: {entry:#x} refused: {message}\n"));
    let fatal = *FATAL.lock().unwrap();
    if let (Some(fatal), Ok(message)) = (fatal, CString::new(message)) {
        // SAFETY: the host fatal hook takes the entry and a NUL-terminated reason.
        unsafe { fatal(entry, message.as_ptr()) };
    }
    std::process::abort()
}

/// `dsound_effects_binding_set_enabled`: the headless-effects policy.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_effects_binding_set_enabled(value: bool) {
    with_binding(|b| b.enabled = value);
}

/// `dsound_effects_binding_set_gp_enabled`: configure quiescent; false preserves the policy when a binding is live.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_effects_binding_set_gp_enabled(value: bool) -> bool {
    with_binding(|b| {
        let ok = b.heap == 0;
        if ok {
            b.gp_enabled = value;
        }
        ok
    })
}

/// `dsound_effects_binding_set_fatal`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_effects_binding_set_fatal(handler: Option<FatalFn>) {
    *FATAL.lock().unwrap() = handler;
}

/// `dsound_effects_binding_reset`: quiescent reset before the device reset. The enabled policy survives.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_effects_binding_reset() {
    with_binding(|b| {
        b.gp = None;
        b.gp_metadata = None;
        b.gp_mirror.clear();
        if b.heap != 0 {
            let ops = memory_ops();
            // SAFETY: `ops` is a live callback table.
            let _ = unsafe { tsfp_effects_binding_destroy_rust(b.heap, &ops) };
        }
        b.heap = 0;
        b.device = 0;
        b.device_heap = 0;
        b.descriptor = 0;
        b.code = 0;
        b.state = 0;
        b.workspace = 0;
        b.y = 0;
        b.staging = 0;
        b.announced = false;
    });
}

fn addresses(b: &Binding) -> [u32; 6] {
    [b.descriptor, b.code, b.state, b.workspace, b.y, b.staging]
}

fn intact(b: &Binding) -> bool {
    let ops = memory_ops();
    let addresses = addresses(b);
    if b.immutable_descriptor.len() != DESCRIPTOR_BYTES || b.immutable_code.len() != CODE_BYTES {
        return false;
    }
    // SAFETY: the table, address array and expected views are live with their contract extents.
    unsafe {
        tsfp_effects_binding_intact_rust(
            pid(),
            b.heap,
            b.gp_enabled,
            &ops,
            addresses.as_ptr(),
            b.immutable_descriptor.as_ptr(),
            b.immutable_code.as_ptr(),
        )
    }
}

struct BindRequest {
    image: u32,
    bytes: u32,
    location: u32,
    output: u32,
    error: Option<&'static str>,
    announce: bool,
}

fn bind_locked(
    b: &mut Binding,
    internal: u32,
    device_heap: u32,
    r: &mut BindRequest,
) -> Option<u32> {
    if !b.enabled {
        r.error = Some("explicit --headless-effects policy is disabled");
        return None;
    }
    if b.heap != 0 {
        r.error = Some(
            if b.device != internal || b.device_heap != device_heap || !intact(b) {
                "binding ownership or immutable views changed"
            } else {
                "repeat binding is not recovered"
            },
        );
        return None;
    }
    let mut location_words = [0u8; 8];
    let mut out_before = [0u8; 4];
    if !copy_read(r.location, &mut location_words)
        || u32::from_le_bytes(location_words[..4].try_into().unwrap()) != 3
        || u32::from_le_bytes(location_words[4..].try_into().unwrap()) != 4
        || !copy_read(r.output, &mut out_before)
    {
        r.error = Some("expected readable location {3,4} and output DWORD");
        return None;
    }
    if overlaps(r.output, 4, r.location, 8)
        || overlaps(r.output, 4, r.image, r.bytes)
        || overlaps(r.output, 4, internal, 44)
        || overlaps(r.location, 8, internal, 44)
        || overlaps(r.location, 8, r.image, r.bytes)
        || overlaps(r.output, 4, DEVICE_GLOBAL, 4)
        || overlaps(r.output, 4, DEVICE_VTABLE, 12)
    {
        r.error = Some("argument alias with input/device state is not recovered");
        return None;
    }
    let mut image = vec![0u8; IMAGE_BYTES as usize];
    let mut metadata = std::mem::MaybeUninit::<Metadata>::uninit();
    // SAFETY: the buffers have the exact fixed sizes; the metadata slot is written on success only.
    let known = r.image == IMAGE_ADDRESS
        && r.bytes == IMAGE_BYTES
        && unsafe {
            dsound_effects_snapshot_known_guest_rust(
                pid(),
                r.image,
                r.bytes as usize,
                image.as_mut_ptr(),
                metadata.as_mut_ptr(),
            )
        };
    if !known {
        r.error = Some("image identity/layout is not recovered");
        return None;
    }
    // SAFETY: initialised by the successful snapshot above.
    let metadata = unsafe { metadata.assume_init() };
    // Verify write permission before allocation; writing identical bytes preserves the output value.
    if !copy_write(r.output, &out_before) {
        r.error = Some("output is not writable");
        return None;
    }
    let mut prepared = None;
    if b.gp_enabled {
        let mut firmware = vec![0u8; GP_FIRMWARE_BYTES];
        if !copy_read(GP_FIRMWARE_ADDRESS, &mut firmware) {
            r.error = Some("GP bootstrap source unreadable");
            return None;
        }
        prepared = prepare(&firmware, &image).and_then(|p| GpDsp::new(p, false).ok());
        if prepared.is_none() {
            r.error = Some("GP attestation/decrypt/real firmware bootstrap failed");
            return None;
        }
    }
    let ops = memory_ops();
    let mut allocation = EffectsAllocation::default();
    // SAFETY: `ops` and `allocation` are live.
    let status = unsafe { tsfp_effects_binding_allocate_rust(b.gp_enabled, &ops, &mut allocation) };
    if status == OOM {
        return Some(OOM);
    }
    if status != 0 {
        r.error = Some("Rust refused effects guest allocation");
        return None;
    }
    let new_heap = allocation.heap;
    let addresses = allocation.addresses;
    let destroy = || {
        // SAFETY: `ops` is a live callback table.
        let _ = unsafe { tsfp_effects_binding_destroy_rust(new_heap, &ops) };
    };
    let mut descriptor = [0u8; DESCRIPTOR_BYTES];
    // SAFETY: metadata and the output span are live with their contract sizes.
    let built = unsafe {
        tsfp_effects_binding_build_descriptor_rust(
            &metadata,
            addresses[1],
            addresses[2],
            addresses[3],
            addresses[4],
            descriptor.as_mut_ptr(),
            descriptor.len(),
        )
    };
    if !built {
        destroy();
        r.error = Some("Rust refused effects descriptor construction");
        return None;
    }
    let code_at = metadata.code_offset as usize;
    let state_at = metadata.state_offset as usize;
    // SAFETY: the sources are live spans inside the validated image; the addresses are the six fresh views.
    let published = unsafe {
        tsfp_effects_binding_publish_rust(
            pid(),
            descriptor.as_ptr(),
            image[code_at..].as_ptr(),
            image[state_at..].as_ptr(),
            addresses.as_ptr(),
            r.output,
        )
    };
    if !published {
        destroy();
        r.error = Some("Rust refused effects descriptor/view publication");
        return None;
    }
    b.heap = new_heap;
    b.device = internal;
    b.device_heap = device_heap;
    b.staging = addresses[5];
    b.descriptor = addresses[0];
    b.code = addresses[1];
    b.state = addresses[2];
    b.workspace = addresses[3];
    b.y = addresses[4];
    b.immutable_descriptor = descriptor.to_vec();
    b.immutable_code = image[code_at..code_at + CODE_BYTES].to_vec();
    b.gp = prepared;
    b.gp_metadata = Some(metadata);
    if b.gp_enabled {
        b.gp_mirror = image[state_at..state_at + STATE_BYTES].to_vec();
    }
    r.announce = !b.announced;
    b.announced = true;
    Some(0)
}

unsafe extern "C" fn bind_owned(words: *const u32, userdata: *mut c_void, result: *mut u32) {
    // SAFETY: the device calls back with (heap, address) and the request/result slots the caller passed.
    let (device_heap, internal, r, result) = unsafe {
        (
            *words,
            *words.add(1),
            &mut *userdata.cast::<BindRequest>(),
            &mut *result,
        )
    };
    if let Some(value) = with_binding(|b| bind_locked(b, internal, device_heap, r)) {
        *result = value;
    }
}

/// `dsound_effects_binding_bind`.
pub fn bind(interface: u32, image: u32, bytes: u32, location: u32, output: u32) -> u32 {
    let mut request = BindRequest {
        image,
        bytes,
        location,
        output,
        error: None,
        announce: false,
    };
    let mut result = 0u32;
    // SAFETY: the callback runs synchronously under the device lock with these live slots.
    let owned = unsafe {
        tsfp_audio_device_with_owned_identity(
            interface,
            Some(bind_owned),
            (&raw mut request).cast(),
            &mut result,
        )
    };
    if !owned {
        // MEASURED default-boot state: the title never tests DirectSoundCreate's HRESULT (lifted 0x000279D5 uses no
        // eax), so a NOT_READY codec leaves the interface global 0x581988 zero and this call arrives with a NULL
        // interface. The ORIGINAL maps it to a NULL device (neg/sbb/and at 0x004079F2..F6) and faults reading
        // [device+8] at 0x00406BCA. This loud stop is that fault's honest stand-in.
        refuse_at(
            ENTRY,
            if interface == 0 {
                "NULL interface: DirectSoundCreate's NOT_READY HRESULT was ignored at 0x000279D5; the original maps \
                 NULL interface to NULL device (0x004079F2..F6) and faults reading [device+8] at 0x00406BCA; the \
                 default boot models the always-present console codec (--no-ac97-ready opts out)"
            } else {
                "SILENT device ownership/header/generation is invalid"
            },
        );
    }
    if let Some(error) = request.error {
        if degrade(ENTRY, error) {
            return 0;
        }
        refuse_at(ENTRY, error);
    }
    let gp_enabled = with_binding(|b| b.gp_enabled);
    if request.announce && gp_enabled {
        log_line(
            "dsound: INFERRED opt-in owned CPU-view/GP bridge; attested firmware command3 consumed, no fabricated ACK; \
             no APU/host PCM integration\n",
        );
    }
    if request.announce && !gp_enabled {
        log_line(
            "dsound: explicit HEADLESS effects binding; passive CPU descriptor/plaintext initial state, opaque \
             encrypted code, zero workspace/Y; no DSP execution, acknowledgement or nested hardware objects\n",
        );
    }
    result
}

#[derive(Default)]
struct GpRequest {
    index: u32,
    offset: u32,
    source: u32,
    bytes: u32,
    flags: u32,
    error: Option<String>,
    valid: bool,
}

/// Which GP consumer action runs under the owned device.
enum GpAction<'a> {
    Apply,
    Commit,
    Frame(&'a [u32; FRAME_WORDS], &'a mut [u32; FRAME_WORDS]),
    View(&'a mut GpView),
}

/// `dsound_effects_binding_gp_view`: observation of the bound GP device (C layout).
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct GpView {
    heap: u32,
    device_heap: u32,
    internal: u32,
    descriptor: u32,
    state: u32,
    staging: u32,
    pending_start: u32,
    pending_bytes: u32,
}

fn sync_live(b: &mut Binding, r: &mut GpRequest) -> bool {
    let mut actual = vec![0u8; STATE_BYTES];
    if !copy_read(b.state, &mut actual) {
        r.error = Some("GP CPU state view unreadable".into());
        return false;
    }
    let (Some(metadata), Some(gp)) = (b.gp_metadata, b.gp.as_mut()) else {
        return false;
    };
    for map in 0..metadata.map_count as usize {
        let m = &metadata.maps[map];
        let base = (m.state_offset - metadata.state_offset) as usize;
        for off in (0..m.state_bytes as usize).step_by(4) {
            if actual[base + off..base + off + 4] == b.gp_mirror[base + off..base + off + 4] {
                continue;
            }
            if gp
                .live_aligned(map, off, &actual[base + off..base + off + 4])
                .is_err()
            {
                r.error =
                    Some("GP direct CPU-view write is outside aligned24-bit MMIO domain".into());
                return false;
            }
            b.gp_mirror[base + off..base + off + 4]
                .copy_from_slice(&actual[base + off..base + off + 4]);
        }
    }
    true
}

fn publish_live(b: &mut Binding, r: &mut GpRequest) -> bool {
    let mut actual = b.gp_mirror.clone();
    let (Some(metadata), Some(gp)) = (b.gp_metadata, b.gp.as_ref()) else {
        return false;
    };
    for map in 0..metadata.map_count as usize {
        let m = &metadata.maps[map];
        let base = (m.state_offset - metadata.state_offset) as usize;
        if gp
            .read(map, 0, &mut actual[base..base + m.state_bytes as usize])
            .is_err()
        {
            r.error = Some("GP live state read failed".into());
            return false;
        }
    }
    if !copy_write(b.state, &actual) {
        r.error = Some("GP live state mirror unwritable".into());
        return false;
    }
    b.gp_mirror = actual;
    true
}

fn gp_forward_copy(b: &mut Binding, r: &mut GpRequest, live: bool) -> bool {
    let Some(metadata) = b.gp_metadata else {
        return false;
    };
    let m = metadata.maps[r.index as usize];
    let base = m.state_offset - metadata.state_offset;
    let whole = r.bytes - (r.bytes & 3);
    let mut done = 0u32;
    while done < r.bytes {
        let width = if done < whole { 4 } else { 1 };
        let mut value = [0u8; 4];
        if !copy_read(r.source + done, &mut value[..width as usize]) {
            r.error = Some("GP source read fault after original copy prefix".into());
            return false;
        }
        let value = &value[..width as usize];
        let target = (if live { b.state } else { b.staging }) + base + r.offset + done;
        let Some(gp) = b.gp.as_mut() else {
            return false;
        };
        if live {
            if gp
                .live_aligned(r.index as usize, (r.offset + done) as usize, value)
                .is_err()
            {
                r.error = Some("GP immediate data outside aligned24-bit MMIO domain".into());
                return false;
            }
            if !copy_write(target, value) {
                r.error = Some("GP CPU live mirror write fault".into());
                return false;
            }
            let at = (base + r.offset + done) as usize;
            b.gp_mirror[at..at + value.len()].copy_from_slice(value);
        } else {
            if !copy_write(target, value) {
                r.error = Some("GP staging write fault after original copy prefix".into());
                return false;
            }
            if gp
                .stage(r.index as usize, (r.offset + done) as usize, value)
                .is_err()
            {
                r.error = Some("GP real staging range unavailable".into());
                return false;
            }
        }
        done += width;
    }
    true
}

fn gp_locked(
    b: &mut Binding,
    internal: u32,
    device_heap: u32,
    action: &mut GpAction,
    r: &mut GpRequest,
) -> Option<u32> {
    if !b.enabled || !b.gp_enabled || b.gp.is_none() {
        r.error = Some("opt-in genuine GP binding is not active".into());
        return None;
    }
    if b.device != internal || b.device_heap != device_heap || !intact(b) {
        r.error = Some("GP device incarnation or binding ownership changed".into());
        return None;
    }
    match action {
        GpAction::View(out) => {
            let (pending_start, pending_bytes) = b.gp.as_ref()?.pending();
            **out = GpView {
                heap: b.heap,
                device_heap: b.device_heap,
                internal: b.device,
                descriptor: b.descriptor,
                state: b.state,
                staging: b.staging,
                pending_start,
                pending_bytes,
            };
            r.valid = true;
            Some(0)
        }
        GpAction::Apply => {
            let metadata = b.gp_metadata?;
            if r.index >= metadata.map_count {
                r.valid = true;
                return Some(0x8878_0032);
            }
            let m = metadata.maps[r.index as usize];
            if r.offset > m.state_bytes
                || r.bytes > m.state_bytes - r.offset
                || u64::from(r.source) + u64::from(r.bytes) > 1 << 32
            {
                r.error = Some("GP effect span outside verified backing domain".into());
                return None;
            }
            if r.bytes != 0 && !sync_live(b, r) {
                return None;
            }
            if r.bytes != 0 && r.flags & 1 == 0 && !copy_write(b.state, &b.gp_mirror) {
                r.error = Some("GP immediate CPU state view unwritable".into());
                return None;
            }
            if !gp_forward_copy(b, r, false) {
                return None;
            }
            if r.flags & 1 == 0 && r.bytes != 0 && (r.offset & 3 != 0 || r.bytes & 3 != 0) {
                r.error = Some("GP immediate MMIO requires aligned four-byte accesses".into());
                return None;
            }
            if r.flags & 1 != 0 {
                if b.gp
                    .as_mut()?
                    .defer(r.index as usize, r.offset as usize, r.bytes as usize)
                    .is_err()
                {
                    r.error = Some("GP deferred range is not supported".into());
                    return None;
                }
            } else if !gp_forward_copy(b, r, true) {
                return None;
            }
            r.valid = true;
            Some(0)
        }
        GpAction::Commit | GpAction::Frame(..) => {
            if !sync_live(b, r) {
                return None;
            }
            let mut probe = vec![0u8; STATE_BYTES];
            if !copy_read(b.state, &mut probe) || !copy_write(b.state, &probe) {
                r.error = Some("GP consumer CPU state mirror unwritable".into());
                return None;
            }
            let gp = b.gp.as_mut()?;
            let ok = match action {
                GpAction::Commit => gp.commit().is_ok(),
                GpAction::Frame(input, output) => match gp.frame(input) {
                    Ok(words) => {
                        **output = words;
                        true
                    }
                    Err(_) => false,
                },
                GpAction::Apply | GpAction::View(_) => unreachable!(),
            };
            if !ok {
                let text =
                    b.gp.as_ref()?
                        .error()
                        .map(|m| m.trim_end_matches('\0').to_owned());
                r.error = Some(text.unwrap_or_else(|| "GP consumer span/operand refused".into()));
                return None;
            }
            if !publish_live(b, r) {
                return None;
            }
            r.valid = true;
            Some(0)
        }
    }
}

struct GpCall<'a, 'b> {
    request: &'a mut GpRequest,
    action: &'a mut GpAction<'b>,
}

unsafe extern "C" fn gp_owned(words: *const u32, userdata: *mut c_void, result: *mut u32) {
    // SAFETY: the device calls back with (heap, address) and the call/result slots the caller passed.
    let (device_heap, internal, call, result) = unsafe {
        (
            *words,
            *words.add(1),
            &mut *userdata.cast::<GpCall>(),
            &mut *result,
        )
    };
    if let Some(value) =
        with_binding(|b| gp_locked(b, internal, device_heap, call.action, call.request))
    {
        *result = value;
    }
}

fn gp_call(interface: u32, request: &mut GpRequest, action: &mut GpAction) -> Option<u32> {
    let mut call = GpCall { request, action };
    let mut result = 0u32;
    // SAFETY: the callback runs synchronously under the device lock with these live slots.
    let owned = unsafe {
        tsfp_audio_device_with_owned_identity(
            interface,
            Some(gp_owned),
            (&raw mut call).cast(),
            &mut result,
        )
    };
    owned.then_some(result)
}

/// `dsound_effects_binding_apply`: SetEffectData against the genuine GP state.
pub fn apply(interface: u32, index: u32, offset: u32, source: u32, bytes: u32, flags: u32) -> u32 {
    let mut blocked = [0u8; 4];
    if !copy_read(BLOCKED_GLOBAL, &mut blocked) {
        refuse_at(APPLY_ENTRY, "original audio global unreadable");
    }
    if u32::from_le_bytes(blocked) != 0 {
        return 0x8000_4005;
    }
    let mut request = GpRequest {
        index,
        offset,
        source,
        bytes,
        flags,
        ..GpRequest::default()
    };
    let Some(result) = gp_call(interface, &mut request, &mut GpAction::Apply) else {
        refuse_at(APPLY_ENTRY, "owned GP device interface invalid");
    };
    if let Some(error) = &request.error {
        if degrade(APPLY_ENTRY, error) {
            return 0;
        }
        refuse_at(APPLY_ENTRY, error);
    }
    result
}

/// `dsound_effects_binding_gp_commit`: the real firmware consumer.
pub fn gp_commit(interface: u32) -> bool {
    let mut request = GpRequest::default();
    gp_call(interface, &mut request, &mut GpAction::Commit).is_some() && request.valid
}

/// `dsound_effects_binding_gp_frame`: one frame of genuine preprojection bins.
pub fn gp_frame(
    interface: u32,
    input: &[u32; FRAME_WORDS],
    output: &mut [u32; FRAME_WORDS],
) -> bool {
    let mut request = GpRequest::default();
    gp_call(interface, &mut request, &mut GpAction::Frame(input, output)).is_some() && request.valid
}

fn frame_args<const N: usize>(
    context: *mut c_void,
    entry: u32,
    caller_expected: u32,
    caller_message: &str,
) -> [u32; N] {
    // SAFETY: the dispatcher passes the live `kernel_call_frame`, whose first field is the guest stack pointer.
    let stack = if context.is_null() {
        0
    } else {
        unsafe { *context.cast::<u32>() }
    };
    let mut caller = 0;
    // SAFETY: `caller` is a live out slot.
    if context.is_null()
        || !unsafe { kernel_guest_read_u32(stack, &mut caller) }
        || caller != caller_expected
    {
        refuse_at(entry, caller_message);
    }
    let mut arguments = [0u32; N];
    for (index, slot) in arguments.iter_mut().enumerate() {
        // SAFETY: the frame is live and `slot` is a live out slot.
        if !unsafe { kernel_frame_arg(context, index as u32, slot) } {
            refuse_at(
                entry,
                if entry == ENTRY {
                    "unreadable argument"
                } else {
                    "unreadable SetEffectData argument"
                },
            );
        }
    }
    arguments
}

unsafe extern "C" fn apply_handler(context: *mut c_void) -> u32 {
    let args = frame_args::<6>(
        context,
        APPLY_ENTRY,
        APPLY_CALLER,
        "only measured gain caller return38669F is supported",
    );
    // The recovered caller emits only four gain bytes at +20, immediate. Its supported firmware maps are the four
    // measured effect programs 5..8.
    if !(5..=8).contains(&args[1]) || args[2] != 0x20 || args[4] != 4 || args[5] != 0 {
        refuse_at(
            APPLY_ENTRY,
            "gain caller operands outside measured GP effect domain",
        );
    }
    apply(args[0], args[1], args[2], args[3], args[4], args[5])
}

unsafe extern "C" fn bind_handler(context: *mut c_void) -> u32 {
    let args = frame_args::<5>(
        context,
        ENTRY,
        CALLER,
        "only measured startup caller return 0x000270AF is recovered",
    );
    bind(args[0], args[1], args[2], args[3], args[4])
}

fn register_entry(address: u32, handler: Handler) -> bool {
    tsfp_audio_hle_register_handler_rust(address, handler)
        // SAFETY: the compatibility telemetry row exists once the HLE table is initialised.
        && unsafe { dsound_hle_register_rust(address) }
}

/// `dsound_effects_binding_register`: DownloadEffectsImage, plus SetEffectData with the GP policy on.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_effects_binding_register() -> usize {
    let mut registered = usize::from(register_entry(ENTRY, bind_handler));
    if with_binding(|b| b.gp_enabled) && register_entry(APPLY_ENTRY, apply_handler) {
        registered += 1;
    }
    registered
}

/// `dsound_effects_binding_bind`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_effects_binding_bind(
    interface: u32,
    image: u32,
    bytes: u32,
    location: u32,
    output: u32,
) -> u32 {
    bind(interface, image, bytes, location, output)
}

/// `dsound_effects_binding_apply`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_effects_binding_apply(
    interface: u32,
    index: u32,
    offset: u32,
    source: u32,
    bytes: u32,
    flags: u32,
) -> u32 {
    apply(interface, index, offset, source, bytes, flags)
}

/// `dsound_effects_binding_gp_view_get`.
///
/// # Safety
/// `output` must be null or valid for one write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_effects_binding_gp_view_get(
    interface: u32,
    output: *mut GpView,
) -> bool {
    if output.is_null() {
        return false;
    }
    let mut view = GpView::default();
    let mut request = GpRequest::default();
    let ok =
        gp_call(interface, &mut request, &mut GpAction::View(&mut view)).is_some() && request.valid;
    if ok {
        // SAFETY: checked non-null above; the caller guarantees validity.
        unsafe { output.write(view) };
    }
    ok
}

/// `dsound_effects_binding_gp_commit`.
#[unsafe(no_mangle)]
pub extern "C" fn dsound_effects_binding_gp_commit(interface: u32) -> bool {
    gp_commit(interface)
}

/// `dsound_effects_binding_gp_frame`.
///
/// # Safety
/// `input` and `output` must be null or point at 1024 words.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn dsound_effects_binding_gp_frame(
    interface: u32,
    input: *const u32,
    output: *mut u32,
) -> bool {
    if input.is_null() || output.is_null() {
        return false;
    }
    let mut frame = [0u32; FRAME_WORDS];
    // SAFETY: caller contract: 1024 readable words.
    frame.copy_from_slice(unsafe { std::slice::from_raw_parts(input, FRAME_WORDS) });
    let mut out = [0u32; FRAME_WORDS];
    let ok = gp_frame(interface, &frame, &mut out);
    if ok {
        // SAFETY: caller contract: 1024 writable words.
        unsafe { std::slice::from_raw_parts_mut(output, FRAME_WORDS) }.copy_from_slice(&out);
    }
    ok
}
