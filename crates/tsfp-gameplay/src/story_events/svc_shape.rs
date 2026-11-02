//! T1866: the shape and box queries under the object shape setup `0x97C00` and the placement `0xA6530`
//! (scratch blocks, shape slot buffers, light registration, bounding box aggregation). MEASURED against the Unicorn
//! oracle (`docs/t-story-event-resource-t1866.md`); names are INFERRED, addresses, offsets and control flow MEASURED.
//!
//! Float constants of the image (`0x475CD0`, `0x475CD4`, `0x475CC0`, `0x475DCC`, `0x47D54C`, `0x47D550`,
//! `0x475D5C`, `0x475CAC`, `0x475C78`) are read from guest memory, so the port runs on any memory that holds them.
//! Engine routines below stay behind [`DestroyEffects`]; routines with a stack frame take `entry`, the esp at entry.

use super::strip_update::GuestMemory;
use super::svc_objects::DestroyEffects;

pub mod callee {
    pub const BLOCK_ALLOC: u32 = 0x3_D6E0; // P
    pub const BLOCK_FREE: u32 = 0x3_D6B0; // P
    pub const ARCHIVE_MODEL: u32 = 0x1_2EC0; // P
    pub const SLOT_BUSY: u32 = 0x9_E3C0; // P
    pub const MODEL_KNOWN: u32 = 0x8_3790; // P
    pub const DISPATCH: u32 = 0x9_E330; // P
    pub const IS_DYNAMIC: u32 = 0x9_EAB0; // P
    pub const BOX_CORNERS: u32 = 0x9_EB60; // P
    pub const BOX_TRANSFORM: u32 = 0x9_E840; // P
    pub const SCALE_FACTOR: u32 = 0xB_E9E0; // P
    pub const SHAPE_MESH: u32 = 0x9_65F0; // P
    pub const SHAPE_FILL: u32 = 0x9_7A00; // P
    pub const SHAPE_REGISTER: u32 = 0x8_C910; // P
    pub const SHAPE_ATTACH: u32 = 0x9_66E0; // P
    pub const ALLOC_PLAIN: u32 = 0x3_DFA0; // E alloc(size, alignment)
    pub const REPORT: u32 = 0x3_89B0; // E
    pub const MATRIX_POINT: u32 = 0x15_1E80; // E (matrix, point) transform in place
    pub const SHAPE_DONE: u32 = 0x8_FE30; // E (esi = slot, eax = pointer to a zeroed local) stack: matrices
    pub const SHAPE_DYNAMIC: u32 = 0x9_76E0; // E (ecx = slot, eax = table word) stack: flag
    pub const SHAPE_REGISTER_BLOCK: u32 = 0x8_FB20; // E (eax = table, esi = buffer) stack: block
    pub const LIGHT_SLOT: u32 = 0x6_A540; // E () -> light record
    pub const MODEL_SCALE: u32 = 0x8_E5F0; // P
    pub const RECORD_TABLE: u32 = 0xB_E860; // P
    pub const WEAPON_RECORD: u32 = 0x1B_99B0; // E (index) -> index
    pub const DETACH: u32 = 0xA_5CE0; // E (tail jump)
}

const SCRATCH_MASK: u32 = 0x6B_7B00;
const SCRATCH_LAST: u32 = 0x6B_7B04;
const SCRATCH_CLEAR: u32 = 0x6B_7AF8;
const SCRATCH_STARTS: u32 = 0x6B_7AF4;
const SCRATCH_BASE: u32 = 0x7E_72E0;
const FLAGS_C: u32 = 0x7D_E458;
const MODE_BYTE: u32 = 0x7D_E455;
const LEVEL: u32 = 0x79_094C;
const POOL_BASE: u32 = 0x72_F5E8;
const POOL_USED: u32 = 0x7B_4FAC;
const MODEL_INFO: u32 = 0x7B_9778;
const MODEL_INFO_TABLE: u32 = 0x70_87A4;
const KIND_HANDLERS: u32 = 0x72_F2E0;
const LIGHT_TABLE: u32 = 0x7B_69AC;
const LIGHT_COUNT: u32 = 0x70_AAE0;
const LIGHT_PROFILE: u32 = 0x7E_3960;
const SHAPE_WORDS: u32 = 0x7E_03C0;
const CONST_DYNAMIC_Y: u32 = 0x47_5CD0;
const CONST_HALF: u32 = 0x47_5CD4;
const CONST_BYTE_SCALE: u32 = 0x47_5CC0;
const CONST_COLOUR_SCALE: u32 = 0x47_5DCC;
const CONST_BOX_MIN_INIT: u32 = 0x47_D550;
const CONST_BOX_MAX_INIT: u32 = 0x47_D54C;
const CONST_BOX_SKIP: u32 = 0x47_5D5C;
const CONST_SCALE_KEY: u32 = 0x47_5CAC;
const CONST_SCALE_DEFAULT: u32 = 0x47_5C78;

fn r32(memory: &dyn GuestMemory, address: u32) -> u32 {
    memory.read_u32(address)
}

