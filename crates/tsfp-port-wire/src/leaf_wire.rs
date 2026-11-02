// SPDX-License-Identifier: GPL-3.0-or-later
//! T2091 (S3 writer wiring 2): adapter between the small leaf writers and the Rust owners of
//! crates/tsfp-state (SaveSlots and TextBank in `save_text.rs`, ModeSetup in `modes.rs`, PlayerSet stats in
//! `player.rs`). Same constraint and trick as `state_wire.rs` (T2082): tsfp-ports cannot depend on tsfp-state
//! as a crate, so the owner modules are compiled in by path with `GuestMemory`, the generated views and the
//! `game_state` module re-exported at this module's level. Pattern and limits: docs/t-s3-wiring-leaves.md.
//!
//! Each function is the only write path of a wired port to an owned cell. It mirrors the owner in
//! (`load`), calls the owner's write-through setter and returns. When the address is not a cell of the owner (the
//! original is also called with other bases) it falls back to the raw guest store, so behaviour is unchanged.
pub use crate::memory::GuestMemory;
pub use crate::state_wire::game_state;
pub use crate::views::{generated_views, values};
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-state/src/character.rs"]
pub mod character;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-state/src/inventory.rs"]
pub mod inventory;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-state/src/missions.rs"]
pub mod missions;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-state/src/missions_owned.rs"]
pub mod missions_owned;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-state/src/modes.rs"]
pub mod modes;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-state/src/object_pool.rs"]
pub mod object_pool;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-state/src/pad_audio.rs"]
pub mod pad_audio;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-state/src/player.rs"]
pub mod player;
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-state/src/save_text.rs"]
pub mod save_text;

use crate::state_wire::mutate_state;

/// 16-bit store (the shared `GuestMemory` leaf trait has 8 and 32 bit stores); the same two byte stores as `c2r::rt::write16`.
fn write16<M: GuestMemory>(m: &mut M, address: u32, value: u16) {
    m.write8(address, value as u8);
    m.write8(address.wrapping_add(1), (value >> 8) as u8);
}
use save_text::{
    SLOT_COUNT, SLOT_STRIDE, SLOT_TABLE, SaveSlots, TextBank, VOICE_BANK_BASE, VOICE_BANK_COUNT,
    VOICE_BANK_STRIDE,
};

/// `[slot] = record`: the head write of 0x69D90 and 0x699D0 (`slot` is `0x7BEDC0 + index * 0x1F54`).
pub fn set_slot_head<M: GuestMemory>(m: &mut M, slot: u32, record: u32) {
    let offset = slot.wrapping_sub(SLOT_TABLE);
    if slot >= SLOT_TABLE
        && offset % SLOT_STRIDE == 0
        && ((offset / SLOT_STRIDE) as usize) < SLOT_COUNT
    {
        let mut slots = SaveSlots::load(m);
        slots.set_head(
            m,
            (offset / SLOT_STRIDE) as usize,
            mutate_state(SLOT_TABLE, record),
        );
    } else {
        m.write32(slot, record);
    }
}

/// A declared dword of one voice-bank descriptor. Padding and addresses outside
/// the two descriptors have no owner and retain the raw guest-store path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VoiceBankField {
    Samples,
    Count,
    LabelBase,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct VoiceBankCell {
    index: usize,
    field: VoiceBankField,
}

impl VoiceBankCell {
    fn decode(address: u32) -> Option<Self> {
        let offset = address.checked_sub(VOICE_BANK_BASE)?;
        let index = (offset / VOICE_BANK_STRIDE) as usize;
        if index >= VOICE_BANK_COUNT {
            return None;
        }
        let field = match offset % VOICE_BANK_STRIDE {
            0 => VoiceBankField::Samples,
            4 => VoiceBankField::Count,
            8 => VoiceBankField::LabelBase,
            _ => return None,
        };
        Some(Self { index, field })
    }
}

/// Dword store into a voice bank descriptor (`bank + 0` samples, `+4` count, `+8` label base), 0x46030.
pub fn set_bank_field<M: GuestMemory>(m: &mut M, address: u32, value: u32) {
    if let Some(cell) = VoiceBankCell::decode(address) {
        let value = mutate_state(VOICE_BANK_BASE, value);
        let mut text = TextBank::load(m);
        let mut bank = text.banks[cell.index];
        match cell.field {
            VoiceBankField::Samples => bank.samples = values::GuestPtr::new(value),
            VoiceBankField::Count => bank.count = value as _,
            VoiceBankField::LabelBase => bank.label_base = value as _,
        }
        text.set_bank(m, cell.index, bank);
    } else {
        m.write32(address, value);
    }
}

/// The two memo cells of 0x6A850 (`[0x4D2CE8] = mode`, then `[0x4D2CE4] = arg`).
pub fn set_mode_memo<M: GuestMemory>(m: &mut M, arg: u32, mode: u32) {
    let mut setup = modes::ModeSetup::load_pointer(m);
    setup.set_memo(m, mutate_state(modes::MEMO_ARG, arg), mode);
}

/// `[0x6F32F0] = record`: the cached mode record pointer of 0x6A850.
pub fn set_mode_current<M: GuestMemory>(m: &mut M, base: u32) {
    let mut setup = modes::ModeSetup::load_pointer(m);
    setup.set_pointer(m, mutate_state(modes::CURRENT_CELL, base));
}

/// `[base + offset] = value`, a dword store into the mode record M (0x69F30 relocates its pointer fields, `base` is
/// the record, `offset` below 0x154). Other offsets stay raw.
pub fn set_record_word<M: GuestMemory>(m: &mut M, base: u32, offset: u32, value: u32) {
    if offset % 4 == 0 && offset < modes::RECORD_SIZE {
        let mut record = modes::ModeRecord::load(m, base);
        record.set_word(
            m,
            (offset / 4) as usize,
            mutate_state(modes::CURRENT_CELL, value),
        );
    } else {
        m.write32(base.wrapping_add(offset), value);
    }
}

