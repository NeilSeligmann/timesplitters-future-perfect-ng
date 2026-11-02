//! Callees of the level 6 hook (`0x20B630`) and the level 0xB hook (`0x1F5A10`) that `hooks_big` left as `CallSub`
//! (T1845 round 3, task H).
//!
//! Static reading of `build/default.xbe` (SHA-256 `3cfd001a...1816bc`), no xemu run. Control flow, tags, ids and
//! constants are MEASURED; the game meaning of a field is INFERRED (names from `tools/data/function_names.csv`).
//! Each function is a pure model: facts the original reads in, an ordered effect list out. Details, the exact
//! integration points for `hooks_big.rs` and the open points: `docs/t-story-event-hooks-l06-0b.md`.
//!
//! | Callee | Level | Rust |
//! |---|---|---|
//! | `0x208810` | 6 | [`SwitchTracker`] |
//! | `0x2085B0` (+ `0x208550`) | 6 | [`level06_npc_rules`], [`weapon_handover`] |
//! | `0x20B480` (+ `0x208A40`) | 6 | [`Level06Fade`], [`level06_character_ok`] |
//! | callback `0x20B8C0` | 6 | [`callback_20b8c0`] |
//! | `0x1EC300` | 0xB | [`Level0bBarrier`] |
//! | `0x1F1EC0` | 0xB | [`level0b_trn3`] |
//! | `0x1F1F70` | 0xB | [`nuke_countdown_slots`] |
//! | `0x1EFE90` | 0xB | [`level0b_fbn_slot`] |
//! | `0x1F0290` | 0xB | [`level0b_pete`] |
//! | `0x1EAB10` (+ `0x1EA8D0`, `0x1EA7D0`, `0x1EA9E0`) | 0xB | [`CameraShake`] |

/// One side effect, in call order.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// `0x2274C0(slot, arg)`: finish the static slot.
    SlotFinish { slot: u32, arg: u32 },
    /// `0x227710(slot, arg)`: activate the static slot.
    SlotActivate { slot: u32, arg: u32 },
    /// Controller (`object+0x7C` extension) word `+0x94` (the AI state).
    SetState { target: Target, state: u32 },
    /// Controller byte store.
    SetByte {
        target: Target,
        offset: u32,
        value: u8,
    },
    /// `0x1BA220(controller, ammo_type, amount)`.
    AddAmmo {
        target: Target,
        ammo_type: u32,
        amount: u32,
    },
    /// `0x468A0(handle, position, id, volume, arg5, pitch, flags)`.
    Sound {
        handle: SoundHandle,
        position: Option<[f32; 3]>,
        id: u32,
        volume: f32,
        arg5: f32,
        pitch: f32,
        flags: u32,
    },
    /// `0x45820` one-shot sound request at a position (`0x35F`).
    SoundRequest { id: u32, position: [f32; 3] },
    /// `0x46CB0(id, 0, 1)`: start a sound into a free slot of the table `0x6D6028`.
    SoundFreeSlot { id: u32 },
    /// `0x164310(text_id, frequency, 0)`: voice line (INFERRED from the name).
    VoiceLine { text_id: u32, argument: u32 },
    /// Store a float into a field of the object (offset).
    SetObjectFloat {
        target: Target,
        offset: u32,
        value: f32,
    },
    /// Store a dword into a field of the object (offset).
    SetObjectWord {
        target: Target,
        offset: u32,
        value: u32,
    },
    /// `0x9E720(object, 0)`: clear the hook at `+0x19C`.
    ClearField19c { target: Target },
    /// Create the particle emitter (`0x111AE0(&[0x78E8E4], template 0x78ED40, 0x10000, a, b)`).
    CreateEmitter {
        vector_a: [f32; 3],
        vector_b: [f32; 3],
    },
    /// `0x10E6E0` (byte `+0x12` = 1) or `0x10E6F0` (= 0) on the emitter.
    EmitterEnable(bool),
    /// `0x10E830(emitter, 0, box)`: particle bounds box (min, max).
    EmitterBounds { min: [f32; 3], max: [f32; 3] },
    /// `0xBB6C0`: commit a damage record to the victim.
    Damage {
        victim: Target,
        amount: f32,
        kind: u32,
        flags: u32,
    },
    /// `0x12C800` + `0x12C830(binding, object)`: bind a hud health record to the object (word `+0x268`).
    BindHud { target: Target },
    /// Local player record `+0xFF4` (the object being drained) set (`Some`) or cleared.
    SetPlayerFf4 {
        player: usize,
        target: Option<Target>,
    },
    /// `0x45460` stop the sound handle stored in `[0x4FFE64]`, then store -1.
    StopBarrierSound,
    /// `0x45820` request of sound `0x398`, its handle is stored in `[0x4FFE64]`.
    StartBarrierSound,
    /// `0xC5B90(vector)`: add to the accumulator at `0x7330A0`.
    AddAccumulatorA([f32; 3]),
    /// `0xC5BE0(vector)`: add to the accumulator at `0x733094`.
    AddAccumulatorB([f32; 3]),
}