fn w32(memory: &mut dyn GuestMemory, address: u32, value: u32) {
    memory.write_u32(address, value);
}

fn rf(memory: &dyn GuestMemory, address: u32) -> f32 {
    f32::from_bits(memory.read_u32(address))
}

fn wf(memory: &mut dyn GuestMemory, address: u32, value: f32) {
    memory.write_u32(address, value.to_bits());
}

fn r16s(memory: &dyn GuestMemory, address: u32) -> i32 {
    i32::from(
        (u16::from(memory.read_u8(address))
            | u16::from(memory.read_u8(address.wrapping_add(1))) << 8) as i16,
    )
}

fn scratch_index(pointer: u32) -> u32 {
    let delta = pointer.wrapping_sub(SCRATCH_BASE) as i32;
    let adjusted = delta.wrapping_add(((delta >> 31) as u32 & 0xFFF) as i32);
    (adjusted >> 12) as u32
}

/// `0x3D6E0(units)`: takes `units` consecutive 4 KiB scratch blocks (bitmap byte `[0x6B7B00]`, block starts at
/// `0x7E72E0 + 0x1000 * index`, the mask of each start is kept in the bytes at `0x6B7AF4`). When none is free the
/// block named by `[0x6B7B04]` is released first and the search repeated once. Returns the address or 0. MEASURED.
pub fn block_alloc(memory: &mut dyn GuestMemory, units: u32) -> u32 {
    let mut used = memory.read_u8(SCRATCH_MASK);
    let want = (1u32 << (units & 31)).wrapping_sub(1);
    let limit = 4i32.wrapping_sub(units as i32);
    let take = |memory: &mut dyn GuestMemory, used: &mut u8, index: u32| {
        let bits = (want << (index & 31)) as u8;
        *used |= bits;
        memory.write_u8(SCRATCH_STARTS.wrapping_add(index), bits);
        (index << 12).wrapping_add(SCRATCH_BASE)
    };
    if limit >= 0 {
        let mut shifted = u32::from(used);
        let mut index = 0i32;
        loop {
            if want & shifted == 0 {
                let address = take(memory, &mut used, index as u32);
                memory.write_u8(SCRATCH_MASK, used);
                return address;
            }
            shifted >>= 1;
            index += 1;
            if index > limit {
                break;
            }
        }
    }
    let last = r32(memory, SCRATCH_LAST);
    if last == 0 {
        return 0;
    }
    let bits = !memory.read_u8(SCRATCH_STARTS.wrapping_add(scratch_index(last)));
    used &= bits;
    memory.write_u8(SCRATCH_MASK, used);
    w32(memory, SCRATCH_LAST, 0);
    w32(memory, SCRATCH_CLEAR, 0);
    if limit < 0 {
        return 0;
    }
    let mut shifted = u32::from(used);
    let mut index = 0i32;
    loop {
        if want & shifted == 0 {
            let address = take(memory, &mut used, index as u32);
            memory.write_u8(SCRATCH_MASK, used);
            return address;
        }
        shifted >>= 1;
        index += 1;
        if index > limit {
            return 0;
        }
    }
}

/// `0x3D6B0(block)`: releases the scratch block starting at `block` (clears its mask bits). MEASURED.
pub fn block_free(memory: &mut dyn GuestMemory, block: u32) {
    let bits = !memory.read_u8(SCRATCH_STARTS.wrapping_add(scratch_index(block)));
    let used = memory.read_u8(SCRATCH_MASK);
    memory.write_u8(SCRATCH_MASK, used & bits);
}

/// `0x12EC0(descriptor)`: flag `0x2000000` of the descriptor word `+0x28` for archive models (not flag
/// `[0x7DE458] & 0x40`, level not `0x64`/`0x38`, mode byte `0xA` or level `0x66`), else 0. MEASURED.
pub fn archive_model(memory: &dyn GuestMemory, descriptor: u32) -> u32 {
    let level = r32(memory, LEVEL);
    if r32(memory, FLAGS_C) & 0x40 != 0 || level == 0x64 || level == 0x38 {
        return 0;
    }
    if memory.read_u8(MODE_BYTE) != 0xA && level != 0x66 {
        return 0;
    }
    r32(memory, descriptor.wrapping_add(0x28)) & 0x200_0000
}

/// `0x9E3C0(index)`: the pool object (`0x2E8` byte records at `[0x72F5E8]`) whose sub pool index (`+0x88`) is
/// `index` and whose free bit (`+0x28` sign) is clear; the record `index` itself is tried first. 0 when none. MEASURED.
pub fn slot_busy(memory: &dyn GuestMemory, index: u32) -> u32 {
    if (index as i32) < 0 {
        return 0;
    }
    let base = r32(memory, POOL_BASE);
    let record = index.wrapping_mul(0x2E8).wrapping_add(base);
    if (r32(memory, record.wrapping_add(0x28)) as i32) >= 0
        && r32(memory, record.wrapping_add(0x88)) == index
    {
        return record;
    }
    let used = r32(memory, POOL_USED) as i32;
    for position in 0..used.max(0) as u32 {
        let candidate = position.wrapping_mul(0x2E8).wrapping_add(base);
        if (r32(memory, candidate.wrapping_add(0x28)) as i32) >= 0
            && r32(memory, candidate.wrapping_add(0x88)) == index
        {
            return candidate;
        }
    }
    0
}

