//! Typed world-side effects of the event verbs (spec section 6) and the completion model of
//! section 5.2. `decode` turns a definition into an [`Action`] by reading exactly the fields the
//! original handler reads (offsets in the comments are record offsets, MEASURED in T1838).
//! Verbs the engine runs itself (0, 1, 3, 16, 27, 29 to 37, 56, 57, 84, 88) are not actions.

use super::record::EventRecord;

pub const NO_TAG: u32 = 0x3F3F_3F3F;

/// Verb 60's original mask table. Each definition flag contributes its own bit to the
/// D4B00 freeze mask; the entries are intentionally independent (OR-composed).
pub const CAMERA_FREEZE_MASK_TABLE: [(u32, u32); 2] = [(0x0010_0000, 4), (0x2000_0000, 8)];

/// Decode the verb-60 definition flags using the original independent mask entries.
pub fn camera_freeze_mask(flags: u32) -> u32 {
    CAMERA_FREEZE_MASK_TABLE
        .iter()
        .filter(|(definition_flag, _)| flags & definition_flag != 0)
        .fold(0, |mask, (_, freeze_flag)| mask | freeze_flag)
}

/// How a flag word is changed by verbs 13, 14, 15 (OR, AND NOT, XOR with the mask).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlagOp {
    Set,
    Clear,
    Toggle,
}

/// Verbs 38 to 42.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AudioControl {
    /// 38 and 39: audio flag `[0x6D5EC0]` set to 1 or 0.
    Flag(bool),
    /// 40: music volume scale `[0x4D1848] = value * 3.3` (constant at `0x47DAE0`), divided by 1.0.
    MusicScale(f32),
    /// 41: `[0x4D1850] = value`.
    MusicLevel(f32),
    /// 42: as 41 through the other helper (`0x462B0`); done when the tick callback id is 0.
    MusicLevelAlt(f32),
}

