//! Executors and tail-pass completion of the event verbs (spec sections 5.2, 6). The engine runs
//! verbs 0, 1, 3, 16, 27, 29 to 37, 56, 57, 72, 84 and 88 itself (they touch instance, slot or
//! session state); every other verb is decoded by [`actions::decode`] and applied by the world.

use super::actions::{self, Action, ActionResult, Completion, NO_TAG, Wait};
use super::engine::{Engine, EngineError, StepReport};
use super::slots::SESSION_COMPLETE;
use super::world::{CallbackTarget, PropRequest, SpawnRequest, VoiceRequest, World};

const FLAG_KEEP_OBJECT: u32 = 0x10;
/// Voice tail thresholds (floats at `0x4760DC` and `0x4760C4`, MEASURED).
const VOICE_NO_SOUND_TIMER: f32 = -5.0;
const VOICE_MIN_TIMER: f32 = -0.5;
/// Verb 10 re-trigger: the delay is reloaded with 10.0 (`0x47862C`) on every tail pass.
const EFFECT_RELOAD_DELAY: f32 = 10.0;

/// Executor outcome. `Aborted` undoes the running/executed marks (failed lookup of verbs 43, 46).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
    Ran,
    Aborted,
}

/// Verbs the engine knows (every verb 0 to 92 has a row in the spec; verbs that need a world
/// effect still need [`World::apply_action`] to do anything).
pub fn is_implemented(verb: u32) -> bool {
    verb <= 92
}

/// Verbs with a pre-wait on a mission-state predicate (spec 4.1 step 5; T1841 added verb 7).
pub fn has_prewait(verb: u32) -> bool {
    matches!(verb, 3 | 7 | 24 | 43 | 46 | 48 | 49 | 51 | 74)
}

pub(crate) fn run(
    engine: &mut Engine,
    index: usize,
    world: &mut dyn World,
) -> Result<Outcome, EngineError> {
    let record = engine.records[index];
    match record.verb {
        0 | 29 | 30 => {
            if record.callback != 0 {
                engine.dispatch_callback(
                    record.callback,
                    CallbackTarget::Instance(index),
                    index,
                    world,
                );
            }
        }
        1 => voice(engine, index, world),
        72 => world.set_objective(record.arg(0x14), record.arg(0x10)),
        16 => world.control_prop(&PropRequest {
            key: record.arg(0x14),
            activate: record.arg(0xC) != 0,
            hold: record.arg(0x10) != 0,
        }),
        3 => spawn(engine, index, world),
        // Executor does nothing (target 0x237302): 26 and 49/50 act in the tail, 27 at fire time.
        26 | 27 | 49 | 50 | 52 | 91 => {}
        31 => engine.finish_slot(record.arg(0xC), world),
        32 => engine.slots.complete_quiet(record.arg(0xC)),
        33 => slot_change(
            engine,
            world,
            |slots, id| slots.reactivate(id),
            record.arg(0xC),
        ),
        34 => slot_change(
            engine,
            world,
            |slots, id| slots.start_pending(id),
            record.arg(0xC),
        ),
        35 => engine.slots.deactivate(record.arg(0xC)),
        36 => slot_change(
            engine,
            world,
            |slots, id| slots.activate(id),
            record.arg(0xC),
        ),
        37 => slot_change(
            engine,
            world,
            |slots, id| slots.mark_failed(id),
            record.arg(0xC),
        ),
        56 => {
            for offset in [0xC, 0x10, 0x14, 0x18, 0x1C] {
                let id = record.arg(offset);
                if id != 0
                    && let Some(target) = engine.find_by_id(id)
                {
                    engine.reset_instance(target, Some(world));
                }
            }
        }
        57 => {
            // `0x2344D0` with the reset flag: instances keyed on mode 5 links are reset, then the
            // keyed object is toggled by the world (`0xB80A0`, result ignored).
            engine.signal_key(record.arg(0xC), true, &[5]);
            if let Some(action) = actions::decode(&record) {
                let _ = world.apply_action(&action);
            }
        }
        84 => release_handles(engine, &record, world),
        88 => {
            engine.session_flags |= SESSION_COMPLETE;
            world.end_match();
        }
        other => return apply(engine, index, other, world),
    }
    Ok(Outcome::Ran)
}