/// `ModeRecord::clear` of the BSS record at `base` (the rep stosd at 0x23F8DA of 0x23F390).
pub fn clear_mode_record<M: GuestMemory>(m: &mut M, base: u32) {
    modes::ModeRecord::load(m, base).clear(m);
}

/// Float store into the stats table at `address` (0x238510 and 0x2385B0 accumulate into
/// `[0x75CCC0] + (player * 0x273 + stat) * 4`): the cell is decoded back to (player, stat) and written through
/// `PlayerSet::set_stat`. An address outside the table (or with a null `[0x75CCC0]`) stays a raw store.
pub fn set_stat_at<M: GuestMemory>(m: &mut M, address: u32, bits: u32) {
    // T2138: retire modes 1 and 2 go through the persistent owner (mode 0: the path below, unchanged)
    if crate::player_wire::store(m, address, bits, 4, player::STATS_CELL) {
        return;
    }
    let base = m.read32(player::STATS_CELL);
    let offset = address.wrapping_sub(base);
    if base != 0 && offset % 4 == 0 && offset < player::STATS_SIZE {
        let cell = offset / 4;
        let mut set = player::PlayerSet::load(m);
        let bits = mutate_state(player::STATS_CELL, bits);
        if set.set_stat(
            m,
            cell / player::STATS_PER_PLAYER,
            cell % player::STATS_PER_PLAYER,
            bits,
        ) {
            return;
        }
    }
    m.write32(address, bits);
}

/// Sub-dword and dword stores into the player record (T2118): the writers of docs/data/t2057-write-attribution.csv
/// (family player) call these for their stores at attributed record offsets. An address inside
/// `[0x7B0C7C] .. +0x2000` goes through `PlayerSet::set_record_word` (merging a byte or half into the guest word first),
/// anything else (another base, no record, a half crossing a dword) stays a raw store.
fn player_store<M: GuestMemory>(m: &mut M, address: u32, value: u32, width: u32) {
    // T2138: retire modes 1 and 2 go through the persistent owner (mode 0: the path below, unchanged)
    if crate::player_wire::store(m, address, value, width, player::RECORD_CELL) {
        return;
    }
    let base = m.read32(player::RECORD_CELL);
    let offset = address.wrapping_sub(base);
    if base != 0 && offset < player::RECORD_SIZE && (offset % 4) + width <= 4 {
        let shift = 8 * (offset % 4);
        let mask = if width == 4 {
            u32::MAX
        } else {
            ((1u32 << (8 * width)) - 1) << shift
        };
        let aligned = offset & !3;
        let word = (m.read32(base.wrapping_add(aligned)) & !mask) | ((value << shift) & mask);
        let word = mutate_state(player::RECORD_CELL, word);
        if player::PlayerSet::load_cells(m).set_record_word(m, aligned, word) {
            return;
        }
    }
    match width {
        1 => m.write8(address, value as u8),
        2 => write16(m, address, value as u16),
        _ => m.write32(address, value),
    }
}
pub fn set_player_word<M: GuestMemory>(m: &mut M, address: u32, value: u32) {
    player_store(m, address, value, 4);
}
pub fn set_player_half<M: GuestMemory>(m: &mut M, address: u32, value: u16) {
    player_store(m, address, u32::from(value), 2);
}
pub fn set_player_byte<M: GuestMemory>(m: &mut M, address: u32, value: u8) {
    if value == 1 && crate::weapon_hands_hook::enabled() {
        for slot in 0..4 {
            let Some(ext) = inventory::Inventory::slot_base(m, slot) else {
                continue;
            };
            let start = ext.wrapping_add(inventory::EXT_EQUIPPED);
            let end = start.wrapping_add(inventory::ENTRY_COUNT as u32);
            if address >= start && address < end {
                let entry = (address - start) as usize;
                if crate::weapon_hands_hook::mark_equipped_once(m, ext, entry) {
                    return;
                }
            }
        }
    }
    player_store(m, address, u32::from(value), 1);
}

/// Merges a `width`-byte store at `address` into the guest dword that contains it. None when the store crosses a dword.
fn merged_word<M: GuestMemory>(m: &M, address: u32, value: u32, width: u32) -> Option<u32> {
    let shift = 8 * (address % 4);
    if (address % 4) + width > 4 {
        return None;
    }
    if width == 4 {
        return Some(value);
    }
    let mask = ((1u32 << (8 * width)) - 1) << shift;
    Some((m.read32(address & !3) & !mask) | ((value << shift) & mask))
}

fn raw_store<M: GuestMemory>(m: &mut M, address: u32, value: u32, width: u32) {
    match width {
        1 => m.write8(address, value as u8),
        2 => write16(m, address, value as u16),
        _ => m.write32(address, value),
    }
}

/// Store into the object pool block or an owned pool record (T2122): `ObjectPool::classify` decodes the address
/// (header word, table cell, pool scalar, member arena dword, record dword), `write_through` stores the dword. Anything else (update list nodes, records past the used count, a store crossing a dword) stays a raw store. The mutation
/// key is the block (`HEADER`) for header, tables and scalars and the records base cell for records.
fn pool_store<M: GuestMemory>(m: &mut M, address: u32, value: u32, width: u32) {
    if let Some(slot) = object_pool::ObjectPool::classify(m, address) {
        if let Some(word) = merged_word(m, address, value, width) {
            let key = match slot {
                object_pool::PoolSlot::Record { .. } => {
                    object_pool::SCALARS[object_pool::S_RECORDS_BASE].0
                }
                _ => object_pool::HEADER,
            };
            object_pool::ObjectPool::write_through(m, slot, mutate_state(key, word));
            return;
        }
    }
    raw_store(m, address, value, width);
}
pub fn set_pool_word<M: GuestMemory>(m: &mut M, address: u32, value: u32) {
    pool_store(m, address, value, 4);
}
pub fn set_pool_half<M: GuestMemory>(m: &mut M, address: u32, value: u16) {
    pool_store(m, address, u32::from(value), 2);
}
pub fn set_pool_byte<M: GuestMemory>(m: &mut M, address: u32, value: u8) {
    pool_store(m, address, u32::from(value), 1);
}

