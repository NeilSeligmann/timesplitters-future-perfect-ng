//! Story level hooks, part 1 (T1845, batch 5): levels 1, 2, 4, 5, 9, 0xE, 0x12, 0x1A, 0x40 and 0x50.
//!
//! Static reading of `build/default.xbe` (SHA-256 `3cfd001a...1816bc`), no xemu run. Control flow, tags, ids and
//! constants are MEASURED; the game meaning of a field is INFERRED. Hooks are modelled as functions from a read-only
//! [`HookView`] to a list of [`HookEffect`], so a headless world can run them. Callees that were not decoded are
//! emitted as [`HookEffect::Unsupported`] (never silently skipped). Details: `docs/t-story-event-hooks-story.md`.
//!
//! Conventions: object tags are the 4 character keys of the level scripts, written high byte first (`0x44423031` is
//! "DB01"). "Player 0 area" is the dword `+0xF54` of the first local player's block (`[0x7B0C7C]`).

/// One NPC found by tag (`0x163730` / `0x15BA10` / `0x23FF20`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NpcView {
    /// Model record id, the first dword of `0x59CF0(object)`.
    pub model: u32,
    /// `0x61570(object, 1)` is non-zero (the object is alive and active).
    pub alive: bool,
}

/// Read-only world facts the hooks consult.
pub trait HookView {
    /// `0x2273C0(slot)`: the static slot is in status 5 (finished).
    fn slot_finished(&self, slot: u32) -> bool;
    /// Object by tag, `None` when absent.
    fn npc(&self, tag: u32) -> Option<NpcView>;
    /// Dword `+0xF54` of local player 0's block, `None` without a player block.
    fn player_area(&self) -> Option<u32>;
    /// Build/mode byte `[0x7DE455]`.
    fn mode_byte(&self) -> u8 {
        0
    }
    /// Game flag word `[0x7DE458]`.
    fn game_flags(&self) -> u32 {
        0
    }
    /// Object by tag whose `0x61570(object, mask)` test passes (the `mask` is the second argument: 1 for alive,
    /// `0x2000` for the "armed" test used by the level 5 and level 0x1A weapon hand-overs).
    fn npc_gate(&self, _tag: u32, _mask: u32) -> Option<NpcView> {
        None
    }
    /// Entry `0x3EC` of table `0x6B9908` (`0x44850`): `Some(flag)` with the flag being bit 0 of the dword `+0x60`.
    fn table_3ec_flag(&self) -> Option<bool> {
        None
    }
}

