//! Guest-visible file API: mounts, symlinks, handles. Contract: docs/t-contract-data-assets.md.
//!
//! Path rules mirror the C host `kernel_file.c`: ASCII case-insensitive, `\` and `/` are one
//! separator (the guest mixes them in one string), a symlink or mount prefix matches only a
//! whole leading component, the longest mount prefix wins, `.` and `..` components are refused.
use crate::xiso::{Entry, Xiso, XisoError};
use std::fs::{self, File, OpenOptions};
use std::io::SeekFrom;
use std::os::unix::fs::FileExt;
use std::path::{Path, PathBuf};

/// NTSTATUS values the C host returns for the same situations (kernel_file.h).
pub mod status {
    pub const SUCCESS: u32 = 0;
    pub const INVALID_PARAMETER: u32 = 0xC000_000D;
    pub const NO_SUCH_FILE: u32 = 0xC000_000F;
    pub const INVALID_DEVICE_REQUEST: u32 = 0xC000_0010;
    pub const END_OF_FILE: u32 = 0xC000_0011;
    pub const INVALID_HANDLE: u32 = 0xC000_0008;
    pub const ACCESS_DENIED: u32 = 0xC000_0022;
    pub const OBJECT_NAME_NOT_FOUND: u32 = 0xC000_0034;
    pub const OBJECT_NAME_COLLISION: u32 = 0xC000_0035;
    pub const OBJECT_PATH_NOT_FOUND: u32 = 0xC000_003A;
    pub const OBJECT_TYPE_MISMATCH: u32 = 0xC000_0024;
    pub const INSUFFICIENT_RESOURCES: u32 = 0xC000_009A;
    pub const UNRECOGNIZED_VOLUME: u32 = 0xC000_014F;
}
pub type NtStatus = u32;
pub const SYMLINK_DEPTH_MAX: usize = 8;
pub const PATH_MAX: usize = 512;
pub const OPEN_MAX: usize = 64;

/// What to do when the file does and does not exist (NtCreateFile disposition subset).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disposition { Open, OpenIf, Create, OverwriteIf }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Access { pub read: bool, pub write: bool }
impl Access {
    pub const READ: Self = Self { read: true, write: false };
    pub const READ_WRITE: Self = Self { read: true, write: true };
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Handle(pub u32);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileInfo { pub size: u64, pub is_directory: bool }

/// One name of a directory listing (disc or host-dir volume).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirEntry { pub name: String, pub is_directory: bool, pub size: u64 }

enum Backing { Disc(Xiso), HostDir(PathBuf) }
struct Mount { prefix: String, backing: Backing }
enum Target { Disc(Entry, usize), Host(File, bool), HostDirectory }
struct Open { target: Target, position: u64, access: Access }

#[derive(Default)]
pub struct Vfs {
    mounts: Vec<Mount>,
    symlinks: Vec<(String, String)>,
    handles: Vec<Option<Open>>,
}

/// Lowercase, `\` -> `/`, collapse duplicate separators, trim a trailing one.
fn canon(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        let c = if c == '\\' { '/' } else { c.to_ascii_lowercase() };
        if c == '/' && out.ends_with('/') { continue; }
        out.push(c);
    }
    while out.len() > 1 && out.ends_with('/') { out.pop(); }
    out
}
/// If `prefix` (canonical) covers a whole leading component of `path` (canonical), the tail.
fn strip_component_prefix<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    let rest = path.strip_prefix(prefix)?;
    if rest.is_empty() || rest.starts_with('/') || prefix.ends_with('/') { Some(rest) } else { None }
}

impl Vfs {
    pub fn new() -> Self { Self::default() }

