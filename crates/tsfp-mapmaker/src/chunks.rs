//! mapmaker/storage chunk buffer reader (T1946): readable port of 0x301a90
//! `game_mapedit_deserialize_map_state_from_chunk_buffer`, built chunk by chunk. Format reference:
//! docs/t1717-mapmaker-format.md. Tier 1 (differential vs the lifted original) holds for buffers made of
//! the ported chunk kinds listed in `PORTED_TAGS`; any other tag is routed to
//! `DeserializeHost::unported_chunk`, so the function is NOT yet complete (verification debt).
//! Confidence INFERRED (static disassembly; names are guesses from the call shapes).
use tsfp_gameplay::memory::GuestMemory;

/// Pointer to the mapedit state record.
pub const EDITOR_PTR: u32 = 0x0078_44a8;
const EDITOR_FLAGS: u32 = 0x25c;
const EDITOR_SAVED_FIELD_264: u32 = 0x264;
const EDITOR_SAVED_FIELD_254: u32 = 0x254;
const EDITOR_FIELD_268: u32 = 0x268;
const EDITOR_DIRTY_FLAGS: u32 = 0x238;
const PALETTE: u32 = 0x0078_3a80;
const PACKED_FLAG_ENTRIES: u32 = 0x113c4;
const PREVIEW_BLOCK: u32 = 0x1183c;
/// Magic and version of the buffer prefix.
pub const BUFFER_MAGIC: u32 = 0x00c7_7667;
pub const BUFFER_VERSION: u32 = 0x12d;
/// First settings tag; the table of handlers is indexed by `tag - TAG_SETTINGS`.
pub const TAG_SETTINGS: u32 = 0x00c7_7668;
pub const TAG_TERMINATOR: u32 = 0x00c7_766a;
pub const TAG_SETTINGS_PORTED: u32 = TAG_SETTINGS;
pub const TAG_CHANGE: u32 = 0x00c7_766b;
pub const TAG_E4: u32 = 0x00c7_766c;
pub const TAG_B4: u32 = 0x00c7_766d;
pub const TAG_RECORD_220: u32 = 0x00c7_766e;
pub const TAG_RECORD_8C: u32 = 0x00c7_766f;
pub const TAG_PACKED_FLAGS: u32 = 0x00c7_7670;
pub const TAG_PREVIEW: u32 = 0x00c7_7671;
pub const TAG_PALETTE: u32 = 0x00c7_7672;
pub const TAG_NAMES: u32 = 0x00c7_7673;
pub const TAG_GROUPED: u32 = 0x00c7_7674;
pub const TAG_KIND3: u32 = 0x00c7_7675;
/// Chunk kinds with a readable handler so far.
pub const PORTED_TAGS: [u32; 12] = [
    TAG_SETTINGS_PORTED,
    TAG_CHANGE,
    TAG_E4,
    TAG_B4,
    TAG_RECORD_220,
    TAG_RECORD_8C,
    TAG_PACKED_FLAGS,
    TAG_PREVIEW,
    TAG_PALETTE,
    TAG_NAMES,
    TAG_GROUPED,
    TAG_KIND3,
];

