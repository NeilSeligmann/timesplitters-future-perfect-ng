//! T1865: the pool support behind `0xA6880` (`svc_destroy::pool_get`) and `0xA7090` (`destroy_pool_object`): pool
//! slot allocation, model loaders, shape slots, placement and the release set. MEASURED against the Unicorn oracle
//! (`docs/t-story-event-pool-t1865.md`); names are INFERRED, addresses, offsets and control flow MEASURED.
//!
//! Each function takes the [`DestroyEffects`] sink for the engine routines below it (resource cache and file open,
//! bounds queries, voice start, ...). Routines with a stack frame that hands the address of a local to a callee take
//! `entry`, the value of esp at the function's entry (the return address slot), exactly as in `svc_destroy_callees`.
//! [`PortedPool`] is the dispatcher that runs these ports for the callee addresses of `0xA6880` and `0xA7090` and
//! forwards everything else to `inner`.

use super::strip_update::GuestMemory;
use super::svc_destroy::{GLOBAL_LEVEL, pool_callee};
use super::svc_objects::DestroyEffects;

/// Original addresses of the engine routines below the ported pool support.
pub mod callee {
    /// `0x9E3C0(index)`: non zero when a sub pool slot is in use.
    pub const SLOT_BUSY: u32 = 0x9_E3C0;
    pub const CACHE_LOOKUP: u32 = 0x6_31C0;
    pub const NAME_LENGTH: u32 = 0x6_2FB0;
    pub const FORMAT: u32 = 0x3C_85D4;
    pub const OPEN_NAMED: u32 = 0x6_24C0;
    pub const NOTE_MODEL: u32 = 0x6_B380;
    pub const FIND_FILE: u32 = 0x6_2C30;
    pub const LOAD_FILE: u32 = 0x6_2F00;
    pub const FIXUP_FILE: u32 = 0x9_6330;
    pub const BIND_FILE: u32 = 0x8_DF60;
    pub const REPORT_MISSING: u32 = 0x3_89B0;
    pub const ARCHIVE_ID: u32 = 0x1_8380;
    pub const ARCHIVE_ADD: u32 = 0x1_8A30;
    pub const MODEL_FINISH: u32 = 0x8_E420;
    pub const BLOCK_ALLOC: u32 = 0x3_D6E0;
    pub const BLOCK_FREE: u32 = 0x3_D6B0;
    pub const SHAPE_MESH: u32 = 0x9_65F0;
    pub const SHAPE_FILL: u32 = 0x9_7A00;
    pub const SHAPE_BOUNDS: u32 = 0x18_FDC0;
    pub const SHAPE_REGISTER: u32 = 0x8_C910;
    pub const MODEL_KNOWN: u32 = 0x8_3790;
    pub const IS_DYNAMIC: u32 = 0x9_EAB0;
    pub const BOX_CORNERS: u32 = 0x9_EB60;
    pub const WORLD_MATRIX: u32 = 0xA_14B0;
    pub const BOX_TRANSFORM: u32 = 0x9_E840;
    pub const SCALE_FACTOR: u32 = 0xB_E9E0;
    pub const SHAPE_QUERY: u32 = 0x8_73C0;
    pub const CLEAR_LISTS: u32 = 0x9_EF60;
    pub const TAIL_A: u32 = 0xA_5F40;
    pub const TAIL_B: u32 = 0xA_6290;
    pub const TAIL_C: u32 = 0x12_9920;
    pub const TAIL_D: u32 = 0xA_64D0;
    pub const TAIL_E: u32 = 0x12_C1F0;
    pub const TAIL_F: u32 = 0x12_8710;
    pub const TAIL_G: u32 = 0x12_8320;
    pub const KEY_CLEAR: u32 = 0x23_FF70;
    pub const SPAWN_SLOT_RELEASE: u32 = 0x10_E070;
    pub const REGION_RELEASE: u32 = 0x11_23C0;
    pub const HITBOX_CLEAR: u32 = 0x15_3240;
    pub const VOICE_PREPARE: u32 = 0x16_5FE0;
    pub const VOICE_DROP: u32 = 0x15_3200;
    pub const VOICE_STOP: u32 = 0x16_C300;
    pub const OBJECT_DETACH: u32 = 0xA_5CE0;
    pub const OBJECT_REMOVE: u32 = 0x9_E720;
    pub const BUFFER_TAIL: u32 = 0x5_6FF0;
    pub const SOUND_RELEASE: u32 = 0x2_73E0;
    pub const LINK_LOOKUP: u32 = 0x27_7FE0;
    pub const LINK_REFRESH: u32 = 0x15_A540;
    pub const LINK_IS_LOCAL: u32 = 0x27_5620;
    pub const LINK_TEST_A: u32 = 0x1_7510;
    pub const LINK_TEST_B: u32 = 0x1_74E0;
    pub const LINK_DROP: u32 = 0x1_7570;
    pub const HUD_RELEASE: u32 = 0x22_71F0;
    pub const PHYSICS_NETWORK: u32 = 0xE_03D0;
    pub const PHYSICS_LOCAL: u32 = 0xE_0300;
}

const GLOBAL_POOL_BASE: u32 = 0x72_F5E8;
const GLOBAL_POOL_SUBCOUNT: u32 = 0x72_F5E4;
const GLOBAL_POOL_USED: u32 = 0x7B_4FAC;
const GLOBAL_POOL_FREE: u32 = 0x7B_65B8;
const GLOBAL_FLAGS_C: u32 = 0x7D_E458;
const GLOBAL_MODE_BYTE: u32 = 0x7D_E455;
const GLOBAL_FILE_HANDLE: u32 = 0x6D_7878;
const GLOBAL_MODEL_INDEX: u32 = 0x78_B860;
const GLOBAL_MODEL_TABLE: u32 = 0x79_1D4C;
const GLOBAL_SHAPE_CALLBACK: u32 = 0x4D_9704;
const GLOBAL_SHAPE_SCRATCH: u32 = 0x70_AAD4;
const GLOBAL_SHAPE_BASE: u32 = 0x70_AADC;
const GLOBAL_SHAPE_USED: u32 = 0x7B_67D0;
const GLOBAL_SHAPE_FREE: u32 = 0x7B_6984;
const GLOBAL_SHAPE_WORD: u32 = 0x7E_03C0;
const GLOBAL_SLOT_COUNT: u32 = 0x79_0950;
const GLOBAL_SLOT_EXTRA: u32 = 0x7A_2958;
const GLOBAL_SLOT_TABLE: u32 = 0x73_56D8;
const FORMAT_MODEL: u32 = 0x47_8354;
const FORMAT_MISSING: u32 = 0x47_D3DC;
const FORMAT_ARCHIVE: u32 = 0x47_833C;
const SUFFIX_ARCHIVE: u32 = 0x47_834C;
const CONST_SHAPE_SCALE: u32 = 0x47_8630;
const CONST_BOX_SCALE: u32 = 0x47_5CD4;
const CONST_RELEASE_FLOAT: u32 = 0x47_D9D4;

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

fn r16(memory: &dyn GuestMemory, address: u32) -> u32 {
    u32::from(memory.read_u8(address)) | u32::from(memory.read_u8(address.wrapping_add(1))) << 8
}

fn w16(memory: &mut dyn GuestMemory, address: u32, value: u32) {
    memory.write_u8(address, value as u8);
    memory.write_u8(address.wrapping_add(1), (value >> 8) as u8);
}

/// `movsx eax, word ptr [address]`.
fn sx16(memory: &dyn GuestMemory, address: u32) -> u32 {
    r16(memory, address) as u16 as i16 as i32 as u32
}

