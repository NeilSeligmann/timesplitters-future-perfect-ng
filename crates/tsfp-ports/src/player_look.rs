// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-ports/c2r/gameplay_player
// portreg: replaces=tsfp-ports/c2r/player_wired
//! T2194 native look flow; guest projection/commit is the hybrid data boundary.
//! Original controller flow admitted by whole-route Tier 2. docs/t-player-look.md.
//! Verification Tier 2 on observed original look routes; unobserved branches Tier 1 FABRICATED, not whole-mission proof.
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};
#[allow(dead_code)] // T2454: shared source, dead here is live in its own crate
#[path = "../../tsfp-input/src/look_native.rs"]
mod native;
use core::sync::atomic::{AtomicU8, Ordering};
use native::*;
pub const ROOTS: [u32; 1] = [0x25d520];
static SELECTION: AtomicU8 = AtomicU8::new(0);
unsafe extern "C" {
    fn getenv(name: *const core::ffi::c_char) -> *const core::ffi::c_char;
}
pub fn selected() -> bool {
    let mut value = SELECTION.load(Ordering::Relaxed);
    if value == 0 {
        let enabled = unsafe {
            let env = getenv(c"TSFP_NATIVE_LOOK".as_ptr());
            env.is_null() || env.read() == b'1' as core::ffi::c_char
        };
        value = if enabled { 2 } else { 1 };
        SELECTION.store(value, Ordering::Relaxed);
    }
    value == 2
}
fn optional_word<M: GuestMemory>(m: &M, base: u32, offset: u32) -> u32 {
    if base == 0 {
        0
    } else {
        m.read32(base + offset)
    }
}
fn optional_float<M: GuestMemory>(m: &M, base: u32, offset: u32) -> f32 {
    f32::from_bits(optional_word(m, base, offset))
}
fn float<M: GuestMemory>(m: &M, addr: u32) -> f32 {
    f32::from_bits(m.read32(addr))
}
fn pair<M: GuestMemory>(m: &M, addr: u32) -> Vec2 {
    Vec2::new(float(m, addr), float(m, addr + 4))
}
fn write_pair<M: GuestMemory>(m: &mut M, addr: u32, value: Vec2) {
    m.write32(addr, value.x.to_bits());
    m.write32(addr + 4, value.y.to_bits());
}
fn call_float<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
    va: u32,
    arg: Option<u32>,
) -> f64 {
    let saved = (r.esp, r.ebx, r.esi, r.edi, r.ebp);
    if let Some(arg) = arg {
        r.esp = r.esp.wrapping_sub(4);
        m.write32(r.esp, arg);
    }
    guest.call(va, r);
    let value = r.fpu.st(0);
    r.fpu.pop();
    (r.esp, r.ebx, r.esi, r.edi, r.ebp) = saved;
    value
}
/// Native logic changes owned typed state. Reads and writes here are solely the
/// explicit hybrid attachment to retained consumers, never synthetic input.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    enabled: bool,
    m: &mut M,
    guest: &mut C,
    r: &mut ExtRegisters,
) -> bool {
    if !enabled || va != ROOTS[0] {
        return false;
    }
    let m = &mut crate::player_wire::Wired(m);
    let player = m.read32(0x7b0c7c);
    let count_step = crate::raw_look::begin_native(m, player);
    let count_input = count_step.as_ref().is_some_and(|step| step.apply);
    let assists_disabled = crate::raw_look::assists_disabled(m, player);
    let profile = m.read32(player + 0x1500);
    let owner = m.read32(player + 0x14);
    let owner_ext = if owner == 0 {
        0
    } else {
        m.read32(owner + 0x7c)
    };
    let attached = if owner_ext == 0 {
        0
    } else {
        m.read32(owner_ext + 0xc14)
    };
    let attached_ext = if attached == 0 {
        0
    } else {
        m.read32(attached + 0x7c)
    };
    let camera = m.read32(0x7b0cc0);
    let scene = if camera == 0 {
        0
    } else {
        m.read32(camera + 0x26c)
    };
    let mode = m.read32(player + 0x114);
    let flags = m.read32(player + 0xc);
    let dispatch = LookDispatchState {
        player_mode: mode,
        flags,
        attached_kind: optional_word(m, attached_ext, 0x10),
        camera_scene_tag18: camera != 0 && scene != 0 && m.read32(scene) == 0x18,
    };
    if dispatch.angle_route() == LookAngleRoute::Ignore {
        return true;
    }
    let input_addr = m.read32(r.esp + 4);
    let move_addr = m.read32(r.esp + 8);
    let look_addr = m.read32(r.esp + 12);
    let mut history = LookEdgeHistory {
        charge: pair(m, player + 0xfdc),
    };
    let mut lookahead = m.read32(player + 0x108);
    let mut pairs = LookAxisPairs {
        movement: pair(m, move_addr),
        look: pair(m, look_addr),
    };
    let match_fade = if crate::state_wire::get_match_state(m) == 8 {
        let duration = call_float(m, guest, r, 0x65270, None);
        Some(float(m, 0x475c78) as f64 - float(m, 0x7de300) as f64 / duration)
    } else {
        None
    };
    let preparation = LookPreparation {
        input: if count_input {
            Vec2::ZERO
        } else {
            pair(m, input_addr)
        },
        alternate_input: m.read32(player + 0xf5c) != 0,
        profile: LookProfile {
            flags: m.read8(profile + 2)
                & if assists_disabled {
                    !crate::raw_look::ASSIST_BITS
                } else {
                    0xff
                },
            crosshair: m.read32(profile + 0x74),
            turn_speed: float(m, profile + 0x78),
        },
        pitch_dead_zone: if mode == 0x40 {
            float(m, 0x475ce8)
        } else {
            float(m, 0x475d24)
        },
        alternate_dead_zone: float(m, 0x475cd4),
        edge: LookEdgeConfig {
            threshold_positive: float(m, 0x47d594),
            threshold_negative: float(m, 0x49f2d4),
            charge_rate: float(m, 0x475d18),
            step_rate: m.read32(0x4e8cec) as i32 as f32,
            time_scale: float(m, 0x4e7948),
            minimum_negative_charge: float(m, 0x4760d0),
            direct_gain: float(m, 0x476040),
            history_gain: float(m, 0x475cd4),
        },
        boundary: LookBoundary {
            player_mode: mode,
            slow_look: flags & 0x10 != 0,
            attached: AttachedLookBoundary {
                active: optional_word(m, attached, 0x94) as u16 != 0,
                flags: optional_word(m, attached_ext, 8),
            },
            attached_active_gain: float(m, 0x475d4c),
            attached_inactive_gain: float(m, 0x478674),
            slow_gain: float(m, 0x475cc8),
            match_fade,
        },
    };
    let route = dispatch.prepare_pairs(&mut history, &mut lookahead, &mut pairs, preparation);
    let integrates = matches!(
        route,
        LookAngleRoute::Player { integrate: true }
            | LookAngleRoute::AttachedPlayer
            | LookAngleRoute::AttachedCamera
    );
    let zoom = if (integrates || count_input) && camera != 0 {
        let default = call_float(m, guest, r, 0xcb3b0, Some(camera));
        (float(m, camera + 0x384) as f64 / default) as f32
    } else {
        1.0
    };
    let integration = Integration {
        zoom,
        time_scale: float(m, 0x4e794c),
        yaw_rate: float(m, 0x7b0cb8),
        pitch_rate: float(m, 0x7b0cbc),
        pitch_limit: float(m, 0x7b0c80),
        step_rate: m.read32(0x4e8cec) as i32 as f32,
        slew_row1: None,
    };
    let state = LookState {
        yaw: float(m, player + 0x100),
        pitch: float(m, player + 0x104),
        ramp: float(m, player + 0xf80),
        lookahead,
        ..LookState::default()
    };
    let bound = |a, b| {
        let low = optional_float(m, attached_ext, a);
        let high = optional_float(m, attached_ext, b);
        if low == high { None } else { Some((low, high)) }
    };
    let mut angles = NativeLookAngles {
        player: state,
        player_config: PlayerAngleConfig {
            integration,
            ramp_gain: float(m, 0x475cdc),
            special_ramp_target: if m.read32(player + 0xf7c) != 0 {
                Some(float(m, 0x47cfe0))
            } else {
                None
            },
            alternate_input: preparation.alternate_input,
            auto_lookahead: preparation.profile.flags & 8 != 0,
            lookahead_decay: float(m, 0x475ce8),
            lookahead_settle: float(m, 0x475cd4),
        },
        attached_bounds: AttachedBounds {
            yaw_origin: optional_float(m, attached, 0x70),
            pitch: bound(0x34, 0x38),
            yaw: bound(0x3c, 0x40),
        },
        camera: LookCameraState {
            pitch: optional_float(m, attached_ext, 0x1c),
            yaw: optional_float(m, attached_ext, 0x24),
            roll: optional_float(m, attached_ext, 0x2c),
        },
        camera_config: LookCameraConfig {
            integration,
            input_gain: if optional_word(m, attached_ext, 8) & 0x1000000 != 0 {
                optional_float(m, attached_ext, 0x5c)
            } else {
                1.0
            },
            pitch_bounds: (
                optional_float(m, attached_ext, 0x34),
                optional_float(m, attached_ext, 0x38),
            ),
            yaw_bounds: (
                optional_float(m, attached_ext, 0x3c),
                optional_float(m, attached_ext, 0x40),
            ),
            roll_bounds: (
                optional_float(m, attached_ext, 0x44),
                optional_float(m, attached_ext, 0x48),
            ),
        },
    };
    match route {
        LookAngleRoute::Player { integrate } => {
            angles.player(&mut pairs, &mut lookahead, integrate)
        }
        LookAngleRoute::AttachedPlayer => angles.attached_player(&mut pairs, &mut lookahead),
        LookAngleRoute::AttachedCamera => angles.attached_camera(&mut pairs, &mut lookahead),
        _ => {}
    }
    if let Some(step) = count_step {
        let mut view = crate::raw_look::NativeViewAngles {
            yaw: angles.player.yaw,
            pitch: angles.player.pitch,
        };
        step.apply_angles(
            &mut view,
            crate::raw_look::NativeViewConstraints {
                zoom,
                pitch_limit: integration.pitch_limit,
            },
        );
        angles.player.yaw = view.yaw;
        angles.player.pitch = view.pitch;
    }
    dispatch.finish_movement(&mut pairs.movement, float(m, 0x475cc8));
    crate::raw_look::log_pairs(
        player,
        profile,
        (preparation.input.x, preparation.input.y),
        (pairs.movement.x, pairs.movement.y),
        (pairs.look.x, pairs.look.y),
        count_input,
    );
    write_pair(m, move_addr, pairs.movement);
    write_pair(m, look_addr, pairs.look);
    write_pair(m, player + 0xfdc, history.charge);
    m.write32(player + 0x108, lookahead);
    if matches!(
        route,
        LookAngleRoute::Player { .. } | LookAngleRoute::AttachedPlayer
    ) {
        for (offset, value) in [
            (0x100, angles.player.yaw),
            (0x104, angles.player.pitch),
            (0xf80, angles.player.ramp),
        ] {
            m.write32(player + offset, value.to_bits());
        }
    }
    if route == LookAngleRoute::AttachedCamera {
        for (offset, value) in [
            (0x1c, angles.camera.pitch),
            (0x24, angles.camera.yaw),
            (0x2c, angles.camera.roll),
        ] {
            m.write32(attached_ext + offset, value.to_bits());
        }
    }
    true
}