/// Services outside mapmaker/storage (INFERRED names).
pub trait DeserializeHost {
    /// 0x3c9d90 stack probe; the original's frame of 0x13f4 bytes is claimed here (no effect in Rust).
    fn stack_probe(&mut self, size: u32);
    /// 0x32a600 find the terminator chunk of the buffer (result unused).
    fn find_terminator(&mut self, buffer: u32);
    /// 0x2febf0 reset the editor selection state (argument 0).
    fn reset_selection(&mut self, argument: u32);
    /// 0x321c00 clear the content tables before loading.
    fn clear_content_tables(&mut self);
    /// 0x3dfa0 arena allocate (size, kind).
    fn arena_alloc(&mut self, size: u32, kind: u32) -> u32;
    /// 0x3e1e0 release an arena block.
    fn arena_release(&mut self, block: u32);
    /// 0x2fe160 finish a non preview load.
    fn finish_load(&mut self);
    /// 0x31bdb0 non zero when the loaded map needs the dirty flag.
    fn needs_dirty_flag(&mut self) -> u32;
    /// 0x325610, 0x320f50, 0x31e160 post load refreshes.
    fn refresh_a(&mut self);
    fn refresh_b(&mut self);
    fn refresh_c(&mut self);
    /// 0x2fe0f0 load the preview block (payload, destination, flag).
    fn load_preview(&mut self, payload: u32, destination: u32, flag: u32);
    /// 0x31dd10 set one packed flag entry (entry address, value).
    fn set_packed_entry(&mut self, entry: u32, value: u32);
    /// 0x3210c0 address of the name storage block.
    fn name_storage(&mut self) -> u32;
    /// 0x320ea0 install grouped records (table, count).
    fn install_grouped(&mut self, table: u32, count: u32);
    /// 0x320fc0 install kind 3 entries (table, byte size).
    fn install_kind3(&mut self, table: u32, size: u32);
    /// 0x311890 map selection setup for a map id -> mode value stored at 0x78402c.
    fn map_mode_for(&mut self, map: u32) -> u32;
    /// 0x3118f0 setup preset value stored at editor+0x208.
    fn setup_preset(&mut self) -> u32;
    /// 0x311910 number of mode slots.
    fn mode_slot_count(&mut self) -> u32;
    /// 0x311920 address of the mode availability array (dword per mode).
    fn mode_slots(&mut self) -> u32;
    /// 0x332330 resolve a point reference (node pointer, destination, value).
    fn resolve_point(&mut self, node: u32, destination: u32, value: u32);
    /// Chunk kinds without a readable handler yet.
    fn unported_chunk(&mut self, tag: u32);
}

fn copy_bytes(m: &mut impl GuestMemory, source: u32, destination: u32, count: u32) {
    for i in 0..count {
        m.write8(destination.wrapping_add(i), m.read8(source.wrapping_add(i)));
    }
}

fn zero_bytes(m: &mut impl GuestMemory, destination: u32, count: u32) {
    for i in 0..count {
        m.write8(destination.wrapping_add(i), 0);
    }
}

/// Frame addresses of the original (`s` = frame base after the three register pushes).
struct Frame {
    s: u32,
}

impl Frame {
    fn new(entry_esp: u32) -> Self {
        let aligned = entry_esp.wrapping_sub(4) & !7;
        Self {
            s: aligned.wrapping_sub(0x13f4).wrapping_sub(12),
        }
    }
    fn header_slot(&self) -> u32 {
        self.s + 0x18
    }
    fn block_slot(&self) -> u32 {
        self.s + 0x1c
    }
    fn preview_flag(&self) -> u32 {
        self.s + 0x28
    }
    fn grouped_table(&self) -> u32 {
        self.s + 0x78
    }
    fn kind3_table(&self) -> u32 {
        self.s + 0x848
    }
}

/// 0x302873 palette chunk: 64 dwords copied to the palette table.
fn chunk_palette(m: &mut impl GuestMemory, payload: u32) {
    copy_bytes(m, payload, PALETTE, 0x100);
}

/// 0x302853 preview chunk.
fn chunk_preview(
    m: &mut impl GuestMemory,
    h: &mut impl DeserializeHost,
    frame: &Frame,
    payload: u32,
) {
    let editor = m.read32(EDITOR_PTR);
    let flag = m.read32(frame.preview_flag());
    h.load_preview(payload, editor + PREVIEW_BLOCK, flag);
}

/// 0x302807 packed flags chunk: eight entries of 0x18 bytes, one payload byte each. The original keeps
/// a running payload pointer in its frame (+0x10), advanced by 4 after every group of four entries.
fn chunk_packed_flags(
    m: &mut impl GuestMemory,
    h: &mut impl DeserializeHost,
    frame: &Frame,
    payload: u32,
) {
    let editor = m.read32(EDITOR_PTR);
    let mut cursor = payload;
    m.write32(frame.s + 0x10, cursor);
    for k in 0..8u32 {
        let value = u32::from(m.read8(payload + k));
        h.set_packed_entry(editor + PACKED_FLAG_ENTRIES + k * 0x18, value);
        if k % 4 == 3 {
            cursor = cursor.wrapping_add(4);
            m.write32(frame.s + 0x10, cursor);
        }
    }
}

