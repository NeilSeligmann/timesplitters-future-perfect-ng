// SPDX-License-Identifier: GPL-3.0-or-later
//! T1815 closure model: call-containing roots, C replacement == Rust port, with call-trace
//! equality and trace-level mutant controls. Child of `wave4b` so it reuses the callee stub
//! (`Stub`, `shim_script`, `shim_trace_*`). See docs/t1815-closure-model.md.
//!
//! Model: every nested guest call lands in a stub on both sides. The stub records the callee
//! address, its ESP, return PC, the first three stack words and the full register file, and
//! returns the scripted (seeded) EAX/ECX/EDX triple. It writes no memory. Per callee the
//! contract is its stack argument count (`callee_argc`) and the clobber set {EAX, ECX, EDX}.
use super::*;

use tsfp_engine_object::closure::callee::{Callee, Cpu};
impl Callee<View> for Stub {
    fn call(&mut self, m: &mut View, target: u32, cpu: &mut Cpu) {
        let regs = [
            cpu.eax, cpu.ecx, cpu.edx, cpu.ebx, cpu.esi, cpu.edi, cpu.ebp,
        ];
        let [eax, ecx, edx] = self.step(m, target, cpu.esp, regs);
        (cpu.eax, cpu.ecx, cpu.edx) = (eax, ecx, edx);
        let index = self.trace.len() as u32 - 1;
        for [call, address, value] in self.writes.clone() {
            if call == index {
                for (i, b) in value.to_le_bytes().into_iter().enumerate() {
                    m.set(address.wrapping_add(i as u32), b);
                }
            }
        }
    }
}
const POOL_C: u32 = 0x0060_0000;

/// Mutations of the port's call behaviour, applied at the call boundary of the Rust side.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Skew {
    None,
    /// The port never makes its first call.
    SkipFirst,
    /// The port passes the first stack argument of every call changed by one bit.
    WrongArg,
    /// The port makes its first two calls in swapped order.
    Reorder,
    /// The Rust callee model ignores the seeded callee memory writes (T1815 w8).
    DropWrites,
}

struct Skewed<'a> {
    inner: &'a mut Stub,
    kind: Skew,
    seen: u32,
}
impl Callee<View> for Skewed<'_> {
    fn call(&mut self, m: &mut View, target: u32, cpu: &mut Cpu) {
        self.seen += 1;
        match self.kind {
            Skew::SkipFirst if self.seen == 1 => {
                // The skipped call also consumes its script row, so later calls keep their
                // results (a shifted row would turn a pointer into a loop count and hang).
                if !self.inner.script.is_empty() {
                    self.inner.script.remove(0);
                }
                return;
            }
            Skew::WrongArg => {
                let argument = m.read32w(cpu.esp.wrapping_add(4)) ^ 1;
                for (i, b) in argument.to_le_bytes().into_iter().enumerate() {
                    m.set(cpu.esp.wrapping_add(4 + i as u32), b);
                }
            }
            _ => {}
        }
        self.inner.call(m, target, cpu);
    }
}

type Port = fn(&mut Skewed, &mut View, &mut Cpu);

/// Result of one skewed run: (cases whose result differs from C, cases where the skew applies).
fn drive_closure(
    seed: u64,
    kind: Skew,
    mut build: impl FnMut(&mut Rng, &mut Pair) -> Case2,
    port: Port,
) -> (usize, usize) {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let pair = &mut *guard;
    let mut rng = Rng(seed);
    let (mut detected, mut eligible, mut with_calls) = (0, 0, 0);
    for index in 0..CASES {
        for word in 0..(FRAME_LEN / 4) {
            let value = rng.next();
            pair.put32(FRAME + 4 * word, value);
        }
        let mut case = build(&mut rng, pair);
        let logged: Vec<u32> = pair.log.drain(..).collect();
        let mut watch = vec![(FRAME, FRAME_LEN)];
        watch.extend(logged.into_iter().map(|address| (address, 4)));
        watch.extend(case.watch.drain(..));
        pair.put32(STACK, 0);
        for (i, arg) in case.args.iter().enumerate() {
            pair.put32(STACK + 4 + 4 * i as u32, *arg);
        }
        pair.log.clear();
        let flat: Vec<u32> = case.script.iter().flatten().copied().collect();
        unsafe { shim_script(flat.as_ptr(), case.script.len() as u32) };
        let wflat: Vec<u32> = case.writes.iter().flatten().copied().collect();
        unsafe { shim_script_writes(wflat.as_ptr(), case.writes.len() as u32) };
        let differing: Vec<u32> = case
            .writes
            .iter()
            .filter(|w| {
                (0..4).any(|i| pair.get(false, w[1].wrapping_add(i)) != w[2].to_le_bytes()[i as usize])
            })
            .map(|w| w[0])
            .collect();
        for w in &case.writes {
            watch.push((w[1], 4));
        }
        let mut regs = [
            case.regs[0],
            case.regs[1],
            case.regs[2],
            case.regs[3],
            case.regs[4],
            case.regs[5],
            case.regs[6],
            STACK,
        ];
        assert_eq!(
            unsafe { shim_run(case.va, regs.as_mut_ptr()) },
            1,
            "unknown root"
        );
        let c_trace: Vec<[u32; 16]> = (0..unsafe { shim_trace_len() })
            .map(|i| unsafe { *(shim_trace_entry(i) as *const [u32; 16]) })
            .collect();
        let mut view = View(pair.rust);
        let mut stub = Stub {
            script: case.script.clone(),
            writes: if kind == Skew::DropWrites { vec![] } else { case.writes.clone() },
            trace: vec![],
        };
        let mut cpu = Cpu {
            eax: case.regs[0],
            ecx: case.regs[1],
            edx: case.regs[2],
            ebx: case.regs[3],
            esi: case.regs[4],
            edi: case.regs[5],
            ebp: case.regs[6],
            esp: STACK,
        };
        {
            let mut skewed = Skewed {
                inner: &mut stub,
                kind,
                seen: 0,
            };
            port(&mut skewed, &mut view, &mut cpu);
        }
        if kind == Skew::Reorder && stub.trace.len() >= 2 {
            stub.trace.swap(0, 1);
        }
        let mut expected = regs_of!(cpu);
        expected[7] = expected[7].wrapping_add(4);
        let applies = match kind {
            Skew::None => false,
            Skew::SkipFirst | Skew::WrongArg => !c_trace.is_empty(),
            Skew::Reorder => c_trace.len() >= 2 && c_trace[0] != c_trace[1],
            Skew::DropWrites => differing.iter().any(|&call| (call as usize) < c_trace.len()),
        };
        with_calls += usize::from(!c_trace.is_empty());
        let mut mismatch = lifted_policy::mismatch(case.va, lifted_policy::diff8(&regs, &expected)) || lifted_policy::trace(case.va, &c_trace, &stub.trace);
        for (start, len) in &watch {
            for offset in 0..*len {
                let address = start.wrapping_add(offset);
                mismatch |= lifted_policy::mem(case.va, address, pair.get(false, address) != pair.get(true, address));
            }
        }
        if kind == Skew::None {
            assert!(
                !mismatch,
                "root {:08X} case {index}: C {regs:x?} trace {c_trace:x?}, Rust {expected:x?} trace {:x?}",
                case.va, stub.trace
            );
        } else {
            // A skew that does not apply must not change the outcome, one that applies must.
            assert_eq!(
                mismatch, applies,
                "root {:08X} case {index} skew {kind:?}",
                case.va
            );
            detected += usize::from(mismatch);
            eligible += usize::from(applies);
            for (start, len) in &watch {
                for offset in 0..*len {
                    let address = start.wrapping_add(offset);
                    let byte = pair.get(false, address);
                    unsafe { *pair.rust.add(address as usize) = byte };
                }
            }
        }
    }
    assert!(
        with_calls > CASES / 20,
        "builder exercises too few calls ({with_calls})"
    );
    (detected, eligible)
}

