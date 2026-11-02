//! T1856: the small object and handle engine services behind `StripServices` (`0x1F48A0`) and the prelude.
//!
//! Every function reads and writes guest memory through [`GuestMemory`] and mirrors the original machine code
//! (MEASURED against the Unicorn oracle, see `docs/t-story-event-objects-t1856.md`). Names are INFERRED, addresses,
//! offsets and control flow MEASURED. The destroy path `0xBABA0` calls about twenty engine routines of other
//! subsystems (audio, particles, physics); it is ported as control flow and the callees stay behind
//! [`DestroyEffects`], keyed by their original address.

use super::strip_update::{GuestMemory, Service, StripServices};

/// Pool of world objects: base pointer `[0x72F5E8]`, stride `0x2E8`, handle at `+0x2C` (slot in the top 10 bits).
pub const GLOBAL_OBJECT_POOL: u32 = 0x72_F5E8;
/// Head of the sorted key list used by `0x23FF20` (node: handle `+0`, key `+4`, next `+8`).
pub const GLOBAL_KEY_LIST: u32 = 0x75_D318;
/// Index of the first player slot `[0x790950]` and number of slots `[0x7A2958]` scanned by `0x15BA10`.
pub const GLOBAL_FIRST_SLOT: u32 = 0x79_0950;
pub const GLOBAL_SLOT_COUNT: u32 = 0x7A_2958;
/// Base of the 0xEA0 byte slot records `[0x7356D8]`.
pub const GLOBAL_SLOT_BASE: u32 = 0x73_56D8;
/// Mode byte `[0x7DE455]`: value 0xA reads the previous frame copy of the slot header.
pub const GLOBAL_MODE_A: u32 = 0x7D_E455;
pub const GLOBAL_MODE_B: u32 = 0x7D_E456;
pub const GLOBAL_FLAGS_C: u32 = 0x7D_E458;
pub const GLOBAL_FLAGS_D: u32 = 0x7D_E45C;
pub const GLOBAL_VALUE_E: u32 = 0x7D_E470;
/// Table of 0x3C byte controller descriptors at `0x4FA134` and the controller array `[0x4F9BAC]` (0x29C stride).
pub const GLOBAL_CONTROLLER_ARRAY: u32 = 0x4F_9BAC;
/// Controller registry used by `0x163730`: `[0x74C37C]` end index, pointers at `0x7A3580 + 4 * index`.
pub const GLOBAL_REGISTRY_END: u32 = 0x74_C37C;
pub const REGISTRY_TABLE: u32 = 0x7A_3580;
/// Table of 0xE0 byte mode rows at `0x4EA3F8` and tick rate `[0x4E8CEC]` (int, 60).
pub const MODE_ROWS: u32 = 0x4E_A3F8;
pub const GLOBAL_TICK_RATE: u32 = 0x4E_8CEC;
/// Array `[0x7B0C48]` of 0x1584 byte player records.
pub const GLOBAL_PLAYER_ARRAY: u32 = 0x7B_0C48;

/// `[0x4D1994]` = 60.0: animation frame scale of `0x585E0`.
pub const STATE_SCALE: f32 = 60.0;
/// `[0x478394]` = 1/60 (rounded to f32), `[0x47D580]` = 9.0, `[0x47D5D4]` = 900.0, `[0x475CD4]` = 0.5.
pub const SECONDS_SCALE: f32 = f32::from_bits(0x3C88_8889);
pub const NEAR_SQUARED: f32 = 9.0;
pub const FAR_SQUARED: f32 = 900.0;
pub const GAIN_STEP: f32 = 0.5;

fn read_u16(memory: &dyn GuestMemory, address: u32) -> u16 {
    u16::from(memory.read_u8(address)) | u16::from(memory.read_u8(address.wrapping_add(1))) << 8
}

fn read_i16(memory: &dyn GuestMemory, address: u32) -> i32 {
    i32::from(read_u16(memory, address) as i16)
}

fn write_u16(memory: &mut dyn GuestMemory, address: u32, value: u16) {
    memory.write_u8(address, value as u8);
    memory.write_u8(address.wrapping_add(1), (value >> 8) as u8);
}

fn read_f32(memory: &dyn GuestMemory, address: u32) -> f32 {
    f32::from_bits(memory.read_u32(address))
}

/// `cvttss2si`: truncate toward zero, `0x80000000` for NaN or out of range.
fn cvtt(value: f32) -> i32 {
    if value.is_nan() || value >= 2_147_483_648.0 || value < -2_147_483_648.0 {
        i32::MIN
    } else {
        value as i32
    }
}

// ---------------------------------------------------------------------------------------------------------
// Lookups
// ---------------------------------------------------------------------------------------------------------