/// The object an effect applies to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Looked up by 4 character tag (`0x15BA10`, e.g. "OS01").
    Tag(u32),
    /// The first player object (`0x277FE0`).
    FirstPlayer,
    /// Player `i` of the player array.
    Player(usize),
    /// A member of the callback's group, by index.
    Member(usize),
    /// An object the loop is visiting, by index in the object list `[0x7B6030]`.
    Listed(usize),
    /// The object the callback runs on.
    SelfObject,
}

/// Handle slots of the level 6 sound block `[0x75C090] + offset`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundHandle {
    Level06 { offset: u32 },
}

/// Facts about one tagged object. `flags` lists the masks for which the predicate `0x61570(object, mask)` is true,
/// `items` the ids for which `0x59D20(object, id)` is true.
#[derive(Clone, Debug, Default)]
pub struct ObjectFacts {
    pub present: bool,
    pub flags: Vec<u32>,
    pub items: Vec<u32>,
    /// `0x59CF0(object)[0]`.
    pub model: u32,
}

impl ObjectFacts {
    pub fn flag(&self, mask: u32) -> bool {
        self.flags.contains(&mask)
    }
    pub fn has(&self, item: u32) -> bool {
        self.items.contains(&item)
    }
}

// ---------------------------------------------------------------------------------------------------------
// Level 6
// ---------------------------------------------------------------------------------------------------------

/// Tags "SW00" to "SW05" (MEASURED).
pub const SWITCH_TAGS: [u32; 6] = [
    0x5357_3030,
    0x5357_3031,
    0x5357_3032,
    0x5357_3033,
    0x5357_3034,
    0x5357_3035,
];
/// Slot finished once all six switches have gone away (MEASURED).
pub const SWITCH_SLOT: u32 = 8;

/// `0x208810` state: the block at `[0x75C090] + 0x560` (counter) and `+0x564..+0x578` (six latches).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SwitchTracker {
    pub count: u32,
    pub latches: [u32; 6],
}

impl SwitchTracker {
    /// One call. A latch goes 0 to 1 when its object exists and 1 to 2 (count + 1) when the object is gone. The
    /// slot is finished at the END of every call whose count equals 6, so it is finished again every frame after
    /// that (MEASURED: the compare is not guarded by a once flag; finishing a finished slot is the slot manager's
    /// business).
    pub fn step(&mut self, present: [bool; 6]) -> Vec<Effect> {
        for (index, &here) in present.iter().enumerate() {
            let latch = &mut self.latches[index];
            if here {
                if *latch == 0 {
                    *latch = 1;
                }
            } else if *latch == 1 {
                *latch = 2;
                self.count += 1;
            }
        }
        if self.count == 6 {
            vec![Effect::SlotFinish {
                slot: SWITCH_SLOT,
                arg: 0,
            }]
        } else {
            Vec::new()
        }
    }
}

/// AI state, controller bytes and ammo amounts used by `0x2085B0` / `0x208550` (MEASURED).
pub const AMMO_AMOUNT: u32 = 1000;

/// `0x208550` (register convention `esi` = object): when the owner has item `0xAC8` or `0xACC` and the predicate
/// `0x61570(object, 0x8000)` holds: AI state 8, controller byte `+0x54C` = 0 and 1000 of ammo type 3.
pub fn weapon_handover(target: Target, object: &ObjectFacts) -> Vec<Effect> {
    if !object.present || !(object.has(0xAC8) || object.has(0xACC)) || !object.flag(0x8000) {
        return Vec::new();
    }
    vec![
        Effect::SetState { target, state: 8 },
        Effect::SetByte {
            target,
            offset: 0x54C,
            value: 0,
        },
        Effect::AddAmmo {
            target,
            ammo_type: 3,
            amount: AMMO_AMOUNT,
        },
    ]
}

/// Tags "MT01" to "MT03" (`0x208550` each), "MT07" and "OS01" (MEASURED).
pub const MT_TAGS: [u32; 3] = [0x4D54_3031, 0x4D54_3032, 0x4D54_3033];
pub const MT07_TAG: u32 = 0x4D54_3037;
pub const OS01_TAG: u32 = 0x4F53_3031;