fn zero_region(p: &mut Pair, start: u32, len: u32) {
    unsafe {
        std::ptr::write_bytes(p.c.add(start as usize), 0, len as usize);
        std::ptr::write_bytes(p.rust.add(start as usize), 0, len as usize);
    }
}

// ---- 0006B4E0 ----
const RECORDS: u32 = 0x006F_1E70;
const RECORD_SLOTS: u32 = (0x006F_2ED8 - RECORDS) / 12 + 1;

fn build_6b4e0(r: &mut Rng, p: &mut Pair) -> Case2 {
    zero_region(p, RECORDS, RECORD_SLOTS * 12 + 12);
    let table = POOL_A + r.below(4) * 0x4000;
    p.put32(0x0072_F5E8, table);
    let mut handles = vec![];
    for owner in 0..6u32 {
        let base = table + owner * 0x2E8;
        zero_region(p, base, 0x2E8);
        let handle = (owner << 22) | (r.next() & 0x003f_ffff) | 1;
        handles.push(handle);
        p.put32(
            base + 0x2C,
            if r.below(8) == 0 {
                handle ^ 0x10
            } else {
                handle
            },
        );
        let predicate = [0, 0, 1, POOL_B + 4 * r.below(64)][r.below(4) as usize];
        p.put32(base + 0x80, predicate);
        p.put32(base + 0x20, if r.below(2) == 0 { 0x40 } else { r.next() });
        p.put32(base + 0x28, r.next());
    }
    let count = 1 + r.below(30);
    for _ in 0..count {
        let slot = r.below(RECORD_SLOTS - 1);
        let address = RECORDS + 12 * slot;
        let handle = match r.below(8) {
            0 => 0,
            1 => r.next(),
            _ => handles[r.below(6) as usize],
        };
        p.put32(address, handle);
        p.put32(address + 4, r.word(&[1, 2, 0x8000_0000, u32::MAX]));
        let flags = (r.next() & 0xff) | if r.below(4) == 0 { 0 } else { 4 };
        p.put32(address + 8, flags);
    }
    let mask = r.word(&[1, 2, 0x8000_0000, u32::MAX]);
    let mut case = mk2(r, 0x6b4e0, vec![mask], 0);
    case.script = script_of(r, 40, &[0, 1, 2]);
    case
}

// ---- 000AF330 ----
fn build_af330(r: &mut Rng, p: &mut Pair) -> Case2 {
    let holder = if r.below(8) == 0 {
        r.below(8) * 4
    } else {
        r.pool(POOL_A)
    };
    let object = r.pool(POOL_B);
    let entry = r.pool(POOL_C);
    let middle = r.pool(POOL_C) + 0x8000;
    let target = match r.below(8) {
        0 => STACK - 4 - 0x30,
        1 => object + 0x54 - 0x30,
        _ => r.pool(POOL_C) + 0x10000,
    };
    p.put32(holder.wrapping_add(0x7C), object);
    p.put32(object + 0x60, r.next());
    p.put32(object + 0x54, r.next());
    p.put32(entry + 8, middle);
    p.put32(middle + 4, target);
    let found = match r.below(4) {
        0 => 0,
        1 => r.below(4),
        _ => entry,
    };
    let mut case = mk2(r, 0xaf330, vec![holder], 0);
    case.script = vec![[found, r.next(), r.next()]];
    case.watch.push((STACK - 4, 4));
    case.watch.push((target + 0x30, 4));
    case
}

// ---- 00160B80 ----
fn build_160b80(r: &mut Rng, p: &mut Pair) -> Case2 {
    let object = r.pool(POOL_A);
    let table = r.pool(POOL_C) & !0x1f;
    p.put32(0x0071_5F2C, table);
    p.put8(
        0x007D_E455,
        if r.below(4) == 0 {
            r.next() as u8
        } else {
            0x0A
        },
    );
    let index = if r.below(8) == 0 {
        r.next()
    } else {
        r.below(8)
    };
    p.put32(object + 0x678, index);
    let holder = if r.below(5) == 0 { 0 } else { r.pool(POOL_B) };
    p.put32(object + 0xBF0, holder);
    p.put32(holder.wrapping_add(0x10), r.next());
    let entry = if r.below(5) == 0 {
        0
    } else {
        r.pool(POOL_C) + 0x20000
    };
    p.put32(
        entry.wrapping_add(4),
        if r.below(4) == 0 {
            r.next()
        } else {
            0x800 | r.next()
        },
    );
    let word = r.next() as u16;
    p.put8(entry.wrapping_add(8), word as u8);
    p.put8(entry.wrapping_add(9), (word >> 8) as u8);
    let record = (index << 5).wrapping_add(table);
    let value = if r.below(2) == 0 {
        i32::from(word as i16) as u32
    } else {
        r.next()
    };
    p.put32(record, value);
    let mut case = mk2(r, 0x160b80, vec![object], 0);
    case.script = vec![[entry, r.next(), r.next()]];
    case
}

fn port_6b4e0(s: &mut Skewed, v: &mut View, c: &mut Cpu) {
    tsfp_engine_object::closure::calls::pending_records_refresh_owner_flag(s, v, c)
}
fn port_af330(s: &mut Skewed, v: &mut View, c: &mut Cpu) {
    tsfp_engine_object::closure::calls::linked_object_flags_from_ext(s, v, c)
}
fn port_160b80(s: &mut Skewed, v: &mut View, c: &mut Cpu) {
    tsfp_engine_object::closure::calls::record_entry_by_field_or_mode_override(s, v, c)
}

macro_rules! closure_suite {
    ($name:ident, $seed:expr, $build:expr, $port:expr) => {
        mod $name {
            use super::*;
            #[test]
            fn c_equals_rust() {
                drive_closure($seed, Skew::None, $build, $port);
            }
            #[test]
            fn skip_first_call_detected() {
                let (detected, eligible) = drive_closure($seed, Skew::SkipFirst, $build, $port);
                assert!(
                    eligible > 0 && detected == eligible,
                    "{detected}/{eligible}"
                );
            }
            #[test]
            fn wrong_argument_detected() {
                let (detected, eligible) = drive_closure($seed, Skew::WrongArg, $build, $port);
                assert!(
                    eligible > 0 && detected == eligible,
                    "{detected}/{eligible}"
                );
            }
            #[test]
            fn reordered_calls_detected() {
                let (detected, eligible) = drive_closure($seed, Skew::Reorder, $build, $port);
                // Single-call roots cannot be reordered: nothing applies and nothing may differ.
                assert_eq!(detected, eligible);
            }
        }
    };
}
closure_suite!(r_0006b4e0, 0x1815_0001, build_6b4e0, port_6b4e0);
closure_suite!(r_000af330, 0x1815_0002, build_af330, port_af330);
closure_suite!(r_00160b80, 0x1815_0003, build_160b80, port_160b80);

// ---- T1815 w8: memory-writing callees and 12 further call roots ----

/// Seeded callee memory writes: each candidate (address, value) becomes, with probability 1/2, a
/// write performed by call `below(calls)`. Addresses must be distinct.
fn writes_of(r: &mut Rng, calls: u32, targets: &[(u32, u32)]) -> Vec<[u32; 3]> {
    let mut out = vec![];
    for &(address, value) in targets {
        if r.below(2) == 0 {
            out.push([r.below(calls), address, value]);
        }
    }
    out
}
const POOL_D: u32 = 0x0070_0000;