/// `0x83790(model)`: 1 without model info or for model ids below `0x908`, else bit 1 of the info word (`0x18` byte
/// records at `[0x7087A4]`). MEASURED.
pub fn model_known(memory: &dyn GuestMemory, model: u32) -> u32 {
    if r32(memory, MODEL_INFO) == 0 || (model as i32) < 0x908 {
        return 1;
    }
    let table = r32(memory, MODEL_INFO_TABLE);
    r32(memory, table.wrapping_add(model.wrapping_mul(24))) & 2
}

/// `0x9E330(object, group, argument)`: calls the handler `[0x72F2E0 + 4 * (group + 6 * highest set bit of
/// object[+0x20])]` with `(object, argument)` when it is not null (returns 1), else 0. MEASURED.
pub fn dispatch(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    object: u32,
    group: u32,
    argument: u32,
) -> u32 {
    let mut kind = r32(memory, object.wrapping_add(0x20)) as i32;
    let mut bit = 0i32;
    while kind != 1 {
        kind >>= 1;
        bit += 1;
        if bit >= 0x20 {
            bit = -1;
            break;
        }
    }
    let index = group.wrapping_add((bit as u32).wrapping_mul(6));
    let handler = r32(memory, index.wrapping_mul(4).wrapping_add(KIND_HANDLERS));
    if handler == 0 {
        return 0;
    }
    effects.call(memory, handler, &[object, argument]);
    1
}

/// `0x8E5F0(model)` -> f32 bits: `model[+0x30]`, or the default `[0x475C78]` when it equals `[0x475CAC]`. MEASURED.
pub fn model_scale(memory: &dyn GuestMemory, model: u32) -> u32 {
    let value = rf(memory, model.wrapping_add(0x30));
    if value == rf(memory, CONST_SCALE_KEY) {
        r32(memory, CONST_SCALE_DEFAULT)
    } else {
        value.to_bits()
    }
}

/// `0x9EAB0` (esi = `object`): for shape kinds 8 and `0x1000`: bounds `+0x1A8..` from the model block (`+0x30/34/38`
/// of the table at `+0x10`, or `0, [0x475CD0], 0` when object flag `0x100`) and `+0x1C0/1C4/1C8` = the model scale
/// (`0x8E5F0`); returns 1, else 0. MEASURED.
pub fn is_dynamic(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    object: u32,
) -> u32 {
    let kind = r32(memory, object.wrapping_add(0x20));
    if kind != 8 && kind != 0x1000 {
        return 0;
    }
    let model = r32(memory, object.wrapping_add(4));
    let info = r32(memory, model.wrapping_add(4));
    let scale = effects.call(memory, callee::MODEL_SCALE, &[info]);
    if r32(memory, object.wrapping_add(0x28)) & 0x100 != 0 {
        w32(memory, object.wrapping_add(0x1A8), 0);
        let height = r32(memory, CONST_DYNAMIC_Y);
        w32(memory, object.wrapping_add(0x1AC), height);
        w32(memory, object.wrapping_add(0x1B0), 0);
    } else {
        let model = r32(memory, object.wrapping_add(4));
        let block = r32(memory, model.wrapping_add(0x10));
        for (target, source) in [(0x1A8u32, 0x30u32), (0x1AC, 0x34), (0x1B0, 0x38)] {
            let value = r32(memory, block.wrapping_add(source));
            w32(memory, object.wrapping_add(target), value);
        }
    }
    for offset in [0x1C0u32, 0x1C4, 0x1C8] {
        w32(memory, object.wrapping_add(offset), scale);
    }
    1
}

/// `0xBE860(kind, index)`: the address of the record `index` in the table of `kind` (stride `0x54`; kinds 1, 2, 8,
/// 0x10, 0x20, 0x80, 0x100, 0x200, 0x400 and 0x1000, kind 0x800 is one record, kind 0x40 goes through the weapon
/// index `0x1B99B0`, stride `0xE0`, valid 1..`0x2B`), else 0. A negative index gives 0. MEASURED.
pub fn record_table(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    kind: u32,
    index: u32,
) -> u32 {
    if (index as i32) < 0 {
        return 0;
    }
    let stride54 = |base: u32| index.wrapping_mul(0x54).wrapping_add(base);
    match kind {
        0x80 => stride54(0x4D_C3C0),
        1 => stride54(0x4D_C218),
        2 => stride54(0x4F_B190),
        8 => stride54(0x4D_BA38),
        0x10 => stride54(0x4D_BE28),
        0x20 => stride54(0x4D_C668),
        0x40 => {
            let resolved = effects.call(memory, callee::WEAPON_RECORD, &[index]);
            if (resolved as i32) <= 0 || (resolved as i32) >= 0x2C {
                0
            } else {
                resolved.wrapping_mul(0xE0).wrapping_add(0x4E_A484)
            }
        }
        0x400 => stride54(0x4D_C510),
        0x200 => stride54(0x4D_DAE8),
        0x100 | 0x1000 => stride54(0x4D_C768),
        0x800 => 0x4D_C568,
        _ => 0,
    }
}