/// 0x302886 names chunk: copied to the name storage block (0x4b00 bytes), the rest zeroed.
fn chunk_names(
    m: &mut impl GuestMemory,
    h: &mut impl DeserializeHost,
    frame: &Frame,
    payload: u32,
) {
    let header = m.read32(frame.header_slot());
    let size = m.read32(header + 4);
    m.write32(frame.s + 0x24, size);
    let remainder = 0x4b00u32.wrapping_sub(size);
    m.write32(frame.s + 0x14, remainder);
    let base = h.name_storage();
    copy_bytes(m, payload, base, size);
    let base = h.name_storage();
    zero_bytes(m, base.wrapping_add(m.read32(header + 4)), remainder);
}

/// 0x3028de grouped records chunk (five dwords each): staged in the frame, then installed.
fn chunk_grouped(
    m: &mut impl GuestMemory,
    h: &mut impl DeserializeHost,
    frame: &Frame,
    payload: u32,
) {
    let header = m.read32(frame.header_slot());
    let size = m.read32(header + 4);
    zero_bytes(m, frame.grouped_table(), 0x1f4 * 4);
    copy_bytes(m, payload, frame.grouped_table(), size);
    h.install_grouped(frame.grouped_table(), size / 20);
}

/// 0x30291f kind 3 chunk (six byte entries): staged in the frame, then installed.
fn chunk_kind3(
    m: &mut impl GuestMemory,
    h: &mut impl DeserializeHost,
    frame: &Frame,
    payload: u32,
) {
    let header = m.read32(frame.header_slot());
    let size = m.read32(header + 4);
    zero_bytes(m, frame.kind3_table(), 0x2ee * 4);
    copy_bytes(m, payload, frame.kind3_table(), size);
    h.install_kind3(frame.kind3_table(), size);
}

use crate::content::write16;

fn increment16(m: &mut impl GuestMemory, address: u32) {
    let value = m.read16(address).wrapping_add(1);
    write16(m, address, value);
}

/// 0x302032 change chunk (packed 12 byte editor change record): appended to the change table at
/// editor+0x3070 and counted per kind (0 move, 1 and 2 other kinds) in the 16 bit counters at
/// +0x3068, +0x306a and +0x306c; +0x306e is the total.
fn chunk_change(m: &mut impl GuestMemory, payload: u32) {
    let editor = m.read32(EDITOR_PTR);
    let index = u32::from(m.read16(editor + 0x306e));
    let record = editor + 0x3070 + index * 12;
    let signed =
        |m: &mut dyn GuestMemory, offset: u32| i16::from(m.read8(payload + offset) as i8) as u16;
    let (x, z, y) = (signed(m, 0), signed(m, 2), signed(m, 1));
    write16(m, record, x);
    write16(m, record + 2, z);
    write16(m, record + 4, y);
    m.write8(record + 6, m.read8(payload + 3));
    m.write8(record + 7, m.read8(payload + 4));
    let shifted = m.read16(payload + 8);
    write16(m, record + 0xa, shifted);
    m.write8(record + 8, m.read8(payload + 5));
    m.write8(record + 9, m.read8(payload + 6));
    match m.read8(record + 7) {
        0 => increment16(m, editor + 0x3068),
        1 => increment16(m, editor + 0x306a),
        2 => increment16(m, editor + 0x306c),
        _ => {}
    }
    increment16(m, editor + 0x306e);
}

fn copy_dwords(m: &mut impl GuestMemory, source: u32, destination: u32, count: u32) {
    copy_bytes(m, source, destination, count * 4);
}

/// 0x30267f b4 record chunk (45 dwords) appended to the table at editor+0x81a8.
fn chunk_b4(m: &mut impl GuestMemory, payload: u32) {
    let editor = m.read32(EDITOR_PTR);
    let count = m.read32(editor + 0x81a4);
    copy_dwords(m, payload, editor + 0x81a8 + count.wrapping_mul(0xb4), 0x2d);
    m.write32(editor + 0x81a4, m.read32(editor + 0x81a4).wrapping_add(1));
}

