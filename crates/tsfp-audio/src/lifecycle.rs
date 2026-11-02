//! DirectSound object lifecycle (T2046 slice 9b): the SILENT public device facade, the device lease
//! transaction and the passive stream and buffer headers, as a Rust port of `crates/tsfp-audio-conformance/oracle/dsound_device.c`,
//! the create halves of `dsound_stream.c` / `dsound_buffer.c` and their checked resets. No C, no locks
//! (single owner, the C mutexes only order its host threads). Guest heap objects come from the
//! [`GuestWorld`] trait, which the Rust `GuestRegions` model of tsfp-engine-memory implements (see
//! tests/lifecycle.rs). Refusals are `Err(Refusal)`: the C oracle stops the guest there.
//!
//! Evidence class: Tier 1 differential, ported from the C host (not the original): the header words
//! and reference counts are the C host's observations of the original (6 references when the codec is
//! ready, 1 otherwise). Not modelled, same as the C facade: nested APU/voice objects, IRQ/DPC/timer,
//! the guest critical section and IRQL (the facade never takes them, so no KernelMemory read is needed).

use crate::adapter::{GuestMemory, Refusal};

pub const CREATE_ENTRY: u32 = 0x0040_9635;
pub const RELEASE_ENTRY: u32 = 0x0040_6A8A;
pub const SINGLETON: u32 = 0x0041_2B30;
pub const GLOBAL_STATE: u32 = 0x0041_24A8;
pub const DEVICE_VTABLE: u32 = 0x004A_1CB0;
pub const DEVICE_BYTES: u32 = 44;
pub const STREAM_VTABLE: u32 = 0x004A_1D00;
pub const STREAM_SECONDARY: u32 = 0x004A_1CF0;
pub const STREAM_BYTES: u32 = 40;
/// The 14 original vtable targets a new stream may leave in the secondary table (oracle `safe_tables`:
/// unordered safety facts, each an unconditional T161 boundary of the embedding host, not a copy of the
/// original vtable bytes).
pub const STREAM_STOPPED_TARGETS: [u32; 14] = [
    0x0038_4859,
    0x0040_686C,
    0x0040_6879,
    0x0040_688A,
    0x0040_723F,
    0x0040_7286,
    0x0040_72D4,
    0x0040_733B,
    0x0040_7388,
    0x0040_73D3,
    0x0040_7424,
    0x0040_7883,
    0x0040_788D,
    0x0040_93AD,
];
pub const BUFFER_VTABLE: u32 = 0x004A_1CE0;
pub const BUFFER_BYTES: u32 = 36;
/// Offset of the public buffer interface inside its 36 byte header.
pub const BUFFER_PUBLIC_OFFSET: u32 = 28;
pub const E_OUTOFMEMORY: u32 = 0x8007_000E;
pub const DSERR_NODRIVER: u32 = 0x8878_0078;
/// The original device Release that would leave this many references destroys the device.
pub const DESTROY_REFERENCES: u32 = 6;

/// Guest memory plus the guest heap operations the lifecycle needs (the C `guest_heap_*` API).
pub trait GuestWorld: GuestMemory {
    fn mapped(&self, address: u32, len: u32) -> bool;
    fn write_bytes(&mut self, address: u32, data: &[u8]) -> bool;
    /// New private heap (`GUEST_HEAP_CHUNK_MIN` initial size, growable). 0 on failure.
    fn heap_create(&mut self) -> u32;
    fn heap_alloc(&mut self, heap: u32, bytes: u32) -> u32;
    fn heap_destroy(&mut self, heap: u32) -> bool;
    fn heap_valid(&self, heap: u32) -> bool;
    fn heap_block_size(&self, heap: u32, address: u32) -> Option<u32>;
}

use crate::guest_ranges::overlaps;

fn words_bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|w| w.to_le_bytes()).collect()
}

fn read_words<W: GuestWorld + ?Sized>(w: &W, address: u32, count: usize) -> Option<Vec<u32>> {
    let b = w.read_bytes(address, count * 4)?;
    Some(
        b.chunks_exact(4)
            .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect(),
    )
}

/// An identity token of one device lease (oracle: `dsound_device_lease`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lease {
    pub device_heap: u32,
    pub internal_address: u32,
    pub serial: u64,
}

