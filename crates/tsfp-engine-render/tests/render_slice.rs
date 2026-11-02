// SPDX-License-Identifier: GPL-3.0-or-later
//! T1898 Tier 0 smoke tests of the render slice over the recorded-call device. The Tier 1
//! differential against the lifted originals is tools/t1898_render_diff.py.
use tsfp_engine_render::{
    Renderer,
    device::{DeviceCall, HostFunction, RecordingDevice, RecordingHost},
    map::*,
    mem::{GuestAccess, SparseMemory},
    memory::GuestMemory,
    particle,
    state::{StateSlot, slot},
};

struct Rig {
    memory: SparseMemory,
    device: RecordingDevice,
    host: RecordingHost,
}

impl Rig {
    fn new() -> Self {
        let mut memory = SparseMemory::new();
        memory.write32(DIRTY_NEWEST, DIRTY_SENTINEL);
        memory.write32(DIRTY_OLDEST, DIRTY_SENTINEL);
        memory.write32(DIRTY_SENTINEL + 0x10, DIRTY_SENTINEL);
        memory.write32(DIRTY_SENTINEL + 0x14, DIRTY_SENTINEL);
        for index in 0..35u32 {
            memory.write32(StateSlot::address(index), index);
        }
        Self {
            memory,
            device: RecordingDevice::default(),
            host: RecordingHost::default(),
        }
    }
    fn renderer(&mut self) -> Renderer<'_, SparseMemory, RecordingDevice, RecordingHost> {
        Renderer::new(&mut self.memory, &mut self.device, &mut self.host)
    }
}

#[test]
fn slot_set_links_then_unlinks_on_the_committed_value() {
    let mut rig = Rig::new();
    let previous = rig.renderer().state_slot_set(3, 5);
    assert_eq!(previous, 0);
    let record = StateSlot::read(&rig.memory, StateSlot::address(3));
    assert_eq!((record.pending, record.committed), (5, 0));
    assert_eq!(
        (record.older, record.newer),
        (DIRTY_SENTINEL, DIRTY_SENTINEL)
    );
    assert_eq!(rig.memory.read32(DIRTY_NEWEST), StateSlot::address(3));
    assert!(rig.renderer().dirty_list_has_entries());
    rig.renderer().state_slot_set(3, 0);
    let record = StateSlot::read(&rig.memory, StateSlot::address(3));
    assert_eq!((record.older, record.newer), (0, 0));
    assert_eq!(rig.memory.read32(DIRTY_NEWEST), DIRTY_SENTINEL);
}

#[test]
fn apply_sends_newest_first_and_commits() {
    let mut rig = Rig::new();
    for index in 0..4u32 {
        rig.memory
            .write32(METHOD_HEADER_TABLE + 4 * index, 0x1000 + index);
    }
    rig.renderer().state_slot_set(1, 11);
    rig.renderer().state_slot_set(2, 22);
    assert_eq!(rig.renderer().state_apply(), DIRTY_SENTINEL);
    assert_eq!(
        rig.device.calls,
        vec![
            DeviceCall::PushMethod {
                header: 0x1002,
                value: 22
            },
            DeviceCall::PushMethod {
                header: 0x1001,
                value: 11
            },
        ]
    );
    assert_eq!(
        StateSlot::read(&rig.memory, StateSlot::address(1)).committed,
        11
    );
    assert_eq!(rig.memory.read32(DIRTY_NEWEST), DIRTY_SENTINEL);
    assert_eq!(rig.memory.read32(StateSlot::address(2) + 0x10), 0);
}

#[test]
fn apply_routes_state_index_classes() {
    let mut rig = Rig::new();
    rig.memory.write32(StateSlot::address(1), 0x60);
    rig.memory.write32(StateSlot::address(2), 0x90);
    rig.memory.write32(StateSlot::address(3), 0xA6);
    for (slot_index, value) in [(1, 7), (2, 8), (3, 9)] {
        rig.renderer().state_slot_set(slot_index, value);
    }
    rig.renderer().state_apply();
    assert_eq!(
        rig.device.calls,
        vec![
            DeviceCall::StateHandler {
                index: 0x90,
                value: 8
            },
            DeviceCall::DeferState {
                index: 0x60,
                value: 7
            }
        ]
    );
}

#[test]
fn state_group_runs_its_script_once() {
    let mut rig = Rig::new();
    let previous = rig.renderer().set_state_group(2);
    assert_eq!(previous, 0);
    assert_eq!(rig.memory.read32(STATE_GROUP), 2);
    let pending = |rig: &Rig, target: u32| rig.memory.read32(StateSlot::address(target) + 8);
    assert_eq!(pending(&rig, slot::DEPTH_FUNC), 0x203);
    assert_eq!(pending(&rig, slot::GROUP_RESET_A), 1);
    assert_eq!(pending(&rig, slot::STENCIL_ENABLE), 0);
    let before = rig.memory.read32(DIRTY_NEWEST);
    assert_eq!(rig.renderer().set_state_group(2), 2);
    assert_eq!(rig.memory.read32(DIRTY_NEWEST), before);
    assert_eq!(rig.renderer().set_state_group(u32::MAX), 2);
}

