// SPDX-License-Identifier: GPL-3.0-or-later
//! TEMPORARY-SHIM removal T2244: native room-order inlet and legacy renderer
//! outlet. Game geometry drawing is a RenderDevice neighbour, not prep logic.
use crate::{
    abi::{ExtRegisters, GuestCall, call_cdecl},
    memory::GuestMemory,
    scene::scene_collect::{DrawRoom, RoomOrder, collect_rooms},
};
use alloc::vec::Vec;

use tsfp_render_prep_wire::{CameraSample, NativeSceneView, copy_collector_view};

#[cfg(not(any(test, tsfp_mmx_harness)))]
unsafe extern "C" {
    fn tsfp_native_scene_bridge_capture_enabled() -> i32;
    fn tsfp_native_scene_bridge_begin_draw_capture();
    fn tsfp_native_scene_bridge_end_draw_capture();
    fn tsfp_native_scene_bridge_publish(
        rooms: *const u8,
        room_count: u32,
        draw_depth: i32,
        object_records: *const u32,
        object_count: u32,
        view: *const NativeSceneView,
    );
    fn tsfp_native_scene_bridge_note_collector_result(
        outcome: u32,
        room_count: i32,
        draw_depth: i32,
    );
}

#[cfg(not(any(test, tsfp_mmx_harness)))]
fn capture_enabled() -> bool {
    // The host switch is deliberately opt-in; keep the uninstrumented
    // collector's single retained call as the default runtime path.
    unsafe { tsfp_native_scene_bridge_capture_enabled() != 0 }
}

#[cfg(not(any(test, tsfp_mmx_harness)))]
fn note_collector_result(outcome: u32, room_count: i32, draw_depth: i32) {
    unsafe {
        tsfp_native_scene_bridge_note_collector_result(outcome, room_count, draw_depth);
    }
}

#[cfg(any(test, tsfp_mmx_harness))]
fn capture_enabled() -> bool {
    false
}

#[cfg(any(test, tsfp_mmx_harness))]
fn note_collector_result(_outcome: u32, _room_count: i32, _draw_depth: i32) {}

#[cfg(any(test, tsfp_mmx_harness))]
fn tsfp_native_scene_bridge_begin_draw_capture() {}

#[cfg(any(test, tsfp_mmx_harness))]
fn tsfp_native_scene_bridge_end_draw_capture() {}

fn ordered_rooms(order: &RoomOrder) -> Vec<u8> {
    if order.draw_max_depth < 0 {
        return Vec::new();
    }
    order.buckets[..=order.draw_max_depth as usize]
        .iter()
        .flat_map(|bucket| bucket.iter().copied())
        .collect()
}

/// Single replaceable outlet for a typed native room plan.
pub trait RoomSubmission {
    fn submit(&mut self, order: &RoomOrder, scissor: [i32; 4]);
}
/// Default live sink for the native room plan. The retained guest renderer is
/// still a TEMPORARY-SHIM until typed object admission and RenderDevice exist.
struct NativeRoomPlan<'a, S> {
    downstream: &'a mut S,
    rooms: Vec<u8>,
    object_records: Vec<u32>,
    view: Option<NativeSceneView>,
}
impl<S: RoomSubmission> RoomSubmission for NativeRoomPlan<'_, S> {
    fn submit(&mut self, order: &RoomOrder, scissor: [i32; 4]) {
        self.rooms.clear();
        self.rooms.extend(ordered_rooms(order));
        // The C host owns a bounded mailbox shared with Bevy. This one-way C
        // ABI keeps the no_std ports lane independent of tsfp-engine-render.
        // Record indexes are temporary ObjectPool lookup keys; the Rust owner
        // allocates native slots and generations after this one-way handoff.
        #[cfg(not(any(test, tsfp_mmx_harness)))]
        unsafe {
            tsfp_native_scene_bridge_publish(
                self.rooms.as_ptr(),
                self.rooms.len() as u32,
                order.draw_max_depth,
                self.object_records.as_ptr(),
                self.object_records.len() as u32,
                self.view
                    .as_ref()
                    .map_or(core::ptr::null(), |view| view as *const NativeSceneView),
            );
        }
        self.downstream.submit(order, scissor);
    }
}
struct Legacy<'a, M, C> {
    memory: &'a mut M,
    calls: &'a mut C,
    registers: &'a mut ExtRegisters,
    camera: u32,
    capture_draws: bool,
}
impl<M: GuestMemory, C: GuestCall> RoomSubmission for Legacy<'_, M, C> {
    fn submit(&mut self, order: &RoomOrder, scissor: [i32; 4]) {
        let m = &mut *self.memory;
        write_room_scratch(m, order);
        m.write32(0x70aad0, 0);
        m.write32(self.camera + 0xd28, order.draw_max_depth as u32);
        if m.read32(self.camera + 0x214) == 0 {
            m.write32(0x6b7acc, 0);
        }
        let entry = self.registers.esp;
        self.registers.esp = entry.wrapping_sub(0x100);
        call_cdecl(
            self.calls,
            m,
            self.registers,
            0x1a010,
            &scissor.map(|x| x as u32),
        );
        call_cdecl(self.calls, m, self.registers, 0xa79b0, &[0xd5]);
        call_cdecl(self.calls, m, self.registers, 0x3442b0, &[]);
        if self.capture_draws {
            // Capture spans the original complete retained pass. The observer
            // deliberately has no room key: splitting 0x86AE0 by room changes
            // guest scratch and draw ordering.
            unsafe { tsfp_native_scene_bridge_begin_draw_capture() };
        }
        call_cdecl(
            self.calls,
            m,
            self.registers,
            0x86ae0,
            &[order.restore_max_depth as u32],
        );
        if self.capture_draws {
            unsafe { tsfp_native_scene_bridge_end_draw_capture() };
        }
        call_cdecl(self.calls, m, self.registers, 0x344350, &[]);
        self.registers.esp = entry;
        let current = m.read32(0x7b0cc0);
        m.write32(current + 0xd28, order.restore_max_depth as u32);
    }
}

