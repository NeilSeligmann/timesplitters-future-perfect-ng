//! T1866: the resource and file layer below the model loaders: pak entry lookup (key = CRC32 of the name),
//! resident pak slots, file size/read/load by name, the loaded model cache and the pointer fix-up of a loaded
//! model file. MEASURED against the Unicorn oracle (`docs/t-story-event-resource-t1866.md`); names are INFERRED,
//! addresses, offsets and control flow MEASURED.
//!
//! Hardware and file system effects (`open`, `seek`, `read`, `close`, the inflater, the allocator and the "file
//! missing" report) go through [`DestroyEffects::call`] with the original addresses of [`callee`]. Routines that
//! take register arguments list them after the stack arguments, as the oracle stubs do.

use super::strip_update::GuestMemory;
use super::svc_objects::DestroyEffects;

/// Original addresses of the routines this module ports (`P`) or forwards as effects (`E`).
pub mod callee {
    pub const CRC32_BYTES: u32 = 0x15_04F0; // P
    pub const CRC32_STRING: u32 = 0x15_0530; // P
    pub const STRSTR: u32 = 0x3C_8690; // P (CRT)
    pub const PAK_SELECT: u32 = 0x6_2380; // P (edi = slot index, ebx = key)
    pub const PAK_FIND: u32 = 0x6_2B70; // P (eax = key)
    pub const FIND_FILE: u32 = 0x6_2C30; // P
    pub const OPEN_NAMED: u32 = 0x6_24C0; // P
    pub const FILE_SIZE: u32 = 0x6_2CA0; // P
    pub const FILE_READ: u32 = 0x6_2D20; // P
    pub const LOAD_FILE: u32 = 0x6_2F00; // P
    pub const FIND_OR_LOAD: u32 = 0x6_2F50; // P
    pub const BASE_NAME: u32 = 0x6_2FB0; // P
    pub const CACHE_LOOKUP: u32 = 0x6_31C0; // P
    pub const NOTE_MODEL: u32 = 0x6_B380; // P
    pub const FIXUP_FILE: u32 = 0x9_6330; // P
    pub const BIND_FILE: u32 = 0x8_DF60; // P
    pub const MODEL_FINISH: u32 = 0x8_E420; // P (ecx = mode, edi = file)
    pub const ARCHIVE_ID: u32 = 0x1_8380; // P
    pub const ARCHIVE_ADD: u32 = 0x1_8A30; // P
    pub const SLOT_NAME: u32 = 0x6_2260; // P
    pub const SLOT_ALLOC: u32 = 0x6_1FD0; // P
    pub const MOUNT_MEMORY: u32 = 0x6_2040; // P
    pub const FILE_OPEN: u32 = 0x2_96F0; // E open(name, 1) -> fd
    pub const FILE_SEEK: u32 = 0x2_9030; // E seek(fd, offset, whence) -> position
    pub const FILE_CLOSE: u32 = 0x2_9090; // E close(fd)
    pub const FILE_READ_RAW: u32 = 0x2_8D50; // E read(fd, buffer, size) -> bytes
    pub const INFLATE: u32 = 0x35_DE70; // E inflate(source, stored size, destination)
    pub const REPORT_MISSING: u32 = 0x3_89B0; // E
    pub const ALLOC_ALIGNED: u32 = 0x3_E0B0; // E alloc(0x80, size, alignment)
    pub const ALLOC_PLAIN: u32 = 0x3_DFA0; // E alloc(size, alignment)
    pub const RELOCATE_NAMES: u32 = 0x8_E3E0; // E
    pub const RELOCATE_TABLE: u32 = 0x8_DD10; // E
    pub const REGISTER_BY_INDEX: u32 = 0x8_E310; // E (eax = index, ecx = records)
    pub const REGISTER_ID: u32 = 0x6_37E0; // E
    pub const ARCHIVE_STORE: u32 = 0x1_8390; // E (edi = first argument)
    pub const BLOCK_READY: u32 = 0x18_FB10; // E
}

pub const PAK_COUNT: u32 = 0x6D_7880;
pub const PAK_SLOTS: u32 = 0x6D_7900;
pub const PAK_SLOT_STRIDE: u32 = 0x1C;
pub const PAK_MAX_SLOTS: u32 = 0x1E;
pub const PAK_CURRENT_SLOT: u32 = 0x6D_78EC;
pub const PAK_LAST_ENTRY: u32 = 0x6D_7C50;
pub const PAK_SKIP_SLOT: u32 = 0x4D_19EC;
pub const PAK_FORCED_SLOT: u32 = 0x4D_19F0;
pub const PAK_MRU_A: u32 = 0x4D_19F4;
pub const PAK_MRU_B: u32 = 0x4D_19F8;
pub const FILE_FLAGS: u32 = 0x7D_E398;
pub const LEVEL: u32 = 0x79_094C;
pub const CACHE_COUNT: u32 = 0x6F_0B00;
pub const CACHE_RECORDS: u32 = 0x6D_7C88;
pub const NOTED_COUNT: u32 = 0x6F_1E20;
pub const NOTED_ARRAY: u32 = 0x6F_1E18;
/// The device prefix that is cut from names before hashing (`strstr` anywhere in the name, then six bytes).
pub const HOST_PREFIX: &[u8] = b"host0:"; // image string at 0x47879C

