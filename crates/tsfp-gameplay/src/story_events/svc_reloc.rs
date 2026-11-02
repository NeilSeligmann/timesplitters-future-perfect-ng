//! T1869: the table relocators and registrars the model loaders call below `0x96330` / `0x8E420` / `0x8DF60`
//! (`0x8E3E0`, `0x8E310`, `0x637E0` with `0x63770` and `0x62EA0`, `0x8DD10` with `0x8DC70`, `0x18FB10`, `0x18390`
//! with `0x180E0`) and the small shape services `0x6A540` and `0x1B99B0`. MEASURED against the Unicorn oracle
//! (`docs/t-story-event-relocators-t1869.md`); names are INFERRED, addresses, offsets and control flow MEASURED.
//! The hardware below stays behind [`DestroyEffects`]: the formatter `0x3C85D4`, the host stream seek `0x290F0`,
//! the texture creation `0x3E6684` and `0x3D4D70`, the texture slot query `0x94620`.

use super::strip_update::GuestMemory;
use super::svc_objects::DestroyEffects;
use super::svc_resource::{PAK_COUNT, PAK_CURRENT_SLOT, crc32_string, pak_find, pak_name};

pub mod callee {
    pub const FORMAT: u32 = 0x3C_85D4; // E cdecl (buffer, format, a, b)
    pub const OPEN_NAMED: u32 = 0x6_24C0; // P through the resource layer
    pub const ARCHIVE_ID: u32 = 0x1_8380; // P
    pub const ARCHIVE_ADD: u32 = 0x1_8A30; // P
    pub const STREAM_SEEK: u32 = 0x2_90F0; // E (handle, offset)
    pub const TEXTURE_CREATE: u32 = 0x3E_6684; // E stdcall, nine arguments
    pub const TEXTURE_BIND: u32 = 0x3D_4D70; // E stdcall (record, descriptor + 0x80)
    pub const TEXTURE_PRESENT: u32 = 0x9_4620; // E (table, index) -> non zero when the slot has data
}

const FORMAT_ARCHIVE: u32 = 0x47_833C;
const FORMAT_SUFFIX: u32 = 0x47_834C;
const FORMAT_STREAM: u32 = 0x47_87B4;
const GLOBAL_FILE_HANDLE: u32 = 0x6D_7878;
const STREAM_COUNT: u32 = 0x6D_C2D8;
const STREAM_RECORDS: u32 = 0x6D_C2E0;
const STREAM_HEAD: u32 = 0x6D_7C80;
const STREAM_STRIDE: u32 = 0x1C;
const LIGHT_SLOT_TABLE: u32 = 0x51_5FFC;
const LEVEL: u32 = 0x79_094C;
const WEAPON_INDEX: u32 = 0x4F_A138;
const WEAPON_POOL: u32 = 0x4F_9BAC;
const TEXTURE_REGISTRY: u32 = 0x4B_85C8;

fn r32(memory: &dyn GuestMemory, address: u32) -> u32 {
    memory.read_u32(address)
}

fn w32(memory: &mut dyn GuestMemory, address: u32, value: u32) {
    memory.write_u32(address, value);
}

fn r16(memory: &dyn GuestMemory, address: u32) -> u32 {
    u32::from(memory.read_u8(address)) | u32::from(memory.read_u8(address.wrapping_add(1))) << 8
}

fn w16(memory: &mut dyn GuestMemory, address: u32, value: u32) {
    memory.write_u8(address, value as u8);
    memory.write_u8(address.wrapping_add(1), (value >> 8) as u8);
}

/// `base + value` for a non zero offset, 0 stays 0 (the relocation idiom of every function here).
fn rebase(value: u32, base: u32) -> u32 {
    if value != 0 {
        value.wrapping_add(base)
    } else {
        0
    }
}