fn build_606e0(r: &mut Rng, p: &mut Pair) -> Case2 {
    let table = r.pool(POOL_C) & !3;
    zero_region(p, table, 0x200);
    for word in 0..128 {
        let value = r.below(4);
        p.put32(table + 4 * word, value);
    }
    let count = if r.below(8) == 0 { r.next() } else { r.below(41) };
    p.put32(table + 8, count);
    p.put32(0x006D_780C, table);
    p.put32(0x007B_B260, r.word(&[0, 1, 20, u32::MAX]) & if r.below(4) == 0 { u32::MAX } else { 0x1f });
    p.put32(0x007B_B288, if r.below(3) == 0 { r.next() | 1 } else { 0 });
    let index = r.word(&[0, 1, 19, u32::MAX, 0x8000_0000]);
    let mut case = mk2(r, 0x606e0, vec![index], 0);
    case.script = script_of(r, 1, &[0, 1]);
    let value = r.next();
    case.writes = writes_of(r, 1, &[(STACK - 4, value)]);
    case
}

fn build_60740(r: &mut Rng, p: &mut Pair) -> Case2 {
    build_60740_alias(r, p, false)
}
fn build_60740_aliased(r: &mut Rng, p: &mut Pair) -> Case2 {
    build_60740_alias(r, p, true)
}
/// With `alias` a destination may overlap the record (the copy then overwrites record words, so
/// the drop-writes control cannot see every seeded write: only the equality test uses it).
fn build_60740_alias(r: &mut Rng, p: &mut Pair, alias: bool) -> Case2 {
    let record = if r.below(5) == 0 { 0 } else { r.pool(POOL_C) & !3 };
    zero_region(p, POOL_C + 0x40000, 0x400);
    for word in 0..8 {
        let value = r.next();
        p.put32(record.wrapping_add(4 * word), value);
    }
    let dst = |r: &mut Rng, slot: u32| -> u32 {
        match r.below(5) {
            0 => 0,
            1 if alias => record.wrapping_add(4 * r.below(4)),
            _ => POOL_D + slot * 0x100 + 4 * r.below(8),
        }
    };
    let (d16, d12, d4) = (dst(r, 0), dst(r, 1), dst(r, 2));
    let index = r.next();
    let mut case = mk2(r, 0x60740, vec![index, d16, d12, d4], 0);
    case.script = vec![[record, r.next(), r.next()]];
    let (v1, v2, v3, v4) = (r.next(), r.next(), POOL_D + 0x300 + 4 * r.below(8), r.next());
    case.writes = writes_of(
        r,
        1,
        &[(record.wrapping_add(0x1C), v1), (record.wrapping_add(0x10), v4), (STACK + 8, v3), (STACK + 16, v3 + 0x40), (STACK + 12, v2)],
    );
    case
}

fn build_15c390(r: &mut Rng, p: &mut Pair) -> Case2 {
    let object = r.pool(POOL_A);
    let owner = r.pool(POOL_B);
    let target = if r.below(8) == 0 { owner } else { r.pool(POOL_C) };
    zero_region(p, owner & !0xff, 0x200);
    p.put32(object + 0x7C, owner);
    p.put32(owner + 0x2C, if r.below(4) == 0 { r.next() | 4 } else { r.next() & !4 });
    p.put32(owner, target);
    p.put32(owner + 8, r.word(&[0, 5, u32::MAX, 0x8000_0000]));
    p.put32(target + 0x28, r.next());
    p.put32(0x0079_0950, r.word(&[0, 5, u32::MAX, 0x8000_0000]));
    let mut case = mk2(r, 0x15c390, vec![object], 0);
    case.script = vec![[r.next(), r.next(), r.next()]];
    let (a, b) = (r.next(), r.next());
    case.writes = writes_of(r, 1, &[(target + 0x28, a), (owner + 0x2C, b)]);
    case
}

fn build_17a190(r: &mut Rng, p: &mut Pair) -> Case2 {
    let object = r.pool(POOL_A);
    let table = POOL_B + r.below(4) * 0x4000;
    p.put32(0x0072_F5E8, table);
    let row = r.below(6);
    let record = if r.below(6) == 0 { 0 } else { r.pool(POOL_C) & !3 };
    let found = if r.below(5) == 0 { 0 } else { POOL_D + 0x1000 + 4 * r.below(64) };
    for owner in 0..6u32 {
        let base = table + owner * 0x2E8;
        zero_region(p, base, 0x2E8);
        p.put32(base + 0x2C, (owner << 22) | 1);
        p.put32(base + 0x7C, if owner == row { record } else { r.next() });
    }
    let handle = match r.below(6) {
        0 => 0,
        1 => r.next(),
        2 => ((row << 22) | 1) ^ 0x10,
        _ => (row << 22) | 1,
    };
    p.put32(object + 0xC08, handle);
    p.put32(record.wrapping_add(0x164), r.word(&[0, 1, 6, u32::MAX]) & 7 | if r.below(8) == 0 { 0x8000_0000 } else { 0 });
    p.put32(found.wrapping_add(0x2C), r.word(&[0, 1, 3, 7, u32::MAX]));
    let mut case = mk2(r, 0x17a190, vec![object], 0);
    case.script = vec![[found, r.next(), r.next()]];
    let (a, b, c) = (r.below(8), r.below(8), r.next());
    case.writes = writes_of(r, 1, &[(found.wrapping_add(0x2C), a), (record.wrapping_add(0x164), b), (STACK - 4, c)]);
    case
}

fn build_18ac60(r: &mut Rng, p: &mut Pair) -> Case2 {
    let found = if r.below(4) == 0 { 0 } else { r.pool(POOL_B) & !3 };
    let child = if r.below(8) == 0 { found.wrapping_sub(0x114).wrapping_add(0x7C) } else { r.pool(POOL_C) };
    p.put32(found.wrapping_add(0x7C), child);
    p.put32(child.wrapping_add(0x114), r.next());
    let key = r.next();
    let mut case = mk2(r, 0x18ac60, vec![key], 0);
    case.script = vec![[found, r.next(), r.next()]];
    let other = r.pool(POOL_C);
    case.writes = writes_of(r, 1, &[(found.wrapping_add(0x7C), other)]);
    case
}

fn build_1ec8e0(r: &mut Rng, p: &mut Pair) -> Case2 {
    p.put32(0x0075_BF0C, r.word(&[0, 1, 2, u32::MAX]));
    let mut case = mk2(r, 0x1ec8e0, vec![], 0);
    case.script = vec![[if r.below(2) == 0 { 0 } else { r.next() | 1 }, r.next(), r.next()]];
    let v = r.below(3);
    case.writes = writes_of(r, 1, &[(0x0075_BF0C, v)]);
    case
}

fn build_gas_single(va: u32) -> impl FnMut(&mut Rng, &mut Pair) -> Case2 {
    move |r, p| {
        p.put32(0x0075_BF0C, r.word(&[0, 1, 2, u32::MAX]));
        let record = if r.below(4) == 0 { 0 } else { r.pool(POOL_C) & !3 };
        p.put32(record.wrapping_add(0x34), if r.below(2) == 0 { 0x4C } else { r.word(&[0x4C, 0x4D]) });
        let mut case = mk2(r, va, vec![], 0);
        case.script = vec![[record, r.next(), r.next()]];
        let (a, b, c) = (r.word(&[0x4C]), r.below(2), r.next());
        case.writes = writes_of(r, 1, &[(record.wrapping_add(0x34), a), (0x0075_BF0C, b), (STACK - 4, c)]);
        case
    }
}

fn build_1eca00(r: &mut Rng, p: &mut Pair) -> Case2 {
    let first = if r.below(4) == 0 { 0 } else { r.pool(POOL_C) & !3 };
    let second = match r.below(5) {
        0 => 0,
        1 => first,
        _ => (r.pool(POOL_C) & !3) + 0x10000,
    };
    for rec in [first, second] {
        p.put32(rec.wrapping_add(0x34), if r.below(2) == 0 { 0x4C } else { r.word(&[0x4C, 0x4D]) });
    }
    let mut case = mk2(r, 0x1eca00, vec![], 0);
    case.script = vec![[first, r.next(), r.next()], [second, r.next(), r.next()]];
    let (a, b, c, d) = (r.word(&[0x4C]), r.word(&[0x4C]), r.next(), r.next());
    case.writes = writes_of(r, 2, &[(first.wrapping_add(0x34), a), (second.wrapping_add(0x34).wrapping_add(if second == first { 0x40 } else { 0 }), b), (STACK - 4, c), (STACK - 8, d)]);
    case
}

