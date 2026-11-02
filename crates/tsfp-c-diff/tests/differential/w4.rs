// SPDX-License-Identifier: GPL-3.0-or-later
//! T1895 wave 4 Tier 1: the 12 batch 1 ports against the PRIVATE LIFTED ORIGINAL (not a src/game
//! replacement, none exists). `build.rs` + `w4_lifted.rs` extract the lifted functions into OUT_DIR
//! at build time (gitignored source, nothing copied into tracked files) and `w4_run` runs them on
//! the C arena while the Rust port runs on an identical Rust arena. Every nested guest call lands in
//! the shared `recomp_lookup` stub of c/shim.c (argc contracts in `callee_argc_or_neg`) and in
//! `Stub` below; both record the call trace and return the same scripted EAX/ECX/EDX.
//! Compared per case: all eight registers (policy in `Regs`), the call trace, the pool and the
//! argument frame above ESP. Dead stack contents below ESP (saved-register spills) are not
//! compared. No mutants (owner instruction). Without the lifted tree the file compiles to nothing.
#![cfg(w4_lifted)]
use super::*;

use tsfp_engine_core::w4c_b as ec;
use tsfp_engine_fileio::story_progress::StoryProgress;
use tsfp_engine_memory::crt_atexit::{self, AtexitHost};
use tsfp_engine_memory::crt_memmove;
use tsfp_engine_object::closure::callee::{Callee, Cpu};
use tsfp_engine_object::w4_object as obj;
use tsfp_gameplay::w4_weapons as weap;
use tsfp_ui::w4_ui;

impl tsfp_engine_memory::GuestMemory for View {
    fn read(
        &mut self,
        address: tsfp_types::GuestAddr,
        out: &mut [u8],
    ) -> Result<(), tsfp_engine_memory::MemoryFault> {
        for (index, byte) in out.iter_mut().enumerate() {
            *byte = self.byte(address.0.wrapping_add(index as u32));
        }
        Ok(())
    }

    fn write(
        &mut self,
        address: tsfp_types::GuestAddr,
        bytes: &[u8],
    ) -> Result<(), tsfp_engine_memory::MemoryFault> {
        for (index, byte) in bytes.iter().copied().enumerate() {
            self.set(address.0.wrapping_add(index as u32), byte);
        }
        Ok(())
    }
}
unsafe extern "C" {
    fn w4_run(va: u32, regs: *mut u32) -> i32;
    fn shim_script(words: *const u32, entries: u32);
    fn shim_callee_argc(va: u32) -> i32;
    fn shim_trace_len() -> u32;
    fn shim_trace_entry(index: u32) -> *const u32;
}

const SEEDS: usize = 3000;
const POOL: u32 = 0x0090_0000;
const POOL_LEN: u32 = 0x8000;
const ARG_FRAME: u32 = 0x40;

/// Exit register comparison policy (bit i set = register i compared, order EAX ECX EDX EBX ESI EDI
/// EBP ESP). Caller-saved EAX/ECX/EDX residue of a void function is dead by the cdecl ABI, so it
/// is only compared where the function returns a value or tail-calls.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Regs(u8);
impl Regs {
    const ALL: Regs = Regs(0xFF);
    /// Void function: callee-saved registers and ESP.
    const VOID: Regs = Regs(0xF8);
    /// EAX is the return value.
    const RET: Regs = Regs(0xF9);
    const ESP_ONLY: Regs = Regs(0x80);
}

/// Registers at nested call entry are not compared except those a callee is known to take in a
/// register (EAX = 0xFF into 001C4A50). Stack arguments, ESP and return PC are always compared.
fn mask_entry(trace: &mut [[u32; 16]]) {
    for e in trace {
        let keep_eax = matches!(e[0], 0x1C4A50 | 0xC0C50 | 0xBF080);
        let eax = e[6];
        e[6..13].fill(0);
        if keep_eax {
            e[6] = eax;
        }
    }
}

struct Stub {
    script: Vec<[u32; 3]>,
    trace: Vec<[u32; 16]>,
}
fn rd(v: &View, a: u32) -> u32 {
    u32::from_le_bytes(std::array::from_fn(|i| v.byte(a.wrapping_add(i as u32))))
}
impl Callee<View> for Stub {
    fn call(&mut self, m: &mut View, target: u32, cpu: &mut Cpu) {
        if target == 0x1622C0 {
            // Composition: on the C side 1622C0 is a lifted root and runs for real, so the Rust side
            // runs its own port of it (no trace entry on either side).
            obj::array_entry_flag_when_slot_matches_record_ids(m, cpu);
            return;
        }
        let index = self.trace.len();
        let argc = unsafe { shim_callee_argc(target) };
        assert!(argc >= 0, "unknown callee {target:x}");
        let mut e = [0u32; 16];
        e[0] = target;
        e[1] = cpu.esp;
        e[2] = rd(m, cpu.esp);
        for a in 0..3u32 {
            e[3 + a as usize] = if (a as i32) < argc {
                rd(m, cpu.esp.wrapping_add(4 + 4 * a))
            } else {
                0
            };
        }
        e[6..13].copy_from_slice(&[
            cpu.eax, cpu.ecx, cpu.edx, cpu.ebx, cpu.esi, cpu.edi, cpu.ebp,
        ]);
        self.trace.push(e);
        let [a, c, d] = self.script.get(index).copied().unwrap_or([0; 3]);
        (cpu.eax, cpu.ecx, cpu.edx) = (a, c, d);
    }
}

struct W4Case {
    va: u32,
    args: Vec<u32>,
    regs: [u32; 7],
    script: Vec<[u32; 3]>,
}
fn case(r: &mut Rng, va: u32, args: Vec<u32>) -> W4Case {
    W4Case {
        va,
        args,
        regs: std::array::from_fn(|_| r.next()),
        script: vec![],
    }
}
fn zero(p: &mut Pair, start: u32, len: u32) {
    unsafe {
        std::ptr::write_bytes(p.c.add(start as usize), 0, len as usize);
        std::ptr::write_bytes(p.rust.add(start as usize), 0, len as usize);
    }
}
fn rows(r: &mut Rng, n: usize, special: &[u32]) -> Vec<[u32; 3]> {
    (0..n)
        .map(|_| [r.word(special), r.next(), r.next()])
        .collect()
}
/// Float bit patterns that exercise compares and float to int edges.
const FLOATS: [u32; 16] = [
    0,
    0x8000_0000,
    0x3F80_0000,
    0xBF80_0000,
    0x3F00_0000,
    0x42C8_0000,
    0x43A0_0000,
    0x7FC0_0000,
    0x7F80_0000,
    0xFF80_0000,
    0x4F00_0000,
    0xCF00_0000,
    0x4E99_6000,
    0x0000_0001,
    0x4B80_0000,
    0x3DCC_CCCD,
];
fn fl(r: &mut Rng) -> u32 {
    if r.below(2) == 0 {
        FLOATS[r.below(16) as usize]
    } else {
        (((r.next() % 4000) as i32 - 2000) as f32 / 8.0).to_bits()
    }
}

/// Runs the lifted original on the C arena. Returns final registers and the call trace.
fn run_c(p: &mut Pair, c: &W4Case) -> ([u32; 8], Vec<[u32; 16]>) {
    p.put32(STACK, 0);
    for (i, a) in c.args.iter().enumerate() {
        p.put32(STACK + 4 + 4 * i as u32, *a);
    }
    let flat: Vec<u32> = c.script.iter().flatten().copied().collect();
    unsafe { shim_script(flat.as_ptr(), c.script.len() as u32) };
    let mut regs = [
        c.regs[0], c.regs[1], c.regs[2], c.regs[3], c.regs[4], c.regs[5], c.regs[6], STACK,
    ];
    assert_eq!(
        unsafe { w4_run(c.va, regs.as_mut_ptr()) },
        1,
        "unknown root"
    );
    let trace = (0..unsafe { shim_trace_len() })
        .map(|i| unsafe { *(shim_trace_entry(i) as *const [u32; 16]) })
        .collect();
    (regs, trace)
}

fn mem_diff(p: &Pair, logged: &[u32], va: u32) -> Option<String> {
    // 25F520 keeps a loop counter in its own argument slot (cdecl: the slot is callee scratch).
    let frame = if va == 0x25F520 { 0 } else { ARG_FRAME };
    let ranges = [(POOL, POOL_LEN), (STACK, frame)];
    for (start, len) in ranges.iter().copied().chain(logged.iter().map(|a| (*a, 4))) {
        for off in 0..len {
            let a = start.wrapping_add(off);
            if p.get(false, a) != p.get(true, a) {
                return Some(format!(
                    "memory {a:08x}: C {:02x} Rust {:02x}",
                    p.get(false, a),
                    p.get(true, a)
                ));
            }
        }
    }
    None
}

#[path = "w4_capdiff.rs"]
pub(super) mod capdiff;

/// Differential over `SEEDS` cases for a port with the common `(stub, memory, cpu)` shape.
fn drive(
    seed: u64,
    mode: Regs,
    expect: &[u32],
    mut build: impl FnMut(&mut Rng, &mut Pair) -> W4Case,
    port: impl Fn(&mut Stub, &mut View, &mut Cpu),
) {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let mut r = Rng(seed);
    let (mut with_calls, mut changed) = (0usize, 0usize);
    let mut seen = std::collections::BTreeMap::<u32, usize>::new();
    for index in 0..SEEDS {
        zero(p, POOL, POOL_LEN);
        zero(p, STACK, ARG_FRAME);
        p.log.clear();
        let c = build(&mut r, p);
        let logged: Vec<u32> = p.log.drain(..).collect();
        let (cregs, ctrace) = run_c(p, &c);
        let mut stub = Stub {
            script: c.script.clone(),
            trace: vec![],
        };
        let mut cpu = Cpu {
            eax: c.regs[0],
            ecx: c.regs[1],
            edx: c.regs[2],
            ebx: c.regs[3],
            esi: c.regs[4],
            edi: c.regs[5],
            ebp: c.regs[6],
            esp: STACK,
        };
        let mut view = View(p.rust);
        port(&mut stub, &mut view, &mut cpu);
        let rregs = [
            cpu.eax,
            cpu.ecx,
            cpu.edx,
            cpu.ebx,
            cpu.esi,
            cpu.edi,
            cpu.ebp,
            cpu.esp.wrapping_add(4),
        ];
        let mut why = vec![];
        let differs = (0..8).any(|i| mode.0 >> i & 1 == 1 && cregs[i] != rregs[i]);
        if differs {
            why.push(format!(
                "regs (mask {:#x}) C {cregs:x?} Rust {rregs:x?}",
                mode.0
            ));
        }
        let mut ctrace = ctrace;
        mask_entry(&mut ctrace);
        mask_entry(&mut stub.trace);
        if ctrace != stub.trace {
            why.push(format!("trace C {ctrace:x?} Rust {:x?}", stub.trace));
        }
        why.extend(mem_diff(p, &logged, c.va));
        assert!(
            why.is_empty(),
            "root {:08X} seed {seed:#x} case {index} args {:?}: {why:?}",
            c.va,
            c.args
        );
        for e in &ctrace {
            *seen.entry(e[0]).or_default() += 1;
        }
        with_calls += usize::from(!ctrace.is_empty());
        changed += usize::from(cregs[0] != c.regs[0] || !logged.is_empty());
    }
    for callee in expect {
        assert!(
            seen.get(callee).copied().unwrap_or(0) >= 20,
            "callee {callee:x} reached too rarely: {seen:x?}"
        );
    }
    eprintln!(
        "w4 root {seed:#x}: {SEEDS} cases, {with_calls} with calls, {changed} non-trivial, callees {seen:x?}"
    );
}

macro_rules! w4_test {
    ($name:ident, $seed:expr, $mode:expr, $expect:expr, $build:expr, $port:expr) => {
        #[test]
        fn $name() {
            drive($seed, $mode, $expect, $build, $port);
        }
    };
}

// ---- 00210FF0 random-point pool owner reclaim callback ----
fn build_210ff0(r: &mut Rng, p: &mut Pair) -> W4Case {
    p.put32(0x75C0A8, r.next());
    p.put32(0x75C0CC, r.next());
    case(r, 0x210FF0, vec![])
}
w4_test!(
    lifted_00210ff0,
    0x210FF0,
    Regs::VOID,
    &[],
    build_210ff0,
    |_, memory, _| {
        tsfp_engine_memory::wrappers::clear_random_point_pool_owner(memory)
            .expect("point-pool owner globals are writable")
    }
);

// ---- 0035DE60 return constant 1 ----
w4_test!(
    lifted_0035de60,
    0x35DE60,
    Regs::ALL,
    &[],
    |r, _| case(r, 0x35DE60, vec![]),
    |_, _, c| c.eax = w4_ui::return_constant_1()
);

// ---- 00162200 array entry flag from object field 0x3c ----
fn build_162200(r: &mut Rng, p: &mut Pair) -> W4Case {
    let (t, rec, flags) = (POOL, POOL + 0x100, POOL + 0x800);
    p.put32(0x74C36C, t);
    p.put32(t + 0x74, rec);
    p.put32(0x7A2820, flags);
    let index = r.below(8);
    p.put32(flags + index * 24 + 0x10, r.next());
    p.put32(rec + index * 0x98 + 0x3C, r.next());
    let mut c = case(r, 0x162200, vec![index]);
    c.script = rows(r, 1, &[0, 1, 0xFFFF_FFFF]);
    c
}
w4_test!(
    lifted_00162200,
    0x162200,
    Regs::VOID,
    &[0x234270],
    build_162200,
    |s, v, c| obj::array_entry_flag_from_object_field_3c(s, v, c)
);