/// `0x8E3E0(records, base)`: the 16 byte name records (ended by id `-1`): an id other than `0x40000000` whose record
/// flags (`+0xC`) have `0x10000000` is rebased onto `base`. MEASURED.
pub fn relocate_names(memory: &mut dyn GuestMemory, records: u32, base: u32) {
    let mut record = records;
    if r32(memory, record) == 0xFFFF_FFFF {
        return;
    }
    loop {
        let id = r32(memory, record);
        if id != 0x4000_0000 && r32(memory, record.wrapping_add(0xC)) & 0x1000_0000 != 0 {
            w32(memory, record, rebase(id, base));
        }
        let next = r32(memory, record.wrapping_add(0x10));
        record = record.wrapping_add(0x10);
        if next == 0xFFFF_FFFF {
            break;
        }
    }
}

/// `0x8E310` (eax = `index`, ecx = `records`; frame `0x100`, `entry` = esp at entry): resolves the archive id of the
/// record `index`: formats its id into the frame buffer (`entry - 0x100`), opens that archive entry (`0x624C0`), and
/// stores `0x18380(opened)` into the record when positive, else registers it (`0x18A30(opened, record, record)`).
/// MEASURED.
pub fn register_by_index(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    entry: u32,
    index: u32,
    records: u32,
) {
    let record = records.wrapping_add(index << 4);
    let id = r32(memory, record);
    let buffer = entry.wrapping_sub(0x100);
    effects.call(
        memory,
        callee::FORMAT,
        &[buffer, FORMAT_ARCHIVE, id, FORMAT_SUFFIX],
    );
    let handle = r32(memory, GLOBAL_FILE_HANDLE);
    let opened = effects.call(memory, callee::OPEN_NAMED, &[buffer, handle]);
    let resolved = effects.call(memory, callee::ARCHIVE_ID, &[opened]);
    if (resolved as i32) > 0 {
        w32(memory, record, resolved);
    } else {
        effects.call(memory, callee::ARCHIVE_ADD, &[opened, record, record]);
    }
}

/// `0x62EA0(name)`: the stream position of a pak entry: nothing mounted gives 0, the name (device prefix cut) is
/// hashed and found over the slots (`0x62B70`), then, for a file backed slot (handle `!= -1`), the host seek
/// `0x290F0(handle, entry offset)` answers; a resident slot or a miss gives 0. MEASURED.
pub fn stream_open(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    name: u32,
) -> u32 {
    if r32(memory, PAK_COUNT) == 0 {
        return 0;
    }
    let hashed = pak_name(memory, name);
    let key = crc32_string(memory, hashed);
    let entry = pak_find(memory, key);
    if entry == 0 {
        return 0;
    }
    let slot = r32(memory, PAK_CURRENT_SLOT);
    let handle = r32(memory, slot);
    if handle == 0xFFFF_FFFF {
        return 0;
    }
    let offset = r32(memory, entry.wrapping_add(4));
    effects.call(memory, callee::STREAM_SEEK, &[handle, offset])
}

/// `0x63770` (ecx = `record`): inserts a stream record into the list at `[0x6D7C80]`, ascending by the signed key
/// `+0xC` (an equal key goes in front of the existing one); `+0x18` is the next pointer. MEASURED.
pub fn stream_insert(memory: &mut dyn GuestMemory, record: u32) {
    let head = r32(memory, STREAM_HEAD);
    if head == 0 {
        w32(memory, STREAM_HEAD, record);
        return;
    }
    let key = r32(memory, record.wrapping_add(0xC)) as i32;
    if key <= r32(memory, head.wrapping_add(0xC)) as i32 {
        w32(memory, record.wrapping_add(0x18), head);
        w32(memory, STREAM_HEAD, record);
        return;
    }
    let mut previous = head;
    let mut current = r32(memory, head.wrapping_add(0x18));
    while current != 0 {
        if key <= r32(memory, current.wrapping_add(0xC)) as i32 {
            w32(memory, record.wrapping_add(0x18), current);
            w32(memory, previous.wrapping_add(0x18), record);
            return;
        }
        previous = current;
        current = r32(memory, current.wrapping_add(0x18));
    }
    w32(memory, record.wrapping_add(0x18), 0);
    w32(memory, previous.wrapping_add(0x18), record);
}

