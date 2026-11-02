// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    c2r::gameplay_player as oracle,
    memory::GuestMemory as OracleMemory,
};
#[derive(Default, Clone)]
struct Memory(Rc<RefCell<HashMap<u32, u8>>>);
impl OracleMemory for Memory {
    fn read8(&self, a: u32) -> u8 {
        *self.0.borrow().get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.borrow_mut().insert(a, v);
    }
}
struct Calls {
    object: bool,
}
struct GateCalls {
    object: bool,
    calls: u32,
}
impl GuestCall for GateCalls {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        assert_eq!(va, 0x1b6ae0);
        self.calls += 1;
        regs.eax = u32::from(self.object);
    }
}
impl GuestCall for Calls {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        match va {
            0x1b6ae0 => r.eax = u32::from(self.object),
            // ABI trampoline's double argument at ESP (CALL does not push a return address).
            _ => panic!("unexpected oracle call {va:x}"),
        }
    }
}
fn registers() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ebx: 0,
        ecx: 0,
        edx: 0,
        esi: 0,
        edi: 0,
        ebp: 0,
        esp: 0x9000,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0x37f,
        },
        xmm: [Xmm { lanes: [0; 4] }; 5],
        xmm_hi: [Xmm { lanes: [0; 4] }; 3],
    }
}
#[test]
fn gate_tier1_translated_root_4096_fabricated_states() {
    for seed in 0..4096u32 {
        let states = [2, 0x40, 0x8000, 0x80, 0x400, 0x10000];
        let state = LookState {
            state: states[seed as usize % states.len()],
            alternate_input: seed & 1 != 0,
            ..Default::default()
        };
        let flags = (seed >> 2) as u8;
        let context = TargetingContext {
            game_flags: if seed & 2 != 0 { 0x40 } else { 0 },
            mode: [2, 10, 11, 16, 0][seed as usize % 5],
            object_allows_targeting: seed & 32 != 0,
        };
        let mut m = Memory::default();
        let mut r = registers();
        m.write32(r.esp + 4, 0x10000);
        m.write32(0x10000 + 0x1500, 0x20000);
        m.write8(0x20002, flags);
        m.write32(0x10114, state.state);
        m.write32(0x10f5c, u32::from(state.alternate_input));
        // game_state::get_flags_low8 reads 0x6D60D8 (confirmed in the oracle accessor).
        m.write32(0x7de458, context.game_flags as u32);
        m.write32(0x7de470, context.mode);
        let mut oracle_calls = GateCalls {
            object: context.object_allows_targeting,
            calls: 0,
        };
        oracle::game_player_targeting_enabled_from_control_record_flags_and_state(
            &mut m,
            &mut oracle_calls,
            &mut r,
        );
        let effective = LookProfile {
            flags,
            crosshair: 2,
            turn_speed: 0.5,
        }
        .effective(AssistMask::PROFILE);
        assert_eq!(
            targeting_enabled(&state, effective, context),
            r.eax != 0,
            "seed {seed}"
        );
        assert!(!targeting_enabled(&state, AssistMask::NONE, context));
        let mut native_regs = registers();
        native_regs.ebx = 0x1234abcd;
        native_regs.esi = 0x5678abcd;
        let before = m.0.borrow().clone();
        let mut native_calls = GateCalls {
            object: context.object_allows_targeting,
            calls: 0,
        };
        assert!(tsfp_ports::player_look_gate::run(
            0xcff20,
            true,
            &mut m,
            &mut native_calls,
            &mut native_regs,
        ));
        assert_eq!(native_regs.eax, r.eax, "native gate seed {seed}");
        assert_eq!(
            native_calls.calls, oracle_calls.calls,
            "lazy boundary seed {seed}"
        );
        assert_eq!(
            (native_regs.esp, native_regs.ebx, native_regs.esi),
            (0x9000, 0x1234abcd, 0x5678abcd)
        );
        for (address, value) in before {
            if !(0x8ff4..0x9000).contains(&address) {
                assert_eq!(m.read8(address), value, "no gameplay writes seed {seed}");
            }
        }
    }
}
#[test]
fn raw_and_original_are_opt_in_and_use_typed_state() {
    let mut integration = LookIntegration::<OriginalLook>::default();
    let mut state = LookState {
        yaw: 1.0,
        pitch: 59.0,
        ramp: 1.0,
        state: 2,
        integration_allowed: true,
        ..Default::default()
    };
    let old = state;
    let profile = LookProfile {
        flags: 0xff,
        crosshair: 2,
        turn_speed: 0.5,
    };
    let delta = Look::Delta {
        yaw_deg: 4.0,
        pitch_deg: 4.0,
    };
    assert!(!integration.step(
        &mut state,
        delta,
        Vec2::ZERO,
        profile,
        AssistMask::NONE,
        Integration::default()
    ));
    assert_eq!(state, old);
    integration.enabled = true;
    assert!(integration.step(
        &mut state,
        delta,
        Vec2::ZERO,
        profile,
        AssistMask::NONE,
        Integration::default()
    ));
    assert_eq!((state.yaw, state.pitch), (357.0, 60.0));
    state.locked = true;
    integration.step(
        &mut state,
        delta,
        Vec2::ZERO,
        profile,
        AssistMask::NONE,
        Integration::default(),
    );
    assert_eq!((state.yaw, state.pitch), (357.0, 60.0));
    state.state = 0x80;
    let old = state;
    assert!(!integration.step(
        &mut state,
        delta,
        Vec2::ZERO,
        profile,
        AssistMask::NONE,
        Integration::default()
    ));
    assert_eq!(state, old);
}
#[test]
fn lookahead_retains_latch_and_has_profile_gate() {
    let mut state = LookState {
        pitch: 60.0,
        ramp: 1.0,
        move_previous: Vec2::new(0.0, 1.0),
        ..Default::default()
    };
    integrate_default(
        &mut state,
        Vec2::ZERO,
        false,
        AssistMask(AssistMask::AUTO_LOOKAHEAD),
        Integration::default(),
    );
    assert_eq!((state.lookahead, state.pitch), (2, 57.0));
    state.move_previous = Vec2::ZERO;
    integrate_default(
        &mut state,
        Vec2::ZERO,
        false,
        AssistMask::NONE,
        Integration::default(),
    );
    assert_eq!((state.lookahead, state.pitch), (2, 57.0));
}

