//! Readable table leaves used by the offline recorder. Addresses describe the ABI;
//! the contents are supplied by synthetic memory or the private runtime image.
use tsfp_ports::memory::GuestMemory;

/// 0x22E050: the upper table keeps the absolute index, unlike `record`.
pub fn descriptor(memory: &impl GuestMemory, index: u32) -> u32 {
    let base: u32 = if index < 0x15 { 0x510788 } else { 0x513d04 };
    memory.read32(base.wrapping_add(index.wrapping_mul(4)))
}

/// 0x22E070: 0x38-byte records, with the upper table rebased at index seven.
pub fn record(index: u32) -> u32 {
    if index < 7 {
        0x5107e0u32.wrapping_add(index.wrapping_mul(0x38))
    } else {
        0x513b60u32.wrapping_add(index.wrapping_sub(7).wrapping_mul(0x38))
    }
}

/// 0x69620: profile head, not the settings body (+0x7c).
pub fn profile(slot: u32) -> u32 {
    0x7bedc4u32.wrapping_add(slot.wrapping_mul(0x1f54))
}

/// 0x66470: return the bank id; resolving/localising belongs to the text seam.
pub fn text_id(memory: &impl GuestMemory, index: u32) -> u32 {
    memory.read32(0x4d28b8u32.wrapping_add(index.wrapping_mul(4)))
}
