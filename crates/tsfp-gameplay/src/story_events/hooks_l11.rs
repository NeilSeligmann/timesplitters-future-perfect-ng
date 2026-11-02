//! Level 0x11 callees, the level 0x1D weather remainder and the level 0x16 pieces (T1845 round 3, task I).
//!
//! Static reading of `build/default.xbe` (SHA-256 `3cfd001a...1816bc`), no xemu run. Control flow, constants and
//! offsets are MEASURED; the game meaning of a field or helper is INFERRED (names from `tools/data/function_names.csv`).
//! Every function is a pure model: world facts in, an ordered effect list out. Doc: `docs/t-story-event-hooks-l11.md`.
//!
//! | Original | Level | Rust |
//! |---|---|---|
//! | `0x213900` | 0x11 per-frame "JBTH" tally (called by the hook `0x213F50`) | [`jbth_tally`] |
//! | `0x213580` | 0x11 CZ01..CZ11 flag pass | [`cz_flag_pass`] |
//! | `0x2106A0`, `0x210770` | 0x11 object tick functions | [`tick_2106a0`], [`tick_210770`] |
//! | `0x2102D0`, `0x2103C0` | 0x11 "1ZGH"/"2ZGH" spawn tick and teardown | [`tick_2102d0`], [`teardown_2103c0`] |
//! | `0x214220` | 0x11 init (particle templates) | [`level11_init`], [`particle_templates`] |
//! | `0x207A70`, `0x207830` | 0x1D (mode 3) weather | [`Rng`], [`rain_step`], [`room_drop_step`] |
//! | `0x20FBD0` remainder, `0x20EA60` | 0x16 | [`level16_emitters`], [`level16_colour_word`], [`level16_scene`] |

/// Sound request (`0x468A0`): handle slot, id, volume, range, pitch, flags and a position.
#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    pub handle: u32,
    pub id: u32,
    pub volume: f32,
    pub range: f32,
    pub pitch: f32,
    pub flags: u32,
    pub position: Option<[f32; 3]>,
}

/// One side effect, in call order.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// `0x2274C0(slot, arg)` finish a static slot.
    SlotFinish { slot: u32, arg: f32 },
    /// `0x2276E0(slot, arg)` status 4 (failed) unless 0 or 4.
    SlotFail { slot: u32, arg: f32 },
    /// Local player extension word `+0xFF4` store.
    SetLocalWordFf4 { value: u32 },
    /// `0xBB6C0` damage commit on the JBTH object: `source` is the record's attacker slots (an object key or none).
    CommitDamage { amount: f32, source_is_jbth: bool },
    /// Object model flag word `+0x28`: set or clear bit 0.
    SetFlagBit0 { tag: u32, set: bool },
    /// `0x3734E0(lcp2)` (INFERRED: scene-object start) when the JBTH type is 0x225.
    Call3734e0 { tag: u32 },
    /// `0x373500()` when the JBTH type is 0x227.
    Call373500,
    /// Extension `+0x94` (script state) store with the byte `+0x54C` (or the named byte offset) cleared.
    SetScriptState {
        state: u32,
        clear_byte_offset: Option<u32>,
    },
    /// `0x1BA220(ext, ammo_type, amount)`.
    AddAmmo { ammo_type: u32, amount: u32 },
    /// `0x1727A0(...)` clone-object spawn followed by `0xAE240(object, seconds, 8)` timed state.
    SpawnTimedClone {
        args: [u32; 9],
        seconds: f32,
        timed_flags: u32,
    },
    /// `0x352120(obj, key, a, b, &pos)` composite instance init.
    CompositeInit {
        key: u32,
        kind: u32,
        resource: ResourceSlot,
        position: [f32; 3],
    },
    /// `0x9E720(object, function)` plus `0xA5CE0(object, 3, 0)`: set `+0x19C` and drop the state 3 tick.
    ReplaceTick { function: u32 },
    /// `0x16B190(slot)` free a composite slot of the table at `0x74C464`.
    FreeCompositeSlot { slot: i32 },
    /// `0x3516A0(entry)` activate composite parts.
    ActivateParts,
    /// Tail call to `0x351260(arg)`.
    TailCall351260,
    /// `0x468A0` sound loop.
    Sound(Sound),
    /// `0x1515E0(&value, target, step)` approach of a state float.
    Approach { slot: u32, target: f32, step: f32 },
    /// Store to a game global or a state float.
    SetGlobalF32 { address: u32, value: f32 },
    /// Particle spawn `0x111AE0(handle, template, 0, position, vector)`.
    SpawnParticle {
        handle_slot: u32,
        template: u32,
        position: [f32; 3],
        vector: [f32; 3],
    },
    /// `0x10E730(handle, &position)`.
    MoveParticle {
        handle_slot: u32,
        position: [f32; 3],
    },
    /// `0xEFBD0(kind, 0, position, 0, 0, 0, 1)` table entry spawn.
    SpawnTableEntry { kind: u32, position: [f32; 3] },
    /// Level 0x16 colour words `+0x1AC` and `+0x1B8` of the state block `[0x75C0A0]`.
    SetColourWords { value: u32 },
    /// Set bits of the object flag word `+0x28`.
    SetFlagBits { tag: u32, bits: u32 },
    /// Call `address(tag)` (`0x18AC60` or `0x18AC80`, INFERRED: start a scripted scene by tag).
    CallWithTag { address: u32, tag: u32 },
    /// A callee not decoded (address).
    CallSub { address: u32 },
}

