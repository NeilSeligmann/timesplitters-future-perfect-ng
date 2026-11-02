//! Line protocol shared with the unchanged C bridge; synthetic fixtures only.
use std::io::{self, BufRead};
use tsfp_kernel_object::*;
const BASE: u32 = 0x22000000;
const SIZE: usize = 16384;
struct Memory {
    bytes: [u8; SIZE],
    writable: usize,
    readable: usize,
}
impl GuestMemory for Memory {
    fn readable(&self, a: GuestAddress, n: u32) -> bool {
        a.0 >= BASE && (a.0 as u64 + n as u64) <= BASE as u64 + self.readable as u64
    }
    fn read_word(&self, a: GuestAddress) -> Option<u32> {
        if !self.readable(a, 4) {
            return None;
        }
        let i = (a.0 - BASE) as usize;
        Some(u32::from_le_bytes(self.bytes[i..i + 4].try_into().unwrap()))
    }
    fn write_word(&mut self, a: GuestAddress, v: u32) -> bool {
        if !self.readable(a, 4) {
            return false;
        }
        let i = (a.0 - BASE) as usize;
        let n = self.writable.saturating_sub(i).min(4);
        self.bytes[i..i + n].copy_from_slice(&v.to_le_bytes()[..n]);
        n == 4
    }
}
fn hash_word(h: &mut u64, v: u32) {
    for b in v.to_le_bytes() {
        *h = (*h ^ b as u64).wrapping_mul(1099511628211);
    }
}
fn snapshot(t: &ObjectTable, m: &Memory, result: u32) -> String {
    let mut h = 14695981039346656037u64;
    for (slot, e) in t.entries().iter().enumerate() {
        let mut words = [0; 10];
        if let Some(e) = e {
            words[0] = e.handle.0;
            words[1] = e.kind as u32;
            words[2] = e.owner.0;
            words[3] = e.references;
            words[4] = 1;
            match e.state {
                ObjectState::Thread {
                    open,
                    body,
                    body_references,
                } => {
                    words[5] = open as u32;
                    words[6] = body.0;
                    words[7] = body_references;
                }
                ObjectState::File { duplicate_root } => words[8] = duplicate_root.0,
                ObjectState::Mutant { owner, depth } => {
                    words[8] = owner.0;
                    words[9] = depth;
                }
                _ => {}
            }
        }
        for w in words {
            hash_word(&mut h, w);
        }
        hash_word(&mut h, t.generations()[slot] as u32);
    }
    hash_word(&mut h, t.fresh_count() as u32);
    let free: Vec<_> = t.free_slots().collect();
    hash_word(&mut h, free.len() as u32);
    for slot in free {
        hash_word(&mut h, slot as u32);
    }
    let mut mh = 14695981039346656037u64;
    for (index, mut b) in m.bytes.into_iter().enumerate() {
        if index >= m.readable {
            b = 0;
        }
        mh = (mh ^ b as u64).wrapping_mul(1099511628211);
    }
    let d = &t.diagnostics;
    let mut dh = 14695981039346656037u64;
    for message in &d.messages {
        hash_word(&mut dh, *message as u32);
    }
    format!(
        "{result} {h} {mh} {} {} {} {} {} {} {} {dh}",
        t.live_count(),
        t.retired_count(),
        d.bad_close,
        d.references_by_handle,
        d.unchecked_types,
        d.unowned_releases,
        d.messages.len()
    )
}
fn main() {
    let mut t = ObjectTable::default();
    let mut m = Memory {
        bytes: [0; SIZE],
        writable: SIZE,
        readable: SIZE,
    };
    let mut files = NoFileBodies;
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let p: Vec<u32> = line
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        assert_eq!(p.len(), 10);
        let [op, a, b, c, d, e, f, g, h, i] = p[..].try_into().unwrap();
        let result = match op {
            0 => {
                t.reset();
                m.bytes.fill(0);
                m.writable = SIZE;
                m.readable = SIZE;
                0
            }
            1 => {
                let kind = match a {
                    0 => Kind::None,
                    1 => Kind::Thread,
                    2 => Kind::File,
                    3 => Kind::Event,
                    4 => Kind::Semaphore,
                    5 => Kind::Mutant,
                    6 => Kind::Symlink,
                    _ => Kind::Other,
                };
                t.create(kind, Owner(b)).0
            }
            2 => {
                for (n, v) in [0, b, c, d].into_iter().enumerate() {
                    m.write_word(GuestAddress(BASE + 64 + 4 * n as u32), v);
                }
                let frame = if e == u32::MAX {
                    None
                } else {
                    Some(CallFrame {
                        stack: GuestAddress(e),
                        limit: GuestAddress(f),
                        ecx: if h != 0 { Some(g) } else { None },
                    })
                };
                match t.dispatch(a, frame, &mut m, Owner(i), &mut files) {
                    CallOutcome::Returned(v) => v,
                    CallOutcome::Unregistered => u32::MAX,
                }
            }
            3 => t.bind_thread_body(Handle(a), Owner(b), GuestAddress(c), &m) as u32,
            4 => {
                t.seed_reference_counts(Handle(a), b, c);
                0
            }
            5 => t.set_mutant_ownership(Handle(a), Owner(b), c) as u32,
            6 => m.write_word(GuestAddress(a), b) as u32,
            7 => t.detach_thread_bodies() as u32,
            8 => t.release(Handle(a), &mut files) as u32,
            9 => {
                m.writable = if a != 0 { SIZE - 4096 } else { SIZE };
                m.readable = if a == 2 { SIZE - 4096 } else { SIZE };
                0
            }
            10 => {
                t.seed_retirement_boundary();
                0
            }
            12 => t.publish_thread_body(
                Handle(a),
                Owner(b),
                GuestAddress(c),
                GuestAddress(d),
                &mut m,
            ) as u32,
            13 => t.write_optional_thread_id(GuestAddress(a), b, &mut m) as u32,
            14 => t.rollback_thread_body(Handle(a), Owner(b)) as u32,
            15 => t.referenced_thread_body(GuestAddress(a)).is_some() as u32,
            _ => panic!("unknown operation"),
        };
        println!("{}", snapshot(&t, &m, result));
    }
}
