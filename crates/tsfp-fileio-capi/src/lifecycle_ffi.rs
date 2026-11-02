//! Rust-owned lifecycle records with callbacks for remaining C resource effects.
use std::ffi::{CStr, c_char, c_void};
use std::sync::Mutex;
use tsfp_engine_fileio::{
    acquisition::Backing,
    lifecycle::{self, Descriptor, Volume, VolumeHost, VolumeTable},
};
const C_BACKING_DISC: u32 = 1;

// Explicit Rust-runtime ABI for the removable-media lifecycle. Keeping these
// names out of C callers makes the owner boundary visible to the retirement
// planner while preserving the selected C oracle in legacy builds.
unsafe extern "C" {
    fn kernel_file_mount_fatx(
        prefix: *const c_char,
        image: *mut c_void,
        flush: Option<unsafe extern "C" fn(*mut c_void) -> bool>,
        context: *mut c_void,
    ) -> bool;
    fn kernel_file_unmount_fatx(prefix: *const c_char) -> bool;
    fn kernel_file_add_symlink(name: *const c_char, target: *const c_char) -> bool;
    fn kernel_file_remove_symlink(name: *const c_char) -> bool;
    fn kernel_file_run_locked(job: Option<unsafe extern "C" fn(*mut c_void)>, context: *mut c_void);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_mount_fatx(
    prefix: *const c_char,
    image: *mut c_void,
    flush: Option<unsafe extern "C" fn(*mut c_void) -> bool>,
    context: *mut c_void,
) -> bool {
    unsafe { kernel_file_mount_fatx(prefix, image, flush, context) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_unmount_fatx(prefix: *const c_char) -> bool {
    unsafe { kernel_file_unmount_fatx(prefix) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_add_symlink(name: *const c_char, target: *const c_char) -> bool {
    unsafe { kernel_file_add_symlink(name, target) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_remove_symlink(name: *const c_char) -> bool {
    unsafe { kernel_file_remove_symlink(name) }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rfio_run_locked(
    job: Option<unsafe extern "C" fn(*mut c_void)>,
    context: *mut c_void,
) {
    unsafe { kernel_file_run_locked(job, context) }
}
#[repr(C)]
pub struct Callbacks {
    context: *mut c_void,
    close_disc: unsafe extern "C" fn(*mut c_void, usize),
    clear: unsafe extern "C" fn(*mut c_void, i32),
    acquire_device: unsafe extern "C" fn(*mut c_void, usize, i32) -> bool,
    count: usize,
}
struct Host<'a>(&'a Callbacks);
impl Host<'_> {
    fn count(&self) -> usize {
        self.0.count
    }
}
static VOLUME_TABLE: Mutex<Option<VolumeTable>> = Mutex::new(None);

fn publish_rust_volume(slot: usize, slot_count: usize, volume: Volume) {
    let mut tables = VOLUME_TABLE.lock().unwrap();
    if tables
        .as_ref()
        .is_none_or(|table| table.capacity() != slot_count)
    {
        *tables = Some(VolumeTable::new(slot_count));
    }
    let _ = tables.as_mut().unwrap().set(slot, Some(volume));
}

/// Publish a FATX logical record when the Rust epoch table commits a mount.
/// Publish using the kernel's configured capacity, even before any lifecycle
/// query has initialized the Rust logical table.
pub(crate) fn publish_fatx_volume(slot: usize, slot_count: usize, prefix: &[u8]) {
    publish_rust_volume(
        slot,
        slot_count,
        Volume {
            prefix: prefix.to_vec(),
            backing: Backing::Fatx,
            root: None,
            content: None,
            device: Vec::new(),
            capacity: 0,
            disc_handle: None,
        },
    );
}

fn clear_rust_volume(slot: Option<usize>, slot_count: usize) {
    let mut tables = VOLUME_TABLE.lock().unwrap();
    if tables
        .as_ref()
        .is_none_or(|table| table.capacity() != slot_count)
    {
        *tables = Some(VolumeTable::new(slot_count));
    }
    let table = tables.as_mut().unwrap();
    match slot {
        Some(slot) => {
            let _ = table.set(slot, None);
        }
        None => *table = VolumeTable::new(slot_count),
    }
}

/// Rust becomes the logical owner of a cache directory descriptor as soon as
/// the selected cache adapter publishes it. The C field remains a shadow for
/// code paths that still inspect the compatibility record.
pub(crate) unsafe extern "C" fn publish_content_descriptor(slot: usize, fd: i32) -> bool {
    let mut tables = VOLUME_TABLE.lock().unwrap();
    tables
        .as_mut()
        .is_some_and(|table| table.set_content(slot, Descriptor(fd)))
}

/// Return the Rust-owned logical volume table, initializing only its configured
/// capacity. C callbacks remain the resource host; they are no longer consulted
/// to reconstruct Rust's slot or liveness decisions.
fn logical_volume_table(slot_count: usize) -> VolumeTable {
    let mut tables = VOLUME_TABLE.lock().unwrap();
    if tables
        .as_ref()
        .is_none_or(|table| table.capacity() != slot_count)
    {
        *tables = Some(VolumeTable::new(slot_count));
    }
    tables.as_ref().unwrap().clone()
}
impl VolumeHost for Host<'_> {
    fn close_disc(&mut self, slot: usize, backend_handle: Option<u32>) {
        if let Some(handle) = backend_handle {
            unsafe { crate::close(handle) };
        } else {
            unsafe { (self.0.close_disc)(self.0.context, slot) };
        }
    }
    fn close(&mut self, fd: Descriptor) {
        unsafe { crate::hostio_ffi::close(fd.0) };
    }
    fn clear(&mut self, slot: Option<usize>) {
        unsafe { (self.0.clear)(self.0.context, slot.map_or(-1, |i| i as i32)) };
    }
    fn acquire_device(&mut self, slot: usize, root: Descriptor) -> bool {
        unsafe { (self.0.acquire_device)(self.0.context, slot, root.0) }
    }
}
pub unsafe extern "C" fn mount_slot(host: *const Callbacks, unique: *const c_char) -> i32 {
    if host.is_null() {
        return -1;
    }
    crate::counters::bump(crate::counters::VOLUME_LIFECYCLE);
    let unique = if unique.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(unique) }.to_bytes())
    };
    let mut host = Host(unsafe { &*host });
    logical_volume_table(host.count())
        .mount_slot(unique)
        .map_or(-1, |i| i as i32)
}

pub unsafe extern "C" fn volume_count(host: *const Callbacks) -> u32 {
    if host.is_null() {
        return 0;
    }
    crate::counters::bump(crate::counters::VOLUME_LIFECYCLE);
    let host = Host(unsafe { &*host });
    logical_volume_table(host.count()).live_count() as u32
}

pub unsafe extern "C" fn unmount_all(host: *const Callbacks) {
    if host.is_null() {
        return;
    }
    crate::counters::bump(crate::counters::VOLUME_LIFECYCLE);
    let mut host = Host(unsafe { &*host });
    let mut table = logical_volume_table(host.count());
    table.unmount_all(&mut host);
    clear_rust_volume(None, host.count());
}
pub unsafe extern "C" fn unmount_fatx(host: *const Callbacks, prefix: *const c_char) -> bool {
    if host.is_null() || prefix.is_null() {
        return false;
    }
    crate::counters::bump(crate::counters::VOLUME_LIFECYCLE);
    let mut host = Host(unsafe { &*host });
    let mut table = logical_volume_table(host.count());
    let slot = (0..table.capacity()).find(|&slot| {
        table.get(slot).is_some_and(|volume| {
            volume.backing == Backing::Fatx
                && volume.prefix == unsafe { CStr::from_ptr(prefix) }.to_bytes()
        })
    });
    let result = table.unmount_fatx(&mut host, unsafe { CStr::from_ptr(prefix) }.to_bytes());
    if result && let Some(slot) = slot {
        clear_rust_volume(Some(slot), host.count());
    }
    result
}
pub unsafe extern "C" fn acquire_device(host: *const Callbacks, prefix: *const c_char) -> bool {
    if host.is_null() || prefix.is_null() {
        return false;
    }
    crate::counters::bump(crate::counters::VOLUME_LIFECYCLE);
    let mut host = Host(unsafe { &*host });
    logical_volume_table(host.count())
        .acquire_device(&mut host, unsafe { CStr::from_ptr(prefix) }.to_bytes())
}
pub unsafe extern "C" fn duplicate_cloexec(fd: i32) -> i32 {
    unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) }
}