/// Which resource pointer `0x2102D0` passes to `0x352120` (INFERRED: a model resource table entry).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceSlot {
    /// `[[obj+4]+0x8C]+0x18`
    Offset18,
    /// `[[obj+4]+0x8C]+0x50`
    Offset50,
}

// ------------------------------------------------------------------------------------------------------------------
// 0x213900

/// Tag "JBTH" (`0x4A425448`): the protected object of the level 0x11 tally.
pub const TAG_JBTH: u32 = 0x4A42_5448;
/// Table at `0x506E40`: 10 entries, stride 0xC: `{ net_type, key, state }` (state 0 empty, 1 tracked, 2 gone).
pub const TALLY_ENTRIES: usize = 10;
/// Slots tested with `0x59D20` (INFERRED: the owner holds one of the objective items).
pub const TALLY_OWNER_SLOTS: [u32; 4] = [0x116, 0x117, 0x119, 0x118];
/// Progress fraction above which a held object counts (`[0x476040]`).
pub const TALLY_PROGRESS_LIMIT: f32 = 0.9;
/// Damage added per qualifying object (`[0x475CD0]`).
pub const TALLY_DAMAGE_PER_OBJECT: f32 = 0.8;
/// Damage dealt when the local player is alive and not in state 6 or 7 (`[0x47D1E4]`).
pub const TALLY_FIXED_DAMAGE: f32 = 8.0;
/// Mode byte `[0x7DE457]` multipliers: 0 gives 0.5, 2 gives 1.5, other values 1.0.
pub fn tally_mode_scale(mode_byte: u8) -> f32 {
    match mode_byte {
        0 => 0.5,
        2 => 1.5,
        _ => 1.0,
    }
}

/// One tracked object as the world sees it (object resolved from the key, `0x72F5E8` pool, stride 0x2E8).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrackedObject {
    /// The pool entry's stored key still equals the table key (otherwise the object is gone).
    pub key_matches: bool,
    /// Health `+0x1CC`.
    pub health: f32,
    /// Extension flag `+0x24 & 8`.
    pub ext_flag_8: bool,
    /// `0x59D20` over [`TALLY_OWNER_SLOTS`] is true for any of them.
    pub owner_has_item: bool,
    /// The extension word `+4` equals the JBTH object.
    pub owned_by_jbth: bool,
    /// `0x59EC0` progress fraction.
    pub progress: f32,
}

/// Table entry state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TallyEntry {
    pub net_type: u32,
    pub key: u32,
    pub state: u32,
}

/// What the tally reads this frame.
#[derive(Clone, Debug, Default)]
pub struct TallyView {
    pub jbth_exists: bool,
    pub jbth_health: f32,
    pub mode_byte: u8,
    /// `[0x7B0C7C]` local player state `+0xF54`, `None` for a null local player.
    pub local_state: Option<u32>,
    /// For each entry, the key `0x15BA10(net_type)` would find now (state 0 entries only).
    pub found_keys: [Option<u32>; TALLY_ENTRIES],
    /// For each entry, the resolved object when the key is non-zero.
    pub objects: [Option<TrackedObject>; TALLY_ENTRIES],
}

/// `0x213900` (called every frame by the hook `0x213F50`): advance the ten tracked entries, finish slot 1 when all
/// are gone (and clear the local `+0xFF4`), otherwise punish the JBTH object: a fixed 8.0 damage when the local player
/// is alive in a state other than 6 and 7, else 0.8 per tracked object that is held and above 0.9 progress scaled
/// by the mode. Returns the effects and the new table.
pub fn jbth_tally(view: &TallyView, table: &mut [TallyEntry; TALLY_ENTRIES]) -> Vec<Effect> {
    let mut out = Vec::new();
    let mut gone = 0;
    let mut accumulator = 0.0f32;
    let mut first_alive = false;
    for (index, entry) in table.iter_mut().enumerate() {
        match entry.state {
            0 => {
                if let Some(key) = view.found_keys[index] {
                    entry.key = key;
                    entry.state = 1;
                    first_alive = true;
                }
            }
            1 => {
                let alive = entry.key != 0
                    && view.objects[index]
                        .is_some_and(|o| o.key_matches && o.health > 0.0 && !o.ext_flag_8);
                if !alive {
                    entry.key = 0;
                    entry.state = 2;
                    gone += 1;
                } else if let Some(o) = view.objects[index] {
                    first_alive = true;
                    if o.owner_has_item && o.owned_by_jbth && o.progress > TALLY_PROGRESS_LIMIT {
                        accumulator += TALLY_DAMAGE_PER_OBJECT;
                    }
                }
            }
            _ => gone += 1,
        }
    }
    let _ = first_alive;
    if gone == TALLY_ENTRIES {
        out.push(Effect::SlotFinish { slot: 1, arg: 3.0 });
        out.push(Effect::SetLocalWordFf4 { value: 0 });
        return out;
    }
    if view.jbth_exists {
        let local_alive = view.local_state.is_some_and(|s| s != 7 && s != 6);
        if local_alive {
            out.push(Effect::CommitDamage {
                amount: TALLY_FIXED_DAMAGE,
                source_is_jbth: true,
            });
        } else if accumulator > 0.0 {
            let amount = accumulator * tally_mode_scale(view.mode_byte);
            out.push(Effect::CommitDamage {
                amount,
                source_is_jbth: true,
            });
        }
        if view.jbth_health <= 0.0 {
            out.push(Effect::SlotFail { slot: 1, arg: 0.0 });
        }
    }
    out
}

