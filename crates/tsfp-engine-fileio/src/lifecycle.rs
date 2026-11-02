//! Typed orchestration over existing volume slots; no second VFS registry.
use crate::acquisition::Backing;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Descriptor(pub i32);
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Volume {
    pub prefix: Vec<u8>,
    pub backing: Backing,
    pub root: Option<Descriptor>,
    pub content: Option<Descriptor>,
    pub device: Vec<u8>,
    pub capacity: u64,
    /// Rust XDVDFS backend handle, when this disc was opened by the selected reader.
    pub disc_handle: Option<u32>,
}

/// Rust-owned logical view of the mounted volume table. Resource effects still
/// use `VolumeHost`; selected slot and count decisions can read this snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VolumeTable {
    slots: Vec<Option<Volume>>,
}

impl VolumeTable {
    pub fn new(capacity: usize) -> Self {
        Self {
            slots: vec![None; capacity],
        }
    }

    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    pub fn set(&mut self, slot: usize, volume: Option<Volume>) -> bool {
        let Some(record) = self.slots.get_mut(slot) else {
            return false;
        };
        *record = volume;
        true
    }

    pub fn get(&self, slot: usize) -> Option<&Volume> {
        self.slots.get(slot).and_then(Option::as_ref)
    }

    pub fn set_content(&mut self, slot: usize, content: Descriptor) -> bool {
        let Some(Some(volume)) = self.slots.get_mut(slot) else {
            return false;
        };
        volume.content = Some(content);
        true
    }

    pub fn mount_slot(&self, unique: Option<&[u8]>) -> Option<usize> {
        let mut free = None;
        for (slot, volume) in self.slots.iter().enumerate() {
            match volume {
                Some(volume) if unique == Some(volume.prefix.as_slice()) => return None,
                None if free.is_none() => free = Some(slot),
                _ => (),
            }
        }
        free
    }

    pub fn live_count(&self) -> usize {
        self.slots.iter().filter(|slot| slot.is_some()).count()
    }

    pub fn unmount_all(&mut self, host: &mut impl VolumeHost) {
        for slot in 0..self.slots.len() {
            let Some(volume) = self.slots[slot].as_ref() else {
                continue;
            };
            if volume.backing == Backing::Disc {
                host.close_disc(slot, volume.disc_handle);
            }
            if let Some(root) = volume.root {
                host.close(root);
            }
            if let Some(content) = volume.content {
                host.close(content);
            }
        }
        host.clear(None);
        self.slots.fill(None);
    }

    pub fn unmount_fatx(&mut self, host: &mut impl VolumeHost, prefix: &[u8]) -> bool {
        let Some(slot) = self.slots.iter().position(|volume| {
            volume
                .as_ref()
                .is_some_and(|volume| volume.backing == Backing::Fatx && volume.prefix == prefix)
        }) else {
            return false;
        };
        if let Some(content) = self.slots[slot].as_ref().and_then(|volume| volume.content) {
            host.close(content);
        }
        host.clear(Some(slot));
        self.slots[slot] = None;
        true
    }

    pub fn acquire_device(&self, host: &mut impl VolumeHost, prefix: &[u8]) -> bool {
        for (slot, volume) in self.slots.iter().enumerate() {
            let Some(volume) = volume else {
                continue;
            };
            if volume.backing != Backing::HostDirectory
                || volume.device.is_empty()
                || volume.capacity == 0
                || !volume.prefix.eq_ignore_ascii_case(prefix)
            {
                continue;
            }
            let Some(root) = volume.root else {
                continue;
            };
            return host.acquire_device(slot, root);
        }
        false
    }
}

pub trait VolumeHost {
    fn close_disc(&mut self, slot: usize, backend_handle: Option<u32>);
    fn close(&mut self, fd: Descriptor);
    fn clear(&mut self, slot: Option<usize>);
    fn acquire_device(&mut self, slot: usize, root: Descriptor) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Effects(Vec<String>);

    impl VolumeHost for Effects {
        fn close_disc(&mut self, slot: usize, backend_handle: Option<u32>) {
            self.0.push(format!("disc:{slot}:{backend_handle:?}"));
        }
        fn close(&mut self, fd: Descriptor) {
            self.0.push(format!("close:{}", fd.0));
        }
        fn clear(&mut self, slot: Option<usize>) {
            self.0.push(format!("clear:{slot:?}"));
        }
        fn acquire_device(&mut self, slot: usize, root: Descriptor) -> bool {
            self.0.push(format!("device:{slot}:{}", root.0));
            true
        }
    }