/// 0x3026b0 220 record chunk (0x220 bytes) appended at editor+0x41e4 with two derived counters.
fn chunk_record_220(m: &mut impl GuestMemory, payload: u32) {
    let editor = m.read32(EDITOR_PTR);
    let count = m.read32(editor + 0x41e0);
    copy_dwords(
        m,
        payload,
        editor + 0x41e4 + count.wrapping_mul(0x220),
        0x88,
    );
    m.write32(editor + 0x41e0, m.read32(editor + 0x41e0).wrapping_add(1));
    let marker = m.read32(payload + 0x21c);
    let editor = m.read32(EDITOR_PTR);
    let sum = m
        .read32(editor + 0x41d8)
        .wrapping_add(u32::from(marker == 1));
    m.write32(editor + 0x41d8, sum);
    let first = u8::from(m.read8(payload + 4) == 1);
    let editor = m.read32(EDITOR_PTR);
    m.write8(
        editor + 0x41dc,
        m.read8(editor + 0x41dc).wrapping_add(first),
    );
}

/// 0x30271e 8c record chunk (0x23 dwords) appended at editor+0xdd14. Kind 3 and kind 0xa records carry
/// index references that are turned into pointers through the table in the arena block of the frame.
fn chunk_record_8c(m: &mut impl GuestMemory, frame: &Frame, payload: u32) {
    let editor = m.read32(EDITOR_PTR);
    let count = m.read32(editor + 0xdd10);
    let base = editor.wrapping_add(count.wrapping_mul(0x8c));
    copy_dwords(m, payload, base + 0xdd14, 0x23);
    let editor = m.read32(EDITOR_PTR);
    let base = editor.wrapping_add(m.read32(editor + 0xdd10).wrapping_mul(0x8c));
    let table = m.read32(frame.block_slot());
    match m.read32(base + 0xdd94) {
        3 => {
            let mut k = 0u32;
            while (m.read32(base + 0xdd34) as i32) > 0 {
                let entry = base + 0xdd38 + 8 * k;
                let index = m.read32(entry);
                m.write32(entry, m.read32(table.wrapping_add(index.wrapping_mul(4))));
                k += 1;
                let bound = m.read32(base + 0xdd34) as i32;
                if (k as i32) >= bound {
                    break;
                }
            }
        }
        0xa => {
            let mut k = 0u32;
            while (m.read32(base + 0xdd84) as i32) > 0 {
                let entry = base + 0xdd34 + 8 * k;
                let index = m.read32(entry + 4);
                m.write32(
                    entry,
                    m.read32(table.wrapping_add(index.wrapping_mul(4)).wrapping_sub(4)),
                );
                k += 1;
                if (k as i32) >= m.read32(base + 0xdd84) as i32 {
                    break;
                }
            }
        }
        _ => {}
    }
    let editor = m.read32(EDITOR_PTR);
    m.write32(editor + 0xdd10, m.read32(editor + 0xdd10).wrapping_add(1));
}