#[repr(C)]
pub struct CacheCallbacks {
    context: *mut c_void,
    open_image: unsafe extern "C" fn(*mut c_void) -> i32,
    regular: unsafe extern "C" fn(*mut c_void, i32) -> bool,
    magic: unsafe extern "C" fn(*mut c_void, i32, *mut u8) -> usize,
    content: unsafe extern "C" fn(*mut c_void) -> i32,
    mkdir: unsafe extern "C" fn(*mut c_void) -> i32,
    open_directory: unsafe extern "C" fn(*mut c_void) -> i32,
    set_content: unsafe extern "C" fn(*mut c_void, i32),
    diagnostic: unsafe extern "C" fn(*mut c_void, u32, bool),
}
struct Cache<'a>(&'a CacheCallbacks);
use lifecycle::{CacheEvent, CacheHost};
fn descriptor(fd: i32) -> Option<Descriptor> {
    (fd >= 0).then_some(Descriptor(fd))
}
impl CacheHost for Cache<'_> {
    fn open_image(&mut self) -> Option<Descriptor> {
        descriptor(unsafe { (self.0.open_image)(self.0.context) })
    }
    fn regular(&mut self, fd: Descriptor) -> bool {
        unsafe { (self.0.regular)(self.0.context, fd.0) }
    }
    fn magic(&mut self, fd: Descriptor, out: &mut [u8; 4]) -> usize {
        unsafe { (self.0.magic)(self.0.context, fd.0, out.as_mut_ptr()) }
    }
    fn close(&mut self, fd: Descriptor) {
        unsafe { crate::hostio_ffi::close(fd.0) };
    }
    fn content(&self) -> Option<Descriptor> {
        descriptor(unsafe { (self.0.content)(self.0.context) })
    }
    fn make_directory(&mut self) -> Result<bool, ()> {
        match unsafe { (self.0.mkdir)(self.0.context) } {
            1 => Ok(true),
            0 => Ok(false),
            _ => Err(()),
        }
    }
    fn open_directory(&mut self) -> Option<Descriptor> {
        descriptor(unsafe { (self.0.open_directory)(self.0.context) })
    }
    fn set_content(&mut self, fd: Descriptor) {
        unsafe { (self.0.set_content)(self.0.context, fd.0) };
    }
    fn diagnostic(&mut self, event: CacheEvent) {
        let (code, created) = match event {
            CacheEvent::Unformatted => (0, false),
            CacheEvent::CreateFailed => (1, false),
            CacheEvent::OpenFailed => (2, false),
            CacheEvent::Served { created } => (3, created),
        };
        unsafe { (self.0.diagnostic)(self.0.context, code, created) };
    }
}
pub unsafe extern "C" fn prepare_cache(host: *const CacheCallbacks) -> u32 {
    if host.is_null() {
        return 0xc000000d;
    }
    crate::counters::bump(crate::counters::CACHE_LIFECYCLE);
    lifecycle::prepare_cache(&mut Cache(unsafe { &*host }))
}

