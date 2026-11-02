//! data/save file level enumerations (T1936 readable rewrites of the c2rust ports of 0x25e40, 0x25ff0
//! and 0x26320). Tier 0 with the Tier 1 differential in `crates/tsfp-c-diff/.../save_t1.rs`.
//! Confidence INFERRED (static disassembly; XDK names of the host methods are guesses from the call
//! shapes). The functions keep the original's stack frame addresses: the host receives pointers into
//! the caller's frame exactly like the original, so `entry_esp` (the guest ESP at function entry, the
//! return address slot) is a parameter.
use crate::memory::GuestMemory;
use crate::save_device::{
    DEVICE_COUNT, DEVICE_DRIVE_LETTER, DEVICE_PRESENT, DEVICE_STATUS, DEVICE_STRIDE, DEVICE_TABLE,
    error_to_status, word_string_copy,
};

/// "%c" style format used to build the drive root from the device's drive letter.
pub const FMT_DRIVE_ROOT: u32 = 0x0047_60e8;
/// "%s\\%s" style format used to build the file path (saves).
pub const FMT_SAVE_PATH: u32 = 0x0047_6030;
/// Path format of the content enumeration.
pub const FMT_CONTENT_PATH: u32 = 0x0047_610c;
/// Format used to copy the content entry name.
pub const FMT_CONTENT_NAME: u32 = 0x0047_6108;
/// Content device (device 10) record: present flag and drive letter.
const CONTENT_PRESENT: u32 = 0x0056_5790;
const CONTENT_LETTER: u32 = 0x0056_5793;
/// Value stored as the entry owner by the content enumeration.
pub const CONTENT_OWNER: u32 = 0xa;