/// Facts read by `0x2085B0`.
#[derive(Clone, Debug, Default)]
pub struct Level06NpcView {
    pub os01: ObjectFacts,
    pub mt: [ObjectFacts; 3],
    pub mt07: ObjectFacts,
    /// `0x277FE0()` returned an entry whose object is non-null, and that object's facts.
    pub first_player: ObjectFacts,
}

/// The "model `0xA34`" rule shared by OS01 and MT07: when the predicate `0x61570(object, 1)` holds and the model
/// word is `0xA34` the controller gets AI state `0x24`, byte `+0x54C` = 0 and 1000 of ammo type `0x29`.
fn model_a34_rule(target: Target, object: &ObjectFacts, out: &mut Vec<Effect>) {
    if object.present && object.flag(1) && object.model == 0xA34 {
        out.push(Effect::SetState {
            target,
            state: 0x24,
        });
        out.push(Effect::SetByte {
            target,
            offset: 0x54C,
            value: 0,
        });
        out.push(Effect::AddAmmo {
            target,
            ammo_type: 0x29,
            amount: AMMO_AMOUNT,
        });
    }
}

/// `0x2085B0` (MEASURED control flow, order of the calls kept): MT01 to MT03 weapon hand-over, the OS01 and MT07
/// model rule, then the rules of the first player by the items it owns (`0xA3F`, `0xA48`, `0xA46`, `0xA4F`, `0xA50`).
pub fn level06_npc_rules(view: &Level06NpcView) -> Vec<Effect> {
    let mut out = Vec::new();
    for (index, object) in view.mt.iter().enumerate() {
        out.extend(weapon_handover(Target::Tag(MT_TAGS[index]), object));
    }
    model_a34_rule(Target::Tag(OS01_TAG), &view.os01, &mut out);
    model_a34_rule(Target::Tag(MT07_TAG), &view.mt07, &mut out);
    let player = &view.first_player;
    if !player.present {
        return out;
    }
    let me = Target::FirstPlayer;
    if player.has(0xA3F) {
        if player.flag(0x4000) {
            out.push(Effect::SetState {
                target: me,
                state: 0,
            });
            out.push(Effect::SetByte {
                target: me,
                offset: 0x554,
                value: 0,
            });
        }
        if player.flag(1) {
            out.push(Effect::SetState {
                target: me,
                state: 0x41,
            });
            out.push(Effect::SetByte {
                target: me,
                offset: 0x54C,
                value: 0,
            });
            out.push(Effect::SetByte {
                target: me,
                offset: 0x58D,
                value: 1,
            });
        }
        if player.flag(2) {
            out.push(Effect::SetState {
                target: me,
                state: 0,
            });
            out.push(Effect::SetByte {
                target: me,
                offset: 0x58D,
                value: 0,
            });
        }
    }
    if player.has(0xA48) && player.flag(1) {
        out.push(Effect::SetState {
            target: me,
            state: 0,
        });
        out.push(Effect::SetByte {
            target: me,
            offset: 0x554,
            value: 0,
        });
    }
    if player.has(0xA46) && player.flag(1) {
        out.push(Effect::SetState {
            target: me,
            state: 8,
        });
        out.push(Effect::SetByte {
            target: me,
            offset: 0x54C,
            value: 0,
        });
        out.push(Effect::AddAmmo {
            target: me,
            ammo_type: 3,
            amount: AMMO_AMOUNT,
        });
    }
    if player.has(0xA4F) && player.flag(1) {
        out.push(Effect::SetState {
            target: me,
            state: 0,
        });
        out.push(Effect::SetByte {
            target: me,
            offset: 0x554,
            value: 0,
        });
    }
    if player.has(0xA50) && player.flag(1) {
        out.push(Effect::SetState {
            target: me,
            state: 8,
        });
        out.push(Effect::SetByte {
            target: me,
            offset: 0x54C,
            value: 0,
        });
        out.push(Effect::AddAmmo {
            target: me,
            ammo_type: 3,
            amount: AMMO_AMOUNT,
        });
    }
    out
}

/// `0x208A40` (register convention `esi` = character entry id). `followed` is `0x9D4A0(0x9A420(esi))` (the entry id
/// after following the link) and `key` is `0x9A4C0(esi)`. Followed ids 0 to 4, 6 to 8 and 0x23 are rejected, 5 is
/// accepted for keys `0x50B..=0x50E` and `0x5E0`, ids 9 to 0x22 and everything above 0x23 (including -1) are accepted.
pub fn level06_character_ok(followed: u32, key: u32) -> bool {
    match followed {
        0..=4 | 6..=8 | 0x23 => false,
        5 => (0x50B..=0x50E).contains(&key) || key == 0x5E0,
        _ => true,
    }
}

