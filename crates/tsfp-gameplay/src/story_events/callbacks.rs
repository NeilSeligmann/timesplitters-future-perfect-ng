//! Callback bodies of the table at `0x4D2B10` (T1842). A **callback** is the id stored at record
//! offset `+0x38` (run by verbs 0, 29, 30 and after creation by 3, 4, 5) or `+0x3C` (per-tick).
//! `0x69F30` replaces the id by a code address when the SETU entry loads; every body takes one
//! stack argument, the event instance (a 0x48-byte pool entry whose word 0 points at the
//! definition R) for verbs 0, 29 and 30, or the created object for verbs 3, 4, 5.
//!
//! [`decode`] reads exactly the instance and definition fields the body reads and returns the
//! steps it performs: [`SlotOp`]s (slot manager, run by the engine) and [`WorldOp`]s (game objects,
//! sound, camera, particles, run by [`World::apply_callback`](super::world::World)). Ids that
//! occur nowhere on the disc and bodies that were not decoded stay explicit
//! [`CallbackGap`]s with a reason. Confidence is MEASURED (read from the instructions of the
//! retail XBE) unless a comment says INFERRED (deduced from callee shapes) or notes an unresolved
//! callee. Tag keys are four-character codes (`tag(b"ARTR")`, big endian, as pushed).
//!
//! Disc usage (55 SETU paks, `tools/setu_census.py --links`, recounted in T1842): 83 distinct ids (80 in `+0x38`, 1 to 3 in `+0x3C`),
//! led by 28 (104), 32 (40), 33 (39), 75 (20), 10 (14), 60 (10). Uses are on verb 0 definitions
//! except 28 and 26 (verb 29), 25 (verb 30), 29 (verb 56) and 6 (verb 38); verbs 38 and 56 do not
//! run `+0x38` as a callback, so ids 6 and 29 are never executed on the disc (INFERRED from the
//! executors, which read other fields of those records).

use super::record::EventRecord;

/// Four-character code as the original pushes it (first character is the high byte).
pub const fn tag(code: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*code)
}

/// What the engine knows when a callback runs.
#[derive(Debug, Clone, Copy)]
pub struct CallbackContext {
    /// The definition R (`[instance+0]`).
    pub record: EventRecord,
    /// Instance word `+0x34` is non-zero (the verb executed once): the test `0x234270`.
    pub executed: bool,
    /// Instance words `+0x40` and `+0x44`. The pool builder and the state pass never write them,
    /// so they read as zero for an event instance (INFERRED from the absence of stores in
    /// `0x235630`, `0x237500` and `0x237E70`). Callbacks 57 and 63 to 70, 77 read them as the key
    /// and the object word of an interaction message: a message dispatcher that fills them was
    /// not found, so a world may supply real values.
    pub words: [u32; 2],
    /// Slot 0's registered actor of class 0x254 (`0x227980(0, 0x254, 0)`), only used by id 5.
    pub class_254_actor: u32,
}

/// Slot manager steps. The engine applies them to its [`SlotManager`](super::slots::SlotManager).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SlotOp {
    /// `0x2274C0(slot, x)`: finish.
    Finish(u32),
    /// `0x227650(slot, 0)`: pending to active.
    Start(u32),
    /// `0x227710(slot, 0)`: any started status except active returns to active.
    Reactivate(u32),
    /// Callback 98: remember the slot, the frame stamp `[0x7DE318]` and the float
    /// (`[0x75C094..0x75C09C]`) and start the slot.
    StartTimed { slot: u32, value: f32 },
    /// Callback 99: finish the remembered slot and forget it.
    FinishTimed,
    /// `0x227740(slot)`: make `slot` the current slot (`[0x78CED0]`).
    SetCurrent(u32),
}

/// Sound or particle at a fixed point is described by the id and the source of the position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Place {
    /// Position of the object found by `tag`.
    Object(u32),
    /// Object position plus the offset stored in the object (`+0x1A8..+0x1B0`).
    ObjectWithAnchor(u32),
    /// Position of a table entry (`0x7B2780`, stride 0x54) object (`entry+8` then `+0x64`).
    Entry(u32),
}

/// Which sub-record word of an object a state write targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateWord {
    /// Object `+0x1A0`, the script state of tagged props.
    Script,
    /// `[object+4]+0x30`, the state of the object's sub-record (animation or controller bits).
    Subrecord,
}

