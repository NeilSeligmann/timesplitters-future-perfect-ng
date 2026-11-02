//! Guest file-handle slot index (T2074 slice 3): which of the `KERNEL_FILE_OPEN_MAX` open-file slots of
//! `src/xbox/kernel_file.c` a guest handle owns, with the share mode and desired access each open asked for.
//! Port of the C slot logic (`claim_open_locked`: reclaim slots whose handle the object table no longer knows, take the
//! lowest free one; and the lookups by handle identity). The slot PAYLOAD (host descriptors, volume, FATX path, enumeration
//! state) stays in C and is indexed by the slot number this table hands out.
//! Share modes follow the NT rule of `nt.rs` (an open conflicts when a held open denies what it asks for, or it denies
//! what a held open uses). `commit` COUNTS the conflicts a route produces (`would_conflict`); `check` answers the opt-in
//! enforcement of T2074 slice 10 (`kernel_file_set_share_enforcement`, STATUS_SHARING_VIOLATION) before a commit.

pub const FILE_SHARE_READ: u32 = 1;
pub const FILE_SHARE_WRITE: u32 = 2;
pub const FILE_SHARE_DELETE: u32 = 4;
const FILE_READ_DATA: u32 = 1;
const FILE_WRITE_DATA: u32 = 2;
const FILE_APPEND_DATA: u32 = 4;
const DELETE: u32 = 0x0001_0000;
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const GENERIC_ALL: u32 = 0x1000_0000;

#[derive(Clone, Default)]
struct Slot {
    in_use: bool,
    handle: u32,
    path: Vec<u8>,
    desired: u32,
    share: u32,
}

#[derive(Default)]
pub struct OpenIndex {
    slots: Vec<Slot>,
    would_conflict: u32,
}

/// What an access mask needs from the share mode of the other opens: (read, write, delete).
fn uses(desired: u32) -> (bool, bool, bool) {
    let read = desired & (FILE_READ_DATA | GENERIC_READ | GENERIC_ALL) != 0;
    let write = desired & (FILE_WRITE_DATA | FILE_APPEND_DATA | GENERIC_WRITE | GENERIC_ALL) != 0;
    let delete = desired & (DELETE | GENERIC_ALL) != 0;
    (read, write, delete)
}

/// NT share rule between two opens of the same file.
pub fn share_conflicts(
    held_desired: u32,
    held_share: u32,
    new_desired: u32,
    new_share: u32,
) -> bool {
    let (hr, hw, hd) = uses(held_desired);
    let (nr, nw, nd) = uses(new_desired);
    let denies = |share: u32, r: bool, w: bool, d: bool| {
        (r && share & FILE_SHARE_READ == 0)
            || (w && share & FILE_SHARE_WRITE == 0)
            || (d && share & FILE_SHARE_DELETE == 0)
    };
    denies(held_share, nr, nw, nd) || denies(new_share, hr, hw, hd)
}

impl OpenIndex {
    pub fn new(count: usize) -> Self {
        OpenIndex {
            slots: vec![Slot::default(); count],
            would_conflict: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn reset(&mut self) {
        for slot in &mut self.slots {
            *slot = Slot::default();
        }
        self.would_conflict = 0;
    }

    /// `claim_open_locked`'s slot choice: every in-use slot whose handle is no longer live is freed (and reported through
    /// `reclaimed` so the caller can release its payload), then the lowest free slot is returned.
    pub fn claim_slot(
        &mut self,
        is_live: &mut dyn FnMut(u32) -> bool,
        reclaimed: &mut Vec<usize>,
    ) -> Option<usize> {
        let mut first_free = None;
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if slot.in_use && !is_live(slot.handle) {
                *slot = Slot::default();
                reclaimed.push(index);
            }
            if !slot.in_use && first_free.is_none() {
                first_free = Some(index);
            }
        }
        first_free
    }

    /// True when an open of `path` asking `desired`/`share` breaks the NT share rule against a held open (T2074 slice 10,
    /// the question the opt-in enforcement asks before committing; nothing is recorded).
    pub fn check(&self, path: &[u8], desired: u32, share: u32) -> bool {
        self.slots.iter().any(|held| {
            held.in_use
                && crate::hostdir::names_equal(&held.path, path)
                && share_conflicts(held.desired, held.share, desired, share)
        })
    }

    /// Records an open in `slot`. Returns true when it conflicts with another held open of the same path by the NT share rule.
    pub fn commit(
        &mut self,
        slot: usize,
        handle: u32,
        path: &[u8],
        desired: u32,
        share: u32,
    ) -> bool {
        let conflict = self.slots.iter().enumerate().any(|(i, held)| {
            i != slot
                && held.in_use
                && crate::hostdir::names_equal(&held.path, path)
                && share_conflicts(held.desired, held.share, desired, share)
        });
        if conflict {
            self.would_conflict += 1;
        }
        self.slots[slot] = Slot {
            in_use: true,
            handle,
            path: path.to_vec(),
            desired,
            share,
        };
        conflict
    }

    pub fn release(&mut self, slot: usize) {
        if let Some(s) = self.slots.get_mut(slot) {
            *s = Slot::default();
        }
    }

    /// The guest path an in-use `slot` was opened with (T3786: the root of a directory-relative open).
    pub fn path(&self, slot: usize) -> Option<&[u8]> {
        self.slots
            .get(slot)
            .filter(|s| s.in_use)
            .map(|s| s.path.as_slice())
    }

    /// First in-use slot owning `handle` (the lookup every C accessor does).
    pub fn find(&self, handle: u32) -> Option<usize> {
        self.slots
            .iter()
            .position(|s| s.in_use && s.handle == handle)
    }

    pub fn would_conflict(&self) -> u32 {
        self.would_conflict
    }
}
