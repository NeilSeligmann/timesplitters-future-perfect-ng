// SPDX-License-Identifier: GPL-3.0-or-later
//! T1900 slice B: multiplayer team pickup (flag) object, Tier 0 (translated from the lifted
//! original, compiles, smoke tested; NOT differentially compared). Nested guest calls go through
//! the T1815 closure model; callee contracts were read from the call sites only.
//!
//! Object model (INFERRED): a *flag* object (`esi`, extension at `+0x7C`) is carried by a player
//! record (`[record+0x80]`), dropped, counted down and returned to a *base* object (extension at
//! `+0x7C`, team at `+0x1C`). Player records are `PLAYER_RECORD_BASE + index * PLAYER_RECORD_STRIDE`.
use crate::closure_api::{Callee, Cpu, GuestMemory, guest_call, stack_arg};

/// Closure-model names required by the shared `tsfp-ports` lane adapter.
pub mod closure_api {
    pub use tsfp_engine_object::closure::callee::{Callee, Cpu, guest_call, stack_arg};
    pub use tsfp_engine_object::closure::memory::GuestMemory;
}

// ---- globals -------------------------------------------------------------------------------
/// Game mode byte; 6 is the assign-team mode, 0xA suppresses the drop shake timer.
pub const GAME_MODE: u32 = 0x007d_e455;
pub const MODE_ASSIGN_TEAM: u8 = 6;
pub const MODE_NO_DROP_TIMER: u8 = 0x0a;
/// Frame tick (integer) subtracted from the dropped countdown.
pub const FRAME_TICKS: u32 = 0x004e_7944;
/// Seconds-per-frame float used for motion integration.
pub const TICK_SCALE: u32 = 0x004e_794c;
/// Ticks per second (integer), used for timers.
pub const TICK_COUNT: u32 = 0x004e_8cec;
/// Number of player records in the player list.
pub const PLAYER_COUNT: u32 = 0x0079_0950;
/// Pointer to the player-entry array (stride `PLAYER_ENTRY_STRIDE`).
pub const PLAYER_ENTRIES: u32 = 0x007b_0c48;
pub const PLAYER_ENTRY_STRIDE: u32 = 0x1584;
/// Entry fields: `+4` handle passed to message/flash calls, `+0x14` player object.
pub const ENTRY_HANDLE: u32 = 4;
pub const ENTRY_OBJECT: u32 = 0x14;
/// Player record array (base pointer global) and stride.
pub const PLAYER_RECORD_BASE: u32 = 0x0073_56d8;
pub const PLAYER_RECORD_STRIDE: u32 = 0xea0;
/// Pointer to a table of 32-byte rows whose dword at `+4` holds flag bits.
pub const PICKUP_TABLE: u32 = 0x0071_5f2c;
pub const PICKUP_ROW_SHIFT: u32 = 5;
pub const PICKUP_ROW_FLAGS: u32 = 4;
pub const PICKUP_ROW_CARRIED_BIT: u32 = 0x0020_0000;
/// Fallback team colour passed to the colour dispatcher.
pub const DEFAULT_TEAM_COLOR: u32 = 0x007a_d288;
/// printf-style format used for the capture message.
pub const CAPTURE_FORMAT: u32 = 0x0047_8904;

// ---- constants ------------------------------------------------------------------------------
pub const STAT_FLAG_CAPTURE: u32 = 0x28;
pub const SOUND_FLAG_DELIVER: u32 = 0xd6e;
pub const SOUND_FLAG_DROPPED: u32 = 0xd44;
/// Capture message: team name texts for teams 1..=4, then the "captured" text pair.
pub const TEAM_TEXTS: [u16; 4] = [0x18fe, 0x18ff, 0x1900, 0x1901];
pub const TEXT_CAPTURE_OTHERS: u32 = 0x1902;
pub const TEXT_CAPTURE_OWN: u32 = 0x1903;
pub const MESSAGE_KIND: u32 = 3;
pub const MESSAGE_BASE_TICKS: u32 = 0x40;
pub const DROP_SHAKE_TICKS_FACTOR: u32 = 12;
const SCRATCH_BYTES: u32 = 0x5c;