/// `0x637E0(id, a, b)` (frame `0x80`, `entry` = esp at entry): appends the stream record `[0x6DC2D8]` (`0x1C` bytes
/// at `0x6DC2E0`): halfword tag 3, `+4` = id, `+0x10` = a, the formatted name (`entry - 0x80`) resolved by `0x62EA0`
/// into `+0xC`, `+0x14` = b, `+0x18` = 0, then the list insert `0x63770`. MEASURED.
pub fn register_id(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    entry: u32,
    id: u32,
    a: u32,
    b: u32,
) {
    let record = r32(memory, STREAM_COUNT)
        .wrapping_mul(STREAM_STRIDE)
        .wrapping_add(STREAM_RECORDS);
    w16(memory, record, 3);
    w32(memory, record.wrapping_add(4), id);
    w32(memory, record.wrapping_add(0x10), a);
    let buffer = entry.wrapping_sub(0x80);
    effects.call(
        memory,
        callee::FORMAT,
        &[buffer, FORMAT_STREAM, id, FORMAT_SUFFIX],
    );
    let position = stream_open(memory, effects, buffer);
    w32(memory, record.wrapping_add(0xC), position);
    w32(memory, record.wrapping_add(0x14), b);
    let count = r32(memory, STREAM_COUNT);
    w32(memory, record.wrapping_add(0x18), 0);
    w32(memory, STREAM_COUNT, count.wrapping_add(1));
    stream_insert(memory, record);
}

/// `0x6A540()`: the light record (stride `0x80` from `0x515FF8`, first word non zero marks a used record) whose
/// `+4` equals the level `[0x79094C]`, else 0. MEASURED.
pub fn light_slot(memory: &dyn GuestMemory) -> u32 {
    if r16(memory, LIGHT_SLOT_TABLE) == 0 {
        return 0;
    }
    let level = r32(memory, LEVEL);
    let mut at = LIGHT_SLOT_TABLE;
    let mut index = 0u32;
    loop {
        if r32(memory, at.wrapping_add(4)) == level {
            return (index << 7).wrapping_add(LIGHT_SLOT_TABLE - 4);
        }
        at = at.wrapping_add(0x80);
        index = index.wrapping_add(1);
        if r16(memory, at) == 0 {
            return 0;
        }
    }
}

/// `0x1B99B0(index)`: weapon class record field `+0x14` through the index tables (`0x4FA138`, stride `0x3C`, below
/// `0x46`; `[0x4F9BAC]`, stride `0x29C`, below `0x42`), else 0. MEASURED (also ported as
/// `svc_spawn_c1950::class_index_1b99b0`, T1856: the two agree on the golden).
pub fn weapon_class(memory: &dyn GuestMemory, index: u32) -> u32 {
    if !(0..0x46).contains(&(index as i32)) {
        return 0;
    }
    let class = r32(memory, index.wrapping_mul(0x3C).wrapping_add(WEAPON_INDEX)) as i32;
    if !(0..0x42).contains(&class) {
        return 0;
    }
    let pool = r32(memory, WEAPON_POOL);
    r32(
        memory,
        (class as u32)
            .wrapping_mul(0x29C)
            .wrapping_add(pool)
            .wrapping_add(0x14),
    )
}

/// `0x8DC70(row, base)`: rebases the five pointer cells of a table row (`+0` to `+0x10`) that are still below
/// `base` (a cell already at or above `base` is left alone, which makes the call idempotent); returns whether any
/// cell is non zero afterwards. MEASURED.
pub fn row_relocate(memory: &mut dyn GuestMemory, row: u32, base: u32) -> u32 {
    let mut any = false;
    for k in 0..5u32 {
        let at = row.wrapping_add(4 * k);
        let value = r32(memory, at);
        if value < base {
            w32(memory, at, rebase(value, base));
        }
    }
    for k in 0..5u32 {
        any |= r32(memory, row.wrapping_add(4 * k)) != 0;
    }
    u32::from(any)
}

