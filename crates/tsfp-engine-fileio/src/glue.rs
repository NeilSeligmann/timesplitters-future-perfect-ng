//! Symbolic-link handle objects and the FATX volume glue (T2107), ported from `src/xbox/kernel_file.c`.
//!
//! `NtOpenSymbolicLinkObject` copies the link target at open time, because a handle names the link object and not its
//! name: an `IoDeleteSymbolicLink` after the open must not change what `NtQuerySymbolicLinkObject` returns through it.
//! The kernel is never told about `NtClose`, so a slot is reclaimed when the object table no longer knows its handle
//! (the `live` predicate). FATX volumes carry a mount generation so a handle opened on an image that was unmounted and
//! remounted in the same slot is refused instead of reading the new image.

/// Fixed table of live link handles, indexed like the C `symlink_handles` slots so the slot choice matches.
pub struct LinkHandles {
    slots: Vec<Option<(u32, Vec<u8>)>>,
}

impl LinkHandles {
    pub fn new(capacity: usize) -> Self {
        Self {
            slots: vec![None; capacity],
        }
    }

    pub fn clear(&mut self) {
        self.slots.iter_mut().for_each(|slot| *slot = None);
    }

    /// Record `handle` with its copied `target` in the first free slot, first reclaiming every slot whose handle `live`
    /// says the guest has closed. Returns the slot, or None when every slot holds a live handle.
    pub fn claim(
        &mut self,
        handle: u32,
        target: &[u8],
        live: impl Fn(u32) -> bool,
    ) -> Option<usize> {
        let mut free = None;
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if slot.as_ref().is_some_and(|(held, _)| !live(*held)) {
                *slot = None;
            }
            if slot.is_none() && free.is_none() {
                free = Some(index);
            }
        }
        let index = free?;
        self.slots[index] = Some((handle, target.to_vec()));
        Some(index)
    }

    /// The target copied when `handle` was opened (first matching slot, as the C scan).
    pub fn target(&self, handle: u32) -> Option<&[u8]> {
        self.slots
            .iter()
            .flatten()
            .find(|(held, _)| *held == handle)
            .map(|(_, target)| target.as_slice())
    }

    /// Slots whose handle the object table still knows.
    pub fn live_count(&self, live: impl Fn(u32) -> bool) -> usize {
        self.slots
            .iter()
            .flatten()
            .filter(|(held, _)| live(*held))
            .count()
    }
}

/// Mount generation counter of the FATX volumes. 0 is never issued, so a zeroed record never matches.
#[derive(Default)]
pub struct Generations {
    last: u64,
}

impl Generations {
    pub const fn new() -> Self {
        Self { last: 0 }
    }

    /// The generation for the next FATX mount, None once the counter is exhausted (the mount is refused).
    pub fn issue(&mut self) -> Option<u64> {
        self.last = self.last.checked_add(1)?;
        Some(self.last)
    }

    /// Resume after `last`, so a backend installed mid-run continues the C counter.
    pub const fn starting_at(last: u64) -> Self {
        Self { last }
    }

    pub fn last(&self) -> u64 {
        self.last
    }
}

/// Rust-owned logical FATX mount records, indexed by the host volume slot.
/// C may keep a shadow of these fields while the A/B oracle remains enabled,
/// but selected liveness and exact-prefix lookup read this table directly.
pub struct FatxVolumes {
    generations: Generations,
    slots: Vec<Option<VolumeState>>,
    prefixes: Vec<Option<Vec<u8>>>,
    open_handles: Vec<Option<FatxOpenHandle>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FatxOpenHandle {
    handle: u32,
    volume_slot: usize,
    generation: u64,
}

impl FatxVolumes {
    pub fn new(capacity: usize, open_capacity: usize, last_generation: u64) -> Self {
        Self {
            generations: Generations::starting_at(last_generation),
            slots: vec![None; capacity],
            prefixes: vec![None; capacity],
            open_handles: vec![None; open_capacity],
        }
    }

    pub fn issue_generation(&mut self) -> Option<u64> {
        self.generations.issue()
    }

    /// Publish a newly mounted FATX image and return its unique slot epoch.
    /// Prefix bytes are preserved exactly, matching `strcmp` in the C host model.
    pub fn mount(&mut self, slot: usize, has_image: bool, prefix: &[u8]) -> Option<u64> {
        if !has_image || slot >= self.slots.len() {
            return None;
        }
        let generation = self.generations.issue()?;
        self.slots[slot] = Some(VolumeState {
            in_use: true,
            fatx: true,
            has_image: true,
            generation,
        });
        self.prefixes[slot] = Some(prefix.to_vec());
        Some(generation)
    }

