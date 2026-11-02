//! C ABI for Rust directory enumeration. Host-directory iteration, metadata and selection run here; FATX directory bytes
//! remain supplied by the C adapter to the same Rust selector.
use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, OnceLock};
use tsfp_engine_fileio::dirpick::{
    Begin, HostKind, OwnedCursor, Pick, Selection, admit_host_entry, status,
};

static CURSORS: OnceLock<Mutex<HashMap<u32, OwnedCursor>>> = OnceLock::new();

fn cursors() -> &'static Mutex<HashMap<u32, OwnedCursor>> {
    CURSORS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Mirrors `kernel_file_dir_entry` in src/xbox/kernel_file.h.
#[repr(C)]
#[derive(Clone)]
pub struct DirEntry {
    name: [u8; 256],
    is_directory: bool,
    size: u64,
    creation_time: u64,
    last_access_time: u64,
    last_write_time: u64,
    image_backed: bool,
    attributes: u32,
    allocation: u64,
}

impl DirEntry {
    fn name_bytes(&self) -> &[u8] {
        let end = self
            .name
            .iter()
            .position(|b| *b == 0)
            .unwrap_or(self.name.len());
        &self.name[..end]
    }
}

// The selected FATX path scans physical directory slots directly from the
// borrowed image. `fatx_ffi` owns format decoding; this function owns selection
// and result construction. The C caller holds the kernel-file lock throughout.
unsafe extern "C" fn c_pick_fatx(
    volume: *const c_void,
    path: *const c_char,
    mask: *const c_char,
    last: *const c_char,
    max_name: u32,
    out: *mut DirEntry,
    fatx_status: *mut u32,
) -> c_int {
    if volume.is_null()
        || path.is_null()
        || mask.is_null()
        || out.is_null()
        || fatx_status.is_null()
    {
        return -2;
    }
    unsafe { *fatx_status = 0 };
    let result = catch_unwind(AssertUnwindSafe(|| unsafe {
        let path = CStr::from_ptr(path).to_bytes();
        let mask = CStr::from_ptr(mask).to_bytes();
        let last = (!last.is_null()).then(|| CStr::from_ptr(last).to_bytes());
        let mut selection = Selection::new(mask, last);
        let mut best = None;
        let entries =
            tsfp_engine_fileio::fatx_img::CLUSTER_BYTES / tsfp_engine_fileio::fatx_img::ENTRY_BYTES;
        for index in 0..entries {
            let (candidate, end) =
                match crate::fatx_ffi::directory_entry_at(volume.cast(), path, index as u32) {
                    Ok(value) => value,
                    Err(status) => {
                        *fatx_status = status;
                        return -2;
                    }
                };
            if end {
                break;
            }
            let name_end = candidate
                .name
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(candidate.name.len());
            let name = &candidate.name[..name_end];
            if !name.is_empty() && selection.offer(index, name) {
                best = Some(candidate);
            }
        }
        match (selection.finish(max_name as usize), best) {
            (Pick::Found(_), Some(candidate)) => {
                let mut entry = DirEntry {
                    name: [0; 256],
                    is_directory: candidate.directory,
                    size: u64::from(candidate.size),
                    creation_time: candidate.creation_time,
                    last_access_time: candidate.access_time,
                    last_write_time: candidate.write_time,
                    image_backed: true,
                    attributes: u32::from(candidate.attributes),
                    allocation: candidate.allocation,
                };
                let name_end = candidate
                    .name
                    .iter()
                    .position(|byte| *byte == 0)
                    .unwrap_or(candidate.name.len());
                entry.name[..name_end].copy_from_slice(&candidate.name[..name_end]);
                *out = entry;
                1
            }
            (Pick::None, _) => 0,
            (Pick::TooLong, _) => -1,
            _ => -2,
        }
    }));
    crate::counters::bump(crate::counters::DIR_PICK);
    match result {
        Ok(result) => result,
        Err(_) => -2,
    }
}