/// Verbs 76 to 82: player HUD notice flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HudNotice {
    /// 76: low byte flags (`0x14EE40`), argument `+0xC`.
    LowByte(u32),
    Flag1000,
    Flag2000,
    Flag8000,
    Flag10,
    /// 81: flag 1 (`0x14DC00`).
    Flag1,
    /// 82: flag 8 (`0x14E1D0`).
    Flag8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// 2 (never on the disc): letterbox fade, message label, then callback.
    LetterboxMessage {
        fade_in: bool,
        label: u32,
        callback: u32,
    },
    /// 4 and 5: start the object animation, `value` is 1.0 for verb 4 and `+0x1C` for verb 5.
    ObjectAnimStart {
        key: u32,
        value: f32,
        callback: u32,
    },
    /// 6: animate to the end (`0x5A2F0(obj, large time)`).
    ObjectAnimToEnd {
        key: u32,
    },
    /// 7: re-initialise an object (`0x5F460` with `alt_init`, else `0x966E0`).
    ObjectReinit {
        key: u32,
        state: u32,
        value: f32,
        alt_init: bool,
    },
    /// 8: weather preset `0x344AC0`.
    Weather {
        preset: u32,
    },
    /// 9: actor state words (`+0`, `+4`, `+0x18`) into actor extension `+0xA08`, `+0xA0C`, `+0xC64`.
    ActorState {
        actor: u32,
        head: [u32; 2],
        value: u32,
    },
    /// 10: positional effect/sound request (`0x217B0`), `words` are `+0x20..+0x30`, `weight` is `+0xA8`.
    Effect {
        sound: u32,
        room: u32,
        sub_entry: u32,
        words: [u32; 5],
        weight: f32,
    },
    /// Verb 10 completion helper `0x233BD0`: spawn effect `effect` (`+0x10`, `0xEFBD0`) at the room
    /// table position (`0x877C0(room, sub_entry)`, scaled) or, when `sub_entry` is negative, at the
    /// inline position `+0x20..+0x28` with the extra triple `+0x2C..+0x34`.
    EffectBurst {
        effect: u32,
        room: u32,
        sub_entry: i32,
        position: [f32; 3],
        extra: [f32; 3],
    },
    /// 11: sound by id at a position, `flags` is the record flag word (bits 0x2000000, 0x8000000, 0x800, 0x4000, 0x20000, 0x10000).
    Sound {
        sound: u32,
        position: [f32; 3],
        flags: u32,
    },
    /// 12: set and clear masks on the object's table word (`+0x30`) and mark `+0x18 |= 2`.
    ObjectFlags {
        key: u32,
        set: u32,
        clear: u32,
    },
    /// 13, 14, 15: change the table word of a key (table `0x732A04` entry word `+0xC`).
    PropFlags {
        key: u32,
        mask: u32,
        op: FlagOp,
    },
    /// 17: restart the prop state machine (`0xB2330`, `0xB2270`).
    PropReset {
        key: u32,
        hold: u32,
    },
    /// 18: OR 0x4000000 into the prop word.
    PropMark {
        key: u32,
    },
    /// 19: force prop state (`0xB0DD0` / `0xB0E30`).
    PropStop {
        key: u32,
        activate: u32,
        hold: u32,
    },
    /// 20: four floats written to entry offsets +0x30, +0x3C, +0x40, +0x34 (each only if >= 0).
    PropFloats {
        values: [f32; 4],
    },
    /// 21, 22, 23: bit 0x20 of word +0x60 of table `0x6B9908` entry `key`.
    TableBit {
        key: u32,
        op: FlagOp,
    },
    /// 24: movie or cutscene (`0x23E9D0(index, parameter)`).
    Movie {
        index: u32,
        parameter: u32,
        camera_flag: bool,
    },
    /// 25: camera mode word with record flag bits 0x100000, 0x200000, 0x1000000.
    CameraMode {
        word: u32,
        flags: u32,
    },
    /// 28: start a timed view (`0x2347C0`): when `mode == 1` a block holds `seconds * 60` frames.
    TimedView {
        mode: u32,
        seconds: f32,
    },
    /// 38 to 42.
    Audio(AudioControl),
    /// 43: create the placed spawn record with this key unless the tag exists.
    SpawnPlaced {
        key: u32,
    },
    /// 44: destroy the object with this tag.
    DestroyObject {
        key: u32,
    },
    /// 45: destroy up to 20 objects of a group.
    DestroyGroup {
        group: u32,
    },
    /// 46: create from the spawn table if no object has the tag.
    CreateIfAbsent {
        key: u32,
    },
    /// 47: `0x177930(obj, value)`.
    ChildFloat {
        key: u32,
        value: f32,
    },
    /// 48: block init of a child (`0x177EC0` twice).
    ChildInit {
        key: u32,
        first: u32,
        second: u32,
    },
    /// 51: pose restore, positions optional by record flags 0x4000 and 0x400000.
    PoseRestore {
        key: u32,
        position: Option<[f32; 3]>,
        orientation: Option<[f32; 3]>,
    },
    /// 53: impulse on the tagged object (`0x233CC0`): `direction * scale`, at `point` when flag 0x4000.
    Impulse {
        key: u32,
        impulse: [f32; 3],
        point: Option<[f32; 3]>,
    },
    /// 54 (only when `[0x7DE402] > 0`) and 55.
    GroupSelect {
        first: u32,
        second: u32,
    },
    GroupTransfer {
        first: u32,
        second: u32,
        third: u32,
    },
    /// 57: toggle the object state of the keyed table entry (`0xB80A0`); the instance reset part is the engine's.
    ActivateByKey {
        key: u32,
    },
    /// 58: flag 0x20000000 on network slot objects of five keys.
    FlagObjects {
        keys: [u32; 5],
    },
    /// 59: clear the actor's target.
    ClearTarget {
        actor: u32,
    },
    /// 60: camera cue with player freeze flags (4 for record flag 0x100000, 8 for 0x20000000).
    CameraCue {
        camera: u32,
        argument: u32,
        freeze_flags: u32,
    },
    /// 61: camera target.
    CameraTarget {
        camera: u32,
        argument: u32,
    },
    /// 62: first activation of a table prop. 63: set field 0xA8.
    PropActivate {
        key: u32,
    },
    PropField {
        key: u32,
    },
    /// 64 and 65: copy ext position words (+0x30..+0x38) to the object (+0x64..+0x6C).
    CopyPosition {
        key: u32,
        variant: u32,
    },
    /// 66: alert group members (`0x267FF0(2, record, 0)`).
    AlertGroup {
        flags: u32,
        group: u32,
    },
    /// 67: toggle bit 0x1000000 of the player word +0x28 (negative index counts from the end).
    PlayerFlag {
        player: i32,
    },
    /// 68 and 69: letterbox fade in / clear.
    LetterboxFadeIn,
    LetterboxClear,
    /// 70 and 71: OR / AND NOT 0x8000 into every player record word +0xC.
    PlayersFlag8000 {
        set: bool,
    },
    /// 73: `0x221C20` sets `[0x75C114]` from the table `0x715F2C`.
    TableGlobal {
        index: u32,
    },
    /// 74 give / 75 take an item: player, tag, value.
    GiveItem {
        player: u32,
        tag: u32,
        value: u32,
    },
    TakeItem {
        player: u32,
        tag: u32,
        value: u32,
    },
    /// 76 to 82.
    HudNotice(HudNotice),
    /// 83: object nav-node loop variant B over four keys.
    NavLoopB {
        keys: [u32; 4],
    },
    /// 85: `[0x75F5E0] = value`.
    Global {
        value: u32,
    },
    /// 86: sample by text id with optional tag. 87: sample by sample id.
    SampleByText {
        label: u32,
        tag: Option<u32>,
    },
    Sample {
        sample: u32,
        tag: u32,
    },
    /// 89 and 90: deactivate / reactivate non-player characters.
    DeactivateCharacters,
    ReactivateCharacters,
    /// 92: letterbox hold.
    LetterboxHold,
}

