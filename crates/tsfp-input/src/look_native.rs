// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared readable native look flow. No guest memory, pad replay or ABI registers.
//! Evidence and attachment contracts: docs/t-player-look.md.
#[path = "look_vector.rs"]
mod vector;
pub use vector::Vec2;

fn wrap_positive(value: f32) -> f32 {
    let wrapped = value % 360.0;
    if wrapped < 0.0 {
        wrapped + 360.0
    } else {
        wrapped
    }
}
fn wrap_signed(value: f32) -> f32 {
    let wrapped = ((value as f64 + 180.0) % 360.0 + 360.0) % 360.0;
    (wrapped - 180.0) as f32
}

/// Owned semantic state. Guest offsets belong only to the PlayerSet adapter below.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LookState {
    pub yaw: f32,
    pub pitch: f32,
    pub ramp: f32,
    pub lookahead: u32,
    pub move_previous: Vec2,
    pub state: u32,
    pub alternate_input: bool,
    pub locked: bool,
    pub integration_allowed: bool,
    pub special_ramp: bool,
    pub slew_row: u32,
}

/// Immutable profile projection: never changes saved flags or Turn Speed.
#[derive(Clone, Copy, Debug)]
pub struct LookProfile {
    pub flags: u8,
    pub crosshair: u32,
    pub turn_speed: f32,
}

/// Owned per-axis edge charge from the crosshair-mode-2 branch of 0x25D520.
/// Advances at the fixed simulation rate; no guest image or pad replay is involved.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LookEdgeHistory {
    pub charge: Vec2,
}

/// Runtime constants for the original crosshair edge response.
#[derive(Clone, Copy, Debug)]
pub struct LookEdgeConfig {
    pub threshold_positive: f32,
    pub threshold_negative: f32,
    pub charge_rate: f32,
    pub step_rate: f32,
    pub time_scale: f32,
    pub minimum_negative_charge: f32,
    pub direct_gain: f32,
    pub history_gain: f32,
}
impl LookEdgeHistory {
    /// Ordinary controller branch of the dispatcher, before target-boundary scaling.
    /// The caller supplies runtime dead zones; attached input uses its own smaller one.
    /// Alternate input uses `preprocess_alternate` instead.
    pub fn preprocess_controller(
        &mut self,
        input: Vec2,
        profile: LookProfile,
        pitch_dead_zone: f32,
        edge: LookEdgeConfig,
    ) -> Vec2 {
        let dead_zone = if profile.crosshair == 2 {
            0.0
        } else {
            pitch_dead_zone
        };
        let pitch = if input.y > dead_zone {
            input.y - dead_zone
        } else if input.y < -dead_zone {
            input.y + dead_zone
        } else {
            0.0
        };
        let mut output = Vec2::new(input.x, (1.0 / (1.0 - dead_zone)) * pitch);
        if profile.flags & 1 != 0 {
            output.y = -output.y;
        }
        if profile.crosshair == 2 {
            output = self.shape(output, edge);
        }
        output
    }

    /// Alternate-input branch: both axes share a dead zone and ignore profile inversion.
    /// The runtime alternate dead zone is bypassed by crosshair mode 2, whose edge
    /// history still runs after preprocessing. This does not reinterpret count input.
    pub fn preprocess_alternate(
        &mut self,
        input: Vec2,
        profile: LookProfile,
        alternate_dead_zone: f32,
        edge: LookEdgeConfig,
    ) -> Vec2 {
        let dead_zone = if profile.crosshair == 2 {
            0.0
        } else {
            alternate_dead_zone
        };
        let gain = 1.0 / (1.0 - dead_zone);
        let axis = |value: f32| {
            let outside = if value > dead_zone {
                value - dead_zone
            } else if value < -dead_zone {
                value + dead_zone
            } else {
                0.0
            };
            gain * outside
        };
        let mut output = Vec2::new(axis(input.x), axis(input.y));
        if profile.crosshair == 2 {
            output = self.shape(output, edge);
        }
        output
    }