// ---- 001622C0 array entry flag when slot matches record ids ----
fn build_1622c0(r: &mut Rng, p: &mut Pair) -> W4Case {
    let (t, rec, flags, players, ids) = (
        POOL,
        POOL + 0x100,
        POOL + 0x800,
        POOL + 0x1000,
        POOL + 0x6000,
    );
    p.put32(0x74C36C, t);
    p.put32(t + 0x74, rec);
    p.put32(0x7A2820, flags);
    p.put32(0x7356D8, players);
    p.put32(0x715F2C, ids);
    let count = [0u32, 1, 2, 3, 4, 0xFFFF_FFFF, 0x8000_0000][r.below(7) as usize];
    p.put32(0x790950, count);
    let index = r.below(8);
    p.put32(flags + index * 24 + 0x10, r.below(3));
    let mut fields = [0u32; 4];
    for (i, f) in fields.iter_mut().enumerate() {
        *f = [0xFFFF_FFFFu32, 1, 2, 3, 4, r.next()][r.below(6) as usize];
        p.put32(rec + index * 0x98 + 0x38 + 4 * i as u32, *f);
    }
    for player in 0..4u32 {
        let id = r.below(16);
        p.put32(players + player * 0xEA0 + 0x678, id);
        let value = match r.below(4) {
            0 => 0xFFFF_FFFF,
            1 => fields[r.below(4) as usize],
            2 => r.below(6),
            _ => r.next(),
        };
        p.put32(ids + id * 32, value);
    }
    case(r, 0x1622C0, vec![index])
}
w4_test!(
    lifted_001622c0,
    0x1622C0,
    Regs::VOID,
    &[],
    build_1622c0,
    |_, v, c| obj::array_entry_flag_when_slot_matches_record_ids(v, c)
);

// ---- 00087260 room collect unmarked records into list ----
fn build_87260(r: &mut Rng, p: &mut Pair) -> W4Case {
    let (base, list, ids) = (POOL, POOL + 0x2000, POOL + 0x3000);
    p.put32(0x708788, base);
    let count = [0u32, 1, 2, 3, 6, 0xFFFF_FFFF][r.below(6) as usize];
    for i in 0..8u32 {
        let id = r.below(12);
        p.put8(ids + 2 * i, id as u8);
        p.put8(ids + 2 * i + 1, 0);
    }
    for id in 0..12u32 {
        let flag = r.next() as u8;
        p.put8(base + id * 0xB4 + 0xB2, flag);
    }
    let used = r.below(4);
    p.put8(list, used as u8);
    p.put8(list + 1, 0);
    let (a, b) = (r.next(), r.next());
    let mut c = case(r, 0x87260, vec![a, b, list, count, ids]);
    c.script = rows(r, 8, &[0, 1, 0xFFFF, 0x8000]);
    c
}
w4_test!(
    lifted_00087260,
    0x87260,
    Regs::RET,
    &[0x47560],
    build_87260,
    |s, v, c| obj::room_collect_unmarked_records_into_list(s, v, c)
);

// ---- 0005F430 prop anim advance track then set flags ----
fn build_5f430(r: &mut Rng, p: &mut Pair) -> W4Case {
    let prop = POOL + 0x100;
    p.put32(prop + 4, r.next());
    p.put32(prop + 0x18, r.next());
    p.put32(prop + 0x28, r.next());
    let mut c = case(r, 0x5F430, vec![prop]);
    c.script = rows(r, 1, &[0, 1]);
    c
}
w4_test!(
    lifted_0005f430,
    0x5F430,
    Regs::VOID,
    &[0x5F040],
    build_5f430,
    |s, v, c| obj::prop_anim_advance_track_then_set_flags(s, v, c)
);

// ---- 0033CC90 tail jump to 0033CB90 ----
w4_test!(
    lifted_0033cc90,
    0x33CC90,
    Regs::ALL,
    &[0x33CB90],
    |r, _| {
        let (a, b) = (r.next(), r.next());
        let mut c = case(r, 0x33CC90, vec![a, b]);
        c.script = rows(r, 1, &[0, 1]);
        c
    },
    |s, v, c| obj::tail_jump_to_33cb90(s, v, c)
);

// ---- 00167780 object copy matrices to entry, clear fields ----
fn build_167780(r: &mut Rng, p: &mut Pair) -> W4Case {
    let (object, link, inner, node_base, matrices) = (
        POOL + 0x100,
        POOL + 0x400,
        POOL + 0x500,
        POOL + 0x600,
        POOL + 0x1000,
    );
    p.put32(0x74C458, matrices.wrapping_sub(r.below(2) * 0x200));
    p.put32(object + 4, link);
    p.put32(object + 0x80, (r.next() & 0xFFFF_0003) | r.below(3));
    for f in [0xC0u32, 0x70, 0xC8, 0x28] {
        p.put32(object + f, r.next());
    }
    p.put32(link + 0x10, [0, 0x1234, r.next()][r.below(3) as usize]);
    p.put32(link + 4, inner);
    p.put32(inner + 4, node_base + 4 * r.below(8));
    let mut c = case(r, 0x167780, vec![object]);
    c.script = rows(r, 1, &[0, 1]);
    c
}
w4_test!(
    lifted_00167780,
    0x167780,
    Regs::VOID,
    &[0x153190],
    build_167780,
    |s, v, c| obj::object_copy_matrices_to_entry_clear_fields(s, v, c)
);

// ---- 000D9030 record decay list timers ----
w4_test!(
    lifted_000d9030,
    0xD9030,
    Regs::ALL,
    &[0xDB4C0],
    |r, p| {
        let player = POOL + 0x100;
        p.put32(
            player + 0x7C,
            [POOL + 0x2000, r.next()][r.below(2) as usize],
        );
        let mut c = case(r, 0xD9030, vec![player]);
        c.script = rows(r, 1, &[0, 1]);
        c
    },
    |s, v, c| weap::record_decay_list_timers(s, v, c)
);

// ---- 000D8E30 player frame update ----
fn build_d8e30(r: &mut Rng, p: &mut Pair) -> W4Case {
    let (player, record) = (POOL + 0x100, POOL + 0x1000);
    p.put32(player + 0x7C, record);
    p.put32(0x4E794C, fl(r));
    p.put32(
        0x4E8CEC,
        [0, 1, 2, 30, 60, 0xFFFF_FFFF, r.next()][r.below(7) as usize],
    );
    p.put32(0x475C78, fl(r));
    p.put32(0x7DE30C, if r.below(2) == 0 { 7 } else { r.below(10) });
    p.put32(0x7A2958, r.below(8));
    p.put32(0x790950, r.below(5));
    for f in [0xAD4u32, 0xB40] {
        p.put32(record + f, fl(r));
    }
    p.put32(record + 8, r.below(6));
    p.put32(record + 0x94, [0, 0x55, r.next()][r.below(3) as usize]);
    p.put32(record + 0x80, [0, 1, r.next()][r.below(3) as usize]);
    p.put32(
        record + 0xA68,
        [1, 0, 2, 3, 4, r.next()][r.below(6) as usize],
    );
    p.put32(record + 0x2C, r.next());
    let mut c = case(r, 0xD8E30, vec![player]);
    let pick = |r: &mut Rng, zero_pct: u32, specials: &[u32]| {
        if r.below(100) < zero_pct {
            0
        } else {
            r.word(specials)
        }
    };
    c.script = vec![
        [
            if r.below(6) == 0 { 9 } else { r.below(9) },
            r.next(),
            r.next(),
        ],
        [pick(r, 75, &[1]), r.next(), r.next()],
        [
            if r.below(5) == 0 { 0 } else { r.word(&[1, 2]) },
            r.next(),
            r.next(),
        ],
        [r.next(), r.next(), r.next()],
        [r.next(), r.next(), r.next()],
        [pick(r, 50, &[1]), r.next(), r.next()],
        [r.next(), r.next(), r.next()],
        [r.next(), r.next(), r.next()],
        [r.below(5), r.next(), r.next()],
        [r.next(), r.next(), r.next()],
        [pick(r, 50, &[1]), r.next(), r.next()],
        [pick(r, 50, &[1]), r.next(), r.next()],
        [r.next(), r.next(), r.next()],
        [r.next(), r.next(), r.next()],
    ];
    c
}
w4_test!(
    lifted_000d8e30,
    0xD8E30,
    Regs::VOID,
    &[
        0xC63D0, 0x1CEBA0, 0x3568C0, 0xD8C80, 0xDC110, 0x1B6A80, 0x2385B0, 0x238510, 0x356940,
        0x1BDE40, 0x3533F0, 0x358560, 0x15EAE0
    ],
    build_d8e30,
    |s, v, c| weap::player_frame_update(s, v, c)
);

// ---- 001C6AA0 fpistol record 14 event callback ----
fn build_1c6aa0(r: &mut Rng, p: &mut Pair) -> W4Case {
    // The jump and index tables live in the XBE image, not in the arena. Seed a fixed dword table
    // of the seven targets and random in-range index bytes (table contents are INFERRED, the
    // dispatch logic is what is compared).
    const TARGETS: [u32; 7] = [
        0x1C6AB7, 0x1C6AC4, 0x1C6AC7, 0x1C6AE9, 0x1C6AF4, 0x1C6B30, 0x1C6B40,
    ];
    for (i, t) in TARGETS.iter().enumerate() {
        p.put32(0x1C6BC8 + 4 * i as u32, *t);
    }
    for code in 0..0x12u32 {
        p.put8(0x1C6BE4 + code, r.below(7) as u8);
    }
    p.put32(0x47CFF4, fl(r));
    p.put32(0x4782C4, fl(r));
    let (owner, sound_owner, out, target_a, target_b) = (
        POOL + 0x100,
        POOL + 0x300,
        POOL + 0x500,
        POOL + 0x600,
        POOL + 0x700,
    );
    p.put32(owner + 0x54, [0, r.next()][r.below(2) as usize]);
    p.put32(owner + 0xE8, [9, 0x15, r.below(30)][r.below(3) as usize]);
    p.put32(owner + 0x40, [0, 1, r.next()][r.below(3) as usize]);
    let link = if r.below(2) == 0 {
        sound_owner.wrapping_sub(0xA4)
    } else {
        r.next()
    };
    p.put32(sound_owner + 0xEC, link);
    p.put32(target_a + 0x34, r.next() % 2000);
    p.put32(target_a + 0x2C, r.next());
    p.put32(target_b + 0x34, r.next());
    p.put32(target_b + 0x2C, r.next() % 5000);
    let code = [
        r.below(0x12),
        r.below(0x12),
        r.below(0x12),
        0x12 + r.below(4),
        r.next(),
    ][r.below(5) as usize];
    let mut c = case(r, 0x1C6AA0, vec![code, owner, sound_owner, out]);
    c.script = vec![
        [target_a, r.next(), r.next()],
        [target_b, r.next(), r.next()],
        [r.next(), r.next(), r.next()],
    ];
    c
}
w4_test!(
    lifted_001c6aa0,
    0x1C6AA0,
    Regs::RET,
    &[0x45590, 0x1C4A50, 0x1C4C60, 0xF8AE0, 0x1B8710],
    build_1c6aa0,
    |s, v, c| weap::fpistol_event_callback(s, v, c)
);

// ---- 00040260 and 002D8060: ui draw ports (observable draw call) ----
impl w4_ui::W4Memory for View {
    fn read32(&self, address: u32) -> u32 {
        rd(self, address)
    }
}

/// Drive a ui draw port. The lifted function pushes its draw arguments on the C stack, so the
/// full argument list of the one draw call is read back from the C arena at the callee ESP.
fn drive_ui(
    seed: u64,
    mut build: impl FnMut(&mut Rng, &mut Pair) -> W4Case,
    port: impl Fn(&View, u32, &mut Vec<w4_ui::DrawCall>),
    argc: usize,
) {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let mut r = Rng(seed);
    let (mut draws, mut skipped) = (0usize, 0usize);
    for index in 0..SEEDS {
        zero(p, POOL, POOL_LEN);
        zero(p, STACK, ARG_FRAME);
        p.log.clear();
        let c = build(&mut r, p);
        let (cregs, ctrace) = run_c(p, &c);
        let mut expected = vec![];
        port(&View(p.rust), c.args[0], &mut expected);
        let seen: Vec<w4_ui::DrawCall> = ctrace
            .iter()
            .map(|e| w4_ui::DrawCall {
                callee: e[0],
                args: (0..argc as u32)
                    .map(|i| rd(&View(p.c), e[1].wrapping_add(4 + 4 * i)))
                    .collect(),
            })
            .collect();
        assert_eq!(cregs[7], STACK + 4, "esp, case {index}");
        assert_eq!(
            seen, expected,
            "root {:08X} seed {seed:#x} case {index}",
            c.va
        );
        // Callee-saved registers survive the lifted function (the port does not model them).
        assert_eq!(
            &cregs[3..7],
            &c.regs[3..7],
            "callee-saved regs, case {index}"
        );
        draws += seen.len();
        skipped += usize::from(seen.is_empty());
    }
    assert!(
        draws > SEEDS / 10 && skipped > SEEDS / 20,
        "weak coverage: {draws} draws, {skipped} skips"
    );
    eprintln!("w4 ui {seed:#x}: {SEEDS} cases, {draws} draws, {skipped} skips");
}

fn build_40260(r: &mut Rng, p: &mut Pair) -> W4Case {
    let rect = POOL + 0x100;
    p.put32(
        0x4E8CF0,
        [0, 1, 320, 640, 0xFFFF_FFFF, r.next() % 2000][r.below(6) as usize],
    );
    p.put32(
        0x4E8CF4,
        [0, 1, 240, 480, 0xFFFF_FFFF, r.next() % 2000][r.below(6) as usize],
    );
    for off in (0..0x38u32).step_by(4) {
        let v = if (0x18..0x28).contains(&off) {
            fl(r)
        } else {
            r.next()
        };
        p.put32(rect + off, v);
    }
    // Bias towards the draw path: small positive extents and an inside origin.
    if r.below(2) == 0 {
        p.put32(rect + 0x18, (r.below(300) as f32).to_bits());
        p.put32(rect + 0x1C, (r.below(200) as f32).to_bits());
        p.put32(rect + 0x20, (r.below(50) as f32 + 0.5).to_bits());
        p.put32(rect + 0x24, (r.below(50) as f32 + 0.5).to_bits());
        p.put32(0x4E8CF0, 320);
        p.put32(0x4E8CF4, 240);
    }
    case(r, 0x40260, vec![rect])
}
#[test]
fn lifted_00040260() {
    drive_ui(
        0x40260,
        build_40260,
        |v, rect, out| w4_ui::hud_draw_textured_rect(v, rect, out),
        9,
    );
}