fn r32(memory: &dyn GuestMemory, address: u32) -> u32 {
    memory.read_u32(address)
}

fn w32(memory: &mut dyn GuestMemory, address: u32, value: u32) {
    memory.write_u32(address, value);
}

fn r16(memory: &dyn GuestMemory, address: u32) -> i16 {
    (u16::from(memory.read_u8(address)) | u16::from(memory.read_u8(address.wrapping_add(1))) << 8)
        as i16
}

/// The CRC32 table of the original (`0x4E74B8`, reflected polynomial `0xEDB88320`).
pub fn crc32_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    for (index, slot) in table.iter_mut().enumerate() {
        let mut value = index as u32;
        for _ in 0..8 {
            value = if value & 1 != 0 {
                0xEDB8_8320 ^ (value >> 1)
            } else {
                value >> 1
            };
        }
        *slot = value;
    }
    table
}

/// `0x1504F0(bytes, length)`: init `0xFFFFFFFF`, final `not`; length 0 gives 0. MEASURED.
pub fn crc32_bytes(bytes: &[u8]) -> u32 {
    if bytes.is_empty() {
        return 0;
    }
    let table = crc32_table();
    let mut value = 0xFFFF_FFFFu32;
    for &byte in bytes {
        value = (value >> 8) ^ table[usize::from((value as u8) ^ byte)];
    }
    !value
}

/// Reads a NUL terminated string out of guest memory (the NUL is not included).
pub fn read_cstring(memory: &dyn GuestMemory, address: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut cursor = address;
    loop {
        let byte = memory.read_u8(cursor);
        if byte == 0 {
            return bytes;
        }
        bytes.push(byte);
        cursor = cursor.wrapping_add(1);
    }
}

/// `0x150530(name)`: CRC32 of the string. MEASURED.
pub fn crc32_string(memory: &dyn GuestMemory, name: u32) -> u32 {
    crc32_bytes(&read_cstring(memory, name))
}

/// `0x3C8690(haystack, needle)`: the CRT `strstr` (empty needle gives the haystack). MEASURED.
pub fn c_strstr(memory: &dyn GuestMemory, haystack: u32, needle: u32) -> u32 {
    strstr_bytes(memory, haystack, &read_cstring(memory, needle))
}

fn strstr_bytes(memory: &dyn GuestMemory, haystack: u32, pin: &[u8]) -> u32 {
    let hay = read_cstring(memory, haystack);
    if pin.is_empty() {
        return haystack;
    }
    match hay.windows(pin.len()).position(|window| window == pin) {
        Some(index) => haystack.wrapping_add(index as u32),
        None => 0,
    }
}

/// The name as hashed by the pak lookups: from just after a `host0:` prefix when the name contains one.
pub(super) fn pak_name(memory: &dyn GuestMemory, name: u32) -> u32 {
    let found = strstr_bytes(memory, name, HOST_PREFIX);
    if found != 0 {
        found.wrapping_add(6)
    } else {
        name
    }
}

/// `0x62380` (edi = slot `index`, ebx = `key`): entry with `key` in the resident table of one pak slot (16 byte
/// entries: key, offset, size, stored size) or 0. A hit becomes the current entry and slot and rotates the two
/// most recently used slot cells. The slot kept in `[0x4D19EC]` is never searched. MEASURED.
pub fn pak_select(memory: &mut dyn GuestMemory, index: u32, key: u32) -> u32 {
    let skip = r32(memory, PAK_SKIP_SLOT);
    let slot = index.wrapping_mul(PAK_SLOT_STRIDE);
    let mut entry = r32(memory, slot.wrapping_add(PAK_SLOTS + 0x10));
    if index == skip {
        return 0;
    }
    let count = r32(memory, slot.wrapping_add(PAK_SLOTS + 8)) as i32;
    for _ in 0..count.max(0) {
        if r32(memory, entry) == key {
            let previous = r32(memory, PAK_MRU_A);
            w32(memory, PAK_LAST_ENTRY, entry);
            w32(memory, PAK_MRU_B, previous);
            w32(memory, PAK_MRU_A, index);
            w32(memory, PAK_CURRENT_SLOT, slot.wrapping_add(PAK_SLOTS));
            return entry;
        }
        entry = entry.wrapping_add(16);
    }
    0
}