    /// Mirror an already mounted C volume when the Rust backend is enabled.
    pub fn seed(
        &mut self,
        slot: usize,
        in_use: bool,
        fatx: bool,
        has_image: bool,
        generation: u64,
        prefix: &[u8],
    ) -> bool {
        let Some(record) = self.slots.get_mut(slot) else {
            return false;
        };
        *record = (in_use && fatx).then_some(VolumeState {
            in_use,
            fatx,
            has_image,
            generation,
        });
        self.prefixes[slot] = (in_use && fatx).then(|| prefix.to_vec());
        true
    }

    /// Find a mounted FATX record by the exact guest prefix used at mount time.
    /// The C caller supplies the legacy scan as an oracle and keeps ownership of resources.
    pub fn find_prefix(&self, prefix: &[u8]) -> Option<usize> {
        self.prefixes
            .iter()
            .position(|mounted| mounted.as_deref() == Some(prefix))
    }

    pub fn unmount(&mut self, slot: usize) -> bool {
        let Some(record) = self.slots.get_mut(slot) else {
            return false;
        };
        *record = None;
        self.prefixes[slot] = None;
        true
    }

    pub fn clear(&mut self) {
        self.slots.fill(None);
        self.prefixes.fill(None);
        self.open_handles.fill(None);
    }

    /// Record the FATX epoch captured by a successfully committed open slot.
    pub fn track_open_handle(
        &mut self,
        open_slot: usize,
        handle: u32,
        volume_slot: usize,
        generation: u64,
    ) -> bool {
        if open_slot >= self.open_handles.len() || !self.live(volume_slot, generation) {
            return false;
        }
        let candidate = FatxOpenHandle {
            handle,
            volume_slot,
            generation,
        };
        if self.open_handles[open_slot].is_some_and(|existing| existing != candidate) {
            return false;
        }
        self.open_handles[open_slot] = Some(candidate);
        true
    }

    /// Forget the snapshot when the host open slot is retired or reused.
    pub fn release_open_handle(&mut self, open_slot: usize) -> bool {
        let Some(record) = self.open_handles.get_mut(open_slot) else {
            return false;
        };
        *record = None;
        true
    }

    /// Validate the Rust-owned open snapshot against the Rust-owned mounted volume.
    pub fn open_handle_live(&self, open_slot: usize, handle: u32, volume_slot: usize) -> bool {
        self.open_handles
            .get(open_slot)
            .and_then(Option::as_ref)
            .is_some_and(|record| {
                record.handle == handle
                    && record.volume_slot == volume_slot
                    && self.live(volume_slot, record.generation)
            })
    }