// ------------------------------------------------------------------------------------------------------------------
// 0x213580

/// Tags "CZ01".."CZ11" (`0x435A3031`..`0x435A3131`), 11 objects.
pub const CZ_TAGS: [u32; 11] = [
    0x435A_3031,
    0x435A_3032,
    0x435A_3033,
    0x435A_3034,
    0x435A_3035,
    0x435A_3036,
    0x435A_3037,
    0x435A_3038,
    0x435A_3039,
    0x435A_3130,
    0x435A_3131,
];
/// Tag "Lcp2" (`0x4C637032`).
pub const TAG_LCP2: u32 = 0x4C63_7032;

/// `0x213580`: per CZ object by its class id (`0x59CF0(obj)[0]`): 0x22A clears bit 0 of `+0x28`, 0x228 and 0x229 set it.
/// Then the JBTH net-slot object's class 0x225 starts the Lcp2 scene (`0x3734E0`) and 0x227 calls `0x373500`.
pub fn cz_flag_pass(class_ids: &[Option<u32>; 11], jbth_class: Option<u32>) -> Vec<Effect> {
    let mut out = Vec::new();
    for (index, class) in class_ids.iter().enumerate() {
        match class {
            Some(0x22A) => out.push(Effect::SetFlagBit0 {
                tag: CZ_TAGS[index],
                set: false,
            }),
            Some(0x228) | Some(0x229) => out.push(Effect::SetFlagBit0 {
                tag: CZ_TAGS[index],
                set: true,
            }),
            _ => {}
        }
    }
    match jbth_class {
        Some(0x225) => out.push(Effect::Call3734e0 { tag: TAG_LCP2 }),
        Some(0x227) => out.push(Effect::Call373500),
        _ => {}
    }
    out
}

// ------------------------------------------------------------------------------------------------------------------
// Tick functions

/// `0x2106A0`: when the key flag `0x61570(obj, 1)` is set, reset the script state by class id: 0x22D and 0x17F give
/// state 0x3E (byte `+0x54C` cleared), 0x22E and 0x187 give 0 (byte `+0x58A` cleared), 0x1FF gives 0 (byte `+0x581`),
/// 0x1FE gives 0x35 (byte `+0x54C`) and grants 1000 ammo of type 9.
pub fn tick_2106a0(key_flag_1: bool, class_id: u32) -> Vec<Effect> {
    if !key_flag_1 {
        return Vec::new();
    }
    match class_id {
        0x22D | 0x17F => vec![Effect::SetScriptState {
            state: 0x3E,
            clear_byte_offset: Some(0x54C),
        }],
        0x22E | 0x187 => vec![Effect::SetScriptState {
            state: 0,
            clear_byte_offset: Some(0x58A),
        }],
        0x1FF => vec![Effect::SetScriptState {
            state: 0,
            clear_byte_offset: Some(0x581),
        }],
        0x1FE => vec![
            Effect::SetScriptState {
                state: 0x35,
                clear_byte_offset: Some(0x54C),
            },
            Effect::AddAmmo {
                ammo_type: 9,
                amount: 1000,
            },
        ],
        _ => Vec::new(),
    }
}

/// Seconds of the timed state `0xAE240` started by [`tick_210770`] and its flag argument (8).
pub const CLONE_TIMED_SECONDS: f32 = 10.0;

/// `0x210770`: class 0x233 only. Key flag `0x8000` sets state 0x3F (byte `+0x54C` cleared). Key flag `0x4000` spawns a
/// clone of the model entry (`0x1727A0`, arguments 8, 0x751, four zeros, 0x11, 0x400000 and 3.66 as a float) and
/// starts a 10 second timed state on it, then clears the state and the byte `+0x58B`. `model_entry` is `[[ext]+4]`.
pub fn tick_210770(
    class_id: u32,
    flag_8000: bool,
    flag_4000: bool,
    model_entry: u32,
    clone_ok: bool,
) -> Vec<Effect> {
    let mut out = Vec::new();
    if class_id != 0x233 {
        return out;
    }
    if flag_8000 {
        out.push(Effect::SetScriptState {
            state: 0x3F,
            clear_byte_offset: Some(0x54C),
        });
    }
    if flag_4000 {
        out.push(Effect::SpawnTimedClone {
            args: [model_entry, 8, 0x751, 0, 0, 0, 0, 0x11, 0x0040_0000],
            seconds: CLONE_TIMED_SECONDS,
            timed_flags: if clone_ok { 8 } else { 0 },
        });
        out.push(Effect::SetScriptState {
            state: 0,
            clear_byte_offset: Some(0x58B),
        });
    }
    out
}