/// `0xBE9E0(object)` -> f32 bits (st0): the scale of the record `0xBE860(object[+0x7C][+0xC], [+0x14])`: its
/// `+0x20` when the mode byte is `0xA` or bit 0 of `object[+0x7C][+0x10]` is set, else its `+0x34`. MEASURED.
pub fn scale_factor(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    object: u32,
) -> u32 {
    let context = r32(memory, object.wrapping_add(0x7C));
    let index = r32(memory, context.wrapping_add(0x14));
    let kind = r32(memory, context.wrapping_add(0xC));
    let record = effects.call(memory, callee::RECORD_TABLE, &[kind, index]);
    let context = r32(memory, object.wrapping_add(0x7C));
    if memory.read_u8(MODE_BYTE) == 0xA || memory.read_u8(context.wrapping_add(0x10)) & 1 != 0 {
        r32(memory, record.wrapping_add(0x20))
    } else {
        r32(memory, record.wrapping_add(0x34))
    }
}

/// `0x9E840(matrix, extent, centre)` (frame `0x40`, `entry` = esp at entry): `0x151E80(matrix, centre)`, then the
/// absolute values of the 3x3 part of the matrix (translation zero) are built at `entry - 0x40` and
/// `0x151E80(absolute, extent)` follows. Only the words the original writes are written. MEASURED.
pub fn box_transform(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    entry: u32,
    matrix: u32,
    extent: u32,
    centre: u32,
) {
    effects.call(memory, callee::MATRIX_POINT, &[matrix, centre]);
    let local = entry.wrapping_sub(0x40);
    for (slot, source) in [
        (0u32, 0u32),
        (1, 1),
        (2, 2),
        (4, 4),
        (5, 5),
        (6, 6),
        (8, 8),
        (9, 9),
        (10, 10),
    ] {
        let value = r32(memory, matrix.wrapping_add(4 * source)) & 0x7FFF_FFFF;
        w32(memory, local.wrapping_add(4 * slot), value);
    }
    for slot in [12u32, 13, 14] {
        w32(memory, local.wrapping_add(4 * slot), 0);
    }
    effects.call(memory, callee::MATRIX_POINT, &[local, extent]);
}

/// `0x9EB60(object, extent, centre)` (frame `0x24`): the bounding box of all mesh parts of the object's model (parts
/// `0xC8` bytes before the block; a part is skipped when its `+0x90` float equals `[0x475D5C]`; for skinned
/// objects, flags `8` set and `0x100` clear and block `+4 > 0`, three bone matrices (bytes `+0x49 +0x4B +0x4A`) are
/// applied through `0x9E840`, else only the first corner) as `extent` = half size and `centre` = centre, with
/// the strict compare chain of `comiss` (a NaN candidate replaces the current value). MEASURED.
pub fn box_corners(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    entry: u32,
    object: u32,
    extent: u32,
    centre: u32,
) {
    let model = r32(memory, object.wrapping_add(4));
    let block = r32(memory, model.wrapping_add(4));
    let count = r32(memory, block) as i32;
    let first = block.wrapping_sub((count as u32).wrapping_mul(0xC8));
    let minimum_start = rf(memory, CONST_BOX_MIN_INIT);
    let maximum_start = rf(memory, CONST_BOX_MAX_INIT);
    let mut low = [minimum_start; 3];
    let mut high = [maximum_start; 3];
    let skip_value = rf(memory, CONST_BOX_SKIP);
    if count > 0 {
        let mut index = 0i32;
        let mut part = first.wrapping_add(0x90);
        loop {
            let probe = rf(memory, part);
            if probe != skip_value {
                let flags = r32(memory, object.wrapping_add(0x28));
                let skinned = flags & 8 != 0
                    && (r32(memory, block.wrapping_add(4)) as i32) > 0
                    && flags & 0x100 == 0;
                for step in 0..3u32 {
                    for k in 0..3u32 {
                        let value = r32(memory, part.wrapping_add(4 * k));
                        w32(memory, centre.wrapping_add(4 * k), value);
                        let value = r32(memory, part.wrapping_add(0xC + 4 * k));
                        w32(memory, extent.wrapping_add(4 * k), value);
                    }
                    let mut transform = None;
                    let accumulate;
                    if skinned {
                        let bone = match step {
                            0 => memory.read_u8(part.wrapping_sub(0x47)) as i8,
                            1 => memory.read_u8(part.wrapping_sub(0x45)) as i8,
                            _ => memory.read_u8(part.wrapping_sub(0x46)) as i8,
                        };
                        if step == 0 {
                            accumulate = true;
                            if bone >= 0 {
                                transform = Some(bone);
                            }
                        } else if bone < 0 {
                            continue;
                        } else {
                            let owner =
                                first.wrapping_add((i32::from(bone) as u32).wrapping_mul(0xC8));
                            let second = memory.read_u8(owner.wrapping_add(0x49)) as i8;
                            if second < 0 {
                                continue;
                            }
                            accumulate = true;
                            transform = Some(second);
                        }
                    } else {
                        if step > 0 {
                            continue;
                        }
                        accumulate = true;
                    }
                    if let Some(bone) = transform {
                        let table = r32(memory, model.wrapping_add(0x10));
                        let matrix = (i32::from(bone) as u32).wrapping_shl(6).wrapping_add(table);
                        if matrix != 0 {
                            box_transform(
                                memory,
                                effects,
                                entry.wrapping_sub(0x44),
                                matrix,
                                extent,
                                centre,
                            );
                        } else if step > 0 {
                            continue;
                        }
                    }
                    if accumulate {
                        let a = [
                            rf(memory, centre),
                            rf(memory, centre.wrapping_add(4)),
                            rf(memory, centre.wrapping_add(8)),
                        ];
                        let b = [
                            rf(memory, extent),
                            rf(memory, extent.wrapping_add(4)),
                            rf(memory, extent.wrapping_add(8)),
                        ];
                        for k in 0..3 {
                            let lowest = a[k] - b[k];
                            if !(lowest > low[k]) {
                                low[k] = lowest;
                            }
                            let highest = b[k] + a[k];
                            if !(high[k] > highest) {
                                high[k] = highest;
                            }
                        }
                    }
                }
            }
            index += 1;
            part = part.wrapping_add(0xC8);
            if index >= r32(memory, block) as i32 {
                break;
            }
        }
    }
    let half = rf(memory, CONST_HALF);
    let size = [
        (high[0] - low[0]) * half,
        (high[1] - low[1]) * half,
        (high[2] - low[2]) * half,
    ];
    for k in 0..3u32 {
        wf(memory, extent.wrapping_add(4 * k), size[k as usize]);
    }
    for k in 0..3u32 {
        let centre_value = if k == 0 {
            size[0] + low[0]
        } else {
            rf(memory, extent.wrapping_add(4 * k)) + low[k as usize]
        };
        wf(memory, centre.wrapping_add(4 * k), centre_value);
    }
}