fn slot_change(
    engine: &mut Engine,
    world: &mut dyn World,
    change: impl FnOnce(&mut super::slots::SlotManager, u32) -> super::slots::SlotChange,
    id: u32,
) {
    let change = change(&mut engine.slots, id);
    engine.apply_slot_change(change, world);
}

/// Verb 84 (`0x233E20` per key): the "no tag" key stops the sound of every active verb-1 instance,
/// any other key clears that actor's target.
fn release_handles(
    engine: &mut Engine,
    record: &super::record::EventRecord,
    world: &mut dyn World,
) {
    for offset in [0xC, 0x10, 0x14, 0x18] {
        let key = record.arg(offset);
        if key == 0 {
            continue;
        }
        if key == NO_TAG {
            for &active in &engine.active().to_vec() {
                if engine.records[active].verb == 1
                    && let Some(handle) = engine.instances[active].sound_handle.take()
                {
                    world.stop_sound(handle);
                }
            }
        } else {
            world.clear_target(key);
        }
    }
}

fn apply(
    engine: &mut Engine,
    index: usize,
    verb: u32,
    world: &mut dyn World,
) -> Result<Outcome, EngineError> {
    let record = engine.records[index];
    let unsupported = EngineError::Unsupported {
        what: "verb",
        value: verb,
    };
    let Some(action) = actions::decode(&record) else {
        return if world.other_verb(verb, index) {
            Ok(Outcome::Ran)
        } else {
            Err(unsupported)
        };
    };
    match world.apply_action(&action) {
        ActionResult::Unsupported => {
            if world.other_verb(verb, index) {
                Ok(Outcome::Ran)
            } else {
                Err(unsupported)
            }
        }
        ActionResult::Done => Ok(Outcome::Ran),
        ActionResult::Handle(handle) => {
            match action {
                Action::Effect { .. } | Action::Sound { .. } => {
                    engine.instances[index].sound_handle = Some(handle)
                }
                _ => engine.instances[index].object_handle = Some(handle),
            }
            if let Action::ObjectAnimStart { callback, .. } = action
                && callback != 0
            {
                engine.dispatch_callback(callback, CallbackTarget::Object(handle), index, world);
            }
            Ok(Outcome::Ran)
        }
        ActionResult::Failed => Ok(if matches!(verb, 43 | 46) {
            Outcome::Aborted
        } else {
            Outcome::Ran
        }),
    }
}

/// `0x233BD0`, called from the state pass when a verb-10 instance with a sound id is done: spawn
/// the burst effect and run the definition's callback with the instance.
pub(crate) fn effect_burst(engine: &mut Engine, index: usize, world: &mut dyn World) {
    let record = engine.records[index];
    let action = Action::EffectBurst {
        effect: record.arg(0x10),
        room: record.arg(0x14),
        sub_entry: record.arg(0x18) as i32,
        position: [
            record.arg_f32(0x20),
            record.arg_f32(0x24),
            record.arg_f32(0x28),
        ],
        extra: [
            record.arg_f32(0x2C),
            record.arg_f32(0x30),
            record.arg_f32(0x34),
        ],
    };
    let _ = world.apply_action(&action);
    if record.callback != 0 {
        engine.dispatch_callback(
            record.callback,
            CallbackTarget::Instance(index),
            index,
            world,
        );
    }
}

fn voice_request(engine: &Engine, index: usize) -> VoiceRequest {
    let record = &engine.records[index];
    VoiceRequest {
        label: record.arg(0x10),
        tag: record.arg(0x14),
        speaker_tag: record.arg(0x18),
        volume: record.arg_f32(0x1C),
        priority: record.has_voice_priority(),
    }
}