/// 0x302428 e4 record chunk: a 0xe4 byte record with up to `n` (payload +0x2c) points of 4 bytes each
/// (payload +0x44), resolved through the pointer table of the frame's arena block, plus the name and
/// the scalar fields. The original keeps its point counter in the frame (+0x14).
fn chunk_e4(m: &mut impl GuestMemory, h: &mut impl DeserializeHost, frame: &Frame, payload: u32) {
    let editor = m.read32(EDITOR_PTR);
    let count = m.read32(editor + 0x280);
    let record = editor.wrapping_add(count.wrapping_mul(0xe4));
    let mut points = record + 0x2ec;
    m.write32(frame.s + 0x14, 0);
    let wanted = m.read16(payload + 0x2c);
    if wanted > 0 {
        let mut source = payload + 0x44;
        loop {
            let slot = u32::from(m.read8(source + 2));
            let table = m.read32(frame.block_slot());
            let node = m.read32(table.wrapping_add(slot.wrapping_mul(4)));
            m.write32(points, node);
            let value = u32::from(m.read16(source));
            h.resolve_point(node, points + 4, value);
            m.write32(points + 0xc, u32::from(m.read8(source + 3)));
            let done = m.read32(frame.s + 0x14).wrapping_add(1);
            m.write32(frame.s + 0x14, done);
            source = source.wrapping_add(4);
            points = points.wrapping_add(0x10);
            if (done as i32) >= i32::from(m.read16(payload + 0x2c)) {
                break;
            }
        }
    }
    let record = editor.wrapping_add(m.read32(editor + 0x280).wrapping_mul(0xe4));
    m.write32(record + 0x2e0, u32::from(m.read16(payload + 0x2c)));
    m.write32(record + 0x2e4, u32::from(m.read16(payload + 0x2e)));
    let mut at = 0;
    loop {
        let byte = m.read8(payload + at);
        m.write8(record + 0x288 + at, byte);
        at += 1;
        if byte == 0 {
            break;
        }
    }
    let signed16 =
        |m: &mut dyn GuestMemory, offset: u32| i32::from(m.read16(payload + offset) as i16) as u32;
    let unsigned16 = |m: &mut dyn GuestMemory, offset: u32| u32::from(m.read16(payload + offset));
    let byte = |m: &mut dyn GuestMemory, offset: u32| u32::from(m.read8(payload + offset));
    let a = signed16(m, 0x22);
    m.write32(record + 0x2ac, a);
    let v = m.read32(payload + 0x38);
    m.write32(record + 0x2c0, v);
    let v = m.read32(payload + 0x3c);
    m.write32(record + 0x2c4, v);
    let v = unsigned16(m, 0x40);
    m.write32(record + 0x2c8, v);
    let v = unsigned16(m, 0x24);
    m.write32(record + 0x2b0, v);
    let v = signed16(m, 0x26);
    m.write32(record + 0x2b4, v);
    let v = byte(m, 0x28);
    m.write32(record + 0x2b8, v);
    let v = i32::from(m.read8(payload + 0x20) as i8) as u32;
    m.write32(record + 0x2bc, v);
    let v = signed16(m, 0x2a).wrapping_add(1);
    m.write32(record + 0x2d0, v);
    let v = m.read32(payload + 0x34);
    m.write32(record + 0x2dc, v);
    let v = byte(m, 0x30);
    m.write32(record + 0x2d4, v);
    let v = byte(m, 0x31);
    m.write32(record + 0x2cc, v);
    let v = unsigned16(m, 0x40);
    m.write32(record + 0x2c8, v);
    let v = unsigned16(m, 0x42);
    m.write32(record + 0x2e8, v);
    m.write32(editor + 0x280, m.read32(editor + 0x280).wrapping_add(1));
}

const MAP_SELECTION: u32 = 0x0078_4020;
const MODE_VALUE: u32 = 0x0078_402c;
const PRESET_VALUE: u32 = 0x0078_4024;
const WEAPON_PRESET: u32 = 0x0078_4028;
const STRING_A: u32 = 0x0078_3f00;
const STRING_B: u32 = 0x0078_3e00;
const STRING_C: u32 = 0x0078_3ce0;
const SETUP_ENTRIES: u32 = 0x11860;
const AUX_BLOCK: u32 = 0x11488;

fn sign16(m: &impl GuestMemory, address: u32) -> u32 {
    i32::from(m.read16(address) as i16) as u32
}

fn sign8(m: &impl GuestMemory, address: u32) -> u32 {
    i32::from(m.read8(address) as i8) as u32
}

fn copy_c_string(m: &mut impl GuestMemory, source: u32, destination: u32) {
    let mut at = 0;
    loop {
        let byte = m.read8(source.wrapping_add(at));
        m.write8(destination.wrapping_add(at), byte);
        at += 1;
        if byte == 0 {
            break;
        }
    }
}

