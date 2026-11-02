// SPDX-License-Identifier: GPL-3.0-or-later
//! `src/input/mu_device.c` in Rust. The slot table lives in `STATE`, guarded by the same kind of pthread mutex the C
//! file used (a fatal-handler catcher may unwind past `refuse` with the lock already released, exactly as in C).
use std::cell::UnsafeCell;
use std::ffi::{CString, c_char, c_void};

use tsfp_input_devices::mu::{MU_ENTRIES, MuTable, index_of};

use crate::errors::{ALREADY_ASSIGNED, DEVICE_NOT_CONNECTED, INVALID_DRIVE, UNRECOGNIZED_VOLUME};
use crate::sys;

const PORTS: u32 = 4;
const SLOTS: u32 = 2;
const ENTRY_MOUNT: u32 = 0x0046_D7C4;
const ENTRY_UNMOUNT: u32 = 0x0046_D8F6;
const MASK_ADDRESS: u32 = 0x0076_E348;
const TITLE_DRIVE_ADDRESS: u32 = 0x0076_E344;
const CERTIFICATE_POINTER: u32 = 0x0001_0118;
const HANDLER_MOUNT: u32 = 1;
const HANDLER_UNMOUNT: u32 = 2;
const FATX_OK: u32 = 0;
const FATX_E_EXISTS: u32 = 11;
const FATX_SECTOR_BYTES: usize = 512;
const FATX_MIN_IMAGE: usize = 4096 * 2 + 32 * 512 * 2;
const WRITE_FAULT: u32 = 0x1D;

type FatalFn = unsafe extern "C" fn(u32, *const c_char);

struct Slot {
    image: Option<Vec<u8>>,
    path: Option<String>,
    device_id: u32,
    device_path: [u8; 40],
    drive_link: [u8; 16],
    volume: sys::Volume,
}

impl Slot {
    const EMPTY: Slot = Slot {
        image: None,
        path: None,
        device_id: 0,
        device_path: [0; 40],
        drive_link: [0; 16],
        volume: sys::Volume {
            image: std::ptr::null_mut(),
            size: 0,
            volume_id: 0,
            sectors_per_cluster: 0,
            root_cluster: 0,
            cluster_count: 0,
            fat_offset: 0,
            fat_size: 0,
            data_offset: 0,
            fat32: false,
        },
    };

    fn size(&self) -> usize {
        self.image.as_ref().map_or(0, Vec::len)
    }
}

struct State {
    table: Option<MuTable>,
    slots: [Slot; MU_ENTRIES],
    guest_filesystem: bool,
    fatal: Option<FatalFn>,
}

struct Global<T>(UnsafeCell<T>);
// SAFETY: every access happens under `LOCK` (or during single-threaded startup, as in the C file).
unsafe impl<T> Sync for Global<T> {}

static LOCK: Global<sys::Mutex> = Global(UnsafeCell::new(sys::Mutex { storage: [0; 8] }));
static STATE: Global<State> = Global(UnsafeCell::new(State {
    table: None,
    slots: [Slot::EMPTY; MU_ENTRIES],
    guest_filesystem: false,
    fatal: None,
}));

fn lock() {
    // SAFETY: glibc's all-zero bytes are PTHREAD_MUTEX_INITIALIZER.
    unsafe { sys::pthread_mutex_lock(LOCK.0.get()) };
}

fn unlock() {
    // SAFETY: paired with `lock`.
    unsafe { sys::pthread_mutex_unlock(LOCK.0.get()) };
}

/// The state; callers hold `LOCK`.
#[allow(clippy::mut_from_ref)]
fn state() -> &'static mut State {
    // SAFETY: the lock serialises every caller.
    unsafe { &mut *STATE.0.get() }
}

fn table() -> &'static mut MuTable {
    state().table.get_or_insert_with(MuTable::default)
}

fn cstring(text: &str) -> CString {
    CString::new(text.replace('\0', "")).unwrap_or_default()
}

fn put(buffer: &mut [u8], text: &str) {
    let length = text.len().min(buffer.len() - 1);
    buffer.fill(0);
    buffer[..length].copy_from_slice(&text.as_bytes()[..length]);
}

