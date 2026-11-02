//! C ABI of the Rust mount and symbolic-link namespace (T2104). `kernel_file.c` keeps the tables' slot decisions and mirrors
//! each slot here (`kernel_file_namespace_backend`); with `--rust-fileio` link resolution and the longest-prefix volume
//! choice come from Rust, cross-checked against the C walk (kernel_file_namespace_shadow_mismatches).
use std::ffi::{CStr, c_char, c_int};
use std::sync::Mutex;
use tsfp_engine_fileio::namespace::{LinkInsertError, Namespace, Resolve};

static NAMESPACE: Mutex<Namespace> = Mutex::new(Namespace::new());

fn bytes<'a>(text: *const c_char) -> Option<&'a [u8]> {
    (!text.is_null()).then(|| unsafe { CStr::from_ptr(text) }.to_bytes())
}

unsafe extern "C" fn c_reset() {
    NAMESPACE.lock().unwrap().clear();
}

unsafe extern "C" fn c_set_mount(slot: u32, prefix: *const c_char) {
    NAMESPACE
        .lock()
        .unwrap()
        .set_mount(slot as usize, bytes(prefix));
}

unsafe extern "C" fn c_set_link(slot: u32, name: *const c_char, target: *const c_char) {
    let link = bytes(name).zip(bytes(target));
    NAMESPACE.lock().unwrap().set_link(slot as usize, link);
}

// 1 settled (out filled), 0 too long, -1 cyclic.
unsafe extern "C" fn c_resolve(path: *const c_char, out: *mut c_char, out_bytes: usize) -> c_int {
    let Some(path) = bytes(path) else { return 0 };
    if out.is_null() {
        return 0;
    }
    crate::counters::bump(crate::counters::NAMESPACE);
    match NAMESPACE.lock().unwrap().resolve(path, out_bytes) {
        Resolve::Settled(name) => {
            unsafe {
                std::ptr::copy_nonoverlapping(name.as_ptr(), out.cast::<u8>(), name.len());
                *out.add(name.len()) = 0;
            }
            1
        }
        Resolve::TooLong => 0,
        Resolve::Cyclic => -1,
    }
}

// Volume slot or -1; `*rest` receives the prefix length.
unsafe extern "C" fn c_volume_for(path: *const c_char, rest: *mut usize) -> c_int {
    let Some(path) = bytes(path) else { return -1 };
    match NAMESPACE.lock().unwrap().volume_for(path) {
        Some((slot, length)) => {
            if !rest.is_null() {
                unsafe { *rest = length };
            }
            slot as c_int
        }
        None => -1,
    }
}

unsafe extern "C" fn c_mount_slot(unique: *const c_char) -> c_int {
    let unique = bytes(unique);
    NAMESPACE
        .lock()
        .unwrap()
        .mount_slot(unique)
        .map_or(-1, |slot| slot as c_int)
}

// HLE-specific insertion. The generic C helper retains its replace-existing contract.
unsafe extern "C" fn c_insert_link(
    name: *const c_char,
    target: *const c_char,
    capacity: u32,
) -> c_int {
    let (Some(name), Some(target)) = (bytes(name), bytes(target)) else {
        return -1;
    };
    match NAMESPACE
        .lock()
        .unwrap()
        .insert_link(name, target, capacity as usize)
    {
        Ok(_) => 1,
        Err(LinkInsertError::Duplicate) => 0,
        Err(_) => -1,
    }
}

unsafe extern "C" fn c_remove_link(name: *const c_char) -> c_int {
    let Some(name) = bytes(name) else { return 0 };
    NAMESPACE.lock().unwrap().remove_link(name).map_or(0, |_| 1)
}

unsafe extern "C" fn c_replace_link(
    name: *const c_char,
    target: *const c_char,
    capacity: u32,
) -> c_int {
    let (Some(name), Some(target)) = (bytes(name), bytes(target)) else {
        return -1;
    };
    NAMESPACE
        .lock()
        .unwrap()
        .replace_link(name, target, capacity as usize)
        .map_or(-1, |slot| slot as c_int)
}

#[repr(C)]
pub struct NamespaceBackend {
    reset: unsafe extern "C" fn(),
    set_mount: unsafe extern "C" fn(u32, *const c_char),
    set_link: unsafe extern "C" fn(u32, *const c_char, *const c_char),
    resolve: unsafe extern "C" fn(*const c_char, *mut c_char, usize) -> c_int,
    volume_for: unsafe extern "C" fn(*const c_char, *mut usize) -> c_int,
    mount_slot: unsafe extern "C" fn(*const c_char) -> c_int,
    insert_link: unsafe extern "C" fn(*const c_char, *const c_char, u32) -> c_int,
    remove_link: unsafe extern "C" fn(*const c_char) -> c_int,
    replace_link: unsafe extern "C" fn(*const c_char, *const c_char, u32) -> c_int,
}
static BACKEND: NamespaceBackend = NamespaceBackend {
    reset: c_reset,
    set_mount: c_set_mount,
    set_link: c_set_link,
    resolve: c_resolve,
    volume_for: c_volume_for,
    mount_slot: c_mount_slot,
    insert_link: c_insert_link,
    remove_link: c_remove_link,
    replace_link: c_replace_link,
};

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_namespace_backend() -> *const NamespaceBackend {
    &BACKEND
}
