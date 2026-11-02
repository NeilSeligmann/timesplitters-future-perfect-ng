//! data/save storage devices and memory units (T1899). Tier 0, confidence INFERRED (static
//! disassembly only). Outside calls (XDK/kernel, text, sprintf, arena) go through `SaveDeviceHost`.
//! Ported here: 0x24810 0x248a0 0x24900 0x24950 0x24b30 0x24d80 0x24e00 0x24e40 0x24e60 0x24eb0
//! 0x24ec0 0x24f40 0x25180 0x25670 0x25860 0x25980 0x25b80 0x25bb0 0x25be0 0x25c30 0x25d60 0x3f890
//! 0x827c0 0x827d0 0x827f0 0x82800 0x82820 0x190690 0x190a70 0x196030.
//! Not yet ported (debt): 0x25280 0x25710 0x25e40 0x25ff0 0x26320 0x263e0 0x26740 0x82950.
use crate::memory::GuestMemory;

/// Device table: 11 records of 0x30 bytes. +0 present, +1 and +2 flags, +3 drive letter, +4 status
/// (1 ok), +8 device mask, +0xc volume serial, +0x10 display name.
pub const DEVICE_TABLE: u32 = 0x0056_55b0;
pub const DEVICE_STRIDE: u32 = 0x30;
pub const DEVICE_COUNT: u32 = 11;
/// Byte fields of a device record; these offsets are shared by the mount and slot APIs.
pub const DEVICE_PRESENT: u32 = 0;
/// Transient insertion and removal flags maintained by the storage poll (VA 0x25980).
const DEVICE_INSERTED: u32 = 1;
const DEVICE_REMOVED: u32 = 2;
pub const DEVICE_DRIVE_LETTER: u32 = 3;
pub const DEVICE_STATUS: u32 = 4;
const DEVICE_MASK: u32 = 8;
const DEVICE_VOLUME_SERIAL: u32 = 0xc;
const DEVICE_DISPLAY_NAME: u32 = 0x10;
pub const SIGNATURE_BUFFER: u32 = 0x0056_51a8;
pub const SIGNATURE_LENGTH: u32 = 0x0056_55a8;
pub const SELECTED_SLOT: u32 = 0x007b_979c;
pub const SLOT_GLOBAL_7B9798: u32 = 0x007b_9798;
pub const FLAG_4D9658: u32 = 0x004d_9658;
pub const FLAG_7509F0: u32 = 0x0075_09f0;
pub const NO_DEVICE_RECORD: u32 = 0x0047_66d8;
pub const FILTER_TABLE: u32 = 0x004c_9040;
/// Literal format fragments in the original data section (pointers passed as sprintf arguments).
pub const LIT_4760E0: u32 = 0x0047_60e0;
pub const LIT_4760E4: u32 = 0x0047_60e4;
/// Text ids (label bank) used by the save error messages.
pub const TEXT_NAME_FORMAT: u32 = 0x198f;
pub const TEXT_NAME_SUFFIX: u32 = 0x199b;
pub const TEXT_DEVICE_FREE: u32 = 0x1973;
pub const TEXT_DEVICE_NAME: u32 = 0x1972;
/// Memory unit device masks: slot 2p has 1 << p, slot 2p+1 has 0x10000 << p.
pub const MAX_BLOCKS_DISPLAY: u32 = 0xc350;
/// Reset clears the complete eleven-record table before assigning the eight memory-unit masks.
const DEVICE_TABLE_WORDS: u32 = DEVICE_COUNT * DEVICE_STRIDE / 4;
const SIGNATURE_BUFFER_WORDS: u32 = 0x100;
const MEMORY_UNIT_SLOTS: u32 = 8;
const INTERNAL_STORAGE_SLOT: u32 = 9;
const CONTENT_STORAGE_SLOT: u32 = 10;
/// The mount path formats the unit name in the record and stages a volume label past it.
const MOUNT_NAME_FORMAT: u32 = 0x0047_60f8;
const MOUNT_NAME_CAPACITY: u32 = 0x1d;
const MOUNT_LABEL_CAPACITY: u32 = 0x0f;
const MOUNT_WIDE_LABEL_SCRATCH: u32 = 0x100;
const MOUNT_NARROW_LABEL_SCRATCH: u32 = 0x300;
const TRUNCATED_LABEL_SUFFIX: &[u8; 4] = b"...\0";
/// Two-byte control sequence emitted for a wide character rejected by the display filter.
const FILTERED_LABEL_CHAR: [u8; 2] = [0x0b, 0x38];
/// Text-bank selections for save error formatting at VA 0x24950. Meanings are inferred
/// from the existing branch arguments; the guest text itself is supplied at run time.
const TEXT_GENERIC_ERROR: u32 = 0x1988;
const TEXT_DELETE_ERROR: u32 = 0x1989;
const TEXT_DETAIL_ERROR: u32 = 0x1987;
const TEXT_KIND_DEFAULT: u32 = 0x197b;
const TEXT_KIND_SAVE: u32 = 0x197c;
const TEXT_KIND_LOAD: u32 = 0x197d;
const TEXT_KIND_DELETE: u32 = 0x197e;
const TEXT_SAVE_WITH_SUBJECT: u32 = 0x1980;
const TEXT_SAVE_WITHOUT_SUBJECT: u32 = 0x1985;
const TEXT_LOAD_WITH_SUBJECT: u32 = 0x1982;
const TEXT_LOAD_WITHOUT_SUBJECT: u32 = 0x1983;
const TEXT_DELETE_WITH_SUBJECT: u32 = 0x197f;
const TEXT_DELETE_WITHOUT_SUBJECT: u32 = 0x1984;

/// One printf-style argument (guest pointer or integer).
pub type FormatArg = u32;

