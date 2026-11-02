//! Game-world side of the event system. The engine only talks to the world through this trait,
//! so a headless test world can drive it (spec section 9, items 5 and 6). Every method added in
//! T1841 has a default, so worlds written for T1840 keep compiling.

use super::actions::{Action, ActionResult, PreWait, Wait};
use super::callbacks::WorldOp;
use super::record::Link;
use super::slots::{MissionEvent, SlotEvent};

/// Which id-set test a player condition uses (spec 4.4): character id, id following the link,
/// resource-membership id. The three tests themselves live in the game world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdVariant {
    CharacterId,
    FollowingLink,
    ResourceMembership,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceRequest {
    pub label: u32,
    pub tag: u32,
    pub speaker_tag: u32,
    pub volume: f32,
    pub priority: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropRequest {
    pub key: u32,
    pub activate: bool,
    pub hold: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpawnRequest {
    pub group_key: u32,
    pub object_type: u32,
    pub tag: u32,
    pub variant: u32,
    pub scale: f32,
    pub position: [f32; 3],
    pub orientation: [f32; 3],
    pub callback: u32,
    pub tick_callback: u32,
    pub flags: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackTarget {
    /// Verbs 0, 29, 30: the event instance index.
    Instance(usize),
    /// Verbs 3, 4, 5 after creation: the created object handle.
    Object(u32),
}

/// World-state conditions (spec 4.4) that no engine state can answer. The doc comments give the
/// decoded predicate so a world can implement it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionQuery {
    /// Kinds 1, 3, 4, 5, 6, 7, 8, 9, 29, 30, 31: scene predicates, hard-coded per level, see
    /// docs/t-story-event-engine.md section 5. `args` are the four condition arguments.
    Scene { kind: u32, args: [u32; 4] },
    /// Kind 33: table `0x7B2780` (stride 0x54) entry with this key, field `+0x18` non-zero.
    TableField { key: u32 },
    /// Kind 36: tagged object's child state (`0x188920`) is 2, 3, 5 or 7.
    ChildState { key: u32 },
    /// Kind 37: fewer than two of the non-player characters pass `0xD6EC0` (`0x234650`).
    FewerThanTwoOfClass,
}

pub trait World {
    /// Start a voice line, returning a sound handle, or `None` when it cannot play.
    fn play_voice(&mut self, request: &VoiceRequest) -> Option<u32>;
    fn voice_finished(&mut self, handle: u32) -> bool;
    /// Legacy hook: called with the raw callback id when the id is not decodable
    /// ([`callbacks::decode`](super::callbacks::decode) gap) or when [`World::apply_callback`]
    /// does not model one of the decoded steps.
    fn run_callback(&mut self, callback: u32, target: CallbackTarget);
    /// Apply one decoded world step of a callback (see [`WorldOp`]). Return `false` when the
    /// world does not model it, the engine then falls back to [`World::run_callback`].
    fn apply_callback(&mut self, _op: &WorldOp, _target: CallbackTarget) -> bool {
        false
    }
    /// Apply a decoded static slot callback step (ids 5 and 6 of table `0x4D2CA0`). Default: not modelled.
    fn apply_slot_op(&mut self, _slot: u32, _op: &WorldOp) -> bool {
        false
    }
    /// Is state saved in `0x78D340` (slot callback 5)? The engine fires event "ETRC" before restoring it.
    fn shtm_saved(&self) -> bool {
        false
    }
    /// Words `+0x40` and `+0x44` of the callback argument: the instance (verbs 0, 29, 30) or the
    /// object (verbs 3, 4, 5). The pool builder never writes them for instances, default zero.
    fn callback_words(&self, _target: CallbackTarget) -> [u32; 2] {
        [0; 2]
    }
    fn set_objective(&mut self, actor_key: u32, index: u32);
    fn control_prop(&mut self, request: &PropRequest);
    fn prop_settled(&mut self, key: u32) -> bool;
    fn spawn_object(&mut self, request: &SpawnRequest) -> Option<u32>;
    /// Verb 3 tail: the spawned object finished (`0x59DD0`: extension `+0x124` when `+0x12C` is 0).
    fn object_ready(&mut self, handle: u32) -> bool;
    fn destroy_object(&mut self, handle: u32);
    /// Does the generation-checked handle still resolve (`0x12F00`)? A missing object completes the verb.
    fn object_exists(&self, _handle: u32) -> bool {
        true
    }

    fn player_count(&self) -> usize;
    /// Does player `player` pass the id test `variant` against `ids` (-1 entries already removed)?
    fn player_matches(&self, player: usize, variant: IdVariant, ids: &[u32]) -> bool;
    /// Kinds 23 to 27 use `0x277FE0`, which is null outside Story mode or with several players.
    /// When false: kinds 23, 24, 26, 27 are false and kind 25 is true (MEASURED).
    fn first_player_available(&self) -> bool {
        self.player_count() > 0
    }
    /// Kind 27: the first player's extension word at +0xC08 is non-zero.
    fn first_player_flag(&self) -> bool;
    fn tag_exists(&self, key: u32) -> bool;
    /// World-state condition kinds (see [`ConditionQuery`]). `None` = not modelled: Unsupported.
    fn condition_query(&mut self, _query: &ConditionQuery) -> Option<bool> {
        None
    }
    /// Polled link modes 1, 6, 8, 10, 11, 15, 16, 17, 18 (called every frame by the slot update
    /// for event slots, like callback code 3 of `0x235240`). Modes 4 and 9 are answered by the
    /// engine (instance executed, slot status 5). Modes 5, 12, 13, 19, 20 are never polled.
    fn poll_link(&mut self, link: &Link) -> bool;
    /// Pre-wait of verbs 3, 7, 24, 43, 46, 48, 49, 51, 74 (spec 4.1 step 5). Legacy hook by verb.
    fn prewait_ready(&mut self, _verb: u32) -> bool {
        true
    }
    /// Decoded pre-wait (see [`PreWait`]), answered `true` when the verb may proceed.
    /// The default defers to the legacy [`World::prewait_ready`].
    fn prewait_check(&mut self, verb: u32, _check: &PreWait) -> bool {
        self.prewait_ready(verb)
    }
    /// Executor for verbs outside the engine set. Legacy hook, called after
    /// [`World::apply_action`] answered `Unsupported`. Return `false` for Unsupported.
    fn other_verb(&mut self, _verb: u32, _instance: usize) -> bool {
        false
    }
    /// Run the decoded effect of a verb. The default models nothing.
    fn apply_action(&mut self, _action: &Action) -> ActionResult {
        ActionResult::Unsupported
    }
    /// Tail-pass wait of a verb. The default answers done, except prop state which asks
    /// [`World::prop_settled`].
    fn wait_done(&mut self, wait: &Wait) -> bool {
        match wait {
            Wait::PropState { key, .. } => self.prop_settled(*key),
            _ => true,
        }
    }

    /// Slot HUD notification (`0x227430` and the finish path).
    fn slot_event(&mut self, _event: &SlotEvent) {}
    /// A static slot callback fires with code 4 (finish). `callback` is the slot table callback id.
    fn slot_callback(&mut self, _slot: u32, _callback: u32) {}
    /// Per frame poll of a slot callback (code 3 from `0x2282B0`, which runs for every slot,
    /// static or event). Event link slots are answered by the engine, this is for the static
    /// slot callback ids of table `0x4D2CA0`.
    fn slot_poll(&mut self, _slot: u32, _callback: u32) {}
    /// Known health of a registered actor (`+0x1CC`). Static slot progress counts actors with
    /// health at or below zero as dead. Missing health is unsupported and must not be guessed.
    fn actor_health(&self, _actor: u32) -> Option<f32> {
        None
    }
    /// Known health of local player `player`'s character (`+0x1CC`). Missing health is unknown.
    fn player_health(&self, _player: usize) -> Option<f32> {
        None
    }
    /// Mission evaluation result of the per frame slot update (`0x228870`).
    fn mission_event(&mut self, _event: MissionEvent) {}
    /// All local players are in state 0x10 (used by the mission evaluation).
    fn players_all_down(&self) -> bool {
        false
    }
    /// Verb 88 and mission end: `0x67450(0)` posts player messages and enters state 8.
    fn end_match(&mut self) {}
    /// A verb 27 instance fired: the checkpoint capture entry `0x25F0C0` runs.
    fn checkpoint_fired(&mut self) {}
    /// Verb 84 with a key: clear the actor's target (`0x1642C0`). Sounds are stopped by the engine through `stop_sound`.
    fn clear_target(&mut self, _actor_key: u32) {}
    fn stop_sound(&mut self, _handle: u32) {}
    /// Verb 10 repeat: uniform random number in [0, 1) (`0x153B40` times 2^-32).
    fn random_unit(&mut self) -> f32 {
        0.5
    }
    /// Verb 10 repeat: byte `[0x7B977C + room]` is non-zero.
    fn room_active(&self, _room: u32) -> bool {
        true
    }
    /// Snapshot restore (spec 5.3): save and restore the word of restore function `id` (`0x513DC4/8`).
    fn restore_save(&mut self, _id: u32) -> u32 {
        0
    }
    fn restore_apply(&mut self, _id: u32, _word: u32) {}
    /// Snapshot capture of a verb 3 object: extension words `+0xDC` and `+0x108`, if it has any.
    fn object_words(&self, _handle: u32) -> Option<(u32, u32)> {
        None
    }
    fn set_object_words(&mut self, _handle: u32, _first: u32, _second: u32) {}
    /// Snapshot restore of a persistent verb 3 object: may it be recreated now (`0x58270` asset
    /// loaded, mode checks of `0x23765B`)? When false the instance is queued for `apply_pending_restore`.
    fn persistent_spawn_ready(&mut self, _object_type: u32) -> bool {
        true
    }
    /// Class flags of an object type (class table `0x791D4C`, stride 0x30, `+0x28`), used by the persistent
    /// restore gate (`levels::CLASS_FLAG_NO_RESTORE`). Default 0.
    fn object_class_flags(&self, _object_type: u32) -> u32 {
        0
    }
    /// The per-level hook `[level+0x110]` runs (`0x228870`); `address` is the static hook function of
    /// `levels::LEVEL_HOOKS`. The hook bodies are per-level scripts and not decoded, so the default ignores it.
    ///
    /// T1846: the default runs the ported model through [`World::hook_host`] and hands every result to
    /// [`World::hook_output`]; a world without a host ignores the call like before.
    fn run_level_hook(&mut self, address: u32) {
        let outputs = match self.hook_host() {
            Some(host) => super::hook_wiring::dispatch_level_hook(address, host),
            None => return,
        };
        for output in &outputs {
            self.hook_output(output);
        }
    }
    /// The world's view for the hook ports (T1846). `None` = hooks are not modelled by this world.
    fn hook_host(&mut self) -> Option<&mut dyn super::hook_wiring::HookHost> {
        None
    }
    /// One result of a hook run (sound loop, slot change, strike, unwired marker).
    fn hook_output(&mut self, _output: &super::hook_wiring::HookOutput) {}
    /// Mode 5 of `0x228870` (`0x2284D0`): the proximity beeper. The default runs the model through the host.
    fn run_mode5_beeper(&mut self, step: f32) {
        let output = match self.hook_host() {
            Some(host) => super::hook_wiring::run_mode5_beeper(host, step),
            None => return,
        };
        if let Some(output) = output {
            self.hook_output(&output);
        }
    }
    /// Level stage `0x25EEB0(0)` compared with link mode 19 on restore.
    fn mission_stage(&self) -> u32 {
        0
    }
}