fn refuse(entry: u32, message: &str) -> ! {
    let text = cstring(message);
    // SAFETY: the format has one %#x and one %s, both supplied.
    unsafe { (sys::xinput_hle_log())(c"mu: %#x refused: %s\n".as_ptr(), entry, text.as_ptr()) };
    unlock();
    if let Some(handler) = state().fatal {
        // SAFETY: the handler was registered by the host with this signature.
        unsafe { handler(entry, text.as_ptr()) };
    }
    std::process::abort()
}

#[unsafe(no_mangle)]
pub extern "C" fn mu_set_fatal(handler: Option<FatalFn>) {
    state().fatal = handler;
}

#[unsafe(no_mangle)]
pub extern "C" fn mu_drive_letter(port: u32, slot: u32) -> c_char {
    crate::drive_letter(port, slot) as c_char
}

fn mirror_mask() {
    // SAFETY: guest accessors validate the address range themselves.
    unsafe {
        if !sys::kernel_guest_at_ro(MASK_ADDRESS, 4).is_null() {
            let mask = table().mounted_mask();
            let _ = sys::kernel_guest_write_bytes(MASK_ADDRESS, (&raw const mask).cast(), 4);
        }
    }
}

fn cptr(buffer: &[u8]) -> *const c_char {
    buffer.as_ptr().cast()
}

fn locked_reset() {
    lock();
    for index in 0..MU_ENTRIES {
        let slot = &mut state().slots[index];
        if table().is_guest_backed(index) {
            // SAFETY: the buffers are NUL terminated by `put`.
            unsafe {
                let _ = sys::tsfp_rfio_remove_symlink(cptr(&slot.drive_link));
                let _ = sys::tsfp_rfio_unmount_fatx(cptr(&slot.device_path));
            }
        }
        *slot = Slot::EMPTY;
    }
    table().reset();
    mirror_mask(); // Reset ownership and its mapped guest mirror under the same lock.
    unlock();
}

fn slot_index(port: u32, slot: u32) -> Option<usize> {
    if port < PORTS && slot < SLOTS {
        index_of(port, slot)
    } else {
        None
    }
}

fn locked_attach_blank(port: u32, slot: u32, size: usize, format: bool, volume_id: u32) -> bool {
    lock();
    let mut ok = false;
    if let Some(index) = slot_index(port, slot) {
        let s = &mut state().slots[index];
        ok = s.image.is_none() && size >= FATX_MIN_IMAGE && size % FATX_SECTOR_BYTES == 0;
        if ok {
            let mut image = vec![0u8; size];
            // SAFETY: the buffer is `size` bytes.
            ok = !format
                || unsafe { sys::fatx_format(image.as_mut_ptr(), size, volume_id) } == FATX_OK;
            if ok {
                s.image = Some(image);
            }
        }
    }
    unlock();
    ok
}

fn locked_attach_image(port: u32, slot: u32, image: &[u8]) -> bool {
    lock();
    let mut ok = false;
    if let Some(index) = slot_index(port, slot) {
        let s = &mut state().slots[index];
        ok = s.image.is_none()
            && image.len() >= FATX_MIN_IMAGE
            && image.len() % FATX_SECTOR_BYTES == 0;
        if ok {
            s.image = Some(image.to_vec());
        }
    }
    unlock();
    ok
}

fn locked_attach_file(port: u32, slot: u32, path: &str, create_size: usize) -> bool {
    let ok = match std::fs::read(path) {
        Ok(bytes) => !bytes.is_empty() && locked_attach_image(port, slot, &bytes),
        Err(_) if create_size != 0 => locked_attach_blank(
            port,
            slot,
            create_size,
            true,
            0x4D55_0000 | port << 8 | slot,
        ),
        Err(_) => false,
    };
    if !ok {
        return false;
    }
    lock();
    if let Some(index) = slot_index(port, slot) {
        state().slots[index].path = Some(path.to_owned());
    }
    unlock();
    true
}

fn flush(slot: &Slot) -> bool {
    match (&slot.path, &slot.image) {
        (None, _) => true,
        (Some(path), Some(image)) => std::fs::write(path, image).is_ok(),
        (Some(path), None) => std::fs::write(path, []).is_ok(),
    }
}