    /// Input is the dead-zone/inversion result, before boundary and Turn Speed scaling.
    /// Runtime constants come from the caller; this handler invents no original data.
    pub fn shape(&mut self, input: Vec2, config: LookEdgeConfig) -> Vec2 {
        let step = (1.0 / config.step_rate) * config.time_scale * config.charge_rate;
        let axis = |input: f32, charge: &mut f32| {
            if input > config.threshold_positive {
                *charge = (*charge + step).min(1.0);
            } else {
                *charge = charge.min(0.0);
            }
            if input < config.threshold_negative {
                *charge = (*charge - step).max(config.minimum_negative_charge);
            } else {
                *charge = charge.max(0.0);
            }
            input * config.direct_gain + *charge * config.history_gain
        };
        Vec2::new(
            axis(input.x, &mut self.charge.x),
            axis(input.y, &mut self.charge.y),
        )
    }
}

/// Angle destination selected by the original dispatcher, after pair scaling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LookAngleRoute {
    /// Attached-object kinds 1 and 9 leave all output pairs and history untouched.
    Ignore,
    /// States 0x20 and 0x100 produce pairs but do not integrate angles.
    PairsOnly,
    AttachedPlayer,
    AttachedCamera,
    Player {
        integrate: bool,
    },
}

/// Owned state projection for 0x25D520 routing. The caller resolves the optional
/// camera scene tag and attached-object kind; pointer traversal stays at the boundary.
#[derive(Clone, Copy, Debug)]
pub struct LookDispatchState {
    pub player_mode: u32,
    pub flags: u32,
    pub attached_kind: u32,
    pub camera_scene_tag18: bool,
}
impl LookDispatchState {
    pub fn angle_route(self) -> LookAngleRoute {
        match self.player_mode {
            0x80 if matches!(self.attached_kind, 1 | 9) => LookAngleRoute::Ignore,
            0x80 => LookAngleRoute::AttachedCamera,
            0x40 => LookAngleRoute::AttachedPlayer,
            0x20 | 0x100 => LookAngleRoute::PairsOnly,
            _ => LookAngleRoute::Player {
                integrate: self.flags & 0x80 != 0 || self.scene_allows_input(),
            },
        }
    }

    fn scene_allows_input(self) -> bool {
        self.camera_scene_tag18 && !matches!(self.player_mode, 0x400 | 0x200)
    }

    /// Input shaping and angle integration have deliberately different gates.
    /// A closed shaping gate preserves the caller's output look pair (not zero).
    pub fn preprocess(self) -> bool {
        self.angle_route() != LookAngleRoute::Ignore
            && ((self.flags & 0x80 != 0 && self.player_mode != 0x100) || self.scene_allows_input())
    }

    /// State 0x40 forces lookahead before shaping; early-return modes do nothing.
    pub fn prepare_lookahead(self, lookahead: &mut u32) {
        if self.player_mode == 0x40 {
            *lookahead = 1;
        }
    }

    /// Final movement-only slow scaling, after angle handling. State 0x80 uses
    /// the earlier pair boundary instead; no new constants are invented here.
    pub fn finish_movement(self, movement: &mut Vec2, slow_gain: f32) {
        if self.flags & 0x10 != 0 && !matches!(self.player_mode, 0x80 | 0x40 | 0x400 | 0x200) {
            movement.x *= slow_gain;
            movement.y *= slow_gain;
        }
    }
}

/// Owned inputs for the fixed-step dispatcher preparation stage.
#[derive(Clone, Copy, Debug)]
pub struct LookPreparation {
    pub input: Vec2,
    pub alternate_input: bool,
    pub profile: LookProfile,
    pub pitch_dead_zone: f32,
    pub alternate_dead_zone: f32,
    pub edge: LookEdgeConfig,
    pub boundary: LookBoundary,
}

/// Native angle destinations of the dispatcher. Implementations own their typed
/// player/camera state; no guest pointers or synthetic controller events cross here.
/// The pair boundary has already run when a handler is called. Player handlers
/// must preserve the original distinction between a closed integration gate and
/// a pairs-only route. Attached handlers implement their separate bounds contracts.
pub trait LookAngleHandlers {
    fn player(&mut self, pairs: &mut LookAxisPairs, lookahead: &mut u32, integrate: bool);
    fn attached_player(&mut self, pairs: &mut LookAxisPairs, lookahead: &mut u32);
    fn attached_camera(&mut self, pairs: &mut LookAxisPairs, lookahead: &mut u32);
}