// ---- callee VAs (owning system INFERRED) ---------------------------------------------------
pub const CALL_PLAYER_POSITION: u32 = 0x0004_5cf0;
pub const CALL_EVENT_INIT: u32 = 0x0004_5390;
pub const CALL_EVENT_POST: u32 = 0x0004_5820;
pub const CALL_STAT_ADD: u32 = 0x0023_8510;
pub const CALL_PLAYER_SCORE_POINT: u32 = 0x0006_4460;
pub const CALL_ENTRY_FLASH: u32 = 0x0012_e3b0;
pub const CALL_TEXT_LOOKUP: u32 = 0x0003_d7b0;
pub const CALL_FORMAT: u32 = 0x0015_1030;
pub const CALL_ENTRY_POST_TEXT: u32 = 0x000c_f4d0;
pub const CALL_ENTRY_MESSAGE: u32 = 0x0012_d350;
pub const CALL_TEAM_SCORE: u32 = 0x0016_4a30;
pub const CALL_RETURNED_HOME: u32 = 0x000b_e1d0;
pub const CALL_RESET_HOME: u32 = 0x000b_ef00;
pub const CALL_DELIVER: u32 = 0x000c_09b0;
pub const CALL_SPAWN_LOOKUP: u32 = 0x000b_e860;
pub const CALL_EFFECT_AT: u32 = 0x0002_17b0;
pub const CALL_DROP_NOTIFY: u32 = 0x0003_1310;
pub const CALL_PLAYER_BY_SLOT: u32 = 0x0016_0ac0;
pub const CALL_CARRIER_VALID: u32 = 0x0027_43a0;
pub const CALL_CLEAR_COLLISION: u32 = 0x0009_66e0;
pub const CALL_RESET_PART: u32 = 0x0015_3240;
pub const CALL_DROP_CARRIED: u32 = 0x000d_83d0;
pub const CALL_TEAM_REFRESH: u32 = 0x0010_ec20;
pub const CALL_TEAM_COLOR: u32 = 0x0010_e6e0;

/// Pickup (flag) extension record at `[object+0x7C]` (INFERRED names).
pub mod ext {
    pub const OWNER_PLAYER: u32 = 0x00; // passed to the carrier validity call
    pub const SPAWN_MODE: u32 = 0x0c; // 1 flag, 2 dropped-effect variant
    pub const SPAWN_PARAM: u32 = 0x14;
    pub const HOME_POS: u32 = 0x20; // vec3, home position
    pub const HOME_VEL_Y: u32 = 0x2c; // copied to object+0xC0
    pub const HOME_VEL_X: u32 = 0x30; // copied to object+0x70
    pub const HOME_VEL_Z: u32 = 0x34; // copied to object+0xC8
    pub const CARRIER_SLOT: u32 = 0x38;
    pub const HOME_SLOT: u32 = 0x3c;
    pub const DROP_POS: u32 = 0x90; // vec3 drop position
    pub const CARRIER_INDEX: u32 = 0x9c; // player index or -1
    pub const STATE: u32 = 0xa0; // 1 home/carried, 2 dropped countdown
    pub const RECORD_INDEX: u32 = 0xa4;
    pub const COUNTDOWN: u32 = 0xa8;
    pub const DROP_VEL: u32 = 0xac; // 3 floats
    /// Team of a base object extension (also the flag's owning team).
    pub const TEAM: u32 = 0x1c;
    /// Fields of a player record (`PLAYER_RECORD_BASE` rows) used here.
    pub const RECORD_SLOT: u32 = 0x08;
    pub const RECORD_CARRIED_FLAG: u32 = 0x80;
    pub const RECORD_TEAM: u32 = 0x84;
}

pub const STATE_HOME: u32 = 1;
pub const STATE_DROPPED: u32 = 2;
pub const SPAWN_MODE_FLAG: u32 = 1;
pub const SPAWN_MODE_DROPPED_EFFECT: u32 = 2;

/// Typed accessor over a guest extension/record base address.
#[derive(Clone, Copy, Debug)]
pub struct Rec(pub u32);
impl Rec {
    pub fn get<M: GuestMemory>(self, m: &M, offset: u32) -> u32 {
        m.read32(self.0.wrapping_add(offset))
    }
    pub fn set<M: GuestMemory>(self, m: &mut M, offset: u32, value: u32) {
        m.write32(self.0.wrapping_add(offset), value);
    }
}

