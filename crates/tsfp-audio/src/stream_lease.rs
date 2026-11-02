//! Typed ownership and header validation for the passive DirectSound stream adapter.
//!
//! The host adapter supplies only the probes that require its guest-memory and heap APIs.
//! This module owns the lease/header predicate and preserves its short-circuit order.

use crate::scope::{DESCRIPTOR_BYTES, FORMAT_BYTES, stream_scope_validate};

pub const STREAM_HEADER_WORDS: usize = 10;
pub const STREAM_ALLOCATION_BYTES: u32 = 40;

/// Immutable creation inputs retained with the Rust-owned stream lease.
/// The layout matches `dsound_stream_scope` at the C adapter boundary.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamLeaseScope {
    pub descriptor_address: u32,
    pub format_address: u32,
    pub flags: u32,
    pub max_packets: u32,
    pub sample_rate: u32,
    pub average_bytes_per_second: u32,
    pub channels: u16,
    pub block_align: u16,
    pub descriptor: [u8; DESCRIPTOR_BYTES],
    pub format: [u8; FORMAT_BYTES],
}

/// Mutable format state accepted by stream SetFormat handlers.
///
/// This is separate from `StreamLeaseScope::format`, which is the immutable format
/// snapshot consumed during Create. The bytes here begin zeroed and change only
/// after an admitted SetFormat transaction commits.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamFormatState {
    pub bytes: [u8; FORMAT_BYTES],
    pub sets: u32,
}

/// Last accepted stream SetVolume state, retained with its exact active lease.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamVolumeState {
    pub seen: u32,
    pub value: i32,
}

/// Caller facts for the bounded live stream SetVolume policy.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamVolumeRequest {
    pub argument: u32,
    pub count: u32,
    pub completion_available: u32,
}

pub const STREAM_VOLUME_ACCEPTED: u32 = 0;
pub const STREAM_VOLUME_STALE_LEASE: u32 = 1;
pub const STREAM_VOLUME_UNSUPPORTED_SCOPE: u32 = 2;
pub const STREAM_VOLUME_BAD_REQUEST: u32 = 3;
pub const STREAM_VOLUME_NEEDS_FORMAT: u32 = 4;
pub const STREAM_VOLUME_BAD_STATE: u32 = 5;
pub const STREAM_VOLUME_POLICY_REFUSED: u32 = 6;

/// Last accepted stream control observations used by measured startup gates.
/// This records handler state only; it does not pause playback, flush packets,
/// or publish completion.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamControlState {
    pub pause_seen: u32,
    pub pause_mode: u32,
    pub flush_seen: u32,
    pub flush_time_low: u32,
    pub flush_time_high: u32,
    pub flush_flags: u32,
    pub discontinuity_seen: u32,
    pub revision: u32,
}

pub const STREAM_CONTROL_DISCONTINUITY: u32 = 1;
pub const STREAM_CONTROL_FLUSH_EX: u32 = 2;
pub const STREAM_CONTROL_PAUSE: u32 = 3;

/// Inputs to the bounded live stream-control policy used by the Rust audio host.
/// The request contains only caller facts; the lease registry supplies the saved
/// creation scope, ordered startup cache, format count, and current control state.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamControlRequest {
    pub action: u32,
    pub argument: u32,
    pub count: u32,
    pub apply: u32,
    pub completion_available: u32,
    pub pause_callback_available: u32,
}

pub const STREAM_CONTROL_ACCEPTED: u32 = 0;
pub const STREAM_CONTROL_STALE_LEASE: u32 = 1;
pub const STREAM_CONTROL_UNSUPPORTED_SCOPE: u32 = 2;
pub const STREAM_CONTROL_BAD_SEQUENCE: u32 = 3;
pub const STREAM_CONTROL_BAD_FLUSH: u32 = 4;
pub const STREAM_CONTROL_BAD_RUNTIME_PAUSE: u32 = 5;
pub const STREAM_CONTROL_BAD_PAUSE: u32 = 6;
pub const STREAM_CONTROL_PAUSE_NEEDS_FORMAT: u32 = 7;
pub const STREAM_CONTROL_INVALID_REQUEST: u32 = 8;
pub const STREAM_CONTROL_STATE_REJECTED: u32 = 9;

pub const STREAM_SPATIAL_ACCEPTED: u32 = 0;
pub const STREAM_SPATIAL_STALE_LEASE: u32 = 1;
pub const STREAM_SPATIAL_UNSUPPORTED_SCOPE: u32 = 2;
pub const STREAM_SPATIAL_BAD_SEQUENCE: u32 = 3;
pub const STREAM_SPATIAL_BAD_REQUEST: u32 = 4;

/// Accepted spatial stream observations retained with one exact active lease.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamSpatialState {
    /// Ordered startup settings cache progression: 0, 1, 3, then 7.
    pub startup_cache_mask: u32,
    /// First I3DL2 input retained as a raw host observation.
    pub i3dl2_address: u32,
    pub i3dl2_words: [u32; 9],
    pub min_distance_bits: u32,
    pub position_bits: [u32; 3],
    pub position_sets: u32,
    pub max_distance_seen: u32,
    pub max_distance_bits: u32,
    pub curve_address: u32,
    pub curve_count: u32,
    pub revision: u32,
}