unsafe extern "C" fn flush_guest_image(opaque: *mut c_void) -> bool {
    lock();
    // SAFETY: `opaque` is the slot passed to `tsfp_rfio_mount_fatx`, a static array element.
    let ok = flush(unsafe { &*opaque.cast::<Slot>() });
    unlock();
    ok
}

fn locked_detach(port: u32, slot: u32) -> bool {
    lock();
    let mut ok = false;
    if let Some(index) = slot_index(port, slot) {
        ok = state().slots[index].image.is_some() && !table().mounted_flag(index);
        if ok {
            state().slots[index] = Slot::EMPTY;
            table().set_eject_pending(index, false);
        }
    }
    unlock();
    ok
}

fn locked_request_detach(port: u32, slot: u32) -> bool {
    lock();
    let mut ok = false;
    if let Some(index) = slot_index(port, slot) {
        ok = state().slots[index].image.is_some();
        if ok {
            if table().mounted_flag(index) {
                table().set_eject_pending(index, true);
            } else {
                state().slots[index] = Slot::EMPTY;
                table().set_eject_pending(index, false);
            }
        }
    }
    unlock();
    ok
}

#[unsafe(no_mangle)]
pub extern "C" fn mu_attached(port: u32, slot: u32) -> bool {
    lock();
    let ok = slot_index(port, slot).is_some_and(|index| state().slots[index].image.is_some());
    unlock();
    ok
}

/// Raw block image, or NULL. Writes through it bypass the mount.
#[unsafe(no_mangle)]
pub extern "C" fn mu_image(port: u32, slot: u32, size: *mut usize) -> *mut u8 {
    lock();
    let image = slot_index(port, slot).and_then(|index| state().slots[index].image.as_mut());
    let (pointer, length) = image.map_or((std::ptr::null_mut(), 0), |bytes| {
        (bytes.as_mut_ptr(), bytes.len())
    });
    if !size.is_null() {
        // SAFETY: a non-null out pointer is writable by contract.
        unsafe { *size = length };
    }
    unlock();
    pointer
}

#[unsafe(no_mangle)]
pub extern "C" fn mu_volume(port: u32, slot: u32) -> *mut sys::Volume {
    lock();
    let volume = slot_index(port, slot)
        .filter(|&index| table().mounted_flag(index))
        .map_or(std::ptr::null_mut(), |index| {
            &raw mut state().slots[index].volume
        });
    unlock();
    volume
}

#[unsafe(no_mangle)]
pub extern "C" fn mu_mounted_mask() -> u32 {
    lock();
    let mask = table().mounted_mask();
    unlock();
    mask
}

/// `drive` is written only when non-null (the C `drive_wanted`).
fn locked_mount(port: u32, slot: u32, drive: *mut u8) -> u32 {
    let Some(index) = index_of(port, slot) else {
        refuse(
            ENTRY_MOUNT,
            "port/slot outside the 8-entry device table: the original does not range-check and would index past it",
        )
    };
    let letter = 0x46 + index as u8;
    let wanted = !drive.is_null();
    let set_drive = |value: u8| {
        if wanted {
            // SAFETY: a non-null drive output is writable by contract.
            unsafe { *drive = value };
        }
    };
    set_drive(0); // 0046D7D4 clears before locking
    if table().is_mounted(index) {
        set_drive(letter);
        return ALREADY_ASSIGNED;
    }
    let guest = state().guest_filesystem;
    let s = &mut state().slots[index];
    let size = s.size();
    let Some(image) = s.image.as_mut() else {
        return DEVICE_NOT_CONNECTED;
    };
    // SAFETY: the volume view borrows the slot's image, which outlives the mount.
    if unsafe { sys::fatx_open(&raw mut s.volume, image.as_mut_ptr(), size) } != FATX_OK {
        return UNRECOGNIZED_VOLUME;
    }
    if guest {
        let (mut certificate, mut title_id) = (0u32, 0u32);
        // SAFETY: out pointers are locals.
        let readable = unsafe {
            sys::kernel_guest_read_u32(CERTIFICATE_POINTER, &mut certificate)
                && certificate <= u32::MAX - 8
                && sys::kernel_guest_read_u32(certificate + 8, &mut title_id)
        };
        if !readable {
            return 0x57;
        }
        let Some(device_id) = table().peek_free_id() else {
            return 8;
        };
        let title = cstring(&format!("{title_id:08x}"));
        let (mut directory, mut length) = (false, 0u32);
        // SAFETY: the volume was opened above and the strings are NUL terminated.
        let result = unsafe {
            let made = sys::fatx_mkdir_path(&raw mut s.volume, title.as_ptr());
            if made == FATX_E_EXISTS {
                sys::fatx_stat_path(
                    &raw const s.volume,
                    title.as_ptr(),
                    &mut directory,
                    &mut length,
                )
            } else {
                directory = made == FATX_OK;
                made
            }
        };
        if result != FATX_OK || !directory {
            return UNRECOGNIZED_VOLUME;
        }
        s.device_id = device_id;
        put(&mut s.device_path, &format!("\\Device\\MU_{device_id:x}"));
        put(&mut s.drive_link, &format!("\\??\\{}:", letter as char));
        let target = cstring(&format!(
            "{}\\{}",
            String::from_utf8_lossy(
                &s.device_path[..s.device_path.iter().position(|&b| b == 0).unwrap_or(0)]
            ),
            title.to_string_lossy()
        ));
        // SAFETY: the slot is a static array element, valid for the mount's lifetime.
        unsafe {
            if !sys::tsfp_rfio_mount_fatx(
                cptr(&s.device_path),
                &raw mut s.volume,
                flush_guest_image,
                (&raw mut *s).cast(),
            ) {
                return 8;
            }
            if !sys::tsfp_rfio_add_symlink(cptr(&s.drive_link), target.as_ptr()) {
                let _ = sys::tsfp_rfio_unmount_fatx(cptr(&s.device_path));
                return 8;
            }
        }
    }
    table().mark_mounted(index, guest);
    mirror_mask();
    set_drive(letter);
    0
}