/// `0x965F0(slot, descriptor)` (frame, `entry` = esp at entry): the shape buffer of a descriptor without one
/// (`descriptor[+0x20]` == 0): size `((block[+4] + 2 * block[0]) * 4 + 0x8F) & ~0xF`, reusing the block's own
/// buffer (`block[+0x44]`) when it is large enough, else `0x3DFA0(size, 1)` (and `0x389B0` when the block had a
/// buffer of another size); the buffer gets seven cleared cells (`+0x60..+0x78`), three table pointers (`+0`,
/// `+4`, `+0x5C`). Then `slot[+0x88] = 0`, `slot[+0x8C]` = the buffer and `0x8FE30` with a zeroed local at
/// `entry + 8`. MEASURED.
pub fn shape_mesh(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    entry: u32,
    slot: u32,
    descriptor: u32,
) {
    if r32(memory, descriptor.wrapping_add(0x20)) == 0 {
        let block = r32(memory, descriptor.wrapping_add(4));
        let size = ((r32(memory, block.wrapping_add(4))
            .wrapping_add(r32(memory, block).wrapping_mul(2)))
        .wrapping_mul(4)
        .wrapping_add(0x8F))
            & 0xFFFF_FFF0;
        let have = if block != 0 && r32(memory, block.wrapping_add(0x44)) != 0 {
            r32(memory, r32(memory, block.wrapping_add(0x44)))
        } else {
            0
        };
        let buffer = if (have as i32) < size as i32 {
            effects.call(memory, callee::ALLOC_PLAIN, &[size, 1])
        } else {
            r32(memory, block.wrapping_add(0x44))
        };
        let block_now = r32(memory, descriptor.wrapping_add(4));
        if block_now != 0 && r32(memory, block_now.wrapping_add(0x44)) != 0 && have != size {
            effects.call(memory, callee::REPORT, &[]);
        }
        w32(memory, descriptor.wrapping_add(0x20), buffer);
        for offset in (0x60u32..=0x78).step_by(4) {
            w32(memory, buffer.wrapping_add(offset), 0);
        }
        let head = buffer.wrapping_add(0x80);
        w32(memory, buffer, head);
        let block = r32(memory, descriptor.wrapping_add(4));
        let second = head.wrapping_add(r32(memory, block.wrapping_add(4)).wrapping_mul(4));
        w32(memory, buffer.wrapping_add(4), second);
        let block = r32(memory, descriptor.wrapping_add(4));
        let third = second.wrapping_add(r32(memory, block).wrapping_mul(4));
        w32(memory, buffer.wrapping_add(0x5C), third);
    }
    w32(memory, slot.wrapping_add(0x88), 0);
    let buffer = r32(memory, descriptor.wrapping_add(0x20));
    w32(memory, slot.wrapping_add(0x8C), buffer);
    let local = entry.wrapping_add(8);
    w32(memory, local, 0);
    effects.call(memory, callee::SHAPE_DONE, &[0, local, slot]);
}