/// Positions of the two `0x359` emitters (MEASURED constants `[0x49E76C]`, 0.5, `[0x49E768]` and `[0x49E3D8]`,
/// `[0x47DAC0]`, `[0x49E764]`).
pub const LEVEL06_SOUND_POSITIONS: [[f32; 3]; 2] = [[53.47, 0.5, 40.92], [5.2, -4.0, -18.07]];
/// Fade rate: `(1 / [0x4E8CEC]) * 1.3333334` per call (the frame multiplier is NOT used here).
pub const LEVEL06_FADE_RATE: f32 = 1.333_333_4;

/// `0x20B480`: the level 6 ambience. `fade` is the float at `[0x75C090] + 4`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Level06Fade {
    pub fade: f32,
}

impl Level06Fade {
    /// `all_ok`: every player with record `+4 > -1` passes [`level06_character_ok`] (vacuously true with none).
    /// The fade rises by `rate / 60` when everybody passes, falls otherwise, is clamped to 0..=1 and drives the
    /// loop `0x11` (handle `+8`, argument 5 = 50.0); the two positional sounds `0x359` (handles `+0xC`, `+0x10`,
    /// volume 1.5, argument 5 = 5.0) are refreshed unconditionally.
    pub fn step(&mut self, all_ok: bool) -> Vec<Effect> {
        let delta = (1.0 / 60.0) * LEVEL06_FADE_RATE;
        let mut fade = self.fade + if all_ok { delta } else { -delta };
        fade = fade.clamp(0.0, 1.0);
        self.fade = fade;
        let mut out = vec![Effect::Sound {
            handle: SoundHandle::Level06 { offset: 8 },
            position: None,
            id: 0x11,
            volume: fade,
            arg5: 50.0,
            pitch: 1.0,
            flags: 0,
        }];
        for (index, position) in LEVEL06_SOUND_POSITIONS.iter().enumerate() {
            out.push(Effect::Sound {
                handle: SoundHandle::Level06 {
                    offset: 0xC + 4 * index as u32,
                },
                position: Some(*position),
                id: 0x359,
                volume: 1.5,
                arg5: 5.0,
                pitch: 1.0,
                flags: 0,
            });
        }
        out
    }
}

/// Facts of the callback `0x20B8C0` message: kind (`+0x14`), the object's group key members (`0x240070`, at most 5)
/// with their state words (`+0x94`), the object position (`+0x64`), the optional vector at message `+0x28` and the
/// object's offset vector (`+0x1A8`).
#[derive(Clone, Debug, Default)]
pub struct Callback20b8c0Input {
    pub kind: u32,
    pub member_states: Vec<u32>,
    pub position: [f32; 3],
    pub message_vector: Option<[f32; 3]>,
    pub offset: [f32; 3],
    /// The low 32 bits of the LCG state after one `0x153B40` advance.
    pub random: u32,
}

/// Message kind handled by the callback.
pub const CALLBACK_20B8C0_KIND: u32 = 0x2F;

/// Extension words set on members whose state is `0x4E0` or `0x4E1`: `+4` = 1 and float `+0xC` = 0.
pub fn callback_20b8c0(input: &Callback20b8c0Input) -> Vec<Effect> {
    if input.kind != CALLBACK_20B8C0_KIND {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (index, &state) in input.member_states.iter().take(5).enumerate() {
        if state == 0x4E0 || state == 0x4E1 {
            let target = Target::Member(index);
            out.push(Effect::SetObjectWord {
                target,
                offset: 0x7C_0004,
                value: 1,
            });
            out.push(Effect::SetObjectFloat {
                target,
                offset: 0x7C_000C,
                value: 0.0,
            });
        }
    }
    let me = Target::SelfObject;
    out.push(Effect::SoundRequest {
        id: 0x35F,
        position: input.position,
    });
    out.push(Effect::ClearField19c { target: me });
    // (u / 2^32 + 1.0) * 0.5, the delay timer at +0x1A4.
    let unit = input.random as f64 * (1.0 / 4_294_967_296.0);
    out.push(Effect::SetObjectFloat {
        target: me,
        offset: 0x1A4,
        value: ((unit + 1.0) * 0.5) as f32,
    });
    let spot = match input.message_vector {
        Some(vector) => vector,
        None => [
            input.offset[0] + input.position[0],
            input.offset[1] + input.position[1],
            input.offset[2] + input.position[2],
        ],
    };
    for (index, value) in spot.iter().enumerate() {
        out.push(Effect::SetObjectFloat {
            target: me,
            offset: 0xB4 + 4 * index as u32,
            value: *value,
        });
    }
    out.push(Effect::SetObjectFloat {
        target: me,
        offset: 0x1CC,
        value: 0.0,
    });
    out
}

// ---------------------------------------------------------------------------------------------------------
// Level 0xB
// ---------------------------------------------------------------------------------------------------------

/// The game's LCG (`0x153B40`): `state = state * 0x343FD + 0x269EC3` (64 bit); callers use the low 32 bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lcg {
    pub state: u64,
}