/// The private heap block a lease guards (oracle: `dsound_device_lease_child`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Child {
    pub heap: u32,
    pub address: u32,
    pub bytes: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaseError {
    Invalid,
    ReferenceLimit,
    SerialLimit,
    PrepareFailed,
    WriteRefused,
}

/// Why [`Device::release_lease_with`] failed, and whether the caller's `prepare` already ran.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReleaseError {
    /// Refused before `prepare` ran: no callback to undo.
    Refused(LeaseError),
    /// Refused after `prepare` ran: the caller aborts its prepared state.
    AfterPrepare(LeaseError),
}

/// The SILENT public device facade and its lease table.
#[derive(Default)]
pub struct Device {
    owned_heap: u32,
    owned_address: u32,
    shadow_reference: u32,
    leases: Vec<(Lease, Child)>,
    lease_serial: u64,
    announced: bool,
    pub movie_calls: bool,
    movie_references: u32,
}

impl Device {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn owned_address(&self) -> u32 {
        self.owned_address
    }
    pub fn owned_heap(&self) -> u32 {
        self.owned_heap
    }
    /// True once the first facade or child object has been announced (peek of [`Device::take_announce`]).
    pub fn announced(&self) -> bool {
        self.announced
    }
    pub fn reference_count(&self) -> u32 {
        self.shadow_reference
    }
    pub fn movie_references(&self) -> u32 {
        self.movie_references
    }
    pub fn lease_count(&self) -> usize {
        self.leases.len()
    }
    /// True once, the first time a facade or child object is announced (the C log line).
    pub fn take_announce(&mut self) -> bool {
        !std::mem::replace(&mut self.announced, true)
    }

    fn header_words(address: u32, reference: u32) -> [u32; 11] {
        let mut w = [0u32; 11];
        w[0] = DEVICE_VTABLE;
        w[1] = reference;
        w[4] = address + 16;
        w[5] = address + 16;
        w
    }

    /// Full ownership validation (oracle `validate_owned`): header bytes, singleton, heap block size.
    pub fn validate_owned<W: GuestWorld + ?Sized>(&self, w: &W, interface: u32) -> bool {
        if self.owned_address == 0
            || interface != self.owned_address + 8
            || self.shadow_reference == 0
        {
            return false;
        }
        let Some(actual) = read_words(w, self.owned_address, 11) else {
            return false;
        };
        let Some(singleton) = w.read_u32(SINGLETON) else {
            return false;
        };
        singleton == self.owned_address
            && w.heap_valid(self.owned_heap)
            && w.heap_block_size(self.owned_heap, self.owned_address) == Some(DEVICE_BYTES)
            && actual == Self::header_words(self.owned_address, self.shadow_reference)
    }

    /// DirectSoundCreate 0x409635 (guid, output, outer). Returns the HRESULT. `codec_ready` is the
    /// `dsound_hle_codec_ready()` answer. `movie` marks the XMV caller 0x0044570B.
    pub fn create<W: GuestWorld>(
        &mut self,
        w: &mut W,
        guid: u32,
        output: u32,
        outer: u32,
        movie: bool,
        codec_ready: bool,
    ) -> Result<u32, Refusal> {
        self.create_with_note(w, guid, output, outer, movie, codec_ready, |_, _| true)
    }