#[repr(C)]
pub struct DeviceResult {
    root: i32,
    error: i32,
    size: i64,
    failure: u32,
}
struct Device<'a> {
    root: &'a CStr,
    name: &'a CStr,
}
fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}
fn device_metadata(stat: libc::stat) -> lifecycle::DeviceMetadata {
    lifecycle::DeviceMetadata {
        regular: stat.st_mode & libc::S_IFMT == libc::S_IFREG,
        size: stat.st_size,
    }
}
impl lifecycle::DeviceHost for Device<'_> {
    fn open_root(&mut self) -> Result<Descriptor, i32> {
        let fd = unsafe {
            crate::hostio_ffi::open_at(
                libc::AT_FDCWD,
                self.root.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
                0,
            )
        };
        descriptor(fd).ok_or_else(errno)
    }
    fn existing(&mut self, root: Descriptor) -> Option<lifecycle::DeviceMetadata> {
        let mut stat = std::mem::MaybeUninit::uninit();
        let result = unsafe {
            crate::hostio_ffi::metadata_at(
                root.0,
                self.name.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        (result == 0).then(|| device_metadata(unsafe { stat.assume_init() }))
    }
    fn open_file(&mut self, root: Descriptor) -> Result<Descriptor, i32> {
        let fd = unsafe {
            crate::hostio_ffi::open_at(
                root.0,
                self.name.as_ptr(),
                libc::O_RDWR | libc::O_CREAT | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        descriptor(fd).ok_or_else(errno)
    }
    fn metadata(&mut self, file: Descriptor) -> Option<lifecycle::DeviceMetadata> {
        let mut stat = std::mem::MaybeUninit::uninit();
        let result = unsafe { crate::hostio_ffi::metadata(file.0, stat.as_mut_ptr()) };
        (result == 0).then(|| device_metadata(unsafe { stat.assume_init() }))
    }
    fn close(&mut self, fd: Descriptor) {
        unsafe { crate::hostio_ffi::close(fd.0) };
    }
}
pub unsafe extern "C" fn prepare_device(
    root: *const c_char,
    name: *const c_char,
    capacity: u64,
    out: *mut DeviceResult,
) {
    if root.is_null() || name.is_null() || out.is_null() {
        return;
    }
    crate::counters::bump(crate::counters::VOLUME_LIFECYCLE);
    let mut host = Device {
        root: unsafe { CStr::from_ptr(root) },
        name: unsafe { CStr::from_ptr(name) },
    };
    let result = match lifecycle::prepare_device(&mut host, capacity) {
        Ok((root, size)) => DeviceResult {
            root: root.0,
            size,
            error: 0,
            failure: 0,
        },
        Err((failure, error)) => DeviceResult {
            root: -1,
            size: 0,
            error,
            failure: failure as u32,
        },
    };
    unsafe { *out = result };
}
pub unsafe extern "C" fn filetime(seconds: i64, nanoseconds: i64) -> u64 {
    lifecycle::filetime(seconds, nanoseconds)
}
#[repr(C)]
pub struct GuestMetadata {
    size: u64,
    access: u64,
    write: u64,
    creation: u64,
    directory: bool,
}
pub unsafe extern "C" fn guest_metadata(stat: *const libc::stat, out: *mut GuestMetadata) {
    if stat.is_null() || out.is_null() {
        return;
    }
    let stat = unsafe { &*stat };
    let converted = lifecycle::guest_metadata(lifecycle::HostMetadata {
        directory: stat.st_mode & libc::S_IFMT == libc::S_IFDIR,
        size: stat.st_size,
        access_seconds: stat.st_atime,
        access_nanoseconds: stat.st_atime_nsec,
        write_seconds: stat.st_mtime,
        write_nanoseconds: stat.st_mtime_nsec,
    });
    unsafe {
        *out = GuestMetadata {
            size: converted.size,
            access: converted.access,
            write: converted.write,
            creation: converted.creation,
            directory: converted.directory,
        }
    };
}
pub unsafe extern "C" fn space_units(
    blocks: u64,
    available: u64,
    fragment: u64,
    total: *mut u64,
    free: *mut u64,
) -> bool {
    if total.is_null() || free.is_null() {
        return false;
    }
    let Some((total_units, available_units)) = lifecycle::space_units(blocks, available, fragment)
    else {
        return false;
    };
    unsafe {
        *total = total_units;
        *free = available_units;
    }
    true
}
pub unsafe extern "C" fn validate_device(
    prefix: *const c_char,
    root: *const c_char,
    image: *const c_char,
    content: *const c_char,
    cache: u32,
    capacity: u64,
) -> u32 {
    if prefix.is_null() || root.is_null() || image.is_null() {
        return 1;
    }
    let mount = lifecycle::DeviceMount {
        prefix: unsafe { CStr::from_ptr(prefix) }.to_bytes(),
        root: unsafe { CStr::from_ptr(root) }.to_bytes(),
        image: unsafe { CStr::from_ptr(image) }.to_bytes(),
        content: if content.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(content) }.to_bytes())
        },
        cache,
        capacity,
    };
    lifecycle::validate_device(mount)
}