/// World-side effects. Every variant documents the callback that produces it.
#[derive(Debug, Clone, PartialEq)]
pub enum WorldOp {
    // ---- characters and players
    /// 28 (`0x234A30` into `0x161E50`): for every non-local actor (index from the local player
    /// count up, `0x278040` false, goal mask not 0x600001) whose character id (`0x9C590(0x9D4A0(0x9A420(+0x678)))`)
    /// equals `character`: clean up (`0xDAAE0`), set AI goal mask `0x400000` (`0x17570`) and OR `0x80000000` into `+0x2C`. INFERRED reading: activate the scripted NPC.
    NpcGoal { character: u32 },
    /// 32, 33 (`0x234A50`, `0x234AC0`): look the character up (`0x9CC40`), reset the input-driven
    /// state of local player `player` (`0x25C440(player, character, mode, 0, 0)`) and zero the
    /// player's three input floats (`[0x7B0C48] + 0xFA0 + player * 0x1584`). `mode` is 5 when record flag
    /// `0x800000` is set, else 1. Player 1 only when two or more local players exist and the second player exists.
    ResetInputDriven {
        player: u32,
        character: u32,
        mode: u32,
    },
    /// 59 (`0x1FCE90`): the first local player whose character key equals `group` gets item `item`
    /// (`0xDA280(extension, 0x9CC40(item, 1))`), then the first player's input floats are zeroed.
    GiveItemToCharacter { character: u32, item: u32 },
    /// 7 and 12 (`0x1E4B70`, `0x1E4D50`): camera cue `camera` from the camera table
    /// (`0xC6100(camera, 0, 4, 0xB, 0)`) for the current player (`0xD4B00(player+0x150, entry)`, frozen state).
    PlayerCameraCue { camera: u32 },
    /// 49 (`0x1D89E0`): as 7 with arguments (0, 4, 0, 0) and key "TFLC".
    PlayerCameraCueLevel { camera: u32 },
    /// 67 (`0x1E69E0`): unless a local player is already in state 0x100000 or 0x100, put the
    /// trigger's player (`words[1]`) in state 0x100000 with the scene camera (pad axis update
    /// `0x1E5300`, camera target `0xCB450`, state word `+0x1574 = 0x1B`).
    PlayerEnterScene { player_object: u32 },
    /// 77 (`0x20FB50`): freeze the player of `words[1]` (player 0 when none) with the pad aim controller
    /// (`0x20EE40`, `0xD4B00(state, 2, 0x1B, ...)`) unless it is in state 0x100.
    PlayerFreezeAim { player_object: u32 },
    /// 82 (`0x1F9CE0`): Story with one local player: register the player's actor as the active entity
    /// (`[[0x7B0C7C]+0xFF4]`) and, if the saved health ratio `[0x78DF44]` is still -1.0, allocate a HUD slot
    /// (`0x12C800`, `0x12C830`, flag `[0x75C028]`), else restore health = max health * ratio (flag `[0x75C02C]`).
    PlayerHudRegister,
    /// 83 (`0x1F9D50`): the counterpart: free the HUD slot, clear the active entity, store health / max health into `[0x78DF44]`, clear both flags.
    PlayerHudUnregister,