/// Callees outside the data/save slice.
pub trait SaveDeviceHost {
    /// 0x37e89e signature size lookup for a size class.
    fn signature_size_lookup(&mut self, class: u32) -> u32;
    /// 0x37e900 create the signature handle (u32::MAX on failure).
    fn signature_open(&mut self, class: u32) -> u32;
    /// 0x37e9a7 last error code.
    fn last_error(&mut self) -> u32;
    /// 0x3dfa0 arena allocate (size, kind 0).
    fn arena_alloc(&mut self, size: u32) -> u32;
    /// 0x3d7b0 + 0x3d7f0 sprintf the text `text_id` with `args` into guest buffer `dst`.
    fn text_sprintf(&mut self, m: &mut dyn GuestMemory, dst: u32, text_id: u32, args: &[FormatArg]);
    /// 0x3d7b0 + byte copy: copy the text `text_id` to `dst`.
    fn text_copy(&mut self, m: &mut dyn GuestMemory, dst: u32, text_id: u32);
    /// Length of the text with the given id (0x3d7b0 then strlen).
    fn text_len(&mut self, text_id: u32) -> u32;
    /// 0x46d7c4 XMountMU(port, slot) -> (error code, drive letter).
    fn mount_memory_unit(&mut self, port: u32, slot: u32) -> (u32, u8);
    /// 0x46d8f6 XUnmountMU(port, slot) -> error code.
    fn unmount_memory_unit(&mut self, port: u32, slot: u32) -> u32;
    /// 0x37dbf1 volume label of a drive as wide characters, `None` when the call fails.
    fn volume_label(&mut self, letter: u8) -> Option<Vec<u16>>;
    /// 0x37d5ed volume serial of a drive written to guest address `serial_dst`.
    fn volume_serial(&mut self, m: &mut dyn GuestMemory, letter: u8, serial_dst: u32);
    /// 0x37e37d delete save game `name` on drive `letter`; returns the error code.
    fn delete_save_game(&mut self, letter: u8, name: &[u16]) -> u32;
    /// 0x46dbd2 connected memory unit mask.
    fn memory_unit_mask(&mut self) -> u32;
    /// 0x46dbf4 device changes: (insertions, removals) or `None` when nothing changed.
    fn device_changes(&mut self) -> Option<(u32, u32)>;
    /// 0x37d4f9 disk free space of a drive: (free bytes, total bytes) or `None` on failure.
    fn disk_space(&mut self, letter: u8) -> Option<(u64, u64)>;
    /// 0x35e710 enter net live mode (tail call of 0x196030).
    fn net_enter_live_mode(&mut self);
}

fn dev(index: u32) -> u32 {
    DEVICE_TABLE.wrapping_add(index.wrapping_mul(DEVICE_STRIDE))
}

/// 0x24810 reset the device table, signature buffer and default masks.
pub fn device_table_reset_defaults(m: &mut impl GuestMemory) {
    for i in 0..DEVICE_TABLE_WORDS {
        m.write32(DEVICE_TABLE + i * 4, 0);
    }
    for i in 0..SIGNATURE_BUFFER_WORDS {
        m.write32(SIGNATURE_BUFFER + i * 4, 0);
    }
    m.write32(SIGNATURE_LENGTH, 0);
    for slot in 0..MEMORY_UNIT_SLOTS {
        let port = slot >> 1;
        let mask = if slot & 1 == 0 {
            1 << port
        } else {
            0x10000 << port
        };
        m.write32(dev(slot) + DEVICE_MASK, mask);
    }
    m.write8(dev(INTERNAL_STORAGE_SLOT) + DEVICE_PRESENT, 1);
    m.write8(dev(INTERNAL_STORAGE_SLOT) + DEVICE_DRIVE_LETTER, b'U');
    m.write8(dev(CONTENT_STORAGE_SLOT) + DEVICE_PRESENT, 1);
    m.write8(dev(CONTENT_STORAGE_SLOT) + DEVICE_DRIVE_LETTER, b'T');
}

/// 0x3f890 character passes the range filter (printable, not mapped to underscore 0x5f).
pub fn word_char_passes_filter(m: &impl GuestMemory, ch: u16) -> bool {
    if ch < 0x20 {
        return false;
    }
    if ch <= 0x80 {
        return true;
    }
    let index = u32::from(ch) - 0x80;
    if index > 0x80 {
        return false;
    }
    m.read8(FILTER_TABLE + index) != 0x5f
}

/// 0x248a0 copy a wide device label into a narrow buffer of `capacity` bytes.
/// Rejected characters use the two-byte display sequence `FILTERED_LABEL_CHAR`. The guest
/// routine reserves one byte for a terminator and reports whether it consumed the whole source.
pub fn word_string_copy(m: &mut impl GuestMemory, capacity: u32, src: u32, dst: u32) -> bool {
    let mut remaining = capacity.wrapping_sub(1);
    let mut source = src;
    let mut destination = dst;
    let mut character = m.read16(source);
    while character != 0 && remaining != 0 {
        if !word_char_passes_filter(m, character) {
            source += 2;
            if remaining > 1 {
                m.write8(destination, FILTERED_LABEL_CHAR[0]);
                m.write8(destination + 1, FILTERED_LABEL_CHAR[1]);
                destination += 2;
                remaining -= 1;
            } else {
                m.write8(destination, 0);
                destination += 1;
            }
        } else {
            m.write8(destination, m.read8(source));
            destination += 1;
            source += 2;
        }
        character = m.read16(source);
        remaining = remaining.wrapping_sub(1);
    }
    m.write8(destination, 0);
    m.read16(source) == 0
}

/// 0x24900 map an XDK error code to a device status (1 ok, 2, 3, 6, 0xc) or `default`.
pub fn error_to_status(error: u32, default: u32) -> u32 {
    match error {
        0 => 1,
        0x55 | 0x0e | 0x52 => 3,
        0x70 => 2,
        0x48f => 6,
        0x3ed => 0xc,
        _ => default,
    }
}

/// Text with the optional argument pattern shared by the load, save and delete messages.
fn device_text(
    m: &mut dyn GuestMemory,
    h: &mut impl SaveDeviceHost,
    buf: u32,
    with_arg: u32,
    without_arg: u32,
    arg: u32,
    name: u32,
) {
    if arg == 0 {
        h.text_sprintf(m, buf, without_arg, &[name]);
    } else {
        h.text_sprintf(m, buf, with_arg, &[LIT_4760E0, arg, name]);
    }
}