impl LookDispatchState {
    /// Fixed-step native dispatcher boundary. The caller supplies readable angle
    /// handlers, including the camera contract, rather than a fallback to guest code.
    /// Movement scaling observes handler output and runs last, as in 0x25D520.
    pub fn dispatch<H: LookAngleHandlers>(
        self,
        history: &mut LookEdgeHistory,
        lookahead: &mut u32,
        pairs: &mut LookAxisPairs,
        preparation: LookPreparation,
        movement_slow_gain: f32,
        handlers: &mut H,
    ) -> LookAngleRoute {
        let route = self.prepare_pairs(history, lookahead, pairs, preparation);
        match route {
            LookAngleRoute::Ignore => return route,
            LookAngleRoute::PairsOnly => {}
            LookAngleRoute::Player { integrate } => handlers.player(pairs, lookahead, integrate),
            LookAngleRoute::AttachedPlayer => handlers.attached_player(pairs, lookahead),
            LookAngleRoute::AttachedCamera => handlers.attached_camera(pairs, lookahead),
        }
        self.finish_movement(&mut pairs.movement, movement_slow_gain);
        route
    }

    /// Compose routing, history preprocessing and pair scaling in original order.
    /// The returned route is the explicit attachment point for the angle handler.
    /// Final movement scaling must run after that handler via `finish_movement`.
    pub fn prepare_pairs(
        self,
        history: &mut LookEdgeHistory,
        lookahead: &mut u32,
        pairs: &mut LookAxisPairs,
        preparation: LookPreparation,
    ) -> LookAngleRoute {
        let route = self.angle_route();
        if route == LookAngleRoute::Ignore {
            return route;
        }
        self.prepare_lookahead(lookahead);
        if self.preprocess() {
            pairs.look = if preparation.alternate_input {
                history.preprocess_alternate(
                    preparation.input,
                    preparation.profile,
                    preparation.alternate_dead_zone,
                    preparation.edge,
                )
            } else {
                history.preprocess_controller(
                    preparation.input,
                    preparation.profile,
                    preparation.pitch_dead_zone,
                    preparation.edge,
                )
            };
        }
        // The source advances lookahead from the pre-boundary pairs even when
        // the angle integration gate is closed (0x25D8FC).
        advance_lookahead(lookahead, pairs.look.y, pairs.movement.x, 0.75);
        // Routing is authoritative: callers cannot supply a conflicting boundary mode.
        LookBoundary {
            player_mode: self.player_mode,
            ..preparation.boundary
        }
        .apply(pairs);
        let turn_speed = preparation.profile.turn_speed + 0.5;
        pairs.look.x *= turn_speed;
        pairs.look.y *= turn_speed;
        route
    }
}

/// Original lookahead transition, before state-dependent pair scaling.
/// Movement x here is the source's first output word, not the shaper's y axis.
pub fn advance_lookahead(state: &mut u32, pitch: f32, forward: f32, threshold: f32) {
    match *state {
        0 if pitch != 0.0 && forward != 0.0 => *state = 1,
        0 if pitch == 0.0 && forward.abs() > threshold => *state = 2,
        1 if forward == 0.0 => *state = 0,
        2 if pitch != 0.0 => *state = 1,
        _ => {}
    }
}

/// Inline player angle destination, taking the already prepared guest-convention
/// pair. No profile preprocessing or lookahead transition occurs here.
#[derive(Clone, Copy, Debug)]
pub struct PlayerAngleConfig {
    pub integration: Integration,
    pub ramp_gain: f32,
    pub special_ramp_target: Option<f32>,
    pub alternate_input: bool,
    pub auto_lookahead: bool,
    pub lookahead_decay: f32,
    pub lookahead_settle: f32,
}

