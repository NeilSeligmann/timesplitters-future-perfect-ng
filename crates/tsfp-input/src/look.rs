// SPDX-License-Identifier: GPL-3.0-or-later
//! Readable T2194 look slice. Contract, scope and evidence: docs/t-player-look.md.
#[path = "look_native.rs"]
mod native;
use crate::player::{AssistMask, Look};
use crate::shaping::{PlayerSettings, Preset, Shaper};
pub use native::*;
use tsfp_state::{GuestMemory, player::PlayerSet};

#[path = "view.rs"]
mod view;
pub use view::{ViewAngles, ViewConstraints};

impl ViewAngles {
    /// Native count handler; controllers retain fixed-step `LookIntegration::step`.
    pub fn apply_mouse(&mut self, degrees: Vec2, constraints: ViewConstraints) -> bool {
        self.apply_degrees(degrees.x, degrees.y, constraints)
    }
}
impl LookState {
    pub fn from_player(player: &PlayerSet) -> Self {
        let float = |offset| f32::from_bits(player.record_word(offset));
        Self {
            yaw: float(0x100),
            pitch: float(0x104),
            ramp: float(0xf80),
            lookahead: player.record_word(0x108),
            move_previous: Vec2::new(float(0xfc8), float(0xfc4)),
            state: player.record_word(0x114),
            alternate_input: player.record_word(0xf5c) != 0,
            locked: player.record_word(0xc) & 0x8000 != 0,
            integration_allowed: player.record_word(0xc) & 0x80 != 0,
            special_ramp: player.record_word(0xf7c) != 0,
            slew_row: player.record_word(0x11c),
        }
    }
    pub fn write_player<M: GuestMemory>(&self, player: &mut PlayerSet, memory: &mut M) -> bool {
        if player.record_ptr == 0 {
            return false;
        }
        for (offset, value) in [
            (0x100, self.yaw),
            (0x104, self.pitch),
            (0xf80, self.ramp),
            (0xfc4, self.move_previous.y),
            (0xfc8, self.move_previous.x),
        ] {
            player.set_record_word(memory, offset, value.to_bits());
        }
        player.set_record_word(memory, 0x108, self.lookahead);
        true
    }
}

impl LookProfile {
    pub fn effective(self, allowed: AssistMask) -> AssistMask {
        let mut bits = 0;
        for (flag, assist) in [
            (1, AssistMask::INVERSE_LOOK),
            (8, AssistMask::AUTO_LOOKAHEAD),
            (128, AssistMask::AUTO_AIM),
        ] {
            if self.flags & flag != 0 {
                bits |= assist;
            }
        }
        bits |= if self.crosshair == 2 {
            AssistMask::EDGE_BOOST
        } else {
            AssistMask::PITCH_DEAD_ZONE
        };
        AssistMask(bits & allowed.0)
    }
}

/// External object predicate is the result of 0x1B6AE0, not a fabricated target pull.
#[derive(Clone, Copy, Debug)]
pub struct TargetingContext {
    pub game_flags: u8,
    pub mode: u32,
    pub object_allows_targeting: bool,
}
pub fn targeting_enabled(
    state: &LookState,
    assists: AssistMask,
    context: TargetingContext,
) -> bool {
    crate::look_gate::TargetingState {
        auto_aim: assists.0 & AssistMask::AUTO_AIM != 0,
        alternate_input: state.alternate_input,
        player_mode: state.state,
        session_flags: context.game_flags,
        session_mode: context.mode,
    }
    .allows(context.object_allows_targeting)
}

impl SlewRow {
    /// Projects row 1 of the original interleaved table into typed rates.
    /// Every column carries the runtime frame scale at 0x4E794C.
    /// Verification Tier 1: 512 fabricated-state projected-rate differentials.
    pub fn from_guest_row1<M: GuestMemory>(memory: &M) -> Self {
        let frame_scale = f32::from_bits(memory.read32(0x4e794c));
        Self {
            forward_up: f32::from_bits(memory.read32(0x49f2a4)) * frame_scale,
            forward_down: f32::from_bits(memory.read32(0x49f2ac)) * frame_scale,
            strafe: f32::from_bits(memory.read32(0x49f2b4)) * frame_scale,
        }
    }
}

