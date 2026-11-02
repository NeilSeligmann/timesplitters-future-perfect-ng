//! Rust-owned reliable packet queue for the measured enqueue/acknowledgement boundary.
//!
//! The queue owns packet bytes and identifies peers by their copied value. It does not expose
//! guest addresses or borrow caller storage. Its layout-independent operations reproduce the
//! measured 0x353160 enqueue and 0x3532F0 acknowledgement effects for payload lengths up to the
//! fixed 255-byte copied extent. Retry callback/peer lifetime policy is intentionally separate:
//! see `docs/t1692-rea-session.md` and the still-open lifetime unknown recorded there.

/// Number of original reliable records (stride 0x114, active bit at +0x110).
pub const RELIABLE_CAPACITY: usize = 2048;
/// Bytes copied into each reliable record, independent of its transmitted length.
pub const RELIABLE_PAYLOAD_BYTES: usize = 255;

/// Copied peer identity from the destination record, not its guest address or a roster slot.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct PeerId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacketHandle {
    slot: u16,
    generation: u64,
}

impl PacketHandle {
    pub fn slot(self) -> usize {
        self.slot as usize
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnqueueError {
    /// The original record stores 255 copied bytes; larger lengths would read outside that
    /// owned extent and are outside this native API's accepted input domain.
    LengthExceedsCopiedPayload,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcknowledgeError {
    /// The original message starts with an unsigned byte count.
    TooManySequences,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Entry {
    active: bool,
    sequence: u16,
    peer: PeerId,
    transmitted_len: u16,
    elapsed: u32,
    retry_count: u8,
    payload: [u8; RELIABLE_PAYLOAD_BYTES],
    generation: u64,
}

impl Default for Entry {
    fn default() -> Self {
        Self {
            active: false,
            sequence: u16::MAX,
            peer: PeerId(u32::MAX),
            transmitted_len: 0,
            elapsed: 0,
            retry_count: 0,
            payload: [0; RELIABLE_PAYLOAD_BYTES],
            generation: 0,
        }
    }
}

/// Read-only typed view of one queue record, including retained inactive length state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReliableEntryView<'a> {
    pub active: bool,
    pub sequence: u16,
    pub peer: PeerId,
    pub transmitted_len: u16,
    pub elapsed: u32,
    pub retry_count: u8,
    pub payload: &'a [u8; RELIABLE_PAYLOAD_BYTES],
    generation: u64,
}

impl ReliableEntryView<'_> {
    pub fn handle(self, slot: usize) -> Option<PacketHandle> {
        (self.active && slot < RELIABLE_CAPACITY).then_some(PacketHandle {
            slot: slot as u16,
            generation: self.generation,
        })
    }
}

/// Owned fixed-capacity reliable packet state corresponding to the original 2048 x 0x114 table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReliableQueue {
    entries: Box<[Entry]>,
    cursor: usize,
}

impl Default for ReliableQueue {
    fn default() -> Self {
        Self::with_cursor(0)
    }
}

impl ReliableQueue {
    pub fn with_cursor(cursor: usize) -> Self {
        let entries = (0..RELIABLE_CAPACITY)
            .map(|_| Entry::default())
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            entries,
            cursor: cursor % RELIABLE_CAPACITY,
        }
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn len(&self) -> usize {
        self.entries.iter().filter(|entry| entry.active).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn entry(&self, slot: usize) -> Option<ReliableEntryView<'_>> {
        self.entries.get(slot).map(|entry| ReliableEntryView {
            active: entry.active,
            sequence: entry.sequence,
            peer: entry.peer,
            transmitted_len: entry.transmitted_len,
            elapsed: entry.elapsed,
            retry_count: entry.retry_count,
            payload: &entry.payload,
            generation: entry.generation,
        })
    }

/// Copy a full fixed-size payload into the first free record from the retained cursor.
    /// Returns `Ok(None)` when all 2048 records are active; the cursor then stays unchanged.
    pub fn enqueue(
        &mut self,
        peer: PeerId,
        sequence: u16,
        payload: &[u8; RELIABLE_PAYLOAD_BYTES],
        transmitted_len: u16,
    ) -> Result<Option<PacketHandle>, EnqueueError> {
        if transmitted_len as usize > RELIABLE_PAYLOAD_BYTES {
            return Err(EnqueueError::LengthExceedsCopiedPayload);
        }
        for distance in 0..RELIABLE_CAPACITY {
            let slot = (self.cursor + distance) % RELIABLE_CAPACITY;
            let entry = &mut self.entries[slot];
            if entry.active {
                continue;
            }
            entry.active = true;
            entry.sequence = sequence;
            entry.peer = peer;
            entry.transmitted_len = transmitted_len;
            entry.elapsed = 0;
            entry.retry_count = 0;
            entry.payload.copy_from_slice(payload);
            entry.generation = entry.generation.wrapping_add(1);
            self.cursor = slot;
            return Ok(Some(PacketHandle {
                slot: slot as u16,
                generation: entry.generation,
            }));
        }
        Ok(None)
    }

    /// Retire the first active record matching each sequence and copied peer identity.
    /// As in 0x3532F0, the transmitted-length field is left intact when a record is cleared.
    pub fn acknowledge(
        &mut self,
        peer: PeerId,
        sequences: &[u16],
    ) -> Result<usize, AcknowledgeError> {
        if sequences.len() > u8::MAX as usize {
            return Err(AcknowledgeError::TooManySequences);
        }
        let mut removed = 0;
        for sequence in sequences {
            let found = self.entries.iter().position(|entry| {
                entry.active && entry.sequence == *sequence && entry.peer == peer
            });
            if let Some(slot) = found {
                let entry = &mut self.entries[slot];
                entry.active = false;
                entry.elapsed = 0;
                entry.retry_count = 0;
                entry.sequence = u16::MAX;
                entry.peer = PeerId(u32::MAX);
                entry.payload.fill(0);
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// Resolve a current handle. Acknowledgement and later slot reuse invalidate stale handles.
    pub fn get(&self, handle: PacketHandle) -> Option<ReliableEntryView<'_>> {
        let entry = self.entries.get(handle.slot as usize)?;
        if !entry.active || entry.generation != handle.generation {
            return None;
        }
        self.entry(handle.slot as usize)
    }
}