    // ---- tagged object state
    /// 75, 76 (tag R+0x14, script state 1 / 0), 91 ("MTHS" script state 2), 85 ("LKDL" sub-record state 2), 97 ("XFKY" sub-record state 1):
    /// write `value` into the state word of the object found by `tag`.
    SetObjectState {
        tag: u32,
        word: StateWord,
        value: u32,
    },
    /// 47 (`0x212130`): the instance's own tagged object (key R+0xAC looked up, result unused) has its
    /// extension word `+0x38` set to 1 when it is 0. INFERRED: the written record belongs to the callback's object.
    EnableExtensionState { tag: u32 },
    /// 93 (`0x224980`): XOR the script state of `tag` with `mask`; type 0x2CB with mask 1 plays sound 0x37F (volume 1.5) at the object.
    ToggleScriptState { tag: u32, mask: u32 },
    /// 10 (`0x1E2020`): commit damage to the object found by key R+0xAC: hit record cleared by
    /// `0xB91F0`, direction (0, 1, 0), amount 1000.0, kind 4, target and source the object, applied by `0xBB6C0`.
    DamageObject {
        tag: u32,
        amount: f32,
        kind: u32,
        direction: [f32; 3],
    },
    /// 60 (`0x216820`): group object found by key R+0xAC. Type 0x539: script state 3. Otherwise, when the object has an
    /// extension and its effect handle (`+0x128`) is not live, start particle table `0x78D200` (flags 0x8001000,
    /// unit scale) at its position.
    GroupTrigger { tag: u32 },
    /// 78 and 79 (`0x224480`, `0x224550`): advance the script state of the object (states 0 -> 2, 1 -> 3, 2 -> 3, 3 -> 2; 79 plays
    /// only on 0 and 3), start sound `sound` at object position + anchor, OR 0x40 into its flag word and install tick `0x224330`.
    SoundAdvance {
        tag: u32,
        sound: u32,
        plays_on: [bool; 4],
    },
    /// 86 (`0x234D00`): call the object's callback slot (`+0x154`) with `(object, 0)`, then `0x2345D0(object)` (remove).
    RemoveKeyedObject { tag: u32 },
    /// 87 (`0x234DA0`): the toss filter flag `[0x75CCBC]` is raised around `0x36EEC0(object, player)`, which requests state 0x2000.
    RequestObjectState { tag: u32, state: u32 },
    /// 88 (`0x376550`): apply effect mode `mode` (R+0xAC, one of NOPW NOP2 NOP3 DONE PWRD PWR2 DON2) to the object `tag`.
    /// `0x362D00(R+0xC)` runs first. TOSS objects use index pair (5, 4), others (7, 6). Unknown modes only clear `+0x44`.
    ApplyEffectMode { group: u32, tag: u32, mode: u32 },
    /// 81 (`0x205FC0`): for i in 0..6 place tagged object R+0x14+i at table position i (x, y, z of `0x503A60`; yaw 180 when x > 100),
    /// `+0x1A0 = i`, `+0x1A4 = +0xD0 = z`, `+0xCC = 1.0`, animation 1.0 and install ticks.
    PlaceNumberedProps { first_tag: u32 },
    /// 84 (`0x1FE9E0`): R+0xAC "BRDG": chain starting at "BDG1" gets state 0 and weight 1.5. "SENT": chain starting at "BNH1"
    /// gets sub state 2 and a vector (0, 0, 1) * 12 transformed by the object matrix.
    BridgeReset { mode: u32 },
    /// 92 (`0x21B440`): for tags "TRAM" and "TRA2" that are idle: randomise `+0xBC` (0.95 to 1.05), start sound 0x624 or 0x625, install tick.
    TramStart,
    /// 31 (`0x212440`): tagged "JBPK" object gets tick `0x212220` (slot 3), state 0 and timer 0.
    JbpkReset,
    /// 62 (`0x3757E0`): re-enable tagged "NFC4" (clear inactive flag 4, `0x370D80`, `0x36CF10`) and every dormant character (`0x15C3D0`).
    ActivateNfc4,
    /// 16 (`0x24D220`): destroy the tagged "TRCP" object and stop its effect handle `[0x75EB20]`.
    DestroyTrcp,
    /// 13 (`0x1E0250`): tagged "WNLF": script word `+0xAC = 0`, `+0x10 = 1.0`, position copied from room entry (0xF, 0).
    WnlfReset,
    /// 65 (`0x1E80E0`): when the trigger's object has a player state and "WNLF" is idle (`+0xAC == 0`), install tick `0x1DFA60` and set `+0xAC = 4`.
    WnlfEnter { player_object: u32 },
    /// 18, 19, 20, 23 (ARTR): `0x234270` must hold (the instance executed). 18: `+0x18 = -R+0xC`, `+0xAC = 1`.
    /// 19: `+0x10 = +0x18 = -R+0xC`, `+0xA8 = 0`, `+0xAC = 1`. 20: `+0xA8 = 1.7`. 23: `[object+4]+0x30 = R+0xC bits`.
    ArtrControl { op: ArtrOp, value: f32, bits: u32 },
    /// 21, 22 (LPTR): `0x234270` must hold. 21: `+0x18 = R+0xC - 5.46`, then `0x1EDDB0` and `0x1EDB90(1)` flag the tracked handles
    /// (`+0x28 |= 0x61`). 22: `+0x10 = R+0xC - 5.46` (0 when R+0xC is 0), `+0x18 = +0x10`, `+0xA8 = 0`.
    LptrControl { op: LptrOp, value: f32 },