/// `0x15BA10(tag)` (cdecl, eax): scans the slot records `[0x790950] .. [0x790950] + [0x7A2958]` (stride 0xEA0 from
/// `[0x7356D8]`, signed index compare). A slot is live when its qword at `+0x18` (`+0x10` in mode `[0x7DE455] == 0xA`)
/// is non zero, `[[slot + 0x18 + 0xBD8] + 8] == tag` and bit 8 of `[slot + 0x24]` is clear. Returns the object pointer
/// stored at the slot head (`slot + 0`) or 0. MEASURED.
pub fn find_object_a(memory: &dyn GuestMemory, tag: u32) -> u32 {
    let count = memory.read_u32(GLOBAL_SLOT_COUNT);
    let first = memory.read_u32(GLOBAL_FIRST_SLOT);
    let end = first.wrapping_add(count) as i32;
    let mut index = first as i32;
    if index >= end {
        return 0;
    }
    let base = memory.read_u32(GLOBAL_SLOT_BASE);
    let previous_mode = memory.read_u8(GLOBAL_MODE_A) == 0xA;
    let mut slot = index
        .wrapping_mul(0xEA0)
        .wrapping_add(base as i32)
        .wrapping_add(0x18) as u32;
    loop {
        let (low, high) = if previous_mode {
            (
                memory.read_u32(slot.wrapping_sub(8)),
                memory.read_u32(slot.wrapping_sub(4)),
            )
        } else {
            (memory.read_u32(slot), memory.read_u32(slot.wrapping_add(4)))
        };
        if low | high != 0 {
            let record = memory.read_u32(slot.wrapping_add(0xBD8));
            if memory.read_u32(record.wrapping_add(8)) == tag
                && memory.read_u8(slot.wrapping_add(0xC)) & 8 == 0
            {
                return memory.read_u32(slot.wrapping_sub(0x18));
            }
        }
        index = index.wrapping_add(1);
        slot = slot.wrapping_add(0xEA0);
        if index >= end {
            return 0;
        }
    }
}

/// `0x163730(tag)` (cdecl, eax): scans registry indices `[0x790950] .. [0x74C37C]` (signed); entry pointers at
/// `0x7A3580 + 4 * index`. An entry matches when bit 8 of `[entry + 0x24]` is clear, `[entry + 0xBF0]` is non zero and
/// `[[entry + 0xBF0] + 8] == tag`; returns `[entry]` or 0. MEASURED.
pub fn find_object_b(memory: &dyn GuestMemory, tag: u32) -> u32 {
    let mut index = memory.read_u32(GLOBAL_FIRST_SLOT) as i32;
    let end = memory.read_u32(GLOBAL_REGISTRY_END) as i32;
    while index < end {
        let entry = memory.read_u32(REGISTRY_TABLE.wrapping_add((index as u32).wrapping_mul(4)));
        if memory.read_u8(entry.wrapping_add(0x24)) & 8 == 0 {
            let record = memory.read_u32(entry.wrapping_add(0xBF0));
            if record != 0 && memory.read_u32(record.wrapping_add(8)) == tag {
                return memory.read_u32(entry);
            }
        }
        index = index.wrapping_add(1);
    }
    0
}

/// `0x23FF20(key)` (cdecl, eax): walks the key list `[0x75D318]` (sorted ascending by the signed key at `+4`, stops
/// at a larger key). On a key match: handle `[node]` zero gives 0; the object is `[0x72F5E8] + (handle >> 22) *
/// 0x2E8` and is returned only when its `+0x2C` equals the handle, a stale handle is cleared in the node
/// (`[node] = 0`) and 0 returned. MEASURED.
pub fn lookup_by_key(memory: &mut dyn GuestMemory, key: u32) -> u32 {
    let mut node = memory.read_u32(GLOBAL_KEY_LIST);
    while node != 0 {
        let node_key = memory.read_u32(node.wrapping_add(4)) as i32;
        if node_key == key as i32 {
            let handle = memory.read_u32(node);
            if handle == 0 {
                return 0;
            }
            let object = (handle >> 22)
                .wrapping_mul(0x2E8)
                .wrapping_add(memory.read_u32(GLOBAL_OBJECT_POOL));
            if memory.read_u32(object.wrapping_add(0x2C)) == handle {
                return object;
            }
            memory.write_u32(node, 0);
            return 0;
        }
        if node_key > key as i32 {
            return 0;
        }
        node = memory.read_u32(node.wrapping_add(8));
    }
    0
}

/// The anchor semantics of [`super::strip_update::StripObjects::anchor_value`] over guest memory: the float at
/// `+0x64` of the object found by [`lookup_by_key`].
pub fn anchor_value(memory: &mut dyn GuestMemory, key: u32) -> Option<f32> {
    let object = lookup_by_key(memory, key);
    (object != 0).then(|| read_f32(memory, object.wrapping_add(0x64)))
}

/// `0xBDA30(index)` (cdecl, eax): address of the 36 byte record `0x4DB9A8 + 36 * index` (a player table row,
/// INFERRED). MEASURED.
pub fn owner_record(index: u32) -> u32 {
    index.wrapping_mul(36).wrapping_add(0x4D_B9A8)
}

// ---------------------------------------------------------------------------------------------------------
// Class record, state and flag test
// ---------------------------------------------------------------------------------------------------------