/// Borrowed destinations in one existing C volume slot. All name buffers have
/// the validated mount capacities (261, 4096, 256, 256). No pointer is retained.
#[repr(C)]
pub struct PublicationTarget {
    slot: u32,
    slot_count: u32,
    disc_backend_handle: u32,
    prefix: *mut u8,
    host_root: *mut u8,
    device_file: *mut u8,
    content_dir: *mut u8,
    root_plus_one: *mut i32,
    capacity: *mut u64,
    cache: *mut u32,
    backing: *mut u32,
    in_use: *mut bool,
    namespace_dirty: *mut bool,
}
unsafe fn publish_name(target: *mut u8, name: &[u8]) {
    unsafe {
        std::ptr::copy_nonoverlapping(name.as_ptr(), target, name.len());
        target.add(name.len()).write(0);
    }
}
pub(crate) unsafe extern "C" fn publish_device(
    target: *const PublicationTarget,
    prefix: *const c_char,
    root: *const c_char,
    image: *const c_char,
    content: *const c_char,
    cache: u32,
    capacity: u64,
    descriptor: i32,
) {
    let target = unsafe { &*target };
    let publication = lifecycle::device_publication(
        lifecycle::DeviceMount {
            prefix: unsafe { CStr::from_ptr(prefix) }.to_bytes(),
            root: unsafe { CStr::from_ptr(root) }.to_bytes(),
            image: unsafe { CStr::from_ptr(image) }.to_bytes(),
            content: (!content.is_null()).then(|| unsafe { CStr::from_ptr(content) }.to_bytes()),
            cache,
            capacity,
        },
        Descriptor(descriptor),
    );
    let volume = Volume {
        prefix: publication.prefix.to_vec(),
        backing: Backing::HostDirectory,
        root: Some(Descriptor(descriptor)),
        content: None,
        device: publication.image.to_vec(),
        capacity: publication.capacity,
        disc_handle: None,
    };
    publish_rust_volume(target.slot as usize, target.slot_count as usize, volume);
    unsafe {
        publish_name(target.prefix, publication.prefix);
        target.backing.write(2);
        publish_name(target.host_root, publication.root);
        target
            .root_plus_one
            .write(publication.root_descriptor_plus_one);
        publish_name(target.device_file, publication.image);
        target.capacity.write(publication.capacity);
        if let Some((number, name)) = publication.cache {
            target.cache.write(number);
            publish_name(target.content_dir, name);
        }
        // Make the complete record visible last, under the existing file lock.
        target.in_use.write(true);
        target.namespace_dirty.write(true);
    }
}