/// An effect a hook asks the world for.
#[derive(Clone, Debug, PartialEq)]
pub enum HookEffect {
    /// `0x2274C0(slot, arg)`: finish the static slot.
    SlotFinish { slot: u32, arg: u32 },
    /// `0x227650(slot, value)`: promote status 1 to 2.
    SlotPromote { slot: u32, value: f32 },
    /// `0x2276E0(slot, arg)`: set status 4.
    SlotSetStatus4 { slot: u32, arg: u32 },
    /// NPC AI state dword `ctrl+0x94` set, and one byte of the controller cleared.
    NpcState {
        tag: u32,
        state: u32,
        clear_byte: u32,
    },
    /// Only the byte is cleared and the state reset to 0 (`state` kept for clarity).
    /// `0x1BA220(ctrl, ammo_type, amount)`: add ammo.
    NpcAmmo {
        tag: u32,
        ammo_type: u32,
        amount: u32,
    },
    /// Object flag word `+0x28` bit(s) set or cleared.
    ObjectFlag { tag: u32, mask: u32, set: bool },
    /// Start the level ambience sound when the handle `[0x507DB8]` is negative.
    StartAmbience { sound_id: u32 },
    /// `0x352120(object, 0x7363316A, 3, model+0x30, vec3)` attach for the object tagged `tag`.
    AttachScript { tag: u32, vector: [f32; 3] },
    /// `0x1515E0(state, target, step)` then `0x468A0(handle, 0, sound, state, 50.0, 1.0, 0)`: move the loop volume
    /// `state_addr` towards `target` by `step` and set the loop `handle_addr` playing at that volume.
    ApproachLoop {
        state_addr: u32,
        handle_addr: u32,
        sound: u32,
        target: f32,
        step: f32,
    },
    /// A call into the weapon/ammo helpers on the controller of `subject` (a tag, or 0 for local player 0's linked
    /// object `[[0x7B0C7C]+0x14]+0x7C`): `callee(controller, args...)`.
    ControllerCall {
        subject: u32,
        callee: u32,
        args: Vec<u32>,
    },
    /// `0x879C0(i)` for the three entries: word `+0x16` of entry `i` of the table at `[0x70878C]` becomes `0xFFFF`.
    TableWordFfff { entries: [u32; 3] },
    /// A decoded callee that needs run-time state the view does not carry: the world runs the named port function
    /// (T1845 round 4, see [`callee_port`]).
    Call { callee: u32, port: &'static str },
    /// A callee whose body was not decoded (address and reason).
    Unsupported { callee: u32, reason: &'static str },
    /// Tail jump into an undecoded function.
    TailUnsupported { callee: u32, reason: &'static str },
}

/// Every decoded callee of the story and big level hooks and the port function that models it (T1845 round 4).
/// `Call` effects carry this name; a callee missing here is genuinely undecoded.
pub const CALLEE_PORTS: &[(u32, &str)] = &[
    (
        0x36B7A0,
        "hooks_scene::blast_step (frame, 0x368A10 update and 0x369940 pieces; geometry: face_geometry::RoomGeometry)",
    ),
    (
        0x1E2B80,
        "prelude_1e2b80::prelude_1e2b80 (T1846 batch 8, effects through PreludeEnv)",
    ),
    (0x1DAAB0, "hooks_scene::worm_frame"),
    (0x1DD510, "hooks_scene::beam_frame"),
    (
        0x24ABA0,
        "hooks_fx::wrth_frame (state 1 integrator only, rest Unsupported there)",
    ),
    (0x1D7A70, "hooks_audio::storm_hook"),
    (0x21D520, "hooks_fx::level5_spark_tick"),
    (0x21C110, "hooks_fx::spark_pool_tick"),
    (
        0x1FEB00,
        "hooks_fx::bridge_hulls (hull recipe; matrix inputs by the world)",
    ),
    (0x1F9DA0, "hooks_fx::roof_tick"),
    (0x1F84E0, "hooks_fx::htip_step"),
    (0x216230, "hooks_story::timed_fx_step"),
    (0x219CA0, "hooks_story::sky_streak_tick"),
    (0x218C50, "hooks_story::proximity_effect_step"),
    (0x21C580, "hooks_story::repel_close_objects"),
    (0x1FD350, "hooks_story::spark_burst_step"),
    (0x1FD1B0, "hooks_story::spark_burst_step"),
    (0x223FB0, "hooks_story::level9_music_targets"),
    (0x218910, "hooks_story::weapon_handover"),
    (0x20B480, "hooks_l06_0b (ambience fade)"),
    (0x208AF0, "hooks_big::level06_fade_step"),
    (0x2085B0, "hooks_l06_0b::level06_npc_rules"),
    (0x208810, "hooks_l06_0b (SW00..SW05 latches)"),
    (0x1EA640, "hooks_big::level0b_music_targets"),
    (0x1EC300, "hooks_l06_0b (zone barrier)"),
    (0x1F1EC0, "hooks_l06_0b::level0b_trn3"),
    (0x1F1F70, "hooks_l06_0b::nuke_countdown_slots"),
    (0x1EFE90, "hooks_l06_0b::level0b_fbn_slot"),
    (0x1F0290, "hooks_l06_0b::level0b_pete"),
    (0x1EAB10, "hooks_l06_0b (camera shake)"),
    (0x213900, "hooks_l11::jbth_tally"),
    (0x213580, "hooks_l11::cz_flag_pass"),
];

/// The port function of a decoded callee.
pub fn callee_port(address: u32) -> Option<&'static str> {
    CALLEE_PORTS
        .iter()
        .find(|row| row.0 == address)
        .map(|row| row.1)
}

fn ported(out: &mut Vec<HookEffect>, callee: u32) {
    let port = callee_port(callee).expect("callee is in CALLEE_PORTS");
    out.push(HookEffect::Call { callee, port });
}

/// One row of an NPC rule table: when the object's model is in `models`, set AI state `state`, clear controller byte
/// `clear_byte`, optionally add ammo.
#[derive(Clone, Copy, Debug)]
pub struct NpcRule {
    pub tag: u32,
    pub models: &'static [u32],
    pub state: u32,
    pub clear_byte: u32,
    pub ammo: Option<(u32, u32)>,
}

/// Apply rules for a tag: only an alive object is touched (`0x61570(object, 1)`).
pub fn apply_npc_rules(view: &dyn HookView, rules: &[NpcRule], out: &mut Vec<HookEffect>) {
    for rule in rules {
        let Some(npc) = view.npc(rule.tag) else {
            continue;
        };
        if !npc.alive || !rule.models.contains(&npc.model) {
            continue;
        }
        out.push(HookEffect::NpcState {
            tag: rule.tag,
            state: rule.state,
            clear_byte: rule.clear_byte,
        });
        if let Some((ammo_type, amount)) = rule.ammo {
            out.push(HookEffect::NpcAmmo {
                tag: rule.tag,
                ammo_type,
                amount,
            });
        }
    }
}

/// Tags of the 12 objects of `0x1D90D0`, in order: "DZ01".."DZ10", "TBL1", "TBL2" (MEASURED).
pub const DZ_TBL_TAGS: [u32; 12] = [
    0x445A_3031,
    0x445A_3032,
    0x445A_3033,
    0x445A_3034,
    0x445A_3035,
    0x445A_3036,
    0x445A_3037,
    0x445A_3038,
    0x445A_3039,
    0x445A_3130,
    0x5442_4C31,
    0x5442_4C32,
];

/// `0x1D90D0` (level 1, areas 4 and 5; MEASURED): for each tag, if the object exists its latch (`0x75B14C + 4*i`) becomes 1
/// and the object's flag bit 0 (`+0x28`) is cleared for model `0x15A`, set for `0x159` and `0x15B`; if it is absent and
/// the latch is 1 the latch becomes 2. `latches` is updated in place.
pub fn dz_tbl_sync(view: &dyn HookView, latches: &mut [u32; 12]) -> Vec<HookEffect> {
    let mut out = Vec::new();
    for (index, &tag) in DZ_TBL_TAGS.iter().enumerate() {
        match view.npc(tag) {
            Some(object) => {
                latches[index] = 1;
                match object.model {
                    0x15A => out.push(HookEffect::ObjectFlag {
                        tag,
                        mask: 1,
                        set: false,
                    }),
                    0x159 | 0x15B => out.push(HookEffect::ObjectFlag {
                        tag,
                        mask: 1,
                        set: true,
                    }),
                    _ => {}
                }
            }
            None if latches[index] == 1 => latches[index] = 2,
            None => {}
        }
    }
    out
}

/// `0x21B540` (level 5; MEASURED): "R110" (`0x52313130`) passing `0x61570(.., 0x2000)` with model `0x7CB` hands local
/// player 0's linked object 8 clips of weapon entry `0x19` (`0x1BA2B0`), collects/selects it (`0x1BFCA0`), makes the
/// "R110" controller select entry `0x19` (`0x1BA130`), releases its entry `0x1B` (`0x1BFC00`) and acquires entry `0x38`
/// for the player (`0x1BFAA0`).
pub fn level5_weapon_handover(view: &dyn HookView) -> Vec<HookEffect> {
    let tag = 0x5231_3130;
    let mut out = Vec::new();
    if matches!(
        view.npc_gate(tag, 0x2000),
        Some(NpcView { model: 0x7CB, .. })
    ) {
        out.push(HookEffect::ControllerCall {
            subject: 0,
            callee: 0x1BA2B0,
            args: vec![0x19, 8],
        });
        out.push(HookEffect::ControllerCall {
            subject: 0,
            callee: 0x1BFCA0,
            args: vec![0x19],
        });
        out.push(HookEffect::ControllerCall {
            subject: tag,
            callee: 0x1BA130,
            args: vec![0x19],
        });
        out.push(HookEffect::ControllerCall {
            subject: tag,
            callee: 0x1BFC00,
            args: vec![0x1B],
        });
        out.push(HookEffect::ControllerCall {
            subject: 0,
            callee: 0x1BFAA0,
            args: vec![0x38],
        });
    }
    out
}

/// `0x21CE80` (level 5; MEASURED): loop sound `0x20` (handle `0x75C0F8`, state `0x75C0F4`) approaches 0.25 while game flag
/// `0x200` is set, else 1.0, at 0.02 per call.
pub fn level5_loop(view: &dyn HookView) -> HookEffect {
    let target = if view.game_flags() & 0x200 != 0 {
        0.25
    } else {
        1.0
    };
    HookEffect::ApproachLoop {
        state_addr: 0x75C0F4,
        handle_addr: 0x75C0F8,
        sound: 0x20,
        target,
        step: 0.02,
    }
}

/// `0x1F8320` (level 0x12; MEASURED): targets for the loops `7` (state `0x75C018`, handle `0x75C01C`, step 0.03) and `8`
/// (state `0x75C020`, handle `0x75C024`, step 0.02) from player 0's area (`+0xF54`, minus 1, unsigned, jump table):
/// area 1 gives loop 7 the target 1.0 unless table entry `0x3EC` exists with flag bit 0 set; areas 2 to 16, 20, 21 and 26
/// give loop 7 1.0; areas 17 to 19, 22, 24, 25, 27 to 36 and anything above 40 give loop 8 the target 0.6; areas 23 and
/// 37 to 40 give both 0.
pub fn level12_mix(area: u32, table_3ec_flag: Option<bool>) -> [HookEffect; 2] {
    const MAP: [u8; 40] = [
        0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 3, 3, 3, 1, 1, 3, 2, 3, 3, 1, 3, 3, 3, 3,
        3, 3, 3, 3, 3, 3, 2, 2, 2, 2,
    ];
    let index = area.wrapping_sub(1);
    let case = if (index as usize) < MAP.len() {
        MAP[index as usize]
    } else {
        3
    };
    let (first, second) = match case {
        0 => (
            if table_3ec_flag == Some(true) {
                0.0
            } else {
                1.0
            },
            0.0,
        ),
        1 => (1.0, 0.0),
        2 => (0.0, 0.0),
        _ => (0.0, 0.6),
    };
    [
        HookEffect::ApproachLoop {
            state_addr: 0x75C018,
            handle_addr: 0x75C01C,
            sound: 7,
            target: first,
            step: 0.03,
        },
        HookEffect::ApproachLoop {
            state_addr: 0x75C020,
            handle_addr: 0x75C024,
            sound: 8,
            target: second,
            step: 0.02,
        },
    ]
}

/// `0x221A40` (level 0x1A; MEASURED): with "RBS1" (`0x52425331`) alive and model `0xA97` its controller selects entry `0x1B`
/// (`0x1BA130`) and releases `0x19` (`0x1BFC00`). Then, when "RBS1" passes the `0x2000` test with
/// model `0xA97`, the "PTCZ" (`0x5054435A`) controller gets state `0x19`, owned byte `0x54C` cleared, 1000 of ammo type
/// `0x12`, and the "RBS1" controller selects `0x19` and releases `0x1B`. Finally "PTCZ" alive with model `0xA93` and a state
/// other than `0x19` gets state `0x19`, byte cleared and 1000 of ammo type `0x12`. `ptcz_state` is the dword `+0x94` of the "PTCZ" controller.
pub fn level1a_scene(view: &dyn HookView, ptcz_state: u32) -> Vec<HookEffect> {
    let (rbs1, ptcz) = (0x5242_5331, 0x5054_435A);
    let mut out = Vec::new();
    if matches!(
        view.npc(rbs1),
        Some(NpcView {
            alive: true,
            model: 0xA97
        })
    ) {
        out.push(HookEffect::ControllerCall {
            subject: rbs1,
            callee: 0x1BA130,
            args: vec![0x1B],
        });
        out.push(HookEffect::ControllerCall {
            subject: rbs1,
            callee: 0x1BFC00,
            args: vec![0x19],
        });
    }
    if matches!(
        view.npc_gate(rbs1, 0x2000),
        Some(NpcView { model: 0xA97, .. })
    ) {
        out.push(HookEffect::NpcState {
            tag: ptcz,
            state: 0x19,
            clear_byte: 0x54C,
        });
        out.push(HookEffect::NpcAmmo {
            tag: ptcz,
            ammo_type: 0x12,
            amount: SCRIPT_AMMO,
        });
        out.push(HookEffect::ControllerCall {
            subject: rbs1,
            callee: 0x1BA130,
            args: vec![0x19],
        });
        out.push(HookEffect::ControllerCall {
            subject: rbs1,
            callee: 0x1BFC00,
            args: vec![0x1B],
        });
    }
    if matches!(
        view.npc(ptcz),
        Some(NpcView {
            alive: true,
            model: 0xA93
        })
    ) && ptcz_state != 0x19
    {
        out.push(HookEffect::NpcState {
            tag: ptcz,
            state: 0x19,
            clear_byte: 0x54C,
        });
        out.push(HookEffect::NpcAmmo {
            tag: ptcz,
            ammo_type: 0x12,
            amount: SCRIPT_AMMO,
        });
    }
    out
}

/// Ammo amount granted by the scripted NPC rules (1000).
pub const SCRIPT_AMMO: u32 = 0x3E8;

// ---------------------------------------------------------------- level 1: 0x1DDC90 (+ 0x1D9270)

/// `0x1D9270`: model ids of the tag "JBTH" (`0x4A425448`) mapped to AI states (MEASURED).
pub const LEVEL1_JBTH_RULES: &[NpcRule] = &[
    NpcRule {
        tag: 0x4A42_5448,
        models: &[0x17F],
        state: 0x3E,
        clear_byte: 0x54C,
        ammo: None,
    },
    NpcRule {
        tag: 0x4A42_5448,
        models: &[0x187],
        state: 0,
        clear_byte: 0x58A,
        ammo: None,
    },
    NpcRule {
        tag: 0x4A42_5448,
        models: &[0x13F, 0x190],
        state: 0x35,
        clear_byte: 0x54C,
        ammo: Some((9, SCRIPT_AMMO)),
    },
    NpcRule {
        tag: 0x4A42_5448,
        models: &[0x140, 0x191],
        state: 0,
        clear_byte: 0x581,
        ammo: None,
    },
];

/// Vector passed to the "SCI2" attach in level 1: `(-1.95, 5.26, 2.4)` (`0x49DF30`, `0x49DF2C`, `0x47D798`).
pub const LEVEL1_SCI2_VECTOR: [f32; 3] = [-1.95, 5.26, 2.4];

/// Facts of the level 1 hook that are not NPC lookups.
#[derive(Clone, Copy, Debug, Default)]
pub struct Level1Inputs {
    /// Model of "DB01" (`0x44423031`), if present.
    pub db01_model: Option<u32>,
    /// A non-player object keyed "GRDN" (`0x4752444E`) exists, with bit 0 of its state word set (word at `-0xBE0`
    /// in the story mode 10, else `-0xBD8`).
    pub grdn_flag: bool,
    /// An object tagged "SCI2" (kind 2 search `0x15BA90`) exists.
    pub sci2_present: bool,
    /// The 12 latches at `0x75B14C` (see [`dz_tbl_sync`]).
    pub dz_latches: [u32; 12],
}

/// `0x1DDC90`, the level 1 hook (MEASURED control flow). Order: `0x36B7A0`, `0x1D7A70`, area 4 or 5: `0x1D90D0` and
/// the "DB01" flag `0x10` (cleared for models `0x15C` and `0x15D`, set otherwise); area `0x14` or `0x18` with the
/// "GRDN" flag: slot 1 promoted (value 0) then status 4 unless already finished; then `0x1D9270`, `0x1DAAB0`,
/// `0x1DD510`, the "SCI2" attach and `0x24ABA0`.
pub fn level1_hook(view: &dyn HookView, inputs: &Level1Inputs) -> Vec<HookEffect> {
    let mut out = Vec::new();
    ported(&mut out, 0x36B7A0);
    ported(&mut out, 0x1D7A70);
    if let Some(area) = view.player_area() {
        if area == 4 || area == 5 {
            let mut latches = inputs.dz_latches;
            out.extend(dz_tbl_sync(view, &mut latches));
            if let Some(model) = inputs.db01_model {
                let clear = model == 0x15C || model == 0x15D;
                out.push(HookEffect::ObjectFlag {
                    tag: 0x4442_3031,
                    mask: 0x10,
                    set: !clear,
                });
            }
        }
    }
    if let Some(area) = view.player_area() {
        if (area == 0x18 || area == 0x14) && inputs.grdn_flag && !view.slot_finished(1) {
            out.push(HookEffect::SlotPromote {
                slot: 1,
                value: 0.0,
            });
            out.push(HookEffect::SlotSetStatus4 { slot: 1, arg: 0 });
        }
    }
    apply_npc_rules(view, LEVEL1_JBTH_RULES, &mut out);
    ported(&mut out, 0x1DAAB0);
    ported(&mut out, 0x1DD510);
    if inputs.sci2_present {
        out.push(HookEffect::AttachScript {
            tag: 0x5343_4932,
            vector: LEVEL1_SCI2_VECTOR,
        });
    }
    ported(&mut out, 0x24ABA0);
    out
}

// ---------------------------------------------------------------- levels 2 and 0x40: 0x21AD00

/// Ambience sound id chosen by `0x218DC0` from the mode word `[0x79094C]` (2 gives 0x10, 0x40 gives 0x1627, else 0x12).
pub fn ambience_sound_id(mode_word: u32) -> u32 {
    match mode_word {
        2 => 0x10,
        0x40 => 0x1627,
        _ => 0x12,
    }
}

/// `0x218DC0`: starts the ambience only while the handle is negative; returns the sound id to submit (`0x45820`).
pub fn start_ambience(handle: i32, mode_word: u32) -> Option<HookEffect> {
    (handle < 0).then(|| HookEffect::StartAmbience {
        sound_id: ambience_sound_id(mode_word),
    })
}

/// `0x218E60`: threshold of the "OMBH" (`0x4F4D4248`) count by the mode byte: 0 gives 7, 1 gives 0xF, 2 gives 0x19, else
/// 0xA (the count is `0x25F370(key, 0)`).
pub fn ombh_threshold(mode_byte: u8) -> u32 {
    match mode_byte {
        0 => 7,
        1 => 0xF,
        2 => 0x19,
        _ => 0xA,
    }
}

/// Inputs of the level 2/0x40 hook.
#[derive(Clone, Copy, Debug)]
pub struct Level2Inputs {
    pub ambience_handle: i32,
    pub mode_word: u32,
    pub ombh_count: u32,
    /// Game flags `[0x7DE458]` bit 0x40 (the three extra steps are skipped when set).
    pub flag_40: bool,
}

/// `0x21AD00` (levels 2 and 0x40): `0x218DC0`, `0x219CA0`, timed effect table `0x507BF8` (10 entries, `0x216230`), then
/// without game flag 0x40: `0x218C50`, `0x218910`, the "OMBH" count check (slot 5 finish) and `0x879C0` for 0xE, 0x14, 0x18.
pub fn level2_hook(view: &dyn HookView, inputs: &Level2Inputs) -> Vec<HookEffect> {
    let mut out = Vec::new();
    out.extend(start_ambience(inputs.ambience_handle, inputs.mode_word));
    out.push(HookEffect::Call {
        callee: 0x219CA0,
        port: "hooks_story::sky_streak_tick",
    });
    out.push(HookEffect::Call {
        callee: 0x216230,
        port: "hooks_story::timed_fx_step",
    });
    if !inputs.flag_40 {
        out.push(HookEffect::Call {
            callee: 0x218C50,
            port: "hooks_story::proximity_effect_step",
        });
        out.extend(weapon_handover(view));
        if inputs.ombh_count >= ombh_threshold(view.mode_byte()) {
            out.push(HookEffect::SlotFinish { slot: 5, arg: 0 });
        }
        out.push(HookEffect::TableWordFfff {
            entries: [0xE, 0x14, 0x18],
        });
    }
    out
}

/// Subject 0 of [`HookEffect::ControllerCall`]: local player 0's linked object controller.
pub const SUBJECT_PLAYER0: u32 = 0;

/// `0x218910` (MEASURED, round 4): weapon hand-over between local player 0 and the tagged NPCs "RBS1" (`0x52425331`,
/// model `0xA1C`) and "SD01" (`0x53443031`, model `0xA03`). Callees: `0x1BA2B0` (add clips of a weapon slot to the reserve),
/// `0x1BFCA0` (collect and select the weapon entry), `0x1BA130` (set the current weapon slot), `0x1BFC00` (release the
/// weapon entry). Conditions are the object tests `0x61570(object, mask)`; model = first dword of `0x59CF0(object)`.
/// Order: RBS1 mask `0x2000` (player gets weapon `0x19` with 8 clips and selects it, RBS1 selects `0x19`/releases `0x1B`),
/// RBS1 mask 1 (selects `0x1B`, releases `0x19`), SD01 mask 1 (selects `0x20`, releases `0x19`), SD01 mask 2 (SD01
/// slot 0 / release `0x20`; the player gets weapon `0x20` with 6 clips and selects it), SD01 mask `0x2000` (selects
/// `0x19`, releases 0). The player controller is `[[0x7B0C7C]+0x14]+0x7C`. Roles of weapons 0x19/0x20: INFERRED.
pub fn weapon_handover(view: &dyn HookView) -> Vec<HookEffect> {
    const RBS1: u32 = 0x5242_5331;
    const SD01: u32 = 0x5344_3031;
    let call = |subject, callee, args: &[u32]| HookEffect::ControllerCall {
        subject,
        callee,
        args: args.to_vec(),
    };
    let mut out = Vec::new();
    if view.npc(RBS1).is_some() {
        if matches!(
            view.npc_gate(RBS1, 0x2000),
            Some(NpcView { model: 0xA1C, .. })
        ) {
            out.push(call(SUBJECT_PLAYER0, 0x1BA2B0, &[0x19, 8]));
            out.push(call(SUBJECT_PLAYER0, 0x1BFCA0, &[0x19]));
            out.push(call(RBS1, 0x1BA130, &[0x19]));
            out.push(call(RBS1, 0x1BFC00, &[0x1B]));
        }
        if matches!(
            view.npc(RBS1),
            Some(NpcView {
                model: 0xA1C,
                alive: true
            })
        ) {
            out.push(call(RBS1, 0x1BA130, &[0x1B]));
            out.push(call(RBS1, 0x1BFC00, &[0x19]));
        }
    }
    if view.npc(SD01).is_some() {
        if matches!(
            view.npc(SD01),
            Some(NpcView {
                model: 0xA03,
                alive: true
            })
        ) {
            out.push(call(SD01, 0x1BA130, &[0x20]));
            out.push(call(SD01, 0x1BFC00, &[0x19]));
        }
        if matches!(view.npc_gate(SD01, 2), Some(NpcView { model: 0xA03, .. })) {
            out.push(call(SD01, 0x1BA130, &[0]));
            out.push(call(SD01, 0x1BFC00, &[0x20]));
            out.push(call(SUBJECT_PLAYER0, 0x1BA2B0, &[0x20, 6]));
            out.push(call(SUBJECT_PLAYER0, 0x1BFCA0, &[0x20]));
            out.push(call(SUBJECT_PLAYER0, 0x1BA130, &[0x20]));
        }
        if matches!(
            view.npc_gate(SD01, 0x2000),
            Some(NpcView { model: 0xA03, .. })
        ) {
            out.push(call(SD01, 0x1BA130, &[0x19]));
            out.push(call(SD01, 0x1BFC00, &[0]));
        }
    }
    out
}

/// Kept for callers of the round 1 name: the hand-over needs the view now, see [`weapon_handover`].
pub fn inventory_by_tag_ops() -> Vec<HookEffect> {
    vec![HookEffect::Call {
        callee: 0x218910,
        port: "hooks_story::weapon_handover",
    }]
}

// ---------------------------------------------------------------- levels 4 and 0x50: 0x1E40D0

/// Tag/model/state table of `0x1E40D0` for "SOLY" (`0x534F4C59`), "UEIL" (`0x4C494555`) and "OLOC" (`0x434F4C4F`):
/// the first model resets the AI state to 0 and clears byte `0x57C`, the second sets state `0x30` and clears `0x54C`.
pub const LEVEL4_PAIR_RULES: &[NpcRule] = &[
    NpcRule {
        tag: 0x534F_4C59,
        models: &[0xB61],
        state: 0,
        clear_byte: 0x57C,
        ammo: None,
    },
    NpcRule {
        tag: 0x534F_4C59,
        models: &[0xB62],
        state: 0x30,
        clear_byte: 0x54C,
        ammo: None,
    },
    NpcRule {
        tag: 0x4C49_4555,
        models: &[0xB5D],
        state: 0,
        clear_byte: 0x57C,
        ammo: None,
    },
    NpcRule {
        tag: 0x4C49_4555,
        models: &[0xB5E],
        state: 0x30,
        clear_byte: 0x54C,
        ammo: None,
    },
    NpcRule {
        tag: 0x434F_4C4F,
        models: &[0xB5F],
        state: 0,
        clear_byte: 0x57C,
        ammo: None,
    },
    NpcRule {
        tag: 0x434F_4C4F,
        models: &[0xB60],
        state: 0x30,
        clear_byte: 0x54C,
        ammo: None,
    },
];

/// "CPTA" (`0x43505441`) rules when the counter `[0x75BC04]+0x668` is zero (or `[0x75B63C]` clear): state, byte, ammo.
pub const LEVEL4_CPTA_RULES: &[NpcRule] = &[
    NpcRule {
        tag: 0x4350_5441,
        models: &[0xB4F],
        state: 0x3C,
        clear_byte: 0x551,
        ammo: None,
    },
    NpcRule {
        tag: 0x4350_5441,
        models: &[0xB58],
        state: 5,
        clear_byte: 0x588,
        ammo: Some((1, SCRIPT_AMMO)),
    },
    NpcRule {
        tag: 0x4350_5441,
        models: &[0xB8C, 0xB7A],
        state: 0,
        clear_byte: 0x551,
        ammo: None,
    },
    NpcRule {
        tag: 0x4350_5441,
        models: &[0xB8D],
        state: 5,
        clear_byte: 0x54C,
        ammo: Some((1, SCRIPT_AMMO)),
    },
];

/// Weight written to slot 1/2/4/`10` by the player class scan of `0x1E40D0` (model key ids from `0x9CC40`):
/// key `0x802` gives slot 1 value 11.0 (21.0 when several local players outside mode 10), keys `0xA89` / `0x481` give
/// slot 2 value 5.0, key `0xAE2` slot 4 value 3.0, keys `0x9CB` / `0x9E0` / `0xA42` slot 10 value 12.0 (MEASURED).
pub fn level4_class_weight(key: u32, mode_byte: u8, local_players: u32) -> Option<(u32, f32)> {
    match key {
        0x802 => Some((
            1,
            if mode_byte == 10 && local_players > 1 {
                11.0
            } else {
                21.0
            },
        )),
        0xA89 | 0x481 => Some((2, 5.0)),
        0xAE2 => Some((4, 3.0)),
        0x9CB | 0x9E0 | 0xA42 => Some((10, 12.0)),
        _ => None,
    }
}

/// Keys `0x5A0`/`0x642` or `0x737` of the player class finish slot 0x14 and set the counter `+0x670` to 1.
pub const LEVEL4_FINISH_KEYS: &[u32] = &[0x642, 0x737];

/// Counter step at `+0x670` in area `0x3F`: 1 to 3 increment, and at 4 with no "SRT2" (`0x53525432`) object slot 0xC finishes.
pub fn level4_counter_step(counter: u32) -> u32 {
    if (1..=3).contains(&counter) {
        counter + 1
    } else {
        counter
    }
}

/// Inputs of the level 4/0x50 hook.
#[derive(Clone, Copy, Debug, Default)]
pub struct Level4Inputs {
    pub controller_byte_57c: bool,
    pub player_health: f32,
    pub cpta_gate_open: bool,
    pub area: u32,
    pub counter_670: u32,
    pub srt2_present: bool,
}

/// `0x1E40D0` (levels 4 and 0x50): `0x1E2B80`, `0x1E12C0`, slot 0x15 finish when byte `+0x57C` of the player controller is 1,
/// slot 0x17 finish when the player health is at most 10.5, the NPC rule tables, area `0x3F` counter and slot 0xC.
pub fn level4_hook(view: &dyn HookView, inputs: &Level4Inputs) -> Vec<HookEffect> {
    let mut out = Vec::new();
    ported(&mut out, 0x1E2B80);
    out.push(HookEffect::Call {
        callee: 0x1E12C0,
        port: "hooks_story::ember_spawn",
    });
    if inputs.controller_byte_57c {
        out.push(HookEffect::SlotFinish { slot: 0x15, arg: 0 });
    }
    if inputs.player_health <= 10.5 {
        out.push(HookEffect::SlotFinish { slot: 0x17, arg: 0 });
    }
    apply_npc_rules(view, LEVEL4_PAIR_RULES, &mut out);
    if inputs.cpta_gate_open {
        apply_npc_rules(view, LEVEL4_CPTA_RULES, &mut out);
    }
    if inputs.area == 0x3F && level4_counter_step(inputs.counter_670) == 4 && !inputs.srt2_present {
        out.push(HookEffect::SlotFinish { slot: 0xC, arg: 0 });
    }
    out.push(HookEffect::Call {
        callee: 0x9CC40,
        port: "table 0x715F2C binary search (key to id), see level4_class_weight",
    });
    out
}

// ---------------------------------------------------------------- level 5: 0x220600

/// Inputs of the level 5 hook (`[0x78D01C]`, `[0x78D02C]`, "BIGR" object, `[0x78CED8]`, `[0x78D030]`).
#[derive(Clone, Copy, Debug, Default)]
pub struct Level5Inputs {
    pub boss_present: bool,
    pub boss_state_present: bool,
    pub bigr_found: bool,
    pub bigr_health: f32,
    pub finish_armed: bool,
    pub finish_blocked: bool,
}

/// `0x220600`: timed effect table `0x508EF8` (4 entries), `0x21D520`, `0x21B540`, `0x21C110`, `0x21CE80`; with the boss
/// state and slot 4 not finished: look up "BIGR" (`0x42494752`, kind 3); when its health is at or above 0 slot 4 finishes if armed and not blocked.
/// Without the boss state the tail call is `0x21C580`.
pub fn level5_hook(view: &dyn HookView, inputs: &Level5Inputs) -> Vec<HookEffect> {
    let mut out = Vec::new();
    ported(&mut out, 0x216230);
    ported(&mut out, 0x21D520);
    out.extend(level5_weapon_handover(view));
    ported(&mut out, 0x21C110);
    out.push(level5_loop(view));
    if !(inputs.boss_present && inputs.boss_state_present) || view.slot_finished(4) {
        out.push(HookEffect::Call {
            callee: 0x21C580,
            port: "hooks_story::repel_close_objects",
        });
        return out;
    }
    if inputs.bigr_found {
        out.push(HookEffect::Call {
            callee: 0x21C7B0,
            port: "hooks_story::boss_aim_vectors",
        });
        if inputs.bigr_health >= 0.0 && inputs.finish_armed && !inputs.finish_blocked {
            out.push(HookEffect::SlotFinish { slot: 4, arg: 0 });
        }
    }
    out
}

// ---------------------------------------------------------------- level 9: 0x224160 and its tick 0x224170

/// `0x223AD0` rule: "WORK" (`0x574F524B`) with model `0x6E1` gets state 5, byte `0x54C` cleared and 1000 ammo of type 2.
pub const LEVEL9_WORK_RULE: NpcRule = NpcRule {
    tag: 0x574F_524B,
    models: &[0x6E1],
    state: 5,
    clear_byte: 0x54C,
    ammo: Some((2, SCRIPT_AMMO)),
};

/// `0x224160`: `0x223AD0` then `0x223FB0`.
pub fn level9_hook(view: &dyn HookView) -> Vec<HookEffect> {
    let mut out = Vec::new();
    apply_npc_rules(view, &[LEVEL9_WORK_RULE], &mut out);
    out.push(HookEffect::Call {
        callee: 0x223FB0,
        port: "hooks_story::level9_music_targets",
    });
    out
}

/// `0x223FB0`: per local player the key `0x9D4A0(0x9A420(player weapon))` class table; classes with a health term subtract
/// `(clamp(health, 100, 113) - 113) * 0.0769` from the mix, others add 1. The two sound handles at `[0x75C120]` (ids 0x35 and 8)
/// get the volumes. The class jump table was not decoded: only the arithmetic is modelled.
pub fn level9_music_mix(health_terms: &[f32], plain_count: u32) -> f32 {
    let mut mix = 0.0f32;
    for &health in health_terms {
        let clamped = health.clamp(100.0, 113.0);
        mix -= (clamped - 113.0) * 0.076_923_08;
    }
    mix + plain_count as f32
}

/// State of the level 9 prop tick `0x224170`: field `+0x1A0` mode 0 falls, mode 1 rises, field `+0x70` is the scalar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Level9Tick {
    pub mode: u32,
    pub value: f32,
    pub sound_started: bool,
}

/// Result of [`level9_tick`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Level9TickResult {
    pub state: Level9Tick,
    /// The flag `0x4000000` of `+0x28` stays set (cleared on completion).
    pub busy: bool,
    pub started_sound: Option<u32>,
    pub stopped_sound: Option<u32>,
}