fn build_2346a0(r: &mut Rng, p: &mut Pair) -> Case2 {
    let object = if r.below(6) == 0 { 0 } else { r.pool(POOL_B) & !3 };
    let keys = r.pool(POOL_A) & !3;
    let value = r.word(&[0, 7, 0x8000_0000, u32::MAX]);
    p.put32(object.wrapping_add(0x678), r.next());
    for slot in 1..5u32 {
        let word = match r.below(5) {
            0 => value,
            1 => value ^ 1,
            2 => r.word(&[0x8000_0000, u32::MAX]),
            _ => r.below(8),
        };
        p.put32(keys + 4 * slot, word);
    }
    let mut case = mk2(r, 0x2346a0, vec![keys, object], 0);
    case.script = vec![[value, r.next(), r.next()]];
    let (a, b) = (r.below(8), value);
    case.writes = writes_of(r, 1, &[(keys + 8, b), (keys + 0x10, a), (object.wrapping_add(0x678), a)]);
    case
}

fn build_247a10(r: &mut Rng, p: &mut Pair) -> Case2 {
    let base = POOL_C + 0x40000 + r.below(4) * 0x1000;
    let count = if r.below(5) == 0 { 0 } else { 1 + r.below(6) };
    let id = r.word(&[0x230, 1]);
    zero_region(p, base, 0x600);
    let mut candidates = vec![];
    for entry in 0..count {
        let at = base + entry * 0x5C;
        let object = if r.below(4) == 0 { 0 } else { POOL_D + 0x2000 + entry * 0x100 };
        p.put32(at + 4, if r.below(3) == 0 { 0 } else { r.next() | 1 });
        p.put32(at + 0xC, object);
        p.put32(object.wrapping_add(0x230), if r.below(3) == 0 { id } else { r.below(4) });
        candidates.push(object);
    }
    let mut case = mk2(r, 0x247a10, vec![id], 0);
    case.script = vec![[base, r.next(), r.next()], [count, r.next(), r.next()]];
    let mut targets = vec![];
    if let Some(&object) = candidates.iter().find(|o| **o != 0) {
        targets.push((object + 0x230, id));
    }
    targets.push((base + 4, r.next() | 1));
    targets.push((STACK - 4, r.next()));
    targets.push((STACK - 8, r.next()));
    case.writes = writes_of(r, 2, &targets);
    case
}

fn build_248910(r: &mut Rng, p: &mut Pair) -> Case2 {
    let object = r.pool(POOL_A) & !3;
    let state = r.pool(POOL_B) & !3;
    p.put32(object + 0x150, state);
    p.put32(state + 0x114, if r.below(4) == 0 { r.next() } else { 0x1000 });
    let mut script = vec![];
    let mut targets = vec![];
    for call in 0..4u32 {
        let base = POOL_C + 0x40000 + call * 0x1000;
        zero_region(p, base, 0x400);
        for entry in 0..5u32 {
            let at = base + entry * 0x5C;
            p.put32(at + 4, if r.below(2) == 0 { 0 } else { r.next() | 1 });
            p.put32(at + 0x10, if r.below(4) == 0 { object } else { r.next() });
            p.put32(at + 0xC, r.next());
        }
        let used = base + call * 0x5C;
        targets.push((used + 0x10, object));
        script.push([base, r.next(), r.next()]);
    }
    targets.push((STACK - 12, r.next()));
    targets.push((STACK - 4, r.next()));
    let mut case = mk2(r, 0x248910, vec![object], 0);
    case.script = script;
    case.writes = writes_of(r, 4, &targets);
    case
}

macro_rules! port_fn {
    ($name:ident, $path:ident) => {
        fn $name(s: &mut Skewed, v: &mut View, c: &mut Cpu) {
            tsfp_engine_object::closure::calls_w8::$path(s, v, c)
        }
    };
}
port_fn!(port_606e0, clamped_index_to_record_pointer);
port_fn!(port_60740, record_by_index_copy_to_optional_outputs);
port_fn!(port_15c390, owner_mark_pending_and_remove_from_active_list);
port_fn!(port_17a190, object_handle_to_owner_row_pointer);
port_fn!(port_18ac60, object_by_key_set_child_field_0x114);
port_fn!(port_1ec8e0, object_gas0_absent_and_global_is_one);
port_fn!(port_1ec980, object_key_gas0_type_0x4c);
port_fn!(port_1ec9c0, object_key_gas1_type_0x4c);
port_fn!(port_1eca00, object_keys_gas0_or_gas1_type_0x4c);
port_fn!(port_2346a0, object_key_matches_any_of_four);
port_fn!(port_247a10, pool_find_entry_object_by_sequence_id);
port_fn!(port_248910, pool_find_entry_object_by_owner_when_state_0x1000);

fn build_db480(r: &mut Rng, p: &mut Pair) -> Case2 {
    let object = r.pool(POOL_A) & !3;
    let count = if r.below(6) == 0 { 0 } else { 1 + r.below(5) };
    zero_region(p, POOL_D + 0x4000, 0x400);
    let mut head = 0;
    for i in (0..count).rev() {
        let node = POOL_D + 0x4000 + i * 0x40;
        p.put32(node, r.below(3));
        p.put32(node + 4, r.below(3));
        p.put32(node + 0x18, head);
        head = node;
    }
    p.put32(object + 0x230, head);
    p.put32(object + 0x234, r.next());
    p.put32(object + 0x238, r.next());
    let (k1, k2) = (r.below(3), r.below(3));
    let mut case = mk2(r, 0xdb480, vec![object, k1, k2], 0);
    case.script = vec![[r.next(), r.next(), r.next()]];
    let (a, b, c) = (r.next(), r.next(), r.next());
    case.writes = writes_of(r, 1, &[(object + 0x234, a), (object + 0x238, b), (STACK - 4, c)]);
    case
}

fn build_188840(r: &mut Rng, p: &mut Pair) -> Case2 {
    build_188840_alias(r, p, false)
}
fn build_188840_aliased(r: &mut Rng, p: &mut Pair) -> Case2 {
    build_188840_alias(r, p, true)
}
/// `alias` lets the final store hit the saved-ESI slot, which hides a dropped seeded write there
/// (only the equality test uses it).
fn build_188840_alias(r: &mut Rng, p: &mut Pair, alias: bool) -> Case2 {
    let object = match r.below(8) {
        0 => STACK - 0x34,
        1 => STACK - 8 - 0x28,
        2 if alias => STACK - 4 - 0x1A0,
        _ => r.pool(POOL_B) & !3,
    };
    p.put32(object.wrapping_add(0x28), r.next());
    let value = r.next();
    let mut case = mk2(r, 0x188840, vec![value, object], 0);
    case.script = vec![[r.next(), r.next(), r.next()]];
    case.watch.push((object.wrapping_add(0x198), 0x10));
    let (a, b) = (r.next(), r.next());
    case.writes = writes_of(r, 1, &[(STACK - 4, a), (STACK + 4, b)]);
    case
}

fn build_ae910(r: &mut Rng, _p: &mut Pair) -> Case2 {
    let mut case = mk2(r, 0xae910, vec![], 0);
    case.script = vec![[r.next(), r.next(), r.next()]];
    let a = r.next();
    case.writes = writes_of(r, 1, &[(0x0073_29F0, a), (STACK - 16, a ^ 1)]);
    case
}
port_fn!(port_db480, object_list_find_node_by_key_pair_and_move_to_tail);
port_fn!(port_188840, object_set_flag_install_callback_store_value);
port_fn!(port_ae910, pool_7329f0_init);

