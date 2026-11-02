//! Tier 1 against existing mechanical Rust ports; FABRICATED inputs and stub callbacks.
#![allow(dead_code, unexpected_cfgs)]
#[path = "../../tsfp-ports/src/abi.rs"]
mod abi;
#[path = "../../tsfp-ports/src/memory.rs"]
mod memory;
#[path = "../../tsfp-ports/src/c2r/ui_settings.rs"]
mod original;
#[path = "../../tsfp-ports/src/c2r/rt.rs"]
mod rt;
// The full state_wire includes unrelated systems; only this accessor is used by this port module.
mod state_wire {
    pub fn get_level(m: &impl crate::memory::GuestMemory) -> u32 {
        m.read32(0x79094c)
    }
}
use abi::{ExtRegisters, GuestCall};
use memory::GuestMemory;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use tsfp_format_profile::{Profile, settings::*};
#[derive(Clone, Default)]
struct Mem(Rc<RefCell<BTreeMap<u32, u8>>>);
impl GuestMemory for Mem {
    fn read8(&self, a: u32) -> u8 {
        *self.0.borrow().get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.borrow_mut().insert(a, v);
    }
}
struct Host(Mem);
impl GuestCall for Host {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        r.eax = match va {
            0x71940 => 0x20000 + self.0.read32(r.esp + 4) * 0x100,
            0x69640 => 0x30080,
            0x79c00 => 0,
            _ => panic!("unexpected callee {va:x}"),
        };
    }
}
fn random(s: &mut u32) -> u32 {
    *s ^= *s << 13;
    *s ^= *s >> 17;
    *s ^= *s << 5;
    *s
}
fn regs() -> ExtRegisters {
    ExtRegisters {
        esp: 0x10000,
        esi: 0x40000,
        eax: 0,
        ecx: 0,
        edx: 0,
        ebx: 0,
        ebp: 0,
        edi: 0,
        eflags: 0,
        fpu: abi::Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0x27f,
        },
        xmm: [abi::Xmm::default(); 5],
        xmm_hi: [abi::Xmm::default(); 3],
    }
}
#[test]
fn controls_against_load_store_ports_4096_seeds() {
    let mut seed = 2196;
    for _ in 0..4096 {
        let mut head = [0u8; HEAD_SIZE];
        for b in &mut head {
            *b = random(&mut seed) as u8;
        }
        let mut s = Settings::from_head(&head).unwrap();
        let c = Controls {
            crosshair: [Crosshair::Off, Crosshair::Moving, Crosshair::Fixed]
                [random(&mut seed) as usize % 3],
            weapon_change: [
                WeaponChange::Always,
                WeaponChange::Never,
                WeaponChange::Best,
                WeaponChange::IfNew,
                WeaponChange::IfNewAndBest,
            ][random(&mut seed) as usize % 5],
            inverse_look: random(&mut seed) & 1 != 0,
            auto_aim: random(&mut seed) & 2 != 0,
            lookahead: random(&mut seed) & 4 != 0,
            aim_toggle: random(&mut seed) & 8 != 0,
            crouch_toggle: random(&mut seed) & 16 != 0,
            vibration: [
                Vibration::Off,
                Vibration::Fire,
                Vibration::Hit,
                Vibration::FireAndHit,
            ][random(&mut seed) as usize % 4],
        };
        let mut m = Mem::default();
        let vp = random(&mut seed) % 4;
        m.write32(0x7ba9e0, vp);
        let base = 0x7ba364 + vp * 0x1e0;
        for (i, b) in head.iter().enumerate() {
            m.write8(base + i as u32, *b);
        }
        for (id, value) in [
            (1, c.crosshair as u32),
            (2, u32::from(c.auto_aim)),
            (3, u32::from(c.inverse_look)),
            (4, u32::from(c.lookahead)),
            (5, u32::from(!c.aim_toggle)),
            (6, c.weapon_change as u32),
            (7, u32::from(!c.crouch_toggle)),
            (8, c.vibration as u32),
        ] {
            m.write32(0x200c8 + id * 0x100, value);
        }
        let mut host = Host(m.clone());
        original::game_player_options_menu_store_item_values_into_player_block(
            &mut m,
            &mut host,
            &mut regs(),
        );
        s.set_controls(c);
        let actual: Vec<_> = (0..HEAD_SIZE).map(|i| m.read8(base + i as u32)).collect();
        assert_eq!(actual, s.head());
        assert_eq!(s.controls().unwrap(), c);
        for id in 1..=8 {
            m.write32(0x200c8 + id * 0x100, 0xdeadbeef);
        }
        original::game_player_options_menu_load_item_values_from_player_block(
            &mut m,
            &mut host,
            &mut regs(),
        );
        for (id, value) in [
            (1, c.crosshair as u32),
            (2, u32::from(c.auto_aim)),
            (3, u32::from(c.inverse_look)),
            (4, u32::from(c.lookahead)),
            (5, u32::from(!c.aim_toggle)),
            (6, c.weapon_change as u32),
            (7, u32::from(!c.crouch_toggle)),
            (8, c.vibration as u32),
        ] {
            assert_eq!(m.read32(0x200c8 + id * 0x100), value);
        }
    }
}
#[test]
fn audio_accept_against_port_4096_seeds() {
    let mut seed = 1234;
    for _ in 0..4096 {
        let mut payload = vec![0; 0x1f54];
        for b in &mut payload {
            *b = random(&mut seed) as u8;
        }
        let a = AudioVideo {
            music_enabled: random(&mut seed),
            sound_enabled: random(&mut seed),
            music_volume: random(&mut seed) % 17,
            sound_volume: random(&mut seed) % 17,
            slider_4: random(&mut seed) % 65,
            option_5: random(&mut seed),
        };
        let mut m = Mem::default();
        for (i, b) in payload.iter().enumerate() {
            m.write8(0x30000 + i as u32, *b);
        }
        m.write32(0x79094c, 0x65);
        for (addr, value) in [
            (0x5236b4, a.music_enabled),
            (0x5236b8, a.sound_enabled),
            (0x5236bc, a.music_volume),
            (0x5236c0, a.sound_volume),
            (0x7613d8, a.slider_4),
            (0x7613dc, a.option_5),
        ] {
            m.write32(addr, value);
        }
        let mut host = Host(m.clone());
        original::game_menu_audio_video_accept_item_handler_save_settings_to_profile_and_pop_context(&mut m,&mut host,&mut regs());
        let mut p = Profile {
            payload,
            signature: [0; 20],
        };
        p.accept_audio_video(a).unwrap();
        assert_eq!(p.audio_video().unwrap(), a);
        assert_eq!(
            p.payload,
            (0..0x1f54)
                .map(|i| m.read8(0x30000 + i))
                .collect::<Vec<_>>()
        );
    }
}