/// 0x301c15 settings chunk (0x3dc bytes, layout in docs/t1717-mapmaker-format.md). Mirrors the map
/// selection into the globals, the option ids, the mode availability flags, the ten setup entries, the
/// weapon slots, the three level strings and the auxiliary block. The original keeps the payload size
/// word in its frame (+0x20) and NUL terminates the three string fields inside the payload first.
fn chunk_settings(
    m: &mut impl GuestMemory,
    h: &mut impl DeserializeHost,
    frame: &Frame,
    payload: u32,
) {
    let editor = m.read32(EDITOR_PTR);
    m.write32(MODE_VALUE, u32::from(m.read8(payload + 0xc)));
    let map = u32::from(m.read16(payload + 0xa));
    if m.read32(MAP_SELECTION) != map {
        m.write32(MAP_SELECTION, map);
        let mode = h.map_mode_for(map);
        m.write32(MODE_VALUE, mode);
        let editor = m.read32(EDITOR_PTR);
        m.write32(
            editor + EDITOR_DIRTY_FLAGS,
            m.read32(editor + EDITOR_DIRTY_FLAGS) | 0x12,
        );
    }
    let editor_now = m.read32(EDITOR_PTR);
    let _ = editor;
    let editor = editor_now;
    let v = sign16(m, payload + 0xe);
    m.write32(editor + 0x210, v);
    let v = sign16(m, payload + 0x10);
    m.write32(editor + 0x220, v);
    let v = u32::from(m.read8(payload + 0x12));
    m.write32(editor + 0x224, v);
    m.write32(editor + 0x200, 0);
    m.write32(editor + 0x214, 0);
    let v = sign16(m, payload + 0x40);
    m.write32(WEAPON_PRESET, v);
    let v = sign16(m, payload + 0x14);
    m.write32(PRESET_VALUE, v);
    let selection = m.read32(MAP_SELECTION);
    m.write32(editor + 0x204, selection);
    let preset = h.setup_preset();
    let editor = m.read32(EDITOR_PTR);
    m.write32(editor + 0x208, preset);
    let mode_flags = m.read32(payload + 0x244);
    if mode_flags != 0 {
        let count = h.mode_slot_count();
        let slots = h.mode_slots();
        if (count as i32) > 1 {
            for mode in 1..count {
                if mode_flags & (1u32 << (mode & 31)) != 0 {
                    m.write32(slots.wrapping_add(mode.wrapping_mul(4)), 1);
                }
            }
        }
        let editor = m.read32(EDITOR_PTR);
        if mode_flags & 1 != 0 {
            m.write32(editor + 0x200, 1);
        }
        let editor = m.read32(EDITOR_PTR);
        if mode_flags & 0x8000 != 0 {
            m.write32(editor + 0x214, 1);
        }
        let editor = m.read32(EDITOR_PTR);
        m.write32(editor + 0x218, u32::from(m.read8(payload + 0x246)) & 1);
        let editor = m.read32(EDITOR_PTR);
        m.write32(editor + 0x21c, (mode_flags >> 17) & 1);
    }
    let editor = m.read32(EDITOR_PTR);
    m.write32(editor + SETUP_ENTRIES, u32::from(m.read8(payload + 0x16)));
    for j in 0..10u32 {
        let editor = m.read32(EDITOR_PTR);
        let entry = editor + 0x11864 + 8 * j;
        let source = payload + 0x18 + 4 * j;
        let word = m.read16(source);
        write16(m, entry, word);
        let wide = sign8(m, source + 3) as u16;
        write16(m, entry + 6, wide);
        let byte = m.read8(source + 2);
        m.write8(entry + 3, byte);
    }
    let editor = m.read32(EDITOR_PTR);
    m.write32(editor + 0x11908, u32::from(m.read8(payload + 0x42)));
    let mut k = 0u32;
    if (m.read32(editor + 0x11908) as i32) > 0 {
        loop {
            let editor = m.read32(EDITOR_PTR);
            let v = sign16(m, payload + 0x44 + 2 * k);
            m.write32(editor + 0x1190c + 4 * k, v);
            k += 1;
            let editor = m.read32(EDITOR_PTR);
            if (k as i32) >= m.read32(editor + 0x11908) as i32 {
                break;
            }
        }
    }
    m.write8(payload + 0x81, 0);
    m.write8(payload + 0x161, 0);
    m.write8(payload + 0x241, 0);
    copy_c_string(m, payload + 0x62, STRING_A);
    copy_c_string(m, payload + 0x82, STRING_B);
    copy_c_string(m, payload + 0x162, STRING_C);
    let editor = m.read32(EDITOR_PTR);
    let v = sign8(m, payload + 0x242);
    m.write32(editor + 0x228, v);
    let size_word = m.read32(payload + 4);
    m.write32(frame.s + 0x20, size_word);
    copy_dwords(m, payload + 0x24c, editor + AUX_BLOCK, 0x64);
}