/// Marker words of the record `+0xBF0` field `+8`: "1ZGH" (`0x48475A31`) and "2ZGH" (`0x48475A32`).
pub const ZGH1: u32 = 0x4847_5A31;
pub const ZGH2: u32 = 0x4847_5A32;
/// Composite keys passed to `0x352120`.
pub const COMPOSITE_KEY_1: u32 = 0x7363_316A;
pub const COMPOSITE_KEY_2: u32 = 0x7363_326A;

/// `0x2102D0`: by the record marker of `+0xBF0`: "1ZGH" takes the room entry `0x877C0(0x13, 0)` position lowered by
/// 0.5 and inits composite `0x7363316A` (kind 1, resource offset 0x18); "2ZGH" takes `0x877C0(0x13, 1)` raised by 0.5 and
/// inits `0x7363326A` (kind 7, offset 0x50). `room_position` is the vec3 the lookup returns.
pub fn tick_2102d0(marker: Option<u32>, room_position: [f32; 3]) -> Vec<Effect> {
    match marker {
        Some(ZGH1) => vec![Effect::CompositeInit {
            key: COMPOSITE_KEY_1,
            kind: 1,
            resource: ResourceSlot::Offset18,
            position: [room_position[0], room_position[1] - 0.5, room_position[2]],
        }],
        Some(ZGH2) => vec![Effect::CompositeInit {
            key: COMPOSITE_KEY_2,
            kind: 7,
            resource: ResourceSlot::Offset50,
            position: [room_position[0], room_position[1] + 0.5, room_position[2]],
        }],
        _ => Vec::new(),
    }
}

/// `0x2103C0` inputs: the record marker, whether the per-state table entry `[[model+0x5C]][0xD6A80()]` is 4, the
/// composite slot index `0x16B8C0(0x7363316A)` (-1 when none) and whether extension `+0x4C` is non-null.
#[derive(Clone, Copy, Debug)]
pub struct Teardown {
    pub marker: Option<u32>,
    pub state_table_is_4: bool,
    pub composite_slot: i32,
    pub has_parts: bool,
}

/// `0x2103C0`: when the marker is "1ZGH" and the table entry is 4 the object replaces its field `+0x19C` function by
/// `0x351260`, drops the state 3 tick, clears `[0x75C0C4]` and frees the composite slot found. Then, with parts, it
/// activates them if a slot was found, else tail calls `0x351260`.
pub fn teardown_2103c0(input: &Teardown) -> Vec<Effect> {
    let mut out = Vec::new();
    let mut slot = -1;
    if input.marker == Some(ZGH1) && input.state_table_is_4 {
        slot = input.composite_slot;
        out.push(Effect::SetGlobalF32 {
            address: 0x75C0C4,
            value: 0.0,
        });
        out.push(Effect::ReplaceTick { function: 0x351260 });
        if slot >= 0 {
            out.push(Effect::FreeCompositeSlot { slot });
        }
    }
    if input.has_parts {
        out.push(if slot >= 0 {
            Effect::ActivateParts
        } else {
            Effect::TailCall351260
        });
    }
    out
}

// ------------------------------------------------------------------------------------------------------------------
// 0x214220

/// One call of the particle template builders, arguments in call order (the original pushes them in reverse).
#[derive(Clone, Debug, PartialEq)]
pub enum TemplateCall {
    /// `0x1115F0(record, 0, 0, 1, 1.0, kind, 0, 1.0)` draw descriptor (`kind` 0x329 or 0x30A).
    DrawInit { record: u32, kind: u32 },
    /// `0x111680(colour_a, colour_b, 0, 0.6, 0, 0, 0.5)` ramp points (packed colours).
    RampPoints { colour_a: u32, colour_b: u32 },
    /// `0x111860(180.0, 360.0, 0.1, 0.1)`.
    Floats78([f32; 4]),
    /// `0x111920(0.001, 0.75, -4.0, 0.0)`.
    Floats88([f32; 4]),
    /// `0x1117F0(0, 0, a, 0.1, 1.0, b)`.
    SixFloats([f32; 6]),
    /// `0x111960(20.0)`.
    Field98(f32),
    /// `0x1113E0(emitter, record, spawn_flag, 0, 3.0, 0, 0.01)` emitter descriptor.
    EmitterInit {
        emitter: u32,
        record: u32,
        flag: u32,
    },
    /// `0x111510(a, 0, b, 0)`.
    EmitterFloats3c([f32; 4]),
    /// `0x1114F0(a, 0)`.
    EmitterFloats34([f32; 2]),
}