/// `0x97A00(slot, descriptor, flag)` (frame): like [`shape_mesh`] for the full shape buffer (size `block[+4] * 0x44 +
/// block[0] * 8 + 0x20F` rounded to 16, seven matrix cells `+0x60..+0x78` at stride `0x40` from `+0x80`, tables
/// `+0`, `+4`, `+0x5C`); for a kind 8 owner (`slot[0][+0x20]`) a second buffer (`slot[+0x90]`, `0x3DFA0`) of the
/// same layout, else `slot[+0x90] = 0`. Then the light table entry `0x7E3960 + 12 * descriptor[+0x1C]` (0 for -1) goes
/// to `slot[+0x88]`, the buffer to `slot[+0x8C]` and `slot[+0x94]`, `0x976E0(ecx = slot, eax = table[+8])(flag)` and
/// `0x8FB20(eax = slot[+0x88], esi = buffer)(block)`. MEASURED.
pub fn shape_fill(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    slot: u32,
    descriptor: u32,
    flag: u32,
) {
    fn layout(memory: &mut dyn GuestMemory, buffer: u32, block: u32) {
        let mut cursor = buffer.wrapping_add(0x80);
        for cell in 0..7u32 {
            w32(memory, buffer.wrapping_add(0x60 + 4 * cell), cursor);
            if cell < 6 {
                cursor = cursor.wrapping_add(0x40);
            }
        }
        let first = r32(memory, block.wrapping_add(4))
            .wrapping_shl(6)
            .wrapping_add(cursor);
        w32(memory, buffer, first);
        let second = first.wrapping_add(r32(memory, block.wrapping_add(4)).wrapping_mul(4));
        w32(memory, buffer.wrapping_add(4), second);
        let third = second.wrapping_add(r32(memory, block).wrapping_mul(4));
        w32(memory, buffer.wrapping_add(0x5C), third);
    }
    if r32(memory, descriptor.wrapping_add(0x20)) == 0 {
        let block = r32(memory, descriptor.wrapping_add(4));
        let size = (r32(memory, block.wrapping_add(4))
            .wrapping_mul(0x44)
            .wrapping_add(r32(memory, block).wrapping_mul(8))
            .wrapping_add(0x20F))
            & 0xFFFF_FFF0;
        let have = if block != 0 && r32(memory, block.wrapping_add(0x44)) != 0 {
            r32(memory, r32(memory, block.wrapping_add(0x44)))
        } else {
            0
        };
        let buffer = if (have as i32) < size as i32 {
            effects.call(memory, callee::ALLOC_PLAIN, &[size, 1])
        } else {
            r32(memory, block.wrapping_add(0x44))
        };
        let block_now = r32(memory, descriptor.wrapping_add(4));
        if block_now != 0 && r32(memory, block_now.wrapping_add(0x44)) != 0 && have != size {
            effects.call(memory, callee::REPORT, &[]);
        }
        w32(memory, descriptor.wrapping_add(0x20), buffer);
        let block = r32(memory, descriptor.wrapping_add(4));
        layout(memory, buffer, block);
        let owner = r32(memory, slot);
        if r32(memory, owner.wrapping_add(0x20)) == 8 {
            let size = size.wrapping_add(0xF) & 0xFFFF_FFF0;
            let second = effects.call(memory, callee::ALLOC_PLAIN, &[size, 1]);
            w32(memory, slot.wrapping_add(0x90), second);
            let block = r32(memory, descriptor.wrapping_add(4));
            layout(memory, second, block);
        } else {
            w32(memory, slot.wrapping_add(0x90), 0);
        }
    }
    let profile = r32(memory, descriptor.wrapping_add(0x1C));
    let table = if profile == 0xFFFF_FFFF {
        0
    } else {
        profile.wrapping_mul(12).wrapping_add(LIGHT_PROFILE)
    };
    w32(memory, slot.wrapping_add(0x88), table);
    let buffer = r32(memory, descriptor.wrapping_add(0x20));
    w32(memory, slot.wrapping_add(0x94), buffer);
    w32(memory, slot.wrapping_add(0x8C), buffer);
    let word = r32(memory, table.wrapping_add(8));
    effects.call(memory, callee::SHAPE_DYNAMIC, &[flag, word, slot]);
    let block = r32(memory, descriptor.wrapping_add(4));
    let buffer = r32(memory, descriptor.wrapping_add(0x20));
    let table = r32(memory, slot.wrapping_add(0x88));
    effects.call(
        memory,
        callee::SHAPE_REGISTER_BLOCK,
        &[block, table, buffer],
    );
}

