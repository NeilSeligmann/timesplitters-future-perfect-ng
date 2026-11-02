//! T2074 Linux descriptor seam. Borrowed descriptors retain C ownership and errno.
use libc::{c_char, c_int, c_void, size_t, ssize_t};
#[repr(C)]
pub struct Backend {
    pub open_at: unsafe extern "C" fn(c_int, *const c_char, c_int, u32) -> c_int,
    pub close: unsafe extern "C" fn(c_int) -> c_int,
    pub read_at: unsafe extern "C" fn(c_int, *mut c_void, size_t, i64) -> ssize_t,
    pub allocate: unsafe extern "C" fn(c_int, c_int, i64, i64) -> c_int,
    pub metadata: unsafe extern "C" fn(c_int, *mut libc::stat) -> c_int,
    pub metadata_at: unsafe extern "C" fn(c_int, *const c_char, *mut libc::stat, c_int) -> c_int,
    pub space: unsafe extern "C" fn(c_int, *mut libc::statvfs) -> c_int,
    pub dir_open: unsafe extern "C" fn(c_int) -> *mut c_void,
    pub dir_next: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    pub dir_close: unsafe extern "C" fn(*mut c_void) -> c_int,
    pub read_name: unsafe extern "C" fn(*const NameHost, u32, *mut u8, usize) -> bool,
    pub rename_guest: unsafe extern "C" fn(*const NameHost, u32, u32, u32) -> u32,
    pub duplicate: unsafe extern "C" fn(c_int) -> c_int,
    pub make_dir: unsafe extern "C" fn(c_int, *const c_char, u32) -> c_int,
    pub remove_at: unsafe extern "C" fn(c_int, *const c_char, c_int) -> c_int,
    pub move_at: unsafe extern "C" fn(c_int, *const c_char, c_int, *const c_char) -> c_int,
    pub acquire: unsafe extern "C" fn(*const AcquireCallbacks, *mut u32) -> u32,
    pub resolve: unsafe extern "C" fn(*const ResolveCallbacks, bool, bool) -> u32,
    pub prepare_create: unsafe extern "C" fn(
        *const crate::create_ffi::Callbacks,
        u32,
        *const crate::disposition_ffi::Facts,
        u32,
        *mut crate::create_ffi::Outcome,
    ) -> bool,
    pub commit_rename: unsafe extern "C" fn(
        *const crate::rename_ffi::Callbacks,
        *const crate::commit_ffi::Rename,
        *const crate::rename_ffi::Paths,
    ) -> u32,
    pub mount_slot:
        unsafe extern "C" fn(*const crate::lifecycle_ffi::Callbacks, *const c_char) -> i32,
    pub volume_count: unsafe extern "C" fn(*const crate::lifecycle_ffi::Callbacks) -> u32,
    pub unmount_all: unsafe extern "C" fn(*const crate::lifecycle_ffi::Callbacks),
    pub unmount_fatx:
        unsafe extern "C" fn(*const crate::lifecycle_ffi::Callbacks, *const c_char) -> bool,
    pub acquire_device:
        unsafe extern "C" fn(*const crate::lifecycle_ffi::Callbacks, *const c_char) -> bool,
    pub duplicate_cloexec: unsafe extern "C" fn(i32) -> i32,
    pub prepare_cache: unsafe extern "C" fn(*const crate::lifecycle_ffi::CacheCallbacks) -> u32,
    pub attempt_reset: unsafe extern "C" fn(),
    pub attempt_count: unsafe extern "C" fn() -> u32,
    pub attempt_record: unsafe extern "C" fn(*const c_char, u32, u32, u32) -> i32,
    pub attempt_get: unsafe extern "C" fn(u32, *mut crate::activity_ffi::Snapshot) -> bool,
    pub attempt_opened: unsafe extern "C" fn(u32),
    pub observer_set: unsafe extern "C" fn(Option<unsafe extern "C" fn(u32, *const c_char, u64)>),
    pub observer_event: unsafe extern "C" fn(u32, *const c_char, u64),
    pub observer_enabled: unsafe extern "C" fn() -> bool,
    pub prepare_device: unsafe extern "C" fn(
        *const c_char,
        *const c_char,
        u64,
        *mut crate::lifecycle_ffi::DeviceResult,
    ),
    pub filetime: unsafe extern "C" fn(i64, i64) -> u64,
    pub guest_metadata:
        unsafe extern "C" fn(*const libc::stat, *mut crate::lifecycle_ffi::GuestMetadata),
    pub space_units: unsafe extern "C" fn(u64, u64, u64, *mut u64, *mut u64) -> bool,
    pub validate_device: unsafe extern "C" fn(
        *const c_char,
        *const c_char,
        *const c_char,
        *const c_char,
        u32,
        u64,
    ) -> u32,
    pub publish_device: unsafe extern "C" fn(
        *const crate::lifecycle_ffi::PublicationTarget,
        *const c_char,
        *const c_char,
        *const c_char,
        *const c_char,
        u32,
        u64,
        i32,
    ),
    pub read_attributes:
        unsafe extern "C" fn(*const NameHost, u32, *mut u32, *mut u32, *mut u32) -> bool,
    pub openable_reset: unsafe extern "C" fn(),
    pub openable_add: unsafe extern "C" fn(*const c_char) -> bool,
    pub openable_count: unsafe extern "C" fn() -> u32,
    pub openable_contains: unsafe extern "C" fn(*const c_char) -> bool,
    pub missing_policy_set: unsafe extern "C" fn(u32),
    pub missing_policy_get: unsafe extern "C" fn() -> u32,
    pub validate_host_directory: unsafe extern "C" fn(*const c_char, *const c_char) -> u32,
    pub open_host_directory: unsafe extern "C" fn(*const c_char) -> i32,
    pub publish_host_directory: unsafe extern "C" fn(
        *const crate::lifecycle_ffi::PublicationTarget,
        *const c_char,
        *const c_char,
        i32,
    ),
    pub root_path: unsafe extern "C" fn(u32, *mut c_char, usize) -> bool,
    pub publish_disc:
        unsafe extern "C" fn(*const crate::lifecycle_ffi::PublicationTarget, *const c_char),
    pub publish_content_descriptor: unsafe extern "C" fn(usize, i32) -> bool,
    pub validate_disc_prefix: unsafe extern "C" fn(*const c_char) -> u32,
}
pub(crate) unsafe extern "C" fn open_at(
    fd: c_int,
    path: *const c_char,
    flags: c_int,
    mode: u32,
) -> c_int {
    crate::counters::bump(crate::counters::HOST_OPEN);
    unsafe { libc::openat(fd, path, flags, mode as libc::mode_t) }
}
pub(crate) unsafe extern "C" fn close(fd: c_int) -> c_int {
    crate::counters::bump(crate::counters::HOST_CLOSE);
    unsafe { libc::close(fd) }
}
unsafe extern "C" fn read_at(fd: c_int, buf: *mut c_void, len: size_t, offset: i64) -> ssize_t {
    crate::counters::bump(crate::counters::CACHE_READ);
    unsafe { libc::pread(fd, buf, len, offset) }
}
unsafe extern "C" fn allocate(fd: c_int, mode: c_int, offset: i64, len: i64) -> c_int {
    crate::counters::bump(crate::counters::HOST_ALLOCATE);
    unsafe { libc::fallocate(fd, mode, offset, len) }
}
// The OS owns stream storage. We preserve fdopendir's transfer-on-success rule
// and readdir's borrowed entry, EOF/errno and shared cursor semantics. No Rust
// allocation, UTF-8 conversion, sorting or timestamp policy is introduced.
pub(crate) unsafe extern "C" fn metadata(fd: c_int, out: *mut libc::stat) -> c_int {
    crate::counters::bump(crate::counters::HOST_METADATA);
    unsafe { libc::fstat(fd, out) }
}
pub(crate) unsafe extern "C" fn metadata_at(
    fd: c_int,
    name: *const c_char,
    out: *mut libc::stat,
    flags: c_int,
) -> c_int {
    crate::counters::bump(crate::counters::HOST_METADATA_AT);
    unsafe { libc::fstatat(fd, name, out, flags) }
}
unsafe extern "C" fn space(fd: c_int, out: *mut libc::statvfs) -> c_int {
    crate::counters::bump(crate::counters::HOST_SPACE);
    unsafe { libc::fstatvfs(fd, out) }
}
pub(crate) unsafe extern "C" fn dir_open(fd: c_int) -> *mut c_void {
    crate::counters::bump(crate::counters::DIR_OPEN);
    unsafe { libc::fdopendir(fd).cast() }
}
pub(crate) unsafe extern "C" fn dir_next(dir: *mut c_void) -> *mut c_void {
    crate::counters::bump(crate::counters::DIR_NEXT);
    unsafe { libc::readdir(dir.cast()).cast() }
}
pub(crate) unsafe extern "C" fn dir_close(dir: *mut c_void) -> c_int {
    crate::counters::bump(crate::counters::DIR_CLOSE);
    unsafe { libc::closedir(dir.cast()) }
}
#[repr(C)]
pub struct NameHost {
    byte: unsafe extern "C" fn(u32, *mut u8) -> bool,
    word: unsafe extern "C" fn(u32, *mut u32) -> bool,
    rename: unsafe extern "C" fn(u32, bool, *const c_char) -> u32,
    relative_refused: unsafe extern "C" fn(u32, *const c_char),
}
struct Names<'a>(&'a NameHost);
use tsfp_engine_fileio::guest_names::{self, Address, NameMemory, RenameHost};
impl NameMemory for Names<'_> {
    fn byte(&mut self, address: Address) -> Option<u8> {
        let mut out = 0;
        unsafe { (self.0.byte)(address.0, &mut out) }.then_some(out)
    }
    fn word(&mut self, address: Address) -> Option<u32> {
        let mut out = 0;
        unsafe { (self.0.word)(address.0, &mut out) }.then_some(out)
    }
}
impl RenameHost for Names<'_> {
    fn relative_refused(&mut self, root: u32, name: &[u8]) {
        let mut terminated = [0u8; 261];
        terminated[..name.len()].copy_from_slice(name);
        unsafe { (self.0.relative_refused)(root, terminated.as_ptr().cast()) };
    }
    fn rename(&mut self, handle: u32, replace: bool, name: &[u8]) -> u32 {
        let mut terminated = [0u8; 261];
        terminated[..name.len()].copy_from_slice(name);
        unsafe { (self.0.rename)(handle, replace, terminated.as_ptr().cast()) }
    }
}
/// T3786: `name` joined to the guest path of the live directory open `root`; `None` when `root` is no open
/// directory (pseudo roots, files, stale handles) or the name escapes or does not fit.
fn directory_root(root: u32, name: &[u8], capacity: usize) -> Option<Vec<u8>> {
    let (slot, directory) = crate::handles_ffi::open_path(root)?;
    if !crate::payload_ffi::is_directory(slot) {
        return None;
    }
    guest_names::join_directory_root(&directory, name, capacity)
}
unsafe extern "C" fn root_path(root: u32, path: *mut c_char, capacity: usize) -> bool {
    if root == 0 {
        return true;
    }
    if path.is_null() {
        return false;
    }
    let bytes = unsafe { std::ffi::CStr::from_ptr(path) }.to_bytes();
    match guest_names::root_path(root, bytes, capacity) {
        guest_names::RootPath::Unchanged => true,
        guest_names::RootPath::Refused => {
            // T3786: a live directory handle roots a relative name (XDeleteSaveGame's recursive delete).
            let Some(joined) = directory_root(root, bytes, capacity) else {
                return false;
            };
            unsafe {
                std::ptr::copy_nonoverlapping(joined.as_ptr().cast(), path, joined.len());
                *path.add(joined.len()) = 0;
            }
            true
        }
        guest_names::RootPath::DriveAbsolute => {
            let length = bytes.len();
            unsafe {
                std::ptr::copy(path, path.add(4), length + 1);
                std::ptr::copy_nonoverlapping(b"\\??\\".as_ptr().cast(), path, 4);
            }
            true
        }
    }
}
unsafe extern "C" fn read_attributes(
    host: *const NameHost,
    address: u32,
    root: *mut u32,
    name: *mut u32,
    attributes: *mut u32,
) -> bool {
    if host.is_null() {
        return false;
    }
    guest_names::read_attributes(
        &mut Names(unsafe { &*host }),
        Address(address),
        |field| match field {
            guest_names::AttributeField::RootDirectory => !root.is_null(),
            guest_names::AttributeField::ObjectName => !name.is_null(),
            guest_names::AttributeField::Attributes => !attributes.is_null(),
        },
        |field, value| {
            let output = match field {
                guest_names::AttributeField::RootDirectory => root,
                guest_names::AttributeField::ObjectName => name,
                guest_names::AttributeField::Attributes => attributes,
            };
            unsafe { output.write(value) };
        },
    )
}
unsafe extern "C" fn read_name(
    host: *const NameHost,
    address: u32,
    out: *mut u8,
    len: usize,
) -> bool {
    crate::counters::bump(crate::counters::GUEST_NAME);
    if host.is_null() || out.is_null() || len > isize::MAX as usize {
        return false;
    }
    let out = unsafe { std::slice::from_raw_parts_mut(out, len) };
    guest_names::read_name(&mut Names(unsafe { &*host }), Address(address), out)
}
unsafe extern "C" fn rename_guest(
    host: *const NameHost,
    handle: u32,
    address: u32,
    len: u32,
) -> u32 {
    crate::counters::bump(crate::counters::GUEST_RENAME);
    if host.is_null() {
        return guest_names::INVALID_PARAMETER;
    }
    guest_names::rename_from_guest(&mut Names(unsafe { &*host }), handle, Address(address), len)
}
pub(crate) unsafe extern "C" fn duplicate(fd: c_int) -> c_int {
    crate::counters::bump(crate::counters::HOST_DUP);
    unsafe { libc::dup(fd) }
}
unsafe extern "C" fn make_dir(fd: c_int, name: *const c_char, mode: u32) -> c_int {
    crate::counters::bump(crate::counters::HOST_MKDIR);
    unsafe { libc::mkdirat(fd, name, mode as libc::mode_t) }
}
unsafe extern "C" fn remove_at(fd: c_int, name: *const c_char, flags: c_int) -> c_int {
    crate::counters::bump(crate::counters::HOST_UNLINK);
    unsafe { libc::unlinkat(fd, name, flags) }
}
unsafe extern "C" fn move_at(
    from_fd: c_int,
    from: *const c_char,
    to_fd: c_int,
    to: *const c_char,
) -> c_int {
    crate::counters::bump(crate::counters::HOST_RENAME);
    unsafe { libc::renameat(from_fd, from, to_fd, to) }
}
#[repr(C)]
pub struct AcquireCallbacks {
    context: *mut c_void,
    allocate: unsafe extern "C" fn(*mut c_void) -> u32,
    claim: unsafe extern "C" fn(*mut c_void, u32, *mut u32) -> bool,
    release_resolution: unsafe extern "C" fn(*mut c_void),
    release_handle: unsafe extern "C" fn(*mut c_void, u32),
}
use tsfp_engine_fileio::acquisition::{self, AcquireHost, Handle};
struct Acquiring<'a>(&'a AcquireCallbacks);
impl AcquireHost for Acquiring<'_> {
    fn allocate(&mut self) -> Option<Handle> {
        let handle = unsafe { (self.0.allocate)(self.0.context) };
        (handle != 0).then_some(Handle(handle))
    }
    fn claim(&mut self, handle: Handle) -> Result<(), u32> {
        let mut status = acquisition::INSUFFICIENT_RESOURCES;
        if unsafe { (self.0.claim)(self.0.context, handle.0, &mut status) } {
            Ok(())
        } else {
            Err(status)
        }
    }
    fn release_resolution(&mut self) {
        unsafe { (self.0.release_resolution)(self.0.context) };
    }
    fn release_handle(&mut self, handle: Handle) {
        unsafe { (self.0.release_handle)(self.0.context, handle.0) };
    }
}
unsafe extern "C" fn acquire(host: *const AcquireCallbacks, out: *mut u32) -> u32 {
    crate::counters::bump(crate::counters::ACQUIRE);
    if host.is_null() || out.is_null() {
        return guest_names::INVALID_PARAMETER;
    }
    match acquisition::acquire(&mut Acquiring(unsafe { &*host })) {
        Ok(handle) => {
            unsafe { *out = handle.0 };
            0
        }
        Err(status) => status,
    }
}
#[repr(C)]
pub struct ResolveCallbacks {
    context: *mut c_void,
    locate: unsafe extern "C" fn(*mut c_void, *mut u32, *mut bool, *mut u32) -> bool,
    real: unsafe extern "C" fn(*mut c_void, u32, *mut u32) -> bool,
    empty: unsafe extern "C" fn(*mut c_void, bool),
    refused: unsafe extern "C" fn(*mut c_void),
}
use acquisition::{Backing, Located, Missing, ResolveHost};
struct Resolving<'a>(&'a ResolveCallbacks);
impl ResolveHost for Resolving<'_> {
    fn locate(&mut self) -> Result<Located, u32> {
        let (mut backing, mut declared, mut status) = (0, false, 0);
        if !unsafe { (self.0.locate)(self.0.context, &mut backing, &mut declared, &mut status) } {
            return Err(status);
        }
        let backing = match backing {
            0 => Backing::Empty,
            1 => Backing::Fatx,
            2 => Backing::HostDirectory,
            3 => Backing::Disc,
            _ => return Err(guest_names::INVALID_PARAMETER),
        };
        Ok(Located { backing, declared })
    }
    fn real(&mut self, backing: Backing) -> Result<(), u32> {
        let code = match backing {
            Backing::Fatx => 1,
            Backing::HostDirectory => 2,
            Backing::Disc => 3,
            Backing::Empty => 0,
        };
        let mut status = 0;
        if unsafe { (self.0.real)(self.0.context, code, &mut status) } {
            Ok(())
        } else {
            Err(status)
        }
    }
    fn empty(&mut self, fabricated: bool) {
        unsafe { (self.0.empty)(self.0.context, fabricated) };
    }
    fn refused(&mut self) {
        unsafe { (self.0.refused)(self.0.context) };
    }
}
unsafe extern "C" fn resolve(
    host: *const ResolveCallbacks,
    allow_empty: bool,
    fabricate: bool,
) -> u32 {
    crate::counters::bump(crate::counters::RESOLVE);
    if host.is_null() {
        return guest_names::INVALID_PARAMETER;
    }
    acquisition::resolve(
        &mut Resolving(unsafe { &*host }),
        allow_empty,
        if fabricate {
            Missing::Fabricate
        } else {
            Missing::Refuse
        },
    )
    .err()
    .unwrap_or(0)
}
static BACKEND: Backend = Backend {
    open_at,
    close,
    read_at,
    allocate,
    metadata,
    metadata_at,
    space,
    dir_open,
    dir_next,
    dir_close,
    read_name,
    rename_guest,
    duplicate,
    make_dir,
    remove_at,
    move_at,
    acquire,
    resolve,
    prepare_create: crate::create_ffi::prepare,
    commit_rename: crate::rename_ffi::execute,
    mount_slot: crate::lifecycle_ffi::mount_slot,
    volume_count: crate::lifecycle_ffi::volume_count,
    unmount_all: crate::lifecycle_ffi::unmount_all,
    unmount_fatx: crate::lifecycle_ffi::unmount_fatx,
    acquire_device: crate::lifecycle_ffi::acquire_device,
    duplicate_cloexec: crate::lifecycle_ffi::duplicate_cloexec,
    prepare_cache: crate::lifecycle_ffi::prepare_cache,
    attempt_reset: crate::activity_ffi::reset,
    attempt_count: crate::activity_ffi::count,
    attempt_record: crate::activity_ffi::record,
    attempt_get: crate::activity_ffi::get,
    attempt_opened: crate::activity_ffi::opened,
    observer_set: crate::activity_ffi::set_observer,
    observer_event: crate::activity_ffi::event,
    observer_enabled: crate::activity_ffi::observer_enabled,
    prepare_device: crate::lifecycle_ffi::prepare_device,
    filetime: crate::lifecycle_ffi::filetime,
    validate_device: crate::lifecycle_ffi::validate_device,
    guest_metadata: crate::lifecycle_ffi::guest_metadata,
    space_units: crate::lifecycle_ffi::space_units,
    publish_device: crate::lifecycle_ffi::publish_device,
    read_attributes,
    openable_reset: crate::activity_ffi::openable_reset,
    openable_add: crate::activity_ffi::openable_add,
    openable_count: crate::activity_ffi::openable_count,
    openable_contains: crate::activity_ffi::openable_contains,
    missing_policy_set: crate::activity_ffi::missing_policy_set,
    missing_policy_get: crate::activity_ffi::missing_policy_get,
    validate_host_directory: crate::lifecycle_ffi::validate_host_directory,
    open_host_directory: crate::lifecycle_ffi::open_host_directory,
    publish_host_directory: crate::lifecycle_ffi::publish_host_directory,
    root_path,
    publish_disc: crate::lifecycle_ffi::publish_disc,
    publish_content_descriptor: crate::lifecycle_ffi::publish_content_descriptor,
    validate_disc_prefix: crate::lifecycle_ffi::validate_disc_prefix,
};
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_rfio_hostio_backend() -> *const Backend {
    &BACKEND
}

