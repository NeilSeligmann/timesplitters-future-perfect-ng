// SPDX-License-Identifier: GPL-3.0-or-later
//! T2378 Tier 1 differential: terminal focus, physical input and script transitions against
//! mechanically lifted C2R roots, using fabricated valid state.
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    c2r::{data_missions_3, engine_input, t2378_terminal_reference},
    memory::GuestMemory as PortMemory,
    pose_leaf,
};
use tsfp_state::{GuestMemory, terminals::*};

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<BTreeMap<u32, u8>>>);
impl PortMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.borrow().get(&address).copied().unwrap_or(0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
    }
}
impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        PortMemory::read8(self, address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        PortMemory::write8(self, address, value)
    }
}
fn put32(memory: &mut Memory, address: u32, value: u32) {
    for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
        PortMemory::write8(memory, address + i as u32, byte);
    }
}
fn get32(memory: &Memory, address: u32) -> u32 {
    u32::from_le_bytes(core::array::from_fn(|i| {
        PortMemory::read8(memory, address + i as u32)
    }))
}
fn registers(stack: u32) -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: stack,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.; 8],
            top: 0,
            control: 0x27f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

struct RankCall(Memory);
impl GuestCall for RankCall {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        assert_eq!(
            va, 0x36e3a0,
            "only the documented focus delegate is expected"
        );
        let caller_stack = regs.esp;
        regs.esp = regs.esp.wrapping_sub(4);
        put32(&mut self.0, regs.esp, 0xfeed_c0de); // CALL return slot
        data_missions_3::game_terminal_bring_widget_to_front_reorder_layers_and_set_focus(
            &mut self.0,
            regs,
        );
        regs.esp = caller_stack; // callee RET; caller cleans its two arguments
    }
}

struct ScriptCall(Memory);
impl GuestCall for ScriptCall {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        assert_eq!(va, 0x371ed0, "only the documented script move helper is expected");
        let caller_stack = regs.esp;
        regs.esp = regs.esp.wrapping_sub(4);
        put32(&mut self.0, regs.esp, 0xfeed_c0de);
        t2378_terminal_reference::sub_00371ed0(&mut self.0, regs);
        regs.esp = caller_stack;
    }
}

fn encode_script_record(memory: &mut Memory, address: u32, record: ScriptRecord) {
    match record {
        ScriptRecord::Wait { duration } => {
            put32(memory, address, 0);
            put32(memory, address + 4, duration.to_bits());
        }
        ScriptRecord::MoveTo { x, y } => {
            put32(memory, address, 1);
            put32(memory, address + 4, x.to_bits());
            put32(memory, address + 8, y.to_bits());
        }
        ScriptRecord::Hold { duration } => {
            put32(memory, address, 2);
            put32(memory, address + 4, duration.to_bits());
        }
        ScriptRecord::End => put32(memory, address, 3),
    }
}