/// `0x8C910(descriptor)`: registers the lights of a loaded model (parts `0xC8` bytes before the block, `(i8)part[+0x4E]`
/// lights of `0xB4` bytes at `part[+0x6C]`): flag `4` or `0x80` lights get a light slot (`0x6A540`) filled with the
/// colour bytes (`+0x95..+0x97` times `[0x475DCC]`) and the negated position (`+0x88/8C/90`), all others a record of
/// the light table at `0x7B69AC` (stride `0x24`, first free record at or after the cursor, at most `0x145`, count
/// `[0x70AAE0]`). The byte `+0xAD` is copied from `+0xAC` first. MEASURED.
pub fn shape_register(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    descriptor: u32,
) {
    let block = r32(memory, descriptor.wrapping_add(4));
    let count = r32(memory, block);
    let mut part = block.wrapping_sub(count.wrapping_mul(0xC8));
    let mut cursor = 0i32;
    let mut part_index = 0u32;
    if part >= block {
        return;
    }
    loop {
        let lights = memory.read_u8(part.wrapping_add(0x4E)) as i8;
        if lights > 0 {
            let mut light = 0i32;
            let mut offset = 0u32;
            loop {
                let record = r32(memory, part.wrapping_add(0x6C)).wrapping_add(offset);
                let flags = r32(memory, record.wrapping_add(4));
                let copy = memory.read_u8(record.wrapping_add(0xAC));
                memory.write_u8(record.wrapping_add(0xAD), copy);
                if flags & 4 != 0 || (flags as u8 as i8) < 0 {
                    let target = effects.call(memory, callee::LIGHT_SLOT, &[]);
                    if target != 0 {
                        let scale = rf(memory, CONST_COLOUR_SCALE);
                        let (colour, position) = if flags & 4 != 0 {
                            (0x38u32, 0x2Cu32)
                        } else {
                            (0x50, 0x44)
                        };
                        for (k, byte) in [0x97u32, 0x96, 0x95].iter().enumerate() {
                            let value = f32::from(memory.read_u8(record.wrapping_add(*byte)));
                            wf(
                                memory,
                                target.wrapping_add(colour + 4 * k as u32),
                                value * scale,
                            );
                        }
                        for (k, source) in [0x88u32, 0x8C, 0x90].iter().enumerate() {
                            let value = 0.0f32 - rf(memory, record.wrapping_add(*source));
                            wf(memory, target.wrapping_add(position + 4 * k as u32), value);
                        }
                    }
                } else {
                    // The original has no bound; 0x10000 records stop a corrupt table (outside the contract).
                    for _ in 0..0x1_0000 {
                        let row = LIGHT_TABLE.wrapping_add((cursor as u32).wrapping_mul(0x24));
                        let owner = r32(memory, row.wrapping_add(0x10));
                        let tag = r32(memory, row.wrapping_add(0x14));
                        if owner == 0 && tag != 0xFFFF_FFFF {
                            break;
                        }
                        cursor += 1;
                    }
                    let used = r32(memory, LIGHT_COUNT) as i32;
                    if cursor >= used {
                        cursor = used;
                    }
                    if cursor < 0x145 {
                        let row = LIGHT_TABLE.wrapping_add((cursor as u32).wrapping_mul(0x24));
                        let radius = rf(memory, record.wrapping_add(0xA8));
                        wf(memory, row, radius * radius);
                        let mesh_cell = r32(memory, part.wrapping_add(0x5C));
                        w32(memory, row.wrapping_add(0xC), mesh_cell);
                        w32(memory, row.wrapping_add(0x10), descriptor);
                        w32(memory, row.wrapping_add(0x14), part_index);
                        w32(memory, row.wrapping_add(8), record);
                        let mut bits =
                            ((r32(memory, record.wrapping_add(4)) as i32) >> 4) as u8 & 1;
                        memory.write_u8(row.wrapping_add(4), bits);
                        if r16s(memory, record.wrapping_add(0xE)) != 0 {
                            let scale = rf(memory, CONST_BYTE_SCALE);
                            for (k, source) in [0x88u32, 0x8C, 0x90].iter().enumerate() {
                                let value = rf(memory, record.wrapping_add(*source)) * scale;
                                memory.write_u8(
                                    row.wrapping_add(5 + k as u32),
                                    truncate_to_i32(value) as u8,
                                );
                            }
                            bits |= 2;
                            memory.write_u8(row.wrapping_add(4), bits);
                        }
                        if r32(memory, part.wrapping_add(0x5C)) != 0 {
                            bits |= 4;
                            memory.write_u8(row.wrapping_add(4), bits);
                        }
                        if cursor >= used {
                            w32(memory, LIGHT_COUNT, (used as u32).wrapping_add(1));
                        }
                    }
                }
                light += 1;
                offset = offset.wrapping_add(0xB4);
                if light >= i32::from(memory.read_u8(part.wrapping_add(0x4E)) as i8) {
                    break;
                }
            }
        }
        part = part.wrapping_add(0xC8);
        part_index += 1;
        if part >= block {
            break;
        }
    }
}

/// `cvttss2si`: truncation toward zero, `0x80000000` for NaN and out of range values.
fn truncate_to_i32(value: f32) -> i32 {
    if value.is_nan() || value >= 2147483648.0 || value < -2147483648.0 {
        i32::MIN
    } else {
        value as i32
    }
}