/// Result of [`World::apply_action`](super::world::World::apply_action).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionResult {
    /// The world does not model this action: the engine reports `Unsupported`.
    Unsupported,
    Done,
    /// A handle for the instance (sound for verbs 10 and 11, object for 4 to 7 and 43).
    Handle(u32),
    /// The target lookup failed: executor aborts and verbs 43 and 46 reset the instance.
    Failed,
}

/// What the tail pass waits on after an executor ran (spec 5.2). The world answers
/// [`World::wait_done`](super::world::World::wait_done).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Wait {
    /// 10 and 11: the sound handle stopped playing.
    SoundFinished { handle: u32 },
    /// 16 and 18: prop controller state reached (`0xB0F60` when `activate`, else `0xB0F90`).
    PropState { key: u32, activate: bool },
    /// 4, 5, 7: the object's animation state `state` finished (`0x59DB0`).
    ObjectAnim { handle: u32, state: u32 },
    /// 24: movie finished (`0x23EF30`).
    MovieFinished { index: u32 },
    /// 25: camera flag bit 0x200000 of `[0x7B0CC0]+0x21C`.
    CameraFlag,
    /// 26: `0x61E90(R+0x10, 1)` drives the mission stage, done when it reports true.
    MissionStage { stage: u32 },
    /// 28: timed view block (`0x234800`), `mode` is `+0xC`, `seconds` `+0x10`, `fade` `+0x14`.
    TimedView { mode: u32, seconds: f32, fade: f32 },
    /// 49: wait for the item hand-over (`0x1766E0`/`0x17CF30`): object `+0x14`, value `+0xC`, player `+0x10`.
    ItemGive {
        object: u32,
        value: u32,
        player: u32,
    },
    /// 50: player `+0x10` in state 0x400 then take the item (`0x17C740`).
    ItemTake { object: u32, player: u32 },
    /// 60: the player's camera reached camera key `+0xC` (`+0x26C` equals `0xC6100(key)`).
    CameraReached { camera: u32 },
    /// 61: as 60 but only once the camera flag bit 0 is set (`0xC9F50`).
    CameraTargetReached { camera: u32 },
    /// 62: prop check `0xAEBC0(0xAEB80(key))`.
    PropCheck { key: u32 },
}

/// Pre-wait before the delay gate (spec 4.1 step 5, decoded in `0x237E70`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreWait {
    /// Verbs 43, 46, 48, 49, 51 (key `+0x14`) and 74 (key `+0x10`): blocked while
    /// `0x6B3D0(key)` holds and `0x61D20(0x66560())` is non-zero.
    KeyStage { key: u32 },
    /// Verbs 3 and 7: blocked while the object type `+0x18` is not loaded, or its table entry
    /// `0x12EC0` holds while `0x61D20(stage)` is non-zero.
    ObjectType { type_word: u32, table_index: u32 },
    /// Verb 24: blocked until the movie finished or the mission stage allows it (`0x23EF30`, `0x23EF20`, `0x61E90`).
    Movie { index: u32 },
}

/// Pre-wait descriptor of a definition, or `None` when its verb has none.
pub fn prewait(record: &EventRecord) -> Option<PreWait> {
    match record.verb {
        3 | 7 => Some(PreWait::ObjectType {
            type_word: record.arg(0x18) & 0xFFFF,
            table_index: record.arg(0x10),
        }),
        24 => Some(PreWait::Movie {
            index: record.arg(0x18),
        }),
        43 | 46 | 48 | 49 | 51 => Some(PreWait::KeyStage {
            key: record.arg(0x14),
        }),
        74 => Some(PreWait::KeyStage {
            key: record.arg(0x10),
        }),
        _ => None,
    }
}

