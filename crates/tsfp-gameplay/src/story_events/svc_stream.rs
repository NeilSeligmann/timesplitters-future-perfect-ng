//! T1873: the streamed key sampler `0x58DF0` and the stream key cache `0x60410` / `0x60390` below `0x59320`
//! (clip flag bit 1). MEASURED against the original under Unicorn on synthetic stream worlds (goldens
//! `tests/data/t1873/stream_*.golden`, generator `tools/t1846_gold/stream_engine.py`); no real clip uses the streamed
//! form (flag bit 1 is never set on the disc), so the layouts below are FABRICATED worlds shaped by the code.
//! Names are INFERRED, addresses, offsets and control flow MEASURED. See `docs/t-rust-anim.md`.
//!
//! Globals (MEASURED): header `[0x6D7818]` (`+4` resident base offset, `+0x14` chunk size), block range table
//! `[0x6D7834]` (8 bytes per block: start, length), per key offsets `[0x6D7838]`, node offsets `[0x6D783C]`, key count
//! `[0x7BB260]`, node stride `[0x7BB284]`, window buffer `[0x7DE530]`, window block ids `0x7DE540 + 4 slot`, window
//! pending flags `0x7DE520 + 4 slot`, descriptor table `0x7DE560 + 4 id`; cache: context `[0x4D19CC]`, keys
//! `[0x4D19D0]` / `[0x4D19D4]`, pointers `[0x6D782C]` / `[0x6D7830]`.
use super::strip_update::GuestMemory;
use super::svc_objects::DestroyEffects;

const HEADER: u32 = 0x6D_7818;
const RANGES: u32 = 0x6D_7834;
const KEY_OFFSETS: u32 = 0x6D_7838;
const NODE_OFFSETS: u32 = 0x6D_783C;
const KEY_COUNT: u32 = 0x7B_B260;
const NODE_STRIDE: u32 = 0x7B_B284;
const BUFFER: u32 = 0x7D_E530;
const BLOCK_IDS: u32 = 0x7D_E540;
const PENDING: u32 = 0x7D_E520;
const DESCRIPTORS: u32 = 0x7D_E560;
const CACHE_CONTEXT: u32 = 0x4D_19CC;
const CACHE_KEY_A: u32 = 0x4D_19D0;
const CACHE_KEY_B: u32 = 0x4D_19D4;
const CACHE_POINTER_A: u32 = 0x6D_782C;
const CACHE_POINTER_B: u32 = 0x6D_7830;
const C_ONE: u32 = 0x47_5C78;
const C_QUAT_STEP: u32 = 0x47_8650;
/// `0x389B0(0)`: the error report of an unknown node selector.
pub const ERROR_REPORT: u32 = 0x3_89B0;
pub const STREAM_SAMPLER: u32 = 0x5_8DF0;
pub const KEY_CACHE: u32 = 0x6_0410;
pub const KEY_BASE: u32 = 0x6_0390;

fn r32(memory: &dyn GuestMemory, address: u32) -> u32 {
    memory.read_u32(address)
}

/// `0x60390(key)`: base address of the key's data. Key 0, a key outside every window slot and a pending slot give the
/// resident base `[H + 4] + H`; a ready slot gives `buffer + slot * [H + 0x14] + [[0x6D7838] + 4 key]`.
pub fn stream_key_base(memory: &dyn GuestMemory, key: u32) -> u32 {
    let header = r32(memory, HEADER);
    let resident = r32(memory, header.wrapping_add(4)).wrapping_add(header);
    if key == 0 {
        return resident;
    }
    let ranges = r32(memory, RANGES);
    for slot in 0..4u32 {
        let block = r32(memory, BLOCK_IDS.wrapping_add(4 * slot));
        let entry = ranges.wrapping_add(block.wrapping_shl(3));
        let start = r32(memory, entry) as i32;
        if (key as i32) < start {
            continue;
        }
        let end = start.wrapping_add(r32(memory, entry.wrapping_add(4)) as i32);
        if (key as i32) < end {
            if r32(memory, PENDING.wrapping_add(4 * slot)) != 0 {
                return resident;
            }
            let chunk = r32(memory, header.wrapping_add(0x14)).wrapping_mul(slot);
            let offset = r32(
                memory,
                r32(memory, KEY_OFFSETS).wrapping_add(key.wrapping_mul(4)),
            );
            return offset.wrapping_add(chunk).wrapping_add(r32(memory, BUFFER));
        }
    }
    resident
}

/// `0x60410(key, node)`: the key's node data pointer through the two entry cache. A null window buffer or a negative key
/// samples key 0, a key at or above the key count samples the last one. A node change invalidates both keys
/// (`-1000`). A miss replaces slot A when `keyA < keyB` (signed), else slot B.
pub fn stream_key_cache(memory: &mut dyn GuestMemory, key: u32, node: u32) -> u32 {
    let mut key = key as i32;
    if r32(memory, BUFFER) == 0 || key < 0 {
        key = 0;
    }
    let count = r32(memory, KEY_COUNT) as i32;
    if key >= count {
        key = count.wrapping_sub(1);
    }
    let (key_a, key_b) = if r32(memory, CACHE_CONTEXT) == node {
        let key_a = r32(memory, CACHE_KEY_A) as i32;
        if key == key_a {
            return r32(memory, CACHE_POINTER_A);
        }
        let key_b = r32(memory, CACHE_KEY_B) as i32;
        if key == key_b {
            return r32(memory, CACHE_POINTER_B);
        }
        (key_a, key_b)
    } else {
        memory.write_u32(CACHE_KEY_A, (-1000i32) as u32);
        memory.write_u32(CACHE_KEY_B, (-1000i32) as u32);
        memory.write_u32(CACHE_CONTEXT, node);
        (-1000, -1000)
    };
    let base = stream_key_base(memory, key as u32);
    let index = r32(memory, NODE_STRIDE)
        .wrapping_mul(key as u32)
        .wrapping_add(node);
    let offset = r32(
        memory,
        r32(memory, NODE_OFFSETS).wrapping_add(index.wrapping_mul(4)),
    );
    let pointer = base.wrapping_add(offset);
    if key_a < key_b {
        memory.write_u32(CACHE_KEY_A, key as u32);
        memory.write_u32(CACHE_POINTER_A, pointer);
    } else {
        memory.write_u32(CACHE_KEY_B, key as u32);
        memory.write_u32(CACHE_POINTER_B, pointer);
    }
    pointer
}