    // ---- scene scripts (level specific props)
    /// 4 (`0x1E4D80`): "DRAW" object state machine: states 0 and 1 spawn the "B_DB" effect record, start sounds 0x1E1 and 0x1E2 (handle cached at `+0x22C`), OR 0x4000000 into `+0x28`, state 2; states 2 and 3 go to state 1.
    DrawToggle,
    /// 5 (`0x1DE790`): slot 0's class 0x254 actor (`actor`) gets flag 0x4000000, state `+0xAC = 1`, `[[0x75BC04]+0x654] = 1`, and four particles are started
    /// from room 0x41 entries 6 to 9 into `[0x75BC04]+0xCC` (live handles are refreshed by `0x10E6E0`, vector pair by entry index).
    RoomParticles { actor: u32 },
    /// 6 (`0x1F20A0`): stop the cached sound `[0x50025C]` and set it to -1.
    StopCachedSound,
    /// 9 and 29 (`0x1DE520`, `0x210820`): fade slot weather in over 0.5 s (`0x3445E0(handle, 0.5)`).
    WeatherFadeIn { which: WeatherSlot },
    /// 24 (`0x1F4320`): "TRST" entry reset (`0x966E0`) and, when the "HTIP" object is the player's own in state 0x4C: reactivate slot 0xF, finish slot 0x10, health 1.4;
    /// then swap the entry's tick callbacks (`0x1EBF90`, `0x1EC250`), state 2 or 3, timer, sound 0x3AA.
    TrstReset,
    /// 25 (`0x1F2AA0`): pop the pair stack `[0x75BF10]` into `[0x75BC20]` and `[0x75BFD4]` (R+0xC low 16 bits, R+0x10).
    PopPair { first: u32, second: u32 },
    /// 26 (`0x1F2AE0`): `[0x75BD9C] = (R+0xC != 0)`.
    SetFlag75BD9C(bool),
    /// 35 (`0x1DD420`): once per scene (word `+0x20` of `[0x75B0C4]` is 0) create effect 0x1F and mark the scene (`+0x20 = 1`).
    SceneEffectOnce,
    /// 36 (`0x1D5ED0`): scene state 1 becomes 2.
    ScenePromote,
    /// 38 (`0x1D84E0`): "ATFL" object breaks into 10 pieces (`0xA6EC0` x10 with collision bit 1 << i, impulse down, 10.0 s life) and is destroyed.
    BreakAtfl,
    /// 39 (`0x1D5E10`): `[0x75B4BC] = 1`, second draw pass of entity array `[0x75B4B4]`.
    EnableSecondPass,
    /// 40 (`0x3661F0`): `[[0x76C238] + 0x3364] = 1` (scripted blast active).
    BlastActive,
    /// 41 (`0x1D9600`): every "PLNK" object (up to 9) without a child gets one (`0x16C170(obj, 1)`) attached and an impulse.
    PlankPhysics,
    /// 42 (`0x1D8C40`): "TRAP" object armed: `+0xC0 = [0x49DEDC]`, `+0xE0 = 0`, byte `+0x32 = 2`, `+0x1A0 = 0`, tick `0x1D8AB0`.
    ArmTrap,
    /// 45 (`0x24A100`): five effect records `0x51AF4C..0x51AF88` (`0x249C00`), then `[0x78B6D4] = 1`.
    SpawnFixedEffects,
    /// 50 (`0x1D8A10`): "TREE" object: `0x1D8660`, `+0xC8 = [0x475D24]`, `+0xE0 = 0`, byte `+0x32 = 2`, `+0x1A0 = 1`, particle table `0x78F500`, tick `0x1D88B0`.
    TreeEffect,
    /// 51 (`0x1FB860`): swap puzzle reset: "SWWL" entry (animation 1, `+0x1A4 = 0`, `+0xD0 = 1.0`, three tick swaps), "SWWT" object, "DIAL" and "DGBT" entry; up to 10 "DMLT" objects randomised to bit 1 << (rand & 1).
    SwwlReset,
    /// 52 (`0x1FB5E0`): "DGBT" and "DGTR" objects swap callbacks (`0x1F8830`, `0x1FB490`, `0x1F8880`), `[object+4]+0x30 = 0x1C`, up to 2 "DRAM" objects get tick `0x1FB3D0`.
    DgbtDgtr,
    /// 53 (`0x1F8C60`): "DGBT" sub state 0x1C, up to 2 "DARM" objects get `+0x1A4 = [0x47CFF0]` and tick `0x1F88E0`.
    DgbtDarm,
    /// 54 (`0x1FBD60`): "EGSW" entry enabled (`+0x10 |= 1`), ticks on EGSW and "EENG", sound 0x615 at EENG, then slot 1 is finished.
    EgswStart,
    /// 55 (`0x1FBC80`): "BDEG"/"BDBT" entries, sound 0x607 at BDEG, ticks on BDEG, "BFAN" and BDBT, BDBT sub state 2.
    BdegStart,
    /// 56 (`0x1FBC20`): "BDBT" entry ticks, sub state 1, toggle its linked prop (`0xB8590`), then slot 3 is finished.
    BdbtStart,
    /// 1 (`0x1E1EE0`, per-tick callback of spawned objects, 16 uses): restart the ambient sound of the object every tick from the config block
    /// `[0x75BC04]`: sound id 0x1629 for object type 0xFB else `+0x678`, pitch `object+0xFC / 3 + 1`, falloff `+0x67C`, volume `+0x680 * 1.5`, position
    /// 10 units below the object (`0x468A0`, handle kept at `+0x22C`). With game flag `[0x7DE458] & 0x40` and a finished parent (`0x59DD0`) the object's destroy
    /// callback (`+0x158`) runs, its slot 1 is cleared and the object is destroyed (`0xA9070`).
    AmbientSoundTick,
    /// 2 (`0x1E1E00`, 1 use): every tick copy the material words `+0x4 -> +0xDC` and `+0x108` of the object onto the tagged "DRIV" and "PASS"
    /// objects and restart sound 0x1A0 (volume 2.0, range 80.0) attached to the object (`0x468A0`, `0x454B0`). The vector length computed first is unused (dead code).
    DriverCopyTick,
    /// 3 (`0x1E21E0`, 1 use): runs id 1, then spark emitter for type "SPEN" objects (`0x1E20B0`: accumulate the frame delta in `+0x1A4`; below z -2 after 2.0 or
    /// below -15 after 1.0 (and above -50) spawn two effects (table `0x7AC660`) at x +- 1.5, y -49.61 and reset). Types 0x270 and 0x26F instead aim at "PLNE" objects
    /// within 300 units whose direction dot product with the emitter direction exceeds 0.8 (player within 200 units, dot above 0.9, for type 0x270 only), every 8th
    /// (0x270) or 15th (0x26F) tick, spraying jittered effect 4 (`0xEA9C0`) and sound 0x19C.
    SparkTick,
    /// Slot callback 5, code 2 (`0x215660`): copy the "SHTM" object's state (`+0x1A0`, position `+0x64..+0x6C`, `+0xC0`, `+0x70`, `+0xC8`, `+0x1CC`)
    /// into the globals `0x78D340..0x78D35C`; -1 in `0x78D340` when the object does not exist.
    ShtmSave,
    /// Slot callback 5, code 1 (`0x2156F0`): when `0x78D340` is not -1, fire the event with id "ETRC" (`0x2378D0`, the engine does it) and copy the saved
    /// state back into the "SHTM" object.
    ShtmRestore,
    /// Slot callback 6, codes 1 and 2 (`0x221A10`): the "ZCTP" character's health (`+0x1CC`) is stored in the float `0x50A69C`.
    ZctpHealth,
    /// 57, 64 (`0x1FD530`, `0x1E4BA0`): `0xB86A0(entry(key), words[1])`. 57 uses key words[0], 64 the fixed key "PER1".
    LinkEntry { key: u32, object_word: u32 },
    /// 63 (`0xB8E90`) and 66 (`0xB8EE0`): find the `0x7B2780` entry whose first word is `key` (63: link A id R+0x44, 66: words[0]) and toggle its
    /// linked prop with the first player (`0xB8590`, 63) or link it to `words[1]` (`0xB86A0`, 66). A miss passes a null entry.
    EntryLookup {
        key: u32,
        link: bool,
        object_word: u32,
    },
    /// 68 and 69 (`0x1E4BC0`, `0x1E4C80`): first use only (flag word at `[0x75BC04] + 0x660` / `+0x65C`): camera target 5.5 s ("PCEC") or freeze
    /// ("RSUB"), slot 0xB / 0xA finished (SlotOp), sound 0x1E7 at the entry, flag set. Always then toggle the entry's linked prop (`0xB8590`).
    FirstUseEntry {
        entry: u32,
        camera: u32,
        freeze: bool,
        flag_offset: u32,
        sound: u32,
    },
    /// 70 (`0x1F1D90`): first use of "KHLR" (flag `[0x75BFE8]`): camera "GODC" 3.0, props "GSD1" and "BARD" reset, then toggle the linked prop.
    KhlrFirstUse,
    /// 72 (`0x1F73F0`): "NKLC" entry whose object is type 0x32C with `+0xD0 == 0` swaps ticks (`0x1EB240`, `0x1F2B00`). Otherwise the
    /// difficulty table entry `0x4FFE58[[0x7DE49A]]` selects: equal to the type, the "NUKE" object goes to state 1 (weight 0) unless it is state 4;
    /// else health is cleared for type 0x32C and `0x1EB1F0(nuke)` runs.
    NuklcSetup,
    /// 73 (`0x1F3360`): "ROCK" in state 4 and "CONS" entry present: entry object health 0, ROCK state 0, `0x24C920` constants, ticks `0x1EB240`, `0x1EB9F0`, sound 0xD6C.
    RockReset,
    /// 80 (`0x225350`): entry of key `words[0]` (instance word `+0x40`, no null check in the original, which would fault on a missing key): animation 1, `+0x1A4 = 0`, `+0xD0 = 3.0`, ticks `0x223EE0`, `0x225260`, entry `+0x40 = 0`.
    EntryAnimInit { key: u32 },
    /// 90 (`0x20E430`): global effect `P = [0x75C0A0]`: R+0xC non-zero starts particle table `P+0x108` at (-121.5, 3.67, 15.3) dir (0, -1, 0) and `P+0x450 = 1.0`, zero stops it.
    GlobalParticle { start: bool },
    /// 95 (`0x21C040`): toggle two per-index flags (`0xFA360`), bits 0 of two bytes at `+0x29BA`, `+0x2A1A`; sound 0x351 when R+0xC is non-zero.
    ToggleIndexFlags { on: bool },
    /// 96 (`0x208A20`): "SEMM" entry: `[object + 0x1D4] = 0`.
    SemmClear,
    /// 61 (`0x2160D0`): two fixed effect spawns of type 0x12 (`0xEFBD0`).
    TwoEffects,
    /// 58 (`0x2021B0`): the object of key R+0x14 starts effect 0x4F6 at its position (handle to `+0x22C`) and gets ticks `0x201D70`, `0x201CD0`.
    ObjectLoopSound { tag: u32 },
}

