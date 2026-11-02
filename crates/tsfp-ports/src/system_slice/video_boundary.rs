// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared video globals presented as values; addresses/contracts source-inspected, names INFERRED.
use crate::memory::GuestMemory;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoViewport { pub width: i32, pub height: i32, pub mode: u32 }
impl VideoViewport {
    pub fn read<M: GuestMemory>(memory: &M) -> Self {
        Self { width: memory.read32(0x687e44) as i32, height: memory.read32(0x687e48) as i32,
            mode: memory.read32(0x4bfc6c) }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoTiming { pub display_width: i32, pub display_height: i32, pub interval_x: i32,
    pub interval_y: i32, pub sound_volume: i32 }
impl VideoTiming {
    pub fn read<M: GuestMemory>(memory: &M) -> Self {
        Self { display_width: memory.read32(0x4c0458) as i32,
            display_height: memory.read32(0x4c045c) as i32,
            interval_x: memory.read32(0x4e8cf0) as i32,
            interval_y: memory.read32(0x4e8cf4) as i32,
            sound_volume: memory.read32(0x5236bc) as i32 }
    }
}
