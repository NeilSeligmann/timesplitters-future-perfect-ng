// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic partial diagnostic. Never accepts a capture as complete.
use std::{
    collections::{BTreeMap, HashMap},
    env, fs,
};
use tsfp_engine_fileio::file_object::{Backing, BodyHost, File, FileObjects as Bodies};
use tsfp_host_memory_capture::events;
use tsfp_kernel_object::{
    CallFrame, CallOutcome, FileObjects, GuestAddress, GuestMemory, Handle, Kind, ObjectTable,
    Owner,
};
fn u32at(b: &[u8], n: usize) -> u32 {
    u32::from_le_bytes(b[n..n + 4].try_into().unwrap())
}
fn u64at(b: &[u8], n: usize) -> u64 {
    u64::from_le_bytes(b[n..n + 8].try_into().unwrap())
}
#[derive(Default)]
struct Memory(BTreeMap<u32, u32>);
impl GuestMemory for Memory {
    fn read_word(&self, p: GuestAddress) -> Option<u32> {
        self.0.get(&p.0).copied()
    }
    fn write_word(&mut self, p: GuestAddress, v: u32) -> bool {
        self.0.insert(p.0, v);
        true
    }
    fn readable(&self, p: GuestAddress, _: u32) -> bool {
        self.0.contains_key(&p.0)
    }
}
struct Host {
    handle: u32,
    body: u32,
    backing: Backing,
    bytes: [u8; 48],
    name: u32,
    freed: bool,
}
impl BodyHost for Host {
    fn file(&self, h: u32) -> Option<File> {
        (h == self.handle).then_some(File {
            existing_body: 0,
            backing: self.backing,
        })
    }
    fn allocate(&mut self) -> u32 {
        self.body
    }
    fn write_body(&mut self, b: u32, bytes: &[u8; 48]) -> bool {
        b == self.body && bytes == &self.bytes
    }
    fn free(&mut self, b: u32) -> bool {
        self.freed = b == self.body;
        self.freed
    }
    fn bind(&mut self, h: u32, b: u32, n: u32) -> bool {
        [h, b, n] == [self.handle, self.body, self.name]
    }
    fn find(&self, _: u32) -> Option<u32> {
        None
    }
    fn write_u32(&mut self, _: u32, _: u32) -> bool {
        false
    }
    fn reset_heap(&mut self) {}
}
struct Provider {
    bodies: Bodies,
    host: Host,
    bound: bool,
    references: u32,
}
impl FileObjects for Provider {
    fn reference(&mut self, table: &mut ObjectTable, address: GuestAddress) -> bool {
        if !self.bound || address.0 != self.host.body {
            return false;
        }
        if self.references != u32::MAX && table.file_body_reference(Handle(self.host.handle)) {
            self.references += 1;
        }
        true
    }
    fn dereference(&mut self, table: &mut ObjectTable, address: GuestAddress) -> bool {
        if !self.bound || address.0 != self.host.body || self.references == 0 {
            return false;
        }
        self.references -= 1;
        if table
            .file_body_dereference(Handle(self.host.handle))
            .unwrap()
        {
            self.release(Handle(self.host.handle));
        }
        true
    }
    fn reference_by_handle(
        &mut self,
        table: &mut ObjectTable,
        mem: &mut impl GuestMemory,
        h: Handle,
        _: u32,
        out: GuestAddress,
    ) -> Option<u32> {
        if !self.bound || h.0 != self.host.handle {
            return None;
        }
        assert!(mem.write_word(out, self.host.body));
        assert!(table.file_body_reference(h));
        self.references += 1;
        Some(0)
    }
    fn release(&mut self, h: Handle) {
        assert_eq!(h.0, self.host.handle);
        let body = self.host.body;
        self.bodies.release(&mut self.host, body);
        self.bound = false;
    }
}
#[derive(Clone)]
struct End {
    op: u32,
    args: Vec<u32>,
    result: u32,
    outputs: Vec<u32>,
}
fn frame(args: &[u32], mem: &mut Memory) -> CallFrame {
    let stack = 0x1000;
    for (i, v) in args.iter().enumerate() {
        mem.0.insert(stack + 4 + i as u32 * 4, *v);
    }
    CallFrame {
        stack: GuestAddress(stack),
        limit: GuestAddress(0),
        ecx: None,
    }
}
fn main() {
    let path = env::args().nth(1).unwrap();
    assert!(path.ends_with(".part"));
    let data = fs::read(path).unwrap();
    assert_eq!(&data[..8], b"TSFPHC1\n");
    let mut pos = 8;
    let mut calls = HashMap::new();
    let mut ended = Vec::new();
    let (mut outer, mut handle, mut body, mut name, mut backing, mut sector) = (0, 0, 0, 0, 0, 0);
    let mut writes = Vec::new();
    let mut sequence = 0;
    while pos < data.len() {
        let seq = u64at(&data, pos);
        assert_eq!(seq, sequence);
        sequence += 1;
        let kind = u32at(&data, pos + 8);
        let len = u32at(&data, pos + 12) as usize;
        let p = &data[pos + 16..pos + 16 + len];
        if kind == events::IDENTITY {
            assert_eq!(p, b"synthetic-c-fileobject-partial");
        }
        if kind == events::CALL_BEGIN {
            let id = u64at(p, 0);
            let op = u32at(p, 24);
            let n = u32at(p, 28) as usize;
            assert_eq!(len, 32 + n * 4);
            let args = (0..n).map(|i| u32at(p, 32 + i * 4)).collect::<Vec<_>>();
            if op == 1001 {
                outer = id;
                handle = args[0];
            }
            calls.insert(id, (op, args));
        } else if kind == events::MEMORY_WRITE && u64at(p, 0) == outer && outer != 0 {
            let n = u32at(p, 12) as usize;
            assert_eq!(len, 16 + n);
            writes.push((u32at(p, 8), p[16..].to_vec()));
        } else if kind == events::CALL_END {
            let id = u64at(p, 0);
            let (op, args) = calls.get(&id).unwrap().clone();
            let result = u32at(p, 8);
            let n = u32at(p, 24) as usize;
            assert_eq!(len, 28 + n * 4);
            let outputs = (0..n).map(|i| u32at(p, 28 + i * 4)).collect();
            if op == 15 {
                body = result;
            }
            if op == 1102 {
                backing = u32at(p, 28);
                sector = u32at(p, 36);
            }
            if op == 2002 {
                name = args[2];
            }
            ended.push(End {
                op,
                args,
                result,
                outputs,
            });
        }
        pos += 16 + len;
    }
    assert!(outer != 0 && body != 0 && handle != 0);
    let mut bytes = [0u8; 48];
    for (addr, data) in writes {
        let n = (addr - body) as usize;
        bytes[n..n + data.len()].copy_from_slice(&data);
    }
    let backing = match backing {
        1 => Backing::Disc { sector },
        2 => Backing::HostDirectory,
        _ => panic!("unknown backing"),
    };
    let mut provider = Provider {
        bodies: Bodies::default(),
        host: Host {
            handle,
            body,
            backing,
            bytes,
            name,
            freed: false,
        },
        bound: false,
        references: 0,
    };
    let mut table = ObjectTable::default();
    let mut mem = Memory::default();
    let mut seen = 0u32;
    let mut reference_write = None;
    for e in ended {
        match e.op {
            2001 => {
                assert_eq!(e.args[0], Kind::File as u32);
                assert_eq!(table.create(Kind::File, Owner(e.args[1])).0, e.result);
                seen |= 1;
            }
            2002 => {
                assert_eq!(e.args, [handle, body, name]);
                assert!(
                    table
                        .find(Handle(handle))
                        .is_some_and(|entry| entry.kind == Kind::File)
                );
                assert!(!provider.bound);
                assert_eq!(provider.bodies.fabricate(&mut provider.host, handle), body);
                provider.bound = true;
                assert_eq!(e.result, 1);
                seen |= 2;
            }
            2003 => {
                let live = table.find(Handle(e.args[0])).is_some();
                assert_eq!(reference_write.take(), live.then_some((e.args[2], body)));
                let f = frame(&e.args, &mut mem);
                assert_eq!(
                    table.dispatch(246, Some(f), &mut mem, Owner(0), &mut provider),
                    CallOutcome::Returned(e.result)
                );
                if live {
                    assert_eq!(mem.0.get(&e.args[2]), Some(&body));
                }
                seen |= 4;
            }
            2005 => {
                let f = frame(&e.args, &mut mem);
                assert_eq!(
                    table.dispatch(187, Some(f), &mut mem, Owner(0), &mut provider),
                    CallOutcome::Returned(e.result)
                );
                seen |= 8;
            }
            2004 => {
                let f = CallFrame {
                    stack: GuestAddress(0),
                    limit: GuestAddress(0),
                    ecx: Some(e.args[0]),
                };
                assert_eq!(
                    table.dispatch(250, Some(f), &mut mem, Owner(0), &mut provider),
                    CallOutcome::Returned(e.result)
                );
                seen |= 16;
            }
            2008 => {
                let f = CallFrame {
                    stack: GuestAddress(0),
                    limit: GuestAddress(0),
                    ecx: Some(e.args[0]),
                };
                assert_eq!(
                    table.dispatch(251, Some(f), &mut mem, Owner(0), &mut provider),
                    CallOutcome::Returned(e.result)
                );
                seen |= 64;
            }
            2006 => {
                table.reset();
                assert_eq!(table.fresh_count(), 0);
                seen |= 32;
            }
            1101 => {
                let entry = table.find(Handle(handle)).unwrap();
                assert_eq!(e.result, 1);
                assert_eq!(
                    e.outputs,
                    [
                        u32::from(entry.kind == Kind::File),
                        if provider.bound { body } else { 0 }
                    ]
                );
            }
            1104 => {
                let found =
                    provider.bound && e.args[0] == body && table.find(Handle(handle)).is_some();
                assert_eq!(e.result, u32::from(found));
                assert_eq!(
                    e.outputs,
                    if found {
                        vec![handle, name]
                    } else {
                        vec![0, 0]
                    }
                );
            }
            2007 => {
                assert_eq!(e.args.len(), 2);
                assert!(reference_write.replace((e.args[0], e.args[1])).is_none());
                assert_eq!(e.result, 1);
            }
            _ => continue,
        }
        if [2002, 2003, 2004, 2005, 2008].contains(&e.op) {
            let entry = table.find(Handle(handle));
            let live = entry.is_some();
            let expected = vec![
                u32::from(live),
                entry.map_or(0, |x| x.references),
                if live { provider.references } else { 0 },
                if live && provider.bound { body } else { 0 },
                if live && provider.bound { name } else { 0 },
            ];
            assert_eq!(e.outputs, expected, "object state after operation {}", e.op);
        }
    }
    assert_eq!(seen, 127);
    assert!(provider.host.freed && !provider.bound);
    assert_eq!(provider.bodies.counters()[0], 0);
    eprintln!("INCOMPLETE: synthetic object and FILE provider lifecycle agrees");
    std::process::exit(2);
}
