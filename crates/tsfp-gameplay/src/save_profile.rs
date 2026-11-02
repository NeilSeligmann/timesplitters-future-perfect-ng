//! data/save profile and local-slot tables (T1899, batch 1 of save_profile: the small table
//! accessors at 0x69630..0x69930). Tier 0 for every function here: translated, compiles, smoke
//! tested with synthetic memory. Confidence INFERRED (static disassembly only, nothing executed).
//! Not yet ported (verification debt): 0x69a60 stats aggregate and the 0x190d90.. save flows.
use crate::memory::GuestMemory;

/// Pointer to the active profile block (`game_global_6f1e0c_get`, 0x69630 reads it).
pub const PROFILE_PTR: u32 = 0x006f_1e0c;
/// Local slot table: 16 entries of `SLOT_STRIDE` bytes, dword 0 = pointer to the allocation record.
pub const SLOT_TABLE: u32 = 0x007b_edc0;
pub const SLOT_COUNT: i32 = 16;
pub const SLOT_STRIDE: u32 = 0x1f54;
/// Default (slot-less) slot record base used when no net session is active.
pub const DEFAULT_SLOT_RECORD: u32 = 0x007b_ee40;
/// Allocation records: 144 records of 0x40 bytes (`0x7bc9c0..0x7bedc0`), dword 0 = in use.
pub const ALLOC_TABLE: u32 = 0x007b_c9c0;
pub const ALLOC_STRIDE: u32 = 0x40;
/// Allocation record fields shared by the slot accessors, allocator and save flow.
/// The meaning of the two payload words is call-site dependent; their positions are
/// known, but a more specific type is not established from the available evidence.
const RECORD_PAYLOAD_24: u32 = 0x24;
const RECORD_PAYLOAD_28: u32 = 0x28;
/// Serial of the storage device holding this record's saved profile.
const RECORD_DEVICE_SERIAL: u32 = 0x2c;
const RECORD_PENDING: u32 = 0x34;
const RECORD_BACK_POINTER: u32 = 0x3c;
/// Per slot stats blocks (stride is `SLOT_STRIDE`, base here) and the aggregate fallback.
pub const STATS_BASE: u32 = 0x007c_0050;
pub const STATS_AGGREGATE: u32 = 0x006f_11c0;
/// Mode flag word; bit 0x800000 forces the aggregate block.
pub const MODE_FLAGS: u32 = 0x007d_e458;
pub const MODE_FLAG_USE_AGGREGATE: u32 = 0x0080_0000;
/// Callees outside this slice (network/session, profile time, string copy).
pub trait ProfileHost {
    /// 0x3533f0 `game_net_is_session_active`.
    fn net_is_session_active(&mut self) -> bool;
    /// 0x356800 local slot index for a player index.
    fn local_slot_index(&mut self, player: u32) -> u32;
    /// 0x150230 `game_local_time_encode_31day_timestamp` (low 26 bits are used).
    fn local_time_encode(&mut self) -> u32;
    /// 0x3c8960 bounded string copy of `len` bytes from `src` to `dst`.
    fn string_copy(&mut self, mem: &mut dyn GuestMemory, dst: u32, src: u32, len: u32);
}

/// 0x69630 `game_global_6f1e0c_get`.
pub fn profile_ptr(m: &impl GuestMemory) -> u32 {
    m.read32(PROFILE_PTR)
}

/// 0x69640 profile record base for local slot zero, or the default record without a session.
pub fn record_base_for_local_slot_zero(h: &mut impl ProfileHost) -> u32 {
    if !h.net_is_session_active() {
        return DEFAULT_SLOT_RECORD;
    }
    let slot = h.local_slot_index(0);
    slot.wrapping_mul(SLOT_STRIDE)
        .wrapping_add(DEFAULT_SLOT_RECORD)
}

/// 0x69670 clear the slot table (0x7d50 dwords), the allocation table (0x900 dwords) and the
/// profile pointer.
pub fn slots_clear_all(m: &mut impl GuestMemory) {
    for i in 0..0x7d50u32 {
        m.write32(SLOT_TABLE + i * 4, 0);
    }
    for i in 0..0x900u32 {
        m.write32(ALLOC_TABLE + i * 4, 0);
    }
    m.write32(PROFILE_PTR, 0);
}

/// 0x696a0 pack the local time: seconds-of-clock modulo 60 in the top 6 bits, the 26 low bits
/// of the encoded timestamp below. `seconds` is the signed dword at 0x7497b0.
/// A negative input retains the original signed remainder before the six-bit field is packed.
///
/// ```
/// use tsfp_gameplay::save_profile::{ProfileHost, pack_local_time};
/// use tsfp_gameplay::memory::GuestMemory;
/// struct Clock;
/// impl ProfileHost for Clock {
///     fn net_is_session_active(&mut self) -> bool { false }
///     fn local_slot_index(&mut self, _: u32) -> u32 { 0 }
///     fn local_time_encode(&mut self) -> u32 { 0xffff_ffff }
///     fn string_copy(&mut self, _: &mut dyn GuestMemory, _: u32, _: u32, _: u32) {}
/// }
/// assert_eq!(pack_local_time(61, &mut Clock), (1 << 26) | 0x03ff_ffff);
/// ```
pub fn pack_local_time(seconds: i32, h: &mut impl ProfileHost) -> u32 {
    let sec = (seconds % 60) as u32;
    (h.local_time_encode() & 0x03ff_ffff) | (sec << 26)
}

fn slot_entry(index: i32) -> Option<u32> {
    (0..SLOT_COUNT)
        .contains(&index)
        .then(|| SLOT_TABLE + (index as u32).wrapping_mul(SLOT_STRIDE))
}

/// 0x696d0 slot is active (allocation pointer non-zero). Unlike the bounded accessors it does
/// not range check the index.
pub fn slot_is_active(m: &impl GuestMemory, index: u32) -> bool {
    m.read32(SLOT_TABLE.wrapping_add(index.wrapping_mul(SLOT_STRIDE))) != 0
}

/// 0x69720 allocation record pointer plus 4 for an in-range active slot, else 0.
pub fn slot_entry_ptr_plus_4(m: &impl GuestMemory, index: i32) -> u32 {
    match slot_entry(index).map(|e| m.read32(e)) {
        Some(p) if p != 0 => p.wrapping_add(4),
        _ => 0,
    }
}

/// Record pointer holder used by 0x69750, 0x69790 and 0x697d0: the slot entry when the index is in
/// range and the slot is active, else guest address 0 (the original dereferences a null pointer,
/// which reads the dword stored at guest address 0, normally zero).
fn slot_record_holder(m: &impl GuestMemory, index: i32) -> u32 {
    match slot_entry(index) {
        Some(entry) if m.read32(entry) != 0 => entry,
        _ => 0,
    }
}

/// 0x69750 field +0x24 of the slot record (a missing slot reads through guest address 0).
pub fn slot_field_0x24(m: &impl GuestMemory, index: i32) -> u32 {
    let record = m.read32(slot_record_holder(m, index));
    m.read32(record.wrapping_add(RECORD_PAYLOAD_24))
}

/// 0x69790 field +0x28 of the slot record, same null-slot behaviour as `slot_field_0x24`.
pub fn slot_field_0x28(m: &impl GuestMemory, index: i32) -> u32 {
    let record = m.read32(slot_record_holder(m, index));
    m.read32(record.wrapping_add(RECORD_PAYLOAD_28))
}

/// 0x697d0 set fields +0x24 and +0x28 and clear +0x34 of the slot record (null-slot behaviour as
/// above: the stores land relative to the dword at guest address 0).
pub fn slot_set_fields(m: &mut impl GuestMemory, index: i32, f24: u32, f28: u32) {
    let record = m.read32(slot_record_holder(m, index));
    m.write32(record.wrapping_add(RECORD_PENDING), 0);
    let record = m.read32(slot_record_holder(m, index));
    m.write32(record.wrapping_add(RECORD_PAYLOAD_24), f24);
    let record = m.read32(slot_record_holder(m, index));
    m.write32(record.wrapping_add(RECORD_PAYLOAD_28), f28);
}

/// 0x69820 allocate an allocation record, copy the name (at most 0x1e bytes of its C string
/// length) to +4 and fill the header. Returns the record address or 0 when the table is full.
pub fn alloc_record_with_name(
    m: &mut impl GuestMemory,
    h: &mut impl ProfileHost,
    name: u32,
    field_24: u32,
    field_28: u32,
) -> u32 {
    let mut len = 0u32;
    while m.read8(name.wrapping_add(len)) != 0 {
        len += 1;
    }
    let len = len.min(0x1e);
    let end = SLOT_TABLE;
    let mut rec = ALLOC_TABLE;
    while rec < end {
        if m.read32(rec) == 0 {
            for off in (4..0x20).step_by(4) {
                m.write32(rec + off, 0);
            }
            m.write8(rec + 0x20, 0); // word at +0x20
            m.write8(rec + 0x21, 0);
            m.write8(rec + 0x22, 0); // byte at +0x22
            h.string_copy(m, rec + 4, name, len);
            m.write32(rec + RECORD_PAYLOAD_28, field_28);
            m.write32(rec, 1);
            m.write32(rec + RECORD_PAYLOAD_24, field_24);
            m.write32(rec + 0x30, 1);
            m.write32(rec + RECORD_PENDING, 0);
            m.write32(rec + 0x38, 0);
            m.write32(rec + RECORD_BACK_POINTER, 0);
            return rec;
        }
        rec += ALLOC_STRIDE;
    }
    0
}