struct LookCalls {
    memory: Memory,
    axes: [f32; 4],
}
impl GuestCall for LookCalls {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        match va {
            0xcf200 => {
                let axis = self.memory.read32(r.esp + 4);
                let idx = match axis {
                    0x11 => 0,
                    0x10 => 1,
                    0xe => 2,
                    0xf => 3,
                    _ => panic!("axis {axis}"),
                };
                r.fpu.push(self.axes[idx] as f64);
            }
            0x42920 => {} // record dead-zone writer, axis reader supplied above
            0x3c848c => {
                r.eax = r.fpu.st(0) as i32 as u32;
                r.fpu.pop();
            }
            0x3c8501 => {
                let bits = self.memory.read32(r.esp) as u64
                    | ((self.memory.read32(r.esp + 4) as u64) << 32);
                r.fpu.push(f64::from_bits(bits).floor());
            }
            0x258910 => {} // boundary scale identity for this slice's fabricated state
            _ => panic!("unexpected look call {va:x}"),
        }
    }
}
fn constants(m: &mut Memory) {
    for (addr, value) in [
        (0x475c78, 1.0f32),
        (0x475cdc, 0.3),
        (0x475cd4, 0.5),
        (0x475ce0, 2048.0 / std::f32::consts::TAU),
        (0x475c8c, -180.0),
        (0x475c84, 180.0),
        (0x475c88, 360.0),
        (0x475cb4, 1.0 / 360.0),
        (0x475d24, 0.1),
        (0x475ce8, 0.05),
        (0x475d18, 3.0),
        (0x476040, 0.9),
        (0x47d594, 0.95),
        (0x49f2d4, -0.95),
        (0x475d40, 0.75),
        (0x4e794c, 1.0),
        (0x4e7948, 1.0),
        (0x7b0cb8, 160.0),
        (0x7b0cbc, 68.0),
        (0x7b0c80, 60.0),
        (0x49f2a0, 5.0),
        (0x49f2a8, 5.0),
        (0x49f2b0, 8.0),
        (0x475cac, 0.0),
        (0x4760d0, 0.00001),
    ] {
        m.write32(addr, value.to_bits());
    }
    m.write32(0x4e8cec, 60);
    m.write32(0x7b0c7c, 0x10000);
    m.write32(0x10114, 2);
    m.write32(0x11500, 0x20000);
    m.write32(0x20078, 0.5f32.to_bits());
    for i in 0..2048 {
        m.write32(
            0x7a38a0 + i * 4,
            ((i as f64 * std::f64::consts::TAU / 2048.0).sin() as f32).to_bits(),
        );
    }
}
#[test]
fn shaping_tier1_translated_root_4096_byte_pairs() {
    use crate::shaping::axis_float;
    for seed in 0..4096u32 {
        let rx = (seed * 73) as u8;
        let ry = (seed * 37 + seed / 256) as u8;
        let mut m = Memory::default();
        constants(&mut m);
        let axes = [axis_float(rx, 0.25), axis_float(ry, 0.25), 0.0, 0.0];
        let mut calls = LookCalls {
            memory: m.clone(),
            axes,
        };
        let mut r = registers();
        m.write32(r.esp + 4, 0x30000);
        m.write32(r.esp + 8, 0x31000);
        oracle::game_player_7b0c7c_apply_input_pad_floats(&mut m, &mut calls, &mut r);
        let mut shaper = Shaper::new(Preset::original(), PlayerSettings::default());
        let got = shaper.original_look_bytes(rx, ry, AssistMask::NONE);
        for (addr, value) in [
            (0x30000, got.x / (160.0 / 60.0)),
            (0x30004, got.y / (68.0 / 60.0)),
        ] {
            let expected = f32::from_bits(m.read32(addr));
            assert!(
                (value - expected).abs() < 1e-5,
                "seed {seed}, {addr:x}: {value} vs {expected}"
            );
        }
    }
}
#[test]
fn attached_tier1_translated_root_2048_states() {
    for seed in 0..2048u32 {
        let mut m = Memory::default();
        constants(&mut m);
        let mut r = registers();
        r.ebx = 0x40000;
        r.edi = 0x30000;
        m.write32(r.ebx + 0x7c, 0x41000);
        let mut state = LookState {
            yaw: (seed % 300) as f32 - 150.0,
            pitch: (seed % 100) as f32 - 50.0,
            ramp: (seed % 101) as f32 / 100.0,
            ..Default::default()
        };
        let pair = Vec2::new(
            (seed % 21) as f32 / 10.0 - 1.0,
            (seed % 31) as f32 / 15.0 - 1.0,
        );
        for (addr, v) in [
            (0x10100, state.yaw),
            (0x10104, state.pitch),
            (0x10f80, state.ramp),
            (0x30000, pair.x),
            (0x30004, pair.y),
            (0x40070, 20.0),
            (0x41034, -40.0),
            (0x41038, 40.0),
            (0x4103c, -60.0),
            (0x41040, 60.0),
        ] {
            m.write32(addr, v.to_bits());
        }
        let mut calls = LookCalls {
            memory: m.clone(),
            axes: [0.0; 4],
        };
        oracle::game_player_7b0c7c_apply_look_input_yaw_pitch_zoom_scaled_clamped(
            &mut m, &mut calls, &mut r,
        );
        integrate_attached(
            &mut state,
            pair,
            Integration::default(),
            AttachedBounds {
                yaw_origin: 20.0,
                yaw: Some((-60.0, 60.0)),
                pitch: Some((-40.0, 40.0)),
            },
        );
        for (addr, v) in [
            (0x10100, state.yaw),
            (0x10104, state.pitch),
            (0x10f80, state.ramp),
        ] {
            let expected = f32::from_bits(m.read32(addr));
            assert!(
                (v - expected).abs() < 1e-4,
                "seed {seed} {addr:x}: {v} vs {expected}"
            );
        }
    }
}
#[test]
fn default_tier1_translated_tail_2048_states() {
    for seed in 0..2048u32 {
        let mut m = Memory::default();
        constants(&mut m);
        m.write32(0x1000c, 0x80); // default look integration eligibility
        let mut r = registers();
        // cdecl: input look, movement, shaped look output, third output.
        for (offset, value) in [(4, 0x30000), (8, 0x31000), (12, 0x32000), (16, 0x33000)] {
            m.write32(r.esp + offset, value);
        }
        let mut state = LookState {
            yaw: (seed % 360) as f32,
            pitch: (seed % 121) as f32 - 60.0,
            ramp: (seed % 101) as f32 / 100.0,
            lookahead: seed % 3,
            move_previous: Vec2::new(0.0, (seed % 5) as f32 / 4.0),
            ..Default::default()
        };
        let flags = (seed % 4) as u8;
        m.write8(0x20002, (flags & 1) | ((flags & 2) << 2));
        m.write32(0x31000, state.move_previous.y.to_bits());
        let pair = Vec2::new(
            (seed % 21) as f32 / 10.0 - 1.0,
            (seed % 19) as f32 / 10.0 - 0.9,
        );
        for (addr, v) in [
            (0x10100, state.yaw),
            (0x10104, state.pitch),
            (0x10f80, state.ramp),
            (0x30000, pair.x),
            (0x30004, pair.y),
        ] {
            m.write32(addr, v.to_bits());
        }
        m.write32(0x10108, state.lookahead);
        let mut calls = LookCalls {
            memory: m.clone(),
            axes: [0.0; 4],
        };
        oracle::game_player_camera_update_dispatch_by_state_flags(&mut m, &mut calls, &mut r);
        // Profile 0 / crosshair 0: pitch dead zone, no boost, inverse or lookahead.
        let pitch = if pair.y.abs() <= 0.1 {
            0.0
        } else {
            pair.y.signum() * (pair.y.abs() - 0.1) / 0.9
        };
        integrate_default(
            &mut state,
            Vec2::new(
                -pair.x * 160.0 / 60.0,
                pitch * (if flags & 1 != 0 { -1.0 } else { 1.0 }) * 68.0 / 60.0,
            ),
            false,
            AssistMask(if flags & 2 != 0 {
                AssistMask::AUTO_LOOKAHEAD
            } else {
                0
            }),
            Integration::default(),
        );
        for (addr, v) in [
            (0x10100, state.yaw),
            (0x10104, state.pitch),
            (0x10f80, state.ramp),
        ] {
            let expected = f32::from_bits(m.read32(addr));
            assert!(
                (v - expected).abs() < 1e-4,
                "seed {seed} {addr:x}: {v} vs {expected}"
            );
        }
    }
}