/// `0x62B70` (eax = `key`): entry of a key over all mounted paks: a forced slot (`[0x4D19F0]`, -2 disables the
/// lookup), else the last entry, the two most recently used slots, every slot in order, and last the skipped slot
/// `[0x4D19EC]` (restored afterwards). MEASURED.
pub fn pak_find(memory: &mut dyn GuestMemory, key: u32) -> u32 {
    let forced = r32(memory, PAK_FORCED_SLOT);
    if forced == 0xFFFF_FFFE {
        return 0;
    }
    if (forced as i32) >= 0 {
        return pak_select(memory, forced, key);
    }
    let last = r32(memory, PAK_LAST_ENTRY);
    if last != 0 && r32(memory, last) == key {
        return last;
    }
    let count = r32(memory, PAK_COUNT) as i32;
    for cell in [PAK_MRU_A, PAK_MRU_B] {
        let index = r32(memory, cell);
        if (index as i32) >= 0 && (index as i32) < count {
            let found = pak_select(memory, index, key);
            if found != 0 {
                return found;
            }
        }
    }
    let mut index = 0i32;
    while index < count {
        if index as u32 != r32(memory, PAK_MRU_A) && index as u32 != r32(memory, PAK_MRU_B) {
            let found = pak_select(memory, index as u32, key);
            if found != 0 {
                return found;
            }
        }
        index += 1;
    }
    let skipped = r32(memory, PAK_SKIP_SLOT);
    w32(memory, PAK_SKIP_SLOT, 0xFFFF_FFFF);
    let mut found = 0;
    if skipped != 0xFFFF_FFFF {
        found = pak_select(memory, skipped, key);
        w32(memory, PAK_SKIP_SLOT, skipped);
    }
    found
}

/// `0x62C30(name, out_size)`: the in memory image of a resident, uncompressed, 16 byte aligned pak entry found by
/// name (device prefix cut), else 0. Writes the entry size to `out_size` when given. MEASURED.
pub fn find_file(memory: &mut dyn GuestMemory, name: u32, out_size: u32) -> u32 {
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
    let base = r32(memory, slot.wrapping_add(0xC));
    if base == 0
        || r32(memory, entry.wrapping_add(0xC)) != 0
        || memory.read_u8(entry.wrapping_add(4)) & 0xF != 0
    {
        return 0;
    }
    if out_size != 0 {
        let size = r32(memory, entry.wrapping_add(8));
        w32(memory, out_size, size);
    }
    base.wrapping_add(r32(memory, entry.wrapping_add(4)))
}

/// `0x624C0(name, slot)`: like [`find_file`] but the whole name is hashed and only slot `slot` is searched
/// ([`pak_select`]); no size output. MEASURED.
pub fn open_named(memory: &mut dyn GuestMemory, name: u32, slot_index: u32) -> u32 {
    if r32(memory, PAK_COUNT) == 0 {
        return 0;
    }
    let key = crc32_string(memory, name);
    let entry = pak_select(memory, slot_index, key);
    if entry == 0 {
        return 0;
    }
    let slot = r32(memory, PAK_CURRENT_SLOT);
    let base = r32(memory, slot.wrapping_add(0xC));
    if base == 0 || r32(memory, entry.wrapping_add(0xC)) != 0 {
        return 0;
    }
    let offset = r32(memory, entry.wrapping_add(4));
    if offset & 0xF != 0 {
        return 0;
    }
    offset.wrapping_add(base)
}

/// `0x62CA0(name)`: size of a file by name: the pak entry size (plus `0x100` for a compressed entry, room for the
/// inflater) or, outside the paks, the length of the file (`open`, `seek` to the end, `close`); -1 when missing.
pub fn file_size(memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects, name: u32) -> u32 {
    if r32(memory, PAK_COUNT) != 0 {
        let hashed = pak_name(memory, name);
        let key = crc32_string(memory, hashed);
        let entry = pak_find(memory, key);
        if entry != 0 {
            let stored = r32(memory, entry.wrapping_add(0xC));
            let size = r32(memory, entry.wrapping_add(8));
            return size.wrapping_add(if stored != 0 { 0x100 } else { 0 });
        }
    }
    let fd = effects.call(memory, callee::FILE_OPEN, &[name, 1]);
    if (fd as i32) < 0 {
        return 0xFFFF_FFFF;
    }
    let length = effects.call(memory, callee::FILE_SEEK, &[fd, 0, 0]);
    effects.call(memory, callee::FILE_CLOSE, &[fd]);
    length
}