    /// [`Device::create`] with the optional PCM epoch observer of `dsound_device_set_create_note`: `note(heap,
    /// address)` sees a new device before it is published and may reject it, which rolls the allocation back
    /// and refuses (oracle `create_device`).
    #[allow(clippy::too_many_arguments)]
    pub fn create_with_note<W: GuestWorld>(
        &mut self,
        w: &mut W,
        guid: u32,
        output: u32,
        outer: u32,
        movie: bool,
        codec_ready: bool,
        note: impl FnOnce(u32, u32) -> bool,
    ) -> Result<u32, Refusal> {
        if guid != 0 || outer != 0 {
            return Err(Refusal("only NULL GUID/outer are recovered"));
        }
        if !w.mapped(output, 4) || !w.mapped(SINGLETON, 4) || !w.mapped(DEVICE_VTABLE, 12) {
            return Err(Refusal(
                "output, singleton or original vtable is not mapped",
            ));
        }
        if overlaps(output, 4, SINGLETON, 4)
            || overlaps(output, 4, DEVICE_VTABLE, 12)
            || (self.owned_address != 0 && overlaps(output, 4, self.owned_address, DEVICE_BYTES))
        {
            return Err(Refusal("output alias with device state is not recovered"));
        }
        let singleton = w
            .read_u32(SINGLETON)
            .ok_or(Refusal("singleton is unreadable"))?;
        let mut status = 0;
        if movie && self.owned_address == 0 {
            return Err(Refusal(
                "a movie DirectSoundCreate needs the device the startup already created",
            ));
        }
        if self.owned_address != 0 {
            if !self.validate_owned(&*w, self.owned_address + 8)
                || self.shadow_reference == u32::MAX
            {
                return Err(Refusal(
                    "cached singleton/header/heap generation/reference changed",
                ));
            }
            if movie && self.movie_references == u32::MAX {
                return Err(Refusal("movie reference count would overflow"));
            }
            if !w.write_u32(self.owned_address + 4, self.shadow_reference + 1) {
                return Err(Refusal("cached reference is not writable"));
            }
            self.shadow_reference += 1;
            if movie {
                self.movie_references += 1;
            }
        } else {
            if singleton != 0 {
                return Err(Refusal("existing device is not owned by this facade"));
            }
            let heap = w.heap_create();
            if heap == 0 {
                return Ok(E_OUTOFMEMORY);
            }
            let address = w.heap_alloc(heap, DEVICE_BYTES);
            if address == 0 {
                w.heap_destroy(heap);
                return Ok(E_OUTOFMEMORY);
            }
            if !w.mapped(address, DEVICE_BYTES) {
                w.heap_destroy(heap);
                return Err(Refusal("new owned device is not mapped"));
            }
            if !note(heap, address) {
                return Err(Refusal(if w.heap_destroy(heap) {
                    "PCM epoch rejected retained audio or inactive renderer"
                } else {
                    "PCM epoch rejected and unpublished allocation rollback failed"
                }));
            }
            self.owned_heap = heap;
            self.owned_address = address;
            self.shadow_reference = if codec_ready { 6 } else { 1 };
            w.write_bytes(
                address,
                &words_bytes(&Self::header_words(address, self.shadow_reference)),
            );
            w.write_u32(SINGLETON, address);
            if !codec_ready {
                status = DSERR_NODRIVER;
            }
        }
        if status == 0 {
            w.write_u32(output, self.owned_address + 8);
        }
        self.announced = true;
        Ok(status)
    }

    /// Device Release 0x406A8A from the XMV caller 0x00445726. Returns the new reference count.
    pub fn release_movie<W: GuestWorld>(
        &mut self,
        w: &mut W,
        interface: u32,
    ) -> Result<u32, Refusal> {
        if !self.movie_calls {
            return Err(Refusal("movie device calls are disabled"));
        }
        if self.shadow_reference == u32::MAX || !self.validate_owned(&*w, interface) {
            return Err(Refusal(
                "Release argument is not the owned device interface",
            ));
        }
        if self.movie_references == 0 {
            return Err(Refusal(
                "no movie reference is outstanding, releasing the startup reference would run the original device destructor",
            ));
        }
        if self.shadow_reference - 1 < DESTROY_REFERENCES {
            return Err(Refusal(
                "Release would reach the original device destructor",
            ));
        }
        if !w.write_u32(self.owned_address + 4, self.shadow_reference - 1) {
            return Err(Refusal("device reference is not writable"));
        }
        self.shadow_reference -= 1;
        self.movie_references -= 1;
        Ok(self.shadow_reference)
    }

    fn valid_child<W: GuestWorld + ?Sized>(&self, w: &W, c: &Child) -> bool {
        c.heap != 0
            && c.heap != self.owned_heap
            && c.address != 0
            && c.bytes != 0
            && u64::from(c.address) + u64::from(c.bytes) <= 1 << 32
            && w.heap_valid(c.heap)
            && w.heap_block_size(c.heap, c.address) == Some(c.bytes)
            && !overlaps(c.address, c.bytes, self.owned_address, DEVICE_BYTES)
    }

