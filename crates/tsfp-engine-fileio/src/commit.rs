//! Create, delete and rename commit decisions (T2103, rule 16 S1). `kernel_file.c` keeps the host side effects (the
//! volume walk, `mkdir`/`openat`/`unlinkat`, the FATX writes); these tables say which of them a call takes.
//!
//! - [`create_plan`]: the NtCreateFile creation arm (disposition plan `TryCreate`), ported from `kernel_file.c`.
//! - [`delete_plan`]: the NtDeleteFile refusal ladder (`hostdir_delete_locked`), ported.
//! - [`delete_on_close_at_open`], [`set_disposition`], [`close_commits_delete`]: delete-on-close and
//!   FileDispositionInformation. Spec from the NT semantics, INFERRED; kernel_file.c calls it since T2105.
//! - [`rename_plan`]: FileRenameInformation. No C implementation: spec from the NT semantics, INFERRED.
//! - [`rebase_path`]: the open-slot path rewrite after a rename moves a directory (T2113, INFERRED).

pub const FILE_DIRECTORY_FILE: u32 = 0x0000_0001;
pub const FILE_NON_DIRECTORY_FILE: u32 = 0x0000_0040;
pub const FILE_DELETE_ON_CLOSE: u32 = 0x0000_1000;
pub const DELETE: u32 = 0x0001_0000;

pub const STATUS_INVALID_PARAMETER: u32 = 0xC000_000D;
pub const STATUS_ACCESS_DENIED: u32 = 0xC000_0022;
pub const STATUS_OBJECT_NAME_COLLISION: u32 = 0xC000_0035;
pub const STATUS_SHARING_VIOLATION: u32 = 0xC000_0043;
pub const STATUS_DELETE_PENDING: u32 = 0xC000_0056;
pub const STATUS_CANNOT_DELETE: u32 = 0xC000_0121;
pub const STATUS_DIRECTORY_NOT_EMPTY: u32 = 0xC000_0101;
pub const STATUS_NOT_SAME_DEVICE: u32 = 0xC000_00D4;

/// Backing of the volume a missing name sits on, as the second volume lookup found it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreateVolume {
    None,
    Fatx,
    HostDir,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CreatePlan {
    /// No writable volume behind the name: the resolution's status stands.
    Refuse,
    FatxMkdir,
    FatxFile,
    /// Both directory bits on a FATX volume: FATX_E_ARGUMENT.
    FatxArgument,
    HostMkdir,
    HostFile,
    /// Both directory bits on a host-directory volume: INVALID_PARAMETER.
    HostInvalid,
}

/// Which creation the `TryCreate` arm runs. Neither directory bit means a file (NT's default).
pub fn create_plan(volume: CreateVolume, create_options: u32) -> CreatePlan {
    let directory = create_options & FILE_DIRECTORY_FILE != 0;
    let contradicts = directory && create_options & FILE_NON_DIRECTORY_FILE != 0;
    match volume {
        CreateVolume::None => CreatePlan::Refuse,
        CreateVolume::Fatx if contradicts => CreatePlan::FatxArgument,
        CreateVolume::Fatx if directory => CreatePlan::FatxMkdir,
        CreateVolume::Fatx => CreatePlan::FatxFile,
        CreateVolume::HostDir if contradicts => CreatePlan::HostInvalid,
        CreateVolume::HostDir if directory => CreatePlan::HostMkdir,
        CreateVolume::HostDir => CreatePlan::HostFile,
    }
}

/// What the NtDeleteFile walk found. Facts past the first refusal are not gathered and stay false.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeleteFacts {
    pub raw_view: bool,
    pub volume_root: bool,
    pub found: bool,
    pub is_directory: bool,
    pub is_link: bool,
    pub is_regular: bool,
    pub held_open: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeletePlan {
    Unlink,
    RefuseRawDevice,
    RefuseRoot,
    NotFound,
    RefuseDirectory,
    RefuseLink,
    RefuseNotFile,
    Sharing,
}