/// `0x62D20(name, buffer, size)`: reads a file by name: a pak entry is read from the pak stream (seek to the
/// entry offset, a compressed entry is read into the end of the buffer, 32 byte aligned, and inflated), else the
/// file itself (only when `[0x7DE398] & 0x100000`, otherwise the missing file report and 0). MEASURED.
pub fn file_read(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    name: u32,
    buffer: u32,
    size: u32,
) -> u32 {
    if r32(memory, PAK_COUNT) != 0 {
        let hashed = pak_name(memory, name);
        let key = crc32_string(memory, hashed);
        let entry = pak_find(memory, key);
        if entry != 0 {
            let offset = r32(memory, entry.wrapping_add(4));
            let slot = r32(memory, PAK_CURRENT_SLOT);
            let handle = r32(memory, slot);
            effects.call(memory, callee::FILE_SEEK, &[handle, offset, 1]);
            let stored = r32(memory, entry.wrapping_add(0xC));
            if stored == 0 {
                let slot = r32(memory, PAK_CURRENT_SLOT);
                let handle = r32(memory, slot);
                return effects.call(memory, callee::FILE_READ_RAW, &[handle, buffer, size]);
            }
            let source = buffer.wrapping_sub(stored).wrapping_add(size) & 0xFFFF_FFE0;
            let slot = r32(memory, PAK_CURRENT_SLOT);
            let handle = r32(memory, slot);
            effects.call(memory, callee::FILE_READ_RAW, &[handle, source, stored]);
            let stored = r32(memory, entry.wrapping_add(0xC));
            return effects.call(memory, callee::INFLATE, &[source, stored, buffer]);
        }
    }
    if r32(memory, FILE_FLAGS) & 0x10_0000 == 0 {
        effects.call(memory, callee::REPORT_MISSING, &[0]);
        return 0;
    }
    let fd = effects.call(memory, callee::FILE_OPEN, &[name, 1]);
    if (fd as i32) < 0 {
        effects.call(memory, callee::REPORT_MISSING, &[0]);
    }
    let read = effects.call(memory, callee::FILE_READ_RAW, &[fd, buffer, size]);
    effects.call(memory, callee::FILE_CLOSE, &[fd]);
    read
}

/// `0x62F00(name, alignment, out_size)`: loads a whole file into a fresh buffer (`0x3E0B0(0x80, size,
/// alignment)`); `*out_size` receives the read size (or the negative size when missing). MEASURED.
pub fn load_file(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    name: u32,
    alignment: u32,
    out_size: u32,
) -> u32 {
    let mut buffer = 0;
    let mut size = file_size(memory, effects, name);
    if (size as i32) > 0 {
        buffer = effects.call(memory, callee::ALLOC_ALIGNED, &[0x80, size, alignment]);
        size = file_read(memory, effects, name, buffer, size);
    }
    if out_size != 0 {
        w32(memory, out_size, size);
    }
    buffer
}

/// `0x62F50(name, alignment, out_size)`: [`find_file`] first (resident pak entry, `out_size` written by it),
/// else the [`load_file`] path. MEASURED.
pub fn find_or_load(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    name: u32,
    alignment: u32,
    out_size: u32,
) -> u32 {
    let found = find_file(memory, name, out_size);
    if found != 0 {
        return found;
    }
    let mut buffer = 0;
    let mut size = file_size(memory, effects, name);
    if (size as i32) > 0 {
        buffer = effects.call(memory, callee::ALLOC_ALIGNED, &[0x80, size, alignment]);
        size = file_read(memory, effects, name, buffer, size);
    }
    if out_size != 0 {
        w32(memory, out_size, size);
    }
    buffer
}

/// `0x62FB0(path)`: pointer after the last `/` of the path (the path itself when there is none). MEASURED.
pub fn base_name(memory: &dyn GuestMemory, path: u32) -> u32 {
    let length = read_cstring(memory, path).len() as u32;
    let mut cursor = path.wrapping_add(length);
    if cursor > path {
        loop {
            if memory.read_u8(cursor) == b'/' {
                return cursor.wrapping_add(1);
            }
            cursor = cursor.wrapping_sub(1);
            if cursor <= path {
                break;
            }
        }
    }
    if memory.read_u8(cursor) == b'/' {
        cursor.wrapping_add(1)
    } else {
        cursor
    }
}