/// Completion class of a verb (`0x2350A0` table `0x235188`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Completion {
    /// Done at once (every verb not listed below).
    Immediate,
    /// The instance `done` flag set by the tail pass (verbs 1, 3 to 7, 16, 18, 28, 49, 50, 60, 61).
    Flag,
    /// `R+0x3C == 0` (verbs 0, 12, 17, 19, 29, 30, 38, 39, 42).
    NoTickCallback,
    /// 10: done at once when `R+0xC == -1`, else the sound stopped.
    SoundOrNone,
    /// 11: sound stopped.
    Sound,
    /// 24, 25, 26, 62: a world wait.
    World,
}

pub fn completion(verb: u32) -> Completion {
    match verb {
        1 | 3 | 4 | 5 | 6 | 7 | 16 | 18 | 28 | 49 | 50 | 60 | 61 => Completion::Flag,
        0 | 12 | 17 | 19 | 29 | 30 | 38 | 39 | 42 => Completion::NoTickCallback,
        10 => Completion::SoundOrNone,
        11 => Completion::Sound,
        24..=26 | 62 => Completion::World,
        _ => Completion::Immediate,
    }
}

/// Tail-pass class (`0x237E0C` table): 0 sets the done flag at once, 13 never touches it, the
/// rest are special routines. Only class 0 is a pure flag setter.
pub fn tail_sets_flag_at_once(verb: u32) -> bool {
    matches!(verb, 0 | 5 | 6 | 53..=56 | 58 | 66 | 67 | 72..=74 | 88..=90)
}

fn position(record: &EventRecord, at: usize) -> [f32; 3] {
    [
        record.arg_f32(at),
        record.arg_f32(at + 4),
        record.arg_f32(at + 8),
    ]
}