/// `0x8DD10(table, base, names)`: relocates a model's table of `0x30` byte rows (ended by a row whose `+0x10` is
/// `-1`): the four pointer cells `+0x20..+0x2C` are rebased and checked with `0x8DC70` (row flag `4` when one holds
/// data), then the five link cells `+0 +4 +8 +0xC` (flag `8`, `0x10`, `0x20`, `0x40` select a list of `8` byte
/// entries at `+0x14` instead of a single index): every referenced name record gets `0x20` in byte `+0xE` (list) or
/// `0x200000` in `+0xC` (single index, skipped for a negative one; the third cell is skipped when row flag `0x100`
/// is set), a row of kind `0x13` (byte `+0x18`) gets flag `0x20000`. `names` is the name record array. MEASURED.
pub fn relocate_table(memory: &mut dyn GuestMemory, table: u32, base: u32, names: u32) {
    let mut row = table;
    if r32(memory, row.wrapping_add(0x10)) == 0xFFFF_FFFF {
        return;
    }
    loop {
        for offset in [0x20u32, 0x24, 0x28, 0x2C] {
            let cell = row.wrapping_add(offset);
            let value = rebase(r32(memory, cell), base);
            w32(memory, cell, value);
            if value != 0 && row_relocate(memory, value, base) != 0 {
                let flags = r32(memory, row.wrapping_add(0x1C)) | 4;
                w32(memory, row.wrapping_add(0x1C), flags);
            }
        }
        // (link cell, row flag selecting the list form, extra row flag that skips the single form)
        for (cell, flag, skip) in [
            (0u32, 8u32, 0u32),
            (4, 0x10, 0),
            (8, 0x20, 0x100),
            (0xC, 0x40, 0),
        ] {
            let at = row.wrapping_add(cell);
            let flags = r32(memory, row.wrapping_add(0x1C));
            if flags & flag != 0 {
                let list = rebase(r32(memory, at), base);
                w32(memory, at, list);
                let mut index = 0u32;
                if (r32(memory, list) as i32) > 0 {
                    let mut entry = list.wrapping_add(0x14);
                    loop {
                        let name = r32(memory, entry).wrapping_shl(4);
                        let target = names.wrapping_add(name).wrapping_add(0xE);
                        let byte = memory.read_u8(target) | 0x20;
                        memory.write_u8(target, byte);
                        index = index.wrapping_add(1);
                        entry = entry.wrapping_add(8);
                        if (index as i32) >= (r32(memory, list) as i32) {
                            break;
                        }
                    }
                }
            } else if flags & skip == 0 {
                let name = r32(memory, at);
                if (name as i32) >= 0 {
                    let target = names.wrapping_add(name << 4).wrapping_add(0xC);
                    let word = r32(memory, target) | 0x20_0000;
                    w32(memory, target, word);
                }
            }
        }
        if memory.read_u8(row.wrapping_add(0x18)) == 0x13 {
            let flags = r32(memory, row.wrapping_add(0x1C)) | 0x2_0000;
            w32(memory, row.wrapping_add(0x1C), flags);
        }
        let next = r32(memory, row.wrapping_add(0x40));
        row = row.wrapping_add(0x30);
        if next == 0xFFFF_FFFF {
            break;
        }
    }
}

/// `0x18FB10(file)`: classifies the three mesh command streams of every part of a loaded model (parts of `0xC8`
/// bytes ending at `file`, the stream pointers at `part + 0x60 + 4 * k`). A stream is a list of `10` byte commands:
/// byte `+3` (negative ends the list) is the command kind, word `+0` the row index into the texture rows (`0x30`
/// bytes from `[file + 0x14]`, negative skips), the result class is stored in the word `+8`. Kinds 0 to 4, 7 to 9 and
/// `0xB` choose the class from the row's fields `+4`, `+0xC` and the flags `+0x1C` (and the texture slot query
/// `0x94620` through `effects`), kinds 5 and 6 are left alone, any other kind clears word `+8` and word `+0`.
/// MEASURED.
pub fn classify_streams(memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects, file: u32) {
    let count = r32(memory, file) as i32;
    if count <= 0 {
        return;
    }
    let rows = r32(memory, file.wrapping_add(0x14));
    let mut base = file
        .wrapping_sub((count as u32).wrapping_mul(0xC8))
        .wrapping_add(0x60);
    let mut part = 0i32;
    loop {
        for slot in 0..3u32 {
            let mut command = r32(memory, base.wrapping_add(4 * slot));
            if command == 0 {
                continue;
            }
            while memory.read_u8(command.wrapping_add(3)) as i8 >= 0 {
                let index = r16(memory, command) as u16 as i16;
                if index >= 0 {
                    classify_command(memory, effects, command, rows, i32::from(index));
                }
                command = command.wrapping_add(10);
                if command == 0 {
                    break;
                }
            }
        }
        part += 1;
        base = base.wrapping_add(0xC8);
        // the part count is read from the file again every time round
        if part >= r32(memory, file) as i32 {
            break;
        }
    }
}