fn file_locked_mount(port: u32, slot: u32, drive: *mut u8) -> u32 {
    lock();
    let result = locked_mount(port, slot, drive);
    unlock();
    result
}

fn file_locked_unmount(port: u32, slot: u32) -> u32 {
    lock();
    let Some(index) = index_of(port, slot) else {
        refuse(
            ENTRY_UNMOUNT,
            "port/slot outside the 8-entry device table: the original does not range-check",
        )
    };
    if !table().is_mounted(index) {
        unlock();
        return INVALID_DRIVE;
    }
    let s = &mut state().slots[index];
    let flushed = flush(s);
    if table().is_guest_backed(index) {
        // SAFETY: the buffers were written by `put` at mount time.
        unsafe {
            let _ = sys::tsfp_rfio_remove_symlink(cptr(&s.drive_link));
            let _ = sys::tsfp_rfio_unmount_fatx(cptr(&s.device_path));
        }
    }
    table().mark_unmounted(index);
    s.volume = Slot::EMPTY.volume;
    if flushed && table().eject_pending(index) {
        *s = Slot::EMPTY;
        table().set_eject_pending(index, false);
    }
    mirror_mask();
    unlock();
    if flushed { 0 } else { WRITE_FAULT } // host flush failure, FABRICATED mapping
}

struct MountRequest {
    port: u32,
    slot: u32,
    result: u32,
    wanted: bool,
    unmount: bool,
    drive: *mut u8,
}

unsafe extern "C" fn mount_job(opaque: *mut c_void) {
    // SAFETY: `opaque` is the request on the caller's stack.
    let r = unsafe { &mut *opaque.cast::<MountRequest>() };
    r.result = if r.unmount {
        file_locked_unmount(r.port, r.slot)
    } else {
        file_locked_mount(
            r.port,
            r.slot,
            if r.wanted {
                r.drive
            } else {
                std::ptr::null_mut()
            },
        )
    };
}

fn run_mount(mut request: MountRequest) -> u32 {
    // SAFETY: the request outlives the synchronous call.
    unsafe { sys::tsfp_rfio_run_locked(mount_job, (&raw mut request).cast()) };
    request.result
}

/// Host-side XMountMUA: returns the Win32 error. `*drive` is 0 or the letter, as the guest byte would be.
#[unsafe(no_mangle)]
pub extern "C" fn mu_xmount(port: u32, slot: u32, wanted: bool, drive: *mut u8) -> u32 {
    lock();
    let Some(index) = index_of(port, slot) else {
        refuse(
            ENTRY_MOUNT,
            "port/slot outside the 8-entry device table: original unchecked access",
        )
    };
    if !wanted && table().is_mounted(index) {
        refuse(
            ENTRY_MOUNT,
            "duplicate mount NULL drive output faults at original 0046D80B",
        );
    }
    unlock();
    run_mount(MountRequest {
        port,
        slot,
        result: 0,
        wanted,
        unmount: false,
        drive,
    })
}