fn assert_script_matches_lifted(
    records: Vec<ScriptRecord>,
    initial_pointer: PointerPosition,
    dt: f32,
    rate: i32,
    ticks: usize,
) {
    let object = 0x10000;
    let parent = 0x11000;
    let record_base = 0x12000;
    let held_out = 0x13000;
    let mut memory = Memory::default();
    put32(&mut memory, object + 0x7c, parent);
    put32(&mut memory, parent + 0x58, record_base);
    put32(&mut memory, parent + 0x38, initial_pointer.x.to_bits());
    put32(&mut memory, parent + 0x3c, initial_pointer.y.to_bits());
    put32(&mut memory, 0x4e8cec, rate as u32);
    put32(&mut memory, 0x4e794c, dt.to_bits());
    put32(&mut memory, 0x4785c8, 100.0f32.to_bits());
    put32(&mut memory, 0x4785e4, 0.2f32.to_bits());
    put32(&mut memory, 0x476240, 0.0001f32.to_bits());
    put32(&mut memory, 0x475d10, 2.0f32.to_bits());
    for (index, target) in [0x372023, 0x37202b, 0x37200c, 0x372050].into_iter().enumerate() {
        put32(&mut memory, 0x372060 + index as u32 * 4, target);
    }
    for (index, &record) in records.iter().enumerate() {
        encode_script_record(&mut memory, record_base + index as u32 * 12, record);
    }

    let mut state = ScriptState { records, cursor: 0, elapsed: 0.0 };
    let mut pointer = initial_pointer;
    for tick in 0..ticks {
        let active_at_entry = state.active();
        put32(&mut memory, held_out, 0);
        let mut regs = registers(0x70000);
        put32(&mut memory, regs.esp + 4, object);
        put32(&mut memory, regs.esp + 8, held_out);
        let mut guest = ScriptCall(memory.clone());
        t2378_terminal_reference::sub_00371fd0(&mut memory, &mut guest, &mut regs);

        let rust_result = state.tick(&mut pointer, dt, rate);
        assert_eq!(regs.eax, u32::from(active_at_entry), "script return tick {tick}");
        let original_cursor = get32(&memory, parent + 0x58);
        if original_cursor == 0 {
            assert!(!state.active(), "cleared cursor tick {tick}");
        } else {
            assert_eq!(
                state.cursor as u32,
                original_cursor.wrapping_sub(record_base) / 12,
                "cursor tick {tick}"
            );
        }
        assert_eq!(state.elapsed.to_bits(), get32(&memory, parent + 0x5c), "elapsed tick {tick}");
        let original_pointer = PointerPosition {
            x: f32::from_bits(get32(&memory, parent + 0x38)),
            y: f32::from_bits(get32(&memory, parent + 0x3c)),
        };
        assert!((pointer.x - original_pointer.x).abs() <= 0.001, "pointer x tick {tick}: rust={} lifted={}", pointer.x, original_pointer.x);
        assert!((pointer.y - original_pointer.y).abs() <= 0.001, "pointer y tick {tick}: rust={} lifted={}", pointer.y, original_pointer.y);
        let original_held = get32(&memory, held_out) != 0;
        assert_eq!(rust_result.override_input.map(|input| input.held).unwrap_or(false), original_held, "held tick {tick}");
        assert_eq!(rust_result.override_input.is_some(), active_at_entry, "override lifetime tick {tick}");
    }
}

fn terminal_fixture(ranks: &[i32], flags: &[u32], children: &[usize]) -> (Memory, TerminalState) {
    let mut memory = Memory::default();
    let context = 0x10000;
    let array = 0x11000;
    let screen_addr = 0x12000;
    let root = 0x13000;
    put32(&mut memory, context, root);
    put32(&mut memory, root + 0x54, 0.25f32.to_bits());
    put32(&mut memory, 0x76c274, context);
    put32(&mut memory, context + 0x0c, array);
    put32(&mut memory, context + 0x10, ranks.len() as u32);
    put32(&mut memory, screen_addr + 0x154, children.len() as u32);
    for (i, (&rank, &flag)) in ranks.iter().zip(flags).enumerate() {
        let addr = array + i as u32 * 0x4c;
        put32(&mut memory, addr + 0x18, rank as u32);
        put32(&mut memory, addr + 0x14, flag);
    }
    for (i, child) in children.iter().enumerate() {
        put32(
            &mut memory,
            screen_addr + 0x12c + i as u32 * 4,
            *child as u32,
        );
    }
    let widgets = ranks
        .iter()
        .zip(flags)
        .enumerate()
        .map(|(i, (&rank, &flags))| TerminalWidget {
            id: WidgetId(i),
            rank,
            flags,
        })
        .collect();
    let screen = TerminalScreen {
        id: ScreenId(0),
        children: children.iter().copied().map(WidgetId).collect(),
        pointer: PointerPosition { x: 0.0, y: 0.0 },
        timer: 0,
        previous_held: 0,
        script: ScriptState::default(),
    };
    (memory, TerminalState::new(vec![screen], widgets))
}