/// 0x24950 save error message text by device, code and kind. Returns `buf`.
pub fn save_error_message_text(
    m: &mut impl GuestMemory,
    h: &mut impl SaveDeviceHost,
    device: u32,
    code: u32,
    buf: u32,
    kind: u32,
    subject: u32,
    text: u32,
) -> u32 {
    let record = if device < DEVICE_COUNT {
        dev(device)
    } else {
        0
    };
    let name = record.wrapping_add(DEVICE_DISPLAY_NAME);
    let kind_text = |kind: u32| match kind {
        1 => TEXT_KIND_SAVE,
        2 => TEXT_KIND_LOAD,
        3 => TEXT_KIND_DELETE,
        _ => TEXT_KIND_DEFAULT,
    };
    match code {
        1 | 2 => {}
        4 | 6 => {
            if record != 0 {
                h.text_sprintf(m, buf, kind_text(kind), &[LIT_4760E4, name]);
            }
        }
        5 => h.text_copy(m, buf, TEXT_DELETE_ERROR),
        7 | 12 => {
            if text != 0 {
                h.text_sprintf(m, buf, TEXT_DETAIL_ERROR, &[LIT_4760E4, text]);
            } else {
                h.text_copy(m, buf, TEXT_GENERIC_ERROR);
            }
        }
        8 => {
            if record != 0 {
                device_text(
                    m,
                    h,
                    buf,
                    TEXT_SAVE_WITH_SUBJECT,
                    TEXT_SAVE_WITHOUT_SUBJECT,
                    subject,
                    name,
                );
            }
        }
        11 => {
            if record != 0 {
                device_text(
                    m,
                    h,
                    buf,
                    TEXT_DELETE_WITH_SUBJECT,
                    TEXT_DELETE_WITHOUT_SUBJECT,
                    subject,
                    name,
                );
            }
        }
        _ => {
            if record != 0 {
                match kind {
                    1 => device_text(
                        m,
                        h,
                        buf,
                        TEXT_SAVE_WITH_SUBJECT,
                        TEXT_SAVE_WITHOUT_SUBJECT,
                        subject,
                        name,
                    ),
                    2 => device_text(
                        m,
                        h,
                        buf,
                        TEXT_LOAD_WITH_SUBJECT,
                        TEXT_LOAD_WITHOUT_SUBJECT,
                        subject,
                        name,
                    ),
                    3 => device_text(
                        m,
                        h,
                        buf,
                        TEXT_DELETE_WITH_SUBJECT,
                        TEXT_DELETE_WITHOUT_SUBJECT,
                        subject,
                        name,
                    ),
                    _ => h.text_copy(m, buf, TEXT_GENERIC_ERROR),
                }
            }
        }
    }
    buf
}

/// 0x24b30 mount memory unit slot `index`: mounts it, builds the display name and volume serial,
/// and stores the mapped status at +4.
pub fn memory_unit_mount(m: &mut impl GuestMemory, h: &mut impl SaveDeviceHost, index: u32) {
    let record = dev(index);
    if m.read8(record + DEVICE_PRESENT) == 0 {
        return;
    }
    let (port, slot) = (index >> 1, index & 1);
    let (error, letter) = h.mount_memory_unit(port, slot);
    let name = record + DEVICE_DISPLAY_NAME;
    let name_args = [MOUNT_NAME_FORMAT, slot_digit(port), slot_letter(slot)];
    let status_error;
    if error != 0 {
        h.text_sprintf(
            m,
            name,
            TEXT_NAME_FORMAT,
            &[name_args[0], name_args[1], name_args[2]],
        );
        append_text(m, h, name, TEXT_NAME_SUFFIX);
        m.write8(record + DEVICE_DRIVE_LETTER, 0);
        status_error = error;
    } else {
        m.write8(record + DEVICE_DRIVE_LETTER, letter);
        let budget = MOUNT_NAME_CAPACITY
            .wrapping_sub(h.text_len(TEXT_NAME_FORMAT))
            .min(MOUNT_LABEL_CAPACITY);
        h.text_sprintf(m, name, TEXT_NAME_FORMAT, &name_args);
        if let Some(label) = h
            .volume_label(letter)
            .filter(|l| l.first().is_some_and(|c| *c != 0))
        {
            let scratch = record + MOUNT_WIDE_LABEL_SCRATCH;
            for (i, c) in label.iter().chain(std::iter::once(&0)).enumerate() {
                m.write8(scratch + 2 * i as u32, *c as u8);
                m.write8(scratch + 2 * i as u32 + 1, (*c >> 8) as u8);
            }
            let narrow = record + MOUNT_NARROW_LABEL_SCRATCH;
            if !word_string_copy(m, budget, scratch, narrow) {
                for (i, b) in TRUNCATED_LABEL_SUFFIX.iter().enumerate() {
                    m.write8(narrow + budget.wrapping_sub(4) + i as u32, *b);
                }
            }
            let end = c_string_end(m, name);
            m.write8(end, b' ');
            let mut pos = end + 1;
            let mut at = narrow;
            while m.read8(at) != 0 {
                m.write8(pos, m.read8(at));
                pos += 1;
                at += 1;
            }
            m.write8(pos, 0);
        }
        h.volume_serial(m, letter, record + DEVICE_VOLUME_SERIAL);
        status_error = 0;
    }
    m.write32(record + DEVICE_STATUS, error_to_status(status_error, 1));
}

fn slot_digit(port: u32) -> u32 {
    port + 0x31
}
fn slot_letter(slot: u32) -> u32 {
    slot + 0x41
}
fn c_string_end(m: &impl GuestMemory, mut at: u32) -> u32 {
    while m.read8(at) != 0 {
        at += 1;
    }
    at
}
fn append_text(m: &mut impl GuestMemory, h: &mut impl SaveDeviceHost, dst: u32, text_id: u32) {
    let end = c_string_end(m, dst);
    h.text_copy(m, end, text_id);
}

/// 0x24d80 unmount memory unit slot `index`, clearing the drive letter and mapping the status.
pub fn memory_unit_unmount(m: &mut impl GuestMemory, h: &mut impl SaveDeviceHost, index: u32) {
    let record = dev(index);
    let error = h.unmount_memory_unit(index >> 1, index & 1);
    m.write8(record + DEVICE_DRIVE_LETTER, 0);
    m.write32(record + DEVICE_STATUS, error_to_status(error, 1));
}