/// Store into a live character body (T2122): `CharacterSet::classify` finds the table slot whose body contains the
/// address, `write_through` stores the dword. Mutation key: the character table. Other addresses stay raw.
fn char_store<M: GuestMemory>(m: &mut M, address: u32, value: u32, width: u32) {
    if let Some(at) = character::CharacterSet::classify(m, address) {
        if let Some(word) = merged_word(m, address, value, width) {
            character::CharacterSet::write_through(m, at, mutate_state(character::TABLE, word));
            return;
        }
    }
    raw_store(m, address, value, width);
}
pub fn set_char_word<M: GuestMemory>(m: &mut M, address: u32, value: u32) {
    char_store(m, address, value, 4);
}
pub fn set_char_half<M: GuestMemory>(m: &mut M, address: u32, value: u16) {
    char_store(m, address, u32::from(value), 2);
}
pub fn set_char_byte<M: GuestMemory>(m: &mut M, address: u32, value: u8) {
    char_store(m, address, u32::from(value), 1);
}

/// Store from an object-family writer (T2122): the target is either a pool record or a character body, whichever
/// contains the address; otherwise raw. Used by the object-family lane roots that write both.
pub fn set_obj_word<M: GuestMemory>(m: &mut M, address: u32, value: u32) {
    obj_store(m, address, value, 4);
}
pub fn set_obj_half<M: GuestMemory>(m: &mut M, address: u32, value: u16) {
    obj_store(m, address, u32::from(value), 2);
}
pub fn set_obj_byte<M: GuestMemory>(m: &mut M, address: u32, value: u8) {
    obj_store(m, address, u32::from(value), 1);
}
/// T2129: a 16-byte row store (`movups` / `movaps`) of an object-family writer, four dword stores low lane first
/// (same order and result as `xmm_store`), each decoded like `set_obj_word`.
pub fn set_obj_xmm<M: GuestMemory>(m: &mut M, address: u32, lanes: [u32; 4]) {
    for (index, lane) in lanes.into_iter().enumerate() {
        obj_store(m, address.wrapping_add(4 * index as u32), lane, 4);
    }
}
fn obj_store<M: GuestMemory>(m: &mut M, address: u32, value: u32, width: u32) {
    if object_pool::ObjectPool::classify(m, address).is_some() {
        pool_store(m, address, value, width);
    } else {
        char_store(m, address, value, width);
    }
}

/// T2147: the last indirect call through a collision-hit record (`call [record+0x170]` at 0xC4A3F, root 0xC4700): site, record,
/// target, the record's type dword (+0x20) and flag dword (+0x28). Relaxed stores, read by `tsfp_bridge_fail` when the callee is not callable.
static HIT_NOTE: [core::sync::atomic::AtomicU32; 5] =
    [const { core::sync::atomic::AtomicU32::new(0) }; 5];
pub fn note_hit_callback<M: GuestMemory>(m: &M, site: u32, record: u32, target: u32) {
    use core::sync::atomic::Ordering::Relaxed;
    let readable = record >= 0x1_0000 && record < 0x8000_0000;
    let (kind, flags) = if readable {
        (
            m.read32(record.wrapping_add(0x20)),
            m.read32(record.wrapping_add(0x28)),
        )
    } else {
        (0, 0)
    };
    for (slot, value) in HIT_NOTE.iter().zip([site, record, target, kind, flags]) {
        slot.store(value, Relaxed);
    }
    census_hit_callback(kind, target, flags);
}
/// First sighting of a (type, target) pair is written to stderr as `t2147 hit-callback type=.. target=.. flags=..` (at most 64 lines per run), the
/// census of the callees this call site really reaches.
static HIT_SEEN: [core::sync::atomic::AtomicU64; 64] =
    [const { core::sync::atomic::AtomicU64::new(0) }; 64];
fn census_hit_callback(kind: u32, target: u32, flags: u32) {
    use core::sync::atomic::Ordering::Relaxed;
    let key = (u64::from(kind) << 32) | u64::from(target) | (1 << 63);
    for slot in &HIT_SEEN {
        let seen = slot.load(Relaxed);
        if seen == key {
            return;
        }
        if seen == 0 && slot.compare_exchange(0, key, Relaxed, Relaxed).is_ok() {
            let mut line = [0u8; 96];
            let mut at = 0;
            for part in [
                &b"t2147 hit-callback type="[..],
                &hex(kind),
                &b" target="[..],
                &hex(target),
                &b" flags="[..],
                &hex(flags),
                &b"\n"[..],
            ] {
                for &byte in part {
                    if at < line.len() {
                        line[at] = byte;
                        at += 1;
                    }
                }
            }
            unsafe { write(2, line.as_ptr(), at) };
            return;
        }
    }
}
fn hex(value: u32) -> [u8; 8] {
    let mut out = [b'0'; 8];
    for (index, digit) in out.iter_mut().enumerate() {
        *digit = b"0123456789abcdef"[((value >> (28 - 4 * index)) & 15) as usize];
    }
    out
}
unsafe extern "C" {
    fn write(fd: i32, buffer: *const u8, count: usize) -> isize;
}
/// Copies the five dwords of the last [`note_hit_callback`] to `out` (site, record, target, type, flags).
///
/// # Safety
/// `out` has room for 5 u32.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tsfp_rust_last_hit_callback(out: *mut u32) {
    for (index, slot) in HIT_NOTE.iter().enumerate() {
        unsafe { *out.add(index) = slot.load(core::sync::atomic::Ordering::Relaxed) };
    }
}