/// 0x698d0 zero a 0x40 byte record when its first dword is set, first nulling the dword that
/// field +0x3c points to.
pub fn record_zero_and_null_pointer_field(m: &mut impl GuestMemory, rec: u32) {
    if rec == 0 || m.read32(rec) == 0 {
        return;
    }
    let ptr = m.read32(rec + RECORD_BACK_POINTER);
    if ptr != 0 {
        m.write32(ptr, 0);
    }
    for i in 0..0x10 {
        m.write32(rec + i * 4, 0);
    }
}

/// 0x69900 stats record for a slot, or the aggregate block when the mode flag is set or the slot
/// is empty.
pub fn stats_record_ptr(m: &impl GuestMemory, slot: u32) -> u32 {
    if m.read32(MODE_FLAGS) & MODE_FLAG_USE_AGGREGATE != 0 {
        return STATS_AGGREGATE;
    }
    let off = slot.wrapping_mul(SLOT_STRIDE);
    if m.read32(SLOT_TABLE.wrapping_add(off)) == 0 {
        STATS_AGGREGATE
    } else {
        off.wrapping_add(STATS_BASE)
    }
}

/// 0x69930 stats entry pointer: record base plus 0x490 plus 0x18 per entry. The original reads
/// the slot argument at a different stack offset in the aggregate-flag branch (it uses the
/// entry index only).
pub fn stats_entry_ptr(m: &impl GuestMemory, slot: u32, entry: u32) -> u32 {
    let base = if m.read32(MODE_FLAGS) & MODE_FLAG_USE_AGGREGATE != 0 {
        STATS_AGGREGATE
    } else {
        stats_record_ptr(m, slot)
    };
    base.wrapping_add(entry.wrapping_mul(0x18))
        .wrapping_add(0x490)
}

/// Profile list (0x79ee00, 0x150 byte records, count at 0x74f9fc, at most 0x1e). The record name is
/// at +0x104 and the owner at +0x148.
pub const PROFILE_LIST: u32 = 0x0079_ee00;
pub const PROFILE_LIST_COUNT: u32 = 0x0074_f9fc;
pub const PROFILE_LIST_MAX: u32 = 0x1e;
pub const PROFILE_RECORD_STRIDE: u32 = 0x150;
pub const PROFILE_NAME_OFFSET: u32 = 0x104;
pub const PROFILE_OWNER_OFFSET: u32 = 0x148;
/// Player menu state array (dword per player) and its device selection array.
pub const PLAYER_STATE: u32 = 0x0074_f9ec;
pub const PLAYER_DEVICE: u32 = 0x0074_e924;
pub const PENDING_PLAYER_BITS: u32 = 0x0074_f9e4;
pub const SAVE_STATE_TEXT_OBJECT: u32 = 0x007b_a16c;
pub const SAVE_STATE_TEXT_ENTRY: u32 = 0x007b_a178;

/// Callees outside this slice used by the profile list and save menu flows.
pub trait ProfileFlowHost {
    /// 0x1906b0 detect the map prefix of a profile name (non-zero rejects the entry).
    fn fn_001906b0_map_prefix_detect(&mut self, name: u32, flag: u32) -> u32;
    /// 0x25e40 enumerate saves with a callback (`callback` is a guest address).
    fn fn_00025e40_find_enumerate(&mut self, kind: u32, text: u32, callback: u32, context: u32);
    /// 0x69a60 aggregate stats for a slot.
    fn fn_00069a60_stats_aggregate(&mut self, slot: u32);
    /// 0x79b70 enter a state descriptor.
    fn fn_00079b70_enter_state(&mut self, state_word: u32, descriptor: u32);
    /// 0x746a0 sound request with zeroed floats (player record argument).
    fn fn_000746a0_sound_request(&mut self, player: u32);
    /// 0x45390 + 0x45820 show the text id as a popup.
    fn text_popup(&mut self, text_id: u32);
    /// 0x192580 save game write for slot (ebx = player).
    fn fn_00192580_save_game_write_for_slot(&mut self, player: u32);
    /// 0x37f057 local time as SYSTEMTIME words (year, month, weekday, day, hour, minute, second).
    fn local_time_fields(&mut self) -> [u16; 7];
    /// 0x14ff30 pack the 8 BCD time bytes into the timestamp.
    fn fn_0014ff30_pack_time(&mut self, bytes: [u8; 8]) -> u32;
    /// 0x24e60 memory unit slot is active.
    fn memory_unit_slot_is_active(&mut self, device: u32) -> bool;
}

fn name_equals(m: &impl GuestMemory, a: u32, b: u32) -> bool {
    let (mut a, mut b) = (a, b);
    loop {
        let (ca, cb) = (m.read8(a), m.read8(b));
        if ca != cb {
            return false;
        }
        if ca == 0 {
            return true;
        }
        a += 1;
        b += 1;
    }
}

/// 0x190d90 the name is not used by any list entry of this owner (1 free, 0 taken).
pub fn profile_list_name_is_free_for_owner(m: &impl GuestMemory, name: u32, owner: u32) -> u32 {
    u32::from(profile_list_find_entry(m, name, owner) == 0)
}

/// 0x190e00 find the list entry with this name and owner, or 0.
pub fn profile_list_find_entry(m: &impl GuestMemory, name: u32, owner: u32) -> u32 {
    let count = m.read32(PROFILE_LIST_COUNT) as i32;
    for index in 0..count.max(0) as u32 {
        let record = PROFILE_LIST + index.wrapping_mul(PROFILE_RECORD_STRIDE);
        if m.read32(record + PROFILE_OWNER_OFFSET) == owner
            && name_equals(m, record + PROFILE_NAME_OFFSET, name)
        {
            return record;
        }
    }
    0
}

/// 0x190f20 append a 0x150 byte entry unless the list is full or the name is rejected. Returns the
/// rejection code (EAX is residue when the list is full).
pub fn save_enumerate_append_entry(
    m: &mut impl GuestMemory,
    h: &mut impl ProfileFlowHost,
    entry: u32,
) -> Option<u32> {
    if m.read32(PROFILE_LIST_COUNT) as i32 >= PROFILE_LIST_MAX as i32 {
        return None;
    }
    let rejected = h.fn_001906b0_map_prefix_detect(entry + PROFILE_NAME_OFFSET, 0);
    if rejected != 0 {
        return Some(rejected);
    }
    let count = m.read32(PROFILE_LIST_COUNT);
    let dst = PROFILE_LIST.wrapping_add(count.wrapping_mul(PROFILE_RECORD_STRIDE));
    m.write32(PROFILE_LIST_COUNT, count.wrapping_add(1));
    for i in 0..0x54 {
        m.write32(dst + i * 4, m.read32(entry + i * 4));
    }
    None
}

/// 0x190f70 enumeration callback: entries whose first field is not 0xa trigger a find-enumerate.
pub fn save_enumerate_entry_callback(
    m: &impl GuestMemory,
    h: &mut impl ProfileFlowHost,
    context: u32,
    entry: u32,
) {
    let kind = m.read32(entry);
    if kind != 0xa {
        h.fn_00025e40_find_enumerate(kind, 0x0047_dfc8, 0x0019_0f20, context);
    }
}

/// 0x190fd0 clear bit `bit` of the pending player word; when it empties, aggregate the stats and
/// enter the state descriptor, else flag the menu and set the player state to 0xe.
pub fn clear_pending_bit_then_enter_state(
    m: &mut impl GuestMemory,
    h: &mut impl ProfileFlowHost,
    bit: u32,
) {
    let remaining = m.read32(PENDING_PLAYER_BITS) & !(1u32 << (bit & 31));
    m.write32(PENDING_PLAYER_BITS, remaining);
    if remaining == 0 {
        h.fn_00069a60_stats_aggregate(m.read32(0x0074_e934));
        h.fn_00079b70_enter_state(m.read32(0x0074_f9e0), 0x0052_4598);
        m.write32(PENDING_PLAYER_BITS, 0);
    } else {
        m.write32(0x0074_e938, 1);
        m.write32(PLAYER_STATE.wrapping_add(bit.wrapping_mul(4)), 0xe);
    }
}

fn player_device_slot(m: &impl GuestMemory, player: u32) -> u32 {
    m.read32(0x0079_ec60u32.wrapping_add(m.read32(player + 0x18c).wrapping_mul(36)))
}

fn device_feedback(
    m: &mut impl GuestMemory,
    h: &mut impl ProfileFlowHost,
    player: u32,
    index: u32,
    state: u32,
) {
    let device = player_device_slot(m, player);
    if h.memory_unit_slot_is_active(device) {
        let device = player_device_slot(m, player);
        m.write32(PLAYER_DEVICE.wrapping_add(index.wrapping_mul(4)), device);
        h.fn_000746a0_sound_request(player);
        let index = if state == 7 {
            m.read32(player + 0x38)
        } else {
            m.read32(0x0078_ad34)
        };
        m.write32(PLAYER_STATE.wrapping_add(index.wrapping_mul(4)), state);
    } else if m.read32(SAVE_STATE_TEXT_OBJECT) != 0 {
        let entry = m.read32(SAVE_STATE_TEXT_ENTRY);
        let text_id = m.read32(entry + 0xc);
        h.text_popup(text_id);
    }
}