/// Template for the two particle effects created by `0x214220` (INFERRED: smoke/dust at the objective).
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleTemplate {
    pub calls: Vec<TemplateCall>,
    /// Handle global written by `0x111AE0` (then `0x10E6F0` clears byte `+0x12`).
    pub handle_global: u32,
    /// Both position and vector arguments of the spawn are zero vectors.
    pub emitter: u32,
}

/// The two templates in the order `0x214220` builds and spawns them (first `0x78DAC0`, then `0x78D940`).
pub fn particle_templates() -> [ParticleTemplate; 2] {
    let first = ParticleTemplate {
        calls: vec![
            TemplateCall::DrawInit {
                record: 0x78DAC0,
                kind: 0x329,
            },
            TemplateCall::RampPoints {
                colour_a: 0x7070_7FFF,
                colour_b: 0x7F7F_7FFF,
            },
            TemplateCall::Floats78([180.0, 360.0, 0.1, 0.1]),
            TemplateCall::Floats88([0.001, 0.75, -4.0, 0.0]),
            TemplateCall::SixFloats([0.0, 0.0, 2.0, 0.1, 1.0, 3.0]),
            TemplateCall::Field98(20.0),
            TemplateCall::EmitterInit {
                emitter: 0x78DA20,
                record: 0x78DAC0,
                flag: 1,
            },
            TemplateCall::EmitterFloats3c([5.0, 0.0, 100.0, 0.0]),
            TemplateCall::EmitterFloats34([0.5, 0.0]),
        ],
        handle_global: 0x78DA10,
        emitter: 0x78DA20,
    };
    let second = ParticleTemplate {
        calls: vec![
            TemplateCall::DrawInit {
                record: 0x78D940,
                kind: 0x30A,
            },
            TemplateCall::RampPoints {
                colour_a: 0x7F4F_5F7F,
                colour_b: 0x7F4F_5F7F,
            },
            TemplateCall::Floats78([180.0, 360.0, 0.1, 0.1]),
            TemplateCall::Floats88([0.001, 0.75, -4.0, 0.0]),
            TemplateCall::SixFloats([0.0, 0.0, 1.0, 0.1, 1.0, 1.0]),
            TemplateCall::Field98(20.0),
            TemplateCall::EmitterInit {
                emitter: 0x78D8A0,
                record: 0x78D940,
                flag: 0,
            },
            TemplateCall::EmitterFloats3c([15.0, 0.0, 7.0, 0.0]),
            TemplateCall::EmitterFloats34([1.0, 0.0]),
        ],
        handle_global: 0x78DA0C,
        emitter: 0x78D8A0,
    };
    [first, second]
}

/// The non-particle part of `0x214220` (the slot declarations are in `hooks_big::level11_slot_declarations`): the
/// 12-dword state record at `[0x75C0B0]` is cleared with words 1, 3 and 4 set to -1, `[0x75C0BC]` = 0.0, `[0x75C0C0]` = 0,
/// the two tables `0x5069F8` and `0x506AB8` (8 entries each) are cleared by `0x616D0` and linked by `0x61720`, `[0x76C284]`
/// = 2 (`0x36CEA0`), slot 0 gets text `0x12D0`, the tally table states (`0x506E48` + 12 * i) are zeroed, `[0x75C0C4]` = 1.
#[derive(Clone, Debug, PartialEq)]
pub struct Level11Init {
    pub state_record_words: [i32; 12],
    pub tally_table: [TallyEntry; TALLY_ENTRIES],
    pub global_76c284: u32,
    pub flag_75c0c4: u32,
}

pub fn level11_init() -> Level11Init {
    let mut words = [0i32; 12];
    words[1] = -1;
    words[3] = -1;
    words[4] = -1;
    Level11Init {
        state_record_words: words,
        tally_table: [TallyEntry::default(); TALLY_ENTRIES],
        global_76c284: 2,
        flag_75c0c4: 1,
    }
}

// ------------------------------------------------------------------------------------------------------------------
// Level 0x1D weather

/// The game's RNG (`0x153B40`): 64-bit LCG, `state = state * 0x343FD + 0x269EC3`, returns the low dword.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rng {
    pub state: u64,
    pub calls: u32,
}

impl Rng {
    pub fn next(&mut self) -> u32 {
        self.calls = self.calls.wrapping_add(1);
        self.state = self.state.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
        self.state as u32
    }
    /// The original converts the dword as unsigned and multiplies by 2^-32.
    pub fn unit(&mut self) -> f32 {
        self.next() as f32 * 2.328_306_4e-10
    }
}

/// Table size of the sine table `0x7A38A0` (2048 entries); the cosine is the entry 0x200 ahead.
pub const SIN_TABLE_SIZE: usize = 2048;
/// Radians to table index (`2048 / 2pi`).
pub const RADIANS_TO_INDEX: f32 = 325.949_3;

