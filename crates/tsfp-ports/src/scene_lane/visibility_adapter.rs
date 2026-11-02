// SPDX-License-Identifier: GPL-3.0-or-later
//! TEMPORARY-SHIM, removal T2244: read-only legacy visibility inlet and
//! original render-scratch outlet. All decisions use immutable owned records.
use crate::{abi::ExtRegisters, memory::GuestMemory, scene::visibility::*};
use alloc::vec::Vec;

use core::sync::atomic::{AtomicU64, Ordering};
static PROJECTION_CALLS: AtomicU64 = AtomicU64::new(0);
static VISIBILITY_CALLS: AtomicU64 = AtomicU64::new(0);
static DECLINED: AtomicU64 = AtomicU64::new(0);
/// Diagnostic receipts:0 projected polygons,1 owned room walks,2 declined inputs.
/// They are host statistics, never simulation state or acceptance counters.
#[cfg(not(test))]
#[unsafe(no_mangle)]
pub extern "C" fn tsfp_scene_visibility_stats(index: u32) -> u64 {
    match index {
        0 => PROJECTION_CALLS.load(Ordering::Relaxed),
        1 => VISIBILITY_CALLS.load(Ordering::Relaxed),
        2 => DECLINED.load(Ordering::Relaxed),
        _ => 0,
    }
}

fn vector<M: GuestMemory>(m: &M, address: u32) -> [f32; 3] {
    core::array::from_fn(|i| f32::from_bits(m.read32(address.wrapping_add(i as u32 * 4))))
}
fn rect<M: GuestMemory>(m: &M, address: u32) -> Rect {
    Rect {
        left: i32::from(m.read16(address) as i16),
        top: i32::from(m.read16(address + 2) as i16),
        right: i32::from(m.read16(address + 4) as i16),
        bottom: i32::from(m.read16(address + 6) as i16),
    }
}
fn write_rect<M: GuestMemory>(m: &mut M, address: u32, rect: Rect) {
    for (offset, value) in [
        (0, rect.left),
        (2, rect.top),
        (4, rect.right),
        (6, rect.bottom),
    ] {
        let bytes = (value as i16).to_le_bytes();
        m.write8(address + offset, bytes[0]);
        m.write8(address + offset + 1, bytes[1]);
    }
}
fn viewport<M: GuestMemory>(m: &M, address: u32) -> ProjectionViewport {
    ProjectionViewport {
        x_offset: m.read32(address + 8) as i32 as f32,
        x_scale: m.read32(address + 0x10) as i32 as f32,
        y_offset: m.read32(address + 0x1c) as i32 as f32,
        y_scale: m.read32(address + 0x24) as i32 as f32,
    }
}
fn count(value: u32, max: u32) -> Option<u32> {
    if value as i32 <= 0 {
        Some(0)
    } else if value <= max {
        Some(value)
    } else {
        None
    }
}

/// Internal output status fields are preserved for original callers. Unused
/// failure-only polygon scratch residues are outside this semantic contract.
pub fn run<M: GuestMemory>(va: u32, m: &mut M, r: &mut ExtRegisters) -> bool {
    if va != 0x87bb0 && va != 0x87f80 {
        return false;
    }
    run_impl(va, m, r)
}
/// Called only after the lane guard accepts the prepared write footprint.
pub fn receipt(va: u32, accepted: bool) {
    let counter = if !accepted {
        &DECLINED
    } else if va == 0x87bb0 {
        &PROJECTION_CALLS
    } else {
        &VISIBILITY_CALLS
    };
    counter.fetch_add(1, Ordering::Relaxed);
}