/// `0x9EE00()` -> object: takes the next object of the pool (`0x2E8` byte records at `[0x72F5E8]`). With nothing
/// recycled (`[0x7B65B8] == 0`) it extends the pool (`[0x7B4FAC]` records used), otherwise it takes the first record
/// whose free bit (`+0x2B & 0x80`) is set and clears `+0x28`. Then it picks the sub pool index: the record index
/// itself when `0x9E3C0(index)` is 0, else the highest index below `[0x72F5E4]` with `0x9E3C0 == 0` (-1 when none),
/// stored at `+0x88`; `+0` is cleared. MEASURED.
pub fn pool_alloc(memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects) -> u32 {
    let (object, mut index);
    if r32(memory, GLOBAL_POOL_FREE) == 0 {
        let used = r32(memory, GLOBAL_POOL_USED);
        let base = r32(memory, GLOBAL_POOL_BASE);
        object = used.wrapping_mul(0x2E8).wrapping_add(base);
        index = used;
        w32(memory, GLOBAL_POOL_USED, used.wrapping_add(1));
    } else {
        let used = r32(memory, GLOBAL_POOL_USED);
        let base = r32(memory, GLOBAL_POOL_BASE);
        let (mut found, mut found_index) = (0u32, 0xFFFF_FFFFu32);
        let (mut scan, mut record) = (0u32, base);
        while (scan as i32) < (used as i32) {
            if memory.read_u8(record.wrapping_add(0x2B)) & 0x80 != 0 {
                found = record;
                found_index = scan;
            }
            scan = scan.wrapping_add(1);
            record = record.wrapping_add(0x2E8);
            if found != 0 {
                break;
            }
        }
        if found != 0 {
            let free = r32(memory, GLOBAL_POOL_FREE).wrapping_sub(1);
            w32(memory, GLOBAL_POOL_FREE, free);
            w32(memory, found.wrapping_add(0x28), 0);
            object = found;
            index = found_index;
        } else {
            object = used.wrapping_mul(0x2E8).wrapping_add(base);
            index = used;
            w32(memory, GLOBAL_POOL_USED, used.wrapping_add(1));
            w32(memory, GLOBAL_POOL_FREE, 0);
        }
    }
    w32(memory, object.wrapping_add(0x88), 0xFFFF_FFFF);
    if effects.call(memory, callee::SLOT_BUSY, &[index]) != 0 {
        index = r32(memory, GLOBAL_POOL_SUBCOUNT).wrapping_sub(1);
        if (index as i32) < 0 {
            index = 0xFFFF_FFFF;
        } else {
            loop {
                if effects.call(memory, callee::SLOT_BUSY, &[index]) == 0 {
                    break;
                }
                index = index.wrapping_sub(1);
                if (index as i32) < 0 {
                    index = 0xFFFF_FFFF;
                    break;
                }
            }
        }
    }
    w32(memory, object.wrapping_add(0x88), index);
    w32(memory, object, 0);
    object
}

/// The test shared by `0x96410`: not the flag `[0x7DE458] & 0x40`, level not `0x64` / `0x38`, mode byte `0xA` or
/// level `0x66`, and descriptor flag `0x2000000` of the model's descriptor.
fn archive_model(memory: &dyn GuestMemory, model: u32) -> bool {
    let level = r32(memory, GLOBAL_LEVEL);
    if r32(memory, GLOBAL_FLAGS_C) & 0x40 != 0 || level == 0x64 || level == 0x38 {
        return false;
    }
    if memory.read_u8(GLOBAL_MODE_BYTE) != 0xA && level != 0x66 {
        return false;
    }
    let index = sx16(
        memory,
        GLOBAL_MODEL_INDEX.wrapping_add(model.wrapping_mul(2)),
    );
    let descriptor = r32(memory, GLOBAL_MODEL_TABLE).wrapping_add(index.wrapping_mul(0x30));
    r32(memory, descriptor.wrapping_add(0x28)) & 0x200_0000 != 0
}

/// `0x96410(name, model)` -> model block (cdecl, frame `0xCC`): finds a model file by name (cache `0x631C0`, else the
/// named archive entry `0x624C0` of the formatted archive path for archive models, else `0x62C30` / `0x62F00`),
/// reports a missing archive entry (`0x389B0`, returns 0), and on first use fixes the block up (`0x96330` for archive
/// models), points `+0x10` / `+0x14` at the relative strings of the file header, binds it (`0x8DF60`) and marks it
/// loaded (`+0xC |= 0x800000`, cache entry byte `+2 |= 1`). `entry` is esp at entry. MEASURED.
pub fn model_find(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    entry: u32,
    name: u32,
    model: u32,
) -> u32 {
    let buffer = entry.wrapping_sub(0xC8);
    let cached = effects.call(memory, callee::CACHE_LOOKUP, &[name]);
    let mut file;
    if cached != 0 {
        file = r32(memory, cached.wrapping_add(0x10));
    } else {
        if archive_model(memory, model) {
            let length = effects.call(memory, callee::NAME_LENGTH, &[name]);
            effects.call(
                memory,
                callee::FORMAT,
                &[buffer, FORMAT_MODEL, name, length],
            );
            let handle = r32(memory, GLOBAL_FILE_HANDLE);
            file = effects.call(memory, callee::OPEN_NAMED, &[buffer, handle]);
            if file == 0 {
                effects.call(memory, callee::FORMAT, &[buffer, FORMAT_MISSING, name]);
                effects.call(memory, callee::REPORT_MISSING, &[]);
                return 0;
            }
            let index = sx16(
                memory,
                GLOBAL_MODEL_INDEX.wrapping_add(model.wrapping_mul(2)),
            );
            effects.call(memory, callee::NOTE_MODEL, &[index]);
        } else {
            file = effects.call(memory, callee::FIND_FILE, &[name, 0]);
        }
        if file == 0 {
            file = effects.call(
                memory,
                callee::LOAD_FILE,
                &[name, 1, entry.wrapping_sub(0xCC)],
            );
        }
    }
    let offset = r32(memory, file.wrapping_add(4));
    let block = if offset != 0 {
        offset.wrapping_add(file)
    } else {
        0
    };
    if cached != 0 && memory.read_u8(cached.wrapping_add(2)) & 1 != 0 {
        return block;
    }
    if r32(memory, block.wrapping_add(0xC)) & 0x80_0000 != 0 {
        return block;
    }
    if archive_model(memory, model) {
        effects.call(memory, callee::FIXUP_FILE, &[file, 1]);
    }
    let first = r32(memory, file);
    w32(
        memory,
        block.wrapping_add(0x10),
        if first != 0 {
            first.wrapping_add(file)
        } else {
            0
        },
    );
    let second = r32(memory, file.wrapping_add(8));
    w32(
        memory,
        block.wrapping_add(0x14),
        if second != 0 {
            second.wrapping_add(file)
        } else {
            0
        },
    );
    effects.call(memory, callee::BIND_FILE, &[block, file]);
    let flags = r32(memory, block.wrapping_add(0xC)) | 0x80_0000;
    w32(memory, block.wrapping_add(0xC), flags);
    if cached != 0 {
        let byte = memory.read_u8(cached.wrapping_add(2)) | 1;
        memory.write_u8(cached.wrapping_add(2), byte);
    }
    block
}

/// `0x8E4B0(name, mode)` (cdecl, frame `0x1CC`): loads a model archive: looks the name up (`0x631C0`), opens the
/// formatted archive entry (`0x624C0`) and walks its 16 byte records until the id `-1`, resolving each record's
/// archive id (`0x18380` from the formatted name, else registering it with `0x18A30`), then finishes with
/// `0x8E420(ecx = mode, edi = archive block)(lookup)`. MEASURED (the frame is written like the original: the lookup
/// result lives at `entry - 0x1CC`).
pub fn model_load_a(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    entry: u32,
    name: u32,
    mode: u32,
) -> u32 {
    let handle_cell = entry.wrapping_sub(0x1CC);
    let looked = effects.call(memory, callee::CACHE_LOOKUP, &[name]);
    w32(memory, handle_cell, looked);
    let length = effects.call(memory, callee::NAME_LENGTH, &[name]);
    let buffer = entry.wrapping_sub(0x1C8);
    effects.call(
        memory,
        callee::FORMAT,
        &[buffer, FORMAT_MODEL, name, length],
    );
    let handle = r32(memory, GLOBAL_FILE_HANDLE);
    let archive = effects.call(memory, callee::OPEN_NAMED, &[buffer, handle]);
    let first = r32(memory, archive);
    let mut record = if first != 0 {
        first.wrapping_add(archive)
    } else {
        0
    };
    if r32(memory, record) != 0xFFFF_FFFF {
        let line = entry.wrapping_sub(0x100);
        loop {
            let id = r32(memory, record);
            effects.call(
                memory,
                callee::FORMAT,
                &[line, FORMAT_ARCHIVE, id, SUFFIX_ARCHIVE],
            );
            let handle = r32(memory, GLOBAL_FILE_HANDLE);
            let opened = effects.call(memory, callee::OPEN_NAMED, &[line, handle]);
            let resolved = effects.call(memory, callee::ARCHIVE_ID, &[opened]);
            if (resolved as i32) > 0 {
                w32(memory, record, resolved);
            } else {
                effects.call(memory, callee::ARCHIVE_ADD, &[opened, record, record]);
            }
            let next = r32(memory, record.wrapping_add(0x10));
            record = record.wrapping_add(0x10);
            if next == 0xFFFF_FFFF {
                break;
            }
        }
    }
    let saved = r32(memory, handle_cell);
    effects.call(memory, callee::MODEL_FINISH, &[saved, mode, archive])
}