impl StreamLeaseScope {
    /// Validate the saved byte snapshots and require the cached projection to agree.
    pub fn is_valid(&self) -> bool {
        if self.descriptor_address == 0
            || u64::from(self.descriptor_address) + DESCRIPTOR_BYTES as u64 > (1u64 << 32)
        {
            return false;
        }
        let Some(parsed) = stream_scope_validate(&self.descriptor, &self.format) else {
            return false;
        };
        parsed.format_address == self.format_address
            && parsed.flags == self.flags
            && parsed.max_packets == self.max_packets
            && parsed.sample_rate == self.sample_rate
            && parsed.average_bytes_per_second == self.average_bytes_per_second
            && parsed.channels == self.channels
            && parsed.block_align == self.block_align
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceLeaseIdentity {
    pub device_heap: u32,
    pub internal_address: u32,
    pub serial: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamLeaseHeader {
    pub lease: DeviceLeaseIdentity,
    pub stream_address: u32,
    pub stream_heap: u32,
    pub header: [u32; STREAM_HEADER_WORDS],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StreamLeaseLifecycle {
    Pending,
    Active,
    Detached,
    Rollback,
    RetiringDetached,
    RetiringRollback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamLeaseRecord {
    pub stream_address: u32,
    pub stream_heap: u32,
    pub publication_address: u32,
    pub header: [u32; STREAM_HEADER_WORDS],
    /// Validated creation scope is present for production Rust-host leases.
    pub scope: Option<StreamLeaseScope>,
    /// Mutable SetFormat state; distinct from the immutable creation scope above.
    pub format_state: StreamFormatState,
    /// Last accepted SetVolume call; never projected across lease generations.
    pub volume_state: StreamVolumeState,
    /// Accepted Pause/FlushEx/Discontinuity observations for this generation.
    pub control_state: StreamControlState,
    /// Accepted host-observed spatial requests; playback and voice state remain elsewhere.
    pub spatial_state: StreamSpatialState,
    pub lease: Option<DeviceLeaseIdentity>,
    pub lifecycle: StreamLeaseLifecycle,
    /// Creation order for active entries, then detach order for detached entries.
    order: u64,
}

/// Rust-owned lease lifecycle index. The C adapter may keep an address-keyed payload
/// for mutable handler fields; lease identity, creation scope, and lifecycle membership live here.
#[derive(Debug, Default)]
pub struct StreamLeaseRegistry {
    records: Vec<StreamLeaseRecord>,
    next_order: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RollbackCleanupFailure {
    Refused,
    RetireInvariant,
}

impl StreamLeaseRegistry {
    pub const fn new() -> Self {
        Self {
            records: Vec::new(),
            next_order: 0,
        }
    }

    pub fn reserve(
        &mut self,
        stream_address: u32,
        stream_heap: u32,
        publication_address: u32,
        header: [u32; STREAM_HEADER_WORDS],
    ) -> bool {
        self.reserve_with_scope(
            stream_address,
            stream_heap,
            publication_address,
            header,
            None,
        )
    }

    pub fn reserve_scoped(
        &mut self,
        stream_address: u32,
        stream_heap: u32,
        publication_address: u32,
        header: [u32; STREAM_HEADER_WORDS],
        scope: StreamLeaseScope,
    ) -> bool {
        if !scope.is_valid() {
            return false;
        }
        self.reserve_with_scope(
            stream_address,
            stream_heap,
            publication_address,
            header,
            Some(scope),
        )
    }

    fn reserve_with_scope(
        &mut self,
        stream_address: u32,
        stream_heap: u32,
        publication_address: u32,
        header: [u32; STREAM_HEADER_WORDS],
        scope: Option<StreamLeaseScope>,
    ) -> bool {
        if stream_address == 0 || stream_heap == 0 || self.record(stream_address).is_some() {
            return false;
        }
        let Some(order) = self.next_order.checked_add(1) else {
            return false;
        };
        if self.records.try_reserve(1).is_err() {
            return false;
        }
        self.next_order = order;
        self.records.push(StreamLeaseRecord {
            stream_address,
            stream_heap,
            publication_address,
            header,
            scope,
            format_state: StreamFormatState::default(),
            volume_state: StreamVolumeState::default(),
            control_state: StreamControlState::default(),
            spatial_state: StreamSpatialState::default(),
            lease: None,
            lifecycle: StreamLeaseLifecycle::Pending,
            order,
        });
        true
    }

    pub fn bind(&mut self, stream_address: u32, lease: DeviceLeaseIdentity) -> bool {
        let Some(record) = self.record_mut(stream_address) else {
            return false;
        };
        if record.lifecycle != StreamLeaseLifecycle::Pending
            || record.header[3] != lease.internal_address
        {
            return false;
        }
        record.lease = Some(lease);
        record.lifecycle = StreamLeaseLifecycle::Active;
        true
    }

    pub fn cancel_pending(&mut self, stream_address: u32) -> bool {
        self.remove_if(stream_address, StreamLeaseLifecycle::Pending)
    }

    pub fn retain_rollback(&mut self, stream_address: u32) -> bool {
        let Some(record) = self.record_mut(stream_address) else {
            return false;
        };
        if record.lifecycle != StreamLeaseLifecycle::Pending {
            return false;
        }
        record.lifecycle = StreamLeaseLifecycle::Rollback;
        true
    }

    pub fn detach(&mut self, stream_address: u32, lease: DeviceLeaseIdentity) -> bool {
        if self.record(stream_address).is_none_or(|record| {
            record.lifecycle != StreamLeaseLifecycle::Active || record.lease != Some(lease)
        }) {
            return false;
        }
        let Some(order) = self.next_order.checked_add(1) else {
            return false;
        };
        self.next_order = order;
        let Some(record) = self.record_mut(stream_address) else {
            return false;
        };
        record.lifecycle = StreamLeaseLifecycle::Detached;
        record.order = order;
        true
    }

    pub fn begin_retire_detached(
        &mut self,
        stream_address: u32,
        lease: DeviceLeaseIdentity,
    ) -> bool {
        let Some(record) = self.record_mut(stream_address) else {
            return false;
        };
        if record.lifecycle != StreamLeaseLifecycle::Detached || record.lease != Some(lease) {
            return false;
        }
        record.lifecycle = StreamLeaseLifecycle::RetiringDetached;
        true
    }

    pub fn restore_detached(&mut self, stream_address: u32, lease: DeviceLeaseIdentity) -> bool {
        let Some(record) = self.record_mut(stream_address) else {
            return false;
        };
        if record.lifecycle != StreamLeaseLifecycle::RetiringDetached || record.lease != Some(lease)
        {
            return false;
        }
        record.lifecycle = StreamLeaseLifecycle::Detached;
        true
    }

    pub fn retire_detached(&mut self, stream_address: u32, lease: DeviceLeaseIdentity) -> bool {
        if self.record(stream_address).is_none_or(|record| {
            record.lifecycle != StreamLeaseLifecycle::RetiringDetached
                || record.lease != Some(lease)
        }) {
            return false;
        }
        self.remove_if(stream_address, StreamLeaseLifecycle::RetiringDetached)
    }

    pub fn begin_retire_rollback(&mut self, stream_address: u32) -> bool {
        let Some(record) = self.record_mut(stream_address) else {
            return false;
        };
        if record.lifecycle != StreamLeaseLifecycle::Rollback {
            return false;
        }
        record.lifecycle = StreamLeaseLifecycle::RetiringRollback;
        true
    }

    pub fn restore_rollback(&mut self, stream_address: u32) -> bool {
        let Some(record) = self.record_mut(stream_address) else {
            return false;
        };
        if record.lifecycle != StreamLeaseLifecycle::RetiringRollback {
            return false;
        }
        record.lifecycle = StreamLeaseLifecycle::Rollback;
        true
    }

    pub fn retire_rollback(&mut self, stream_address: u32) -> bool {
        self.remove_if(stream_address, StreamLeaseLifecycle::RetiringRollback)
    }

    pub fn owns_live_lease(&self, stream_address: u32, lease: DeviceLeaseIdentity) -> bool {
        self.record(stream_address).is_some_and(|record| {
            matches!(
                record.lifecycle,
                StreamLeaseLifecycle::Active | StreamLeaseLifecycle::Detached
            ) && record.lease == Some(lease)
        })
    }

    pub fn owns_live_stream(&self, stream: &StreamLeaseHeader) -> bool {
        self.record(stream.stream_address).is_some_and(|record| {
            matches!(
                record.lifecycle,
                StreamLeaseLifecycle::Active | StreamLeaseLifecycle::Detached
            ) && record.lease == Some(stream.lease)
                && record.stream_heap == stream.stream_heap
                && record.header == stream.header
        })
    }

    /// Rebuild the validation view from the authoritative Rust record. Host adapters must not
    /// feed their keyed payload mirror back as the expected lease/header identity.
    pub fn validation_header(&self, stream_address: u32) -> Option<StreamLeaseHeader> {
        let record = self.record(stream_address)?;
        Some(StreamLeaseHeader {
            lease: record.lease?,
            stream_address: record.stream_address,
            stream_heap: record.stream_heap,
            header: record.header,
        })
    }

    pub fn lifecycle(&self, stream_address: u32) -> Option<StreamLeaseLifecycle> {
        self.record(stream_address).map(|record| record.lifecycle)
    }

    /// Return the generation identity stored for a live active or detached lease.
    pub fn live_identity(&self, stream_address: u32) -> Option<DeviceLeaseIdentity> {
        let record = self.record(stream_address)?;
        matches!(
            record.lifecycle,
            StreamLeaseLifecycle::Active | StreamLeaseLifecycle::Detached
        )
        .then_some(record.lease?)
    }

    /// Active nodes historically reset newest first; lifecycle ownership is the
    /// authoritative ordering source for the Rust-host adapter.
    pub fn first_active(&self) -> Option<u32> {
        self.records
            .iter()
            .filter(|record| record.lifecycle == StreamLeaseLifecycle::Active)
            .max_by_key(|record| record.order)
            .map(|record| record.stream_address)
    }

    /// Detached nodes historically clean up in the order they appear at the
    /// C list head; each detach inserts at the head, so the newest detach wins.
    pub fn first_detached(&self) -> Option<u32> {
        self.records
            .iter()
            .filter(|record| record.lifecycle == StreamLeaseLifecycle::Detached)
            .max_by_key(|record| record.order)
            .map(|record| record.stream_address)
    }

    pub fn is_active(&self, stream_address: u32) -> bool {
        self.lifecycle(stream_address) == Some(StreamLeaseLifecycle::Active)
    }

    /// Copy the active stream's scope only for its exact bound lease generation.
    pub fn scope(
        &self,
        stream_address: u32,
        lease: DeviceLeaseIdentity,
    ) -> Option<StreamLeaseScope> {
        self.record(stream_address)
            .filter(|record| {
                record.lifecycle == StreamLeaseLifecycle::Active && record.lease == Some(lease)
            })?
            .scope
    }

    /// Copy the mutable SetFormat state for the exact active lease generation.
    pub fn format_state(
        &self,
        stream_address: u32,
        lease: DeviceLeaseIdentity,
    ) -> Option<StreamFormatState> {
        let record = self.record(stream_address).filter(|record| {
            record.lifecycle == StreamLeaseLifecycle::Active && record.lease == Some(lease)
        })?;
        Some(record.format_state)
    }

    /// Commit SetFormat bytes only if the caller's snapshot is still current.
    /// The existing stream lock serializes this operation with C adapter handlers.
    pub fn commit_format_state(
        &mut self,
        stream_address: u32,
        lease: DeviceLeaseIdentity,
        expected: StreamFormatState,
        bytes: [u8; FORMAT_BYTES],
    ) -> bool {
        let Some(record) = self.record_mut(stream_address) else {
            return false;
        };
        if record.lifecycle != StreamLeaseLifecycle::Active
            || record.lease != Some(lease)
            || record.format_state != expected
        {
            return false;
        }
        record.format_state.bytes = bytes;
        record.format_state.sets = record.format_state.sets.wrapping_add(1);
        true
    }

    /// Copy the accepted SetVolume projection for the exact active generation.
    pub fn volume_state(
        &self,
        stream_address: u32,
        lease: DeviceLeaseIdentity,
    ) -> Option<StreamVolumeState> {
        let record = self.record(stream_address).filter(|record| {
            record.lifecycle == StreamLeaseLifecycle::Active && record.lease == Some(lease)
        })?;
        Some(record.volume_state)
    }

    /// Compare and commit an accepted SetVolume projection for the exact active generation.
    pub fn commit_volume_state(
        &mut self,
        stream_address: u32,
        lease: DeviceLeaseIdentity,
        expected: StreamVolumeState,
        next: StreamVolumeState,
    ) -> bool {
        let Some(record) = self.record_mut(stream_address) else {
            return false;
        };
        if record.lifecycle != StreamLeaseLifecycle::Active
            || record.lease != Some(lease)
            || record.volume_state != expected
            || next.seen != 1
        {
            return false;
        }
        record.volume_state = next;
        true
    }

    /// Plan the measured SetVolume startup/update policy without mutating lease state.
    /// The caller commits the returned projection only after its routing transaction succeeds.
    pub fn plan_volume_request(
        &self,
        stream_address: u32,
        lease: DeviceLeaseIdentity,
        request: StreamVolumeRequest,
    ) -> Result<StreamVolumeState, u32> {
        if request.completion_available > 1 {
            return Err(STREAM_VOLUME_BAD_REQUEST);
        }
        let Some(record) = self.record(stream_address) else {
            return Err(STREAM_VOLUME_STALE_LEASE);
        };
        if record.lifecycle != StreamLeaseLifecycle::Active || record.lease != Some(lease) {
            return Err(STREAM_VOLUME_STALE_LEASE);
        }
        let Some(scope) = record.scope else {
            return Err(STREAM_VOLUME_UNSUPPORTED_SCOPE);
        };
        let startup_cache_mask = record.spatial_state.startup_cache_mask;
        let spatial = scope.flags == 0x10 && startup_cache_mask == 7;
        let stereo = scope.flags == 0
            && startup_cache_mask == 0
            && scope.max_packets == 3
            && scope.sample_rate == 44_100
            && scope.channels == 2
            && scope.block_align == 72
            && scope.average_bytes_per_second == 49_612;
        let control = record.control_state;
        let fresh =
            control.pause_seen == 0 && control.flush_seen == 0 && control.discontinuity_seen == 0;
        let paused = stereo
            && control.pause_seen != 0
            && control.pause_mode == 1
            && control.flush_seen != 0
            && control.flush_time_low == 0
            && control.flush_time_high == 0
            && control.flush_flags == 1
            && control.discontinuity_seen != 0;
        let update = request.completion_available == 1
            && (spatial || stereo)
            && control.discontinuity_seen != 0
            && record.volume_state.seen != 0;
        if !spatial && !stereo {
            return Err(STREAM_VOLUME_UNSUPPORTED_SCOPE);
        }
        if !update && ((!fresh && !paused) || request.argument != 0xFFFF_D8F0) {
            return Err(STREAM_VOLUME_POLICY_REFUSED);
        }
        if request.count == 1 && record.format_state.sets == 0 {
            return Err(STREAM_VOLUME_NEEDS_FORMAT);
        }
        if record.volume_state.seen > 1 {
            return Err(STREAM_VOLUME_BAD_STATE);
        }
        Ok(StreamVolumeState {
            seen: 1,
            value: if update {
                request.argument as i32
            } else {
                -10_000
            },
        })
    }

    /// Copy accepted stream control observations for the exact active generation.
    pub fn control_state(
        &self,
        stream_address: u32,
        lease: DeviceLeaseIdentity,
    ) -> Option<StreamControlState> {
        let record = self.record(stream_address).filter(|record| {
            record.lifecycle == StreamLeaseLifecycle::Active && record.lease == Some(lease)
        })?;
        Some(record.control_state)
    }

    /// Compare and commit one accepted control observation for the exact lease.
    /// HLE admission remains in the C adapter; Rust owns the committed state.
    pub fn commit_control_state(
        &mut self,
        stream_address: u32,
        lease: DeviceLeaseIdentity,
        expected: StreamControlState,
        next: StreamControlState,
    ) -> bool {
        let Some(record) = self.record_mut(stream_address) else {
            return false;
        };
        if record.lifecycle != StreamLeaseLifecycle::Active
            || record.lease != Some(lease)
            || record.control_state != expected
            || next.revision != expected.revision.wrapping_add(1)
            || !valid_control_state_transition(expected, next)
        {
            return false;
        }
        record.control_state = next;
        true
    }

    /// Apply the measured startup/control policy and record it atomically for one
    /// active lease. The C adapter still owns HLE dispatch and the Pause callback.
    pub fn apply_control_request(
        &mut self,
        stream_address: u32,
        lease: DeviceLeaseIdentity,
        request: StreamControlRequest,
    ) -> u32 {
        if request.completion_available > 1 || request.pause_callback_available > 1 {
            return STREAM_CONTROL_INVALID_REQUEST;
        }
        let Some(record) = self.record_mut(stream_address) else {
            return STREAM_CONTROL_STALE_LEASE;
        };
        if record.lifecycle != StreamLeaseLifecycle::Active || record.lease != Some(lease) {
            return STREAM_CONTROL_STALE_LEASE;
        }
        let Some(scope) = record.scope else {
            return STREAM_CONTROL_UNSUPPORTED_SCOPE;
        };
        let current = record.control_state;
        let next = match next_stream_control_state(
            scope,
            record.spatial_state.startup_cache_mask,
            record.format_state.sets,
            current,
            request,
        ) {
            Ok(next) => next,
            Err(refusal) => return refusal,
        };
        if !valid_control_state_transition(current, next) {
            return STREAM_CONTROL_STATE_REJECTED;
        }
        record.control_state = next;
        STREAM_CONTROL_ACCEPTED
    }

    /// Copy accepted spatial observations for the exact active generation.
    pub fn spatial_state(
        &self,
        stream_address: u32,
        lease: DeviceLeaseIdentity,
    ) -> Option<StreamSpatialState> {
        let record = self.record(stream_address).filter(|record| {
            record.lifecycle == StreamLeaseLifecycle::Active && record.lease == Some(lease)
        })?;
        Some(record.spatial_state)
    }

    /// Record the measured apply=1 SetMaxDistance observation for one configured lease.
    pub fn record_max_distance(
        &mut self,
        stream_address: u32,
        lease: DeviceLeaseIdentity,
        apply: u32,
        distance_bits: u32,
    ) -> bool {
        if apply != 1
            || self
                .scope(stream_address, lease)
                .is_none_or(|scope| scope.flags != 0x10)
        {
            return false;
        }
        let Some(expected) = self.spatial_state(stream_address, lease) else {
            return false;
        };
        if expected.startup_cache_mask != 7 {
            return false;
        }
        let next = StreamSpatialState {
            max_distance_seen: 1,
            max_distance_bits: distance_bits,
            revision: expected.revision.wrapping_add(1),
            ..expected
        };
        self.commit_spatial_state(stream_address, lease, expected, next)
    }

    /// Apply one measured startup spatial setter to the exact configured lease.
    /// The caller has already performed the required guest read for I3DL2.
    pub fn apply_startup_spatial(
        &mut self,
        stream_address: u32,
        lease: DeviceLeaseIdentity,
        action: u32,
        argument: u32,
        count: u32,
        i3dl2_words: [u32; 9],
    ) -> u32 {
        let Some(scope) = self.scope(stream_address, lease) else {
            return STREAM_SPATIAL_STALE_LEASE;
        };
        if scope.flags != 0x10 {
            return STREAM_SPATIAL_UNSUPPORTED_SCOPE;
        }
        let Some(expected) = self.spatial_state(stream_address, lease) else {
            return STREAM_SPATIAL_STALE_LEASE;
        };
        let mut next = expected;
        match action {
            1 if expected.startup_cache_mask == 0
                && count == 0
                && i3dl2_words == [0, 0, 0xffff_f448, 0, 0, 0, 0, 0, 0] =>
            {
                next.i3dl2_address = argument;
                next.i3dl2_words = i3dl2_words;
                next.startup_cache_mask = 1;
            }
            2 if expected.startup_cache_mask == 1 && argument == 0x3f80_0000 && count == 0 => {
                next.min_distance_bits = argument;
                next.startup_cache_mask = 3;
            }
            3 if expected.startup_cache_mask == 3 && argument == 0x004b_914c && count == 4 => {
                next.curve_address = argument;
                next.curve_count = count;
                next.startup_cache_mask = 7;
            }
            _ => return STREAM_SPATIAL_BAD_SEQUENCE,
        }
        next.revision = expected.revision.wrapping_add(1);
        if self.commit_spatial_state(stream_address, lease, expected, next) {
            STREAM_SPATIAL_ACCEPTED
        } else {
            STREAM_SPATIAL_BAD_REQUEST
        }
    }

    /// Compare and commit one accepted spatial observation for the exact lease.
    pub fn commit_spatial_state(
        &mut self,
        stream_address: u32,
        lease: DeviceLeaseIdentity,
        expected: StreamSpatialState,
        next: StreamSpatialState,
    ) -> bool {
        let Some(record) = self.record_mut(stream_address) else {
            return false;
        };
        if record.lifecycle != StreamLeaseLifecycle::Active
            || record.lease != Some(lease)
            || record.spatial_state != expected
            || next.revision != expected.revision.wrapping_add(1)
            || !valid_spatial_cache_transition(expected, next)
            || (next.max_distance_seen != 0 && next.max_distance_seen != 1)
            || (next.position_sets != expected.position_sets
                && next.position_sets != expected.position_sets.wrapping_add(1))
        {
            return false;
        }
        record.spatial_state = next;
        true
    }

    /// Return one retained failed-create allocation for the serialized host cleanup loop.
    pub fn first_rollback(&self) -> Option<(u32, u32)> {
        self.records
            .iter()
            .rev()
            .find(|record| record.lifecycle == StreamLeaseLifecycle::Rollback)
            .map(|record| (record.stream_address, record.stream_heap))
    }

    /// Release retained failed-create heaps in registry order, restoring the record if destroy refuses.
    pub fn cleanup_rollback_records(
        &mut self,
        mut heap_valid: impl FnMut(u32) -> bool,
        mut heap_destroy: impl FnMut(u32) -> bool,
    ) -> Result<(), RollbackCleanupFailure> {
        while let Some((address, heap)) = self.first_rollback() {
            if address == 0
                || heap == 0
                || !heap_valid(heap)
                || !self.begin_retire_rollback(address)
            {
                return Err(RollbackCleanupFailure::Refused);
            }
            if !heap_destroy(heap) {
                let _ = self.restore_rollback(address);
                return Err(RollbackCleanupFailure::Refused);
            }
            if !self.retire_rollback(address) {
                return Err(RollbackCleanupFailure::RetireInvariant);
            }
        }
        Ok(())
    }

    /// Test overlap with dynamic guest spans owned by live, detached, or rollback leases.
    pub fn overlaps_guest_span(&self, address: u32, bytes: u32) -> bool {
        if bytes == 0 {
            return false;
        }
        let overlaps = |start: u32, length: u32| {
            u64::from(address) < u64::from(start) + u64::from(length)
                && u64::from(start) < u64::from(address) + u64::from(bytes)
        };
        self.records.iter().any(|record| match record.lifecycle {
            StreamLeaseLifecycle::Active => {
                overlaps(record.stream_address, STREAM_ALLOCATION_BYTES)
                    || overlaps(record.publication_address, 4)
            }
            StreamLeaseLifecycle::Detached | StreamLeaseLifecycle::Rollback => {
                overlaps(record.stream_address, STREAM_ALLOCATION_BYTES)
            }
            StreamLeaseLifecycle::Pending
            | StreamLeaseLifecycle::RetiringDetached
            | StreamLeaseLifecycle::RetiringRollback => false,
        })
    }

    /// Test overlap with an active stream's retained I3DL2 guest input.
    pub fn overlaps_i3dl2_span(&self, address: u32, bytes: u32) -> bool {
        if bytes == 0 {
            return false;
        }
        let overlaps = |start: u32| {
            start != 0
                && u64::from(address) < u64::from(start) + 36
                && u64::from(start) < u64::from(address) + u64::from(bytes)
        };
        self.records.iter().any(|record| {
            record.lifecycle == StreamLeaseLifecycle::Active
                && overlaps(record.spatial_state.i3dl2_address)
        })
    }

    pub fn guest_span_count(&self) -> Option<usize> {
        self.records.iter().try_fold(0usize, |count, record| {
            let spans = match record.lifecycle {
                // Create snapshots descriptor/format bytes; their caller-owned addresses are
                // not protected guest object state once the lease is active.
                StreamLeaseLifecycle::Active => 2,
                StreamLeaseLifecycle::Detached | StreamLeaseLifecycle::Rollback => 1,
                StreamLeaseLifecycle::Pending
                | StreamLeaseLifecycle::RetiringDetached
                | StreamLeaseLifecycle::RetiringRollback => 0,
            };
            count.checked_add(spans)
        })
    }

    /// Append guest spans owned by active, detached, and failed-cleanup streams.
    /// Pending and retiring entries are excluded because the C adapter serializes
    /// this snapshot under its stream lock, outside create/reset transactions.
    pub fn append_guest_spans(&self, output: &mut [StreamLeaseGuestSpan]) -> Option<usize> {
        let count = self.guest_span_count()?;
        if output.len() < count {
            return None;
        }

        let mut index = 0;
        for record in &self.records {
            match record.lifecycle {
                StreamLeaseLifecycle::Active => {
                    output[index] = StreamLeaseGuestSpan {
                        address: record.stream_address,
                        bytes: STREAM_ALLOCATION_BYTES,
                    };
                    output[index + 1] = StreamLeaseGuestSpan {
                        address: record.publication_address,
                        bytes: 4,
                    };
                    index += 2;
                }
                StreamLeaseLifecycle::Detached | StreamLeaseLifecycle::Rollback => {
                    output[index] = StreamLeaseGuestSpan {
                        address: record.stream_address,
                        bytes: STREAM_ALLOCATION_BYTES,
                    };
                    index += 1;
                }
                StreamLeaseLifecycle::Pending
                | StreamLeaseLifecycle::RetiringDetached
                | StreamLeaseLifecycle::RetiringRollback => {}
            }
        }
        Some(index)
    }

    pub fn record(&self, stream_address: u32) -> Option<&StreamLeaseRecord> {
        self.records
            .iter()
            .find(|record| record.stream_address == stream_address)
    }

    fn record_mut(&mut self, stream_address: u32) -> Option<&mut StreamLeaseRecord> {
        self.records
            .iter_mut()
            .find(|record| record.stream_address == stream_address)
    }

    fn remove_if(&mut self, stream_address: u32, lifecycle: StreamLeaseLifecycle) -> bool {
        let Some(index) = self.records.iter().position(|record| {
            record.stream_address == stream_address && record.lifecycle == lifecycle
        }) else {
            return false;
        };
        self.records.swap_remove(index);
        true
    }
}

fn next_stream_control_state(
    scope: StreamLeaseScope,
    startup_cache_mask: u32,
    format_sets: u32,
    current: StreamControlState,
    request: StreamControlRequest,
) -> Result<StreamControlState, u32> {
    let stereo = scope.flags == 0 && startup_cache_mask == 0;
    let spatial = scope.flags == 0x10 && startup_cache_mask == 7;
    if !stereo && !spatial {
        return Err(STREAM_CONTROL_UNSUPPORTED_SCOPE);
    }

    let mut next = current;
    next.revision = current.revision.wrapping_add(1);
    match request.action {
        STREAM_CONTROL_DISCONTINUITY => {
            if current.pause_seen != 1
                || current.pause_mode > 1
                || current.flush_seen != 1
                || current.flush_time_low != 0
                || current.flush_time_high != 0
                || current.flush_flags != 1
            {
                return Err(STREAM_CONTROL_BAD_SEQUENCE);
            }
            next.discontinuity_seen = 1;
        }
        STREAM_CONTROL_FLUSH_EX => {
            if current.pause_seen != 1
                || current.pause_mode != 1
                || request.argument != 0
                || request.count != 0
                || request.apply != 1
            {
                return Err(STREAM_CONTROL_BAD_FLUSH);
            }
            next.flush_seen = 1;
            next.flush_time_low = request.argument;
            next.flush_time_high = request.count;
            next.flush_flags = request.apply;
        }
        STREAM_CONTROL_PAUSE => {
            let completion = request.completion_available == 1;
            let pause_callback = request.pause_callback_available == 1;
            if request.count == 2
                && (!completion
                    || !pause_callback
                    || current.discontinuity_seen == 0
                    || request.argument != 1)
            {
                return Err(STREAM_CONTROL_BAD_RUNTIME_PAUSE);
            }
            let resume_ok = completion
                && current.discontinuity_seen != 0
                && (request.argument == 0 || request.argument == 1);
            if request.argument != 1 && !resume_ok {
                return Err(STREAM_CONTROL_BAD_PAUSE);
            }
            if request.count == 1 && format_sets == 0 {
                return Err(STREAM_CONTROL_PAUSE_NEEDS_FORMAT);
            }
            next.pause_seen = 1;
            next.pause_mode = request.argument;
        }
        _ => return Err(STREAM_CONTROL_INVALID_REQUEST),
    }
    Ok(next)
}

fn valid_control_state_transition(expected: StreamControlState, next: StreamControlState) -> bool {
    if expected.pause_seen > 1
        || expected.flush_seen > 1
        || expected.discontinuity_seen > 1
        || next.pause_seen > 1
        || next.flush_seen > 1
        || next.discontinuity_seen > 1
        || expected.pause_seen > next.pause_seen
        || expected.flush_seen > next.flush_seen
        || expected.discontinuity_seen > next.discontinuity_seen
        || (expected.flush_seen != 0
            && (expected.flush_time_low != next.flush_time_low
                || expected.flush_time_high != next.flush_time_high
                || expected.flush_flags != next.flush_flags))
        || (next.pause_seen == 0 && next.pause_mode != 0)
        || (next.pause_seen != 0 && next.pause_mode > 1)
        || (next.flush_seen == 0
            && (next.flush_time_low != 0 || next.flush_time_high != 0 || next.flush_flags != 0))
        || (next.flush_seen != 0
            && (next.flush_time_low != 0 || next.flush_time_high != 0 || next.flush_flags != 1))
    {
        return false;
    }

    let pause_changed =
        expected.pause_seen != next.pause_seen || expected.pause_mode != next.pause_mode;
    let flush_changed = expected.flush_seen != next.flush_seen
        || expected.flush_time_low != next.flush_time_low
        || expected.flush_time_high != next.flush_time_high
        || expected.flush_flags != next.flush_flags;
    let discontinuity_changed = expected.discontinuity_seen != next.discontinuity_seen;
    u8::from(pause_changed) + u8::from(flush_changed) + u8::from(discontinuity_changed) <= 1
        && (next.pause_seen != 0 || next.flush_seen == 0 && next.discontinuity_seen == 0)
        && (!flush_changed || next.flush_seen == 0 || next.pause_seen != 0 && next.pause_mode == 1)
        && (next.discontinuity_seen == 0
            || next.pause_seen != 0 && next.flush_seen != 0 && next.flush_flags == 1)
}

fn valid_spatial_cache_transition(expected: StreamSpatialState, next: StreamSpatialState) -> bool {
    let transition = match (expected.startup_cache_mask, next.startup_cache_mask) {
        (0, 0) | (1, 1) | (3, 3) | (7, 7) => true,
        (0, 1) => {
            next.i3dl2_address != 0 && next.i3dl2_words == [0, 0, 0xffff_f448, 0, 0, 0, 0, 0, 0]
        }
        (1, 3) => next.min_distance_bits == 0x3f80_0000,
        (3, 7) => next.curve_address == 0x004b_914c && next.curve_count == 4,
        _ => false,
    };
    if !transition {
        return false;
    }
    if expected.startup_cache_mask >= 1
        && next.startup_cache_mask >= 1
        && (next.i3dl2_address != expected.i3dl2_address
            || next.i3dl2_words != expected.i3dl2_words)
    {
        return false;
    }
    if expected.startup_cache_mask >= 3
        && next.startup_cache_mask >= 3
        && next.min_distance_bits != expected.min_distance_bits
    {
        return false;
    }
    true
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamLeaseGuestSpan {
    pub address: u32,
    pub bytes: u32,
}

/// Host-only operations that cannot be represented as portable stream state.
pub trait StreamLeaseProbes {
    fn device_heap_valid(&mut self, heap: u32) -> bool;
    fn read_header(&mut self, address: u32) -> Option<[u32; STREAM_HEADER_WORDS]>;
    fn stream_heap_valid(&mut self, heap: u32) -> bool;
    fn stream_block_size(&mut self, heap: u32, address: u32) -> Option<u32>;
}

/// Validate one owned stream against the caller's lease while preserving the C adapter's
/// measured check order. Host probes run only after the earlier typed checks pass.
pub fn validate_stream_lease<P: StreamLeaseProbes>(
    stream: Option<&StreamLeaseHeader>,
    identity: &DeviceLeaseIdentity,
    required_internal_address: u32,
    probes: &mut P,
) -> bool {
    let Some(stream) = stream else {
        return false;
    };
    if stream.lease != *identity || identity.internal_address != required_internal_address {
        return false;
    }
    if !probes.device_heap_valid(identity.device_heap) {
        return false;
    }
    let Some(actual_header) = probes.read_header(stream.stream_address) else {
        return false;
    };
    if actual_header != stream.header {
        return false;
    }
    if !probes.stream_heap_valid(stream.stream_heap) {
        return false;
    }
    probes.stream_block_size(stream.stream_heap, stream.stream_address)
        == Some(STREAM_ALLOCATION_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Probes {
        calls: Vec<&'static str>,
        header: Option<[u32; STREAM_HEADER_WORDS]>,
        device_heap: bool,
        stream_heap: bool,
        block_size: Option<u32>,
    }

    impl StreamLeaseProbes for Probes {
        fn device_heap_valid(&mut self, _heap: u32) -> bool {
            self.calls.push("device-heap");
            self.device_heap
        }

        fn read_header(&mut self, _address: u32) -> Option<[u32; STREAM_HEADER_WORDS]> {
            self.calls.push("guest-header");
            self.header
        }

        fn stream_heap_valid(&mut self, _heap: u32) -> bool {
            self.calls.push("stream-heap");
            self.stream_heap
        }

        fn stream_block_size(&mut self, _heap: u32, _address: u32) -> Option<u32> {
            self.calls.push("block-size");
            self.block_size
        }
    }

    fn state() -> (StreamLeaseHeader, DeviceLeaseIdentity) {
        let lease = DeviceLeaseIdentity {
            device_heap: 7,
            internal_address: 0x1234,
            serial: 0x1_0000_0002,
        };
        (
            StreamLeaseHeader {
                lease,
                stream_address: 0x2000,
                stream_heap: 8,
                header: [1, 2, 3, 0x1234, 0, 0, 0, 0, 0, 0],
            },
            lease,
        )
    }

    const PUBLICATION_ADDRESS: u32 = 0x3000;

    fn valid_probes(header: [u32; STREAM_HEADER_WORDS]) -> Probes {
        Probes {
            header: Some(header),
            device_heap: true,
            stream_heap: true,
            block_size: Some(STREAM_ALLOCATION_BYTES),
            ..Probes::default()
        }
    }

    fn valid_scope() -> StreamLeaseScope {
        let mut descriptor = [0; DESCRIPTOR_BYTES];
        descriptor[4..8].copy_from_slice(&3u32.to_le_bytes());
        descriptor[8..12].copy_from_slice(&0x4000u32.to_le_bytes());
        let mut format = [0; FORMAT_BYTES];
        format[0..2].copy_from_slice(&0x69u16.to_le_bytes());
        format[2..4].copy_from_slice(&2u16.to_le_bytes());
        format[4..8].copy_from_slice(&44_100u32.to_le_bytes());
        format[8..12].copy_from_slice(&49_612u32.to_le_bytes());
        format[12..14].copy_from_slice(&72u16.to_le_bytes());
        format[14..16].copy_from_slice(&4u16.to_le_bytes());
        format[16..18].copy_from_slice(&2u16.to_le_bytes());
        format[18..20].copy_from_slice(&64u16.to_le_bytes());
        StreamLeaseScope {
            descriptor_address: 0x5000,
            format_address: 0x4000,
            flags: 0,
            max_packets: 3,
            sample_rate: 44_100,
            average_bytes_per_second: 49_612,
            channels: 2,
            block_align: 72,
            descriptor,
            format,
        }
    }

    #[test]
    fn validates_token_header_heap_and_exact_block_size_in_order() {
        let (stream, identity) = state();
        let mut probes = valid_probes(stream.header);
        assert!(validate_stream_lease(
            Some(&stream),
            &identity,
            identity.internal_address,
            &mut probes
        ));
        assert_eq!(
            probes.calls,
            ["device-heap", "guest-header", "stream-heap", "block-size"]
        );
    }

    #[test]
    fn stale_identity_and_wrong_internal_address_stop_before_host_probes() {
        let (stream, identity) = state();
        let mut stale = identity;
        stale.serial += 1;
        let mut probes = valid_probes(stream.header);
        assert!(!validate_stream_lease(
            Some(&stream),
            &stale,
            stale.internal_address,
            &mut probes
        ));
        assert!(probes.calls.is_empty());

        assert!(!validate_stream_lease(
            Some(&stream),
            &identity,
            identity.internal_address + 1,
            &mut probes
        ));
        assert!(probes.calls.is_empty());
        assert!(!validate_stream_lease(
            None,
            &identity,
            identity.internal_address,
            &mut probes
        ));
    }

    #[test]
    fn host_probe_refusals_short_circuit_at_the_original_boundary() {
        let (stream, identity) = state();
        let mut probes = valid_probes(stream.header);
        probes.device_heap = false;
        assert!(!validate_stream_lease(
            Some(&stream),
            &identity,
            identity.internal_address,
            &mut probes
        ));
        assert_eq!(probes.calls, ["device-heap"]);

        let mut probes = valid_probes(stream.header);
        let mut changed = stream.header;
        changed[9] = 1;
        probes.header = Some(changed);
        assert!(!validate_stream_lease(
            Some(&stream),
            &identity,
            identity.internal_address,
            &mut probes
        ));
        assert_eq!(probes.calls, ["device-heap", "guest-header"]);

        let mut probes = valid_probes(stream.header);
        probes.stream_heap = false;
        assert!(!validate_stream_lease(
            Some(&stream),
            &identity,
            identity.internal_address,
            &mut probes
        ));
        assert_eq!(probes.calls, ["device-heap", "guest-header", "stream-heap"]);

        let mut probes = valid_probes(stream.header);
        probes.block_size = Some(STREAM_ALLOCATION_BYTES - 4);
        assert!(!validate_stream_lease(
            Some(&stream),
            &identity,
            identity.internal_address,
            &mut probes
        ));
        assert_eq!(probes.calls.len(), 4);
    }

    #[test]
    fn registry_owns_pending_active_detached_and_retired_transitions() {
        let (stream, identity) = state();
        let mut registry = StreamLeaseRegistry::new();
        assert!(registry.reserve(
            stream.stream_address,
            stream.stream_heap,
            PUBLICATION_ADDRESS,
            stream.header
        ));
        assert_eq!(
            registry.lifecycle(stream.stream_address),
            Some(StreamLeaseLifecycle::Pending)
        );
        assert!(!registry.reserve(
            stream.stream_address,
            stream.stream_heap,
            PUBLICATION_ADDRESS,
            stream.header
        ));
        assert!(!registry.bind(
            stream.stream_address,
            DeviceLeaseIdentity {
                internal_address: 1,
                ..identity
            }
        ));
        assert!(registry.bind(stream.stream_address, identity));
        assert!(registry.owns_live_lease(stream.stream_address, identity));

        let mut stale = identity;
        stale.serial += 1;
        assert!(!registry.detach(stream.stream_address, stale));
        assert!(registry.detach(stream.stream_address, identity));
        assert_eq!(
            registry.lifecycle(stream.stream_address),
            Some(StreamLeaseLifecycle::Detached)
        );
        assert!(registry.owns_live_lease(stream.stream_address, identity));
        assert!(!registry.begin_retire_detached(stream.stream_address, stale));
        assert!(registry.begin_retire_detached(stream.stream_address, identity));
        assert!(!registry.retire_detached(stream.stream_address, stale));
        assert!(registry.retire_detached(stream.stream_address, identity));
        assert_eq!(registry.lifecycle(stream.stream_address), None);
    }

    #[test]
    fn registry_keeps_failed_private_cleanup_separate_from_committed_leases() {
        let (stream, _) = state();
        let mut registry = StreamLeaseRegistry::new();
        assert!(registry.reserve(
            stream.stream_address,
            stream.stream_heap,
            PUBLICATION_ADDRESS,
            stream.header
        ));
        assert!(registry.retain_rollback(stream.stream_address));
        assert_eq!(
            registry.lifecycle(stream.stream_address),
            Some(StreamLeaseLifecycle::Rollback)
        );
        assert!(!registry.owns_live_lease(stream.stream_address, stream.lease));
        assert!(!registry.cancel_pending(stream.stream_address));
        assert!(registry.begin_retire_rollback(stream.stream_address));
        assert!(registry.restore_rollback(stream.stream_address));
        assert!(registry.begin_retire_rollback(stream.stream_address));
        assert!(registry.retire_rollback(stream.stream_address));
        assert!(!registry.retire_rollback(stream.stream_address));
    }

    #[test]
    fn protected_spans_follow_lifecycle_and_keep_publication_only_while_active() {
        let (stream, identity) = state();
        let mut registry = StreamLeaseRegistry::new();
        assert!(registry.reserve(
            stream.stream_address,
            stream.stream_heap,
            PUBLICATION_ADDRESS,
            stream.header
        ));
        let mut spans = [StreamLeaseGuestSpan {
            address: 0,
            bytes: 0,
        }; 2];
        assert_eq!(registry.guest_span_count(), Some(0));
        assert_eq!(registry.append_guest_spans(&mut spans), Some(0));
        assert!(registry.bind(stream.stream_address, identity));
        assert_eq!(registry.guest_span_count(), Some(2));
        assert_eq!(registry.append_guest_spans(&mut spans), Some(2));
        assert_eq!(
            spans[0],
            StreamLeaseGuestSpan {
                address: stream.stream_address,
                bytes: STREAM_ALLOCATION_BYTES,
            }
        );
        assert_eq!(
            spans[1],
            StreamLeaseGuestSpan {
                address: PUBLICATION_ADDRESS,
                bytes: 4,
            }
        );
        assert!(registry.detach(stream.stream_address, identity));
        assert_eq!(registry.guest_span_count(), Some(1));
        assert_eq!(registry.append_guest_spans(&mut spans), Some(1));
        assert_eq!(spans[0].address, stream.stream_address);
        assert!(registry.begin_retire_detached(stream.stream_address, identity));
        assert_eq!(registry.guest_span_count(), Some(0));
    }

    #[test]
    fn scoped_lease_keeps_creation_snapshots_but_protects_only_owned_guest_spans() {
        let (stream, identity) = state();
        let scope = valid_scope();
        assert!(scope.is_valid());
        assert_eq!(core::mem::size_of::<StreamLeaseScope>(), 72);
        let mut registry = StreamLeaseRegistry::new();
        let mut inconsistent_scope = scope;
        inconsistent_scope.sample_rate = 48_000;
        assert!(!registry.reserve_scoped(
            stream.stream_address,
            stream.stream_heap,
            PUBLICATION_ADDRESS,
            stream.header,
            inconsistent_scope,
        ));
        assert_eq!(registry.lifecycle(stream.stream_address), None);
        assert!(registry.reserve_scoped(
            stream.stream_address,
            stream.stream_heap,
            PUBLICATION_ADDRESS,
            stream.header,
            scope,
        ));
        let mut spans = [StreamLeaseGuestSpan::default(); 4];
        assert!(registry.scope(stream.stream_address, identity).is_none());
        assert!(registry.bind(stream.stream_address, identity));
        assert_eq!(registry.scope(stream.stream_address, identity), Some(scope));
        assert_eq!(registry.guest_span_count(), Some(2));
        assert_eq!(registry.append_guest_spans(&mut spans), Some(2));
        assert_eq!(spans[0].address, stream.stream_address);
        assert_eq!(spans[1].address, PUBLICATION_ADDRESS);
        assert!(!registry.overlaps_guest_span(scope.descriptor_address + 8, 4));
        assert!(!registry.overlaps_guest_span(scope.format_address + 4, 4));
        assert!(registry.overlaps_guest_span(stream.stream_address + 8, 4));
        assert!(registry.overlaps_guest_span(PUBLICATION_ADDRESS, 4));
        let mut stale = identity;
        stale.serial += 1;
        assert!(registry.scope(stream.stream_address, stale).is_none());
        assert!(registry.detach(stream.stream_address, identity));
        assert_eq!(registry.guest_span_count(), Some(1));
        assert!(!registry.overlaps_guest_span(scope.descriptor_address, 4));
        assert!(registry.scope(stream.stream_address, identity).is_none());
    }

    #[test]
    fn control_observations_follow_exact_generation_and_compare_commit() {
        let (stream, identity) = state();
        let mut registry = StreamLeaseRegistry::new();
        assert!(registry.reserve(
            stream.stream_address,
            stream.stream_heap,
            PUBLICATION_ADDRESS,
            stream.header,
        ));
        assert!(registry.bind(stream.stream_address, identity));

        let initial = registry
            .control_state(stream.stream_address, identity)
            .unwrap();
        assert_eq!(initial, StreamControlState::default());
        let paused = StreamControlState {
            pause_seen: 1,
            pause_mode: 1,
            revision: 1,
            ..initial
        };
        assert!(registry.commit_control_state(stream.stream_address, identity, initial, paused));
        let flushed = StreamControlState {
            flush_seen: 1,
            flush_flags: 1,
            revision: 2,
            ..paused
        };
        assert!(registry.commit_control_state(stream.stream_address, identity, paused, flushed));
        let started = StreamControlState {
            discontinuity_seen: 1,
            revision: 3,
            ..flushed
        };
        assert!(registry.commit_control_state(stream.stream_address, identity, flushed, started));
        let resumed = StreamControlState {
            pause_mode: 0,
            revision: 4,
            ..started
        };
        assert!(registry.commit_control_state(stream.stream_address, identity, started, resumed));
        assert_eq!(
            registry.control_state(stream.stream_address, identity),
            Some(resumed)
        );

        let mut stale = identity;
        stale.serial += 1;
        assert_eq!(registry.control_state(stream.stream_address, stale), None);
        assert!(!registry.commit_control_state(stream.stream_address, stale, resumed, resumed));
        let malformed = StreamControlState {
            flush_time_low: 1,
            revision: 5,
            ..resumed
        };
        assert!(!registry.commit_control_state(
            stream.stream_address,
            identity,
            resumed,
            malformed
        ));
        assert!(registry.detach(stream.stream_address, identity));
        assert_eq!(
            registry.control_state(stream.stream_address, identity),
            None
        );
    }

    #[test]
    fn control_requests_apply_measured_startup_policy_atomically() {
        let (stream, identity) = state();
        let mut registry = StreamLeaseRegistry::new();
        assert!(registry.reserve_scoped(
            stream.stream_address,
            stream.stream_heap,
            PUBLICATION_ADDRESS,
            stream.header,
            valid_scope(),
        ));
        assert!(registry.bind(stream.stream_address, identity));

        let request =
            |action, argument, count, apply, completion_available, pause_callback_available| {
                StreamControlRequest {
                    action,
                    argument,
                    count,
                    apply,
                    completion_available,
                    pause_callback_available,
                }
            };
        assert_eq!(
            registry.apply_control_request(
                stream.stream_address,
                identity,
                request(STREAM_CONTROL_PAUSE, 1, 1, 0, 0, 0),
            ),
            STREAM_CONTROL_PAUSE_NEEDS_FORMAT
        );
        assert_eq!(
            registry.control_state(stream.stream_address, identity),
            Some(StreamControlState::default())
        );
        assert_eq!(
            registry.apply_control_request(
                stream.stream_address,
                identity,
                request(STREAM_CONTROL_PAUSE, 1, 0, 0, 0, 0),
            ),
            STREAM_CONTROL_ACCEPTED
        );
        assert_eq!(
            registry.apply_control_request(
                stream.stream_address,
                identity,
                request(STREAM_CONTROL_FLUSH_EX, 0, 0, 1, 0, 0),
            ),
            STREAM_CONTROL_ACCEPTED
        );
        assert_eq!(
            registry.apply_control_request(
                stream.stream_address,
                identity,
                request(STREAM_CONTROL_DISCONTINUITY, 0, 0, 0, 0, 0),
            ),
            STREAM_CONTROL_ACCEPTED
        );
        assert_eq!(
            registry.apply_control_request(
                stream.stream_address,
                identity,
                request(STREAM_CONTROL_PAUSE, 0, 0, 0, 0, 0),
            ),
            STREAM_CONTROL_BAD_PAUSE
        );
        assert_eq!(
            registry.apply_control_request(
                stream.stream_address,
                identity,
                request(STREAM_CONTROL_PAUSE, 0, 0, 0, 1, 0),
            ),
            STREAM_CONTROL_ACCEPTED
        );

        let mut stale = identity;
        stale.serial += 1;
        assert_eq!(
            registry.apply_control_request(
                stream.stream_address,
                stale,
                request(STREAM_CONTROL_PAUSE, 1, 0, 0, 0, 0),
            ),
            STREAM_CONTROL_STALE_LEASE
        );
    }

    #[test]
    fn rollback_lookup_and_overlap_follow_registry_lifecycle() {
        let mut registry = StreamLeaseRegistry::new();
        let address = 0x1000;
        let heap = 0x2000;
        let publication = 0x3000;
        let newer_address = 0x4000;
        let newer_heap = 0x5000;
        assert!(registry.reserve(address, heap, publication, [0; STREAM_HEADER_WORDS]));
        assert!(registry.reserve(
            newer_address,
            newer_heap,
            publication + 4,
            [0; STREAM_HEADER_WORDS]
        ));
        assert_eq!(registry.first_rollback(), None);
        assert!(!registry.overlaps_guest_span(address, STREAM_ALLOCATION_BYTES));
        assert!(registry.retain_rollback(address));
        assert!(registry.retain_rollback(newer_address));
        assert_eq!(registry.first_rollback(), Some((newer_address, newer_heap)));
        assert!(registry.begin_retire_rollback(newer_address));
        assert!(registry.retire_rollback(newer_address));
        assert_eq!(registry.first_rollback(), Some((address, heap)));
        assert!(registry.overlaps_guest_span(address + 4, 4));
        assert!(!registry.overlaps_guest_span(publication, 4));
        assert!(!registry.overlaps_guest_span(u32::MAX, 0));
        assert!(registry.begin_retire_rollback(address));
        assert_eq!(registry.first_rollback(), None);
        assert!(!registry.overlaps_guest_span(address, 4));
        assert!(registry.restore_rollback(address));
        assert_eq!(registry.first_rollback(), Some((address, heap)));
        assert!(registry.begin_retire_rollback(address));
        assert!(registry.retire_rollback(address));
        assert_eq!(registry.first_rollback(), None);
    }

    #[test]
    fn rollback_cleanup_preserves_order_and_restores_on_heap_refusal() {
        let mut registry = StreamLeaseRegistry::new();
        let address = 0x1000;
        let heap = 0x2000;
        assert!(registry.reserve(address, heap, 0x3000, [0; STREAM_HEADER_WORDS]));
        assert!(registry.retain_rollback(address));
        let mut valid_calls = Vec::new();
        let mut destroy_calls = Vec::new();

        let refused = registry.cleanup_rollback_records(
            |candidate| {
                valid_calls.push(candidate);
                candidate == heap
            },
            |candidate| {
                destroy_calls.push(candidate);
                false
            },
        );
        assert_eq!(refused, Err(RollbackCleanupFailure::Refused));
        assert_eq!(valid_calls, [heap]);
        assert_eq!(destroy_calls, [heap]);
        assert_eq!(registry.first_rollback(), Some((address, heap)));

        assert_eq!(
            registry.cleanup_rollback_records(|_| true, |_| true),
            Ok(())
        );
        assert_eq!(registry.first_rollback(), None);
    }

    #[test]
    fn lifecycle_iteration_preserves_active_and_detached_head_order() {
        let mut registry = StreamLeaseRegistry::new();
        let identity = |internal_address, serial| DeviceLeaseIdentity {
            device_heap: 0x9000,
            internal_address,
            serial,
        };
        for address in [0x1000, 0x2000] {
            let lease = identity(address + 0x100, u64::from(address));
            assert!(registry.reserve(
                address,
                address + 0x400,
                address + 0x800,
                [0, 0, 1, lease.internal_address, 0, 0, 0, 0, 0, 0]
            ));
            assert!(registry.bind(address, lease));
        }
        assert_eq!(registry.first_active(), Some(0x2000));
        assert!(registry.detach(0x2000, identity(0x2100, 0x2000)));
        assert!(registry.detach(0x1000, identity(0x1100, 0x1000)));
        assert_eq!(registry.first_active(), None);
        // Reset detaches newest first, and the old C list pushes each node at its head.
        assert_eq!(registry.first_detached(), Some(0x1000));

        let lease = identity(0x3100, 0x3000);
        assert!(registry.reserve(
            0x3000,
            0x3400,
            0x3800,
            [0, 0, 1, lease.internal_address, 0, 0, 0, 0, 0, 0]
        ));
        assert!(registry.bind(0x3000, lease));
        assert_eq!(registry.first_active(), Some(0x3000));
        assert!(registry.detach(0x3000, lease));
        assert_eq!(registry.first_detached(), Some(0x3000));
    }

    #[test]
    fn mutable_format_state_is_generation_checked_and_compare_committed() {
        let mut registry = StreamLeaseRegistry::new();
        let address = 0x5100;
        let lease = DeviceLeaseIdentity {
            device_heap: 0x5200,
            internal_address: 0x5300,
            serial: 9,
        };
        assert!(registry.reserve(
            address,
            0x5400,
            0x5500,
            [0, 0, 0, lease.internal_address, 0, 0, 0, 0, 0, 0]
        ));
        assert!(registry.bind(address, lease));

        let initial = registry.format_state(address, lease).unwrap();
        assert_eq!(initial, StreamFormatState::default());
        let next_bytes = [0x69; FORMAT_BYTES];
        assert!(registry.commit_format_state(address, lease, initial, next_bytes));
        let updated = registry.format_state(address, lease).unwrap();
        assert_eq!(updated.bytes, next_bytes);
        assert_eq!(updated.sets, 1);
        assert!(!registry.commit_format_state(address, lease, initial, [0x11; FORMAT_BYTES]));
        assert_eq!(registry.format_state(address, lease), Some(updated));

        let mut stale_lease = lease;
        stale_lease.serial += 1;
        assert_eq!(registry.format_state(address, stale_lease), None);
        assert!(!registry.commit_format_state(address, stale_lease, updated, [0x22; FORMAT_BYTES]));
        assert_eq!(registry.format_state(address, lease), Some(updated));
        assert!(registry.detach(address, lease));
        assert_eq!(registry.format_state(address, lease), None);
    }

    #[test]
    fn accepted_volume_state_is_generation_checked_and_compare_committed() {
        let mut registry = StreamLeaseRegistry::new();
        let address = 0x6100;
        let lease = DeviceLeaseIdentity {
            device_heap: 0x6200,
            internal_address: 0x6300,
            serial: 10,
        };
        assert!(registry.reserve(
            address,
            0x6400,
            0x6500,
            [0, 0, 0, lease.internal_address, 0, 0, 0, 0, 0, 0]
        ));
        assert!(registry.bind(address, lease));

        let initial = registry.volume_state(address, lease).unwrap();
        assert_eq!(initial, StreamVolumeState::default());
        let next = StreamVolumeState {
            seen: 1,
            value: -10_000,
        };
        assert!(registry.commit_volume_state(address, lease, initial, next));
        assert_eq!(registry.volume_state(address, lease), Some(next));
        assert!(!registry.commit_volume_state(address, lease, initial, next));
        assert!(!registry.commit_volume_state(address, lease, next, StreamVolumeState::default()));

        let mut stale_lease = lease;
        stale_lease.serial += 1;
        assert_eq!(registry.volume_state(address, stale_lease), None);
        assert!(!registry.commit_volume_state(address, stale_lease, next, next));
        assert!(registry.detach(address, lease));
        assert_eq!(registry.volume_state(address, lease), None);
    }

    #[test]
    fn volume_policy_plans_startup_and_started_updates_for_the_exact_lease() {
        let (stream, lease) = state();
        let mut registry = StreamLeaseRegistry::new();
        assert!(registry.reserve_scoped(
            stream.stream_address,
            stream.stream_heap,
            PUBLICATION_ADDRESS,
            stream.header,
            valid_scope(),
        ));
        assert!(registry.bind(stream.stream_address, lease));

        let startup = StreamVolumeRequest {
            argument: 0xFFFF_D8F0,
            ..StreamVolumeRequest::default()
        };
        let planned = registry
            .plan_volume_request(stream.stream_address, lease, startup)
            .unwrap();
        assert_eq!(
            planned,
            StreamVolumeState {
                seen: 1,
                value: -10_000
            }
        );
        assert_eq!(
            registry.volume_state(stream.stream_address, lease),
            Some(StreamVolumeState::default())
        );
        assert_eq!(
            registry.plan_volume_request(
                stream.stream_address,
                lease,
                StreamVolumeRequest {
                    argument: 0xFFFF_FFFF,
                    ..startup
                }
            ),
            Err(STREAM_VOLUME_POLICY_REFUSED)
        );
        assert_eq!(
            registry.plan_volume_request(
                stream.stream_address,
                lease,
                StreamVolumeRequest {
                    count: 1,
                    ..startup
                }
            ),
            Err(STREAM_VOLUME_NEEDS_FORMAT)
        );
        assert!(registry.commit_volume_state(
            stream.stream_address,
            lease,
            StreamVolumeState::default(),
            planned,
        ));

        let control = |action, argument, count, apply, completion_available| StreamControlRequest {
            action,
            argument,
            count,
            apply,
            completion_available,
            ..StreamControlRequest::default()
        };
        assert_eq!(
            registry.apply_control_request(
                stream.stream_address,
                lease,
                control(STREAM_CONTROL_PAUSE, 1, 0, 0, 0),
            ),
            STREAM_CONTROL_ACCEPTED
        );
        assert_eq!(
            registry.apply_control_request(
                stream.stream_address,
                lease,
                control(STREAM_CONTROL_FLUSH_EX, 0, 0, 1, 0),
            ),
            STREAM_CONTROL_ACCEPTED
        );
        assert_eq!(
            registry.apply_control_request(
                stream.stream_address,
                lease,
                control(STREAM_CONTROL_DISCONTINUITY, 0, 0, 0, 0),
            ),
            STREAM_CONTROL_ACCEPTED
        );
        let gameplay_volume = StreamVolumeRequest {
            argument: (-1_137i32) as u32,
            completion_available: 1,
            ..StreamVolumeRequest::default()
        };
        assert_eq!(
            registry.plan_volume_request(stream.stream_address, lease, gameplay_volume),
            Ok(StreamVolumeState {
                seen: 1,
                value: -1_137,
            })
        );
        assert_eq!(
            registry.plan_volume_request(
                stream.stream_address,
                lease,
                StreamVolumeRequest {
                    completion_available: 2,
                    ..gameplay_volume
                }
            ),
            Err(STREAM_VOLUME_BAD_REQUEST)
        );
        let mut stale = lease;
        stale.serial += 1;
        assert_eq!(
            registry.plan_volume_request(stream.stream_address, stale, startup),
            Err(STREAM_VOLUME_STALE_LEASE)
        );
    }

    #[test]
    fn spatial_state_is_generation_checked_and_compare_committed() {
        let mut registry = StreamLeaseRegistry::new();
        let address = 0x7100;
        let lease = DeviceLeaseIdentity {
            device_heap: 0x7200,
            internal_address: 0x7300,
            serial: 11,
        };
        assert!(registry.reserve(
            address,
            0x7400,
            0x7500,
            [0, 0, 0, lease.internal_address, 0, 0, 0, 0, 0, 0]
        ));
        assert!(registry.bind(address, lease));

        let initial = registry.spatial_state(address, lease).unwrap();
        assert_eq!(initial, StreamSpatialState::default());
        let i3dl2 = StreamSpatialState {
            startup_cache_mask: 1,
            i3dl2_address: 0x7600,
            i3dl2_words: [0, 0, 0xffff_f448, 0, 0, 0, 0, 0, 0],
            revision: 1,
            ..initial
        };
        assert!(!registry.commit_spatial_state(
            address,
            lease,
            initial,
            StreamSpatialState {
                startup_cache_mask: 3,
                min_distance_bits: 0x3f80_0000,
                revision: 1,
                ..initial
            }
        ));
        assert!(registry.commit_spatial_state(address, lease, initial, i3dl2));
        assert!(registry.overlaps_i3dl2_span(0x7604, 4));
        assert!(!registry.overlaps_i3dl2_span(0x7624, 4));
        assert!(!registry.overlaps_i3dl2_span(0x7600, 0));
        let minimum = StreamSpatialState {
            startup_cache_mask: 3,
            min_distance_bits: 0x3f80_0000,
            revision: 2,
            ..i3dl2
        };
        assert!(registry.commit_spatial_state(address, lease, i3dl2, minimum));
        let configured = StreamSpatialState {
            startup_cache_mask: 7,
            curve_address: 0x004b_914c,
            curve_count: 4,
            revision: 3,
            ..minimum
        };
        assert!(registry.commit_spatial_state(address, lease, minimum, configured));
        let next = StreamSpatialState {
            position_bits: [0x3f80_0000, 0xbf00_0000, 0x3e80_0000],
            position_sets: 1,
            revision: 4,
            ..configured
        };
        assert!(registry.commit_spatial_state(address, lease, configured, next));
        assert_eq!(registry.spatial_state(address, lease), Some(next));
        assert!(!registry.commit_spatial_state(address, lease, configured, next));
        let max_distance = StreamSpatialState {
            max_distance_seen: 1,
            max_distance_bits: 0x4120_0000,
            revision: 5,
            ..next
        };
        assert!(registry.commit_spatial_state(address, lease, next, max_distance));
        let curve = StreamSpatialState {
            curve_address: 0x4400,
            curve_count: 4,
            revision: 6,
            ..max_distance
        };
        assert!(registry.commit_spatial_state(address, lease, max_distance, curve));
        assert!(!registry.commit_spatial_state(
            address,
            lease,
            curve,
            StreamSpatialState {
                revision: 5,
                ..curve
            }
        ));

        let mut stale_lease = lease;
        stale_lease.serial += 1;
        assert_eq!(registry.spatial_state(address, stale_lease), None);
        assert!(registry.detach(address, lease));
        assert!(!registry.overlaps_i3dl2_span(0x7600, 4));
        assert!(!registry.commit_spatial_state(address, stale_lease, curve, curve));
        assert!(!registry.detach(address, lease));
        assert_eq!(registry.spatial_state(address, lease), None);
    }

    #[test]
    fn max_distance_requires_apply_one_and_configured_spatial_lease() {
        let mut registry = StreamLeaseRegistry::new();
        let address = 0x7100;
        let lease = DeviceLeaseIdentity {
            device_heap: 0x7200,
            internal_address: 0x7300,
            serial: 12,
        };
        let mut scope = valid_scope();
        scope.flags = 0x10;
        scope.descriptor[0..4].copy_from_slice(&scope.flags.to_le_bytes());
        scope.channels = 1;
        scope.block_align = 36;
        scope.average_bytes_per_second = 24_806;
        scope.format[2..4].copy_from_slice(&scope.channels.to_le_bytes());
        scope.format[8..12].copy_from_slice(&scope.average_bytes_per_second.to_le_bytes());
        scope.format[12..14].copy_from_slice(&scope.block_align.to_le_bytes());
        assert!(registry.reserve_scoped(
            address,
            0x7400,
            0x7500,
            [0, 0, 0, lease.internal_address, 0, 0, 0, 0, 0, 0],
            scope,
        ));
        assert!(registry.bind(address, lease));
        assert!(!registry.record_max_distance(address, lease, 1, 0x4120_0000));

        let initial = registry.spatial_state(address, lease).unwrap();
        let i3dl2 = StreamSpatialState {
            startup_cache_mask: 1,
            i3dl2_address: 0x7600,
            i3dl2_words: [0, 0, 0xffff_f448, 0, 0, 0, 0, 0, 0],
            revision: 1,
            ..initial
        };
        assert!(registry.commit_spatial_state(address, lease, initial, i3dl2));
        let minimum = StreamSpatialState {
            startup_cache_mask: 3,
            min_distance_bits: 0x3f80_0000,
            revision: 2,
            ..i3dl2
        };
        assert!(registry.commit_spatial_state(address, lease, i3dl2, minimum));
        let configured = StreamSpatialState {
            startup_cache_mask: 7,
            curve_address: 0x004b_914c,
            curve_count: 4,
            revision: 3,
            ..minimum
        };
        assert!(registry.commit_spatial_state(address, lease, minimum, configured));
        assert!(!registry.record_max_distance(address, lease, 0, 0x4120_0000));
        assert!(registry.record_max_distance(address, lease, 1, 0x4120_0000));
        let accepted = registry.spatial_state(address, lease).unwrap();
        assert_eq!(accepted.max_distance_seen, 1);
        assert_eq!(accepted.max_distance_bits, 0x4120_0000);
        assert_eq!(accepted.revision, 4);
    }

    #[test]
    fn startup_spatial_policy_owns_order_and_exact_values() {
        let mut registry = StreamLeaseRegistry::new();
        let address = 0x8100;
        let lease = DeviceLeaseIdentity {
            device_heap: 0x8200,
            internal_address: 0x8300,
            serial: 33,
        };
        let mut scope = valid_scope();
        scope.flags = 0x10;
        scope.descriptor[0..4].copy_from_slice(&scope.flags.to_le_bytes());
        scope.channels = 1;
        scope.block_align = 36;
        scope.average_bytes_per_second = 24_806;
        scope.format[2..4].copy_from_slice(&scope.channels.to_le_bytes());
        scope.format[8..12].copy_from_slice(&scope.average_bytes_per_second.to_le_bytes());
        scope.format[12..14].copy_from_slice(&scope.block_align.to_le_bytes());
        assert!(registry.reserve_scoped(
            address,
            0x8400,
            0x8500,
            [0, 0, 0, lease.internal_address, 0, 0, 0, 0, 0, 0],
            scope
        ));
        assert!(registry.bind(address, lease));
        let words = [0, 0, 0xffff_f448, 0, 0, 0, 0, 0, 0];
        assert_eq!(
            registry.apply_startup_spatial(address, lease, 2, 0x3f80_0000, 0, [0; 9]),
            STREAM_SPATIAL_BAD_SEQUENCE
        );
        assert_eq!(
            registry.apply_startup_spatial(address, lease, 1, 0x7600, 0, [0; 9]),
            STREAM_SPATIAL_BAD_SEQUENCE
        );
        assert_eq!(
            registry.apply_startup_spatial(address, lease, 1, 0x7600, 0, words),
            STREAM_SPATIAL_ACCEPTED
        );
        assert_eq!(
            registry.apply_startup_spatial(address, lease, 1, 0x7610, 0, words),
            STREAM_SPATIAL_BAD_SEQUENCE
        );
        assert_eq!(
            registry.apply_startup_spatial(address, lease, 2, 0x3f80_0000, 0, [0; 9]),
            STREAM_SPATIAL_ACCEPTED
        );
        assert_eq!(
            registry.apply_startup_spatial(address, lease, 3, 0x004b_914c, 4, [0; 9]),
            STREAM_SPATIAL_ACCEPTED
        );
        let accepted = registry.spatial_state(address, lease).unwrap();
        assert_eq!(accepted.startup_cache_mask, 7);
        assert_eq!(accepted.i3dl2_address, 0x7600);
        assert_eq!(accepted.min_distance_bits, 0x3f80_0000);
        assert_eq!(
            (
                accepted.curve_address,
                accepted.curve_count,
                accepted.revision
            ),
            (0x004b_914c, 4, 3)
        );
    }
}
