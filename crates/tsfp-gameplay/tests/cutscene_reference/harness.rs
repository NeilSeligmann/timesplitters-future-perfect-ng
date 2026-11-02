// SPDX-License-Identifier: GPL-3.0-or-later
// Standalone reference harness: source-includes the retained translations without a host link.
// Run through python3 -m tools.t2195_cutscene_diff; no UI/menu/HUD root is executed.
#![allow(dead_code, unused_imports)]
#[path = "../../../tsfp-ports/src/abi.rs"]
mod abi;
#[path = "../../../tsfp-ports/src/system_slice/cutscene_camera_boundary.rs"]
mod camera_boundary;
#[path = "../../../tsfp-ports/src/combat.rs"]
mod combat;
#[path = "../../../tsfp-ports/src/ext.rs"]
mod ext;
#[path = "../../../tsfp-ports/src/inventory.rs"]
mod inventory;
#[path = "../../../tsfp-ports/src/inventory_ammo.rs"]
mod inventory_ammo;
#[path = "../../../tsfp-ports/src/inventory_grant.rs"]
mod inventory_grant;
#[path = "../../../tsfp-ports/src/inventory_pickup.rs"]
mod inventory_pickup;
#[path = "../../../tsfp-ports/src/inventory_select.rs"]
mod inventory_select;
#[path = "../../../tsfp-ports/src/inventory_slots.rs"]
mod inventory_slots;
#[path = "../../../tsfp-ports/src/memory.rs"]
mod memory;
#[path = "../../../tsfp-ports/src/system_slice/cutscene_movie_frame_boundary.rs"]
mod movie_frame_boundary;
mod runtime {
    pub fn root_selected(_: u32) -> bool {
        false
    }
}
fn enabled() -> bool {
    false
}
#[path = "../../../tsfp-ports/src/system_slice/cutscene_overlay_boundary.rs"]
mod overlay_boundary;
#[path = "../../../tsfp-ports/src/pose.rs"]
mod pose;
#[path = "../../../tsfp-ports/src/pose_anim.rs"]
mod pose_anim;
#[path = "../../../tsfp-ports/src/pose_leaf.rs"]
mod pose_leaf;
#[path = "../../../tsfp-ports/src/system_slice/mod.rs"]
mod system_slice;
#[path = "../../../tsfp-ports/src/vehicles.rs"]
mod vehicles;
#[path = "../../../tsfp-ports/src/video.rs"]
mod video;
#[path = "../../../tsfp-port-wire/src/views.rs"]
mod views;
use abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use memory::GuestMemory;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

