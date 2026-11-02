//! Bodies of the four large per-level hooks of the level table `0x515FFC` (T1845, task C).
//!
//! Static reading of `build/default.xbe` (SHA-256 `3cfd001a...1816bc`), no xemu run. Control flow, compared
//! constants and tables are MEASURED; the game meaning of a helper is INFERRED (names from
//! `tools/data/function_names.csv`). Each hook is modelled as a pure function from a view of the world
//! (facts the original reads) to an ordered effect list (the calls and stores the original makes), so a
//! headless test can drive it. Details and open points: `docs/t-story-event-hooks-big.md`.
//!
//! | Hook | Level(s) | Rust |
//! |---|---|---|
//! | `0x20B630` | 6 (story struct) | [`level06_hook`] |
//! | `0x1F5A10` | 0xB (story struct) | [`level0b_hook`] |
//! | `0x213F50` (+ tail mixer `0x213C40`) | 0x11 (story struct) | [`level11_hook`], [`Level11Mixer`] |
//! | `0x1F6620` | 0x21 and 0x22 | [`strip_scroll_speed`] |

/// One side effect of a hook, in call order.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// `0x2274C0(slot, arg)`: finish the static slot (status 5, hud messages, callbacks).
    SlotFinish { slot: u32, arg: f32 },
    /// `0x227650(slot, arg)`: status 1 becomes 2 (INFERRED: `arg` is a time or amount).
    SlotPromote { slot: u32, arg: f32 },
    /// `0x227710(slot, arg)`: status becomes 2 unless it is 0 or 2.
    SlotActivate { slot: u32, arg: f32 },
    /// A call whose body is a separate function not decoded here (address, no arguments).
    CallSub { address: u32 },
    /// `0xBB6C0` after `0xB91F0`: `amount` of damage is committed on the tagged object (`+0x1CC` falls by `amount`), attacker
    /// local player 0's object (INFERRED roles of the two helpers).
    ObjectDamage { tag: u32, amount: f32 },
    /// `0x12C800` + `0x12C830`: bind the first player's object to a hud record (health thresholds) and, in
    /// level 6, store the object in the local player's word `+0xFF4`.
    BindHud { local_word_ff4: bool },
    /// `0xA5CE0(object, 3, function)`: install a state 3 tick function on a tagged object.
    SetStateTick { tag: u32, function: u32 },
    /// `0x9E720(object, function)`: store `function` at object `+0x19C`.
    SetField19c { tag: u32, function: u32 },
    /// Store to a game global.
    SetGlobal { address: u32, value: u32 },
    /// `0x468A0(handle, 0, id, volume, 50.0, pitch, flags)`: update or start a looping sound on a handle slot.
    SoundLoop {
        handle: u32,
        id: u32,
        volume: f32,
        pitch: f32,
        flags: u32,
    },
    /// Stop the sound handle stored in a global (`0x45460`) and store -1 there.
    SoundStop { handle: u32 },
    /// `0xB0DB0(prop, bits)`: clear bits of the prop extension flags (`[[obj+0x7C]+0xC]`).
    PropClearFlags { prop: u32, bits: u32 },
    /// `0xB2330(prop)`: restart the prop extension state machine unless flag `0x1000` with state 1.
    PropRestart { prop: u32 },
    /// `0xB0D90(prop, bits)`: set bits of the prop extension flags.
    PropSetFlags { prop: u32, bits: u32 },
    /// Store a float into a field of the tagged object.
    SetObjectFloat { tag: u32, offset: u32, value: f32 },
    /// Store the camera's `+0x27C` dword into the tagged object's `+0x64`.
    CopyCameraToObject { tag: u32 },
    /// `0x119C40(flag)`.
    SetFlag119c40 { flag: bool },
    /// `0x10E6C0(handle, value)` on the handle stored at `[0x78E468]`.
    SetHandleValue { value: u32 },
    /// Tail jump out of the hook (the rest of the hook is not run).
    TailCall { address: u32 },
}

// ---------------------------------------------------------------------------------------------------------
// Character id resolution and the shared tables
// ---------------------------------------------------------------------------------------------------------

/// Resolves a table key to the character entry id (`0x9CC40`: binary search of the table at `[0x715F2C]`).
/// Hooks compare the entry id stored in a player's `+0x678` with the result.
pub trait KeyResolver {
    fn entry_id(&self, key: u32) -> u32;
}

impl<F: Fn(u32) -> u32> KeyResolver for F {
    fn entry_id(&self, key: u32) -> u32 {
        self(key)
    }
}

/// Player view shared by the hooks: the fields the original reads from a player record (stride 0x1584 at
/// `[0x7B0C48]`, the player object at record `+0x14`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerView {
    /// Object `+0x1CC` is above 0.0 (`[0x475CAC]`).
    pub alive: bool,
    /// `[[object+0x7C]+0x678]`: character entry id.
    pub character: u32,
    /// Player record `+0xF54`: the player state id.
    pub state: u32,
}

// ---------------------------------------------------------------------------------------------------------
// Level 6 (hook 0x20B630)
// ---------------------------------------------------------------------------------------------------------