/// Callee boundary: typed wrappers over the outward calls so other systems can attach (the
/// closure `Callee` implementation decides what each call does).
pub struct PickupCalls<'a, M: GuestMemory, C: Callee<M>> {
    pub callee: &'a mut C,
    pub mem: &'a mut M,
    pub cpu: &'a mut Cpu,
}
impl<M: GuestMemory, C: Callee<M>> PickupCalls<'_, M, C> {
    fn call(&mut self, target: u32, pc: u32, args: &[u32]) -> u32 {
        self.call_deep(target, pc, SCRATCH_BYTES, args)
    }
    /// Call with the original's stack depth at that call site (bytes below the entry ESP).
    fn call_deep(&mut self, target: u32, pc: u32, bytes: u32, args: &[u32]) -> u32 {
        guest_call(self.callee, self.mem, self.cpu, target, pc, bytes, args);
        self.cpu.eax
    }
    /// Stat add (0x238510): (stat, slot, amount bits).
    pub fn add_stat(&mut self, stat: u32, slot: u32, amount: f32) {
        self.call(CALL_STAT_ADD, 0x000c_0a15, &[stat, slot, amount.to_bits()]);
    }
    /// Text lookup (0x3D7B0): string id to text pointer.
    pub fn text(&mut self, id: u32, pc: u32, bytes: u32) -> u32 {
        self.call_deep(CALL_TEXT_LOOKUP, pc, bytes, &[id])
    }
    /// Entry message post (0x12D350): (handle, kind, text, 0).
    pub fn post_message(&mut self, handle: u32, text: u32) {
        self.call(
            CALL_ENTRY_MESSAGE,
            0x000c_0b61,
            &[handle, MESSAGE_KIND, text, 0],
        );
    }
    pub fn entry_flash(&mut self, handle: u32, pc: u32) {
        let ticks = self
            .mem
            .read32(TICK_COUNT)
            .wrapping_mul(4)
            .wrapping_add(MESSAGE_BASE_TICKS);
        self.call(CALL_ENTRY_FLASH, pc, &[handle, ticks]);
    }
}

fn f32_at<M: GuestMemory>(m: &M, address: u32) -> f32 {
    f32::from_bits(m.read32(address))
}

/// Player list entries as `(entry pointer, team of the entry's player)`, capped at 16 (the game has at most 4
/// players; a fixed array keeps the module `no_std` for the hybrid lane).
struct Entries {
    items: [(u32, u32); 16],
    len: usize,
}
impl Entries {
    fn as_slice(&self) -> &[(u32, u32)] {
        &self.items[..self.len]
    }
}
fn entries<M: GuestMemory>(m: &M) -> Entries {
    let count = (m.read32(PLAYER_COUNT) as i32).clamp(0, 16) as u32;
    let base = m.read32(PLAYER_ENTRIES);
    let mut list = Entries {
        items: [(0, 0); 16],
        len: count as usize,
    };
    for i in 0..count {
        let entry = base.wrapping_add(i.wrapping_mul(PLAYER_ENTRY_STRIDE));
        let object = m.read32(entry.wrapping_add(ENTRY_OBJECT));
        let team = Rec(Rec(object).get(m, 0x7c)).get(m, ext::RECORD_TEAM);
        list.items[i as usize] = (entry, team);
    }
    list
}