/// The `hostdir_delete_locked` ladder, in its order.
pub fn delete_plan(f: &DeleteFacts) -> DeletePlan {
    if f.raw_view {
        DeletePlan::RefuseRawDevice
    } else if f.volume_root {
        DeletePlan::RefuseRoot
    } else if !f.found {
        DeletePlan::NotFound
    } else if f.is_directory {
        DeletePlan::RefuseDirectory
    } else if f.is_link {
        DeletePlan::RefuseLink
    } else if !f.is_regular {
        DeletePlan::RefuseNotFile
    } else if f.held_open {
        DeletePlan::Sharing
    } else {
        DeletePlan::Unlink
    }
}

/// FILE_DELETE_ON_CLOSE at open (INFERRED, NT IoCreateFile): it needs DELETE access, and on a directory-only open the
/// later delete still needs the directory empty, checked at close. Returns the refusal status, or None to accept.
pub fn delete_on_close_at_open(create_options: u32, desired_access: u32) -> Option<u32> {
    (create_options & FILE_DELETE_ON_CLOSE != 0 && desired_access & DELETE == 0).then_some(STATUS_INVALID_PARAMETER)
}

/// The object a FileDispositionInformation or a close-time delete is aimed at.
#[derive(Clone, Copy, Debug, Default)]
pub struct DeleteTarget {
    pub granted_delete: bool,
    pub read_only: bool,
    pub is_directory: bool,
    pub directory_empty: bool,
    pub volume_root: bool,
    pub device: bool,
    pub writable_volume: bool,
}

/// NtSetInformationFile(FileDispositionInformation) (INFERRED, NT/FATX semantics): returns the new delete-pending
/// state, or the refusal status. Clearing the flag always succeeds on a handle with DELETE access.
pub fn set_disposition(delete_file: bool, t: &DeleteTarget) -> Result<bool, u32> {
    if !t.granted_delete {
        return Err(STATUS_ACCESS_DENIED);
    }
    if !delete_file {
        return Ok(false);
    }
    if t.volume_root || t.device || !t.writable_volume {
        return Err(STATUS_ACCESS_DENIED);
    }
    if t.read_only {
        return Err(STATUS_CANNOT_DELETE);
    }
    if t.is_directory && !t.directory_empty {
        return Err(STATUS_DIRECTORY_NOT_EMPTY);
    }
    Ok(true)
}

/// Whether closing this handle removes the name (INFERRED): the last handle of a delete-pending object.
pub fn close_commits_delete(delete_pending: bool, other_live_handles: u32) -> bool {
    delete_pending && other_live_handles == 0
}