fn sin_cos_index(sine: &[f32], radians: f32) -> (f32, f32) {
    let index = (radians * RADIANS_TO_INDEX + 0.5) as i32;
    (
        sine[(index + 0x200) as usize & 0x7FF],
        sine[index as usize & 0x7FF],
    )
}

/// Two particle slots of the level 0x1D rain: positions, velocities and spawned flags.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RainState {
    pub position: [[f32; 3]; 2],
    pub velocity: [[f32; 3]; 2],
    pub live: [bool; 2],
}

/// Spawn height and horizontal radius (`[0x49E6CC]` 182.5, `[0x47D5C0]` 500.0), offsets 20.0 and 2.5, speed -35.0.
pub const RAIN_HEIGHT: f32 = 182.5;
pub const RAIN_RADIUS: f32 = 500.0;
pub const RAIN_SPEED: f32 = -35.0;
/// Gravity per step on the vertical velocity (`[0x475D24]`).
pub const RAIN_GRAVITY: f32 = 0.1;

/// `0x207A70` for both slots: with probability `steps / (60 * 14)` per slot and frame an empty slot spawns a particle at
/// a random point of a quarter disc (angle in [0, pi/4), radius 500) and a velocity of 35 along a direction within
/// +-pi/8 of it; a live slot integrates `v.y -= 0.1 dt`, `p += v dt` (dt = steps / 60) and moves the particle.
/// `sine` is the 2048 entry table, `steps` is `[0x4E7944]`. The original ignores particle death (the particle system
/// frees the handle), `particle_alive` reports it.
pub fn rain_step(
    state: &mut RainState,
    rng: &mut Rng,
    sine: &[f32],
    steps: u32,
    particle_alive: [bool; 2],
) -> Vec<Effect> {
    use std::f32::consts::{FRAC_PI_4, PI};
    let mut out = Vec::new();
    for slot in 0..2 {
        state.live[slot] = state.live[slot] && particle_alive[slot];
        let roll = rng.next() % (60 * 14);
        if roll < steps && !state.live[slot] {
            let angle = rng.unit() * FRAC_PI_4;
            let (cosine, sine_value) = sin_cos_index(sine, angle);
            state.position[slot] = [
                cosine * RAIN_RADIUS - 20.0,
                RAIN_HEIGHT,
                sine_value * RAIN_RADIUS - 2.5,
            ];
            let mut heading = (rng.unit() - 0.5) * FRAC_PI_4 + angle;
            if heading > PI {
                heading -= 2.0 * PI;
            }
            if heading < -PI {
                heading += 2.0 * PI;
            }
            let (cosine, sine_value) = sin_cos_index(sine, heading);
            state.velocity[slot] = [cosine * RAIN_SPEED, 0.0, sine_value * RAIN_SPEED];
            state.live[slot] = true;
            out.push(Effect::SpawnParticle {
                handle_slot: 0x75C060 + 4 * slot as u32,
                template: 0x78DBA0,
                position: state.position[slot],
                vector: [1.0, 0.0, 0.0],
            });
        }
        if state.live[slot] {
            let dt = (1.0 / 60.0) * steps as f32;
            state.velocity[slot][1] -= dt * RAIN_GRAVITY;
            state.position[slot][1] += state.velocity[slot][1] * dt;
            state.position[slot][0] += state.velocity[slot][0] * dt;
            state.position[slot][2] += state.velocity[slot][2] * dt;
            out.push(Effect::MoveParticle {
                handle_slot: 0x75C060 + 4 * slot as u32,
                position: state.position[slot],
            });
        }
    }
    out
}

/// Fixed point and kind used by `0x207830` (`[0x49E6C8]` 17.8, `[0x47D6A4]` -0.6, `[0x49E6C4]` -7.03).
pub const ROOM_DROP_POINT: [f32; 3] = [17.8, -0.6, -7.03];
/// Kind passed to `0xEFBD0`.
pub const ROOM_DROP_KIND: u32 = 0x25;
/// Squared camera distance the point must exceed (`[0x47D994]` 400.0, i.e. 20 units).
pub const ROOM_DROP_MIN_DISTANCE_SQUARED: f32 = 400.0;

/// `0x207830` (mode 3 only): every `steps / (60 * 1.2)` chance a ray is cast from the fixed point (three random numbers
/// give a jitter vector whose middle component is forced negative, but only the ray cast consumes them) and, when it hits,
/// the up component of the probe vector exceeds 0.1 and the point is more than 20 units from the camera, a table entry of
/// kind 0x25 appears 0.1 above the point. `ray_hit` is the `0x88C60` result, `camera` the position at `[0x7B0CC0]+0x27C`.
pub fn room_drop_step(rng: &mut Rng, steps: u32, ray_hit: bool, camera: [f32; 3]) -> Vec<Effect> {
    let interval = (60.0f64 * 1.2) as u32;
    let roll = rng.next() % interval;
    if roll >= steps {
        return Vec::new();
    }
    for _ in 0..3 {
        rng.next();
    }
    let up = [0.0f32, 1.0, 0.0];
    let distance_squared: f32 = (0..3)
        .map(|i| (ROOM_DROP_POINT[i] - camera[i]).powi(2))
        .sum();
    if ray_hit && up[1] > 0.1 && distance_squared > ROOM_DROP_MIN_DISTANCE_SQUARED {
        let position = [
            ROOM_DROP_POINT[0] + up[0] * 0.1,
            ROOM_DROP_POINT[1] + up[1] * 0.1,
            ROOM_DROP_POINT[2] + up[2] * 0.1,
        ];
        return vec![Effect::SpawnTableEntry {
            kind: ROOM_DROP_KIND,
            position,
        }];
    }
    Vec::new()
}