/// 000C09B0 game_team_pickup_object_deliver_post_team_messages_stat_0x28_and_reset_to_home.
/// Register ABI: EAX = base object, ECX = delivering player record, ESI = flag object (preserved).
/// If the player's team owns the base and the flag belongs to another team it is a capture
/// (stat 0x28, score point, messages to both teams, team score); if the flag already belongs to
/// the base team it is just returned. Then the flag is reset to its home position and (unless
/// returned) the reset-home callee runs.
pub fn deliver_flag<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let base = cpu.eax;
    let player = cpu.ecx;
    let flag = cpu.esi;
    let flag_ext = Rec(m.read32(flag.wrapping_add(0x7c)));
    let base_ext = Rec(m.read32(base.wrapping_add(0x7c)));
    let scratch = cpu.esp.wrapping_sub(SCRATCH_BYTES);
    let _ = scratch;
    m.write32(cpu.esp.wrapping_sub(0x5c).wrapping_add(0x10), 0);
    m.write32(cpu.esp.wrapping_sub(0x5c).wrapping_add(0x14), base_ext.0);
    let (vec_out, event) = (cpu.esp.wrapping_sub(0x3c), cpu.esp.wrapping_sub(0x30));
    guest_call(
        c,
        m,
        cpu,
        CALL_PLAYER_POSITION,
        0x000c_09d5,
        SCRATCH_BYTES,
        &[player, vec_out],
    );
    let pos = [
        f32_at(m, vec_out),
        f32_at(m, vec_out + 4),
        f32_at(m, vec_out + 8),
    ];

    m.write32(
        flag.wrapping_add(0x28),
        (m.read32(flag.wrapping_add(0x28)) & !0x80) | 2,
    );
    m.write32(
        flag.wrapping_add(0x18),
        m.read32(flag.wrapping_add(0x18)) | 2,
    );

    let base_team = base_ext.get(m, ext::TEAM);
    let player_team = Rec(player).get(m, ext::RECORD_TEAM);
    let mut returned = false;
    if player_team == base_team {
        if flag_ext.get(m, ext::TEAM) == base_team {
            returned = true;
            guest_call(
                c,
                m,
                cpu,
                CALL_RETURNED_HOME,
                0x000c_0ba4,
                SCRATCH_BYTES,
                &[],
            );
        } else {
            let slot = Rec(player).get(m, ext::RECORD_SLOT);
            let flag_team = flag_ext.get(m, ext::TEAM);
            let mut calls = PickupCalls {
                callee: c,
                mem: m,
                cpu,
            };
            calls.add_stat(STAT_FLAG_CAPTURE, slot, 1.0);
            calls.call_deep(CALL_PLAYER_SCORE_POINT, 0x000c_0a1d, 0x68, &[player, 1]);
            for &(entry, team) in entries(calls.mem).as_slice() {
                let handle = Rec(entry).get(calls.mem, ENTRY_HANDLE);
                if team == player_team {
                    calls.entry_flash(handle, 0x000c_0a6d);
                }
                if team == flag_team {
                    // The original indexes a u16 array in its own frame: frame+0x18 holds the four
                    // team-name texts, frame+0x16 is the high half of the spilled base extension
                    // pointer (team 0) and later words are other locals (read as found).
                    let frame = calls.cpu.esp.wrapping_sub(0x5c);
                    for (k, id) in TEAM_TEXTS.iter().enumerate() {
                        {
                            let at = frame.wrapping_add(0x18 + 2 * k as u32);
                            calls.mem.write8(at, *id as u8);
                            calls.mem.write8(at.wrapping_add(1), (*id >> 8) as u8);
                        }
                    }
                    let name = calls.mem.read16(
                        frame
                            .wrapping_add(0x16)
                            .wrapping_add(player_team.wrapping_mul(2)),
                    ) as i16 as i32 as u32;
                    let others = calls.text(TEXT_CAPTURE_OTHERS, 0x000c_0aad, SCRATCH_BYTES);
                    let named = calls.text(name, 0x000c_0ac2, 0x60);
                    let line =
                        calls.call(CALL_FORMAT, 0x000c_0ad0, &[CAPTURE_FORMAT, named, others]);
                    calls.call_deep(CALL_ENTRY_POST_TEXT, 0x000c_0ae0, 0x68, &[entry, line]);
                }
            }
            for &(entry, team) in entries(calls.mem).as_slice() {
                if team == player_team {
                    let handle = Rec(entry).get(calls.mem, ENTRY_HANDLE);
                    calls.entry_flash(handle, 0x000c_0b3d);
                    let text = calls.text(TEXT_CAPTURE_OWN, 0x000c_0b4c, 0x60);
                    calls.post_message(handle, text);
                }
            }
            calls.call(
                CALL_TEAM_SCORE,
                0x000c_0b92,
                &[base_team.wrapping_add(0x10)],
            );
        }
    }

    guest_call(
        c,
        m,
        cpu,
        CALL_EVENT_INIT,
        0x000c_0bae,
        SCRATCH_BYTES,
        &[event],
    );
    m.write32(event, SOUND_FLAG_DELIVER);
    m.write32(event + 0x1c, pos[0].to_bits());
    m.write32(event + 0x20, pos[1].to_bits());
    m.write32(event + 0x24, pos[2].to_bits());
    m.write8(event + 0x28, 1);
    guest_call(c, m, cpu, CALL_EVENT_POST, 0x000c_0be9, 0x60, &[event]);

    // Previous position saved, then the flag snaps to its home position and velocities.
    for (to, from) in [(0xec, 0x64), (0xf0, 0x68), (0xf4, 0x6c)] {
        m.write32(flag + to, m.read32(flag + from));
    }
    for (to, from) in [
        (0x64, 0x20),
        (0x68, 0x24),
        (0x6c, 0x28),
        (0xc0, 0x2c),
        (0x70, 0x30),
        (0xc8, 0x34),
    ] {
        m.write32(flag + to, flag_ext.get(m, from));
    }
    flag_ext.set(m, ext::CARRIER_SLOT, flag_ext.get(m, ext::HOME_SLOT));
    if !returned {
        cpu.esi = flag;
        guest_call(c, m, cpu, CALL_RESET_HOME, 0x000c_0c48, 0x50, &[flag]);
    }
    cpu.esi = flag;
}