/// These are per-view render scratch counts, not simulated state. Keep the
/// complete collector output intact for the retained 0x86AE0 call, whether or
/// not host-side draw observation is enabled.
fn write_room_scratch<M: GuestMemory>(m: &mut M, order: &RoomOrder) {
    for i in 0..80 {
        let bucket = &order.buckets[i as usize];
        let deferred = &order.deferred[i as usize];
        m.write8(0x702db0 + i, bucket.len() as u8);
        m.write8(0x702d60 + i, deferred.len() as u8);
        for (j, room) in bucket.iter().enumerate() {
            m.write8(0x7042b8 + i * 200 + j as u32, *room);
        }
        for (j, room) in deferred.iter().enumerate() {
            m.write8(0x702a40 + i * 10 + j as u32, *room);
        }
    }
}

pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    c: &mut C,
    r: &mut ExtRegisters,
) -> bool {
    if va != 0x86c40 {
        return false;
    }
    let camera = m.read32(0x7b0cc0);
    let count = m.read32(0x7b975c) as i32;
    if count < 0 || count > 200 {
        note_collector_result(1, count, 0);
        return false;
    }
    let max_depth = m.read32(camera + 0xd28) as i32;
    let rooms = m.read32(0x708788);
    let mut metadata = Vec::new();
    for room in 1..=count as u32 {
        metadata.push(DrawRoom {
            room: room as u8,
            depth: m.read32(camera + 0x3bc + room * 12) as i32,
            after_ordinary: m.read8(rooms + room * 0xb4 + 0xb2) & 8 != 0,
        });
    }
    let order = match collect_rooms(&metadata, max_depth) {
        Ok(order) => order,
        Err(_) => {
            note_collector_result(2, count, max_depth);
            return false;
        }
    };
    let view = m.read32(0x7e71c8);
    let scissor = [
        m.read32(view) as i32,
        m.read32(view + 0x14) as i32,
        m.read32(view + 4) as i32,
        m.read32(view + 0x18) as i32,
    ];
    let native_view = capture_enabled()
        .then(|| {
            let sample = crate::camera::camera_for_reference(camera)
                .and_then(|handle| crate::camera::sample_camera(handle, 1.0))
                .map(|sample| CameraSample {
                    eye: sample.eye,
                    view: sample.view,
                    projection: sample.projection.matrix,
                    tangent_extents: sample.projection.tangent_extents,
                    visible_rooms: sample.visible_rooms,
                });
            copy_collector_view(|address| m.read32(address), camera, view, sample)
        })
        .flatten();
    let mut object_records = Vec::new();
    #[cfg(not(test))]
    {
        // The shared ObjectPool owner validates exact record membership and
        // insertion order. This is lifecycle membership input only; missing
        // render components are not invented by this audit bridge.
        let pool = crate::leaf_wire::object_pool::ObjectPool::load(m);
        if let Ok(audit) = pool.audit_membership() {
            for room in ordered_rooms(&order) {
                if let Some(membership) = audit
                    .rooms
                    .iter()
                    .find(|entry| entry.room == u16::from(room))
                {
                    for &slot in &membership.records {
                        if object_records.contains(&(slot as u32)) {
                            continue;
                        }
                        object_records.push(slot as u32);
                    }
                }
            }
        }
    }
    let mut legacy = Legacy {
        memory: m,
        calls: c,
        registers: r,
        camera,
        capture_draws: capture_enabled(),
    };
    NativeRoomPlan {
        downstream: &mut legacy,
        rooms: Vec::new(),
        object_records,
        view: native_view,
    }
    .submit(&order, scissor);
    note_collector_result(0, count, max_depth);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::GuestCall;
    use std::{cell::RefCell, rc::Rc};

    #[derive(Clone)]
    struct Memory(Rc<RefCell<Vec<u8>>>);
    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0.borrow()[address as usize]
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.borrow_mut()[address as usize] = value;
        }
    }

    struct Calls {
        memory: Memory,
        calls: Vec<u32>,
        scratch_at_scene_draw: Vec<Vec<u8>>,
    }
    impl GuestCall for Calls {
        fn call(&mut self, va: u32, _registers: &mut ExtRegisters) {
            self.calls.push(va);
            if va == 0x86ae0 {
                let memory = self.memory.0.borrow();
                self.scratch_at_scene_draw
                    .push(memory[0x702a40..0x708788].to_vec());
            }
        }
    }

    #[test]
    fn published_room_sequence_uses_flattened_depth_order() {
        let order = RoomOrder {
            buckets: vec![vec![5, 2], vec![9], vec![4]],
            draw_max_depth: 2,
            restore_max_depth: 1,
            deferred: vec![Vec::new(); 80],
        };
        assert_eq!(ordered_rooms(&order), [5, 2, 9, 4]);
    }

    #[test]
    fn empty_room_order_does_not_underflow_depth() {
        let order = RoomOrder {
            buckets: vec![Vec::new(); 80],
            draw_max_depth: -1,
            restore_max_depth: -1,
            deferred: vec![Vec::new(); 80],
        };
        assert!(ordered_rooms(&order).is_empty());
    }

    #[test]
    fn capture_on_and_off_preserve_the_guest_draw_call_sequence_and_scratch() {
        let mut order = RoomOrder {
            buckets: vec![vec![5, 2], vec![9], vec![], vec![]],
            draw_max_depth: 1,
            restore_max_depth: 1,
            deferred: vec![vec![2], vec![9, 5], vec![], vec![]],
        };
        order.buckets.resize_with(80, Vec::new);
        order.deferred.resize_with(80, Vec::new);
        let run = |capture_draws| {
            let memory = Memory(Rc::new(RefCell::new(vec![0; 0x800000])));
            let mut guest_memory = memory.clone();
            let camera: u32 = 0x7b1000;
            guest_memory.0.borrow_mut()[0x7b0cc0..0x7b0cc4].copy_from_slice(&camera.to_le_bytes());
            guest_memory.0.borrow_mut()[0x7e71c8..0x7e71cc]
                .copy_from_slice(&0x7e7200u32.to_le_bytes());
            guest_memory.0.borrow_mut()[0x7e7200..0x7e7204].copy_from_slice(&10u32.to_le_bytes());
            let mut calls = Calls {
                memory: memory.clone(),
                calls: Vec::new(),
                scratch_at_scene_draw: Vec::new(),
            };
            // The all-zero integer and FP bit patterns are valid for this call fixture.
            let mut registers: ExtRegisters = unsafe { core::mem::zeroed() };
            registers.esp = 0x700000;
            let mut legacy = Legacy {
                memory: &mut guest_memory,
                calls: &mut calls,
                registers: &mut registers,
                camera,
                capture_draws,
            };
            legacy.submit(&order, [1, 2, 3, 4]);
            let bytes = memory.0.borrow().clone();
            (calls.calls, calls.scratch_at_scene_draw, bytes, registers)
        };

        let (off_calls, off_scratch, off_memory, off_registers) = run(false);
        let (on_calls, on_scratch, on_memory, on_registers) = run(true);
        let expected = [0x1a010, 0xa79b0, 0x3442b0, 0x86ae0, 0x344350];
        assert_eq!(off_calls, expected);
        assert_eq!(on_calls, expected);
        assert_eq!(off_scratch.len(), 1);
        assert_eq!(on_scratch.len(), 1);
        assert_eq!(off_scratch, on_scratch);
        assert_eq!(off_memory, on_memory);
        assert_eq!(off_registers, on_registers);
        assert_eq!(off_scratch[0][0x702db0 - 0x702a40], 2);
        assert_eq!(off_scratch[0][0x7042b8 - 0x702a40], 5);
        assert_eq!(off_scratch[0][0x7042b9 - 0x702a40], 2);
    }
}