pub fn integrate_player_pair(
    state: &mut LookState,
    pair: Vec2,
    integrate: bool,
    config: PlayerAngleConfig,
) {
    let step = config.integration;
    if integrate {
        let mut target = config.special_ramp_target.unwrap_or(1.0);
        if config.alternate_input {
            state.ramp = 1.0;
            target = 1.0;
        }
        state.ramp += (target - state.ramp) * config.ramp_gain;
        state.yaw +=
            state.ramp * step.yaw_rate / step.step_rate * pair.x * step.time_scale * step.zoom;
        if state.lookahead == 2 && config.auto_lookahead {
            state.pitch -= state.pitch * step.time_scale * config.lookahead_decay;
            if state.pitch.abs() < config.lookahead_settle {
                state.lookahead = 1;
            }
        } else {
            let pitch_ramp = 1.0 - (1.0 - state.ramp) * 0.5;
            state.pitch += pitch_ramp * step.pitch_rate / step.step_rate
                * step.time_scale
                * step.zoom
                * pair.y;
        }
        state.yaw = wrap_positive(state.yaw);
    }
    // Even a closed angle gate clamps the pitch, without wrapping the yaw.
    state.pitch = state.pitch.clamp(-step.pitch_limit, step.pitch_limit);
}

/// Rust-owned attached camera angles (the source uses three scalar axes).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LookCameraState {
    pub pitch: f32,
    pub yaw: f32,
    pub roll: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct LookCameraConfig {
    pub integration: Integration,
    pub input_gain: f32,
    pub pitch_bounds: (f32, f32),
    pub yaw_bounds: (f32, f32),
    pub roll_bounds: (f32, f32),
}

/// Clamp finite-width limits, wrap full-period/reversed limits, and pin equal
/// limits to their shared value, as in 0x258A20 -> 0x153F90.
fn camera_bound(value: f32, bounds: (f32, f32)) -> f32 {
    let (low, high) = bounds;
    if low == high {
        low
    } else if high > low && (high - low).abs() < 360.0 {
        value.clamp(low, high)
    } else {
        wrap_signed(value)
    }
}
pub fn integrate_camera_pair(state: &mut LookCameraState, pair: Vec2, config: LookCameraConfig) {
    let step = config.integration;
    let x = pair.x * config.input_gain;
    let y = pair.y * config.input_gain;
    state.pitch -= (1.0 / step.step_rate) * step.pitch_rate * step.time_scale * y * step.zoom;
    state.yaw += (1.0 / step.step_rate) * step.yaw_rate * step.time_scale * x * step.zoom;
    state.pitch = camera_bound(state.pitch, config.pitch_bounds);
    state.yaw = camera_bound(state.yaw, config.yaw_bounds);
    // Roll has no input delta but is still constrained on every call.
    state.roll = camera_bound(state.roll, config.roll_bounds);
}

/// Concrete dispatcher handlers own the angle destinations; the caller commits
/// state at its compatibility boundary after the whole flow succeeds.
pub struct NativeLookAngles {
    pub player: LookState,
    pub player_config: PlayerAngleConfig,
    pub attached_bounds: AttachedBounds,
    pub camera: LookCameraState,
    pub camera_config: LookCameraConfig,
}
impl LookAngleHandlers for NativeLookAngles {
    fn player(&mut self, pairs: &mut LookAxisPairs, lookahead: &mut u32, integrate: bool) {
        self.player.lookahead = *lookahead;
        integrate_player_pair(&mut self.player, pairs.look, integrate, self.player_config);
        *lookahead = self.player.lookahead;
    }
    fn attached_player(&mut self, pairs: &mut LookAxisPairs, _lookahead: &mut u32) {
        integrate_attached(
            &mut self.player,
            pairs.look,
            self.player_config.integration,
            self.attached_bounds,
        );
    }
    fn attached_camera(&mut self, pairs: &mut LookAxisPairs, _lookahead: &mut u32) {
        integrate_camera_pair(&mut self.camera, pairs.look, self.camera_config);
    }
}

/// Owned pairs crossing the player-state scaling boundary (0x258910).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LookAxisPairs {
    pub movement: Vec2,
    pub look: Vec2,
}

/// Attached-object fields projected by the caller; no object pointers escape here.
#[derive(Clone, Copy, Debug)]
pub struct AttachedLookBoundary {
    pub active: bool,
    pub flags: u32,
}