#[test]
fn rank_reorder_matches_lifted_root_for_seeded_valid_contexts() {
    let mut seed = 0x36e3_a0u32;
    for case in 0..2048 {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let count = 1 + (seed as usize % 32);
        let mut ranks = Vec::with_capacity(count);
        let mut flags = Vec::with_capacity(count);
        for _ in 0..count {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            ranks.push((seed as i32 % 41) - 20);
            flags.push(seed.rotate_left(13));
        }
        let selected = (seed.rotate_right(7) as usize) % count;
        let (mut memory, mut state) = terminal_fixture(&ranks, &flags, &[]);
        let mut regs = registers(0x70000);
        put32(&mut memory, regs.esp, 0xfeed0000);
        put32(&mut memory, regs.esp + 4, 0);
        put32(&mut memory, regs.esp + 8, selected as u32);
        data_missions_3::game_terminal_bring_widget_to_front_reorder_layers_and_set_focus(
            &mut memory,
            &mut regs,
        );
        state.bring_to_front(WidgetId(selected)).unwrap();
        assert_eq!(regs.esp, 0x70000, "ABI stack case {case}");
        for index in 0..count {
            assert_eq!(
                state.widgets[index].rank as u32,
                get32(&memory, 0x11000 + index as u32 * 0x4c + 0x18),
                "rank case {case}, widget {index}"
            );
        }
        assert_eq!(
            get32(&memory, 0x76c290),
            0x11000 + selected as u32 * 0x4c,
            "focus publication case {case}"
        );
    }
}

#[test]
fn topmost_child_and_rank_publication_match_lifted_root() {
    let cases: &[(&[i32], &[u32], &[usize])] = &[
        (&[0, 2, 5, 4], &[2, 2, 0, 2], &[0, 1, 2, 3]),
        (&[-3, -1, 8], &[2, 2, 2], &[0, 1, 2]),
        (&[7, 7, 6], &[2, 2, 2], &[1, 0, 2]),
        (&[4, 3], &[0, 0], &[0, 1]),
        (&[2, 8], &[2, 2], &[]),
    ];
    for (case, &(ranks, flags, children)) in cases.iter().enumerate() {
        let (mut memory, mut state) = terminal_fixture(ranks, flags, children);
        state.focus = Some(WidgetId(0));
        put32(&mut memory, 0x76c290, 0x11000);
        let mut regs = registers(0x70000);
        put32(&mut memory, regs.esp, 0xfeed0000);
        put32(&mut memory, regs.esp + 4, 0x12000);
        let mut guest = RankCall(memory.clone());
        data_missions_3::game_terminal_focus_topmost_interactive_child_widget_of_screen(
            &mut memory,
            &mut guest,
            &mut regs,
        );
        let selected = state.focus_topmost(ScreenId(0)).unwrap();
        let mut expected = None;
        let mut best_rank = -1;
        for &index in children {
            if flags[index] & 2 != 0 && ranks[index] > best_rank {
                expected = Some(index);
                best_rank = ranks[index];
            }
        }
        assert_eq!(selected.map(|id| id.0), expected);
        assert_eq!(
            get32(&memory, 0x76c290),
            state
                .focus
                .map_or(0x11000, |id| 0x11000 + id.0 as u32 * 0x4c),
            "focus case {case}"
        );
        for (index, widget) in state.widgets.iter().enumerate() {
            assert_eq!(
                widget.rank as u32,
                get32(&memory, 0x11000 + index as u32 * 0x4c + 0x18),
                "rank case {case}, widget {index}"
            );
        }
        assert_eq!(regs.esp, 0x70000);
    }
}