    /// The mapping the title expects: `D:` is the disc, `T:`/`U:`/`Z:`-style save letters are
    /// added by the caller via [`Vfs::add_symlink`]. Default devices match `host_options.h`.
    pub fn standard(disc: Xiso, save_dir: &Path) -> Result<Self, NtStatus> {
        let mut v = Self::new();
        v.mount_disc("\\Device\\CdRom0", disc);
        v.mount_host_dir("\\Device\\Harddisk0\\partition1", save_dir)?;
        v.add_symlink("\\??\\D:", "\\Device\\CdRom0");
        v.add_symlink("D:", "\\Device\\CdRom0");
        v.add_symlink("\\??\\C:", "\\Device\\Harddisk0\\partition1");
        v.add_symlink("C:", "\\Device\\Harddisk0\\partition1");
        Ok(v)
    }
    pub fn mount_disc(&mut self, prefix: &str, disc: Xiso) {
        self.mounts.push(Mount { prefix: canon(prefix), backing: Backing::Disc(disc) });
    }
    /// Writable directory-backed volume (saves, profile). Created if missing.
    pub fn mount_host_dir(&mut self, prefix: &str, dir: &Path) -> Result<(), NtStatus> {
        fs::create_dir_all(dir).map_err(|_| status::ACCESS_DENIED)?;
        self.mounts.push(Mount { prefix: canon(prefix), backing: Backing::HostDir(dir.to_path_buf()) });
        Ok(())
    }
    pub fn add_symlink(&mut self, name: &str, target: &str) {
        self.symlinks.push((canon(name), target.to_string()));
    }

