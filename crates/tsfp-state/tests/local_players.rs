// SPDX-License-Identifier: GPL-3.0-or-later
// T2214: FABRICATED input, translated oracle; ordered boundary effects only.
use std::{cell::RefCell, rc::Rc};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
};
use tsfp_state::local_players::*;
#[path = "support/local_players_reference.rs"]
mod reference;
#[derive(Clone)]
struct Image(Rc<RefCell<std::collections::BTreeMap<u32, u8>>>);
impl GuestMemory for Image {
    fn read8(&self, a: u32) -> u8 {
        *self.0.borrow().get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.borrow_mut().insert(a, v);
    }
}
impl tsfp_state::GuestMemory for Image {
    fn read8(&self, a: u32) -> u8 {
        GuestMemory::read8(self, a)
    }
    fn write8(&mut self, a: u32, v: u8) {
        GuestMemory::write8(self, a, v);
    }
}
const EXT: u32 = 0x200000;
const PLAYER: u32 = 0x210000;
const STACK: u32 = 0x240000;
fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: STACK,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.; 8],
            top: 0,
            control: 0x300,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}
fn image() -> Image {
    Image(Rc::new(RefCell::new(std::collections::BTreeMap::new())))
}
fn word(m: &mut Image, a: u32, v: u32) {
    GuestMemory::write32(m, a, v);
}