fn build_2d8060(r: &mut Rng, p: &mut Pair) -> W4Case {
    let widget = POOL + 0x100;
    for off in (0..0x100u32).step_by(4) {
        p.put32(widget + off, fl(r));
    }
    p.put32(widget + 0xF4, r.next());
    for g in [0x475D10u32, 0x49F648, 0x475CD4, 0x475C78, 0x475D08] {
        p.put32(g, fl(r));
    }
    if r.below(2) == 0 {
        // Finite window so cvttss2si stays in range most of the time.
        for off in [0xE4u32, 0xE8, 0xEC, 0xF0, 0xB4] {
            p.put32(widget + off, ((r.next() % 1000) as f32).to_bits());
        }
    }
    let extra = r.next();
    case(r, 0x2D8060, vec![widget, extra])
}
#[test]
fn lifted_002d8060() {
    // The loadbar always draws, so skips are not expected: relax the coverage check below.
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let mut r = Rng(0x2D8060);
    let mut high_level = 0usize;
    for index in 0..SEEDS {
        zero(p, POOL, POOL_LEN);
        zero(p, STACK, ARG_FRAME);
        let c = build_2d8060(&mut r, p);
        let (cregs, ctrace) = run_c(p, &c);
        let mut expected = vec![];
        w4_ui::widget_draw_loadbar(&View(p.rust), c.args[0], &mut expected);
        let seen: Vec<w4_ui::DrawCall> = ctrace
            .iter()
            .map(|e| w4_ui::DrawCall {
                callee: e[0],
                args: (0..6u32)
                    .map(|i| rd(&View(p.c), e[1].wrapping_add(4 + 4 * i)))
                    .collect(),
            })
            .collect();
        assert_eq!(cregs[7], STACK + 4);
        assert_eq!(seen, expected, "case {index}");
        assert_eq!(
            &cregs[3..7],
            &c.regs[3..7],
            "callee-saved regs, case {index}"
        );
        high_level += usize::from(seen[0].args[4] != 0 && seen[0].args[4] != 0x8000_0000);
    }
    assert!(high_level > SEEDS / 10, "weak coverage {high_level}");
    eprintln!("w4 ui 2d8060: {SEEDS} cases");
}

// ======== T1920: engine/core second half (crates/tsfp-engine-core/src/w4c_b.rs) ========
/// Absolute-address globals of a root, seeded with random words (the put32 calls are logged, so
/// the comparison covers them).
fn seed(r: &mut Rng, p: &mut Pair, addresses: &[u32]) {
    for a in addresses {
        let v = r.next();
        p.put32(*a, v);
    }
}

fn build_3d2e0(r: &mut Rng, p: &mut Pair) -> W4Case {
    seed(r, p, &[0x6B7A90]);
    let mut c = case(r, 0x3D2E0, vec![POOL + 0x100]);
    c.script = rows(r, 1, &[0, 1, 0xFFFF_FFFF]);
    c
}
w4_test!(
    lifted_0003d2e0,
    0x3D2E0,
    Regs::RET,
    &[0x3C8FD0],
    build_3d2e0,
    |s, v, c| ec::launch_arg_set_6b7a90_from_atol(s, v, c)
);

macro_rules! flag_test {
    ($name:ident, $va:expr, $global:expr, $port:path, $mode:expr) => {
        w4_test!(
            $name,
            $va,
            $mode,
            &[],
            |r, p| {
                seed(r, p, &[$global]);
                case(r, $va, vec![])
            },
            |_, v, c| $port(v, c)
        );
    };
}
flag_test!(
    lifted_0003d300,
    0x3D300,
    0x6B7A94,
    ec::launch_arg_set_flag_6b7a94,
    Regs::RET
);
flag_test!(
    lifted_0003d310,
    0x3D310,
    0x6B7A98,
    ec::launch_arg_set_flag_6b7a98,
    Regs::RET
);
flag_test!(
    lifted_0003d320,
    0x3D320,
    0x4C0468,
    ec::launch_arg_clear_4c0468,
    Regs::RET
);
flag_test!(
    lifted_0003d330,
    0x3D330,
    0x6B7A9C,
    ec::launch_arg_set_flag_6b7a9c,
    Regs::RET
);
flag_test!(
    lifted_0003d340,
    0x3D340,
    0x6B7AA0,
    ec::launch_arg_set_flag_6b7aa0,
    Regs::RET
);
flag_test!(
    lifted_0003d350,
    0x3D350,
    0x6B7AA4,
    ec::launch_arg_set_flag_6b7aa4,
    Regs::RET
);

fn build_3d360(r: &mut Rng, p: &mut Pair) -> W4Case {
    for i in 0..300u32 {
        let v = r.next();
        p.put32(0x6B7A28 + 4 * i, v);
    }
    let index = [r.below(300), r.below(5), 0xFFFF_FFFF, 0x4000_0000][r.below(4) as usize];
    case(r, 0x3D360, vec![index])
}
w4_test!(
    lifted_0003d360,
    0x3D360,
    Regs::RET,
    &[],
    build_3d360,
    |_, v, c| ec::launch_array_6b7a28_get(v, c)
);

w4_test!(
    lifted_0003db70,
    0x3DB70,
    Regs::VOID,
    &[],
    |r, p| {
        seed(r, p, &[0x6B7B10]);
        case(r, 0x3DB70, vec![])
    },
    |_, v, _| ec::global_6b7b10_clear(v)
);

w4_test!(
    lifted_0003ed60,
    0x3ED60,
    Regs::RET,
    &[],
    |r, p| {
        seed(
            r,
            p,
            &[
                0x6B83D8, 0x6B83DC, 0x6B83E0, 0x6B83E4, 0x6B83E8, 0x6B83EC, 0x6B83F0, 0x6B83F4,
            ],
        );
        case(r, 0x3ED60, vec![])
    },
    |_, v, c| ec::globals_6b83d8_block_clear(v, c)
);

/// 0003ED90 without callbacks: every entry's +8 word is zero, so the indirect callback is not
/// reached (the harness cannot service indirect calls). The callback path is verification debt.
fn build_3ed90(r: &mut Rng, p: &mut Pair) -> W4Case {
    let group = r.below(2);
    for g in 0..2u32 {
        let count = [0u32, 1, 2, 3, 5, 0xFFFF_FFFF][r.below(6) as usize];
        p.put32(0x6B83D8 + 16 * g, count);
        let entries = POOL + 0x400 + g * 0x400;
        p.put32(0x6B83E4 + 16 * g, entries);
        for e in 0..6u32 {
            for field in [0u32, 4, 0xC, 0x10, 0x14, 0x18, 0x24, 0x28] {
                let v = r.next();
                p.put32(entries + e * 0x30 + field, v);
            }
        }
    }
    case(r, 0x3ED90, vec![group])
}
w4_test!(
    lifted_0003ed90,
    0x3ED90,
    Regs::RET,
    &[],
    build_3ed90,
    |s, v, c| ec::stream_block_pool_group_reset(s, v, c)
);

w4_test!(
    lifted_000454f0,
    0x454F0,
    Regs::RET,
    &[0x27700],
    |r, _| {
        let (a, b) = (r.next(), r.next());
        let mut c = case(r, 0x454F0, vec![a, b]);
        c.script = rows(r, 1, &[0, 1]);
        c
    },
    |s, v, c| ec::wrapper_sound_data_load(s, v, c)
);
w4_test!(
    lifted_00196450,
    0x196450,
    Regs::RET,
    &[0x37300],
    |r, _| {
        let (a, b) = (r.next(), r.next());
        let mut c = case(r, 0x196450, vec![a, b]);
        c.script = rows(r, 1, &[0, 1]);
        c
    },
    |s, v, c| ec::thunk_launch_new_image_31b10_wrapper(s, v, c)
);

fn build_56ec0(r: &mut Rng, p: &mut Pair) -> W4Case {
    seed(
        r,
        p,
        &[
            0x6B7A94, 0x6B7A98, 0x6B7A9C, 0x6B7AA0, 0x6B7AA4, 0x4C0468, 0x7B0CC0, 0x6D6128,
            0x6D612C, 0x6D6108, 0x6D610C, 0x6D6110, 0x4D1940, 0x6D615C, 0x6D6140,
        ],
    );
    let mut c = case(r, 0x56EC0, vec![]);
    c.script = rows(r, 2, &[0, 1, 0xFFFF_FFFF]);
    c
}
w4_test!(
    lifted_00056ec0,
    0x56EC0,
    Regs::VOID,
    &[0xCB3B0, 0x3C848C],
    build_56ec0,
    |s, v, c| ec::launch_flags_copy_and_init(s, v, c)
);

fn build_6a500(r: &mut Rng, p: &mut Pair) -> W4Case {
    let n = r.below(6);
    let key = [1u32, 2, 3, r.next()][r.below(4) as usize];
    for i in 0..n {
        let rec = 0x515FF8 + i * 0x80;
        let tag = 1 + r.below(0xFFFF);
        p.put32(rec + 4, tag | (r.next() & 0xFFFF_0000)); // low word nonzero
        let value = if r.below(3) == 0 {
            key
        } else {
            [1u32, 2, 3, r.next()][r.below(4) as usize]
        };
        p.put32(rec + 8, value);
    }
    // the record after the last one has a zero word at +4 (end of table)
    p.put32(0x515FF8 + n * 0x80 + 4, r.next() & 0xFFFF_0000);
    case(r, 0x6A500, vec![key])
}
w4_test!(
    lifted_0006a500,
    0x6A500,
    Regs::RET,
    &[],
    build_6a500,
    |_, v, c| ec::table_515ff8_find_entry(v, c)
);

// 00121A90 has a C replacement in src/game (game_round4_low_d.c): it is not compiled from the lifted
// tree here (duplicate symbol); compare the port with that replacement instead (verification debt).

// 18E8D0 never returns (3C2D0 is noreturn): the harness unwinds with longjmp, so ESP is not compared.
w4_test!(
    lifted_0018e8d0,
    0x18E8D0,
    Regs(0x7F),
    &[0x22040, 0x3C330, 0x3C2D0],
    |r, p| {
        seed(r, p, &[0x74CF68, 0x74CF6C]);
        let (a, b) = (r.next(), r.next());
        let mut c = case(r, 0x18E8D0, vec![a, b]);
        c.script = rows(r, 3, &[0, 1]);
        c
    },
    |s, v, c| ec::launch_info_query_store_pair(s, v, c)
);

fn build_257b50(r: &mut Rng, p: &mut Pair) -> W4Case {
    let mode = [0x1Eu32, 0x21, 0x22, 0x1D, 0x1F, r.next()][r.below(6) as usize];
    p.put32(0x79094C, mode);
    seed(r, p, &[0x75F5A8, 0x75F5AC, 0x75F5B0]);
    let mut c = case(r, 0x257B50, vec![]);
    c.script = rows(r, 2, &[0, 1]);
    c
}
w4_test!(
    lifted_00257b50,
    0x257B50,
    Regs::VOID,
    &[0x128440, 0x87600, 0x1F2560],
    build_257b50,
    |s, v, c| ec::init_globals_75f5a8_then_dispatch_by_mode(s, v, c)
);

fn build_288b80(r: &mut Rng, p: &mut Pair) -> W4Case {
    let base = POOL + 0x100;
    p.put32(0x78B1EC, base);
    seed(r, p, &[0x475D5C]);
    for off in (0..0x3E90u32).step_by(0x20) {
        let v = r.next();
        p.put32(base + off + 0x18, v);
    }
    case(r, 0x288B80, vec![])
}
w4_test!(
    lifted_00288b80,
    0x288B80,
    Regs::VOID,
    &[],
    build_288b80,
    |_, v, _| ec::init_buffer_78b1ec_floats_and_terminator(v)
);

fn build_2bd970(r: &mut Rng, p: &mut Pair) -> W4Case {
    let params = POOL + 0x200;
    seed(r, p, &[0x47DE90, 0x475D20, 0x49F52C, 0x49F528]);
    p.put32(0x4C0454, [4u32, 4, 5, 6, r.next()][r.below(5) as usize]);
    for off in (0..0x24u32).step_by(4) {
        let v = r.next();
        p.put32(params + off, v);
    }
    let mut c = case(r, 0x2BD970, vec![params]);
    c.script = rows(r, 2, &[0, 1, 0xFFFF_FFFF]);
    c
}
w4_test!(
    lifted_002bd970,
    0x2BD970,
    Regs::VOID,
    &[0x80F20],
    build_2bd970,
    |s, v, c| ec::init_effect_params_with_two_texture_entries(s, v, c)
);

// ======== T1920: the big five (w4b_object.rs, w4b_gameplay.rs) ========
use tsfp_engine_object::w4b_object as obj_b;
use tsfp_gameplay::w4b_gameplay as game_b;

fn objects(r: &mut Rng) -> [u32; 4] {
    [
        0,
        POOL + 0x2000,
        POOL + 0x2100,
        POOL + 0x2000 + 0x100 * r.below(3),
    ]
}

fn build_c1850(r: &mut Rng, p: &mut Pair) -> W4Case {
    let (pickup, block, record) = (POOL + 0x100, POOL + 0x200, POOL + 0x300);
    p.put32(pickup + 0x7C, block);
    p.put32(pickup + 0x28, r.below(8));
    p.put32(
        block + 0xC0,
        [0u32, 1, 2, 2, 3, 0xFFFF_FFFF][r.below(6) as usize],
    );
    p.put32(block + 0x18, if r.below(4) == 0 { 0 } else { record });
    p.put8(record, r.next() as u8);
    p.put8(block + 0x10, r.next() as u8);
    p.put32(block + 0xD0, fl(r));
    for i in 0..16u32 {
        let v = r.next() & 0x0007_0007; // small i16 counters, including 0
        p.put32(block + 0x50 + 4 * i, v);
    }
    let (zero, unit, scale) = (fl(r), fl(r), fl(r));
    p.put32(0x475CAC, zero);
    p.put32(0x475C78, unit);
    p.put32(0x4E794C, scale);
    p.put32(
        0x4E8CEC,
        [0u32, 1, 2, 3, 60, 0xFFFF_FFFF][r.below(6) as usize],
    );
    let mut c = case(r, 0xC1850, vec![pickup]);
    c.script = rows(r, 5, &[0, 1]);
    c
}
w4_test!(
    lifted_000c1850,
    0xC1850,
    Regs::VOID,
    &[0xC0C50, 0xBDD50, 0xBF080, 0xC15B0],
    build_c1850,
    |s, v, c| game_b::pickup_frame_tick(s, v, c)
);