/// `0x8E590(name, mode)`: loads a plain model file: cache hit (`0x631C0`, file at `+0x10`), else `0x62C30(name, 0)`
/// then `0x62F00(name, 1, 0)`; finishes with `0x8E420(ecx = mode, edi = file)(cache entry or 0)`. MEASURED.
pub fn model_load_b(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    name: u32,
    mode: u32,
) -> u32 {
    let cached = effects.call(memory, callee::CACHE_LOOKUP, &[name]);
    if cached != 0 {
        let file = r32(memory, cached.wrapping_add(0x10));
        return effects.call(memory, callee::MODEL_FINISH, &[cached, mode, file]);
    }
    let mut file = effects.call(memory, callee::FIND_FILE, &[name, 0]);
    if file == 0 {
        file = effects.call(memory, callee::LOAD_FILE, &[name, 1, file]);
    }
    effects.call(memory, callee::MODEL_FINISH, &[0, mode, file])
}

/// `0x8E3D0(value)`: stores the argument at `[0x4D9704]`.
pub fn model_flush(memory: &mut dyn GuestMemory, value: u32) {
    w32(memory, GLOBAL_SHAPE_CALLBACK, value);
}

/// `0x97C00(object, owner)` -> shape slot (`0x200` byte records at `[0x70AADC]`, `[0x7B67D0]` used, `[0x7B6984]`
/// recycled count): allocates the scratch block of `((owner.block.+4 << 6) >> 12) + 1` units (`0x3D6E0`, kept in
/// `[0x70AAD4]`), takes a free slot (first with `+4 == 0`, else a new one), initialises about 100 fields, fills the
/// shape from the owner (`0x965F0` for type 3, `0x97A00`, flag `0x4000000` of the owner), registers it (`0x8C910`),
/// sets the default rotation bytes `0x66` and frees the scratch block (`0x3D6B0`). MEASURED.
pub fn object_shape(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    object: u32,
    owner: u32,
) -> u32 {
    let block = r32(memory, owner.wrapping_add(4));
    let units = ((r32(memory, block.wrapping_add(4)) << 6) >> 12).wrapping_add(1);
    let scratch = effects.call(memory, callee::BLOCK_ALLOC, &[units]);
    w32(memory, GLOBAL_SHAPE_SCRATCH, scratch);
    let mut slot = 0u32;
    if r32(memory, GLOBAL_SHAPE_FREE) == 0 {
        let used = r32(memory, GLOBAL_SHAPE_USED);
        let base = r32(memory, GLOBAL_SHAPE_BASE);
        slot = (used << 9).wrapping_add(base);
        w32(memory, GLOBAL_SHAPE_USED, used.wrapping_add(1));
    } else {
        let base = r32(memory, GLOBAL_SHAPE_BASE);
        let used = r32(memory, GLOBAL_SHAPE_USED);
        let (mut scan, mut record) = (0u32, base);
        loop {
            if (scan as i32) >= (used as i32) {
                if slot != 0 {
                    let free = r32(memory, GLOBAL_SHAPE_FREE).wrapping_sub(1);
                    w32(memory, GLOBAL_SHAPE_FREE, free);
                } else {
                    slot = (used << 9).wrapping_add(base);
                    w32(memory, GLOBAL_SHAPE_USED, used.wrapping_add(1));
                    w32(memory, GLOBAL_SHAPE_FREE, 0);
                }
                break;
            }
            if r32(memory, record.wrapping_add(4)) == 0 {
                slot = record;
            }
            scan = scan.wrapping_add(1);
            record = record.wrapping_add(0x200);
            if slot != 0 {
                let free = r32(memory, GLOBAL_SHAPE_FREE).wrapping_sub(1);
                w32(memory, GLOBAL_SHAPE_FREE, free);
                break;
            }
        }
    }
    let at = |offset: u32| slot.wrapping_add(offset);
    let owner_block = r32(memory, owner.wrapping_add(4));
    w32(memory, at(0xC), owner_block);
    w32(memory, at(4), owner_block);
    w32(memory, at(8), 0);
    w32(memory, at(0), object);
    for offset in [0x1C, 0x98, 0xD4] {
        w32(memory, at(offset), 0);
    }
    let word = sx16(memory, GLOBAL_SHAPE_WORD);
    w32(memory, at(0xD8), word);
    for offset in [0xDC, 0xE0, 0x108] {
        wf(memory, at(offset), 0.0);
    }
    for offset in [0x124, 0x128, 0x12C] {
        w32(memory, at(offset), 0);
    }
    let word = sx16(memory, GLOBAL_SHAPE_WORD);
    w32(memory, at(0x130), word);
    for offset in [0x134, 0x138, 0x160] {
        wf(memory, at(offset), 0.0);
    }
    for offset in [0x17C, 0x180, 0x1A4] {
        w32(memory, at(offset), 0);
    }
    for offset in [0x198, 0x19C, 0x1A0, 0x190, 0x194, 0xAC] {
        wf(memory, at(offset), 0.0);
    }
    w32(memory, at(0xA4), 0);
    w32(memory, at(0xA8), 0);
    wf(memory, at(0xB8), 0.0);
    wf(memory, at(0xBC), 0.0);
    w32(memory, at(0xD0), 1);
    for offset in [0xC8, 0xCC, 0x184] {
        wf(memory, at(offset), 0.0);
    }
    w32(memory, at(0x18C), 0xFFFF_FFFF);
    for offset in [0x1A8, 0x1AC, 0x1B0, 0x1B4] {
        w32(memory, at(offset), 0);
    }
    wf(memory, at(0x188), 0.0);
    for offset in [
        0x1BC, 0x1C8, 0x1D4, 0x1E0, 0x1C0, 0x1CC, 0x1D8, 0x1E4, 0x1C4, 0x1D0, 0x1DC, 0x1E8,
    ] {
        w32(memory, at(offset), 0);
    }
    memory.write_u8(at(0x1F4), 0);
    memory.write_u8(at(0x1F5), 0);
    w32(memory, at(0x1B8), 0);
    w32(memory, at(0x1EC), 0);
    w32(memory, at(0x1F0), 0);
    w16(memory, at(0x1F6), 0);
    memory.write_u8(at(0x1F8), 0xFF);
    memory.write_u8(at(0x1F9), 0xFF);
    memory.write_u8(at(0x1FA), 0);
    memory.write_u8(at(0x87), 0);
    memory.write_u8(at(0x1FB), 0);
    memory.write_u8(at(0x1FC), 0);
    memory.write_u8(at(0x1FD), 0);
    w16(memory, at(0x7C), 0xFFFF);
    let owner_block = r32(memory, owner.wrapping_add(4));
    let kind = r32(memory, owner.wrapping_add(0x1C));
    if r32(memory, owner_block.wrapping_add(4)) == 0 || kind == 0xFFFF_FFFF {
        w32(memory, at(0x10), 0);
    } else {
        let own = r32(memory, at(4));
        let count = r32(memory, own.wrapping_add(4));
        let scratch = r32(memory, GLOBAL_SHAPE_SCRATCH);
        w32(memory, at(0x10), (count << 7).wrapping_add(scratch));
        for offset in [0x20, 0x24, 0x28, 0x2C] {
            w32(memory, at(offset), 0);
        }
    }
    if kind == 0xFFFF_FFFF || kind == 2 {
        w32(memory, at(0x88), 0);
        w32(memory, at(0x8C), 0);
    } else if kind == 3 {
        effects.call(memory, callee::SHAPE_MESH, &[slot, owner]);
    } else if r32(memory, owner.wrapping_add(0x28)) & 0x400_0000 != 0 {
        effects.call(memory, callee::SHAPE_FILL, &[slot, owner, 0]);
    } else {
        effects.call(memory, callee::SHAPE_FILL, &[slot, owner, 1]);
        let flags = r32(memory, owner.wrapping_add(0x28)) | 0x400_0000;
        w32(memory, owner.wrapping_add(0x28), flags);
    }
    let level = r32(memory, GLOBAL_LEVEL);
    if (level as i32) <= 0x64 && r32(memory, object.wrapping_add(0x20)) == 2 && level == 0x64 {
        let own = r32(memory, at(4));
        effects.call(
            memory,
            callee::SHAPE_BOUNDS,
            &[own, 0, 0x1D, 0xFFFF_FFFF, 0],
        );
    }
    effects.call(memory, callee::SHAPE_REGISTER, &[slot]);
    let own = r32(memory, at(4));
    let wide = r32(memory, own.wrapping_add(0xC)) & 0x100 != 0;
    for offset in [0x54, 0x58, 0x5C, 0x60, 0x64, 0x68] {
        wf(memory, at(offset), 0.0);
    }
    let constant = r32(memory, CONST_SHAPE_SCALE);
    let scratch = r32(memory, GLOBAL_SHAPE_SCRATCH);
    w32(memory, at(0x30), if wide { 0xFFFF_FFFF } else { 0 });
    w32(memory, at(0x1B8), 0);
    w32(memory, at(0x34), 0xFFFF_FFFF);
    w32(memory, at(0x38), 0xFFFF_FFFF);
    for offset in [0x80, 0x7F, 0x7E] {
        memory.write_u8(at(offset), 0x66);
    }
    w32(memory, at(0xC0), constant);
    w32(memory, at(0xC4), constant);
    effects.call(memory, callee::BLOCK_FREE, &[scratch]);
    w32(memory, GLOBAL_SHAPE_SCRATCH, 0);
    slot
}