#[path = "../../src/cutscene_presentation.rs"]
mod model;
use tsfp_ports::c2r::t2049_exec_009 as reference;
#[path = "../../../tsfp-ports/src/c2r/rt.rs"]
mod rt;
mod state_wire {
    pub fn get_flags<M: super::GuestMemory>(m: &M) -> u32 {
        m.read32(0x7de458)
    }
    pub fn get_level<M: super::GuestMemory>(m: &M) -> u32 {
        m.read32(0x79094c)
    }
}
use model::*;
#[derive(Default)]
struct Mem(Rc<RefCell<BTreeMap<u32, u8>>>);
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        *self.0.borrow().get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.borrow_mut().insert(a, v);
    }
}
fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: 0x10000,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0x27f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}
struct Guest {
    mem: Mem,
    input: OverlayInput,
    commands: Vec<OverlayCommand>,
    state_calls: Vec<(u32, u32)>,
    blend: Blend,
}
fn arg(m: &Mem, r: &ExtRegisters, n: u32) -> u32 {
    m.read32(r.esp + n * 4)
}
impl GuestCall for Guest {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        if matches!(va, 0x19d70 | 0x19650) {
            self.state_calls.push((va, arg(&self.mem, r, 0)));
        }
        match va {
            0x98da0 => {
                r.eax = if self.input.clock.is_some() {
                    0x20000
                } else {
                    0
                }
            }
            0x607a0 => {
                for (n, v) in [
                    (2, self.input.sample.strip_height.to_bits()),
                    (3, self.input.sample.gray.to_bits()),
                    (4, self.input.sample.color),
                    (5, self.input.sample.additive_color),
                ] {
                    self.mem.write32(arg(&self.mem, r, n), v);
                }
            }
            0x3c848c => {
                r.eax = r.fpu.st(0).trunc() as i32 as u32;
                r.fpu.pop();
            }
            0xf8bb0 => self.commands.push(OverlayCommand::Strips {
                height: f32::from_bits(arg(&self.mem, r, 1)),
                viewport: Rect {
                    left: arg(&self.mem, r, 8) as i32,
                    top: arg(&self.mem, r, 9) as i32,
                    right: arg(&self.mem, r, 11) as i32,
                    bottom: arg(&self.mem, r, 10) as i32,
                },
            }),
            0xf8ed0 => self.commands.push(OverlayCommand::Gray {
                alpha: arg(&self.mem, r, 0) as i32,
            }),
            0x19d70 => {
                self.blend = if arg(&self.mem, r, 0) == 2 {
                    Blend::Additive
                } else {
                    Blend::Normal
                };
            }
            0x19650 => {}
            0x1efb0 => {
                let blend = self.blend;
                self.commands.push(OverlayCommand::Fill {
                    rect: Rect {
                        left: arg(&self.mem, r, 0) as i32,
                        top: arg(&self.mem, r, 1) as i32,
                        right: arg(&self.mem, r, 2) as i32,
                        bottom: arg(&self.mem, r, 3) as i32,
                    },
                    color: arg(&self.mem, r, 4),
                    blend,
                });
            }
            0x3c420 => r.eax = self.input.wide_display as u32,
            0x6eea0 => self.commands.push(OverlayCommand::Subtitles),
            _ => panic!("unexpected callee {va:x}"),
        }
    }
}
fn input(seed: u32) -> OverlayInput {
    OverlayInput {
        clock: if seed % 11 == 0 {
            None
        } else {
            Some(PlaybackClock {
                time: (seed % 80) as f32 / 20.0 - 0.2,
                ending: seed & 1 != 0,
                end_elapsed: (seed % 37) as f32 / 10.0,
                end_duration: (seed % 7) as f32,
            })
        },
        sample: OverlaySample {
            strip_height: (seed % 31) as f32 - 1.0,
            gray: (seed % 23) as f32 / 10.0 - 0.1,
            color: if seed & 2 != 0 { 0x12345678 } else { 0 },
            additive_color: if seed & 4 != 0 { 0xaabbcc00 } else { 0 },
        },
        viewport: Rect {
            left: 3,
            top: 4,
            right: 639,
            bottom: 479,
        },
        interval: Extent {
            width: 640 + (seed % 99) as i32,
            height: 480,
        },
        wide_display: seed & 8 != 0,
        suppress_fallback: seed & 16 != 0,
        force_black: seed & 32 != 0,
        record_allows_scene: seed & 64 != 0,
        constants: OverlayConstants {
            threshold: 0.0,
            opening_end: 0.5,
            opening_rate: 2.0,
            alpha_scale: 255.0,
            fixed_bar_fraction: 0.125,
        },
    }
}
#[test]
fn overlays_differential_4096() {
    for seed in 0..4096 {
        let i = input(seed);
        let mut m = Mem::default();
        let mut r = regs();
        for (a, v) in [
            (0x475cac, i.constants.threshold),
            (0x475cd4, i.constants.opening_end),
            (0x475d10, i.constants.opening_rate),
            (0x475cc0, i.constants.alpha_scale),
            (0x475c78, 1.0),
            (0x478920, i.constants.fixed_bar_fraction),
        ] {
            m.write32(a, v.to_bits());
        }
        m.write32(0x4e8cf0, i.interval.width as u32);
        m.write32(0x4e8cf4, i.interval.height as u32);
        m.write32(0x7e71c8, 0x30000);
        for (o, v) in [
            (0, i.viewport.left),
            (4, i.viewport.bottom),
            (0x14, i.viewport.top),
            (0x18, i.viewport.right),
        ] {
            m.write32(0x30000 + o, v as u32);
        }
        m.write32(
            0x7de458,
            if i.force_black { 0x20000000 } else { 0 }
                | if i.suppress_fallback { 0x200 } else { 0 },
        );
        if let Some(clock) = i.clock {
            m.write32(0x20008, clock.time.to_bits());
            m.write32(0x2001c, clock.ending as u32);
            m.write32(0x20018, clock.end_elapsed.to_bits());
            m.write32(0x20010, clock.end_duration.to_bits());
            m.write32(0x20020, i.record_allows_scene as u32);
        }
        let mut g = Guest {
            mem: Mem(m.0.clone()),
            input: i,
            commands: vec![],
            state_calls: vec![],
            blend: Blend::Normal,
        };
        let mut adapter_memory = Mem(Rc::new(RefCell::new(m.0.borrow().clone())));
        let mut adapter_regs = r;
        let mut adapter_guest = Guest {
            mem: Mem(adapter_memory.0.clone()),
            input: i,
            commands: vec![],
            state_calls: vec![],
            blend: Blend::Normal,
        };
        overlay_boundary::draw_original(&mut adapter_memory, &mut adapter_guest, &mut adapter_regs);
        reference::game_cutscene_draw_letterbox_bars_gray_overlay_and_subtitles(
            &mut m, &mut g, &mut r,
        );
        let mut expected = vec![];
        original_overlays(&i, |v| expected.push(v));
        assert_eq!(g.commands, expected, "seed {seed}");
        assert_eq!(
            adapter_guest.commands, g.commands,
            "overlay adapter seed {seed}"
        );
        assert_eq!(
            adapter_guest.state_calls, g.state_calls,
            "renderer state sequence seed {seed}"
        );
        assert_eq!(adapter_regs.esp, 0x10000);
        assert_eq!(r.esp, 0x10000);
    }
}
struct NoCall;
impl GuestCall for NoCall {
    fn call(&mut self, va: u32, _: &mut ExtRegisters) {
        panic!("unexpected {va:x}");
    }
}
#[test]
fn movie_setup_differential_4096() {
    let choices = [-2, -1, 0, 1, 480, 640, i32::MAX];
    for seed in 0..4096usize {
        let request = Extent {
            width: choices[seed % 7],
            height: choices[(seed / 7) % 7],
        };
        let mode = (seed % 5) as i32 - 1;
        let mut m = Mem::default();
        let mut r = regs();
        for (a, v) in [
            (0x10004, request.width as u32),
            (0x10008, request.height as u32),
            (0x1000c, mode as u32),
            (0x687e44, 11),
            (0x687e48, 22),
            (0x4bfc6c, (seed % 2) as u32),
            (0x476688, (4.0f32 / 3.0).to_bits()),
            (0x475d40, 0.75f32.to_bits()),
        ] {
            m.write32(a, v);
        }
        let mut state = MovieState {
            crop_inset: Extent {
                width: 11,
                height: 22,
            },
            placement: if seed % 2 == 0 {
                MoviePlacement::Stretch
            } else {
                MoviePlacement::OriginalAspect
            },
        };
        state.configure(request, mode, 4.0 / 3.0, 0.75);
        let mut adapter_memory = Mem(Rc::new(RefCell::new(m.0.borrow().clone())));
        let mut adapter_regs = r;
        video::f30210(&mut adapter_memory, &mut NoCall, &mut adapter_regs);
        video::f30210_reference(&mut m, &mut NoCall, &mut r);
        assert_eq!(adapter_regs, r, "setup ABI seed {seed}");
        assert_eq!(
            *adapter_memory.0.borrow(),
            *m.0.borrow(),
            "setup writes seed {seed}"
        );
        assert_eq!(
            state.crop_inset.width,
            m.read32(0x687e44) as i32,
            "seed {seed}"
        );
        assert_eq!(
            state.crop_inset.height,
            m.read32(0x687e48) as i32,
            "seed {seed}"
        );
        assert_eq!(
            state.placement == MoviePlacement::OriginalAspect,
            m.read32(0x4bfc6c) == 1
        );
        assert_eq!(r.esp, 0x10000);
    }
}
#[test]
fn movie_setup_adapter_invalid_state_and_scalar_edges() {
    let requests = [
        (-1, 1),
        (1, -1),
        (-2, 1),
        (1, -2),
        (-1, -1),
        (0, 0),
        (640, 480),
    ];
    let scales = [
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        0.0,
        -1.0,
        f32::MAX,
        0.75,
    ];
    for (width, height) in requests {
        for scale in scales {
            for mode in [-1i32, 0, 1, 9] {
                let mut reference_memory = Mem::default();
                let mut reference_regs = regs();
                reference_regs.xmm[0].lanes = [0x12345678, 2, 3, 4];
                for (address, value) in [
                    (0x10004, width as u32),
                    (0x10008, height as u32),
                    (0x1000c, mode as u32),
                    (0x687e44, 11),
                    (0x687e48, 22),
                    (0x4bfc6c, 7),
                    (0x476688, scale.to_bits()),
                    (0x475d40, scale.to_bits()),
                ] {
                    reference_memory.write32(address, value);
                }
                let mut adapter_memory =
                    Mem(Rc::new(RefCell::new(reference_memory.0.borrow().clone())));
                let mut adapter_regs = reference_regs;
                video::f30210_reference(&mut reference_memory, &mut NoCall, &mut reference_regs);
                video::f30210(&mut adapter_memory, &mut NoCall, &mut adapter_regs);
                assert_eq!(adapter_regs, reference_regs);
                assert_eq!(*adapter_memory.0.borrow(), *reference_memory.0.borrow());
            }
        }
    }
}