/// 0x24e00 unmount every memory unit record (the first 8, loop bound 0x180 / 0x30) that is present
/// and mounted. Found by the Tier 1 differential: an 11 record loop was wrong.
pub fn memory_unit_unmount_all(m: &mut impl GuestMemory, h: &mut impl SaveDeviceHost) {
    for index in 0..8 {
        if m.read8(dev(index) + DEVICE_PRESENT) != 0
            && m.read8(dev(index) + DEVICE_DRIVE_LETTER) != 0
        {
            memory_unit_unmount(m, h, index);
        }
    }
}

/// 0x24e40 status dword (+4) of a device record (no range check).
pub fn memory_unit_slot_status(m: &impl GuestMemory, index: u32) -> u32 {
    m.read32(dev(index) + DEVICE_STATUS)
}

/// 0x24e60 device is present and mounted.
pub fn memory_unit_slot_is_active(m: &impl GuestMemory, index: u32) -> bool {
    index < DEVICE_COUNT
        && m.read8(dev(index) + DEVICE_PRESENT) != 0
        && m.read8(dev(index) + DEVICE_DRIVE_LETTER) != 0
}

/// 0x24eb0 address of the device record field +0x10 (display name).
pub fn memory_unit_slot_name_addr(index: u32) -> u32 {
    dev(index) + DEVICE_DISPLAY_NAME
}

/// 0x24ec0 call `callback(ctx, index, name_addr)` for each present device, starting at `start`
/// (out of range starts at 9) and wrapping around the 11 records.
pub fn save_slots_enumerate(m: &impl GuestMemory, start: i32, mut callback: impl FnMut(u32, u32)) {
    let first = if (0..DEVICE_COUNT as i32).contains(&start) {
        start as u32
    } else {
        9
    };
    let mut index = first;
    loop {
        if m.read8(dev(index) + DEVICE_PRESENT) != 0 {
            callback(index, dev(index) + DEVICE_DISPLAY_NAME);
        }
        index = (index + 1) % DEVICE_COUNT;
        if index == first {
            break;
        }
    }
}

/// Signature state used by 0x24f40: `{handle, size, size_class, buffer}`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SignatureState {
    pub handle: u32,
    pub size: u32,
    pub size_class: u32,
    pub buffer: u32,
}

/// 0x24f40 `game_save_signature_begin` (custom ABI: state in eax, flags in ebx). Returns 1 on
/// success or a mapped error status.
pub fn save_signature_begin(
    m: &mut impl GuestMemory,
    h: &mut impl SaveDeviceHost,
    state: &mut SignatureState,
    flags: u32,
) -> u32 {
    if flags & 6 == 0 {
        return 1;
    }
    state.size_class = u32::from(flags & 8 != 0);
    if flags & 0x4000_0000 != 0 {
        state.size_class |= 4;
    } else if flags & 4 != 0 {
        state.size_class |= 8;
    }
    state.size = h.signature_size_lookup(state.size_class);
    if flags & 0x8000_0020 == 0 {
        state.handle = h.signature_open(state.size_class);
        if state.handle == u32::MAX {
            let _ = h.last_error();
            return error_to_status(3, 3);
        }
    }
    if flags & 0x20 == 0 {
        state.buffer = h.arena_alloc(state.size);
        for i in 0..state.size {
            m.write8(state.buffer.wrapping_add(i), 0);
        }
    } else {
        let length = m.read32(SIGNATURE_LENGTH);
        state.buffer = h.arena_alloc(length);
        for i in 0..length {
            m.write8(state.buffer.wrapping_add(i), m.read8(SIGNATURE_BUFFER + i));
        }
    }
    1
}

/// 0x25180 load the signature buffer from a state record (+4 size, +0xc source pointer).
pub fn signature_buffer_load(m: &mut impl GuestMemory, state: &SignatureState) {
    if state.buffer == 0 || state.size == 0 {
        return;
    }
    for i in 0..0x100 {
        m.write32(SIGNATURE_BUFFER + i * 4, 0);
    }
    m.write32(SIGNATURE_LENGTH, state.size);
    for i in 0..state.size.min(0x400) {
        m.write8(SIGNATURE_BUFFER + i, m.read8(state.buffer.wrapping_add(i)));
    }
}

/// 0x25670 delete a save game by name. Status 6 when the device is absent or unmounted. The default
/// status for unmapped errors is 0xb: the original pushes 0xb below the arguments of the stdcall
/// delete, which pops only its own two, and `error_to_status` reads the leftover word as its default
/// (T1936 correction of the earlier "saved ESI" reading).
pub fn save_game_delete(
    m: &mut impl GuestMemory,
    h: &mut impl SaveDeviceHost,
    index: u32,
    name: &[u8],
) -> u32 {
    let record = dev(index);
    if index >= DEVICE_COUNT || m.read8(record) == 0 || m.read8(record + 3) == 0 {
        m.write32(record + 4, 6);
        return 6;
    }
    let wide: Vec<u16> = name.iter().map(|b| u16::from(*b)).collect();
    let status = error_to_status(h.delete_save_game(m.read8(record + 3), &wide), 0xb);
    m.write32(record + 4, status);
    status
}

/// 0x25860 enumerate the storage devices: volume serials of the fixed drives and the presence
/// flags of the 8 memory unit records from the connected-device mask.
pub fn storage_devices_enumerate(m: &mut impl GuestMemory, h: &mut impl SaveDeviceHost) {
    let internal = dev(INTERNAL_STORAGE_SLOT);
    let content = dev(CONTENT_STORAGE_SLOT);
    let internal_letter = m.read8(internal + DEVICE_DRIVE_LETTER);
    let content_letter = m.read8(content + DEVICE_DRIVE_LETTER);
    h.volume_serial(m, internal_letter, internal + DEVICE_VOLUME_SERIAL);
    h.volume_serial(m, content_letter, content + DEVICE_VOLUME_SERIAL);
    let connected = h.memory_unit_mask();
    for index in 0..MEMORY_UNIT_SLOTS {
        let record = dev(index);
        m.write8(record + DEVICE_PRESENT, 0);
        m.write8(record + DEVICE_INSERTED, 0);
        m.write8(record + DEVICE_REMOVED, 0);
        m.write32(record + DEVICE_STATUS, 1);
        if m.read32(record + DEVICE_MASK) & connected != 0 {
            m.write8(record + DEVICE_PRESENT, 1);
            m.write8(record + DEVICE_INSERTED, 1);
            m.write8(record + DEVICE_DRIVE_LETTER, 0);
        }
    }
}