/// 0x192e60 memory unit slot inactive: sound request or popup (player index from +0x38).
pub fn memory_unit_slot_inactive_sound_request(
    m: &mut impl GuestMemory,
    h: &mut impl ProfileFlowHost,
    player: u32,
) {
    let index = m.read32(player + 0x38);
    device_feedback(m, h, player, index, 7);
}

/// 0x193310 same with the player index from the global at 0x78ad34 and state 4.
pub fn memory_unit_slot_inactive_sound_request_global_index(
    m: &mut impl GuestMemory,
    h: &mut impl ProfileFlowHost,
    player: u32,
) {
    let index = m.read32(0x0078_ad34);
    device_feedback(m, h, player, index, 4);
}

/// 0x192ef0 thunk: save game write for the slot in ebx.
pub fn thunk_save_game_write_for_slot(h: &mut impl ProfileFlowHost, player: u32) {
    h.fn_00192580_save_game_write_for_slot(player);
}

fn to_bcd(value: u16) -> u8 {
    let rem = (value % 100) as u8;
    rem + 6 * (rem / 10)
}

/// 0x150230 encode the local time: BCD bytes year%100, month, day, hour, minute, second (bytes
/// 0 and 4 are stack residue in the original, zero here) packed by 0x14ff30.
pub fn local_time_encode_31day_timestamp(h: &mut impl ProfileFlowHost) -> u32 {
    let [year, month, _weekday, day, hour, minute, second] = h.local_time_fields();
    let bytes = [
        0,
        to_bcd(second),
        to_bcd(minute),
        to_bcd(hour),
        0,
        to_bcd(day),
        to_bcd(month),
        to_bcd(year),
    ];
    h.fn_0014ff30_pack_time(bytes)
}

/// Callees outside this slice used by the save menu flows (0x2cbd20.. 0x32a4a0 0x191030).
pub trait SaveMenuHost {
    /// 0x69620 profile entry address for the menu player.
    fn fn_00069620_profile_entry(&mut self, player: u32) -> u32;
    /// 0x79b70 enter a state descriptor.
    fn fn_00079b70_enter_state(&mut self, state_word: u32, descriptor: u32);
    /// 0x42720 number of active records.
    fn fn_00042720_active_record_count(&mut self) -> u32;
    /// 0x71940 widget child by index.
    fn fn_00071940_child(&mut self, widget: u32, index: u32) -> u32;
    /// 0x71c70 set a child enabled (1) or disabled (0).
    fn fn_00071c70_set_enabled(&mut self, child: u32, enabled: u32);
    /// 0x71a60 the focused child.
    fn fn_00071a60_focused_child(&mut self, widget: u32) -> u32;
    /// 0x77110 move focus.
    fn fn_00077110_move_focus(&mut self, widget: u32, first: u32);
    /// 0x71de0 widget has a focusable child.
    fn fn_00071de0_has_focusable(&mut self, widget: u32) -> u32;
    /// 0x2d2490 count of pending items.
    fn fn_002d2490_pending_count(&mut self) -> i32;
    /// 0x3377d0 record for a slot index (text id at +8).
    fn fn_003377d0_slot_record(&mut self, index: u32) -> u32;
    /// 0x3d7b0 text pointer by id.
    fn text_by_id(&mut self, id: u32) -> u32;
    /// 0x3c8960 bounded string copy (dst, src, limit).
    fn fn_003c8960_string_copy(&mut self, m: &mut dyn GuestMemory, dst: u32, src: u32, limit: u32);
    /// 0x746a0 sound request with zeroed floats.
    fn fn_000746a0_sound_request(&mut self, player: u32);
    /// 0x24950 save error message text.
    fn fn_00024950_error_message(
        &mut self,
        m: &mut dyn GuestMemory,
        device: u32,
        code: u32,
        buffer: u32,
        kind: u32,
        subject: u32,
        text: u32,
    );
}

/// 0x2cbd20 copy the selected slot record (0x1f dwords from 0x7ba364 + 0x1e0 per entry) into the
/// menu player's profile entry, flag the menu and enter the state.
pub fn save_player_block_to_profile_entry(m: &mut impl GuestMemory, h: &mut impl SaveMenuHost) {
    let entry = h.fn_00069620_profile_entry(m.read32(0x0078_ad34));
    let source = m
        .read32(0x007b_a9e0)
        .wrapping_mul(0x1e0)
        .wrapping_add(0x007b_a364);
    for i in 0..0x1f {
        m.write32(
            entry.wrapping_add(i * 4),
            m.read32(source.wrapping_add(i * 4)),
        );
    }
    let flags = m.read32(0x0078_ad24) | 0x1_0000;
    m.write32(0x0078_ad24, flags);
    h.fn_00079b70_enter_state(m.read32(0x0076_13e4), 0x0052_3910);
}

/// 0x2d6d50 profile menu open: enable three items by the active record count and push the child.
pub fn profile_menu_open_enable_items(
    m: &mut impl GuestMemory,
    h: &mut impl SaveMenuHost,
    widget: u32,
) {
    let count = h.fn_00042720_active_record_count();
    for index in 0..3u32 {
        let child = h.fn_00071940_child(widget, index + 1);
        let enabled = u32::from(index >= count || m.read32(0x0078_ae00) == 0);
        h.fn_00071c70_set_enabled(child, enabled);
        if index < count {
            let flags = m.read32(child + 0x28) & !4;
            m.write32(child + 0x28, flags);
        } else if index != 0 {
            let flags = m.read32(child + 0x28) | 4;
            m.write32(child + 0x28, flags);
            if h.fn_00071a60_focused_child(widget) == child {
                h.fn_00077110_move_focus(widget, 1);
            }
        } else {
            let flags = m.read32(child + 0x28) & !4;
            m.write32(child + 0x28, flags);
        }
    }
    if h.fn_00071de0_has_focusable(widget) == 0 && h.fn_002d2490_pending_count() > 0 {
        m.write32(0x0078_ad2c, 1);
        h.fn_00079b70_enter_state(m.read32(0x0076_13e4), 0x0052_36dc);
    }
}

/// 0x32a4a0 fill the eight slot names (list record 0x778d38, entry stride 0x6c) from the slot
/// records' text ids.
pub fn save_slot_buffer_fill_eight_slot_names(m: &mut impl GuestMemory, h: &mut impl SaveMenuHost) {
    let list = m.read32(0x0077_8d38);
    m.write32(list + 8, 8);
    for index in 0..8u32 {
        let record = h.fn_003377d0_slot_record(index);
        let text = h.text_by_id(m.read32(record + 8));
        let list = m.read32(0x0077_8d38);
        h.fn_003c8960_string_copy(m, list + index * 0x6c + 0xc, text, 0x1f);
        m.write8(list + index * 0x6c + 0x2b, 0);
    }
}

/// 0x32a940 append a 0x6c byte entry (27 dwords) to the slot buffer; returns its index or -1 when
/// the buffer already holds 0x32 entries.
pub fn save_slot_buffer_append_entry(m: &mut impl GuestMemory, entry: u32) -> u32 {
    let list = m.read32(0x0077_8d38);
    let count = m.read32(list + 8);
    if count as i32 >= 0x32 {
        return u32::MAX;
    }
    m.write32(list + 8, count + 1);
    let dst = list + count.wrapping_mul(0x6c) + 0xc;
    for i in 0..0x1b {
        m.write32(dst + i * 4, m.read32(entry + i * 4));
    }
    count
}

/// 0x191030 profile create: with room in the list, go to state 3; when full show the "list full"
/// message and go to state 8.
pub fn save_profile_create_check_list_limit(
    m: &mut impl GuestMemory,
    h: &mut impl SaveMenuHost,
    player: u32,
) {
    h.fn_000746a0_sound_request(player);
    let slot = m.read32(player + 0x38);
    if (m.read32(PROFILE_LIST_COUNT) as i32) < PROFILE_LIST_MAX as i32 {
        m.write32(PLAYER_STATE.wrapping_add(slot.wrapping_mul(4)), 3);
        return;
    }
    m.write32(0x004e_8d50, u32::MAX);
    let text = h.text_by_id(0x198d);
    let device = m.read32(PLAYER_DEVICE.wrapping_add(slot.wrapping_mul(4)));
    h.fn_00024950_error_message(m, device, 5, 0x0074_e960, 2, text, text);
    let mut end = 0x0074_e960;
    while m.read8(end) != 0 {
        end += 1;
    }
    let separator = m.read16(0x0047_80b8);
    m.write8(end, separator as u8);
    m.write8(end + 1, (separator >> 8) as u8);
    let suffix = h.text_by_id(0x1991);
    let mut at = 0;
    loop {
        let byte = m.read8(suffix + at);
        m.write8(end + at, byte);
        if byte == 0 {
            break;
        }
        at += 1;
    }
    m.write32(0x007a_1560, 2);
    m.write32(0x0074_e938, 1);
    m.write32(PLAYER_STATE.wrapping_add(slot.wrapping_mul(4)), 8);
}

