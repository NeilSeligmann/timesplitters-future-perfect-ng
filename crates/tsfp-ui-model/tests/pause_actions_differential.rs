//! T2191 Tier 1 for the resume handler (0x2EBB80, the only pause handler with a c2r
//! translation) and a Tier 0 cross-check that every handler the list builder attaches maps to a
//! ported action. FABRICATED guest state; the widget push 0x79B70 is a recording stub.

#[path = "support/pause_memory.rs"]
mod pause_memory;
use pause_memory::Mem;

use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::c2r::ui_menu_4::game_ingame_menu_item_activate_open_child_widget_for_current_slot_and_set_flag_8 as lifted_resume;
use tsfp_ports::memory::GuestMemory;
use tsfp_ui_model::pause::{PauseInputs, PauseMenu};
use tsfp_ui_model::pause_actions::{PauseAction, PauseActionInputs, PauseActionState, PauseEffect};

/// Records 0x79B70 calls as (stack id, page).
struct Guest {
    mem: Mem,
    pushes: Vec<(u32, u32)>,
}
impl GuestCall for Guest {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        assert_eq!(va, 0x79b70, "resume handler calls only the widget push");
        self.pushes
            .push((self.mem.read32(r.esp), self.mem.read32(r.esp + 4)));
    }
}

const STACK: u32 = 0x9000;
const WIDGET: u32 = 0x3_0000;

fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: STACK,
        ebx: 0,
        ebp: 0,
        esi: 0x5151,
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

#[test]
fn resume_matches_c2r() {
    let mut cases = 0;
    for slot in 0..4u32 {
        for widget_flags in [0u32, 1, 8, 0x40, 0xffff_fff7] {
            let mut mem = Mem::default();
            let stack_id = 0x100 + slot;
            mem.write32(0x7ba9e0, slot);
            mem.write32(0x761bbc + 4 * slot, stack_id);
            mem.write32(0x761c48 + 4 * slot, WIDGET);
            mem.write32(WIDGET, widget_flags);
            mem.write32(WIDGET + 0xc, 1.5f32.to_bits());
            let mut guest = Guest {
                mem: mem.clone(),
                pushes: Vec::new(),
            };
            let mut r = regs();
            lifted_resume(&mut mem, &mut guest, &mut r);
            assert_eq!((r.esp, r.esi), (STACK, 0x5151), "balanced, esi restored");

            let effects = PauseActionState::default().activate(
                PauseAction::Resume,
                &PauseActionInputs {
                    current_slot: slot,
                    ..Default::default()
                },
            );
            let PauseEffect::PushPage { slot: s, page } = effects[0] else {
                panic!()
            };
            assert_eq!(guest.pushes, [(mem.read32(0x761bbc + 4 * s), page)]);
            assert_eq!(effects[1], PauseEffect::MarkPauseWidgetClosed { slot });
            // The effect's meaning, applied by the original.
            assert_eq!(mem.read32(WIDGET), widget_flags | 8);
            assert_eq!(mem.read32(WIDGET + 0xc), 0);
            cases += 1;
        }
    }
    assert_eq!(cases, 20);
}

#[test]
fn every_list_handler_is_ported() {
    let mut seen = std::collections::BTreeSet::new();
    for bits in 0u32..1 << 10 {
        let b = |i: u32| bits >> i & 1 != 0;
        let menu = PauseMenu::original(&PauseInputs {
            level: if b(0) { 3 } else { 101 },
            player_count: 1 + (bits >> 1 & 3) as i32,
            local_slot: if b(3) { 0 } else { 2 },
            mode: if b(4) { 0x0a } else { 0 },
            session_active: b(5),
            host_side: b(6),
            live_mode: b(7),
            slot_record_present: b(8),
            flags: if b(9) { 0 } else { 0x1000_0000 },
            ..Default::default()
        });
        for item in &menu.items {
            seen.insert(PauseAction::from_handler(item.handler).expect("ported handler"));
        }
    }
    assert_eq!(
        seen.len(),
        PauseAction::ALL.len(),
        "the grid reaches all eight handlers"
    );
}
