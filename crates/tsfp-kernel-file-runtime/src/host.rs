//! Descriptor-relative host operations. Guest components are never joined to host paths.
use std::ffi::{CStr, CString};
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
use tsfp_engine_fileio::hostdir::{self, HostOps, Kind, WalkError};
use tsfp_engine_fileio::vfs::status;

pub const SYMLINK_REFUSED: u32 = 0xe0000001;

pub fn nt_error(error: io::Error) -> u32 {
    match error.raw_os_error() {
        Some(libc::ENOENT) => status::OBJECT_NAME_NOT_FOUND,
        Some(libc::ENOTDIR) => status::OBJECT_PATH_NOT_FOUND,
        Some(libc::ENOSPC | libc::EDQUOT) => 0xc000007f,
        Some(libc::EEXIST) => status::OBJECT_NAME_COLLISION,
        Some(libc::ENOTEMPTY) => 0xc0000101,
        _ => status::ACCESS_DENIED,
    }
}
pub fn last_error() -> u32 { nt_error(io::Error::last_os_error()) }
pub fn cstring(bytes: &[u8]) -> Result<CString, u32> {
    CString::new(bytes).map_err(|_| status::INVALID_PARAMETER)
}
fn owned(fd: i32) -> Result<File, u32> {
    if fd < 0 { Err(last_error()) } else { Ok(unsafe { File::from_raw_fd(fd) }) }
}
pub fn root(path: &[u8]) -> Result<File, u32> {
    let name = cstring(path)?;
    owned(unsafe { libc::open(name.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC) })
}
pub fn open_at(dir: i32, name: &[u8], flags: i32) -> Result<File, u32> {
    let name = cstring(name)?;
    owned(unsafe { libc::openat(dir, name.as_ptr(), flags | libc::O_CLOEXEC | libc::O_NOFOLLOW, 0o600) })
}
pub fn stat_at(dir: i32, name: &[u8]) -> Result<libc::stat, u32> {
    let name = cstring(name)?;
    let mut result = std::mem::MaybeUninit::uninit();
    if unsafe { libc::fstatat(dir, name.as_ptr(), result.as_mut_ptr(), libc::AT_SYMLINK_NOFOLLOW) } < 0 {
        Err(last_error())
    } else { Ok(unsafe { result.assume_init() }) }
}
pub fn stat(file: &File) -> Result<libc::stat, u32> {
    let mut result = std::mem::MaybeUninit::uninit();
    if unsafe { libc::fstat(file.as_raw_fd(), result.as_mut_ptr()) } < 0 { Err(last_error()) }
    else { Ok(unsafe { result.assume_init() }) }
}
pub fn kind(s: &libc::stat) -> Kind {
    match s.st_mode & libc::S_IFMT { libc::S_IFDIR => Kind::Dir, libc::S_IFLNK => Kind::Link, _ => Kind::Other }
}
pub fn regular(s: &libc::stat) -> bool { s.st_mode & libc::S_IFMT == libc::S_IFREG }
pub fn mkdir(dir: i32, name: &[u8]) -> Result<(), u32> {
    let name = cstring(name)?;
    if unsafe { libc::mkdirat(dir, name.as_ptr(), 0o700) } < 0 { Err(last_error()) } else { Ok(()) }
}
pub fn unlink(dir: i32, name: &[u8], directory: bool) -> Result<(), u32> {
    let name = cstring(name)?;
    if unsafe { libc::unlinkat(dir, name.as_ptr(), if directory { libc::AT_REMOVEDIR } else { 0 }) } < 0 {
        Err(last_error())
    } else { Ok(()) }
}
pub fn rename(from: i32, old: &[u8], to: i32, new: &[u8]) -> Result<(), u32> {
    let old = cstring(old)?; let new = cstring(new)?;
    if unsafe { libc::renameat(from, old.as_ptr(), to, new.as_ptr()) } < 0 { Err(last_error()) } else { Ok(()) }
}
pub fn names(dir: i32) -> Result<Vec<Vec<u8>>, u32> {
    // openat(".") creates an independent directory position; dup alone shares it.
    let fd = open_at(dir, b".", libc::O_RDONLY | libc::O_DIRECTORY)?.into_raw_fd();
    let stream = unsafe { libc::fdopendir(fd) };
    if stream.is_null() { unsafe { libc::close(fd); } return Err(last_error()); }
    let mut result = Vec::new();
    let error = loop {
        unsafe { *libc::__errno_location() = 0; }
        let entry = unsafe { libc::readdir(stream) };
        if entry.is_null() { break unsafe { *libc::__errno_location() }; }
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
        if name != b"." && name != b".." { result.push(name.to_vec()); }
    };
    unsafe { libc::closedir(stream); }
    if error == 0 { Ok(result) } else { Err(nt_error(io::Error::from_raw_os_error(error))) }
}
pub struct Ops;
impl HostOps for Ops {
    fn dup(&mut self, fd: i32) -> Option<i32> {
        let fd = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 0) };
        (fd >= 0).then_some(fd)
    }
    fn close(&mut self, fd: i32) { unsafe { libc::close(fd); } }
    fn stat_at(&mut self, dir: i32, name: &[u8]) -> Option<Kind> { stat_at(dir, name).ok().map(|s| kind(&s)) }
    fn open_dir(&mut self, dir: i32, name: &[u8]) -> Option<i32> {
        open_at(dir, name, libc::O_RDONLY | libc::O_DIRECTORY).ok().map(IntoRawFd::into_raw_fd)
    }
    fn scan(&mut self, dir: i32, visit: &mut dyn FnMut(&[u8]) -> bool) -> bool {
        let Ok(names) = names(dir) else { return false; };
        for name in names { if visit(&name) { break; } }
        true
    }
}
pub fn walk(root: &File, rest: &[u8]) -> Result<(File, Vec<u8>), u32> {
    hostdir::walk(&mut Ops, root.as_raw_fd(), rest, 261, 261)
        .map(|(fd, leaf)| (unsafe { File::from_raw_fd(fd) }, leaf))
        .map_err(|e| match e {
            WalkError::Dot => status::ACCESS_DENIED,
            WalkError::Link(_) => SYMLINK_REFUSED,
            WalkError::NoDescriptor => status::INSUFFICIENT_RESOURCES,
            _ => status::OBJECT_PATH_NOT_FOUND,
        })
}
pub fn real_name(dir: &File, name: &[u8]) -> Result<Vec<u8>, u32> {
    let (real, kind) = hostdir::real_name(&mut Ops, dir.as_raw_fd(), name, 261)
        .ok_or(status::OBJECT_NAME_NOT_FOUND)?;
    if kind == Kind::Link { Err(SYMLINK_REFUSED) } else { Ok(real) }
}
pub fn space(root: &File) -> Result<(u64, u64), u32> {
    let mut result = std::mem::MaybeUninit::uninit();
    if unsafe { libc::fstatvfs(root.as_raw_fd(), result.as_mut_ptr()) } < 0 { return Err(last_error()); }
    let s = unsafe { result.assume_init() };
    tsfp_engine_fileio::lifecycle::space_units(s.f_blocks, s.f_bavail, s.f_frsize).ok_or(status::INVALID_PARAMETER)
}