/// Callees of the stats aggregate 0x69a60.
pub trait StatsHost {
    /// 0x356870 local slot index of player `player` (negative when absent).
    fn fn_00356870_local_slot(&mut self, player: u32) -> i32;
    /// 0x230530 stats entry descriptor; the value at +0xc selects the merge rule.
    fn fn_00230530_merge_rule(&mut self, entry: u32) -> u32;
    /// 0x1cd170 refresh after the profile pointer changes.
    fn fn_001cd170_refresh(&mut self);
}

/// Merge rule for a stat entry with the descriptor value at +0xc: 0 and 1 take the minimum, 3 and 4
/// the maximum, 2 and above 4 keep the aggregate.
fn merge_stat(rule: u32, entry: i32, aggregate: i32) -> i32 {
    match rule {
        0 | 1 if entry < aggregate => entry,
        3 | 4 if entry > aggregate => entry,
        _ => aggregate,
    }
}

/// 0x69a60 aggregate all local slots' stats blocks into the 0x6f11c0 summary and point the profile
/// pointer at it. `count` is the number of local players.
pub fn profile_stats_aggregate_all_slots(
    m: &mut impl GuestMemory,
    h: &mut impl StatsHost,
    count: i32,
) {
    let aggregate = STATS_AGGREGATE;
    let clear = |m: &mut dyn GuestMemory| {
        for i in 0..0x313u32 {
            m.write32(aggregate + i * 4, 0);
        }
    };
    if m.read32(MODE_FLAGS) & MODE_FLAG_USE_AGGREGATE != 0 {
        clear(m);
        m.write32(PROFILE_PTR, aggregate);
        return;
    }
    if count == 1 {
        let slot = h.fn_00356870_local_slot(0);
        if slot >= 0 && slot_is_active(m, slot as u32) {
            let block = stats_record_ptr(m, slot as u32);
            m.write32(PROFILE_PTR, block);
            h.fn_001cd170_refresh();
        } else {
            m.write32(PROFILE_PTR, aggregate);
        }
        return;
    }
    clear(m);
    for player in 0..count.max(0) as u32 {
        let slot = h.fn_00356870_local_slot(player);
        if !(0..SLOT_COUNT).contains(&slot) {
            continue;
        }
        let block = stats_record_ptr(m, slot as u32);
        for off in (8..0x288u32).step_by(4) {
            let sum = m
                .read32(aggregate + off)
                .wrapping_add(m.read32(block + off));
            m.write32(aggregate + off, sum);
        }
        for off in (0xc0c..0xc4cu32).step_by(4) {
            let merged = m.read32(aggregate + off) | m.read32(block + off);
            m.write32(aggregate + off, merged);
        }
        for off in [0xbf0u32, 0xbf4, 0xbf8, 0xbfc, 0xc00, 0xc04, 0xc08] {
            let merged = m.read32(aggregate + off) | m.read32(block + off);
            m.write32(aggregate + off, merged);
        }
        for entry in 0..0x30u32 {
            let off = 0x490 + entry * 0x18;
            let (agg_count, block_count) = (m.read32(aggregate + off), m.read32(block + off));
            let block_value = m.read32(block + off + 4);
            if agg_count == 0 {
                m.write32(aggregate + off + 4, block_value);
            } else if block_count != 0 {
                let rule = h.fn_00230530_merge_rule(entry);
                let current = m.read32(aggregate + off + 4) as i32;
                let merged = merge_stat(rule, block_value as i32, current);
                m.write32(aggregate + off + 4, merged as u32);
            }
            m.write32(aggregate + off, agg_count.wrapping_add(block_count));
        }
    }
    m.write32(PROFILE_PTR, aggregate);
    h.fn_001cd170_refresh();
}