/// ARTR operations (ids 18, 19, 20, 23).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtrOp {
    Start,
    Reset,
    Weight,
    CopyBits,
}

/// LPTR operations (ids 21, 22).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LptrOp {
    Start,
    Set,
}

/// Weather handle sources of ids 9 and 29: `[[0x75BC04]+0x4FC]` and `[0x75C0B8]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherSlot {
    Scene,
    Level,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Slot(SlotOp),
    World(WorldOp),
}

/// A callback that cannot be decoded: reported, never guessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallbackGap {
    pub id: u32,
    pub reason: &'static str,
}

const NOT_ON_DISC: &str =
    "id occurs on no definition of the 55 SETU paks and its body was not decoded";

/// Ids of the table that occur on the disc (T1842 census over the 55 SETU paks), each decoded by [`decode`].
pub const DISC_IDS: [u32; 83] = [
    1, 2, 3, 4, 5, 6, 7, 9, 10, 12, 13, 16, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 31, 32,
    33, 35, 36, 38, 39, 40, 41, 42, 45, 47, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62,
    63, 64, 65, 66, 67, 68, 69, 70, 72, 73, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88,
    90, 91, 92, 93, 95, 96, 97, 98, 99,
];

fn object(op: WorldOp) -> Vec<Step> {
    vec![Step::World(op)]
}

