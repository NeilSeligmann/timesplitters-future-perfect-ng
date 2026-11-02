// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded producer packet shared by the no_std collector and native inlet.
#![no_std]

pub const ROOM_CAPACITY: usize = 200;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RoomCoverage {
    pub depth: i32,
    /// left, top, right, bottom (the mailbox convention).
    pub rectangle: [i32; 4],
    pub after_ordinary: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CameraSample {
    pub eye: [f32; 3],
    pub view: [f32; 16],
    pub projection: [f32; 16],
    pub tangent_extents: [f32; 2],
    pub visible_rooms: [i32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NativeSceneView {
    pub viewer: u32,
    pub position: [f32; 3],
    pub matrix: [f32; 16],
    pub viewport: [f32; 4],
    pub rectangle: [i32; 4],
    /// Zero means the Rust camera owner has no completed current sample.
    pub camera_valid: u32,
    pub camera: CameraSample,
    pub room_count: u32,
    pub max_depth: i32,
    /// Index zero is reserved by the original collector; entries are 1..=count.
    pub coverage: [RoomCoverage; ROOM_CAPACITY + 1],
}

impl Default for NativeSceneView {
    fn default() -> Self {
        Self {
            viewer: 0,
            position: [0.; 3],
            matrix: [0.; 16],
            viewport: [0.; 4],
            rectangle: [0; 4],
            camera_valid: 0,
            camera: CameraSample::default(),
            room_count: 0,
            max_depth: -1,
            coverage: [RoomCoverage::default(); ROOM_CAPACITY + 1],
        }
    }
}

/// TEMPORARY-SHIM, removal T2244. Copy at the collector boundary, before its
/// retained draw changes scratch. The supplied camera comes from the Rust owner;
/// this function never reconstructs a projection from a guest matrix.
pub fn copy_collector_view(
    read32: impl Fn(u32) -> u32,
    camera_record: u32,
    viewport_record: u32,
    camera: Option<CameraSample>,
) -> Option<NativeSceneView> {
    if camera_record == 0 || viewport_record == 0 {
        return None;
    }
    let matrix = read32(camera_record + 0x20c);
    let count = read32(0x7b975c);
    if matrix == 0 || count > ROOM_CAPACITY as u32 {
        return None;
    }
    let vector = |address| core::array::from_fn(|i| f32::from_bits(read32(address + i as u32 * 4)));
    let rectangle = |address| {
        [
            read32(address) as i32,
            read32(address + 0x14) as i32,
            read32(address + 4) as i32,
            read32(address + 0x18) as i32,
        ]
    };
    let mut packet = NativeSceneView {
        viewer: read32(camera_record + 0x214),
        position: vector(camera_record + 0x27c),
        matrix: core::array::from_fn(|i| f32::from_bits(read32(matrix + i as u32 * 4))),
        viewport: [8, 0x10, 0x1c, 0x24]
            .map(|offset| read32(viewport_record + offset) as i32 as f32),
        rectangle: rectangle(viewport_record),
        camera_valid: u32::from(camera.is_some()),
        camera: camera.unwrap_or_default(),
        room_count: count,
        max_depth: read32(camera_record + 0xd28) as i32,
        ..NativeSceneView::default()
    };
    let rooms = read32(0x708788);
    if count != 0 && rooms == 0 {
        return None;
    }
    for room in 1..=count {
        let slot = camera_record + 0x3bc + room * 12;
        let first = read32(slot + 4);
        let second = read32(slot + 8);
        packet.coverage[room as usize] = RoomCoverage {
            depth: read32(slot) as i32,
            rectangle: [
                first as i16 as i32,
                (first >> 16) as i16 as i32,
                second as i16 as i32,
                (second >> 16) as i16 as i32,
            ],
            after_ordinary: (read32(rooms + room * 0xb4 + 0xb0) >> 16) & 8,
        };
    }
    Some(packet)
}
