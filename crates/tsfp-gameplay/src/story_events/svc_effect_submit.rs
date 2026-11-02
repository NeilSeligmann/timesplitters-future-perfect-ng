//! T1856: effect/audio request submit `0x45820` and the closure behind it, the nearby object disturbance
//! `0xE94C0`/`0xE9380` and the positional sound starter `0x468A0` (named "light ramp" in the prelude notes, INFERRED
//! to be a positional sound loop starter: its callees are the sound request system, not the renderer).
//!
//! Ported bit-exact from `build/default.xbe` (SHA-256 `3cfd001a...1816bc`), validated against golden vectors of the
//! original under Unicorn (`tests/data/t1856/effect_submit_*.golden`, see `docs/t-story-event-effect-submit-t1856.md`).
//! All state lives in guest memory behind [`SvcMemory`]. The sound hardware (DirectSound wrappers, stream objects,
//! the `0x274A0` voice start and friends) stays behind [`SoundHw`], which records every call.
//!
//! Ported functions: `0x27F80`, `0x45710`, `0x45190`, `0x453D0`, `0x45410`, `0x26B00`, `0x28240` (voice start, the
//! hardware calls behind the trait), `0x45820`, `0x45200`, `0x45310`, `0x45460`, `0x46BC0`, `0x46D80`, `0x468A0`,
//! `0x45390`, `0x160F50`, `0xE9380`, `0xE94C0`. Adapters for [`ImpactServices`] are in [`EffectSubmitServices`].

#![allow(clippy::too_many_arguments, clippy::neg_cmp_op_on_partial_ord)]

use super::sweep_dynamic_shape::ImpactServices;
use super::sweep_shapes::segment_closest_point;
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------------------------------------------
// Memory
// ---------------------------------------------------------------------------------------------------------------

/// Little endian guest memory with writes. Unmapped reads give zero in the tests.
pub trait SvcMemory {
    fn r8(&self, address: u32) -> u8;
    fn w8(&mut self, address: u32, value: u8);
    fn r16(&self, address: u32) -> u16 {
        u16::from(self.r8(address)) | u16::from(self.r8(address.wrapping_add(1))) << 8
    }
    fn r32(&self, address: u32) -> u32 {
        u32::from(self.r16(address)) | u32::from(self.r16(address.wrapping_add(2))) << 16
    }
    fn w16(&mut self, address: u32, value: u16) {
        self.w8(address, value as u8);
        self.w8(address.wrapping_add(1), (value >> 8) as u8);
    }
    fn w32(&mut self, address: u32, value: u32) {
        self.w16(address, value as u16);
        self.w16(address.wrapping_add(2), (value >> 16) as u16);
    }
    fn rf(&self, address: u32) -> f32 {
        f32::from_bits(self.r32(address))
    }
    fn wf(&mut self, address: u32, value: f32) {
        self.w32(address, value.to_bits());
    }
}

/// Byte map memory (tests and offline use).
#[derive(Default, Debug, Clone)]
pub struct SparseSvcMemory {
    pub bytes: BTreeMap<u32, u8>,
}

impl SparseSvcMemory {
    pub fn load_words(&mut self, address: u32, words: &[u32]) {
        for (index, word) in words.iter().enumerate() {
            self.w32(address.wrapping_add(4 * index as u32), *word);
        }
    }
}