/// Static slot facts (`0x227400` status above 1, `0x2273C0` status 5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SlotFacts {
    pub above_one: bool,
    pub finished: bool,
}

/// Facts the level 6 hook reads.
#[derive(Clone, Debug, Default)]
pub struct Level06View {
    /// `[0x7DE455]` (mode id) and `[0x790950]` (player count).
    pub mode_id: u8,
    pub player_count: usize,
    /// Slot 9 is status 5.
    pub slot9_finished: bool,
    /// Per player: player record `+0x570` equals 1 (MEASURED compare).
    pub player_flag_570: Vec<bool>,
    /// Slot 4, slot 2 and slot 0xA.
    pub slot4: SlotFacts,
    pub slot2: SlotFacts,
    pub slot_a_finished: bool,
    /// `[0x7B0C7C]` local player is non-null, and its word `+0xFF4` equals the first player's object.
    pub local_player: bool,
    pub local_ff4_matches_first: bool,
    /// Per player: record `+0xF54` and record `+0x1568`.
    pub states: Vec<u32>,
    pub words_1568: Vec<i32>,
    /// A "CDWD" prop (net table `0x732A04`, key `0x43445744`) exists and `0xB0F60` (field `+0x10` is 1 or 2).
    pub cdwd_active: bool,
    /// A non-player object whose `+0xBF0` record kind is "CPTB" (`0x54504342`) exists.
    pub cptb_present: bool,
    /// `9A4C0(cptb +0x678)` equals `0x549`, and the low bit of its flag dword (`+0x10` in mode 0xA else `+0x18`).
    pub cptb_key_is_549: bool,
    pub cptb_flag_bit0: bool,
}

/// Fixed constants of the level 6 hook.
pub const LEVEL06_CPTB_KIND: u32 = 0x5450_4342;
pub const LEVEL06_CDWD_KEY: u32 = 0x4344_5744;
pub const LEVEL06_PROP_CLEAR: u32 = 0x4000;
pub const LEVEL06_PROP_SET: u32 = 2;

