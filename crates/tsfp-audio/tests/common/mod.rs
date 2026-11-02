//! Shared test world: static image bytes plus the Rust `GuestRegions` guest heaps.
#![allow(dead_code)]

use std::collections::HashMap;
use tsfp_audio::adapter::GuestMemory;
use tsfp_audio::lifecycle::*;
use tsfp_engine_memory::guest_regions::{GuestRegions, WindowPlacement};

/// Static image bytes (title globals, vtables, stack) plus the Rust guest heaps.
pub struct World {
    pub regions: GuestRegions<WindowPlacement>,
    pub image: HashMap<u32, u8>,
    pub fail_heaps: bool,
}

impl World {
    pub fn new() -> Self {
        let mut w = Self {
            regions: GuestRegions::new(WindowPlacement::default_window()),
            image: HashMap::new(),
            fail_heaps: false,
        };
        w.map(SINGLETON, 4);
        w.map(DEVICE_VTABLE, 12);
        w.map(STREAM_SECONDARY, 60);
        for i in 0..15u32 {
            let target = STREAM_STOPPED_TARGETS[(i % 14) as usize].to_le_bytes();
            for (j, b) in target.iter().enumerate() {
                w.image.insert(STREAM_SECONDARY + i * 4 + j as u32, *b);
            }
        }
        w.map(BUFFER_VTABLE, 16);
        w.map(0x0010_0000, 0x1000); // title stack: outputs and descriptors
        w.map(GLOBAL_STATE, 4);
        w.map(0x0058_18E8, 160); // ordinary buffer output slots
        w.map(0x0058_35F0, 160); // spatial buffer output slots
        w.map(0x004B_914C, 0x30); // stream rolloff curves
        w.map(0x0064_BC98, 4); // buffer rolloff curve
        w
    }
    pub fn map(&mut self, a: u32, n: u32) {
        for i in 0..n {
            self.image.entry(a + i).or_insert(0);
        }
    }
    fn in_image(&self, a: u32, n: u32) -> bool {
        (0..n).all(|i| self.image.contains_key(&(a + i)))
    }
}

impl GuestMemory for World {
    fn read_u32(&self, a: u32) -> Option<u32> {
        self.read_bytes(a, 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn write_u32(&mut self, a: u32, v: u32) -> bool {
        self.write_bytes(a, &v.to_le_bytes())
    }
    fn read_bytes(&self, a: u32, len: usize) -> Option<Vec<u8>> {
        if self.in_image(a, len as u32) {
            return Some((0..len as u32).map(|i| self.image[&(a + i)]).collect());
        }
        self.regions.read_heap_bytes(a, len as u32)
    }
    fn range_readable(&self, a: u32, len: u32) -> bool {
        self.read_bytes(a, len as usize).is_some()
    }
}
impl GuestWorld for World {
    fn mapped(&self, a: u32, n: u32) -> bool {
        self.read_bytes(a, n as usize).is_some()
    }
    fn write_bytes(&mut self, a: u32, d: &[u8]) -> bool {
        if self.in_image(a, d.len() as u32) {
            for (i, b) in d.iter().enumerate() {
                self.image.insert(a + i as u32, *b);
            }
            return true;
        }
        self.regions.write_heap_bytes(a, d)
    }
    fn heap_create(&mut self) -> u32 {
        if self.fail_heaps {
            return 0;
        }
        self.regions.heap_create(0, 0x10000, 0)
    }
    fn heap_alloc(&mut self, h: u32, n: u32) -> u32 {
        self.regions.heap_alloc(h, n)
    }
    fn heap_destroy(&mut self, h: u32) -> bool {
        self.regions.heap_destroy(h)
    }
    fn heap_valid(&self, h: u32) -> bool {
        self.regions.heap_valid(h)
    }
    fn heap_block_size(&self, h: u32, p: u32) -> Option<u32> {
        self.regions.heap_block_size(h, p)
    }
}

/// Checked fixture write: invalid test addresses fail at setup.
pub fn put(w: &mut World, at: u32, bytes: &[u8]) {
    assert!(w.write_bytes(at, bytes));
}

/// Encode fixture descriptor words in guest little-endian order.
pub fn words_bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|x| x.to_le_bytes()).collect()
}