#[cfg(test)]
mod directory_root_tests {
    /// T3786 (I-T3786-1): XDeleteSaveGame opens each save file relative to the save directory's handle. The only test
    /// touching the handle index and payload statics, so it cannot race another test.
    #[test]
    fn a_live_directory_handle_roots_a_relative_open_and_nothing_else_does() {
        crate::handles_ffi::test_open(8, 3, 0x00e1_00e4, b"\\??\\U:\\17A81BE8F2A4\\", true);
        crate::handles_ffi::test_open(
            8,
            4,
            0x00e1_0128,
            b"\\??\\U:\\17A81BE8F2A4\\SaveMeta.xbx",
            false,
        );
        let mut buffer = [0u8; 260];
        let resolve = |root: u32, buffer: &mut [u8; 260]| {
            buffer.fill(0);
            buffer[..12].copy_from_slice(b"SaveMeta.xbx");
            let accepted =
                unsafe { super::root_path(root, buffer.as_mut_ptr().cast(), buffer.len()) };
            let end = buffer.iter().position(|byte| *byte == 0).unwrap();
            (accepted, buffer[..end].to_vec())
        };
        assert_eq!(
            resolve(0x00e1_00e4, &mut buffer),
            (true, b"\\??\\U:\\17A81BE8F2A4\\SaveMeta.xbx".to_vec())
        );
        // A file handle, an unknown handle and the unused pseudo root stay refused, the name untouched.
        for root in [0x00e1_0128, 0x00e1_0200, 0xffff_fffc] {
            assert_eq!(
                resolve(root, &mut buffer),
                (false, b"SaveMeta.xbx".to_vec()),
                "{root:#x}"
            );
        }
    }
}