const GLOBALS: [u32; 4] = [0, 1, 0x100, u32::MAX];

fn simple_script(r: &mut Rng, rows: usize) -> Vec<[u32; 3]> {
    script_of(r, rows, &GLOBALS)
}
fn build_190680(r: &mut Rng, _p: &mut Pair) -> Case2 {
    let mut case = mk2(r, 0x190680, vec![], 0);
    case.script = simple_script(r, 1);
    let v = r.next();
    case.writes = writes_of(r, 1, &[(0x007B_9798, v), (STACK - 12, v ^ 3)]);
    case.watch.push((0x007B_9798, 4));
    case
}
fn build_190a70(r: &mut Rng, _p: &mut Pair) -> Case2 {
    let mut case = mk2(r, 0x190a70, vec![], 0);
    case.script = simple_script(r, 2);
    let v = r.next();
    case.writes = writes_of(r, 2, &[(0x007B_979C, v)]);
    case
}
fn build_190690(r: &mut Rng, _p: &mut Pair) -> Case2 {
    let mut case = mk2(r, 0x190690, vec![], 0);
    case.script = simple_script(r, 2);
    let v = r.next();
    case.writes = writes_of(r, 2, &[(0x007B_979C, v)]);
    case
}
fn build_190ac0(r: &mut Rng, _p: &mut Pair) -> Case2 {
    let index = r.next();
    let mut case = mk2(r, 0x190ac0, vec![index], 0);
    case.script = simple_script(r, 1);
    let v = r.next();
    case.writes = writes_of(r, 1, &[(STACK + 4, v)]);
    case
}
fn build_240560(r: &mut Rng, p: &mut Pair) -> Case2 {
    let found = if r.below(3) == 0 { 0 } else { r.pool(POOL_B) & !3 };
    p.put32(found.wrapping_add(0x7C), r.next());
    let key = r.next();
    let mut case = mk2(r, 0x240560, vec![key], 0);
    case.script = vec![[found, r.next(), r.next()]];
    let v = r.next();
    case.writes = writes_of(r, 1, &[(found.wrapping_add(0x7C), v)]);
    case
}
fn build_1de560(r: &mut Rng, p: &mut Pair) -> Case2 {
    let found = r.pool(POOL_B) & !3;
    let inner = r.pool(POOL_C) & !3;
    p.put32(found.wrapping_add(0x7C), inner);
    p.put32(inner.wrapping_add(0xAC), if r.below(2) == 0 { 0 } else { r.next() });
    p.put32(0x7C, r.next());
    p.put32(0xAC, 0);
    let mut case = mk2(r, 0x1de560, vec![], 0);
    case.script = vec![[if r.below(6) == 0 { 0 } else { found }, r.next(), r.next()]];
    let v = r.below(3);
    case.writes = writes_of(r, 1, &[(inner.wrapping_add(0xAC), v), (found.wrapping_add(0x7C), inner + 0x1000)]);
    case
}
fn build_24d260(r: &mut Rng, p: &mut Pair) -> Case2 {
    let found = if r.below(4) == 0 { 0 } else { r.pool(POOL_B) & !3 };
    for k in [0x64, 0x68, 0x6C] {
        p.put32(found.wrapping_add(k), r.next());
    }
    let dst = POOL_D + 0x6000 + 4 * r.below(16);
    let mut case = mk2(r, 0x24d260, vec![dst], 0);
    case.script = vec![[found, r.next(), r.next()]];
    case.watch.push((dst, 12));
    let (a, b) = (r.next(), r.next());
    case.writes = writes_of(r, 1, &[(found.wrapping_add(0x68), a), (found.wrapping_add(0x6C), b), (STACK + 4, dst + 0x100)]);
    case
}
fn build_1cd560(r: &mut Rng, p: &mut Pair) -> Case2 {
    p.put32(0x0075_AEF4, if r.below(3) == 0 { r.next() | 1 } else { 0 });
    let profile = r.pool(POOL_B) & !3;
    p.put32(profile + 0xBF0, r.next());
    let bit = r.next();
    let mut case = mk2(r, 0x1cd560, vec![bit], 0);
    case.script = vec![[profile, r.next(), r.next()]];
    let v = r.next();
    case.writes = writes_of(r, 1, &[(profile + 0xBF0, v), (STACK + 4, bit ^ v)]);
    case
}
fn build_25ef30(r: &mut Rng, p: &mut Pair) -> Case2 {
    let player = r.pool(POOL_B) & !3;
    let table = POOL_C + 0x50000;
    zero_region(p, table, 0x1000);
    let index = r.below(3);
    p.put32(0x007B_0C7C, player);
    p.put32(player, index);
    p.put32(0x0075_F5D0, table);
    p.put32(table + index * 0x238 + 4, r.next());
    p.put32(0x007D_E45C, if r.below(2) == 0 { 0x10000 } else { r.below(3) });
    let bit = r.next();
    let mut case = mk2(r, 0x25ef30, vec![bit], 0);
    case.script = vec![[if r.below(2) == 0 { 0 } else { r.next() | 1 }, r.next(), r.next()]];
    let v = r.next();
    case.writes = writes_of(r, 1, &[(table + index * 0x238 + 4, v), (0x007D_E45C, 0x10000)]);
    case
}
fn build_1b7e80(r: &mut Rng, p: &mut Pair) -> Case2 {
    let base = POOL_C + 0x60000;
    zero_region(p, base, 0x29C * 4);
    p.put32(0x004F_9BAC, base);
    p.put8(0x007D_E455, if r.below(4) == 0 { r.next() as u8 } else { 0x0A });
    let index = if r.below(8) == 0 { r.next() | 0x8000_0000 } else { r.below(3) };
    let at = base.wrapping_add(index.wrapping_mul(0x29C));
    p.put32(at.wrapping_add(0x1F8), r.word(&[0x60A, 0x8000_0000, u32::MAX]));
    p.put32(at.wrapping_add(0xC), r.next());
    let mut case = mk2(r, 0x1b7e80, vec![index], 0);
    case.script = simple_script(r, 1);
    let v = r.next();
    case.writes = writes_of(r, 1, &[(at.wrapping_add(0x1F8), v), (0x007D_E455, v)]);
    case
}
fn build_208ac0(r: &mut Rng, _p: &mut Pair) -> Case2 {
    let mut case = mk2(r, 0x208ac0, vec![], 0);
    case.script = simple_script(r, 2);
    let v = r.next();
    case.writes = writes_of(r, 2, &[(STACK - 16, v)]);
    case
}
port_fn!(port_190680, set_globals_to_4_return_3);
port_fn!(port_190a70, memory_unit_current_slot_field_0x4);
port_fn!(port_190690, memory_unit_current_slot_active_one_else_six);
port_fn!(port_190ac0, scaled_table_byte_zero_extended);
port_fn!(port_240560, field_7c_of_object_found_by_key);
port_fn!(port_1de560, object_key_draw_field7c_ac_zero);
port_fn!(port_24d260, position_of_tagged_object);
port_fn!(port_1cd560, profile_bit_set_or_override);
port_fn!(port_25ef30, player_record_bit_test_returns_two_else_default);
port_fn!(port_1b7e80, record_29c_id_or_default);
port_fn!(port_208ac0, slot_4c_status_above_1_and_not_5);