fn read(m: &Image, a: u32) -> u32 {
    GuestMemory::read32(m, a)
}
fn random(s: &mut u32) -> u32 {
    *s = s.wrapping_mul(1664525).wrapping_add(1013904223);
    s.rotate_right(13)
}
struct Calls {
    m: Image,
    count: u32,
    remap: u32,
    events: Vec<(u32, Vec<u32>)>,
}
impl GuestCall for Calls {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        let n = match va {
            0x356800 => 1,
            0x55e40 => 4,
            0x1cdae0 => 0,
            0x55d90 => 5,
            0x17c740 => 4,
            _ => panic!("unexpected {va:x}"),
        };
        self.events
            .push((va, (0..n).map(|i| read(&self.m, r.esp + 4 * i)).collect()));
        if va == 0x356800 {
            r.eax = self.remap;
        }
        if va == 0x1cdae0 {
            r.eax = self.count;
        }
    }
}
#[test]
fn assignment_differential() {
    let mut seed = 2214;
    for _ in 0..4096 {
        let mut m = image();
        let mut slots = LocalSlots {
            setup_count: random(&mut seed) % 5,
            pads: core::array::from_fn(|_| random(&mut seed) % 6),
        };
        slots.store(&mut m);
        assert_eq!(LocalSlots::load(&m), slots);
        let needle = random(&mut seed) % 8;
        let mut r = regs();
        word(&mut m, STACK + 4, needle);
        reference::game_input_pad_map_find_index(&mut m, &mut r);
        assert_eq!(r.eax, slots.find(needle).map_or(u32::MAX, |i| i as u32));
        let slot = (random(&mut seed) % 4) as usize;
        word(&mut m, STACK + 4, slot as u32);
        r = regs();
        reference::game_input_pad_map_get(&mut m, &mut r);
        assert_eq!(Some(r.eax), slots.pad(slot));
        let pad = random(&mut seed);
        word(&mut m, STACK + 8, pad);
        r = regs();
        tsfp_ports::c2r::leaf3_wired::game_input_pad_map_set(&mut m, &mut r);
        assert!(slots.assign(slot, pad));
        assert_eq!(LocalSlots::load(&m), slots);
        assert!(!slots.assign(4, pad));
        assert_eq!(slots.pad(4), None);
    }
}
#[test]
fn layout_differential() {
    let mut seed = 25;
    for _ in 0..4096 {
        let mut m = image();
        let count = random(&mut seed) % 7;
        let slot = random(&mut seed) % 8;
        let w = (random(&mut seed) % 2000 + 2) as i32;
        let h = (random(&mut seed) % 1200 + 2) as i32;
        let half = random(&mut seed) & 1 != 0;
        let edge = (random(&mut seed) % 100) as f32;
        word(&mut m, 0x4d1920, edge.to_bits());
        word(&mut m, 0x4760a8, 3f32.to_bits());
        word(&mut m, 0x4e8cf0, w as u32);
        word(&mut m, 0x4e8cf4, h as u32);
        word(&mut m, 0x7de458, if half { 0x40 } else { 0 });
        word(&mut m, 0x7de470, 0x13);
        for (i, t) in [0x25a5e4, 0x25a635, 0x25a683, 0x25a701].iter().enumerate() {
            word(&mut m, 0x25a76c + i as u32 * 4, *t);
        }
        word(&mut m, 0x478900, 45f32.to_bits());
        word(&mut m, 0x4785cc, 55f32.to_bits());
        word(&mut m, 0x7b0c48, PLAYER);
        let remap = random(&mut seed) % 4;
        let camera = 0x300000;
        word(&mut m, PLAYER + remap * 0x1584 + 8, camera);
        word(&mut m, STACK + 4, slot);
        word(&mut m, STACK + 8, EXT);
        let mut calls = Calls {
            m: m.clone(),
            count,
            remap,
            events: vec![],
        };
        let mut r = regs();
        reference::game_viewport_setup_for_player_slot_split_screen(&mut m, &mut calls, &mut r);
        let result = Original.layout(LayoutRequest {
            width: w,
            height: h,
            count,
            slot,
            half_width_single: half,
        });
        let rect = calls.events.iter().find(|e| e.0 == 0x55d90).map(|e| Rect {
            left: e.1[1] as i32,
            top: e.1[2] as i32,
            right: e.1[3] as i32,
            bottom: e.1[4] as i32,
        });
        assert_eq!(result.map(|l| l.rect), rect, "count {count} slot {slot}");
        assert_eq!(
            read(&m, camera + 0x384),
            result.and_then(|l| l.camera_fov).unwrap_or(0)
        );
        assert_eq!(calls.events[0], (0x356800, vec![slot]));
        assert_eq!(calls.events[1].0, 0x55e40);
        assert_eq!(calls.events[2].0, 0x1cdae0);
        let mut e = Setup {
            count,
            remap,
            shift: edge + 3.0,
            events: vec![],
            fov: None,
        };
        setup_viewport(
            LayoutRequest {
                width: w,
                height: h,
                count: 0,
                slot,
                half_width_single: half,
            },
            &mut e,
        );
        assert_eq!(calls.events, e.events);
        assert_eq!(e.fov, result.and_then(|l| l.camera_fov).map(|v| (remap, v)));
    }
}
struct Effects {
    valid: [bool; 4],
    events: Vec<(u32, Vec<u32>)>,
}
impl PairEffects for Effects {
    fn handle_valid(&mut self, p: usize, _: u32) -> bool {
        self.valid[p]
    }
    fn attach(&mut self, p: usize, h: u32) {
        self.events.push((
            0x17c740,
            vec![
                0x400000 + (h >> 22) * 0x2e8,
                0x320000 + p as u32 * 0x100,
                0,
                0,
            ],
        ));
    }
    fn full_screen(&mut self, s: u32, r: Rect) {
        self.events.push((
            0x55d90,
            vec![
                0x300000 + s * 0x1000 + 0x224,
                r.left as u32,
                r.top as u32,
                r.right as u32,
                r.bottom as u32,
            ],
        ));
    }
}
#[test]
fn pair_differential() {
    let mut seed = 110;
    let mut pair_branches = [false; 2];
    let mut saw_attach = false;
    let mut saw_stale = false;
    for _ in 0..4096 {
        let mut m = image();
        let count = (random(&mut seed) % 5) as usize;
        let mode = if random(&mut seed) & 1 == 0 { 0xa } else { 2 };
        let selected = random(&mut seed) % 40;
        let mut s = PairState {
            mode,
            active_count: count,
            players: [PlayerBinding::default(); 4],
            selected_mask: random(&mut seed),
        };
        let valid = core::array::from_fn(|_| random(&mut seed) & 1 == 0);
        word(&mut m, 0x790950, count as u32);
        GuestMemory::write8(&mut m, 0x7de455, mode);
        word(&mut m, 0x7b0c48, PLAYER);
        word(&mut m, 0x72f5e8, 0x400000);
        word(&mut m, 0x7330d8, s.selected_mask);
        word(&mut m, 0x4e8cf0, 641);
        word(&mut m, 0x4e8cf4, 481);
        for i in 0..4 {
            let handle = if random(&mut seed) % 3 == 0 {
                0
            } else {
                ((i as u32 + 1) << 22) | 19
            };
            let camera = if random(&mut seed) & 1 == 0 {
                Some(random(&mut seed))
            } else {
                None
            };
            s.players[i] = PlayerBinding {
                pending_handle: handle,
                flags: random(&mut seed),
                camera_flags: camera,
            };
            let p = PLAYER + i as u32 * 0x1584;
            let obj = 0x320000 + i as u32 * 0x100;
            let record = 0x330000 + i as u32 * 0x2000;
            word(&mut m, p + 0x14, obj);
            word(&mut m, obj + 0x7c, record);
            word(&mut m, record + 0xc08, handle);
            word(
                &mut m,
                0x400000 + (handle >> 22) * 0x2e8 + 0x2c,
                if valid[i] { handle } else { handle ^ 1 },
            );
            word(&mut m, p + 0xc, s.players[i].flags);
            word(
                &mut m,
                p + 8,
                if camera.is_some() {
                    0x350000 + i as u32 * 0x1000
                } else {
                    0
                },
            );
            if let Some(f) = camera {
                word(&mut m, 0x350000 + i as u32 * 0x1000 + 0x21c, f);
            }
        }
        word(&mut m, STACK + 4, EXT);
        word(&mut m, EXT, selected);
        word(&mut m, EXT + 8, 0x300000 + selected * 0x1000);
        let mut calls = Calls {
            m: m.clone(),
            count: 0,
            remap: 0,
            events: vec![],
        };
        let mut r = regs();
        tsfp_ports::c2r::gameplay_player::game_players_validate_0xc08_handles_and_set_split_screen_camera_viewport_for_selected_play(&mut m,&mut calls,&mut r);
        if mode == 0xa && count > 1 {
            pair_branches[(selected != 0) as usize] = true;
        }
        saw_attach |= calls.events.iter().any(|e| e.0 == 0x17c740);
        saw_stale |= (0..count).any(|i| s.players[i].pending_handle != 0 && !valid[i]);
        let mut e = Effects {
            valid,
            events: vec![],
        };
        s.select(selected, 641, 481, &mut e);
        assert_eq!(calls.events, e.events);
        assert_eq!(s.selected_mask, read(&m, 0x7330d8));
        for i in 0..4 {
            assert_eq!(
                s.players[i].pending_handle,
                read(&m, 0x330000 + i as u32 * 0x2000 + 0xc08)
            );
            assert_eq!(
                s.players[i].flags,
                read(&m, PLAYER + i as u32 * 0x1584 + 0xc)
            );
            if let Some(f) = s.players[i].camera_flags {
                assert_eq!(f, read(&m, 0x350000 + i as u32 * 0x1000 + 0x21c));
            }
        }
    }
    assert!(pair_branches.into_iter().all(|v| v));
    assert!(saw_attach && saw_stale);
}
#[test]
fn reference_default() {
    assert_eq!(
        Preset::default().run(|| 17, || panic!("readable enabled")),
        17
    );
    assert_eq!(
        Preset::ReadableOriginal.run(|| panic!("reference selected"), || 29),
        29
    );
    assert_eq!(Preset::default(), Preset::Reference);
    assert_eq!(Switches::default().pair, Preset::Reference);
    assert_eq!(Switches::default().layout, Preset::Reference);
}

struct Setup {
    count: u32,
    remap: u32,
    shift: f32,
    events: Vec<(u32, Vec<u32>)>,
    fov: Option<(u32, u32)>,
}
impl ViewportEffects for Setup {
    fn remap_slot(&mut self, s: u32) -> u32 {
        self.events.push((0x356800, vec![s]));
        self.remap
    }
    fn edge_shift(&mut self) -> f32 {
        self.shift
    }
    fn shape(&mut self, x: f32, y: f32, z: f32) {
        self.events
            .push((0x55e40, vec![EXT, x.to_bits(), y.to_bits(), z.to_bits()]));
    }
    fn layout_count(&mut self) -> u32 {
        self.events.push((0x1cdae0, vec![]));
        self.count
    }
    fn publish(&mut self, r: Rect) {
        self.events.push((
            0x55d90,
            vec![
                EXT,
                r.left as u32,
                r.top as u32,
                r.right as u32,
                r.bottom as u32,
            ],
        ));
    }
    fn camera_fov(&mut self, p: u32, b: u32) {
        self.fov = Some((p, b));
    }
}