/// Callees of the save game write flows 0x192580 and 0x192f00.
pub trait SaveWriteHost {
    /// 0x69d90 prepare the profile name slot (player index, name buffer, 0).
    fn fn_00069d90_prepare_name(&mut self, index: u32, name_buffer: u32);
    /// 0x696f0 address of the slot entry (the dword at +0 is the record pointer).
    fn fn_000696f0_slot_entry(&mut self, index: u32) -> u32;
    /// 0x696a0 packed local time.
    fn fn_000696a0_packed_time(&mut self) -> u32;
    /// 0x263e0 write the save: (device, label, name, buffer, size, flags) -> status (1 ok, 2 retry).
    fn fn_000263e0_save_game_write(
        &mut self,
        m: &mut dyn GuestMemory,
        device: u32,
        label: u32,
        name: u32,
        buffer: u32,
        size: u32,
        flags: u32,
    ) -> u32;
    /// 0x1504f0 crc32 of a buffer.
    fn fn_001504f0_crc32(&mut self, m: &mut dyn GuestMemory, buffer: u32, size: u32) -> u32;
    /// 0x24950 save error message text.
    fn fn_00024950_error_message(
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
    fn text_by_id(&mut self, id: u32) -> u32;
    /// 0x45390 + 0x45820 show the text id as a popup.
    fn text_popup(&mut self, text_id: u32);
    /// 0x25bb0 volume serial of the device (forwarded to `save_device`).
    fn device_serial(&mut self, device: u32) -> u32;
    /// 0x69820 allocate an allocation record for a name (name, field 0x24, field 0x28).
    fn fn_00069820_alloc_record_with_name(
        &mut self,
        m: &mut dyn GuestMemory,
        name: u32,
        f24: u32,
        f28: u32,
    ) -> u32;
    /// 0x190fd0 clear the pending bit and continue (tail call).
    fn fn_00190fd0_clear_pending_bit(&mut self, m: &mut dyn GuestMemory, bit: u32);
    /// 0x746a0 sound request for a player record.
    fn fn_000746a0_sound_request(&mut self, player: u32);
    /// 0x25280 save file load verify (device, name, label, buffer, size, flags) -> status.
    fn fn_00025280_load_verify(
        &mut self,
        m: &mut dyn GuestMemory,
        device: u32,
        name: u32,
        label: u32,
        buffer: u32,
        size: u32,
        flags: u32,
    ) -> u32;
    /// 0x26320 find the device holding a save: (label, name, 0, 0, serial, device, out) -> index.
    fn fn_00026320_find_device(
        &mut self,
        m: &mut dyn GuestMemory,
        label: u32,
        name: u32,
        serial: u32,
        device: u32,
        out: u32,
    ) -> i32;
    /// 0x150560 verify the trailing crc of a buffer against `expected`.
    fn fn_00150560_crc_verify(
        &mut self,
        m: &mut dyn GuestMemory,
        buffer: u32,
        size: u32,
        expected: u32,
    ) -> u32;
    /// 0x79b70 enter a state descriptor.
    fn fn_00079b70_enter_state(&mut self, state_word: u32, descriptor: u32);
    /// 0x79860 state change helper (target, argument, -1).
    fn fn_00079860_state_change(&mut self, target: u32, argument: u32, value: u32);
    /// 0x25980 poll storage device changes.
    fn fn_00025980_device_changes_poll(&mut self);
}

/// Label pointer passed to the save write (the anicemap file name literal).
const SAVE_LABEL_LITERAL: u32 = 0x0047_dfc8;
const SLOT_SIZE: u32 = SLOT_STRIDE;
const SLOT_TIME_FIELD: u32 = 0x1edc;
const SLOT_STATE_FIELD: u32 = 0x1ee0;
const PROFILE_NAME_TABLE: u32 = 0x0074_ed60;
const ERROR_BUFFER: u32 = 0x0074_e960;

fn append_error_suffix(m: &mut impl GuestMemory, h: &mut impl SaveWriteHost) {
    let mut end = ERROR_BUFFER;
    while m.read8(end) != 0 {
        end += 1;
    }
    let separator = m.read16(0x0047_80b8);
    m.write8(end, separator as u8);
    m.write8(end + 1, (separator >> 8) as u8);
    let suffix = h.text_by_id(0x1991);
    let mut at = 0;
    loop {
        let byte = m.read8(suffix + at);
        m.write8(end + at, byte);
        if byte == 0 {
            break;
        }
        at += 1;
    }
}

/// Save failure reporting shared by the two flows: error text, flags and state.
fn report_save_failure(
    m: &mut impl GuestMemory,
    h: &mut impl SaveWriteHost,
    device: u32,
    status: u32,
    name: u32,
) {
    let text = h.text_by_id(0x198d);
    h.fn_00024950_error_message(m, device, status, ERROR_BUFFER, 2, name, text);
    append_error_suffix(m, h);
}

/// 0x192580 save game write for the player in ebx, then clear its pending bit.
pub fn save_game_write_for_slot_then_clear_bit(
    m: &mut impl GuestMemory,
    h: &mut impl SaveWriteHost,
    player: u32,
) {
    let index = m.read32(player + 0x38);
    h.fn_00069d90_prepare_name(
        index,
        PROFILE_NAME_TABLE.wrapping_add(index.wrapping_mul(0x1e)),
    );
    let name = slot_entry_ptr_plus_4(m, index as i32);
    let slot = h.fn_000696f0_slot_entry(index);
    m.write32(slot + SLOT_STATE_FIELD, 2);
    let time = h.fn_000696a0_packed_time();
    let saved = m.read32(slot);
    m.write32(slot + SLOT_TIME_FIELD, time);
    m.write32(slot, 0);
    let device = m.read32(PLAYER_DEVICE.wrapping_add(index.wrapping_mul(4)));
    let status =
        h.fn_000263e0_save_game_write(m, device, SAVE_LABEL_LITERAL, name, slot, SLOT_SIZE, 3);
    m.write32(slot, saved);
    if status == 1 {
        let entry = h.fn_000696f0_slot_entry(index);
        let record = m.read32(entry);
        m.write32(entry, 0);
        let crc = h.fn_001504f0_crc32(m, entry, SLOT_SIZE);
        m.write32(entry, record);
        let device = m.read32(PLAYER_DEVICE.wrapping_add(index.wrapping_mul(4)));
        let serial = h.device_serial(device);
        let record = m.read32(slot);
        m.write32(record + 0x2c, serial);
        slot_set_fields(m, index as i32, device, crc);
        let table_entry = PROFILE_NAME_TABLE.wrapping_add(index.wrapping_mul(0x1e));
        if name != 0 {
            let mut at = 0;
            loop {
                let byte = m.read8(name + at);
                m.write8(table_entry + at, byte);
                if byte == 0 {
                    break;
                }
                at += 1;
            }
        } else {
            m.write8(table_entry, 0);
        }
        h.fn_00190fd0_clear_pending_bit(m, index);
    } else if status == 2 {
        m.write32(PLAYER_STATE.wrapping_add(index.wrapping_mul(4)), 0xb);
    } else {
        m.write32(0x0074_e938, 1);
        report_save_failure(m, h, device, status, name);
        m.write32(0x004e_8d50, u32::MAX);
        m.write32(0x007a_1560, 2);
        m.write32(PLAYER_STATE.wrapping_add(index.wrapping_mul(4)), 8);
    }
}

/// 0x192f00 save game write for the global player index (0x78ad34), with popup and sound feedback.
pub fn save_game_write_for_slot_with_sound_feedback(
    m: &mut impl GuestMemory,
    h: &mut impl SaveWriteHost,
) {
    let index = m.read32(0x0078_ad34);
    let name = slot_entry_ptr_plus_4(m, index as i32);
    let slot = h.fn_000696f0_slot_entry(index);
    let old_time = m.read32(slot + SLOT_TIME_FIELD);
    let new_time = h.fn_000696a0_packed_time();
    m.write32(slot + SLOT_TIME_FIELD, new_time);
    let saved = m.read32(slot);
    m.write32(slot, 0);
    let device = m.read32(PLAYER_DEVICE.wrapping_add(index.wrapping_mul(4)));
    let status =
        h.fn_000263e0_save_game_write(m, device, SAVE_LABEL_LITERAL, name, slot, SLOT_SIZE, 3);
    m.write32(slot, saved);
    m.write32(slot + SLOT_TIME_FIELD, old_time);
    let popup_entry = m.read32(SAVE_STATE_TEXT_ENTRY);
    let popup = m.read32(SAVE_STATE_TEXT_OBJECT) != 0;
    if status == 1 {
        if popup {
            h.text_popup(m.read32(popup_entry));
        }
        let index = m.read32(0x0078_ad34);
        m.write32(PLAYER_STATE.wrapping_add(index.wrapping_mul(4)), 6);
        let record = m.read32(slot);
        if m.read32(record + RECORD_PAYLOAD_24) == u32::MAX {
            m.write32(slot + SLOT_TIME_FIELD, new_time);
            let device =
                m.read32(PLAYER_DEVICE.wrapping_add(m.read32(0x0078_ad34).wrapping_mul(4)));
            let serial = h.device_serial(device);
            let record = m.read32(slot);
            m.write32(record + RECORD_DEVICE_SERIAL, serial);
            let entry = h.fn_000696f0_slot_entry(m.read32(0x0078_ad34));
            let saved_record = m.read32(entry);
            m.write32(entry, 0);
            let crc = h.fn_001504f0_crc32(m, entry, SLOT_SIZE);
            m.write32(entry, saved_record);
            let index = m.read32(0x0078_ad34);
            let device = m.read32(PLAYER_DEVICE.wrapping_add(index.wrapping_mul(4)));
            slot_set_fields(m, index as i32, device, crc);
        }
        return;
    }
    if popup {
        let text = m.read32(popup_entry.wrapping_add(0xc));
        h.text_popup(text);
    }
    let index = m.read32(0x0078_ad34);
    if status == 2 {
        m.write32(PLAYER_STATE.wrapping_add(index.wrapping_mul(4)), 3);
        return;
    }
    m.write32(0x0074_e938, 1);
    m.write32(0x007a_1560, 1);
    m.write32(0x004e_8d50, u32::MAX);
    let device = m.read32(PLAYER_DEVICE.wrapping_add(index.wrapping_mul(4)));
    report_save_failure(m, h, device, status, name);
    let index = m.read32(0x0078_ad34);
    m.write32(PLAYER_STATE.wrapping_add(index.wrapping_mul(4)), 5);
}

/// 0x191250 profile load verify: load the selected profile's save, verify it and register the
/// slot with its crc32, or report the failure.
pub fn save_profile_load_verify_register_slot(
    m: &mut impl GuestMemory,
    h: &mut impl SaveWriteHost,
    player: u32,
) {
    let index = m.read32(player + 0x38);
    let record = PROFILE_LIST + m.read32(player + 0x18c).wrapping_mul(PROFILE_RECORD_STRIDE);
    let slot = h.fn_000696f0_slot_entry(index);
    if slot == 0 {
        return;
    }
    h.fn_000746a0_sound_request(player);
    let saved = m.read32(slot);
    let owner = m.read32(record + PROFILE_OWNER_OFFSET);
    let buffer = h.fn_000696f0_slot_entry(index);
    let mut status =
        h.fn_00025280_load_verify(m, owner, record, SAVE_LABEL_LITERAL, buffer, SLOT_SIZE, 2);
    let state = m.read32(slot + SLOT_STATE_FIELD);
    m.write32(slot, saved);
    if state != 2 {
        status = 7;
    }
    if state == 2 && status == 1 {
        let name = record + PROFILE_NAME_OFFSET;
        record_zero_and_null_pointer_field(m, saved);
        let new_record = h.fn_00069820_alloc_record_with_name(m, name, u32::MAX, 0);
        m.write32(slot, new_record);
        m.write32(new_record + RECORD_BACK_POINTER, slot);
        m.write32(new_record + RECORD_PENDING, 1);
        let serial = h.device_serial(owner);
        let new_record = m.read32(slot);
        m.write32(new_record + RECORD_DEVICE_SERIAL, serial);
        let entry = h.fn_000696f0_slot_entry(index);
        let held = m.read32(entry);
        m.write32(entry, 0);
        let crc = h.fn_001504f0_crc32(m, entry, SLOT_SIZE);
        m.write32(entry, held);
        m.write32(PLAYER_DEVICE.wrapping_add(index.wrapping_mul(4)), owner);
        slot_set_fields(m, index as i32, owner, crc);
        h.fn_00190fd0_clear_pending_bit(m, index);
        let table_entry = PROFILE_NAME_TABLE.wrapping_add(index.wrapping_mul(0x1e));
        let mut at = 0;
        loop {
            let byte = m.read8(name + at);
            m.write8(table_entry + at, byte);
            if byte == 0 {
                break;
            }
            at += 1;
        }
        return;
    }
    let text = h.text_by_id(0x198d);
    h.fn_00024950_error_message(
        m,
        owner,
        status,
        ERROR_BUFFER,
        1,
        record + PROFILE_NAME_OFFSET,
        text,
    );
    append_error_suffix(m, h);
    m.write32(0x004e_8d50, u32::MAX);
    m.write32(0x007a_1560, 2);
    m.write32(0x0074_e938, 1);
    m.write32(PLAYER_STATE.wrapping_add(index.wrapping_mul(4)), 8);
}

/// 0x1922d0 save game write/verify flow over the players in `mask` (or all active ones when
/// `which` is -1): find each player's device, verify or rewrite the slot, and report failures
/// through the state variables at 0x74e9xx. Returns 1 when a failure was reported.
pub fn save_game_write_verify_flow(
    m: &mut impl GuestMemory,
    h: &mut impl SaveWriteHost,
    target: u32,
    argument: u32,
    which: u32,
) -> u32 {
    m.write32(0x0074_f9e0, target);
    if m.read32(0x0078_ad24) & 0x1_0000 == 0 {
        h.fn_00079860_state_change(target, argument, u32::MAX);
        let flags = m.read32(0x0078_ad24) & !1;
        m.write32(0x0078_ad24, flags);
        return 0;
    }
    h.fn_00025980_device_changes_poll();
    let flags = m.read32(0x0078_ad24) | 1;
    m.write32(0x0078_ad24, flags);
    let mut mask = 0u32;
    if which == u32::MAX {
        for player in 0..(m.read32(0x0074_e934) as i32).max(0) as u32 {
            if slot_field_0x24(m, player as i32) != 0xb {
                mask |= 1 << player;
            }
        }
    } else {
        mask = 1u32.wrapping_shl(which);
    }
    let mut failed = 0u32;
    let out = 0x0074_e9a0; // scratch for the device name found by 0x26320
    let mut player = 0u32;
    while mask != 0 {
        if mask & 1 != 0 {
            let slot = h.fn_000696f0_slot_entry(player);
            if slot != 0 {
                let record = m.read32(slot);
                let name = record + 4;
                let (serial, device_word) = (
                    m.read32(record + RECORD_DEVICE_SERIAL),
                    m.read32(record + RECORD_PAYLOAD_24),
                );
                let found = h.fn_00026320_find_device(
                    m,
                    SAVE_LABEL_LITERAL,
                    name,
                    serial,
                    device_word,
                    out,
                );
                if (0..11).contains(&found) {
                    let device = found as u32;
                    let buffer = 0x0074_ea00; // scratch buffer of the original's stack frame
                    let mut status = h.fn_00025280_load_verify(
                        m,
                        device,
                        out,
                        SAVE_LABEL_LITERAL,
                        buffer,
                        SLOT_SIZE,
                        2,
                    );
                    let mut done = false;
                    if status == 1 {
                        let expected = slot_field_0x28(m, player as i32);
                        if h.fn_00150560_crc_verify(m, buffer, SLOT_SIZE, expected) == 0 {
                            status = 7;
                        } else {
                            let held = m.read32(slot);
                            m.write32(slot, 0);
                            status = h.fn_000263e0_save_game_write(
                                m,
                                device,
                                SAVE_LABEL_LITERAL,
                                name,
                                slot,
                                SLOT_SIZE,
                                2,
                            );
                            m.write32(slot, held);
                            m.write32(held + RECORD_PAYLOAD_24, device);
                            let serial = h.device_serial(device);
                            let current = m.read32(slot);
                            m.write32(current + 0x2c, serial);
                            if status == 1 {
                                let entry = h.fn_000696f0_slot_entry(player);
                                let saved = m.read32(entry);
                                m.write32(entry, 0);
                                let crc = h.fn_001504f0_crc32(m, entry, SLOT_SIZE);
                                m.write32(entry, saved);
                                slot_set_fields(m, player as i32, device, crc);
                                done = true;
                            }
                        }
                    }
                    if !done {
                        m.write32(0x0074_e948, 1);
                        m.write32(0x0074_e944, argument);
                        m.write32(0x0074_f9d8, status);
                        m.write32(0x0074_e910, device);
                        h.fn_00079b70_enter_state(m.read32(0x0074_f9e0), 0x004e_8d40);
                        failed = 1;
                    }
                } else {
                    m.write32(0x0074_f9d8, u32::MAX);
                    m.write32(0x0074_e910, u32::MAX);
                    m.write32(0x0074_e948, 2);
                    m.write32(0x0074_e944, argument);
                    m.write32(0x0074_f9dc, player);
                    h.fn_00079b70_enter_state(m.read32(0x0074_f9e0), 0x004e_8d40);
                    failed = 1;
                }
            }
        }
        mask >>= 1;
        player += 1;
    }
    let flags = m.read32(0x0078_ad24) & !0x1_0000;
    m.write32(0x0078_ad24, flags);
    if failed == 0 {
        h.fn_00079860_state_change(target, argument, u32::MAX);
        let flags = m.read32(0x0078_ad24) & !1;
        m.write32(0x0078_ad24, flags);
    }
    failed
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
        session: bool,
        time: u32,
    }
    impl ProfileHost for Host {
        fn net_is_session_active(&mut self) -> bool {
            self.session
        }
        fn local_slot_index(&mut self, _p: u32) -> u32 {
            2
        }
        fn local_time_encode(&mut self) -> u32 {
            self.time
        }
        fn string_copy(&mut self, m: &mut dyn GuestMemory, dst: u32, src: u32, len: u32) {
            for i in 0..len {
                let b = m.read8(src + i);
                m.write8(dst + i, b);
            }
        }
    }