fn build_1c5db0(r: &mut Rng, p: &mut Pair) -> W4Case {
    let (owner, hands) = (POOL + 0x100, POOL + 0x200);
    p.put32(owner + 0x54, if r.below(4) == 0 { 0 } else { hands });
    p.put32(
        owner + 0xE8,
        [0x14u32, 0x15, 0x16, 0x17, r.next()][r.below(5) as usize],
    );
    p.put32(hands + 0x28, r.next());
    let event = [3u32, 4, 0x11, 0x11, 0, 5, 0x10, r.next()][r.below(8) as usize];
    let context = r.next();
    let mut c = case(r, 0x1C5DB0, vec![event, owner, context]);
    c.script = rows(r, 1, &[0, 1]);
    c
}
w4_test!(
    lifted_001c5db0,
    0x1C5DB0,
    Regs::RET,
    &[0x1B8690],
    build_1c5db0,
    |s, v, c| game_b::weapon_record_37_default_hands_callback(s, v, c)
);

fn build_25f520(r: &mut Rng, p: &mut Pair) -> W4Case {
    let (table, recs, flags) = (POOL + 0x100, POOL + 0x400, POOL + 0x2000);
    let [_, obj_a, obj_b2, _] = objects(r);
    p.put32(0x74C36C, table);
    p.put32(table + 0x74, recs);
    let count = [0u32, 1, 2, 3, 5, 0xFFFF_FFFF][r.below(6) as usize];
    p.put32(table + 0x78, count);
    p.put32(0x7A2820, flags);
    let keys = [1u32, 2, 3, r.next() % 5];
    for i in 0..6u32 {
        let rec = recs + i * 0x98;
        p.put32(rec + 8, keys[r.below(4) as usize]);
        p.put32(rec + 0xC, r.next());
        p.put32(rec + 0x2C, r.next());
        p.put32(rec + 0x34, keys[r.below(4) as usize]);
        p.put32(flags + i * 24 + 0x10, [0u32, 1, 2][r.below(3) as usize]);
        p.put32(flags + i * 24 + 8, [0u32, 1, 2][r.below(3) as usize]);
    }
    for (object, player) in [(obj_a, POOL + 0x3100), (obj_b2, POOL + 0x3300)] {
        p.put32(object + 0x7C, player);
        p.put32(player + 0xBF0, POOL + 0x3500);
    }
    p.put32(POOL + 0x3500 + 8, keys[r.below(4) as usize]);
    let index = r.below(6);
    let mut c = case(r, 0x25F520, vec![index]);
    c.script = (0..8)
        .map(|_| {
            [
                [0u32, 1, obj_a, obj_b2][r.below(4) as usize],
                r.next(),
                r.next(),
            ]
        })
        .collect();
    c
}
w4_test!(
    lifted_0025f520,
    0x25F520,
    Regs::VOID,
    &[],
    build_25f520,
    |s, v, c| obj_b::slot_node_flag_entry_update(s, v, c)
);

/// Code addresses of the 18-entry jump table at 0x2355B8 (kind - 1 indexes it). Addresses only.
const JT_235240: [u32; 18] = [
    0x2353ba, 0x235372, 0x235372, 0x2352ce, 0x235372, 0x235437, 0x235372, 0x2354df, 0x235548,
    0x2354c3, 0x235575, 0x235372, 0x235372, 0x235372, 0x23545f, 0x2352fb, 0x235510, 0x2352a7,
];
fn build_235240(r: &mut Rng, p: &mut Pair) -> W4Case {
    for (i, t) in JT_235240.iter().enumerate() {
        p.put32(0x2355B8 + 4 * i as u32, *t);
    }
    let (table, event) = (POOL + 0x100, POOL + 0x80);
    p.put32(0x78CD90, table);
    p.put32(0x78CED0, r.next());
    let entry_index = r.below(4);
    let wanted = r.below(4);
    p.put8(event, wanted as u8);
    p.put8(event + 1, 0);
    p.put8(event + 0x2A, entry_index as u8);
    p.put8(event + 0x2B, 0);
    let entry = table + entry_index * 72;
    let head = POOL + 0x1000 + entry_index * 0x100;
    p.put32(entry, head);
    p.put32(
        entry + 0xC,
        if r.below(2) == 0 { wanted } else { wanted ^ 1 },
    );
    for slot in [entry + 4, entry + 0x10] {
        p.put32(slot, r.next());
        p.put32(slot + 4, [0u32, 1, 0x100][r.below(3) as usize]);
    }
    let [_, o1, o2, _] = objects(r);
    for node in [head + 0x40, head + 0x64] {
        p.put32(node, r.below(22));
        p.put32(node + 4, [1u32, 2, 3, r.next()][r.below(4) as usize]);
        p.put32(
            node + 8,
            [1u32, 2, 3, 0xFFFF_FFFF, r.below(8)][r.below(5) as usize],
        );
        p.put32(
            node + 0xC,
            [1u32, 2, 0xFFFF_FFFF, 0xFFFF_FFFE, r.next()][r.below(5) as usize],
        );
        p.put32(node + 0x10, [1u32, 2, 3][r.below(3) as usize]);
        p.put32(node + 0x14, [1u32, 2, 3][r.below(3) as usize]);
    }
    p.put32(0x790950, r.below(4));
    p.put32(0x7B0C48, POOL + 0x4000);
    for i in 0..4u32 {
        let player = POOL + 0x4800 + i * 0x100;
        p.put32(POOL + 0x4000 + i * 0x1584 + 0x14, player);
        p.put32(player + 0x1CC, fl(r));
        p.put32(player + 0x7C, POOL + 0x5000);
    }
    p.put32(0x475CAC, 0);
    p.put32(POOL + 0x5000 + 0x678, r.below(4));
    p.put32(POOL + 0x5000 + 8, r.below(3));
    for o in [o1, o2] {
        p.put32(
            o + 0x20,
            [8u32, 0x40_0000, 8, r.next()][r.below(4) as usize],
        );
        p.put32(o + 0x7C, POOL + 0x5000);
    }
    for i in 0..20u32 {
        let v = r.below(6);
        p.put32(0x7A2D24 + 96 * i, v);
        p.put32(0x78B2F4 + 32 * i, [0u32, 1, 2][r.below(3) as usize]);
    }
    let mode = [3u32, 3, 3, 4, 4, 0, 5][r.below(7) as usize];
    let mut c = case(r, 0x235240, vec![event, mode]);
    c.script = (0..8)
        .map(|_| {
            [
                [0u32, 1, o1, o2, 3][r.below(5) as usize],
                r.next(),
                r.next(),
            ]
        })
        .collect();
    c
}
w4_test!(
    lifted_00235240,
    0x235240,
    Regs::VOID,
    &[0x2274C0],
    build_235240,
    |s, v, c| obj_b::trigger_condition_check(s, v, c)
);

// ======== T1920: engine/memory arena core (crates/tsfp-engine-memory/src/w4m_arena.rs) ========
use tsfp_engine_memory::w4m_arena::{Arena, ArenaHooks};
use tsfp_engine_memory::{GuestMemory as ArenaMemory, MemoryFault};
use tsfp_types::GuestAddr;

struct AMem<'a>(&'a mut View);
impl ArenaMemory for AMem<'_> {
    fn read(&mut self, a: GuestAddr, out: &mut [u8]) -> Result<(), MemoryFault> {
        for (i, b) in out.iter_mut().enumerate() {
            *b = self.0.byte(a.0.wrapping_add(i as u32));
        }
        Ok(())
    }
    fn write(&mut self, a: GuestAddr, bytes: &[u8]) -> Result<(), MemoryFault> {
        for (i, b) in bytes.iter().enumerate() {
            self.0.set(a.0.wrapping_add(i as u32), *b);
        }
        Ok(())
    }
}
#[derive(Default)]
struct Oom(Vec<u32>);
impl ArenaHooks for Oom {
    fn out_of_memory(&mut self, argument: u32) {
        self.0.push(argument);
    }
}

/// Differential for an arena root: EAX where `ret`, the pool memory and the out-of-memory reports
/// (callee 000389B0 argument of every stub call).
fn arena_drive(
    seed: u64,
    ret: bool,
    mut build: impl FnMut(&mut Rng, &mut Pair) -> W4Case,
    port: impl Fn(&mut Arena<AMem, Oom>) -> u32,
) {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let mut r = Rng(seed);
    let (mut reports, mut moved) = (0usize, 0usize);
    for index in 0..SEEDS {
        zero(p, POOL, POOL_LEN);
        zero(p, STACK, ARG_FRAME);
        p.log.clear();
        let c = build(&mut r, p);
        let logged: Vec<u32> = p.log.drain(..).collect();
        let (cregs, ctrace) = run_c(p, &c);
        let c_reports: Vec<u32> = ctrace
            .iter()
            .filter(|e| e[0] == 0x389B0)
            .map(|e| e[3])
            .collect();
        let mut view = View(p.rust);
        let mut oom = Oom::default();
        let eax = port(&mut Arena {
            memory: &mut AMem(&mut view),
            hooks: &mut oom,
        });
        let mut why = vec![];
        if ret && cregs[0] != eax {
            why.push(format!("eax C {:x} Rust {eax:x}", cregs[0]));
        }
        if !why.is_empty() && c.va == 0x211040 {
            why.push(format!(
                "C callees {:x?} C regs {:x?}",
                ctrace.iter().map(|e| e[0]).collect::<Vec<_>>(),
                cregs
            ));
        }
        if c_reports != oom.0 {
            why.push(format!("oom reports C {c_reports:x?} Rust {:x?}", oom.0));
        }
        why.extend(mem_diff(p, &logged, c.va));
        assert!(
            why.is_empty(),
            "root {:08X} seed {seed:#x} case {index}: {why:?}",
            c.va
        );
        reports += usize::from(!c_reports.is_empty());
        moved += usize::from(!logged.is_empty());
    }
    eprintln!("w4 arena {seed:#x}: {SEEDS} cases, {reports} with oom reports");
}

fn arena_state(r: &mut Rng, p: &mut Pair) {
    let top = POOL + 0x6000;
    p.put32(0x6B8388, top); // top cursor
    p.put32(
        0x6B8384,
        [0u32, 0x40, 0x100, 0x400, 0x2000, 0xFFFF_FF00][r.below(6) as usize],
    ); // main remaining
    p.put32(0x6B837C, POOL + 0x1000);
    p.put32(0x6B8370, POOL + 0x2000);
    p.put32(0x6B8378, [0u32, 0x100, 0x1000][r.below(3) as usize]);
    p.put32(0x6B8380, top);
    p.put32(0x6B8390, POOL + 0x3000);
    p.put32(0x6B839C, POOL + 0x400);
    p.put32(0x6B83B8, r.below(0x1000));
    for (i, t) in [0x3DFBFu32, 0x3E032, 0x3E063, 0x3E08D, 0x3E07B]
        .iter()
        .enumerate()
    {
        p.put32(0x3E094 + 4 * i as u32, *t); // jump table of 3DFA0 (kind 3 = zero arm 3E08D, kind 4 = unchecked arm 3E07B; T2048 fixed the swapped order)
    }
    // T2048: jump table of 3E0B0 (kind order 0 top, 1 region B, 2 region A, 3 default, 4 unchecked)
    for (i, t) in [0x3E13Bu32, 0x3E0C5, 0x3E100, 0x3E1B2, 0x3E199]
        .iter()
        .enumerate()
    {
        p.put32(0x3E1C4 + 4 * i as u32, *t);
    }
}

fn size_arg(r: &mut Rng) -> u32 {
    [
        0u32,
        1,
        0x10,
        0x11,
        0x25,
        0x7F,
        0x200,
        r.below(0x300),
        0xFFFF_FFF0,
    ][r.below(9) as usize]
}

#[test]
fn lifted_0003e970() {
    let mut case_index = 0usize;
    arena_drive(
        0x3E970,
        true,
        |r, p| {
            arena_state(r, p);
            // Force size-rounding and wrapping boundaries before randomized coverage, following
            // the measured `(size + 0xF) & !0xF` arithmetic.
            let s = match case_index {
                0 => 0,
                1 => 1,
                2 => 15,
                3 => 16,
                4 => 17,
                5 => 0xFFFF_FFF1,
                6 => 0xFFFF_FFFE,
                7 => 0xFFFF_FFFF,
                8 => {
                    // The size rounds to 16; both the guest cursor and accounting total wrap.
                    p.put32(0x6B839C, 0xFFFF_FFF0);
                    p.put32(0x6B83B8, 0xFFFF_FFF0);
                    16
                }
                9 => {
                    // A nonzero post-wrap cursor distinguishes wrapping from saturation.
                    p.put32(0x6B839C, 0xFFFF_FFF1);
                    p.put32(0x6B83B8, 0xFFFF_FFF1);
                    1
                }
                _ => size_arg(r),
            };
            case_index += 1;
            case(r, 0x3E970, vec![s])
        },
        |a| {
            let s = rd(&View(a.memory.0.0), STACK + 4);
            a.bump_allocate(s).unwrap()
        },
    );
}

#[test]
fn lifted_0003dfa0() {
    let mut boundary_index = 0usize;
    arena_drive(
        0x3DFA0,
        true,
        |r, p| {
            arena_state(r, p);
            let (k, s) = if boundary_index < 20 {
                let sizes = [0u32, 1, 15, 16, 17];
                let kinds = [0u32, 1, 2, 4];
                let s = sizes[boundary_index % sizes.len()];
                let k = kinds[boundary_index / sizes.len()];
                boundary_index += 1;
                (k, s)
            } else {
                (r.below(6), size_arg(r))
            };
            case(r, 0x3DFA0, vec![s, k])
        }, // original stack order: (size, kind)
        |a| {
            let (s, k) = (
                rd(&View(a.memory.0.0), STACK + 4),
                rd(&View(a.memory.0.0), STACK + 8),
            );
            a.alloc_round16(k, s).unwrap()
        },
    );
}

/// Exercise the measured wrapping boundary in `(size + 15) & !15` directly.
#[test]
fn lifted_0003dfa0_rounding_wrap_boundaries() {
    let mut boundary_index = 0usize;
    let cases = [0xFFFF_FFF0u32, 0xFFFF_FFF1, 0xFFFF_FFFF, 0xFFFF_FFEF];
    arena_drive(
        0x3DFA0,
        true,
        |r, p| {
            arena_state(r, p);
            let s = cases[boundary_index % cases.len()];
            boundary_index += 1;
            case(r, 0x3DFA0, vec![s, 1])
        },
        |a| {
            let s = rd(&View(a.memory.0.0), STACK + 4);
            a.alloc_round16(1, s).unwrap()
        },
    );
}