    pub fn live(&self, slot: usize, handle_generation: u64) -> bool {
        self.slots
            .get(slot)
            .and_then(Option::as_ref)
            .is_some_and(|volume| fatx_live(*volume, handle_generation))
    }
}

/// What an open handle needs to know about the volume slot it was opened on.
#[derive(Clone, Copy, Debug)]
pub struct VolumeState {
    pub in_use: bool,
    pub fatx: bool,
    pub has_image: bool,
    pub generation: u64,
}

/// A FATX handle is live while its slot still holds the same mounted image (`live_fatx`).
pub fn fatx_live(volume: VolumeState, handle_generation: u64) -> bool {
    volume.in_use && volume.fatx && volume.generation == handle_generation && volume.has_image
}

/// NtFlushBuffersFile on a FATX handle: the NTSTATUS for (handle live, a flush hook exists, the hook succeeded).
pub fn flush_status(live: bool, has_flush: bool, flushed: bool) -> u32 {
    const STATUS_SUCCESS: u32 = 0;
    const STATUS_UNSUCCESSFUL: u32 = 0xC000_0001;
    const STATUS_INVALID_HANDLE: u32 = 0xC000_0008;
    if !live {
        STATUS_INVALID_HANDLE
    } else if has_flush && flushed {
        STATUS_SUCCESS
    } else {
        STATUS_UNSUCCESSFUL
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_survives_and_closed_slots_reclaim() {
        let mut table = LinkHandles::new(2);
        assert_eq!(table.claim(4, b"\\Device\\CdRom0", |_| true), Some(0));
        assert_eq!(
            table.claim(8, b"\\Device\\Harddisk0\\Partition1", |_| true),
            Some(1)
        );
        assert_eq!(table.claim(12, b"x", |_| true), None);
        assert_eq!(table.claim(12, b"x", |h| h != 4), Some(0));
        assert_eq!(
            table.target(8),
            Some(&b"\\Device\\Harddisk0\\Partition1"[..])
        );
        assert_eq!(table.live_count(|h| h == 12), 1);
    }

    #[test]
    fn generations_and_liveness() {
        let mut generations = Generations::new();
        assert_eq!(generations.issue(), Some(1));
        generations.last = u64::MAX;
        assert_eq!(generations.issue(), None);
        let volume = VolumeState {
            in_use: true,
            fatx: true,
            has_image: true,
            generation: 3,
        };
        assert!(fatx_live(volume, 3));
        assert!(!fatx_live(volume, 2));
        assert_eq!(flush_status(false, true, true), 0xC000_0008);
        assert_eq!(flush_status(true, false, true), 0xC000_0001);
    }

    #[test]
    fn mounted_slot_epoch_is_owned_and_replaced_on_remount() {
        let mut volumes = FatxVolumes::new(2, 64, 40);
        let first = volumes
            .mount(1, true, b"\\Device\\Harddisk0\\Partition1")
            .unwrap();
        assert_eq!(first, 41);
        assert_eq!(
            volumes.find_prefix(b"\\Device\\Harddisk0\\Partition1"),
            Some(1)
        );
        assert_eq!(
            volumes.find_prefix(b"\\device\\harddisk0\\partition1"),
            None
        );
        assert!(volumes.live(1, first));
        assert!(!volumes.live(0, first));
        assert!(volumes.unmount(1));
        assert!(!volumes.live(1, first));
        assert_eq!(
            volumes.find_prefix(b"\\Device\\Harddisk0\\Partition1"),
            None
        );

        let second = volumes
            .mount(1, true, b"\\Device\\Harddisk0\\Partition1")
            .unwrap();
        assert_eq!(second, 42);
        assert!(!volumes.live(1, first));
        assert!(volumes.live(1, second));
    }

    #[test]
    fn invalid_mount_does_not_consume_or_publish_an_epoch() {
        let mut volumes = FatxVolumes::new(1, 64, 0);
        assert_eq!(volumes.mount(1, true, b"\\Device\\FATX"), None);
        assert_eq!(volumes.mount(0, false, b"\\Device\\FATX"), None);
        assert!(!volumes.live(0, 1));
        assert_eq!(volumes.mount(0, true, b"\\Device\\FATX"), Some(1));
    }

    #[test]
    fn seeded_mounts_survive_backend_install_and_clear_on_unmount_all() {
        let mut volumes = FatxVolumes::new(2, 64, 19);
        assert!(volumes.seed(1, true, true, true, 19, b"\\Device\\FATX1"));
        assert!(volumes.live(1, 19));
        assert_eq!(volumes.find_prefix(b"\\Device\\FATX1"), Some(1));
        assert!(volumes.unmount(1));
        assert!(!volumes.live(1, 19));
        assert!(volumes.seed(0, true, true, true, 20, b"\\Device\\FATX0"));
        volumes.clear();
        assert!(!volumes.live(0, 20));
        assert_eq!(volumes.find_prefix(b"\\Device\\FATX0"), None);
    }

    #[test]
    fn open_handle_uses_its_rust_owned_mount_epoch() {
        let mut volumes = FatxVolumes::new(2, 4, 10);
        let first = volumes.mount(1, true, b"\\Device\\FATX").unwrap();
        assert!(volumes.track_open_handle(2, 0x100, 1, first));
        assert!(volumes.open_handle_live(2, 0x100, 1));
        assert!(!volumes.open_handle_live(2, 0x101, 1));
        assert!(!volumes.open_handle_live(2, 0x100, 0));

        assert!(volumes.unmount(1));
        assert!(!volumes.open_handle_live(2, 0x100, 1));
        let second = volumes.mount(1, true, b"\\Device\\FATX").unwrap();
        assert_ne!(first, second);
        assert!(!volumes.open_handle_live(2, 0x100, 1));

        assert!(volumes.release_open_handle(2));
        assert!(volumes.track_open_handle(2, 0x101, 1, second));
        assert!(volumes.open_handle_live(2, 0x101, 1));
        assert!(volumes.release_open_handle(2));
        assert!(!volumes.open_handle_live(2, 0x101, 1));
    }

    #[test]
    fn open_handle_tracking_rejects_bad_slots_and_stale_epochs() {
        let mut volumes = FatxVolumes::new(1, 2, 0);
        assert!(!volumes.track_open_handle(2, 7, 0, 1));
        let generation = volumes.mount(0, true, b"\\Device\\FATX").unwrap();
        assert!(!volumes.track_open_handle(0, 7, 0, generation + 1));
        assert!(volumes.track_open_handle(0, 7, 0, generation));
        assert!(volumes.track_open_handle(0, 7, 0, generation));
        assert!(!volumes.track_open_handle(0, 8, 0, generation));
        assert!(volumes.open_handle_live(0, 7, 0));
        assert!(!volumes.release_open_handle(2));
    }

    #[test]
    fn exhausted_generation_does_not_replace_a_live_slot() {
        let mut volumes = FatxVolumes::new(1, 64, u64::MAX);
        assert!(volumes.seed(0, true, true, true, u64::MAX, b"\\Device\\FATX"));
        assert_eq!(volumes.mount(0, true, b"\\Device\\FATX"), None);
        assert!(volumes.live(0, u64::MAX));
    }
}