struct MovieGuest {
    mem: Mem,
    decoded: Extent,
    frames: Vec<MovieFrame>,
    calls: usize,
    decode_count: usize,
}
impl GuestCall for MovieGuest {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        self.calls += 1;
        assert!(self.calls < 128);
        r.eax = 0;
        if va == 0x445055 {
            let out = arg(&self.mem, r, 1);
            self.mem.write32(out, self.decoded.width as u32);
            self.mem.write32(out + 4, self.decoded.height as u32);
        }
        if va == 0x3d4e10 {
            r.eax = if self.calls % 2 == 0 {
                0x40000
            } else {
                0x50000
            };
        }
        if va == 0x44525d {
            self.decode_count += 1;
            self.mem.write32(
                arg(&self.mem, r, 2),
                if self.decode_count == 1 { 1 } else { 2 },
            );
        }
        if va == 0x3d9810 {
            let rect = |a| Rect {
                left: self.mem.read32(a) as i32,
                top: self.mem.read32(a + 4) as i32,
                right: self.mem.read32(a + 8) as i32,
                bottom: self.mem.read32(a + 12) as i32,
            };
            self.frames.push(MovieFrame {
                source: rect(arg(&self.mem, r, 1)),
                destination: rect(arg(&self.mem, r, 2)),
            });
        }
        if va == 0x3c848c {
            r.eax = r.fpu.st(0).trunc() as i32 as u32;
            r.fpu.pop();
        }
        let args = match va {
            0x444a2d => 3,
            0x4450c2 => 5,
            0x445241 => 2,
            0x445055 => 2,
            0x3e6684 => 9,
            0x3d4d70 => 2,
            0x3d4e10 => 2,
            0x3d5eb0 => 6,
            0x3d8e50 | 0x3d9990 | 0x444f71 | 0x3d4c90 | 0x445252 => 1,
            0x44525d => 4,
            0x3d9810 => 5,
            0x407b14 => 2,
            _ => 0,
        };
        r.esp += args * 4;
    }
}
#[test]
fn movie_placement_differential_2048() {
    for seed in 0..2048 {
        let state = MovieState {
            crop_inset: Extent {
                width: seed % 5,
                height: seed % 3,
            },
            placement: if seed % 2 == 0 {
                MoviePlacement::Stretch
            } else {
                MoviePlacement::OriginalAspect
            },
        };
        let decoded = Extent {
            width: 8,
            height: 6,
        };
        let display = Extent {
            width: 640 + seed % 700,
            height: 480 + seed % 80,
        };
        let interval = Extent {
            width: 640 + seed % 19,
            height: 480 + seed % 13,
        };
        let mut m = Mem::default();
        let mut r = regs();
        for (a, v) in [
            (0x476688, (4.0f32 / 3.0).to_bits()),
            (0x475cd4, 0.5f32.to_bits()),
            (0x4c0458, display.width as u32),
            (0x4c045c, display.height as u32),
            (0x4e8cf0, interval.width as u32),
            (0x4e8cf4, interval.height as u32),
            (0x687e44, state.crop_inset.width as u32),
            (0x687e48, state.crop_inset.height as u32),
            (
                0x4bfc6c,
                (state.placement == MoviePlacement::OriginalAspect) as u32,
            ),
        ] {
            m.write32(a, v);
        }
        let mut g = MovieGuest {
            mem: Mem(m.0.clone()),
            decoded,
            frames: vec![],
            calls: 0,
            decode_count: 0,
        };
        let frame = movie_frame_boundary::rectangles([
            state.crop_inset.width as u32,
            state.crop_inset.height as u32,
            (state.placement == MoviePlacement::OriginalAspect) as u32,
            decoded.width as u32,
            decoded.height as u32,
            display.width as u32,
            display.height as u32,
            interval.width as u32,
            interval.height as u32,
            (4.0f32 / 3.0).to_bits(),
        ])
        .unwrap();
        let expected_frame = state.rectangles(decoded, display, interval, (4.0f32 / 3.0) as f64);
        assert_eq!(
            frame,
            [
                expected_frame.source.left,
                expected_frame.source.top,
                expected_frame.source.right,
                expected_frame.source.bottom,
                expected_frame.destination.left,
                expected_frame.destination.top,
                expected_frame.destination.right,
                expected_frame.destination.bottom
            ]
        );
        video::f30280(&mut m, &mut g, &mut r);
        assert_eq!(
            g.frames,
            vec![state.rectangles(decoded, display, interval, (4.0f32 / 3.0) as f64)],
            "seed {seed}"
        );
        assert_eq!(r.esp, 0x10000);
    }
}
struct CameraGuest {
    mem: Mem,
    keys: [CameraKey; 2],
    indices: Vec<i32>,
    fraction: Option<f32>,
    discontinuity: bool,
    wide: bool,
    fov: Option<f32>,
}
impl GuestCall for CameraGuest {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        match va {
            0x70330 => r.eax = 0x20000,
            0x60740 => {
                self.indices.push(arg(&self.mem, r, 0) as i32);
                let k = self.keys[self.indices.len() - 1];
                for (n, values) in [
                    (1, &k.rotation[..]),
                    (2, &k.position[..]),
                    (3, &[k.lens][..]),
                ] {
                    let out = arg(&self.mem, r, n);
                    for (i, v) in values.iter().enumerate() {
                        self.mem.write32(out + i as u32 * 4, v.to_bits());
                    }
                }
            }
            0x60920 => r.eax = self.discontinuity as u32,
            0x150960 => {
                self.fraction = Some(f32::from_bits(arg(&self.mem, r, 2)));
            }
            0x1506b0 => {
                let out = arg(&self.mem, r, 0);
                // Script only the neighbouring quaternion/matrix service, not the root under test.
                for i in 0..12 {
                    self.mem
                        .write32(out + i * 4, if i % 4 == 0 { 1.0f32.to_bits() } else { 0 });
                }
            }
            0x3c420 => r.eax = self.wide as u32,
            0xc77c0 => self.fov = Some(f32::from_bits(arg(&self.mem, r, 3))),
            0xc3c50 => r.eax = 1,
            _ => panic!("unexpected camera callee {va:x}"),
        }
    }
}
struct IdentityMath;
impl CameraMath for IdentityMath {
    fn interpolate_rotation(&mut self, _: [f32; 4], _: [f32; 4], _: f32) -> [[f32; 3]; 3] {
        [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
    }
}
#[test]
fn camera_selection_position_lens_differential_4096() {
    for seed in 0..4096 {
        let time = (seed % 160) as f32 / 30.0 - 0.5;
        let rate = 30.0;
        let count = 90;
        let calibration = LensCalibration {
            lens_scale: 0.25,
            numerator: 36.0,
            aperture_scale: 0.5,
            aperture: 1.5,
            angle_units: 57.29578,
            wide_scale: 0.75,
        };
        let first = CameraKey {
            position: [seed as f32 / 10.0, -2.0, 7.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            lens: 10.0 + (seed % 31) as f32,
        };
        let next = CameraKey {
            position: [seed as f32 / 8.0, 3.0, -9.0],
            lens: first.lens + 5.0,
            ..first
        };
        let discontinuity = seed % 2 == 0;
        let wide = seed % 3 == 0;
        let mut m = Mem::default();
        let mut r = regs();
        for (a, v) in [
            (0x4d1994, rate),
            (0x20008, time),
            (0x475c78, 1.0),
            (0x475cd4, 0.5),
            (0x47d630, 1.0),
            (0x47d62c, calibration.lens_scale),
            (0x4765e0, calibration.numerator),
            (0x47d628, calibration.aperture_scale),
            (0x47d61c, calibration.aperture),
            (0x478638, calibration.angle_units),
            (0x47d624, calibration.wide_scale),
            (0x475d1c, 1.0),
        ] {
            m.write32(a, v.to_bits());
        }
        for (a, v) in [
            (0x6d780c, 1),
            (0x20000, count as u32),
            (0x7b0cc0, 0x30000),
            (0x30208, 0x40000),
            (0x4c0458, 640),
            (0x4c045c, 480),
            (0x4e8cf0, 640),
            (0x4e8cf4, 480),
        ] {
            m.write32(a, v);
        }
        let mut g = CameraGuest {
            mem: Mem(m.0.clone()),
            keys: [first, next],
            indices: vec![],
            fraction: None,
            discontinuity,
            wide,
            fov: None,
        };
        let mut adapter_memory = Mem(Rc::new(RefCell::new(m.0.borrow().clone())));
        let mut adapter_regs = r;
        let mut adapter_guest = CameraGuest {
            mem: Mem(adapter_memory.0.clone()),
            keys: [first, next],
            indices: vec![],
            fraction: None,
            discontinuity,
            wide,
            fov: None,
        };
        camera_boundary::apply_original(&mut adapter_memory, &mut adapter_guest, &mut adapter_regs);
        reference::game_cutscene_camera_apply_interpolated_key_record_to_camera_7b0cc0_with_fov(
            &mut m, &mut g, &mut r,
        );
        assert_eq!(
            adapter_guest.indices, g.indices,
            "camera adapter indices {seed}"
        );
        assert_eq!(
            adapter_guest.fraction, g.fraction,
            "camera adapter fraction {seed}"
        );
        assert_eq!(adapter_regs.esp, r.esp);
        for address in (0x30000..0x303a0)
            .step_by(4)
            .chain((0x40000..0x40040).step_by(4))
        {
            let actual = adapter_memory.read32(address);
            let expected = m.read32(address);
            if address == 0x30384 {
                assert!(
                    (f32::from_bits(actual) - f32::from_bits(expected)).abs() < 0.00002,
                    "camera adapter FOV {seed}"
                );
            } else {
                assert_eq!(
                    actual, expected,
                    "camera adapter write {address:x} seed {seed}"
                );
            }
        }
        match select_keys(time, rate, count, discontinuity, 0.5) {
            None => assert!(g.indices.is_empty()),
            Some(selection) => {
                assert_eq!(
                    g.indices,
                    vec![selection.index, selection.index + 1],
                    "seed {seed}"
                );
                assert_eq!(g.fraction, Some(selection.fraction), "seed {seed}");
                let output = camera_sample(
                    first,
                    next,
                    selection.fraction,
                    calibration,
                    wide,
                    &mut IdentityMath,
                );
                for (axis, value) in output.position.iter().enumerate() {
                    assert_eq!(
                        m.read32(0x40030 + axis as u32 * 4),
                        value.to_bits(),
                        "seed {seed} axis {axis}"
                    );
                }
                let actual = g.fov.unwrap();
                assert!(
                    (actual - output.fov).abs() < 0.00002,
                    "seed {seed}: {actual} vs {}",
                    output.fov
                );
            }
        }
        assert_eq!(r.esp, 0x10000);
    }
}

#[test]
fn original_policy_and_lens_service_contract() {
    let c = LensCalibration {
        lens_scale: 0.25,
        numerator: 36.0,
        aperture_scale: 0.5,
        aperture: 1.5,
        angle_units: 57.29578,
        wide_scale: 0.79,
    };
    let mut state = LensState {
        current_fov: 12.0,
        target_fov: 17.0,
    };
    state.apply(LensSource::Preserve, c);
    assert_eq!(
        state,
        LensState {
            current_fov: 12.0,
            target_fov: 17.0
        }
    );
    state.apply(LensSource::CameraService { fov: 55.0 }, c);
    assert_eq!(
        state,
        LensState {
            current_fov: 55.0,
            target_fov: 55.0
        }
    );
    state.apply(
        LensSource::Optical {
            lens: 25.0,
            wide: true,
        },
        c,
    );
    assert_eq!(state.current_fov, c.original_fov(25.0, true));
    assert_eq!(state.target_fov, state.current_fov);
    assert_eq!(
        original_projection_ratio(
            Extent {
                width: 1280,
                height: 720
            },
            Extent {
                width: 640,
                height: 480
            }
        ),
        (4.0f64 / 3.0) as f32
    );
    let i = input(5);
    let mut expected = vec![];
    original_overlays(&i, |v| expected.push(v));
    let state = PresentationState {
        clock: i.clock.unwrap(),
        movie: MovieState {
            crop_inset: Extent {
                width: 0,
                height: 0,
            },
            placement: MoviePlacement::Stretch,
        },
        camera: None,
    };
    let mut actual = vec![];
    state.overlays(i, &Original, |v| actual.push(v));
    assert_eq!(actual, expected);
    let camera = CameraPresentation {
        position: [0.0; 3],
        basis: [[0.0; 3]; 3],
        fov: 55.0,
    };
    assert_eq!(Original.camera(camera), camera);
    let frame = state.movie.rectangles(
        Extent {
            width: 640,
            height: 480,
        },
        i.interval,
        i.interval,
        4.0 / 3.0,
    );
    assert_eq!(Original.movie(frame), frame);
}