fn classify_command(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    command: u32,
    rows: u32,
    index: i32,
) {
    let kind = memory.read_u8(command.wrapping_add(3)) as i8;
    let row = rows.wrapping_add((index as u32).wrapping_mul(0x30));
    let flags = r32(memory, row.wrapping_add(0x1C));
    let field =
        |memory: &dyn GuestMemory, offset: u32| r32(memory, row.wrapping_add(offset)) as i32;
    let class = match kind {
        0 | 0xB => {
            if field(memory, 0xC) >= 0 {
                0x22
            } else if effects.call(memory, callee::TEXTURE_PRESENT, &[rows, index as u32]) != 0 {
                memory.write_u8(command.wrapping_add(3), 7);
                0x18
            } else if flags & 0x8000 != 0 {
                0x18
            } else if flags & 0x4030_0000 != 0 {
                blend_class(flags)
            } else if flags & 0x80_0000 != 0 {
                0x29
            } else {
                0
            }
        }
        1 | 2 => blend_class(flags),
        3 => {
            if field(memory, 4) >= 0 {
                0xF
            } else if flags & 0x80 != 0 {
                if flags & 0x100 != 0 { 7 } else { 8 }
            } else if field(memory, 0xC) >= 0 {
                0x23
            } else if effects.call(memory, callee::TEXTURE_PRESENT, &[rows, index as u32]) != 0
                || flags & 0x8000 != 0
            {
                0x2B
            } else if flags & 0x4030_0000 != 0 {
                if flags & 0x10_0000 != 0 {
                    0x34
                } else {
                    (flags >> 30) | 0x32
                }
            } else {
                3
            }
        }
        4 => {
            if field(memory, 4) >= 0 {
                0x10
            } else if flags & 0x80 != 0 {
                if flags & 0x100 != 0 { 0xA } else { 0xC }
            } else {
                4
            }
        }
        5 | 6 => return,
        7 => 0x18,
        8 => 1,
        9 => {
            0x29 + u32::from(
                effects.call(memory, callee::TEXTURE_PRESENT, &[rows, index as u32]) != 0,
            )
        }
        _ => {
            w16(memory, command.wrapping_add(8), 0);
            w16(memory, command, 0);
            return;
        }
    };
    w16(memory, command.wrapping_add(8), class);
}

/// Blend class of a texture row: `0x28` with flag `0x100000`, else `2` plus `0x25` with flag `0x40000000`.
fn blend_class(flags: u32) -> u32 {
    if flags & 0x10_0000 != 0 {
        0x28
    } else if flags & 0x4000_0000 != 0 {
        0x27
    } else {
        2
    }
}

fn ceil_pow2(value: u32) -> u32 {
    if value & value.wrapping_sub(1) == 0 {
        return value;
    }
    // the original loops forever on a negative value that is not a power of two (outside the contract)
    let mut bits = 0u32;
    let mut rest = value as i32;
    while rest != 0 && bits < 32 {
        rest >>= 1;
        bits += 1;
    }
    1u32.wrapping_shl(bits)
}

/// `0x180E0()`: takes a free texture record index from the registry `[0x4B85C8]` (`+0` records of `0x34` bytes,
/// `+8` stack top index, `+0xC` count): a non negative `+8` is returned and decremented, else the first record whose
/// flags (`+0x18`) have `0x800000` is claimed (flag cleared), `-1` when none. MEASURED.
pub fn registry_alloc(memory: &mut dyn GuestMemory) -> u32 {
    let registry = r32(memory, TEXTURE_REGISTRY);
    let top = r32(memory, registry.wrapping_add(8));
    if (top as i32) >= 0 {
        w32(memory, registry.wrapping_add(8), top.wrapping_sub(1));
        return top;
    }
    let count = r32(memory, registry.wrapping_add(0xC)) as i32;
    let records = r32(memory, registry);
    let mut index = 0i32;
    while index < count {
        let flags = records
            .wrapping_add((index as u32).wrapping_mul(0x34))
            .wrapping_add(0x18);
        let value = r32(memory, flags);
        if value & 0x80_0000 != 0 {
            w32(memory, flags, value & 0xFF7F_FFFF);
            return index as u32;
        }
        index += 1;
    }
    0xFFFF_FFFF
}