/// FileRenameInformation as seen by the host.
#[derive(Clone, Copy, Debug, Default)]
pub struct RenameFacts {
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
    /// Source and target name the same object (a case-only rename on a case-insensitive volume).
    pub same_object: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenamePlan {
    /// Move the name, nothing at the target.
    Rename,
    /// Remove the target file, then move the name.
    Replace,
    Refuse(u32),
}

/// NtSetInformationFile(FileRenameInformation) (INFERRED, NT semantics, no C implementation to port; hooked into kernel_file.c by T2105).
pub fn rename_plan(f: &RenameFacts) -> RenamePlan {
    const PATH_NOT_FOUND: u32 = 0xC000_003A;
    if !f.granted_delete || f.source_is_root || !f.writable_volume {
        return RenamePlan::Refuse(STATUS_ACCESS_DENIED);
    }
    if f.source_delete_pending {
        return RenamePlan::Refuse(STATUS_DELETE_PENDING);
    }
    if !f.same_volume {
        return RenamePlan::Refuse(STATUS_NOT_SAME_DEVICE);
    }
    if !f.target_parent_exists {
        return RenamePlan::Refuse(PATH_NOT_FOUND);
    }
    if !f.target_exists || f.same_object {
        return RenamePlan::Rename;
    }
    if !f.replace_if_exists {
        return RenamePlan::Refuse(STATUS_OBJECT_NAME_COLLISION);
    }
    if f.target_is_directory {
        return RenamePlan::Refuse(STATUS_ACCESS_DENIED);
    }
    if f.target_held_open {
        return RenamePlan::Refuse(STATUS_SHARING_VIOLATION);
    }
    RenamePlan::Replace
}

/// Path of an open handle after a rename moved `old` to `new` (T2113, INFERRED NT semantics: open children of a renamed
/// directory follow it). `None` when `path` is neither `old` nor below it. ASCII case-insensitive like the Xbox name
/// compare, either `\\` or `/` separates the components; the remainder keeps its own spelling.
pub fn rebase_path(path: &[u8], old: &[u8], new: &[u8]) -> Option<Vec<u8>> {
    if old.is_empty() || path.len() < old.len() || !path[..old.len()].eq_ignore_ascii_case(old) {
        return None;
    }
    let rest = &path[old.len()..];
    if !rest.is_empty() && rest[0] != b'\\' && rest[0] != b'/' {
        return None;
    }
    let mut out = new.to_vec();
    out.extend_from_slice(rest);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_create_sites() {
        // Both measured NtCreateFile creation sites pass options 0x4021 (FILE_DIRECTORY_FILE set).
        assert_eq!(create_plan(CreateVolume::HostDir, 0x4021), CreatePlan::HostMkdir);
        assert_eq!(create_plan(CreateVolume::Fatx, 0x4021), CreatePlan::FatxMkdir);
        assert_eq!(create_plan(CreateVolume::HostDir, 0), CreatePlan::HostFile);
        assert_eq!(create_plan(CreateVolume::HostDir, 0x41), CreatePlan::HostInvalid);
        assert_eq!(create_plan(CreateVolume::Fatx, 0x41), CreatePlan::FatxArgument);
        assert_eq!(create_plan(CreateVolume::None, 1), CreatePlan::Refuse);
    }

    #[test]
    fn delete_ladder_order() {
        let file = DeleteFacts { found: true, is_regular: true, ..DeleteFacts::default() };
        assert_eq!(delete_plan(&file), DeletePlan::Unlink);
        assert_eq!(delete_plan(&DeleteFacts { held_open: true, ..file }), DeletePlan::Sharing);
        assert_eq!(delete_plan(&DeleteFacts { is_link: true, ..file }), DeletePlan::RefuseLink);
        assert_eq!(delete_plan(&DeleteFacts { raw_view: true, ..file }), DeletePlan::RefuseRawDevice);
        assert_eq!(delete_plan(&DeleteFacts::default()), DeletePlan::NotFound);
    }

    #[test]
    fn delete_on_close_and_disposition() {
        assert_eq!(delete_on_close_at_open(FILE_DELETE_ON_CLOSE, 0), Some(STATUS_INVALID_PARAMETER));
        assert_eq!(delete_on_close_at_open(FILE_DELETE_ON_CLOSE, DELETE), None);
        let file = DeleteTarget { granted_delete: true, writable_volume: true, ..DeleteTarget::default() };
        assert_eq!(set_disposition(true, &file), Ok(true));
        assert_eq!(set_disposition(false, &file), Ok(false));
        assert_eq!(set_disposition(true, &DeleteTarget { read_only: true, ..file }), Err(STATUS_CANNOT_DELETE));
        assert_eq!(set_disposition(true, &DeleteTarget { is_directory: true, ..file }), Err(STATUS_DIRECTORY_NOT_EMPTY));
        assert_eq!(set_disposition(true, &DeleteTarget { granted_delete: false, ..file }), Err(STATUS_ACCESS_DENIED));
        assert!(close_commits_delete(true, 0));
        assert!(!close_commits_delete(true, 1));
    }

    #[test]
    fn rename_rules() {
        let base = RenameFacts {
            granted_delete: true, same_volume: true, writable_volume: true, target_parent_exists: true,
            ..RenameFacts::default()
        };
        assert_eq!(rename_plan(&base), RenamePlan::Rename);
        let taken = RenameFacts { target_exists: true, ..base };
        assert_eq!(rename_plan(&taken), RenamePlan::Refuse(STATUS_OBJECT_NAME_COLLISION));
        assert_eq!(rename_plan(&RenameFacts { replace_if_exists: true, ..taken }), RenamePlan::Replace);
        assert_eq!(rename_plan(&RenameFacts { same_object: true, ..taken }), RenamePlan::Rename);
        assert_eq!(rename_plan(&RenameFacts { replace_if_exists: true, target_is_directory: true, ..taken }),
                   RenamePlan::Refuse(STATUS_ACCESS_DENIED));
        assert_eq!(rename_plan(&RenameFacts { same_volume: false, ..base }), RenamePlan::Refuse(STATUS_NOT_SAME_DEVICE));
    }

    #[test]
    fn rebase_path_moves_children_only() {
        let r = |p: &str| rebase_path(p.as_bytes(), b"T:\\Save", b"T:\\Old").map(|v| String::from_utf8(v).unwrap());
        assert_eq!(r("T:\\Save").as_deref(), Some("T:\\Old"));
        assert_eq!(r("t:\\SAVE\\a.dat").as_deref(), Some("T:\\Old\\a.dat"));
        assert_eq!(r("T:\\Save/x").as_deref(), Some("T:\\Old/x"));
        assert_eq!(r("T:\\Saves\\a"), None);
        assert_eq!(r("T:\\Sa"), None);
        assert_eq!(rebase_path(b"x", b"", b"y"), None);
    }
}

/// Typed rename transaction boundary. Slot snapshots are owned bytes, never guest offsets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenameScope {
    HostDirectory,
    Fatx { volume: u32 },
}
pub struct SlotPaths {
    pub scope: RenameScope,
    pub guest: Vec<u8>,
    pub storage: Vec<u8>,
}
pub trait RenameCommitHost {
    fn move_name(&mut self, replace: bool) -> Result<(), u32>;
    fn slot_count(&self) -> usize;
    fn slot(&mut self, index: usize) -> Option<SlotPaths>;
    fn set_path(&mut self, index: usize, storage: bool, path: &[u8]);
    fn too_long(&mut self, path: &[u8]);
    fn flush(&mut self);
    fn completed(&mut self, replace: bool);
    fn close_parents(&mut self);
}
pub struct RenamePaths<'a> {
    pub scope: RenameScope,
    pub old: &'a [u8],
    pub new: &'a [u8],
    pub old_storage: &'a [u8],
    pub new_storage: &'a [u8],
    pub capacity: usize,
}
pub fn rename_commit(
    host: &mut impl RenameCommitHost,
    facts: &RenameFacts,
    paths: &RenamePaths<'_>,
) -> u32 {
    let status = match rename_plan(facts) {
        RenamePlan::Refuse(status) => status,
        plan => {
            let replace = plan == RenamePlan::Replace;
            match host.move_name(replace) {
                Err(status) => status,
                Ok(()) => {
                    for index in 0..host.slot_count() {
                        let Some(slot) = host.slot(index) else {
                            continue;
                        };
                        if slot.scope != paths.scope {
                            continue;
                        }
                        if let Some(rebased) = rebase_path(&slot.guest, paths.old, paths.new) {
                            if rebased.len() >= paths.capacity {
                                host.too_long(&slot.guest);
                            } else {
                                host.set_path(index, false, &rebased);
                            }
                        }
                        if matches!(paths.scope, RenameScope::Fatx { .. }) {
                            if let Some(rebased) =
                                rebase_path(&slot.storage, paths.old_storage, paths.new_storage)
                            {
                                if rebased.len() >= paths.capacity {
                                    host.too_long(&slot.storage);
                                } else {
                                    host.set_path(index, true, &rebased);
                                }
                            }
                        }
                    }
                    if matches!(paths.scope, RenameScope::Fatx { .. }) {
                        host.flush();
                    }
                    host.completed(replace);
                    0
                }
            }
        }
    };
    host.close_parents();
    status
}