/// `0x59CF0(object)` (cdecl, eax): `p = [object + 4]`; the class record is `p + 0x12C` when the float at `p + 0x198`
/// is greater than 0.0 (NaN is not), else `p + 0xD4`. MEASURED.
pub fn class_of(memory: &dyn GuestMemory, object: u32) -> u32 {
    let owner = memory.read_u32(object.wrapping_add(4));
    if read_f32(memory, owner.wrapping_add(0x198)) > 0.0 {
        owner.wrapping_add(0x12C)
    } else {
        owner.wrapping_add(0xD4)
    }
}

/// `0x585E0(class, out_state, out_aux)` (cdecl, void): stores `trunc(f32[class + 8] * 60.0)` and
/// `trunc(f32[class + 0xC] * 60.0)` (single precision product, `cvttss2si`) to the non null output pointers. MEASURED.
pub fn query_state(memory: &mut dyn GuestMemory, class: u32, out_state: u32, out_aux: u32) {
    if out_state != 0 {
        let value = cvtt(read_f32(memory, class.wrapping_add(8)) * STATE_SCALE);
        memory.write_u32(out_state, value as u32);
    }
    if out_aux != 0 {
        let value = cvtt(read_f32(memory, class.wrapping_add(0xC)) * STATE_SCALE);
        memory.write_u32(out_aux, value as u32);
    }
}

/// `0x56FC0(id)` (cdecl, eax): ids `0x25 <= id < 0xC03` map through the i16 table `0x7E03C0` to the dword table
/// `0x7E1BE0` (pointer to a ranges record: `i16` count at `+6`, 4 byte entries from `+8`: `u16` frame, `u16` mask);
/// anything else gives 0. MEASURED.
pub fn ranges_record(memory: &dyn GuestMemory, id: u32) -> u32 {
    let id = id as i32;
    if !(0x25..0xC03).contains(&id) {
        return 0;
    }
    let index = read_i16(
        memory,
        0x7E_03C0u32.wrapping_add((id as u32).wrapping_mul(2)),
    );
    memory.read_u32(0x7E_1BE0u32.wrapping_add((index as u32).wrapping_mul(4)))
}

/// `0x61570(object, mask)` (cdecl, eax 0 or 1): flag test of the object's class: the class id `[class]` selects a
/// ranges record ([`ranges_record`]); `state` and `aux` come from [`query_state`] (written to `frame - 4` and
/// `frame - 8`, `frame` = esp at entry). The result is 1 when an entry whose mask word `& mask` is non zero has
/// frame 0 and `state == 0`, or a frame in `(aux, state]` (`aux <= state`), or a frame above `aux` or at most
/// `state` (`aux > state`), signed compares; else 0. MEASURED.
pub fn flag_test(memory: &mut dyn GuestMemory, object: u32, mask: u32, frame: u32) -> u32 {
    let class = class_of(memory, object);
    let id = memory.read_u32(class);
    let record = ranges_record(memory, id);
    if record == 0 {
        return 0;
    }
    // Locals of the original: aux at frame - 8, state at frame - 4 (`frame` is esp at entry).
    query_state(memory, class, frame.wrapping_sub(4), frame.wrapping_sub(8));
    let state = memory.read_u32(frame.wrapping_sub(4)) as i32;
    let aux = memory.read_u32(frame.wrapping_sub(8)) as i32;
    let count = read_i16(memory, record.wrapping_add(6));
    let mut entry = record.wrapping_add(8);
    let mut index = 0;
    while index < count {
        let masked = read_u16(memory, entry.wrapping_add(2)) as u32 & mask;
        if masked as u16 != 0 {
            let at = i32::from(read_u16(memory, entry));
            if at == 0 {
                if state == 0 {
                    return 1;
                }
            } else if aux > state {
                if at > aux || at <= state {
                    return 1;
                }
            } else if at > aux && at <= state {
                return 1;
            }
        }
        index += 1;
        entry = entry.wrapping_add(4);
    }
    0
}

// ---------------------------------------------------------------------------------------------------------
// Poll
// ---------------------------------------------------------------------------------------------------------

/// Key `PCRT` (0x54524350) of the object `0x24D290(1)` repositions.
pub const KEY_PCRT: u32 = 0x5452_4350;

/// `0x24D290(mode)` (cdecl, eax): scene reset switch. Mode 0 and 4 set `[0x78B678] = 1` / `0`, then
/// `[0x78B600] = 0xA`, `[0x78B664] = 0x10`, `[0x78B680] = 3`; mode 1 looks up `PCRT`, returns 0 when missing, else copies
/// the four floats at `0x51B0A0` to `object + 0x64..0x70` and sets `[0x78B600] = 1`, `[0x78B664] = 2`. Modes 0, 1 and
/// 4 then clear `[0x78B668]`, `[0x78B66C]`, `[0x78B670]`; every path but the missing object returns 1. MEASURED.
pub fn poll(memory: &mut dyn GuestMemory, mode: u32) -> u32 {
    match mode {
        0 | 4 => {
            memory.write_u32(0x78_B678, u32::from(mode == 0));
            memory.write_u32(0x78_B600, 0xA);
            memory.write_u32(0x78_B664, 0x10);
            memory.write_u32(0x78_B680, 3);
        }
        1 => {
            let object = lookup_by_key(memory, KEY_PCRT);
            if object == 0 {
                return 0;
            }
            for k in 0..4 {
                let value = memory.read_u32(0x51_B0A0 + 4 * k);
                memory.write_u32(object.wrapping_add(0x64 + 4 * k), value);
            }
            memory.write_u32(0x78_B600, 1);
            memory.write_u32(0x78_B664, 2);
        }
        _ => return 1,
    }
    memory.write_u32(0x78_B670, 0);
    memory.write_u32(0x78_B66C, 0);
    memory.write_u32(0x78_B668, 0);
    1
}

