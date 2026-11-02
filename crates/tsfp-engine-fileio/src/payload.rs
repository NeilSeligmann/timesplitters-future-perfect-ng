//! Per-open payload of the guest open-file slots (T2089): what each open asked for and where it stands. The slot index of
//! `handles.rs` says which slot a handle owns; this table owns the slot's state (desired access, share mode, open options,
//! the writable/directory/device capabilities copied from the resolution, the file position and the size). With
//! `--rust-fileio` `src/xbox/kernel_file.c` writes every change here and reads the copy-out and the write-path size from
//! here, keeping only the host descriptors, the volume link and the enumeration cursor. Its own C fields stay as the
//! cross-check (`kernel_file_payload_shadow_mismatches`).

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct OpenPayload {
    pub handle: u32,
    pub desired_access: u32,
    pub share_access: u32,
    pub open_options: u32,
    pub offset: u64,
    pub size: u64,
    pub writable: bool,
    pub is_directory: bool,
    pub device: bool,
}

#[derive(Default)]
pub struct PayloadTable {
    slots: Vec<Option<OpenPayload>>,
}

impl PayloadTable {
    pub fn new(count: usize) -> Self {
        PayloadTable { slots: vec![None; count] }
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }

    pub fn reset(&mut self) {
        self.slots.iter_mut().for_each(|s| *s = None);
    }

    /// A new open takes `slot`. Whatever the slot held before is replaced (the C side has released it already).
    pub fn open(&mut self, slot: usize, payload: OpenPayload) -> bool {
        match self.slots.get_mut(slot) {
            Some(s) => {
                *s = Some(payload);
                true
            }
            None => false,
        }
    }

    pub fn release(&mut self, slot: usize) {
        if let Some(s) = self.slots.get_mut(slot) {
            *s = None;
        }
    }

    pub fn get(&self, slot: usize) -> Option<OpenPayload> {
        self.slots.get(slot).copied().flatten()
    }

    /// NtReadFile/NtWriteFile/FilePositionInformation move the position. Refused on a free slot.
    pub fn set_offset(&mut self, slot: usize, offset: u64) -> bool {
        self.live(slot).map(|p| p.offset = offset).is_some()
    }

    /// A write past the end, a truncate, an end-of-file set or a FATX re-stat changes the size.
    pub fn set_size(&mut self, slot: usize, size: u64) -> bool {
        self.live(slot).map(|p| p.size = size).is_some()
    }

    /// Size after a write of `length` bytes at `offset`: grows to the end of the write, never shrinks.
    pub fn grow_to(&mut self, slot: usize, offset: u64, length: u64) -> Option<u64> {
        let p = self.live(slot)?;
        let end = offset.saturating_add(length);
        if end > p.size {
            p.size = end;
        }
        Some(p.size)
    }

    fn live(&mut self, slot: usize) -> Option<&mut OpenPayload> {
        self.slots.get_mut(slot).and_then(Option::as_mut)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_move_release() {
        let mut t = PayloadTable::new(4);
        assert!(t.open(1, OpenPayload { handle: 7, size: 10, writable: true, ..Default::default() }));
        assert!(!t.open(4, OpenPayload::default()));
        assert!(t.set_offset(1, 5));
        assert_eq!(t.grow_to(1, 8, 4), Some(12));
        assert_eq!(t.grow_to(1, 0, 1), Some(12));
        assert_eq!(t.get(1).unwrap().offset, 5);
        t.release(1);
        assert_eq!(t.get(1), None);
        assert!(!t.set_offset(1, 1));
        assert!(!t.set_size(2, 1));
    }
}
