//! Creation scope parsing (T2046 9b): Rust port of `dsound_stream_scope.c` and `dsound_buffer_scope.c`.
//! The scope is the measured startup shape the C host admits (NOT the full XDK format rules): 24 byte
//! descriptor, 20 byte WAVEFORMATEX prefix, Xbox ADPCM (tag 0x69, 4 bits, cbSize 2, 64 samples per block).
//! Evidence: Tier 1 vs the C source. The constants are the C host's measured values, copied.

use crate::adapter::GuestMemory;
use crate::byte_words::read_u32 as w32;
use crate::lifecycle::Scope;

pub const DESCRIPTOR_BYTES: usize = 24;
pub const FORMAT_BYTES: usize = 20;

/// A validated stream creation scope (oracle `dsound_stream_scope`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamScope {
    pub descriptor_address: u32,
    pub format_address: u32,
    pub flags: u32,
    pub max_packets: u32,
    pub descriptor: [u8; DESCRIPTOR_BYTES],
    pub sample_rate: u32,
    pub average_bytes_per_second: u32,
    pub channels: u16,
    pub block_align: u16,
    pub format: [u8; FORMAT_BYTES],
}

/// A validated buffer creation scope (oracle `dsound_buffer_scope`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferScope {
    pub descriptor_address: u32,
    pub format_address: u32,
    pub flags: u32,
    pub buffer_bytes: u32,
    pub sample_rate: u32,
    pub average_bytes_per_second: u32,
    pub channels: u16,
    pub block_align: u16,
    pub format: [u8; FORMAT_BYTES],
}

fn w16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

/// Bytes-only validation (oracle `dsound_stream_scope_validate`). `None` means refused.
pub fn stream_scope_validate(desc: &[u8], format: &[u8]) -> Option<StreamScope> {
    if desc.len() != DESCRIPTOR_BYTES || format.len() != FORMAT_BYTES {
        return None;
    }
    let flags = w32(desc, 0);
    let max_packets = w32(desc, 4);
    let format_address = w32(desc, 8);
    let s = StreamScope {
        descriptor_address: 0,
        format_address,
        flags,
        max_packets,
        descriptor: desc.try_into().ok()?,
        channels: w16(format, 2),
        sample_rate: w32(format, 4),
        average_bytes_per_second: w32(format, 8),
        block_align: w16(format, 12),
        format: format.try_into().ok()?,
    };
    if (flags != 0 && flags != 0x10)
        || max_packets != 3
        || format_address == 0
        || u64::from(format_address) + FORMAT_BYTES as u64 > 1 << 32
        || w32(desc, 12) != 0
        || w32(desc, 16) != 0
        || w32(desc, 20) != 0
        || w16(format, 0) != 0x69
        || s.sample_rate != 44100
        || w16(format, 14) != 4
        || w16(format, 16) != 2
        || w16(format, 18) != 64
    {
        return None;
    }
    let channels: u16 = if flags == 0 { 2 } else { 1 };
    if s.channels != channels
        || s.block_align != 36 * channels
        || s.average_bytes_per_second != (u32::from(s.block_align) * 44100) >> 6
    {
        return None;
    }
    Some(s)
}

/// Bytes-only validation (oracle `dsound_buffer_scope_validate`).
pub fn buffer_scope_validate(desc: &[u8], format: &[u8]) -> Option<BufferScope> {
    if desc.len() != DESCRIPTOR_BYTES || format.len() != FORMAT_BYTES {
        return None;
    }
    let flags = w32(desc, 4);
    let format_address = w32(desc, 12);
    let s = BufferScope {
        descriptor_address: 0,
        format_address,
        flags,
        buffer_bytes: w32(desc, 8),
        channels: w16(format, 2),
        sample_rate: w32(format, 4),
        average_bytes_per_second: w32(format, 8),
        block_align: w16(format, 12),
        format: format.try_into().ok()?,
    };
    if (flags != 0 && flags != 0x10)
        || w32(desc, 0) != 24
        || s.buffer_bytes != 0
        || format_address == 0
        || u64::from(format_address) + FORMAT_BYTES as u64 > 1 << 32
        || w32(desc, 16) != 0
        || w32(desc, 20) != 0
        || w16(format, 0) != 0x69
        || s.sample_rate != 44000
        || w16(format, 14) != 4
        || w16(format, 16) != 2
        || w16(format, 18) != 64
    {
        return None;
    }
    if s.channels != 1
        || s.block_align != 36
        || s.average_bytes_per_second != (u32::from(s.block_align) * 44000) >> 6
    {
        return None;
    }
    Some(s)
}