// ---------------------------------------------------------------------------------------------------------
// Controller counters
// ---------------------------------------------------------------------------------------------------------

/// `0x1B67E0(controller, index)` (cdecl, eax): sums the i16 counters `+0x38 + 2 * index` of up to two controller
/// halves (`controller + 0xA4` and `controller + 0x1D4`). The descriptor row is `0x3C * [controller + 0x98]` of the
/// table at `0x4FA134` (two controller ids, `-1` none); the id selects a 0x29C byte record of `[0x4F9BAC]` whose
/// dword `+0x14` must equal `index`, else its dword `+0x1C` must be non zero and equal `index`. MEASURED.
pub fn controller_sum(memory: &dyn GuestMemory, controller: u32, index: u32) -> i32 {
    let array = memory.read_u32(GLOBAL_CONTROLLER_ARRAY);
    let row = memory
        .read_u32(controller.wrapping_add(0x98))
        .wrapping_mul(0x3C);
    let mut total = 0i32;
    let first = memory.read_u32(row.wrapping_add(0x4F_A134)) as i32;
    let half = controller.wrapping_add(0xA4);
    if first != -1 && half != 0 {
        let record = (first as u32).wrapping_mul(0x29C).wrapping_add(array);
        let low = memory.read_u32(record.wrapping_add(0x14));
        if low == index {
            total = read_i16(
                memory,
                half.wrapping_add(low.wrapping_mul(2)).wrapping_add(0x38),
            );
        } else {
            let high = memory.read_u32(record.wrapping_add(0x1C));
            if high != 0 && high == index {
                total = read_i16(
                    memory,
                    half.wrapping_add(high.wrapping_mul(2)).wrapping_add(0x38),
                );
            }
        }
    }
    let second = memory.read_u32(row.wrapping_add(0x4F_A138)) as i32;
    let half = controller.wrapping_add(0x1D4);
    if second == -1 || half == 0 {
        return total;
    }
    let record = (second as u32).wrapping_mul(0x29C).wrapping_add(array);
    let low = memory.read_u32(record.wrapping_add(0x14));
    if low == index {
        return total.wrapping_add(read_i16(
            memory,
            half.wrapping_add(low.wrapping_mul(2)).wrapping_add(0x38),
        ));
    }
    let high = memory.read_u32(record.wrapping_add(0x1C));
    if high != 0 && high == index {
        total = total.wrapping_add(read_i16(
            memory,
            half.wrapping_add(high.wrapping_mul(2)).wrapping_add(0x38),
        ));
    }
    total
}

/// `0x1BA220(controller, slot, delta)` (cdecl, eax = applied change): adds `delta` to the i16 counter at
/// `controller + 0x61E + 2 * slot` (truncated to 16 bits), then clamps so `counter + controller_sum(controller, slot)`
/// does not exceed the cap of the 0xE0 byte row `slot` at `0x4EA3F8`: the dword at `+0`, or when bit 0 of the byte at
/// `+8` is set `trunc(f32(dword) * f32([0x4E8CEC]) * (1/60))`. The result is the new minus the old counter. The
/// clamp stores `cap - sum` truncated to 16 bits. MEASURED.
pub fn adjust_counter(memory: &mut dyn GuestMemory, controller: u32, slot: u32, delta: i32) -> i32 {
    let sum = controller_sum(memory, controller, slot);
    let address = controller
        .wrapping_add(slot.wrapping_mul(2))
        .wrapping_add(0x61E);
    let old = read_i16(memory, address);
    let row = slot.wrapping_mul(0xE0);
    let raw = memory.read_u32(row.wrapping_add(MODE_ROWS));
    let cap = if memory.read_u8(row.wrapping_add(0x4E_A400)) & 1 != 0 {
        let rate = memory.read_u32(GLOBAL_TICK_RATE) as i32 as f32;
        cvtt((raw as i32 as f32) * rate * SECONDS_SCALE)
    } else {
        raw as i32
    };
    let updated = (old.wrapping_add(delta)) as u16;
    write_u16(memory, address, updated);
    let mut counter = i32::from(updated as i16);
    if counter.wrapping_add(sum) > cap {
        counter = cap.wrapping_sub(sum);
        write_u16(memory, address, counter as u16);
    }
    read_i16(memory, address).wrapping_sub(old)
}

// ---------------------------------------------------------------------------------------------------------
// 0xD1EC0, 0xD1F10, 0xC7880
// ---------------------------------------------------------------------------------------------------------

/// Player flags that block the push: `0x1121E0`.
pub const PUSH_BLOCK_MASK: u32 = 0x11_21E0;