/// `0x224170`: `dt = (1/60) * steps`, speed 6.0 per second. Mode 1 rises to 91.0, mode 0 falls to 0; reaching the end clears
/// the busy flag, stops sound `0x378` and plays `0x379` once. Otherwise sound `0x378` is started if not running.
pub fn level9_tick(state: Level9Tick, steps: i32) -> Option<Level9TickResult> {
    if steps == 0 {
        return None;
    }
    let delta = (1.0 / 60.0) * steps as f32 * 6.0;
    let (value, done) = match state.mode {
        0 => {
            let value = state.value - delta;
            (value, 0.0 > value)
        }
        1 => {
            let value = state.value + delta;
            (value, value > 91.0)
        }
        _ => (state.value, false),
    };
    if done {
        let next = Level9Tick {
            value: 0.0,
            sound_started: false,
            ..state
        };
        // With no sound handle the completion path only sets the flag word (no stop, no 0x379).
        return Some(Level9TickResult {
            state: next,
            busy: false,
            started_sound: state.sound_started.then_some(0x379),
            stopped_sound: state.sound_started.then_some(0x378),
        });
    }
    let next = Level9Tick {
        value,
        sound_started: true,
        ..state
    };
    Some(Level9TickResult {
        state: next,
        busy: true,
        started_sound: (!state.sound_started).then_some(0x378),
        stopped_sound: None,
    })
}