/// `0x631C0(name)`: the loaded resource record (`0x18` bytes at `0x6D7C88`, count `[0x6F0B00]`) of type 1 or 5
/// whose name string (`+0xC`) equals `name`, else 0. MEASURED.
pub fn cache_lookup(memory: &dyn GuestMemory, name: u32) -> u32 {
    if name == 0 {
        return 0;
    }
    let wanted = read_cstring(memory, name);
    let count = r32(memory, CACHE_COUNT) as i32;
    for index in 0..count.max(0) as u32 {
        let record = CACHE_RECORDS.wrapping_add(index.wrapping_mul(0x18));
        let kind = r16(memory, record) as u16;
        if (kind == 1 || kind == 5)
            && read_cstring(memory, r32(memory, record.wrapping_add(0xC))) == wanted
        {
            return record;
        }
    }
    0
}

/// `0x6B380(value)`: remembers a model id in the array `[0x6F1E18]` (count `[0x6F1E20]`): known ids stop at their
/// index (returned), a new id is stored at the count, which grows while below 30 (55 in level `0x66`). The slot is
/// written even at the limit. Returns the limit, or the index of a known id. MEASURED.
pub fn note_model(memory: &mut dyn GuestMemory, value: u32) -> u32 {
    let count = r32(memory, NOTED_COUNT);
    let array = r32(memory, NOTED_ARRAY);
    for index in 0..(count as i32).max(0) as u32 {
        if r32(memory, array.wrapping_add(index.wrapping_mul(4))) == value {
            return index;
        }
    }
    w32(memory, array.wrapping_add(count.wrapping_mul(4)), value);
    let limit: u32 = if r32(memory, LEVEL) == 0x66 {
        0x37
    } else {
        0x1E
    };
    if (count as i32) < limit as i32 {
        w32(memory, NOTED_COUNT, count.wrapping_add(1));
    }
    limit
}

/// `0x18380(record)` -> `[record + 0x24]`. MEASURED.
pub fn archive_id(memory: &dyn GuestMemory, record: u32) -> u32 {
    r32(memory, record.wrapping_add(0x24))
}

/// `0x18A30(a, b, c)`: tail forwarding to `0x18390` (edi = a) with the two last arguments. MEASURED.
pub fn archive_add(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    first: u32,
    second: u32,
    third: u32,
) -> u32 {
    effects.call(memory, callee::ARCHIVE_STORE, &[second, third, first])
}