/// `0xC7880(object, x, y, z)` (cdecl, void): stores the three dwords at `object + 0x398 / 0x39C / 0x3A0`
/// (an external velocity or push vector, INFERRED) unless `object` is null. MEASURED.
pub fn set_push_vector(memory: &mut dyn GuestMemory, object: u32, vector: [u32; 3]) {
    if object != 0 {
        for (k, word) in vector.iter().enumerate() {
            memory.write_u32(object.wrapping_add(0x398 + 4 * k as u32), *word);
        }
    }
}

fn push_target(memory: &dyn GuestMemory, record: u32) -> Option<u32> {
    let object = memory.read_u32(record.wrapping_add(8));
    (object != 0
        && memory.read_u32(record.wrapping_add(0x114)) & PUSH_BLOCK_MASK == 0
        && memory.read_u8(object.wrapping_add(0x21C)) & 4 != 0)
        .then_some(object)
}

/// `0xD1EC0(record, x, y, z)` (cdecl, void; tail call of `0xC7880`): for one player record (`+8` object, `+0x114`
/// flags, object byte `+0x21C` bit 2) applies [`set_push_vector`] when no flag of [`PUSH_BLOCK_MASK`] is set. MEASURED.
pub fn push_player(memory: &mut dyn GuestMemory, record: u32, vector: [u32; 3]) {
    if let Some(object) = push_target(memory, record) {
        set_push_vector(memory, object, vector);
    }
}