/// Runtime data and state for the original boundary. Match fade is computed lazily
/// by the caller only in match state 8: (unit - elapsed / duration).
#[derive(Clone, Copy, Debug)]
pub struct LookBoundary {
    pub player_mode: u32,
    pub slow_look: bool,
    pub attached: AttachedLookBoundary,
    pub attached_active_gain: f32,
    pub attached_inactive_gain: f32,
    pub slow_gain: f32,
    pub match_fade: Option<f64>,
}
impl LookBoundary {
    pub fn apply(self, pairs: &mut LookAxisPairs) {
        let scale = |v: &mut Vec2, gain: f32| {
            v.x *= gain;
            v.y *= gain;
        };
        if self.player_mode == 0x80 {
            pairs.movement = Vec2::ZERO;
            if self.attached.active
                && self.attached.flags & 2 != 0
                && self.attached.flags & 0x400 == 0
            {
                pairs.look = Vec2::ZERO;
            } else {
                scale(
                    &mut pairs.look,
                    if self.attached.active {
                        self.attached_active_gain
                    } else {
                        self.attached_inactive_gain
                    },
                );
            }
        } else if !matches!(self.player_mode, 0x40 | 0x400 | 0x200) && self.slow_look {
            scale(&mut pairs.look, self.slow_gain);
        }
        if let Some(gain) = self.match_fade {
            // The original stores only after the x87 multiply, not before it.
            for pair in [&mut pairs.movement, &mut pairs.look] {
                pair.x = (gain * pair.x as f64) as f32;
                pair.y = (gain * pair.y as f64) as f32;
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Integration {
    pub zoom: f32,
    pub time_scale: f32,
    pub yaw_rate: f32,
    pub pitch_rate: f32,
    pub pitch_limit: f32,
    /// Move-slew row 1 (state +0x11C = 1), read from the original table at run time.
    /// `None` keeps that row with the reference path; row 0 is the shaper's measured slew.
    pub slew_row1: Option<SlewRow>,
    /// Frames per second the slew rates are divided by (original integer at 0x4E8CEC).
    pub step_rate: f32,
}
impl Default for Integration {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            time_scale: 1.0,
            yaw_rate: 160.0,
            pitch_rate: 68.0,
            pitch_limit: 60.0,
            slew_row1: None,
            step_rate: 60.0,
        }
    }
}

/// One row of the move-slew table of 0x25CE80: three interleaved two-row columns
/// (forward increase, forward decrease, strafe either way), units per second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SlewRow {
    pub forward_up: f32,
    pub forward_down: f32,
    pub strafe: f32,
}

/// Rate-limits the move vector (x strafe, y forward) towards `target`, as 0x25CE80 does
/// from 0x25D188: each axis moves at most `rate * time_scale / step_rate` from `previous`.
pub fn slew_movement(target: Vec2, previous: Vec2, row: SlewRow, config: Integration) -> Vec2 {
    let step = config.time_scale / config.step_rate;
    let limit =
        |to: f32, from: f32, up: f32, down: f32| to.min(from + up * step).max(from - down * step);
    Vec2::new(
        limit(target.x, previous.x, row.strafe, row.strafe),
        limit(target.y, previous.y, row.forward_up, row.forward_down),
    )
}

/// State 0x40 helper 0x25D270, with bounds supplied by the attached object's contract.
#[derive(Clone, Copy, Debug)]
pub struct AttachedBounds {
    pub yaw_origin: f32,
    pub yaw: Option<(f32, f32)>,
    pub pitch: Option<(f32, f32)>,
}
pub fn integrate_attached(
    state: &mut LookState,
    guest_pair: Vec2,
    config: Integration,
    bounds: AttachedBounds,
) {
    state.ramp += (1.0 - state.ramp) * 0.3;
    state.yaw += state.ramp * config.yaw_rate / config.step_rate
        * guest_pair.x
        * config.time_scale
        * config.zoom;
    state.pitch += state.ramp * config.pitch_rate / config.step_rate
        * guest_pair.y
        * config.time_scale
        * config.zoom;
    let signed = wrap_signed;
    let mut yaw = signed(state.yaw - bounds.yaw_origin);
    state.pitch = signed(state.pitch);
    let (low, high) = bounds.pitch.unwrap_or((-180.0, 180.0));
    state.pitch = state.pitch.clamp(low, high);
    if let Some((low, high)) = bounds.yaw {
        yaw = yaw.clamp(low, high);
    }
    state.yaw = signed(yaw + bounds.yaw_origin);
}