// ---------------------------------------------------------------- level 0xE: 0x205BC0

/// Level 0xE phase byte `[0x75C034]` (0 none, 1 "HNA1" seen, 2 "HNA2" seen).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Level0eState {
    pub phase: u32,
}

/// Inputs for the level 0xE hook.
#[derive(Clone, Copy, Debug, Default)]
pub struct Level0eInputs {
    pub hna1: bool,
    pub hna2: bool,
    /// The record `[0x75C03C]+0x90` is non-zero.
    pub record_flag: bool,
    /// Player 0 field `+0x1568` or `+0x2AEC` of the player array is 10.
    pub player_ten: bool,
}

/// `0x205BC0`: `0x1FD060`, `0x1FEB00`; in area 3 or 4 "HNA1" (`0x484E4131`) sets phase 1 and "HNA2" (`0x484E4132`) phase 2; when a
/// tag vanishes in its phase slot 5 (HNA1) or 6 (HNA2) finishes. Then slot 3 finished: tail `0x1FD350` when the record flag is set,
/// else tail `0x1FD1B0` when the player field is 10. The MECH ("MECH" `0x4D454348`) scan of areas 0x13 and 0x14 has no effect.
pub fn level0e_hook(
    view: &dyn HookView,
    state: &mut Level0eState,
    inputs: &Level0eInputs,
) -> Vec<HookEffect> {
    let mut out = Vec::new();
    out.push(HookEffect::Call {
        callee: 0x1FD060,
        port: "hooks_story::level0e_ambient_loops",
    });
    ported(&mut out, 0x1FEB00);
    if let Some(area) = view.player_area() {
        if area == 3 || area == 4 {
            if inputs.hna1 {
                state.phase = 1;
            } else if state.phase == 1 {
                out.push(HookEffect::SlotFinish { slot: 5, arg: 0 });
            }
            if inputs.hna2 {
                state.phase = 2;
            } else if state.phase == 2 {
                out.push(HookEffect::SlotFinish { slot: 6, arg: 0 });
            }
        }
    }
    if inputs.record_flag && view.slot_finished(3) {
        out.push(HookEffect::Call {
            callee: 0x1FD350,
            port: "hooks_story::spark_burst_step",
        });
    } else if view.slot_finished(3) && inputs.player_ten {
        out.push(HookEffect::Call {
            callee: 0x1FD1B0,
            port: "hooks_story::spark_burst_step",
        });
    }
    out
}

