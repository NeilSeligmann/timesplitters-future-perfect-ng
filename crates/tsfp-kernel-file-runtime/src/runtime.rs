use crate::{host, storage::{self, Cache, Device, FatxImage, Listed, Metadata, Resolved, Resource, Storage, Volume}};
use std::os::fd::AsRawFd;
use std::os::unix::{ffi::OsStrExt, fs::FileExt};
use std::path::Path;
use std::sync::OnceLock;
use tsfp_engine_fileio::{commit, dirpick, disposition, fatx_img as fatx, handles, lifecycle,
    namespace::{Namespace, Resolve}, vfs::status, xiso::Xiso};

#[derive(Default)]
pub struct Counters {
    pub refused: u32, pub fabricated: u32, pub relative_refused: u32,
    pub unmodelled_attributes: u32, pub attribute_queries: u32, pub deleted: u32,
    pub delete_refused: u32, pub disc_bytes_read: u64, pub disc_opened: u32,
    pub host_bytes_read: u64, pub host_bytes_written: u64, pub host_opened: u32,
    pub write_refused: u32, pub created: u32, pub escape_refused: u32,
    pub cache_view_opened: u32, pub device_opened: u32, pub device_zero_bytes: u64,
    pub share_conflicts: u32, pub share_refusals: u32, pub delete_on_close: u32,
}
pub struct Attempt {
    pub path: Vec<u8>, pub desired_access: u32, pub share_access: u32,
    pub open_options: u32, pub attempts: u32, pub opened: bool,
}
pub struct Open {
    pub handle: u32, pub path: Vec<u8>, pub storage_path: Vec<u8>, pub backing: u32,
    pub metadata: Metadata, pub offset: u64, pub desired_access: u32, pub share_access: u32,
    pub open_options: u32, pub writable: bool, pub cache_partition: u32,
    pub volume: Option<usize>, pub generation: u64, pub resource: Resource,
    pub delete_pending: bool, pub cursor: dirpick::OwnedCursor,
}
pub struct Runtime {
    pub namespace: Namespace, pub volumes: Vec<Option<Volume>>, pub opens: Vec<Option<Open>>,
    pub links: Vec<Option<(Vec<u8>, Vec<u8>)>>, pub symlink_handles: Vec<(u32, Vec<u8>)>,
    pub counters: Counters, pub attempts: Vec<Attempt>, pub openable: Vec<Vec<u8>>,
    pub missing_empty: bool, pub enforce_shares: bool, pub last_write_count: usize, generation: u64,
}
pub(crate) fn refused_overwrite(counters: &mut Counters) -> u32 {
    counters.write_refused = counters.write_refused.wrapping_add(1);
    status::ACCESS_DENIED
}
impl Default for Runtime {
    fn default() -> Self {
        let mut namespace = Namespace::new();
        for slot in 0..16 { namespace.set_mount(slot, None); }
        Self { namespace, volumes: (0..16).map(|_| None).collect(), opens: (0..64).map(|_| None).collect(),
            links: vec![None; 32], symlink_handles: Vec::new(), counters: Counters::default(),
            attempts: Vec::new(), openable: Vec::new(), missing_empty: false, enforce_shares: false, last_write_count: 0, generation: 0 }
    }
}
pub fn runtime() -> &'static crate::runtime_lock::RuntimeCell {
    static RUNTIME: OnceLock<crate::runtime_lock::RuntimeCell> = OnceLock::new();
    RUNTIME.get_or_init(|| crate::runtime_lock::RuntimeCell::new(Runtime::default()))
}
fn equal(a: &[u8], b: &[u8]) -> bool { a.eq_ignore_ascii_case(b) }
fn valid_path(path: &[u8]) -> bool { !path.is_empty() && path.len() < 261 && !path.contains(&0) }
fn has_dot_component(path: &[u8]) -> bool {
    path.split(|byte| matches!(byte, b'/' | b'\\')).any(|part| part == b"." || part == b"..")
}
fn delete_access(access: u32) -> bool { access & (commit::DELETE | 0x10000000 | 0x02000000) != 0 }
impl Runtime {
    pub fn reset(&mut self) {
        let enforce_shares = self.enforce_shares;
        let generation = self.generation;
        let delete_on_close = self.counters.delete_on_close;
        let share_conflicts = self.counters.share_conflicts;
        *self = Self::default();
        self.enforce_shares = enforce_shares;
        self.generation = generation;
        self.counters.delete_on_close = delete_on_close;
        self.counters.share_conflicts = share_conflicts;
    }
    pub fn add_openable(&mut self, path: &[u8]) -> bool {
        if !valid_path(path) { return false; }
        if self.openable.iter().any(|p| equal(p, path)) { return true; }
        if self.openable.len() == 32 { return false; }
        self.openable.push(path.to_vec()); true
    }
    pub fn record_attempt(&mut self, path: &[u8], desired: u32, share: u32, options: u32) -> usize {
        if let Some(slot) = self.attempts.iter().position(|a| equal(&a.path, path)) {
            self.attempts[slot].attempts = self.attempts[slot].attempts.wrapping_add(1); return slot;
        }
        if self.attempts.len() == 64 { return usize::MAX; }
        self.attempts.push(Attempt { path: path.to_vec(), desired_access: desired, share_access: share,
            open_options: options, attempts: 1, opened: false }); self.attempts.len() - 1
    }
    pub fn mark_opened(&mut self, index: usize) { if let Some(a) = self.attempts.get_mut(index) { a.opened = true; } }
    pub fn add_symlink(&mut self, name: &[u8], target: &[u8]) -> bool {
        if !valid_path(name) || !valid_path(target) { return false; }
        let Some(slot) = self.namespace.replace_link(name, target, 32) else { return false; };
        self.links[slot] = Some((name.to_vec(), target.to_vec())); true
    }
    pub fn remove_symlink(&mut self, name: &[u8]) -> bool {
        let Some(slot) = self.namespace.remove_link(name) else { return false; };
        self.links[slot] = None; true
    }
    pub fn symlink_target(&self, name: &[u8]) -> Option<&[u8]> {
        self.links.iter().flatten().find(|(n, _)| equal(n, name)).map(|(_, t)| t.as_slice())
    }
    pub fn link_target(&self, name: &[u8]) -> Option<Vec<u8>> { self.symlink_target(name).map(<[u8]>::to_vec) }
    pub fn add_link(&mut self, name: &[u8], target: &[u8], replace: bool) -> Result<(), u32> {
        if !replace && self.symlink_target(name).is_some() { return Err(status::OBJECT_NAME_COLLISION); }
        if self.add_symlink(name, target) { Ok(()) } else { Err(status::INSUFFICIENT_RESOURCES) }
    }
    pub fn remove_link(&mut self, name: &[u8]) -> bool { self.remove_symlink(name) }
    fn mount(&mut self, prefix: &[u8], storage: Storage, unique: bool) -> bool {
        if !valid_path(prefix) { return false; }
        let Some(slot) = self.namespace.mount_slot(unique.then_some(prefix)) else { return false; };
        self.generation = self.generation.wrapping_add(1);
        self.volumes[slot] = Some(Volume { prefix: prefix.to_vec(), generation: self.generation, storage });
        self.namespace.set_mount(slot, Some(prefix)); true
    }
    pub fn mount_disc(&mut self, prefix: &[u8], path: &[u8]) -> bool {
        if prefix.last().is_some_and(|b| *b == b'\\' || *b == b'/') {
            crate::guest::log_mount_refused(prefix, path, true);
            return false;
        }
        if lifecycle::validate_disc_prefix(prefix).is_err() { return false; }
        let Ok(disc) = Xiso::open(Path::new(std::ffi::OsStr::from_bytes(path))) else { return false; };
        self.mount(prefix, Storage::Disc(disc), false)
    }
    pub fn mount_host(&mut self, prefix: &[u8], path: &[u8]) -> bool {
        if prefix.last().is_some_and(|b| *b == b'\\' || *b == b'/') {
            crate::guest::log_mount_refused(prefix, path, true);
            return false;
        }
        if lifecycle::validate_host_directory(prefix, path).is_err() { return false; }
        let Ok(root) = host::root(path) else {
            crate::guest::log_mount_refused(prefix, path, false);
            return false;
        };
        self.mount(prefix, Storage::Host { root, device: None }, false)
    }
    pub fn mount_device(&mut self, prefix: &[u8], path: &[u8], name: &[u8], capacity: u64,
        cache: Option<(u32, &[u8])>) -> bool {
        let cache_number = cache.map(|(number, _)| number);
        let spec = lifecycle::DeviceMount { prefix, root: path, image: name,
            content: cache.map(|(_, name)| name), cache: cache.map_or(0, |(number, _)| number), capacity };
        if lifecycle::validate_device(spec) != 0 { return false; }
        let Ok(root) = host::root(path) else { return false; };
        match host::stat_at(root.as_raw_fd(), name) {
            Ok(s) if !host::regular(&s) => {
                crate::guest::log_device_mount_nonregular(prefix, path, name);
                return false;
            }
            Ok(s) if s.st_size < 0 || s.st_size as u64 > capacity => return false,
            Ok(_) | Err(status::OBJECT_NAME_NOT_FOUND) => (), Err(_) => return false,
        }
        let Ok(file) = host::open_at(root.as_raw_fd(), name, libc::O_RDWR | libc::O_CREAT) else { return false; };
        let Ok(s) = host::stat(&file) else { return false; };
        if !host::regular(&s) || s.st_size < 0 || s.st_size as u64 > capacity { return false; }
        let mounted = self.mount(prefix, Storage::Host { root, device: Some(Device { name: name.to_vec(), capacity,
            cache: cache.map(|(number, name)| Cache { number, name: name.to_vec(), root: None }) }) }, true);
        if mounted { crate::guest::log_device_mount(prefix, path, name, capacity, cache_number); }
        mounted
    }
    /// Caller retains the MU byte image until unmount; its access must share this runtime lock.
    pub unsafe fn mount_fatx(&mut self, prefix: &[u8], address: *mut u8, length: usize,
        flush: Option<unsafe extern "C" fn(*mut libc::c_void) -> bool>, context: usize) -> bool {
        let Ok(image) = (unsafe { FatxImage::new(address, length, flush, context) }) else { return false; };
        self.mount(prefix, Storage::Fatx(image), true)
    }
    pub fn unmount_fatx(&mut self, prefix: &[u8]) -> bool {
        let Some(slot) = self.volumes.iter().position(|v| v.as_ref().is_some_and(|v|
            v.prefix == prefix && matches!(v.storage, Storage::Fatx(_)))) else { return false; };
        self.volumes[slot] = None; self.namespace.set_mount(slot, None); true
    }
    pub fn unmount_all(&mut self) {
        self.opens.iter_mut().for_each(|o| *o = None);
        for (slot, v) in self.volumes.iter_mut().enumerate() { *v = None; self.namespace.set_mount(slot, None); }
    }
    pub fn locate(&mut self, path: &[u8]) -> Result<(Vec<u8>, Option<(usize, usize)>), u32> {
        if !valid_path(path) { return Err(status::INVALID_PARAMETER); }
        if has_dot_component(path) {
            self.counters.escape_refused = self.counters.escape_refused.wrapping_add(1);
            crate::guest::log_path_escape(path);
            return Err(status::ACCESS_DENIED);
        }
        let Resolve::Settled(resolved) = self.namespace.resolve(path, 261) else { return Err(status::OBJECT_PATH_NOT_FOUND); };
        let volume = self.namespace.volume_for(&resolved); Ok((resolved, volume))
    }
    fn resolve(&mut self, path: &[u8], access: u32, stored_only: bool) -> Result<(Resolved, Option<usize>, Vec<u8>, u64), u32> {
        let (resolved, location) = self.locate(path)?;
        if let Some((slot, at)) = location {
            let volume = self.volumes[slot].as_mut().ok_or(status::OBJECT_PATH_NOT_FOUND)?;
            let target = match volume.resolve(&resolved[at..], access) {
                Err(host::SYMLINK_REFUSED) => {
                    self.counters.escape_refused = self.counters.escape_refused.wrapping_add(1);
                    crate::guest::log_host_symlink(path);
                    return Err(status::ACCESS_DENIED);
                }
                result => result?,
            };
            return Ok((target, Some(slot), resolved[at..].to_vec(), volume.generation));
        }
        if !stored_only && (self.openable.iter().any(|p| equal(p, path)) || self.missing_empty) {
            return Ok((Resolved { resource: Resource::Empty, metadata: Metadata::default(), backing: 0, cache: false }, None, Vec::new(), 0));
        }
        Err(status::OBJECT_NAME_NOT_FOUND)
    }
    pub fn query(&mut self, path: &[u8]) -> Result<Metadata, u32> { self.resolve(path, 0, false).map(|v| v.0.metadata) }
    pub fn open(&mut self, path: &[u8], desired: u32, share: u32, options: u32, disposition: u32,
        handle: u32, stored_only: bool) -> Result<u32, u32> {
        self.open_allocate(path, desired, share, options, disposition, stored_only, || handle).map(|(_, info)| info)
    }
    pub fn open_allocate(&mut self, path: &[u8], desired: u32, share: u32, options: u32, disposition: u32,
        stored_only: bool, allocate: impl FnOnce() -> u32) -> Result<(u32, u32), u32> {
        let result = self.open_inner(path, desired, share, options, disposition, stored_only, allocate);
        if result.is_err() { self.counters.refused = self.counters.refused.wrapping_add(1); }
        result
    }
    fn open_inner(&mut self, path: &[u8], desired: u32, share: u32, options: u32, disposition: u32,
        stored_only: bool, allocate: impl FnOnce() -> u32) -> Result<(u32, u32), u32> {
        if !valid_path(path) { return Err(status::INVALID_PARAMETER); }
        if self.opens.iter().flatten().any(|o| equal(&o.path, path) && o.delete_pending) { return Err(commit::STATUS_DELETE_PENDING); }
        if let Some(e) = commit::delete_on_close_at_open(options, desired) { return Err(e); }
        let mut information = disposition::INFO_OPENED;
        let resolution = self.resolve(path, desired, stored_only);
        let (mut from, volume_slot, storage_path, generation) = match resolution {
            Ok(value) => value,
            Err(status::OBJECT_NAME_NOT_FOUND) if disposition::creates(disposition) => {
                let (resolved, location) = self.locate(path)?;
                let Some((volume_slot, at)) = location else {
                    crate::guest::log_create_refused(path, disposition, status::OBJECT_NAME_NOT_FOUND);
                    return Err(status::OBJECT_NAME_NOT_FOUND);
                };
                let volume = self.volumes[volume_slot].as_mut().unwrap();
                if volume.backing() == 1 { return Err(status::OBJECT_NAME_NOT_FOUND); }
                if matches!(&volume.storage, Storage::Host { device: Some(device), .. } if device.cache.is_none())
                    && resolved[at..].is_empty() { return Err(status::ACCESS_DENIED); }
                volume.create(&resolved[at..], options & 1 != 0)?;
                if options & 1 != 0 {
                    crate::guest::log_created_directory(path);
                } else {
                    crate::guest::log_created_file(path, storage::writable(desired));
                }
                self.counters.created += 1; information = disposition::INFO_CREATED;
                self.resolve(path, desired, stored_only)?
            }
            Err(e) => return Err(e),
        };
        if information != disposition::INFO_CREATED {
            if from.metadata.device && disposition::creates(disposition) && storage::writable(desired) {
                return Err(status::ACCESS_DENIED);
            }
            let facts = disposition::Target { exists: true, fatx: from.backing == 3, host_dir: from.backing == 2,
                is_directory: from.metadata.directory, writable: storage::writable(desired),
                has_host_fd: matches!(from.resource, Resource::Host(_)), device: from.metadata.device, ..Default::default() };
            match disposition::plan(disposition, &facts) {
                disposition::Plan::Collision => return Err(status::OBJECT_NAME_COLLISION),
                disposition::Plan::RefuseOverwrite => {
                    crate::guest::log_overwrite_disposition_refused(path);
                    return Err(refused_overwrite(&mut self.counters));
                }
                disposition::Plan::TruncateHost { information: info } => {
                    let Resource::Host(file) = &from.resource else { unreachable!() };
                    file.set_len(0).map_err(host::nt_error)?;
                    crate::guest::log_truncated_to_zero(path, disposition);
                    from.metadata.size = 0; information = info;
                }
                disposition::Plan::TruncateFatx { information: info } => {
                    if !storage::writable(desired) || from.metadata.directory { return Err(status::ACCESS_DENIED); }
                    let Storage::Fatx(image) = &mut self.volumes[volume_slot.unwrap()].as_mut().unwrap().storage else { unreachable!() };
                    image.change(|g, bytes| fatx::resize_path(g, bytes, &storage_path, 0, 0, &[]))?;
                    crate::guest::log_truncated_to_zero(path, disposition);
                    from.metadata.size = 0; information = info;
                }
                _ => (),
            }
        }
        if from.metadata.device { crate::guest::log_device_open(path); }
        let cache_partition = volume_slot.and_then(|v| self.volumes[v].as_ref()).and_then(|v| match &v.storage {
            Storage::Host { device: Some(device), .. } if from.metadata.device => device.cache.as_ref().map(|c| c.number), _ => None,
        }).unwrap_or(0);
        let handle = allocate();
        if handle == 0 { return Err(status::INSUFFICIENT_RESOURCES); }
        let slot = self.opens.iter().position(Option::is_none).ok_or(status::INSUFFICIENT_RESOURCES)?;
        let conflict = self.opens.iter().flatten().any(|o| equal(&o.path, path) &&
            handles::share_conflicts(o.desired_access, o.share_access, desired, share));
        if conflict {
            self.counters.share_conflicts = self.counters.share_conflicts.wrapping_add(1);
            if self.enforce_shares { self.counters.share_refusals += 1; return Err(commit::STATUS_SHARING_VIOLATION); }
        }
        match from.backing { 1 => self.counters.disc_opened += 1, 2 => self.counters.host_opened += 1,
            0 if !self.openable.iter().any(|p| equal(p, path)) => self.counters.fabricated += 1, _ => () }
        if from.cache { self.counters.cache_view_opened += 1; }
        if from.metadata.device { self.counters.device_opened += 1; }
        let writable = matches!(from.backing, 2 | 3) && !from.metadata.directory && storage::writable(desired);
        self.opens[slot] = Some(Open { handle, path: path.to_vec(), storage_path, backing: from.backing,
            metadata: from.metadata, offset: 0, desired_access: desired, share_access: share, open_options: options,
            writable, cache_partition, volume: volume_slot, generation, resource: from.resource,
            delete_pending: options & commit::FILE_DELETE_ON_CLOSE != 0, cursor: dirpick::OwnedCursor::default() });
        Ok((handle, information))
    }
    fn index(&self, handle: u32) -> Result<usize, u32> {
        self.opens.iter().position(|o| o.as_ref().is_some_and(|o| o.handle == handle)).ok_or(status::INVALID_HANDLE)
    }
    pub fn open_info(&self, handle: u32) -> Option<&Open> { self.index(handle).ok().and_then(|i| self.opens[i].as_ref()) }
    pub fn set_offset(&mut self, handle: u32, offset: u64) -> bool {
        let Ok(slot) = self.index(handle) else { return false; }; self.opens[slot].as_mut().unwrap().offset = offset; true
    }
    fn live_volume(&self, entry: &Open) -> Result<&Volume, u32> {
        entry.volume.and_then(|v| self.volumes[v].as_ref()).filter(|v| v.generation == entry.generation).ok_or(status::INVALID_HANDLE)
    }
    pub fn read(&mut self, handle: u32, offset: u64, buffer: &mut [u8], stored_only: bool) -> Result<usize, u32> {
        let slot = self.index(handle)?;
        let entry = self.opens[slot].as_ref().unwrap();
        if entry.metadata.directory {
            crate::guest::log_directory_read_refused(&entry.path);
            return Err(status::INVALID_PARAMETER);
        }
        match &entry.resource {
            Resource::Empty => if stored_only { Err(status::INVALID_HANDLE) } else { Ok(0) },
            Resource::Host(file) => {
                let wanted = if entry.metadata.device { buffer.len().min(entry.metadata.size.saturating_sub(offset) as usize) } else { buffer.len() };
                if wanted != 0 { offset.checked_add(wanted as u64).filter(|end| *end <= i64::MAX as u64).ok_or(status::INVALID_PARAMETER)?; }
                let mut done = 0;
                while done < wanted { match file.read_at(&mut buffer[done..wanted], offset + done as u64) {
                    Ok(0) => break, Ok(n) => done += n,
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(host::nt_error(e)),
                } }
                self.counters.host_bytes_read += done as u64;
                if entry.metadata.device && !stored_only && done < wanted {
                    let zeroes = wanted - done;
                    buffer[done..wanted].fill(0);
                    self.counters.device_zero_bytes += zeroes as u64; done = wanted;
                    crate::guest::log_device_zero_fill(&entry.path, offset, zeroes);
                }
                Ok(done)
            }
            Resource::Disc(target) => {
                let Storage::Disc(disc) = &self.live_volume(entry)?.storage else { return Err(status::INVALID_HANDLE); };
                let count = disc.read(target, offset, buffer).map_err(|e| tsfp_engine_fileio::vfs::nt_from_xiso(&e))?;
                self.counters.disc_bytes_read += count as u64; Ok(count)
            }
            Resource::Fatx => {
                let Storage::Fatx(image) = &self.live_volume(entry)?.storage else { return Err(status::INVALID_HANDLE); };
                fatx::read_range(&image.geometry, image.bytes(), &entry.storage_path, offset, buffer).map(|n| n as usize).map_err(storage::fatx_status)
            }
        }
    }
    pub fn write(&mut self, handle: u32, offset: u64, buffer: &[u8]) -> Result<usize, u32> {
        self.last_write_count = 0;
        let result = self.write_inner(handle, offset, buffer);
        if result.is_err() { self.counters.write_refused += 1; } result
    }
    fn write_inner(&mut self, handle: u32, offset: u64, buffer: &[u8]) -> Result<usize, u32> {
        let slot = self.index(handle)?;
        let entry = self.opens[slot].as_ref().unwrap();
        if entry.metadata.device {
            if let Err(error) = validate_device_write_range(offset, buffer.len(), entry.metadata.size) {
                crate::guest::log_device_write_past_capacity(&entry.path, offset, buffer.len(), entry.metadata.size);
                return Err(error);
            }
        }
        if !entry.writable || entry.metadata.directory {
            crate::guest::log_write_refused(&entry.path, entry.backing, entry.writable, entry.metadata.directory);
            return Err(status::ACCESS_DENIED);
        }
        if !matches!(entry.backing, 2 | 3) {
            crate::guest::log_write_refused(&entry.path, entry.backing, entry.writable, false);
            return Err(status::ACCESS_DENIED);
        }
        if matches!(entry.resource, Resource::Fatx) {
            let Storage::Fatx(image) = &self.live_volume(entry)?.storage else { return Err(status::INVALID_HANDLE); };
            // The image, rather than another handle's cached EOF, owns the size.
            // Even an empty write validates the mount generation and current name.
            let (_, current_size) = fatx::stat_path(&image.geometry, image.bytes(), &entry.storage_path)
                .map_err(storage::fatx_status)?;
            let size = if buffer.is_empty() { u64::from(current_size) } else {
                let end = offset.checked_add(buffer.len() as u64).filter(|end| *end <= u64::from(u32::MAX))
                    .ok_or(status::INVALID_PARAMETER)?;
                u64::from(current_size).max(end)
            };
            if !buffer.is_empty() {
                let path = entry.storage_path.clone(); let volume = entry.volume.unwrap();
                let Storage::Fatx(image) = &mut self.volumes[volume].as_mut().unwrap().storage else { return Err(status::INVALID_HANDLE); };
                image.change(|g, bytes| fatx::resize_path(g, bytes, &path, size, offset, buffer))?;
            }
            self.opens[slot].as_mut().unwrap().metadata.size = size;
            self.last_write_count = buffer.len();
            return Ok(buffer.len());
        }
        if buffer.is_empty() { return Ok(0); }
        let end = offset.checked_add(buffer.len() as u64).filter(|end| *end <= i64::MAX as u64).ok_or(status::INVALID_PARAMETER)?;
        match &entry.resource {
            Resource::Host(file) => {
                let mut done = 0;
                while done < buffer.len() { match file.write_at(&buffer[done..], offset + done as u64) {
                    Ok(0) => return Err(status::ACCESS_DENIED), Ok(n) => { done += n; self.last_write_count = done; self.counters.host_bytes_written += n as u64; },
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(host::nt_error(e)),
                } }
            }
            _ => return Err(status::ACCESS_DENIED),
        }
        let entry = self.opens[slot].as_mut().unwrap(); entry.metadata.size = entry.metadata.size.max(end);
        self.last_write_count = buffer.len();
        Ok(buffer.len())
    }
    pub fn resize(&mut self, handle: u32, length: u64, allocation_only: bool) -> Result<(), u32> {
        let slot = self.index(handle)?; let entry = self.opens[slot].as_ref().unwrap();
        if !entry.writable || entry.metadata.directory { self.counters.write_refused += 1; return Err(status::ACCESS_DENIED); }
        if entry.metadata.device {
            self.counters.write_refused += 1;
            crate::guest::log_device_fixed_length(&entry.path);
            return Err(status::INVALID_PARAMETER);
        }
        if length > i64::MAX as u64 { self.counters.write_refused += 1; return Err(status::INVALID_PARAMETER); }
        match &entry.resource {
            Resource::Host(file) => {
                if allocation_only {
                    let s = host::stat(file)?;
                    if s.st_size < 0 || s.st_blksize <= 0 { return Err(0xc0000001); }
                    let retained = length.max(s.st_size as u64);
                    let block = s.st_blksize as u64;
                    let release_at = retained.checked_add(block - 1).filter(|n| *n <= i64::MAX as u64)
                        .ok_or(status::INVALID_PARAMETER)? / block * block;
                    if retained != 0 && unsafe { libc::fallocate(file.as_raw_fd(), libc::FALLOC_FL_KEEP_SIZE, 0, retained as i64) } < 0 { return Err(host::last_error()); }
                    if release_at < i64::MAX as u64 && unsafe { libc::fallocate(file.as_raw_fd(), libc::FALLOC_FL_PUNCH_HOLE | libc::FALLOC_FL_KEEP_SIZE,
                        release_at as i64, i64::MAX - release_at as i64) } < 0 { return Err(host::last_error()); }
                } else { file.set_len(length).map_err(host::nt_error)?; }
            }
            Resource::Fatx if !allocation_only => {
                self.live_volume(entry)?; let path = entry.storage_path.clone(); let volume = entry.volume.unwrap();
                let Storage::Fatx(image) = &mut self.volumes[volume].as_mut().unwrap().storage else { return Err(status::INVALID_HANDLE); };
                image.change(|g, bytes| fatx::resize_path(g, bytes, &path, length, 0, &[]))?;
            }
            Resource::Fatx => return Err(0xc0000002),
            _ => return Err(status::ACCESS_DENIED),
        }
        if !allocation_only { self.opens[slot].as_mut().unwrap().metadata.size = length; } Ok(())
    }
    pub fn dir_next(&mut self, handle: u32, restart: bool, mask: Option<&[u8]>, max_name: usize) -> Result<Listed, u32> {
        let slot = self.index(handle)?;
        let entry = self.opens[slot].as_mut().unwrap();
        if !entry.metadata.directory { return Err(status::INVALID_PARAMETER); }
        if !matches!(entry.backing, 2 | 3) {
            crate::guest::log_directory_query_not_implemented(&entry.path);
            return Err(0xc0000002);
        }
        if entry.cursor.begin(entry.backing == 2, restart, mask) == dirpick::Begin::Refused { return Err(status::INVALID_PARAMETER); }
        let entry = self.opens[slot].as_ref().unwrap();
        let volume = self.live_volume(entry)?;
        let mut entries = volume.list(&entry.resource, &entry.storage_path)?;
        entries.sort_by(|a, b| dirpick::name_order(&a.name, &b.name));
        let picked = entries.into_iter().find(|e| dirpick::mask_matches(&entry.cursor.mask, &e.name) &&
            entry.cursor.last.as_ref().is_none_or(|last| dirpick::name_order(&e.name, last).is_gt()));
        let entry = self.opens[slot].as_mut().unwrap();
        let Some(picked) = picked else {
            return Err(entry.cursor.finish(0, None));
        };
        if picked.name.len() > max_name { return Err(0xc0000004); }
        entry.cursor.finish(1, Some(&picked.name)); Ok(picked)
    }
    pub fn volume_space(&self, handle: u32) -> Result<(u64, u64, u32), u32> {
        let entry = self.open_info(handle).ok_or(status::INVALID_HANDLE)?;
        match &self.live_volume(entry)?.storage {
            Storage::Host { root, device: None } => host::space(root).map(|(total, free)| (total, free, 32)),
            Storage::Fatx(image) => Ok((u64::from(image.geometry.cluster_count),
                u64::from(fatx::free_clusters(&image.geometry, image.bytes())), fatx::CLUSTER_SECTORS)),
            _ => Err(status::INVALID_DEVICE_REQUEST),
        }
    }
    pub fn flush(&self, handle: u32) -> Result<(), u32> {
        let entry = self.open_info(handle).ok_or(status::INVALID_HANDLE)?;
        let Storage::Fatx(image) = &self.live_volume(entry)?.storage else { return Err(status::INVALID_HANDLE); };
        if image.flush() { Ok(()) } else { Err(status::ACCESS_DENIED) }
    }
    pub fn delete(&mut self, path: &[u8]) -> u32 {
        let result = self.delete_inner(path);
        match result {
            Ok(()) => {
                self.counters.deleted += 1;
                crate::guest::log_delete_succeeded(path);
                0
            }
            Err(e) if e == status::OBJECT_NAME_NOT_FOUND || e == status::OBJECT_PATH_NOT_FOUND => e,
            Err(e) => { self.counters.delete_refused += 1; e }
        }
    }
    fn delete_inner(&mut self, path: &[u8]) -> Result<(), u32> {
        if self.opens.iter().flatten().any(|o| equal(&o.path, path)) {
            crate::guest::log_delete_refused(path, "SHARING violation");
            return Err(commit::STATUS_SHARING_VIOLATION);
        }
        let (resolved, location) = self.locate(path)?;
        let Some((slot, at)) = location else {
            if self.openable.iter().any(|candidate| equal(candidate, path)) || self.missing_empty {
                crate::guest::log_delete_refused(path, "no volume backs this synthetic file");
                return Err(0xC000_0002); // STATUS_NOT_IMPLEMENTED: synthetic files are not deletable.
            }
            return Err(status::OBJECT_NAME_NOT_FOUND);
        };
        let volume = self.volumes[slot].as_mut().ok_or(status::INVALID_HANDLE)?;
        if resolved[at..].iter().all(|byte| matches!(byte, b'/' | b'\\')) {
            crate::guest::log_delete_refused(&resolved[at..], "volume root");
            return Err(status::ACCESS_DENIED);
        }
        if matches!(&volume.storage, Storage::Disc(_)) {
            crate::guest::log_delete_refused(&resolved[at..], "disc volume");
            return Err(status::ACCESS_DENIED);
        }
        if matches!(&volume.storage, Storage::Host { device: Some(device), .. } if device.cache.is_none()) {
            crate::guest::log_delete_refused(&resolved[at..], "raw device");
            return Err(status::ACCESS_DENIED);
        }
        let target = match volume.resolve(&resolved[at..], 0) {
            Ok(target) => target,
            Err(host::SYMLINK_REFUSED) => {
                self.counters.escape_refused = self.counters.escape_refused.wrapping_add(1);
                crate::guest::log_path_escape(&resolved[at..]);
                crate::guest::log_delete_refused(&resolved[at..], "symbolic link escape");
                return Err(status::ACCESS_DENIED);
            }
            Err(error) => return Err(error),
        };
        if target.metadata.device {
            crate::guest::log_delete_refused(&resolved[at..], "raw device");
            return Err(status::ACCESS_DENIED);
        }
        if target.metadata.directory {
            crate::guest::log_delete_refused(&resolved[at..], "directory");
            return Err(0xC000_0002); // STATUS_NOT_IMPLEMENTED: directories are never deleted.
        }
        volume.remove(&resolved[at..])
    }
    pub fn set_disposition(&mut self, handle: u32, delete: bool) -> u32 {
        let Ok(slot) = self.index(handle) else { return status::INVALID_HANDLE; };
        let entry = self.opens[slot].as_ref().unwrap();
        let directory_empty = if entry.metadata.directory {
            self.live_volume(entry).and_then(|v| v.list(&entry.resource, &entry.storage_path)).is_ok_and(|list| list.is_empty())
        } else { true };
        let facts = commit::DeleteTarget { granted_delete: delete_access(entry.desired_access), read_only: false,
            is_directory: entry.metadata.directory, directory_empty, volume_root: entry.storage_path.iter().all(|b| matches!(b, b'/' | b'\\')),
            device: entry.metadata.device, writable_volume: matches!(entry.backing, 2 | 3) };
        match commit::set_disposition(delete, &facts) {
            Ok(pending) => { let path = entry.path.clone(); for o in self.opens.iter_mut().flatten().filter(|o| equal(&o.path, &path)) { o.delete_pending = pending; } 0 },
            Err(e) => e,
        }
    }
    pub fn close(&mut self, handle: u32) -> Result<(), u32> {
        let slot = self.index(handle)?; let entry = self.opens[slot].take().unwrap();
        if entry.delete_pending && !self.opens.iter().flatten().any(|o| equal(&o.path, &entry.path)) {
            if let Some(volume) = entry.volume.and_then(|v| self.volumes[v].as_mut()).filter(|v| v.generation == entry.generation) {
                volume.remove(&entry.storage_path)?; self.counters.delete_on_close += 1;
            }
        }
        Ok(())
    }
    pub fn rename(&mut self, handle: u32, replace: bool, target: &[u8]) -> u32 {
        self.rename_inner(handle, replace, target).map_or_else(|e| e, |_| 0)
    }
    fn rename_inner(&mut self, handle: u32, replace: bool, target: &[u8]) -> Result<(), u32> {
        let slot = self.index(handle)?;
        let (desired_access, device, backing, delete_pending, old_volume, is_root, old, old_storage) = {
            let entry = self.opens[slot].as_ref().unwrap();
            (entry.desired_access, entry.metadata.device, entry.backing, entry.delete_pending,
             entry.volume, entry.storage_path.iter().all(|b| matches!(b, b'/' | b'\\')),
             entry.path.clone(), entry.storage_path.clone())
        };
        if !delete_access(desired_access) || device || !matches!(backing, 2 | 3) { return Err(status::ACCESS_DENIED); }
        if delete_pending { return Err(commit::STATUS_DELETE_PENDING); }
        if is_root { return Err(status::ACCESS_DENIED); }
        {
            let entry = self.opens[slot].as_ref().unwrap();
            self.live_volume(entry)?;
        }
        let (resolved, location) = self.locate(target)?;
        let (volume, at) = location.ok_or(status::OBJECT_PATH_NOT_FOUND)?;
        if old_volume != Some(volume) { return Err(commit::STATUS_NOT_SAME_DEVICE); }
        let new_storage = resolved[at..].to_vec();
        if equal(&old, target) { return Ok(()); }
        if self.opens.iter().flatten().any(|o| equal(&o.path, target)) { return Err(commit::STATUS_SHARING_VIOLATION); }
        self.volumes[volume].as_mut().unwrap().rename(&old_storage, &new_storage, replace)?;
        for entry in self.opens.iter_mut().flatten() {
            if entry.volume != Some(volume) { continue; }
            if let Some(new) = commit::rebase_path(&entry.path, &old, target) { if new.len() < 261 { entry.path = new; } }
            if let Some(new) = commit::rebase_path(&entry.storage_path, &old_storage, &new_storage) { if new.len() < 261 { entry.storage_path = new; } }
        }
        Ok(())
    }
}

pub(crate) fn validate_device_write_range(offset: u64, length: usize, size: u64) -> Result<(), u32> {
    if length == 0 { return Ok(()); }
    let end = offset.checked_add(length as u64).filter(|end| *end <= i64::MAX as u64)
        .ok_or(status::INVALID_PARAMETER)?;
    if end > size { return Err(status::INVALID_PARAMETER); }
    Ok(())
}