/// `0x8DF60(block, base)`: turns the file relative offsets of a loaded model into pointers (`offset + base`, 0
/// stays 0): the block's `+0x44`, then per mesh (`0xC8` bytes, `[block]` of them ending at the block) the three
/// material cells and their records, the part, bone, morph and skin tables and their records, the matrix cells
/// and the three terminated animation tables. Sets bit `k` of `block[+0xC]` for every non zero material cell `k`.
/// Ends with `0x18FB10(block)`. MEASURED.
pub fn bind_file(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    block: u32,
    base: u32,
) {
    fn reloc(memory: &mut dyn GuestMemory, cell: u32, base: u32) {
        let value = memory.read_u32(cell);
        memory.write_u32(
            cell,
            if value != 0 {
                value.wrapping_add(base)
            } else {
                0
            },
        );
    }
    reloc(memory, block.wrapping_add(0x44), base);
    let count = r32(memory, block);
    let first = block.wrapping_sub(count.wrapping_mul(0xC8));
    if (count as i32) > 0 {
        let mut index = 0u32;
        loop {
            let mesh = first.wrapping_add(index.wrapping_mul(0xC8));
            for k in 0..3u32 {
                let cell = mesh.wrapping_add(0x60 + 4 * k);
                let value = r32(memory, cell);
                if value != 0 {
                    w32(memory, cell, value.wrapping_add(base));
                }
                let q = mesh.wrapping_add(8 + 0x18 * k);
                for offset in [0xFFFF_FFF8u32, 0xFFFF_FFFC, 0, 8, 4, 0xC] {
                    reloc(memory, q.wrapping_add(offset), base);
                }
                if r32(memory, cell) != 0 {
                    let flags = r32(memory, block.wrapping_add(0xC));
                    w32(memory, block.wrapping_add(0xC), flags | 1 << k);
                }
            }
            reloc(memory, mesh.wrapping_add(0x6C), base);
            reloc(memory, mesh.wrapping_add(0x78), base);
            reloc(memory, mesh.wrapping_add(0x7C), base);
            if r32(memory, mesh.wrapping_add(0x7C)) != 0 && r16(memory, mesh.wrapping_add(0x50)) > 0
            {
                let mut j = 0i32;
                loop {
                    let table = r32(memory, mesh.wrapping_add(0x7C));
                    reloc(memory, table.wrapping_add(4 * j as u32), base);
                    j += 1;
                    if j >= i32::from(r16(memory, mesh.wrapping_add(0x50))) {
                        break;
                    }
                }
            }
            reloc(memory, mesh.wrapping_add(0x80), base);
            if r32(memory, mesh.wrapping_add(0x80)) != 0 && r16(memory, mesh.wrapping_add(0x52)) > 0
            {
                let mut j = 0i32;
                loop {
                    let cell = |memory: &dyn GuestMemory| {
                        r32(memory, mesh.wrapping_add(0x80)).wrapping_add(4 * j as u32)
                    };
                    let at = cell(memory);
                    reloc(memory, at, base);
                    for offset in [0x30u32, 0x34, 0x38] {
                        let at = cell(memory);
                        let record = r32(memory, at);
                        reloc(memory, record.wrapping_add(offset), base);
                    }
                    j += 1;
                    if j >= i32::from(r16(memory, mesh.wrapping_add(0x52))) {
                        break;
                    }
                }
            }
            reloc(memory, mesh.wrapping_add(0x84), base);
            if r32(memory, mesh.wrapping_add(0x84)) != 0 && r16(memory, mesh.wrapping_add(0x54)) > 0
            {
                let mut j = 0i32;
                loop {
                    let table = r32(memory, mesh.wrapping_add(0x84));
                    reloc(memory, table.wrapping_add(4 * j as u32), base);
                    j += 1;
                    if j >= i32::from(r16(memory, mesh.wrapping_add(0x54))) {
                        break;
                    }
                }
            }
            reloc(memory, mesh.wrapping_add(0x88), base);
            if r32(memory, mesh.wrapping_add(0x88)) != 0 && r16(memory, mesh.wrapping_add(0x56)) > 0
            {
                let mut j = 0i32;
                loop {
                    let table = r32(memory, mesh.wrapping_add(0x88));
                    let at = table.wrapping_add(4 * j as u32);
                    reloc(memory, at, base);
                    let record = r32(memory, at);
                    reloc(memory, record.wrapping_add(0x4C), base);
                    j += 1;
                    if j >= i32::from(r16(memory, mesh.wrapping_add(0x56))) {
                        break;
                    }
                }
            }
            reloc(memory, mesh.wrapping_add(0xB4), base);
            reloc(memory, mesh.wrapping_add(0x8C), base);
            let matrix = r32(memory, mesh.wrapping_add(0xA8));
            if matrix != 0 {
                w32(memory, mesh.wrapping_add(0xA8), matrix.wrapping_add(base));
                for offset in [0x1Cu32, 0x24, 0x2C, 0x34] {
                    let record = r32(memory, mesh.wrapping_add(0xA8));
                    reloc(memory, record.wrapping_add(offset), base);
                }
            }
            reloc(memory, mesh.wrapping_add(0xAC), base);
            reloc(memory, mesh.wrapping_add(0xB0), base);
            for k in 0..3u32 {
                let cell = mesh.wrapping_add(0xBC + 4 * k);
                reloc(memory, cell, base);
                let mut record = r32(memory, cell);
                let source = r32(memory, cell.wrapping_sub(0x5C));
                if record != 0 && source != 0 {
                    let mut flag = source.wrapping_add(3);
                    while (memory.read_u8(flag) as i8) >= 0 {
                        reloc(memory, record, base);
                        flag = flag.wrapping_add(0xA);
                        record = record.wrapping_add(8);
                    }
                }
            }
            index += 1;
            if (index as i32) >= (r32(memory, block) as i32) {
                break;
            }
        }
    }
    effects.call(memory, callee::BLOCK_READY, &[block]);
}

/// `0x96330(file, flag)`: first use fix-up of a loaded model file: relative name list (`+0`), block (`+4`) and
/// table (`+8`) offsets become pointers, then every record (`0x10` bytes, ended by id -1) of the name list that is
/// not id `0x40000000` and has flag `0x200000` is registered: `0x18A30(id, record, record)` for flag `0x10000000`,
/// else `0x8E310` (register by index, when `flag` is set) or `0x637E0(id, record, record)`. Returns the block
/// pointer (0 when the file has none). MEASURED.
pub fn fixup_file(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    file: u32,
    flag: u32,
) -> u32 {
    let relative = |memory: &dyn GuestMemory, offset: u32| {
        let value = r32(memory, file.wrapping_add(offset));
        if value != 0 {
            value.wrapping_add(file)
        } else {
            0
        }
    };
    let names = relative(memory, 0);
    let table = relative(memory, 8);
    effects.call(memory, callee::RELOCATE_NAMES, &[names, file]);
    let block = relative(memory, 4);
    effects.call(memory, callee::RELOCATE_TABLE, &[table, file, names]);
    let mut index = 0u32;
    if r32(memory, names) != 0xFFFF_FFFF {
        let mut record = names;
        loop {
            let id = r32(memory, record);
            let flags = r32(memory, record.wrapping_add(0xC));
            if id != 0x4000_0000 && flags & 0x20_0000 != 0 {
                if flags & 0x1000_0000 != 0 {
                    archive_add(memory, effects, id, record, record);
                } else if flag != 0 {
                    effects.call(memory, callee::REGISTER_BY_INDEX, &[index, names]);
                } else {
                    effects.call(memory, callee::REGISTER_ID, &[id, record, record]);
                }
            }
            let next = r32(memory, record.wrapping_add(0x10));
            record = record.wrapping_add(0x10);
            index += 1;
            if next == 0xFFFF_FFFF {
                break;
            }
        }
    }
    block
}