/// Dword store into the PadState input scalars `0x6B9888..0x6B98B8` (T2114: 0x426A0 gate exchange, the two state
/// stores of 0x42640) or the AudioRamp words `0x6B98B8..0x6B9934` (0x450E0 count clears, 0x45160 table release). The
/// address picks the owner; anything else stays a raw store.
pub fn set_pad_audio_word<M: GuestMemory>(m: &mut M, address: u32, value: u32) {
    let input_end = pad_audio::INPUT_BASE + 4 * pad_audio::INPUT_WORDS as u32;
    if address >= pad_audio::INPUT_BASE && address < input_end && address % 4 == 0 {
        let value = mutate_state(pad_audio::INPUT_BASE, value);
        let mut pad = pad_audio::PadState::load(m);
        pad.set_input(m, ((address - pad_audio::INPUT_BASE) / 4) as usize, value);
    } else if address >= pad_audio::AUDIO_BASE
        && address < pad_audio::AUDIO_BASE + 4 * pad_audio::AUDIO_WORDS as u32
        && address % 4 == 0
    {
        let value = mutate_state(pad_audio::AUDIO_BASE, value);
        let mut audio = pad_audio::AudioRamp::load(m);
        audio.set(m, address, value);
    } else {
        m.write32(address, value);
    }
}

/// Dword store into a PickupCells cell (T2114: the zero blocks of 0x9CE00 and 0x9A20C, the allocations and counts of
/// 0x9A140, the pointer clear 0x9A260): pointer cells, the three counts and the eight aux scalars go through the
/// owner, every other address stays a raw store. The mutation key is per class (pointers, counts, aux scalars).
pub fn set_mission_cell<M: GuestMemory>(m: &mut M, address: u32, value: u32) {
    if crate::missions_wire::store(m, address, value, 4) {
        return;
    }
    use missions::{
        AUX_CELLS, LINK_COUNT, MAIN_COUNT, MAIN_PTR, PTR_CELLS, PickupCells, T34_COUNT,
    };
    if let Some(index) = PTR_CELLS.iter().position(|&cell| cell == address) {
        let value = mutate_state(MAIN_PTR, value);
        PickupCells::load(m).set_ptr(m, index, value);
    } else if address == MAIN_COUNT || address == LINK_COUNT || address == T34_COUNT {
        let value = mutate_state(MAIN_COUNT, value);
        let mut cells = PickupCells::load(m);
        match address {
            MAIN_COUNT => cells.set_main_count(m, value),
            LINK_COUNT => cells.set_link_count(m, value),
            _ => cells.set_t34_count(m, value),
        }
    } else if let Some(index) = AUX_CELLS.iter().position(|&cell| cell == address) {
        let value = mutate_state(AUX_CELLS[0], value);
        PickupCells::load(m).set_aux(m, index, value);
    } else {
        m.write32(address, value);
    }
}

/// Byte store `[address] = 1` of 0x9A630: inside the 32 byte visited table it is `Visited::mark`, otherwise raw (the
/// original writes past the cleared extent for ids past 0x20, see docs/t-s3-missions.md).
pub fn mark_visited<M: GuestMemory>(m: &mut M, address: u32) {
    if crate::missions_wire::store(m, address, 1, 1) {
        return;
    }
    let id = address.wrapping_sub(missions::VISITED) as usize;
    if id < missions::VISITED_SIZE {
        let value = mutate_state(missions::VISITED, 1) as u8;
        missions::Visited::load(m).mark(m, id, value);
    } else {
        m.write8(address, 1);
    }
}

/// Group word store of 0x1B6B40 (`entry + 0x38`, the unrolled loop computes the address): the owner decodes the entry
/// index of the weapon table and writes through `WeaponEntries::set_group`; any other address stays raw.
pub fn set_entry_group<M: GuestMemory>(m: &mut M, address: u32, value: u32) {
    use inventory::{ENTRY_COUNT, ENTRY_STRIDE, ENTRY_TABLE, GROUP_WORD, WeaponEntries};
    let offset = address.wrapping_sub(ENTRY_TABLE);
    let index = (offset / ENTRY_STRIDE) as usize;
    if address >= ENTRY_TABLE
        && index < ENTRY_COUNT
        && offset % ENTRY_STRIDE == 4 * GROUP_WORD as u32
    {
        let value = mutate_state(ENTRY_TABLE, value);
        WeaponEntries::load(m).set_group(m, index, value);
    } else {
        m.write32(address, value);
    }
}

/// Highest group number cell `0x75A024` (0x1B6B40, twice).
pub fn set_max_group<M: GuestMemory>(m: &mut M, value: u32) {
    let value = mutate_state(inventory::MAX_GROUP_CELL, value);
    inventory::WeaponEntries::load(m).set_max_group(m, value);
}

/// `owned[entry] = 1` byte of 0x1BFAA0 in the extension block at `base` (the store is `base + entry + 0x54C`).
pub fn set_owned<M: GuestMemory>(m: &mut M, base: u32, entry: u32) {
    if (entry as usize) < inventory::ENTRY_COUNT {
        let value = mutate_state(inventory::EXT_POINTER, 1) as u8;
        inventory::Inventory::load(m, base).set_owned(m, entry as usize, value);
    } else {
        m.write8(
            base.wrapping_add(entry).wrapping_add(inventory::EXT_OWNED),
            1,
        );
    }
}

/// Throwable weapon cell `base + 0xA0` of 0x1BFAA0.
pub fn set_throwable<M: GuestMemory>(m: &mut M, base: u32, value: u32) {
    let value = mutate_state(inventory::EXT_POINTER, value);
    inventory::Inventory::load(m, base).set_throwable(m, value);
}