    #[test]
    fn owned_volume_table_preserves_slot_order_and_exact_prefix_policy() {
        let mut table = VolumeTable::new(4);
        let volume = Volume {
            prefix: b"D:".to_vec(),
            backing: Backing::Disc,
            root: Some(Descriptor(7)),
            content: None,
            device: Vec::new(),
            capacity: 0,
            disc_handle: None,
        };
        assert!(table.set(2, Some(volume.clone())));
        assert_eq!(table.live_count(), 1);
        assert_eq!(table.mount_slot(None), Some(0));
        assert_eq!(table.mount_slot(Some(b"D:")), None);
        assert_eq!(table.mount_slot(Some(b"d:")), Some(0));
        assert_eq!(table.get(2), Some(&volume));
        assert!(!table.set(4, None));
    }

    #[test]
    fn table_unmount_uses_rust_records_for_resource_close_selection() {
        let mut table = VolumeTable::new(3);
        table.set(
            0,
            Some(Volume {
                prefix: b"D:".to_vec(),
                backing: Backing::Disc,
                root: Some(Descriptor(5)),
                content: Some(Descriptor(6)),
                device: Vec::new(),
                capacity: 0,
                disc_handle: Some(42),
            }),
        );
        table.set(
            2,
            Some(Volume {
                prefix: b"C:".to_vec(),
                backing: Backing::HostDirectory,
                root: Some(Descriptor(9)),
                content: None,
                device: Vec::new(),
                capacity: 0,
                disc_handle: None,
            }),
        );
        let mut effects = Effects::default();
        table.unmount_all(&mut effects);
        assert_eq!(
            effects.0,
            [
                "disc:0:Some(42)",
                "close:5",
                "close:6",
                "close:9",
                "clear:None"
            ]
        );
        assert_eq!(table.live_count(), 0);
    }

    #[test]
    fn table_unmount_fatx_clears_first_exact_prefix_match() {
        let volume = Volume {
            prefix: b"U:".to_vec(),
            backing: Backing::Fatx,
            root: None,
            content: Some(Descriptor(8)),
            device: Vec::new(),
            capacity: 0,
            disc_handle: None,
        };
        let mut table = VolumeTable::new(3);
        table.set(1, Some(volume));
        let mut effects = Effects::default();
        assert!(!table.unmount_fatx(&mut effects, b"u:"));
        assert!(table.unmount_fatx(&mut effects, b"U:"));
        assert_eq!(effects.0, ["close:8", "clear:Some(1)"]);
        assert_eq!(table.live_count(), 0);
    }

    #[test]
    fn table_publishes_lazy_content_only_to_live_slot() {
        let mut table = VolumeTable::new(2);
        table.set(
            1,
            Some(Volume {
                prefix: b"U:".to_vec(),
                backing: Backing::Fatx,
                root: None,
                content: None,
                device: Vec::new(),
                capacity: 0,
                disc_handle: None,
            }),
        );
        assert!(table.set_content(1, Descriptor(9)));
        assert_eq!(table.get(1).unwrap().content, Some(Descriptor(9)));
        assert!(!table.set_content(0, Descriptor(10)));
        assert!(!table.set_content(2, Descriptor(11)));
    }
}

#[derive(Clone, Copy, Debug)]
pub enum CacheEvent {
    Unformatted,
    CreateFailed,
    OpenFailed,
    Served { created: bool },
}
pub trait CacheHost {
    fn open_image(&mut self) -> Option<Descriptor>;
    fn regular(&mut self, fd: Descriptor) -> bool;
    fn magic(&mut self, fd: Descriptor, out: &mut [u8; 4]) -> usize;
    fn close(&mut self, fd: Descriptor);
    fn content(&self) -> Option<Descriptor>;
    fn make_directory(&mut self) -> Result<bool, ()>;
    fn open_directory(&mut self) -> Option<Descriptor>;
    fn set_content(&mut self, fd: Descriptor);
    fn diagnostic(&mut self, event: CacheEvent);
}
pub fn cache_formatted(host: &mut impl CacheHost) -> bool {
    let Some(fd) = host.open_image() else {
        return false;
    };
    let mut magic = [0u8; 4];
    let formatted = if host.regular(fd) {
        let got = host.magic(fd, &mut magic);
        got > 0 && crate::views::image_formatted(&magic[..got.min(4)])
    } else {
        false
    };
    host.close(fd);
    formatted
}
pub fn prepare_cache(host: &mut impl CacheHost) -> u32 {
    if !cache_formatted(host) {
        host.diagnostic(CacheEvent::Unformatted);
        return 0xc000014f;
    }
    if host.content().is_some() {
        return 0;
    }
    let created = match host.make_directory() {
        Ok(created) => created,
        Err(()) => {
            host.diagnostic(CacheEvent::CreateFailed);
            return 0xc0000001;
        }
    };
    let Some(fd) = host.open_directory() else {
        host.diagnostic(CacheEvent::OpenFailed);
        return 0xc0000022;
    };
    host.set_content(fd);
    host.diagnostic(CacheEvent::Served { created });
    0
}