/// `0x966E0(object, index, value)`: resets the attach state of a shape owner (`+0xD4` index, `+0xD8` the word of
/// the table `[0x7E03C0 + 2 * index]` sign extended, zeros at `+0xDC..+0x108`, `+0x108` = `value`, ...). For
/// index 1 and an owner whose `[object][+0x15C]` is `0x5CEF0` or `0x5FEB0` it tail jumps to `0xA5CE0([object], 2, 0)`.
/// MEASURED.
pub fn shape_attach(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    object: u32,
    index: u32,
    value: u32,
) {
    w32(memory, object.wrapping_add(0xD4), index);
    let word = r16s(memory, index.wrapping_mul(2).wrapping_add(SHAPE_WORDS)) as u32;
    w32(memory, object.wrapping_add(0xD8), word);
    for offset in [0xDCu32, 0xE0] {
        w32(memory, object.wrapping_add(offset), 0);
    }
    w32(memory, object.wrapping_add(0x108), value);
    for offset in [0xFCu32, 0x100, 0x104, 0xE4, 0xE8, 0xEC, 0xF0, 0xF4, 0xF8] {
        w32(memory, object.wrapping_add(offset), 0);
    }
    for offset in [0x128u32, 0x124, 0x180] {
        w32(memory, object.wrapping_add(offset), 0);
    }
    for offset in [
        0x154u32, 0x158, 0x15C, 0x13C, 0x140, 0x144, 0x148, 0x14C, 0x150,
    ] {
        w32(memory, object.wrapping_add(offset), 0);
    }
    w32(memory, object.wrapping_add(0x17C), 0);
    w32(memory, object.wrapping_add(0xC8), 0);
    w32(memory, object.wrapping_add(0x1A4), 0);
    if index == 1 {
        let target = r32(memory, object);
        let class = r32(memory, target.wrapping_add(0x15C));
        if class == 0x5_CEF0 || class == 0x5_FEB0 {
            effects.call(memory, callee::DETACH, &[target, 2, 0]);
        }
    }
}

/// Where the running routine sits, which decides the esp of the frames of the ported callees.
#[derive(Clone, Copy, Debug)]
pub enum ShapeContext {
    /// Running `0x97C00`: the callee `0x965F0` sees `entry - 0x1C`.
    ObjectShape,
    /// Running `0xA6530`: `0x9EB60` sees `entry - 0x78`, `0x9E840` sees `entry - 0x94`.
    ObjectPlace,
}

/// Runs the ported shape and box queries for their callee addresses and forwards the rest to `inner`.
/// `entry` is the esp at entry of the running routine selected by `context`. MEASURED by the whole-root goldens.
pub struct PortedShape<'a> {
    pub inner: &'a mut dyn DestroyEffects,
    pub entry: u32,
    pub context: ShapeContext,
}

impl DestroyEffects for PortedShape<'_> {
    fn call(&mut self, memory: &mut dyn GuestMemory, address: u32, args: &[u32]) -> u32 {
        let arg = |index: usize| args.get(index).copied().unwrap_or(0);
        let entry = self.entry;
        match address {
            callee::BLOCK_ALLOC => block_alloc(memory, arg(0)),
            callee::BLOCK_FREE => {
                block_free(memory, arg(0));
                0
            }
            callee::ARCHIVE_MODEL => archive_model(memory, arg(0)),
            callee::SLOT_BUSY => slot_busy(memory, arg(0)),
            callee::MODEL_KNOWN => model_known(memory, arg(0)),
            callee::DISPATCH => dispatch(memory, self, arg(0), arg(1), arg(2)),
            callee::IS_DYNAMIC => is_dynamic(memory, self, arg(0)),
            callee::MODEL_SCALE => model_scale(memory, arg(0)),
            callee::RECORD_TABLE => record_table(memory, self, arg(0), arg(1)),
            callee::SCALE_FACTOR => scale_factor(memory, self, arg(0)),
            callee::SHAPE_MESH if matches!(self.context, ShapeContext::ObjectShape) => {
                shape_mesh(memory, self, entry.wrapping_sub(0x1C), arg(0), arg(1));
                0
            }
            callee::SHAPE_FILL => {
                shape_fill(memory, self, arg(0), arg(1), arg(2));
                0
            }
            callee::SHAPE_REGISTER => {
                shape_register(memory, self, arg(0));
                0
            }
            callee::SHAPE_ATTACH => {
                shape_attach(memory, self, arg(0), arg(1), arg(2));
                0
            }
            callee::BOX_CORNERS if matches!(self.context, ShapeContext::ObjectPlace) => {
                box_corners(
                    memory,
                    self,
                    entry.wrapping_sub(0x78),
                    arg(0),
                    arg(1),
                    arg(2),
                );
                0
            }
            callee::BOX_TRANSFORM if matches!(self.context, ShapeContext::ObjectPlace) => {
                box_transform(
                    memory,
                    self,
                    entry.wrapping_sub(0x94),
                    arg(0),
                    arg(1),
                    arg(2),
                );
                0
            }
            _ => self.inner.call(memory, address, args),
        }
    }
}
