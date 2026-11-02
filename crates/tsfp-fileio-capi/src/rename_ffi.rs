//! Borrowed primitive rename services and owned path snapshots.
use crate::commit_ffi::Rename;
use std::ffi::{CStr, c_char, c_void};
use tsfp_engine_fileio::commit::{
    self, RenameCommitHost, RenameFacts, RenamePaths, RenameScope, SlotPaths,
};
#[repr(C)]
pub struct Slot {
    backing: u32,
    volume: u32,
    guest: [u8; 261],
    storage: [u8; 261],
}
#[repr(C)]
pub struct Callbacks {
    context: *mut c_void,
    move_name: unsafe extern "C" fn(*mut c_void, bool) -> u32,
    slot: unsafe extern "C" fn(*mut c_void, usize, *mut Slot) -> bool,
    set_path: unsafe extern "C" fn(*mut c_void, usize, bool, *const c_char),
    too_long: unsafe extern "C" fn(*mut c_void, *const c_char),
    flush: unsafe extern "C" fn(*mut c_void),
    completed: unsafe extern "C" fn(*mut c_void, bool),
    close: unsafe extern "C" fn(*mut c_void),
    count: usize,
}
struct Host<'a>(&'a Callbacks);
fn bytes(array: &[u8]) -> Vec<u8> {
    array[..array.iter().position(|&c| c == 0).unwrap_or(array.len())].to_vec()
}
fn terminated(path: &[u8]) -> Vec<u8> {
    let mut out = path.to_vec();
    out.push(0);
    out
}
impl RenameCommitHost for Host<'_> {
    fn move_name(&mut self, replace: bool) -> Result<(), u32> {
        let status = unsafe { (self.0.move_name)(self.0.context, replace) };
        if status == 0 { Ok(()) } else { Err(status) }
    }
    fn slot_count(&self) -> usize {
        self.0.count
    }
    fn slot(&mut self, index: usize) -> Option<SlotPaths> {
        let mut slot = Slot {
            backing: 0,
            volume: 0,
            guest: [0; 261],
            storage: [0; 261],
        };
        if !unsafe { (self.0.slot)(self.0.context, index, &mut slot) } {
            return None;
        }
        let scope = match slot.backing {
            1 => RenameScope::Fatx {
                volume: slot.volume,
            },
            2 => RenameScope::HostDirectory,
            _ => return None,
        };
        Some(SlotPaths {
            scope,
            guest: bytes(&slot.guest),
            storage: bytes(&slot.storage),
        })
    }
    fn set_path(&mut self, index: usize, storage: bool, path: &[u8]) {
        unsafe {
            (self.0.set_path)(
                self.0.context,
                index,
                storage,
                terminated(path).as_ptr().cast(),
            )
        };
    }
    fn too_long(&mut self, path: &[u8]) {
        unsafe { (self.0.too_long)(self.0.context, terminated(path).as_ptr().cast()) };
    }
    fn flush(&mut self) {
        unsafe { (self.0.flush)(self.0.context) };
    }
    fn completed(&mut self, replace: bool) {
        unsafe { (self.0.completed)(self.0.context, replace) };
    }
    fn close_parents(&mut self) {
        unsafe { (self.0.close)(self.0.context) };
    }
}
#[repr(C)]
pub struct Paths {
    backing: u32,
    volume: u32,
    old: *const c_char,
    new: *const c_char,
    old_storage: *const c_char,
    new_storage: *const c_char,
}
pub unsafe extern "C" fn execute(
    host: *const Callbacks,
    facts: *const Rename,
    paths: *const Paths,
) -> u32 {
    if host.is_null() || facts.is_null() || paths.is_null() {
        return commit::STATUS_INVALID_PARAMETER;
    }
    crate::counters::bump(crate::counters::RENAME_COMMIT);
    let f = unsafe { &*facts };
    let p = unsafe { &*paths };
    if [p.old, p.new, p.old_storage, p.new_storage]
        .iter()
        .any(|p| p.is_null())
    {
        return commit::STATUS_INVALID_PARAMETER;
    }
    let scope = match p.backing {
        1 => RenameScope::Fatx { volume: p.volume },
        2 => RenameScope::HostDirectory,
        _ => return commit::STATUS_INVALID_PARAMETER,
    };
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
    // Own inputs before callbacks can mutate any slot's source path.
    let old = unsafe { CStr::from_ptr(p.old) }.to_bytes().to_vec();
    let new = unsafe { CStr::from_ptr(p.new) }.to_bytes().to_vec();
    let old_storage = unsafe { CStr::from_ptr(p.old_storage) }.to_bytes().to_vec();
    let new_storage = unsafe { CStr::from_ptr(p.new_storage) }.to_bytes().to_vec();
    commit::rename_commit(
        &mut Host(unsafe { &*host }),
        &facts,
        &RenamePaths {
            scope,
            old: &old,
            new: &new,
            old_storage: &old_storage,
            new_storage: &new_storage,
            capacity: 261,
        },
    )
}