fn f32_at(memory: &dyn GuestMemory, address: u32) -> f32 {
    f32::from_bits(r32(memory, address))
}

fn copy_words(memory: &mut dyn GuestMemory, dest: u32, source: u32, count: u32) {
    for k in 0..count {
        let word = r32(memory, source.wrapping_add(4 * k));
        memory.write_u32(dest.wrapping_add(4 * k), word);
    }
}

/// Four 16 bit fields at `source` into the quaternion cells `out + 0xC..0x18` (`u16 * [0x478650] - 1.0`).
fn packed_quaternion(memory: &mut dyn GuestMemory, out: u32, source: u32) {
    let step = f32_at(memory, C_QUAT_STEP);
    let one = f32_at(memory, C_ONE);
    for k in 0..4u32 {
        let low = u32::from(memory.read_u8(source.wrapping_add(2 * k)));
        let high = u32::from(memory.read_u8(source.wrapping_add(2 * k + 1)));
        let value = ((low | high << 8) as f32) * step - one;
        memory.write_u32(out.wrapping_add(0xC + 4 * k), value.to_bits());
    }
}

/// `0x58DF0(clip, node, key, out, scale)`: the sample of a streamed clip (flags bit 1). The clip's metadata `[clip+0x20]`
/// names a descriptor (`[meta]`, `[0x7DE560 + 4 id]`, its `+0x18` times 24 is added to `[meta + 4]` and the header)
/// that holds the fixed block (28 byte nodes, 20 byte nodes with flag bit 0). Node kind `[record + 4]` (record
/// `[clip+0x28] + 32 node`): 0 or key 0 take everything from the fixed block, 2 the quaternion from the stream, 4 the
/// position from the stream, 8 both from the stream (stream data at `[0x60410] + [record + 0x14]`); other kinds report
/// through `0x389B0(0)` and leave `out` untouched. The position is scaled. Flag bit 4 (derived w) is not used.
pub fn stream_sample(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    clip: u32,
    node: u32,
    key: u32,
    out: u32,
    scale: u32,
) {
    let meta = r32(memory, clip.wrapping_add(0x20));
    let id = r32(memory, meta);
    let descriptor = r32(memory, DESCRIPTORS.wrapping_add(id.wrapping_mul(4)));
    let packed = r32(memory, clip.wrapping_add(8)) & 1 != 0;
    let keys = r32(memory, clip.wrapping_add(0x10)) as i32;
    let mut key = key as i32;
    if key >= keys {
        key = keys.wrapping_sub(1);
    }
    let record = r32(memory, clip.wrapping_add(0x28)).wrapping_add(node.wrapping_shl(5));
    let blocks = r32(memory, descriptor.wrapping_add(0x18)).wrapping_mul(3);
    let mut fixed = r32(memory, meta.wrapping_add(4))
        .wrapping_add(blocks.wrapping_mul(8))
        .wrapping_add(r32(memory, HEADER));
    fixed = fixed.wrapping_add(if packed {
        node.wrapping_mul(20)
    } else {
        node.wrapping_mul(0x1C)
    });
    let from_fixed = |memory: &mut dyn GuestMemory, position: bool| {
        if position {
            copy_words(memory, out, fixed, 3);
        }
        if packed {
            packed_quaternion(memory, out, fixed.wrapping_add(0xC));
        } else {
            copy_words(memory, out.wrapping_add(0xC), fixed.wrapping_add(0xC), 4);
        }
    };
    if key == 0 {
        from_fixed(memory, true);
    } else {
        let cached = stream_key_cache(memory, key as u32, id);
        let stream = r32(memory, record.wrapping_add(0x14)).wrapping_add(cached);
        match r32(memory, record.wrapping_add(4)) {
            0 => from_fixed(memory, true),
            2 => {
                copy_words(memory, out, fixed, 3);
                if packed {
                    packed_quaternion(memory, out, stream);
                } else {
                    copy_words(memory, out.wrapping_add(0xC), stream, 4);
                }
            }
            4 => {
                copy_words(memory, out, stream, 3);
                if packed {
                    packed_quaternion(memory, out, fixed.wrapping_add(0xC));
                } else {
                    copy_words(memory, out.wrapping_add(0xC), fixed.wrapping_add(0xC), 4);
                }
            }
            8 => {
                copy_words(memory, out, stream, 3);
                if packed {
                    packed_quaternion(memory, out, stream.wrapping_add(0xC));
                } else {
                    copy_words(memory, out.wrapping_add(0xC), stream.wrapping_add(0xC), 4);
                }
            }
            _ => {
                effects.call(memory, ERROR_REPORT, &[0]);
            }
        }
    }
    let scale = f32::from_bits(scale);
    for k in 0..3u32 {
        let value = f32_at(memory, out.wrapping_add(4 * k)) * scale;
        memory.write_u32(out.wrapping_add(4 * k), value.to_bits());
    }
}