macro_rules! closure_suite_w {
    ($name:ident, $seed:expr, $build:expr, $port:expr) => {
        mod $name {
            use super::*;
            #[test]
            fn c_equals_rust() {
                drive_closure($seed, Skew::None, $build, $port);
            }
            #[test]
            fn skip_first_call_detected() {
                let (d, e) = drive_closure($seed, Skew::SkipFirst, $build, $port);
                assert!(e > 0 && d == e, "{d}/{e}");
            }
            #[test]
            fn wrong_argument_detected() {
                let (d, e) = drive_closure($seed, Skew::WrongArg, $build, $port);
                assert!(e > 0 && d == e, "{d}/{e}");
            }
            #[test]
            fn reordered_calls_detected() {
                let (d, e) = drive_closure($seed, Skew::Reorder, $build, $port);
                assert_eq!(d, e);
            }
            /// Negative control for the callee memory-write model: ignoring the seeded writes on
            /// the Rust side must be detected in every case where a write changes memory.
            #[test]
            fn dropped_callee_writes_detected() {
                let (d, e) = drive_closure($seed, Skew::DropWrites, $build, $port);
                assert!(e > 0 && d == e, "{d}/{e}");
            }
        }
    };
}
closure_suite_w!(r_000606e0, 0x1815_0101, build_606e0, port_606e0);
closure_suite_w!(r_00060740, 0x1815_0102, build_60740, port_60740);
#[test]
fn r_00060740_aliased_destinations_equal() {
    drive_closure(0x1815_0112, Skew::None, build_60740_aliased, port_60740);
}
closure_suite_w!(r_0015c390, 0x1815_0103, build_15c390, port_15c390);
closure_suite_w!(r_0017a190, 0x1815_0104, build_17a190, port_17a190);
closure_suite_w!(r_0018ac60, 0x1815_0105, build_18ac60, port_18ac60);
closure_suite_w!(r_001ec8e0, 0x1815_0106, build_1ec8e0, port_1ec8e0);
closure_suite_w!(r_001ec980, 0x1815_0107, build_gas_single(0x1ec980), port_1ec980);
closure_suite_w!(r_001ec9c0, 0x1815_0108, build_gas_single(0x1ec9c0), port_1ec9c0);
closure_suite_w!(r_001eca00, 0x1815_0109, build_1eca00, port_1eca00);
closure_suite_w!(r_002346a0, 0x1815_010a, build_2346a0, port_2346a0);
closure_suite_w!(r_00247a10, 0x1815_010b, build_247a10, port_247a10);
closure_suite_w!(r_00248910, 0x1815_010c, build_248910, port_248910);

/// Single source of truth: argc comes from the C shim (`shim_callee_argc`), a callee the C does not
/// know is rejected, and every callee the ports use is known with the documented count.
#[test]
fn callee_argc_single_source() {
    let table: &[(u32, i32)] = &[
        (0x1656D0, 1), (0xB7600, 1), (0x9CCA0, 1), (0x60410, 2), (0x606E0, 1), (0x15BCC0, 1),
        (0x17A120, 1), (0x23FF20, 1), (0x9A4C0, 1), (0x156E50, 1), (0x156E80, 1), (0x15BA10, 1),
        (0x3C8960, 3), (0x3533F0, 0), (0xDB3B0, 2), (0x827B0, 1), (0x827F0, 0), (0x24E40, 1), (0x69630, 0), (0x35DE60, 0), (0x153B40, 0), (0x2273C0, 1), (0x9E720, 2), (0x156DB0, 4),
    ];
    for &(va, argc) in table {
        assert_eq!(unsafe { shim_callee_argc(va) }, argc, "{va:x}");
        assert_eq!(callee_argc(va) as i32, argc);
    }
    assert_eq!(unsafe { shim_callee_argc(0xDEAD0) }, -1);
}
closure_suite_w!(r_000db480, 0x1815_010d, build_db480, port_db480);
closure_suite_w!(r_00188840, 0x1815_010e, build_188840, port_188840);
closure_suite_w!(r_000ae910, 0x1815_010f, build_ae910, port_ae910);
#[test]
fn r_00188840_saved_esi_alias_equal() {
    drive_closure(0x1815_011e, Skew::None, build_188840_aliased, port_188840);
}
closure_suite_w!(r_00190680, 0x1815_0201, build_190680, port_190680);
closure_suite_w!(r_00190a70, 0x1815_0202, build_190a70, port_190a70);
closure_suite_w!(r_00190690, 0x1815_0203, build_190690, port_190690);
closure_suite_w!(r_00190ac0, 0x1815_0204, build_190ac0, port_190ac0);
closure_suite_w!(r_00240560, 0x1815_0205, build_240560, port_240560);
closure_suite_w!(r_001de560, 0x1815_0206, build_1de560, port_1de560);
closure_suite_w!(r_0024d260, 0x1815_0207, build_24d260, port_24d260);
closure_suite_w!(r_001cd560, 0x1815_0208, build_1cd560, port_1cd560);
closure_suite_w!(r_0025ef30, 0x1815_0209, build_25ef30, port_25ef30);
closure_suite_w!(r_001b7e80, 0x1815_020a, build_1b7e80, port_1b7e80);
closure_suite_w!(r_00208ac0, 0x1815_020b, build_208ac0, port_208ac0);

// ---- T1836 wave 13: call-bearing gameplay and ui roots (docs/t1836-rust-wave13.md) ----

fn build_2ef160(r: &mut Rng, p: &mut Pair) -> Case2 {
    let context = r.pool(POOL_B) & !3;
    let mode = r.below(6);
    let profile = r.below(4);
    p.put32(0x007D_E454, (r.next() & 0xFF00_00FF) | (mode << 8) | (profile << 16));
    let kind = [0u32, 1, 2, 2, r.next()][r.below(5) as usize];
    p.put32((profile << 5) + 0x004D_1F84, kind);
    p.put32(context + mode * 20 + 0x24, if r.below(2) == 0 { 0 } else { r.next() });
    let mut case = mk2(r, 0x2ef160, vec![], 0);
    case.script = vec![[context, r.next(), r.next()]];
    case
}
fn build_2e4470(r: &mut Rng, p: &mut Pair) -> Case2 {
    let mode = [0x0Au32, 0x0A, 0x0A, r.below(16)][r.below(4) as usize];
    p.put32(0x007D_E454, (r.next() & 0xFFFF_00FF) | (mode << 8));
    p.put32(0x0079_0950, [0u32, 1, 2, 4, u32::MAX, r.next()][r.below(6) as usize]);
    p.put32(0x007D_E458, if r.below(2) == 0 { 0x200 } else { r.next() });
    let mut case = mk2(r, 0x2e4470, vec![], 0);
    case.script = vec![[[0u32, 1, r.next()][r.below(3) as usize], r.next(), r.next()]];
    case
}
fn build_22de90(r: &mut Rng, p: &mut Pair) -> Case2 {
    p.put32(0x007D_E470, [r.below(12), u32::MAX, 0x8000_0000, r.next()][r.below(4) as usize]);
    let mut case = mk2(r, 0x22de90, vec![], 0);
    case.script = vec![[[0u32, 0, 1, r.next()][r.below(4) as usize], r.next(), r.next()]];
    case
}
fn build_2bfec0(r: &mut Rng, p: &mut Pair) -> Case2 {
    let context = r.pool(POOL_B) & !3;
    p.put32(0x0078_ADAC, (r.next() & 0xFF00_FFFF) | ([0x13u32, 0x13, 0x13, r.below(256)][r.below(4) as usize] << 16));
    p.put32(context + 0xC, [r.below(12), u32::MAX, r.next()][r.below(3) as usize]);
    let count = [r.below(12), u32::MAX, 0x8000_0000, r.next()][r.below(4) as usize];
    let mut case = mk2(r, 0x2bfec0, vec![count], 0);
    case.script = vec![[context, r.next(), r.next()]];
    case
}
fn build_66280_wrapper(va: u32) -> impl FnMut(&mut Rng, &mut Pair) -> Case2 {
    move |r, _| {
        let (first, second) = (r.next(), r.next());
        let mut case = mk2(r, va, vec![first, second], 0);
        case.script = vec![[r.next(), r.next(), r.next()]];
        case
    }
}