// ---------------------------------------------------------------- level 0x12: 0x1FA9C0

/// Rules of `0x1FA9C0`: the "DP07".."DP10" tags (`0x44503037`, `38`, `39`, `3130`) of models `0x693` to `0x696` get AI state 2,
/// byte `0x54C` cleared and 1000 ammo of type 1.
pub const LEVEL12_RULES: &[NpcRule] = &[
    NpcRule {
        tag: 0x4450_3037,
        models: &[0x693, 0x695],
        state: 2,
        clear_byte: 0x54C,
        ammo: Some((1, SCRIPT_AMMO)),
    },
    NpcRule {
        tag: 0x4450_3038,
        models: &[0x693, 0x695, 0x696],
        state: 2,
        clear_byte: 0x54C,
        ammo: Some((1, SCRIPT_AMMO)),
    },
    NpcRule {
        tag: 0x4450_3039,
        models: &[0x693, 0x694],
        state: 2,
        clear_byte: 0x54C,
        ammo: Some((1, SCRIPT_AMMO)),
    },
    NpcRule {
        tag: 0x4450_3130,
        models: &[0x693],
        state: 2,
        clear_byte: 0x54C,
        ammo: Some((1, SCRIPT_AMMO)),
    },
];

/// `0x1FA9C0`: `0x1F8320`, `0x1F9DA0`, the "HTIP" (`0x48544950`) handler `0x1F84E0` (models 0x644 and 0x645 flags), the DP rules and,
/// in area 0x15 or 0x1A, slot 8 finishes when the player's linked object has model `0x667` and its field `+8` exceeds 3.0.
pub fn level12_hook(
    view: &dyn HookView,
    linked_model: Option<u32>,
    linked_value: f32,
) -> Vec<HookEffect> {
    let mut out = Vec::new();
    if let Some(area) = view.player_area() {
        out.extend(level12_mix(area, view.table_3ec_flag()));
    }
    ported(&mut out, 0x1F9DA0);
    ported(&mut out, 0x1F84E0);
    apply_npc_rules(view, LEVEL12_RULES, &mut out);
    if matches!(view.player_area(), Some(0x15) | Some(0x1A))
        && linked_model == Some(0x667)
        && linked_value > 3.0
    {
        out.push(HookEffect::SlotFinish { slot: 8, arg: 0 });
    }
    out
}

// ---------------------------------------------------------------- level 0x1A: 0x221C70

/// `0x221C70`: `0x218DC0` (ambience), `0x219CA0`, `0x216810`, `0x221A40`, tail `0x218C50`.
pub fn level1a_hook(
    view: &dyn HookView,
    ptcz_state: u32,
    ambience_handle: i32,
    mode_word: u32,
) -> Vec<HookEffect> {
    let mut out = Vec::new();
    out.extend(start_ambience(ambience_handle, mode_word));
    out.push(HookEffect::Call {
        callee: 0x219CA0,
        port: "hooks_story::sky_streak_tick",
    });
    out.push(HookEffect::Call {
        callee: 0x216230,
        port: "hooks_story::timed_fx_step",
    });
    out.extend(level1a_scene(view, ptcz_state));
    out.push(HookEffect::Call {
        callee: 0x218C50,
        port: "hooks_story::proximity_effect_step",
    });
    out
}

/// Hooks covered by this module: `(hook address, levels using it, decoded)`; `decoded` false means the top level is modelled but
/// callees are `Unsupported`.
pub const COVERED: &[(u32, &[u32], bool)] = &[
    (0x1DDC90, &[0x1], false),
    (0x21AD00, &[0x2, 0x40], false),
    (0x1E40D0, &[0x4, 0x50], false),
    (0x220600, &[0x5], false),
    (0x224160, &[0x9], false),
    (0x205BC0, &[0xE], false),
    (0x1FA9C0, &[0x12], false),
    (0x221C70, &[0x1A], false),
];

// ================================================================ round 4 (T1845): shared callees of the story hooks

/// Frame step seconds of the hooks: `(1 / [0x4E8CEC]) * [0x4E794C]` (the float step multiplier, -1.0 in the image
/// but written every frame by the game loop; the models take it as an input).
pub fn step_seconds(multiplier: f32) -> f32 {
    (1.0 / 60.0) * multiplier
}

/// `0x1515E0(value, target, step)`: move `value` towards `target` by at most `step`; returns true when it arrived.
pub fn approach_value(value: &mut f32, target: f32, step: f32) -> bool {
    if (target - *value).abs() < step {
        *value = target;
        true
    } else {
        if target > *value {
            *value += step;
        } else {
            *value -= step;
        }
        false
    }
}

// ---------------------------------------------------------------- 0x218C50: proximity effect decay

/// Effects of [`proximity_effect_step`].
#[derive(Clone, Debug, PartialEq)]
pub enum ProximityEffect {
    /// `0xFA380(pad, strength)`: controller rumble.
    Rumble { pad: u32, strength: f32 },
    /// `0xD1F10(0.1, a, b)`: apply the two object floats (`+0x398`, `+0x3A0`, INFERRED from the name) to the players.
    ObjectFloats { rate: f32, first: f32, second: f32 },
    /// Player `index` words `+0xFBC` and `+0xFC0` (INFERRED: screen shake offsets) are set.
    PlayerJitter {
        player: usize,
        word_fbc: f32,
        word_fc0: f32,
    },
}

/// `0x218C50` (levels 2, 0x1A and 0x40 tail, MEASURED): with the timer `[0x75C0E0] = t > 0` rumble pad 0 with `t / 3 * 0.6`
/// (and pad 1 in mode 10 with several local players), `0xD1F10(0.1, t * 1.5, t * 0.7)`, then `t -= step`; every player whose
/// object field `+0x1CC` (health) is above 0 gets `+0xFBC = rand01 * 0.2 + 0.8` and `+0xFC0 = 0.94`. With `t <= 0` it only
/// stops the rumble of pad 0 (`0xFA380(0, 0)`). `players_alive` has one flag per local player (object present and
/// health above 0), `rand01` yields `rand_u32 * 2^-32` per use.
pub fn proximity_effect_step(
    timer: &mut f32,
    multiplier: f32,
    mode_byte: u8,
    players_alive: &[bool],
    rand01: &mut dyn FnMut() -> f32,
) -> Vec<ProximityEffect> {
    let mut out = Vec::new();
    if *timer <= 0.0 {
        out.push(ProximityEffect::Rumble {
            pad: 0,
            strength: 0.0,
        });
        return out;
    }
    let strength = *timer * (1.0 / 3.0) * 0.6;
    out.push(ProximityEffect::Rumble { pad: 0, strength });
    if mode_byte == 0xA && players_alive.len() > 1 {
        out.push(ProximityEffect::Rumble { pad: 1, strength });
    }
    out.push(ProximityEffect::ObjectFloats {
        rate: 0.1,
        first: *timer * 1.5,
        second: *timer * 0.7,
    });
    *timer -= step_seconds(multiplier);
    for (player, &alive) in players_alive.iter().enumerate() {
        if alive {
            out.push(ProximityEffect::PlayerJitter {
                player,
                word_fbc: rand01() * 0.2 + 0.8,
                word_fc0: 0.94,
            });
        }
    }
    out
}

// ---------------------------------------------------------------- 0x216230: timed effect table

/// Effect template (`0x507A18 + 0x44 * type`, MEASURED): up to four points, emission mode, interval and lifetimes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FxTemplate {
    pub point_count: u16,
    /// Byte `+2`: 0 emits every point at each tick, non-zero one point per tick, cycling.
    pub cycle_points: bool,
    /// Byte `+3`: randomise the length of each pulse (`rand01` instead of 1.0).
    pub randomise: bool,
    pub points: [[f32; 3]; 4],
    /// `+0x34`: seconds between emissions.
    pub interval: f32,
    /// `+0x38`: pulse length factor.
    pub lifetime: f32,
    /// `+0x3C`: idle wait factor.
    pub wait: f32,
    /// Bytes `+0x40..+0x43` read big endian (colour word handed to `0xFFE40`, INFERRED as RGBA).
    pub colour: u32,
}

const fn template(
    count: u16,
    cycle: bool,
    randomise: bool,
    pts: [[f32; 3]; 4],
    interval: f32,
    lifetime: f32,
    wait: f32,
    colour: u32,
) -> FxTemplate {
    FxTemplate {
        point_count: count,
        cycle_points: cycle,
        randomise,
        points: pts,
        interval,
        lifetime,
        wait,
        colour,
    }
}

const Z3: [f32; 3] = [0.0; 3];

