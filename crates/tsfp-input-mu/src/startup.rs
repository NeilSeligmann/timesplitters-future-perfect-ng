// SPDX-License-Identifier: GPL-3.0-or-later
//! `src/input/mu_startup.c` in Rust: attach writable EXISTING images before guest initialisation.
use std::ffi::{CStr, c_char, c_void};

use crate::device::{mu_attach_file, mu_attached, mu_detach};
use crate::sys;

const PORTS: u32 = 4;
const SLOTS: u32 = 2;
const STARTUP_MAX: usize = 8;

#[repr(C)]
pub struct StartupImage {
    pub port: u32,
    pub slot: u32,
    pub path: *const c_char,
}

struct Request {
    images: *const StartupImage,
    count: usize,
    error: *mut c_char,
    error_size: usize,
    result: bool,
}

impl Request {
    fn fail(&self, index: usize, reason: &str) {
        if self.error.is_null() || self.error_size == 0 {
            return;
        }
        let text = format!("MU image {index}: {reason}");
        let length = text.len().min(self.error_size - 1);
        // SAFETY: the caller's buffer holds `error_size` bytes.
        unsafe {
            std::ptr::copy_nonoverlapping(text.as_ptr(), self.error.cast::<u8>(), length);
            *self.error.add(length) = 0;
        }
    }

    fn path(image: &StartupImage) -> Option<&str> {
        // SAFETY: a non-null path is a NUL terminated string.
        (!image.path.is_null())
            .then(|| unsafe { CStr::from_ptr(image.path) }.to_str().ok())
            .flatten()
    }

    fn attach(&mut self) {
        if self.count > STARTUP_MAX || (self.count != 0 && self.images.is_null()) {
            self.fail(0, "invalid image list");
            return;
        }
        let images: &[StartupImage] = if self.count == 0 {
            &[]
        } else {
            // SAFETY: `count` entries were validated non-null above.
            unsafe { std::slice::from_raw_parts(self.images, self.count) }
        };
        let mut used = 0u32;
        for (i, image) in images.iter().enumerate() {
            if image.port >= PORTS
                || image.slot >= SLOTS
                || Self::path(image).is_none_or(str::is_empty)
            {
                self.fail(i, "expected port 0..3, slot 0..1 and a nonempty image path");
                return;
            }
            let bit = 1u32 << (image.port * SLOTS + image.slot);
            if used & bit != 0 || mu_attached(image.port, image.slot) {
                self.fail(i, "duplicate or already attached slot");
                return;
            }
            used |= bit;
        }
        let mut attached = 0;
        while attached < images.len() {
            let image = &images[attached];
            let path = Self::path(image).unwrap_or_default();
            // Validate write access without writing/creating/truncating the image.
            if std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)
                .is_err()
            {
                self.fail(attached, "cannot open existing image for read/write");
                break;
            }
            if !mu_attach_file(image.port, image.slot, image.path, 0) {
                self.fail(
                    attached,
                    "invalid image geometry or allocation/read failure",
                );
                break;
            }
            attached += 1;
        }
        if attached == images.len() {
            self.result = true;
            return;
        }
        for image in images[..attached].iter().rev() {
            let _ = mu_detach(image.port, image.slot);
        }
    }
}

unsafe extern "C" fn attach_job(opaque: *mut c_void) {
    // SAFETY: `opaque` is the request on the caller's stack.
    unsafe { &mut *opaque.cast::<Request>() }.attach();
}

#[unsafe(no_mangle)]
pub extern "C" fn mu_startup_attach(
    images: *const StartupImage,
    count: usize,
    error: *mut c_char,
    error_size: usize,
) -> bool {
    if !error.is_null() && error_size != 0 {
        // SAFETY: the caller's buffer holds `error_size` bytes.
        unsafe { *error = 0 };
    }
    let mut request = Request {
        images,
        count,
        error,
        error_size,
        result: false,
    };
    // SAFETY: the request outlives the synchronous call.
    unsafe { sys::xinput_devices_run_locked(attach_job, (&raw mut request).cast()) };
    request.result
}