/// Tier 1 for the exact allocation request issued by effect-mesh wrapper `0x2BC150`.
/// The wrapper's full tail builder remains on the existing route; this isolates the fixed
/// kind-1 allocator request over the same 3,000 FABRICATED arena states as the allocator suite.
#[test]
fn lifted_0003dfa0_effect_mesh_buffer_request() {
    arena_drive(
        0x3DFA0,
        true,
        |r, p| {
            arena_state(r, p);
            case(r, 0x3DFA0, vec![0xD818, 1])
        },
        |a| a.alloc_round16(1, 0xD818).unwrap(),
    );
}

struct W4AtexitHost<'a> {
    memory: &'a mut View,
    stub: &'a mut Stub,
    cpu: &'a mut Cpu,
}

impl W4AtexitHost<'_> {
    fn nested(&mut self, va: u32, args: &[u32]) -> u32 {
        let entry_sp = self.cpu.esp;
        for value in args.iter().rev() {
            self.cpu.esp = self.cpu.esp.wrapping_sub(4);
            self.memory
                .write(tsfp_types::GuestAddr(self.cpu.esp), &value.to_le_bytes())
                .unwrap();
        }
        self.cpu.esp = self.cpu.esp.wrapping_sub(4);
        self.memory
            .write(tsfp_types::GuestAddr(self.cpu.esp), &0u32.to_le_bytes())
            .unwrap();
        self.stub.call(self.memory, va, self.cpu);
        let result = self.cpu.eax;
        self.cpu.esp = entry_sp;
        result
    }
}

impl AtexitHost for W4AtexitHost<'_> {
    fn read32(&mut self, address: u32) -> Result<u32, tsfp_engine_memory::MemoryFault> {
        Ok(rd(self.memory, address))
    }

    fn write32(&mut self, address: u32, value: u32) -> Result<(), tsfp_engine_memory::MemoryFault> {
        self.memory
            .write(tsfp_types::GuestAddr(address), &value.to_le_bytes())
            .unwrap();
        Ok(())
    }

    fn heap_size(
        &mut self,
        heap_handle: u32,
        pointer: u32,
    ) -> Result<u32, tsfp_engine_memory::MemoryFault> {
        Ok(self.nested(0x3CC96B, &[pointer, heap_handle]))
    }

    fn reallocate(
        &mut self,
        pointer: u32,
        requested_size: u32,
    ) -> Result<u32, tsfp_engine_memory::MemoryFault> {
        Ok(self.nested(0x3CAEEF, &[pointer, requested_size]))
    }
}

fn build_3c8a84(r: &mut Rng, p: &mut Pair) -> W4Case {
    const BASE_GLOBAL: u32 = 0x7EB848;
    const END_GLOBAL: u32 = 0x7EB844;
    let base = POOL + 0x1000;
    let used: u32 = [0, 4, 8, 12, 20, 28][r.below(6) as usize];
    let scenario = r.below(4);
    let capacity = match scenario {
        0 => used.wrapping_add(4).wrapping_add(r.below(12)),
        1 => used,
        2 => used.saturating_sub(4),
        _ => 0,
    };
    p.put32(BASE_GLOBAL, base);
    p.put32(END_GLOBAL, base.wrapping_add(used));
    for offset in (0..used).step_by(4) {
        p.put32(base.wrapping_add(offset), r.next());
    }

    let moved = POOL + 0x5000;
    let first_realloc = match scenario {
        1 => moved,
        2 => 0,
        3 => 0,
        _ => 0,
    };
    let retry_realloc = match scenario {
        2 => moved,
        3 => 0,
        _ => 0,
    };
    let entry = r.next();
    let mut test_case = case(r, 0x3C8A84, vec![entry]);
    test_case.script = vec![
        [capacity, 0, 0],
        [first_realloc, 0, 0],
        [retry_realloc, 0, 0],
    ];
    test_case
}

fn build_3c9800(r: &mut Rng, p: &mut Pair) -> W4Case {
    // The lifted leaf reads these immutable code-adjacent tables for its byte/word/dword
    // copy tails. They are part of the original function's dispatch state, not payload bytes.
    const TABLES: &[(u32, &[u32])] = &[
        (0x3C_994C, &[0x3C_995C, 0x3C_9964, 0x3C_9970, 0x3C_9984]),
        // The unaligned path indexes destination & 3 (indices 1..=3); the table starts at +4.
        (0x3C_9864, &[0x3C_9870, 0x3C_989C, 0x3C_98C0]),
        (0x3C_995C, &[0x3C_995C, 0x3C_9964, 0x3C_9970, 0x3C_9984]),
        (0x3C_99EC, &[0x3C_99FC, 0x3C_9A20, 0x3C_9A48]),
        (
            0x3C_98E0,
            &[
                0x3C_9900, 0x3C_9908, 0x3C_9910, 0x3C_9918, 0x3C_9920, 0x3C_9928, 0x3C_9930,
                0x3C_9943,
            ],
        ),
        (
            0x3C_9A98,
            &[
                0x3C_9A9C, 0x3C_9AA4, 0x3C_9AAC, 0x3C_9AB4, 0x3C_9ABC, 0x3C_9AC4, 0x3C_9ACC,
                0x3C_9ADF,
            ],
        ),
        (0x3C_9AE8, &[0x3C_9AF8, 0x3C_9B00, 0x3C_9B10, 0x3C_9B24]),
    ];
    for (base, targets) in TABLES {
        for (index, target) in targets.iter().enumerate() {
            p.put32(base.wrapping_add(index as u32 * 4), *target);
        }
    }
    let base = POOL + 0x1000;
    for offset in 0..512 {
        p.put8(base + offset, r.next() as u8);
    }
    // The lifted leaf's 0..=7 byte tail dispatch is still unresolved in this fixture.
    // Use only dword multiples so the differential stays on the covered bulk-copy path.
    let count = 8 * (1 + r.below(24));
    // Restrict this differential to dword-aligned disjoint ranges. The earlier unaligned
    // fixture diverged in the lifted dispatch path; this corpus isolates the bulk-copy path.
    let source = base + 16 + 4 * r.below(16);
    let destination = base + 320 + 4 * r.below(16);
    case(r, 0x3C9800, vec![destination, source, count])
}

w4_test!(
    lifted_003c9800_aligned_disjoint,
    0x3C9800,
    Regs::RET,
    &[],
    build_3c9800,
    |_, memory, cpu| {
        let destination = rd(memory, cpu.esp.wrapping_add(4));
        let source = rd(memory, cpu.esp.wrapping_add(8));
        let count = rd(memory, cpu.esp.wrapping_add(12));
        crt_memmove::move_payload(memory, destination, source, count)
            .expect("aligned disjoint fixture range is mapped");
        cpu.eax = destination;
    }
);

#[test]
fn realloc_copy_fixture_contains_every_lifted_dispatch_table() {
    let expected: &[(u32, &[u32])] = &[
        (0x3C_994C, &[0x3C_995C, 0x3C_9964, 0x3C_9970, 0x3C_9984]),
        (0x3C_9864, &[0x3C_9870, 0x3C_989C, 0x3C_98C0]),
        (0x3C_995C, &[0x3C_995C, 0x3C_9964, 0x3C_9970, 0x3C_9984]),
        (0x3C_99EC, &[0x3C_99FC, 0x3C_9A20, 0x3C_9A48]),
        (
            0x3C_98E0,
            &[
                0x3C_9900, 0x3C_9908, 0x3C_9910, 0x3C_9918, 0x3C_9920, 0x3C_9928, 0x3C_9930,
                0x3C_9943,
            ],
        ),
        (
            0x3C_9A98,
            &[
                0x3C_9A9C, 0x3C_9AA4, 0x3C_9AAC, 0x3C_9AB4, 0x3C_9ABC, 0x3C_9AC4, 0x3C_9ACC,
                0x3C_9ADF,
            ],
        ),
        (0x3C_9AE8, &[0x3C_9AF8, 0x3C_9B00, 0x3C_9B10, 0x3C_9B24]),
    ];
    let mut random = Rng(0x3C_9800);
    let mut pair = pair().lock().unwrap();
    pair.log.clear();
    let _ = build_3c9800(&mut random, &mut pair);
    for (base, targets) in expected {
        let actual: Vec<u32> = (0..targets.len())
            .map(|index| {
                u32::from_le_bytes(std::array::from_fn(|byte| {
                    pair.get(false, base.wrapping_add(index as u32 * 4 + byte as u32))
                }))
            })
            .collect();
        assert_eq!(
            actual, *targets,
            "wrong indexed entries for dispatch table {base:#x}"
        );
    }
}

fn trace_call_args(trace: &[[u32; 16]]) -> Vec<(u32, [u32; 3])> {
    trace
        .iter()
        .map(|entry| (entry[0], [entry[3], entry[4], entry[5]]))
        .collect()
}

/// Tier 1 against the lifted CRT atexit caller over 3,000 deterministic FABRICATED tables.
/// This covers in-capacity append, first growth success, retry success and two-failure preservation;
/// it does not claim the underlying CRT allocator's general realloc semantics.
#[test]
fn lifted_003c8a84() {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let pair = &mut *guard;
    let mut rng = Rng(0x3C8A84);
    for index in 0..SEEDS {
        zero(pair, POOL, POOL_LEN);
        zero(pair, STACK, ARG_FRAME);
        pair.log.clear();
        let test_case = build_3c8a84(&mut rng, pair);
        let logged: Vec<u32> = pair.log.drain(..).collect();
        let (c_regs, c_trace) = run_c(pair, &test_case);

        let mut stub = Stub {
            script: test_case.script.clone(),
            trace: vec![],
        };
        let mut cpu = Cpu {
            eax: test_case.regs[0],
            ecx: test_case.regs[1],
            edx: test_case.regs[2],
            ebx: test_case.regs[3],
            esi: test_case.regs[4],
            edi: test_case.regs[5],
            ebp: test_case.regs[6],
            esp: STACK,
        };
        let mut view = View(pair.rust);
        let entry = rd(&view, STACK + 4);
        let heap_handle = cpu.esi;
        let result = {
            let mut host = W4AtexitHost {
                memory: &mut view,
                stub: &mut stub,
                cpu: &mut cpu,
            };
            crt_atexit::register_atexit(&mut host, heap_handle, entry)
                .expect("fabricated guest image is mapped")
        };
        cpu.eax = result;
        let rust_regs = [
            cpu.eax,
            cpu.ecx,
            cpu.edx,
            cpu.ebx,
            cpu.esi,
            cpu.edi,
            cpu.ebp,
            cpu.esp.wrapping_add(4),
        ];

        let mut why = vec![];
        if c_regs[0] != rust_regs[0] || c_regs[4] != rust_regs[4] || c_regs[7] != rust_regs[7] {
            why.push(format!(
                "result/callee-saved regs C {c_regs:x?} Rust {rust_regs:x?}"
            ));
        }
        if trace_call_args(&c_trace) != trace_call_args(&stub.trace) {
            why.push(format!(
                "nested call target/args C {:?} Rust {:?}",
                trace_call_args(&c_trace),
                trace_call_args(&stub.trace)
            ));
        }
        why.extend(mem_diff(pair, &logged, test_case.va));
        assert!(why.is_empty(), "seed {index}: {why:?}");
    }
}

/// Tier 1 against the lifted audio workspace constructor over 3,000 FABRICATED arena states.
#[test]
fn lifted_000298b0() {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let mut rng = Rng(0x298B0);
    for index in 0..SEEDS {
        zero(p, POOL, POOL_LEN);
        zero(p, STACK, ARG_FRAME);
        p.log.clear();
        arena_state(&mut rng, p);
        let owner = POOL + 0x300;
        p.put32(owner + 8, rng.next());
        p.put32(0x6B837C, POOL + 0x1000);
        p.put32(
            0x6B8378,
            [0u32, 0x35FF0, 0x36000, 0x40000][rng.below(4) as usize],
        );
        let mut test_case = case(&mut rng, 0x298B0, vec![]);
        test_case.regs[1] = owner;
        let logged: Vec<u32> = p.log.drain(..).collect();
        let (c_regs, c_trace) = run_c(p, &test_case);

        let mut view = View(p.rust);
        let mut hooks = Oom::default();
        let eax = tsfp_engine_memory::wrappers::AllocWrappers {
            memory: &mut view,
            hooks: &mut hooks,
        }
        .alloc_audio_workspace_298b0(owner)
        .expect("fabricated guest image is mapped");
        let c_reports: Vec<u32> = c_trace
            .iter()
            .filter(|entry| entry[0] == 0x389B0)
            .map(|entry| entry[3])
            .collect();
        let mut why = vec![];
        if c_regs[0] != eax {
            why.push(format!("eax C {:x} Rust {eax:x}", c_regs[0]));
        }
        if c_reports != hooks.0 {
            why.push(format!("OOM reports C {c_reports:x?} Rust {:x?}", hooks.0));
        }
        why.extend(mem_diff(p, &logged, test_case.va));
        assert!(why.is_empty(), "seed {index}: {why:?}");
    }
}

fn build_3e1e0(r: &mut Rng, p: &mut Pair, boundary_index: &mut usize) -> W4Case {
    arena_state(r, p);
    if *boundary_index < 4 {
        let sizes = [0u32, 0x10, 0x30, 0x1F0];
        let size = sizes[*boundary_index];
        *boundary_index += 1;
        let top = POOL + 0x6000;
        let block = top + 0x10;
        p.put32(0x6B8388, top);
        p.put32(0x6B8384, 0x200);
        p.put32(block - 16, 0x4143_4F4C);
        p.put32(block - 12, 0x4D45_4D4C);
        p.put32(block - 8, size);
        p.put32(block - 4, top);
        return case(r, 0x3E1E0, vec![block]);
    }
    let alias_top = r.below(2) == 0;
    let top = POOL + 0x6000;
    let block = if alias_top {
        p.put32(0x6B8380, 0x4143_4F4C);
        p.put32(0x6B8384, 0x4D45_4D4C);
        p.put32(0x6B838C, top);
        0x6B8390
    } else {
        let block = POOL + 0x5010;
        p.put32(
            block - 16,
            if r.below(5) == 0 {
                r.next()
            } else {
                0x4143_4F4C
            },
        );
        p.put32(
            block - 12,
            if r.below(5) == 0 {
                r.next()
            } else {
                0x4D45_4D4C
            },
        );
        p.put32(
            block - 8,
            [0u32, 0x10, 0x30, r.below(0x200)][r.below(4) as usize],
        );
        p.put32(block - 4, if r.below(4) == 0 { r.next() } else { top });
        block
    };
    case(r, 0x3E1E0, vec![block])
}
#[test]
fn lifted_0003e1e0() {
    let mut boundary_index = 0usize;
    arena_drive(
        0x3E1E0,
        true,
        |r, p| build_3e1e0(r, p, &mut boundary_index),
        |a| {
            let b = rd(&View(a.memory.0.0), STACK + 4);
            a.release_tagged_block_with_registers(b).unwrap().eax
        },
    );
}