/// `0x18390` (edi = `descriptor`; stack `settings`, `out`; frame `0x20`, `entry` = esp at entry): registers a texture
/// descriptor (`+0` width, `+4` height, `+8` and `+0xC` depth-like extents, `+0x10` levels, `+0x14` format kind,
/// `+0x18` tag, `+0x1C`/`+0x20` floats). A negative width or height stores `-1` in `*out` and returns the width.
/// Otherwise a record is claimed (`0x180E0`), filled (`+0x18` settings `+0xC`, `+0x20`/`+0x21` the settings flag
/// bits `1`/`2`, `+0x22` format kind, `+0x23` tag byte, `+0x24`/`+0x28`/`+0x26`/`+0x2A` the extents rounded up to
/// powers of two, `+0x2C`/`+0x30` the floats), created through `0x3E6684` (stdcall: width and height rounded up,
/// levels + 1, the pixel format code of the kind table, the record, the raw width) and bound with `0x3D4D70`; the
/// index is stored in `*out` and `descriptor + 0x24` and returned. A format kind above 7 leaves the format code to
/// whatever the frame cell `entry - 0x20` holds. MEASURED.
pub fn texture_register(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    entry: u32,
    descriptor: u32,
    settings: u32,
    out: u32,
) -> u32 {
    let width = r32(memory, descriptor);
    let height = r32(memory, descriptor.wrapping_add(4));
    let extent_a = r32(memory, descriptor.wrapping_add(8));
    let extent_b = r32(memory, descriptor.wrapping_add(0xC));
    let levels = r32(memory, descriptor.wrapping_add(0x10));
    let kind = r32(memory, descriptor.wrapping_add(0x14));
    let tag = r32(memory, descriptor.wrapping_add(0x18));
    let float_a = r32(memory, descriptor.wrapping_add(0x1C));
    let float_b = r32(memory, descriptor.wrapping_add(0x20));
    const FORMATS: [u32; 8] = [0xC, 0xE, 6, 5, 0xF, 0x19, 0, 0x1A];
    // the format code lives in the frame cell `entry - 0x20` (written for a known kind, stale otherwise)
    if kind <= 7 {
        w32(memory, entry.wrapping_sub(0x20), FORMATS[kind as usize]);
    }
    let format = r32(memory, entry.wrapping_sub(0x20));
    if (width as i32) < 0 || (height as i32) < 0 {
        w32(memory, out, 0xFFFF_FFFF);
        return width;
    }
    let index = registry_alloc(memory);
    let registry = r32(memory, TEXTURE_REGISTRY);
    let record = index.wrapping_mul(0x34).wrapping_add(r32(memory, registry));
    if settings != 0 {
        let word = r32(memory, settings.wrapping_add(0xC));
        w32(memory, record.wrapping_add(0x18), word);
        let bits = memory.read_u8(settings.wrapping_add(8));
        memory.write_u8(record.wrapping_add(0x20), bits & 1);
        memory.write_u8(record.wrapping_add(0x21), bits & 2);
    } else {
        memory.write_u8(record.wrapping_add(0x21), 0);
        memory.write_u8(record.wrapping_add(0x20), 0);
        w32(memory, record.wrapping_add(0x18), 0);
    }
    memory.write_u8(record.wrapping_add(0x23), tag as u8);
    w32(memory, record.wrapping_add(0x2C), float_a);
    w32(memory, record.wrapping_add(0x30), float_b);
    memory.write_u8(record.wrapping_add(0x22), kind as u8);
    let wide = ceil_pow2(width);
    w16(memory, record.wrapping_add(0x24), wide);
    let high = ceil_pow2(height);
    w16(memory, record.wrapping_add(0x28), high);
    w16(memory, record.wrapping_add(0x26), ceil_pow2(extent_a));
    w16(memory, record.wrapping_add(0x2A), ceil_pow2(extent_b));
    effects.call(
        memory,
        callee::TEXTURE_CREATE,
        &[
            wide,
            high,
            levels.wrapping_add(1),
            0,
            format,
            0,
            record,
            0,
            width,
        ],
    );
    effects.call(
        memory,
        callee::TEXTURE_BIND,
        &[record, descriptor.wrapping_add(0x80)],
    );
    w32(memory, out, index);
    w32(memory, descriptor.wrapping_add(0x24), index);
    index
}