#[test]
fn activation_keeps_reorder_effects_on_both_return_arms() {
    for already_active in [false, true] {
        let flags = if already_active { 2 } else { 0 };
        let (mut memory, mut state) = terminal_fixture(&[3, 1], &[0, flags], &[1]);
        put32(&mut memory, 0x4e8cec, 100);
        put32(&mut memory, 0x12000 + 0xb4, 1);
        put32(&mut memory, 0x76c298, 0xaaaa0001);
        put32(&mut memory, 0x76c28c, 0xaaaa0002);
        put32(&mut memory, 0x76c294, 0xaaaa0003);
        state.current_parent = Some(ScreenId(99));
        state.current_widget = Some(WidgetId(99));
        state.current_timer = -7;
        let mut regs = registers(0x70000);
        put32(&mut memory, regs.esp, 0xfeed0000);
        put32(&mut memory, regs.esp + 4, 0x12000);
        put32(&mut memory, regs.esp + 8, 0);
        let mut guest = RankCall(memory.clone());
        data_missions_3::game_terminal_activate_widget_and_set_focus(
            &mut memory,
            &mut guest,
            &mut regs,
        );
        let activated = state.activate_child(ScreenId(0), 0, 25).unwrap();
        assert_eq!(activated, !already_active);
        assert_eq!(regs.eax, u32::from(!already_active));
        assert_eq!(get32(&memory, 0x12000 + 0x50), 25);
        assert_eq!(state.screens[0].timer, 25);
        assert_eq!(get32(&memory, 0x11000 + 0x4c + 0x14), 2);
        assert_eq!(state.widgets[1].flags, 2);
        assert_eq!(get32(&memory, 0x11000 + 0x4c + 0x18), 1);
        assert_eq!(state.widgets[1].rank, 1);
        if already_active {
            assert_eq!(state.current_parent, Some(ScreenId(99)));
            assert_eq!(state.current_widget, Some(WidgetId(99)));
            assert_eq!(state.current_timer, -7);
            assert_eq!(get32(&memory, 0x76c298), 0xaaaa0001);
            assert_eq!(get32(&memory, 0x76c28c), 0xaaaa0002);
            assert_eq!(get32(&memory, 0x76c294), 0xaaaa0003);
        } else {
            assert_eq!(state.current_parent, Some(ScreenId(0)));
            assert_eq!(state.current_widget, Some(WidgetId(1)));
            assert_eq!(state.current_timer, 25);
            assert_eq!(get32(&memory, 0x76c298), 0x12000);
            assert_eq!(get32(&memory, 0x76c28c), 0x11000 + 0x4c);
            assert_eq!(get32(&memory, 0x76c294), 25);
        }
        assert_eq!(regs.esp, 0x70000);
    }
}

#[test]
fn physical_axis_normalization_matches_lifted_root_for_all_byte_values() {
    for raw in 0..=255u32 {
        let mut memory = Memory::default();
        put32(&mut memory, 0x6b9888, 0);
        put32(&mut memory, 0x76b0fc, 0);
        put32(&mut memory, 0x6b93c0, 0);
        put32(&mut memory, 0x6b93d0, 0.25f32.to_bits());
        put32(&mut memory, 0x6b93d0 + 0x1c, raw);
        put32(&mut memory, 0x4783a8, (1.0f32 / 128.0).to_bits());
        put32(&mut memory, 0x476038, (1.0f32 / 127.0).to_bits());
        put32(&mut memory, 0x475c78, 1.0f32.to_bits());
        let mut regs = registers(0x70000);
        put32(&mut memory, regs.esp, 0xfeed0000);
        put32(&mut memory, regs.esp + 4, 0);
        put32(&mut memory, regs.esp + 8, 1);
        engine_input::game_input_pad_axis_get_float_with_deadzone(&mut memory, &mut regs);
        let expected = regs.fpu.st(0) as f32;
        assert_eq!(
            normalize_axis(raw as i32, 0.25).to_bits(),
            expected.to_bits(),
            "raw={raw}, lifted={expected}, record={}, slot={}, axis={}",
            get32(&memory, 0x6b93d0 + 0x1c),
            get32(&memory, 0x70004),
            get32(&memory, 0x70008),
        );
    }
}