    fn resolve(&self, path: &str) -> Result<String, NtStatus> {
        if path.len() >= PATH_MAX { return Err(status::INVALID_PARAMETER); }
        let mut cur = canon(path);
        for _ in 0..SYMLINK_DEPTH_MAX {
            let hit = self.symlinks.iter().find_map(|(n, t)| strip_component_prefix(&cur, n).map(|r| format!("{}{}", canon(t), r)));
            match hit { Some(next) => cur = next, None => return Ok(cur) }
        }
        Err(status::OBJECT_NAME_NOT_FOUND) // cyclic link table, refused like the C host
    }
    fn mount_for<'a>(&'a self, resolved: &'a str) -> Option<(&'a Mount, &'a str)> {
        self.mounts.iter().filter_map(|m| strip_component_prefix(resolved, &m.prefix).map(|r| (m, r)))
            .max_by_key(|(m, _)| m.prefix.len())
    }

    fn components(rest: &str) -> Result<Vec<&str>, NtStatus> {
        let parts: Vec<&str> = rest.split('/').filter(|p| !p.is_empty()).collect();
        if parts.iter().any(|p| *p == "." || *p == "..") { return Err(status::OBJECT_NAME_NOT_FOUND); }
        Ok(parts)
    }
    /// Walk `parts` below `base`, matching each component case-insensitively against the real
    /// directory listing (host FS is case-sensitive, FATX is not). `create_last` returns the
    /// would-be path for a missing final component.
    fn host_path(base: &Path, parts: &[&str], create_last: bool) -> Result<(PathBuf, bool), NtStatus> {
        let mut cur = base.to_path_buf();
        for (i, part) in parts.iter().enumerate() {
            let last = i + 1 == parts.len();
            let found = fs::read_dir(&cur).ok().and_then(|rd| rd.filter_map(Result::ok)
                .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(part)).map(|e| e.path()));
            match found {
                Some(p) => cur = p,
                None if last && create_last => return Ok((cur.join(part), false)),
                None if last => return Err(status::OBJECT_NAME_NOT_FOUND),
                None => return Err(status::OBJECT_PATH_NOT_FOUND),
            }
        }
        Ok((cur, true))
    }

    /// The canonical, symlink-resolved form of a guest path: the key identity of one file (share modes).
    pub fn canonical(&self, path: &str) -> Result<String, NtStatus> { self.resolve(path) }

    /// Every name in a directory, sorted by case-folded name (the order the C host enumerates host
    /// directories in). Disc directories are listed too; the C host answers NOT_IMPLEMENTED there.
    pub fn list_dir(&self, path: &str) -> Result<Vec<DirEntry>, NtStatus> {
        let resolved = self.resolve(path)?;
        let Some((m, rest)) = self.mount_for(&resolved) else { return Err(status::OBJECT_PATH_NOT_FOUND) };
        let parts = Self::components(rest)?;
        let mut out: Vec<DirEntry> = match &m.backing {
            Backing::Disc(disc) => {
                let mut cur = disc.root().clone();
                for part in &parts {
                    if !cur.is_directory() { return Err(status::OBJECT_PATH_NOT_FOUND); }
                    cur = disc.find_child(&cur, part.as_bytes()).map_err(|e| nt_from_xiso(&e))?.ok_or(status::OBJECT_PATH_NOT_FOUND)?;
                }
                if !cur.is_directory() { return Err(status::INVALID_PARAMETER); }
                disc.read_dir(&cur).map_err(|e| nt_from_xiso(&e))?.into_iter().map(|e| DirEntry {
                    name: String::from_utf8_lossy(&e.name).into_owned(), is_directory: e.is_directory(), size: u64::from(e.size) }).collect()
            }
            Backing::HostDir(base) => {
                let (dir, _) = Self::host_path(base, &parts, false).map_err(|s| if s == status::OBJECT_NAME_NOT_FOUND { status::OBJECT_PATH_NOT_FOUND } else { s })?;
                if !dir.is_dir() { return Err(status::INVALID_PARAMETER); }
                fs::read_dir(&dir).map_err(|_| status::ACCESS_DENIED)?.filter_map(Result::ok).filter_map(|e| {
                    let md = e.metadata().ok()?;
                    if e.file_type().ok()?.is_symlink() { return None; }
                    Some(DirEntry { name: e.file_name().to_string_lossy().into_owned(), is_directory: md.is_dir(), size: if md.is_dir() { 0 } else { md.len() } })
                }).collect()
            }
        };
        out.sort_by_key(|e| e.name.to_ascii_lowercase());
        Ok(out)
    }

    pub fn open(&mut self, path: &str, access: Access, disposition: Disposition) -> Result<Handle, NtStatus> {
        let resolved = self.resolve(path)?;
        let (mount_index, rest) = match self.mount_for(&resolved) {
            Some((m, r)) => (self.mounts.iter().position(|x| std::ptr::eq(x, m)).unwrap(), r.to_string()),
            None => return Err(status::OBJECT_PATH_NOT_FOUND),
        };
        let parts = Self::components(&rest)?;
        let target = match &self.mounts[mount_index].backing {
            Backing::Disc(disc) => {
                if access.write || disposition != Disposition::Open { return Err(status::ACCESS_DENIED); }
                let mut cur = disc.root().clone();
                for (i, part) in parts.iter().enumerate() {
                    if !cur.is_directory() { return Err(status::OBJECT_PATH_NOT_FOUND); }
                    cur = disc.find_child(&cur, part.as_bytes()).map_err(|e| nt_from_xiso(&e))?.ok_or(
                        if i + 1 == parts.len() { status::OBJECT_NAME_NOT_FOUND } else { status::OBJECT_PATH_NOT_FOUND })?;
                }
                Target::Disc(cur, mount_index)
            }
            Backing::HostDir(base) => {
                let creating = disposition != Disposition::Open;
                let (p, exists) = Self::host_path(base, &parts, creating)?;
                if exists && p.is_dir() { Target::HostDirectory }
                else {
                    if exists && disposition == Disposition::Create { return Err(status::OBJECT_NAME_COLLISION); }
                    if !exists && parts.len() > 1 && !p.parent().is_some_and(Path::exists) { return Err(status::OBJECT_PATH_NOT_FOUND); }
                    let write = access.write || (!exists && creating);
                    let mut o = OpenOptions::new();
                    o.read(true).write(write).create(creating && !exists);
                    if exists && disposition == Disposition::OverwriteIf { o.truncate(true); o.write(true); }
                    let f = o.open(&p).map_err(|_| status::ACCESS_DENIED)?;
                    Target::Host(f, write || disposition == Disposition::OverwriteIf)
                }
            }
        };
        let slot = match self.handles.iter().position(Option::is_none) {
            Some(i) => i,
            None if self.handles.len() < OPEN_MAX => { self.handles.push(None); self.handles.len() - 1 }
            None => return Err(status::INSUFFICIENT_RESOURCES),
        };
        self.handles[slot] = Some(Open { target, position: 0, access });
        Ok(Handle(slot as u32 + 1))
    }
    fn get(&mut self, h: Handle) -> Result<&mut Open, NtStatus> {
        self.handles.get_mut((h.0 as usize).wrapping_sub(1)).and_then(Option::as_mut).ok_or(status::INVALID_HANDLE)
    }
    pub fn close(&mut self, h: Handle) -> Result<(), NtStatus> {
        self.get(h)?;
        self.handles[h.0 as usize - 1] = None;
        Ok(())
    }
    pub fn open_count(&self) -> usize { self.handles.iter().flatten().count() }
    pub fn size(&mut self, h: Handle) -> Result<u64, NtStatus> {
        match &self.get(h)?.target {
            Target::Disc(e, _) => Ok(u64::from(e.size)),
            Target::Host(f, ..) => f.metadata().map(|m| m.len()).map_err(|_| status::ACCESS_DENIED),
            Target::HostDirectory => Ok(0),
        }
    }
    /// Read at an explicit offset without moving the position. `Ok(0)` is end of file; callers
    /// that mirror NtReadFile turn a zero-byte read of a positive length into `END_OF_FILE`.
    pub fn read_at(&mut self, h: Handle, offset: u64, buf: &mut [u8]) -> Result<usize, NtStatus> {
        let (mount, entry) = match &self.get(h)?.target {
            Target::Disc(e, m) => (Some(*m), Some(e.clone())),
            _ => (None, None),
        };
        if let (Some(m), Some(e)) = (mount, entry) {
            if e.is_directory() { return Err(status::INVALID_DEVICE_REQUEST); }
            let Backing::Disc(d) = &self.mounts[m].backing else { return Err(status::INVALID_HANDLE) };
            return d.read(&e, offset, buf).map_err(|e| nt_from_xiso(&e));
        }
        match &self.get(h)?.target {
            Target::Host(f, ..) => read_full_at(f, buf, offset).map_err(|_| status::ACCESS_DENIED),
            _ => Err(status::INVALID_DEVICE_REQUEST),
        }
    }
    /// Sequential read: advances the position by the bytes returned.
    pub fn read(&mut self, h: Handle, buf: &mut [u8]) -> Result<usize, NtStatus> {
        let pos = self.get(h)?.position;
        let n = self.read_at(h, pos, buf)?;
        self.get(h)?.position += n as u64;
        Ok(n)
    }
    pub fn seek(&mut self, h: Handle, from: SeekFrom) -> Result<u64, NtStatus> {
        let size = self.size(h)? as i128;
        let o = self.get(h)?;
        let next = match from {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::Current(d) => i128::from(o.position) + i128::from(d),
            SeekFrom::End(d) => size + i128::from(d),
        };
        if next < 0 { return Err(status::INVALID_PARAMETER); }
        o.position = next as u64;
        Ok(o.position)
    }
    pub fn position(&mut self, h: Handle) -> Result<u64, NtStatus> { Ok(self.get(h)?.position) }
    /// Sequential write (host-dir volumes only; the disc is read-only).
    pub fn write(&mut self, h: Handle, data: &[u8]) -> Result<usize, NtStatus> {
        let o = self.get(h)?;
        if !o.access.write && !matches!(&o.target, Target::Host(_, true)) { return Err(status::ACCESS_DENIED); }
        let pos = o.position;
        match &o.target {
            Target::Host(f, true) => f.write_all_at(data, pos).map_err(|_| status::ACCESS_DENIED)?,
            _ => return Err(status::ACCESS_DENIED),
        }
        o.position += data.len() as u64;
        Ok(data.len())
    }
    pub fn set_end_of_file(&mut self, h: Handle) -> Result<(), NtStatus> {
        let o = self.get(h)?;
        match &o.target {
            Target::Host(f, true) => f.set_len(o.position).map_err(|_| status::ACCESS_DENIED),
            _ => Err(status::ACCESS_DENIED),
        }
    }
    /// Existence and size query without opening a handle.
    pub fn query(&mut self, path: &str) -> Result<FileInfo, NtStatus> {
        let h = self.open(path, Access::READ, Disposition::Open)?;
        let info = FileInfo { size: self.size(h)?, is_directory: matches!(self.get(h)?.target, Target::HostDirectory) || matches!(&self.get(h)?.target, Target::Disc(e, _) if e.is_directory()) };
        self.close(h)?;
        Ok(info)
    }
    /// Rename within one writable volume; replaces an existing target (MoveFileEx-style).
    pub fn rename(&mut self, from: &str, to: &str) -> Result<(), NtStatus> {
        let (rf, rt) = (self.resolve(from)?, self.resolve(to)?);
        let (Some((mf, restf)), Some((mt, restt))) = (self.mount_for(&rf), self.mount_for(&rt)) else { return Err(status::OBJECT_PATH_NOT_FOUND) };
        let (Backing::HostDir(base), true) = (&mf.backing, std::ptr::eq(mf, mt)) else { return Err(status::ACCESS_DENIED) };
        let (pf, _) = Self::host_path(base, &Self::components(restf)?, false)?;
        let (pt, _) = Self::host_path(base, &Self::components(restt)?, true)?;
        fs::rename(pf, pt).map_err(|_| status::ACCESS_DENIED)
    }
    /// Reserve a new writable directory without reusing an existing save identity.
    /// Parent must exist. Existing case-insensitive names and read-only mounts fail.
    pub fn create_directory(&mut self, path: &str) -> Result<(), NtStatus> {
        let resolved = self.resolve(path)?;
        let Some((mount, rest)) = self.mount_for(&resolved) else {
            return Err(status::OBJECT_PATH_NOT_FOUND);
        };
        let Backing::HostDir(base) = &mount.backing else {
            return Err(status::ACCESS_DENIED);
        };
        let parts = Self::components(rest)?;
        if parts.is_empty() {
            return Err(status::OBJECT_NAME_COLLISION);
        }
        let (target, exists) = Self::host_path(base, &parts, true)?;
        if exists {
            return Err(status::OBJECT_NAME_COLLISION);
        }
        fs::create_dir(target).map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                status::OBJECT_NAME_COLLISION
            } else {
                status::ACCESS_DENIED
            }
        })
    }
    pub fn delete(&mut self, path: &str) -> Result<(), NtStatus> {
        let resolved = self.resolve(path)?;
        let Some((m, rest)) = self.mount_for(&resolved) else { return Err(status::OBJECT_PATH_NOT_FOUND) };
        let Backing::HostDir(base) = &m.backing else { return Err(status::ACCESS_DENIED) };
        let parts = Self::components(rest)?;
        if parts.is_empty() { return Err(status::ACCESS_DENIED); }
        let (p, _) = Self::host_path(base, &parts, false)?;
        if p.is_dir() { fs::remove_dir(&p) } else { fs::remove_file(&p) }.map_err(|_| status::ACCESS_DENIED)
    }
}

fn read_full_at(f: &File, buf: &mut [u8], offset: u64) -> std::io::Result<usize> {
    let mut got = 0;
    while got < buf.len() {
        let n = f.read_at(&mut buf[got..], offset + got as u64)?;
        if n == 0 { break; }
        got += n;
    }
    Ok(got)
}

pub fn nt_from_xiso(e: &XisoError) -> NtStatus {
    match e {
        XisoError::NotFound => status::OBJECT_NAME_NOT_FOUND,
        XisoError::Magic => status::UNRECOGNIZED_VOLUME,
        XisoError::Image(_) | XisoError::Truncated | XisoError::Malformed(_) | XisoError::WalkLimit => status::UNRECOGNIZED_VOLUME,
    }
}