macro_rules! port_gameplay {
    ($name:ident, $path:ident) => {
        fn $name(s: &mut Skewed, v: &mut View, c: &mut Cpu) {
            tsfp_gameplay::call_leaves::$path(s, v, c)
        }
    };
}
macro_rules! port_ui {
    ($name:ident, $path:ident) => {
        fn $name(s: &mut Skewed, v: &mut View, c: &mut Cpu) {
            tsfp_ui::call_leaves::$path(s, v, c)
        }
    };
}
port_gameplay!(port_2ef160, profile_unlock_flag_is_set);
port_gameplay!(port_2e4470, multiplayer_pair_state_active);
port_ui!(port_22de90, state_6_to_8_unless_blocked);
port_ui!(port_2bfec0, setup_slots_available);
port_ui!(port_662d0, wrapper_66280_mode_0);
port_ui!(port_662f0, wrapper_66280_mode_1);

closure_suite!(r_002ef160, 0x1836_1301, build_2ef160, port_2ef160);
closure_suite!(r_002e4470, 0x1836_1302, build_2e4470, port_2e4470);
closure_suite!(r_0022de90, 0x1836_1303, build_22de90, port_22de90);
closure_suite!(r_002bfec0, 0x1836_1304, build_2bfec0, port_2bfec0);
closure_suite!(r_000662d0, 0x1836_1305, build_66280_wrapper(0x662d0), port_662d0);
closure_suite!(r_000662f0, 0x1836_1306, build_66280_wrapper(0x662f0), port_662f0);

#[test]
fn wave13_callee_argc_pins() {
    for (va, argc) in [(0x69640u32, 0), (0xCB0F0, 0), (0x64D40, 0), (0x66280, 3)] {
        assert_eq!(unsafe { shim_callee_argc(va) }, argc, "{va:x}");
    }
}

// ---- T1836 wave 14: remaining call-bearing roots (docs/t1836-rust-wave14.md) ----

fn build_12fe0(r: &mut Rng, p: &mut Pair) -> Case2 {
    let entry = r.pool(POOL_B) & !3;
    p.put32(entry + 8, r.next());
    let args = vec![r.next(), r.next()];
    let mut case = mk2(r, 0x12fe0, args, 0);
    case.script = vec![[entry, r.next(), r.next()]];
    case
}
fn build_2b2390(r: &mut Rng, p: &mut Pair) -> Case2 {
    let mut script = vec![];
    let mut count = [0u32, 1, 2, 3, u32::MAX][r.below(5) as usize];
    script.push([count, r.next(), r.next()]);
    for _ in 0..4 {
        let obj = r.pool(POOL_B) & !3;
        let inner = r.pool(POOL_C) & !3;
        p.put32(obj + 4, inner);
        p.put8(inner + 0x28, if r.below(4) == 0 { 1 } else { (r.next() & 0xfe) as u8 });
        script.push([obj, r.next(), r.next()]);
        count = [r.below(5), 4][r.below(2) as usize];
        script.push([count, r.next(), r.next()]);
    }
    let mut case = mk2(r, 0x2b2390, vec![], 0);
    case.script = script;
    case
}
fn build_25f320(r: &mut Rng, p: &mut Pair) -> Case2 {
    let character = r.pool(POOL_A) & !3;
    let ext = r.pool(POOL_B) & !3;
    let brain = r.pool(POOL_C) & !3;
    p.put32(character + 0x7c, ext);
    p.put32(ext + 0xbf0, brain);
    p.put32(brain + 0x6c, if r.below(5) == 0 { r.next() } else { 0x0016_3830 });
    p.put32(brain + 0x38, r.next());
    let index = r.below(8);
    let object = if r.below(4) == 0 { 0 } else { r.pool(POOL_D) & !3 };
    p.put32((index << 5) + 0x78b2e0 + 0x10, object);
    p.put32(object + 0x20, if r.below(3) == 0 { r.next() } else { 8 });
    p.put32(ext + 4, r.next());
    let mut case = mk2(r, 0x25f320, vec![character], 0);
    case.script = vec![[index, r.next(), r.next()]];
    case.watch.push((ext + 4, 4));
    case
}
fn build_75f40(r: &mut Rng, p: &mut Pair) -> Case2 {
    let object = r.pool(POOL_B) & !3;
    p.put8(object + 0x40, if r.below(2) == 0 { (r.next() & 0xcf) as u8 } else { r.next() as u8 });
    let mut case = mk2(r, 0x75f40, vec![object], 0);
    case.script = vec![[[0u32, 1, r.next()][r.below(3) as usize], r.next(), r.next()]];
    case
}
fn build_64ef0(r: &mut Rng, p: &mut Pair) -> Case2 {
    let source = r.pool(POOL_B);
    let length = r.below(7);
    for i in 0..length {
        p.put8(source + i, 1 + (r.next() % 255) as u8);
    }
    p.put8(source + length, 0);
    let dest = r.pool(POOL_C);
    p.put32(dest, r.next());
    p.put32(dest + 4, r.next());
    let id = r.next();
    let mut case = mk2(r, 0x64ef0, vec![dest, id], 0);
    case.script = vec![[source, r.next(), r.next()]];
    case.watch.push((dest, 12));
    case
}
fn build_2be9b0(r: &mut Rng, p: &mut Pair) -> Case2 {
    let context = r.pool(POOL_B) & !3;
    let (mode, setup) = (r.below(6), r.below(4));
    p.put32(0x0078_ADAC, (r.next() & 0xFFFF_00FF) | (mode << 8) | (setup << 16));
    p.put32((setup << 5) + 0x004D_1F84, [0u32, 1, 2, 2, r.next()][r.below(5) as usize]);
    p.put32(context + mode * 20 + 0x24, if r.below(2) == 0 { 0 } else { r.next() });
    p.put32(0x0079_094C, [0x65u32, r.next(), 0x65][r.below(3) as usize]);
    let mut case = mk2(r, 0x2be9b0, vec![], 0);
    case.script = vec![[context, r.next(), r.next()], [[0u32, 1, r.next()][r.below(3) as usize], r.next(), r.next()]];
    case
}
fn build_190a80(r: &mut Rng, p: &mut Pair) -> Case2 {
    p.put32(0x007B_978C, r.next());
    let mut case = mk2(r, 0x190a80, vec![], 0);
    case.script = simple_script(r, 1);
    case.watch.push((0x007B_978C, 4));
    case
}
fn build_12d1e0(r: &mut Rng, p: &mut Pair) -> Case2 {
    for word in 0..14 {
        p.put32(0x0074_90AC + 4 * word, r.next());
    }
    let mut case = mk2(r, 0x12d1e0, vec![], 0);
    case.script = vec![
        [[0u32, 1, 2, 3, 4, u32::MAX][r.below(6) as usize], r.next(), r.next()],
        [[0u32, 1, 2, 3, 4, u32::MAX][r.below(6) as usize], r.next(), r.next()],
    ];
    case.watch.push((0x0074_90AC, 56));
    case
}
fn build_2e4cb0(r: &mut Rng, p: &mut Pair) -> Case2 {
    let player = r.pool(POOL_A) & !3;
    let index = r.below(6);
    p.put32(0x007B_0C7C, player);
    p.put32(player, r.next());
    p.put32(player + 4, index);
    let flags = r.pool(POOL_B);
    p.put8(flags, r.next() as u8);
    p.put32(index * 4 + 0x0076_1C48, flags);
    p.put32(0x007B_0CC0, r.next());
    p.put32(0x007B_A9E0, r.next());
    let mut case = mk2(r, 0x2e4cb0, vec![], 0);
    case.script = simple_script(r, 2);
    case.watch.push((0x007B_A9E0, 4));
    case
}
fn build_33acb0(r: &mut Rng, p: &mut Pair) -> Case2 {
    let count = [0u32, 1, 2, 3, u32::MAX][r.below(5) as usize];
    p.put32(0x0079_0950, count);
    let base = r.pool(POOL_A) & !3;
    p.put32(0x007B_0C48, base);
    let mut watch = vec![];
    for player in 0..3u32 {
        let record = r.pool(POOL_B) & !3;
        let object = r.pool(POOL_C) & !3;
        p.put32(player * 0x1584 + base + 0x14, record);
        p.put32(record + 0x7c, object);
        p.put32(0x0078_4034 + player * 4, r.next());
        let slot = r.below(8);
        p.put32(object + 8, slot);
        let class = if r.below(2) == 0 { r.next() & 0xff } else { r.next() };
        p.put32(object + 0x84, class);
        p.put8(slot + 0x007D_E424, if r.below(3) == 0 { class as u8 } else { r.next() as u8 });
        watch.push((slot + 0x007D_E424, 1));
    }
    let mut case = mk2(r, 0x33acb0, vec![], 0);
    case.script = simple_script(r, 3);
    case.watch.extend(watch);
    case
}