/// `0x8E420` (ecx = `mode`, edi = `file`, stack = `cache`): finishes a model load: the record `mode * 0xB4`
/// bytes into the file block (offset value, 0 stays 0) gets `+4 = 1`; for a cached file whose first use is done
/// (`cache[+2] & 1`) and `mode <= 1` that is all, else `+0x10` = names pointer, `+0x14` = table pointer, then
/// `0x8DF60(record, file)`, `0x8DD10(table, file, names)` and the cache first use bit. Mode 0 gives 0. MEASURED.
pub fn model_finish(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    cache: u32,
    mode: u32,
    file: u32,
) -> u32 {
    let offset = |memory: &dyn GuestMemory, at: u32| {
        let value = r32(memory, file.wrapping_add(at));
        if value != 0 {
            value.wrapping_add(file)
        } else {
            0
        }
    };
    let block = offset(memory, 4);
    if mode == 0 {
        return 0;
    }
    let entry = r32(memory, mode.wrapping_mul(0xB4).wrapping_add(block));
    let record = if entry != 0 {
        entry.wrapping_add(file)
    } else {
        0
    };
    w32(memory, record.wrapping_add(4), 1);
    if cache != 0 && memory.read_u8(cache.wrapping_add(2)) & 1 != 0 && (mode as i32) <= 1 {
        return record;
    }
    let names = offset(memory, 0);
    w32(memory, record.wrapping_add(0x10), names);
    let table = offset(memory, 0x28);
    w32(memory, record.wrapping_add(0x14), table);
    effects.call(memory, callee::BIND_FILE, &[record, file]);
    let names = r32(memory, record.wrapping_add(0x10));
    let table = r32(memory, record.wrapping_add(0x14));
    effects.call(memory, callee::RELOCATE_TABLE, &[table, file, names]);
    if cache != 0 {
        let byte = memory.read_u8(cache.wrapping_add(2));
        memory.write_u8(cache.wrapping_add(2), byte | 1);
    }
    record
}

/// `0x62260(name)`: index of the mounted pak slot whose name (`+0x14`) equals `name`, else -1. MEASURED.
pub fn slot_by_name(memory: &dyn GuestMemory, name: u32) -> u32 {
    let wanted = read_cstring(memory, name);
    let count = r32(memory, PAK_COUNT) as i32;
    for index in 0..count.max(0) as u32 {
        let slot = PAK_SLOTS.wrapping_add(index.wrapping_mul(PAK_SLOT_STRIDE));
        let cell = r32(memory, slot.wrapping_add(0x14));
        if cell != 0 && read_cstring(memory, cell) == wanted {
            return index;
        }
    }
    0xFFFF_FFFF
}

/// `0x61FD0(name, alignment)`: the first free pak slot (`+0x18 == -1`; a new one when none), with a copy of the
/// name at `+0x14` (allocated with `0x3DFA0`). Returns the slot index. MEASURED.
pub fn slot_alloc(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    name: u32,
    alignment: u32,
) -> u32 {
    let count = r32(memory, PAK_COUNT);
    let mut index = 0u32;
    let mut found = false;
    if (count as i32) > 0 {
        while (index as i32) < count as i32 {
            let slot = PAK_SLOTS.wrapping_add(index.wrapping_mul(PAK_SLOT_STRIDE));
            if r32(memory, slot.wrapping_add(0x18)) == 0xFFFF_FFFF {
                found = true;
                break;
            }
            index += 1;
        }
    }
    if !found {
        index = count;
        w32(memory, PAK_COUNT, count.wrapping_add(1));
    }
    let slot = index.wrapping_mul(PAK_SLOT_STRIDE).wrapping_add(PAK_SLOTS);
    let bytes = read_cstring(memory, name);
    let copy = effects.call(
        memory,
        callee::ALLOC_PLAIN,
        &[bytes.len() as u32 + 1, alignment],
    );
    w32(memory, slot.wrapping_add(0x14), copy);
    for (offset, &byte) in bytes.iter().enumerate() {
        memory.write_u8(copy.wrapping_add(offset as u32), byte);
    }
    memory.write_u8(copy.wrapping_add(bytes.len() as u32), 0);
    index
}