/// 000C0C50 game_team_pickup_object_dropped_countdown_tick_then_return_home.
/// Register ABI: EAX = flag object. Only runs when the object is idle (`+0x78`, `+0x128` zero),
/// has an extension in STATE_DROPPED and a live `[[obj+4]+0x10]`. Counts `[ext+0xA8]` down by the
/// frame ticks. While positive the object drifts (positions `+0x64..`, `+0xC0`, `+0x70`, `+0xC8`
/// integrated by TICK_SCALE). At or below zero it returns home via the spawn lookup; for a flag
/// (SPAWN_MODE_FLAG) whose carrier slot resolves it also clears the table bit, forgets the
/// carrier and runs `deliver_flag`.
pub fn dropped_countdown_tick<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let saved = (cpu.ebx, cpu.esi, cpu.edi);
    dropped_countdown_tick_body(c, m, cpu);
    (cpu.ebx, cpu.esi, cpu.edi) = saved;
}

fn dropped_countdown_tick_body<M: GuestMemory, C: Callee<M>>(c: &mut C, m: &mut M, cpu: &mut Cpu) {
    let flag = cpu.eax;
    let rec = Rec(m.read32(flag.wrapping_add(0x7c)));
    if m.read32(flag.wrapping_add(0x78)) != 0
        || m.read32(flag.wrapping_add(0x128)) != 0
        || rec.0 == 0
    {
        return;
    }
    if rec.get(m, ext::STATE) != STATE_DROPPED {
        return;
    }
    let inner = m.read32(flag.wrapping_add(4));
    if m.read32(inner.wrapping_add(0x10)) == 0 {
        return;
    }
    let remaining = rec
        .get(m, ext::COUNTDOWN)
        .wrapping_sub(m.read32(FRAME_TICKS));
    cpu.edx = remaining;
    rec.set(m, ext::COUNTDOWN, remaining);
    if (remaining as i32) > 0 {
        let step = f32_at(m, TICK_SCALE);
        for (field, source, from_ext) in [
            (0x64, 0xb4, false),
            (0x68, 0xb8, false),
            (0x6c, 0xbc, false),
            (0xc0, ext::DROP_VEL, true),
            (0x70, ext::DROP_VEL + 4, true),
            (0xc8, ext::DROP_VEL + 8, true),
        ] {
            let velocity = if from_ext {
                f32_at(m, rec.0 + source)
            } else {
                f32_at(m, flag + source)
            };
            let value = velocity * step + f32_at(m, flag + field);
            m.write32(flag + field, value.to_bits());
        }
        return;
    }
    let mode = rec.get(m, ext::SPAWN_MODE);
    let param = rec.get(m, ext::SPAWN_PARAM);
    guest_call(
        c,
        m,
        cpu,
        CALL_SPAWN_LOOKUP,
        0x000c_0cbf,
        0xc,
        &[mode, param],
    );
    let spawn = cpu.eax;
    if mode == SPAWN_MODE_DROPPED_EFFECT {
        guest_call(
            c,
            m,
            cpu,
            CALL_EFFECT_AT,
            0x000c_0cda,
            0xc,
            &[SOUND_FLAG_DROPPED, flag + 0x64],
        );
        if m.read8(GAME_MODE) != MODE_NO_DROP_TIMER {
            m.write32(flag + 0x1a0, 1);
            m.write32(flag + 0x1a4, m.read32(TICK_COUNT).wrapping_mul(3) << 2);
        }
    }
    m.write32(flag + 0xc0, m.read32(spawn.wrapping_add(0x28)));
    m.write32(flag + 0xc8, m.read32(spawn.wrapping_add(0x30)));
    let carrier = rec.get(m, ext::CARRIER_INDEX);
    rec.set(m, ext::STATE, STATE_HOME);
    rec.set(m, ext::CARRIER_SLOT, carrier);
    if mode == SPAWN_MODE_DROPPED_EFFECT {
        guest_call(c, m, cpu, CALL_DROP_NOTIFY, 0x000c_0d33, 0x8, &[flag]);
    }
    for (to, from) in [
        (0x64, ext::DROP_POS),
        (0x68, ext::DROP_POS + 4),
        (0x6c, ext::DROP_POS + 8),
    ] {
        m.write32(flag + to, rec.get(m, from));
    }
    if mode != SPAWN_MODE_FLAG {
        return;
    }
    guest_call(
        c,
        m,
        cpu,
        CALL_PLAYER_BY_SLOT,
        0x000c_0d64,
        8,
        &[rec.get(m, ext::CARRIER_SLOT)],
    );
    let by_slot = cpu.eax;
    if by_slot == 0 {
        return;
    }
    let index = rec.get(m, ext::CARRIER_INDEX);
    if (index as i32) >= 0 {
        let row = m
            .read32(PICKUP_TABLE)
            .wrapping_add(index << PICKUP_ROW_SHIFT)
            .wrapping_add(PICKUP_ROW_FLAGS);
        m.write32(row, m.read32(row) & !PICKUP_ROW_CARRIED_BIT);
    }
    let record = rec
        .get(m, ext::RECORD_INDEX)
        .wrapping_mul(PLAYER_RECORD_STRIDE)
        .wrapping_add(m.read32(PLAYER_RECORD_BASE));
    rec.set(m, ext::CARRIER_INDEX, u32::MAX);
    cpu.eax = m.read32(by_slot.wrapping_add(4));
    cpu.ecx = record;
    cpu.esi = flag;
    guest_call(c, m, cpu, CALL_DELIVER, 0x000c_0db2, 8, &[]);
    cpu.esi = flag;
}