/// Existing mount buffers and a successfully acquired descriptor are borrowed
/// only for this call; the caller publishes under the file lock.
pub unsafe extern "C" fn validate_host_directory(
    prefix: *const c_char,
    root: *const c_char,
) -> u32 {
    if prefix.is_null() || root.is_null() {
        return 1;
    }
    lifecycle::validate_host_directory(
        unsafe { CStr::from_ptr(prefix) }.to_bytes(),
        unsafe { CStr::from_ptr(root) }.to_bytes(),
    )
    .err()
    .map_or(0, |failure| failure as u32)
}
pub unsafe extern "C" fn validate_disc_prefix(prefix: *const c_char) -> u32 {
    if prefix.is_null() {
        return 1;
    }
    lifecycle::validate_disc_prefix(unsafe { CStr::from_ptr(prefix) }.to_bytes())
        .err()
        .map_or(0, |_| 2)
}
pub unsafe extern "C" fn open_host_directory(root: *const c_char) -> i32 {
    if root.is_null() {
        unsafe { *libc::__errno_location() = libc::EINVAL };
        return -1;
    }
    // No creation and no NOFOLLOW: root symlinks were accepted by the oracle.
    unsafe {
        crate::hostio_ffi::open_at(
            libc::AT_FDCWD,
            root,
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC,
            0,
        )
    }
}
pub unsafe extern "C" fn publish_host_directory(
    target: *const PublicationTarget,
    prefix: *const c_char,
    root: *const c_char,
    fd: i32,
) {
    if target.is_null() || prefix.is_null() || root.is_null() {
        return;
    }
    let target = unsafe { &*target };
    if target.slot >= target.slot_count
        || target.prefix.is_null()
        || target.host_root.is_null()
        || target.root_plus_one.is_null()
        || target.backing.is_null()
        || target.in_use.is_null()
        || target.namespace_dirty.is_null()
    {
        return;
    }
    let publication = lifecycle::directory_publication(
        unsafe { CStr::from_ptr(prefix) }.to_bytes(),
        unsafe { CStr::from_ptr(root) }.to_bytes(),
        Descriptor(fd),
    );
    publish_rust_volume(
        target.slot as usize,
        target.slot_count as usize,
        Volume {
            prefix: publication.prefix.to_vec(),
            backing: Backing::HostDirectory,
            root: Some(Descriptor(fd)),
            content: None,
            device: Vec::new(),
            capacity: 0,
            disc_handle: None,
        },
    );
    unsafe {
        publish_name(target.prefix, publication.prefix);
        publish_name(target.host_root, publication.root);
        target
            .root_plus_one
            .write(publication.root_descriptor_plus_one);
        target.backing.write(2);
        target.in_use.write(true);
        target.namespace_dirty.write(true);
    }
}