/// 0x301a90 `game_mapedit_deserialize_map_state_from_chunk_buffer`: load the editor state from a chunk
/// buffer. `flags` bit 0 = overlay load (keep selection), bit 1 = preview flag. Returns 1 on success and 0
/// when the buffer prefix is wrong. The frame addresses of the original are kept (`entry_esp`).
pub fn deserialize_map_state(
    m: &mut impl GuestMemory,
    h: &mut impl DeserializeHost,
    entry_esp: u32,
    buffer: u32,
    flags: u32,
) -> u32 {
    let frame = Frame::new(entry_esp);
    h.stack_probe(0x13f4);
    let editor = m.read32(EDITOR_PTR);
    let saved_264 = m.read32(editor + EDITOR_SAVED_FIELD_264);
    let saved_254 = m.read32(editor + EDITOR_SAVED_FIELD_254);
    let overlay = flags & 1;
    m.write32(frame.preview_flag(), (flags >> 1) & 1);
    m.write32(frame.s + 0x20, u32::MAX);
    m.write32(frame.s + 0x38, overlay);
    h.find_terminator(buffer);
    let editor = m.read32(EDITOR_PTR);
    let mut editor_flags = m.read32(editor + EDITOR_FLAGS);
    if overlay == 0 && (flags >> 1) & 1 == 0 {
        editor_flags |= 0x800;
    } else {
        editor_flags &= !0x800;
    }
    m.write32(editor + EDITOR_FLAGS, editor_flags);
    for offset in [0x306au32, 0x3068, 0x306c, 0x306e] {
        let editor = m.read32(EDITOR_PTR);
        m.write8(editor + offset, 0);
        m.write8(editor + offset + 1, 0);
    }
    h.reset_selection(0);
    let editor = m.read32(EDITOR_PTR);
    if overlay == 0 {
        m.write32(editor + EDITOR_SAVED_FIELD_254, 1);
    } else {
        m.write32(editor + EDITOR_SAVED_FIELD_264, saved_264);
        let editor = m.read32(EDITOR_PTR);
        m.write32(editor + EDITOR_SAVED_FIELD_254, saved_254);
        let editor = m.read32(EDITOR_PTR);
        let value = m.read32(editor + EDITOR_SAVED_FIELD_264) as i32;
        let next = if value <= -2 {
            -2
        } else {
            value.wrapping_sub(1)
        };
        m.write32(editor + EDITOR_FIELD_268, next as u32);
    }
    h.clear_content_tables();
    if m.read32(buffer) != BUFFER_MAGIC || m.read32(buffer + 4) != BUFFER_VERSION {
        h.reset_selection(0);
        return 0;
    }
    let block = h.arena_alloc(0x1000, 0);
    m.write32(frame.block_slot(), block);
    let mut header = buffer + 8;
    m.write32(frame.header_slot(), header);
    let mut tag = m.read32(header);
    let mut payload = header + 8;
    while tag != TAG_TERMINATOR {
        match tag {
            TAG_CHANGE => chunk_change(m, payload),
            TAG_SETTINGS => chunk_settings(m, h, &frame, payload),
            TAG_E4 => chunk_e4(m, h, &frame, payload),
            TAG_B4 => chunk_b4(m, payload),
            TAG_RECORD_220 => chunk_record_220(m, payload),
            TAG_RECORD_8C => chunk_record_8c(m, &frame, payload),
            TAG_PACKED_FLAGS => chunk_packed_flags(m, h, &frame, payload),
            TAG_PREVIEW => chunk_preview(m, h, &frame, payload),
            TAG_PALETTE => chunk_palette(m, payload),
            TAG_NAMES => chunk_names(m, h, &frame, payload),
            TAG_GROUPED => chunk_grouped(m, h, &frame, payload),
            TAG_KIND3 => chunk_kind3(m, h, &frame, payload),
            t if (TAG_SETTINGS..TAG_SETTINGS + 14).contains(&t) && t != TAG_TERMINATOR => {
                h.unported_chunk(t)
            }
            _ => {}
        }
        header = m.read32(frame.header_slot());
        payload = payload.wrapping_add(m.read32(header + 4));
        m.write32(frame.header_slot(), payload);
        header = payload;
        tag = m.read32(header);
        payload = header + 8;
    }
    if overlay == 0 {
        h.finish_load();
    }
    h.arena_release(m.read32(frame.block_slot()));
    if h.needs_dirty_flag() != 0 {
        let editor = m.read32(EDITOR_PTR);
        m.write32(
            editor + EDITOR_DIRTY_FLAGS,
            m.read32(editor + EDITOR_DIRTY_FLAGS) | 1,
        );
    }
    h.refresh_a();
    h.refresh_b();
    h.refresh_c();
    1
}