    /// Device lease acquire (oracle `dsound_device_acquire_lease`). `prepare` runs before the reference
    /// is taken and returns the child block or `None`; the caller owns abort (cleanup of what prepare
    /// allocated) on `Err`.
    pub fn acquire_lease<W: GuestWorld>(
        &mut self,
        w: &mut W,
        interface: u32,
        prepare: impl FnOnce(&mut W, &Lease) -> Option<Child>,
    ) -> Result<Lease, LeaseError> {
        if !self.validate_owned(&*w, interface) {
            return Err(LeaseError::Invalid);
        }
        if self.shadow_reference == u32::MAX {
            return Err(LeaseError::ReferenceLimit);
        }
        if self.lease_serial == u64::MAX {
            return Err(LeaseError::SerialLimit);
        }
        if !w.write_u32(self.owned_address + 4, self.shadow_reference) {
            return Err(LeaseError::WriteRefused);
        }
        let token = Lease {
            device_heap: self.owned_heap,
            internal_address: self.owned_address,
            serial: self.lease_serial + 1,
        };
        let child = prepare(w, &token).ok_or(LeaseError::PrepareFailed)?;
        if !self.valid_child(&*w, &child)
            || self.leases.iter().any(|(_, c)| *c == child)
            || !self.validate_owned(&*w, interface)
        {
            return Err(LeaseError::Invalid);
        }
        if !w.write_u32(self.owned_address + 4, self.shadow_reference + 1) {
            return Err(LeaseError::WriteRefused);
        }
        self.shadow_reference += 1;
        self.lease_serial = token.serial;
        self.leases.insert(0, (token, child));
        Ok(token)
    }

    /// Device lease release (oracle `dsound_device_release_lease`): `child` is what the caller's
    /// prepare reports and must equal the recorded one. Consumes the lease, does not free the child.
    pub fn release_lease<W: GuestWorld>(
        &mut self,
        w: &mut W,
        lease: &Lease,
        child: &Child,
    ) -> Result<(), LeaseError> {
        self.release_lease_with(w, lease, |_| Some(*child))
            .map_err(|e| match e {
                ReleaseError::Refused(e) | ReleaseError::AfterPrepare(e) => e,
            })
    }

    /// [`Device::release_lease`] with the C order: the reference probe write comes first, then `prepare`
    /// reports the child, which must equal the recorded one. Errors raised after `prepare` ran are
    /// [`ReleaseError::AfterPrepare`]: the caller must run its abort (oracle `ops->abort`).
    pub fn release_lease_with<W: GuestWorld>(
        &mut self,
        w: &mut W,
        lease: &Lease,
        prepare: impl FnOnce(&mut W) -> Option<Child>,
    ) -> Result<(), ReleaseError> {
        let Some(index) = self.leases.iter().position(|(t, _)| t == lease) else {
            return Err(ReleaseError::Refused(LeaseError::Invalid));
        };
        let recorded = self.leases[index].1;
        if lease.device_heap != self.owned_heap
            || lease.internal_address != self.owned_address
            || !self.validate_owned(&*w, self.owned_address + 8)
            || !self.valid_child(&*w, &recorded)
            || self.shadow_reference <= self.leases.len() as u32
        {
            return Err(ReleaseError::Refused(LeaseError::Invalid));
        }
        if !w.write_u32(self.owned_address + 4, self.shadow_reference) {
            return Err(ReleaseError::Refused(LeaseError::WriteRefused));
        }
        let Some(child) = prepare(w) else {
            return Err(ReleaseError::AfterPrepare(LeaseError::PrepareFailed));
        };
        if child != recorded
            || !self.valid_child(&*w, &child)
            || !self.validate_owned(&*w, self.owned_address + 8)
        {
            return Err(ReleaseError::AfterPrepare(LeaseError::Invalid));
        }
        if !w.write_u32(self.owned_address + 4, self.shadow_reference - 1) {
            return Err(ReleaseError::AfterPrepare(LeaseError::WriteRefused));
        }
        self.shadow_reference -= 1;
        self.leases.remove(index);
        Ok(())
    }

    /// Checked device reset (oracle `dsound_device_reset_checked`): refuses while a lease is live.
    pub fn reset_checked<W: GuestWorld>(&mut self, w: &mut W) -> bool {
        if !self.leases.is_empty() {
            return false;
        }
        if self.owned_address != 0 && w.heap_valid(self.owned_heap) {
            if !self.validate_owned(&*w, self.owned_address + 8) || !w.write_u32(SINGLETON, 0) {
                return false;
            }
            if !w.heap_destroy(self.owned_heap) {
                w.write_u32(SINGLETON, self.owned_address);
                return false;
            }
        }
        self.owned_heap = 0;
        self.owned_address = 0;
        self.shadow_reference = 0;
        self.movie_references = 0;
        self.announced = false;
        true
    }
}