#[test]
fn physical_pad_sampling_matches_lifted_gate_selector_and_remap() {
    let mut seed = 0x42970u32;
    for case in 0..2048 {
        let mut next = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            seed
        };
        let blocked = next() & 1 != 0;
        let selector = if next() & 1 == 0 { 0 } else { next() | 1 };
        let slot = (next() % 4) as usize;
        let ports: Vec<usize> = (0..4).map(|_| (next() % 4) as usize).collect();
        let mut memory = Memory::default();
        let mut physical_records = Vec::new();
        let mut effective_records = Vec::new();
        for bank in 0..2 {
            for index in 0..4 {
                let dead_zone = [0.0, 0.1, 0.25, 0.5][(next() % 4) as usize];
                let record = TerminalPadRecord {
                    held: next(),
                    consumed: next(),
                    dead_zone,
                    axes: [(next() % 256) as i32, (next() % 256) as i32, (next() % 256) as i32],
                };
                let address = if bank == 0 {
                    0x6b93d0 + index as u32 * 0x3c
                } else {
                    0x6b94c8 + index as u32 * 0x3c
                };
                put32(&mut memory, address, dead_zone.to_bits());
                put32(&mut memory, address + 0x0c, record.held);
                put32(&mut memory, address + 0x18, record.axes[0] as u32);
                put32(&mut memory, address + 0x1c, record.axes[1] as u32);
                put32(&mut memory, address + 0x20, record.axes[2] as u32);
                put32(&mut memory, address + 0x38, record.consumed);
                if bank == 0 {
                    physical_records.push(record);
                } else {
                    effective_records.push(record);
                }
            }
        }
        for (index, port) in ports.iter().enumerate() {
            put32(&mut memory, 0x6b93c0 + index as u32 * 4, *port as u32);
        }
        put32(&mut memory, 0x6b9888, u32::from(blocked));
        put32(&mut memory, 0x76b0fc, selector);
        put32(&mut memory, 0x4783a8, (1.0f32 / 128.0).to_bits());
        put32(&mut memory, 0x476038, (1.0f32 / 127.0).to_bits());
        put32(&mut memory, 0x475c78, 1.0f32.to_bits());

        let bank = TerminalPadBank {
            blocked,
            selector,
            physical_port_by_slot: ports,
            physical_records,
            effective_records,
        };
        let expected = sample_terminal_pad(&bank, slot, 0xa5a5_5a5a).unwrap();
        let mut regs = registers(0x70000);
        put32(&mut memory, regs.esp, 0xfeed0000);
        put32(&mut memory, regs.esp + 4, slot as u32);
        pose_leaf::input_pad_gate_zero_when_6b9888_set(&mut memory, &mut regs);
        assert_eq!(expected.held_after_consumption, regs.eax, "held sample case {case}");
        assert_eq!(expected.activation_mask, 0xa5a5_5a5a);
        for axis in 1..=2 {
            let mut axis_regs = registers(0x71000);
            put32(&mut memory, axis_regs.esp, 0xfeed0000);
            put32(&mut memory, axis_regs.esp + 4, slot as u32);
            put32(&mut memory, axis_regs.esp + 8, axis);
            engine_input::game_input_pad_axis_get_float_with_deadzone(
                &mut memory,
                &mut axis_regs,
            );
            let rust_axis = if axis == 1 { expected.left_x } else { expected.left_y };
            assert_eq!(rust_axis.to_bits(), (axis_regs.fpu.st(0) as f32).to_bits(), "axis {axis}, case {case}");
        }
    }
}

#[test]
fn script_cursor_preserves_override_tick_and_physical_handoff() {
    let mut state = ScriptState {
        records: vec![ScriptRecord::Hold { duration: 0.5 }, ScriptRecord::End],
        cursor: 0,
        elapsed: 0.0,
    };
    let mut pointer = PointerPosition { x: 32.0, y: 24.0 };
    let held = state.tick(&mut pointer, 0.25, 1);
    assert_eq!(held.override_input.unwrap().held, true);
    let expiry = state.tick(&mut pointer, 0.25, 1);
    assert_eq!(expiry.override_input.unwrap().held, true);
    assert!(state.active());
    let terminal_tick = state.tick(&mut pointer, 0.25, 1);
    assert_eq!(terminal_tick.override_input.unwrap().held, false);
    assert!(!state.active());
    assert_eq!(state.elapsed, 0.25);
    assert_eq!(state.tick(&mut pointer, 0.25, 1).override_input, None);
}

#[test]
fn script_records_match_lifted_interpreter_for_seeded_valid_states() {
    let mut seed = 0x371f_d0u32;
    for _case in 0..2048 {
        let mut next = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            seed
        };
        let pointer = PointerPosition {
            x: (next() % 256) as f32,
            y: (next() % 256) as f32,
        };
        let target = PointerPosition {
            x: (next() % 256) as f32,
            y: (next() % 256) as f32,
        };
        let dt = 0.01 + (next() % 30) as f32 / 1000.0;
        let rate = 1 + (next() % 60) as i32;
        assert_script_matches_lifted(
            vec![
                ScriptRecord::Wait { duration: 0.01 },
                ScriptRecord::Hold { duration: 0.02 },
                ScriptRecord::MoveTo { x: target.x, y: target.y },
                ScriptRecord::End,
            ],
            pointer,
            dt,
            rate,
            24,
        );
    }
}

