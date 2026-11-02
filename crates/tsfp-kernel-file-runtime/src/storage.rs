//! Mounted resources and operations; no guest memory or kernel object dependencies.
use crate::host;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::unix::fs::FileExt;
use tsfp_engine_fileio::{fatx_img as fatx, lifecycle, xiso::{Xiso, Entry}, vfs::status};

#[derive(Clone, Debug, Default)]
pub struct Metadata {
    pub backing: u32, pub times_known: bool,
    pub size: u64, pub allocation: u64, pub directory: bool, pub device: bool,
    pub attributes: u32, pub creation: u64, pub access: u64, pub write: u64,
}
impl Metadata {
    pub fn host(s: &libc::stat) -> Self {
        let directory = host::kind(s) == tsfp_engine_fileio::hostdir::Kind::Dir;
        let write = lifecycle::filetime(s.st_mtime, s.st_mtime_nsec);
        Self { backing: 2, times_known: true, size: if directory { 0 } else { s.st_size.max(0) as u64 },
            allocation: (s.st_blocks.max(0) as u64).saturating_mul(512), directory,
            attributes: if directory { 0x10 } else { 0x80 }, creation: write, write,
            access: lifecycle::filetime(s.st_atime, s.st_atime_nsec), ..Self::default() }
    }
}
pub struct Listed { pub name: Vec<u8>, pub metadata: Metadata }
pub enum Resource { Empty, Host(File), Disc(Entry), Fatx }
pub struct Resolved { pub resource: Resource, pub metadata: Metadata, pub backing: u32, pub cache: bool }
pub struct Device { pub name: Vec<u8>, pub capacity: u64, pub cache: Option<Cache> }
pub struct Cache { pub number: u32, pub name: Vec<u8>, pub root: Option<File> }
pub enum Storage {
    Disc(Xiso),
    Host { root: File, device: Option<Device> },
    Fatx(FatxImage),
}
/// The MU subsystem lends its byte image until unmount. The runtime owns the
/// mounted-volume identity, geometry and generation; mutation is under its lock.
pub struct FatxImage {
    pub address: usize, pub length: usize, pub geometry: fatx::Geo,
    pub flush: Option<unsafe extern "C" fn(*mut libc::c_void) -> bool>, pub context: usize,
}
impl FatxImage {
    pub unsafe fn new(address: *mut u8, length: usize,
        flush: Option<unsafe extern "C" fn(*mut libc::c_void) -> bool>, context: usize) -> Result<Self, u32> {
        if address.is_null() || length > isize::MAX as usize { return Err(status::INVALID_PARAMETER); }
        let bytes = unsafe { std::slice::from_raw_parts(address, length) };
        let geometry = fatx::open(bytes).map_err(fatx_status)?;
        Ok(Self { address: address as usize, length, geometry, flush, context })
    }
    pub fn bytes(&self) -> &[u8] { unsafe { std::slice::from_raw_parts(self.address as *const u8, self.length) } }
    pub fn change(&mut self, job: impl FnOnce(&fatx::Geo, &mut [u8]) -> u32) -> Result<(), u32> {
        let bytes = unsafe { std::slice::from_raw_parts_mut(self.address as *mut u8, self.length) };
        let status = job(&self.geometry, bytes);
        if status != fatx::OK { return Err(fatx_status(status)); }
        // As in the oracle, a persistence callback failure does not undo the
        // successful in-memory change. Explicit flush reports that failure.
        self.flush();
        Ok(())
    }
    pub fn flush(&self) -> bool {
        self.flush.is_none_or(|flush| unsafe { flush(self.context as *mut libc::c_void) })
    }
}
pub fn fatx_status(value: u32) -> u32 {
    match value { fatx::OK => 0, fatx::E_NOT_FOUND => status::OBJECT_NAME_NOT_FOUND,
        fatx::E_PATH_NOT_FOUND => status::OBJECT_PATH_NOT_FOUND,
        fatx::E_NO_SPACE | fatx::E_DIR_FULL => 0xc000007f,
        fatx::E_EXISTS => status::OBJECT_NAME_COLLISION,
        fatx::E_NOT_DIR | fatx::E_IS_DIR => status::ACCESS_DENIED,
        fatx::E_NAME | fatx::E_ARGUMENT => status::INVALID_PARAMETER, _ => 0xc0000001 }
}
pub struct Volume { pub prefix: Vec<u8>, pub generation: u64, pub storage: Storage }
fn disc_error(e: tsfp_engine_fileio::xiso::XisoError) -> u32 { tsfp_engine_fileio::vfs::nt_from_xiso(&e) }
fn disc_meta(e: &Entry) -> Metadata {
    Metadata { backing: 1, size: u64::from(e.size), allocation: u64::from(e.size), directory: e.is_directory(),
        attributes: u32::from(e.attributes), ..Metadata::default() }
}
pub fn writable(access: u32) -> bool { access & (2 | 4 | 0x40000000 | 0x10000000 | 0x02000000) != 0 }
fn host_root(root: &File, device: &mut Option<Device>, rest: &[u8]) -> Result<(File, bool), u32> {
    if let Some(device) = device {
        if rest.is_empty() { return Err(status::INVALID_DEVICE_REQUEST); }
        let cache = device.cache.as_mut().ok_or(status::OBJECT_NAME_NOT_FOUND)?;
        let regular = host::stat_at(root.as_raw_fd(), &device.name)
            .is_ok_and(|info| host::regular(&info) && info.st_size >= 0
                && info.st_size as u64 <= device.capacity);
        if !regular {
            cache.root = None;
            crate::guest::log_cache_not_formatted(&device.name);
            return Err(status::UNRECOGNIZED_VOLUME);
        }
        let file = host::open_at(root.as_raw_fd(), &device.name, libc::O_RDONLY | libc::O_NONBLOCK)?;
        let mut magic = [0; 4];
        if file.read_at(&mut magic, 0).map_err(host::nt_error)? != 4 || magic != *b"FATX" {
            cache.root = None;
            crate::guest::log_cache_not_formatted(&device.name);
            return Err(status::UNRECOGNIZED_VOLUME);
        }
        if cache.root.is_none() {
            let created = match host::mkdir(root.as_raw_fd(), &cache.name) {
                Ok(()) => true,
                Err(status::OBJECT_NAME_COLLISION) => false,
                Err(e) => return Err(e),
            };
            crate::guest::log_cache_formatted(&device.name, &cache.name, created);
            if !created {
                match host::stat_at(root.as_raw_fd(), &cache.name) {
                    Ok(info) if host::kind(&info) == tsfp_engine_fileio::hostdir::Kind::Dir => (),
                    Ok(_) => {
                        crate::guest::log_cache_directory_refused(&cache.name);
                        return Err(status::ACCESS_DENIED);
                    }
                    Err(error) => return Err(error),
                }
            }
            cache.root = Some(host::open_at(root.as_raw_fd(), &cache.name, libc::O_RDONLY | libc::O_DIRECTORY)?);
        }
        Ok((cache.root.as_ref().unwrap().try_clone().map_err(host::nt_error)?, true))
    } else { Ok((root.try_clone().map_err(host::nt_error)?, false)) }
}
impl Volume {
    pub fn backing(&self) -> u32 { match self.storage { Storage::Disc(_) => 1, Storage::Host { .. } => 2, Storage::Fatx(_) => 3 } }
    pub fn resolve(&mut self, rest: &[u8], access: u32) -> Result<Resolved, u32> {
        match &mut self.storage {
            Storage::Disc(disc) => {
                let path = std::str::from_utf8(rest).map_err(|_| status::OBJECT_NAME_NOT_FOUND)?;
                let entry = disc.lookup(path).map_err(disc_error)?;
                let metadata = disc_meta(&entry);
                Ok(Resolved { resource: Resource::Disc(entry), metadata, backing: 1, cache: false })
            }
            Storage::Fatx(image) => {
                let (directory, size) = fatx::stat_path(&image.geometry, image.bytes(), rest).map_err(fatx_status)?;
                Ok(Resolved { resource: Resource::Fatx, metadata: Metadata { backing: 3, directory, size: size.into(),
                    attributes: if directory { 0x10 } else { 0x80 }, ..Metadata::default() }, backing: 3, cache: false })
            }
            Storage::Host { root, device } => {
                if device.as_ref().is_some_and(|d| d.cache.is_none()) && !rest.is_empty() {
                    return Err(status::OBJECT_PATH_NOT_FOUND);
                }
                if rest.is_empty() { if let Some(device) = device {
                    let file = host::open_at(root.as_raw_fd(), &device.name, if writable(access) { libc::O_RDWR } else { libc::O_RDONLY })?;
                    if !host::regular(&host::stat(&file)?) { return Err(status::ACCESS_DENIED); }
                    return Ok(Resolved { resource: Resource::Host(file), metadata: Metadata {
                        backing: 2, size: device.capacity, allocation: device.capacity, device: true, attributes: 0x80,
                        ..Metadata::default() }, backing: 2, cache: false });
                } }
                let (root, cache) = host_root(root, device, rest)?;
                let (dir, leaf) = host::walk(&root, rest)?;
                let file = if leaf.is_empty() { dir } else {
                    let real = host::real_name(&dir, &leaf)?;
                    let s = host::stat_at(dir.as_raw_fd(), &real)?;
                    let is_dir = host::kind(&s) == tsfp_engine_fileio::hostdir::Kind::Dir;
                    if !is_dir && !host::regular(&s) { return Err(status::ACCESS_DENIED); }
                    host::open_at(dir.as_raw_fd(), &real, if is_dir { libc::O_RDONLY | libc::O_DIRECTORY }
                        else if writable(access) { libc::O_RDWR } else { libc::O_RDONLY })?
                };
                let s = host::stat(&file)?;
                if host::kind(&s) != tsfp_engine_fileio::hostdir::Kind::Dir && !host::regular(&s) { return Err(status::ACCESS_DENIED); }
                Ok(Resolved { metadata: Metadata::host(&s), resource: Resource::Host(file), backing: 2, cache })
            }
        }
    }
    pub fn create(&mut self, rest: &[u8], directory: bool) -> Result<(), u32> {
        match &mut self.storage {
            Storage::Disc(_) => Err(status::ACCESS_DENIED),
            Storage::Fatx(image) => image.change(|g, bytes| if directory { fatx::mkdir_path(g, bytes, rest) }
                else { fatx::write_path(g, bytes, rest, &[]) }),
            Storage::Host { root, device } => {
                let (root, _) = host_root(root, device, rest)?;
                let (dir, leaf) = host::walk(&root, rest)?;
                if leaf.is_empty() { return Err(status::OBJECT_NAME_COLLISION); }
                match host::real_name(&dir, &leaf) {
                    Ok(_) => return Err(status::OBJECT_NAME_COLLISION),
                    Err(status::OBJECT_NAME_NOT_FOUND) => (), Err(e) => return Err(e),
                }
                if directory { host::mkdir(dir.as_raw_fd(), &leaf) }
                else { host::open_at(dir.as_raw_fd(), &leaf, libc::O_RDWR | libc::O_CREAT | libc::O_EXCL).map(drop) }
            }
        }
    }
    pub fn remove(&mut self, rest: &[u8]) -> Result<(), u32> {
        match &mut self.storage {
            Storage::Disc(_) => Err(status::ACCESS_DENIED),
            Storage::Fatx(image) => image.change(|g, bytes| fatx::remove_path(g, bytes, rest)),
            Storage::Host { root, device } => {
                let (root, _) = host_root(root, device, rest)?;
                let (dir, leaf) = host::walk(&root, rest)?;
                if leaf.is_empty() { return Err(status::ACCESS_DENIED); }
                let real = host::real_name(&dir, &leaf)?;
                let s = host::stat_at(dir.as_raw_fd(), &real)?;
                let directory = host::kind(&s) == tsfp_engine_fileio::hostdir::Kind::Dir;
                if !directory && !host::regular(&s) { return Err(status::ACCESS_DENIED); }
                host::unlink(dir.as_raw_fd(), &real, directory)
            }
        }
    }
    pub fn rename(&mut self, old: &[u8], new: &[u8], replace: bool) -> Result<(), u32> {
        match &mut self.storage {
            Storage::Disc(_) => Err(status::ACCESS_DENIED),
            Storage::Fatx(image) => image.change(|g, bytes| fatx::rename_path(g, bytes, old, new, replace)),
            Storage::Host { root, device } => {
                let (root, _) = host_root(root, device, old)?;
                let (from_dir, from_leaf) = host::walk(&root, old)?;
                let (to_dir, to_leaf) = host::walk(&root, new)?;
                if from_leaf.is_empty() || to_leaf.is_empty() { return Err(status::ACCESS_DENIED); }
                let from = host::real_name(&from_dir, &from_leaf)?;
                let to = match host::real_name(&to_dir, &to_leaf) {
                    Ok(real) => { if !replace { return Err(status::OBJECT_NAME_COLLISION); } real },
                    Err(status::OBJECT_NAME_NOT_FOUND) => to_leaf, Err(e) => return Err(e),
                };
                host::rename(from_dir.as_raw_fd(), &from, to_dir.as_raw_fd(), &to)
            }
        }
    }
    pub fn list(&self, resource: &Resource, rest: &[u8]) -> Result<Vec<Listed>, u32> {
        match (&self.storage, resource) {
            (Storage::Disc(disc), Resource::Disc(entry)) => disc.read_dir(entry).map_err(disc_error)
                .map(|entries| entries.into_iter().map(|e| Listed { metadata: disc_meta(&e), name: e.name }).collect()),
            (Storage::Fatx(image), Resource::Fatx) => {
                let mut result = Vec::new();
                for index in 0..=u32::MAX {
                    let (entry, end) = fatx::directory_entry_at(&image.geometry, image.bytes(), rest, index).map_err(fatx_status)?;
                    if end { return Ok(result); }
                    if !entry.name.is_empty() { result.push(Listed { name: entry.name, metadata: Metadata {
                        size: entry.size.into(), allocation: entry.allocation, directory: entry.directory,
                        attributes: entry.attributes.into(), creation: entry.creation_time, access: entry.access_time,
                        write: entry.write_time, device: false, backing: 3, times_known: true,
                    } }); }
                }
                Err(status::ACCESS_DENIED)
            }
            (_, Resource::Host(dir)) => {
                let mut result = Vec::new();
                for name in host::names(dir.as_raw_fd())? {
                    let Ok(s) = host::stat_at(dir.as_raw_fd(), &name) else { continue; };
                    if host::regular(&s) || host::kind(&s) == tsfp_engine_fileio::hostdir::Kind::Dir {
                        result.push(Listed { name, metadata: Metadata::host(&s) });
                    }
                }
                Ok(result)
            }
            _ => Err(status::INVALID_HANDLE),
        }
    }
}