/// `equipped[entry] = 1` byte of 0x1BD820 in the extension block at `base` (the store is `base + entry + 0x592`). T2137.
pub fn set_equipped<M: GuestMemory>(m: &mut M, base: u32, entry: u32) {
    if (entry as usize) < inventory::ENTRY_COUNT {
        let value = mutate_state(inventory::EXT_POINTER, 1) as u8;
        inventory::Inventory::load(m, base).set_equipped(m, entry as usize, value);
    } else {
        m.write8(
            base.wrapping_add(entry)
                .wrapping_add(inventory::EXT_EQUIPPED),
            1,
        );
    }
}

/// Reserve ammo word `base + 2 * index + 0x61E` of 0x1BC650 (T2137): an ammo type below 44 goes through
/// `Inventory::set_ammo`, any other index stays a raw half store.
pub fn set_ammo_word<M: GuestMemory>(m: &mut M, base: u32, index: u32, value: u16) {
    if (index as usize) < inventory::AMMO_TYPES {
        let value = mutate_state(inventory::EXT_POINTER, value as u32) as u16;
        inventory::Inventory::load(m, base).set_ammo(m, index as usize, value);
    } else {
        let at = base
            .wrapping_add(index.wrapping_mul(2))
            .wrapping_add(inventory::EXT_AMMO);
        m.write8(at, value as u8);
        m.write8(at.wrapping_add(1), (value >> 8) as u8);
    }
}

/// Dword store of 0x438B0 into the pad table `0x6B93C0..` (T2137): the owner decodes it, any other address stays raw.
pub fn set_pad_table_word<M: GuestMemory>(m: &mut M, address: u32, value: u32) {
    let in_table = address >= pad_audio::PAD_BASE
        && address < pad_audio::PAD_BASE + 4 * pad_audio::PAD_WORDS as u32
        && address % 4 == 0;
    if in_table {
        let value = mutate_state(pad_audio::PAD_BASE, value);
        pad_audio::PadState::load(m).set_table(m, address, value);
    } else {
        m.write32(address, value);
    }
}

/// One `rep stosd` fill of 0x9A140 (T2137): `count` dwords of `value` from `start`. Dwords inside the allocated main
/// (`[0x715F2C]`), link (`[0x715F30]`) or third (`[0x715F34]`) table go through `PickupRow` / `LinkRow::set_word`,
/// every other dword stays a raw store. The mutation key is the table pointer cell (main rows use `0x715F38`).
pub fn fill_mission_dwords<M: GuestMemory>(m: &mut M, start: u32, count: u32, value: u32) {
    use missions::{
        LINK_CAPACITY, LINK_PTR, LINK_SIZE, LinkRow, MAIN_CAPACITY, PickupCells, PickupRow,
        ROW_SIZE, T34_CAPACITY_LEVEL100, T34_PTR, T38_PTR,
    };
    let cells = PickupCells::load(m);
    // (base, byte extent, row size, link row, mutation key)
    let tables = [
        (
            cells.ptrs[0],
            MAIN_CAPACITY * ROW_SIZE,
            ROW_SIZE,
            false,
            T38_PTR,
        ),
        (
            cells.ptrs[1],
            LINK_CAPACITY * LINK_SIZE,
            LINK_SIZE,
            true,
            LINK_PTR,
        ),
        (
            cells.ptrs[2],
            T34_CAPACITY_LEVEL100 * ROW_SIZE,
            ROW_SIZE,
            false,
            T34_PTR,
        ),
    ];
    for i in 0..count {
        let address = start.wrapping_add(i.wrapping_mul(4));
        if crate::missions_wire::store(m, address, value, 4) {
            continue;
        }
        let hit = tables.iter().find(|(base, extent, _, _, _)| {
            *base != 0 && address >= *base && address - *base < *extent
        });
        match hit {
            Some(&(base, _, size, link, key)) => {
                let offset = address - base;
                let row = base + offset / size * size;
                let word = (offset % size / 4) as usize;
                let value = mutate_state(key, value);
                if link {
                    LinkRow::load(m, row).set_word(m, word, value);
                } else {
                    PickupRow::load(m, row).set_word(m, word, value);
                }
            }
            None => m.write32(address, value),
        }
    }
}

/// Store of any width into the mission tables or cells (T2137 pass 3: `0x9A5B0 0x9A7A0 0x9AF90`, computed heap
/// addresses). `bytes` is 1, 2 or 4. A dword in a PickupCells cell goes through `set_mission_cell`, a store inside the
/// allocated main, link or third table is merged into the guest word and routed through `PickupRow` / `LinkRow::set_word`,
/// everything else (the visited table, other bases) stays a raw store of the original width.
pub fn set_mission_table<M: GuestMemory>(m: &mut M, address: u32, value: u32, bytes: u32) {
    if crate::missions_wire::store(m, address, value, bytes) {
        return;
    }
    use missions::{
        LINK_CAPACITY, LINK_SIZE, LinkRow, MAIN_CAPACITY, PickupCells, PickupRow, ROW_SIZE,
        T34_CAPACITY_LEVEL100,
    };
    let raw = |m: &mut M| match bytes {
        1 => m.write8(address, value as u8),
        2 => {
            m.write8(address, value as u8);
            m.write8(address.wrapping_add(1), (value >> 8) as u8);
        }
        _ => m.write32(address, value),
    };
    if bytes == 4
        && (missions::PTR_CELLS.contains(&address)
            || [
                missions::MAIN_COUNT,
                missions::LINK_COUNT,
                missions::T34_COUNT,
            ]
            .contains(&address)
            || missions::AUX_CELLS.contains(&address))
    {
        set_mission_cell(m, address, value);
        return;
    }
    let cells = PickupCells::load(m);
    let tables = [
        (
            cells.ptrs[0],
            MAIN_CAPACITY * ROW_SIZE,
            ROW_SIZE,
            false,
            missions::T38_PTR,
        ),
        (
            cells.ptrs[1],
            LINK_CAPACITY * LINK_SIZE,
            LINK_SIZE,
            true,
            missions::LINK_PTR,
        ),
        (
            cells.ptrs[2],
            T34_CAPACITY_LEVEL100 * ROW_SIZE,
            ROW_SIZE,
            false,
            missions::T34_PTR,
        ),
    ];
    let hit = tables.iter().find(|(base, extent, _, _, _)| {
        *base != 0 && *base % 4 == 0 && address >= *base && address - *base < *extent
    });
    let aligned = address & !3;
    match hit {
        // the store must stay inside one dword of the row to be merged
        Some(&(base, _, size, link, key)) if (address & 3) + bytes <= 4 => {
            let offset = aligned - base;
            let row = base.wrapping_add(offset / size * size);
            let word = (offset % size / 4) as usize;
            let old = m.read32(aligned);
            let shift = (address & 3) * 8;
            let mask = if bytes == 4 {
                u32::MAX
            } else {
                ((1u32 << (bytes * 8)) - 1) << shift
            };
            let merged = (old & !mask) | ((value << shift) & mask);
            let merged = mutate_state(key, merged);
            // the row handle is built without reading the row (set_word writes one dword), so a store near the end of the
            // mapped memory never faults on the neighbouring words
            if link {
                LinkRow {
                    base: row,
                    words: [0; 5],
                }
                .set_word(m, word, merged);
            } else {
                PickupRow {
                    base: row,
                    words: [0; 8],
                }
                .set_word(m, word, merged);
            }
        }
        _ => raw(m),
    }
}