// Rust owns the host-directory walk on the selected file-I/O path. `fd` remains
// borrowed from the C open record; fdopendir takes only the duplicate, just as
// the old C adapter did. The C build keeps its independent scan as the oracle.
unsafe extern "C" fn c_pick_host(
    fd: libc::c_int,
    mask: *const c_char,
    last: *const c_char,
    max_name: u32,
    out: *mut DirEntry,
    out_errno: *mut libc::c_int,
) -> c_int {
    if mask.is_null() || out.is_null() || out_errno.is_null() {
        return -2;
    }
    let result = catch_unwind(AssertUnwindSafe(|| unsafe {
        let mask = CStr::from_ptr(mask).to_bytes();
        let last = (!last.is_null()).then(|| CStr::from_ptr(last).to_bytes());
        let duplicate = crate::hostio_ffi::duplicate(fd);
        if duplicate < 0 {
            *out_errno = *libc::__errno_location();
            return Err(());
        }
        let stream = crate::hostio_ffi::dir_open(duplicate);
        if stream.is_null() {
            let error = *libc::__errno_location();
            let _ = crate::hostio_ffi::close(duplicate);
            *out_errno = error;
            return Err(());
        }
        struct Stream(*mut c_void);
        impl Drop for Stream {
            fn drop(&mut self) {
                unsafe { crate::hostio_ffi::dir_close(self.0) };
            }
        }
        let _stream = Stream(stream);
        libc::rewinddir(stream.cast());
        let scan_fd = libc::dirfd(stream.cast());
        let mut selection = Selection::new(mask, last);
        let mut best: Option<(usize, DirEntry)> = None;
        let mut index = 0usize;
        loop {
            let item = crate::hostio_ffi::dir_next(stream);
            if item.is_null() {
                break;
            }
            let item = &*item.cast::<libc::dirent>();
            let name = CStr::from_ptr(item.d_name.as_ptr()).to_bytes();
            let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
            let stat_ok = crate::hostio_ffi::metadata_at(
                scan_fd,
                item.d_name.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            ) == 0;
            let info = stat_ok.then(|| stat.assume_init());
            let kind = match info.as_ref().map(|s| s.st_mode & libc::S_IFMT) {
                Some(mode) if mode == libc::S_IFREG => HostKind::Regular,
                Some(mode) if mode == libc::S_IFDIR => HostKind::Directory,
                _ => HostKind::Other,
            };
            if stat_ok && admit_host_entry(name, kind) && selection.offer(index, name) {
                let info = info.expect("stat_ok has metadata");
                let converted = tsfp_engine_fileio::lifecycle::guest_metadata(
                    tsfp_engine_fileio::lifecycle::HostMetadata {
                        directory: kind == HostKind::Directory,
                        size: info.st_size,
                        access_seconds: info.st_atime,
                        access_nanoseconds: info.st_atime_nsec,
                        write_seconds: info.st_mtime,
                        write_nanoseconds: info.st_mtime_nsec,
                    },
                );
                let mut candidate = DirEntry {
                    name: [0; 256],
                    is_directory: converted.directory,
                    size: converted.size,
                    creation_time: converted.creation,
                    last_access_time: converted.access,
                    last_write_time: converted.write,
                    image_backed: false,
                    attributes: 0,
                    allocation: 0,
                };
                candidate.name[..name.len()].copy_from_slice(name);
                best = Some((index, candidate));
            }
            index = index.saturating_add(1);
        }
        Ok((
            selection.finish(max_name as usize),
            best.map(|(_, entry)| entry),
        ))
    }));
    crate::counters::bump(crate::counters::DIR_PICK);
    match result {
        Ok(Ok((Pick::Found(_), Some(entry)))) => {
            unsafe { *out = entry };
            1
        }
        Ok(Ok((Pick::None, _))) => 0,
        Ok(Ok((Pick::TooLong, _))) => -1,
        Ok(Err(())) => -3,
        _ => -2,
    }
}

/// T2225: start or continue a listing over the C-stored cursor flags; on a latch the mask (NUL-terminated) is copied into
/// `mask_out` (at least 256 bytes). Returns SUCCESS or INVALID_PARAMETER (mask too long, cursor left unstarted).
unsafe extern "C" fn c_begin(
    host_directory: bool,
    restart: bool,
    cursor_id: u32,
    has_last: *mut bool,
    mask: *const c_char,
    mask_out: *mut c_char,
    last_out: *mut c_char,
) -> u32 {
    if has_last.is_null() || mask_out.is_null() || last_out.is_null() {
        return status::INVALID_PARAMETER;
    }
    let Ok(mut table) = cursors().lock() else {
        return status::INVALID_PARAMETER;
    };
    let state = table.entry(cursor_id).or_default();
    // Continuing listings ignore the argument entirely, as the C oracle does.
    let mask = if (state.started && !restart) || mask.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(mask) }.to_bytes())
    };
    let decision = state.begin(host_directory, restart, mask);
    let write = |dst: *mut c_char, bytes: &[u8]| -> bool {
        if bytes.len() > 255 {
            return false;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), dst.cast::<u8>(), bytes.len());
            *dst.cast::<u8>().add(bytes.len()) = 0;
        }
        true
    };
    if !write(mask_out, &state.mask) || !write(last_out, state.last.as_deref().unwrap_or_default())
    {
        return status::INVALID_PARAMETER;
    }
    unsafe {
        *has_last = state.last.is_some();
    }
    if decision == Begin::Refused {
        status::INVALID_PARAMETER
    } else {
        status::SUCCESS
    }
}

unsafe extern "C" fn c_finish(
    cursor_id: u32,
    picked: c_int,
    name: *const c_char,
    status_out: *mut u32,
) -> bool {
    if status_out.is_null() {
        return false;
    }
    let Ok(mut table) = cursors().lock() else {
        return false;
    };
    let Some(state) = table.get_mut(&cursor_id) else {
        return false;
    };
    let name = if picked == 1 && !name.is_null() {
        Some(unsafe { CStr::from_ptr(name) }.to_bytes())
    } else {
        None
    };
    let result = state.finish(picked, name);
    unsafe {
        *status_out = result;
    }
    result == status::SUCCESS
}

extern "C" fn c_reset(cursor_id: u32) {
    if let Ok(mut table) = cursors().lock() {
        table.remove(&cursor_id);
    }
}

#[repr(C)]
pub struct DirBackend {
    pick_fatx: unsafe extern "C" fn(
        *const c_void,
        *const c_char,
        *const c_char,
        *const c_char,
        u32,
        *mut DirEntry,
        *mut u32,
    ) -> c_int,
    pick_host: unsafe extern "C" fn(
        c_int,
        *const c_char,
        *const c_char,
        u32,
        *mut DirEntry,
        *mut c_int,
    ) -> c_int,
    begin: unsafe extern "C" fn(
        bool,
        bool,
        u32,
        *mut bool,
        *const c_char,
        *mut c_char,
        *mut c_char,
    ) -> u32,
    finish: unsafe extern "C" fn(u32, c_int, *const c_char, *mut u32) -> bool,
    reset: extern "C" fn(u32),
}
static BACKEND: DirBackend = DirBackend {
    pick_fatx: c_pick_fatx,
    pick_host: c_pick_host,
    begin: c_begin,
    finish: c_finish,
    reset: c_reset,
};

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_dir_backend() -> *const DirBackend {
    &BACKEND
}