/// Decode the world-side action of `record.verb`, or `None` for verbs the engine runs itself and
/// for verbs without an executor (26, 27, 49, 50, 52, 91 do nothing in the executor).
pub fn decode(record: &EventRecord) -> Option<Action> {
    let r = record;
    Some(match r.verb {
        2 => Action::LetterboxMessage {
            fade_in: r.flags & 0x80 != 0,
            label: r.arg(0x10),
            callback: r.callback,
        },
        4 => Action::ObjectAnimStart {
            key: r.arg(0x14),
            value: 1.0,
            callback: r.callback,
        },
        5 => Action::ObjectAnimStart {
            key: r.arg(0x14),
            value: r.arg_f32(0x1C),
            callback: r.callback,
        },
        6 => Action::ObjectAnimToEnd { key: r.arg(0x14) },
        7 => Action::ObjectReinit {
            key: r.arg(0x14),
            state: r.arg(0x18),
            value: r.arg_f32(0x1C),
            alt_init: r.flags & 0x80000 != 0,
        },
        8 => Action::Weather { preset: r.arg(0xC) },
        9 => Action::ActorState {
            actor: r.arg(0x14),
            head: r.head,
            value: r.arg(0x18),
        },
        10 => Action::Effect {
            sound: r.arg(0xC),
            room: r.arg(0x14),
            sub_entry: r.arg(0x18),
            words: [
                r.arg(0x20),
                r.arg(0x24),
                r.arg(0x28),
                r.arg(0x2C),
                r.arg(0x30),
            ],
            weight: r.weight(),
        },
        11 => Action::Sound {
            sound: r.arg(0xC),
            position: position(r, 0x20),
            flags: r.flags,
        },
        12 => Action::ObjectFlags {
            key: r.arg(0x14),
            set: r.arg(0xC),
            clear: r.arg(0x18),
        },
        13 => Action::PropFlags {
            key: r.arg(0x14),
            mask: r.arg(0xC),
            op: FlagOp::Set,
        },
        14 => Action::PropFlags {
            key: r.arg(0x14),
            mask: r.arg(0xC),
            op: FlagOp::Clear,
        },
        15 => Action::PropFlags {
            key: r.arg(0x14),
            mask: r.arg(0xC),
            op: FlagOp::Toggle,
        },
        17 => Action::PropReset {
            key: r.arg(0x14),
            hold: r.arg(0x10),
        },
        18 => Action::PropMark { key: r.arg(0x14) },
        19 => Action::PropStop {
            key: r.arg(0x14),
            activate: r.arg(0xC),
            hold: r.arg(0x10),
        },
        20 => Action::PropFloats {
            values: [
                r.arg_f32(0xC),
                r.arg_f32(0x10),
                r.arg_f32(0x18),
                r.arg_f32(0x1C),
            ],
        },
        21 => Action::TableBit {
            key: r.arg(0xC),
            op: FlagOp::Set,
        },
        22 => Action::TableBit {
            key: r.arg(0xC),
            op: FlagOp::Clear,
        },
        23 => Action::TableBit {
            key: r.arg(0xC),
            op: FlagOp::Toggle,
        },
        24 => Action::Movie {
            index: r.arg(0x18),
            parameter: r.arg(0x1C),
            camera_flag: r.flags & 0x400_0000 != 0,
        },
        25 => Action::CameraMode {
            word: r.arg(0x18),
            flags: (r.flags & 0x10_0000)
                | if r.flags & 0x20_0000 != 0 {
                    0x80_0000
                } else {
                    0
                }
                | (r.flags & 0x100_0000),
        },
        28 => Action::TimedView {
            mode: r.arg(0xC),
            seconds: r.arg_f32(0x10),
        },
        38 => Action::Audio(AudioControl::Flag(true)),
        39 => Action::Audio(AudioControl::Flag(false)),
        40 => Action::Audio(AudioControl::MusicScale(r.arg_f32(0xC))),
        41 => Action::Audio(AudioControl::MusicLevel(r.arg_f32(0xC))),
        42 => Action::Audio(AudioControl::MusicLevelAlt(r.arg_f32(0xC))),
        43 => Action::SpawnPlaced { key: r.arg(0x14) },
        44 => Action::DestroyObject { key: r.arg(0x14) },
        45 => Action::DestroyGroup { group: r.arg(0xC) },
        46 => Action::CreateIfAbsent { key: r.arg(0x14) },
        47 => Action::ChildFloat {
            key: r.arg(0x14),
            value: r.arg_f32(0x1C),
        },
        48 => Action::ChildInit {
            key: r.arg(0x14),
            first: r.arg(0xC),
            second: r.arg(0x10),
        },
        51 => Action::PoseRestore {
            key: r.arg(0x14),
            position: (r.flags & 0x4000 != 0).then(|| position(r, 0x20)),
            orientation: (r.flags & 0x40_0000 != 0).then(|| position(r, 0x2C)),
        },
        53 => {
            let scale = r.arg_f32(0x10);
            Action::Impulse {
                key: r.arg(0x14),
                impulse: [
                    scale * r.arg_f32(0x2C),
                    r.arg_f32(0x30) * scale,
                    r.arg_f32(0x34) * scale,
                ],
                point: (r.flags & 0x4000 != 0).then(|| position(r, 0x20)),
            }
        }
        54 => Action::GroupSelect {
            first: r.arg(0xC),
            second: r.arg(0x10),
        },
        55 => Action::GroupTransfer {
            first: r.arg(0xC),
            second: r.arg(0x10),
            third: r.arg(0x14),
        },
        57 => Action::ActivateByKey { key: r.arg(0xC) },
        58 => Action::FlagObjects {
            keys: [
                r.arg(0xC),
                r.arg(0x10),
                r.arg(0x14),
                r.arg(0x18),
                r.arg(0x1C),
            ],
        },
        59 => Action::ClearTarget { actor: r.arg(0x14) },
        60 => Action::CameraCue {
            camera: r.arg(0xC),
            argument: r.arg(0x10),
            freeze_flags: camera_freeze_mask(r.flags),
        },
        61 => Action::CameraTarget {
            camera: r.arg(0xC),
            argument: r.arg(0x10),
        },
        62 => Action::PropActivate { key: r.arg(0x14) },
        63 => Action::PropField { key: r.arg(0x14) },
        64 | 65 => Action::CopyPosition {
            key: r.arg(0x14),
            variant: r.verb,
        },
        66 => Action::AlertGroup {
            flags: r.arg(0xC),
            group: r.arg(0x10),
        },
        67 => Action::PlayerFlag {
            player: r.arg(0xC) as i32,
        },
        68 => Action::LetterboxFadeIn,
        69 => Action::LetterboxClear,
        70 => Action::PlayersFlag8000 { set: true },
        71 => Action::PlayersFlag8000 { set: false },
        73 => Action::TableGlobal { index: r.arg(0xC) },
        74 => Action::GiveItem {
            player: r.arg(0xC),
            tag: r.arg(0x10),
            value: r.arg(0x14),
        },
        75 => Action::TakeItem {
            player: r.arg(0xC),
            tag: r.arg(0x10),
            value: r.arg(0x14),
        },
        76 => Action::HudNotice(HudNotice::LowByte(r.arg(0xC))),
        77 => Action::HudNotice(HudNotice::Flag1000),
        78 => Action::HudNotice(HudNotice::Flag2000),
        79 => Action::HudNotice(HudNotice::Flag8000),
        80 => Action::HudNotice(HudNotice::Flag10),
        81 => Action::HudNotice(HudNotice::Flag1),
        82 => Action::HudNotice(HudNotice::Flag8),
        83 => Action::NavLoopB {
            keys: [r.arg(0xC), r.arg(0x10), r.arg(0x14), r.arg(0x18)],
        },
        85 => Action::Global { value: r.arg(0xC) },
        86 => Action::SampleByText {
            label: r.arg(0x10),
            tag: Some(r.arg(0x14)).filter(|&tag| tag != NO_TAG),
        },
        87 => Action::Sample {
            sample: r.arg(0xC),
            tag: r.arg(0x14),
        },
        89 => Action::DeactivateCharacters,
        90 => Action::ReactivateCharacters,
        92 => Action::LetterboxHold,
        _ => return None,
    })
}