/// File system and formatting services outside data/save (XDK kernel file API, INFERRED names).
pub trait SaveFileSystem {
    /// 0x3c85d4 sprintf into guest memory (`args` are the first varargs).
    fn sprintf(&mut self, m: &mut dyn GuestMemory, dst: u32, format: u32, args: &[u32]);
    /// 0x37e44d FindFirstFile for saves (root, find data) -> handle or u32::MAX.
    fn find_first_save(&mut self, root: u32, find_data: u32) -> u32;
    /// 0x37f512 FindFirstFile for content (root, 0, find data) -> handle or u32::MAX.
    fn find_first_content(&mut self, root: u32, find_data: u32) -> u32;
    /// 0x37e557 FindNextFile for saves -> non-zero when another entry exists.
    fn find_next_save(&mut self, handle: u32, find_data: u32) -> u32;
    /// 0x37f662 FindNextFile for content.
    fn find_next_content(&mut self, handle: u32, find_data: u32) -> u32;
    /// 0x37e59e FindClose.
    fn find_close(&mut self, handle: u32);
    /// 0x37d231 CreateFile for reading (path) -> handle or u32::MAX.
    fn open_for_read(&mut self, path: u32) -> u32;
    /// 0x37d1f6 GetFileSize.
    fn file_size(&mut self, handle: u32) -> u32;
    /// 0x37cbc4 CloseHandle.
    fn close_handle(&mut self, handle: u32);
    /// 0x37e89e signature size for class 0.
    fn signature_size(&mut self) -> u32;
    /// 0x37f0d6 FileTimeToLocalFileTime(source, destination).
    fn file_time_to_local(&mut self, source: u32, destination: u32);
    /// 0x37eb84 FileTimeToSystemTime(source, destination).
    fn file_time_to_system(&mut self, source: u32, destination: u32);
    /// 0x150350 pack a calendar time into the timestamp stored in the entry.
    fn pack_timestamp(
        &mut self,
        year: u32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> u32;
    /// The caller supplied enumeration callback `callback(context, entry)`.
    fn entry_callback(&mut self, callback: u32, context: u32, entry: u32);
}

/// Offsets (from the frame base = ESP after the prologue) of the enumerators' locals.
struct Layout {
    /// Frame size subtracted by the prologue.
    frame_size: u32,
    /// Narrow file name inside the find data.
    name_in_find: u32,
    /// Wide file name inside the find data.
    wide_name: u32,
}

const ROOT: u32 = 0x20;
const HANDLE: u32 = 0x0c;
const ENTRY: u32 = 0x30;
const FIND_DATA: u32 = 0x288;
const PATH: u32 = 0x180;
const DISPLAY_NAME: u32 = 0x134;
const LOCAL_TIME: u32 = 0x28;
const SYSTEM_TIME: u32 = 0x10;
const FILE_TIME_IN_FIND: u32 = 0x29c;
// SYSTEMTIME fields written by the host conversion into this guest stack frame.
const TIME_YEAR: u32 = 0;
const TIME_MONTH: u32 = 2;
const TIME_DAY: u32 = 6;
const TIME_HOUR: u32 = 8;
const TIME_MINUTE: u32 = 0xa;
const TIME_SECOND: u32 = 0xc;
const ENTRY_TIMESTAMP: u32 = 0x174;
const ENTRY_OWNER: u32 = 0x178;
const ENTRY_SIZE: u32 = 0x17c;
const SAVE_LAYOUT: Layout = Layout {
    frame_size: 0x5c4,
    name_in_find: 0x3c8,
    wide_name: 0x4cc,
};
const CONTENT_LAYOUT: Layout = Layout {
    frame_size: 0x5cc,
    name_in_find: 0x3d4,
    wide_name: 0x4d8,
};

/// Frame base of an `ebp` frame function: the aligned ESP after the prologue minus the three
/// callee saved register pushes.
fn frame_base(entry_esp: u32, layout: &Layout) -> u32 {
    let ebp = entry_esp.wrapping_sub(4);
    (ebp & !7).wrapping_sub(layout.frame_size).wrapping_sub(12)
}

fn c_string_len(m: &impl GuestMemory, mut at: u32) -> u32 {
    let start = at;
    while m.read8(at) != 0 {
        at = at.wrapping_add(1);
    }
    at.wrapping_sub(start)
}

/// Converts the find data's last write time to the packed timestamp.
fn entry_timestamp(m: &mut impl GuestMemory, h: &mut impl SaveFileSystem, base: u32) -> u32 {
    h.file_time_to_local(base + FILE_TIME_IN_FIND, base + LOCAL_TIME);
    h.file_time_to_system(base + LOCAL_TIME, base + SYSTEM_TIME);
    let field =
        |m: &mut dyn GuestMemory, offset: u32| u32::from(m.read16(base + SYSTEM_TIME + offset));
    let (year, month, day) = (
        field(m, TIME_YEAR),
        field(m, TIME_MONTH),
        field(m, TIME_DAY),
    );
    let (hour, minute, second) = (
        field(m, TIME_HOUR),
        field(m, TIME_MINUTE),
        field(m, TIME_SECOND),
    );
    h.pack_timestamp(year, month, day, hour, minute, second)
}

/// Shared enumeration loop of the saves and the content variants.
struct Variant {
    content: bool,
    layout: Layout,
}

fn enumerate(
    m: &mut impl GuestMemory,
    h: &mut impl SaveFileSystem,
    entry_esp: u32,
    variant: &Variant,
    device: u32,
    filter: u32,
    callback: u32,
    context: u32,
) {
    let base = frame_base(entry_esp, &variant.layout);
    let record = DEVICE_TABLE.wrapping_add(device.wrapping_mul(DEVICE_STRIDE));
    let letter = if variant.content {
        if device != CONTENT_OWNER || m.read8(CONTENT_PRESENT) == 0 || m.read8(CONTENT_LETTER) == 0
        {
            return;
        }
        m.read8(record + DEVICE_DRIVE_LETTER)
    } else {
        if device == CONTENT_OWNER
            || device >= DEVICE_COUNT
            || m.read8(record + DEVICE_PRESENT) == 0
            || m.read8(record + DEVICE_DRIVE_LETTER) == 0
        {
            return;
        }
        m.read8(record + DEVICE_DRIVE_LETTER)
    };
    h.sprintf(
        m,
        base + ROOT,
        FMT_DRIVE_ROOT,
        &[i32::from(letter as i8) as u32],
    );
    let handle = if variant.content {
        h.find_first_content(base + ROOT, base + FIND_DATA)
    } else {
        h.find_first_save(base + ROOT, base + FIND_DATA)
    };
    m.write32(base + HANDLE, handle);
    if handle == u32::MAX {
        return;
    }
    let name = base + variant.layout.name_in_find;
    loop {
        let mut size = 0;
        let mut usable = true;
        if filter != 0 {
            let format = if variant.content {
                FMT_CONTENT_PATH
            } else {
                FMT_SAVE_PATH
            };
            h.sprintf(m, base + PATH, format, &[name, filter]);
            let file = h.open_for_read(base + PATH);
            if file == u32::MAX {
                // saves keep the entry with size 0, content skips it
                usable = !variant.content;
            } else {
                size = h.file_size(file);
                h.close_handle(file);
            }
        }
        if usable {
            let signature = h.signature_size();
            word_string_copy(
                m,
                0x40,
                base + variant.layout.wide_name,
                base + DISPLAY_NAME,
            );
            if variant.content {
                h.sprintf(m, base + ENTRY, FMT_CONTENT_NAME, &[name]);
            } else {
                let length = c_string_len(m, name);
                for i in 0..=length {
                    m.write8(base + ENTRY + i, m.read8(name + i));
                }
            }
            let stamp = entry_timestamp(m, h, base);
            m.write32(base + ENTRY_TIMESTAMP, stamp);
            m.write32(
                base + ENTRY_OWNER,
                if variant.content {
                    CONTENT_OWNER
                } else {
                    device
                },
            );
            m.write32(base + ENTRY_SIZE, size.wrapping_sub(signature));
            h.entry_callback(callback, context, base + ENTRY);
        }
        let more = if variant.content {
            h.find_next_content(m.read32(base + HANDLE), base + FIND_DATA)
        } else {
            h.find_next_save(m.read32(base + HANDLE), base + FIND_DATA)
        };
        if more == 0 {
            h.find_close(m.read32(base + HANDLE));
            return;
        }
    }
}

/// 0x25e40 `game_save_game_find_enumerate`: call `callback(context, entry)` for every save file of
/// the (non content) device. The entry is 0x150 bytes: narrow name at +0, display name at +0x104,
/// packed timestamp at +0x144, owner (the device) at +0x148 and data size at +0x14c.
pub fn save_game_find_enumerate(
    m: &mut impl GuestMemory,
    h: &mut impl SaveFileSystem,
    entry_esp: u32,
    device: u32,
    filter: u32,
    callback: u32,
    context: u32,
) {
    let variant = Variant {
        content: false,
        layout: SAVE_LAYOUT,
    };
    enumerate(m, h, entry_esp, &variant, device, filter, callback, context);
}

/// 0x25ff0 `game_content_find_enumerate`: the same for the content device (device 10); entries whose
/// file cannot be opened are skipped and the owner field is `CONTENT_OWNER`.
pub fn content_find_enumerate(
    m: &mut impl GuestMemory,
    h: &mut impl SaveFileSystem,
    entry_esp: u32,
    device: u32,
    filter: u32,
    callback: u32,
    context: u32,
) {
    let variant = Variant {
        content: true,
        layout: CONTENT_LAYOUT,
    };
    enumerate(m, h, entry_esp, &variant, device, filter, callback, context);
}

/// Callbacks and services of `save_slots_enumerate_for_verify`.
pub trait SlotEnumerationHost {
    /// 0x24ec0 enumerate the devices from `start`, calling the guest callback `callback` (0x262d0)
    /// with the context record at `context`; the callback stores the matching device index at the
    /// frame base (`found`) and its display name at frame base + 0xc.
    fn enumerate_slots(&mut self, m: &mut dyn GuestMemory, start: u32, callback: u32, context: u32);
    /// 0x3c8960 bounded string copy (destination, source, limit).
    fn bounded_copy(&mut self, m: &mut dyn GuestMemory, destination: u32, source: u32, limit: u32);
}

/// The guest address of the verify callback passed to the slot enumeration.
pub const VERIFY_CALLBACK: u32 = 0x0002_62d0;
/// Callback context occupies the caller's local frame; the callback receives the name field.
const VERIFY_FRAME_SIZE: u32 = 0x120;
const VERIFY_FOUND_DEVICE: u32 = 0;
const VERIFY_NAME: u32 = 4;
const VERIFY_LABEL: u32 = 8;
const VERIFY_DISPLAY_NAME: u32 = 0xc;
const VERIFY_FRAME_POINTER: u32 = 0x110;
const VERIFY_MATCH_A: u32 = 0x114;
const VERIFY_MATCH_B: u32 = 0x118;
const VERIFY_MATCH_C: u32 = 0x11c;
const VERIFY_DISPLAY_NAME_CAPACITY: u32 = 0x104;

/// 0x26320 `game_save_slots_enumerate_for_verify_with_bounded_copy`: search the devices for the one
/// that holds the save described by (label, name, three more match fields) and return its index
/// (u32::MAX when none). When `out_name` is non-zero and the device is mounted, its display name is
/// copied there (at most 0x104 bytes). The 0x120 byte frame doubles as the callback context:
/// +4 name, +8 label, +0x110 frame pointer, +0x114 match field 1, +0x118 match field 2, +0x11c
/// match field 3 and the result at +0, all INFERRED from the stores (the callback is not ported).
pub fn save_slots_enumerate_for_verify(
    m: &mut impl GuestMemory,
    h: &mut impl SlotEnumerationHost,
    entry_esp: u32,
    label: u32,
    name: u32,
    match_a: u32,
    match_b: u32,
    match_c: u32,
    start: u32,
    out_name: u32,
) -> u32 {
    let frame = entry_esp.wrapping_sub(VERIFY_FRAME_SIZE);
    m.write32(frame + VERIFY_NAME, name);
    m.write32(frame + VERIFY_LABEL, label);
    m.write32(frame + VERIFY_MATCH_A, match_a);
    m.write32(frame + VERIFY_MATCH_B, match_b);
    m.write32(frame + VERIFY_FRAME_POINTER, frame);
    m.write32(frame + VERIFY_FOUND_DEVICE, u32::MAX);
    m.write32(frame + VERIFY_MATCH_C, match_c);
    h.enumerate_slots(m, start, VERIFY_CALLBACK, frame + VERIFY_NAME);
    let found = m.read32(frame + VERIFY_FOUND_DEVICE);
    if out_name != 0 && found < DEVICE_COUNT {
        let record = DEVICE_TABLE.wrapping_add(found.wrapping_mul(DEVICE_STRIDE));
        if m.read8(record + DEVICE_PRESENT) != 0 && m.read8(record + DEVICE_DRIVE_LETTER) != 0 {
            h.bounded_copy(
                m,
                out_name,
                frame + VERIFY_DISPLAY_NAME,
                VERIFY_DISPLAY_NAME_CAPACITY,
            );
        }
    }
    m.read32(frame + VERIFY_FOUND_DEVICE)
}

/// UI and device services used by the free space dialog (0x32b540).
pub trait FreeSpaceDialogHost {
    /// 0x3d7b0 text pointer by id.
    fn text_by_id(&mut self, id: u32) -> u32;
    /// 0x827f0 selected device slot.
    fn selected_slot(&mut self) -> u32;
    /// 0x827c0 global 0x7b9798.
    fn slot_global(&mut self) -> u32;
    /// 0x82820 name record address of the selected slot.
    fn selected_slot_name(&mut self) -> u32;
    /// 0x24950 save error message (device, code, buffer, kind, subject, text).
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
    /// 0x3d7f0 sprintf of the template into `buffer` with two arguments.
    fn sprintf_template(
        &mut self,
        m: &mut dyn GuestMemory,
        buffer: u32,
        template: u32,
        arg_a: u32,
        arg_b: u32,
    );
    /// 0x25c30 free space query of a device: writes block counts through the non null pointers.
    fn free_space_query(
        &mut self,
        m: &mut dyn GuestMemory,
        device: u32,
        out_free: u32,
        out_total: u32,
    );
    /// 0x25be0 blocks needed for a save of `size` bytes.
    fn blocks_for_size(&mut self, size: u32) -> u32;
    /// 0x82560 show the message dialog (context, -1, text buffer, completion handler).
    fn show_message(&mut self, context: u32, text: u32, handler: u32);
    /// 0x825c0 show the confirm dialog (context, -1, text buffer, yes handler, no handler).
    fn show_confirm(&mut self, context: u32, text: u32, yes_handler: u32, no_handler: u32);
}

/// Text buffer shared by the pad level dialogs.
pub const DIALOG_TEXT: u32 = 0x0076_5800;
const DIALOG_DONE: u32 = 0x0032_b2b0;
const DIALOG_CONFIRM_YES: u32 = 0x0032_a3c0;
const LIT_4760E4_ARG: u32 = 0x0047_60e4;
const LIT_4783D8: u32 = 0x0047_83d8;
const LIT_47602C: u32 = 0x0047_602c;

fn append_bytes(m: &mut impl GuestMemory, buffer: u32, from: u32, count: u32) {
    let mut end = buffer;
    while m.read8(end) != 0 {
        end = end.wrapping_add(1);
    }
    for i in 0..count {
        m.write8(end.wrapping_add(i), m.read8(from.wrapping_add(i)));
    }
}

/// 0x32b540 `game_save_pad_level_dialog_show_free_space_status_by_kind`: kind 2 reports the free
/// space of the selected device (not enough room: confirm dialog with the missing blocks, device
/// below slot 8: the device name message), any other kind reports the save error text. The out
/// slot of the free space query is the caller's second argument slot (`entry_esp + 8`), which the
/// original reuses as scratch.
pub fn pad_level_dialog_show_free_space_status(
    m: &mut impl GuestMemory,
    h: &mut impl FreeSpaceDialogHost,
    entry_esp: u32,
    context: u32,
    kind: u32,
) {
    if kind != 2 {
        let text = h.text_by_id(0x198e);
        let subject = h.text_by_id(0x198e);
        let global = h.slot_global();
        let device = h.selected_slot();
        h.error_message(m, device, kind, DIALOG_TEXT, global, subject, text);
        h.show_message(context, DIALOG_TEXT, DIALOG_DONE);
        return;
    }
    let slot = h.selected_slot();
    if (slot as i32) < 8 {
        let name = h.selected_slot_name();
        let template = h.text_by_id(0x1976);
        h.sprintf_template(m, DIALOG_TEXT, template, LIT_4760E4_ARG, name);
        h.show_message(context, DIALOG_TEXT, DIALOG_DONE);
        return;
    }
    let scratch = entry_esp.wrapping_add(8);
    let device = h.selected_slot();
    h.free_space_query(m, device, 0, scratch);
    let needed = h.blocks_for_size(0x8000);
    let missing = needed.wrapping_sub(m.read32(scratch));
    let template = h.text_by_id(0x198b);
    h.sprintf_template(m, DIALOG_TEXT, template, LIT_4783D8, missing);
    append_bytes(m, DIALOG_TEXT, LIT_47602C, 2);
    let tail = h.text_by_id(0x1979);
    let mut length = 0;
    while m.read8(tail.wrapping_add(length)) != 0 {
        length += 1;
    }
    append_bytes(m, DIALOG_TEXT, tail, length + 1);
    h.show_confirm(context, DIALOG_TEXT, DIALOG_CONFIRM_YES, DIALOG_DONE);
}

/// Signature services outside data/save (XDK signature API, INFERRED names).
pub trait SignatureHost {
    /// 0x37e89e signature size for a size class.
    fn signature_size_lookup(&mut self, class: u32) -> u32;
    /// 0x24f40 begin a signature: (state record, flags) -> 1 on success or a mapped error status.
    fn signature_begin(&mut self, m: &mut dyn GuestMemory, state: u32, flags: u32) -> u32;
    /// 0x37e962 finalize a signature handle with its buffer.
    fn signature_finalize(&mut self, handle: u32, buffer: u32);
    /// 0x37e914 feed `length` bytes of `data` to a signature handle.
    fn signature_update(&mut self, handle: u32, data: u32, length: u32);
    /// 0x250c0 compare the two signature states (computed, stored) -> 1 when equal.
    fn signature_compare(&mut self, m: &mut dyn GuestMemory, computed: u32, stored: u32) -> u32;
    /// 0x25180 load the stored signature buffer from a state record.
    fn signature_buffer_load(&mut self, m: &mut dyn GuestMemory, state: u32);
    /// 0x3e1e0 release an arena block.
    fn release_block(&mut self, block: u32);
}

/// Signature state record fields (guest memory): handle, size, size class, buffer.
const STATE_HANDLE: u32 = 0;
const STATE_SIZE: u32 = 4;
const STATE_BUFFER: u32 = 0xc;

/// 0x25710 `game_save_signature_finish`: verify the signature appended to `buffer` (total size
/// `total_size`). Returns 1 on success, 7 when the buffer is too small, else the failing status; the
/// payload length (total minus signature size) is stored through `out_length`. Two signature state
/// records live in the 0x20 byte frame (computed at -0x20, stored at -0x10 from the entry ESP) and
/// the caller's second argument slot (`entry_esp + 8`) is reused for the payload length.
pub fn save_signature_finish(
    m: &mut impl GuestMemory,
    h: &mut impl SignatureHost,
    entry_esp: u32,
    buffer: u32,
    total_size: u32,
    out_length: u32,
    flags: u32,
) -> u32 {
    let computed = entry_esp.wrapping_sub(0x20);
    let stored = entry_esp.wrapping_sub(0x10);
    m.write32(computed + 0x14, 0);
    m.write32(computed, u32::MAX);
    m.write32(stored + 0xc, 0);
    m.write32(stored + 4, u32::MAX);
    m.write32(stored, u32::MAX);
    m.write32(stored + 8, 0);
    m.write32(computed + 8, 8);
    let signature = h.signature_size_lookup(8);
    m.write32(computed + STATE_SIZE, signature);
    if total_size <= signature {
        return 7;
    }
    let payload = total_size - signature;
    let slot = entry_esp.wrapping_add(8);
    m.write32(slot, payload);
    let compare_flag = flags & 4;
    let mut status = h.signature_begin(m, computed, compare_flag | 0x8000_0000);
    if status == 1 {
        let length = m.read32(computed + STATE_SIZE);
        let destination = m.read32(computed + STATE_BUFFER);
        for i in 0..length {
            m.write8(
                destination.wrapping_add(i),
                m.read8(buffer.wrapping_add(payload).wrapping_add(i)),
            );
        }
    }
    let payload = m.read32(slot);
    let computed_handle = m.read32(computed + STATE_HANDLE);
    if computed_handle != u32::MAX {
        h.signature_finalize(computed_handle, m.read32(computed + STATE_BUFFER));
    }
    if status == 1 {
        status = h.signature_begin(m, stored, compare_flag | 0x4000_0000);
    }
    let stored_handle = m.read32(stored + STATE_HANDLE);
    if stored_handle != u32::MAX {
        if buffer != 0 && payload != 0 {
            h.signature_update(stored_handle, buffer, payload);
        }
        h.signature_finalize(stored_handle, m.read32(stored + STATE_BUFFER));
    }
    if status == 1 {
        status = h.signature_compare(m, stored, computed);
        if status == 1 && flags & 0x10 != 0 {
            h.signature_buffer_load(m, computed);
        }
    }
    let stored_buffer = m.read32(stored + STATE_BUFFER);
    if stored_buffer != 0 {
        h.release_block(stored_buffer);
    }
    let computed_buffer = m.read32(computed + STATE_BUFFER);
    if computed_buffer != 0 {
        h.release_block(computed_buffer);
    }
    if out_length != 0 {
        m.write32(out_length, payload);
    }
    status
}

/// File write services of `save_game_write` (XDK names INFERRED from the call shapes).
pub trait SaveWriteFiles {
    /// 0x25c30 free space query of a device; writes the free blocks through the non null pointer and
    /// returns the success byte.
    fn free_space_query(
        &mut self,
        m: &mut dyn GuestMemory,
        device: u32,
        out_total: u32,
        out_free: u32,
    ) -> u32;
    /// 0x37e9a7 last error code of the failed call (GetLastError, no arguments).
    fn last_error(&mut self) -> u32;
    /// 0x25be0 blocks needed for a save of `size` bytes.
    fn blocks_for_size(&mut self, size: u32) -> u32;
    /// 0x37e11a create the save directory: (root, wide name, 4, 0, out, 0x104) -> 0 on success or an error.
    fn create_directory(&mut self, root: u32, wide_name: u32, out: u32) -> u32;
    /// 0x37d231 create the file for writing (path) -> handle or u32::MAX.
    fn create_file_for_write(&mut self, path: u32) -> u32;
    /// 0x37ccf5 write a buffer to a file: (file, data, length, out written) -> non zero on success.
    fn write_file(&mut self, file: u32, data: u32, length: u32, written: u32) -> u32;
    /// 0x25070 append the signature of `state` to an open file -> status.
    fn append_signature(&mut self, m: &mut dyn GuestMemory, state: u32, file: u32) -> u32;
    /// 0x37cdcb flush a file.
    fn flush_file(&mut self, file: u32);
    /// 0x37e37d delete the path (root, wide name).
    fn delete_path(&mut self, root: u32, wide_name: u32);
}

/// Fields of a signature state record (handle, size, class, buffer) at a frame address.
fn state_init(m: &mut impl GuestMemory, state: u32, class: u32) {
    m.write32(state, u32::MAX);
    m.write32(state + 4, u32::MAX);
    m.write32(state + 8, class);
    m.write32(state + 0xc, 0);
}

/// Widens a narrow C string into a UTF-16 buffer (terminator included).
fn widen(m: &mut impl GuestMemory, source: u32, destination: u32) {
    let mut at = 0;
    loop {
        let byte = m.read8(source.wrapping_add(at));
        m.write8(destination + 2 * at, byte);
        m.write8(destination + 2 * at + 1, 0);
        if byte == 0 {
            break;
        }
        at += 1;
    }
}

/// 0x263e0 `game_save_game_write`: write `data` as the save named `name` under the device's drive,
/// with the two signatures the console requires, and store the status at +4 of the device record.
/// Returns that status (1 ok, 2 not enough room, 6 device missing, else a mapped error).
/// Frame addresses follow the original (`entry_esp` is the guest ESP at entry). The sprintf of the
/// path passes the caller's ESI as an unused fourth argument (not modelled).
pub fn save_game_write<H: SaveWriteFiles + SignatureHost + SaveFileSystem>(
    m: &mut impl GuestMemory,
    h: &mut H,
    entry_esp: u32,
    device: u32,
    label: u32,
    name: u32,
    data: u32,
    length: u32,
    flags: u32,
) -> u32 {
    let base = entry_esp.wrapping_sub(0x448);
    let free_slot = base + 8; // also reused as the file handle
    let root = base + 0x1c;
    let wide_name = base + 0x240;
    let path = base + 0x13c;
    let directory_out = base + 0x38;
    let computed = base + 0xc;
    let stored = base + 0x24;
    let record = DEVICE_TABLE.wrapping_add(device.wrapping_mul(DEVICE_STRIDE));
    let set_status =
        |m: &mut dyn GuestMemory, status: u32| m.write32(record + DEVICE_STATUS, status);
    m.write32(free_slot, 0);
    set_status(m, 1);
    if device >= DEVICE_COUNT
        || m.read8(record + DEVICE_PRESENT) == 0
        || m.read8(record + DEVICE_DRIVE_LETTER) == 0
    {
        set_status(m, 6);
        return 6;
    }
    if flags & 1 != 0 {
        if h.free_space_query(m, device, 0, free_slot) & 0xff == 0 {
            let error = h.last_error();
            let status = error_to_status(error, 2);
            set_status(m, status);
            return status;
        }
        let needed = h.blocks_for_size(length);
        if m.read32(free_slot) < needed {
            set_status(m, 2);
            return 2;
        }
    }
    let letter = i32::from(m.read8(record + DEVICE_DRIVE_LETTER) as i8) as u32;
    h.sprintf(m, root, FMT_DRIVE_ROOT, &[letter]);
    widen(m, name, wide_name);
    let directory = h.create_directory(root, wide_name, directory_out);
    if directory != 0 {
        let status = error_to_status(directory, 3);
        set_status(m, status);
        return status;
    }
    h.sprintf(m, path, FMT_SAVE_PATH, &[directory_out, label, 0]);
    let file = h.create_file_for_write(path);
    m.write32(free_slot, file);
    if file == u32::MAX {
        let error = h.last_error();
        let status = error_to_status(error, 3);
        set_status(m, status);
    } else {
        state_init(m, computed, 0);
        state_init(m, stored, 0);
        if m.read32(record + DEVICE_STATUS) == 1 {
            let status = h.signature_begin(m, stored, flags & 0xa);
            set_status(m, status);
            if status == 1 {
                let status = h.signature_begin(m, computed, flags & 0x24);
                set_status(m, status);
            }
        }
        let computed_handle = m.read32(computed);
        let stored_handle = m.read32(stored);
        if data != 0 && length != 0 {
            if computed_handle != u32::MAX {
                h.signature_update(computed_handle, data, length);
            }
            if stored_handle != u32::MAX {
                h.signature_update(stored_handle, data, length);
            }
        }
        if computed_handle != u32::MAX {
            h.signature_finalize(computed_handle, m.read32(computed + 0xc));
        }
        let (computed_buffer, computed_size) = (m.read32(computed + 0xc), m.read32(computed + 4));
        if computed_buffer != 0 && computed_size != 0 && stored_handle != u32::MAX {
            h.signature_update(stored_handle, computed_buffer, computed_size);
        }
        if stored_handle != u32::MAX {
            h.signature_finalize(stored_handle, m.read32(stored + 0xc));
        }
        if m.read32(record + DEVICE_STATUS) == 1 {
            let written = base + 0x34;
            let ok = h.write_file(m.read32(free_slot), data, length, written);
            if ok == 0 || m.read32(written) != length {
                let error = h.last_error();
                let status = error_to_status(error, 3);
                set_status(m, status);
            }
            if m.read32(record + DEVICE_STATUS) == 1 {
                let file = m.read32(free_slot);
                let status = h.append_signature(m, computed, file);
                set_status(m, status);
                if status == 1 {
                    let status = h.append_signature(m, stored, file);
                    set_status(m, status);
                }
            }
        }
        if m.read32(record + DEVICE_STATUS) == 1 && flags & 0x10 != 0 {
            h.signature_buffer_load(m, computed);
        }
        let buffer = m.read32(computed + 0xc);
        if buffer != 0 {
            h.release_block(buffer);
        }
        let buffer = m.read32(stored + 0xc);
        if buffer != 0 {
            h.release_block(buffer);
        }
        h.flush_file(m.read32(free_slot));
        h.close_handle(m.read32(free_slot));
    }
    if m.read32(record + DEVICE_STATUS) != 1 {
        h.delete_path(root, wide_name);
    }
    m.read32(record + DEVICE_STATUS)
}

/// File read services of `save_file_load_verify` (XDK names INFERRED from the call shapes).
pub trait SaveReadFiles {
    /// 0x37cc08 read `length` bytes into `buffer`; the count read is stored at `read_out`; non zero on success.
    fn read_file(&mut self, file: u32, buffer: u32, length: u32, read_out: u32) -> u32;
    /// 0x37ce4f seek to an absolute offset.
    fn seek(&mut self, file: u32, offset: u32);
    /// 0x3dfa0 allocate an arena block of `size` bytes (kind 0).
    fn allocate_block(&mut self, size: u32) -> u32;
    /// 0x25020 read the stored signature of `state` from the file -> status.
    fn read_signature(&mut self, m: &mut dyn GuestMemory, state: u32, file: u32) -> u32;
}

/// 0x25280 `game_save_file_load_verify`: read the save `name`/`label` of a device into `buffer` (`length`
/// bytes), checking the size and the two signatures stored after the data. Status codes as
/// `save_game_write`, 7 when the file is too large. Frame addresses follow the original; five
/// signature state records and the read count live in the 0x154 byte frame.
pub fn save_file_load_verify<H: SaveWriteFiles + SignatureHost + SaveFileSystem + SaveReadFiles>(
    m: &mut impl GuestMemory,
    h: &mut H,
    entry_esp: u32,
    device: u32,
    name: u32,
    label: u32,
    buffer: u32,
    length: u32,
    flags: u32,
) -> u32 {
    let base = entry_esp.wrapping_sub(0x160);
    let path = base + 0x5c;
    let handle_slot = base + 0x34;
    let read_count = base + 0x1c;
    let block_slot = base + 0x20;
    let state_x = base + 0xc;
    let state_y = base + 0x24;
    let state_z = base + 0x38;
    let state_w = base + 0x48;
    let flag_slot = base + 0x58;
    let record = DEVICE_TABLE.wrapping_add(device.wrapping_mul(DEVICE_STRIDE));
    let set_status =
        |m: &mut dyn GuestMemory, status: u32| m.write32(record + DEVICE_STATUS, status);
    set_status(m, 1);
    h.sprintf(m, path, FMT_SAVE_PATH, &[name, label]);
    let file = h.open_for_read(path);
    m.write32(handle_slot, file);
    if file == u32::MAX {
        let error = h.last_error();
        let status = error_to_status(error, 8);
        set_status(m, status);
        return status;
    }
    for (state, class) in [(state_x, 0), (state_y, 0), (state_z, 0), (state_w, 0)] {
        state_init(m, state, class);
    }
    m.write32(block_slot, 0);
    let signature_zero = h.signature_size_lookup(0);
    m.write32(state_y + 4, signature_zero);
    m.write32(state_x + 8, 8);
    let signature_eight = h.signature_size_lookup(8);
    m.write32(state_x + 4, signature_eight);
    let file_size = h.file_size(file);
    let expected = signature_eight
        .wrapping_add(m.read32(state_y + 4))
        .wrapping_add(length);
    if file_size > expected {
        h.close_handle(file);
        set_status(m, 7);
        return 7;
    }
    if !(h.read_file(file, buffer, length, read_count) != 0 && m.read32(read_count) == length) {
        let error = h.last_error();
        let status = error_to_status(error, 8);
        set_status(m, status);
    }
    if m.read32(record + DEVICE_STATUS) == 1 {
        let first = flags & 0xa;
        m.write32(flag_slot, first);
        let status = h.signature_begin(m, state_y, first | 0x8000_0000);
        set_status(m, status);
        if status == 1 {
            let status = h.signature_begin(m, state_x, (flags & 4) | 0x8000_0000);
            set_status(m, status);
        }
        if m.read32(record + DEVICE_STATUS) == 1 {
            let status = h.read_signature(m, state_x, file);
            set_status(m, status);
        }
        let stored_size = m.read32(state_y + 4);
        let computed_size = m.read32(state_x + 4);
        if file_size.wrapping_sub(length) == computed_size.wrapping_add(stored_size) {
            h.seek(file, length);
            let block = h.allocate_block(computed_size);
            m.write32(block_slot, block);
            if !(h.read_file(file, block, computed_size, read_count) != 0
                && m.read32(read_count) == computed_size)
            {
                let error = h.last_error();
                let status = error_to_status(error, 8);
                set_status(m, status);
            }
        }
        h.seek(file, file_size.wrapping_sub(stored_size));
        if m.read32(record + DEVICE_STATUS) == 1 {
            let status = h.read_signature(m, state_y, file);
            set_status(m, status);
        }
        let handle_x = m.read32(state_x);
        if handle_x != u32::MAX {
            h.signature_finalize(handle_x, m.read32(state_x + 0xc));
        }
        let handle_y = m.read32(state_y);
        if handle_y != u32::MAX {
            h.signature_finalize(handle_y, m.read32(state_y + 0xc));
        }
        if m.read32(record + DEVICE_STATUS) == 1 {
            let first_flags = m.read32(flag_slot);
            let status = h.signature_begin(m, state_w, first_flags);
            set_status(m, status);
            if status == 1 {
                let status = h.signature_begin(m, state_z, (flags & 4) | 0x4000_0000);
                set_status(m, status);
            }
        }
        let handle_w = m.read32(state_w);
        let handle_z = m.read32(state_z);
        let block = m.read32(block_slot);
        if buffer != 0 && length != 0 && handle_w != u32::MAX {
            h.signature_update(handle_w, buffer, length);
        }
        if block != 0 && m.read32(state_x + 4) != 0 && handle_w != u32::MAX {
            h.signature_update(handle_w, block, m.read32(state_x + 4));
        }
        if buffer != 0 && length != 0 && handle_z != u32::MAX {
            h.signature_update(handle_z, buffer, length);
        }
        if handle_w != u32::MAX {
            h.signature_finalize(handle_w, m.read32(state_w + 0xc));
        }
        let buffer_z = m.read32(state_z + 0xc);
        if handle_z != u32::MAX {
            h.signature_finalize(handle_z, buffer_z);
        }
        if m.read32(record + DEVICE_STATUS) == 1 {
            let status = h.signature_compare(m, state_w, state_y);
            set_status(m, status);
            if status == 1 {
                let status = h.signature_compare(m, state_z, state_x);
                set_status(m, status);
                if status == 1 && flags & 0x10 != 0 {
                    h.signature_buffer_load(m, state_x);
                }
            }
        }
        for block in [
            buffer_z,
            m.read32(state_w + 0xc),
            m.read32(block_slot),
            m.read32(state_x + 0xc),
            m.read32(state_y + 0xc),
        ] {
            if block != 0 {
                h.release_block(block);
            }
        }
    }
    h.close_handle(file);
    m.read32(record + DEVICE_STATUS)
}

/// Content creation services of `content_write` (XDK content API, INFERRED names).
pub trait ContentFiles {
    /// 0x37f99f create the content entry: (0, content id, title id, out handle) -> non zero on success.
    fn create_content(&mut self, content_id: u32, title_id: u32, out: u32) -> u32;
    /// 0x29650 normalise the content path in place.
    fn normalize_path(&mut self, path: u32);
    /// 0x37f9f1 create the content directory: (0, content id, title id, 0, wide name, out) -> non zero on success.
    fn create_content_directory(
        &mut self,
        content_id: u32,
        title_id: u32,
        wide_name: u32,
        out: u32,
    ) -> u32;
    /// 0x251d0 release the buffers of a signature state.
    fn release_signature(&mut self, state: u32);
    /// 0x37fba9 delete the content created through `out`.
    fn delete_content(&mut self, out: u32);
}

/// Title id the content is created for.
const CONTENT_TITLE_ID: u32 = 0x4541_0066;
/// Number of content id retries after a failed create (the id is incremented each time).
const CONTENT_RETRIES: u32 = 10;

/// 0x26740 `game_content_write`: write the level (`buffer`, `length` bytes whose tail carries a
/// signature, verified first) as downloadable content named `name`, retrying with the next content id
/// (`content_id` + 1 ...) up to ten times when the content cannot be created. Statuses as
/// `save_game_write`. Frame addresses follow the original; the retry counter, the written count and
/// the file handle reuse locals of the frame, and the content id is incremented in the caller's
/// argument slot (`entry_esp + 0x10`).
pub fn content_write<H: SaveWriteFiles + SignatureHost + SaveFileSystem + ContentFiles>(
    m: &mut impl GuestMemory,
    h: &mut H,
    entry_esp: u32,
    device: u32,
    label: u32,
    name: u32,
    content_id: u32,
    buffer: u32,
    length: u32,
    flags: u32,
) -> u32 {
    let base = entry_esp.wrapping_sub(0x444);
    let free_slot = base + 4;
    let counter = base + 8;
    let payload_slot = base + 0xc;
    let state_one = base + 0x10;
    let record_slot = base + 0x20;
    let state_two = base + 0x24;
    let content_out = base + 0x34;
    let path = base + 0x138;
    let wide_name = base + 0x23c;
    let content_id_slot = entry_esp + 0x10;
    let record = DEVICE_TABLE.wrapping_add(device.wrapping_mul(DEVICE_STRIDE));
    let set_status =
        |m: &mut dyn GuestMemory, status: u32| m.write32(record + DEVICE_STATUS, status);
    m.write32(record_slot, record);
    m.write32(free_slot, 0);
    m.write32(counter, 0);
    set_status(m, 1);
    if device >= DEVICE_COUNT
        || m.read8(record + DEVICE_PRESENT) == 0
        || m.read8(record + DEVICE_DRIVE_LETTER) == 0
    {
        set_status(m, 6);
        return 6;
    }
    if flags & 1 != 0 {
        if h.free_space_query(m, device, 0, free_slot) & 0xff == 0 {
            let error = h.last_error();
            let status = error_to_status(error, 2);
            set_status(m, status);
            return status;
        }
        let needed = h.blocks_for_size(length);
        if m.read32(free_slot) < needed {
            set_status(m, 2);
            return 2;
        }
    }
    let status = save_signature_finish(
        m,
        h,
        entry_esp.wrapping_sub(0x45c),
        buffer,
        length,
        payload_slot,
        flags,
    );
    set_status(m, status);
    if status != 1 {
        return status;
    }
    loop {
        widen(m, name, wide_name);
        let id = m.read32(content_id_slot);
        let created = h.create_content(id, CONTENT_TITLE_ID, content_out) != 0;
        let mut failed_create = true;
        if created {
            h.sprintf(m, path, FMT_CONTENT_PATH, &[content_out, label]);
            h.normalize_path(path);
            let id = m.read32(content_id_slot);
            if h.create_content_directory(id, CONTENT_TITLE_ID, wide_name, content_out) != 0 {
                failed_create = false;
                let file = h.create_file_for_write(path);
                m.write32(free_slot, file);
                if file != u32::MAX {
                    write_content_file(m, h, base, file, buffer, length, flags);
                    break;
                }
                let attempt = m.read32(counter);
                if attempt < CONTENT_RETRIES {
                    m.write32(content_id_slot, m.read32(content_id_slot).wrapping_add(1));
                    m.write32(counter, attempt + 1);
                    continue;
                }
                let error = h.last_error();
                set_status(m, error_to_status(error, 3));
                break;
            }
        }
        if failed_create {
            let error = h.last_error();
            let status = error_to_status(error, 3);
            set_status(m, status);
            return status;
        }
    }
    if m.read32(record + DEVICE_STATUS) != 1 {
        h.delete_content(content_out);
    }
    m.read32(record + DEVICE_STATUS)
}

fn write_content_file<H: SaveWriteFiles + SignatureHost + SaveFileSystem + ContentFiles>(
    m: &mut impl GuestMemory,
    h: &mut H,
    base: u32,
    file: u32,
    buffer: u32,
    length: u32,
    flags: u32,
) {
    let record = m.read32(base + 0x20);
    let set_status =
        |m: &mut dyn GuestMemory, status: u32| m.write32(record + DEVICE_STATUS, status);
    let (state_one, state_two) = (base + 0x10, base + 0x24);
    let payload_slot = base + 0xc;
    state_init(m, state_one, 0);
    state_init(m, state_two, 0);
    if m.read32(record + DEVICE_STATUS) == 1 {
        let status = h.signature_begin(m, state_two, flags & 0xa);
        set_status(m, status);
        if status == 1 {
            let status = h.signature_begin(m, state_one, (flags & 4) | 0x8000_0000);
            set_status(m, status);
        }
    }
    let payload = m.read32(payload_slot);
    let one_size = m.read32(state_one + 4);
    let one_buffer = m.read32(state_one + 0xc);
    // When the first begin did not run the size is still u32::MAX and the original's `rep movsd`
    // would copy 4 GiB (a latent bug when the signature begin fails); that case is skipped here.
    if one_size != u32::MAX {
        for i in 0..one_size {
            m.write8(
                one_buffer.wrapping_add(i),
                m.read8(buffer.wrapping_add(payload).wrapping_add(i)),
            );
        }
    }
    let one_handle = m.read32(state_one);
    if one_handle != u32::MAX {
        h.signature_finalize(one_handle, one_buffer);
    }
    let two_handle = m.read32(state_two);
    if two_handle != u32::MAX {
        if buffer != 0 && length != 0 {
            h.signature_update(two_handle, buffer, length);
        }
        h.signature_finalize(two_handle, m.read32(state_two + 0xc));
    }
    if m.read32(record + DEVICE_STATUS) == 1 {
        let ok = h.write_file(file, buffer, payload, payload_slot);
        if ok == 0 || m.read32(payload_slot) != payload {
            let error = h.last_error();
            set_status(m, error_to_status(error, 3));
        }
        if m.read32(record + DEVICE_STATUS) == 1 {
            let status = h.append_signature(m, state_one, file);
            set_status(m, status);
            if status == 1 {
                let status = h.append_signature(m, state_two, file);
                set_status(m, status);
            }
        }
        if m.read32(record + DEVICE_STATUS) == 1 && flags & 0x10 != 0 {
            h.signature_buffer_load(m, state_one);
        }
    } else {
        // the original reloads the file handle slot for the close below
    }
    h.release_signature(state_one);
    h.release_signature(state_two);
    h.flush_file(file);
    h.close_handle(file);
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
    struct Fs {
        log: Vec<String>,
        files: usize,
        open_ok: bool,
        packed_time: Option<[u32; 6]>,
    }
    impl SaveFileSystem for Fs {
        fn sprintf(&mut self, _m: &mut dyn GuestMemory, _d: u32, f: u32, a: &[u32]) {
            self.log.push(format!("sprintf {f:x} {}", a.len()));
        }
        fn find_first_save(&mut self, _r: u32, _d: u32) -> u32 {
            if self.files == 0 { u32::MAX } else { 7 }
        }
        fn find_first_content(&mut self, _r: u32, _d: u32) -> u32 {
            self.find_first_save(0, 0)
        }
        fn find_next_save(&mut self, _h: u32, _d: u32) -> u32 {
            self.files -= 1;
            self.files as u32
        }
        fn find_next_content(&mut self, h: u32, d: u32) -> u32 {
            self.find_next_save(h, d)
        }
        fn find_close(&mut self, _h: u32) {
            self.log.push("close".into());
        }
        fn open_for_read(&mut self, _p: u32) -> u32 {
            if self.open_ok { 9 } else { u32::MAX }
        }
        fn file_size(&mut self, _h: u32) -> u32 {
            1000
        }
        fn close_handle(&mut self, _h: u32) {}
        fn signature_size(&mut self) -> u32 {
            100
        }
        fn file_time_to_local(&mut self, _s: u32, _d: u32) {}
        fn file_time_to_system(&mut self, _s: u32, _d: u32) {}
        fn pack_timestamp(&mut self, y: u32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> u32 {
            self.packed_time = Some([y, mo, d, h, mi, s]);
            y
        }
        fn entry_callback(&mut self, _c: u32, ctx: u32, e: u32) {
            self.log.push(format!("cb {ctx} {e:x}"));
        }
    }

    fn device(m: &mut Mem, index: u32, letter: u8) {
        m.write8(DEVICE_TABLE + index * DEVICE_STRIDE, 1);
        m.write8(DEVICE_TABLE + index * DEVICE_STRIDE + 3, letter);
    }

    #[test]
    fn timestamp_reads_calendar_fields_in_host_order() {
        let mut m = Mem::default();
        let mut fs = Fs::default();
        let base = 0x300000;
        for (field, value) in [
            (TIME_YEAR, 2026),
            (TIME_MONTH, 10),
            (TIME_DAY, 12),
            (TIME_HOUR, 15),
            (TIME_MINUTE, 43),
            (TIME_SECOND, 21),
        ] {
            m.write8(base + SYSTEM_TIME + field, value as u8);
            m.write8(base + SYSTEM_TIME + field + 1, (value >> 8) as u8);
        }
        assert_eq!(entry_timestamp(&mut m, &mut fs, base), 2026);
        assert_eq!(fs.packed_time, Some([2026, 10, 12, 15, 43, 21]));
    }

    #[test]
    fn save_enumeration() {
        let mut m = Mem::default();
        let mut fs = Fs {
            files: 2,
            open_ok: true,
            ..Default::default()
        };
        device(&mut m, 2, b'F');
        let base = frame_base(0x300000, &SAVE_LAYOUT);
        for (i, b) in b"abc\\0".iter().enumerate() {
            m.write8(base + SAVE_LAYOUT.name_in_find + i as u32, *b);
        }
        save_game_find_enumerate(&mut m, &mut fs, 0x300000, 2, 0x500, 0x600, 42);
        assert_eq!(fs.log.iter().filter(|l| l.starts_with("cb 42")).count(), 2);
        assert_eq!(m.read8(base + ENTRY), b'a');
        assert_eq!(m.read32(base + ENTRY_OWNER), 2);
        assert_eq!(m.read32(base + ENTRY_SIZE), 900);
        // content device is rejected by the save variant, unmounted device too
        fs.log.clear();
        fs.files = 1;
        save_game_find_enumerate(&mut m, &mut fs, 0x300000, 0xa, 0, 0, 0);
        save_game_find_enumerate(&mut m, &mut fs, 0x300000, 3, 0, 0, 0);
        assert!(fs.log.is_empty());
    }

    #[test]
    fn content_enumeration_skips_unopenable() {
        let mut m = Mem::default();
        let mut fs = Fs {
            files: 1,
            open_ok: false,
            ..Default::default()
        };
        m.write8(CONTENT_PRESENT, 1);
        m.write8(CONTENT_LETTER, b'T');
        device(&mut m, 10, b'T');
        content_find_enumerate(&mut m, &mut fs, 0x300000, 10, 0x500, 0x600, 1);
        assert!(!fs.log.iter().any(|l| l.starts_with("cb")));
        fs.files = 1;
        fs.open_ok = true;
        content_find_enumerate(&mut m, &mut fs, 0x300000, 10, 0x500, 0x600, 1);
        let base = frame_base(0x300000, &CONTENT_LAYOUT);
        assert_eq!(m.read32(base + ENTRY_OWNER), CONTENT_OWNER);
        assert!(fs.log.iter().any(|l| l.starts_with("cb 1")));
    }

    struct Slots {
        found: u32,
        log: Vec<(u32, u32)>,
    }
    impl SlotEnumerationHost for Slots {
        fn enumerate_slots(
            &mut self,
            m: &mut dyn GuestMemory,
            start: u32,
            callback: u32,
            context: u32,
        ) {
            self.log.push((start, callback));
            m.write32(context - 4, self.found);
        }
        fn bounded_copy(&mut self, _m: &mut dyn GuestMemory, d: u32, s: u32, l: u32) {
            self.log.push((d + s, l));
        }
    }

    #[test]
    fn slot_verify_enumeration() {
        let mut m = Mem::default();
        let mut h = Slots {
            found: u32::MAX,
            log: vec![],
        };
        let frame = 0x300000u32 - 0x120;
        assert_eq!(
            save_slots_enumerate_for_verify(&mut m, &mut h, 0x300000, 1, 2, 3, 4, 5, 6, 0x900),
            u32::MAX
        );
        assert_eq!(h.log, vec![(6, VERIFY_CALLBACK)]);
        assert_eq!(
            (
                m.read32(frame + 4),
                m.read32(frame + 8),
                m.read32(frame + 0x11c)
            ),
            (2, 1, 5)
        );
        // found device, mounted: the name is copied; unmounted or no output: no copy
        h.found = 2;
        h.log.clear();
        device(&mut m, 2, b'F');
        assert_eq!(
            save_slots_enumerate_for_verify(&mut m, &mut h, 0x300000, 1, 2, 3, 4, 5, 6, 0x900),
            2
        );
        assert_eq!(h.log.last(), Some(&(0x900 + frame + 0xc, 0x104)));
        h.log.clear();
        save_slots_enumerate_for_verify(&mut m, &mut h, 0x300000, 1, 2, 3, 4, 5, 6, 0);
        h.found = 3;
        save_slots_enumerate_for_verify(&mut m, &mut h, 0x300000, 1, 2, 3, 4, 5, 6, 0x900);
        assert_eq!(h.log.len(), 2);
    }

    #[derive(Default)]
    struct Dialog {
        log: Vec<String>,
        slot: u32,
    }
    impl FreeSpaceDialogHost for Dialog {
        fn text_by_id(&mut self, id: u32) -> u32 {
            0x8000 + id
        }
        fn selected_slot(&mut self) -> u32 {
            self.slot
        }
        fn slot_global(&mut self) -> u32 {
            9
        }
        fn selected_slot_name(&mut self) -> u32 {
            0x77
        }
        fn error_message(
            &mut self,
            _m: &mut dyn GuestMemory,
            _d: u32,
            c: u32,
            _b: u32,
            _k: u32,
            _s: u32,
            _t: u32,
        ) {
            self.log.push(format!("err {c}"));
        }
        fn sprintf_template(&mut self, _m: &mut dyn GuestMemory, _b: u32, t: u32, a: u32, b: u32) {
            self.log.push(format!("fmt {t:x} {a:x} {b}"));
        }
        fn free_space_query(&mut self, m: &mut dyn GuestMemory, _d: u32, _f: u32, t: u32) {
            m.write32(t, 30);
        }
        fn blocks_for_size(&mut self, _s: u32) -> u32 {
            50
        }
        fn show_message(&mut self, c: u32, _t: u32, _h: u32) {
            self.log.push(format!("msg {c}"));
        }
        fn show_confirm(&mut self, c: u32, _t: u32, _y: u32, _n: u32) {
            self.log.push(format!("confirm {c}"));
        }
    }

    #[test]
    fn free_space_dialog() {
        let mut m = Mem::default();
        let mut h = Dialog {
            slot: 9,
            ..Default::default()
        };
        pad_level_dialog_show_free_space_status(&mut m, &mut h, 0x300000, 5, 3);
        assert_eq!(h.log, vec!["err 3", "msg 5"]);
        h.log.clear();
        pad_level_dialog_show_free_space_status(&mut m, &mut h, 0x300000, 5, 2);
        assert_eq!(
            h.log[0],
            format!("fmt {:x} {:x} 20", 0x8000 + 0x198b, LIT_4783D8)
        );
        assert_eq!(h.log[1], "confirm 5");
        h.log.clear();
        h.slot = 3;
        pad_level_dialog_show_free_space_status(&mut m, &mut h, 0x300000, 5, 2);
        assert_eq!(
            h.log,
            vec![
                format!("fmt {:x} {:x} 119", 0x8000 + 0x1976, LIT_4760E4_ARG),
                "msg 5".into()
            ]
        );
    }

    #[derive(Default)]
    struct Sig {
        log: Vec<String>,
        begin: Vec<u32>,
        compare: u32,
    }
    impl SignatureHost for Sig {
        fn signature_size_lookup(&mut self, _c: u32) -> u32 {
            20
        }
        fn signature_begin(&mut self, m: &mut dyn GuestMemory, state: u32, flags: u32) -> u32 {
            self.log.push(format!("begin {flags:x}"));
            m.write32(state + 4, 4);
            m.write32(state + 0xc, 0x7000);
            m.write32(state, 3);
            self.begin.remove(0)
        }
        fn signature_finalize(&mut self, h: u32, _b: u32) {
            self.log.push(format!("fin {h}"));
        }
        fn signature_update(&mut self, h: u32, _d: u32, l: u32) {
            self.log.push(format!("upd {h} {l}"));
        }
        fn signature_compare(&mut self, _m: &mut dyn GuestMemory, _a: u32, _b: u32) -> u32 {
            self.compare
        }
        fn signature_buffer_load(&mut self, _m: &mut dyn GuestMemory, _s: u32) {
            self.log.push("load".into());
        }
        fn release_block(&mut self, b: u32) {
            self.log.push(format!("free {b:x}"));
        }
    }

    #[test]
    fn signature_finish_flow() {
        let mut m = Mem::default();
        let mut h = Sig {
            begin: vec![1, 1],
            compare: 1,
            ..Default::default()
        };
        assert_eq!(
            save_signature_finish(&mut m, &mut h, 0x300000, 0x2000, 10, 0x900, 0),
            7
        );
        m.write8(0x2000 + 100 + 1, 0x5a);
        assert_eq!(
            save_signature_finish(&mut m, &mut h, 0x300000, 0x2000, 120, 0x900, 0x14),
            1
        );
        assert_eq!(m.read32(0x900), 100);
        assert_eq!(m.read8(0x7000 + 1), 0x5a);
        assert!(
            h.log.contains(&"begin 80000004".to_string())
                && h.log.contains(&"begin 40000004".to_string())
        );
        assert!(h.log.contains(&"load".to_string()) && h.log.contains(&"upd 3 100".to_string()));
        assert_eq!(h.log.iter().filter(|l| l.starts_with("free")).count(), 2);
        // a failing begin skips the second begin and the compare
        h.log.clear();
        h.begin = vec![9];
        assert_eq!(
            save_signature_finish(&mut m, &mut h, 0x300000, 0x2000, 120, 0, 0),
            9
        );
        assert_eq!(h.log.iter().filter(|l| l.starts_with("begin")).count(), 1);
    }
}