macro_rules! port_w14 {
    ($name:ident, $krate:ident, $path:ident) => {
        fn $name(s: &mut Skewed, v: &mut View, c: &mut Cpu) {
            $krate::call_leaves::$path(s, v, c)
        }
    };
}
port_w14!(port_12fe0, tsfp_ai, pair_table_entry_field_8);
port_w14!(port_2b2390, tsfp_ai, table_7a2960_has_no_flagged_entry);
port_w14!(port_25f320, tsfp_ai, char_set_ext_field_4_from_anim_entry);
port_w14!(port_75f40, tsfp_ui, flags_clear_and_chain_predicate_zero);
port_w14!(port_64ef0, tsfp_ui, copy_text_to_buffer);
port_w14!(port_2be9b0, tsfp_ui, setup_entry_available);
port_w14!(port_190a80, tsfp_ui, reset_memory_unit_globals);
port_w14!(port_12d1e0, tsfp_ui, hud_state_reset_by_player_count);
port_w14!(port_2e4cb0, tsfp_gameplay, set_current_player_and_reset_pad);
port_w14!(port_33acb0, tsfp_gameplay, players_apply_class_entry);

/// Like `closure_suite!` for roots whose callees take no stack argument: the wrong-argument
/// control has nothing to corrupt there, so it is left out (the callee contract is pinned instead).
macro_rules! closure_suite_noarg {
    ($name:ident, $seed:expr, $build:expr, $port:expr) => {
        mod $name {
            use super::*;
            #[test]
            fn c_equals_rust() {
                drive_closure($seed, Skew::None, $build, $port);
            }
            #[test]
            fn skip_first_call_detected() {
                let (d, e) = drive_closure($seed, Skew::SkipFirst, $build, $port);
                assert!(e > 0 && d == e, "{d}/{e}");
            }
            #[test]
            fn reordered_calls_detected() {
                let (d, e) = drive_closure($seed, Skew::Reorder, $build, $port);
                assert_eq!(d, e);
            }
        }
    };
}
closure_suite!(r_00012fe0, 0x1836_1401, build_12fe0, port_12fe0);
closure_suite!(r_002b2390, 0x1836_1402, build_2b2390, port_2b2390);
closure_suite!(r_0025f320, 0x1836_1403, build_25f320, port_25f320);
closure_suite!(r_00075f40, 0x1836_1404, build_75f40, port_75f40);
closure_suite!(r_00064ef0, 0x1836_1405, build_64ef0, port_64ef0);
closure_suite_noarg!(r_002be9b0, 0x1836_1406, build_2be9b0, port_2be9b0);
closure_suite!(r_00190a80, 0x1836_1407, build_190a80, port_190a80);
closure_suite_noarg!(r_0012d1e0, 0x1836_1408, build_12d1e0, port_12d1e0);
closure_suite!(r_002e4cb0, 0x1836_1409, build_2e4cb0, port_2e4cb0);
closure_suite!(r_0033acb0, 0x1836_140a, build_33acb0, port_33acb0);

#[test]
fn wave14_callee_argc_pins() {
    for (va, argc) in [
        (0x15BD00u32, 2), (0xD7550, 2), (0x15BE50, 0), (0x1CDAE0, 0), (0x15BE60, 1), (0x25FA10, 1),
        (0x747C0, 1), (0x3D7B0, 1), (0x42870, 1), (0x55D80, 1), (0x3533F0, 0), (0x827B0, 1),
    ] {
        assert_eq!(unsafe { shim_callee_argc(va) }, argc, "{va:x}");
    }
}

// ---- T1836 wave 15: last call-bearing rows (Tier 1, docs/t1836-rust-wave15.md) ----

fn build_probe(va: u32, tables: [u32; 3]) -> impl FnMut(&mut Rng, &mut Pair) -> Case2 {
    move |r, p| {
        let object = r.pool(POOL_A) & !3;
        let owner = r.pool(POOL_B) & !3;
        p.put32(object + 0x7c, owner);
        let rows = r.pool(POOL_C) & !3;
        p.put32(0x004F_9BAC, rows);
        let id = [u32::MAX, r.below(3), r.below(3)][r.below(3) as usize];
        p.put32(owner + 0x204, id);
        for row in 0..3u32 {
            p.put8(rows + row * 0x29c + 0xc, r.next() as u8);
        }
        for table in tables {
            p.put32(table, r.next());
            p.put32(table + 4, r.next());
        }
        let mut case = mk2(r, va, vec![object], 0);
        case.script = (0..3).map(|_| [[0u32, 0, 1][r.below(3) as usize], r.next(), r.next()]).collect();
        case
    }
}
fn build_338150(r: &mut Rng, p: &mut Pair) -> Case2 {
    p.put32(0x0076_5E4C, r.next());
    let slot = r.next();
    let mut case = mk2(r, 0x338150, vec![slot], 0);
    case.script = simple_script(r, 1);
    case.watch.push((0x0076_5E4C, 4));
    case
}
port_w14!(port_26cc80, tsfp_ai, object_any_probe_hits_a);
port_w14!(port_26cd50, tsfp_ai, object_any_probe_hits_b);
port_w14!(port_338150, tsfp_gameplay, clear_indexed_slot);
closure_suite!(r_0026cc80, 0x1836_1501, build_probe(0x26cc80, [0x5202F8, 0x520308, 0x5202E8]), port_26cc80);
closure_suite!(r_0026cd50, 0x1836_1502, build_probe(0x26cd50, [0x520338, 0x520328, 0x520318]), port_26cd50);
closure_suite!(r_00338150, 0x1836_1503, build_338150, port_338150);

#[test]
fn wave15_callee_argc_pins() {
    for (va, argc) in [(0x59D70u32, 2), (0x59D20, 2), (0x45590, 1), (0x274380, 1)] {
        assert_eq!(unsafe { shim_callee_argc(va) }, argc, "{va:x}");
    }
}

fn build_275640(r: &mut Rng, p: &mut Pair) -> Case2 {
    let table = r.pool(POOL_B) & !3;
    p.put32(0x0074_C3B8, table);
    p.put32(0x007D_E400, (r.next() & 0xFF00_FFFF) | ([0u32, 1, 1, 0xff][r.below(4) as usize] << 16));
    p.put32(0x007D_E454, (r.next() & 0xFFFF_00FF) | ([2u32, 2, 2, r.below(4)][r.below(4) as usize] << 8));
    p.put32(0x007D_E318, r.next());
    let side = r.below(4);
    for word in 0..3 {
        p.put32((side << 4).wrapping_add(table).wrapping_sub(8) + 4 * word, [0u32, 1, r.next()][r.below(3) as usize]);
    }
    let mut case = mk2(r, 0x275640, vec![side], 0);
    case.script = vec![[[0u32, 1, r.next()][r.below(3) as usize], r.next(), r.next()]];
    case.watch.push(((side << 4).wrapping_add(table).wrapping_sub(8), 16));
    case
}
port_w14!(port_275640, tsfp_gameplay, stamp_side_entry_on_flag_change);
closure_suite!(r_00275640, 0x1836_1504, build_275640, port_275640);