/// `0x2084A0` (levels 0x1D): the first call is the crossfade `0x208360` (hooks_audio), then in mode 3 only the rain
/// `0x207A70` and, as a tail jump, `0x207830`.
pub fn level1d_runs_weather(mode_byte: u8) -> bool {
    mode_byte == 3
}

// ------------------------------------------------------------------------------------------------------------------
// Level 0x16

/// The three `0x630` emitters (handles `+0x454`, `+0x458`, `+0x45C` of `[0x75C0A0]`): id 0x630, range 9.0, pitches
/// 0.95, 1.05, 1.0, volume the state float `+0x44C` (approached toward `+0x450` by 0.1 per call), positions on the line
/// x = -121.43, y = 3.0 with z 12.0, 15.5 and 19.0.
pub const EMITTER_630_POSITIONS: [[f32; 3]; 3] = [
    [-121.43, 3.0, 12.0],
    [-121.43, 3.0, 15.5],
    [-121.43, 3.0, 19.0],
];
pub const EMITTER_630_PITCHES: [f32; 3] = [0.95, 1.05, 1.0];
/// The two `0x645` emitters (handles `+0x464`, `+0x468`): range 18.0, volume 1.3, pitches 1.05 and 1.0.
pub const EMITTER_645_POSITIONS: [[f32; 3]; 2] = [[-69.17, 5.0, -28.04], [-71.03, 5.0, -21.76]];
pub const EMITTER_645_PITCHES: [f32; 2] = [1.05, 1.0];
/// The base loop 0x62F: handle `+0x460`, volume 1.3, range 30.0, pitch 1.0 at (54.45, 7.77, -81.74).
pub const BASE_LOOP_POSITION: [f32; 3] = [54.45, 7.77, -81.74];

/// Inputs of the emitter section of `0x20FBD0`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Level16Emitters {
    /// Record bytes `[0x7B977C]+6` and `+7`: either non-zero selects the 0x645 pair.
    pub record_flag_6: bool,
    pub record_flag_7: bool,
    /// State words `+0x464` and `+0x468` (the 0x645 handles) are non-negative (already playing).
    pub handle_464_active: bool,
    pub handle_468_active: bool,
    /// Current volume `+0x44C` and target `+0x450`.
    pub volume: f32,
    pub target: f32,
}

/// `0x20FBD0` emitter section after the colour scan: the base loop 0x62F is always updated first; then either the 0x645
/// pair (when a record flag is set or a 0x645 handle is playing) or the volume approach (step 0.1) and the three 0x630
/// emitters.
pub fn level16_emitters(input: &Level16Emitters) -> Vec<Effect> {
    let mut out = vec![Effect::Sound(Sound {
        handle: 0x460,
        id: 0x62F,
        volume: 1.3,
        range: 30.0,
        pitch: 1.0,
        flags: 0,
        position: Some(BASE_LOOP_POSITION),
    })];
    let pair = input.record_flag_6
        || input.record_flag_7
        || input.handle_464_active
        || input.handle_468_active;
    if pair {
        for index in 0..2 {
            out.push(Effect::Sound(Sound {
                handle: 0x464 + 4 * index as u32,
                id: 0x645,
                volume: 1.3,
                range: 18.0,
                pitch: EMITTER_645_PITCHES[index],
                flags: 0,
                position: Some(EMITTER_645_POSITIONS[index]),
            }));
        }
    } else {
        out.push(Effect::Approach {
            slot: 0x44C,
            target: input.target,
            step: 0.1,
        });
        for index in 0..3 {
            out.push(Effect::Sound(Sound {
                handle: 0x454 + 4 * index as u32,
                id: 0x630,
                volume: input.volume,
                range: 9.0,
                pitch: EMITTER_630_PITCHES[index],
                flags: 0,
                position: Some(EMITTER_630_POSITIONS[index]),
            }));
        }
    }
    out
}

/// The colour word from the room entry scan of `0x20FBD0` (state block `+0x1AC` and `+0x1B8`): with `fraction` the
/// `0x1570B0` result, byte = `(int)((1 - fraction) * 63) + 0x10` placed in the top byte, low bits `0x101000`.
pub fn level16_colour_word(fraction: f32) -> u32 {
    let byte = ((1.0 - fraction) * 63.0) as i32 as u8 as u32;
    (((byte + 0x10) << 24) & 0xFF10_1000) | 0x0010_1000
}