/// 0x25980 poll device insertions and removals, unmounting removed and mounting newly present
/// memory units. Returns removals | insertions.
pub fn storage_device_changes_poll(m: &mut impl GuestMemory, h: &mut impl SaveDeviceHost) -> u32 {
    h.text_copy(
        m,
        dev(INTERNAL_STORAGE_SLOT) + DEVICE_DISPLAY_NAME,
        TEXT_DEVICE_NAME,
    );
    h.text_copy(
        m,
        dev(CONTENT_STORAGE_SLOT) + DEVICE_DISPLAY_NAME,
        TEXT_DEVICE_NAME,
    );
    let changes = h.device_changes();
    let changed = changes.is_some();
    let (inserted, removed) = changes.unwrap_or((0, 0));
    if changed {
        for index in 0..MEMORY_UNIT_SLOTS {
            let record = dev(index);
            m.write8(record + DEVICE_INSERTED, 0);
            m.write8(record + DEVICE_REMOVED, 0);
            if m.read32(record + DEVICE_MASK) & removed != 0 {
                m.write8(record + DEVICE_PRESENT, 0);
                m.write8(record + DEVICE_REMOVED, 1);
                if m.read8(record + DEVICE_DRIVE_LETTER) != 0 {
                    memory_unit_unmount(m, h, index);
                }
            }
        }
        for index in 0..MEMORY_UNIT_SLOTS {
            let record = dev(index);
            if m.read32(record + DEVICE_MASK) & inserted != 0 {
                m.write8(record + DEVICE_PRESENT, 1);
                m.write8(record + DEVICE_INSERTED, 1);
                m.write8(record + DEVICE_DRIVE_LETTER, 0);
                m.write32(record + DEVICE_STATUS, 1);
            }
        }
    }
    for index in 0..MEMORY_UNIT_SLOTS {
        let record = dev(index);
        if m.read8(record + DEVICE_PRESENT) != 0
            && m.read8(record + DEVICE_DRIVE_LETTER) == 0
            && m.read32(record + DEVICE_STATUS) == 1
        {
            memory_unit_mount(m, h, index);
        }
    }
    removed | inserted
}

/// 0x25b80 drive letter of an active device, else 0.
pub fn memory_unit_slot_drive_letter(m: &impl GuestMemory, index: u32) -> u8 {
    if memory_unit_slot_is_active(m, index) {
        m.read8(dev(index) + 3)
    } else {
        0
    }
}

/// 0x25bb0 volume serial (+0xc) of an active device, else 0.
pub fn memory_unit_slot_serial(m: &impl GuestMemory, index: u32) -> u32 {
    if memory_unit_slot_is_active(m, index) {
        m.read32(dev(index) + 0xc)
    } else {
        0
    }
}

/// 0x25be0 number of 16 KiB blocks a save of `size` bytes needs (0 size means the default
/// content layout).
pub fn save_block_count_for_size(h: &mut impl SaveDeviceHost, size: u32) -> u32 {
    let base = h
        .signature_size_lookup(8)
        .wrapping_add(h.signature_size_lookup(0));
    let (total, extra) = if size == 0 {
        (base.wrapping_mul(2).wrapping_add(0x9f54), 5)
    } else {
        (base.wrapping_add(size), 2)
    };
    (total >> 14).wrapping_add(extra) + u32::from(total & 0x3fff != 0)
}

/// 0x25c30 query free and total space of a device in 16 KiB blocks: `(ok, free, total)`.
pub fn storage_free_space_query(
    m: &mut impl GuestMemory,
    h: &mut impl SaveDeviceHost,
    index: u32,
) -> (bool, u32, u32) {
    let record = dev(index);
    let mut result = (false, 0, 0);
    if index < DEVICE_COUNT && m.read8(record) != 0 && m.read8(record + 3) != 0 {
        if let Some((free, total)) = h.disk_space(m.read8(record + 3)) {
            result = (true, (free >> 14) as u32, (total >> 14) as u32);
        }
    }
    let error = h.last_error();
    m.write32(record + 4, error_to_status(error, 1));
    result
}

/// 0x25d60 format "free/total" block counts into `dst`; false (empty string) when the query fails.
pub fn storage_free_space_format(
    m: &mut impl GuestMemory,
    h: &mut impl SaveDeviceHost,
    index: u32,
    dst: u32,
) -> bool {
    let (ok, free, total) = storage_free_space_query(m, h, index);
    if !ok {
        m.write8(dst, 0);
        return false;
    }
    let scratch = dst.wrapping_add(0x1000);
    let first = scratch;
    let second = scratch + 0x20;
    write_count(m, first, total);
    write_count(m, second, free);
    h.text_sprintf(m, dst, TEXT_DEVICE_FREE, &[LIT_4760E0, first, second]);
    true
}

fn write_count(m: &mut impl GuestMemory, dst: u32, value: u32) {
    let text = if value > MAX_BLOCKS_DISPLAY {
        ">50000".to_owned()
    } else {
        value.to_string()
    };
    for (i, b) in text.bytes().chain(std::iter::once(0)).enumerate() {
        m.write8(dst + i as u32, b);
    }
}

/// 0x827c0 global 0x7b9798.
pub fn global_7b9798_get(m: &impl GuestMemory) -> u32 {
    m.read32(SLOT_GLOBAL_7B9798)
}

/// 0x827d0 set the selected slot; -1 also raises flag 0x4d9658.
pub fn memory_unit_slot_set(m: &mut impl GuestMemory, slot: u32) {
    if slot == u32::MAX {
        m.write32(FLAG_4D9658, 1);
    }
    m.write32(SELECTED_SLOT, slot);
}

/// 0x827f0 selected slot.
pub fn memory_unit_slot_get(m: &impl GuestMemory) -> u32 {
    m.read32(SELECTED_SLOT)
}

/// 0x82800 name record address for `index` or the default record for -1.
pub fn memory_unit_slot_addr_or_default(index: u32) -> u32 {
    if index == u32::MAX {
        NO_DEVICE_RECORD
    } else {
        memory_unit_slot_name_addr(index)
    }
}