pub fn set_mission_word<M: GuestMemory>(m: &mut M, address: u32, value: u32) {
    set_mission_table(m, address, value, 4);
}
pub fn set_mission_half<M: GuestMemory>(m: &mut M, address: u32, value: u16) {
    set_mission_table(m, address, value as u32, 2);
}
pub fn set_mission_byte<M: GuestMemory>(m: &mut M, address: u32, value: u8) {
    set_mission_table(m, address, value as u32, 1);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Ram(std::collections::BTreeMap<u32, u8>);
    impl GuestMemory for Ram {
        fn read8(&self, a: u32) -> u8 {
            *self.0.get(&a).unwrap_or(&0)
        }
        fn write8(&mut self, a: u32, v: u8) {
            self.0.insert(a, v);
        }
    }

    /// T2147: the note keeps the last hit-callback call (site, record, target, type, flags) for the bridge failure report, and a wild record
    /// pointer is never dereferenced.
    #[test]
    fn hit_callback_note_reports_the_last_call_and_skips_wild_records() {
        let mut m = Ram(Default::default());
        m.write32(0x40_0000 + 0x20, 8);
        m.write32(0x40_0000 + 0x28, 0x40);
        note_hit_callback(&m, 0xc4a3f, 0x40_0000, 0x1234);
        let mut out = [0u32; 5];
        unsafe { tsfp_rust_last_hit_callback(out.as_mut_ptr()) };
        assert_eq!(out, [0xc4a3f, 0x40_0000, 0x1234, 8, 0x40]);
        for wild in [0u32, 4, 0xffff, 0x8000_0000, 0xdead_beef] {
            note_hit_callback(&m, 0xc4a3f, wild, 0x99);
            unsafe { tsfp_rust_last_hit_callback(out.as_mut_ptr()) };
            assert_eq!(out, [0xc4a3f, wild, 0x99, 0, 0], "record {wild:#x}");
        }
    }

    #[test]
    fn voice_bank_cell_decode_rejects_padding_and_outside_addresses() {
        for index in 0..VOICE_BANK_COUNT {
            let base = VOICE_BANK_BASE + index as u32 * VOICE_BANK_STRIDE;
            for (offset, field) in [
                (0, VoiceBankField::Samples),
                (4, VoiceBankField::Count),
                (8, VoiceBankField::LabelBase),
            ] {
                assert_eq!(
                    VoiceBankCell::decode(base + offset),
                    Some(VoiceBankCell { index, field })
                );
            }
            for offset in [1, 2, 3, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15] {
                assert_eq!(VoiceBankCell::decode(base + offset), None);
            }
        }
        for address in [
            VOICE_BANK_BASE - 1,
            VOICE_BANK_BASE + VOICE_BANK_COUNT as u32 * VOICE_BANK_STRIDE,
            u32::MAX,
        ] {
            assert_eq!(VoiceBankCell::decode(address), None);
        }
    }

    #[test]
    fn owners_write_only_their_cells() {
        let mut m = Ram(Default::default());
        set_slot_head(&mut m, 0x7bedc0 + 3 * 0x1f54, 0x7bc9c0);
        assert_eq!(m.read32(0x7bedc0 + 3 * 0x1f54), 0x7bc9c0);
        set_slot_head(&mut m, 0x7bedc0 + 4, 9);
        assert_eq!(m.read32(0x7bedc4), 9, "non head address stays a raw store");
        set_bank_field(&mut m, 0x7e7210 + 8, 0x1dbb);
        set_bank_field(&mut m, 0x7e7210 + 4, 5);
        set_bank_field(&mut m, 0x7e7210, 0x4000);
        assert_eq!(
            (m.read32(0x7e7218), m.read32(0x7e7214), m.read32(0x7e7210)),
            (0x1dbb, 5, 0x4000)
        );
        set_bank_field(&mut m, 0x7e7200 + 0xc, 7);
        assert_eq!(m.read32(0x7e720c), 7, "pad dword stays raw");
        set_mode_memo(&mut m, 2, 0xa);
        assert_eq!((m.read32(0x4d2ce4), m.read32(0x4d2ce8)), (2, 0xa));
        set_mode_current(&mut m, 0x508390);
        assert_eq!(m.read32(0x6f32f0), 0x508390);
        set_record_word(&mut m, 0x9000, 0xc, 0x1234);
        assert_eq!(m.read32(0x900c), 0x1234);
        set_record_word(&mut m, 0x9000, 0x154, 0x77);
        assert_eq!(m.read32(0x9154), 0x77, "past the record: raw");
        clear_mode_record(&mut m, 0x9000);
        assert_eq!(m.read32(0x900c), 0);
        m.write32(0x75ccc0, 0x100000);
        set_stat_at(&mut m, 0x100000 + (0x273 + 5) * 4, 0x3f80_0000);
        assert_eq!(m.read32(0x100000 + (0x273 + 5) * 4), 0x3f80_0000);
        set_stat_at(&mut m, 0x2000, 1);
        assert_eq!(m.read32(0x2000), 1, "outside the table: raw");
        set_mission_cell(&mut m, 0x715f2c, 0x9000);
        set_mission_cell(&mut m, 0x7b6664, 7);
        set_mission_cell(&mut m, 0x7b6674, 8);
        set_mission_cell(&mut m, 0x7b6800, 9);
        mark_visited(&mut m, 0x7b67a3);
        mark_visited(&mut m, 0x7b67c0);
        assert_eq!(
            (
                m.read32(0x715f2c),
                m.read32(0x7b6664),
                m.read32(0x7b6674),
                m.read32(0x7b6800)
            ),
            (0x9000, 7, 8, 9)
        );
        assert_eq!((m.read8(0x7b67a3), m.read8(0x7b67c0)), (1, 1));
        set_entry_group(&mut m, 0x4fa128 + 3 * 0x3c + 0x38, 2);
        set_entry_group(&mut m, 0x4fa128 + 3 * 0x3c + 0x34, 6);
        set_max_group(&mut m, 5);
        set_owned(&mut m, 0x9000, 4);
        set_owned(&mut m, 0x9000, 90);
        set_throwable(&mut m, 0x9000, 0x44);
        assert_eq!(
            (
                m.read32(0x4fa128 + 3 * 0x3c + 0x38),
                m.read32(0x4fa128 + 3 * 0x3c + 0x34),
                m.read32(0x75a024)
            ),
            (2, 6, 5)
        );
        assert_eq!(
            (
                m.read8(0x9000 + 0x54c + 4),
                m.read8(0x9000 + 0x54c + 90),
                m.read32(0x9000 + 0xa0)
            ),
            (1, 1, 0x44)
        );
        m.write32(0x7b0c7c, 0x200000);
        set_player_word(&mut m, 0x200000 + 0x14f8, 0x21);
        set_player_byte(&mut m, 0x200000 + 0x14f9, 0x7);
        set_player_half(&mut m, 0x200000 + 0x1ffe, 0xabcd);
        set_player_half(&mut m, 0x200000 + 0x1ff3, 0x1111);
        set_player_word(&mut m, 0x300000, 5);
        assert_eq!(m.read32(0x200000 + 0x14f8), 0x721);
        assert_eq!(
            m.read32(0x200000 + 0x1ffc),
            0xabcd_0000,
            "half at +2 of a dword"
        );
        assert_eq!(m.read8(0x200000 + 0x1ff3), 0x11, "crossing half stays raw");
        assert_eq!(m.read32(0x300000), 5, "outside the record: raw");
        m.write32(0x72f5e8, 0x400000);
        m.write32(0x7b4fac, 3);
        set_pool_word(&mut m, 0x400000 + 0x2e8 + 0x30, 0x1111);
        set_pool_byte(&mut m, 0x400000 + 2 * 0x2e8 + 0x19, 0x7f);
        set_pool_half(&mut m, 0x400000 + 3 * 0x2e8 + 0x4, 0x9);
        set_pool_word(&mut m, 0x7b54d8, 5);
        set_pool_word(&mut m, 0x7b54e0 + 0x360 * 2 + 8, 6);
        set_pool_half(&mut m, 0x7b54e3, 0x1234);
        assert_eq!(m.read32(0x400000 + 0x2e8 + 0x30), 0x1111);
        assert_eq!(m.read32(0x400000 + 2 * 0x2e8 + 0x18), 0x7f00);
        assert_eq!(
            m.read32(0x400000 + 3 * 0x2e8 + 0x4),
            9,
            "past the used count: raw store, same value"
        );
        assert_eq!(
            (m.read32(0x7b54d8), m.read32(0x7b54e0 + 0x360 * 2 + 8)),
            (5, 6)
        );
        m.write32(object_pool::SCALARS[object_pool::S_ARENA_BASE].0, 0x410000);
        m.write32(object_pool::SCALARS[object_pool::S_ARENA_USED].0, 2);
        set_pool_word(&mut m, 0x410004, 0x12345678);
        set_pool_word(&mut m, 0x410008, 0x87654321);
        assert_eq!(m.read32(0x410004), 0x12345678, "used arena dword is owned");
        assert_eq!(
            m.read32(0x410008),
            0x87654321,
            "past arena prefix remains raw"
        );
        assert_eq!(m.read8(0x7b54e3), 0x34, "half crossing the dword: raw");
        m.write32(0x7a3580 + 4, 0x500000);
        set_char_word(&mut m, 0x500000 + 0x10, 0xaa);
        set_char_byte(&mut m, 0x500000 + 0x15, 0x5);
        set_char_half(&mut m, 0x500000 + 0xe9e, 0x2222);
        set_char_word(&mut m, 0x600000, 4);
        assert_eq!(m.read32(0x500000 + 0x10), 0xaa);
        assert_eq!(m.read32(0x500000 + 0x14), 0x500);
        assert_eq!(m.read32(0x500000 + 0xe9c), 0x2222_0000);
        assert_eq!(m.read32(0x600000), 4, "no body: raw");
        set_obj_word(&mut m, 0x400000 + 0x10, 7);
        set_obj_word(&mut m, 0x500000 + 0x20, 8);
        assert_eq!((m.read32(0x400010), m.read32(0x500020)), (7, 8));
        set_pad_audio_word(&mut m, 0x6b9888, 3);
        set_pad_audio_word(&mut m, 0x6b98a4, 4);
        set_pad_audio_word(&mut m, 0x6b990c, 9);
        set_pad_audio_word(&mut m, 0x6b9934, 5);
        assert_eq!(
            (
                m.read32(0x6b9888),
                m.read32(0x6b98a4),
                m.read32(0x6b990c),
                m.read32(0x6b9934)
            ),
            (3, 4, 9, 5)
        );
    }
    #[test]
    fn pool_leaf_setters_feed_generation_qualified_room_order() {
        use super::object_pool::{
            CAPS, COUNTS, MEMBERS, ObjectId, ObjectPool, S_ARENA_BASE, S_ARENA_USED, S_CAPACITY,
            S_RECORDS_BASE, S_USED_RECORDS, cell_addr, rec_addr,
        };

        let mut m = Ram(Default::default());
        set_pool_word(&mut m, object_pool::SCALARS[S_RECORDS_BASE].0, 0x400000);
        set_pool_word(&mut m, object_pool::SCALARS[S_CAPACITY].0, 2);
        set_pool_word(&mut m, object_pool::SCALARS[S_USED_RECORDS].0, 2);
        set_pool_word(&mut m, object_pool::SCALARS[S_ARENA_BASE].0, 0x410000);
        set_pool_word(&mut m, object_pool::SCALARS[S_ARENA_USED].0, 2);
        set_pool_word(&mut m, cell_addr(COUNTS, 3), 2);
        set_pool_word(&mut m, cell_addr(CAPS, 3), 2);
        set_pool_word(&mut m, cell_addr(MEMBERS, 3), 0x410000);
        set_pool_word(&mut m, 0x410000, rec_addr(0x400000, 1, 0));
        set_pool_word(&mut m, 0x410004, rec_addr(0x400000, 0, 0));

        let pool = ObjectPool::load(&m);
        let rooms = pool
            .ordered_room_objects(|record| match record {
                0 => Some(ObjectId {
                    slot: 8,
                    generation: 2,
                }),
                1 => Some(ObjectId {
                    slot: 4,
                    generation: 7,
                }),
                _ => None,
            })
            .unwrap();
        assert_eq!(rooms.len(), 1);
        assert_eq!(rooms[0].room, 3);
        assert_eq!(
            rooms[0].objects[0].object,
            ObjectId {
                slot: 4,
                generation: 7
            }
        );
        assert_eq!(
            rooms[0].objects[1].object,
            ObjectId {
                slot: 8,
                generation: 2
            }
        );
    }

    #[test]
    fn t2137_inventory_missions_pad_setters() {
        let mut m = Ram(Default::default());
        let base = 0x700000;
        set_equipped(&mut m, base, 3);
        assert_eq!(m.read8(base + 3 + 0x592), 1);
        set_equipped(&mut m, base, 90);
        assert_eq!(
            m.read8(base + 90 + 0x592),
            1,
            "entry past the table: raw byte store"
        );
        set_ammo_word(&mut m, base, 5, 0x1234);
        assert_eq!(m.read8(base + 10 + 0x61e), 0x34);
        assert_eq!(m.read8(base + 11 + 0x61e), 0x12);
        set_ammo_word(&mut m, base, 60, 0xbeef);
        assert_eq!(
            m.read8(base + 120 + 0x61f),
            0xbe,
            "type past the table: raw half store"
        );
        set_pad_table_word(&mut m, 0x6b9404, 7);
        set_pad_table_word(&mut m, 0x6b9000, 8);
        assert_eq!((m.read32(0x6b9404), m.read32(0x6b9000)), (7, 8));
        m.write32(0x715f2c, 0x800000);
        m.write32(0x715f30, 0x900000);
        fill_mission_dwords(&mut m, 0x800000, 10, 0x55);
        fill_mission_dwords(&mut m, 0x900000, 7, 0x66);
        fill_mission_dwords(&mut m, 0xa00000, 2, 0x77);
        assert_eq!(m.read32(0x800000 + 36), 0x55);
        assert_eq!(m.read32(0x800000 + 40), 0, "past the fill");
        assert_eq!(m.read32(0x900000 + 24), 0x66);
        assert_eq!(m.read32(0xa00004), 0x77, "outside every table: raw");
    }

    #[test]
    fn t2137_pass3_mission_table_stores() {
        let mut m = Ram(Default::default());
        m.write32(0x715f2c, 0x800000);
        m.write32(0x715f30, 0x900000);
        set_mission_word(&mut m, 0x800024, 0xdead_beef);
        assert_eq!(m.read32(0x800024), 0xdead_beef);
        set_mission_half(&mut m, 0x80002a, 0x1234); // row 1 word 2, high half
        assert_eq!(m.read32(0x800028), 0x1234_0000);
        set_mission_byte(&mut m, 0x900015, 0x7f); // link row 1 word 0, byte 1
        assert_eq!(m.read32(0x900014), 0x7f00);
        set_mission_word(&mut m, 0xa00000, 9); // outside every table: raw
        assert_eq!(m.read32(0xa00000), 9);
        set_mission_half(&mut m, 0xa00005, 0xabcd); // raw half, little endian
        assert_eq!((m.read8(0xa00005), m.read8(0xa00006)), (0xcd, 0xab));
        set_mission_word(&mut m, 0x7b6664, 5); // count cell goes through PickupCells
        assert_eq!(m.read32(0x7b6664), 5);
        m.write32(0x715f34, 0x802); // unaligned base: raw store, no panic
        set_mission_word(&mut m, 0x803, 1);
        assert_eq!(m.read32(0x803), 1);
    }
}