impl Integration {
    /// Fills the row-1 slew inputs from the current guest data image. Call this
    /// for each step: the table and frame-rate cell are runtime data.
    pub fn with_guest_slew<M: GuestMemory>(mut self, memory: &M) -> Self {
        self.slew_row1 = Some(SlewRow::from_guest_row1(memory));
        self.step_rate = (memory.read32(0x4e8cec) as i32) as f32;
        self.time_scale = f32::from_bits(memory.read32(0x475c78));
        self
    }
}

/// Shaping and assist extension point. Values use contract signs and degrees per step.
pub trait LookPolicy {
    fn shape(&mut self, look: Look, assists: AssistMask) -> Vec2;
    fn movement(&mut self, movement: Vec2, previous: Vec2) -> Vec2;
    /// Unslewed move target for the table-driven slew rows. `None` (the default) keeps
    /// nonzero rows with the reference path.
    fn movement_target(&mut self, _movement: Vec2) -> Option<Vec2> {
        None
    }
    fn assists(&self, profile: LookProfile, allowed: AssistMask) -> AssistMask {
        profile.effective(allowed)
    }
}
pub struct OriginalLook {
    shaper: Shaper,
}
impl Default for OriginalLook {
    fn default() -> Self {
        Self {
            shaper: Shaper::new(Preset::original(), PlayerSettings::default()),
        }
    }
}
impl LookPolicy for OriginalLook {
    fn shape(&mut self, look: Look, assists: AssistMask) -> Vec2 {
        match look {
            Look::Stick(stick) => self.shaper.shape_stick(stick, assists),
            Look::Delta { yaw_deg, pitch_deg } => Vec2::new(yaw_deg, pitch_deg),
        }
    }
    fn movement(&mut self, movement: Vec2, previous: Vec2) -> Vec2 {
        self.shaper.restore_move(previous);
        self.shaper.shape_move(movement)
    }
    fn movement_target(&mut self, movement: Vec2) -> Option<Vec2> {
        Some(self.shaper.move_target(movement))
    }
}

/// Opt-in per-player assist disable preset: wraps any policy and removes the
/// `disabled` assists after the profile projection. An empty mask is the original.
#[derive(Default)]
pub struct AssistOverride<P = OriginalLook> {
    pub inner: P,
    pub disabled: AssistMask,
}
impl<P: LookPolicy> LookPolicy for AssistOverride<P> {
    fn shape(&mut self, look: Look, assists: AssistMask) -> Vec2 {
        self.inner.shape(look, assists)
    }
    fn movement(&mut self, movement: Vec2, previous: Vec2) -> Vec2 {
        self.inner.movement(movement, previous)
    }
    fn movement_target(&mut self, movement: Vec2) -> Option<Vec2> {
        self.inner.movement_target(movement)
    }
    fn assists(&self, profile: LookProfile, allowed: AssistMask) -> AssistMask {
        AssistMask(self.inner.assists(profile, allowed).0 & !self.disabled.0)
    }
}

/// Per-frame intent and policy settings for the typed PlayerSet attachment.
#[derive(Clone, Copy, Debug)]
pub struct LookStep {
    pub look: Look,
    pub movement: Vec2,
    pub profile: LookProfile,
    pub allowed: AssistMask,
    pub config: Integration,
}

/// A/B selection defaults off; unsupported states explicitly stay with the oracle.
#[derive(Default)]
pub struct LookIntegration<P = OriginalLook> {
    pub enabled: bool,
    pub policy: P,
}
impl<P: LookPolicy> LookIntegration<P> {
    /// Display-rate mouse route. Only view angles change: the caller still advances
    /// ramp, movement, timers and gameplay once per fixed 60 Hz simulation tick.
    /// This acts on Rust-owned state and does not use synthetic pad input or memory writes.
    pub fn mouse_view(
        &self,
        state: &mut LookState,
        degrees: Vec2,
        constraints: ViewConstraints,
    ) -> bool {
        if !self.enabled
            || state.state != 2
            || state.alternate_input
            || !state.integration_allowed
            || state.locked
            || state.special_ramp
        {
            return false;
        }
        let mut angles = ViewAngles {
            yaw: state.yaw,
            pitch: state.pitch,
        };
        if !angles.apply_mouse(degrees, constraints) {
            return false;
        }
        state.yaw = angles.yaw;
        state.pitch = angles.pitch;
        true
    }