#[test]
fn blend_mode_script_and_keyed_colour_refresh() {
    let mut rig = Rig::new();
    rig.renderer().set_blend_mode(1);
    let pending = |rig: &Rig, target: u32| rig.memory.read32(StateSlot::address(target) + 8);
    assert_eq!(pending(&rig, slot::BLEND_SOURCE), 0x302);
    assert_eq!(pending(&rig, slot::BLEND_DESTINATION), 0x303);
    assert_eq!(pending(&rig, slot::BLEND_EQUATION), 0x8006);
    rig.memory.write32(BLEND_ALPHA, 0x80);
    rig.renderer().set_blend_mode(0x18);
    assert_eq!(rig.memory.read32(BLEND_COLOUR_CACHE), 0x7F7F_7F7F);
    assert_eq!(pending(&rig, slot::BLEND_COLOUR), 0xFE00_0000);
    assert_eq!(rig.memory.read32(BLEND_MODE), 0x18);
}

#[test]
fn draw_dispatch_draws_a_vertex_run_and_aligns_the_ring() {
    let mut rig = Rig::new();
    rig.memory.write32(VERTEX_RUN_START, 4);
    rig.memory.write32(VERTEX_RUN_END, 10);
    rig.memory.write32(BATCH_PRIMITIVE, 5);
    rig.memory.write32(VERTEX_RING_BASE + 4, 0x45);
    rig.renderer().draw_dispatch();
    assert_eq!(
        rig.device.calls,
        vec![
            DeviceCall::PushMethod {
                header: 0x4_0288,
                value: 0x1000_0000
            },
            DeviceCall::PushMethod {
                header: 0x4_028C,
                value: 0x1C00
            },
            DeviceCall::DrawVertices {
                primitive: 5,
                start: 4,
                count: 6
            },
        ]
    );
    assert_eq!(
        rig.host.calls,
        vec![
            (HostFunction::BindVertexStream, vec![0, 0]),
            (HostFunction::EffectStateSubmit, vec![0])
        ]
    );
    assert_eq!(rig.memory.read32(VERTEX_RUN_START), u32::MAX);
    assert_eq!(rig.memory.read32(VERTEX_RING_BASE + 4), 0x60);
}

#[test]
fn vertex_binding_setup_emits_typed_array_formats_and_offsets() {
    let mut rig = Rig::new();
    {
        let mut renderer = rig.renderer();
        renderer.set_stream_binding(6, 0xF000_1000, 8);
        renderer.set_stream_binding(0, 0xF000_2000, 12);
        renderer.emit_vertex_bindings(0x10 | 0x1);
    }

    let [DeviceCall::SetVertexBindings { formats, offsets }] = rig.device.calls.as_slice() else {
        panic!("expected one typed vertex-binding operation");
    };
    assert_eq!(formats[0], (8 << 8) | 0x25);
    assert_eq!(formats[1], (12 << 8) | 0x32);
    assert_eq!(formats[2], 2);
    assert_eq!(offsets[0], 0x7000_1000);
    assert_eq!(offsets[1], 0x7000_2000);
}

#[test]
fn textured_rect_queues_four_vertices() {
    let mut rig = Rig::new();
    rig.device.lock_address = 0x6000;
    rig.memory.write32(VERTEX_RUN_START, u32::MAX);
    rig.memory.write32(BATCH_STRIDE, 24);
    rig.memory.write32(BATCH_FORMAT, 0xD);
    rig.memory.write32(BATCH_EXTRA, 0x35);
    rig.memory.write32(BATCH_PRIMITIVE, 8);
    rig.memory.write_f32(CONST_ONE, 1.0);
    rig.memory.write_f32(0x0047_5DC4, 0.5);
    rig.renderer()
        .draw_textured_rect(1.0, 2.0, 3.0, 4.0, 10, 20, 30, 40, 0xFFFF_FFFF);
    assert_eq!(rig.memory.read_f32(0x6000), 1.0);
    assert_eq!(rig.memory.read32(0x600C), 0xFFFF_FFFF);
    assert_eq!(rig.memory.read_f32(0x6010), 5.0);
    assert_eq!(rig.memory.read_f32(0x6014), 10.0);
    assert_eq!(rig.memory.read32(VERTEX_CURSOR), 0x6000 + 24 * 4);
    assert_eq!(rig.memory.read32(STAT_TRIANGLES), 2);
}