fn set_state(tag: u32, word: StateWord, value: u32) -> Vec<Step> {
    object(WorldOp::SetObjectState { tag, word, value })
}

/// Decode callback `id` for the given context.
pub fn decode(id: u32, ctx: &CallbackContext) -> Result<Vec<Step>, CallbackGap> {
    let r = &ctx.record;
    let key_c = r.arg(0xC);
    let key_14 = r.arg(0x14);
    let event_key = r.event_id;
    let first_float = r.arg_f32(0xC);
    let steps = match id {
        1 => object(WorldOp::AmbientSoundTick),
        2 => object(WorldOp::DriverCopyTick),
        3 => object(WorldOp::SparkTick),
        4 => object(WorldOp::DrawToggle),
        5 => vec![
            Step::Slot(SlotOp::SetCurrent(0)),
            Step::World(WorldOp::RoomParticles {
                actor: ctx.class_254_actor,
            }),
        ],
        6 => object(WorldOp::StopCachedSound),
        7 => object(WorldOp::PlayerCameraCue {
            camera: tag(b"CRCM"),
        }),
        9 => object(WorldOp::WeatherFadeIn {
            which: WeatherSlot::Scene,
        }),
        10 => object(WorldOp::DamageObject {
            tag: event_key,
            amount: 1000.0,
            kind: 4,
            direction: [0.0, 1.0, 0.0],
        }),
        12 => object(WorldOp::PlayerCameraCue {
            camera: tag(b"BRSC"),
        }),
        13 => object(WorldOp::WnlfReset),
        16 => object(WorldOp::DestroyTrcp),
        18 | 19 | 20 | 23 => {
            if !ctx.executed {
                return Ok(Vec::new());
            }
            let op = match id {
                18 => ArtrOp::Start,
                19 => ArtrOp::Reset,
                20 => ArtrOp::Weight,
                _ => ArtrOp::CopyBits,
            };
            object(WorldOp::ArtrControl {
                op,
                value: first_float,
                bits: key_c,
            })
        }
        21 | 22 => {
            if !ctx.executed {
                return Ok(Vec::new());
            }
            let op = if id == 21 { LptrOp::Start } else { LptrOp::Set };
            object(WorldOp::LptrControl {
                op,
                value: first_float,
            })
        }
        24 => object(WorldOp::TrstReset),
        25 => object(WorldOp::PopPair {
            first: key_c & 0xFFFF,
            second: r.arg(0x10),
        }),
        26 => object(WorldOp::SetFlag75BD9C(key_c != 0)),
        27 => {
            if ctx.executed {
                vec![Step::Slot(SlotOp::Finish(10))]
            } else {
                Vec::new()
            }
        }
        28 => object(WorldOp::NpcGoal { character: key_c }),
        29 => object(WorldOp::WeatherFadeIn {
            which: WeatherSlot::Level,
        }),
        31 => object(WorldOp::JbpkReset),
        32 | 33 => {
            let mode = if r.flags & 0x80_0000 != 0 { 5 } else { 1 };
            object(WorldOp::ResetInputDriven {
                player: id - 32,
                character: key_c,
                mode,
            })
        }
        35 => object(WorldOp::SceneEffectOnce),
        36 => object(WorldOp::ScenePromote),
        38 => object(WorldOp::BreakAtfl),
        39 => object(WorldOp::EnableSecondPass),
        40 => object(WorldOp::BlastActive),
        41 => object(WorldOp::PlankPhysics),
        42 => object(WorldOp::ArmTrap),
        45 => object(WorldOp::SpawnFixedEffects),
        47 => object(WorldOp::EnableExtensionState { tag: event_key }),
        49 => object(WorldOp::PlayerCameraCueLevel {
            camera: tag(b"TFLC"),
        }),
        50 => object(WorldOp::TreeEffect),
        51 => object(WorldOp::SwwlReset),
        52 => object(WorldOp::DgbtDgtr),
        53 => object(WorldOp::DgbtDarm),
        54 => vec![
            Step::World(WorldOp::EgswStart),
            Step::Slot(SlotOp::Finish(1)),
        ],
        55 => object(WorldOp::BdegStart),
        56 => vec![
            Step::World(WorldOp::BdbtStart),
            Step::Slot(SlotOp::Finish(3)),
        ],
        57 => object(WorldOp::LinkEntry {
            key: ctx.words[0],
            object_word: ctx.words[1],
        }),
        58 => object(WorldOp::ObjectLoopSound { tag: key_14 }),
        59 => object(WorldOp::GiveItemToCharacter {
            character: key_c,
            item: r.arg(0x10),
        }),
        60 => object(WorldOp::GroupTrigger { tag: event_key }),
        61 => object(WorldOp::TwoEffects),
        62 => object(WorldOp::ActivateNfc4),
        63 => object(WorldOp::EntryLookup {
            key: r.link_a.id,
            link: false,
            object_word: 0,
        }),
        64 => object(WorldOp::LinkEntry {
            key: tag(b"PER1"),
            object_word: ctx.words[1],
        }),
        65 => object(WorldOp::WnlfEnter {
            player_object: ctx.words[1],
        }),
        66 => object(WorldOp::EntryLookup {
            key: ctx.words[0],
            link: true,
            object_word: ctx.words[1],
        }),
        67 => object(WorldOp::PlayerEnterScene {
            player_object: ctx.words[1],
        }),
        68 => vec![
            Step::World(WorldOp::FirstUseEntry {
                entry: tag(b"PCL1"),
                camera: tag(b"PCEC"),
                freeze: false,
                flag_offset: 0x660,
                sound: 0x1E7,
            }),
            Step::Slot(SlotOp::Finish(0xB)),
        ],
        69 => vec![
            Step::World(WorldOp::FirstUseEntry {
                entry: tag(b"PCL2"),
                camera: tag(b"RSUB"),
                freeze: true,
                flag_offset: 0x65C,
                sound: 0x1E7,
            }),
            Step::Slot(SlotOp::Finish(0xA)),
        ],
        70 => object(WorldOp::KhlrFirstUse),
        72 => object(WorldOp::NuklcSetup),
        73 => object(WorldOp::RockReset),
        75 => set_state(key_14, StateWord::Script, 1),
        76 => set_state(key_14, StateWord::Script, 0),
        77 => object(WorldOp::PlayerFreezeAim {
            player_object: ctx.words[1],
        }),
        78 => object(WorldOp::SoundAdvance {
            tag: tag(b"STRS"),
            sound: 0x374,
            plays_on: [true, true, true, true],
        }),
        79 => object(WorldOp::SoundAdvance {
            tag: tag(b"SRMP"),
            sound: 0x36F,
            plays_on: [true, false, false, true],
        }),
        80 => object(WorldOp::EntryAnimInit { key: ctx.words[0] }),
        81 => object(WorldOp::PlaceNumberedProps { first_tag: key_14 }),
        82 => object(WorldOp::PlayerHudRegister),
        83 => object(WorldOp::PlayerHudUnregister),
        84 => object(WorldOp::BridgeReset { mode: event_key }),
        85 => set_state(tag(b"LKDL"), StateWord::Subrecord, 2),
        86 => object(WorldOp::RemoveKeyedObject { tag: key_14 }),
        87 => object(WorldOp::RequestObjectState {
            tag: key_14,
            state: 0x2000,
        }),
        88 => object(WorldOp::ApplyEffectMode {
            group: key_c,
            tag: key_14,
            mode: event_key,
        }),
        90 => object(WorldOp::GlobalParticle { start: key_c != 0 }),
        91 => set_state(tag(b"MTHS"), StateWord::Script, 2),
        92 => object(WorldOp::TramStart),
        93 => object(WorldOp::ToggleScriptState {
            tag: key_14,
            mask: key_c,
        }),
        95 => object(WorldOp::ToggleIndexFlags { on: key_c != 0 }),
        96 => object(WorldOp::SemmClear),
        97 => set_state(tag(b"XFKY"), StateWord::Subrecord, 1),
        98 => vec![Step::Slot(SlotOp::StartTimed {
            slot: key_c,
            value: r.arg_f32(0x10),
        })],
        99 => vec![Step::Slot(SlotOp::FinishTimed)],
        0 => return Ok(Vec::new()),
        _ => {
            return Err(CallbackGap {
                id,
                reason: NOT_ON_DISC,
            });
        }
    };
    Ok(steps)
}