/// The wait the tail pass runs for `record` once the executor ran, if any.
pub fn wait_for(record: &EventRecord, handle: Option<u32>) -> Option<Wait> {
    let r = record;
    Some(match r.verb {
        4 | 5 | 7 => Wait::ObjectAnim {
            handle: handle?,
            state: r.arg(0x18) & 0xFFFF,
        },
        10 if r.arg(0xC) != u32::MAX => Wait::SoundFinished { handle: handle? },
        11 => Wait::SoundFinished { handle: handle? },
        16 | 18 => Wait::PropState {
            key: r.arg(0x14),
            activate: r.arg(0xC) != 0,
        },
        24 => Wait::MovieFinished { index: r.arg(0x18) },
        25 => Wait::CameraFlag,
        26 => Wait::MissionStage { stage: r.arg(0x10) },
        28 => Wait::TimedView {
            mode: r.arg(0xC),
            seconds: r.arg_f32(0x10),
            fade: r.arg_f32(0x14),
        },
        49 => Wait::ItemGive {
            object: r.arg(0x14),
            value: r.arg(0xC),
            player: r.arg(0x10),
        },
        50 => Wait::ItemTake {
            object: r.arg(0x14),
            player: r.arg(0x10),
        },
        60 => Wait::CameraReached { camera: r.arg(0xC) },
        61 => Wait::CameraTargetReached { camera: r.arg(0xC) },
        62 => Wait::PropCheck { key: r.arg(0x14) },
        _ => return None,
    })
}

#[cfg(test)]
mod camera_mask_tests {
    use super::*;

    // Independent transcription of the two separate flag tests in lifted 0x236070's verb-60 arm.
    fn lifted_verb60_mask(flags: u32) -> u32 {
        let mut mask = 0;
        if flags & 0x0010_0000 != 0 {
            mask |= 4;
        }
        if flags & 0x2000_0000 != 0 {
            mask |= 8;
        }
        mask
    }

    fn verb60_record(flags: u32) -> EventRecord {
        let mut bytes = [0; super::super::record::RECORD_SIZE];
        bytes[8..12].copy_from_slice(&60u32.to_le_bytes());
        bytes[0xB0..0xB4].copy_from_slice(&flags.to_le_bytes());
        EventRecord::from_bytes(&bytes).unwrap()
    }

    #[test]
    fn verb60_mask_table_matches_lifted_independent_flag_branches() {
        let mut state = 0x2372_1045u32;
        let mut cases = [0, 0x0010_0000, 0x2000_0000, 0x2010_0000].into_iter();
        let mut checked = 0;
        for flags in cases.by_ref().chain((0..4092).map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        })) {
            let expected = lifted_verb60_mask(flags);
            assert_eq!(camera_freeze_mask(flags), expected, "flags={flags:#010x}");
            let Action::CameraCue { freeze_flags, .. } = decode(&verb60_record(flags)).unwrap()
            else {
                panic!("verb 60 did not decode as CameraCue");
            };
            assert_eq!(freeze_flags, expected, "decoded flags={flags:#010x}");
            checked += 1;
        }
        assert_eq!(checked, 4096);
    }
}
