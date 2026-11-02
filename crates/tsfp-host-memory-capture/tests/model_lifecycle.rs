// SPDX-License-Identifier: GPL-3.0-or-later
//! Exercise actual Rust FILE_OBJECT and GuestRegions operations through the recorder.
use tsfp_engine_fileio::file_object::{Backing, BodyHost, File, FileObjects};
use tsfp_engine_memory::guest_regions::{GuestRegions, WindowPlacement};
use tsfp_host_memory_capture::{Writer, events, producer::Producer, read};

struct Host {
    regions: GuestRegions<WindowPlacement>,
    heap: u32,
    trace: Producer<Vec<u8>>,
    body: u32,
}
impl BodyHost for Host {
    fn file(&self, _: u32) -> Option<File> {
        Some(File {
            existing_body: self.body,
            backing: Backing::Disc { sector: 7 },
        })
    }
    fn allocate(&mut self) -> u32 {
        let create = self.trace.begin(1, 12, &[0, 0x10000, 0]).unwrap();
        self.heap = self.regions.heap_create(0, 0x10000, 0);
        self.trace.end(1, create, self.heap, &[]).unwrap();
        let alloc = self.trace.begin(1, 15, &[self.heap, 0x30]).unwrap();
        let body = self.regions.heap_alloc(self.heap, 0x30);
        self.trace.end(1, alloc, body, &[]).unwrap();
        body
    }
    fn write_body(&mut self, body: u32, bytes: &[u8; 0x30]) -> bool {
        let ok = self.regions.write_heap_bytes(body, bytes);
        if ok {
            self.trace.memory(1, true, body, bytes).unwrap();
        }
        ok
    }
    fn free(&mut self, body: u32) -> bool {
        let call = self.trace.begin(1, 16, &[self.heap, body]).unwrap();
        let result = self.regions.heap_free(self.heap, body);
        self.trace.end(1, call, result as u32, &[]).unwrap();
        result
    }
    fn bind(&mut self, _: u32, body: u32, _: u32) -> bool {
        self.body = body;
        true
    }
    fn find(&self, _: u32) -> Option<u32> {
        None
    }
    fn write_u32(&mut self, _: u32, _: u32) -> bool {
        false
    }
    fn reset_heap(&mut self) {
        let call = self.trace.begin(1, 13, &[self.heap]).unwrap();
        let result = self.regions.heap_destroy(self.heap);
        self.trace.end(1, call, result as u32, &[]).unwrap();
    }
}

#[test]
fn actual_fileobject_lifecycle_produces_nested_heap_calls() {
    let ops = [
        (90, 1, 0),
        (91, 1, 0),
        (92, 0, 0),
        (12, 3, 0),
        (15, 2, 0),
        (16, 2, 0),
        (13, 1, 0),
    ];
    let trace = Producer::new(
        Writer::new(Vec::new(), 4096).unwrap(),
        b"fixture",
        true,
        &ops,
    )
    .unwrap();
    let mut host = Host {
        regions: GuestRegions::new(WindowPlacement::default_window()),
        heap: 0,
        trace,
        body: 0,
    };
    let objects = FileObjects::default();
    let outer = host.trace.begin(1, 90, &[1]).unwrap();
    let body = objects.fabricate(&mut host, 1);
    assert_ne!(body, 0);
    host.trace.end(1, outer, body, &[]).unwrap();
    let outer = host.trace.begin(1, 91, &[body]).unwrap();
    objects.release(&mut host, body);
    host.trace.end(1, outer, 0, &[]).unwrap();
    let outer = host.trace.begin(1, 92, &[]).unwrap();
    objects.reset(&mut host);
    host.trace.end(1, outer, 0, &[]).unwrap();
    let bytes = host.trace.seal().unwrap();
    let decoded = read(bytes.as_slice(), 4096).unwrap();
    assert_eq!(events::validate(&decoded, b"fixture", &ops), Ok(7));
}