/// `0xD1F10(x, y, z)` (cdecl, void): [`push_player`] for every player record `i < [0x790950]` of the array at
/// `[0x7B0C48]` (stride 0x1584); the count and the array pointer are reloaded after each push. MEASURED.
pub fn push_all_players(memory: &mut dyn GuestMemory, vector: [u32; 3]) {
    if (memory.read_u32(GLOBAL_FIRST_SLOT) as i32) <= 0 {
        return;
    }
    let mut index = 0i32;
    loop {
        let base = memory.read_u32(GLOBAL_PLAYER_ARRAY);
        let record = base.wrapping_add((index as u32).wrapping_mul(0x1584));
        push_player(memory, record, vector);
        index = index.wrapping_add(1);
        if index >= memory.read_u32(GLOBAL_FIRST_SLOT) as i32 {
            return;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// 0xBD090 / 0xBABA0: destroy
// ---------------------------------------------------------------------------------------------------------

/// Engine routines called by [`destroy_core`] (original addresses). Arguments are the cdecl stack words in call
/// order, the result is eax. These belong to other subsystems and are not ported here.
pub mod callee {
    pub const FINISH_SLOT: u32 = 0xD5480;
    pub const OWNER_TEST: u32 = 0x35_CC10;
    pub const IS_NETWORK: u32 = 0x35_33F0;
    pub const NOTIFY_CTX: u32 = 0x17_BF70;
    pub const NOTIFY_BLOCK: u32 = 0x22_DD10;
    pub const KILL_TEST: u32 = 0x27_8040;
    pub const STRING_LOOKUP: u32 = 0x3_D7B0;
    pub const FORMAT: u32 = 0x3C_85D4;
    pub const COUNT_CHANNELS: u32 = 0x1C_DAE0;
    pub const START_CHANNEL: u32 = 0x12_D350;
    pub const SET_FLAGS: u32 = 0x6_7450;
    pub const DEATH_HOOK: u32 = 0xB_9D00;
    pub const RELEASE_LINK: u32 = 0x16_5010;
    pub const REGION_UPDATE: u32 = 0x6_44D0;
    pub const PLAY_SOUND: u32 = 0x23_8510;
    pub const ADD_SCORE: u32 = 0x12_E3B0;
    pub const NOTIFY_LINK: u32 = 0x22_C3A0;
    pub const NOTIFY_OBJECT: u32 = 0xC_F440;
    pub const UPDATE_CTX: u32 = 0x12_DFB0;
    pub const BLEND: u32 = 0x12_52C0;
    pub const FREE_OBJECT: u32 = 0x15_DB20;
    pub const FINAL_HOOK: u32 = 0x2B_24E0;
}

/// Sink for the engine routines of [`callee`]. `memory` is handed so a routine can write results.
pub trait DestroyEffects {
    fn call(&mut self, memory: &mut dyn GuestMemory, address: u32, args: &[u32]) -> u32;
}

/// Offset of the 0x104 byte local format buffer below the entry esp of `0xBABA0`.
pub const DESTROY_BUFFER_BELOW_ENTRY: u32 = 0x100;
/// Constant `0x3FCCCCCD` (1.6) pushed with every channel start.
const CHANNEL_GAIN: u32 = 0x3FCC_CCCD;
const UNIT_GAIN: u32 = 0x3F80_0000;

/// `0xBABA0(block)` (cdecl, void): the destroy handler. `block` is the 0x5C byte request built by `0xBD090`:
/// `+0` object (`s`), `+4` other object (killer), `+8` third object, `+0xC` link, `+0x18` silent flag. `entry` is
/// the value of esp at entry (return address slot), the format buffer lives at `entry - 0x100`.
///
/// Flow (MEASURED against the original with the callees recorded): when the context `ctx = [object + 0x7C]` has bit 3
/// of `[ctx + 0x24]` set only `FINISH_SLOT` runs, and only if the object's `+0x150` record has `+0x110 == 4` and bit
/// 11 of `+0xC` clear. Otherwise: owner test, network notification, kill-test branch (death sound text `0x1645` /
/// `0x1646`, channel starts, `[0x7DE45C] |= 0x10000`, or the `YAW4` hook), release, the killer branch (region
/// update, sound ids `0x5F`, `0x48`, `0x5E`, `0x5B`, `0x5D`, `0x5C`, `0x60`, `0x57`, `0x59`, `0x58`, score) or the
/// no killer branch, then `FINISH_SLOT` or `FREE_OBJECT`, and `[object + 0x258] += 1`.
pub fn destroy_core(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    block: u32,
    entry: u32,
) {
    use callee::*;
    let object = memory.read_u32(block);
    let ctx = memory.read_u32(object.wrapping_add(0x7C));
    let buffer = entry.wrapping_sub(DESTROY_BUFFER_BELOW_ENTRY);
    if memory.read_u8(ctx.wrapping_add(0x24)) & 8 != 0 {
        let record = memory.read_u32(object.wrapping_add(0x150));
        if record != 0
            && memory.read_u32(record.wrapping_add(0x110)) == 4
            && memory.read_u32(record.wrapping_add(0xC)) & 0x800 == 0
        {
            effects.call(memory, FINISH_SLOT, &[block]);
        }
        return;
    }
    let owner_arg = memory.read_u32(object.wrapping_add(0x258));
    if effects.call(memory, OWNER_TEST, &[block, owner_arg]) == 0 {
        return;
    }
    if effects.call(memory, IS_NETWORK, &[]) != 0 {
        effects.call(memory, NOTIFY_CTX, &[ctx]);
    }
    if memory.read_u8(GLOBAL_FLAGS_C) & 0x40 != 0 {
        let head = memory.read_u32(ctx);
        if head != 0 && memory.read_u32(head.wrapping_add(0x150)) == 0 {
            effects.call(memory, NOTIFY_BLOCK, &[block]);
        }
    }
    memory.write_u32(ctx.wrapping_add(0x78), 0);
    let victim = memory.read_u32(block);
    memory.write_u32(victim.wrapping_add(0x1CC), 0);
    if effects.call(memory, KILL_TEST, &[ctx]) != 0 {
        let killer = memory.read_u32(block.wrapping_add(4));
        let id = if killer != 0 && memory.read_u32(killer.wrapping_add(0x150)) != 0 {
            0x1645
        } else {
            0x1646
        };
        let value = memory.read_u32(ctx.wrapping_add(0x70));
        let text = effects.call(memory, STRING_LOOKUP, &[id]);
        effects.call(memory, FORMAT, &[buffer, text, value]);
        let mut channel = 0u32;
        let mut channels = effects.call(memory, COUNT_CHANNELS, &[]) as i32;
        if channels > 0 {
            loop {
                effects.call(memory, START_CHANNEL, &[channel, 4, buffer, CHANNEL_GAIN]);
                channel = channel.wrapping_add(1);
                channels = effects.call(memory, COUNT_CHANNELS, &[]) as i32;
                if (channel as i32) >= channels {
                    break;
                }
            }
        }
        let flags = memory.read_u32(GLOBAL_FLAGS_D) | 0x1_0000;
        memory.write_u32(GLOBAL_FLAGS_D, flags);
        effects.call(memory, SET_FLAGS, &[0]);
    } else {
        let link = memory.read_u32(ctx.wrapping_add(0xBF0));
        if memory.read_u32(link.wrapping_add(0x28)) == 0x3457_4159 {
            effects.call(memory, DEATH_HOOK, &[ctx]);
        }
    }
    if memory.read_u8(GLOBAL_MODE_A) != 0xA {
        effects.call(memory, RELEASE_LINK, &[block]);
    }
    let other = memory.read_u32(block.wrapping_add(4));
    let has_killer = other != 0
        && memory.read_u32(other.wrapping_add(0x20)) == 8
        && other != memory.read_u32(block);
    if has_killer {
        let other_ctx = memory.read_u32(other.wrapping_add(0x7C));
        killer_path(memory, effects, block, ctx, other_ctx);
    } else {
        no_killer_path(memory, effects, block, ctx, other);
    }
    let head = memory.read_u32(block);
    let record = memory.read_u32(head.wrapping_add(0x150));
    if record != 0 {
        effects.call(memory, FINISH_SLOT, &[block]);
    } else {
        effects.call(memory, FREE_OBJECT, &[block]);
    }
    if memory.read_u8(GLOBAL_MODE_B) == 0xA {
        let head_ctx = memory.read_u32(ctx);
        effects.call(memory, FINAL_HOOK, &[head_ctx]);
    }
    let object = memory.read_u32(block);
    let count = memory.read_u32(object.wrapping_add(0x258));
    memory.write_u32(object.wrapping_add(0x258), count.wrapping_add(1));
}

fn sound(memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects, id: u32, record: u32) {
    let value = memory.read_u32(record);
    effects.call(memory, callee::PLAY_SOUND, &[id, value, UNIT_GAIN]);
}

fn add_score_for(memory: &mut dyn GuestMemory, effects: &mut dyn DestroyEffects, record: u32) {
    let rate = memory.read_u32(GLOBAL_TICK_RATE);
    let amount = rate.wrapping_mul(2).wrapping_add(0x40);
    let target = memory.read_u32(record.wrapping_add(4));
    effects.call(memory, callee::ADD_SCORE, &[target, amount]);
}

/// Path with a killer object (`+4` of the block is an object of class 8 other than the victim).
fn killer_path(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    block: u32,
    ctx: u32,
    other_ctx: u32,
) {
    use callee::*;
    let region_arg = memory.read_u32(ctx.wrapping_add(8));
    effects.call(memory, REGION_UPDATE, &[other_ctx, region_arg, 1]);
    let other = memory.read_u32(block.wrapping_add(4));
    let record = memory.read_u32(other.wrapping_add(0x150));
    if record != 0 {
        let silent = memory.read_u32(block.wrapping_add(0x18));
        if silent == 0 {
            sound(memory, effects, 0x5F, record);
        }
        if memory.read_u8(GLOBAL_MODE_A) == 9 && memory.read_u8(GLOBAL_MODE_B) != 0xA {
            let other = memory.read_u32(block.wrapping_add(4));
            let record = memory.read_u32(other.wrapping_add(0x150));
            add_score_for(memory, effects, record);
        }
        let other = memory.read_u32(block.wrapping_add(4));
        if memory.read_u8(GLOBAL_FLAGS_C) & 0x40 != 0 && memory.read_u32(GLOBAL_VALUE_E) == 0x13 {
            let link = memory.read_u32(block.wrapping_add(0xC));
            effects.call(memory, NOTIFY_LINK, &[link, ctx]);
        } else {
            let record = memory.read_u32(other.wrapping_add(0x150));
            effects.call(memory, NOTIFY_OBJECT, &[record, ctx]);
        }
        let other = memory.read_u32(block.wrapping_add(4));
        let record = memory.read_u32(other.wrapping_add(0x150));
        if memory.read_u8(record.wrapping_add(0xC)) & 0x20 != 0 {
            sound(memory, effects, 0x48, record);
        }
        let other = memory.read_u32(block.wrapping_add(4));
        let fuse = read_f32(memory, other.wrapping_add(0x1CC));
        if 0.0 < fuse || fuse.is_nan() {
            let victim = memory.read_u32(block);
            let other = memory.read_u32(block.wrapping_add(4));
            let dy = read_f32(memory, victim.wrapping_add(0x68))
                - read_f32(memory, other.wrapping_add(0x68));
            let dx = read_f32(memory, victim.wrapping_add(0x64))
                - read_f32(memory, other.wrapping_add(0x64));
            let dz = read_f32(memory, victim.wrapping_add(0x6C))
                - read_f32(memory, other.wrapping_add(0x6C));
            let distance = (dz * dz + dy * dy) + dx * dx;
            if NEAR_SQUARED > distance {
                let record = memory.read_u32(other.wrapping_add(0x150));
                sound(memory, effects, 0x5B, record);
            }
            if distance > FAR_SQUARED {
                let other = memory.read_u32(block.wrapping_add(4));
                let record = memory.read_u32(other.wrapping_add(0x150));
                let id = if memory.read_u8(record.wrapping_add(0xC)) & 0x20 != 0 {
                    0x5D
                } else {
                    0x5C
                };
                sound(memory, effects, id, record);
            }
        } else {
            let other = memory.read_u32(block.wrapping_add(4));
            let record = memory.read_u32(other.wrapping_add(0x150));
            sound(memory, effects, 0x5E, record);
        }
    }
    effects.call(memory, UPDATE_CTX, &[other_ctx]);
    if memory.read_u32(block.wrapping_add(0x18)) == 0 {
        let victim = memory.read_u32(block);
        let record = memory.read_u32(victim.wrapping_add(0x150));
        if record != 0 {
            sound(memory, effects, 0x60, record);
        }
    }
    let other = memory.read_u32(block.wrapping_add(4));
    let third = memory.read_u32(block.wrapping_add(8));
    if other != 0
        && third != 0
        && memory.read_u32(third.wrapping_add(0x20)) == 0x40_0000
        && memory.read_u32(other.wrapping_add(0x20)) == 8
    {
        let record = memory.read_u32(other.wrapping_add(0x150));
        if record != 0 {
            sound(memory, effects, 0x57, record);
            if memory.read_u8(GLOBAL_FLAGS_C) & 1 != 0
                && memory.read_u32(other_ctx.wrapping_add(0x84))
                    == memory.read_u32(ctx.wrapping_add(0x84))
            {
                let other = memory.read_u32(block.wrapping_add(4));
                let record = memory.read_u32(other.wrapping_add(0x150));
                sound(memory, effects, 0x59, record);
            }
        }
        let victim = memory.read_u32(block);
        let record = memory.read_u32(victim.wrapping_add(0x150));
        if record != 0 {
            sound(memory, effects, 0x58, record);
        }
    }
    if memory.read_u8(GLOBAL_MODE_B) == 9 {
        let other = memory.read_u32(block.wrapping_add(4));
        let other_ctx_now = memory.read_u32(other.wrapping_add(0x7C));
        let gain = read_f32(memory, other_ctx_now.wrapping_add(0xA88)) + GAIN_STEP;
        memory.write_u32(other_ctx_now.wrapping_add(0xA88), gain.to_bits());
        let stored = if gain > 1.0 { 1.0 } else { gain };
        memory.write_u32(other_ctx_now.wrapping_add(0xA88), stored.to_bits());
    }
}

/// Path without a killer.
fn no_killer_path(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    block: u32,
    ctx: u32,
    other: u32,
) {
    use callee::*;
    if memory.read_u8(GLOBAL_MODE_B) == 0xC {
        effects.call(memory, BLEND, &[ctx.wrapping_add(0xC68), UNIT_GAIN, other]);
    }
    let other_now = memory.read_u32(block.wrapping_add(4));
    let region_arg = memory.read_u32(ctx.wrapping_add(8));
    if other_now != 0 && memory.read_u32(other_now.wrapping_add(0x20)) != 8 {
        effects.call(memory, REGION_UPDATE, &[0, region_arg, 1]);
    } else {
        effects.call(memory, REGION_UPDATE, &[ctx, region_arg, 1]);
    }
    if memory.read_u8(GLOBAL_MODE_A) == 9 && memory.read_u8(GLOBAL_MODE_B) != 0xA {
        let other = memory.read_u32(block.wrapping_add(4));
        if other != 0 {
            let record = memory.read_u32(other.wrapping_add(0x150));
            if record != 0 {
                add_score_for(memory, effects, record);
            }
        }
    }
}

/// `0xBD090(object)` (cdecl, void): clears `[[object + 0x7C] + 0x90]`, builds the zeroed 0x5C byte request block at
/// `entry - 0x5C` with `+0 = object`, dwords at `+0x14`, `+0x18`, `+0x1C`, `+0x24`, `+0x44` set to `-1` and runs
/// [`destroy_core`] with the entry esp of `0xBABA0` (`entry - 0x68`). `entry` is esp at entry of `0xBD090`. MEASURED.
pub fn destroy_object(
    memory: &mut dyn GuestMemory,
    effects: &mut dyn DestroyEffects,
    object: u32,
    entry: u32,
) {
    let (block, core_entry) = build_destroy_block(memory, object, entry);
    destroy_core(memory, effects, block, core_entry);
}

/// The request block part of [`destroy_object`]: returns the block address (`entry - 0x5C`) and the entry esp of
/// `0xBABA0` (`entry - 0x68`).
pub fn build_destroy_block(memory: &mut dyn GuestMemory, object: u32, entry: u32) -> (u32, u32) {
    let ctx = memory.read_u32(object.wrapping_add(0x7C));
    memory.write_u32(ctx.wrapping_add(0x90), 0);
    let block = entry.wrapping_sub(0x5C);
    for k in 0..0x17 {
        memory.write_u32(block.wrapping_add(4 * k), 0);
    }
    for offset in [0x14, 0x18, 0x24, 0x1C, 0x44] {
        memory.write_u32(block.wrapping_add(offset), 0xFFFF_FFFF);
    }
    memory.write_u32(block, object);
    (block, entry.wrapping_sub(0x68))
}

// ---------------------------------------------------------------------------------------------------------
// StripServices
// ---------------------------------------------------------------------------------------------------------

/// [`StripServices`] over guest memory for the object services of this module. Services of other subsystems
/// (`StartEffect`, `EmitterOffset`, `TransformVec`, `SpawnParticle`, `WorldMatrix`, `SpawnObject`) go to `rest`.
/// `destroy` receives the routines of [`callee`], `frame` is the esp at entry of the caller's `0xBD090` call
/// target frame for [`destroy_object`] and for `flag_test` locals (both only name stack addresses).
pub struct ObjectServices<'a> {
    pub rest: &'a mut dyn StripServices,
    pub destroy: &'a mut dyn DestroyEffects,
    /// esp at entry of the service function (below the caller's frame).
    pub frame: u32,
}

impl StripServices for ObjectServices<'_> {
    fn call(&mut self, memory: &mut dyn GuestMemory, service: Service, args: &[u32]) -> u32 {
        match service {
            Service::FindObjectA => find_object_a(memory, args[0]),
            Service::ClassOf => class_of(memory, args[0]),
            Service::QueryState => {
                query_state(memory, args[0], args[1], args[2]);
                0
            }
            Service::LookupByKey => lookup_by_key(memory, args[0]),
            Service::FlagTest => flag_test(memory, args[0], args[1], self.frame),
            Service::CurrentOwner => owner_record(args[0]),
            Service::Poll => poll(memory, args[0]),
            Service::FindObjectB => find_object_b(memory, args[0]),
            Service::SetControllerMode => {
                adjust_counter(memory, args[0], args[1], args[2] as i32) as u32
            }
            Service::Destroy => {
                destroy_object(memory, self.destroy, args[0], self.frame);
                0
            }
            other => self.rest.call(memory, other, args),
        }
    }
}