/// 0x82820 same for the currently selected slot.
pub fn memory_unit_slot_addr_or_default_for_selected(m: &impl GuestMemory) -> u32 {
    memory_unit_slot_addr_or_default(m.read32(SELECTED_SLOT))
}

/// 0x190690 the selected slot is active: 1, else 6.
pub fn memory_unit_current_slot_status(m: &impl GuestMemory) -> u32 {
    if memory_unit_slot_is_active(m, m.read32(SELECTED_SLOT)) {
        1
    } else {
        6
    }
}

/// 0x190a70 status dword of the selected slot.
pub fn memory_unit_current_slot_field_0x4(m: &impl GuestMemory) -> u32 {
    memory_unit_slot_status(m, m.read32(SELECTED_SLOT))
}

/// 0x196030 unmount everything, set flag 0x7509f0 and enter net live mode.
pub fn memory_unit_unmount_all_then_net_live(
    m: &mut impl GuestMemory,
    h: &mut impl SaveDeviceHost,
) {
    memory_unit_unmount_all(m, h);
    m.write32(FLAG_7509F0, 1);
    h.net_enter_live_mode();
}

/// Message buffer returned by `memory_unit_message_text`.
pub const MESSAGE_BUFFER: u32 = 0x0070_2840;
pub const MESSAGE_LIMIT: u32 = 0x200;
pub const LIT_47D28C: u32 = 0x0047_d28c;

/// Callees of 0x82950 outside the slice.
pub trait MessageHost {
    /// 0x24950 `save_error_message_text` (device, code, buffer, kind, subject, text).
    fn error_message(
        &mut self,
        m: &mut dyn GuestMemory,
        device: u32,
        code: u32,
        buffer: u32,
        kind: u32,
        subject: u32,
        text: u32,
    );
    /// 0x3d7b0 text pointer by id.
    fn text_ptr(&mut self, id: u32) -> u32;
    /// 0x3d7f0 sprintf the template pointer `template` into `buffer` with `args`.
    fn sprintf_ptr(&mut self, m: &mut dyn GuestMemory, buffer: u32, template: u32, args: &[u32]);
    /// 0x3c85d4 sprintf (no varargs) of a template pointer into `buffer`.
    fn sprintf_plain(&mut self, m: &mut dyn GuestMemory, buffer: u32, template: u32);
    /// 0x3c8960 bounded copy of a text pointer into `buffer`.
    fn copy_limited(&mut self, m: &mut dyn GuestMemory, buffer: u32, text: u32, limit: u32);
    /// 0x82820 name record address of the selected slot.
    fn selected_slot_name(&mut self, m: &mut dyn GuestMemory) -> u32;
    /// 0x828c0, 0x828f0 and 0x82920 return text pointers / templates (ids unknown, INFERRED).
    fn fn_000828c0(&mut self) -> u32;
    fn fn_000828f0(&mut self) -> u32;
    fn fn_00082920(&mut self) -> u32;
}