#[test]
fn blend_methods_use_the_shadow_words() {
    let mut rig = Rig::new();
    rig.memory.write32(BLEND_MODE, 0x13);
    rig.memory.write32(DEVICE_STATE_SHADOW + 8 * 4, 0xAABB_CCDD);
    rig.memory.write32(DEVICE_STATE_SHADOW + 9 * 4, 0x1122_3344);
    rig.memory.write32(BLEND_ALPHA, 0x0011_2233);
    rig.renderer().refresh_blend_methods();
    assert_eq!(
        rig.device.calls,
        vec![
            DeviceCall::PushMethod {
                header: 0x4_0288,
                value: 0xDD | 0x1000_2000
            },
            DeviceCall::PushMethod {
                header: 0x4_1E20,
                value: 0x3322_1100
            },
            DeviceCall::PushMethod {
                header: 0x4_028C,
                value: 0x1122_1144
            },
        ]
    );
}

#[test]
fn particle_helpers() {
    let mut memory = SparseMemory::new();
    assert_eq!(particle::ftol(3.9), 3);
    assert_eq!(particle::ftol(-3.9), (-3i32) as u32);
    assert_eq!(particle::ftol(f32::NAN), 0);
    // two keys 0 -> 0 and 1 -> 10
    memory.write_f32(0x1000, 0.0);
    memory.write_f32(0x1004, 0.0);
    memory.write_f32(0x1008, 1.0);
    memory.write_f32(0x100C, 10.0);
    assert_eq!(particle::curve_sample(&memory, 0.25, 0x1000, 2), 2.5);
    assert_eq!(particle::curve_sample(&memory, -1.0, 0x1000, 2), 0.0);
    assert_eq!(particle::curve_sample(&memory, 5.0, 0x1000, 2), 10.0);
}

#[test]
fn draw_dispatch_flushes_indexed_range_as_guest_u16_indices() {
    let mut rig = Rig::new();
    rig.memory.write32(INDEX_FIRST, 3);
    rig.memory.write32(INDEX_LAST, 7);
    rig.memory.write32(BATCH_PRIMITIVE, 5);

    rig.renderer().draw_dispatch();

    assert!(rig.device.calls.contains(&DeviceCall::DrawIndexedVertices {
        primitive: 5,
        count: 4,
        indices: INDEX_ARRAY + 3 * 2,
    }));
    assert_eq!(rig.memory.read32(INDEX_FIRST), 7);
    assert_eq!(rig.memory.read32(INDEX_FLAG), 0);
}

#[test]
fn draw_dispatch_flushes_nonindexed_vertex_range() {
    let mut rig = Rig::new();
    rig.memory.write32(INDEX_FIRST, 0);
    rig.memory.write32(INDEX_LAST, 0);
    rig.memory.write32(VERTEX_RUN_START, 11);
    rig.memory.write32(VERTEX_RUN_END, 17);
    rig.memory.write32(BATCH_PRIMITIVE, 4);

    rig.renderer().draw_dispatch();

    assert!(rig.device.calls.contains(&DeviceCall::DrawVertices {
        primitive: 4,
        start: 11,
        count: 6,
    }));
    assert_eq!(rig.memory.read32(VERTEX_RUN_START), u32::MAX);
}

#[test]
fn draw_dispatch_skips_wrapped_vertex_count_but_retires_the_ring_run() {
    let mut rig = Rig::new();
    rig.memory.write32(INDEX_FIRST, 0);
    rig.memory.write32(INDEX_LAST, 0);
    rig.memory.write32(VERTEX_RUN_START, 19);
    rig.memory.write32(VERTEX_RUN_END, 18);
    rig.memory.write32(VERTEX_RING_INDEX, 2);
    let used_address = VERTEX_RING_BASE + 2 * VERTEX_RING_STRIDE + 4;
    rig.memory.write32(used_address, 0x23);

    rig.renderer().draw_dispatch();

    assert!(rig.device.calls.iter().all(|call| !matches!(
        call,
        DeviceCall::DrawVertices { .. } | DeviceCall::DrawIndexedVertices { .. }
    )));
    assert_eq!(rig.memory.read32(VERTEX_RUN_START), u32::MAX);
    assert_eq!(rig.memory.read32(used_address), 0x40);
}

#[test]
fn state_slot_record_round_trips_every_field() {
    let mut memory = SparseMemory::new();
    let slot_record = StateSlot {
        state_index: 1,
        committed: 2,
        pending: 3,
        reserved_0c: 4,
        older: 5,
        newer: 6,
    };
    slot_record.write(&mut memory, 0x1000);
    for (offset, expected) in [
        (0x00, 1),
        (0x04, 2),
        (0x08, 3),
        (0x0C, 4),
        (0x10, 5),
        (0x14, 6),
    ] {
        assert_eq!(memory.read32(0x1000 + offset), expected);
    }
    assert_eq!(StateSlot::read(&memory, 0x1000), slot_record);
}
