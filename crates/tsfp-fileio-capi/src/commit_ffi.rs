//! C ABI of the Rust create/delete commit decisions (T2103). `kernel_file.c` keeps the side effects; with `--rust-fileio`
//! the NtCreateFile creation arm and the NtDeleteFile ladder take their plan from here (`kernel_file_commit_backend`).
use std::ffi::{CStr, c_char, c_int};
use tsfp_engine_fileio::commit::{self, CreatePlan, CreateVolume, DeleteFacts, DeletePlan, DeleteTarget, RenameFacts, RenamePlan};

/// Mirrors `kernel_file_delete_facts` in src/xbox/kernel_file.h.
#[repr(C)]
pub struct Facts {
    raw_view: bool,
    volume_root: bool,
    found: bool,
    is_directory: bool,
    is_link: bool,
    is_regular: bool,
    held_open: bool,
}

// Mirrors KERNEL_FILE_CREATE_* (volume 0 none, 1 FATX, 2 host dir) and KERNEL_FILE_DELETE_* plan codes.
unsafe extern "C" fn c_create(volume: c_int, create_options: u32) -> c_int {
    crate::counters::bump(crate::counters::COMMIT);
    let volume = match volume {
        1 => CreateVolume::Fatx,
        2 => CreateVolume::HostDir,
        _ => CreateVolume::None,
    };
    match commit::create_plan(volume, create_options) {
        CreatePlan::Refuse => 0,
        CreatePlan::FatxMkdir => 1,
        CreatePlan::FatxFile => 2,
        CreatePlan::FatxArgument => 3,
        CreatePlan::HostMkdir => 4,
        CreatePlan::HostFile => 5,
        CreatePlan::HostInvalid => 6,
    }
}

unsafe extern "C" fn c_delete(facts: *const Facts) -> c_int {
    if facts.is_null() {
        return 3;
    }
    crate::counters::bump(crate::counters::COMMIT);
    let f = unsafe { &*facts };
    let facts = DeleteFacts {
        raw_view: f.raw_view,
        volume_root: f.volume_root,
        found: f.found,
        is_directory: f.is_directory,
        is_link: f.is_link,
        is_regular: f.is_regular,
        held_open: f.held_open,
    };
    match commit::delete_plan(&facts) {
        DeletePlan::Unlink => 0,
        DeletePlan::RefuseRawDevice => 1,
        DeletePlan::RefuseRoot => 2,
        DeletePlan::NotFound => 3,
        DeletePlan::RefuseDirectory => 4,
        DeletePlan::RefuseLink => 5,
        DeletePlan::RefuseNotFile => 6,
        DeletePlan::Sharing => 7,
    }
}

/// Mirrors `kernel_file_delete_target` (T2105).
#[repr(C)]
pub struct Target {
    granted_delete: bool,
    read_only: bool,
    is_directory: bool,
    directory_empty: bool,
    volume_root: bool,
    device: bool,
    writable_volume: bool,
}

/// Mirrors `kernel_file_rename_facts` (T2105).
#[repr(C)]
pub struct Rename {
    pub granted_delete: bool,
    pub replace_if_exists: bool,
    pub same_volume: bool,
    pub writable_volume: bool,
    pub source_is_root: bool,
    pub source_delete_pending: bool,
    pub target_parent_exists: bool,
    pub target_exists: bool,
    pub target_is_directory: bool,
    pub target_held_open: bool,
    pub same_object: bool,
}

// FileDispositionInformation: the new delete-pending state, or -1 with *status (INFERRED spec, commit.rs).
unsafe extern "C" fn c_disposition(delete_file: c_int, target: *const Target, status: *mut u32) -> c_int {
    if target.is_null() || status.is_null() {
        return -1;
    }
    crate::counters::bump(crate::counters::COMMIT);
    let t = unsafe { &*target };
    let target = DeleteTarget {
        granted_delete: t.granted_delete,
        read_only: t.read_only,
        is_directory: t.is_directory,
        directory_empty: t.directory_empty,
        volume_root: t.volume_root,
        device: t.device,
        writable_volume: t.writable_volume,
    };
    match commit::set_disposition(delete_file != 0, &target) {
        Ok(pending) => pending as c_int,
        Err(code) => {
            unsafe { *status = code };
            -1
        }
    }
}

unsafe extern "C" fn c_close_commits(delete_pending: c_int, others: u32) -> c_int {
    crate::counters::bump(crate::counters::COMMIT);
    commit::close_commits_delete(delete_pending != 0, others) as c_int
}

// KERNEL_FILE_RENAME_* plan codes: 0 move, 1 replace, 2 refuse with *status.
unsafe extern "C" fn c_rename(facts: *const Rename, status: *mut u32) -> c_int {
    if facts.is_null() || status.is_null() {
        return 2;
    }
    crate::counters::bump(crate::counters::COMMIT);
    let f = unsafe { &*facts };
    let facts = RenameFacts {
        granted_delete: f.granted_delete,
        replace_if_exists: f.replace_if_exists,
        same_volume: f.same_volume,
        writable_volume: f.writable_volume,
        source_is_root: f.source_is_root,
        source_delete_pending: f.source_delete_pending,
        target_parent_exists: f.target_parent_exists,
        target_exists: f.target_exists,
        target_is_directory: f.target_is_directory,
        target_held_open: f.target_held_open,
        same_object: f.same_object,
    };
    match commit::rename_plan(&facts) {
        RenamePlan::Rename => 0,
        RenamePlan::Replace => 1,
        RenamePlan::Refuse(code) => {
            unsafe { *status = code };
            2
        }
    }
}

// T2113: 1 rebased into out (NUL-terminated), 0 not affected, -1 the result does not fit in cap.
unsafe extern "C" fn c_rebase_path(path: *const c_char, old: *const c_char, new: *const c_char, out: *mut c_char,
                                   cap: usize) -> c_int {
    if path.is_null() || old.is_null() || new.is_null() || out.is_null() {
        return 0;
    }
    crate::counters::bump(crate::counters::COMMIT);
    let (path, old, new) = unsafe { (CStr::from_ptr(path), CStr::from_ptr(old), CStr::from_ptr(new)) };
    match commit::rebase_path(path.to_bytes(), old.to_bytes(), new.to_bytes()) {
        None => 0,
        Some(v) if v.len() >= cap => -1,
        Some(v) => {
            unsafe {
                std::ptr::copy_nonoverlapping(v.as_ptr(), out as *mut u8, v.len());
                *out.add(v.len()) = 0;
            }
            1
        }
    }
}

#[repr(C)]
pub struct CommitBackend {
    create: unsafe extern "C" fn(c_int, u32) -> c_int,
    delete: unsafe extern "C" fn(*const Facts) -> c_int,
    disposition: unsafe extern "C" fn(c_int, *const Target, *mut u32) -> c_int,
    close_commits: unsafe extern "C" fn(c_int, u32) -> c_int,
    rename: unsafe extern "C" fn(*const Rename, *mut u32) -> c_int,
    rebase_path: unsafe extern "C" fn(*const c_char, *const c_char, *const c_char, *mut c_char, usize) -> c_int,
}
static BACKEND: CommitBackend = CommitBackend {
    create: c_create,
    delete: c_delete,
    disposition: c_disposition,
    close_commits: c_close_commits,
    rename: c_rename,
    rebase_path: c_rebase_path,
};

#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_commit_backend() -> *const CommitBackend {
    &BACKEND
}