/// `0x9FBE0(object)`: classifies an object for the level scripts. Levels `0x65` / `0x66` clear `+0x34` / `+0x38` /
/// byte `+0x30`. Kind 2 objects take `model id - 0x907`. Others with flag `0x20000` keep their state; a class above
/// 200 is kept; otherwise the object's box (centre `+0x64..` plus `+0x1A8..`, extent `+0x1C0..`) is queried
/// (`0x873C0(0, box, object + 0x38, 10)`, count in `al`) and `+0x34` takes the first queried class
/// unless it is already among them. `entry` is esp at entry (the box lives at `entry - 0x18`). MEASURED.
pub fn object_class(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    entry: u32,
    object: u32,
) {
    let level = r32(memory, GLOBAL_LEVEL);
    let at = |offset: u32| object.wrapping_add(offset);
    if level == 0x65 || level == 0x66 {
        w32(memory, at(0x34), 0);
        w32(memory, at(0x38), 0);
        memory.write_u8(at(0x30), 0);
        return;
    }
    if r32(memory, at(0x20)) == 2 {
        let value = r16(memory, at(0x94)).wrapping_add(0xFFFF_F6F9);
        w32(memory, at(0x38), value);
        w32(memory, at(0x34), value);
        memory.write_u8(at(0x30), 1);
        return;
    }
    if r32(memory, at(0x28)) & 0x2_0000 != 0 {
        return;
    }
    let class = r32(memory, at(0x34));
    if (class as i32) > 0xC8 {
        w32(memory, at(0x38), class);
        memory.write_u8(at(0x30), 1);
        return;
    }
    let box_base = entry.wrapping_sub(0x18);
    let (px, py, pz) = (
        rf(memory, at(0x64)),
        rf(memory, at(0x68)),
        rf(memory, at(0x6C)),
    );
    let (cx, cy, cz) = (
        rf(memory, at(0x1A8)),
        rf(memory, at(0x1AC)),
        rf(memory, at(0x1B0)),
    );
    let (ex, ey, ez) = (
        rf(memory, at(0x1C0)),
        rf(memory, at(0x1C4)),
        rf(memory, at(0x1C8)),
    );
    wf(memory, box_base, (px + cx) - ex);
    wf(memory, box_base.wrapping_add(0xC), (px + cx) + ex);
    wf(memory, box_base.wrapping_add(4), (py + cy) - ey);
    wf(memory, box_base.wrapping_add(0x10), (py + cy) + ey);
    wf(memory, box_base.wrapping_add(8), (cz + pz) - ez);
    wf(memory, box_base.wrapping_add(0x14), (cz + ez) + pz);
    let result = effects.call(memory, callee::SHAPE_QUERY, &[0, box_base, at(0x38), 0xA]);
    let count = result as u8;
    memory.write_u8(at(0x30), count);
    let mut found = false;
    let wanted = r32(memory, at(0x34));
    if (count as i8) > 0 {
        for index in 0..i32::from(memory.read_u8(at(0x30)) as i8) {
            if r32(memory, at(0x38).wrapping_add(4 * index as u32)) == wanted {
                found = true;
                break;
            }
        }
    }
    let current = r32(memory, at(0x34));
    let take = if current != 0 {
        !found
    } else if (count as i8) > 0 {
        true
    } else {
        !found
    };
    if take {
        let first = r32(memory, at(0x38));
        w32(memory, at(0x34), first);
    }
}

