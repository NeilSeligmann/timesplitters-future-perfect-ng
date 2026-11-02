//! The existing FABRICATED FILE_OBJECT preset, reconstructed from the C host.
//! The physical-sort-key caller/ABI evidence is in kernel_file_object.h.
use std::sync::atomic::{AtomicU32, Ordering};

pub const INVALID_PARAMETER: u32 = 0xc000_000d;
pub const INVALID_CLASS: u32 = 0xc000_0003;
pub const LENGTH_MISMATCH: u32 = 0xc000_0004;
pub const FATX: u32 = 0x5854_4146;
pub const GDFX: u32 = 0x5846_4447;
#[derive(Clone, Copy, Debug)]
pub enum Backing {
    Disc { sector: u32 },
    HostDirectory,
    Unsupported,
}
#[derive(Clone, Copy, Debug)]
pub struct File {
    pub existing_body: u32,
    pub backing: Backing,
}
/// The allocator and object registry are shared host services. Implementations
/// must serialize heap operations without holding that lock across bind/find.
pub trait BodyHost {
    fn file(&self, handle: u32) -> Option<File>;
    fn allocate(&mut self) -> u32;
    fn write_body(&mut self, body: u32, bytes: &[u8; 0x30]) -> bool;
    fn free(&mut self, body: u32) -> bool;
    fn bind(&mut self, handle: u32, body: u32, name: u32) -> bool;
    fn find(&self, body: u32) -> Option<u32>;
    fn write_u32(&mut self, address: u32, value: u32) -> bool;
    fn reset_heap(&mut self);
}
#[derive(Default)]
pub struct FileObjects {
    live: AtomicU32,
    fabricated: AtomicU32,
    refused: AtomicU32,
    answered: AtomicU32,
}
impl FileObjects {
    pub fn fabricate(&self, host: &mut impl BodyHost, handle: u32) -> u32 {
        let Some(file) = host.file(handle) else {
            return 0;
        };
        if file.existing_body != 0 {
            return file.existing_body;
        }
        let (name, sector) = match file.backing {
            Backing::Disc { sector } => (GDFX, sector),
            Backing::HostDirectory => (FATX, 0),
            Backing::Unsupported => return 0,
        };
        let body = host.allocate();
        if body == 0 {
            return 0;
        }
        let mut bytes = [0; 0x30];
        bytes[8..12].copy_from_slice(&body.wrapping_add(0x10).to_le_bytes());
        bytes[0x10..0x14].copy_from_slice(&sector.to_le_bytes());
        if !host.write_body(body, &bytes) {
            host.free(body);
            return 0;
        }
        if !host.bind(handle, body, name) {
            host.free(body);
            return host.file(handle).map_or(0, |file| file.existing_body);
        }
        self.live.fetch_add(1, Ordering::Relaxed);
        self.fabricated.fetch_add(1, Ordering::Relaxed);
        body
    }
    pub fn release(&self, host: &mut impl BodyHost, body: u32) {
        if host.free(body) {
            let _ = self
                .live
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
                    Some(n.saturating_sub(1))
                });
        }
    }
    /// Five ordinal-76 arguments, decoded by the boundary in the original order.
    pub fn query(&self, host: &mut impl BodyHost, args: Option<[u32; 5]>) -> u32 {
        let status = self.query_inner(host, args);
        if status == 0 {
            self.answered.fetch_add(1, Ordering::Relaxed);
        } else {
            self.refused.fetch_add(1, Ordering::Relaxed);
        }
        status
    }
    fn query_inner(&self, host: &mut impl BodyHost, args: Option<[u32; 5]>) -> u32 {
        let Some([body, class, length, buffer, returned]) = args else {
            return INVALID_PARAMETER;
        };
        let Some(name) = host.find(body) else {
            return INVALID_PARAMETER;
        };
        if class != 5 {
            return INVALID_CLASS;
        }
        if buffer == 0 {
            return INVALID_PARAMETER;
        }
        if length < 0x10 {
            return LENGTH_MISMATCH;
        }
        // Deliberately preserve partial writes on failure and poison elsewhere.
        if !host.write_u32(buffer.wrapping_add(8), 4)
            || !host.write_u32(buffer.wrapping_add(12), name)
            || (returned != 0 && !host.write_u32(returned, 0x10))
        {
            return INVALID_PARAMETER;
        }
        0
    }
    pub fn counters(&self) -> [u32; 4] {
        [&self.live, &self.fabricated, &self.refused, &self.answered]
            .map(|c| c.load(Ordering::Relaxed))
    }
    /// Quiescent reset after object-table reset, as in the C API.
    pub fn reset(&self, host: &mut impl BodyHost) {
        host.reset_heap();
        for c in [&self.live, &self.fabricated, &self.refused, &self.answered] {
            c.store(0, Ordering::Relaxed);
        }
    }
}