/// The descriptor and format must not overlap each other (oracle: the `descriptor+24 > address` test).
pub fn stream_scope_format_address_ok(descriptor: u32, address: u32) -> bool {
    address != 0
        && u64::from(address) + FORMAT_BYTES as u64 <= 1 << 32
        && !(u64::from(descriptor) + DESCRIPTOR_BYTES as u64 > u64::from(address)
            && u64::from(address) + FORMAT_BYTES as u64 > u64::from(descriptor))
}

/// Whether a guest span overlaps fixed stream-create state protected by the
/// passive-stream adapter. This is the address policy used by the Rust host;
/// dynamic stream spans remain owned by the lease registry.
pub fn stream_create_fixed_state_overlap(
    address: u32,
    bytes: u32,
    internal: u32,
    include_curve: bool,
) -> bool {
    fn overlaps(a: u32, an: u32, b: u32, bn: u32) -> bool {
        u64::from(a) < u64::from(b) + u64::from(bn) && u64::from(b) < u64::from(a) + u64::from(an)
    }

    overlaps(address, bytes, internal, 44)
        || overlaps(address, bytes, 0x00412B30, 4)
        || overlaps(address, bytes, 0x004A1CF0, 60)
        || overlaps(address, bytes, 0x004124A8, 4)
        || (include_curve && overlaps(address, bytes, 0x004B914C, 16))
}

fn format_address_ok(descriptor: u32, address: u32) -> bool {
    stream_scope_format_address_ok(descriptor, address)
}

/// Oracle `dsound_stream_scope_snapshot`: read the descriptor and the format it points at.
pub fn stream_scope_snapshot<M: GuestMemory + ?Sized>(
    m: &M,
    descriptor: u32,
) -> Option<StreamScope> {
    let desc = m.read_bytes(descriptor, DESCRIPTOR_BYTES)?;
    let address = w32(&desc, 8);
    if !stream_scope_format_address_ok(descriptor, address) {
        return None;
    }
    let format = m.read_bytes(address, FORMAT_BYTES)?;
    let mut s = stream_scope_validate(&desc, &format)?;
    s.descriptor_address = descriptor;
    Some(s)
}

/// Oracle `dsound_buffer_scope_snapshot`.
pub fn buffer_scope_snapshot<M: GuestMemory + ?Sized>(
    m: &M,
    descriptor: u32,
) -> Option<BufferScope> {
    let desc = m.read_bytes(descriptor, DESCRIPTOR_BYTES)?;
    let address = w32(&desc, 12);
    if !format_address_ok(descriptor, address) {
        return None;
    }
    let format = m.read_bytes(address, FORMAT_BYTES)?;
    let mut s = buffer_scope_validate(&desc, &format)?;
    s.descriptor_address = descriptor;
    Some(s)
}

impl StreamScope {
    pub fn lifecycle(&self) -> Scope {
        Scope {
            flags: self.flags,
            descriptor_address: self.descriptor_address,
            format_address: self.format_address,
        }
    }
}
impl BufferScope {
    pub fn lifecycle(&self) -> Scope {
        Scope {
            flags: self.flags,
            descriptor_address: self.descriptor_address,
            format_address: self.format_address,
        }
    }
}