/// `0xA6530(object)` -> eax (the callback result or a leftover register): computes the object's local bounding box.
/// Needs a known model (`0x83790`), the object's own callback `[object+0x180](object, 0)` takes over when set.
/// Else the box corners (`0x9EB60`, `0x9E840` with the world matrix `0xA14B0`) are scaled by `0xBE9E0` for kind
/// `0x40`, stored at `+0x1A8` (centre) / `+0x1C0` (half extent), and with `[object+0x1E4]` child records
/// (pointers at `[object+0x1F4]`, each box in the record's first dword) the extents are merged and scaled by
/// `[0x475CD4]`. The original also leaves the max y at `entry - 0x48` and the last child y at the argument slot
/// `entry + 4`; both are written. `entry` is esp at entry (box locals at `entry - 0x64`). MEASURED.
pub fn object_place(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    entry: u32,
    object: u32,
) -> u32 {
    let at = |offset: u32| object.wrapping_add(offset);
    let model = r16(memory, at(0x94));
    let known = effects.call(memory, callee::MODEL_KNOWN, &[model]);
    if known == 0 {
        return 0;
    }
    if r32(memory, at(0x28)) & 0x20_0000 == 0 {
        let first = r32(memory, at(4));
        let second = r32(memory, first.wrapping_add(4));
        if (r32(memory, second.wrapping_add(4)) as i32) > 0 {
            let dynamic = effects.call(memory, callee::IS_DYNAMIC, &[object]);
            if dynamic != 0 {
                return dynamic;
            }
            let flags = r32(memory, at(0x28));
            if flags & 0x100 == 0 {
                return flags;
            }
        }
    }
    let blocked = effects.call(memory, pool_callee::DISPATCH, &[object, 5, 0]);
    if blocked != 0 {
        return blocked;
    }
    let hook = r32(memory, at(0x180));
    if hook != 0 {
        return effects.call(memory, hook, &[object, 0]);
    }
    let low = entry.wrapping_sub(0x64);
    let high = entry.wrapping_sub(0x58);
    let matrix = entry.wrapping_sub(0x40);
    effects.call(memory, callee::BOX_CORNERS, &[object, high, low]);
    effects.call(memory, callee::WORLD_MATRIX, &[matrix, object, 0, 0]);
    effects.call(memory, callee::BOX_TRANSFORM, &[matrix, high, low]);
    if r32(memory, at(0x7C)) != 0 && r32(memory, at(0x20)) == 0x40 {
        let factor = f32::from_bits(effects.call(memory, callee::SCALE_FACTOR, &[object]));
        for index in 0..6u32 {
            let address = low.wrapping_add(4 * index);
            let value = rf(memory, address) * factor;
            wf(memory, address, value);
        }
    }
    let count = r32(memory, at(0x1E4));
    let [px, py, pz, hx, hy, hz] =
        [0, 4, 8, 0xC, 0x10, 0x14].map(|k| rf(memory, low.wrapping_add(k)));
    wf(memory, at(0x1A8), px);
    wf(memory, at(0x1AC), py);
    wf(memory, at(0x1B0), pz);
    wf(memory, at(0x1C0), hx);
    wf(memory, at(0x1C4), hy);
    wf(memory, at(0x1C8), hz);
    if (count as i32) <= 0 {
        return count;
    }
    let list = r32(memory, at(0x1F4));
    let (mut min_x, mut min_y, mut min_z) = (px - hx, py - hy, pz - hz);
    let (mut max_x, mut max_y, mut max_z) = (hx + px, hy + py, hz + pz);
    let mut last = 0;
    for index in 0..count {
        let record = r32(memory, list.wrapping_add(4u32.wrapping_mul(index)));
        last = record;
        let top = rf(memory, record.wrapping_add(0x18)) - rf(memory, at(0x68));
        let x = rf(memory, record.wrapping_add(8)) - rf(memory, at(0x64));
        let low_y = rf(memory, record.wrapping_add(0xC)) - rf(memory, at(0x68));
        let z = rf(memory, record.wrapping_add(0x10)) - rf(memory, at(0x6C));
        wf(memory, entry.wrapping_add(4), top);
        if min_x > x {
            min_x = x;
        }
        if x > max_x {
            max_x = x;
        }
        if min_y > low_y {
            min_y = low_y;
        }
        if top > max_y {
            max_y = top;
        }
        if min_z > z {
            min_z = z;
        }
        if z > max_z {
            max_z = z;
        }
    }
    wf(memory, entry.wrapping_sub(0x48), max_y);
    let scale = rf(memory, CONST_BOX_SCALE);
    let half_x = (max_x - min_x) * scale;
    wf(memory, at(0x1C0), half_x);
    let centre_x = half_x + min_x;
    let half_y = (max_y - min_y) * scale;
    wf(memory, at(0x1C4), half_y);
    let half_z = (max_z - min_z) * scale;
    wf(memory, at(0x1C8), half_z);
    wf(memory, at(0x1A8), centre_x);
    let centre_y = rf(memory, at(0x1C4)) + min_y;
    wf(memory, at(0x1AC), centre_y);
    let centre_z = rf(memory, at(0x1C8)) + min_z;
    wf(memory, at(0x1B0), centre_z);
    last
}

/// `0xA67C0(object)` -> eax: clears the child list fields (`+0x1E0 +0x1E4 +0x1C +0x1E8 +0x1EC +0x1F0 +0x1F4 +0x1F8
/// +0x1FC +0x200`), runs the object setup chain (`0x9EF60`, and `0xA5F40`, `0xA6290`, `0x129920` unless a kind 2
/// object outside level `0x64`, `0xA64D0`, `0x12C1F0`), clears `+0xC` of each of the `+0x1E0` entries at `+0x1F0`
/// and stores the result of `0x128710` / `0x128320` at `+0x204`. MEASURED.
pub fn object_tail(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    object: u32,
) -> u32 {
    let at = |offset: u32| object.wrapping_add(offset);
    for offset in [
        0x1E0, 0x1E4, 0x1C, 0x1E8, 0x1EC, 0x1F0, 0x1F4, 0x1F8, 0x1FC, 0x200,
    ] {
        w32(memory, at(offset), 0);
    }
    effects.call(memory, callee::CLEAR_LISTS, &[object]);
    if r32(memory, at(0x20)) != 2 || r32(memory, GLOBAL_LEVEL) == 0x64 {
        effects.call(memory, callee::TAIL_A, &[object, 0]);
        effects.call(memory, callee::TAIL_B, &[object, 0]);
        effects.call(memory, callee::TAIL_C, &[object, 0]);
    }
    effects.call(memory, callee::TAIL_D, &[object]);
    effects.call(memory, callee::TAIL_E, &[object, 0]);
    let count = r32(memory, at(0x1E0));
    if (count as i32) > 0 {
        let mut index = 0u32;
        loop {
            let list = r32(memory, at(0x1F0));
            let child = r32(memory, list.wrapping_add(index.wrapping_mul(4)));
            w32(memory, child.wrapping_add(0xC), 0);
            let count = r32(memory, at(0x1E0));
            index = index.wrapping_add(1);
            if (index as i32) >= (count as i32) {
                break;
            }
        }
    }
    effects.call(memory, callee::TAIL_F, &[object]);
    let result = effects.call(memory, callee::TAIL_G, &[object]);
    w32(memory, at(0x204), result);
    result
}

/// `0x389B0()`: a bare `ret`.
pub fn report_nothing() {}

/// `0x129EC0(object)`: for each of the `+0x1C` entries at `[object+0x1F8]` (record pointers): sets the record's
/// float at `+0` to 0 when `word +0x28 & 0x40C` else to `[0x47D9D4]`, clears the two player cells selected by the
/// signed bytes at `+0xE` / `+0xF` (`0x7ABFA4 + 0x4C * byte = 0`, byte set to `0xFF`) and zeroes the entries of
/// the 4 element table `0x742360` (stride `0x6C`) that point at the record. MEASURED.
pub fn release_records(memory: &mut dyn GuestMemory, object: u32) {
    let mut count = r32(memory, object.wrapping_add(0x1C));
    let array = r32(memory, object.wrapping_add(0x1F8));
    if (count as i32) <= 0 {
        return;
    }
    let mut slot = array;
    let mut index = 0u32;
    loop {
        let record = r32(memory, slot);
        if r16(memory, record.wrapping_add(0x28)) & 0x40C != 0 {
            wf(memory, record, 0.0);
        } else {
            let value = r32(memory, CONST_RELEASE_FLOAT);
            w32(memory, record, value);
        }
        for offset in [0xE, 0xF] {
            let cell = record.wrapping_add(offset);
            let byte = memory.read_u8(cell) as i8;
            if byte >= 0 {
                w32(
                    memory,
                    0x7A_BFA4u32.wrapping_add((i32::from(byte) as u32).wrapping_mul(0x4C)),
                    0,
                );
                memory.write_u8(cell, 0xFF);
            }
        }
        let mut entry = 0x74_2360u32;
        while (entry as i32) < 0x74_2510 {
            if r32(memory, entry) != 0 && r32(memory, entry.wrapping_add(4)) == r32(memory, slot) {
                w32(memory, entry, 0);
            }
            entry = entry.wrapping_add(0x6C);
        }
        count = r32(memory, object.wrapping_add(0x1C));
        index = index.wrapping_add(1);
        slot = slot.wrapping_add(4);
        if (index as i32) >= (count as i32) {
            break;
        }
    }
}

