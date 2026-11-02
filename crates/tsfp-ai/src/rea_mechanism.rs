// SPDX-License-Identifier: GPL-3.0-or-later
//! Rust-owned boundary for the 0x2A0500 mechanism states 0..14 (state 12 is the
//! original's empty default arm). States 4 and 5 use the readable port of the
//! x87-returning 0x172CD0 (`mechanism_speed_ramp`, f32 result) and the +0x9E0 timer.
//!
//! The pool identity and state effects follow the bounded static contract in
//! `docs/t1662-rea-mechanism.md`. Pool slots and projected navigation records
//! are valid-world inputs; the full controller, pool lifecycle, and common
//! actor tick prologue remain outside this slice. Tier 1 differential coverage
//! is in `tests/t2395_rea.rs`.

use alloc::vec::Vec;

use crate::{
    movement_approach::Position3,
    navigation_scoring::{ActorId, NodeId, ObjectId},
};

/// The upper ten handle bits select a pool slot; the complete DWORD is identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MechanismHandle(pub u32);

impl MechanismHandle {
    pub const fn slot(self) -> usize {
        (self.0 >> 22) as usize
    }
}

/// A zero token is inert; nonzero handles require exact full-token identity.
pub const fn mechanism_handle_matches(handle: MechanismHandle, identity: u32) -> bool {
    handle.0 != 0 && handle.0 == identity
}

/// Borrowed mechanism subrecord owner. `speed` is the literal +0x394 write
/// observed for classification zero; its units and consumer are unspecified.
#[derive(Clone, Debug, PartialEq)]
pub struct MechanismRecord {
    pub identity: u32,
    pub object: ObjectId,
    pub speed: f32,
    /// Literal DWORD at record +0x390 (bit0/bit1 written by states 2..6; meaning unspecified).
    pub flags: u32,
}

#[derive(Debug)]
pub struct MechanismView<'a> {
    pub object: ObjectId,
    pub slot: usize,
    pub record: &'a MechanismRecord,
}

/// A valid loaded pool. As in the original resolver, slot bounds are an
/// external invariant; this model does not make malformed handles safe.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MechanismPool {
    pub slots: Vec<Option<MechanismRecord>>,
}

impl MechanismPool {
    /// Resolves a handle and clears only a stale identity in the actor's slot.
    /// Zero stays zero and does not mutate the slot.
    pub fn resolve<'a>(
        &'a self,
        handle: &mut Option<MechanismHandle>,
    ) -> Option<MechanismView<'a>> {
        let current = (*handle)?;
        if current.0 == 0 {
            return None;
        }
        let Some(record) = self.slots[current.slot()].as_ref() else {
            *handle = None;
            return None;
        };
        if !mechanism_handle_matches(current, record.identity) {
            *handle = None;
            return None;
        }
        Some(MechanismView {
            object: record.object,
            slot: current.slot(),
            record,
        })
    }
}

/// Inputs of 0x172CD0, read from the mechanism object's extension (`object+0x7c`).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct SpeedRampInputs {
    /// Extension +0x58 bit 0: the ramp collapses to the sign of the request.
    pub sign_only: bool,
    /// Extension +0x52c: the reference value subtracted from the request.
    pub reference: f32,
}

fn sign_unit(value: f32) -> f32 {
    f32::from_bits((value.to_bits() & 0x8000_0000) | 0x3f80_0000)
}