    #[test]
    fn record_base_and_time() {
        let mut h = Host::default();
        assert_eq!(record_base_for_local_slot_zero(&mut h), DEFAULT_SLOT_RECORD);
        h.session = true;
        assert_eq!(
            record_base_for_local_slot_zero(&mut h),
            DEFAULT_SLOT_RECORD + 2 * SLOT_STRIDE
        );
        h.time = 0xffff_ffff;
        assert_eq!(pack_local_time(125, &mut h), (5 << 26) | 0x03ff_ffff);
    }

    #[test]
    fn packed_time_preserves_low_bits_and_signed_remainder() {
        let mut h = Host::default();
        for time in [0, 1, 0x03ff_ffff, 0x0400_0000, u32::MAX] {
            h.time = time;
            for seconds in -121i32..=121 {
                let actual = pack_local_time(seconds, &mut h);
                assert_eq!(actual & 0x03ff_ffff, time & 0x03ff_ffff);
                assert_eq!(actual >> 26, ((seconds % 60) as u32) & 0x3f);
            }
        }
    }

    #[test]
    fn slot_accessors() {
        let mut m = Mem::default();
        m.write32(PROFILE_PTR, 0x1234);
        assert_eq!(profile_ptr(&m), 0x1234);
        assert!(!slot_is_active(&m, 3));
        let rec = 0x0100_0000;
        m.write32(SLOT_TABLE + 3 * SLOT_STRIDE, rec);
        assert!(slot_is_active(&m, 3));
        assert_eq!(slot_entry_ptr_plus_4(&m, 3), rec + 4);
        assert_eq!(slot_entry_ptr_plus_4(&m, 16), 0);
        slot_set_fields(&mut m, 3, 7, 9);
        assert_eq!((slot_field_0x24(&m, 3), slot_field_0x28(&m, 3)), (7, 9));
        assert_eq!(slot_field_0x24(&m, 99), 0);
        m.write32(0, 0x4000);
        m.write32(0x4000 + 0x24, 5);
        assert_eq!(slot_field_0x24(&m, 4), 5);
        slots_clear_all(&mut m);
        assert_eq!(profile_ptr(&m), 0);
        assert!(!slot_is_active(&m, 3));
    }

    #[test]
    fn alloc_and_zero() {
        let mut m = Mem::default();
        let mut h = Host::default();
        for (i, b) in b"hero\0".iter().enumerate() {
            m.write8(0x2000 + i as u32, *b);
        }
        let rec = alloc_record_with_name(&mut m, &mut h, 0x2000, 5, 6);
        assert_eq!(rec, ALLOC_TABLE);
        assert_eq!(m.read8(rec + 4), b'h');
        assert_eq!(
            (m.read32(rec), m.read32(rec + 0x24), m.read32(rec + 0x28)),
            (1, 5, 6)
        );
        let rec2 = alloc_record_with_name(&mut m, &mut h, 0x2000, 0, 0);
        assert_eq!(rec2, ALLOC_TABLE + ALLOC_STRIDE);
        record_zero_and_null_pointer_field(&mut m, rec);
        assert_eq!(m.read32(rec), 0);
        assert_eq!(m.read8(rec + 4), 0);
    }

    #[test]
    fn stats_pointers() {
        let mut m = Mem::default();
        assert_eq!(stats_record_ptr(&m, 1), STATS_AGGREGATE);
        m.write32(SLOT_TABLE + SLOT_STRIDE, 1);
        assert_eq!(stats_record_ptr(&m, 1), STATS_BASE + SLOT_STRIDE);
        assert_eq!(
            stats_entry_ptr(&m, 1, 2),
            STATS_BASE + SLOT_STRIDE + 0x490 + 0x30
        );
        m.write32(MODE_FLAGS, MODE_FLAG_USE_AGGREGATE);
        assert_eq!(stats_record_ptr(&m, 1), STATS_AGGREGATE);
        assert_eq!(stats_entry_ptr(&m, 1, 1), STATS_AGGREGATE + 0x490 + 0x18);
    }

    #[derive(Default)]
    struct FlowHost {
        calls: Vec<(u32, u32)>,
        active: bool,
    }
    impl ProfileFlowHost for FlowHost {
        fn fn_001906b0_map_prefix_detect(&mut self, _n: u32, _f: u32) -> u32 {
            0
        }
        fn fn_00025e40_find_enumerate(&mut self, k: u32, _t: u32, _c: u32, ctx: u32) {
            self.calls.push((0x25e40, k + ctx));
        }
        fn fn_00069a60_stats_aggregate(&mut self, s: u32) {
            self.calls.push((0x69a60, s));
        }
        fn fn_00079b70_enter_state(&mut self, a: u32, b: u32) {
            self.calls.push((0x79b70, a + b));
        }
        fn fn_000746a0_sound_request(&mut self, p: u32) {
            self.calls.push((0x746a0, p));
        }
        fn text_popup(&mut self, id: u32) {
            self.calls.push((0x45820, id));
        }
        fn fn_00192580_save_game_write_for_slot(&mut self, p: u32) {
            self.calls.push((0x192580, p));
        }
        fn local_time_fields(&mut self) -> [u16; 7] {
            [2026, 10, 3, 9, 23, 59, 7]
        }
        fn fn_0014ff30_pack_time(&mut self, b: [u8; 8]) -> u32 {
            u32::from_le_bytes([b[1], b[2], b[3], b[7]])
        }
        fn memory_unit_slot_is_active(&mut self, _d: u32) -> bool {
            self.active
        }
    }