#[test]
fn physical_mask_and_script_override_have_explicit_precedence() {
    let physical = PhysicalTerminalInput {
        held_after_consumption: 0x82,
        activation_mask: 0x82,
        left_x: 0.25,
        left_y: 0.5,
    };
    let matched = resolve_terminal_input(physical, false, None);
    assert!(matched.held);
    assert_eq!(matched.cursor_delta, [0.25, -0.5]);
    assert!(
        !resolve_terminal_input(
            PhysicalTerminalInput {
                held_after_consumption: 0x83,
                ..physical
            },
            false,
            None,
        )
        .held
    );
    assert!(!resolve_terminal_input(physical, true, None).held);
    let script = resolve_terminal_input(
        PhysicalTerminalInput {
            held_after_consumption: 0,
            activation_mask: 0,
            ..physical
        },
        true,
        Some(ScriptOverride {
            held: true,
            cursor_delta: [3.0, 4.0],
        }),
    );
    assert_eq!(script.source, InputSource::Script);
    assert!(script.held);
    assert_eq!(script.cursor_delta, [3.0, 4.0]);
}

#[test]
fn physical_bank_preserves_gate_selector_remap_and_consumed_bits() {
    let physical = TerminalPadRecord {
        held: 0x82,
        consumed: 0x80,
        dead_zone: 0.25,
        axes: [128, 255, 128],
    };
    let effective = TerminalPadRecord {
        axes: [128, 0, 255],
        ..physical
    };
    let bank = TerminalPadBank {
        blocked: false,
        selector: 0,
        physical_port_by_slot: vec![1, 0],
        physical_records: vec![physical, physical],
        effective_records: vec![effective, effective],
    };
    let sample = sample_terminal_pad(&bank, 0, 2).unwrap();
    assert_eq!(sample.held_after_consumption, 2);
    assert_eq!([sample.left_x, sample.left_y], [1.0, 0.0]);
    let effective_bank = TerminalPadBank {
        selector: 7,
        ..bank.clone()
    };
    let sample = sample_terminal_pad(&effective_bank, 0, 2).unwrap();
    assert_eq!([sample.left_x, sample.left_y], [-1.0, 1.0]);
    let blocked = TerminalPadBank {
        blocked: true,
        ..bank
    };
    let sample = sample_terminal_pad(&blocked, 99, 0).unwrap();
    assert_eq!(sample.held_after_consumption, 0);
    assert_eq!([sample.left_x, sample.left_y], [0.0, 0.0]);
    assert_eq!(
        sample_terminal_pad(&effective_bank, 2, 1),
        Err(PhysicalInputError::SlotOutOfRange(2))
    );
}

#[test]
fn script_move_updates_owned_pointer_and_keeps_failed_cursor() {
    let mut script = ScriptState {
        records: vec![ScriptRecord::MoveTo { x: 3.0, y: 4.0 }],
        cursor: 0,
        elapsed: 0.0,
    };
    let mut pointer = PointerPosition { x: 0.0, y: 0.0 };
    let result = script.tick(&mut pointer, 1.0, 1);
    assert_eq!(pointer, PointerPosition { x: 0.6, y: 0.8 });
    assert!(!result.move_completed);
    assert!(script.active());
    assert_eq!(result.override_input.unwrap().cursor_delta, [0.6, 0.8]);
}

#[test]
fn terminal_event_phase_uses_current_and_previous_held_values() {
    assert_eq!(terminal_event(false, 0), TerminalEvent::Idle);
    assert_eq!(terminal_event(false, 1), TerminalEvent::Release);
    assert_eq!(terminal_event(true, 1), TerminalEvent::Held);
    assert_eq!(terminal_event(true, 0), TerminalEvent::Press);
}
