//! Shadow guest world for the compare twin: reads fall through to the real guest memory (read-only, through the
//! [`Host`] callbacks), writes and heap blocks live in a private overlay. The Rust lifecycle therefore never touches
//! the C host's memory, and its heap objects sit at shadow addresses (translated by the twin's alias map).

use std::collections::{BTreeMap, HashMap};
use tsfp_audio::adapter::GuestMemory;
use tsfp_audio::lifecycle::GuestWorld;

/// What the C host offers the twin (function pointers in the cdylib, a fake in the tests).
pub trait Host {
    fn read(&self, address: u32, out: &mut [u8]) -> bool;
    fn readable(&self, address: u32, len: u32) -> bool;
    fn irql0(&self) -> bool;
    fn codec_ready(&self) -> bool;
    /// The C `dsound_stream_snapshot` flattened (see `dsound_audio_cmp.c`), `None` when the C does not own the stream.
    fn stream_snapshot(&self, stream: u32) -> Option<Vec<u32>>;
    fn buffer_snapshot(&self, buffer: u32) -> Option<Vec<u32>>;
}

pub const DEFAULT_HEAP_BASE: u32 = 0xF100_0000;

pub struct Shadow<H: Host> {
    pub host: H,
    overlay: HashMap<u32, u8>,
    heaps: HashMap<u32, BTreeMap<u32, u32>>,
    next_heap: u32,
    bump: u32,
}

impl<H: Host> Shadow<H> {
    pub fn new(host: H, heap_base: u32) -> Self {
        Self {
            host,
            overlay: HashMap::new(),
            heaps: HashMap::new(),
            next_heap: 1,
            bump: heap_base,
        }
    }
    /// Make `[address, address + len)` exist in the overlay, zero filled where it does not yet.
    pub fn map_zero(&mut self, address: u32, len: u32) {
        for i in 0..len {
            self.overlay.entry(address.wrapping_add(i)).or_insert(0);
        }
    }
    /// Seed bytes in the overlay (pre-state of words the C host changes and the Rust model expects untouched).
    pub fn seed(&mut self, address: u32, data: &[u8]) {
        for (i, b) in data.iter().enumerate() {
            self.overlay.insert(address.wrapping_add(i as u32), *b);
        }
    }
    fn covered(&self, address: u32, len: usize) -> usize {
        (0..len)
            .filter(|i| self.overlay.contains_key(&address.wrapping_add(*i as u32)))
            .count()
    }
    /// Read the real (C-side, post-call) memory, ignoring the overlay.
    pub fn read_real(&self, address: u32, len: usize) -> Option<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.host.read(address, &mut buf).then_some(buf)
    }
}

impl<H: Host> GuestMemory for Shadow<H> {
    fn read_u32(&self, a: u32) -> Option<u32> {
        let b = self.read_bytes(a, 4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn write_u32(&mut self, a: u32, v: u32) -> bool {
        self.write_bytes(a, &v.to_le_bytes())
    }
    fn read_bytes(&self, a: u32, len: usize) -> Option<Vec<u8>> {
        if len == 0 {
            return Some(Vec::new());
        }
        if u64::from(a) + len as u64 > 1 << 32 {
            return None;
        }
        let covered = self.covered(a, len);
        let mut buf = vec![0u8; len];
        if covered < len && !self.host.read(a, &mut buf) {
            return None;
        }
        if covered > 0 {
            for (i, slot) in buf.iter_mut().enumerate() {
                if let Some(b) = self.overlay.get(&a.wrapping_add(i as u32)) {
                    *slot = *b;
                }
            }
        }
        Some(buf)
    }
    fn range_readable(&self, a: u32, len: u32) -> bool {
        self.mapped(a, len)
    }
}

impl<H: Host> GuestWorld for Shadow<H> {
    fn mapped(&self, a: u32, n: u32) -> bool {
        if n == 0 {
            return true;
        }
        if u64::from(a) + u64::from(n) > 1 << 32 {
            return false;
        }
        self.host.readable(a, n) || self.covered(a, n as usize) == n as usize
    }
    fn write_bytes(&mut self, a: u32, d: &[u8]) -> bool {
        if !self.mapped(a, d.len() as u32) {
            return false;
        }
        self.seed(a, d);
        true
    }
    fn heap_create(&mut self) -> u32 {
        let id = self.next_heap;
        self.next_heap += 1;
        self.heaps.insert(id, BTreeMap::new());
        id
    }
    fn heap_alloc(&mut self, h: u32, n: u32) -> u32 {
        if n == 0 || !self.heaps.contains_key(&h) {
            return 0;
        }
        let size = (n + 15) & !15;
        let Some(next) = self.bump.checked_add(size) else {
            return 0;
        };
        let address = self.bump;
        self.bump = next;
        self.map_zero(address, n);
        if let Some(heap) = self.heaps.get_mut(&h) {
            heap.insert(address, n);
        }
        address
    }
    fn heap_destroy(&mut self, h: u32) -> bool {
        let Some(blocks) = self.heaps.remove(&h) else {
            return false;
        };
        for (address, n) in blocks {
            for i in 0..n {
                self.overlay.remove(&(address + i));
            }
        }
        true
    }
    fn heap_valid(&self, h: u32) -> bool {
        self.heaps.contains_key(&h)
    }
    fn heap_block_size(&self, h: u32, address: u32) -> Option<u32> {
        self.heaps.get(&h)?.get(&address).copied()
    }
}