struct MappingHost {
    memory: Mem,
    bank: u32,
}
impl GuestCall for MappingHost {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        match va {
            0x71940 => r.eax = 0x20000 + self.memory.read32(r.esp + 4) * 0x100,
            0x715d0 => r.eax = self.bank,
            0x2e86a0 => {
                for i in (0..6).chain([13]) {
                    self.memory.write32(0x7ba3e0 + i * 4, 0);
                }
                for i in 0..8 {
                    if self.memory.read32(0x5249a0 + i * 12) != u32::MAX {
                        self.memory.write32(0x7ba3e0 + (14 + i) * 4, 0);
                    }
                }
            }
            0x2e8760 | 0x2e87b0 => {
                let mut memory = self.memory.clone();
                r.esp -= 4; // synthetic cdecl return slot for a nested root
                memory.write32(r.esp, 0);
                if va == 0x2e8760 {
                    original::game_input_mapping_menu_store_binding_by_row_selection(
                        &mut memory,
                        self,
                        r,
                    );
                } else {
                    original::game_input_mapping_menu_store_axis_pair_binding_by_selection(
                        &mut memory,
                        self,
                        r,
                    );
                }
                r.esp += 4;
            }
            _ => panic!("unexpected mapping callee {va:x}"),
        }
    }
}
#[test]
fn mapping_reconstruction_against_mechanical_ports_4096_seeds() {
    use tsfp_format_profile::mapping::{MappingEditor, MappingTables};
    let mut seed = 0x2196;
    for n in 0..4096 {
        let tables = MappingTables {
            digital: core::array::from_fn(|_| random(&mut seed) % 8),
            axes: core::array::from_fn(|_| {
                core::array::from_fn(|_| {
                    let value = random(&mut seed) % 9;
                    if value == 8 { u32::MAX } else { value }
                })
            }),
            pairs: core::array::from_fn(|_| core::array::from_fn(|_| random(&mut seed) % 8)),
        };
        let editor = MappingEditor {
            bank: n % 2,
            selections: core::array::from_fn(|_| random(&mut seed) % 8),
        };
        let before: [u32; 22] = core::array::from_fn(|_| random(&mut seed));
        let mut m = Mem::default();
        for (i, value) in before.iter().enumerate() {
            m.write32(0x7ba3e0 + i as u32 * 4, *value);
        }
        for (i, value) in tables.digital.iter().enumerate() {
            m.write32(0x761bf0 + i as u32 * 4, *value);
        }
        for (base, values) in [
            (0x5249a0, tables.axes.as_slice()),
            (0x5248f8, tables.pairs.as_slice()),
        ] {
            for (i, triple) in values.iter().enumerate() {
                for (j, value) in triple.iter().enumerate() {
                    m.write32(base + (i * 12 + j * 4) as u32, *value);
                }
            }
        }
        for (i, value) in editor.selections.iter().enumerate() {
            m.write32(0x20000 + i as u32 * 0x100 + 0xc8, *value);
        }
        let mut host = MappingHost {
            memory: m.clone(),
            bank: editor.bank as u32,
        };
        original::game_input_mapping_menu_commit_row_selections_to_player_block(
            &mut m,
            &mut host,
            &mut regs(),
        );
        let original: [u32; 22] = core::array::from_fn(|i| m.read32(0x7ba3e0 + i as u32 * 4));
        assert_eq!(
            editor.reconstruct(&tables, before).unwrap(),
            original,
            "seed {n}"
        );
    }
}