    /// Typed PlayerSet attachment. A rejected step leaves both the snapshot and
    /// guest memory untouched; the caller can then run the reference path.
    pub fn step_player<M: GuestMemory>(
        &mut self,
        player: &mut PlayerSet,
        memory: &mut M,
        input: LookStep,
    ) -> bool {
        if player.record_ptr == 0 {
            return false;
        }
        let mut state = LookState::from_player(player);
        if !self.step(
            &mut state,
            input.look,
            input.movement,
            input.profile,
            input.allowed,
            input.config,
        ) {
            return false;
        }
        state.write_player(player, memory)
    }

    /// Auto-aim gate (0xCFF20) with the policy's assist set, so an assist
    /// override also disables targeting for that player.
    pub fn targeting(
        &self,
        state: &LookState,
        profile: LookProfile,
        allowed: AssistMask,
        context: TargetingContext,
    ) -> bool {
        targeting_enabled(state, self.policy.assists(profile, allowed), context)
    }

    pub fn step(
        &mut self,
        state: &mut LookState,
        look: Look,
        movement: Vec2,
        profile: LookProfile,
        allowed: AssistMask,
        config: Integration,
    ) -> bool {
        if !self.enabled
            || state.state != 2
            || state.alternate_input
            || !state.integration_allowed
            || state.locked
            || state.special_ramp
        {
            return false;
        }
        let slewed = match state.slew_row {
            0 => None,
            1 => match (config.slew_row1, self.policy.movement_target(movement)) {
                (Some(row), Some(target)) => {
                    Some(slew_movement(target, state.move_previous, row, config))
                }
                _ => return false,
            },
            _ => return false,
        };
        let assists = self.policy.assists(profile, allowed);
        let mut delta = self.policy.shape(look, assists);
        state.move_previous = match slewed {
            Some(movement) => movement,
            None => self.policy.movement(movement, state.move_previous),
        };
        let raw = matches!(look, Look::Delta { .. });
        if !raw {
            delta.x *= profile.turn_speed + 0.5;
            delta.y *= profile.turn_speed + 0.5;
        }
        integrate_default(state, delta, raw, assists, config);
        true
    }
}

/// Default inline integration of 0x25D520. Rates have already been applied by the shaper.
pub fn integrate_default(
    state: &mut LookState,
    delta: Vec2,
    raw: bool,
    assists: AssistMask,
    config: Integration,
) {
    state.ramp += (1.0 - state.ramp) * 0.3;
    if raw {
        let mut angles = ViewAngles {
            yaw: state.yaw,
            pitch: state.pitch,
        };
        if angles.apply_mouse(
            delta,
            ViewConstraints {
                zoom: config.zoom,
                pitch_limit: config.pitch_limit,
            },
        ) {
            state.yaw = angles.yaw;
            state.pitch = angles.pitch;
        }
        return;
    }
    match state.lookahead {
        0 if delta.y != 0.0 && state.move_previous.y != 0.0 => state.lookahead = 1,
        0 if delta.y == 0.0 && state.move_previous.y.abs() > 0.75 => state.lookahead = 2,
        1 if state.move_previous.y == 0.0 => state.lookahead = 0,
        2 if delta.y != 0.0 => state.lookahead = 1,
        _ => {}
    }
    let yaw_gain = state.ramp * config.yaw_rate / 160.0;
    state.yaw -= delta.x * yaw_gain * config.time_scale * config.zoom;
    if state.lookahead == 2 && assists.0 & AssistMask::AUTO_LOOKAHEAD != 0 {
        state.pitch -= state.pitch * config.time_scale * 0.05;
        if state.pitch.abs() < 0.5 {
            state.lookahead = 1;
        }
    } else {
        let pitch_gain = (1.0 - (1.0 - state.ramp) * 0.5) * config.pitch_rate / 68.0;
        state.pitch += delta.y * pitch_gain * config.time_scale * config.zoom;
    }
    state.yaw = state.yaw.rem_euclid(360.0);
    state.pitch = state.pitch.clamp(-config.pitch_limit, config.pitch_limit);
}

#[cfg(test)]
#[path = "look_tests.rs"]
mod tests;