/// Templates 0 to 6 of the table at `0x507A18` (those used by the entry tables are 2, 5 and 6).
pub const FX_TEMPLATES: [FxTemplate; 7] = [
    template(
        3,
        true,
        false,
        [[-2.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 0.0, 3.0], Z3],
        0.1,
        1.0,
        0.5,
        0x5F3F_7F3F,
    ),
    template(
        4,
        false,
        false,
        [
            [-2.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [-2.0, 0.0, 4.0],
            [2.0, 0.0, 4.0],
        ],
        0.4,
        2.0,
        1.0,
        0x5F3F_3F7F,
    ),
    template(
        2,
        true,
        false,
        [[0.0, 0.0, -2.0], [0.0, 0.0, 2.0], Z3, Z3],
        0.1,
        0.5,
        0.1,
        0x5F3F_3F7F,
    ),
    template(
        3,
        false,
        false,
        [[-2.0, 0.0, 0.0], [2.0, 0.0, 0.0], [0.0, 2.0, 0.0], Z3],
        0.2,
        1.0,
        0.5,
        0x5F7F_3F7F,
    ),
    template(
        2,
        true,
        true,
        [[0.0, 0.0, -2.0], [0.0, 0.0, 2.0], Z3, Z3],
        0.1,
        1.0,
        5.0,
        0x5F3F_3F7F,
    ),
    template(
        3,
        true,
        false,
        [[0.0, 0.0, -2.0], [0.0, 0.0, 2.0], Z3, Z3],
        0.075,
        7.5,
        0.0,
        0x7F3F_3F7F,
    ),
    template(
        3,
        true,
        false,
        [[0.0, 0.0, -2.0], [0.0, 0.0, 2.0], Z3, Z3],
        0.075,
        7.5,
        0.0,
        0x7F3F_3F7F,
    ),
];

/// One entry of a timed effect table (stride `0x28`: type `+0`, phase `+2`, position `+4`, timer `+0x10`, direction
/// `+0x14`, pulse scale `+0x20`, wait scale `+0x24`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FxEntry {
    pub kind: u16,
    /// 0 waiting, 1 emitting.
    pub phase: u16,
    pub position: [f32; 3],
    pub timer: f32,
    pub direction: [f32; 3],
    pub pulse_scale: f32,
    pub wait_scale: f32,
}

const fn fx_entry(kind: u16, position: [f32; 3], direction: [f32; 3], wait_scale: f32) -> FxEntry {
    FxEntry {
        kind,
        phase: 1,
        position,
        timer: 60.0,
        direction,
        pulse_scale: 1.0,
        wait_scale,
    }
}

/// The ten entries of the table at `0x507BF8` used by levels 2, 0x1A and 0x40 as stored in the image (all in phase 1 with
/// timer 60.0, so they start by finishing a pulse).
pub fn default_fx_table() -> [FxEntry; 10] {
    let up = [0.0, 1.0, 0.0];
    [
        fx_entry(2, [-123.15, 40.86, -244.08], up, 1.0),
        fx_entry(2, [-24.78, 30.08, -337.41], up, 1.0),
        fx_entry(2, [29.05, 6.93, -144.49], up, 1.0),
        fx_entry(2, [-9.86, 26.92, 167.49], up, 1.0),
        fx_entry(2, [89.23, 28.06, -11.61], up, 1.0),
        fx_entry(2, [-116.93, -6.09, 4.64], up, 0.0),
        fx_entry(5, [-80.2, -4.4, -37.1], up, 1.0),
        fx_entry(5, [-78.1, -4.4, -98.7], up, 1.0),
        fx_entry(6, [-72.5, 10.9, -15.3], up, 1.0),
        fx_entry(6, [-110.7, -4.0, -138.2], up, 1.0),
    ]
}

/// Inputs of one [`timed_fx_step`].
#[derive(Clone, Debug)]
pub struct FxInput {
    /// `[0x79094C]`.
    pub level_id: u32,
    pub mode_byte: u8,
    pub local_players: u32,
    /// Position `+0xA8..+0xB0` of player block 0.
    pub player0: [f32; 3],
    /// Camera object `[0x7B0CC0]` position `+0x27C`, `+0x284` (x and z).
    pub camera_xz: [f32; 2],
    /// The "SHTM" object (`0x4D544853`) position `+0x64..+0x6C`, when present.
    pub shtm: Option<[f32; 3]>,
    /// Float step multiplier `[0x4E794C]`.
    pub multiplier: f32,
}

/// Effects of [`timed_fx_step`].
#[derive(Clone, Debug, PartialEq)]
pub enum FxEffect {
    /// `0xFFDA0` + `0xFFE40`: a particle at `position` with the template colour.
    Particle {
        entry: usize,
        position: [f32; 3],
        colour: u32,
    },
    /// Level 2 only, one in four emissions of non 5/6 entries: `0x216190(position, 100.0)` damages the "SHTM" group object
    /// when a ray hits within the radius (INFERRED from the name).
    GroupDamage { position: [f32; 3], radius: f32 },
}

/// Distance squared cut: entries farther than 200 units (`40000`) from player 0 are skipped unless the game is a split
/// screen match (mode 10 with several local players).
pub const FX_CULL_DISTANCE_SQUARED: f32 = 40_000.0;

/// `0x2196B0`-style basis helper of `0x152B50`: the rotation used to place template points around `direction`. Row 1
/// is the direction, row 2 the helper axis (x or z, by `|dir.x| > 0.999`) made orthogonal and normalised, row 0 the cross
/// product `direction x row2`. MEASURED. Returns `[row0, row1, row2]`.
pub fn basis_from_direction(direction: [f32; 3]) -> [[f32; 3]; 3] {
    let [nx, ny, nz] = direction;
    let (hx, hz) = if nx.abs() > 0.999 {
        (0.0, 1.0)
    } else {
        (1.0, 0.0)
    };
    let d = hx * nx + hz * nz;
    let (mut x, mut y, mut z) = (hx - d * nx, 0.0 - d * ny, hz - d * nz);
    let len2 = x * x + y * y + z * z;
    if len2 > 1.0e-7 {
        let inverse = 1.0 / len2.sqrt();
        x *= inverse;
        y *= inverse;
        z *= inverse;
    }
    let row0 = [ny * z - y * nz, x * nz - nx * z, nx * y - ny * x];
    [row0, [nx, ny, nz], [x, y, z]]
}

fn rotate_by_basis(basis: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    let mut out = [0.0; 3];
    for axis in 0..3 {
        out[axis] = basis[0][axis] * v[0] + basis[1][axis] * v[1] + basis[2][axis] * v[2];
    }
    out
}

/// Sine table `0x7A38A0` (2048 entries, filled at run time; `sin(2 pi i / 2048)` is INFERRED, no writer found).
pub fn sine_entry(index: i32) -> f32 {
    ((index & 0x7FF) as f32 * (std::f32::consts::TAU / 2048.0)).sin()
}

fn angle_to_index(angle: f32) -> i32 {
    (angle * 325.949_31 + 0.5) as i32
}

/// `0x216230(table, count)` (MEASURED): a list of timed particle emitters. Per entry: types 6 and 5 run only in level 0x1A and
/// 2 respectively and need the "SHTM" object; entries run when it is a split screen match or player 0 is within
/// [`FX_CULL_DISTANCE_SQUARED`]. The timer advances by the frame step. Phase 0 (waiting): once the timer passes
/// `wait_scale * wait` it restarts, the pulse scale becomes `rand01` (template flag `randomise`) or 1.0 and phase 1 begins.
/// Phase 1 (emitting): each time the timer crosses a multiple of `interval` the template points are emitted (all of them, or
/// point `n mod count` for `cycle_points`) rotated by [`basis_from_direction`] around the entry position; past
/// `lifetime * pulse_scale` the entry picks a new direction (types 5 and 6: towards "SHTM", normalised, else a random
/// direction around the bearing to the camera), restarts the timer, picks the wait scale and returns to phase 0.
pub fn timed_fx_step(
    table: &mut [FxEntry],
    input: &FxInput,
    rand01: &mut dyn FnMut() -> f32,
    rand15: &mut dyn FnMut() -> u32,
) -> Vec<FxEffect> {
    let mut out = Vec::new();
    let split_screen = input.mode_byte == 0xA && input.local_players > 1;
    for (index, entry) in table.iter_mut().enumerate() {
        if (entry.kind == 6 && input.level_id != 0x1A) || (entry.kind == 5 && input.level_id != 2) {
            continue;
        }
        if !split_screen {
            let d = [
                entry.position[0] - input.player0[0],
                entry.position[1] - input.player0[1],
                entry.position[2] - input.player0[2],
            ];
            if d[0] * d[0] + d[1] * d[1] + d[2] * d[2] >= FX_CULL_DISTANCE_SQUARED {
                continue;
            }
        }
        let template = &FX_TEMPLATES[entry.kind as usize % FX_TEMPLATES.len()];
        let previous = entry.timer;
        entry.timer += step_seconds(input.multiplier);
        if (entry.kind == 5 || entry.kind == 6) && input.shtm.is_none() {
            continue;
        }
        match entry.phase {
            0 => {
                if entry.timer > entry.wait_scale * template.wait {
                    entry.timer = 0.0;
                    entry.pulse_scale = if template.randomise { rand01() } else { 1.0 };
                    entry.phase = 1;
                }
                continue;
            }
            1 => {}
            _ => continue,
        }
        let inverse = 1.0 / template.interval;
        let (now, before) = ((inverse * entry.timer) as i32, (inverse * previous) as i32);
        if now > before {
            let basis = basis_from_direction(entry.direction);
            let count = template.point_count as usize;
            let indices: Vec<usize> = if template.cycle_points {
                vec![((entry.timer / template.interval) as i32).rem_euclid(count as i32) as usize]
            } else {
                (0..count).collect()
            };
            for point in indices {
                let rotated = rotate_by_basis(&basis, template.points[point]);
                let position = [
                    entry.position[0] + rotated[0],
                    entry.position[1] + rotated[1],
                    entry.position[2] + rotated[2],
                ];
                out.push(FxEffect::Particle {
                    entry: index,
                    position,
                    colour: template.colour,
                });
                if input.level_id == 2 && rand15() & 3 == 0 && entry.kind != 5 && entry.kind != 6 {
                    out.push(FxEffect::GroupDamage {
                        position,
                        radius: 100.0,
                    });
                }
            }
        }
        if entry.timer > template.lifetime * entry.pulse_scale {
            if entry.kind == 5 || entry.kind == 6 {
                if let Some(target) = input.shtm {
                    let mut v = [
                        target[0] - entry.position[0],
                        target[1] - entry.position[1],
                        target[2] - entry.position[2],
                    ];
                    let len2 = v[0] * v[0] + v[1] * v[1] + v[2] * v[2];
                    if len2 > 1.0e-7 {
                        let inverse = 1.0 / len2.sqrt();
                        v = [v[0] * inverse, v[1] * inverse, v[2] * inverse];
                    }
                    entry.direction = v;
                }
            } else {
                let bearing = (input.camera_xz[0] - entry.position[0])
                    .atan2(input.camera_xz[1] - entry.position[2]);
                let yaw = bearing + (rand01() - 0.5) * std::f32::consts::PI;
                let pitch = rand01() * std::f32::consts::FRAC_PI_4;
                let (yaw_index, pitch_index) = (angle_to_index(yaw), angle_to_index(pitch));
                let cos_pitch = sine_entry(pitch_index);
                entry.direction = [
                    sine_entry(yaw_index) * cos_pitch,
                    sine_entry(pitch_index + 0x200),
                    sine_entry(yaw_index + 0x200) * cos_pitch,
                ];
            }
            entry.timer = 0.0;
            entry.wait_scale = if template.randomise { rand01() } else { 1.0 };
            entry.phase = 0;
        }
    }
    out
}