/// The measured creation scope of a stream or buffer (oracle `dsound_stream_scope` / `dsound_buffer_scope`).
#[derive(Clone, Copy, Debug)]
pub struct Scope {
    pub flags: u32,
    pub descriptor_address: u32,
    pub format_address: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Stream,
    Buffer,
}

/// One passive child object (a stream or a buffer) and the lease that pins it to the device.
#[derive(Clone, Debug)]
pub struct Object {
    pub kind: Kind,
    pub lease: Lease,
    pub heap: u32,
    pub header_address: u32,
    /// The guest-visible interface pointer: the header for a stream, header+28 for a buffer.
    pub public_address: u32,
    pub publication_address: u32,
    pub header: Vec<u32>,
    pub scope: Scope,
}

impl Object {
    pub fn bytes_len(&self) -> u32 {
        match self.kind {
            Kind::Stream => STREAM_BYTES,
            Kind::Buffer => BUFFER_BYTES,
        }
    }
    fn child(&self) -> Child {
        Child {
            heap: self.heap,
            address: self.header_address,
            bytes: self.bytes_len(),
        }
    }
}

/// Oracle `safe_tables`: every one of the 15 words of the original secondary table is a protected target.
pub fn stream_tables_are_stopped(table: &[u32]) -> bool {
    table.len() == 15
        && table
            .iter()
            .all(|word| STREAM_STOPPED_TARGETS.contains(word))
}

fn stream_table_is_stopped<W: GuestWorld + ?Sized>(w: &W) -> bool {
    read_words(w, STREAM_SECONDARY, 15).is_some_and(|t| stream_tables_are_stopped(&t))
}

/// The table of passive streams and buffers (oracle: the `streams`/`buffers` lists).
#[derive(Default)]
pub struct Objects {
    pub live: Vec<Object>,
}

impl Objects {
    pub fn new() -> Self {
        Self::default()
    }

    fn aliases_state(&self, address: u32, bytes: u32, internal: u32, kind: Kind) -> bool {
        if overlaps(address, bytes, internal, DEVICE_BYTES)
            || overlaps(address, bytes, SINGLETON, 4)
            || overlaps(address, bytes, GLOBAL_STATE, 4)
        {
            return true;
        }
        if kind == Kind::Stream && overlaps(address, bytes, STREAM_SECONDARY, 60) {
            return true;
        }
        self.live.iter().any(|o| {
            overlaps(address, bytes, o.header_address, o.bytes_len())
                || overlaps(address, bytes, o.publication_address, 4)
                || (o.kind == Kind::Stream
                    && (overlaps(address, bytes, o.scope.descriptor_address, 24)
                        || overlaps(address, bytes, o.scope.format_address, 20)))
        })
    }