/// Host-side XUnmountMU: returns the Win32 error. Fatal original-domain guards precede the file lock.
#[unsafe(no_mangle)]
pub extern "C" fn mu_xunmount(port: u32, slot: u32) -> u32 {
    lock();
    let Some(index) = index_of(port, slot) else {
        refuse(ENTRY_UNMOUNT, "port/slot outside the original device table")
    };
    // SAFETY: the guest accessor validates the address.
    let title = unsafe {
        sys::kernel_guest_at_ro(TITLE_DRIVE_ADDRESS, 1)
            .cast::<u8>()
            .as_ref()
            .copied()
    };
    if table().is_mounted(index) && title == Some(0x46 + index as u8) {
        refuse(
            ENTRY_UNMOUNT,
            "unmounting the title's own MU drive calls 0037DB94(0x58) in the original, not modelled",
        );
    }
    unlock();
    run_mount(MountRequest {
        port,
        slot,
        result: 0,
        wanted: false,
        unmount: true,
        drive: std::ptr::null_mut(),
    })
}

/// Startup opt-in only: live mounts cannot change their namespace contract.
#[unsafe(no_mangle)]
pub extern "C" fn mu_enable_guest_filesystem(enabled: bool) -> bool {
    lock();
    let ok = table().mounted_mask() == 0;
    if ok {
        state().guest_filesystem = enabled;
    }
    unlock();
    ok
}

fn argument(context: *mut c_void, entry: u32, index: u32) -> u32 {
    let mut value = 0u32;
    // SAFETY: `context` is the call frame the dispatcher passed.
    if !unsafe { sys::kernel_frame_arg(context, index, &mut value) } {
        lock();
        refuse(entry, "unreadable stack argument");
    }
    value
}

unsafe extern "C" fn mount_handler(context: *mut c_void) -> u32 {
    let (port, slot, out) = (
        argument(context, ENTRY_MOUNT, 0),
        argument(context, ENTRY_MOUNT, 1),
        argument(context, ENTRY_MOUNT, 2),
    );
    if out != 0 {
        // SAFETY: guest accessors validate the address.
        unsafe {
            if sys::kernel_guest_at_ro(out, 1).is_null() {
                lock();
                refuse(
                    ENTRY_MOUNT,
                    "drive output pointer is not mapped (the original faults)",
                );
            }
            if !sys::kernel_guest_write_u8(out, 0) {
                lock();
                refuse(ENTRY_MOUNT, "drive output pointer is not writable");
            }
        }
    }
    let mut drive = 0u8;
    let result = mu_xmount(port, slot, out != 0, &mut drive);
    // SAFETY: as above.
    if out != 0 && !unsafe { sys::kernel_guest_write_u8(out, drive) } {
        lock();
        refuse(ENTRY_MOUNT, "drive output pointer is not writable");
    }
    result
}

unsafe extern "C" fn unmount_handler(context: *mut c_void) -> u32 {
    mu_xunmount(
        argument(context, ENTRY_UNMOUNT, 0),
        argument(context, ENTRY_UNMOUNT, 1),
    )
}

/// Register both guest handlers. Returns the number registered (2).
#[unsafe(no_mangle)]
pub extern "C" fn mu_register() -> usize {
    // SAFETY: handlers have the dispatcher's signature.
    unsafe {
        usize::from(sys::xinput_hle_register_identified(
            ENTRY_MOUNT,
            mount_handler,
            HANDLER_MOUNT,
        )) + usize::from(sys::xinput_hle_register_identified(
            ENTRY_UNMOUNT,
            unmount_handler,
            HANDLER_UNMOUNT,
        ))
    }
}

/// One ordered lifecycle step: presence notification and image mutation serialised together.
enum Operation<'a> {
    Reset,
    AttachBlank {
        size: usize,
        format: bool,
        volume_id: u32,
    },
    AttachImage(&'a [u8]),
    AttachFile {
        path: &'a str,
        size: usize,
    },
    Detach,
    RequestDetach,
}