/// Resource acquisition for a synthetic raw device. Publication is separate:
/// a successful result transfers the root descriptor to the mounted record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceMetadata {
    pub regular: bool,
    pub size: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum DeviceFailure {
    Root = 1,
    ExistingType = 2,
    Open = 3,
    OpenedTypeOrSize = 4,
}
pub trait DeviceHost {
    fn open_root(&mut self) -> Result<Descriptor, i32>;
    fn existing(&mut self, root: Descriptor) -> Option<DeviceMetadata>;
    fn open_file(&mut self, root: Descriptor) -> Result<Descriptor, i32>;
    fn metadata(&mut self, file: Descriptor) -> Option<DeviceMetadata>;
    fn close(&mut self, descriptor: Descriptor);
}
pub fn prepare_device(
    host: &mut impl DeviceHost,
    capacity: u64,
) -> Result<(Descriptor, i64), (DeviceFailure, i32)> {
    let root = host
        .open_root()
        .map_err(|errno| (DeviceFailure::Root, errno))?;
    // Any failed pre-stat proceeds to open, exactly as in the C oracle. The
    // NOFOLLOW open and descriptor stat are still required after this probe.
    if host
        .existing(root)
        .is_some_and(|metadata| !metadata.regular)
    {
        host.close(root);
        return Err((DeviceFailure::ExistingType, 0));
    }
    let file = match host.open_file(root) {
        Ok(file) => file,
        Err(errno) => {
            host.close(root);
            return Err((DeviceFailure::Open, errno));
        }
    };
    let metadata = host.metadata(file);
    host.close(file);
    match metadata {
        Some(metadata) if metadata.regular && metadata.size as u64 <= capacity => {
            Ok((root, metadata.size))
        }
        _ => {
            host.close(root);
            Err((DeviceFailure::OpenedTypeOrSize, 0))
        }
    }
}

/// Linux timestamps use the existing inferred epoch and wrapping arithmetic.
/// Negative nanoseconds divide toward zero, matching the C integer division.
pub fn filetime(seconds: i64, nanoseconds: i64) -> u64 {
    (seconds as u64)
        .wrapping_add(11_644_473_600)
        .wrapping_mul(10_000_000)
        .wrapping_add((nanoseconds / 100) as u64)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostMetadata {
    pub directory: bool,
    pub size: i64,
    pub access_seconds: i64,
    pub access_nanoseconds: i64,
    pub write_seconds: i64,
    pub write_nanoseconds: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GuestMetadata {
    pub directory: bool,
    pub size: u64,
    pub access: u64,
    pub write: u64,
    pub creation: u64,
}
pub fn guest_metadata(host: HostMetadata) -> GuestMetadata {
    let write = filetime(host.write_seconds, host.write_nanoseconds);
    GuestMetadata {
        directory: host.directory,
        size: if host.directory { 0 } else { host.size as u64 },
        access: filetime(host.access_seconds, host.access_nanoseconds),
        write,
        creation: write,
    }
}
/// The original host-directory volume advertises 16-KiB units and 512-byte
/// sectors. Invalid geometry refuses without publishing any output.
pub fn space_units(blocks: u64, available: u64, fragment: u64) -> Option<(u64, u64)> {
    if fragment == 0 {
        return None;
    }
    Some((
        blocks.checked_mul(fragment)? / 16_384,
        available.checked_mul(fragment)? / 16_384,
    ))
}

/// Validated byte names, deliberately without Unicode or path normalization.
/// Numeric failure codes select the existing C diagnostic and its ordering.
#[derive(Clone, Copy)]
pub struct DeviceMount<'a> {
    pub prefix: &'a [u8],
    pub root: &'a [u8],
    pub image: &'a [u8],
    pub content: Option<&'a [u8]>,
    pub cache: u32,
    pub capacity: u64,
}
fn single_component(name: &[u8]) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name != b"."
        && name != b".."
        && !name.iter().any(|byte| matches!(byte, b'/' | b'\\'))
}
pub fn validate_device(mount: DeviceMount<'_>) -> u32 {
    if mount.prefix.is_empty()
        || mount.prefix.len() >= 261
        || mount.root.is_empty()
        || mount.root.len() >= 4096
    {
        return 1;
    }
    if matches!(mount.prefix.last(), Some(b'/' | b'\\')) {
        return 2;
    }
    if !single_component(mount.image) {
        return 3;
    }
    if (mount.content.is_some()
        && (!(3..=10).contains(&mount.cache)
            || !mount
                .content
                .is_some_and(|content| single_component(content) && content != mount.image)))
        || (mount.content.is_none() && mount.cache != 0)
    {
        return 4;
    }
    if mount.capacity == 0 || mount.capacity > 16 * 1024 * 1024 {
        return 5;
    }
    0
}