/// `0x15BFF0(key)`: for each of the `[0x790950] + [0x7A2958]` player slots (records of `0xEA0` bytes at
/// `[0x73 56D8]`, from slot `[0x790950]`) with a live owner and `+4 == key` and the mode dependent flag word
/// (`+0x10` in mode `0xA`, else `+0x18`) bit 0 clear and the owner's class flag `0x20` clear: refreshes the link
/// (`0x277FE0` lookup, `0x15A540`) and drops it (`0x17570(slot, 0x10 / 8, 0)` or after the tests
/// `0x17510` / `0x174E0`). `entry` is esp at entry (the second flag word is written to `entry - 4`). MEASURED.
pub fn release_links(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    entry: u32,
    key: u32,
) {
    let first = r32(memory, GLOBAL_SLOT_COUNT);
    let end = r32(memory, GLOBAL_SLOT_EXTRA).wrapping_add(first);
    let mut slot_index = first;
    if (slot_index as i32) >= (end as i32) {
        return;
    }
    loop {
        let base = slot_index.wrapping_mul(0xEA0);
        let slot = r32(memory, GLOBAL_SLOT_TABLE).wrapping_add(base);
        'body: {
            if slot == 0 {
                break 'body;
            }
            let owner = r32(memory, slot);
            if owner == 0 || r32(memory, slot.wrapping_add(4)) != key {
                break 'body;
            }
            let (flags, other) = if memory.read_u8(GLOBAL_MODE_BYTE) == 0xA {
                (
                    r32(memory, slot.wrapping_add(0x10)),
                    r32(memory, slot.wrapping_add(0x14)),
                )
            } else {
                (
                    r32(memory, slot.wrapping_add(0x18)),
                    r32(memory, slot.wrapping_add(0x1C)),
                )
            };
            w32(memory, entry.wrapping_sub(4), other);
            if flags & 1 != 0 {
                break 'body;
            }
            let record = r32(memory, owner.wrapping_add(4));
            let class_index = sx16(
                memory,
                GLOBAL_SHAPE_WORD
                    .wrapping_add(2u32.wrapping_mul(r32(memory, record.wrapping_add(0xD4)))),
            );
            if r32(
                memory,
                0x7D_E660u32.wrapping_add(class_index.wrapping_mul(8)),
            ) & 0x20
                != 0
            {
                break 'body;
            }
            let lookup = effects.call(memory, callee::LINK_LOOKUP, &[]);
            let session = r32(memory, 0x7B_0C48);
            let link = r32(memory, session.wrapping_add(0x14));
            let refreshed = effects.call(memory, callee::LINK_REFRESH, &[owner, link, 0]);
            w32(memory, slot.wrapping_add(4), refreshed);
            if lookup != 0 && slot == lookup {
                break 'body;
            }
            if effects.call(memory, callee::LINK_IS_LOCAL, &[]) != 0 {
                effects.call(memory, callee::LINK_DROP, &[slot, 0x10, 0]);
                break 'body;
            }
            if effects.call(memory, callee::LINK_TEST_A, &[slot, 0x448_8000, 0]) != 0 {
                break 'body;
            }
            if effects.call(memory, callee::LINK_TEST_B, &[slot, 0x100, 0]) != 0 {
                break 'body;
            }
            effects.call(memory, callee::LINK_DROP, &[slot, 8, 0]);
        }
        let end = r32(memory, GLOBAL_SLOT_EXTRA).wrapping_add(r32(memory, GLOBAL_SLOT_COUNT));
        slot_index = slot_index.wrapping_add(1);
        if (slot_index as i32) >= (end as i32) {
            break;
        }
    }
}

/// `0xD2D50(key)`: clears every reference to `key` in the four link cells (`+0xF78 +0xF7C +0xF84 +0xFF4`) of
/// each of the `[0x790950]` session records (stride `0x1584`) at `[0x7B0C48]`. MEASURED.
pub fn release_session_keys(memory: &mut dyn GuestMemory, key: u32) {
    let mut table = r32(memory, 0x7B_0C48);
    if table == 0 {
        return;
    }
    let count = r32(memory, GLOBAL_SLOT_COUNT);
    if (count as i32) <= 0 {
        return;
    }
    let mut offset = 0u32;
    let mut index = 0u32;
    loop {
        for cell in [0xF78u32, 0xF7C, 0xF84, 0xFF4] {
            let address = offset.wrapping_add(table).wrapping_add(cell);
            if r32(memory, address) == key {
                w32(memory, address, 0);
                table = r32(memory, 0x7B_0C48);
            }
        }
        let count = r32(memory, GLOBAL_SLOT_COUNT);
        index = index.wrapping_add(1);
        offset = offset.wrapping_add(0x1584);
        if (index as i32) >= (count as i32) {
            break;
        }
    }
}

/// `0x227920(key)`: in network modes (mode byte `0xA` or 3) calls `0x2271F0(id, value, key)` for each of the
/// `[0x78CEC8]` ids at `[0x75C14C]` below `0x2D` (value: the `+0xC` word of the `0x4C` byte record at
/// `[0x78CECC]`). MEASURED.
pub fn release_hud(memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects, key: u32) {
    let mode = memory.read_u8(GLOBAL_MODE_BYTE);
    if mode != 0xA && mode != 3 {
        return;
    }
    if (r32(memory, 0x78_CEC8) as i32) <= 0 {
        return;
    }
    let list = r32(memory, 0x75_C14C);
    let mut index = 0u32;
    loop {
        let id = r32(memory, list.wrapping_add(4u32.wrapping_mul(index)));
        if (id as i32) < 0x2D {
            let records = r32(memory, 0x78_CECC);
            let value = r32(
                memory,
                id.wrapping_mul(0x4C)
                    .wrapping_add(records)
                    .wrapping_add(0xC),
            );
            effects.call(memory, callee::HUD_RELEASE, &[id, value, key]);
        }
        index = index.wrapping_add(1);
        if (index as i32) >= (r32(memory, 0x78_CEC8) as i32) {
            break;
        }
    }
}

/// `0x10E110(object)`: releases the object's attached spawn slots: each of the `+0x1E8` slots at `[object+0x1FC]`
/// either has its byte `+0x12` and `+0x88` cleared (slot flag `+8 & 2`) or goes to `0x10E070(slot)`; `+0x1E8` is
/// cleared; with flag `0x80000: ` of `+0x28` the global list `[0x73BD54]` (next at `+0xF4`) is walked the same way
/// for slots owned by the object (`+0x88`) without flag `0x100000`, and the flag is dropped. MEASURED.
pub fn release_slots(memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects, object: u32) {
    let at = |offset: u32| object.wrapping_add(offset);
    let release = |memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects, slot: u32| {
        if memory.read_u8(slot.wrapping_add(8)) & 2 != 0 {
            memory.write_u8(slot.wrapping_add(0x12), 0);
            w32(memory, slot.wrapping_add(0x88), 0);
        } else {
            effects.call(memory, callee::SPAWN_SLOT_RELEASE, &[slot]);
        }
    };
    if (r32(memory, at(0x1E8)) as i32) > 0 {
        let mut index = 0u32;
        loop {
            let list = r32(memory, at(0x1FC));
            let slot = r32(memory, list.wrapping_add(index.wrapping_mul(4)));
            release(memory, effects, slot);
            index = index.wrapping_add(1);
            if (index as i32) >= (r32(memory, at(0x1E8)) as i32) {
                break;
            }
        }
    }
    let flagged = r32(memory, at(0x28)) & 0x8_0000 != 0;
    w32(memory, at(0x1E8), 0);
    if !flagged {
        return;
    }
    let mut slot = r32(memory, 0x73_BD54);
    while slot != 0 {
        let next = r32(memory, slot.wrapping_add(0xF4));
        if r32(memory, slot.wrapping_add(0x88)) == object
            && r32(memory, slot.wrapping_add(4)) & 0x10_0000 == 0
        {
            release(memory, effects, slot);
        }
        slot = next;
    }
    let flags = r32(memory, at(0x28)) & 0xFFF7_FFFF;
    w32(memory, at(0x28), flags);
}

/// `0xE0520(object)`: tail jump to `0xE03D0` in mode `0xA`, else `0xE0300` (same argument). MEASURED.
pub fn release_physics(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    object: u32,
) {
    let target = if memory.read_u8(GLOBAL_MODE_BYTE) == 0xA {
        callee::PHYSICS_NETWORK
    } else {
        callee::PHYSICS_LOCAL
    };
    effects.call(memory, target, &[object]);
}