struct Lifecycle<'a> {
    operation: Operation<'a>,
    port: u32,
    slot: u32,
    result: bool,
}

unsafe extern "C" fn file_locked_lifecycle_job(opaque: *mut c_void) {
    // SAFETY: `opaque` is the lifecycle on the caller's stack.
    let r = unsafe { &mut *opaque.cast::<Lifecycle>() };
    r.result = match &r.operation {
        Operation::Reset => {
            locked_reset();
            true
        }
        Operation::AttachBlank {
            size,
            format,
            volume_id,
        } => locked_attach_blank(r.port, r.slot, *size, *format, *volume_id),
        Operation::AttachImage(image) => locked_attach_image(r.port, r.slot, image),
        Operation::AttachFile { path, size } => locked_attach_file(r.port, r.slot, path, *size),
        Operation::Detach => locked_detach(r.port, r.slot),
        Operation::RequestDetach => locked_request_detach(r.port, r.slot),
    };
    let mut mask = 0u32;
    lock();
    for index in 0..MU_ENTRIES {
        if state().slots[index].image.is_some() && !table().eject_pending(index) {
            mask |= crate::presence_bit(index);
        }
    }
    unlock();
    // SAFETY: called under the devices lock, as the contract requires.
    unsafe { sys::xinput_devices_set_mu_mask(mask) };
}

unsafe extern "C" fn lifecycle_job(opaque: *mut c_void) {
    // SAFETY: nests the file lock inside the devices lock, as the C file did.
    unsafe { sys::tsfp_rfio_run_locked(file_locked_lifecycle_job, opaque) };
}

fn lifecycle(operation: Operation, port: u32, slot: u32) -> bool {
    let mut request = Lifecycle {
        operation,
        port,
        slot,
        result: false,
    };
    // SAFETY: the request outlives the synchronous call.
    unsafe { sys::xinput_devices_run_locked(lifecycle_job, (&raw mut request).cast()) };
    request.result
}

#[unsafe(no_mangle)]
pub extern "C" fn mu_reset() {
    lifecycle(Operation::Reset, 0, 0);
}

/// Plug a blank (optionally FATX formatted) MU of `size` bytes. False if occupied, bad slot or bad size.
#[unsafe(no_mangle)]
pub extern "C" fn mu_attach_blank(
    port: u32,
    slot: u32,
    size: usize,
    format: bool,
    volume_id: u32,
) -> bool {
    lifecycle(
        Operation::AttachBlank {
            size,
            format,
            volume_id,
        },
        port,
        slot,
    )
}

/// Plug a copy of `image`.
#[unsafe(no_mangle)]
pub extern "C" fn mu_attach_image(port: u32, slot: u32, image: *const u8, size: usize) -> bool {
    if image.is_null() {
        return lifecycle(Operation::AttachImage(&[]), port, slot);
    }
    // SAFETY: the caller passes `size` readable bytes.
    lifecycle(
        Operation::AttachImage(unsafe { std::slice::from_raw_parts(image, size) }),
        port,
        slot,
    )
}

/// Plug a host file as the MU image; flushed back on unmount. Created blank-formatted if absent and `create_size`!=0.
#[unsafe(no_mangle)]
pub extern "C" fn mu_attach_file(
    port: u32,
    slot: u32,
    path: *const c_char,
    create_size: usize,
) -> bool {
    if path.is_null() {
        return false;
    }
    // SAFETY: a non-null path is a NUL terminated string.
    let Ok(path) = unsafe { std::ffi::CStr::from_ptr(path) }.to_str() else {
        return false;
    };
    lifecycle(
        Operation::AttachFile {
            path,
            size: create_size,
        },
        port,
        slot,
    )
}

/// Unplug. Refused (false) while mounted.
#[unsafe(no_mangle)]
pub extern "C" fn mu_detach(port: u32, slot: u32) -> bool {
    lifecycle(Operation::Detach, port, slot)
}

/// Request physical removal; a mounted image is retained until the guest calls XUnmountMU successfully.
#[unsafe(no_mangle)]
pub extern "C" fn mu_request_detach(port: u32, slot: u32) -> bool {
    lifecycle(Operation::RequestDetach, port, slot)
}