#[test]
fn lifted_0003e240() {
    let mut boundary_index = 0usize;
    arena_drive(
        0x3E240,
        false,
        |r, p| {
            arena_state(r, p);
            if boundary_index < 4 {
                let spans = [0u32, 0x10, 0x100, 0x2000];
                let span = spans[boundary_index];
                boundary_index += 1;
                let top = POOL + 0x6000;
                p.put32(0x6B8388, top);
                p.put32(0x6B8380, top.wrapping_add(span));
                p.put32(0x6B8384, 0x400);
                p.put32(top - 0x20, 0xA11C_E001);
                p.put32(top + span, 0xA11C_E002);
            }
            case(r, 0x3E240, vec![])
        },
        |a| {
            a.rewind_top().unwrap();
            0
        },
    );
}

#[test]
fn lifted_0003e420_and_0003e430() {
    arena_drive(
        0x3E420,
        false,
        |r, p| {
            seed(r, p, &[0x4C81B4]);
            let v = r.next();
            case(r, 0x3E420, vec![v])
        },
        |a| {
            let v = rd(&View(a.memory.0.0), STACK + 4);
            a.set_ledger_flag(v).unwrap();
            0
        },
    );
    arena_drive(
        0x3E430,
        true,
        |r, p| {
            seed(r, p, &[0x4C81B4]);
            case(r, 0x3E430, vec![])
        },
        |a| a.ledger_flag().unwrap(),
    );
}

// ======== T1920: network/syslink call-free roots (crates/tsfp-network/src/w4n_syslink.rs) ========
use tsfp_network::w4n_syslink as sl;

struct SH<'a> {
    v: &'a mut View,
    link: u32,
}
impl sl::SyslinkHost for SH<'_> {
    fn read32(&mut self, a: u32) -> u32 {
        rd(self.v, a)
    }
    fn read8(&mut self, a: u32) -> u8 {
        self.v.byte(a)
    }
    fn write32(&mut self, a: u32, value: u32) {
        for (i, b) in value.to_le_bytes().iter().enumerate() {
            self.v.set(a.wrapping_add(i as u32), *b);
        }
    }
    fn xnet_link_status(&mut self) -> u32 {
        self.link
    }
    fn platform_call(&mut self, target: u32, _: &[u32]) -> u32 {
        panic!("unexpected platform call {target:x} in a call-free root");
    }
    fn pool_call(&mut self, target: u32, _: &[u32]) {
        panic!("unexpected pool call {target:x} in a call-free root");
    }
    fn stack_scratch(&mut self) -> u32 {
        panic!("unexpected stack scratch");
    }
}

/// Differential for a syslink root: EAX where `ret` and the seeded globals/tables.
fn sys_drive(
    seed: u64,
    ret: bool,
    mut build: impl FnMut(&mut Rng, &mut Pair) -> W4Case,
    port: impl Fn(&mut SH) -> u32,
) {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let mut r = Rng(seed);
    for index in 0..SEEDS {
        zero(p, POOL, POOL_LEN);
        zero(p, STACK, ARG_FRAME);
        p.log.clear();
        let c = build(&mut r, p);
        let logged: Vec<u32> = p.log.drain(..).collect();
        let (cregs, _) = run_c(p, &c);
        let mut view = View(p.rust);
        let link = c.script.first().map_or(0, |row| row[0]);
        let eax = port(&mut SH { v: &mut view, link });
        let mut why = vec![];
        if ret && cregs[0] != eax {
            why.push(format!("eax C {:x} Rust {eax:x}", cregs[0]));
        }
        why.extend(mem_diff(p, &logged, c.va));
        assert!(
            why.is_empty(),
            "root {:08X} seed {seed:#x} case {index}: {why:?}",
            c.va
        );
    }
}

macro_rules! sys_flag_test {
    ($name:ident, $va:expr, $global:expr, $port:expr, $ret:expr) => {
        #[test]
        fn $name() {
            sys_drive(
                $va,
                $ret,
                |r, p| {
                    seed(r, p, &[$global]);
                    case(r, $va, vec![])
                },
                $port,
            );
        }
    };
}
sys_flag_test!(
    lifted_0002dc50,
    0x2DC50,
    0x665908,
    |h| {
        sl::set_global_665908_to_1(h);
        0
    },
    false
);
sys_flag_test!(
    lifted_0002dc60,
    0x2DC60,
    0x665908,
    |h| sl::set_global_665908_to_2_return_true(h),
    true
);
sys_flag_test!(
    lifted_0002e200,
    0x2E200,
    0x66591C,
    |h| {
        sl::set_global_66591c_to_1(h);
        0
    },
    false
);
sys_flag_test!(
    lifted_0002e210,
    0x2E210,
    0x66591C,
    |h| sl::set_global_66591c_to_2_return_true(h),
    true
);
sys_flag_test!(
    lifted_0002e220,
    0x2E220,
    0x665920,
    |h| {
        sl::set_global_665920_to_1(h);
        0
    },
    false
);
sys_flag_test!(
    lifted_0002e230,
    0x2E230,
    0x665920,
    |h| sl::set_global_665920_to_2_return_true(h),
    true
);
sys_flag_test!(
    lifted_0002e510,
    0x2E510,
    0x686644,
    |h| {
        sl::global_686644_set_one(h);
        0
    },
    false
);
sys_flag_test!(
    lifted_0002df20,
    0x2DF20,
    0x686644,
    |h| {
        sl::global_686644_set_one_wrapper(h);
        0
    },
    false
);

#[test]
fn lifted_0002e240() {
    sys_drive(
        0x2E240,
        false,
        |r, p| {
            seed(r, p, &[0x665920, 0x66591C]);
            case(r, 0x2E240, vec![])
        },
        |h| {
            sl::clear_globals_665920_and_66591c(h);
            0
        },
    );
}
#[test]
fn lifted_0002e520() {
    sys_drive(
        0x2E520,
        true,
        |r, _| case(r, 0x2E520, vec![]),
        |_| sl::return_constant_0x24(),
    );
}
#[test]
fn lifted_0002e500() {
    sys_drive(
        0x2E500,
        true,
        |r, _| {
            let mut c = case(r, 0x2E500, vec![]);
            c.script = rows(r, 1, &[0, 1, 0x30, 0xFFFF_FFFF]);
            c
        },
        |h| sl::ethernet_link_status_nonzero(h),
    );
}

fn peer_tables(r: &mut Rng, p: &mut Pair) {
    for slot in 0..16u32 {
        let used = [0u32, 1, 2][r.below(3) as usize];
        p.put32(0x666040 + 4 * slot, used);
        let addr = if r.below(3) == 0 {
            0x0A0B_0C0D
        } else {
            r.next()
        };
        p.put32(0x66648C + 0x10 * slot, addr);
        p.put32(0x66648C + 0x10 * slot + 4, r.next());
    }
}
#[test]
fn lifted_0002e4a0() {
    sys_drive(
        0x2E4A0,
        true,
        |r, p| {
            peer_tables(r, p);
            let i = r.below(18);
            p.put32(0x774040, i);
            case(r, 0x2E4A0, vec![])
        },
        |h| sl::current_peer_slot_flags(h),
    );
}
#[test]
fn lifted_0002e4b0() {
    sys_drive(
        0x2E4B0,
        true,
        |r, p| {
            peer_tables(r, p);
            let probe = POOL + 0x100;
            p.put32(
                probe,
                if r.below(2) == 0 {
                    0x0A0B_0C0D
                } else {
                    r.next()
                },
            );
            case(r, 0x2E4B0, vec![probe])
        },
        |h| {
            let a = rd(h.v, STACK + 4);
            sl::peer_find_by_addr(h, a)
        },
    );
}

// ======== T2048: engine/memory pool allocator and aligned arena allocation ========
// Roots run for real on the C side with their nested lifted callees (3DFA0, 3E9A0, 3EA40, 3EAF0,
// 3EBD0, 3ECA0 are all roots, 389B0 and the reclaim callback are recorded stubs). The Rust side
// is crates/tsfp-engine-memory/src/pool.rs and service_slice.rs.
use tsfp_engine_memory::pool::{Pool, PoolHooks};
use tsfp_engine_memory::service_slice::{GameAllocator, MemoryServices};
use tsfp_engine_memory::wrappers::PointPoolRandom;

const RING: u32 = POOL + 0x1000; // pool buffer
const OWNERS: u32 = POOL + 0x7000; // words the reclaim path may clear
const CALLBACK: u32 = 0x15BCC0; // recorded stub, one stack argument

#[derive(Default)]
struct PoolHost {
    oom: Vec<u32>,
    reclaimed: Vec<(u32, u32)>,
    script: Vec<[u32; 3]>,
    script_index: usize,
}
impl PoolHost {
    fn next_stub_result(&mut self) -> u32 {
        let result = self
            .script
            .get(self.script_index)
            .map_or(0, |registers| registers[0]);
        self.script_index += 1;
        result
    }
}
impl ArenaHooks for PoolHost {
    fn out_of_memory(&mut self, argument: u32) {
        self.oom.push(argument);
        let _ = self.next_stub_result();
    }
}
impl PoolHooks for PoolHost {
    fn reclaim(&mut self, callback: u32, argument: u32) {
        assert!(matches!(callback, CALLBACK | 0x210FF0 | 0x10C0E0));
        self.reclaimed.push((callback, argument));
        let _ = self.next_stub_result();
    }

    fn reclaim_block_with_memory<M: tsfp_engine_memory::GuestMemory>(
        &mut self,
        memory: &mut M,
        payload: u32,
        callback: u32,
        argument: u32,
    ) -> Result<(), tsfp_engine_memory::pool::PoolError> {
        self.reclaim_block(payload, callback, argument)?;
        match callback {
            0x210FF0 => tsfp_engine_memory::wrappers::clear_random_point_pool_owner(memory)?,
            0x10C0E0 => tsfp_engine_memory::wrappers::release_particle_buffer(
                memory,
                self,
                tsfp_engine_memory::wrappers::ParticleBufferOwner::from_guest_address(argument),
            )?,
            _ => {}
        }
        Ok(())
    }
}
impl PointPoolRandom for PoolHost {
    fn next_point_random(&mut self) -> u32 {
        self.next_stub_result()
    }
}

/// A well formed ring of 2..12 blocks (sizes multiples of 0x40, backlinks consistent), with at
/// least one reclaimable block. Returns (total bytes, header addresses).
fn pool_ring(r: &mut Rng, p: &mut Pair, locked: bool) -> (u32, Vec<u32>) {
    let count = 2 + r.below(11);
    let mut headers = vec![];
    let (mut at, mut previous) = (RING, 0u32);
    for index in 0..count {
        let size = 0x40 * (1 + r.below(16));
        let mut state = if r.below(3) == 0 { 0 } else { 1 };
        if index == 0 && state == 0 && r.below(2) == 0 {
            state = 1;
        }
        if locked && r.below(5) == 0 {
            state = 2;
        }
        // A real ring never holds two adjacent free blocks (free coalesces); keep that invariant.
        if state == 0 && index > 0 && p.get(false, *headers.last().unwrap()) == 0 {
            state = 1;
        }
        p.put32(at, state);
        p.put32(at + 4, size);
        p.put32(at + 8, previous);
        if state == 1 {
            p.put32(
                at + 0xC,
                if r.below(2) == 0 {
                    0
                } else {
                    OWNERS + 4 * r.below(64)
                },
            );
            p.put32(at + 0x10, if r.below(2) == 0 { 0 } else { CALLBACK });
        } else {
            p.put32(at + 0xC, 0);
            p.put32(at + 0x10, 0);
        }
        headers.push(at);
        previous = size;
        at += size;
    }
    if !locked && !headers.iter().any(|h| p.get(false, *h) == 1) {
        p.put32(RING, 1);
    }
    let total = at - RING;
    let free_final: u32 = headers
        .iter()
        .filter(|h| p.get(false, **h) == 0)
        .map(|h| rd(&View(p.c), *h + 4))
        .sum();
    for (i, v) in [
        (0x6B83C8u32, RING),
        (0x6B83D0, RING),
        (0x6B83D4, RING + total),
        (0x6B83C4, total),
        (0x6B83C0, free_final),
        (0x6B83CC, headers[r.below(headers.len() as u32) as usize]),
        (0x6B83BC, r.below(2)),
        (0x4C81B8, total),
    ] {
        p.put32(i, v);
    }
    for i in 0..64 {
        p.put32(OWNERS + 4 * i, 0xA5A5_0000 + i);
    }
    (total, headers)
}