/// `0x1129B0(key)`: clears the entries keyed by `key` in the `[0x4E62DC]` records of `0xB8` bytes at `[0x73D950]`
/// (key at `+0x9C`: halfword `+0x96` set to `0xFFFF`, then `0x1123C0(record)`) and the 15 pointer cells from
/// `0x73BD9C` (stride `0x64`) whose target starts with `key`. MEASURED.
pub fn release_regions(memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects, key: u32) {
    let count = r32(memory, 0x4E_62DC);
    if (count as i32) > 0 {
        let mut cell = r32(memory, 0x73_D950).wrapping_add(0x96);
        for _ in 0..count {
            if r32(memory, cell.wrapping_add(6)) == key {
                let record = cell.wrapping_sub(0x96);
                w16(memory, cell, 0xFFFF);
                effects.call(memory, callee::REGION_RELEASE, &[record]);
            }
            cell = cell.wrapping_add(0xB8);
        }
    }
    let mut cell = 0x73_BD9Cu32;
    while (cell as i32) < 0x73_C378 {
        let target = r32(memory, cell);
        if target != 0 && r32(memory, target) == key {
            w32(memory, cell, 0);
        }
        cell = cell.wrapping_add(0x64);
    }
}

/// `0x8E9D0(shape)`: frees a shape slot: clears the entries of the `[0x70AAE0]` records of `0x24` bytes at
/// `0x7B69BC - 0x24 * k` (compare cell, `-8` and `+4` siblings) that point at it (`-8`, `0` and `+4 = -1`), clears `+4` / `+0x1B8`, counts a recycled slot in
/// `[0x7B6984]` and tail jumps to `0x56FF0(shape)`. MEASURED.
pub fn release_buffer(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    shape: u32,
) -> u32 {
    let last = r32(memory, 0x70_AAE0).wrapping_sub(1);
    if (last as i32) >= 0 {
        let mut cell = last.wrapping_mul(9).wrapping_mul(4).wrapping_add(0x7B_69BC);
        let mut remaining = last.wrapping_add(1);
        while remaining != 0 {
            if r32(memory, cell) == shape {
                w32(memory, cell.wrapping_sub(8), 0);
                w32(memory, cell, 0);
                w32(memory, cell.wrapping_add(4), 0xFFFF_FFFF);
            }
            cell = cell.wrapping_sub(0x24);
            remaining = remaining.wrapping_sub(1);
        }
    }
    w32(memory, shape.wrapping_add(4), 0);
    w32(memory, shape.wrapping_add(0x1B8), 0);
    let free = r32(memory, GLOBAL_SHAPE_FREE).wrapping_add(1);
    w32(memory, GLOBAL_SHAPE_FREE, free);
    effects.call(memory, callee::BUFFER_TAIL, &[shape])
}

/// `0x170ED0(object)` -> eax: stops and releases the voice `[object+0x80]`: looks its class up in the table at
/// `[0x74C458]` (`0x200` byte records, flags `+0x1C0`), `0x165FE0(voice, object)` and, with a model block and flag
/// `0x200000`, `0x153200(block.+0x10, block.+4.+4)`, then `0x16C300(voice)`, clears flag `0x8000` and `+0x80`;
/// unless the voice class has flag `0x10000` it detaches (`0xA5CE0(object, 2, 0)`, `(object, 7, 0)`) and removes
/// the object (`0x9E720(object, 0)`). MEASURED.
pub fn release_voice(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    object: u32,
) -> u32 {
    let at = |offset: u32| object.wrapping_add(offset);
    let voice = r32(memory, at(0x80));
    if voice == 0 {
        return 0;
    }
    let table = r32(memory, 0x74_C458);
    let class_flags = r32(
        memory,
        ((voice & 0xFFFF) << 9)
            .wrapping_add(table)
            .wrapping_add(0x1C0),
    );
    if (r32(memory, at(0x28)) as i32) >= 0 && r32(memory, at(4)) != 0 {
        effects.call(memory, callee::VOICE_PREPARE, &[voice, object]);
        let block = r32(memory, at(4));
        let head = r32(memory, block.wrapping_add(0x10));
        if head != 0 && r32(memory, at(0x28)) & 0x20_0000 != 0 {
            let inner = r32(memory, block.wrapping_add(4));
            let tail = r32(memory, inner.wrapping_add(4));
            effects.call(memory, callee::VOICE_DROP, &[head, tail]);
        }
    }
    let voice = r32(memory, at(0x80));
    let mut result = effects.call(memory, callee::VOICE_STOP, &[voice]);
    let flags = r32(memory, at(0x28)) & 0xFFFF_7FFF;
    w32(memory, at(0x28), flags);
    w32(memory, at(0x80), 0);
    if class_flags & 0x1_0000 == 0 {
        effects.call(memory, callee::OBJECT_DETACH, &[object, 2, 0]);
        effects.call(memory, callee::OBJECT_DETACH, &[object, 7, 0]);
        result = effects.call(memory, callee::OBJECT_REMOVE, &[object, 0]);
    }
    result
}

/// `0x1274A0(record)`: removes `record` from the table of `[0x74227C]` records of `0xC0` bytes at `[0x7AC6E4]`
/// by moving the following records down (`(9 - index) * 0xC0` bytes, forward dword copy) and decrementing the
/// count; the scan continues with the next index and the new count. A match at index above 9 would copy a huge
/// length in the original and is outside the contract (stops the scan). MEASURED.
pub fn release_emitter(memory: &mut dyn GuestMemory, record: u32) {
    let mut count = r32(memory, 0x74_227C);
    if (count as i32) <= 0 {
        return;
    }
    let (mut index, mut offset) = (0u32, 0u32);
    loop {
        let table = r32(memory, 0x7A_C6E4);
        let target = table.wrapping_add(offset);
        if target == record && record != 0 {
            if index > 9 {
                break;
            }
            let length = (9 - index) * 0xC0;
            let mut copied = 0;
            while copied < length {
                let value = r32(memory, target.wrapping_add(0xC0).wrapping_add(copied));
                w32(memory, target.wrapping_add(copied), value);
                copied += 4;
            }
            count = count.wrapping_sub(1);
        }
        index = index.wrapping_add(1);
        offset = offset.wrapping_add(0xC0);
        if (index as i32) >= (count as i32) {
            break;
        }
    }
    w32(memory, 0x74_227C, count);
}

/// `0xA0010(esi = object)`: resets the script state of an object: `+0x28 &= 0xBFFFFFFF`, `+0x12C = -1`, `+0x78 =
/// +0x128 = 0`, floats `+0x134..+0x148` to 0 and `0x153240(block.+0x10 + 0x40 * i)` for each of the model's
/// `block.+4.+4` parts. MEASURED.
pub fn release_script(memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects, object: u32) {
    let at = |offset: u32| object.wrapping_add(offset);
    let flags = r32(memory, at(0x28)) & 0xBFFF_FFFF;
    let block = r32(memory, at(4));
    w32(memory, at(0x12C), 0xFFFF_FFFF);
    w32(memory, at(0x78), 0);
    for offset in [0x134, 0x138, 0x13C, 0x140, 0x144, 0x148] {
        wf(memory, at(offset), 0.0);
    }
    w32(memory, at(0x128), 0);
    w32(memory, at(0x28), flags);
    if block == 0 || r32(memory, block.wrapping_add(0x10)) == 0 {
        return;
    }
    let inner = r32(memory, block.wrapping_add(4));
    if (r32(memory, inner.wrapping_add(4)) as i32) <= 0 {
        return;
    }
    let mut index = 0u32;
    let mut offset = 0u32;
    loop {
        let own = r32(memory, at(4));
        let part = r32(memory, own.wrapping_add(0x10)).wrapping_add(offset);
        effects.call(memory, callee::HITBOX_CLEAR, &[part]);
        let own = r32(memory, at(4));
        let inner = r32(memory, own.wrapping_add(4));
        let parts = r32(memory, inner.wrapping_add(4));
        index = index.wrapping_add(1);
        offset = offset.wrapping_add(0x40);
        if (index as i32) >= (parts as i32) {
            break;
        }
    }
}