/// Is `id` decodable? Used by documentation tests and the census.
pub fn is_decoded(id: u32) -> bool {
    let record =
        EventRecord::from_bytes(&[0u8; super::record::RECORD_SIZE]).expect("zero record parses");
    let ctx = CallbackContext {
        record,
        executed: true,
        words: [0; 2],
        class_254_actor: 0,
    };
    decode(id, &ctx).is_ok()
}

/// Static slot callback ids of table `0x4D2CA0` that occur on the disc (one slot each, 69 slots have none).
pub const SLOT_DISC_IDS: [u32; 9] = [1, 2, 3, 4, 5, 6, 7, 8, 9];

/// Static slot callback id `id` called with `code` (0 level load `0x227B50`, 1 restore `0x227DE0`, 2 save `0x227C90`
/// by the checkpoint capture `0x25EFBC`, 3 per-frame poll `0x2282B0`, 4 finish `0x2274C0`). Ids 1 to 4 and 7 to 9 point at
/// the shared no-op `0x389B0`. Ids 5 and 6 act on codes 1 and 2 only. Ids 10 and up are not used by any slot on the disc.
pub fn decode_slot_callback(id: u32, code: u32) -> Result<Vec<WorldOp>, CallbackGap> {
    match (id, code) {
        (0..=4 | 7..=9, _) => Ok(Vec::new()),
        (5, 2) => Ok(vec![WorldOp::ShtmSave]),
        (5, 1) => Ok(vec![WorldOp::ShtmRestore]),
        (5, _) => Ok(Vec::new()),
        (6, 1 | 2) => Ok(vec![WorldOp::ZctpHealth]),
        (6, _) => Ok(Vec::new()),
        _ => Err(CallbackGap {
            id,
            reason: "slot callback id occurs on no slot of the 55 SETU paks and its body was not decoded",
        }),
    }
}