/// Publication payload for an acquired device. The existing slot owns the
/// descriptor after publication; cache-only fields are untouched for raw mounts.
/// Tier 1: seeded C differential on fabricated storage, no game trace evidence.
pub struct DevicePublication<'a> {
    pub prefix: &'a [u8],
    pub root: &'a [u8],
    pub image: &'a [u8],
    pub root_descriptor_plus_one: i32,
    pub capacity: u64,
    pub cache: Option<(u32, &'a [u8])>,
}
/// Names have passed validate_device; root is a successfully acquired OS fd.
pub fn device_publication(mount: DeviceMount<'_>, root: Descriptor) -> DevicePublication<'_> {
    DevicePublication {
        prefix: mount.prefix,
        root: mount.root,
        image: mount.image,
        root_descriptor_plus_one: root.0 + 1,
        capacity: mount.capacity,
        cache: (mount.cache != 0).then(|| (mount.cache, mount.content.unwrap())),
    }
}

/// Host directory mounts follow the C host contract, including trailing slash
/// admission for the host root and rejection for the guest prefix. Symlinked
/// roots are allowed: opening O_DIRECTORY pins the resolved directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum DirectoryMountFailure {
    Names = 1,
    PrefixSeparator = 2,
}
/// A mount prefix must leave a separator for path resolution to match below it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscPrefixFailure {
    TrailingSeparator,
}
pub fn validate_disc_prefix(prefix: &[u8]) -> Result<(), DiscPrefixFailure> {
    if matches!(prefix.last(), Some(b'/' | b'\\')) {
        return Err(DiscPrefixFailure::TrailingSeparator);
    }
    Ok(())
}
pub fn validate_host_directory(prefix: &[u8], root: &[u8]) -> Result<(), DirectoryMountFailure> {
    if prefix.is_empty() || prefix.len() >= 261 || root.is_empty() || root.len() >= 4096 {
        return Err(DirectoryMountFailure::Names);
    }
    if matches!(prefix.last(), Some(b'/' | b'\\')) {
        return Err(DirectoryMountFailure::PrefixSeparator);
    }
    Ok(())
}

#[cfg(test)]
mod mount_prefix_tests {
    use super::*;

    #[test]
    fn cache_partition_number_range_is_distinct_from_plain_devices() {
        let valid = DeviceMount { prefix: b"\\Device\\Cache", root: b"/tmp", image: b"image",
            content: Some(b"contents"), cache: 3, capacity: 4096 };
        assert_eq!(validate_device(valid), 0);
        assert_ne!(validate_device(DeviceMount { cache: 0, ..valid }), 0);
        assert_ne!(validate_device(DeviceMount { cache: 11, ..valid }), 0);
        assert_eq!(validate_device(DeviceMount { content: None, cache: 0, ..valid }), 0);
        assert_ne!(validate_device(DeviceMount { content: None, cache: 3, ..valid }), 0);
    }

    #[test]
    fn disc_prefix_rejects_trailing_slashes_and_preserves_other_bytes() {
        assert_eq!(validate_disc_prefix(b"D:"), Ok(()));
        assert_eq!(
            validate_disc_prefix(b"D:/"),
            Err(DiscPrefixFailure::TrailingSeparator)
        );
        assert_eq!(
            validate_disc_prefix(b"D:\\"),
            Err(DiscPrefixFailure::TrailingSeparator)
        );
        assert_eq!(validate_disc_prefix(b""), Ok(()));
    }
}
/// Only fields written by host-directory mounts; raw/cache-device fields in a
/// reused slot are deliberately left untouched, matching the existing oracle.
pub struct DirectoryPublication<'a> {
    pub prefix: &'a [u8],
    pub root: &'a [u8],
    pub root_descriptor_plus_one: i32,
}
pub fn directory_publication<'a>(
    prefix: &'a [u8],
    root: &'a [u8],
    descriptor: Descriptor,
) -> DirectoryPublication<'a> {
    DirectoryPublication {
        prefix,
        root,
        root_descriptor_plus_one: descriptor.0 + 1,
    }
}