/// Verb 1 (`0x23635D`, MEASURED): scan the pool for another verb-1 instance that is fired and
/// running (state 1 or 2). If its definition has priority 0x40, a priority definition queues itself
/// and a normal one is dropped (it plays nothing and completes in the tail). If it has none, a
/// priority definition queues and a normal one keeps scanning. No decisive instance: play.
fn voice(engine: &mut Engine, index: usize, world: &mut dyn World) {
    let mine = engine.records[index].has_voice_priority();
    for other in 0..engine.len() {
        let instance = &engine.instances[other];
        if other == index
            || engine.records[other].verb != 1
            || !instance.fired
            || instance.running == 0
        {
            continue;
        }
        if engine.records[other].has_voice_priority() {
            if mine {
                queue_voice(engine, index);
            }
            return;
        }
        if mine {
            queue_voice(engine, index);
            return;
        }
    }
    start_voice(engine, index, world);
}

/// `0x234C00`: running = 2 (suppressed) and checkpoint mask -1, appended to the queue.
fn queue_voice(engine: &mut Engine, index: usize) {
    let instance = &mut engine.instances[index];
    instance.queued = true;
    instance.running = 2;
    instance.checkpoint_mask = u32::MAX;
    engine.voice_queue.push_back(index);
}

fn start_voice(engine: &mut Engine, index: usize, world: &mut dyn World) {
    let request = voice_request(engine, index);
    engine.instances[index].queued = false;
    if let Some(handle) = world.play_voice(&request) {
        engine.instances[index].sound_handle = Some(handle);
    }
}

fn spawn(engine: &mut Engine, index: usize, world: &mut dyn World) {
    let record = engine.records[index];
    let request = SpawnRequest {
        group_key: record.arg(0xC),
        object_type: record.arg(0x10),
        tag: record.arg(0x14),
        variant: record.arg(0x18),
        scale: record.arg_f32(0x1C),
        position: [
            record.arg_f32(0x20),
            record.arg_f32(0x24),
            record.arg_f32(0x28),
        ],
        orientation: [
            record.arg_f32(0x2C),
            record.arg_f32(0x30),
            record.arg_f32(0x34),
        ],
        callback: record.callback,
        tick_callback: record.tick_callback,
        flags: record.flags,
    };
    match world.spawn_object(&request) {
        Some(handle) => {
            engine.instances[index].object_handle = Some(handle);
            if record.callback != 0 {
                engine.dispatch_callback(
                    record.callback,
                    CallbackTarget::Object(handle),
                    index,
                    world,
                );
            }
        }
        // No object was created: the lifetime tail resolves a missing handle as done.
        None => engine.instances[index].done = true,
    }
}

/// Tail pass for one instance (`0x237920`, MEASURED): runs for instances with running == 1 whose
/// done flag is clear. A running instance stays 1 after it finished (only a re-arm clears it).
pub(crate) fn tail(
    engine: &mut Engine,
    index: usize,
    world: &mut dyn World,
    report: &mut StepReport,
) {
    let instance = &engine.instances[index];
    if instance.running != 1 || instance.done {
        return;
    }
    let record = engine.records[index];
    // Per-tick callback of every verb except 3, on the instance's object while its handle resolves.
    if record.verb != 3
        && record.tick_callback != 0
        && let Some(handle) = engine.instances[index].object_handle
    {
        if world.object_exists(handle) {
            engine.dispatch_callback(
                record.tick_callback,
                CallbackTarget::Object(handle),
                index,
                world,
            );
        } else {
            engine.instances[index].object_handle = None;
        }
    }
    match record.verb {
        1 => voice_tail(engine, index, world, report),
        3 => object_lifetime_tail(engine, index, world),
        10 => effect_tail(engine, index, world, report),
        verb if actions::tail_sets_flag_at_once(verb) => engine.instances[index].done = true,
        verb => wait_tail(engine, index, verb, world),
    }
    engine.refresh_done(index);
}