/// `0x62040(buffer, size, id)`: mounts a pak that is resident in memory (header `P5CK`, offset, TOC size; the
/// TOC is the tail of the buffer: entries at `buffer + size - toc size`, count `toc size / 16`) in a free slot,
/// which keeps `id` at `+0x18`, no name and handle -1. At most 30 slots (-1 otherwise). The magic is not checked.
/// Returns the slot index. MEASURED.
pub fn mount_memory(memory: &mut dyn GuestMemory, buffer: u32, size: u32, id: u32) -> u32 {
    let count = r32(memory, PAK_COUNT);
    if (count as i32) >= PAK_MAX_SLOTS as i32 {
        return 0xFFFF_FFFF;
    }
    let toc_size = r32(memory, buffer.wrapping_add(8));
    let mut index = 0u32;
    let mut found = false;
    if (count as i32) > 0 {
        while (index as i32) < count as i32 {
            let slot = PAK_SLOTS.wrapping_add(index.wrapping_mul(PAK_SLOT_STRIDE));
            if r32(memory, slot.wrapping_add(0x18)) == 0xFFFF_FFFF {
                found = true;
                break;
            }
            index += 1;
        }
    }
    if !found {
        index = count;
        w32(memory, PAK_COUNT, count.wrapping_add(1));
    }
    let slot = index.wrapping_mul(PAK_SLOT_STRIDE).wrapping_add(PAK_SLOTS);
    w32(memory, slot.wrapping_add(0x18), id);
    w32(memory, slot.wrapping_add(0xC), buffer);
    w32(
        memory,
        slot.wrapping_add(0x10),
        buffer.wrapping_sub(toc_size).wrapping_add(size),
    );
    w32(memory, slot.wrapping_add(0x14), 0);
    w32(memory, slot, 0xFFFF_FFFF);
    w32(memory, slot.wrapping_add(8), toc_size >> 4);
    index
}

/// The callee addresses [`PortedResource`] runs itself (everything else is forwarded to `inner`).
pub const HANDLED: [u32; 12] = [
    callee::CACHE_LOOKUP,
    callee::BASE_NAME,
    callee::OPEN_NAMED,
    callee::NOTE_MODEL,
    callee::FIND_FILE,
    callee::LOAD_FILE,
    callee::FIND_OR_LOAD,
    callee::FIXUP_FILE,
    callee::BIND_FILE,
    callee::MODEL_FINISH,
    callee::ARCHIVE_ID,
    callee::ARCHIVE_ADD,
];

/// Runs the ported resource layer for the callee addresses of the model loaders (`0x96410`, `0x8E4B0`,
/// `0x8E590`) and forwards everything else (the file system, the allocator, the formatter, the table fix-ups
/// `0x8DD10` / `0x8E3E0` / `0x8E310` / `0x637E0`, ...) to `inner`. MEASURED by the whole-root goldens.
pub struct PortedResource<'a> {
    pub inner: &'a mut dyn DestroyEffects,
}

impl DestroyEffects for PortedResource<'_> {
    fn call(&mut self, memory: &mut dyn GuestMemory, address: u32, args: &[u32]) -> u32 {
        let arg = |index: usize| args.get(index).copied().unwrap_or(0);
        match address {
            callee::CACHE_LOOKUP => cache_lookup(memory, arg(0)),
            callee::BASE_NAME => base_name(memory, arg(0)),
            callee::OPEN_NAMED => open_named(memory, arg(0), arg(1)),
            callee::NOTE_MODEL => note_model(memory, arg(0)),
            callee::FIND_FILE => find_file(memory, arg(0), arg(1)),
            callee::LOAD_FILE => load_file(memory, self, arg(0), arg(1), arg(2)),
            callee::FIND_OR_LOAD => find_or_load(memory, self, arg(0), arg(1), arg(2)),
            callee::FIXUP_FILE => fixup_file(memory, self, arg(0), arg(1)),
            callee::BIND_FILE => {
                bind_file(memory, self, arg(0), arg(1));
                0
            }
            callee::MODEL_FINISH => model_finish(memory, self, arg(0), arg(1), arg(2)),
            callee::ARCHIVE_ID => archive_id(memory, arg(0)),
            callee::ARCHIVE_ADD => archive_add(memory, self, arg(0), arg(1), arg(2)),
            _ => self.inner.call(memory, address, args),
        }
    }
}