/// `0x94620(rows, index)`: whether the texture row `index` (`0x30` bytes from `rows`) holds any of its four data
/// pointers (`+0x20..+0x2C`); a negative index gives 0. MEASURED.
pub fn texture_present(memory: &dyn GuestMemory, rows: u32, index: u32) -> u32 {
    if (index as i32) < 0 {
        return 0;
    }
    let row = rows.wrapping_add(index.wrapping_mul(0x30));
    u32::from(
        (0x20..=0x2C)
            .step_by(4)
            .any(|offset| r32(memory, row.wrapping_add(offset)) != 0),
    )
}

/// The callee addresses [`PortedFiles`] runs for the relocators and registrars.
pub const RELOC_HANDLED: [u32; 7] = [
    super::svc_resource::callee::RELOCATE_NAMES,
    super::svc_resource::callee::REGISTER_BY_INDEX,
    super::svc_resource::callee::REGISTER_ID,
    super::svc_resource::callee::RELOCATE_TABLE,
    super::svc_resource::callee::BLOCK_READY,
    super::svc_resource::callee::ARCHIVE_STORE,
    callee::TEXTURE_PRESENT,
];

/// The whole file and model table layer in one dispatcher: [`super::svc_resource::PortedResource`] (T1866) and the
/// ported relocators and registrars of this module. They call each other (the resource layer relocates tables, the
/// registrars open archive entries), so they cannot be stacked: a call from either goes through this dispatcher
/// again. `frame` is the esp at entry of the registrars (`0x8E310`, `0x637E0`, `0x18390`): FABRICATED for a host
/// that has no real call stack (the frame buffers live below it), the goldens use the oracle frame. The hardware
/// below stays behind `inner`: formatter, host stream seek, texture creation and bind, allocators, host file API.
pub struct PortedFiles<'a> {
    pub inner: &'a mut dyn DestroyEffects,
    pub frame: u32,
}

/// Calls back into the owning [`PortedFiles`].
struct Back<'b, 'a>(&'b mut PortedFiles<'a>);

impl DestroyEffects for Back<'_, '_> {
    fn call(&mut self, memory: &mut dyn GuestMemory, address: u32, args: &[u32]) -> u32 {
        self.0.call(memory, address, args)
    }
}

impl DestroyEffects for PortedFiles<'_> {
    fn call(&mut self, memory: &mut dyn GuestMemory, address: u32, args: &[u32]) -> u32 {
        use super::svc_resource::callee as resource;
        let arg = |index: usize| args.get(index).copied().unwrap_or(0);
        let frame = self.frame;
        match address {
            resource::RELOCATE_NAMES => relocate_names(memory, arg(0), arg(1)),
            resource::REGISTER_BY_INDEX => register_by_index(memory, self, frame, arg(0), arg(1)),
            resource::REGISTER_ID => register_id(memory, self, frame, arg(0), arg(1), arg(2)),
            resource::RELOCATE_TABLE => relocate_table(memory, arg(0), arg(1), arg(2)),
            resource::BLOCK_READY => classify_streams(memory, self, arg(0)),
            resource::ARCHIVE_STORE => {
                return texture_register(memory, self, frame, arg(2), arg(0), arg(1));
            }
            callee::TEXTURE_PRESENT => return texture_present(memory, arg(0), arg(1)),
            _ if super::svc_resource::HANDLED.contains(&address) => {
                let mut back = Back(self);
                let mut resource = super::svc_resource::PortedResource { inner: &mut back };
                return resource.call(memory, address, args);
            }
            _ => return self.inner.call(memory, address, args),
        }
        0
    }
}