/// Inputs of `0x20EA60` (not a colour scan: the earlier description in `hooks_audio` is corrected).
#[derive(Clone, Debug, Default)]
pub struct Level16Scene {
    /// Tag "STIG" object (`0x49475453`): class 0x73B with the key flag 1 set.
    pub stig_class_73b_flag1: bool,
    /// The first object of `0x277FE0` (local player object), its extension and which of items 0x72F/0x735/0x753 it owns.
    pub player: bool,
    pub owns_72f_and_flag1: bool,
    pub owns_735_and_flag1: bool,
    pub owns_753: bool,
    pub flag1: bool,
    pub flag_4000: bool,
    pub flag2: bool,
    /// Tag "CZSC" object (`0x4353435A`, via `0x163730`) of class 0x776: key flags 1 and 2 and flag 0x2000.
    pub czsc_class_776: bool,
    pub czsc_flag1: bool,
    pub czsc_flag2: bool,
    pub czsc_flag_2000: bool,
    /// Tag "CRW1" object (`0x43525731`) of class 0x714 and which key flag is set (1, 2, 4, 8 or 0x10, first match wins).
    pub crw1_class_714: bool,
    pub crw1_flags: u32,
}

/// Documented outcome of `0x20EA60` (INFERRED meanings of the numbers): script states of the player extension `+0x94`:
/// STIG with class 0x73B and flag 1 gives 0x3A; own 0x72F+flag 1 resets state to 0 (byte `+0x554`); own 0x735+flag 1 gives
/// state 8 and 1000 ammo type 1; own 0x753: flag 1 gives 0x41 (byte `+0x58D` = 1), flag 0x4000 fires a random unit
/// direction (`0x345B60`), flag 2 resets state and byte. Then the CZSC object (class 0x776): flag 1 `0xC1360(tag
/// "RN2O", player object `+0x14`)`, flag 2 grants 20 of ammo type 0x21 plus `0x1BFCA0(ext, 0x3A)` and state 0x3A, flag 0x2000
/// sets bit `0x1000000` of its `+0x28`. Then the CRW1 object (class 0x714): `0x124C90(ext+0xC68)` and by key flag 1, 2, 4,
/// 8 the calls `0x18AC60` with tags "NNH1", "NNH4", "NNH3", "NNH2" or flag 0x10 `0x18AC80("SM1G")`.
pub fn level16_scene(input: &Level16Scene) -> Vec<Effect> {
    let mut out = Vec::new();
    if input.stig_class_73b_flag1 {
        out.push(Effect::SetScriptState {
            state: 0x3A,
            clear_byte_offset: Some(0x54C),
        });
    }
    if input.player {
        if input.owns_72f_and_flag1 {
            out.push(Effect::SetScriptState {
                state: 0,
                clear_byte_offset: Some(0x554),
            });
        }
        if input.owns_735_and_flag1 {
            out.push(Effect::SetScriptState {
                state: 8,
                clear_byte_offset: Some(0x54C),
            });
            out.push(Effect::AddAmmo {
                ammo_type: 1,
                amount: 1000,
            });
        }
        if input.owns_753 {
            if input.flag1 {
                out.push(Effect::SetScriptState {
                    state: 0x41,
                    clear_byte_offset: Some(0x54C),
                });
            }
            if input.flag_4000 {
                out.push(Effect::CallSub { address: 0x345B60 });
            }
            if input.flag2 {
                out.push(Effect::SetScriptState {
                    state: 0,
                    clear_byte_offset: Some(0x58D),
                });
            }
        }
    }
    if input.czsc_class_776 {
        if input.czsc_flag1 {
            out.push(Effect::CallSub { address: 0xC1360 });
        } else if input.czsc_flag2 {
            out.push(Effect::AddAmmo {
                ammo_type: 0x21,
                amount: 0x14,
            });
            out.push(Effect::CallSub { address: 0x1BFCA0 });
            out.push(Effect::SetScriptState {
                state: 0x3A,
                clear_byte_offset: Some(0x54C),
            });
        }
        if input.czsc_flag_2000 {
            out.push(Effect::SetFlagBits {
                tag: 0x4353_435A,
                bits: 0x0100_0000,
            });
        }
    }
    if input.crw1_class_714 {
        out.push(Effect::CallSub { address: 0x124C90 });
        let call = match input.crw1_flags {
            f if f & 1 != 0 => Some((0x18AC60, 0x4E4E_4831)),
            f if f & 2 != 0 => Some((0x18AC60, 0x4E4E_4834)),
            f if f & 4 != 0 => Some((0x18AC60, 0x4E4E_4833)),
            f if f & 8 != 0 => Some((0x18AC60, 0x4E4E_4832)),
            f if f & 0x10 != 0 => Some((0x18AC80, 0x534D_3147)),
            _ => None,
        };
        if let Some((address, tag)) = call {
            out.push(Effect::CallWithTag { address, tag });
        }
    }
    out
}