/// Verb 1 tail (`0x237AF2`): done when no sound was started, or when the subtitle timer is below
/// 0.5 s past the fire time and the sample stopped. On done the next queued voice starts.
fn voice_tail(engine: &mut Engine, index: usize, world: &mut dyn World, report: &mut StepReport) {
    let timer = engine.instances[index].delay;
    let going = match engine.instances[index].sound_handle {
        None => timer < VOICE_NO_SOUND_TIMER,
        Some(handle) => timer > VOICE_MIN_TIMER || !world.voice_finished(handle),
    };
    if going {
        return;
    }
    engine.instances[index].sound_handle = None;
    engine.instances[index].done = true;
    if let Some(next) = engine.voice_queue.pop_front() {
        engine.fire(next, world, report);
        start_voice(engine, next, world);
        engine.instances[next].running = 1;
    }
}

/// Verbs 3 (`0x2379B3`): no object means done; a kept object (flag 0x10) is done once finished,
/// otherwise it is destroyed first.
fn object_lifetime_tail(engine: &mut Engine, index: usize, world: &mut dyn World) {
    let record = engine.records[index];
    let Some(handle) = engine.instances[index].object_handle else {
        engine.instances[index].done = true;
        return;
    };
    if !world.object_exists(handle) {
        engine.instances[index].done = true;
        return;
    }
    if !world.object_ready(handle) {
        return;
    }
    if record.flags & FLAG_KEEP_OBJECT == 0 {
        world.destroy_object(handle);
        engine.instances[index].object_handle = None;
    }
    engine.instances[index].done = true;
}

/// Verb 10 tail (`0x237A3A`, MEASURED): every pass clears the link A word and reloads the delay
/// with 10.0 s. When the room is active and the previous play finished, the effect starts again
/// with probability `weight * frame_time` per frame (`u >= 1 - weight/60`).
fn effect_tail(engine: &mut Engine, index: usize, world: &mut dyn World, report: &mut StepReport) {
    let record = engine.records[index];
    let finished = record.arg(0xC) == u32::MAX
        || engine.instances[index]
            .sound_handle
            .is_none_or(|handle| world.wait_done(&Wait::SoundFinished { handle }));
    let room_active = world.room_active(record.arg(0x14));
    engine.instances[index].world_done = finished;
    engine.instances[index].a_set = false;
    engine.instances[index].delay = EFFECT_RELOAD_DELAY;
    if room_active && finished && record.weight() > 0.0 {
        let threshold = 1.0 - record.weight() * engine.last_dt;
        if world.random_unit() >= threshold {
            engine.instances[index].running = 0;
            engine.execute(index, world, report);
        }
    }
}

/// Verbs whose done state comes from a world wait.
fn wait_tail(engine: &mut Engine, index: usize, verb: u32, world: &mut dyn World) {
    let record = engine.records[index];
    let handle = match verb {
        4 | 5 | 7 => engine.instances[index].object_handle,
        _ => engine.instances[index].sound_handle,
    };
    if matches!(verb, 4 | 5 | 7) && handle.is_some_and(|h| !world.object_exists(h)) {
        engine.instances[index].done = true;
        return;
    }
    let Some(wait) = actions::wait_for(&record, handle) else {
        return;
    };
    let class = actions::completion(verb);
    if matches!(class, Completion::Immediate | Completion::NoTickCallback) {
        return;
    }
    let finished = world.wait_done(&wait);
    match class {
        // Sounds are polled live every pass.
        Completion::Sound | Completion::SoundOrNone => {
            engine.instances[index].world_done = finished
        }
        // The other waits also set the done flag, so a one-shot wait is not asked again.
        _ if finished => {
            engine.instances[index].world_done = true;
            engine.instances[index].done = true;
        }
        _ => {}
    }
}