/// 0x82950 memory unit message text by case 0..0x2e formatted into the buffer at 0x702840.
/// `arg` is the second stack argument used by cases 0x1a, 0x1b and 0x27..0x2a.
pub fn memory_unit_message_text(
    m: &mut impl GuestMemory,
    h: &mut impl MessageHost,
    case: u32,
    arg: u32,
) -> u32 {
    let (selected, global) = (m.read32(SELECTED_SLOT), m.read32(SLOT_GLOBAL_7B9798));
    let buffer = MESSAGE_BUFFER;
    // (code, kind) of the plain error message cases.
    let plain = |code: u32, kind: u32| Some((code, kind));
    let message = match case {
        0..=2 => plain(4, global),
        4 => plain(8, 1),
        6 | 7 => plain(3, 2),
        13 => plain(0xb, 3),
        15 | 16 => plain(6, 3),
        23 | 24 => plain(6, global),
        43 => plain(3, global),
        _ => None,
    };
    if let Some((code, kind)) = message {
        h.error_message(m, selected, code, buffer, kind, 0, 0);
        return buffer;
    }
    // Plain text copies (bounded to 0x200 bytes).
    let copy_id = match case {
        3 => Some(0xea4),
        5 => Some(0xe6f),
        10 => Some(0xe72),
        11 => Some(0xe74),
        12 => Some(0xe73),
        14 => Some(0xe76),
        20 => Some(0x1a12),
        25 => Some(0xac8),
        29 | 30 | 33..=36 => Some(0xe7b),
        c if c > 0x2e => Some(0xe7b),
        _ => None,
    };
    if let Some(id) = copy_id {
        let text = h.text_ptr(id);
        h.copy_limited(m, buffer, text, MESSAGE_LIMIT);
        return buffer;
    }
    match case {
        8 => {
            let template = h.text_ptr(0xe71);
            h.sprintf_plain(m, buffer, template);
        }
        21 => {
            let text = h.text_ptr(0x198d);
            h.error_message(m, selected, 7, buffer, global, 0, text);
        }
        26 | 27 => {
            let text = h.text_ptr(0x198e);
            h.error_message(m, selected, 7, buffer, global, arg, text);
        }
        9 | 17 | 18 | 19 | 22 | 28 | 31 | 37 => {
            let id = match case {
                9 => 0x3ad,
                17 => 0x3aa,
                18 => 0x3a9,
                19 => 0xe67,
                22 => 0xe78,
                28 => 0xac9,
                31 => 0x3a5,
                _ => 0x3a4,
            };
            let name = h.selected_slot_name(m);
            let template = h.text_ptr(id);
            h.sprintf_ptr(m, buffer, template, &[LIT_4760E4, name]);
        }
        32 => {
            let name = h.selected_slot_name(m);
            let template = h.text_ptr(0x3cd);
            h.sprintf_ptr(m, buffer, template, &[LIT_47D28C, name, 0x32]);
        }
        39 | 40 | 41 | 42 => {
            let (id, literal) = match case {
                39 => (0x3a6, LIT_4760E0),
                40 => (0xace, LIT_4760E0),
                41 => (0xacd, LIT_4760E4),
                _ => (0xad2, LIT_4760E0),
            };
            let name = h.selected_slot_name(m);
            let template = h.text_ptr(id);
            h.sprintf_ptr(m, buffer, template, &[literal, arg, name]);
        }
        44 => {
            let text = h.fn_000828c0();
            h.copy_limited(m, buffer, text, MESSAGE_LIMIT);
        }
        45 | 46 => {
            let name = h.selected_slot_name(m);
            let template = if case == 45 {
                h.fn_000828f0()
            } else {
                h.fn_00082920()
            };
            h.sprintf_ptr(m, buffer, template, &[LIT_4760E4, name]);
        }
        _ => {}
    }
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Mem(HashMap<u32, u8>);
    impl GuestMemory for Mem {
        fn read8(&self, a: u32) -> u8 {
            *self.0.get(&a).unwrap_or(&0)
        }
        fn write8(&mut self, a: u32, v: u8) {
            self.0.insert(a, v);
        }
    }
    #[derive(Default)]
    struct Host {
        texts: Vec<(u32, Vec<u32>)>,
        mount: (u32, u8),
        unmounted: Vec<(u32, u32)>,
        changes: Option<(u32, u32)>,
        mask: u32,
        error: u32,
        space: Option<(u64, u64)>,
        live: bool,
    }
    impl SaveDeviceHost for Host {
        fn signature_size_lookup(&mut self, c: u32) -> u32 {
            c * 0x1000
        }
        fn signature_open(&mut self, _c: u32) -> u32 {
            7
        }
        fn last_error(&mut self) -> u32 {
            self.error
        }
        fn arena_alloc(&mut self, _s: u32) -> u32 {
            0x9000
        }
        fn text_sprintf(&mut self, _m: &mut dyn GuestMemory, _d: u32, id: u32, a: &[FormatArg]) {
            self.texts.push((id, a.to_vec()));
        }
        fn text_copy(&mut self, _m: &mut dyn GuestMemory, _d: u32, id: u32) {
            self.texts.push((id, vec![]));
        }
        fn text_len(&mut self, _id: u32) -> u32 {
            5
        }
        fn mount_memory_unit(&mut self, _p: u32, _s: u32) -> (u32, u8) {
            self.mount
        }
        fn unmount_memory_unit(&mut self, p: u32, s: u32) -> u32 {
            self.unmounted.push((p, s));
            0
        }
        fn volume_label(&mut self, _l: u8) -> Option<Vec<u16>> {
            None
        }
        fn volume_serial(&mut self, m: &mut dyn GuestMemory, _l: u8, dst: u32) {
            m.write8(dst, 0x42);
        }
        fn delete_save_game(&mut self, _l: u8, _n: &[u16]) -> u32 {
            0x70
        }
        fn memory_unit_mask(&mut self) -> u32 {
            self.mask
        }
        fn device_changes(&mut self) -> Option<(u32, u32)> {
            self.changes
        }
        fn disk_space(&mut self, _l: u8) -> Option<(u64, u64)> {
            self.space
        }
        fn net_enter_live_mode(&mut self) {
            self.live = true;
        }
    }

    #[test]
    fn reset_status_and_filters() {
        let mut m = Mem::default();
        device_table_reset_defaults(&mut m);
        assert_eq!(m.read32(dev(1) + 8), 0x10000);
        assert_eq!(m.read32(dev(7) + 8), 0x80000);
        assert_eq!((m.read8(dev(9) + 3), m.read8(dev(10) + 3)), (0x55, 0x54));
        assert_eq!(error_to_status(0x48f, 9), 6);
        assert_eq!(error_to_status(77, 9), 9);
        assert!(!word_char_passes_filter(&m, 0x10));
        assert!(word_char_passes_filter(&m, 0x41));
        assert!(!word_char_passes_filter(&m, 0x200));
        m.write8(FILTER_TABLE + 1, 0x5f);
        assert!(!word_char_passes_filter(&m, 0x81));
        assert!(word_char_passes_filter(&m, 0x82));
    }

    #[test]
    fn word_copy_replaces_bad_characters() {
        let mut m = Mem::default();
        for (i, c) in [0x41u16, 0x10, 0x42, 0].iter().enumerate() {
            m.write8(0x100 + 2 * i as u32, *c as u8);
        }
        assert!(word_string_copy(&mut m, 8, 0x100, 0x200));
        let out: Vec<u8> = (0..5).map(|i| m.read8(0x200 + i)).collect();
        assert_eq!(out, vec![0x41, 0x0b, 0x38, 0x42, 0]);
        assert!(!word_string_copy(&mut m, 2, 0x100, 0x300));
    }

    #[test]
    fn mount_unmount_and_active() {
        let mut m = Mem::default();
        let mut h = Host {
            mount: (0, b'F'),
            ..Default::default()
        };
        device_table_reset_defaults(&mut m);
        m.write8(dev(3), 1);
        memory_unit_mount(&mut m, &mut h, 3);
        assert_eq!(m.read8(dev(3) + 3), b'F');
        assert_eq!(m.read32(dev(3) + 4), 1);
        assert_eq!(m.read8(dev(3) + 0xc), 0x42);
        assert!(memory_unit_slot_is_active(&m, 3));
        assert_eq!(memory_unit_slot_drive_letter(&m, 3), b'F');
        memory_unit_unmount_all(&mut m, &mut h);
        assert!(h.unmounted.contains(&(1, 1)));
        assert!(!memory_unit_slot_is_active(&m, 3));
        h.mount = (0x70, 0);
        memory_unit_mount(&mut m, &mut h, 3);
        assert_eq!(m.read32(dev(3) + 4), 2);
        memory_unit_unmount_all_then_net_live(&mut m, &mut h);
        assert!(h.live && m.read32(FLAG_7509F0) == 1);
    }

    #[test]
    fn enumerate_poll_and_selection() {
        let mut m = Mem::default();
        let mut h = Host {
            mask: 0x1,
            mount: (0, b'G'),
            ..Default::default()
        };
        device_table_reset_defaults(&mut m);
        storage_devices_enumerate(&mut m, &mut h);
        assert_eq!(m.read8(dev(0)), 1);
        assert_eq!(m.read8(dev(1)), 0);
        h.changes = Some((0x10000, 0));
        let out = storage_device_changes_poll(&mut m, &mut h);
        assert_eq!(out, 0x10000);
        assert_eq!(m.read8(dev(1) + 3), b'G');
        let mut seen = vec![];
        save_slots_enumerate(&m, -5, |i, _| seen.push(i));
        assert_eq!(seen.first(), Some(&9));
        assert_eq!(seen.len(), 4);
        memory_unit_slot_set(&mut m, u32::MAX);
        assert_eq!(m.read32(FLAG_4D9658), 1);
        assert_eq!(
            memory_unit_slot_addr_or_default_for_selected(&m),
            NO_DEVICE_RECORD
        );
        memory_unit_slot_set(&mut m, 1);
        assert_eq!(memory_unit_current_slot_status(&m), 1);
        assert_eq!(memory_unit_current_slot_field_0x4(&m), 1);
        assert_eq!(memory_unit_slot_addr_or_default(2), dev(2) + 0x10);
    }

    #[test]
    fn space_blocks_signature_and_messages() {
        let mut m = Mem::default();
        let mut h = Host {
            space: Some((16384 * 7, 16384 * 9)),
            ..Default::default()
        };
        device_table_reset_defaults(&mut m);
        assert_eq!(storage_free_space_query(&mut m, &mut h, 9), (true, 7, 9));
        assert!(storage_free_space_format(&mut m, &mut h, 9, 0x4000));
        assert_eq!(h.texts.last().map(|t| t.0), Some(TEXT_DEVICE_FREE));
        h.space = None;
        assert!(!storage_free_space_format(&mut m, &mut h, 9, 0x4000));
        assert_eq!(
            save_block_count_for_size(&mut h, 0),
            (((8 + 0) * 0x1000 * 2 + 0x9f54) >> 14) + 5 + 1
        );
        let mut st = SignatureState::default();
        assert_eq!(save_signature_begin(&mut m, &mut h, &mut st, 0), 1);
        assert_eq!(save_signature_begin(&mut m, &mut h, &mut st, 8 | 4), 1);
        assert_eq!((st.size_class, st.handle, st.buffer), (9, 7, 0x9000));
        signature_buffer_load(
            &mut m,
            &SignatureState {
                size: 2,
                buffer: 0x5000,
                ..Default::default()
            },
        );
        assert_eq!(m.read32(SIGNATURE_LENGTH), 2);
        assert_eq!(save_game_delete(&mut m, &mut h, 20, b"x"), 6);
        let r = save_error_message_text(&mut m, &mut h, 1, 5, 0x100, 0, 0, 0);
        assert_eq!((r, h.texts.last().map(|t| t.0)), (0x100, Some(0x1989)));
        save_error_message_text(&mut m, &mut h, 1, 3, 0x100, 2, 5, 0);
        assert_eq!(h.texts.last().map(|t| t.0), Some(0x1982));
        save_error_message_text(&mut m, &mut h, 1, 12, 0x100, 0, 0, 9);
        assert_eq!(h.texts.last().map(|t| t.0), Some(0x1987));
    }

    #[derive(Default)]
    struct MsgHost {
        log: Vec<(&'static str, Vec<u32>)>,
    }
    impl MessageHost for MsgHost {
        fn error_message(
            &mut self,
            _m: &mut dyn GuestMemory,
            d: u32,
            c: u32,
            b: u32,
            k: u32,
            s: u32,
            t: u32,
        ) {
            self.log.push(("err", vec![d, c, b, k, s, t]));
        }
        fn text_ptr(&mut self, id: u32) -> u32 {
            0x1000 + id
        }
        fn sprintf_ptr(&mut self, _m: &mut dyn GuestMemory, b: u32, t: u32, a: &[u32]) {
            let mut v = vec![b, t];
            v.extend_from_slice(a);
            self.log.push(("fmt", v));
        }
        fn sprintf_plain(&mut self, _m: &mut dyn GuestMemory, b: u32, t: u32) {
            self.log.push(("plain", vec![b, t]));
        }
        fn copy_limited(&mut self, _m: &mut dyn GuestMemory, b: u32, t: u32, l: u32) {
            self.log.push(("copy", vec![b, t, l]));
        }
        fn selected_slot_name(&mut self, _m: &mut dyn GuestMemory) -> u32 {
            0x77
        }
        fn fn_000828c0(&mut self) -> u32 {
            0xc0
        }
        fn fn_000828f0(&mut self) -> u32 {
            0xf0
        }
        fn fn_00082920(&mut self) -> u32 {
            0x20
        }
    }

    #[test]
    fn message_text_cases() {
        let mut m = Mem::default();
        let mut h = MsgHost::default();
        m.write32(SELECTED_SLOT, 5);
        m.write32(SLOT_GLOBAL_7B9798, 9);
        assert_eq!(
            memory_unit_message_text(&mut m, &mut h, 0, 0),
            MESSAGE_BUFFER
        );
        assert_eq!(
            h.log.pop(),
            Some(("err", vec![5, 4, MESSAGE_BUFFER, 9, 0, 0]))
        );
        memory_unit_message_text(&mut m, &mut h, 4, 0);
        assert_eq!(
            h.log.pop(),
            Some(("err", vec![5, 8, MESSAGE_BUFFER, 1, 0, 0]))
        );
        memory_unit_message_text(&mut m, &mut h, 26, 3);
        assert_eq!(
            h.log.pop(),
            Some(("err", vec![5, 7, MESSAGE_BUFFER, 9, 3, 0x1000 + 0x198e]))
        );
        memory_unit_message_text(&mut m, &mut h, 5, 0);
        assert_eq!(
            h.log.pop(),
            Some(("copy", vec![MESSAGE_BUFFER, 0x1000 + 0xe6f, 0x200]))
        );
        memory_unit_message_text(&mut m, &mut h, 99, 0);
        assert_eq!(h.log.pop().map(|e| e.1[1]), Some(0x1000 + 0xe7b));
        memory_unit_message_text(&mut m, &mut h, 40, 8);
        assert_eq!(
            h.log.pop(),
            Some((
                "fmt",
                vec![MESSAGE_BUFFER, 0x1000 + 0xace, LIT_4760E0, 8, 0x77]
            ))
        );
        memory_unit_message_text(&mut m, &mut h, 46, 0);
        assert_eq!(
            h.log.pop(),
            Some(("fmt", vec![MESSAGE_BUFFER, 0x20, LIT_4760E4, 0x77]))
        );
        memory_unit_message_text(&mut m, &mut h, 38, 0);
        assert!(h.log.is_empty());
    }
}