/// Publish the logical view of an already-open disc volume. The selected Rust
/// reader handle belongs to the Rust volume record; C retains its compatibility
/// reader for legacy consumers.
pub unsafe extern "C" fn publish_disc(target: *const PublicationTarget, prefix: *const c_char) {
    if target.is_null() || prefix.is_null() {
        return;
    }
    let target = unsafe { &*target };
    let prefix = unsafe { CStr::from_ptr(prefix) }.to_bytes();
    publish_rust_volume(
        target.slot as usize,
        target.slot_count as usize,
        Volume {
            prefix: prefix.to_vec(),
            backing: Backing::Disc,
            root: None,
            content: None,
            device: Vec::new(),
            capacity: 0,
            disc_handle: (target.disc_backend_handle != 0).then_some(target.disc_backend_handle),
        },
    );
    unsafe {
        publish_name(target.prefix, prefix);
        target.backing.write(C_BACKING_DISC);
        target.in_use.write(true);
        target.namespace_dirty.write(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;
    static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn lifecycle_callbacks_do_not_rehydrate_c_volume_snapshots() {
        assert_eq!(
            std::mem::size_of::<Callbacks>(),
            5 * std::mem::size_of::<usize>()
        );
    }

    unsafe extern "C" fn no_close_disc(_: *mut c_void, _: usize) {}
    unsafe extern "C" fn no_clear(_: *mut c_void, _: i32) {}
    unsafe extern "C" fn no_acquire(_: *mut c_void, _: usize, _: i32) -> bool {
        false
    }
    unsafe extern "C" fn no_open_image(_: *mut c_void) -> i32 {
        -1
    }
    unsafe extern "C" fn no_regular(_: *mut c_void, _: i32) -> bool {
        false
    }
    unsafe extern "C" fn no_magic(_: *mut c_void, _: i32, _: *mut u8) -> usize {
        0
    }
    unsafe extern "C" fn no_content(_: *mut c_void) -> i32 {
        -1
    }
    unsafe extern "C" fn no_mkdir(_: *mut c_void) -> i32 {
        -1
    }
    unsafe extern "C" fn no_open_directory(_: *mut c_void) -> i32 {
        -1
    }
    unsafe extern "C" fn no_set_content(_: *mut c_void, _: i32) {}
    unsafe extern "C" fn no_diagnostic(_: *mut c_void, _: u32, _: bool) {}

    #[derive(Default)]
    struct UnmountEffects(Vec<String>);
    unsafe extern "C" fn record_close_disc(context: *mut c_void, slot: usize) {
        unsafe { &mut *context.cast::<UnmountEffects>() }
            .0
            .push(format!("disc:{slot}"));
    }
    unsafe extern "C" fn record_clear(context: *mut c_void, slot: i32) {
        unsafe { &mut *context.cast::<UnmountEffects>() }
            .0
            .push(format!("clear:{slot}"));
    }
    unsafe extern "C" fn record_acquire(_: *mut c_void, _: usize, _: i32) -> bool {
        false
    }

    #[test]
    fn unmount_all_uses_legacy_disc_callback_and_clears_after_each_live_slot() {
        let _guard = TEST_LOCK.lock().unwrap();
        *VOLUME_TABLE.lock().unwrap() = None;
        publish_rust_volume(
            0,
            3,
            Volume {
                prefix: b"D:".to_vec(),
                backing: Backing::Disc,
                root: None,
                content: None,
                device: Vec::new(),
                capacity: 0,
                // Legacy C reader: teardown must delegate the resource close
                // to the host callback for this logical slot.
                disc_handle: None,
            },
        );
        publish_rust_volume(
            2,
            3,
            Volume {
                prefix: b"E:".to_vec(),
                backing: Backing::Fatx,
                root: None,
                content: None,
                device: Vec::new(),
                capacity: 0,
                disc_handle: None,
            },
        );
        let mut effects = UnmountEffects::default();
        let callbacks = Callbacks {
            context: (&mut effects as *mut UnmountEffects).cast(),
            close_disc: record_close_disc,
            clear: record_clear,
            acquire_device: record_acquire,
            count: 3,
        };

        unsafe { unmount_all(&callbacks) };

        assert_eq!(effects.0, ["disc:0", "clear:-1"]);
        assert_eq!(
            VOLUME_TABLE.lock().unwrap().as_ref().unwrap().live_count(),
            0
        );
    }

    #[test]
    fn unmount_all_closes_rust_owned_host_directory_descriptors() {
        let _guard = TEST_LOCK.lock().unwrap();
        *VOLUME_TABLE.lock().unwrap() = None;
        let null = CString::new("/dev/null").unwrap();
        let root = unsafe { libc::open(null.as_ptr(), libc::O_RDONLY) };
        let content = unsafe { libc::open(null.as_ptr(), libc::O_RDONLY) };
        assert!(root >= 0 && content >= 0 && root != content);
        publish_rust_volume(
            1,
            2,
            Volume {
                prefix: b"C:".to_vec(),
                backing: Backing::HostDirectory,
                root: Some(Descriptor(root)),
                content: Some(Descriptor(content)),
                device: Vec::new(),
                capacity: 0,
                disc_handle: None,
            },
        );
        let mut effects = UnmountEffects::default();
        let callbacks = Callbacks {
            context: (&mut effects as *mut UnmountEffects).cast(),
            close_disc: record_close_disc,
            clear: record_clear,
            acquire_device: record_acquire,
            count: 2,
        };

        unsafe { unmount_all(&callbacks) };

        assert_eq!(effects.0, ["clear:-1"]);
        assert_eq!(
            unsafe { libc::fcntl(root, libc::F_GETFD) },
            -1,
            "unmount_all must close the Rust-owned root descriptor"
        );
        assert_eq!(
            unsafe { libc::fcntl(content, libc::F_GETFD) },
            -1,
            "unmount_all must close the Rust-owned content descriptor"
        );
        assert_eq!(
            VOLUME_TABLE.lock().unwrap().as_ref().unwrap().live_count(),
            0
        );
    }

    #[test]
    fn unmount_all_closes_live_rust_reader_without_legacy_fallback() {
        let _guard = TEST_LOCK.lock().unwrap();
        *VOLUME_TABLE.lock().unwrap() = None;
        let image_path = std::env::temp_dir().join(format!(
            "tsfp-fileio-capi-unmount-{}.iso",
            std::process::id()
        ));
        let mut image = vec![0u8; 34 * 2048];
        let descriptor = &mut image[32 * 2048..33 * 2048];
        descriptor[..20].copy_from_slice(b"MICROSOFT*XBOX*MEDIA");
        descriptor[0x14..0x18].copy_from_slice(&33u32.to_le_bytes());
        descriptor[0x18..0x1c].copy_from_slice(&0u32.to_le_bytes());
        descriptor[0x7ec..0x7ec + 20].copy_from_slice(b"MICROSOFT*XBOX*MEDIA");
        std::fs::write(&image_path, image).unwrap();
        let image_path_c = CString::new(image_path.to_str().unwrap()).unwrap();
        let mut handle = 0;
        assert_eq!(
            unsafe { crate::open(image_path_c.as_ptr(), &mut handle, std::ptr::null_mut()) },
            crate::OK
        );
        publish_rust_volume(
            0,
            1,
            Volume {
                prefix: b"D:".to_vec(),
                backing: Backing::Disc,
                root: None,
                content: None,
                device: Vec::new(),
                capacity: 0,
                disc_handle: Some(handle),
            },
        );
        let mut effects = UnmountEffects::default();
        let callbacks = Callbacks {
            context: (&mut effects as *mut UnmountEffects).cast(),
            close_disc: record_close_disc,
            clear: record_clear,
            acquire_device: record_acquire,
            count: 1,
        };

        unsafe { unmount_all(&callbacks) };

        assert_eq!(effects.0, ["clear:-1"]);
        assert_eq!(
            VOLUME_TABLE.lock().unwrap().as_ref().unwrap().live_count(),
            0
        );
        let path = CString::new("/").unwrap();
        let mut entry = crate::CEntry {
            name: [0; 256],
            name_length: 0,
            start_sector: 0,
            size: 0,
            attributes: 0,
            is_directory: false,
        };
        assert_eq!(
            unsafe { crate::lookup(handle, path.as_ptr(), &mut entry) },
            crate::ERR_IMAGE
        );
        std::fs::remove_file(image_path).unwrap();
    }

    #[test]
    fn lifecycle_descriptor_close_runs_in_rust_hostio() {
        let path = CString::new("/dev/null").unwrap();
        let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
        assert!(fd >= 0);
        let callbacks = Callbacks {
            context: std::ptr::null_mut(),
            close_disc: no_close_disc,
            clear: no_clear,
            acquire_device: no_acquire,
            count: 0,
        };

        Host(&callbacks).close(Descriptor(fd));

        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
    }

    #[test]
    fn cache_descriptor_close_runs_in_rust_hostio() {
        let path = CString::new("/dev/null").unwrap();
        let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC) };
        assert!(fd >= 0);
        let callbacks = CacheCallbacks {
            context: std::ptr::null_mut(),
            open_image: no_open_image,
            regular: no_regular,
            magic: no_magic,
            content: no_content,
            mkdir: no_mkdir,
            open_directory: no_open_directory,
            set_content: no_set_content,
            diagnostic: no_diagnostic,
        };

        Cache(&callbacks).close(Descriptor(fd));

        assert_eq!(unsafe { libc::fcntl(fd, libc::F_GETFD) }, -1);
    }

    #[test]
    fn clear_transition_preserves_other_slots_and_full_clear_resets_capacity() {
        let _guard = TEST_LOCK.lock().unwrap();
        *VOLUME_TABLE.lock().unwrap() = None;
        let volume = |prefix: &[u8]| Volume {
            prefix: prefix.to_vec(),
            backing: Backing::Fatx,
            root: None,
            content: None,
            device: Vec::new(),
            capacity: 0,
            disc_handle: None,
        };
        publish_rust_volume(0, 3, volume(b"E:"));
        publish_rust_volume(2, 3, volume(b"F:"));

        clear_rust_volume(Some(0), 3);
        {
            let tables = VOLUME_TABLE.lock().unwrap();
            let table = tables.as_ref().unwrap();
            assert_eq!(table.live_count(), 1);
            assert!(table.get(0).is_none());
            assert_eq!(table.get(2).unwrap().prefix, b"F:");
        }

        clear_rust_volume(None, 3);
        let tables = VOLUME_TABLE.lock().unwrap();
        let table = tables.as_ref().unwrap();
        assert_eq!(table.capacity(), 3);
        assert_eq!(table.live_count(), 0);
    }

    #[test]
    fn disc_publication_owns_logical_record_and_mirrors_c_slot() {
        let _guard = TEST_LOCK.lock().unwrap();
        *VOLUME_TABLE.lock().unwrap() = None;
        let mut prefix = [0u8; 261];
        let mut backing = 0;
        let mut in_use = false;
        let mut namespace_dirty = false;
        let target = PublicationTarget {
            slot: 1,
            slot_count: 3,
            disc_backend_handle: 42,
            prefix: prefix.as_mut_ptr(),
            host_root: std::ptr::null_mut(),
            device_file: std::ptr::null_mut(),
            content_dir: std::ptr::null_mut(),
            root_plus_one: std::ptr::null_mut(),
            capacity: std::ptr::null_mut(),
            cache: std::ptr::null_mut(),
            backing: &mut backing,
            in_use: &mut in_use,
            namespace_dirty: &mut namespace_dirty,
        };
        let guest_prefix = CString::new("\\Device\\CdRom0").unwrap();

        unsafe { publish_disc(&target, guest_prefix.as_ptr()) };

        assert_eq!(
            logical_volume_table(3).get(1).unwrap().disc_handle,
            Some(42)
        );

        assert_eq!(
            &prefix[..guest_prefix.as_bytes().len()],
            guest_prefix.as_bytes()
        );
        assert_eq!(backing, C_BACKING_DISC);
        assert!(in_use && namespace_dirty);
        let tables = VOLUME_TABLE.lock().unwrap();
        let volume = tables.as_ref().unwrap().get(1).unwrap();
        assert_eq!(volume.prefix, guest_prefix.as_bytes());
        assert_eq!(volume.backing, Backing::Disc);
        assert_eq!(volume.root, None);
    }

    #[test]
    fn host_directory_publication_rejects_invalid_slot_before_rust_publication() {
        let _guard = TEST_LOCK.lock().unwrap();
        *VOLUME_TABLE.lock().unwrap() = None;
        let mut prefix = [0u8; 261];
        let mut host_root = [0u8; 4096];
        let mut root_plus_one = 0;
        let mut backing = 0;
        let mut in_use = false;
        let mut namespace_dirty = false;
        let target = PublicationTarget {
            slot: 3,
            slot_count: 3,
            disc_backend_handle: 0,
            prefix: prefix.as_mut_ptr(),
            host_root: host_root.as_mut_ptr(),
            device_file: std::ptr::null_mut(),
            content_dir: std::ptr::null_mut(),
            root_plus_one: &mut root_plus_one,
            capacity: std::ptr::null_mut(),
            cache: std::ptr::null_mut(),
            backing: &mut backing,
            in_use: &mut in_use,
            namespace_dirty: &mut namespace_dirty,
        };
        let guest_prefix = CString::new("U:").unwrap();
        let host_path = CString::new("/tmp/fixture").unwrap();

        unsafe { publish_host_directory(&target, guest_prefix.as_ptr(), host_path.as_ptr(), 17) };

        assert_eq!(logical_volume_table(3).live_count(), 0);
        assert_eq!(root_plus_one, 0);
        assert_eq!(backing, 0);
        assert!(!in_use);
        assert!(!namespace_dirty);
        assert_eq!(prefix[0], 0);
        assert_eq!(host_root[0], 0);
    }

    #[test]
    fn fatx_mount_callback_publishes_logical_volume_record() {
        let _guard = TEST_LOCK.lock().unwrap();
        *VOLUME_TABLE.lock().unwrap() = None;
        publish_fatx_volume(2, 16, b"E:");
        let tables = VOLUME_TABLE.lock().unwrap();
        let table = tables.as_ref().unwrap();
        assert_eq!(table.capacity(), 16);
        assert_eq!(table.live_count(), 1);
        assert_eq!(table.get(2).unwrap().prefix, b"E:");
        assert_eq!(table.get(2).unwrap().backing, Backing::Fatx);
    }

    #[test]
    fn cache_content_descriptor_is_owned_by_the_live_rust_volume() {
        let _guard = TEST_LOCK.lock().unwrap();
        *VOLUME_TABLE.lock().unwrap() = None;
        publish_rust_volume(
            1,
            3,
            Volume {
                prefix: b"Cache:".to_vec(),
                backing: Backing::HostDirectory,
                root: Some(Descriptor(4)),
                content: None,
                device: b"CachePartition1.img".to_vec(),
                capacity: 4096,
                disc_handle: None,
            },
        );

        assert!(unsafe { publish_content_descriptor(1, 9) });
        assert_eq!(
            logical_volume_table(3).get(1).unwrap().content,
            Some(Descriptor(9))
        );
        assert!(!unsafe { publish_content_descriptor(0, 10) });
    }

    #[test]
    fn lifecycle_slot_and_count_decisions_read_rust_publications() {
        let _guard = TEST_LOCK.lock().unwrap();
        *VOLUME_TABLE.lock().unwrap() = None;
        publish_rust_volume(
            1,
            3,
            Volume {
                prefix: b"E:".to_vec(),
                backing: Backing::Disc,
                root: None,
                content: None,
                device: Vec::new(),
                capacity: 0,
                disc_handle: None,
            },
        );

        let table = logical_volume_table(3);
        assert_eq!(table.live_count(), 1);
        assert_eq!(table.mount_slot(None), Some(0));
        assert_eq!(table.mount_slot(Some(b"E:")), None);
        assert_eq!(table.mount_slot(Some(b"missing:")), Some(0));

        publish_rust_volume(
            0,
            3,
            Volume {
                prefix: b"E:".to_vec(),
                backing: Backing::HostDirectory,
                root: Some(Descriptor(7)),
                content: None,
                device: b"cache.img".to_vec(),
                capacity: 4096,
                disc_handle: None,
            },
        );
        let table = logical_volume_table(3);
        assert_eq!(table.live_count(), 2);
        assert_eq!(table.mount_slot(Some(b"E:")), None);
    }
}