/// `0x20B630`, MEASURED control flow. Order: slot 9 sweep, `0x20B480`, the hud binding for the first player
/// (slot 4 or slot 2 path, skipped for several players in mode 0xA), `0x2085B0`, then the per player loop
/// (state 0x25 or 0x26 ends the hook with a tail call to `0x208810`; a player with `+0x1568` in 9..=10 drives
/// the "CDWD" prop).
pub fn level06_hook(view: &Level06View) -> Vec<Effect> {
    let mut out = Vec::new();
    if !view.slot9_finished {
        for index in 0..view.player_count {
            if view.player_flag_570.get(index).copied().unwrap_or(false) {
                out.push(Effect::SlotFinish { slot: 9, arg: 0.0 });
            }
        }
    }
    out.push(Effect::CallSub { address: 0x20B480 });
    let solo = !(view.mode_id == 0x0A && view.player_count > 1);
    if solo {
        if view.slot4.above_one && view.local_player {
            // Slot 4 path (the first object is dereferenced unchecked by the original).
            if view.slot4.finished {
                out.push(Effect::SetGlobal {
                    address: 0x7B0C7C + 0xFF4,
                    value: 0,
                });
            } else if !view.local_ff4_matches_first {
                out.push(Effect::BindHud {
                    local_word_ff4: true,
                });
            }
        } else if view.slot2.above_one && view.local_player {
            if view.slot_a_finished {
                out.push(Effect::SetGlobal {
                    address: 0x7B0C7C + 0xFF4,
                    value: 0,
                });
            } else {
                if !view.local_ff4_matches_first {
                    out.push(Effect::BindHud {
                        local_word_ff4: true,
                    });
                }
                out.push(Effect::CallSub { address: 0x208AF0 });
            }
        }
    }
    out.push(Effect::CallSub { address: 0x2085B0 });
    for index in 0..view.player_count {
        let state = view.states.get(index).copied().unwrap_or(0);
        if state == 0x25 || state == 0x26 {
            out.push(Effect::TailCall { address: 0x208810 });
            return out;
        }
        let word = view.words_1568.get(index).copied().unwrap_or(0);
        if (9..=10).contains(&word) && view.cdwd_active && view.cptb_present {
            let skip = view.cptb_key_is_549 && !view.cptb_flag_bit0;
            if !skip {
                out.push(Effect::PropClearFlags {
                    prop: LEVEL06_CDWD_KEY,
                    bits: LEVEL06_PROP_CLEAR,
                });
                out.push(Effect::PropRestart {
                    prop: LEVEL06_CDWD_KEY,
                });
                out.push(Effect::PropSetFlags {
                    prop: LEVEL06_CDWD_KEY,
                    bits: LEVEL06_PROP_SET,
                });
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------
// Level 0xB (hook 0x1F5A10)
// ---------------------------------------------------------------------------------------------------------

/// Tag keys used by the hook (ASCII, big endian reading of the four characters).
pub mod tags {
    pub const OILY: u32 = 0x4F49_4C59;
    pub const OILX: u32 = 0x4F49_4C58;
    pub const MAC1: u32 = 0x4D41_4331;
    pub const GAS0: u32 = 0x4741_5330;
    pub const GUY1: u32 = 0x4755_5931;
    pub const GUY2: u32 = 0x4755_5932;
    pub const GUY3: u32 = 0x4755_5933;
    pub const DUMB: u32 = 0x4455_4D42;
    pub const WALK: u32 = 0x5741_4C4B;
    pub const WILK: u32 = 0x5749_4C4B;
    pub const HIDE: u32 = 0x4849_4445;
    pub const WIND: u32 = 0x5749_4E44;
    pub const HTBJ: u32 = 0x4A42_5448;
}

/// Player states that count as "in the oil room action" for the music flag (`ebp`), loop part.
pub const LEVEL0B_ACTION_STATES: [u32; 6] = [0x4C, 0x4B, 0x53, 0x50, 0x4F, 0x4E];
/// Local player states that run `0x1ECA60` + `0x1F1F70`.
pub const LEVEL0B_EXTENDED_STATES: [u32; 17] = [
    0x4C, 0x4B, 0x53, 0x50, 0x4F, 0x4E, 0x55, 0x56, 0x57, 0x58, 0x59, 0x4D, 0x46, 0x47, 0x48, 0x49,
    0x4A,
];

/// Table keys compared in the per player loop: `(key, effect)`.
pub const LEVEL0B_KEY_PROMOTE_9: u32 = 0x5EB;
pub const LEVEL0B_KEY_FLAG: u32 = 0x632;
pub const LEVEL0B_KEY_PROMOTE_7: u32 = 0x5E7;
pub const LEVEL0B_KEY_OILY_FINISH: u32 = 0x58C;
/// Entry key (`[0x715F2C + id*0x20]`) of the first player's character tested for states 2 and 0x4B.
pub const LEVEL0B_ENTRY_KEY_STATE2: u32 = 0x404;
pub const LEVEL0B_ENTRY_KEY_STATE4B: u32 = 0x558;

/// Mutable globals of level 0xB (BSS).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Level0bGlobals {
    /// `[0x75BFE4]`: set when the 0x632 character is alive.
    pub flag_e4: u32,
    /// `[0x75BFE8]`, `[0x75BFEC]` (sound volume source), `[0x75BF0C]`.
    pub flag_e8: u32,
    pub volume_ec: f32,
    pub flag_0c: u32,
    /// `[0x4FFD5C]`: volume source of the first looping sound; `[0x4FFE64]` sound handle (-1 none).
    pub volume_5c: f32,
    pub handle_e64: i32,
}

/// Facts read by hook 0xB.
#[derive(Clone, Debug, Default)]
pub struct Level0bView {
    pub players: Vec<PlayerView>,
    /// Local player `[0x7B0C7C]` state (`+0xF54`), `None` when null.
    pub local_state: Option<u32>,
    /// `0x277FE0()`: the first player's character entry id and its entry key, `None` when null.
    pub first_entry_key: Option<u32>,
    /// Tags that exist (`0x15BA10` finds a net slot object).
    pub tags_present: Vec<u32>,
    /// `GAS0` object word `+0x34` equals 0x50.
    pub gas0_state_is_50: bool,
    /// `WIND` object present, camera `+0x27C` bits, first player `+0x1548 == 0x14`.
    pub wind_present: bool,
    pub first_player_1548_is_14: bool,
    /// Camera `+0x280` is above 5.2 (`[0x49E3D8]`).
    pub camera_height_above: bool,
    /// `[0x78E468]` non-null, `[[0x7B977C]+0x5B..0x5E]` any non-zero, camera `+0x2A0` is 0x58 or 0x57.
    pub handle_present: bool,
    pub input_bytes_any: bool,
    pub camera_mode_is_57_or_58: bool,
}

impl Level0bView {
    fn tag(&self, key: u32) -> bool {
        self.tags_present.contains(&key)
    }
}

/// `0x1F5A10`, MEASURED control flow (see the module table for the helper roles).
pub fn level0b_hook(
    view: &Level0bView,
    globals: &mut Level0bGlobals,
    resolver: &dyn KeyResolver,
) -> Vec<Effect> {
    let mut out = vec![Effect::CallSub { address: 0x1EA640 }];
    let mut action = false;
    for player in &view.players {
        if !player.alive {
            continue;
        }
        if player.character == resolver.entry_id(0x5EB) {
            out.push(Effect::SlotPromote { slot: 1, arg: 9.0 });
        }
        if player.character == resolver.entry_id(LEVEL0B_KEY_FLAG) {
            globals.flag_e4 = 1;
            out.push(Effect::SetGlobal {
                address: 0x75BFE4,
                value: 1,
            });
        }
        if player.character == resolver.entry_id(0x5EB) {
            out.push(Effect::SlotPromote { slot: 3, arg: 13.0 });
        }
        if player.character == resolver.entry_id(LEVEL0B_KEY_PROMOTE_7) {
            out.push(Effect::SlotPromote { slot: 0, arg: 7.0 });
        }
        if player.character == resolver.entry_id(LEVEL0B_KEY_OILY_FINISH) && view.tag(tags::OILY) {
            out.push(Effect::SlotFinish {
                slot: 0x17,
                arg: 0.0,
            });
        }
        if !action && LEVEL0B_ACTION_STATES.contains(&player.state) {
            action = true;
        }
    }
    if action {
        out.push(Effect::CallSub { address: 0x1EC300 });
    } else if globals.handle_e64 >= 0 {
        out.push(Effect::SoundStop { handle: 0x4FFE64 });
        globals.handle_e64 = -1;
    }
    if let Some(state) = view.local_state {
        if state == 0x4C {
            out.push(Effect::CallSub { address: 0x1F1EC0 });
        }
        if LEVEL0B_EXTENDED_STATES.contains(&state) {
            out.push(Effect::CallSub { address: 0x1ECA60 });
            out.push(Effect::CallSub { address: 0x1F1F70 });
        }
        if state == 2
            && view.first_entry_key == Some(LEVEL0B_ENTRY_KEY_STATE2)
            && !view.tag(tags::OILX)
        {
            out.push(Effect::SlotFinish {
                slot: 0x1B,
                arg: 0.0,
            });
        }
        if globals.flag_e4 == 1 && state == 1 && !view.tag(tags::OILY) {
            out.push(Effect::SlotFinish {
                slot: 0x0D,
                arg: 0.0,
            });
        }
        if state == 5 {
            if !view.tag(tags::MAC1) {
                out.push(Effect::SlotFinish { slot: 8, arg: 0.0 });
            }
            out.push(Effect::CallSub { address: 0x1EFE90 });
        }
        if state == 0x4B && globals.flag_e8 == 1 {
            if !view.tag(tags::GAS0) {
                if globals.flag_0c != 0 {
                    out.push(Effect::SlotFinish {
                        slot: 0x1D,
                        arg: 0.0,
                    });
                }
            } else if view.gas0_state_is_50 && globals.flag_0c == 0 {
                out.push(Effect::SlotFinish {
                    slot: 0x2A,
                    arg: 0.0,
                });
            }
        }
        if state == 0x4B && view.first_entry_key == Some(LEVEL0B_ENTRY_KEY_STATE4B) {
            if globals.flag_0c != 0 {
                out.push(Effect::SlotActivate {
                    slot: 0x1F,
                    arg: 0.0,
                });
            } else {
                out.push(Effect::SlotFinish {
                    slot: 0x1F,
                    arg: 0.0,
                });
            }
        }
        if state == 0x44 {
            out.push(Effect::CallSub { address: 0x1EFE30 });
        }
        if state == 0x2B && !view.tag(tags::GUY1) && !view.tag(tags::GUY2) && !view.tag(tags::GUY3)
        {
            out.push(Effect::SlotFinish {
                slot: 0x14,
                arg: 0.0,
            });
        }
        if (state == 2 || state == 3)
            && !view.tag(tags::DUMB)
            && !view.tag(tags::WALK)
            && !view.tag(tags::WILK)
            && !view.tag(tags::HIDE)
        {
            out.push(Effect::SlotFinish {
                slot: 0x15,
                arg: 0.0,
            });
        }
        if state == 0x4D || state == 0x4A || state == 0x51 {
            out.push(Effect::CallSub { address: 0x1F0290 });
        }
    }
    out.push(Effect::CallSub { address: 0x1EAB10 });
    out.push(Effect::CallSub { address: 0x1F2810 });
    out.push(Effect::SoundLoop {
        handle: 0x50025C,
        id: 4,
        volume: globals.volume_5c * 0.01,
        pitch: 1.0,
        flags: 0,
    });
    out.push(Effect::SoundLoop {
        handle: 0x500260,
        id: 0x3A2,
        volume: globals.volume_ec * 0.01,
        pitch: 1.0,
        flags: 0,
    });
    out.push(Effect::CallSub { address: 0x1F48A0 });
    if view.wind_present {
        out.push(Effect::CopyCameraToObject { tag: tags::WIND });
        let wind = if view.first_player_1548_is_14 {
            -12.5
        } else {
            0.0
        };
        out.push(Effect::SetObjectFloat {
            tag: tags::WIND,
            offset: 0x6C,
            value: wind,
        });
    }
    out.push(Effect::SetFlag119c40 {
        flag: view.camera_height_above,
    });
    if view.handle_present {
        let value = if view.input_bytes_any && !view.camera_mode_is_57_or_58 {
            0xFFFF
        } else {
            0
        };
        out.push(Effect::SetHandleValue { value });
    }
    out
}

// ---------------------------------------------------------------------------------------------------------
// Level 0x11 (hook 0x213F50, tail mixer 0x213C40, class function 0x210470, slot init 0x214220)
// ---------------------------------------------------------------------------------------------------------

/// Slot finish pairs: `(slot, key a, key b)`; the character matching either finishes the slot (MEASURED).
pub const LEVEL11_SLOT_KEYS: [(u32, u32, u32); 5] = [
    (4, 0x43A, 0x456),
    (5, 0x43C, 0x458),
    (6, 0x43E, 0x45B),
    (7, 0x43F, 0x45C),
    (8, 0x441, 0x45E),
];
/// Key whose character activates slots 4 to 8 instead (`0x227710`).
pub const LEVEL11_KEY_ACTIVATE_ALL: u32 = 0x448;
/// Key whose character runs `0x210270` (promote slot 1 with 3.0 and bind the hud once per level).
pub const LEVEL11_KEY_BIND: u32 = 0x3F4;
/// Tick functions installed in state 3 of the tagged objects (`[object+0x160] == 0`).
pub const LEVEL11_HTBJ_TICK: u32 = 0x2106A0;
pub const LEVEL11_JANI_TICK: u32 = 0x210770;
pub const LEVEL11_HGZ_TICK: u32 = 0x2102D0;
pub const LEVEL11_HGZ_FIELD19C: u32 = 0x2103C0;
pub const LEVEL11_TAG_HTBJ: u32 = 0x4A42_5448;
pub const LEVEL11_TAG_JANI: u32 = 0x4A41_4E49;
pub const LEVEL11_TAG_HGZ1: u32 = 0x4847_5A31;
pub const LEVEL11_TAG_HGZ2: u32 = 0x4847_5A32;

/// Facts read by hook 0x11.
#[derive(Clone, Debug, Default)]
pub struct Level11View {
    pub players: Vec<PlayerView>,
    pub mode_id: u8,
    pub player_count: usize,
    pub local_state: Option<u32>,
    /// Tagged objects that exist, with whether `+0x160` (the tick slot word) is zero.
    pub htbj: Option<bool>,
    pub jani: Option<bool>,
    pub hgz1: Option<bool>,
    pub hgz2: Option<bool>,
    /// `[0x75C0C4]`: set by the level init, gates HGZ1.
    pub hgz_gate: bool,
    pub slot1: SlotFacts,
    /// `0x277FE0()` non-null and `[0x75C0B0]+0x2C == 0` (the one-shot of `0x210270`).
    pub first_player: bool,
    pub bind_done: bool,
}

/// `0x213F50` (MEASURED). `resolver` maps the table keys. The hook ends with a tail jump to the mixer `0x213C40`.
pub fn level11_hook(view: &Level11View, resolver: &dyn KeyResolver) -> Vec<Effect> {
    let mut out = Vec::new();
    let mut bind_done = view.bind_done;
    for player in view.players.iter().filter(|p| p.alive) {
        if player.character == resolver.entry_id(LEVEL11_KEY_BIND) {
            out.push(Effect::SlotPromote { slot: 1, arg: 3.0 });
            if view.first_player && !bind_done {
                out.push(Effect::BindHud {
                    local_word_ff4: true,
                });
                bind_done = true;
            }
        }
        for &(slot, key_a, key_b) in &LEVEL11_SLOT_KEYS {
            if player.character == resolver.entry_id(key_a)
                || player.character == resolver.entry_id(key_b)
            {
                out.push(Effect::SlotFinish { slot, arg: 0.0 });
            }
        }
        if player.character == resolver.entry_id(LEVEL11_KEY_ACTIVATE_ALL) {
            for slot in 4..=8 {
                out.push(Effect::SlotActivate { slot, arg: 0.0 });
            }
        }
    }
    if view.htbj == Some(true) {
        out.push(Effect::SetStateTick {
            tag: LEVEL11_TAG_HTBJ,
            function: LEVEL11_HTBJ_TICK,
        });
    }
    if view.jani == Some(true) {
        out.push(Effect::SetStateTick {
            tag: LEVEL11_TAG_JANI,
            function: LEVEL11_JANI_TICK,
        });
    }
    if view.hgz_gate && view.hgz1.is_some() {
        if view.hgz1 == Some(true) {
            out.push(Effect::SetStateTick {
                tag: LEVEL11_TAG_HGZ1,
                function: LEVEL11_HGZ_TICK,
            });
        }
        out.push(Effect::SetField19c {
            tag: LEVEL11_TAG_HGZ1,
            function: LEVEL11_HGZ_FIELD19C,
        });
    }
    if view.hgz2.is_some() {
        if view.hgz2 == Some(true) {
            out.push(Effect::SetStateTick {
                tag: LEVEL11_TAG_HGZ2,
                function: LEVEL11_HGZ_TICK,
            });
        }
        out.push(Effect::SetField19c {
            tag: LEVEL11_TAG_HGZ2,
            function: LEVEL11_HGZ_FIELD19C,
        });
    }
    let solo = !(view.mode_id == 0x0A && view.player_count > 1);
    if solo && view.local_state.is_some() && view.slot1.above_one && !view.slot1.finished {
        out.push(Effect::CallSub { address: 0x213900 });
    }
    if view.local_state == Some(0x12) {
        out.push(Effect::CallSub { address: 0x213580 });
    }
    out.push(Effect::TailCall { address: 0x213C40 });
    out
}

/// `0x210470`: the music intensity class of a player's object state (`state - 1` indexes a 98 byte class table,
/// the class a jump table of constants). Returns `(level, finishes slot 0xB)`; `None` outside the table.
pub fn level11_state_intensity(state: u32) -> (f32, bool) {
    const CLASS: [u8; 98] = [
        0, 1, 2, 1, 2, 2, 2, 3, 4, 5, 6, 6, 9, 9, 9, 9, 9, 9, 2, 2, 9, 9, 9, 9, 9, 9, 9, 0, 7, 1,
        2, 1, 0, 7, 7, 2, 2, 0, 1, 9, 6, 6, 2, 9, 9, 2, 2, 2, 9, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
        2, 2, 2, 2, 2, 0, 9, 9, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 9, 2, 2, 2, 2, 2, 2,
        2, 2, 8, 2, 1, 9, 2, 2,
    ];
    let index = state.wrapping_sub(1) as usize;
    let class = if index < CLASS.len() { CLASS[index] } else { 9 };
    match class {
        0 => (0.5, false),
        1 => (0.75, false),
        2 => (1.0, false),
        3 => (0.8, false),
        4 => (0.6, false),
        5 => (0.4, false),
        6 => (0.2, false),
        7 => (0.25, false),
        8 => (1.0, true),
        _ => (-0.1, false),
    }
}

/// `0x1515E0(value, target, step)`: move `value` toward `target` by at most `step`; true when it arrived.
pub fn approach(value: &mut f32, target: f32, step: f32) -> bool {
    if (target - *value).abs() < step {
        *value = target;
        return true;
    }
    if target > *value {
        *value += step;
    } else {
        *value -= step;
    }
    false
}

/// The shout table at `0x506B78`: `(character entry id key, sound id, volume)`, 11 rows (MEASURED, the
/// entry id is already a resolved id here, see the note in the doc).
pub const LEVEL11_SHOUTS: [(u32, u32, f32); 11] = [
    (0x41A, 0x56F, 0.5),
    (0x509, 0x570, 1.0),
    (0x50A, 0x571, 0.9),
    (0x50C, 0x56F, 0.8),
    (0x50F, 0x571, 0.5),
    (0x510, 0x570, 0.6),
    (0x512, 0x56F, 0.8),
    (0x514, 0x571, 1.0),
    (0x516, 0x56F, 1.0),
    (0x432, 0x570, 0.8),
    (0x435, 0x571, 0.9),
];

/// The two level 0x11 mixer values and the speaker follow state (`[0x75C0B0]` block).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Level11Mixer {
    /// `+0x00`: music intensity (sound id 6 on handle `+0x04`).
    pub intensity: f32,
    /// `+0x08`: speaker activity (sound id 0x56E on handle `+0x0C`).
    pub activity: f32,
    /// `+0x18` counter and `+0x1C..+0x24` last speaker position, `+0x28` next shout row.
    pub counter: u32,
    pub last_position: [f32; 3],
    pub shout_row: usize,
}

impl Default for Level11Mixer {
    fn default() -> Self {
        Self {
            intensity: 0.0,
            activity: 0.0,
            counter: 0,
            last_position: [0.0; 3],
            shout_row: 0,
        }
    }
}

/// Inputs of one mixer tick.
#[derive(Clone, Debug)]
pub struct Level11MixerInput {
    /// `[0x4E7944]` frame multiplier (0 returns at once) and `[0x4E794C]` float multiplier.
    pub steps: i32,
    pub step_scale: f32,
    /// `0x23EF00()` current cutscene id; -1 means gameplay (the intensity target comes from the players).
    pub cutscene_id: i32,
    /// Object states of the active players (record `+4 != -1`).
    pub player_states: Vec<u32>,
    /// The speaker object (`+0x14`): position and the character entry id of its `+0x678`, `None` when null.
    pub speaker: Option<(u32, [f32; 3])>,
}

/// Result of one mixer tick.
#[derive(Clone, Debug, PartialEq)]
pub struct Level11MixerOutput {
    pub effects: Vec<Effect>,
    /// A shout was requested: `(sound id, volume)` via the `0x45820` request, position in the `0x45390` packet.
    pub shout: Option<(u32, f32)>,
}

/// Cutscene speed constant for the two characters with a fixed speed (entry keys 0x435 and 0x584): 0.45.
pub const LEVEL11_FIXED_SPEED: f32 = 0.45;

/// `0x213C40`. The intensity chases the maximum class level of the players (or -0.1 in a cutscene) at
/// `1/60 * step_scale * 1.5` per call; the activity chases the speaker speed clamped to [-0.1, 1.0] at
/// `1/60 * step_scale * 2.5`, with volume `1.5 * activity` and pitch `0.5 * activity + 0.75`. A speaker that
/// lasted more than 3 calls and matches the current shout row plays that row's sound and advances the row.
pub fn level11_mixer_tick(
    state: &mut Level11Mixer,
    input: &Level11MixerInput,
) -> Level11MixerOutput {
    let mut out = Level11MixerOutput {
        effects: Vec::new(),
        shout: None,
    };
    if input.steps == 0 {
        return out;
    }
    let rate = 1.0f32 / 60.0;
    let mut target = -0.1f32;
    if input.cutscene_id == -1 {
        for &object_state in &input.player_states {
            let (level, _) = level11_state_intensity(object_state);
            if level > target {
                target = level;
            }
        }
    }
    approach(&mut state.intensity, target, rate * input.step_scale * 1.5);
    out.effects.push(Effect::SoundLoop {
        handle: 0x75C0B4,
        id: 6,
        volume: state.intensity,
        pitch: 1.0,
        flags: 0,
    });
    let mut value = -0.1f32;
    if let Some((character, position)) = input.speaker {
        if state.counter > 3 {
            let mut speed = LEVEL11_FIXED_SPEED;
            if character != 0x435 && character != 0x584 {
                let inverse = 1.0 / input.step_scale;
                let delta = [
                    (position[0] - state.last_position[0]) * inverse,
                    (position[1] - state.last_position[1]) * inverse,
                    (position[2] - state.last_position[2]) * inverse,
                ];
                speed = (delta[0] * delta[0] + delta[1] * delta[1] + delta[2] * delta[2]).sqrt()
                    * (1.0 / 0.56);
            }
            value = speed;
            if let Some(&(row_character, sound, volume)) = LEVEL11_SHOUTS.get(state.shout_row) {
                if row_character == character {
                    out.shout = Some((sound, volume));
                    state.shout_row += 1;
                }
            }
        }
        state.counter += 1;
        state.last_position = position;
        value = value.clamp(-0.1, 1.0);
    }
    approach(&mut state.activity, value, rate * input.step_scale * 2.5);
    out.effects.push(Effect::SoundLoop {
        handle: 0x75C0BC,
        id: 0x56E,
        volume: state.activity * 1.5,
        pitch: state.activity * 0.5 + 0.75,
        flags: 1,
    });
    out
}

/// Slot declarations of the level 0x11 init (`0x214220`, MEASURED): `(slot, text id, a, b, c)` via `0x227850`.
/// Slot 1 is declared only outside a multi player mode 0xA game. Slots 4 to 0xB have no text and `2, 2, 2`.
pub fn level11_slot_declarations(mode_id: u8, player_count: usize) -> Vec<(u32, u32, [u32; 3])> {
    let mut out = vec![(0, 0x12D0, [0; 3])];
    if !(mode_id == 0x0A && player_count > 1) {
        out.push((1, 0x12D1, [0; 3]));
    }
    out.push((2, 0x12D2, [0; 3]));
    out.push((3, 0x12D3, [0; 3]));
    for slot in 4..=0xB {
        out.push((slot, 0, [2; 3]));
    }
    out
}

// ---------------------------------------------------------------------------------------------------------
// Levels 0x21 and 0x22 (hook 0x1F6620)
// ---------------------------------------------------------------------------------------------------------

/// The scroll speed `[0x4FFE50]` the hook stores before running the strip update `0x1F2810`: only when the
/// level id `[0x79094C]` is 0x22 and `[0x70E164]` is non-null: 0.0 when its word `+0xC` is in `0x9DF..=0xA95`,
/// else 4.0 (`[0x475DB4]`). `None` means the store is skipped (speed unchanged). The hook always ends with a
/// tail call to `0x1F2810`.
pub fn strip_scroll_speed(level_id: u32, record_word_c: Option<i32>) -> Option<f32> {
    if level_id != 0x22 {
        return None;
    }
    let word = record_word_c?;
    Some(if (0x9DF..=0xA95).contains(&word) {
        0.0
    } else {
        4.0
    })
}

// ---------------------------------------------------------------------------------------------------------
// Round 2 callees (T1845): 0x208AF0 and 0x1EA640
// ---------------------------------------------------------------------------------------------------------

/// `0x208AF0` (level 6 callee, register convention: eax = object): the fade of the object extension word `+0x1CC`.
/// The rate depends on the byte `[0x7DE457]`: 0 gives 0.038, 1 gives 0.045, 2 gives 0.058, anything else 1.0
/// (MEASURED constants). `fade -= (1 / rate_hz) * frame_multiplier * rate`. Returns `(new_fade, expired)`;
/// `expired` (fade below 0) is when the original calls `0xBD090(extension)` (INFERRED: destroy or finish).
pub fn level06_fade_step(fade: f32, mode_byte: u8, frame_multiplier: i32) -> (f32, bool) {
    let rate = match mode_byte {
        0 => 0.038_f32,
        1 => 0.045_f32,
        2 => 0.058_f32,
        _ => 1.0_f32,
    };
    let step = (1.0_f32 / 60.0) * frame_multiplier as f32 * rate;
    let next = fade - step;
    (next, next < 0.0)
}

/// `0x1EA640` (level 0xB callee): target intensities of the music mixer from the per-player state word
/// (`player + 0xF54`). Returns `(primary_target, secondary_target)`, the two values then approached by
/// `frame_multiplier` per call at globals `0x4FFD5C` and `0x75BFEC` ([`approach`]). With game flag `0x200` set
/// or no players both targets are 0. Class map (97 states, MEASURED): `state-1` is looked up in a byte map to a
/// class 0..5: 0 and 5 and out of range give 100, 1 gives 35, 2 gives 50 and sets the secondary to 100,
/// 3 gives 75, 4 gives 50. The primary is the maximum over players.
pub fn level0b_music_targets(game_flags: u32, player_states: &[u32]) -> (i32, i32) {
    const MAP: [u8; 97] = [
        0, 0, 0, 0, 1, 0, 5, 0, 2, 2, 2, 2, 2, 2, 0, 5, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        5, 0, 5, 5, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 0, 5, 0, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4,
        4, 4, 3, 5, 5, 5, 0, 3, 0, 4, 4, 4, 4, 4, 1, 1, 1, 4, 4, 1, 1, 4, 4, 4, 4, 4, 3, 4, 0, 5,
        0, 0, 0, 5, 4, 4, 0,
    ];
    if game_flags & 0x200 != 0 {
        return (0, 0);
    }
    let (mut primary, mut secondary) = (0, 0);
    for &state in player_states {
        let index = state.wrapping_sub(1);
        let (value, second) = if index > 0x60 {
            (100, 0)
        } else {
            match MAP[index as usize] {
                1 => (35, 0),
                2 => (50, 100),
                3 => (75, 0),
                4 => (50, 0),
                _ => (100, 0),
            }
        };
        primary = primary.max(value);
        if second != 0 {
            secondary = second;
        }
    }
    (primary, secondary)
}

// ---------------------------------------------------------------- round 4: small level 0xB callees and the call registry

/// "HITP" (`0x48544950`).
pub const TAG_HITP: u32 = 0x4854_4950;

/// Inputs of `0x1ECA60`.
#[derive(Clone, Copy, Debug)]
pub struct HitpView {
    /// "HITP" object state dword `+0x34`, `None` when absent.
    pub hitp_state: Option<u32>,
    /// Player 0 block word `+0xF54`.
    pub player_area: u32,
    /// `[0x75BFF4]` is non-zero.
    pub flag_f4: bool,
}

/// Damage taken by "HITP" each time `0x1ECA60` fires (`[0x47D3B0]`, read from the image).
pub const HITP_DAMAGE: f32 = 0.002;

/// `0x1ECA60` (level 0xB, run in the 17 extended states, MEASURED): when "HITP" exists in state `0x4C`, player 0 is not in area
/// `0x4C` and `[0x75BFF4]` is set, "HITP" takes [`HITP_DAMAGE`] damage, slot `0xF` finishes and slot `0x10` activates.
pub fn level0b_hitp_damage(view: &HitpView) -> Vec<Effect> {
    match view.hitp_state {
        Some(0x4C) if view.player_area != 0x4C && view.flag_f4 => vec![
            Effect::ObjectDamage {
                tag: TAG_HITP,
                amount: HITP_DAMAGE,
            },
            Effect::SlotFinish {
                slot: 0xF,
                arg: 0.0,
            },
            Effect::SlotActivate {
                slot: 0x10,
                arg: 0.0,
            },
        ],
        _ => Vec::new(),
    }
}

/// `0x1EFE30` (level 0xB state `0x44`, MEASURED): when none of "JUM1", "JUM2", "JUM3" and "BJUM" exists slot 7 finishes (arg 5.0).
pub fn level0b_jum_check(present: [bool; 4]) -> Vec<Effect> {
    if present.iter().any(|&p| p) {
        Vec::new()
    } else {
        vec![Effect::SlotFinish { slot: 7, arg: 5.0 }]
    }
}

/// Where a `CallSub` callee stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubStatus {
    /// Ported: the model function that implements it.
    Ported(&'static str),
    /// Decoded as a render or world update with no gameplay state a model could reproduce (reason).
    NotGameplay(&'static str),
    /// Undecoded (reason).
    Undecoded(&'static str),
}

/// Status of every callee the big hooks emit as `CallSub` (T1845 round 4).
pub fn call_sub_status(address: u32) -> SubStatus {
    if let Some(port) = super::hooks_story::callee_port(address) {
        return SubStatus::Ported(port);
    }
    match address {
        0x1ECA60 => SubStatus::Ported("hooks_big::level0b_hitp_damage"),
        0x1EFE30 => SubStatus::Ported("hooks_big::level0b_jum_check"),
        // T1846 batch 8: both decoded and golden-verified against the original (docs/t-story-event-strip-t1846.md).
        0x1F2810 => SubStatus::Ported(
            "strip_update::strip_update (descriptor table 0x4FFD64 as STRIP_KINDS)",
        ),
        0x1F48A0 => {
            SubStatus::Ported("strip_update::strip_spawner (engine callees behind StripServices)")
        }
        _ => SubStatus::Undecoded("not in the registry"),
    }
}