    #[test]
    fn profile_list_flows() {
        let mut m = Mem::default();
        let mut h = FlowHost::default();
        for (i, b) in b"abc\0".iter().enumerate() {
            m.write8(0x3000 + 0x104 + i as u32, *b);
            m.write8(0x4000 + i as u32, *b);
        }
        m.write32(0x3000 + PROFILE_OWNER_OFFSET, 5);
        assert_eq!(save_enumerate_append_entry(&mut m, &mut h, 0x3000), None);
        assert_eq!(m.read32(PROFILE_LIST_COUNT), 1);
        assert_eq!(profile_list_find_entry(&m, 0x4000, 5), PROFILE_LIST);
        assert_eq!(profile_list_find_entry(&m, 0x4000, 6), 0);
        assert_eq!(profile_list_name_is_free_for_owner(&m, 0x4000, 5), 0);
        assert_eq!(profile_list_name_is_free_for_owner(&m, 0x4000, 6), 1);
        m.write32(PROFILE_LIST_COUNT, 0x1e);
        assert_eq!(save_enumerate_append_entry(&mut m, &mut h, 0x3000), None);
        m.write32(0x5000, 0xa);
        save_enumerate_entry_callback(&m, &mut h, 1, 0x5000);
        assert!(h.calls.is_empty());
        m.write32(0x5000, 3);
        save_enumerate_entry_callback(&m, &mut h, 1, 0x5000);
        assert_eq!(h.calls, vec![(0x25e40, 4)]);
    }

    #[test]
    fn pending_bits_feedback_and_time() {
        let mut m = Mem::default();
        let mut h = FlowHost::default();
        m.write32(PENDING_PLAYER_BITS, 0b101);
        clear_pending_bit_then_enter_state(&mut m, &mut h, 0);
        assert_eq!((m.read32(PLAYER_STATE), m.read32(0x74e938)), (0xe, 1));
        clear_pending_bit_then_enter_state(&mut m, &mut h, 2);
        assert_eq!(m.read32(PENDING_PLAYER_BITS), 0);
        assert_eq!(h.calls.len(), 2);
        h.calls.clear();
        m.write32(0x3000 + 0x38, 2);
        m.write32(SAVE_STATE_TEXT_OBJECT, 1);
        m.write32(SAVE_STATE_TEXT_ENTRY, 0x6000);
        m.write32(0x6000 + 0xc, 0x99);
        memory_unit_slot_inactive_sound_request(&mut m, &mut h, 0x3000);
        assert_eq!(h.calls, vec![(0x45820, 0x99)]);
        h.active = true;
        h.calls.clear();
        memory_unit_slot_inactive_sound_request(&mut m, &mut h, 0x3000);
        assert_eq!(
            (h.calls.clone(), m.read32(PLAYER_STATE + 8)),
            (vec![(0x746a0, 0x3000)], 7)
        );
        m.write32(0x78ad34, 1);
        memory_unit_slot_inactive_sound_request_global_index(&mut m, &mut h, 0x3000);
        assert_eq!(m.read32(PLAYER_STATE + 4), 4);
        thunk_save_game_write_for_slot(&mut h, 9);
        assert_eq!(h.calls.last(), Some(&(0x192580, 9)));
        assert_eq!(
            local_time_encode_31day_timestamp(&mut h),
            u32::from_le_bytes([0x07, 0x59, 0x23, 0x26])
        );
        assert_eq!(to_bcd(1234), 0x34);
    }

    #[derive(Default)]
    struct MenuHost {
        log: Vec<(u32, u32)>,
        count: u32,
        pending: i32,
    }
    impl SaveMenuHost for MenuHost {
        fn fn_00069620_profile_entry(&mut self, p: u32) -> u32 {
            0x8000 + p * 0x100
        }
        fn fn_00079b70_enter_state(&mut self, a: u32, b: u32) {
            self.log.push((0x79b70, a + b));
        }
        fn fn_00042720_active_record_count(&mut self) -> u32 {
            self.count
        }
        fn fn_00071940_child(&mut self, w: u32, i: u32) -> u32 {
            w + i * 0x100
        }
        fn fn_00071c70_set_enabled(&mut self, c: u32, e: u32) {
            self.log.push((c, e));
        }
        fn fn_00071a60_focused_child(&mut self, w: u32) -> u32 {
            w + 0x200
        }
        fn fn_00077110_move_focus(&mut self, w: u32, f: u32) {
            self.log.push((0x77110, w + f));
        }
        fn fn_00071de0_has_focusable(&mut self, _w: u32) -> u32 {
            0
        }
        fn fn_002d2490_pending_count(&mut self) -> i32 {
            self.pending
        }
        fn fn_003377d0_slot_record(&mut self, i: u32) -> u32 {
            0x9000 + i * 0x10
        }
        fn text_by_id(&mut self, id: u32) -> u32 {
            0xa000 + id
        }
        fn fn_003c8960_string_copy(&mut self, m: &mut dyn GuestMemory, d: u32, s: u32, l: u32) {
            m.write8(d, s as u8);
            self.log.push((0x3c8960, l));
        }
        fn fn_000746a0_sound_request(&mut self, p: u32) {
            self.log.push((0x746a0, p));
        }
        fn fn_00024950_error_message(
            &mut self,
            m: &mut dyn GuestMemory,
            _d: u32,
            _c: u32,
            buf: u32,
            _k: u32,
            _s: u32,
            _t: u32,
        ) {
            m.write8(buf, b'E');
        }
    }

    #[test]
    fn save_menu_flows() {
        let mut m = Mem::default();
        let mut h = MenuHost {
            count: 2,
            pending: 1,
            ..Default::default()
        };
        m.write32(0x78ad34, 1);
        m.write32(0x7ba9e0, 0);
        m.write32(0x7ba364 + 4, 0x55);
        save_player_block_to_profile_entry(&mut m, &mut h);
        assert_eq!(
            (m.read32(0x8100 + 4), m.read32(0x78ad24) & 0x10000),
            (0x55, 0x10000)
        );
        h.log.clear();
        m.write32(0x78ae00, 1);
        profile_menu_open_enable_items(&mut m, &mut h, 0x1000);
        assert!(h.log.contains(&(0x1000 + 0x300, 1)));
        assert_eq!(m.read32(0x1000 + 0x300 + 0x28) & 4, 4);
        assert_eq!(m.read32(0x78ad2c), 1);
        h.pending = 0;
        m.write32(0x778d38, 0x20000);
        save_slot_buffer_fill_eight_slot_names(&mut m, &mut h);
        assert_eq!(m.read32(0x20000 + 8), 8);
        let n = save_slot_buffer_append_entry(&mut m, 0x9000);
        assert_eq!((n, m.read32(0x20000 + 8)), (8, 9));
        m.write32(0x20000 + 8, 0x32);
        assert_eq!(save_slot_buffer_append_entry(&mut m, 0x9000), u32::MAX);
        h.log.clear();
        m.write32(0x3000 + 0x38, 1);
        save_profile_create_check_list_limit(&mut m, &mut h, 0x3000);
        assert_eq!(m.read32(PLAYER_STATE + 4), 3);
        m.write32(PROFILE_LIST_COUNT, 0x1e);
        save_profile_create_check_list_limit(&mut m, &mut h, 0x3000);
        assert_eq!(
            (
                m.read32(PLAYER_STATE + 4),
                m.read32(0x7a1560),
                m.read8(0x74e960)
            ),
            (8, 2, b'E')
        );
    }

    #[derive(Default)]
    struct Stats {
        slots: Vec<i32>,
        rule: u32,
        refreshed: u32,
    }
    impl StatsHost for Stats {
        fn fn_00356870_local_slot(&mut self, p: u32) -> i32 {
            self.slots[p as usize]
        }
        fn fn_00230530_merge_rule(&mut self, _e: u32) -> u32 {
            self.rule
        }
        fn fn_001cd170_refresh(&mut self) {
            self.refreshed += 1;
        }
    }