fn run_impl<M: GuestMemory>(va: u32, m: &mut M, r: &mut ExtRegisters) -> bool {
    if va == 0x87bb0 {
        let polygon = m.read32(r.esp + 4);
        let Some(n) = count(m.read32(polygon + 0x194), 20) else {
            return false;
        };
        let points: Vec<_> = (0..n)
            .map(|i| {
                core::array::from_fn(|j| f32::from_bits(m.read32(polygon + i * 16 + j as u32 * 4)))
            })
            .collect();
        let viewport = viewport(m, m.read32(0x7e71c8));
        let rect = Rect {
            left: m.read32(r.esp + 8) as i32,
            right: m.read32(r.esp + 12) as i32,
            top: m.read32(r.esp + 16) as i32,
            bottom: m.read32(r.esp + 20) as i32,
        };
        match clip_project(&points, viewport, rect) {
            None => m.write32(polygon + 0x190, 1),
            Some(points) => {
                m.write32(polygon + 0x194, points.len() as u32);
                for (i, point) in points.iter().enumerate() {
                    m.write32(polygon + i as u32 * 16, point[0].to_bits());
                    m.write32(polygon + i as u32 * 16 + 4, point[1].to_bits());
                }
                m.write32(polygon + 0x190, 0);
            }
        }
        return true;
    }
    if va != 0x87f80 {
        return false;
    }
    // Camera +3BC..+D27 has 201 twelve-byte room slots; +D28 is max depth.
    let Some(room_count) = count(m.read32(0x7b975c), 200) else {
        return false;
    };
    // Retail clip records are bounded by their signed/u16 index formats.
    let Some(portal_count) = count(m.read32(0x7b9754), 65535) else {
        return false;
    };
    let camera = m.read32(0x7b0cc0);
    let rooms = m.read32(0x708788);
    let portal_table = m.read32(0x70878c);
    let portal_scratch = m.read32(0x708778);
    let recursion = m.read32(0x70383c);
    let viewer = m.read32(camera + 0x214);
    if viewer >= 16 {
        return false;
    }
    let start = m.read32(r.esp + 4);
    if start > room_count {
        return false;
    }
    let mut room_defs = Vec::new();
    let mut room_coverage = Vec::new();
    let mut active_rooms = Vec::new();
    for i in 0..=room_count {
        let address = rooms + i * 0xb4;
        let list = m.read32(address + 4);
        let mut links = Vec::new();
        if list != 0 {
            let Some(n) = count(m.read32(list), portal_count) else {
                return false;
            };
            for j in 0..n {
                let index = m.read32(list + 4 + j * 4);
                if index > portal_count {
                    return false;
                }
                links.push(index as usize);
            }
        }
        room_defs.push(Room {
            portals: links,
            crossing_suppressed: m.read8(address + 0xb2) & 0x40 != 0,
        });
        let slot = camera + 0x3bc + i * 12;
        room_coverage.push(RoomCoverage {
            depth: m.read32(slot) as i32,
            rect: rect(m, slot + 4),
        });
        active_rooms.push(m.read32(recursion + i * 4) != 0);
    }
    let mut portal_defs = Vec::new();
    let mut portal_rects = Vec::new();
    let mut portal_visits = Vec::new();
    for i in 0..=portal_count {
        let address = m.read32(portal_table + i * 4);
        let slot = portal_scratch + i * 40;
        // Index0 is reserved and may have no backing geometry.
        if address == 0 {
            if i != 0 {
                return false;
            }
            portal_defs.push(Portal {
                rooms: [None, None],
                normal: [0.; 3],
                origin: [0.; 3],
                points: Vec::new(),
                closed_viewers: 0,
            });
        } else {
            let endpoint = |offset| {
                let room = m.read32(address + offset) as i32;
                if room < 0 { None } else { Some(room as usize) }
            };
            let Some(n) = count(i32::from(m.read16(address + 0x14) as i16) as u32, 20) else {
                return false;
            };
            let points = (0..n).map(|j| vector(m, address + 0x18 + j * 12)).collect();
            portal_defs.push(Portal {
                rooms: [endpoint(0), endpoint(4)],
                normal: vector(m, address + 8),
                origin: vector(m, address + 0x18),
                points,
                closed_viewers: m.read16(address + 0x16),
            });
        }
        portal_rects.push(rect(m, slot));
        portal_visits.push(m.read32(slot + 0x24));
    }
    let view_record = m.read32(0x7e71c8);
    let matrix = m.read32(camera + 0x20c);
    let mut clip_groups = Vec::new();
    let Some(groups) = count(m.read32(0x70a908 + viewer * 4), 10) else {
        return false;
    };
    for i in 0..groups {
        let address = 0x708928 + viewer * 0x550 + i * 0x88;
        let header = m.read32(address);
        let n = i32::from(m.read16(header + 0x10) as i16) + 1;
        if n > 8 {
            return false;
        }
        let mut planes = Vec::new();
        for j in 0..n.max(0) as u32 {
            let plane = address + 8 + j * 16;
            planes.push(crate::scene::spatial::Plane {
                normal: vector(m, plane),
                offset: f32::from_bits(m.read32(plane + 12)),
            });
        }
        clip_groups.push(planes);
    }
    let scene = VisibilityScene {
        rooms: room_defs,
        portals: portal_defs,
    };
    let view = VisibilityView {
        position: vector(m, camera + 0x27c),
        matrix: core::array::from_fn(|i| f32::from_bits(m.read32(matrix + i as u32 * 4))),
        viewport: viewport(m, view_record),
        full_rect: Rect {
            left: m.read32(view_record) as i32,
            right: m.read32(view_record + 4) as i32,
            top: m.read32(view_record + 0x14) as i32,
            bottom: m.read32(view_record + 0x18) as i32,
        },
        viewer,
        near_portal_distance: f32::from_bits(m.read32(0x475dbc)),
        clip_groups,
        mapmaker: m.read32(0x79094c) == 0x64,
    };
    let initial = VisibilityOutput {
        rooms: room_coverage,
        portals: portal_rects,
        portal_visits,
        max_depth: m.read32(camera + 0xd28) as i32,
        active_rooms,
    };
    let root_rect = Rect {
        left: m.read32(r.esp + 12) as i32,
        right: m.read32(r.esp + 16) as i32,
        top: m.read32(r.esp + 20) as i32,
        bottom: m.read32(r.esp + 24) as i32,
    };
    let Ok(output) = room_visibility(
        &scene,
        &view,
        start as usize,
        m.read32(r.esp + 8) as i32,
        root_rect,
        &initial,
    ) else {
        return false;
    };
    // Atomic semantic preparation above; original scratch compatibility below.
    for (i, room) in output.rooms.iter().enumerate() {
        let slot = camera + 0x3bc + i as u32 * 12;
        m.write32(slot, room.depth as u32);
        write_rect(m, slot + 4, room.rect);
        m.write32(recursion + i as u32 * 4, u32::from(output.active_rooms[i]));
    }
    for (i, portal) in output.portals.iter().enumerate() {
        let slot = portal_scratch + i as u32 * 40;
        write_rect(m, slot, *portal);
        m.write32(slot + 0x24, output.portal_visits[i]);
    }
    m.write32(camera + 0xd28, output.max_depth as u32);
    true
}