impl Lcg {
    pub fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(0x343FD).wrapping_add(0x26_9EC3);
        self.state as u32
    }
    /// `fild` of the signed value plus 2^32 when negative, times 2^-32: unsigned / 2^32 in [0, 1).
    pub fn next_unit(&mut self) -> f64 {
        self.next_u32() as f64 * (1.0 / 4_294_967_296.0)
    }
}

/// `0x1F1EC0`: facts. `game_word_7de314` is `[0x7DE314]`.
#[derive(Clone, Copy, Debug, Default)]
pub struct Trn3View {
    pub game_word_7de314: u32,
    /// `0x23EF30(4)` (mask bit test).
    pub mask_bit4: bool,
    pub slot_0x11_finished: bool,
    pub slot_2_finished: bool,
    /// Health (`+0x1CC`) of the object of local player `[0x7B0C7C]`.
    pub local_health: f32,
    /// `[0x75BFDC]`.
    pub voice_done: bool,
}

/// "TRN3" (`0x54524E33`) is the value of `[0x7DE314]` the hook waits for.
pub const TRN3_WORD: u32 = 0x5452_4E33;

/// `0x1F1EC0`. Returns the effects and the new `[0x75BFDC]`. Order: sound `0x3B6` (unless slot `0x11` is finished),
/// finish slot `0x11`, then, with local health at most 7.0, slot 2 unfinished and the voice not yet played, voice
/// line `0xFEF` with argument `0x140` and `[0x75BFDC] = 1`.
pub fn level0b_trn3(view: &Trn3View) -> (Vec<Effect>, bool) {
    let mut out = Vec::new();
    let mut voice_done = view.voice_done;
    if view.game_word_7de314 != TRN3_WORD || !view.mask_bit4 {
        return (out, voice_done);
    }
    if !view.slot_0x11_finished {
        out.push(Effect::SoundFreeSlot { id: 0x3B6 });
    }
    out.push(Effect::SlotFinish { slot: 0x11, arg: 0 });
    if view.local_health <= 7.0 && !view.slot_2_finished && !view.voice_done {
        out.push(Effect::VoiceLine {
            text_id: 0xFEF,
            argument: 0x140,
        });
        voice_done = true;
    }
    (out, voice_done)
}

/// `0x1F1F70`: the nuke countdown `c` (`0x1EB980`, seconds) finishes slot `0x23` in (60.0, 61.5], `0x20` in
/// (29.5, 30.5], `0x21` in (9.0, 10.5] and `0x22` in (0.0, 1.0) (MEASURED; the last upper bound is exclusive).
pub fn nuke_countdown_slots(countdown: f32) -> Vec<Effect> {
    let mut out = Vec::new();
    let c = countdown;
    if c <= 61.5 && c > 60.0 {
        out.push(Effect::SlotFinish { slot: 0x23, arg: 0 });
    }
    if c <= 30.5 && c > 29.5 {
        out.push(Effect::SlotFinish { slot: 0x20, arg: 0 });
    }
    if c <= 10.5 && c > 9.0 {
        out.push(Effect::SlotFinish { slot: 0x21, arg: 0 });
    }
    if c < 1.0 && c > 0.0 {
        out.push(Effect::SlotFinish { slot: 0x22, arg: 0 });
    }
    out
}

/// `0x1EFE90`: `first_player_key` is the table key (`[0x715F2C] + 0x20 * id`) of the first player's character, or
/// `None` without a first player. Finishes slot `0x24` when the key is `0x5CD` or `0x5CE` and neither the object
/// "FBN2" nor "FBNN" exists (the second argument of the call is the `FBNN` lookup, always 0 on that path).
pub fn level0b_fbn_slot(
    first_player_key: Option<u32>,
    fbn2_present: bool,
    fbnn_present: bool,
) -> Vec<Effect> {
    match first_player_key {
        Some(0x5CD) | Some(0x5CE) if !fbn2_present && !fbnn_present => {
            vec![Effect::SlotFinish { slot: 0x24, arg: 0 }]
        }
        _ => Vec::new(),
    }
}

/// "PETE" tag of `0x1F0290`.
pub const PETE_TAG: u32 = 0x5045_5445;