fn pool_drive(
    seed: u64,
    ret: bool,
    mut build: impl FnMut(&mut Rng, &mut Pair) -> W4Case,
    port: impl Fn(&mut Pool<AMem, PoolHost>) -> u32,
) {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let mut r = Rng(seed);
    let (
        mut oom_cases,
        mut reclaim_cases,
        mut zero_returns,
        mut point_callback_cases,
        mut particle_callback_cases,
    ) = (0usize, 0usize, 0usize, 0usize, 0usize);
    for index in 0..SEEDS {
        zero(p, POOL, POOL_LEN);
        zero(p, STACK, ARG_FRAME);
        p.log.clear();
        let c = build(&mut r, p);
        let logged: Vec<u32> = p.log.drain(..).collect();
        let (cregs, ctrace) = run_c(p, &c);
        let c_oom: Vec<u32> = ctrace
            .iter()
            .filter(|e| e[0] == 0x389B0)
            .map(|e| e[3])
            .collect();
        let c_reclaim: Vec<(u32, u32)> = ctrace
            .iter()
            .filter(|e| matches!(e[0], CALLBACK | 0x210FF0 | 0x10C0E0))
            .map(|e| (e[0], e[3]))
            .collect();
        if c.va == 0x211040 && c_reclaim.iter().any(|&(callback, _)| callback == 0x210FF0) {
            point_callback_cases += 1;
        }
        if c.va == 0x10C190 && c_reclaim.iter().any(|&(callback, _)| callback == 0x10C0E0) {
            particle_callback_cases += 1;
        }
        let known_callees: &[u32] = match c.va {
            // The point-pool owner wrapper also calls its fixed allocation helpers.
            0x211040 => &[0x389B0, CALLBACK, 0x15BCC0, 0x153B40, 0x210FF0],
            0x10C190 => &[0x389B0, CALLBACK, 0x10C0E0, 0x10C120],
            _ => &[0x389B0, CALLBACK],
        };
        assert!(
            ctrace.iter().all(|e| known_callees.contains(&e[0])),
            "unexpected callee {ctrace:x?}"
        );
        let mut view = View(p.rust);
        let mut host = PoolHost {
            script: c.script.clone(),
            ..PoolHost::default()
        };
        let eax = port(&mut Pool {
            memory: &mut AMem(&mut view),
            hooks: &mut host,
        });
        // 0x210FF0 is a zero-parameter cdecl callback. The allocator still pushes its stored
        // owner word, but this callback cannot observe it; compare only the callback ABI's
        // declared arguments with the lifted call trace.
        let rust_reclaim: Vec<(u32, u32)> = host
            .reclaimed
            .iter()
            .map(|&(callback, argument)| {
                (callback, if callback == 0x210FF0 { 0 } else { argument })
            })
            .collect();
        let mut why = vec![];
        if ret && cregs[0] != eax {
            why.push(format!("eax C {:x} Rust {eax:x}", cregs[0]));
        }
        if c_oom != host.oom {
            why.push(format!("oom C {c_oom:x?} Rust {:x?}", host.oom));
        }
        if c_reclaim != rust_reclaim {
            let c_reclaim_sites: Vec<(u32, u32, u32)> = ctrace
                .iter()
                .filter(|e| matches!(e[0], CALLBACK | 0x210FF0 | 0x10C0E0))
                .map(|e| (e[0], e[2], e[3]))
                .collect();
            why.push(format!(
                "reclaim callbacks C {c_reclaim:x?} Rust {:x?}; C (target,return-pc,arg) {c_reclaim_sites:x?}",
                rust_reclaim,
            ));
        }
        why.extend(mem_diff(p, &logged, c.va));
        if !why.is_empty() && c.va == 0x211040 {
            let c_pointer = rd(&View(p.c), 0x75C0CC);
            let rust_pointer = rd(&View(p.rust), 0x75C0CC);
            let c_words: Vec<u32> = (0..5)
                .map(|i| rd(&View(p.c), c_pointer.wrapping_add(4 * i)))
                .collect();
            let rust_words: Vec<u32> = (0..5)
                .map(|i| rd(&View(p.rust), rust_pointer.wrapping_add(4 * i)))
                .collect();
            let rng_entries: Vec<(u32, u32)> = ctrace
                .iter()
                .filter(|e| e[0] == 0x153B40)
                .map(|e| (e[2], e[6]))
                .collect();
            why.push(format!(
                "RNG (callsite,EAX-at-entry) {rng_entries:x?}; count C/Rust {:x}/{:x}, ptr C/Rust {c_pointer:x}/{rust_pointer:x}, first point dwords C/Rust {c_words:x?}/{rust_words:x?}",
                rd(&View(p.c), 0x75C0A8),
                rd(&View(p.rust), 0x75C0A8),
            ));
        }
        assert!(
            why.is_empty(),
            "root {:08X} seed {seed:#x} case {index}: {why:?}",
            c.va
        );
        oom_cases += usize::from(!c_oom.is_empty());
        reclaim_cases += usize::from(!c_reclaim.is_empty());
        zero_returns += usize::from(ret && cregs[0] == 0);
    }
    if seed == 0x211040 {
        assert!(
            point_callback_cases > 0,
            "0x211040 fixture did not exercise its native reclaim callback"
        );
    }
    if seed == 0x10C190 {
        assert!(
            particle_callback_cases > 0,
            "0x10C190 fixture did not exercise its native owner reclaim callback"
        );
    }
    eprintln!(
        "w4 pool {seed:#x}: {SEEDS} cases, {oom_cases} oom, {reclaim_cases} with callbacks, {point_callback_cases} point-owner callbacks, {particle_callback_cases} particle-owner callbacks, {zero_returns} zero returns"
    );
}

fn arg(a: &Pool<AMem, PoolHost>, n: u32) -> u32 {
    rd(&View(a.memory.0.0), STACK + 4 + 4 * n)
}
fn pool_unwrap(result: Result<u32, tsfp_engine_memory::pool::PoolError>) -> u32 {
    result.expect("pool op")
}

fn needed(r: &mut Rng, total: u32) -> u32 {
    [
        0u32,
        1,
        0x40,
        0x41,
        r.below(total / 2 + 1),
        total / 2,
        r.below(total + 1),
        0xFFFF_FFC0,
    ][r.below(8) as usize]
}

#[test]
fn lifted_0003eaf0() {
    let mut boundary_index = 0usize;
    pool_drive(
        0x3EAF0,
        true,
        |r, p| {
            let (t, _) = pool_ring(r, p, false);
            let n = if boundary_index < 7 {
                let directed = [0u32, 1, 0x3F, 0x40, 0x41, t, 0xFFFF_FFC0];
                let n = directed[boundary_index];
                boundary_index += 1;
                n
            } else {
                needed(r, t)
            };
            case(r, 0x3EAF0, vec![n])
        },
        |a| {
            let n = arg(a, 0);
            pool_unwrap(a.find_or_reclaim(n))
        },
    );
}

#[test]
fn lifted_0003ebd0() {
    let mut force_wrap = true;
    let mut force_exact_fit = true;
    let mut boundary_index = 0usize;
    pool_drive(
        0x3EBD0,
        true,
        |r, p| {
            let (t, _) = pool_ring(r, p, false);
            let size = if force_wrap {
                force_wrap = false;
                0xFFFF_FFA1
            } else if force_exact_fit {
                force_exact_fit = false;
                // Valid pool spans and rounded requests are multiples of 0x40. Match the
                // current block's span to force the reachable zero-remainder edge.
                let cursor = rd(&View(p.c), 0x6B83CC);
                rd(&View(p.c), cursor + 4).wrapping_sub(0x5F)
            } else if boundary_index < 8 {
                // Exercise both sides of the 64-byte rounding transition before random cases.
                let size = [0u32, 1, 31, 32, 33, 63, 64, 65][boundary_index];
                boundary_index += 1;
                size
            } else {
                [
                    0u32,
                    1,
                    0x20,
                    0x21,
                    0x41,
                    r.below(t),
                    r.below(0x400),
                    t / 3,
                    t,
                ][r.below(9) as usize]
            };
            let cb = if r.below(2) == 0 { 0 } else { CALLBACK };
            let owner = OWNERS + 4 * r.below(64);
            case(r, 0x3EBD0, vec![size, owner, cb])
        },
        |a| {
            let (s, g, c) = (arg(a, 0), arg(a, 1), arg(a, 2));
            pool_unwrap(a.allocate_reclaimable(s, g, c))
        },
    );
}

#[test]
fn lifted_0003ed20() {
    let mut case_index = 0usize;
    pool_drive(
        0x3ED20,
        true,
        |r, p| {
            // state 2 blocks make a kind 1 reclaim loop forever in the original (T1372), so only
            // the locked route sees them
            let directed = case_index < 8;
            let kind = if directed {
                if case_index % 2 == 0 { 1 } else { 2 }
            } else {
                [1u32, 1, 1, 2, 2, 0, 3, 4, r.next()][r.below(9) as usize]
            };
            let (t, _) = pool_ring(r, p, kind == 2);
            let size = if directed {
                [0u32, 1, 31, 32, 33, 63, 64, 65][case_index]
            } else {
                [0u32, 1, 0x20, 0x41, r.below(t / 2), r.below(0x400)][r.below(6) as usize]
            };
            case_index += 1;
            let cb = if r.below(2) == 0 { 0 } else { CALLBACK };
            let owner = OWNERS + 4 * r.below(64);
            case(r, 0x3ED20, vec![size, kind, owner, cb])
        },
        |a| {
            let (s, k, g, c) = (arg(a, 0), arg(a, 1), arg(a, 2), arg(a, 3));
            pool_unwrap(a.allocate_by_kind(s, k, g, c))
        },
    );
}

#[test]
fn lifted_00156db0_fixed_pool_initializer_matches_rust_contract() {
    let mut guard = pair().lock().unwrap_or_else(|error| error.into_inner());
    let p = &mut *guard;
    let mut random = Rng(0x156D_B0_2048);
    for index in 0..SEEDS {
        zero(p, POOL, POOL_LEN);
        zero(p, STACK, ARG_FRAME);
        p.log.clear();
        let pool = POOL + 0x200;
        let storage = POOL + 0x1000;
        let stride = 4 + 4 * random.below(16);
        let count = random.below(33);
        let fixture = case(&mut random, 0x156DB0, vec![pool, stride, storage, count]);
        let _ = run_c(p, &fixture);
        let mut memory = View(p.rust);
        let rust_result = tsfp_engine_memory::fixed_pool::initialize_freelist(
            &mut memory,
            pool,
            stride,
            storage,
            count,
        );
        assert!(rust_result.is_ok(), "case {index}: {rust_result:?}");
        assert_eq!(mem_diff(p, &[], fixture.va), None, "case {index}");
    }
}

#[test]
fn lifted_00211040() {
    pool_drive(
        0x211040,
        true,
        |r, p| {
            let (total, headers) = pool_ring(r, p, false);
            for header in headers {
                if p.get(false, header) == 1 {
                    p.put32(header + 0xC, 0x75C0CC);
                    p.put32(header + 0x10, 0x210FF0);
                }
            }
            p.put32(0x75C0A8, r.next());
            p.put32(0x75C0CC, if r.below(2) == 0 { 0 } else { RING + total });
            let mut c = case(r, 0x211040, vec![]);
            c.script = (0..64).map(|_| [r.next(), r.next(), r.next()]).collect();
            c
        },
        |pool| {
            let mut wrappers = tsfp_engine_memory::wrappers::AllocWrappers {
                memory: &mut *pool.memory,
                hooks: &mut *pool.hooks,
            };
            pool_unwrap(wrappers.alloc_random_point_pool_75c0cc())
        },
    );
}

#[test]
fn lifted_0003ea40_and_0003ead0() {
    for (seed, root) in [(0x3EA40u64, 0x3EA40u32), (0x3EAD0, 0x3EAD0)] {
        let mut case_index = 0usize;
        pool_drive(
            seed,
            false,
            |r, p| {
                let (_, h) = pool_ring(r, p, true);
                // The measured free contract gates every mutation on state == 1. Keep
                // explicit non-1 values in the oracle corpus; random ring generation
                // otherwise exercises mostly states 0 and 1.
                let explicit_state = case_index % 5;
                case_index += 1;
                let pointer = if explicit_state == 2 || explicit_state == 3 || explicit_state == 4 {
                    let header = h[0];
                    p.put32(header, [0, 1, 2, 3, u32::MAX][explicit_state]);
                    header + 0x20
                } else {
                    match r.below(8) {
                        0 => 0,
                        1 => r.next(),
                        _ => h[r.below(h.len() as u32) as usize] + 0x20,
                    }
                };
                let pointer = if root == 0x3EA40 && pointer < 0x20 {
                    0x20 + RING
                } else {
                    pointer
                };
                // a wild pointer is outside the compared pool, keep it inside the arena
                let pointer = if pointer > RING + 0x8000 || pointer < RING {
                    RING + 0x20
                } else {
                    pointer
                };
                case(r, root, vec![pointer])
            },
            |a| {
                let pointer = arg(a, 0);
                let result = Pool {
                    memory: &mut *a.memory,
                    hooks: &mut *a.hooks,
                }
                .free(pointer);
                pool_unwrap(result.map(|()| 0))
            },
        );
    }
}

#[test]
fn lifted_0010c0e0_particle_owner_release() {
    pool_drive(
        0x10C0E0,
        false,
        |r, p| {
            let _ = pool_ring(r, p, false);
            let owner = if r.below(8) == 0 { 0 } else { OWNERS };
            // Nested guest calls are stubbed in this Tier 1 harness, including the pool free.
            // Non-null payload release is covered at the Rust C-ABI boundary below.
            let payload = 0;
            if owner != 0 {
                p.put32(owner + 4, r.below(2));
                p.put32(owner + 0x40, payload);
                p.put32(owner + 0x44, r.next());
                p.put32(owner + 0x48, r.next());
                p.put32(owner + 0x4C, r.next());
                p.put32(owner + 0x50, r.next());
            }
            case(r, 0x10C0E0, vec![owner])
        },
        |a| {
            let owner = arg(a, 0);
            tsfp_engine_memory::wrappers::release_particle_buffer(
                &mut *a.memory,
                &mut *a.hooks,
                tsfp_engine_memory::wrappers::ParticleBufferOwner::from_guest_address(owner),
            )
            .expect("particle owner release");
            0
        },
    );
}

