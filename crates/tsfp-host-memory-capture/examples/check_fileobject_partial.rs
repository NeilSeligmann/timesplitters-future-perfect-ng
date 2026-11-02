// SPDX-License-Identifier: GPL-3.0-or-later
//! Compare selected effects from the explicitly incomplete synthetic C fixture.
//! This diagnostic cannot establish capture completeness or game conformance.
use std::{collections::HashMap, env, fs, path::Path};
use tsfp_engine_fileio::file_object::{Backing, BodyHost, File, FileObjects};
use tsfp_host_memory_capture::events;
fn u32_at(b: &[u8], n: usize) -> u32 {
    u32::from_le_bytes(b[n..n + 4].try_into().unwrap())
}
fn u64_at(b: &[u8], n: usize) -> u64 {
    u64::from_le_bytes(b[n..n + 8].try_into().unwrap())
}
struct Host {
    handle: u32,
    body: u32,
    sector: u32,
    backing: u32,
    expected: [u8; 0x30],
    bind: [u32; 3],
    wrote: bool,
    bound: bool,
    freed: bool,
    query_writes: Vec<(u32, u32)>,
    attempts: Vec<(u32, u32, bool)>,
    attempt_index: usize,
    reset: bool,
}
impl BodyHost for Host {
    fn file(&self, h: u32) -> Option<File> {
        (h == self.handle).then_some(File {
            existing_body: 0,
            backing: match self.backing {
                1 => Backing::Disc {
                    sector: self.sector,
                },
                2 => Backing::HostDirectory,
                _ => Backing::Unsupported,
            },
        })
    }
    fn allocate(&mut self) -> u32 {
        self.body
    }
    fn write_body(&mut self, body: u32, bytes: &[u8; 0x30]) -> bool {
        self.wrote = body == self.body && bytes == &self.expected;
        self.wrote
    }
    fn free(&mut self, body: u32) -> bool {
        self.freed = body == self.body;
        self.freed
    }
    fn bind(&mut self, handle: u32, body: u32, name: u32) -> bool {
        self.bound = [handle, body, name] == self.bind;
        self.bound
    }
    fn find(&self, body: u32) -> Option<u32> {
        (body == self.body).then_some(self.bind[2])
    }
    fn write_u32(&mut self, address: u32, value: u32) -> bool {
        let &(expected_address, expected_value, result) = &self.attempts[self.attempt_index];
        assert_eq!((address, value), (expected_address, expected_value));
        self.attempt_index += 1;
        if result {
            self.query_writes.push((address, value));
        }
        result
    }
    fn reset_heap(&mut self) {
        self.reset = true;
    }
}
fn main() {
    let path = env::args_os().nth(1).expect("partial capture path");
    let expected_backing: u32 = env::args()
        .nth(2)
        .unwrap_or_else(|| "1".into())
        .parse()
        .unwrap();
    assert!(
        Path::new(&path)
            .extension()
            .is_some_and(|ext| ext == "part")
    );
    assert!(fs::metadata(&path).unwrap().len() <= 1024 * 1024);
    let data = fs::read(path).unwrap();
    assert_eq!(data.get(..8), Some(&b"TSFPHC1\n"[..]));
    let mut pos = 8usize;
    let mut sequence = 0u64;
    let mut calls: HashMap<u64, (u64, u32, Vec<u32>)> = HashMap::new();
    let (mut outer, mut handle, mut body, mut sector, mut backing, mut bind, mut outer_result) =
        (0, 0, 0, 0, 0, [0; 3], None);
    let mut body_writes: Vec<(u32, Vec<u8>)> = Vec::new();
    let (mut release, mut released_body, mut free_result) = (0, 0, None);
    let (mut reset, mut destroy_result) = (0, None);
    let mut queries: Vec<(
        u64,
        [u32; 5],
        Option<u32>,
        Vec<(u32, u32)>,
        Vec<(u32, u32, bool)>,
    )> = Vec::new();
    let mut identity = false;
    while pos < data.len() {
        let frame = data.get(pos..pos + 16).expect("complete partial frame");
        assert_eq!(u64_at(frame, 0), sequence);
        let kind = u32_at(frame, 8);
        let len = u32_at(frame, 12) as usize;
        assert!(len <= 1024 * 1024);
        let payload = data
            .get(pos + 16..pos + 16 + len)
            .expect("complete partial payload");
        match kind {
            events::IDENTITY => {
                assert_eq!(payload, b"synthetic-c-fileobject-partial");
                identity = true;
            }
            events::EMPTY_REGISTRY => assert!(payload.is_empty()),
            events::CALL_BEGIN => {
                assert!(len >= 32);
                let id = u64_at(payload, 0);
                let parent = u64_at(payload, 8);
                let op = u32_at(payload, 24);
                let argc = u32_at(payload, 28) as usize;
                assert!(argc <= 16 && len == 32 + argc * 4);
                let args: Vec<u32> = (0..argc).map(|n| u32_at(payload, 32 + n * 4)).collect();
                if op == 1001 {
                    outer = id;
                    handle = args[0];
                }
                if op == 1002 {
                    release = id;
                    released_body = args[0];
                }
                if op == 1004 {
                    reset = id;
                }
                if op == 1003 {
                    queries.push((
                        id,
                        args.as_slice().try_into().unwrap(),
                        None,
                        Vec::new(),
                        Vec::new(),
                    ));
                }
                assert!(calls.insert(id, (parent, op, args)).is_none());
            }
            events::MEMORY_WRITE if len >= 16 && u64_at(payload, 0) == outer && outer != 0 => {
                let address = u32_at(payload, 8);
                let size = u32_at(payload, 12) as usize;
                assert_eq!(len, 16 + size);
                body_writes.push((address, payload[16..].to_vec()));
            }
            events::MEMORY_WRITE if len == 20 => {
                let id = u64_at(payload, 0);
                if let Some(query) = queries.iter_mut().find(|query| query.0 == id) {
                    assert_eq!(u32_at(payload, 12), 4);
                    query.3.push((u32_at(payload, 8), u32_at(payload, 16)));
                }
            }
            events::CALL_END => {
                assert!(len >= 28);
                let id = u64_at(payload, 0);
                let result = u32_at(payload, 8);
                let count = u32_at(payload, 24) as usize;
                assert!(count <= 16 && len == 28 + count * 4);
                let (parent, op, args) = calls.get(&id).expect("begin for end");
                if id == outer {
                    outer_result = Some(result);
                }
                if let Some(query) = queries.iter_mut().find(|query| query.0 == id) {
                    query.2 = Some(result);
                }
                if *op == 1104 && queries.iter().any(|query| query.0 == *parent) {
                    assert_eq!(count, 2);
                    let found = args[0] == body;
                    assert_eq!(result, u32::from(found));
                    assert_eq!(u32_at(payload, 28), if found { handle } else { 0 });
                    assert_eq!(u32_at(payload, 32), if found { bind[2] } else { 0 });
                }
                if *op == 1105 {
                    if let Some(query) = queries.iter_mut().find(|query| query.0 == *parent) {
                        assert!(result <= 1);
                        query.4.push((args[0], args[1], result == 1));
                    }
                }
                if *parent == release && release != 0 && *op == 16 {
                    assert_eq!(args[1], released_body);
                    free_result = Some(result);
                }
                if *parent == reset && reset != 0 && *op == 13 {
                    destroy_result = Some(result);
                }
                if *parent == outer {
                    match *op {
                        1101 => {
                            assert_eq!(result, 1);
                            assert_eq!(count, 2);
                            assert_eq!(u32_at(payload, 32), 0);
                        }
                        1102 => {
                            assert_eq!(result, 1);
                            assert_eq!(count, 3);
                            backing = u32_at(payload, 28);
                            assert!(backing == 1 || backing == 2);
                            assert_eq!(u32_at(payload, 32), 0);
                            sector = u32_at(payload, 36);
                        }
                        15 => body = result,
                        1103 => {
                            assert_eq!(result, 1);
                            bind.copy_from_slice(args);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        pos += 16 + len;
        sequence += 1;
    }
    assert!(identity && outer != 0 && body != 0 && outer_result == Some(body));
    assert!(release != 0 && released_body == body && free_result == Some(1));
    assert!(reset != 0 && destroy_result == Some(1));
    let mut expected = [0u8; 0x30];
    assert_eq!(body_writes.len(), 4);
    for (address, bytes) in body_writes {
        let offset = address.checked_sub(body).expect("body-relative write") as usize;
        let end = offset.checked_add(bytes.len()).expect("write extent");
        expected
            .get_mut(offset..end)
            .expect("body extent")
            .copy_from_slice(&bytes);
    }
    assert_eq!(backing, expected_backing);
    let mut host = Host {
        handle,
        body,
        sector,
        backing,
        expected,
        bind,
        wrote: false,
        bound: false,
        freed: false,
        query_writes: Vec::new(),
        attempts: Vec::new(),
        attempt_index: 0,
        reset: false,
    };
    let objects = FileObjects::default();
    let result = objects.fabricate(&mut host, handle);
    assert_eq!(result, body);
    assert!(host.wrote && host.bound);
    assert_eq!(queries.len(), 7);
    for (_, args, status, writes, attempts) in queries {
        host.query_writes.clear();
        host.attempt_index = 0;
        host.attempts = attempts;
        assert_eq!(Some(objects.query(&mut host, Some(args))), status);
        assert_eq!(host.attempt_index, host.attempts.len());
        assert_eq!(host.query_writes, writes);
    }
    objects.release(&mut host, released_body);
    assert!(host.freed);
    objects.reset(&mut host);
    assert!(host.reset && objects.counters() == [0; 4]);
    println!(
        "T3906: selected synthetic C/Rust FILE_OBJECT effects agree; capture remains incomplete"
    );
    std::process::exit(2);
}