/// `0x1F0290`: model `0xBEB` / `0xBEC` set health `+0x1CC` to 100.0; `0xBED` / `0xBEE` finish slot `0x28` and set
/// health to 0.2.
pub fn level0b_pete(present: bool, model: u32) -> Vec<Effect> {
    let mut out = Vec::new();
    if !present {
        return out;
    }
    let target = Target::Tag(PETE_TAG);
    if model == 0xBEB || model == 0xBEC {
        out.push(Effect::SetObjectFloat {
            target,
            offset: 0x1CC,
            value: 100.0,
        });
    }
    if model == 0xBED || model == 0xBEE {
        out.push(Effect::SlotFinish { slot: 0x28, arg: 0 });
        out.push(Effect::SetObjectFloat {
            target,
            offset: 0x1CC,
            value: 0.2,
        });
    }
    out
}

/// One camera shake channel, a record of stride `0x3C` at `0x75BC30` (field names INFERRED).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShakeChannel {
    /// `+0`, `+4`: period bounds. `+8`: share of the span that the intensity shortens. `+0xC`: timer.
    pub min_period: f32,
    pub max_period: f32,
    pub spread: f32,
    pub timer: f32,
    /// `+0x10` current period, `+0x14` amplitude.
    pub period: f32,
    pub amplitude: f32,
    /// `+0x18..` from, `+0x24..` to, `+0x30..` current.
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub current: [f32; 3],
}

/// The half wave sine table `0x7A38A0` (2048 floats, filled elsewhere; the writer was not found): INFERRED
/// `sin(2 pi i / 2048)` from the use as `table[trunc(angle * 325.949 + 0.5) & 0x7FF]`.
pub fn sine_table(index: i32) -> f32 {
    let wrapped = (index & 0x7FF) as f64;
    (wrapped * std::f64::consts::TAU / 2048.0).sin() as f32
}

impl ShakeChannel {
    /// `0x1EA7D0(index)`: choose the next target. `intensity` is `[0x75BC28]`.
    pub fn retarget(&mut self, intensity: f32, lcg: &mut Lcg) {
        let span = self.max_period - self.min_period;
        let shorten = self.spread * span;
        let period = (((1.0 - intensity) * shorten + (span - shorten)) as f64 * lcg.next_unit())
            as f32
            + self.min_period;
        self.period = period;
        self.from = self.to;
        for slot in self.to.iter_mut() {
            *slot = ((lcg.next_unit() - 0.5) * self.amplitude as f64) as f32;
        }
    }

    /// `0x1EA8D0(index, amplitude)`: advance by `(1 / 60) * step`, retarget on every period crossed, then
    /// interpolate from to with the sine blend. `step` is the float frame multiplier `[0x4E794C]`.
    pub fn advance(&mut self, amplitude: f32, intensity: f32, step: f32, lcg: &mut Lcg) {
        self.amplitude = amplitude;
        self.timer += (1.0 / 60.0) * step;
        while self.timer > self.period {
            self.timer -= self.period;
            self.retarget(intensity, lcg);
        }
        let angle = (self.timer / self.period) * std::f32::consts::PI - 1.570_796_4;
        let index = (angle * 325.949_3 + 0.5) as i32;
        let blend = (sine_table(index) + 1.0) * 0.5;
        for axis in 0..3 {
            self.current[axis] = (self.to[axis] - self.from[axis]) * blend + self.from[axis];
        }
    }
}

/// The camera shake of level 0xB: three channels, the phase `[0x75BD98]` and the intensity `[0x75BC28]`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraShake {
    pub channels: [ShakeChannel; 3],
    pub phase: f32,
    pub intensity: f32,
}

impl CameraShake {
    /// `0x1EA9E0`: the level init. Periods (0.5, 0.7), (0.06, 0.26 with spread 0.9) and (0.01, 0.05); each channel is
    /// retargeted once with intensity 0 (MEASURED constants).
    pub fn init(lcg: &mut Lcg) -> Self {
        let mut shake = CameraShake::default();
        let params = [(0.5, 0.7, 0.0), (0.06, 0.26, 0.9), (0.01, 0.05, 0.0)];
        for (channel, &(min_period, max_period, spread)) in
            shake.channels.iter_mut().zip(params.iter())
        {
            *channel = ShakeChannel {
                min_period,
                max_period,
                spread,
                ..ShakeChannel::default()
            };
            channel.retarget(0.0, lcg);
        }
        shake
    }