#[test]
fn lifted_0010c190_particle_buffer_owner_reclaim() {
    pool_drive(
        0x10C190,
        false,
        |r, p| {
            let (total, headers) = pool_ring(r, p, false);
            let owner = if r.below(8) == 0 { 0 } else { OWNERS };
            // The lifted C differential stubs nested guest calls. Keep the reclaim callback's
            // payload null so this caller comparison measures the same callback boundary on both
            // sides; the callback's non-null free behavior is covered by its own ABI test.
            let entry_count = if owner != 0 {
                let header = headers
                    .iter()
                    .copied()
                    .find(|header| p.get(false, *header) == 1)
                    .expect("pool fixture has a live block");
                let descriptor = 0x00A0_0000;
                let descriptor_table = 0x00A0_0100;
                let reclaim_owner = 0x00A0_0200;
                p.put32(header + 0x0C, reclaim_owner);
                p.put32(header + 0x10, 0x10C0E0);
                for offset in [4, 0x40, 0x44, 0x48, 0x4C, 0x50] {
                    p.put32(reclaim_owner + offset, 0);
                }
                // The callback owner is empty; its release is a no-op before the allocation.
                p.put32(owner + 4, 0);
                p.put32(owner + 0x0C, descriptor);
                p.put32(owner + 0x40, 0);
                p.put32(owner + 0x44, 0);
                p.put32(owner + 0x48, 0);
                p.put32(owner + 0x4C, 0);
                p.put32(owner + 0x50, 0);
                p.put32(descriptor + 0x4C, descriptor_table);
                let descriptor_kind = r.below(9);
                p.put32(descriptor_table + 0x0C, descriptor_kind);
                let stride = if matches!(descriptor_kind, 5 | 6) {
                    0x58
                } else {
                    0x34
                };
                // The lifted allocator has an unbounded search when a request cannot fit in the
                // fabricated pool. Keep every request within this ring's total extent; the signed
                // cap and overflow boundaries are covered by the C ABI/request tests instead.
                let max_count = total.saturating_sub(0x5F) / stride;
                let entry_count = match r.below(4) {
                    0 => 0,
                    1 => max_count,
                    2 => max_count / 2,
                    _ => r.below(max_count + 1),
                };
                entry_count
            } else {
                0
            };
            let mut c = case(r, 0x10C190, vec![owner]);
            if owner != 0 {
                c.script.push([entry_count as u32, 0, 0]);
                c.script.extend(rows(r, 32, &[0]));
            }
            c
        },
        |a| {
            let owner = arg(a, 0);
            if owner == 0 {
                return 0;
            }
            tsfp_engine_memory::wrappers::release_particle_buffer(
                &mut *a.memory,
                &mut *a.hooks,
                tsfp_engine_memory::wrappers::ParticleBufferOwner::from_guest_address(owner),
            )
            .expect("initial particle owner release");
            let entry_count = a.hooks.next_stub_result() as i32;
            let descriptor = a
                .memory
                .read_u32(tsfp_types::GuestAddr(owner + 0x0C))
                .unwrap();
            let descriptor_table = a
                .memory
                .read_u32(tsfp_types::GuestAddr(descriptor + 0x4C))
                .unwrap();
            let descriptor_kind = a
                .memory
                .read_u32(tsfp_types::GuestAddr(descriptor_table + 0x0C))
                .unwrap();
            tsfp_engine_memory::wrappers::allocate_particle_buffer(
                &mut *a.memory,
                &mut *a.hooks,
                tsfp_engine_memory::wrappers::ParticleBufferOwner::from_guest_address(owner),
                entry_count,
                descriptor_kind,
            )
            .expect("particle buffer allocation");
            0
        },
    );
}

#[test]
fn lifted_0003eca0() {
    let mut directed_case = 0usize;
    pool_drive(
        0x3ECA0,
        true,
        |r, p| {
            let (t, _) = pool_ring(r, p, true);
            let size = match directed_case {
                0 => 0xFFFF_FFC0,
                1 => 0xFFFF_FFF0,
                2 => 0,
                3 => 1,
                4 => 31,
                5 => 32,
                6 => 33,
                7 => 63,
                8 => 64,
                9 => 65,
                // 0x41 rounds to 0x80. The locked allocator accepts a block
                // with exactly the additional 0x20-byte trailing header, and
                // rejects a block one byte shorter.
                10 => {
                    let active = (0..4).fold(0u32, |value, byte| {
                        value | ((p.get(false, 0x6B83CC + byte) as u32) << (byte * 8))
                    });
                    p.put32(active + 4, 0x9F);
                    0x41
                }
                11 => {
                    let active = (0..4).fold(0u32, |value, byte| {
                        value | ((p.get(false, 0x6B83CC + byte) as u32) << (byte * 8))
                    });
                    p.put32(active + 4, 0xA0);
                    0x41
                }
                _ => [0u32, 1, 0x20, 0x41, r.below(t), r.below(0x400), 0xFFFF_FFC0]
                    [r.below(7) as usize],
            };
            directed_case += 1;
            case(r, 0x3ECA0, vec![size])
        },
        |a| {
            let s = arg(a, 0);
            MemoryServices {
                memory: &mut *a.memory,
            }
            .allocate_pool_permanent(s)
            .unwrap()
        },
    );
}

#[test]
fn lifted_0003e9a0_and_0003ea20() {
    pool_drive(
        0x3E9A0,
        false,
        |r, p| {
            let (_, _) = pool_ring(r, p, true);
            p.put32(0x4C81B8, r.below(0x2000));
            case(r, 0x3E9A0, vec![])
        },
        |a| {
            MemoryServices {
                memory: &mut *a.memory,
            }
            .reset_pool()
            .unwrap();
            0
        },
    );
    pool_drive(
        0x3EA20,
        false,
        |r, p| {
            let (_, _) = pool_ring(r, p, true);
            p.put32(0x6B7AA8, [0u32, 1, r.next()][r.below(3) as usize]);
            case(r, 0x3EA20, vec![])
        },
        |a| {
            MemoryServices {
                memory: &mut *a.memory,
            }
            .clear_pool()
            .unwrap();
            0
        },
    );
}

#[test]
fn lifted_0003df70() {
    arena_drive(
        0x3DF70,
        false,
        |r, p| {
            arena_state(r, p);
            p.put32(0x6B7AA8, [0u32, 1, r.next()][r.below(3) as usize]);
            case(r, 0x3DF70, vec![])
        },
        |a| {
            MemoryServices {
                memory: &mut *a.memory,
            }
            .clear_arena_after_teardown()
            .unwrap();
            0
        },
    );
}

#[test]
fn lifted_0003ea00() {
    pool_drive(
        0x3EA00,
        false,
        |r, p| {
            arena_state(r, p);
            p.put32(
                0x4C81B8,
                [0u32, 0x40, 0x100, 0x800, r.below(0x1000)][r.below(5) as usize],
            );
            case(r, 0x3EA00, vec![])
        },
        |a| {
            a.allocate_and_reset().unwrap();
            0
        },
    );
}

#[test]
fn lifted_0003e0b0() {
    let mut case_index = 0usize;
    arena_drive(
        0x3E0B0,
        true,
        |r, p| {
            arena_state(r, p);
            // Exercise the measured power-of-two alignment contract at size-rounding edges
            // before the broad randomized corpus. Invalid alignments remain randomized, but
            // carry no portable guarantee in the REA contract.
            let (align, s, k) = match case_index {
                0 => (0x100, 0, 1),
                1 => (0x100, 1, 1),
                2 => (0x100, 15, 1),
                3 => (0x100, 16, 1),
                4 => (0x100, 17, 1),
                5 => (0x20, 0x3f, 2),
                6 => (0x40, 0x40, 4),
                _ => {
                    let k = r.below(6);
                    let align = [
                        0u32,
                        1,
                        2,
                        4,
                        8,
                        16,
                        0x20,
                        0x40,
                        0x100,
                        r.next(),
                        r.below(0x100),
                    ][r.below(11) as usize];
                    (align, size_arg(r), k)
                }
            };
            case_index += 1;
            case(r, 0x3E0B0, vec![align, s, k])
        },
        |a| {
            let v = &View(a.memory.0.0);
            let (al, s, k) = (rd(v, STACK + 4), rd(v, STACK + 8), rd(v, STACK + 12));
            a.allocate_aligned(al, s, k).unwrap()
        },
    );
}

/// Tier 1 allocator differential for the exact `(alignment,size,kind)` request issued by
/// constructor 0x24A180. The constructor's stable guest-global publication has separate C ABI
/// coverage in tsfp-kmem-capi.
#[test]
fn lifted_0003e0b0_model_buffer_request_24a180() {
    arena_drive(
        0x24A180,
        true,
        |r, p| {
            arena_state(r, p);
            case(r, 0x3E0B0, vec![0x20, 0x640, 1])
        },
        |a| {
            let v = &View(a.memory.0.0);
            let alignment = rd(v, STACK + 4);
            let size = rd(v, STACK + 8);
            let kind = rd(v, STACK + 12);
            a.allocate_aligned(alignment, size, kind).unwrap()
        },
    );
}

/// Tier 1 for the allocator request used by Story objective-counter setup (`0x1620C0`). The
/// wrapper's static REA contract supplies `count * 0x18`; compare that request shape over 3,000
/// seeds against the lifted aligned allocator. The wrapper's definition walk has separate ABI
/// coverage in tsfp-kmem-capi because this function is absent from the retained lifted-root set.
#[test]
fn lifted_001620c0_counter_allocation_shape() {
    arena_drive(
        0x1620C0,
        true,
        |r, p| {
            arena_state(r, p);
            let count = 1 + r.below(64);
            case(r, 0x3E0B0, vec![0x10, count * 0x18, 1])
        },
        |a| {
            let v = &View(a.memory.0.0);
            let (alignment, size, kind) = (rd(v, STACK + 4), rd(v, STACK + 8), rd(v, STACK + 12));
            a.allocate_aligned(alignment, size, kind).unwrap()
        },
    );
}

/// Tier 1 allocator-request evidence for the INFERRED weather constructor at `0x119ED0`.
/// The retained table says it requests 0x900 bytes and a 0xF0-byte row array with 32 or 50 rows;
/// compare both request forms against lifted `0x3E0B0` across 3,000 FABRICATED arena states.
#[test]
fn lifted_00119ed0_weather_allocation_requests() {
    arena_drive(
        0x119ED0,
        true,
        |r, p| {
            arena_state(r, p);
            let size = if r.below(2) == 0 {
                0x900
            } else {
                [0x20, 0x32][r.below(2) as usize] * 0xF0
            };
            case(r, 0x3E0B0, vec![0x10, size, 1])
        },
        |a| {
            a.allocate_aligned(0x10, rd(&View(a.memory.0.0), STACK + 8), 1)
                .unwrap()
        },
    );
}

#[test]
fn lifted_rea_story_record_reader_matches_typed_offset() {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let pair = &mut *guard;
    let mut rng = Rng(0x2369_6998_0);
    for index in 0..3000 {
        let slot = rng.below(16);
        let ordinal = rng.below(13) as u8;
        let aggregate = rng.below(2) == 0;
        let present = rng.below(2) != 0;
        let flags = if aggregate { 0x0080_0000 } else { 0 };
        let profile = 0x007b_edc0u32.wrapping_add(slot.wrapping_mul(0x1f54));
        pair.put32(0x007d_e458, flags);
        pair.put32(profile, u32::from(present));

        let c = W4Case {
            va: 0x69980,
            args: vec![slot, u32::from(ordinal)],
            regs: [0; 7],
            script: vec![],
        };
        let (registers, _) = run_c(pair, &c);
        let summary = if aggregate || !present {
            0x006f_11c0
        } else {
            profile + 0x1290
        };
        let expected = summary + StoryProgress::record_offset(ordinal).unwrap() as u32;
        assert_eq!(registers[0], expected, "reader case {index}");
    }
}

#[test]
fn lifted_rea_story_progress_writer_matches_owned_profile_update() {
    use tsfp_engine_fileio::story_progress::{StoryMode, StoryProgressUpdate};
    use tsfp_format_profile::Profile;

    const PROFILE: u32 = 0x007b_edc0;
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let pair = &mut *guard;
    let mut rng = Rng(0x2369_668f_0);
    for case_index in 0..3000 {
        let ordinal = rng.below(13) as u8;
        let selector = rng.below(3) as u8;
        let time_base = (1 + rng.below(20)) as f32;
        let ticks = 1 + rng.below(9);
        let best_time = time_base * ticks as f32;
        let minimum_time = (1 + rng.below(200)) as f32;
        let old_time = (1 + rng.below(200)) as f32;
        let time = PROFILE + 0x288 + u32::from(ordinal) * 0x28 + 0x0c + u32::from(selector) * 4;
        let single = PROFILE + 0xbf4;
        let counter = PROFILE + 0x918;

        unsafe {
            std::ptr::write_bytes(pair.c.add(PROFILE as usize), 0x5a, 0x1f54);
            std::ptr::write_bytes(pair.rust.add(PROFILE as usize), 0x5a, 0x1f54);
        }
        for (address, value) in [
            (PROFILE, 1),
            (PROFILE + 0x1ee0, 2),
            (0x7de458, 0),
            (0x7de45c, 0x8000),
            (0x7de455, 10),
            (0x7de457, u32::from(selector)),
            (0x79094c, 2),
            (0x790950, 1),
            (0x7de30c, 0),
            (0x790958, 1),
            (0x790954, 1),
            (0x4e8cec, 1),
            (0x475c78, time_base.to_bits()),
            (0x7de328, ticks),
            (0x4785b0, minimum_time.to_bits()),
            (time, old_time.to_bits()),
            (single, rng.next()),
            (single + 4, rng.next()),
            (single + 8, rng.next()),
            (counter, rng.next()),
        ] {
            pair.put32(address, value);
        }

        let mut script = vec![[0u32; 3]; 24];
        script[2][0] = 1; // 1CD040: gate passes.
        script[3][0] = 1; // 64D10: Story writer is enabled.
        script[4][0] = 1; // 3568C0: selected player slot is connected.
        script[5][0] = u32::from(ordinal); // 64D60: campaign ordinal.
        let c = W4Case {
            va: 0x668f0,
            args: vec![],
            regs: [0; 7],
            script,
        };
        let _ = run_c(pair, &c);

        let payload =
            unsafe { std::slice::from_raw_parts(pair.rust.add(PROFILE as usize), 0x1f54).to_vec() };
        let mut profile = Profile {
            payload,
            signature: [0; 20],
        };
        let _receipt = tsfp_engine_fileio::story_progress::StoryProgress::apply_success(
            &mut profile,
            StoryProgressUpdate {
                result_flags: 0x8000,
                campaign_ordinal: ordinal,
                selector,
                mode: StoryMode::SinglePlayer,
                best_time,
                minimum_time,
            },
        )
        .unwrap();
        unsafe {
            std::ptr::copy_nonoverlapping(
                profile.payload.as_ptr(),
                pair.rust.add(PROFILE as usize),
                profile.payload.len(),
            );
        }

        let tier_count = u32::from(selector) + 1;
        for (start, length) in [(time, 4), (single, tier_count * 4), (counter, 4)] {
            for offset in 0..length {
                let address = start + offset;
                assert_eq!(
                    pair.get(false, address),
                    pair.get(true, address),
                    "writer case {case_index} mismatch at {address:#x}"
                );
            }
        }
    }
}

// Keep T2048 allocator differentials registered in this harness beside their shared W4 helpers.
#[path = "w4_ledger.rs"]
mod ledger;

// T2409: launch-argument boundary (macros above must precede the module).
#[path = "w4_launch.rs"]
mod launch;