// ---------------------------------------------------------------- 0x219CA0 / 0x2196B0: sky streak

/// One row of the sky quad table at `0x5078C8` (stride `0x1C`: flag, point A, point B). A quad of row `i` is
/// `(A_i, B_i, A_{i+1}, B_{i+1})`, so the slice needs one row more than the 11 candidates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SkyRow {
    pub flag: u32,
    pub a: [f32; 3],
    pub b: [f32; 3],
}

/// State block `0x78D0E0` of the streak: state `+0`, position `+8`, velocity `+0x14`, timer `+0x20`, particle handle `+0x28`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SkyStreak {
    pub state: u32,
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub timer: f32,
    pub handle: i32,
}

/// Effects of the streak code.
#[derive(Clone, Debug, PartialEq)]
pub enum SkyEffect {
    /// `0xFF200(handle)`: the previous particle is stopped.
    StopParticle { handle: i32 },
    /// `0x101660(&handle, 0x7E, 0, position, 0xFFFF)` then `0xFF420(handle, 40.0)`: start particle kind `0x7E`.
    StartParticle {
        kind: u32,
        position: [f32; 3],
        length: f32,
    },
}

fn sub3(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Camera exclusion radius squared (`1600` = 40 units) and the minimum distance squared (`225` = 15 units) of a streak
/// start point in the xz plane; objects closer than 15 units block the spawn.
pub const SKY_FAR: f32 = 1600.0;
pub const SKY_NEAR: f32 = 225.0;

/// `0x2196B0` (MEASURED): pick row `rand % 11` (skipped when its flag is 0), a triangle of its quad by a coin (`rand & 1`:
/// `(A, B, A')` or `(B, A', B')`), a random point `O + a*T + b*S` in it (`b = (1-a)*r` or `-(a*r)`), reject it when the
/// camera is farther than 40 units or an object (`objects`, the non player entries of the object array) is closer than
/// 15 units in xz, then a random direction (azimuth `2 pi r`, polar `pi r`, via the sine table) that must lie within
/// `-1 < dot(direction, normal) < -0.9` of the upward facing triangle normal. The streak starts 4.9 above the point minus
/// 30 units back along the direction, moving 30 units per second the other way; state becomes 2, timer 0.
pub fn sky_streak_spawn(
    streak: &mut SkyStreak,
    table: &[SkyRow],
    camera_xz: [f32; 2],
    objects_xz: &[[f32; 2]],
    rand_u32: &mut dyn FnMut() -> u32,
) -> Vec<SkyEffect> {
    let unit = |value: u32| value as f32 * 2.328_306_4e-10;
    let index = (rand_u32() % 11) as usize;
    let coin = rand_u32() & 1 == 1;
    let (Some(row), Some(next)) = (table.get(index), table.get(index + 1)) else {
        return Vec::new();
    };
    if row.flag == 0 {
        return Vec::new();
    }
    let (origin, first, second) = if coin {
        (row.a, sub3(next.a, row.a), sub3(row.b, row.a))
    } else {
        (row.b, sub3(next.b, row.b), sub3(next.b, next.a))
    };
    // Literal normal formulas of the two branches.
    let mut normal = if coin {
        let (u, w) = (second, first);
        [
            u[2] * w[1] - u[1] * w[2],
            u[0] * w[2] - u[2] * w[0],
            u[1] * w[0] - u[0] * w[1],
        ]
    } else {
        let (s, t) = (second, first);
        [
            s[2] * t[1] - s[1] * t[2],
            s[0] * t[2] - s[2] * t[0],
            s[1] * t[0] - s[0] * t[1],
        ]
    };
    let len2 = normal[0] * normal[0] + normal[1] * normal[1] + normal[2] * normal[2];
    if len2 > 1.0e-7 {
        let inverse = 1.0 / len2.sqrt();
        normal = [
            normal[0] * inverse,
            normal[1] * inverse,
            normal[2] * inverse,
        ];
    }
    if normal[1] < 0.0 {
        normal = [-normal[0], -normal[1], -normal[2]];
    }
    let a = unit(rand_u32());
    let r = unit(rand_u32());
    let b = if coin { (1.0 - a) * r } else { -(r * a) };
    let point = [
        origin[0] + a * first[0] + b * second[0],
        origin[1] + a * first[1] + b * second[1],
        origin[2] + a * first[2] + b * second[2],
    ];
    let (dx, dz) = (point[0] - camera_xz[0], point[2] - camera_xz[1]);
    let camera_distance = dx * dx + dz * dz;
    if camera_distance > SKY_FAR || camera_distance < SKY_NEAR {
        return Vec::new();
    }
    for object in objects_xz {
        let (ox, oz) = (point[0] - object[0], point[2] - object[1]);
        if ox * ox + oz * oz < SKY_NEAR {
            return Vec::new();
        }
    }
    let azimuth = unit(rand_u32()) * std::f32::consts::TAU;
    let polar = unit(rand_u32()) * std::f32::consts::PI;
    let (polar_index, azimuth_index) = (angle_to_index(polar), angle_to_index(azimuth));
    let sin_polar = sine_entry(polar_index);
    let direction = [
        sine_entry(azimuth_index) * sin_polar,
        sine_entry(polar_index + 0x200),
        sine_entry(azimuth_index + 0x200) * sin_polar,
    ];
    let dot = direction[0] * normal[0] + direction[1] * normal[1] + direction[2] * normal[2];
    if !(dot < -0.9 && dot > -1.0) {
        return Vec::new();
    }
    let velocity = [
        direction[0] * 30.0,
        direction[1] * 30.0,
        direction[2] * 30.0,
    ];
    let mut out = Vec::new();
    streak.velocity = velocity;
    streak.position = [
        point[0] - velocity[0],
        point[1] - velocity[1] + 4.9,
        point[2] - velocity[2],
    ];
    streak.timer = 0.0;
    streak.state = 2;
    if streak.handle != 0 {
        out.push(SkyEffect::StopParticle {
            handle: streak.handle,
        });
    }
    out.push(SkyEffect::StartParticle {
        kind: 0x7E,
        position: streak.position,
        length: 40.0,
    });
    out
}

/// `0x219CA0` (MEASURED): the timer `+0x20` always gains the frame step; state 0 runs [`sky_streak_spawn`], state 1 moves the
/// position by `velocity * step` and returns to 0 once the timer passes 3.0, state 3 returns to 0, state 2 (set by the
/// spawn) and anything above 3 do nothing. Who sets state 1 or 3 is not known (no writer found in the hook range), so
/// the streak, once started, is driven by its particle only. UNRESOLVED.
pub fn sky_streak_tick(
    streak: &mut SkyStreak,
    multiplier: f32,
    spawn: &mut dyn FnMut(&mut SkyStreak) -> Vec<SkyEffect>,
) -> Vec<SkyEffect> {
    let step = step_seconds(multiplier);
    streak.timer += step;
    match streak.state {
        0 => spawn(streak),
        1 => {
            for axis in 0..3 {
                streak.position[axis] += step * streak.velocity[axis];
            }
            if streak.timer > 3.0 {
                streak.state = 0;
            }
            Vec::new()
        }
        3 => {
            streak.state = 0;
            Vec::new()
        }
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------- 0x21C580: repel close objects

/// Radius squared under which two objects push each other apart (`49`, the other limit `900` is looser) and the separation
/// they are pushed to (7.0 units).
pub const REPEL_RADIUS_SQUARED: f32 = 49.0;
pub const REPEL_SEPARATION: f32 = 7.0;

/// `0x21C580` (level 5 idle, MEASURED): gather the objects of four tag groups (`0x509028..0x509038`, up to 20 each) whose
/// word `+0x94` is `0x557`, then for every pair `(i < j)` closer than 7 units (squared `< 49`) move both along the line
/// between them by half of the missing distance each, until they are 7 units apart. Positions are `+0x64..+0x6C`.
pub fn repel_close_objects(positions: &mut [[f32; 3]]) {
    for i in 0..positions.len() {
        for j in i + 1..positions.len() {
            let d = sub3(positions[i], positions[j]);
            let len2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
            if len2 >= 900.0 || len2 >= REPEL_RADIUS_SQUARED {
                continue;
            }
            let scale = REPEL_SEPARATION / len2.sqrt();
            let delta = [
                (scale * d[0] - d[0]) * 0.5,
                (scale * d[1] - d[1]) * 0.5,
                (scale * d[2] - d[2]) * 0.5,
            ];
            for axis in 0..3 {
                positions[i][axis] += delta[axis];
                positions[j][axis] -= delta[axis];
            }
        }
    }
}

// ---------------------------------------------------------------- 0x1FD350 / 0x1FD1B0: level 0xE spark bursts

/// Spark spawn table at `0x503A24` (6 rows of x and y) and the mirror constant `217.6`.
pub const SPARK_TABLE: [[f32; 2]; 6] = [
    [115.6, 30.2],
    [122.2, 35.7],
    [130.9, 40.0],
    [138.3, 46.1],
    [143.9, 53.5],
    [0.0, 0.0],
];
pub const SPARK_MIRROR: f32 = 217.6;

/// Record `[0x75C03C]`: flag `+0x90`, timer `+0x94`, z `+0xA0`, count `+0xA4`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SparkRecord {
    pub flag: u32,
    pub timer: f32,
    pub z: f32,
    pub count: u32,
}

/// Effects of the spark bursts.
#[derive(Clone, Debug, PartialEq)]
pub enum SparkEffect {
    /// `0x111AE0` particle kind `0x7B0A80` at `position` with `velocity (+-5, 0.89, 0)`, flags `0x10E270(8, 0)` and sizes `0x10EAE0(15, 15)`.
    Spark {
        position: [f32; 3],
        velocity: [f32; 3],
    },
    /// `0xA5CE0("FOG1" object, state 0xA, 0x1FD0E0)`: the quad draw callback is registered (draw body not decoded).
    RegisterDraw,
    /// Count reached 25: `0xC6100("VMLC", 4.0)` then `0xCEED0`, slot 9 finished, flag `+0x90` cleared.
    BurstsDone,
}

/// `0x1FD350` (`counted == true`, MEASURED) and `0x1FD1B0` (`counted == false`): the timer falls by the frame step; at or
/// below 0 a spark is emitted at row `rand % 6` of [`SPARK_TABLE`] (mirrored in x about 217.6 with velocity x flipped on a coin
/// flip) and the timer restarts at `(rand01 + 0.5) * 0.1`. `0x1FD350` places it at depth `z` (then `z += 10`, count + 1,
/// registers the draw callback and reports 25 bursts), `0x1FD1B0` at a random depth `rand01 * 134 + 56`. Random draws in
/// order: coin, row, timer (`0x1FD1B0`: timer, depth, timer again).
pub fn spark_burst_step(
    record: &mut SparkRecord,
    counted: bool,
    multiplier: f32,
    rand_u32: &mut dyn FnMut() -> u32,
) -> Vec<SparkEffect> {
    let unit = |value: u32| value as f32 * 2.328_306_4e-10;
    let mut out = Vec::new();
    record.timer -= step_seconds(multiplier);
    if record.timer <= 0.0 {
        let coin = rand_u32() & 1 == 1;
        let row = (rand_u32() % 6) as usize;
        let mut velocity = [-5.0, 0.89, 0.0];
        let x = if coin {
            SPARK_TABLE[row][0]
        } else {
            velocity[0] = 5.0;
            SPARK_MIRROR - SPARK_TABLE[row][0]
        };
        if counted {
            record.timer = (unit(rand_u32()) + 0.5) * 0.1;
            out.push(SparkEffect::Spark {
                position: [x, SPARK_TABLE[row][1], record.z],
                velocity,
            });
            out.push(SparkEffect::RegisterDraw);
            record.z += 10.0;
            record.count += 1;
        } else {
            let _first_timer = unit(rand_u32());
            let depth = unit(rand_u32()) * 134.0 + 56.0;
            record.timer = (unit(rand_u32()) + 0.5) * 0.1;
            out.push(SparkEffect::Spark {
                position: [x, SPARK_TABLE[row][1], depth],
                velocity,
            });
        }
    }
    if counted && record.count == 25 {
        out.push(SparkEffect::BurstsDone);
        record.flag = 0;
    }
    out
}

// ---------------------------------------------------------------- 0x223FB0: level 9 music targets

/// How `0x223FB0` treats a player weapon class (`0x9D4A0(0x9A420(weapon))`, byte map `0x224124`): 0 adds 1.0, 1 adds the
/// health term, 2 and every class above `0x38` add nothing. Classes 0 to 7, 0x23, 0x2C and 0x34 to 0x38 are case 0, class 8
/// is case 1, the rest case 2.
pub fn level9_class_case(class: u32) -> u32 {
    match class {
        0..=7 | 0x23 | 0x2C | 0x34..=0x38 => 0,
        8 => 1,
        _ => 2,
    }
}

/// `0x223FB0` (MEASURED): with the ambience gate open (`0x23EF00() == -1`) the mean over active players of their weight (case
/// 0: 1.0, case 1: `(113 - clamp(health, 100, 113)) / 13`) sets the volume of loop `0x35` (handle `[0x75C120]`), and
/// `1 - mean` that of loop 8 (handle `[0x75C124]` is `[0x75C120] + 4`). With the gate closed both are 0. Returns
/// `(volume of 0x35, volume of 8)`; no active player gives `(0, 1)`.
pub fn level9_music_targets(players: &[(u32, f32)], gate_open: bool) -> (f32, f32) {
    if !gate_open {
        return (0.0, 0.0);
    }
    let mut sum = 0.0f32;
    for &(class, health) in players {
        match level9_class_case(class) {
            0 => sum += 1.0,
            1 => sum -= (health.clamp(100.0, 113.0) - 113.0) * 0.076_923_08,
            _ => {}
        }
    }
    if players.is_empty() {
        return (0.0, 1.0);
    }
    let mean = sum / players.len() as f32;
    (mean, 1.0 - mean)
}

// ---------------------------------------------------------------- 0x1FD060: level 0xE ambient loops

/// The class lists of the four loops of `0x1FD060` (`0x503990`, `0x5039B8`, `0x5039D0`, `0x503A14`); `-0x64` ends a list and
/// `-0x65` is the 0.5 weight sentinel.
pub const LEVEL0E_LIST_A: [i32; 10] = [1, 8, 7, -0x65, 3, 4, 6, 7, 9, -0x64];
pub const LEVEL0E_LIST_B: [i32; 5] = [30, -0x65, 31, 29, -0x64];
pub const LEVEL0E_LIST_C: [i32; 12] = [49, 47, 48, 37, 36, 38, 35, 44, 46, 45, 43, -0x65];
pub const LEVEL0E_LIST_D: [i32; 4] = [56, -0x65, 55, -0x64];

/// `0x1FD060` (MEASURED): four `0x46F40` class list loops on the record `[0x75C03C]` (state words `+0, +8, +0x10, +0x18`):
/// sound `0x4F` with list A, `0x4C` with B, `0x4D` with C (the list is null once slot 3 is finished, so that loop fades
/// out) and `0x4E` with D, all with gain 1.0. Each value approaches the mean weight of the active players with step 0.025.
/// `values` are the four states, `classes` the player classes (`+0x2A0`). Returns `(sound, new value)` per loop.
pub fn level0e_ambient_loops(
    values: &mut [f32; 4],
    classes: &[i32],
    slot3_finished: bool,
) -> [(u32, f32); 4] {
    use super::hooks_audio::class_list_weight;
    let lists: [(u32, Option<&[i32]>); 4] = [
        (0x4F, Some(&LEVEL0E_LIST_A)),
        (0x4C, Some(&LEVEL0E_LIST_B)),
        (
            0x4D,
            if slot3_finished {
                None
            } else {
                Some(&LEVEL0E_LIST_C)
            },
        ),
        (0x4E, Some(&LEVEL0E_LIST_D)),
    ];
    let mut out = [(0, 0.0); 4];
    for (slot, (sound, list)) in lists.iter().enumerate() {
        let target = match list {
            Some(list) if !classes.is_empty() => {
                classes
                    .iter()
                    .map(|&class| class_list_weight(list, class, 1.0))
                    .sum::<f32>()
                    / classes.len() as f32
            }
            Some(_) => f32::NAN,
            None => 0.0,
        };
        approach_value(&mut values[slot], target, 0.025);
        out[slot] = (*sound, values[slot]);
    }
    out
}

// ---------------------------------------------------------------- 0x1E12C0: level 4 / 0x50 ember spawner

/// Character classes (`+0x2A0` of the camera object `[0x7B0CC0]`) for which `0x1E12C0` runs.
pub const EMBER_CLASSES: [u32; 7] = [0x48, 0x49, 0x4A, 0x4B, 0x4D, 0x51, 0x52];

/// `0x1E12C0` (MEASURED): for a camera class in [`EMBER_CLASSES`] the timer `[0x75BC08]` falls by `(1/60) * steps` (integer
/// step `[0x4E7944]`); once below 0 one particle (`0x111AE0`, kind `0x7B0760`, direction `(0, 1, 0)`, size `0x10EAE0(2, 2)`)
/// is created at `(r1 * 150 - 391.39, -49.6, r3 * 150 - 160.8)` with `r = (rand01 - 0.5) * 2` (a second draw is discarded)
/// and the timer restarts at `rand01 * 0.5 + 0.2`. Returns the spawn position, if any.
pub fn ember_spawn(
    timer: &mut f32,
    camera_class: Option<u32>,
    steps: i32,
    rand01: &mut dyn FnMut() -> f32,
) -> Option<[f32; 3]> {
    let class = camera_class?;
    if !EMBER_CLASSES.contains(&class) {
        return None;
    }
    *timer -= (1.0 / 60.0) * steps as f32;
    if *timer >= 0.0 {
        return None;
    }
    let r1 = (rand01() - 0.5) * 2.0;
    let _discarded = (rand01() - 0.5) * 2.0;
    let r3 = (rand01() - 0.5) * 2.0;
    let position = [r1 * 150.0 - 391.39, -49.6, r3 * 150.0 - 160.8];
    *timer = rand01() * 0.5 + 0.2;
    Some(position)
}

// ---------------------------------------------------------------- 0x21C7B0: boss aim vectors

/// The camera aim block written by `0x21C7B0` (`0x78CF40`: mode 3, target `+8`, forward `+0x38`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BossAim {
    pub mode: u32,
    pub target: [f32; 3],
    pub forward: [f32; 3],
}

/// `0x21C7B0` (level 5 hook, MEASURED): pitch `[0x75C100]` (degrees, floored at -85) and yaw `[0x75C0FC]` (degrees, negated) become
/// table indices (`deg * 0.017453 * 325.95 + 0.5`); forward = `(cos yaw cos pitch, sin pitch, sin yaw cos pitch)`; the target is
/// `origin + (0, 1, 0) + 4.5 * forward` where origin is the player controller position `+0xBB4..+0xBBC`. Camera role: INFERRED.
pub fn boss_aim_vectors(pitch_deg: f32, yaw_deg: f32, origin: [f32; 3]) -> BossAim {
    let pitch = pitch_deg.max(-85.0) * 0.017_453_293;
    let yaw = -(yaw_deg * 0.017_453_293);
    let (pitch_index, yaw_index) = (angle_to_index(pitch), angle_to_index(yaw));
    let cos_pitch = sine_entry(pitch_index + 0x200);
    let forward = [
        sine_entry(yaw_index + 0x200) * cos_pitch,
        sine_entry(pitch_index),
        sine_entry(yaw_index) * cos_pitch,
    ];
    let target = [
        forward[0] * 4.5 + origin[0],
        forward[1] * 4.5 + origin[1] + 1.0,
        forward[2] * 4.5 + origin[2],
    ];
    BossAim {
        mode: 3,
        target,
        forward,
    }
}