    /// `0x1EAB10`: per call, `step` is `[0x4E794C]`. The phase advances by `(1/60) * step * 1.6666666` and wraps at
    /// 2.0; the intensity is a sine of the phase on the first half and 0 on the second. Channel amplitudes are 0.03,
    /// `intensity * 0.01 + 0.005` and `intensity * 0.003 + 0.002`. The sum of the three current vectors goes to
    /// accumulator A (`0xC5B90`) and its product with -0.15 to accumulator B (`0xC5BE0`).
    pub fn step(&mut self, step: f32, lcg: &mut Lcg) -> Vec<Effect> {
        let mut phase = (1.0 / 60.0) * step * 1.666_666_6 + self.phase;
        if phase > 2.0 {
            phase -= 2.0;
        }
        self.phase = phase;
        self.intensity = 0.0;
        if phase < 1.0 {
            let index = (phase * std::f32::consts::PI * 325.949_3 + 0.5) as i32;
            self.intensity = sine_table(index);
        }
        let intensity = self.intensity;
        let amplitudes = [0.03, intensity * 0.01 + 0.005, intensity * 0.003 + 0.002];
        for (channel, amplitude) in self.channels.iter_mut().zip(amplitudes) {
            channel.advance(amplitude, intensity, step, lcg);
        }
        let mut sum = [0.0f32; 3];
        for axis in 0..3 {
            sum[axis] = self.channels[0].current[axis]
                + self.channels[1].current[axis]
                + self.channels[2].current[axis];
        }
        vec![
            Effect::AddAccumulatorA(sum),
            Effect::AddAccumulatorB([sum[0] * -0.15, sum[1] * -0.15, sum[2] * -0.15]),
        ]
    }
}

/// Tags of the drained objects: "GTLT" for player 0, "SHUT" for the others (MEASURED).
pub const BARRIER_TAGS: [u32; 2] = [0x4754_4C54, 0x5348_5554];
/// Emitter bounds box (MEASURED constants).
pub const BARRIER_BOX: ([f32; 3], [f32; 3]) = ([-454.28, 1.16, -3.16], [-434.88, 4.25, 3.16]);
/// Emitter creation vectors (MEASURED constants).
pub const BARRIER_EMITTER_A: [f32; 3] = [0.0, -1.0, 0.0];
pub const BARRIER_EMITTER_B: [f32; 3] = [-444.4, 4.24, 0.0];
/// Fade rate per call, `(1/60) * 0.2`, and the damage scale 0.01 (MEASURED).
pub const BARRIER_FADE_RATE: f32 = 0.2;
pub const BARRIER_DAMAGE_SCALE: f32 = 0.01;
/// Damage record fields: type word (`0x15`) and flags (`0x200800`) (MEASURED).
pub const BARRIER_DAMAGE_KIND: u32 = 0x15;
pub const BARRIER_DAMAGE_FLAGS: u32 = 0x20_0800;
/// Drained object health ceiling and refill (MEASURED: 15.0 and 0.1 per call).
pub const BARRIER_HEALTH_MAX: f32 = 15.0;
pub const BARRIER_REFILL: f32 = 0.1;
/// Player state in which the drain applies (`+0xF54`).
pub const BARRIER_DRAIN_STATE: u32 = 0x4C;

/// An object the zone scan visits (`[0x7B6030]`).
#[derive(Clone, Copy, Debug, Default)]
pub struct ZoneObject {
    /// `+0x34` is `0xD5`.
    pub type_d5: bool,
    /// `+0x28 < 0`.
    pub flags_negative: bool,
    /// `+0x20 == 8`.
    pub kind_is_8: bool,
    /// `+0x60 == "HTIP"`.
    pub is_htip: bool,
    /// Extension `+0x7C` present and its word `+8` is below the local player count (a player).
    pub is_player_extension: bool,
}

/// One player and the drained object looked up for it.
#[derive(Clone, Copy, Debug, Default)]
pub struct BarrierPlayer {
    pub object_present: bool,
    /// Object `+0x268` (hud binding).
    pub binding: u32,
    /// Object `+0x1CC` health.
    pub health: f32,
    /// Player record `+0xF54`.
    pub state: u32,
}

/// `0x1EC300` (level 0xB per frame, a toxic zone with drained filter objects; meanings INFERRED). `open` is
/// `[0x75BF0C]`, `fade` `[0x75BFF8]`, `sound_handle` `[0x4FFE64]`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Level0bBarrier {
    pub fade: f32,
    pub open: u32,
    pub sound_playing: bool,
    pub emitter_created: bool,
}

/// Result of one barrier step: the effects and the updated per-player health values.
#[derive(Clone, Debug, Default)]
pub struct BarrierOutcome {
    pub effects: Vec<Effect>,
    pub players: Vec<BarrierPlayer>,
}