impl SvcMemory for SparseSvcMemory {
    fn r8(&self, address: u32) -> u8 {
        self.bytes.get(&address).copied().unwrap_or(0)
    }
    fn w8(&mut self, address: u32, value: u8) {
        self.bytes.insert(address, value);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Hardware boundary
// ---------------------------------------------------------------------------------------------------------------

/// Hardware or other subsystem callees (VAs). Calling conventions in the comments (MEASURED from the callers and
/// the `ret N` of each callee).
pub mod hw {
    /// stdcall(voice, a, b): voice parameter block.
    pub const VOICE_BIND: u32 = 0x40_8C0D;
    /// cdecl(5): voice parameter calculation (`0x28180`), returns a dword stored at voice `+0x2C`.
    pub const VOICE_PARAMS: u32 = 0x2_8180;
    /// stdcall(voice, value).
    pub const VOICE_SET: u32 = 0x40_7A64;
    /// stdcall(voice, float bits, flag).
    pub const VOICE_3D_MODE: u32 = 0x40_8532;
    /// stdcall(voice, table pointer, 4, 1): 3D distance curve.
    pub const VOICE_3D_CURVE: u32 = 0x40_858B;
    /// register `eax` = pointer to the position triple, stack (voice).
    pub const VOICE_POSITION: u32 = 0x2_6EC0;
    /// cdecl(volume or token, stat).
    pub const VOICE_VOLUME: u32 = 0x2_8070;
    /// cdecl(token, value): start (or retrigger) the voice of a request (`0x454B0` is a jump to it).
    pub const VOICE_START: u32 = 0x2_74A0;
    /// cdecl(sound table value, 0, pointer, 0x2FA): opens a stream and returns its handle.
    pub const STREAM_OPEN: u32 = 0x6_2E20;
    /// thiscall(ecx = stream voice object) with nine stack dwords, callee pops them (`ret 0x24`).
    pub const STREAM_SETUP: u32 = 0x2_99C0;
    /// thiscall(ecx), one stack dword (`ret 4`).
    pub const STREAM_SET_PRIORITY: u32 = 0x2_9E50;
    pub const STREAM_SET_POSITIONAL: u32 = 0x2_9E40;
    pub const STREAM_SET_POSITION: u32 = 0x2_9DD0;
    /// thiscall(ecx), cdecl style.
    pub const STREAM_COMMIT: u32 = 0x2_9E60;
    pub const STREAM_PLAY: u32 = 0x2_9EA0;
    /// thiscall(ecx): returns al = the object is busy.
    pub const STREAM_BUSY: u32 = 0x2_9CC0;
    /// cdecl(handle): cancel a started voice (`0x273E0`).
    pub const VOICE_CANCEL: u32 = 0x2_73E0;
    /// cdecl(handle, position pointer or 0, volume float): `0x27CC0` (`0x45600` is a jump to it).
    pub const VOICE_MOVE: u32 = 0x2_7CC0;
    /// cdecl(handle, float): `0x28110` (`0x454C0` is a jump to it).
    pub const VOICE_RANGE: u32 = 0x2_8110;
}

/// One recorded hardware call. `reg` is the register argument (`eax` for [`hw::VOICE_POSITION`], `ecx` for the
/// thiscall stream methods, else 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HwCall {
    pub callee: u32,
    pub reg: u32,
    pub args: Vec<u32>,
}

/// Sound hardware and neighbouring subsystems. `call` returns `eax` (the stream busy flag in `al`).
pub trait SoundHw {
    fn call(&mut self, call: HwCall) -> u32;
    /// `0x1CDAE0` (jump to `0x3567C0`): number of local players.
    fn player_count(&mut self) -> i32;
    /// `0x356800(index)`: player slot of local player `index`.
    fn player_slot(&mut self, index: i32) -> i32;
}

// ---------------------------------------------------------------------------------------------------------------
// Guest addresses (MEASURED)
// ---------------------------------------------------------------------------------------------------------------

/// `[0x4CA9B4]` = base of the sound table (20 byte entries: `+4` bank, `+5` volume, `+8` flags, `+0xC` known).
pub const SOUND_TABLE_PTR: u32 = 0x4C_A9B4;
/// 16 bit counts per bank, `[0x4CA9B8 + 2 * bank]`.
pub const BANK_COUNTS: u32 = 0x4C_A9B8;
/// Request token counter.
pub const TOKEN_COUNTER: u32 = 0x4C_A9B0;
/// Dedup frame stamp, compared with `[0x7497B0]`.
pub const DEDUP_STAMP: u32 = 0x4C_AA00;
pub const FRAME_STAMP: u32 = 0x74_97B0;
/// Dedup list (100 ids) and its count.
pub const DEDUP_LIST: u32 = 0x6B_9938;
pub const DEDUP_COUNT: u32 = 0x6D_5EAC;
/// Deferred request ring: 25 slots of 0x40 bytes, cursor `[0x6D5EA8]`.
pub const QUEUE_BASE: u32 = 0x6D_5868;
pub const QUEUE_CURSOR: u32 = 0x6D_5EA8;
pub const QUEUE_SLOTS: i32 = 25;
/// Per sound extra table (20 byte entries): `+0` handle a (0x565B48), `+4` stream (0x565B4C), `+8` 0x565B50,
/// `+0xC` 0x565B54.
pub const SOUND_EXTRA: u32 = 0x56_5B48;
/// Voice free stack: count `[0x565B40]`, entries from `0x5657BC`; voice records of 0x38 bytes from `0x5819B8`.
pub const VOICE_FREE_COUNT: u32 = 0x56_5B40;
pub const VOICE_FREE_LIST: u32 = 0x56_57BC;
pub const VOICE_RECORDS: u32 = 0x58_19B8;
pub const VOICE_STRIDE: u32 = 0x38;
/// Stream voice objects (0x14094 bytes each) from `0x5836D0` up to `0x64BC98`; token array at `0x581990`.
pub const STREAM_BASE: u32 = 0x58_36D0;
pub const STREAM_END: u32 = 0x64_BC98;
pub const STREAM_STRIDE: u32 = 0x1_4094;
pub const STREAM_TOKENS: u32 = 0x58_1990;
/// Positional voice list head `[0x5836C0]` (sentinel `0x583690`), plain list head `[0x5659F4]` (sentinel `0x5659C4`).
pub const POSITIONAL_HEAD: u32 = 0x58_36C0;
pub const POSITIONAL_SENTINEL: u32 = 0x58_3690;
pub const PLAIN_HEAD: u32 = 0x56_59F4;
pub const PLAIN_SENTINEL: u32 = 0x56_59C4;
/// Hardware voice handle pools: used flags and handles. `0x26B00(eax != 0)` (positional) takes from the first pool,
/// `eax == 0` (plain) from the second (MEASURED, the test of `eax` in the original jumps to the second pool).
pub const POSITIONAL_USED: u32 = 0x56_5A00;
pub const POSITIONAL_HANDLES: u32 = 0x58_35F0;
pub const PLAIN_USED: u32 = 0x56_5AA0;
pub const PLAIN_HANDLES: u32 = 0x58_18E8;
/// Retrigger table of the starter: 4 entries of 12 bytes (`+0` sound id or -1, `+4` handle, `+8` word).
pub const HELD_TABLE: u32 = 0x6D_6028;
pub const HELD_ENTRIES: u32 = 4;
/// Highest sound id accepted by the submit.
pub const MAX_SOUND_ID: i32 = 0x1648;

const ONE: f32 = 1.0;
/// `[0x475D1C]` (0.01), `[0x47621C]` (1/15), `[0x4761D0]` (2^-12), `[0x475DBC]` (0.001), `[0x4760CC]` (5.0).
const HUNDREDTH: f32 = f32::from_bits(0x3C23_D70A);
const SIXTEENTH: f32 = f32::from_bits(0x3D88_8889);
const TWO_POW_MINUS_12: f32 = f32::from_bits(0x3980_0000);
const THOUSANDTH: f32 = f32::from_bits(0x3A83_126F);
const FIVE: f32 = 5.0;
/// String constant address passed to the stream open (`0x476220`) and the 3D curve tables.
const STREAM_NAME: u32 = 0x47_6220;
const CURVE_PLAIN: u32 = 0x4B_9114;
const CURVE_ALT: u32 = 0x4B_9124;

/// `cvttss2si`: truncation, the integer indefinite value `i32::MIN` when out of range or NaN.
fn cvttss2si(value: f32) -> i32 {
    if value.is_nan() || value >= 2_147_483_648.0 || value < -2_147_483_648.0 {
        i32::MIN
    } else {
        value as i32
    }
}

fn table_entry(sound: u32) -> u32 {
    sound.wrapping_mul(20)
}

fn sound_word(mem: &dyn SvcMemory, sound: u32, offset: u32) -> u32 {
    mem.r32(
        mem.r32(SOUND_TABLE_PTR)
            .wrapping_add(table_entry(sound))
            .wrapping_add(offset),
    )
}

// ---------------------------------------------------------------------------------------------------------------
// Small pure callees
// ---------------------------------------------------------------------------------------------------------------

/// `0x27F80(id)`: the sound table entry is loaded (`[entry + 0xC] >= 1`, signed).
pub fn sound_known(mem: &dyn SvcMemory, id: u32) -> bool {
    (sound_word(mem, id, 0xC) as i32) >= 1
}

/// `0x45710(request)`: allocates the next request token (1 to 0x3FFFFFF, wraps to 1) and merges the class bits from
/// the sound table flags and the owner field: `0x2000000` (flag bit 12), `0x10000000` (bit 6), `0x4000000` (bit 10),
/// `0x40000000` (bit 4), `0x20000000` (request word 1 nonzero).
pub fn allocate_token(mem: &mut dyn SvcMemory, request: u32) -> u32 {
    let mut token = mem.r32(TOKEN_COUNTER);
    let next = token.wrapping_add(1);
    mem.w32(TOKEN_COUNTER, next);
    if (next as i32) > 0x3FF_FFFF {
        mem.w32(TOKEN_COUNTER, 1);
    }
    let sound = mem.r32(request);
    let flags = sound_word(mem, sound, 8);
    if flags & 0x1000 != 0 {
        token |= 0x200_0000;
    }
    if flags & 0x40 != 0 {
        token |= 0x1000_0000;
    }
    if flags & 0x400 != 0 {
        token |= 0x400_0000;
    }
    if flags & 0x10 != 0 {
        token |= 0x4000_0000;
    }
    if mem.r32(request.wrapping_add(4)) != 0 {
        token |= 0x2000_0000;
    }
    token
}

/// `0x45190(eax = request)`: per frame duplicate filter. Sounds with flags `0x510` always pass. Others pass once per
/// frame (list of up to 100 ids, reset when the frame stamp changes). Returns true when the request may continue.
pub fn dedup_request(mem: &mut dyn SvcMemory, request: u32) -> bool {
    let sound = mem.r32(request);
    if sound_word(mem, sound, 8) & 0x510 != 0 {
        return true;
    }
    let frame = mem.r32(FRAME_STAMP);
    let mut count: i32;
    if frame != mem.r32(DEDUP_STAMP) {
        mem.w32(DEDUP_STAMP, frame);
        count = 0;
    } else {
        count = mem.r32(DEDUP_COUNT) as i32;
        for index in 0..count.max(0) {
            if mem.r32(DEDUP_LIST.wrapping_add(4 * index as u32)) == sound {
                return false;
            }
        }
        if count >= 100 {
            return false;
        }
    }
    mem.w32(
        DEDUP_LIST.wrapping_add(4u32.wrapping_mul(count as u32)),
        sound,
    );
    count += 1;
    mem.w32(DEDUP_COUNT, count as u32);
    true
}

/// `0x453D0()`: next free slot (state word `+0x3C` of the slot header at `0x6D58A4 + 0x40 * i` is zero) of the
/// 25 slot deferred ring, searching from the cursor. Updates the cursor. Returns -1 when all 25 are in use.
pub fn free_queue_slot(mem: &mut dyn SvcMemory) -> i32 {
    let mut cursor = mem.r32(QUEUE_CURSOR) as i32;
    let mut tries = 0;
    loop {
        if tries >= 0x19 {
            mem.w32(QUEUE_CURSOR, cursor as u32);
            return -1;
        }
        tries += 1;
        cursor = cursor.wrapping_add(1);
        // idiv by 25 (remainder keeps the dividend sign, as the original does)
        cursor = cursor.wrapping_rem(QUEUE_SLOTS);
        let state = mem.r32(0x6D_58A4u32.wrapping_add((cursor << 6) as u32));
        if state == 0 {
            mem.w32(QUEUE_CURSOR, cursor as u32);
            return cursor;
        }
    }
}

/// `0x45410(eax = request, ebx = token)`: queues a request copy for later. -2 when the ring is full, else the token.
/// Slot (0x40 bytes at `0x6D5868 + 0x40 * i`): the 12 request words (word 1 cleared), `+0x30` token, `+0x38` the
/// owner (request word 1), `+0x3C` state 2.
pub fn queue_request(mem: &mut dyn SvcMemory, request: u32, token: u32) -> i32 {
    let slot = free_queue_slot(mem);
    if slot < 0 {
        return -2;
    }
    let base = QUEUE_BASE.wrapping_add((slot << 6) as u32);
    mem.w32(base.wrapping_add(0x38), mem.r32(request.wrapping_add(4)));
    mem.w32(base.wrapping_add(0x30), token);
    for word in 0..12 {
        let value = mem.r32(request.wrapping_add(4 * word));
        mem.w32(base.wrapping_add(4 * word), value);
    }
    mem.w32(base.wrapping_add(4), 0);
    mem.w32(base.wrapping_add(0x3C), 2);
    token as i32
}

/// `0x26B00(eax = positional)`: takes a free hardware voice from the plain or positional pool (28 entries
/// of `used` flag and handle). Returns the handle, 0 when none is free.
pub fn take_voice_handle(mem: &mut dyn SvcMemory, positional: bool) -> u32 {
    let (used, handles) = if positional {
        (POSITIONAL_USED, POSITIONAL_HANDLES)
    } else {
        (PLAIN_USED, PLAIN_HANDLES)
    };
    for index in 0..0x28u32 {
        if mem.r32(used + 4 * index) == 0 {
            mem.w32(used + 4 * index, 1);
            return mem.r32(handles + 4 * index);
        }
    }
    0
}

// ---------------------------------------------------------------------------------------------------------------
// 0x28240: start a sound request on a voice or a stream
// ---------------------------------------------------------------------------------------------------------------

fn call(hw: &mut dyn SoundHw, callee: u32, reg: u32, args: &[u32]) -> u32 {
    hw.call(HwCall {
        callee,
        reg,
        args: args.to_vec(),
    })
}

/// `0x28240(request, token)`. Returns 0 on success, -1 when neither a voice nor a stream could be started.
/// Request layout (the 0x30 byte record of `0x45390`): `+0` sound id, `+4` owner, `+8` volume, `+0x10` flags
/// (bit 9 selects the alternate 3D curve), `+0x14` pitch/ramp float, `+0x18` word copied to the voice, `+0x1C..0x24`
/// position, `+0x28` positional flag byte, `+0x2C` priority float.
pub fn start_sound(mem: &mut dyn SvcMemory, hw: &mut dyn SoundHw, request: u32, token: u32) -> i32 {
    let sound = mem.r32(request);
    let entry = table_entry(sound);
    let voice_ready = mem.r32(SOUND_EXTRA.wrapping_add(entry));
    let mut free_count = mem.r32(VOICE_FREE_COUNT);
    if voice_ready != 0 && free_count != 0 {
        let index = mem.r32(VOICE_FREE_LIST.wrapping_add(free_count.wrapping_mul(4))) as i32;
        free_count = free_count.wrapping_sub(1);
        let record_offset = (index as u32).wrapping_mul(VOICE_STRIDE);
        mem.w32(VOICE_FREE_COUNT, free_count);
        mem.w32(
            VOICE_RECORDS.wrapping_add(4).wrapping_add(record_offset),
            token,
        );
        if index != -1 {
            let voice = VOICE_RECORDS.wrapping_add(record_offset);
            let positional = mem.r8(request.wrapping_add(0x28));
            let handle = take_voice_handle(mem, positional != 0);
            if handle == 0 {
                mem.w32(voice, 0);
                mem.w32(VOICE_FREE_COUNT, free_count.wrapping_add(1));
            } else {
                start_on_voice(mem, hw, request, token, voice, handle, voice_ready);
                return 0;
            }
        }
    }
    start_on_stream(mem, hw, request, token)
}

fn start_on_voice(
    mem: &mut dyn SvcMemory,
    hw: &mut dyn SoundHw,
    request: u32,
    token: u32,
    voice: u32,
    handle: u32,
    voice_ready: u32,
) {
    let sound = mem.r32(request);
    let entry = table_entry(sound);
    let extra_d = mem.r32(SOUND_EXTRA.wrapping_add(entry).wrapping_add(0xC));
    call(hw, hw::VOICE_BIND, 0, &[handle, voice_ready, extra_d]);
    let volume_byte =
        u32::from(mem.r8(mem.r32(SOUND_TABLE_PTR).wrapping_add(entry).wrapping_add(5)));
    mem.wf(
        voice.wrapping_add(0x20),
        volume_byte as i32 as f32 * HUNDREDTH,
    );
    let word = mem.r32(request.wrapping_add(0x18));
    mem.w32(voice.wrapping_add(0x28), word);
    let scaled = mem.rf(request.wrapping_add(8)) * SIXTEENTH;
    let clamped = scaled.clamp(0.0, ONE);
    let args = [
        mem.r32(voice.wrapping_add(0x18)),
        mem.r32(voice.wrapping_add(0x20)),
        clamped.to_bits(),
        mem.r32(voice.wrapping_add(0x28)),
        mem.r32(voice.wrapping_add(0x1C)),
    ];
    mem.wf(voice.wrapping_add(0x24), clamped);
    let result = call(hw, hw::VOICE_PARAMS, 0, &args);
    mem.w32(voice.wrapping_add(0x2C), result);
    call(hw, hw::VOICE_SET, 0, &[handle, result]);
    let positional = mem.r8(request.wrapping_add(0x28));
    mem.w8(voice.wrapping_add(0x14), positional);
    let flags = mem.r32(request.wrapping_add(0x10));
    mem.w32(voice.wrapping_add(0x1C), flags);
    mem.w32(voice, handle);
    mem.w32(voice.wrapping_add(0x18), sound);
    mem.w8(voice.wrapping_add(0x15), 0);
    mem.w8(voice.wrapping_add(0x16), 2);
    mem.w8(voice.wrapping_add(0x17), 0);
    if positional != 0 {
        let position = request.wrapping_add(0x1C);
        for axis in 0..3 {
            let value = mem.r32(position.wrapping_add(4 * axis));
            mem.w32(voice.wrapping_add(8 + 4 * axis), value);
        }
        call(hw, hw::VOICE_3D_MODE, 0, &[handle, ONE.to_bits(), 1]);
        let curve = if mem.r32(voice.wrapping_add(0x1C)) & 0x200 != 0 {
            CURVE_ALT
        } else {
            CURVE_PLAIN
        };
        call(hw, hw::VOICE_3D_CURVE, 0, &[handle, curve, 4, 1]);
        call(hw, hw::VOICE_POSITION, position, &[handle]);
        let head = mem.r32(POSITIONAL_HEAD);
        mem.w32(head.wrapping_add(0x34), voice);
        let head = mem.r32(POSITIONAL_HEAD);
        mem.w32(voice.wrapping_add(0x30), head);
        mem.w32(voice.wrapping_add(0x34), POSITIONAL_SENTINEL);
        mem.w32(POSITIONAL_HEAD, voice);
        let priority = mem.r32(request.wrapping_add(0x2C));
        call(hw, hw::VOICE_VOLUME, 0, &[token, priority]);
    } else {
        let head = mem.r32(PLAIN_HEAD);
        mem.w32(head.wrapping_add(0x34), voice);
        let head = mem.r32(PLAIN_HEAD);
        mem.w32(voice.wrapping_add(0x30), head);
        mem.w32(voice.wrapping_add(0x34), PLAIN_SENTINEL);
        mem.w32(PLAIN_HEAD, voice);
    }
    let ramp = mem.r32(request.wrapping_add(0x14));
    call(hw, hw::VOICE_START, 0, &[token, ramp]);
}

fn start_on_stream(mem: &mut dyn SvcMemory, hw: &mut dyn SoundHw, request: u32, token: u32) -> i32 {
    let sound = mem.r32(request);
    let entry = table_entry(sound);
    let stream_value = mem.r32(SOUND_EXTRA.wrapping_add(entry).wrapping_add(4));
    if stream_value == 0 {
        return -1;
    }
    let mut object = STREAM_BASE;
    let mut index = 0u32;
    let wanted;
    loop {
        if object >= STREAM_END {
            return -1;
        }
        let flags = sound_word(mem, sound, 8);
        let candidate = (!(flags >> 9)) & 1;
        if mem.r32(object.wrapping_add(0x50)) == candidate
            && call(hw, hw::STREAM_BUSY, object, &[]) & 0xFF == 0
        {
            wanted = candidate;
            break;
        }
        object = object.wrapping_add(STREAM_STRIDE);
        index += 1;
    }
    let table = mem.r32(SOUND_TABLE_PTR).wrapping_add(entry);
    let volume = f32::from(mem.r8(table.wrapping_add(5))) * HUNDREDTH;
    let handle = call(
        hw,
        hw::STREAM_OPEN,
        0,
        &[stream_value, 0, STREAM_NAME, 0x2FA],
    );
    let direct = (sound_word(mem, sound, 8) >> 4) & 1;
    let ramp = mem.rf(request.wrapping_add(0x14));
    let immediate = THOUSANDTH > ramp;
    if immediate {
        mem.wf(request.wrapping_add(0x14), ONE);
    }
    mem.w32(STREAM_TOKENS.wrapping_add(4 * index), token);
    let table = mem.r32(SOUND_TABLE_PTR).wrapping_add(entry);
    let bank = u32::from(mem.r8(table.wrapping_add(4)));
    let count = f32::from(mem.r16(BANK_COUNTS.wrapping_add(2 * bank)));
    let scaled = cvttss2si(count * mem.rf(request.wrapping_add(0x14)));
    let rate = cvttss2si(scaled.wrapping_mul(0xBB80) as f32 * TWO_POW_MINUS_12);
    let level = (mem.rf(request.wrapping_add(8)) * SIXTEENTH).to_bits();
    let args = [
        sound,
        handle,
        mem.r32(SOUND_EXTRA.wrapping_add(entry).wrapping_add(0xC)),
        volume.to_bits(),
        level,
        direct,
        rate as u32,
        mem.r32(SOUND_EXTRA.wrapping_add(entry).wrapping_add(8)),
        1,
    ];
    call(hw, hw::STREAM_SETUP, object, &args);
    let priority = mem.r32(request.wrapping_add(0x2C));
    call(hw, hw::VOICE_VOLUME, 0, &[token, priority]);
    let word = mem.r32(request.wrapping_add(0x18));
    call(hw, hw::STREAM_SET_PRIORITY, object, &[word]);
    mem.w32(
        object.wrapping_add(0x68),
        mem.r32(request.wrapping_add(0x10)),
    );
    if mem.r8(request.wrapping_add(0x28)) != 0 {
        call(hw, hw::STREAM_SET_POSITIONAL, object, &[wanted]);
        call(
            hw,
            hw::STREAM_SET_POSITION,
            object,
            &[request.wrapping_add(0x1C)],
        );
        call(hw, hw::STREAM_COMMIT, object, &[]);
    }
    if immediate {
        call(hw, hw::STREAM_PLAY, object, &[]);
        mem.w32(object.wrapping_add(0x24), 1);
    }
    0
}

// ---------------------------------------------------------------------------------------------------------------
// 0x45820: submit
// ---------------------------------------------------------------------------------------------------------------

/// `0x45820(request)`: submits an effect request (the 0x30 byte record at `request`).
/// Returns -1 for an invalid or unloaded sound id or a failed start, -3 for a filtered duplicate, -2 when a deferred
/// request finds the ring full, else the request token. Token class bits (`0x45710`): `0x40000000` sets the request
/// flag `0x20` and clears its volume, `0x20000000` (the request has an owner) queues the request (`0x45410`).
pub fn submit_effect(mem: &mut dyn SvcMemory, hw: &mut dyn SoundHw, request: u32) -> i32 {
    submit_effect_with(mem, request, &mut |memory, record, token| {
        start_sound(memory, hw, record, token)
    })
}

/// [`submit_effect`] with the voice start (`0x28240(request, token)`) supplied by the caller.
pub fn submit_effect_with(
    mem: &mut dyn SvcMemory,
    request: u32,
    start: &mut dyn FnMut(&mut dyn SvcMemory, u32, u32) -> i32,
) -> i32 {
    let sound = mem.r32(request) as i32;
    if !(0..=MAX_SOUND_ID).contains(&sound) {
        return -1;
    }
    if !sound_known(mem, sound as u32) {
        return -1;
    }
    let token = allocate_token(mem, request);
    if token & 0x4000_0000 != 0 {
        let flags = mem.r32(request.wrapping_add(0x10));
        mem.w32(request.wrapping_add(0x10), flags | 0x20);
        mem.wf(request.wrapping_add(8), 0.0);
    }
    if token & 0x2000_0000 != 0 {
        return queue_request(mem, request, token);
    }
    if !dedup_request(mem, request) {
        return -3;
    }
    if start(mem, request, token) != 0 {
        return -1;
    }
    token as i32
}

// ---------------------------------------------------------------------------------------------------------------
// Listener distance (0x45200, 0x45310), cancel (0x45460), held sounds (0x46BC0, 0x46D80), starter (0x468A0)
// ---------------------------------------------------------------------------------------------------------------

/// Override listener position pointer, listener mode flags and the player table of `0x45200`.
pub const LISTENER_OVERRIDE: u32 = 0x6D_5EB0;
pub const LISTENER_MODE: u32 = 0x7D_E458;
pub const PLAYER_TABLE_PTR: u32 = 0x7B_0C48;
pub const PLAYER_STRIDE: u32 = 0x1584;

/// `0x45200(src, dst)`: the listener position nearest to `src`. `dst` starts as `src`; an override position
/// (`[0x6D5EB0]`) replaces it outright; otherwise (unless mode bit `0x20000000`) each local player's position
/// (`player + 0x27C`) closer than the best so far (start `FLT_MAX`, strict) replaces it.
pub fn nearest_listener(mem: &mut dyn SvcMemory, hw: &mut dyn SoundHw, source: u32, dest: u32) {
    for axis in 0..3 {
        let value = mem.r32(source.wrapping_add(4 * axis));
        mem.w32(dest.wrapping_add(4 * axis), value);
    }
    let override_ptr = mem.r32(LISTENER_OVERRIDE);
    if override_ptr != 0 {
        for axis in 0..3 {
            let value = mem.r32(override_ptr.wrapping_add(4 * axis));
            mem.w32(dest.wrapping_add(4 * axis), value);
        }
        return;
    }
    if mem.r32(LISTENER_MODE) & 0x2000_0000 != 0 {
        return;
    }
    let mut best = f32::from_bits(0x7F7F_FFFF);
    if hw.player_count() <= 0 {
        return;
    }
    let mut index = 0;
    loop {
        let slot = hw.player_slot(index);
        let player = mem.r32(
            mem.r32(PLAYER_TABLE_PTR)
                .wrapping_add((slot as u32).wrapping_mul(PLAYER_STRIDE))
                .wrapping_add(8),
        );
        if player != 0 {
            let dy = mem.rf(player.wrapping_add(0x280)) - mem.rf(source.wrapping_add(4));
            let dz = mem.rf(player.wrapping_add(0x284)) - mem.rf(source.wrapping_add(8));
            let dx = mem.rf(player.wrapping_add(0x27C)) - mem.rf(source);
            let distance = (dx * dx + dy * dy) + dz * dz;
            if best > distance {
                for axis in 0..3 {
                    let value = mem.r32(player.wrapping_add(0x27C + 4 * axis));
                    mem.w32(dest.wrapping_add(4 * axis), value);
                }
                best = distance;
            }
        }
        index += 1;
        if index >= hw.player_count() {
            break;
        }
    }
}

/// `0x45310(position, radius)`: the position is within `radius` of its nearest listener (`radius^2 >= d^2`).
/// `scratch` holds the listener copy (12 bytes).
pub fn within_listener_range(
    mem: &mut dyn SvcMemory,
    hw: &mut dyn SoundHw,
    position: u32,
    radius: f32,
    scratch: u32,
) -> bool {
    nearest_listener(mem, hw, position, scratch);
    let dx = mem.rf(position) - mem.rf(scratch);
    let dy = mem.rf(position.wrapping_add(4)) - mem.rf(scratch.wrapping_add(4));
    let dz = mem.rf(position.wrapping_add(8)) - mem.rf(scratch.wrapping_add(8));
    let distance = (dx * dx + dy * dy) + dz * dz;
    radius * radius >= distance
}

/// `0x45460(token)`: cancels a request. A queued token (`0x20000000`) marks its ring slot (token at `+0x30`) free
/// (state 1) and cancels the voice when the slot was already started (state 4); other tokens cancel directly.
pub fn cancel_request(mem: &mut dyn SvcMemory, hw: &mut dyn SoundHw, token: u32) {
    if token & 0x2000_0000 == 0 {
        call(hw, hw::VOICE_CANCEL, 0, &[token]);
        return;
    }
    let mut state = 0x6D_58A4u32;
    while state < 0x6D_5EE4 {
        if mem.r32(state.wrapping_sub(0xC)) == token {
            if mem.r32(state) == 4 {
                call(hw, hw::VOICE_CANCEL, 0, &[token]);
            }
            mem.w32(state, 1);
        }
        state = state.wrapping_add(0x40);
    }
}

/// `0x46BC0(id, handle_ptr, flag)`: parks the started handle `[handle_ptr]` of sound `id` in the first free entry of
/// the 4 entry held table (restarting it with value 0 first), or cancels it when the table is full. Clears the handle.
pub fn park_handle(
    mem: &mut dyn SvcMemory,
    hw: &mut dyn SoundHw,
    id: u32,
    handle_ptr: u32,
    flag: u32,
) {
    let mut entry = None;
    for index in 0..HELD_ENTRIES {
        let address = HELD_TABLE + 12 * index;
        if mem.r32(address) == u32::MAX {
            entry = Some(address);
            break;
        }
    }
    let Some(address) = entry else {
        let handle = mem.r32(handle_ptr);
        cancel_request(mem, hw, handle);
        mem.w32(handle_ptr, u32::MAX);
        return;
    };
    let handle = mem.r32(handle_ptr);
    call(hw, hw::VOICE_START, 0, &[handle, 0]);
    let handle = mem.r32(handle_ptr);
    if (handle as i32) >= 0 {
        mem.w32(address.wrapping_add(4), handle);
        mem.w32(address, id);
        mem.w16(address.wrapping_add(8), u16::from(flag != 0));
    }
    mem.w32(handle_ptr, u32::MAX);
}

/// Arguments of `0x46D80` (all on the stack).
#[derive(Debug, Clone, Copy)]
pub struct HeldSound {
    pub id: u32,
    /// `a1`: volume float bits.
    pub volume: u32,
    /// `a2`: value passed to the voice start.
    pub start_value: u32,
    /// `a3`: float bits.
    pub gain: u32,
    /// `a4`: position pointer or 0.
    pub position: u32,
    /// `a5`: when nonzero the range is 5.0 and the request gets flag `0x280`.
    pub range_flag: u32,
}

/// `0x46D80`: retriggers the held handle of sound `id` or submits a new request. Returns the handle (retrigger),
/// the `0x45820` result (new), or -1 for a negative id. `record` is guest scratch for the 0x30 byte request.
pub fn start_held_sound(
    mem: &mut dyn SvcMemory,
    hw: &mut dyn SoundHw,
    sound: HeldSound,
    record: u32,
) -> i32 {
    start_held_sound_with(mem, hw, sound, record, &mut submit_effect)
}

/// Submit callback of the held sound functions (the real one is [`submit_effect`]).
pub type SubmitFn<'a> = &'a mut dyn FnMut(&mut dyn SvcMemory, &mut dyn SoundHw, u32) -> i32;

/// [`start_held_sound`] with the request submit supplied by the caller.
pub fn start_held_sound_with(
    mem: &mut dyn SvcMemory,
    hw: &mut dyn SoundHw,
    sound: HeldSound,
    record: u32,
    submit: SubmitFn,
) -> i32 {
    let range = if sound.range_flag != 0 { FIVE } else { 0.0 };
    if (sound.id as i32) <= -1 {
        return -1;
    }
    let mut found = None;
    for index in 0..HELD_ENTRIES {
        if mem.r32(HELD_TABLE + 12 * index) == sound.id {
            found = Some(HELD_TABLE + 12 * index);
            break;
        }
    }
    let extra_flags = if sound.range_flag != 0 { 0x280 } else { 0 };
    if let Some(address) = found {
        let handle = mem.r32(address.wrapping_add(4));
        call(hw, hw::VOICE_START, 0, &[handle, sound.start_value]);
        let handle = mem.r32(address.wrapping_add(4));
        if sound.position != 0 {
            call(hw, hw::VOICE_RANGE, 0, &[handle, range.to_bits()]);
            let handle = mem.r32(address.wrapping_add(4));
            call(hw, hw::VOICE_VOLUME, 0, &[handle, sound.volume]);
            let handle = mem.r32(address.wrapping_add(4));
            call(hw, hw::VOICE_MOVE, 0, &[handle, sound.position, sound.gain]);
        } else {
            call(hw, hw::VOICE_MOVE, 0, &[handle, 0, sound.gain]);
        }
        let handle = mem.r32(address.wrapping_add(4));
        mem.w32(address, u32::MAX);
        return handle as i32;
    }
    init_request(mem, record);
    if sound.position != 0 {
        mem.w32(record.wrapping_add(0x2C), sound.volume);
        mem.wf(record.wrapping_add(8), range);
        mem.w32(record.wrapping_add(0x18), sound.gain);
        mem.w32(record.wrapping_add(0x14), sound.start_value);
        for axis in 0..3 {
            let value = mem.r32(sound.position.wrapping_add(4 * axis));
            mem.w32(record.wrapping_add(0x1C + 4 * axis), value);
        }
        mem.w32(record, sound.id);
        let flags = mem.r32(record.wrapping_add(0x10)) | extra_flags;
        mem.w8(record.wrapping_add(0x28), 1);
        mem.w32(record.wrapping_add(0x10), flags);
    } else {
        mem.w32(record, sound.id);
        mem.w32(record.wrapping_add(0x18), sound.gain);
        let flags = mem.r32(record.wrapping_add(0x10)) | extra_flags;
        mem.wf(record.wrapping_add(0x14), ONE);
        mem.w32(record.wrapping_add(0x10), flags);
    }
    submit(mem, hw, record)
}

/// `0x45390(record)`: the default effect request (zero, flags `0x14`, scale and volume 1.0, `+0x2C` = 50.0).
pub fn init_request(mem: &mut dyn SvcMemory, record: u32) {
    for word in 0..12 {
        mem.w32(record.wrapping_add(4 * word), 0);
    }
    mem.w32(record.wrapping_add(0x10), 0x14);
    mem.w8(record.wrapping_add(0x28), 0);
    mem.wf(record.wrapping_add(8), ONE);
    mem.wf(record.wrapping_add(0x18), ONE);
    mem.wf(record.wrapping_add(0x2C), 50.0);
    mem.wf(record.wrapping_add(0x14), ONE);
}

/// Arguments of `0x468A0` (stack order).
#[derive(Debug, Clone, Copy)]
pub struct StarterArgs {
    /// `a0`: pointer to the handle cell (negative content: none running).
    pub handle_ptr: u32,
    /// `a1`: position pointer or 0.
    pub position: u32,
    /// `a2`: sound id (negative: do nothing).
    pub id: i32,
    /// `a3`: gain float bits (above 0.001 keeps the sound alive unless flag bit 2).
    pub gain: u32,
    /// `a4`: audible radius float bits (checked against the nearest listener).
    pub radius: u32,
    /// `a5`: value forwarded to the voice start.
    pub start_value: u32,
    /// `a6`: bit 0 restart on update, bit 1 park on stop, bit 2 skip the gain and range tests.
    pub flags: u32,
}

/// `0x468A0` (INFERRED name: positional sound loop starter, called "light ramp" in the prelude notes). Keeps a sound
/// alive while its gain is audible and the position is within the radius of a listener: a running handle is moved
/// (`0x27CC0`) and optionally restarted, an empty cell starts the sound through [`start_held_sound`]. When the
/// conditions fail a running handle is parked (flag bit 1) or cancelled and the cell cleared. Returns 1 when the
/// sound is running or was started, else 0. `scratch` is guest scratch (0x30 + 12 bytes).
pub fn sound_starter(
    mem: &mut dyn SvcMemory,
    hw: &mut dyn SoundHw,
    args: StarterArgs,
    scratch: u32,
) -> u32 {
    sound_starter_with(mem, hw, args, scratch, &mut submit_effect)
}

/// [`sound_starter`] with the request submit supplied by the caller.
pub fn sound_starter_with(
    mem: &mut dyn SvcMemory,
    hw: &mut dyn SoundHw,
    args: StarterArgs,
    scratch: u32,
    submit: SubmitFn,
) -> u32 {
    if args.id <= -1 {
        return 0;
    }
    let skip_tests = (args.flags >> 2) & 1 != 0;
    let gain = f32::from_bits(args.gain);
    if !skip_tests && !(gain > THOUSANDTH) {
        return starter_stop(mem, hw, args);
    }
    if args.position == 0 {
        return starter_run(mem, hw, args, scratch, submit);
    }
    if !skip_tests {
        let radius = f32::from_bits(args.radius);
        if !within_listener_range(mem, hw, args.position, radius, scratch + 0x30) {
            return starter_stop(mem, hw, args);
        }
    }
    starter_run(mem, hw, args, scratch, submit)
}

fn starter_stop(mem: &mut dyn SvcMemory, hw: &mut dyn SoundHw, args: StarterArgs) -> u32 {
    let handle = mem.r32(args.handle_ptr);
    if (handle as i32) < 0 {
        return 0;
    }
    if args.flags & 2 != 0 {
        park_handle(
            mem,
            hw,
            args.id as u32,
            args.handle_ptr,
            u32::from(args.position != 0),
        );
    } else {
        cancel_request(mem, hw, handle);
        mem.w32(args.handle_ptr, u32::MAX);
    }
    0
}

fn starter_run(
    mem: &mut dyn SvcMemory,
    hw: &mut dyn SoundHw,
    args: StarterArgs,
    scratch: u32,
    submit: SubmitFn,
) -> u32 {
    let handle = mem.r32(args.handle_ptr);
    if (handle as i32) < 0 {
        let radius = if args.position == 0 { 0 } else { args.radius };
        let result = start_held_sound_with(
            mem,
            hw,
            HeldSound {
                id: args.id as u32,
                volume: radius,
                start_value: args.start_value,
                gain: args.gain,
                position: args.position,
                range_flag: 0,
            },
            scratch,
            submit,
        );
        mem.w32(args.handle_ptr, result as u32);
        return 1;
    }
    call(hw, hw::VOICE_MOVE, 0, &[handle, args.position, args.gain]);
    if args.flags & 1 != 0 {
        let handle = mem.r32(args.handle_ptr);
        if (handle as i32) >= 0 {
            call(hw, hw::VOICE_START, 0, &[handle, args.start_value]);
        }
    }
    1
}

// ---------------------------------------------------------------------------------------------------------------
// 0x160F50, 0xE9380, 0xE94C0: nearby object disturbance
// ---------------------------------------------------------------------------------------------------------------

/// Object list `[0x7A3580 + 4 * i]` between the cursor `[0x790950]` and the count `[0x74C37C]`, the mode byte
/// `[0x7DE455]` (10 selects the first of two flag word pairs), the frame stamp `[0x7DE318]`.
pub const OBJECT_LIST: u32 = 0x7A_3580;
pub const OBJECT_FIRST: u32 = 0x79_0950;
pub const OBJECT_COUNT: u32 = 0x74_C37C;
pub const OBJECT_MODE: u32 = 0x7D_E455;
pub const DISTURB_STAMP: u32 = 0x7D_E318;

/// `0x160F50(object)`: the object is a vehicle controller (`[object + 0x20] == 8` and the tag at
/// `[[[object + 0x7C] + 0xBF0] + 8]` is `0x45434956` or `0x4F434956`, ASCII "VICE" and "VICO" read backwards).
pub fn is_vehicle_object(mem: &dyn SvcMemory, object: u32) -> bool {
    if mem.r32(object.wrapping_add(0x20)) != 8 {
        return false;
    }
    let inner = mem.r32(object.wrapping_add(0x7C));
    let tag = mem.r32(mem.r32(inner.wrapping_add(0xBF0)).wrapping_add(8));
    tag == 0x4543_4956 || tag == 0x4F43_4956
}

/// `0xE9380(position, shape_end, radius, out)`: marks every candidate object whose centre (`+0x64`, y raised by 1.0)
/// is inside `radius` (a squared distance is compared) of the point `position`, or of the segment
/// `position..shape_end` when `shape_end` is nonzero (`0x49370`). A hit stores the frame stamp at `+0xB60`, the
/// value at `out` (or -1) at `+0xB64` and clears flag bit `0x08000000` of the object's word `+0x28`.
/// Candidates: objects whose flag pair word (`+0x10`/`+0x14` when the mode byte is 10, else `+0x18`/`+0x1C`) has none of
/// `0x09180200`, or vehicle objects (`0x160F50`).
pub fn disturb_objects(
    mem: &mut dyn SvcMemory,
    position: u32,
    shape_end: u32,
    radius: f32,
    out: u32,
) {
    let mut index = mem.r32(OBJECT_FIRST) as i32;
    if index >= mem.r32(OBJECT_COUNT) as i32 {
        return;
    }
    loop {
        let entry = mem.r32(OBJECT_LIST.wrapping_add((index as u32).wrapping_mul(4)));
        let low = if mem.r8(OBJECT_MODE) == 0xA {
            mem.r32(entry.wrapping_add(0x10))
        } else {
            mem.r32(entry.wrapping_add(0x18))
        };
        let candidate = low & 0x0918_0200 != 0 || is_vehicle_object(mem, mem.r32(entry));
        if candidate {
            let body = mem.r32(entry);
            let point = [
                mem.rf(body.wrapping_add(0x64)),
                mem.rf(body.wrapping_add(0x68)) + ONE,
                mem.rf(body.wrapping_add(0x6C)),
            ];
            let distance = if shape_end != 0 {
                let target = [mem.rf(position), mem.rf(position + 4), mem.rf(position + 8)];
                let end = [
                    mem.rf(shape_end),
                    mem.rf(shape_end + 4),
                    mem.rf(shape_end + 8),
                ];
                segment_closest_point(point, target, end).1 as f32
            } else {
                let dx = point[0] - mem.rf(position);
                let dz = point[2] - mem.rf(position + 8);
                let dy = point[1] - mem.rf(position + 4);
                (dz * dz + dy * dy) + dx * dx
            };
            if radius > distance {
                mem.w32(entry.wrapping_add(0xB60), mem.r32(DISTURB_STAMP));
                let value = if out != 0 { mem.r32(out) } else { u32::MAX };
                mem.w32(entry.wrapping_add(0xB64), value);
                let flags = mem.r32(entry.wrapping_add(0x28));
                mem.w32(entry.wrapping_add(0x28), flags & 0xF7FF_FFFF);
            }
        }
        index += 1;
        if index >= mem.r32(OBJECT_COUNT) as i32 {
            return;
        }
    }
}

/// `0xE94C0(position, radius, out)`: `0xE9380(position, 0, radius, out)`.
pub fn disturb_nearby(mem: &mut dyn SvcMemory, position: u32, radius: f32, out: u32) {
    disturb_objects(mem, position, 0, radius, out);
}

// ---------------------------------------------------------------------------------------------------------------
// ImpactServices adapter
// ---------------------------------------------------------------------------------------------------------------

/// Implements the `spawn_request` and `disturb` services of [`ImpactServices`] with the ported code. Guest scratch
/// at `scratch` (at least 0x80 bytes) holds the request record and the disturbance position. `build_debris` and
/// `ground_height` go to `inner`. `submitted` records the `0x45820` results.
pub struct EffectSubmitServices<'a> {
    pub memory: &'a mut dyn SvcMemory,
    pub hw: &'a mut dyn SoundHw,
    pub inner: &'a mut dyn ImpactServices,
    pub scratch: u32,
    pub submitted: Vec<i32>,
}

impl ImpactServices for EffectSubmitServices<'_> {
    fn disturb(&mut self, position: [f32; 3], radius: f32, flags: u32) {
        let at = self.scratch + 0x40;
        for (axis, value) in position.iter().enumerate() {
            self.memory.wf(at + 4 * axis as u32, *value);
        }
        // `flags` is the optional out pointer of the original (0 in every caller)
        disturb_nearby(self.memory, at, radius, flags);
    }

    fn spawn_request(&mut self, record: [u32; 12]) {
        for (word, value) in record.iter().enumerate() {
            self.memory.w32(self.scratch + 4 * word as u32, *value);
        }
        let result = submit_effect(self.memory, self.hw, self.scratch);
        self.submitted.push(result);
    }

    fn build_debris(&mut self, slot: u32) {
        self.inner.build_debris(slot);
    }

    fn ground_height(&mut self, point: [f32; 3], y_bias: f32, half_extent: f32) -> f32 {
        self.inner.ground_height(point, y_bias, half_extent)
    }
}