/// 000C11A0 game_team_pickup_object_touch_assign_team_or_deliver_when_carried.
/// Register ABI: EAX = base object. Stack cdecl(2): (toucher object, out flag pointer). Returns
/// EAX 1 in assign-team mode (MODE_ASSIGN_TEAM) and 0 otherwise; `*out = 1` whenever the touch
/// was handled. Assign-team mode copies the toucher's team into the base, publishes the team
/// bit mask at `[[obj+4]+0x30]` and, when the team changed, refreshes and dispatches the team
/// colour. Otherwise, when the toucher carries a flag of its own team's base and the carrier
/// check passes, the flag is delivered, fully reset and dropped from the carrier.
pub fn touch_assign_team_or_deliver<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    // The original saves and restores the callee-saved registers it uses.
    let saved = (cpu.ebx, cpu.esi, cpu.edi);
    touch_assign_team_or_deliver_body(c, m, cpu);
    (cpu.ebx, cpu.esi, cpu.edi) = saved;
}

fn touch_assign_team_or_deliver_body<M: GuestMemory, C: Callee<M>>(
    c: &mut C,
    m: &mut M,
    cpu: &mut Cpu,
) {
    let base = cpu.eax;
    let toucher = stack_arg(m, cpu, 0);
    let out = stack_arg(m, cpu, 1);
    let record = Rec(m.read32(toucher.wrapping_add(0x7c)));
    let base_ext = Rec(m.read32(base.wrapping_add(0x7c)));
    if m.read8(GAME_MODE) == MODE_ASSIGN_TEAM {
        let team = record.get(m, ext::RECORD_TEAM);
        let old_team = base_ext.get(m, ext::TEAM);
        base_ext.set(m, ext::TEAM, team);
        m.write32(out, 1);
        let inner = m.read32(base.wrapping_add(4));
        m.write32(inner.wrapping_add(0x30), 1u32 << (team & 31));
        if old_team != team {
            guest_call(c, m, cpu, CALL_TEAM_REFRESH, 0x000c_11f0, 0x14, &[base]);
            let count = m.read32(base.wrapping_add(0x1e8)) as i32;
            let list = m.read32(base.wrapping_add(0x1fc));
            let (color, pc) = if team == 1 && count > 0 {
                (m.read32(list), 0x000c_1215)
            } else if team != 1 && count > 1 {
                (m.read32(list.wrapping_add(4)), 0x000c_1233)
            } else {
                (m.read32(DEFAULT_TEAM_COLOR), 0x000c_1249)
            };
            guest_call(c, m, cpu, CALL_TEAM_COLOR, pc, 0x14, &[color]);
        }
        cpu.eax = 1;
        return;
    }
    cpu.eax = 0;
    let flag = record.get(m, ext::RECORD_CARRIED_FLAG);
    if flag == 0 || record.get(m, ext::RECORD_TEAM) != base_ext.get(m, ext::TEAM) {
        return;
    }
    guest_call(
        c,
        m,
        cpu,
        CALL_CARRIER_VALID,
        0x000c_127b,
        0x14,
        &[record.get(m, ext::OWNER_PLAYER)],
    );
    if cpu.eax == 0 {
        return;
    }
    let flag_ext = Rec(m.read32(flag.wrapping_add(0x7c)));
    cpu.eax = base;
    cpu.ecx = record.0;
    cpu.esi = flag;
    guest_call(c, m, cpu, CALL_DELIVER, 0x000c_129c, 0x14, &[]);
    cpu.esi = flag;
    let inner = m.read32(flag.wrapping_add(4));
    m.write32(flag + 0x28, m.read32(flag + 0x28) & !0x4000_0000);
    m.write32(flag + 0x78, 0);
    m.write32(flag + 0x128, 0);
    m.write32(flag + 0x12c, u32::MAX);
    m.write32(flag + 0x130, u32::MAX);
    for offset in [0xc0, 0x70, 0xc8] {
        m.write32(flag + offset, 0);
    }
    m.write8(flag + 0x32, 2);
    m.write32(inner.wrapping_add(0x30), 0);
    let inner = m.read32(flag.wrapping_add(4));
    guest_call(
        c,
        m,
        cpu,
        CALL_CLEAR_COLLISION,
        0x000c_12ed,
        0x14,
        &[inner, 0, 0],
    );
    let part_count = |m: &M| {
        let a = m.read32(flag.wrapping_add(4));
        m.read32(m.read32(a.wrapping_add(4)).wrapping_add(4)) as i32
    };
    let mut index = 0;
    let mut offset = 0u32;
    while index < part_count(m) {
        let parts = m.read32(m.read32(flag.wrapping_add(4)).wrapping_add(0x10));
        guest_call(
            c,
            m,
            cpu,
            CALL_RESET_PART,
            0x000c_130e,
            0x14,
            &[parts.wrapping_add(offset)],
        );
        index += 1;
        offset = offset.wrapping_add(0x40);
    }
    flag_ext.set(m, ext::STATE, STATE_HOME);
    record.set(m, ext::RECORD_CARRIED_FLAG, 0);
    guest_call(
        c,
        m,
        cpu,
        CALL_DROP_CARRIED,
        0x000c_1344,
        0x14,
        &[toucher, flag, 0],
    );
    m.write32(out, 1);
    cpu.eax = 0;
}