/// Port of 0x172CD0 (original returns the value in x87 st0 as a float; the
/// adapter boundary stores it as f32, which is exact). A fast-sqrt estimate of
/// `|request - reference|` (one Newton step on a bit-shift guess), clamped to
/// `0..=1` and signed like the difference.
pub fn mechanism_speed_ramp(request: f32, inputs: SpeedRampInputs) -> f32 {
    if inputs.sign_only {
        return sign_unit(request);
    }
    let delta = request - inputs.reference;
    let magnitude = delta.abs();
    let guess =
        f32::from_bits(((magnitude.to_bits() as i32 >> 1) as u32).wrapping_add(0x1fc0_0000));
    let ratio = (guess * guess + magnitude) / (guess * 2.0);
    let clamped = if ratio > 1.0 {
        1.0
    } else if ratio < 0.0 {
        0.0
    } else {
        ratio
    };
    clamped * sign_unit(delta)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BehaviorRequest {
    pub mask: u32,
}

/// Actor-owned local mechanism state. The mechanism handle is actor-owned;
/// `MechanismView` borrows a separate mechanism subrecord.
#[derive(Clone, Debug, PartialEq)]
pub struct ActorInteractionState {
    pub local_state: u32,
    pub previous_state: u32,
    pub pending_state: Option<u32>,
    pub elapsed: f32,
    /// Extension +0x9E0 countdown used by state 4 (re-armed to 2.0).
    pub timer: f32,
    pub mechanism: Option<MechanismHandle>,
    pub target: Option<ObjectId>,
    pub current_node: Option<NodeId>,
    pub cell: Option<i16>,
    pub behavior_request: Option<BehaviorRequest>,
}

impl Default for ActorInteractionState {
    fn default() -> Self {
        Self {
            local_state: 0,
            previous_state: 0,
            pending_state: None,
            elapsed: 0.0,
            timer: 0.0,
            mechanism: None,
            target: None,
            current_node: None,
            cell: None,
            behavior_request: None,
        }
    }
}

/// External calls at the selected mechanism boundary. The implementation owns
/// interaction semantics, projection, and pending behavior dispatch.
pub trait MechanismWorld {
    /// `argument` is the float passed to 0x17E010 (0.0 except state 6, which passes 1.0).
    fn interact(
        &mut self,
        mechanism: &MechanismView<'_>,
        actor: ActorId,
        state: u32,
        argument: f32,
    ) -> i32;
    /// 0x29FE60, called with no arguments before interaction in states 7, 9, 10, 11.
    fn prepare_interaction(&mut self);
    /// 0x29FE20(actor) used as a bare predicate (state 9).
    fn actor_predicate(&mut self, actor: ActorId) -> bool;
    /// Re-reads the actor's target word (+4), which the interaction call may rewrite.
    fn current_target(&mut self) -> Option<ObjectId>;
    /// 0x177E90 (state 3 gate).
    fn mechanism_ready(&mut self, mechanism: ObjectId) -> bool;
    /// 0x177F20 (state 3, after `mechanism_ready`).
    fn mechanism_begin(&mut self, mechanism: ObjectId);
    /// Bit 0x10 of 0x177950(mechanism).
    fn mechanism_exit_bit(&mut self, mechanism: ObjectId) -> bool;
    /// 0x179F70(mechanism, &out): `None` when it returns 0, else the DWORD at out+0x150.
    fn mechanism_link_field(&mut self, mechanism: ObjectId) -> Option<u32>;
    /// Reads the 0x172CD0 inputs from the mechanism object's extension.
    fn speed_ramp_inputs(&mut self, mechanism: ObjectId) -> SpeedRampInputs;
    /// Per-frame step `1.0 / frame_rate * delta` (same expression as the common prologue).
    fn frame_step(&mut self) -> f32;
    /// 0x177FB0(mechanism, 0x1596F0(actor, record+0x48, 0)) != 0 (state 4 re-arm probe).
    fn mechanism_probe(&mut self, actor: ActorId, mechanism: ObjectId) -> bool;
    fn classification(&mut self, actor: ActorId) -> u32;
    fn choose_state_eight(&mut self, actor: ActorId, mechanism: ObjectId) -> bool;
    fn mechanism_count(&mut self, mechanism: ObjectId) -> i32;
    fn project_position(&mut self, position: Position3) -> (NodeId, i16);
    fn controls_same_actor_extension(&mut self, actor: ActorId) -> bool;
    fn queue_behavior(&mut self, actor: ActorId, request: BehaviorRequest);
    fn refresh_actor(&mut self, actor: ActorId);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MechanismTickResult {
    Returned,
    RequestedLocalState(u32),
    ExitedToBehavior(BehaviorRequest),
}

fn request_local_state(state: &mut ActorInteractionState, next: u32) -> bool {
    if state.local_state == next {
        return false;
    }
    state.previous_state = state.local_state;
    state.local_state = next;
    state.pending_state = Some(next);
    state.elapsed = 0.0;
    true
}

fn requested(state: &mut ActorInteractionState, next: u32) -> MechanismTickResult {
    if request_local_state(state, next) {
        MechanismTickResult::RequestedLocalState(next)
    } else {
        MechanismTickResult::Returned
    }
}

/// Stale or missing handle: every handled state falls to the shared "request 14" tail.
fn to_exit(state: &mut ActorInteractionState) -> MechanismTickResult {
    requested(state, 14)
}

/// Shared 0x2A0C4D tail: a set exit bit returns, otherwise request state 13.
fn exit_or_return(state: &mut ActorInteractionState, exit_bit: bool) -> MechanismTickResult {
    if exit_bit {
        MechanismTickResult::Returned
    } else {
        requested(state, 13)
    }
}

fn record_mut(pool: &mut MechanismPool, slot: usize) -> &mut MechanismRecord {
    pool.slots[slot]
        .as_mut()
        .expect("resolved mechanism record")
}

/// Re-resolves the (already valid) handle and calls the interaction operation.
fn interact_at<W: MechanismWorld>(
    pool: &mut MechanismPool,
    state: &mut ActorInteractionState,
    world: &mut W,
    actor: ActorId,
    argument: f32,
) -> i32 {
    let local = state.local_state;
    let view = pool
        .resolve(&mut state.mechanism)
        .expect("handle resolved by caller");
    world.interact(&view, actor, local, argument)
}

/// Runs only the selected local states established by the mechanism dossier.
/// The common 0x15E120 / 0x15D470 tick prologue is supplied by the caller.
pub fn tick_selected_state<W: MechanismWorld>(
    actor: ActorId,
    actor_position: Position3,
    state: &mut ActorInteractionState,
    pool: &mut MechanismPool,
    world: &mut W,
) -> MechanismTickResult {
    match state.local_state {
        0 | 1 => {
            let Some(view) = pool.resolve(&mut state.mechanism) else {
                return to_exit(state);
            };

            if world.interact(&view, actor, state.local_state, 0.0) != 2 {
                return MechanismTickResult::Returned;
            }

            let next = if state.local_state == 1 {
                4
            } else {
                match world.classification(actor) {
                    0 => {
                        let mechanism = view.object;
                        let slot = view.slot;
                        drop(view);
                        pool.slots[slot]
                            .as_mut()
                            .expect("resolved mechanism record")
                            .speed = 8.0;
                        let choose_eight = world.choose_state_eight(actor, mechanism)
                            && world.mechanism_count(mechanism) <= 1;
                        if choose_eight { 8 } else { 1 }
                    }
                    1 | 2 => 11,
                    3 | 4 => 9,
                    _ => return MechanismTickResult::Returned,
                }
            };
            requested(state, next)
        }
        2..=11 | 13 => {
            let local = state.local_state;
            let Some(view) = pool.resolve(&mut state.mechanism) else {
                return to_exit(state);
            };
            let (object, slot) = (view.object, view.slot);
            drop(view);
            match local {
                2 => {
                    record_mut(pool, slot).flags &= !2;
                    if interact_at(pool, state, world, actor, 0.0) == 2 {
                        requested(state, 4)
                    } else {
                        MechanismTickResult::Returned
                    }
                }
                3 => {
                    if !world.mechanism_ready(object) {
                        return requested(state, 1);
                    }
                    world.mechanism_begin(object);
                    record_mut(pool, slot).flags |= 2;
                    if interact_at(pool, state, world, actor, 0.0) == 2 {
                        requested(state, 4)
                    } else {
                        MechanismTickResult::Returned
                    }
                }
                4 => {
                    world.prepare_interaction();
                    let record = pool.slots[slot]
                        .as_ref()
                        .expect("resolved mechanism record");
                    let (flags, speed) = (record.flags, record.speed);
                    let base = if flags & 1 != 0 { -0.8f32 } else { 1.0 };
                    let argument =
                        mechanism_speed_ramp(base * speed, world.speed_ramp_inputs(object));
                    let result = interact_at(pool, state, world, actor, argument);
                    match result {
                        0 | 1 => {
                            if result == 0 {
                                if state.timer <= 0.0 || flags & 1 == 0 {
                                    return MechanismTickResult::Returned;
                                }
                                state.timer -= world.frame_step();
                                if 0.0 < state.timer {
                                    return MechanismTickResult::Returned;
                                }
                            }
                            if world.mechanism_probe(actor, object) {
                                record_mut(pool, slot).flags |= 1;
                                state.timer = 2.0;
                            } else {
                                record_mut(pool, slot).flags &= !1;
                            }
                            MechanismTickResult::Returned
                        }
                        2 => requested(state, if flags & 2 == 0 { 6 } else { 2 }),
                        5 => {
                            let outcome = requested(state, 5);
                            state.timer = 2.0;
                            outcome
                        }
                        6 => requested(state, 3),
                        10 => {
                            state.target = None;
                            MechanismTickResult::Returned
                        }
                        _ => MechanismTickResult::Returned,
                    }
                }
                5 => {
                    let record = pool.slots[slot]
                        .as_ref()
                        .expect("resolved mechanism record");
                    let base = if record.flags & 1 != 0 { 0.8f32 } else { -0.8 };
                    let argument =
                        mechanism_speed_ramp(base * record.speed, world.speed_ramp_inputs(object));
                    match interact_at(pool, state, world, actor, argument) {
                        2 | 5 => requested(state, 4),
                        10 => {
                            state.target = None;
                            MechanismTickResult::Returned
                        }
                        _ => MechanismTickResult::Returned,
                    }
                }
                6 => {
                    let result = interact_at(pool, state, world, actor, 1.0);
                    if result != 2 && result != 5 {
                        return MechanismTickResult::Returned;
                    }
                    record_mut(pool, slot).flags &= !3;
                    requested(
                        state,
                        if world.mechanism_exit_bit(object) {
                            7
                        } else {
                            13
                        },
                    )
                }
                7 => {
                    world.prepare_interaction();
                    interact_at(pool, state, world, actor, 0.0);
                    MechanismTickResult::Returned
                }
                8 => {
                    if world.mechanism_count(object) < 2 {
                        MechanismTickResult::Returned
                    } else {
                        requested(state, 1)
                    }
                }
                9 => {
                    world.prepare_interaction();
                    interact_at(pool, state, world, actor, 0.0);
                    state.target = world.current_target();
                    if state.target.is_some() {
                        return requested(state, 10);
                    }
                    if !world.actor_predicate(actor)
                        || world.mechanism_exit_bit(object)
                        || world.mechanism_link_field(object).is_some_and(|f| f != 0)
                    {
                        return MechanismTickResult::Returned;
                    }
                    requested(state, 13)
                }
                10 => {
                    world.prepare_interaction();
                    match interact_at(pool, state, world, actor, 0.0) {
                        2 | 6 | 10 => {
                            let exit = world.controls_same_actor_extension(actor)
                                && !world.mechanism_exit_bit(object)
                                && world.mechanism_link_field(object).is_none_or(|f| f == 0);
                            requested(state, if exit { 13 } else { 9 })
                        }
                        5 => exit_or_return(state, world.mechanism_exit_bit(object)),
                        _ => MechanismTickResult::Returned,
                    }
                }
                11 => {
                    world.prepare_interaction();
                    match interact_at(pool, state, world, actor, 0.0) {
                        2 | 4 => exit_or_return(state, world.mechanism_exit_bit(object)),
                        10 => {
                            state.target = None;
                            MechanismTickResult::Returned
                        }
                        _ => MechanismTickResult::Returned,
                    }
                }
                // 13
                _ => {
                    if interact_at(pool, state, world, actor, 0.0) == 2 {
                        to_exit(state)
                    } else {
                        MechanismTickResult::Returned
                    }
                }
            }
        }
        // 12 is the original's empty default arm.
        14 => {
            let (node, cell) = world.project_position(actor_position);
            state.current_node = Some(node);
            state.cell = Some(cell);
            let request = if world.controls_same_actor_extension(actor) {
                BehaviorRequest { mask: 0x2000 }
            } else {
                state.target = None;
                BehaviorRequest { mask: 4 }
            };
            state.behavior_request = Some(request);
            world.queue_behavior(actor, request);
            world.refresh_actor(actor);
            MechanismTickResult::ExitedToBehavior(request)
        }
        _ => MechanismTickResult::Returned,
    }
}