/// `0x45460(key)`: with the key bit `0x20000000` marks every entry (stride `0x40` from `0x6D58A4`, 25 entries)
/// whose key at `-0xC` equals it as stopped (`+0 = 1`, calling `0x273E0(key)` first when it was 4); without the
/// bit it calls `0x273E0(key)` once. MEASURED.
pub fn release_sound(memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects, key: u32) {
    if key & 0x2000_0000 == 0 {
        effects.call(memory, callee::SOUND_RELEASE, &[key]);
        return;
    }
    let mut entry = 0x6D_58A4u32;
    loop {
        if r32(memory, entry.wrapping_sub(0xC)) == key {
            if r32(memory, entry) == 4 {
                effects.call(memory, callee::SOUND_RELEASE, &[key]);
            }
            w32(memory, entry, 1);
        }
        entry = entry.wrapping_add(0x40);
        if (entry as i32) >= 0x6D_5EE4 {
            break;
        }
    }
}

/// `0x240410(handle)`: releases the key list entry of an object handle: finds the object (`[0x72F5E8]`, slot in
/// the top 10 bits, handle at `+0x2C` must match), reads the key `+0x60` (positive and not `0x3F3F3F3F`), finds
/// its list node (`0x23FF70`, -1 when none), frees the node (`0x10` bytes at `0x75DAF0`, count `[0x75EA90]`
/// decremented, head `[0x75D318]` moved when it was the node) and unlinks it from the doubly linked list through
/// the pointers at `+8` / `+0xC`. MEASURED.
pub fn release_key_node(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    handle: u32,
) {
    if handle == 0 {
        return;
    }
    let object = (handle >> 0x16)
        .wrapping_mul(0x2E8)
        .wrapping_add(r32(memory, GLOBAL_POOL_BASE));
    if r32(memory, object.wrapping_add(0x2C)) != handle {
        return;
    }
    let key = r32(memory, object.wrapping_add(0x60));
    if (key as i32) <= 0 || key == 0x3F3F_3F3F {
        return;
    }
    let node = effects.call(memory, callee::KEY_CLEAR, &[key]);
    if node == 0xFFFF_FFFF {
        return;
    }
    let count = r32(memory, 0x75_EA90).wrapping_sub(1);
    let offset = node << 4;
    let cell = offset.wrapping_add(0x75_DAF0);
    w32(memory, 0x75_EA90, count);
    let is_head = r32(memory, 0x75_D318) == cell;
    w32(memory, offset.wrapping_add(0x75_DAF4), 0);
    w32(memory, cell, 0);
    if is_head {
        let next = r32(memory, offset.wrapping_add(0x75_DAF8));
        w32(memory, 0x75_D318, next);
        if next != 0 {
            w32(memory, next.wrapping_add(0xC), 0);
        }
    }
    let previous = r32(memory, offset.wrapping_add(0x75_DAFC));
    if previous != 0 {
        let next = r32(memory, offset.wrapping_add(0x75_DAF8));
        w32(memory, previous.wrapping_add(8), next);
    }
    let next = r32(memory, offset.wrapping_add(0x75_DAF8));
    if next != 0 {
        let previous = r32(memory, offset.wrapping_add(0x75_DAFC));
        w32(memory, next.wrapping_add(0xC), previous);
    }
}

/// `0x2404D0(key)`: removes the first chain node whose key (`+0`) equals `key` from the 150 chains at
/// `0x75CE68` (8 byte heads `(id, first)`, nodes `(key, next)`): unlinks it (an emptied chain gets id -1 and the
/// chain count `[0x75CE60]` decrements), clears the node and decrements the node count `[0x75CE64]`. MEASURED.
pub fn release_key_chain(memory: &mut dyn GuestMemory, key: u32) {
    let mut head = 0x75_CE68u32;
    loop {
        'chain: {
            if r32(memory, head) == 0 {
                break 'chain;
            }
            let mut node = r32(memory, head.wrapping_add(4));
            let mut previous = 0u32;
            while node != 0 {
                if r32(memory, node) == key {
                    if previous == 0 {
                        let next = r32(memory, node.wrapping_add(4));
                        if next != 0 {
                            w32(memory, head.wrapping_add(4), next);
                        } else {
                            let chains = r32(memory, 0x75_CE60).wrapping_sub(1);
                            w32(memory, head, 0xFFFF_FFFF);
                            w32(memory, head.wrapping_add(4), 0);
                            w32(memory, 0x75_CE60, chains);
                        }
                    } else {
                        let next = r32(memory, node.wrapping_add(4));
                        w32(memory, previous.wrapping_add(4), next);
                    }
                    w32(memory, node, 0);
                    w32(memory, node.wrapping_add(4), 0);
                    let nodes = r32(memory, 0x75_CE64).wrapping_sub(1);
                    w32(memory, 0x75_CE64, nodes);
                    return;
                }
                previous = node;
                node = r32(memory, node.wrapping_add(4));
            }
        }
        head = head.wrapping_add(8);
        if (head as i32) >= 0x75_D318 {
            return;
        }
    }
}

/// Runs the ported pool support for the callee addresses of `0xA6880` and `0xA7090` and forwards the rest to
/// `inner`. `entry` is the esp at entry of the running `0xA6880` (return address slot): the callees with a frame
/// see `entry - 0x1C` (`0x96410`, `0x8E4B0`) and `entry - 0x18` (`0xA6530`, `0x9FBE0`), MEASURED from the esp at
/// their call sites. For a running `0xA7090` pass its entry: `0x15BFF0` sees `entry - 0x1C` (its only frame
/// local is a dead store; nested destroy frames of the recursion reuse the depth 0 address).
pub struct PortedPool<'a> {
    pub inner: &'a mut dyn DestroyEffects,
    pub entry: u32,
}

impl DestroyEffects for PortedPool<'_> {
    fn call(&mut self, memory: &mut dyn GuestMemory, address: u32, args: &[u32]) -> u32 {
        use pool_callee as c;
        let arg = |index: usize| args.get(index).copied().unwrap_or(0);
        let entry = self.entry;
        match address {
            c::POOL_ALLOC => return pool_alloc(memory, self),
            c::MODEL_FIND => {
                return model_find(memory, self, entry.wrapping_sub(0x1C), arg(0), arg(1));
            }
            c::MODEL_LOAD_A => {
                return model_load_a(memory, self, entry.wrapping_sub(0x1C), arg(0), arg(1));
            }
            c::MODEL_LOAD_B => return model_load_b(memory, self, arg(0), arg(1)),
            c::MODEL_FLUSH => model_flush(memory, arg(0)),
            c::OBJECT_SHAPE => return object_shape(memory, self, arg(0), arg(1)),
            c::OBJECT_PLACE => return object_place(memory, self, entry.wrapping_sub(0x18), arg(0)),
            c::OBJECT_CLASS_INIT => object_class(memory, self, entry.wrapping_sub(0x18), arg(0)),
            c::OBJECT_TAIL_A => return object_tail(memory, self, arg(0)),
            c::OBJECT_TAIL_B => report_nothing(),
            c::DETACH_A => release_records(memory, arg(0)),
            c::DETACH_B => release_links(memory, self, entry.wrapping_sub(0x1C), arg(0)),
            c::DETACH_C => release_session_keys(memory, arg(0)),
            c::DETACH_D => release_hud(memory, self, arg(0)),
            c::DETACH_E => release_slots(memory, self, arg(0)),
            c::PHYSICS_RELEASE => release_physics(memory, self, arg(0)),
            c::SHAPE_RELEASE => release_regions(memory, self, arg(0)),
            c::BUFFER_FREE => return release_buffer(memory, self, arg(0)),
            c::VOICE_RELEASE => return release_voice(memory, self, arg(0)),
            c::EMITTER_RELEASE => release_emitter(memory, arg(0)),
            c::SCRIPT_RELEASE => release_script(memory, self, arg(0)),
            c::HANDLE_RELEASE => release_sound(memory, self, arg(0)),
            c::KEY_RELEASE_A => release_key_node(memory, self, arg(0)),
            c::KEY_RELEASE_B => release_key_chain(memory, arg(0)),
            _ => return self.inner.call(memory, address, args),
        }
        0
    }
}