    #[test]
    fn stats_aggregate() {
        let mut m = Mem::default();
        let mut h = Stats {
            slots: vec![0, 1],
            rule: 3,
            ..Default::default()
        };
        for slot in 0..2u32 {
            m.write32(SLOT_TABLE + slot * SLOT_STRIDE, 1);
            let block = STATS_BASE + slot * SLOT_STRIDE;
            m.write32(block + 8, 10 + slot);
            m.write32(block + 0xc0c, 1 << slot);
            m.write32(block + 0xbf0, 4 << slot);
            m.write32(block + 0x490, 2);
            m.write32(block + 0x494, 100 + slot);
        }
        profile_stats_aggregate_all_slots(&mut m, &mut h, 2);
        assert_eq!(m.read32(STATS_AGGREGATE + 8), 21);
        assert_eq!(m.read32(STATS_AGGREGATE + 0xc0c), 3);
        assert_eq!(m.read32(STATS_AGGREGATE + 0xbf0), 12);
        assert_eq!(
            (
                m.read32(STATS_AGGREGATE + 0x490),
                m.read32(STATS_AGGREGATE + 0x494)
            ),
            (4, 101)
        );
        assert_eq!((m.read32(PROFILE_PTR), h.refreshed), (STATS_AGGREGATE, 1));
        h.slots = vec![1];
        profile_stats_aggregate_all_slots(&mut m, &mut h, 1);
        assert_eq!(m.read32(PROFILE_PTR), STATS_BASE + SLOT_STRIDE);
        h.slots = vec![-1];
        profile_stats_aggregate_all_slots(&mut m, &mut h, 1);
        assert_eq!(m.read32(PROFILE_PTR), STATS_AGGREGATE);
        assert_eq!(
            (
                merge_stat(0, 1, 5),
                merge_stat(3, 1, 5),
                merge_stat(2, 1, 5),
                merge_stat(9, 1, 5)
            ),
            (1, 5, 5, 5)
        );
        m.write32(MODE_FLAGS, MODE_FLAG_USE_AGGREGATE);
        profile_stats_aggregate_all_slots(&mut m, &mut h, 2);
        assert_eq!(m.read32(STATS_AGGREGATE + 8), 0);
    }

    #[derive(Default)]
    struct WriteHost {
        status: u32,
        verify: u32,
        found: i32,
        crc_ok: u32,
        log: Vec<(&'static str, u32)>,
    }
    impl SaveWriteHost for WriteHost {
        fn fn_00069d90_prepare_name(&mut self, i: u32, _b: u32) {
            self.log.push(("prep", i));
        }
        fn fn_000696f0_slot_entry(&mut self, i: u32) -> u32 {
            SLOT_TABLE + i * SLOT_STRIDE
        }
        fn fn_000696a0_packed_time(&mut self) -> u32 {
            0x1234
        }
        fn fn_000263e0_save_game_write(
            &mut self,
            _m: &mut dyn GuestMemory,
            d: u32,
            _l: u32,
            _n: u32,
            _b: u32,
            size: u32,
            f: u32,
        ) -> u32 {
            self.log.push(("write", d + size + f));
            self.status
        }
        fn fn_001504f0_crc32(&mut self, _m: &mut dyn GuestMemory, _b: u32, _s: u32) -> u32 {
            0xc0de
        }
        fn fn_00024950_error_message(
            &mut self,
            m: &mut dyn GuestMemory,
            _d: u32,
            c: u32,
            b: u32,
            _k: u32,
            _s: u32,
            _t: u32,
        ) {
            self.log.push(("err", c));
            m.write8(b, b'x');
        }
        fn text_by_id(&mut self, id: u32) -> u32 {
            0xb000 + id
        }
        fn text_popup(&mut self, id: u32) {
            self.log.push(("popup", id));
        }
        fn device_serial(&mut self, d: u32) -> u32 {
            d + 1
        }
        fn fn_00190fd0_clear_pending_bit(&mut self, _m: &mut dyn GuestMemory, b: u32) {
            self.log.push(("clear", b));
        }
        fn fn_00069820_alloc_record_with_name(
            &mut self,
            _m: &mut dyn GuestMemory,
            _n: u32,
            f24: u32,
            _f: u32,
        ) -> u32 {
            self.log.push(("alloc", f24));
            0x6000
        }
        fn fn_000746a0_sound_request(&mut self, p: u32) {
            self.log.push(("sound", p));
        }
        fn fn_00025280_load_verify(
            &mut self,
            _m: &mut dyn GuestMemory,
            _d: u32,
            _n: u32,
            _l: u32,
            _b: u32,
            _z: u32,
            _f: u32,
        ) -> u32 {
            self.log.push(("verify", 0));
            self.verify
        }
        fn fn_00026320_find_device(
            &mut self,
            _m: &mut dyn GuestMemory,
            _l: u32,
            _n: u32,
            _s: u32,
            _d: u32,
            _o: u32,
        ) -> i32 {
            self.found
        }
        fn fn_00150560_crc_verify(
            &mut self,
            _m: &mut dyn GuestMemory,
            _b: u32,
            _s: u32,
            _e: u32,
        ) -> u32 {
            self.crc_ok
        }
        fn fn_00079b70_enter_state(&mut self, a: u32, _b: u32) {
            self.log.push(("enter", a));
        }
        fn fn_00079860_state_change(&mut self, t: u32, _a: u32, v: u32) {
            self.log.push(("change", t.wrapping_add(v)));
        }
        fn fn_00025980_device_changes_poll(&mut self) {
            self.log.push(("poll", 0));
        }
    }

    #[test]
    fn save_write_flows() {
        let mut m = Mem::default();
        let mut h = WriteHost {
            status: 1,
            ..Default::default()
        };
        let player = 0x3000;
        m.write32(player + 0x38, 1);
        m.write32(PLAYER_DEVICE + 4, 4);
        let entry = SLOT_TABLE + SLOT_STRIDE;
        m.write32(entry, 0x5000);
        save_game_write_for_slot_then_clear_bit(&mut m, &mut h, player);
        assert_eq!(m.read32(entry), 0x5000);
        assert_eq!(m.read32(0x5000 + 0x2c), 5);
        assert_eq!(
            (m.read32(0x5000 + 0x24), m.read32(0x5000 + 0x28)),
            (4, 0xc0de)
        );
        assert_eq!(h.log.last(), Some(&("clear", 1)));
        assert_eq!(m.read32(entry + SLOT_STATE_FIELD), 2);
        h.status = 2;
        save_game_write_for_slot_then_clear_bit(&mut m, &mut h, player);
        assert_eq!(m.read32(PLAYER_STATE + 4), 0xb);
        h.status = 7;
        save_game_write_for_slot_then_clear_bit(&mut m, &mut h, player);
        assert_eq!(
            (m.read32(PLAYER_STATE + 4), m.read8(ERROR_BUFFER)),
            (8, b'x')
        );
        h.log.clear();
        m.write32(0x78ad34, 1);
        h.status = 1;
        m.write32(0x5000 + 0x24, u32::MAX);
        save_game_write_for_slot_with_sound_feedback(&mut m, &mut h);
        assert_eq!(m.read32(PLAYER_STATE + 4), 6);
        assert_eq!(m.read32(0x5000 + 0x24), 4);
        h.status = 2;
        save_game_write_for_slot_with_sound_feedback(&mut m, &mut h);
        assert_eq!(m.read32(PLAYER_STATE + 4), 3);
        h.status = 9;
        save_game_write_for_slot_with_sound_feedback(&mut m, &mut h);
        assert_eq!((m.read32(PLAYER_STATE + 4), m.read32(0x7a1560)), (5, 1));
    }

    #[test]
    fn profile_load_and_write_verify() {
        let mut m = Mem::default();
        let mut h = WriteHost {
            verify: 1,
            ..Default::default()
        };
        let player = 0x3000;
        m.write32(player + 0x38, 0);
        m.write32(player + 0x18c, 0);
        m.write32(PROFILE_LIST + PROFILE_OWNER_OFFSET, 6);
        m.write8(PROFILE_LIST + PROFILE_NAME_OFFSET, b'p');
        m.write32(SLOT_TABLE, 0x5000);
        m.write32(SLOT_TABLE + SLOT_STATE_FIELD, 2);
        save_profile_load_verify_register_slot(&mut m, &mut h, player);
        assert_eq!(m.read32(SLOT_TABLE), 0x6000);
        assert_eq!(m.read8(PROFILE_NAME_TABLE), b'p');
        assert!(h.log.contains(&("clear", 0)));
        m.write32(SLOT_TABLE + SLOT_STATE_FIELD, 3);
        save_profile_load_verify_register_slot(&mut m, &mut h, player);
        assert_eq!((m.read32(PLAYER_STATE), m.read8(ERROR_BUFFER)), (8, b'x'));
        // flow: no flag set -> state change only
        h.log.clear();
        assert_eq!(save_game_write_verify_flow(&mut m, &mut h, 5, 1, 0), 0);
        assert_eq!(h.log, vec![("change", 4)]);
        // flag set, device not found -> failure
        m.write32(0x78ad24, 0x10000);
        h.found = -1;
        m.write32(SLOT_TABLE, 0x7000);
        assert_eq!(save_game_write_verify_flow(&mut m, &mut h, 5, 1, 0), 1);
        assert_eq!(m.read32(0x74e948), 2);
        // device found, load ok, crc bad -> status 7 failure
        h.found = 3;
        h.crc_ok = 0;
        m.write32(0x78ad24, 0x10000);
        assert_eq!(save_game_write_verify_flow(&mut m, &mut h, 5, 1, 0), 1);
        assert_eq!(m.read32(0x74f9d8), 7);
        // everything ok
        h.crc_ok = 1;
        h.status = 1;
        m.write32(0x78ad24, 0x10000);
        assert_eq!(save_game_write_verify_flow(&mut m, &mut h, 5, 1, 0), 0);
        assert_eq!(m.read32(0x78ad24), 0);
    }
}
