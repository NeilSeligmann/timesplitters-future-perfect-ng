//! T2191 Tier 1: `PauseMenu::original` against the c2r translation of 0x2F5520 over the full
//! grid of its inputs. FABRICATED guest state (synthetic memory, scripted callee results);
//! the callees themselves stay stubs, so this checks the decision logic, not the widgets.
use std::collections::HashMap;

#[path = "support/pause_memory.rs"]
mod pause_memory;
use pause_memory::Mem;

use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::c2r::ui_menu_4::game_ingame_menu_build_pause_item_list_text_ids_0x179d_to_0x17ad_with_session_gates as lifted;
use tsfp_ports::memory::GuestMemory;
use tsfp_state::game_state::{FLAGS, LEVEL, MODE};
use tsfp_ui_model::pause::{PauseInputs, PauseItem, PauseMenu, PauseTail};

const TEXT_TAG: u32 = 0x8000_0000;

#[derive(Default)]
struct Guest {
    mem: Mem,
    results: HashMap<u32, u32>,
    items: Vec<PauseItem>,
    header: Option<u32>,
    widget_flags: Option<u32>,
    flushed: bool,
    centred: bool,
}

impl GuestCall for Guest {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        let g = self;
        let mem = g.mem.clone();
        let arg = |i: u32| mem.read32(r.esp + 4 * i);
        match va {
            0x3d7b0 => r.eax = TEXT_TAG | arg(0),
            0x79d60 => {
                let text = arg(0);
                assert_eq!(arg(1), 0xffff_ffff);
                g.items.push(PauseItem {
                    text: (text != 0).then(|| text & !TEXT_TAG),
                    handler: arg(4),
                    state: arg(2) != 0,
                    metric_bits: arg(3),
                    value: arg(5),
                    enabled: None,
                });
            }
            0x71630 => r.eax = arg(0),
            0x71c70 => {
                g.items.last_mut().expect("enable after an item").enabled = Some(arg(0) != 0)
            }
            0x75b70 => g.header = Some(arg(2)),
            0x71c50 => g.widget_flags = Some(arg(0)),
            0x771d0 => g.flushed = true,
            0x77140 => g.centred = true,
            _ => r.eax = *g.results.get(&va).unwrap_or(&0),
        }
    }
}

const STACK: u32 = 0x9000;
const PAGE: u32 = 0x2_0000;
const PLAYER: u32 = 0x3_0000;
const STEP: f32 = 4.0;
const START_Y: f32 = 100.0;

fn run_lifted(inp: &PauseInputs) -> PauseMenu {
    let mut m = Mem::default();
    m.write32(LEVEL, inp.level as u32);
    m.write8(MODE, inp.mode);
    m.write32(FLAGS, inp.flags);
    m.write32(
        0x7894fc,
        if inp.third_item_enabled {
            1
        } else {
            0xffff_ffff
        },
    );
    m.write32(0x789640, if inp.items_enabled { 2 } else { 0 });
    m.write32(0x475c78, STEP.to_bits());
    m.write32(PAGE + 0x38, inp.local_slot);
    m.write32(PAGE + 0x60, START_Y.to_bits());
    m.write32(0x761c48 + 4 * inp.local_slot, PLAYER);
    m.write32(STACK + 4, PAGE);
    let mut g = Guest {
        mem: m.clone(),
        ..Default::default()
    };
    g.results.insert(0x1cdae0, inp.player_count as u32);
    g.results.insert(0x3533f0, inp.session_active as u32);
    g.results.insert(0x352880, inp.host_side as u32);
    g.results.insert(0x35e960, inp.live_mode as u32);
    g.results.insert(0x25ee60, inp.slot_record_present as u32);
    let mut r = ExtRegisters {
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
            stack: [0.0; 8],
            top: 0,
            control: 0x27f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    };
    lifted(&mut m, &mut g, &mut r);
    assert_eq!(r.esp, STACK, "balanced stack");
    assert_eq!(m.read32(PLAYER + 0x264), 1, "slot record marked");
    let y = f32::from_bits(m.read32(PAGE + 0x60));
    let tail = if g.flushed {
        assert!(!g.centred);
        PauseTail::FlushCurrent
    } else {
        assert!(g.centred);
        PauseTail::Centre {
            shift_up: y != START_Y,
        }
    };
    let header = g.header.expect("header");
    PauseMenu {
        tall: header == 80.0f32.to_bits(),
        header_metric_bits: header,
        widget_flags: g.widget_flags.expect("flags"),
        items: g.items,
        tail,
    }
}

#[test]
fn original_preset_matches_lifted_over_input_grid() {
    let mut cases = 0;
    for level in [3, 100, 101] {
        for player_count in 0..=5 {
            for local_slot in 0..4 {
                for mode in [0, 0x0a] {
                    for flags in [0, 0x0400_0000, 0x1000_0000, 0x1400_0000] {
                        for bits in 0u32..64 {
                            let inp = PauseInputs {
                                level,
                                player_count,
                                local_slot,
                                mode,
                                flags,
                                session_active: bits & 1 != 0,
                                host_side: bits & 2 != 0,
                                live_mode: bits & 4 != 0,
                                slot_record_present: bits & 8 != 0,
                                third_item_enabled: bits & 16 != 0,
                                items_enabled: bits & 32 != 0,
                            };
                            let model = PauseMenu::original(&inp);
                            assert!(model.items.len() >= 3);
                            assert_eq!(model, run_lifted(&inp), "{inp:?}");
                            cases += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases, 3 * 6 * 4 * 2 * 4 * 64);
}