    /// dsound_stream_create / dsound_buffer_create: lease a passive header from the device and
    /// publish its pointer at `output`. Returns the HRESULT (0, or E_OUTOFMEMORY).
    pub fn create<W: GuestWorld>(
        &mut self,
        device: &mut Device,
        w: &mut W,
        kind: Kind,
        scope: Scope,
        output: u32,
    ) -> Result<u32, Refusal> {
        let singleton = w
            .read_u32(SINGLETON)
            .filter(|&d| d != 0 && d <= u32::MAX - 8)
            .ok_or(Refusal("no owned SILENT device"))?;
        let mut error: Option<&'static str> = None;
        let mut oom = false;
        let mut made: Option<(u32, u32, Vec<u32>)> = None;
        let this = &*self;
        let result = device.acquire_lease(w, singleton + 8, |w, cand| {
            let internal = cand.internal_address;
            let vtable_ok = match kind {
                Kind::Stream => w.read_bytes(STREAM_SECONDARY, 60).is_some(),
                Kind::Buffer => w.read_bytes(BUFFER_VTABLE, 16).is_some(),
            };
            let Some(old) = w.read_u32(output).filter(|_| vtable_ok) else {
                error = Some("output/original vtables unreadable");
                return None;
            };
            if kind == Kind::Stream && !stream_table_is_stopped(&*w) {
                error = Some("original vtable contains an unprotected target");
                return None;
            }
            if this.aliases_state(output, 4, internal, kind)
                || overlaps(output, 4, scope.descriptor_address, 24)
                || overlaps(output, 4, scope.format_address, 20)
                || overlaps(scope.descriptor_address, 24, internal, DEVICE_BYTES)
                || overlaps(scope.format_address, 20, internal, DEVICE_BYTES)
            {
                error = Some("unproven output/input alias with owned state");
                return None;
            }
            if !w.write_u32(output, old) {
                error = Some("output is not writable");
                return None;
            }
            let heap = w.heap_create();
            if heap == 0 {
                oom = true;
                return None;
            }
            let bytes = if kind == Kind::Stream {
                STREAM_BYTES
            } else {
                BUFFER_BYTES
            };
            let address = w.heap_alloc(heap, bytes);
            made = Some((heap, address, vec![]));
            if address == 0 {
                oom = true;
                return None;
            }
            let mut header = vec![0u32; (bytes / 4) as usize];
            match kind {
                Kind::Stream => {
                    header[..4].copy_from_slice(&[STREAM_VTABLE, STREAM_SECONDARY, 1, internal])
                }
                Kind::Buffer => header[..3].copy_from_slice(&[BUFFER_VTABLE, 1, internal]),
            }
            made = Some((heap, address, header.clone()));
            if this.aliases_state(address, bytes, internal, kind)
                || overlaps(address, bytes, output, 4)
                || overlaps(address, bytes, scope.descriptor_address, 24)
                || overlaps(address, bytes, scope.format_address, 20)
                || !w.write_bytes(address, &words_bytes(&header))
            {
                error = Some("new private allocation/alias is invalid");
                return None;
            }
            Some(Child {
                heap,
                address,
                bytes,
            })
        });
        match result {
            Ok(lease) => {
                let (heap, address, header) = made.expect("prepare ran");
                let public = if kind == Kind::Buffer {
                    address + BUFFER_PUBLIC_OFFSET
                } else {
                    address
                };
                w.write_u32(output, public);
                device.take_announce();
                self.live.insert(
                    0,
                    Object {
                        kind,
                        lease,
                        heap,
                        header_address: address,
                        public_address: public,
                        publication_address: output,
                        header,
                        scope,
                    },
                );
                Ok(0)
            }
            Err(_) => {
                // abort: the unpublished private heap is destroyed (the C cleanup_aborted_create)
                if let Some((heap, _, _)) = made {
                    if heap != 0 && w.heap_valid(heap) {
                        w.heap_destroy(heap);
                    }
                }
                if oom && error.is_none() {
                    return Ok(E_OUTOFMEMORY);
                }
                Err(Refusal(error.unwrap_or("device lease transaction refused")))
            }
        }
    }

    /// Object count of one kind.
    pub fn count(&self, kind: Kind) -> usize {
        self.live.iter().filter(|o| o.kind == kind).count()
    }

    pub fn find(&self, public_address: u32) -> Option<&Object> {
        self.live
            .iter()
            .find(|o| o.public_address == public_address)
    }

    /// True when the guest header still equals what the lease recorded and its heap block is intact.
    pub fn header_intact<W: GuestWorld + ?Sized>(&self, w: &W, o: &Object) -> bool {
        self.valid(w, o)
    }

    fn valid<W: GuestWorld + ?Sized>(&self, w: &W, o: &Object) -> bool {
        let Some(actual) = read_words(w, o.header_address, o.header.len()) else {
            return false;
        };
        actual == o.header
            && w.heap_valid(o.heap)
            && w.heap_block_size(o.heap, o.header_address) == Some(o.bytes_len())
    }

    /// Checked reset of every object (oracle `reset_streams` / `reset_buffers`): release each lease,
    /// then destroy its private heap. False on the first refusal, earlier objects stay cleaned.
    pub fn reset_checked<W: GuestWorld>(&mut self, device: &mut Device, w: &mut W) -> bool {
        while let Some(o) = self.live.first().cloned() {
            if !self.valid(&*w, &o) || device.release_lease(w, &o.lease, &o.child()).is_err() {
                return false;
            }
            if !w.heap_destroy(o.heap) {
                return false;
            }
            self.live.remove(0);
        }
        true
    }
}