struct PlayerMemory(Memory);
impl tsfp_state::GuestMemory for PlayerMemory {
    fn read8(&self, address: u32) -> u8 {
        self.0.read8(address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.write8(address, value);
    }
}
#[test]
fn row_one_runtime_projection_reads_interleaved_columns_and_scales() {
    let mut memory = PlayerMemory(Memory::default());
    for (address, value) in [
        (0x49f2a4, 5.0f32.to_bits()),
        (0x49f2ac, 7.0f32.to_bits()),
        (0x49f2b4, 9.0f32.to_bits()),
        (0x4e794c, 2.0f32.to_bits()),
        (0x475c78, 0.5f32.to_bits()),
        (0x4e8cec, 50u32),
    ] {
        tsfp_state::GuestMemory::write32(&mut memory, address, value);
    }
    let config = Integration::default().with_guest_slew(&memory);
    assert_eq!(
        config.slew_row1,
        Some(SlewRow {
            forward_up: 10.0,
            forward_down: 14.0,
            strafe: 18.0,
        })
    );
    assert_eq!(config.time_scale, 0.5);
    assert_eq!(config.step_rate, 50.0);
}
#[test]
fn player_adapter_writes_owned_state_and_only_contract_words() {
    let mut player = PlayerSet::EMPTY;
    let mut memory = PlayerMemory(Memory::default());
    assert!(!LookState::default().write_player(&mut player, &mut memory));
    player.record_ptr = 0x10000;
    player.record[0x114 / 4] = 2;
    player.record[0xc / 4] = 0x80;
    let mut state = LookState::from_player(&player);
    assert!(state.integration_allowed);
    state.yaw = 13.0;
    state.pitch = -5.0;
    state.ramp = 0.3;
    state.lookahead = 2;
    state.move_previous = Vec2::new(0.4, 0.2);
    assert!(state.write_player(&mut player, &mut memory));
    assert_eq!(LookState::from_player(&player), state);
    assert_eq!(memory.0.read32(0x10100), 13.0f32.to_bits());
    assert_eq!(memory.0.0.borrow().len(), 24, "six words only");
    assert_eq!(player.record[0xc / 4], 0x80, "eligibility flags unchanged");
}

#[test]
fn player_attachment_falls_back_without_writing_and_commits_supported_step() {
    let mut player = PlayerSet::EMPTY;
    player.record_ptr = 0x10000;
    player.record[0x114 / 4] = 0x40;
    player.record[0xc / 4] = 0x80;
    player.record[0x100 / 4] = 10.0f32.to_bits();
    let mut memory = PlayerMemory(Memory::default());
    let mut integration = LookIntegration::<OriginalLook>::default();
    integration.enabled = true;
    let profile = LookProfile {
        flags: 0,
        crosshair: 0,
        turn_speed: 0.5,
    };
    let step = |integration: &mut LookIntegration<OriginalLook>,
                player: &mut PlayerSet,
                memory: &mut PlayerMemory| {
        integration.step_player(
            player,
            memory,
            LookStep {
                look: Look::Delta {
                    yaw_deg: 2.0,
                    pitch_deg: 1.0,
                },
                movement: Vec2::ZERO,
                profile,
                allowed: AssistMask::NONE,
                config: Integration::default(),
            },
        )
    };
    let before = player.record;
    assert!(!step(&mut integration, &mut player, &mut memory));
    assert_eq!(player.record, before);
    assert!(memory.0.0.borrow().is_empty());

    player.record[0x114 / 4] = 2;
    assert!(step(&mut integration, &mut player, &mut memory));
    assert_eq!(f32::from_bits(player.record[0x100 / 4]), 8.0);
    assert_eq!(f32::from_bits(player.record[0x104 / 4]), 1.0);
    assert_eq!(memory.0.read32(0x10100), 8.0f32.to_bits());

    player.record_ptr = 0;
    let before = player.record;
    assert!(!step(&mut integration, &mut player, &mut memory));
    assert_eq!(player.record, before);
}
#[test]
fn assist_override_disables_per_player_and_reaches_gate() {
    let profile = LookProfile {
        flags: 0x89,
        crosshair: 0,
        turn_speed: 0.5,
    };
    let all = AssistMask::PROFILE;
    let original = LookIntegration::<OriginalLook>::default();
    let none_disabled = LookIntegration::<AssistOverride>::default();
    let base = original.policy.assists(profile, all);
    assert_ne!(base.0, 0);
    assert_eq!(none_disabled.policy.assists(profile, all), base);
    let mut no_aim = LookIntegration::<AssistOverride>::default();
    no_aim.policy.disabled = AssistMask(AssistMask::AUTO_AIM);
    let reduced = no_aim.policy.assists(profile, all);
    assert_eq!(reduced.0, base.0 & !AssistMask::AUTO_AIM);
    let state = LookState {
        state: 0x40,
        ..Default::default()
    };
    let context = TargetingContext {
        game_flags: 0,
        mode: 0,
        object_allows_targeting: false,
    };
    assert!(original.targeting(&state, profile, all, context));
    assert!(none_disabled.targeting(&state, profile, all, context));
    assert!(!no_aim.targeting(&state, profile, all, context));
    // Disabling inverse look changes the shaped pitch sign only for that player.
    let mut normal = LookIntegration::<AssistOverride> {
        enabled: true,
        ..Default::default()
    };
    let mut uninverted = LookIntegration::<AssistOverride> {
        enabled: true,
        policy: AssistOverride {
            disabled: AssistMask(AssistMask::INVERSE_LOOK),
            ..Default::default()
        },
    };
    let start = LookState {
        state: 2,
        integration_allowed: true,
        ramp: 1.0,
        ..Default::default()
    };
    let stick = Look::Stick(Vec2::new(0.0, 1.0));
    let (mut a, mut b) = (start, start);
    for (integration, state) in [(&mut normal, &mut a), (&mut uninverted, &mut b)] {
        assert!(integration.step(
            state,
            stick,
            Vec2::ZERO,
            profile,
            all,
            Integration::default()
        ));
    }
    assert!(a.pitch != 0.0 && b.pitch != 0.0);
    assert_eq!(a.pitch.signum(), -b.pitch.signum());
}

/// Runs the translated 0x25CE80 root with slew row 1 and returns its move output (x strafe, y forward).
fn oracle_row1_move(axes: [f32; 4], previous: Vec2, row: SlewRow) -> Vec2 {
    oracle_row1_move_scaled(axes, previous, row, 1.0, 60)
}
fn oracle_row1_move_scaled(
    axes: [f32; 4],
    previous: Vec2,
    row: SlewRow,
    frame_scale: f32,
    step_rate: u32,
) -> Vec2 {
    let mut m = Memory::default();
    constants(&mut m);
    m.write32(0x4e794c, frame_scale.to_bits());
    m.write32(0x4e8cec, step_rate);
    m.write32(0x1011c, 1);
    for (addr, value) in [
        (0x49f2a4, row.forward_up),
        (0x49f2ac, row.forward_down),
        (0x49f2b4, row.strafe),
        (0x10fc4, previous.y),
        (0x10fc8, previous.x),
    ] {
        m.write32(addr, value.to_bits());
    }
    let mut calls = LookCalls {
        memory: m.clone(),
        axes,
    };
    let mut r = registers();
    m.write32(r.esp + 4, 0x30000);
    m.write32(r.esp + 8, 0x31000);
    oracle::game_player_7b0c7c_apply_input_pad_floats(&mut m, &mut calls, &mut r);
    Vec2::new(
        f32::from_bits(m.read32(0x31004)),
        f32::from_bits(m.read32(0x31000)),
    )
}

#[test]
fn slew_row1_tier1_translated_root_2048_states() {
    use crate::shaping::{axis_float, poll_byte};
    let open = SlewRow {
        forward_up: 1e9,
        forward_down: 1e9,
        strafe: 1e9,
    };
    let unit = |seed: u32, k: u32| ((seed.wrapping_mul(k) >> 3) % 2001) as f32 / 1000.0 - 1.0;
    for seed in 0..2048u32 {
        let stick = Vec2::new(unit(seed, 2246822519), unit(seed, 3266489917));
        let (mx, my) = (poll_byte(stick.x), poll_byte(stick.y));
        // Axis 0xE is forward, 0xF strafe.
        let axes = [0.0, 0.0, axis_float(my, 0.25), axis_float(mx, 0.25)];
        let target = oracle_row1_move(axes, Vec2::ZERO, open);
        // Fabricated rates and previous move, not table bytes.
        let row = SlewRow {
            forward_up: 1.0 + (seed % 13) as f32,
            forward_down: 1.0 + (seed % 7) as f32 * 2.0,
            strafe: 1.0 + (seed % 11) as f32,
        };
        let previous = Vec2::new(unit(seed, 2654435761), unit(seed, 40503));
        let expected = oracle_row1_move(axes, previous, row);
        let got = slew_movement(target, previous, row, Integration::default());
        assert!(
            (got.x - expected.x).abs() < 1e-6 && (got.y - expected.y).abs() < 1e-6,
            "seed {seed}: {got:?} vs {expected:?}"
        );
        // The policy's unslewed target matches the oracle's pre-slew move.
        let mut policy = OriginalLook::default();
        assert_eq!(policy.movement_target(stick), Some(target), "seed {seed}");
    }
}

#[test]
fn slew_rows_dispatch_rejects_unknown_rows_and_slews_row1() {
    let profile = LookProfile {
        flags: 0,
        crosshair: 0,
        turn_speed: 0.5,
    };
    let base = LookState {
        state: 2,
        integration_allowed: true,
        slew_row: 1,
        move_previous: Vec2::new(0.5, -0.5),
        ..LookState::default()
    };
    let row = SlewRow {
        forward_up: 6.0,
        forward_down: 12.0,
        strafe: 3.0,
    };
    let mut look = LookIntegration::<OriginalLook> {
        enabled: true,
        ..Default::default()
    };
    let stick = Look::Stick(Vec2::ZERO);
    let full = Vec2::new(-1.0, 1.0);
    let step = |look: &mut LookIntegration, state: &mut LookState, config| {
        look.step(state, stick, full, profile, AssistMask::NONE, config)
    };
    // Row 1 without run-time table data, and rows past the two-row table, stay with the oracle.
    let mut state = base;
    assert!(!step(&mut look, &mut state, Integration::default()));
    assert_eq!(state, base);
    state.slew_row = 2;
    let config = Integration {
        slew_row1: Some(row),
        ..Integration::default()
    };
    assert!(!step(&mut look, &mut state, config));
    // Row 1 limits strafe symmetrically and forward by the increase rate.
    let mut state = base;
    assert!(step(&mut look, &mut state, config));
    assert!((state.move_previous.x - (0.5 - 3.0 / 60.0)).abs() < 1e-6);
    assert!((state.move_previous.y - (-0.5 + 6.0 / 60.0)).abs() < 1e-6);
    // Decrease uses its own column.
    let mut state = LookState {
        move_previous: Vec2::new(0.0, 0.5),
        ..base
    };
    assert!(look.step(
        &mut state,
        stick,
        Vec2::new(0.0, -1.0),
        profile,
        AssistMask::NONE,
        config
    ));
    assert!((state.move_previous.y - (0.5 - 12.0 / 60.0)).abs() < 1e-6);
}

#[test]
fn t2174_original_lookahead_matches_measured_hybrid_decay() {
    let data = include_str!("../../../docs/data/t2174-lookahead-decay.csv");
    let mut rows = 0;
    for line in data.lines().skip(1) {
        let (before, after) = line.split_once(',').unwrap();
        let mut state = LookState {
            pitch: before.parse().unwrap(),
            lookahead: 2,
            move_previous: Vec2::new(0.0, 1.0),
            ..Default::default()
        };
        integrate_default(
            &mut state,
            Vec2::default(),
            false,
            AssistMask(AssistMask::AUTO_LOOKAHEAD),
            Integration::default(),
        );
        assert!(
            (state.pitch - after.parse::<f32>().unwrap()).abs() < 3e-6,
            "{line}"
        );
        rows += 1;
    }
    assert!(rows >= 80);
}

#[test]
fn native_mouse_view_does_not_advance_fixed_step_state() {
    let integration = LookIntegration {
        enabled: true,
        policy: OriginalLook::default(),
    };
    let original = LookState {
        yaw: 1.0,
        pitch: 4.0,
        ramp: 0.25,
        lookahead: 2,
        move_previous: Vec2::new(0.3, 0.8),
        state: 2,
        integration_allowed: true,
        slew_row: 1,
        ..Default::default()
    };
    for updates in [1, 2, 4, 8] {
        let mut state = original;
        for _ in 0..updates {
            assert!(integration.mouse_view(
                &mut state,
                Vec2::new(8.0 / updates as f32, 2.0 / updates as f32),
                ViewConstraints::default()
            ));
        }
        assert_eq!((state.yaw, state.pitch), (353.0, 6.0));
        let mut expected = original;
        expected.yaw = 353.0;
        expected.pitch = 6.0;
        assert_eq!(state, expected, "display updates cannot advance simulation");
    }
    let mut locked = original;
    locked.locked = true;
    let before = locked;
    assert!(!integration.mouse_view(&mut locked, Vec2::new(1.0, 1.0), ViewConstraints::default()));
    assert_eq!(locked, before);
}

#[test]
fn raw_count_sensitivity_is_independent_of_simulation_time_scale() {
    for time_scale in [0.0, 0.25, 1.0, 2.0] {
        let mut state = LookState {
            yaw: 10.0,
            pitch: 5.0,
            ramp: 0.5,
            lookahead: 2,
            ..Default::default()
        };
        integrate_default(
            &mut state,
            Vec2::new(4.0, 2.0),
            true,
            AssistMask::PROFILE,
            Integration {
                time_scale,
                zoom: 0.5,
                ..Default::default()
            },
        );
        assert_eq!((state.yaw, state.pitch), (8.0, 6.0));
        assert_eq!(state.lookahead, 2);
        assert_eq!(
            state.ramp, 0.65,
            "fixed-step entry still advances ramp once"
        );
    }
}

#[test]
fn native_view_constraints_reject_invalid_updates_without_partial_commit() {
    let initial = ViewAngles {
        yaw: 2.0,
        pitch: 59.0,
    };
    let mut angles = initial;
    assert!(angles.apply_mouse(Vec2::new(4.0, 20.0), ViewConstraints::default()));
    assert_eq!(
        angles,
        ViewAngles {
            yaw: 358.0,
            pitch: 60.0
        }
    );
    for (degrees, constraints) in [
        (Vec2::new(f32::NAN, 1.0), ViewConstraints::default()),
        (Vec2::new(1.0, f32::INFINITY), ViewConstraints::default()),
        (
            Vec2::new(1.0, 1.0),
            ViewConstraints {
                zoom: -1.0,
                ..Default::default()
            },
        ),
        (
            Vec2::new(1.0, 1.0),
            ViewConstraints {
                pitch_limit: -1.0,
                ..Default::default()
            },
        ),
        (
            Vec2::new(f32::MAX, f32::MAX),
            ViewConstraints {
                zoom: 2.0,
                ..Default::default()
            },
        ),
    ] {
        let mut angles = initial;
        assert!(!angles.apply_mouse(degrees, constraints));
        assert_eq!(angles, initial);
    }
}

#[test]
fn row_one_runtime_projection_tier1_scaled_original_512_states() {
    let row = SlewRow {
        forward_up: 5.0,
        forward_down: 7.0,
        strafe: 9.0,
    };
    let open = SlewRow {
        forward_up: 1e9,
        forward_down: 1e9,
        strafe: 1e9,
    };
    for seed in 0..512u32 {
        let frame_scale = [0.25f32, 0.5, 1.0, 2.0][seed as usize % 4];
        let step_rate = [30u32, 60, 120][seed as usize % 3];
        let axis = |salt: u32| ((seed.wrapping_mul(salt) >> 5) % 2001) as f32 / 1000.0 - 1.0;
        let axes = [0.0, 0.0, axis(2246822519), axis(3266489917)];
        let previous = Vec2::new(axis(2654435761), axis(40503));
        let target = oracle_row1_move_scaled(axes, Vec2::ZERO, open, frame_scale, step_rate);
        let expected = oracle_row1_move_scaled(axes, previous, row, frame_scale, step_rate);
        let mut memory = PlayerMemory(Memory::default());
        for (address, value) in [
            (0x49f2a4, row.forward_up.to_bits()),
            (0x49f2ac, row.forward_down.to_bits()),
            (0x49f2b4, row.strafe.to_bits()),
            (0x4e794c, frame_scale.to_bits()),
            (0x475c78, 1.0f32.to_bits()),
            (0x4e8cec, step_rate),
        ] {
            tsfp_state::GuestMemory::write32(&mut memory, address, value);
        }
        let config = Integration::default().with_guest_slew(&memory);
        let got = slew_movement(target, previous, config.slew_row1.unwrap(), config);
        assert!(
            (got.x - expected.x).abs() < 1e-6 && (got.y - expected.y).abs() < 1e-6,
            "seed {seed}, frame_scale {frame_scale}, fps {step_rate}: {got:?} != {expected:?}"
        );
    }
}

#[test]
fn edge_history_tier1_translated_dispatch_4096_states() {
    for seed in 0..4096u32 {
        let mut m = Memory::default();
        constants(&mut m);
        m.write32(0x1000c, 0x80);
        m.write32(0x20074, 2); // original crosshair edge-history branch
        let fps = [30, 60, 120][seed as usize % 3];
        let scale = [0.25f32, 0.5, 1.0, 2.0][seed as usize % 4];
        let minimum = [-1.0f32, -0.5, 0.00001][seed as usize % 3];
        m.write32(0x4760d0, minimum.to_bits());
        m.write32(0x4e8cec, fps);
        m.write32(0x4e7948, scale.to_bits());
        let input = Vec2::new(
            (seed % 23) as f32 / 10.0 - 1.1,
            (seed % 29) as f32 / 10.0 - 1.4,
        );
        let mut history = LookEdgeHistory {
            charge: Vec2::new(
                (seed % 25) as f32 / 10.0 - 1.2,
                (seed % 31) as f32 / 10.0 - 1.5,
            ),
        };
        let mut r = registers();
        for (offset, value) in [(4, 0x30000), (8, 0x31000), (12, 0x32000), (16, 0x33000)] {
            m.write32(r.esp + offset, value);
        }
        for (addr, value) in [
            (0x30000, input.x),
            (0x30004, input.y),
            (0x10fdc, history.charge.x),
            (0x10fe0, history.charge.y),
        ] {
            m.write32(addr, value.to_bits());
        }
        let mut calls = LookCalls {
            memory: m.clone(),
            axes: [0.0; 4],
        };
        oracle::game_player_camera_update_dispatch_by_state_flags(&mut m, &mut calls, &mut r);
        let output = history.shape(
            input,
            LookEdgeConfig {
                threshold_positive: 0.95,
                threshold_negative: -0.95,
                charge_rate: 3.0,
                step_rate: fps as f32,
                time_scale: scale,
                minimum_negative_charge: minimum,
                direct_gain: 0.9,
                history_gain: 0.5,
            },
        );
        for (addr, value) in [
            (0x10fdc, history.charge.x),
            (0x10fe0, history.charge.y),
            (0x32000, output.x),
            (0x32004, output.y),
        ] {
            let expected = f32::from_bits(m.read32(addr));
            assert!(
                (value - expected).abs() < 1e-6,
                "seed {seed} {addr:x}: {value} vs {expected}"
            );
        }
    }
}

#[test]
fn controller_preprocess_tier1_translated_dispatch_4096_states() {
    for seed in 0..4096u32 {
        let mut m = Memory::default();
        constants(&mut m);
        m.write32(0x1000c, 0x80);
        let profile = LookProfile {
            flags: (seed % 2) as u8,
            crosshair: (seed / 2) % 3,
            turn_speed: 0.5,
        };
        m.write8(0x20002, profile.flags);
        m.write32(0x20074, profile.crosshair);
        let input = Vec2::new(
            (seed % 23) as f32 / 10.0 - 1.1,
            (seed % 29) as f32 / 10.0 - 1.4,
        );
        let mut history = LookEdgeHistory {
            charge: Vec2::new(
                (seed % 25) as f32 / 10.0 - 1.2,
                (seed % 31) as f32 / 10.0 - 1.5,
            ),
        };
        let mut r = registers();
        for (offset, value) in [(4, 0x30000), (8, 0x31000), (12, 0x32000), (16, 0x33000)] {
            m.write32(r.esp + offset, value);
        }
        for (addr, value) in [
            (0x30000, input.x),
            (0x30004, input.y),
            (0x10fdc, history.charge.x),
            (0x10fe0, history.charge.y),
        ] {
            m.write32(addr, value.to_bits());
        }
        let mut calls = LookCalls {
            memory: m.clone(),
            axes: [0.0; 4],
        };
        oracle::game_player_camera_update_dispatch_by_state_flags(&mut m, &mut calls, &mut r);
        let output = history.preprocess_controller(
            input,
            profile,
            0.1,
            LookEdgeConfig {
                threshold_positive: 0.95,
                threshold_negative: -0.95,
                charge_rate: 3.0,
                step_rate: 60.0,
                time_scale: 1.0,
                minimum_negative_charge: 0.00001,
                direct_gain: 0.9,
                history_gain: 0.5,
            },
        );
        for (addr, value) in [
            (0x10fdc, history.charge.x),
            (0x10fe0, history.charge.y),
            (0x32000, output.x),
            (0x32004, output.y),
        ] {
            let expected = f32::from_bits(m.read32(addr));
            assert!(
                (value - expected).abs() < 1e-6,
                "seed {seed} crosshair {} flags {} {addr:x}: {value} vs {expected}",
                profile.crosshair,
                profile.flags
            );
        }
    }
}

#[test]
fn alternate_preprocess_tier1_translated_dispatch_4096_states() {
    for seed in 0..4096u32 {
        let mut m = Memory::default();
        constants(&mut m);
        m.write32(0x1000c, 0x80);
        m.write32(0x10f5c, 1);
        let dead_zone = [0.0f32, 0.1, 0.25, 0.5][(seed / 6) as usize % 4];
        m.write32(0x475cd4, dead_zone.to_bits());
        // Keep Turn Speed multiplier at one while varying its shared runtime constant.
        m.write32(0x20078, (1.0 - dead_zone).to_bits());
        let profile = LookProfile {
            flags: (seed % 2) as u8,
            crosshair: (seed / 2) % 3,
            turn_speed: 0.5,
        };
        m.write8(0x20002, profile.flags);
        m.write32(0x20074, profile.crosshair);
        let input = Vec2::new(
            (seed % 23) as f32 / 10.0 - 1.1,
            (seed % 29) as f32 / 10.0 - 1.4,
        );
        let mut history = LookEdgeHistory {
            charge: Vec2::new(
                (seed % 25) as f32 / 10.0 - 1.2,
                (seed % 31) as f32 / 10.0 - 1.5,
            ),
        };
        let mut r = registers();
        for (offset, value) in [(4, 0x30000), (8, 0x31000), (12, 0x32000), (16, 0x33000)] {
            m.write32(r.esp + offset, value);
        }
        for (addr, value) in [
            (0x30000, input.x),
            (0x30004, input.y),
            (0x10fdc, history.charge.x),
            (0x10fe0, history.charge.y),
        ] {
            m.write32(addr, value.to_bits());
        }
        let mut calls = LookCalls {
            memory: m.clone(),
            axes: [0.0; 4],
        };
        oracle::game_player_camera_update_dispatch_by_state_flags(&mut m, &mut calls, &mut r);
        let output = history.preprocess_alternate(
            input,
            profile,
            dead_zone,
            LookEdgeConfig {
                threshold_positive: 0.95,
                threshold_negative: -0.95,
                charge_rate: 3.0,
                step_rate: 60.0,
                time_scale: 1.0,
                minimum_negative_charge: 0.00001,
                direct_gain: 0.9,
                history_gain: dead_zone,
            },
        );
        for (addr, value) in [
            (0x10fdc, history.charge.x),
            (0x10fe0, history.charge.y),
            (0x32000, output.x),
            (0x32004, output.y),
        ] {
            let expected = f32::from_bits(m.read32(addr));
            assert!(
                (value - expected).abs() < 1e-6,
                "seed {seed} crosshair {} flags {} {addr:x}: {value} vs {expected}",
                profile.crosshair,
                profile.flags
            );
        }
    }
}

#[test]
fn boundary_tier1_translated_root_4096_states() {
    struct DurationCall {
        duration: f64,
        count: u32,
    }
    impl GuestCall for DurationCall {
        fn call(&mut self, va: u32, r: &mut ExtRegisters) {
            assert_eq!(va, 0x65270);
            self.count += 1;
            r.fpu.push(self.duration);
        }
    }
    for seed in 0..4096u32 {
        let mut m = Memory::default();
        constants(&mut m);
        let mode = [2, 0x80, 0x40, 0x400, 0x200, 0x20, 0x100, 0][(seed % 8) as usize];
        let active = seed & 8 != 0;
        let flags = [0, 2, 0x400, 0x402][((seed / 16) % 4) as usize];
        let slow = seed & 64 != 0;
        let fade = seed & 128 != 0;
        let elapsed = (seed % 23) as f32 * 0.25;
        let duration = 1.0 + (seed % 11) as f64 * 0.5;
        m.write32(0x10114, mode);
        m.write32(0x1000c, if slow { 0x10 } else { 0 });
        m.write32(0x10014, 0x21000);
        m.write32(0x2107c, 0x22000);
        m.write32(0x22c14, 0x23000);
        m.write32(0x2307c, 0x24000);
        m.write32(0x23094, u32::from(active));
        m.write32(0x24008, flags);
        m.write32(0x475d4c, 0.35f32.to_bits());
        m.write32(0x478674, 0.8f32.to_bits());
        m.write32(0x475cc8, 0.6f32.to_bits());
        m.write32(0x7de30c, if fade { 8 } else { 2 });
        m.write32(0x7de300, elapsed.to_bits());
        let mut pairs = LookAxisPairs {
            movement: Vec2::new(
                (seed % 31) as f32 / 15.0 - 1.0,
                (seed % 17) as f32 / 8.0 - 1.0,
            ),
            look: Vec2::new(
                (seed % 19) as f32 / 9.0 - 1.0,
                (seed % 29) as f32 / 14.0 - 1.0,
            ),
        };
        for (a, v) in [
            (0x30000, pairs.movement.x),
            (0x30004, pairs.movement.y),
            (0x31000, pairs.look.x),
            (0x31004, pairs.look.y),
        ] {
            m.write32(a, v.to_bits());
        }
        let mut r = registers();
        let old_esp = r.esp;
        m.write32(r.esp + 4, 0x30000);
        m.write32(r.esp + 8, 0x31000);
        let mut calls = DurationCall { duration, count: 0 };
        oracle::game_player_scale_two_pad_axis_pairs_by_state_0x114_and_flags(
            &mut m, &mut calls, &mut r,
        );
        LookBoundary {
            player_mode: mode,
            slow_look: slow,
            attached: AttachedLookBoundary { active, flags },
            attached_active_gain: 0.35,
            attached_inactive_gain: 0.8,
            slow_gain: 0.6,
            match_fade: fade.then_some(1.0 - elapsed as f64 / duration),
        }
        .apply(&mut pairs);
        for (a, got) in [
            (0x30000, pairs.movement.x),
            (0x30004, pairs.movement.y),
            (0x31000, pairs.look.x),
            (0x31004, pairs.look.y),
        ] {
            let expected = f32::from_bits(m.read32(a));
            assert!(
                (got - expected).abs() < 1e-6,
                "seed {seed}, {a:x}: {got} != {expected}"
            );
        }
        assert_eq!(calls.count, u32::from(fade));
        assert_eq!(r.esp, old_esp);
    }
}

#[test]
fn dispatch_routes_tier1_translated_root_4096_states() {
    struct DispatchCalls {
        memory: Memory,
        calls: Vec<u32>,
        pair_at_boundary: Option<Vec2>,
    }
    impl GuestCall for DispatchCalls {
        fn call(&mut self, va: u32, r: &mut ExtRegisters) {
            self.calls.push(va);
            match va {
                0x258910 => {
                    let pair = self.memory.read32(r.esp + 4);
                    self.pair_at_boundary = Some(Vec2::new(
                        f32::from_bits(self.memory.read32(pair)),
                        f32::from_bits(self.memory.read32(pair + 4)),
                    ));
                }
                0xcb3b0 => r.fpu.push(1.0),
                0x258a20 | 0x25d270 => {}
                _ => panic!("unexpected dispatcher call {va:x}"),
            }
        }
    }
    for seed in 0..4096u32 {
        let modes = [2, 0x20, 0x40, 0x80, 0x100, 0x200, 0x400, 0x8000];
        let dispatch = LookDispatchState {
            player_mode: modes[(seed % 8) as usize],
            flags: ((seed / 8) % 4) * 0x80 | ((seed / 32) % 2) * 0x10,
            attached_kind: [0, 1, 9, 2][((seed / 64) % 4) as usize],
            camera_scene_tag18: seed / 256 % 2 != 0 && seed / 512 % 2 != 0 && seed / 1024 % 2 != 0,
        };
        let mut m = Memory::default();
        constants(&mut m);
        for (addr, value) in [
            (0x1000c, dispatch.flags),
            (0x10114, dispatch.player_mode),
            (0x10108, 3),
            (0x10014, 0x40000),
            (0x4007c, 0x41000),
            (0x41c14, 0x42000),
            (0x4207c, 0x43000),
            (0x43010, dispatch.attached_kind),
            (0x7b0cc0, if seed / 512 % 2 != 0 { 0x50000 } else { 0 }),
            (0x5026c, if seed / 1024 % 2 != 0 { 0x51000 } else { 0 }),
            (0x51000, if dispatch.camera_scene_tag18 { 0x18 } else { 0 }),
        ] {
            m.write32(addr, value);
        }
        for (addr, value) in [
            (0x50384, 1.0f32),
            (0x10f80, 0.2),
            (0x30000, 0.4),
            (0x30004, 0.6),
            (0x31000, 0.5),
            (0x31004, -0.7),
            (0x32000, -0.2),
            (0x32004, 0.3),
            (0x475cc8, 0.25),
        ] {
            m.write32(addr, value.to_bits());
        }
        let mut r = registers();
        r.ebx = 0x1234;
        r.ebp = 0x2345;
        r.esi = 0x3456;
        r.edi = 0x4567;
        for (offset, value) in [(4, 0x30000), (8, 0x31000), (12, 0x32000), (16, 0x33000)] {
            m.write32(r.esp + offset, value);
        }
        let mut calls = DispatchCalls {
            memory: m.clone(),
            calls: Vec::new(),
            pair_at_boundary: None,
        };
        oracle::game_player_camera_update_dispatch_by_state_flags(&mut m, &mut calls, &mut r);
        let route = dispatch.angle_route();
        let expected_calls: &[u32] = match route {
            LookAngleRoute::Ignore => &[],
            LookAngleRoute::AttachedPlayer => &[0x258910, 0x25d270],
            LookAngleRoute::AttachedCamera => &[0x258910, 0x258a20],
            LookAngleRoute::Player { integrate: true } if seed / 512 % 2 != 0 => {
                &[0x258910, 0xcb3b0]
            }
            _ => &[0x258910],
        };
        assert_eq!(calls.calls, expected_calls, "seed {seed} {dispatch:?}");
        if route != LookAngleRoute::Ignore {
            let expected = if dispatch.preprocess() {
                Vec2::new(
                    0.4,
                    (0.6 - if dispatch.player_mode == 0x40 {
                        0.05
                    } else {
                        0.1
                    }) / (1.0
                        - if dispatch.player_mode == 0x40 {
                            0.05
                        } else {
                            0.1
                        }),
                )
            } else {
                Vec2::new(-0.2, 0.3)
            };
            let actual = calls.pair_at_boundary.unwrap();
            assert!(
                (actual.x - expected.x).abs() < 1e-6 && (actual.y - expected.y).abs() < 1e-6,
                "seed {seed} preprocess {actual:?} vs {expected:?}"
            );
        }
        let mut lookahead = 3;
        dispatch.prepare_lookahead(&mut lookahead);
        assert_eq!(m.read32(0x10108), lookahead, "seed {seed} lookahead");
        let mut movement = Vec2::new(0.5, -0.7);
        dispatch.finish_movement(&mut movement, 0.25);
        for (addr, value) in [(0x31000, movement.x), (0x31004, movement.y)] {
            assert_eq!(
                m.read32(addr),
                value.to_bits(),
                "seed {seed} movement {addr:x}"
            );
        }
        let expected_ramp = if matches!(route, LookAngleRoute::Player { integrate: true }) {
            0.2f32 + (1.0 - 0.2) * 0.3
        } else {
            0.2
        };
        assert_eq!(
            m.read32(0x10f80),
            expected_ramp.to_bits(),
            "seed {seed} integration gate"
        );
        assert_eq!(r.esp, 0x9000, "seed {seed} stack");
        assert_eq!(
            (r.ebx, r.ebp, r.esi, r.edi),
            (0x1234, 0x2345, 0x3456, 0x4567),
            "seed {seed} saved registers"
        );
        if route == LookAngleRoute::Ignore {
            assert_eq!(m.read32(0x32000), (-0.2f32).to_bits());
            assert_eq!(m.read32(0x32004), 0.3f32.to_bits());
        }
    }
}

#[test]
fn composed_preparation_preserves_early_return_and_orders_boundary() {
    let preparation = LookPreparation {
        input: Vec2::new(0.8, 0.6),
        alternate_input: false,
        profile: LookProfile {
            flags: 1,
            crosshair: 0,
            turn_speed: 1.0,
        },
        pitch_dead_zone: 0.2,
        alternate_dead_zone: 0.4,
        edge: LookEdgeConfig {
            threshold_positive: 0.95,
            threshold_negative: -0.95,
            charge_rate: 3.0,
            step_rate: 60.0,
            time_scale: 1.0,
            minimum_negative_charge: -1.0,
            direct_gain: 0.9,
            history_gain: 0.5,
        },
        boundary: LookBoundary {
            player_mode: 0x80,
            slow_look: true,
            attached: AttachedLookBoundary {
                active: false,
                flags: 0,
            },
            attached_active_gain: 0.3,
            attached_inactive_gain: 0.4,
            slow_gain: 0.5,
            match_fade: Some(0.5),
        },
    };
    let initial = LookAxisPairs {
        movement: Vec2::new(0.6, 0.4),
        look: Vec2::new(0.3, 0.2),
    };
    let mut pairs = initial;
    let mut history = LookEdgeHistory {
        charge: Vec2::new(0.7, -0.8),
    };
    let initial_history = history;
    let mut lookahead = 3;
    let mut dispatch = LookDispatchState {
        player_mode: 0x80,
        flags: 0x80,
        attached_kind: 1,
        camera_scene_tag18: false,
    };
    assert_eq!(
        dispatch.prepare_pairs(&mut history, &mut lookahead, &mut pairs, preparation),
        LookAngleRoute::Ignore
    );
    assert_eq!(pairs, initial);
    assert_eq!(history, initial_history);
    assert_eq!(lookahead, 3);
    dispatch.player_mode = 2;
    dispatch.attached_kind = 0;
    dispatch.prepare_pairs(&mut history, &mut lookahead, &mut pairs, preparation);
    assert!((pairs.look.x - 0.3).abs() < 1e-6);
    assert!((pairs.look.y + 0.1875).abs() < 1e-6);
    assert_eq!(pairs.movement, Vec2::new(0.3, 0.2));
    // Closed shaping gate preserves the supplied look pair before boundary scaling.
    dispatch.flags = 0;
    pairs = initial;
    dispatch.prepare_pairs(&mut history, &mut lookahead, &mut pairs, preparation);
    assert_eq!(pairs.look, Vec2::new(0.075 * 1.5, 0.05 * 1.5));
}

#[test]
fn native_dispatch_routes_handlers_before_final_movement_scaling() {
    struct Handlers {
        seen: Option<(LookAngleRoute, LookAxisPairs, u32)>,
    }
    impl Handlers {
        fn record(&mut self, route: LookAngleRoute, pairs: &mut LookAxisPairs, lookahead: u32) {
            assert!(self.seen.is_none());
            self.seen = Some((route, *pairs, lookahead));
            pairs.movement = Vec2::new(0.8, -0.4);
        }
    }
    impl LookAngleHandlers for Handlers {
        fn player(&mut self, pairs: &mut LookAxisPairs, lookahead: &mut u32, integrate: bool) {
            self.record(LookAngleRoute::Player { integrate }, pairs, *lookahead);
        }
        fn attached_player(&mut self, pairs: &mut LookAxisPairs, lookahead: &mut u32) {
            self.record(LookAngleRoute::AttachedPlayer, pairs, *lookahead);
        }
        fn attached_camera(&mut self, pairs: &mut LookAxisPairs, lookahead: &mut u32) {
            self.record(LookAngleRoute::AttachedCamera, pairs, *lookahead);
        }
    }
    for mode in [2, 0x20, 0x40, 0x80, 0x100, 0x200, 0x400] {
        for flags in [0, 0x10, 0x80, 0x90] {
            for kind in [0, 1, 9] {
                let dispatch = LookDispatchState {
                    player_mode: mode,
                    flags,
                    attached_kind: kind,
                    camera_scene_tag18: false,
                };
                let preparation = LookPreparation {
                    input: Vec2::new(0.8, 0.6),
                    alternate_input: false,
                    profile: LookProfile {
                        flags: 0,
                        crosshair: 0,
                        turn_speed: 1.0,
                    },
                    pitch_dead_zone: 0.2,
                    alternate_dead_zone: 0.4,
                    edge: LookEdgeConfig {
                        threshold_positive: 0.95,
                        threshold_negative: -0.95,
                        charge_rate: 3.0,
                        step_rate: 60.0,
                        time_scale: 1.0,
                        minimum_negative_charge: -1.0,
                        direct_gain: 0.9,
                        history_gain: 0.5,
                    },
                    boundary: LookBoundary {
                        player_mode: mode,
                        slow_look: false,
                        attached: AttachedLookBoundary {
                            active: false,
                            flags: 0,
                        },
                        attached_active_gain: 0.3,
                        attached_inactive_gain: 0.4,
                        slow_gain: 0.5,
                        match_fade: Some(0.5),
                    },
                };
                let initial = LookAxisPairs {
                    movement: Vec2::new(0.6, 0.4),
                    look: Vec2::new(0.3, 0.2),
                };
                let mut pairs = initial;
                let mut history = LookEdgeHistory::default();
                let mut lookahead = 3;
                let mut handlers = Handlers { seen: None };
                let route = dispatch.dispatch(
                    &mut history,
                    &mut lookahead,
                    &mut pairs,
                    preparation,
                    0.25,
                    &mut handlers,
                );
                match route {
                    LookAngleRoute::Ignore => {
                        assert!(handlers.seen.is_none());
                        assert_eq!(pairs, initial);
                        assert_eq!(lookahead, 3);
                    }
                    LookAngleRoute::PairsOnly => {
                        assert!(handlers.seen.is_none());
                        let gain = if flags & 0x10 != 0 { 0.125 } else { 0.5 };
                        assert_eq!(pairs.movement, Vec2::new(0.6 * gain, 0.4 * gain));
                    }
                    _ => {
                        let (seen_route, seen_pairs, seen_lookahead) = handlers.seen.unwrap();
                        assert_eq!(seen_route, route);
                        assert_eq!(seen_lookahead, if mode == 0x40 { 1 } else { 3 });
                        assert_eq!(
                            seen_pairs.movement,
                            if mode == 0x80 {
                                Vec2::ZERO
                            } else {
                                Vec2::new(0.3, 0.2)
                            }
                        );
                        let gain = if flags & 0x10 != 0 && mode == 2 {
                            0.25
                        } else {
                            1.0
                        };
                        assert_eq!(pairs.movement, Vec2::new(0.8 * gain, -0.4 * gain));
                    }
                }
            }
        }
    }
}

#[test]
fn player_pair_tier1_translated_tail_4096_routes() {
    for seed in 0..4096u32 {
        let mut m = Memory::default();
        constants(&mut m);
        let integrate = seed & 4 == 0;
        let alternate = seed & 8 != 0;
        let special = seed & 16 != 0;
        let frame_rate = [30.0f32, 60.0, 120.0][seed as usize % 3];
        let time_scale = [0.5f32, 1.0, 1.5][seed as usize / 3 % 3];
        m.write32(0x1000c, if integrate { 0x80 } else { 0 });
        m.write32(0x10f5c, u32::from(alternate));
        m.write32(0x10f7c, u32::from(special));
        m.write32(0x47cfe0, 0.65f32.to_bits());
        m.write32(0x4e8cec, frame_rate as u32);
        m.write32(0x4e794c, time_scale.to_bits());
        let mut r = registers();
        // cdecl: input look, movement, shaped look output, third output.
        for (offset, value) in [(4, 0x30000), (8, 0x31000), (12, 0x32000), (16, 0x33000)] {
            m.write32(r.esp + offset, value);
        }
        let mut state = LookState {
            yaw: (seed % 360) as f32,
            pitch: (seed % 121) as f32 - 60.0,
            ramp: (seed % 101) as f32 / 100.0,
            lookahead: seed % 3,
            move_previous: Vec2::new(0.0, (seed % 5) as f32 / 4.0),
            ..Default::default()
        };
        let flags = (seed % 4) as u8;
        m.write8(0x20002, (flags & 1) | ((flags & 2) << 2));
        m.write32(0x31000, state.move_previous.y.to_bits());
        let pair = Vec2::new(
            (seed % 21) as f32 / 10.0 - 1.0,
            (seed % 19) as f32 / 10.0 - 0.9,
        );
        for (addr, v) in [
            (0x10100, state.yaw),
            (0x10104, state.pitch),
            (0x10f80, state.ramp),
            (0x30000, pair.x),
            (0x30004, pair.y),
        ] {
            m.write32(addr, v.to_bits());
        }
        m.write32(0x10108, state.lookahead);
        let mut calls = LookCalls {
            memory: m.clone(),
            axes: [0.0; 4],
        };
        let mut live_memory = Memory(Rc::new(RefCell::new(m.0.borrow().clone())));
        let mut live_regs = registers();
        let mut live_calls = LookCalls {
            memory: live_memory.clone(),
            axes: [0.0; 4],
        };
        assert!(tsfp_ports::player_look::run(
            0x25d520,
            true,
            &mut live_memory,
            &mut live_calls,
            &mut live_regs
        ));
        oracle::game_player_camera_update_dispatch_by_state_flags(&mut m, &mut calls, &mut r);
        for addr in [
            0x10100, 0x10104, 0x10f80, 0x10108, 0x10fdc, 0x10fe0, 0x31000, 0x31004, 0x32000,
            0x32004,
        ] {
            let expected = f32::from_bits(m.read32(addr));
            let actual = f32::from_bits(live_memory.read32(addr));
            assert!(
                (actual - expected).abs() < 1e-4,
                "seed {seed} live {addr:x}: {actual} vs {expected}"
            );
        }
        assert_eq!(live_regs.esp, 0x9000);
        // Profile 0 / crosshair 0: pitch dead zone, no boost, inverse or lookahead.
        let pitch = if pair.y.abs() <= 0.1 {
            0.0
        } else {
            pair.y.signum() * (pair.y.abs() - 0.1) / 0.9
        };
        let shaped = if integrate {
            if alternate {
                Vec2::new(
                    if pair.x.abs() <= 0.5 {
                        0.0
                    } else {
                        pair.x.signum() * (pair.x.abs() - 0.5) / 0.5
                    },
                    if pair.y.abs() <= 0.5 {
                        0.0
                    } else {
                        pair.y.signum() * (pair.y.abs() - 0.5) / 0.5
                    },
                )
            } else {
                Vec2::new(pair.x, pitch * if flags & 1 != 0 { -1.0 } else { 1.0 })
            }
        } else {
            Vec2::default()
        };
        advance_lookahead(&mut state.lookahead, shaped.y, state.move_previous.y, 0.75);
        integrate_player_pair(
            &mut state,
            shaped,
            integrate,
            PlayerAngleConfig {
                integration: Integration {
                    step_rate: frame_rate,
                    time_scale,
                    ..Integration::default()
                },
                ramp_gain: 0.3,
                special_ramp_target: if special { Some(0.65) } else { None },
                alternate_input: alternate,
                auto_lookahead: flags & 2 != 0,
                lookahead_decay: 0.05,
                lookahead_settle: 0.5,
            },
        );
        assert_eq!(state.lookahead, m.read32(0x10108), "seed {seed} lookahead");
        for (addr, v) in [
            (0x10100, state.yaw),
            (0x10104, state.pitch),
            (0x10f80, state.ramp),
        ] {
            let expected = f32::from_bits(m.read32(addr));
            assert!(
                (v - expected).abs() < 1e-4,
                "seed {seed} {addr:x}: {v} vs {expected}"
            );
        }
    }
}

struct FullLookCalls {
    memory: Memory,
    zoom: f64,
}
impl GuestCall for FullLookCalls {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        let mut m = self.memory.clone();
        let entry = r.esp;
        if matches!(va, 0x258910 | 0x153f90) {
            r.esp = r.esp.wrapping_sub(4);
        }
        match va {
            0x258910 => oracle::game_player_scale_two_pad_axis_pairs_by_state_0x114_and_flags(&mut m, self, r),
            0x25d270 => oracle::game_player_7b0c7c_apply_look_input_yaw_pitch_zoom_scaled_clamped(&mut m, self, r),
            0x258a20 => tsfp_engine_render::scene_t1938::game_camera_apply_pad_axes_to_angles_scaled_by_zoom_and_clamp(&mut m, self, r),
            0x153f90 => tsfp_ports::c2r::t1937_math::game_math_clamp_value_to_limits_lo_hi_with_period_wrap_fallback_via_floor(&mut m, self, r),
            0xcb3b0 => r.fpu.push(self.zoom),
            0x65270 => r.fpu.push(3.0),
            0x3c8501 => {
                let bits = m.read32(r.esp) as u64 | ((m.read32(r.esp + 4) as u64) << 32);
                r.fpu.push(f64::from_bits(bits).floor());
            }
            _ => panic!("unexpected full look call {va:x}"),
        }
        if matches!(va, 0x258910 | 0x153f90) {
            r.esp = entry;
        }
    }
}
#[test]
fn live_native_dispatch_tier1_4096_routes() {
    for seed in 0..4096u32 {
        let mut m = Memory::default();
        constants(&mut m);
        let modes = [2, 0x20, 0x40, 0x80, 0x100, 0x200, 0x400, 0];
        let mode = modes[seed as usize % modes.len()];
        let flags = [0, 0x10, 0x80, 0x90][seed as usize / 8 % 4];
        let alternate = seed & 32 != 0;
        let crosshair = if seed & 64 != 0 { 2 } else { 0 };
        for (addr, value) in [
            (0x10014, 0x40000),
            (0x4007c, 0x41000),
            (0x41c14, 0x42000),
            (0x4207c, 0x43000),
            (0x10114, mode),
            (0x1000c, flags),
            (0x10f5c, u32::from(alternate)),
            (0x20074, crosshair),
            (0x43010, [0, 1, 9, 3][seed as usize / 128 % 4]),
            (0x43008, if seed & 512 != 0 { 0x1000000 } else { 0 }),
            (0x7b0cc0, if seed & 1024 != 0 { 0x44000 } else { 0 }),
            (0x4426c, 0x45000),
            (0x45000, if seed & 2048 != 0 { 0x18 } else { 0 }),
            (0x10108, seed / 16 % 3),
        ] {
            m.write32(addr, value);
        }
        m.write8(0x20002, (seed / 64) as u8);
        for (addr, value) in [
            (0x10100, (seed % 500) as f32 - 70.0),
            (0x10104, (seed % 180) as f32 - 90.0),
            (0x10f80, (seed % 101) as f32 / 100.0),
            (0x10fdc, 0.15),
            (0x10fe0, -0.2),
            (0x20078, (seed % 11) as f32 / 10.0),
            (0x30000, (seed % 21) as f32 / 10.0 - 1.0),
            (0x30004, (seed % 19) as f32 / 9.0 - 1.0),
            (0x31000, 0.8),
            (0x31004, -0.3),
            (0x32000, -0.7),
            (0x32004, 0.4),
            (0x4301c, -65.0),
            (0x43024, 179.0),
            (0x4302c, 185.0),
            (0x43034, -40.0),
            (0x43038, 40.0),
            (0x4303c, -60.0),
            (0x43040, 60.0),
            (0x43044, -180.0),
            (0x43048, 180.0),
            (0x4305c, 0.7),
            (0x42070, 20.0),
            (0x44384, 30.0),
            (0x475cc8, 0.25),
            (0x475d4c, 0.4),
            (0x478674, 0.3),
        ] {
            m.write32(addr, value.to_bits());
        }
        let mut r = registers();
        r.ebx = 123;
        r.esi = 234;
        r.edi = 345;
        r.ebp = 456;
        for (offset, value) in [(4, 0x30000), (8, 0x31000), (12, 0x32000), (16, 0x33000)] {
            m.write32(r.esp + offset, value);
        }
        let mut live_m = Memory(Rc::new(RefCell::new(m.0.borrow().clone())));
        let mut live_r = r.clone();
        let mut calls = FullLookCalls {
            memory: m.clone(),
            zoom: 55.0,
        };
        let mut live_calls = FullLookCalls {
            memory: live_m.clone(),
            zoom: 55.0,
        };
        oracle::game_player_camera_update_dispatch_by_state_flags(&mut m, &mut calls, &mut r);
        assert!(tsfp_ports::player_look::run(
            0x25d520,
            true,
            &mut live_m,
            &mut live_calls,
            &mut live_r
        ));
        for addr in [
            0x10100, 0x10104, 0x10f80, 0x10108, 0x10fdc, 0x10fe0, 0x31000, 0x31004, 0x32000,
            0x32004, 0x4301c, 0x43024, 0x4302c,
        ] {
            let expected = f32::from_bits(m.read32(addr));
            let actual = f32::from_bits(live_m.read32(addr));
            assert!(
                (actual - expected).abs() < 1e-4,
                "seed {seed} mode {mode:x} flags {flags:x} {addr:x}: {actual} vs {expected}"
            );
        }
        assert_eq!(
            (live_r.esp, live_r.ebx, live_r.esi, live_r.edi, live_r.ebp),
            (0x9000, 123, 234, 345, 456)
        );
    }
}

#[test]
fn live_dispatch_does_not_read_absent_camera_or_attachment() {
    struct Guarded(Memory);
    impl OracleMemory for Guarded {
        fn read8(&self, a: u32) -> u8 {
            assert!(a >= 0x6000, "unused nullable field read at {a:x}");
            self.0.read8(a)
        }
        fn write8(&mut self, a: u32, v: u8) {
            self.0.write8(a, v);
        }
    }
    let mut m = Memory::default();
    constants(&mut m);
    m.write32(0x1000c, 0x80);
    m.write32(0x10014, 0x40000);
    m.write32(0x4007c, 0x41000);
    let mut r = registers();
    for (offset, value) in [(4, 0x30000), (8, 0x31000), (12, 0x32000), (16, 0x33000)] {
        m.write32(r.esp + offset, value);
    }
    let mut calls = FullLookCalls {
        memory: m.clone(),
        zoom: 55.0,
    };
    let mut guarded = Guarded(m);
    assert!(tsfp_ports::player_look::run(
        0x25d520,
        true,
        &mut guarded,
        &mut calls,
        &mut r
    ));
}