impl Level0bBarrier {
    /// One call. Order (MEASURED): emitter creation when absent, then (`open == 1`) emitter off, sound stopped, slot
    /// `0x1C` finished, else emitter on, sound started when absent, slot `0x1C` activated; the bounds box; the fade
    /// moves by 0.2 / 60 (up while `open == 0`, down otherwise) and is clamped to 0..=1; with fade above 0 every
    /// listed object that passes the filter takes damage `fade * 0.01`; then per player the drained object logic.
    pub fn step(&mut self, objects: &[ZoneObject], players: &[BarrierPlayer]) -> BarrierOutcome {
        let mut effects = Vec::new();
        if !self.emitter_created {
            effects.push(Effect::CreateEmitter {
                vector_a: BARRIER_EMITTER_A,
                vector_b: BARRIER_EMITTER_B,
            });
            effects.push(Effect::EmitterEnable(true));
            self.emitter_created = true;
        }
        if self.open == 1 {
            effects.push(Effect::EmitterEnable(false));
            if self.sound_playing {
                effects.push(Effect::StopBarrierSound);
                self.sound_playing = false;
            }
            effects.push(Effect::SlotFinish { slot: 0x1C, arg: 0 });
        } else {
            effects.push(Effect::EmitterEnable(true));
            if !self.sound_playing {
                effects.push(Effect::StartBarrierSound);
                self.sound_playing = true;
            }
            effects.push(Effect::SlotActivate { slot: 0x1C, arg: 0 });
        }
        effects.push(Effect::EmitterBounds {
            min: BARRIER_BOX.0,
            max: BARRIER_BOX.1,
        });
        let delta = (1.0 / 60.0) * BARRIER_FADE_RATE;
        self.fade = if self.open == 0 {
            self.fade + delta
        } else {
            self.fade - delta
        };
        let fade = self.fade.clamp(0.0, 1.0);
        self.fade = fade;
        if fade > 0.0 {
            for (index, object) in objects.iter().enumerate() {
                if object.type_d5
                    || object.flags_negative
                    || !object.kind_is_8
                    || object.is_htip
                    || object.is_player_extension
                {
                    continue;
                }
                effects.push(Effect::Damage {
                    victim: Target::Listed(index),
                    amount: fade * BARRIER_DAMAGE_SCALE,
                    kind: BARRIER_DAMAGE_KIND,
                    flags: BARRIER_DAMAGE_FLAGS,
                });
            }
        }
        let mut updated = players.to_vec();
        for (index, player) in updated.iter_mut().enumerate() {
            if !player.object_present {
                continue;
            }
            let object = Target::Tag(BARRIER_TAGS[usize::from(index != 0)]);
            if player.binding == 0 {
                if fade > 0.0 {
                    player.health = BARRIER_HEALTH_MAX;
                    effects.push(Effect::SetObjectFloat {
                        target: object,
                        offset: 0x1CC,
                        value: BARRIER_HEALTH_MAX,
                    });
                    effects.push(Effect::SetObjectFloat {
                        target: object,
                        offset: 0x1D8,
                        value: BARRIER_HEALTH_MAX,
                    });
                    effects.push(Effect::BindHud { target: object });
                    player.binding = 1;
                    effects.push(Effect::SetPlayerFf4 {
                        player: index,
                        target: Some(object),
                    });
                }
                continue;
            }
            if fade > 0.0 && player.state == BARRIER_DRAIN_STATE {
                let drained = player.health - fade * BARRIER_DAMAGE_SCALE;
                player.health = drained;
                if drained < 0.0 {
                    player.health = 0.0;
                    effects.push(Effect::Damage {
                        victim: Target::Player(index),
                        amount: fade * BARRIER_DAMAGE_SCALE,
                        kind: BARRIER_DAMAGE_KIND,
                        flags: BARRIER_DAMAGE_FLAGS,
                    });
                }
                effects.push(Effect::SetObjectFloat {
                    target: object,
                    offset: 0x1CC,
                    value: player.health,
                });
                effects.push(Effect::SetPlayerFf4 {
                    player: index,
                    target: Some(object),
                });
            } else {
                player.health += BARRIER_REFILL;
                effects.push(Effect::SetPlayerFf4 {
                    player: index,
                    target: Some(object),
                });
                if player.health >= BARRIER_HEALTH_MAX {
                    player.health = BARRIER_HEALTH_MAX;
                    effects.push(Effect::SetPlayerFf4 {
                        player: index,
                        target: None,
                    });
                }
                effects.push(Effect::SetObjectFloat {
                    target: object,
                    offset: 0x1CC,
                    value: player.health,
                });
            }
        }
        BarrierOutcome {
            effects,
            players: updated,
        }
    }
}
