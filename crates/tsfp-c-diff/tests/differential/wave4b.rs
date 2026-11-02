// SPDX-License-Identifier: GPL-3.0-or-later
//! T1848 wave 4b: the RC4 key setup, base64 encode, the three stdcall roots and the call-bearing
//! network/input roots, each a random differential Rust port == unmodified C replacement.
//! Nested guest calls land in a stub on both sides (C: `recomp_lookup` in c/shim.c, Rust: [`Stub`])
//! that returns the same scripted EAX/ECX/EDX and records an ordered call trace (target, callee
//! ESP, return PC, stack words, all registers at the call). Compared per case: all seven GPRs,
//! ESP (entry + 4 + 4 * popped words), the call trace, the guest stack frame around ESP and
//! every byte the builder or the root touches. See docs/t1848-rust-wave4.md.
use super::*;

unsafe extern "C" {
    fn shim_run(va: u32, regs: *mut u32) -> i32;
    fn shim_script(words: *const u32, entries: u32);
    fn shim_script_writes(words: *const u32, entries: u32);
    fn shim_callee_argc(va: u32) -> i32;
    fn shim_trace_len() -> u32;
    fn shim_trace_entry(index: u32) -> *const u32;
}

const POOL_A: u32 = 0x0040_0000;
const POOL_B: u32 = 0x0050_0000;
const FRAME: u32 = STACK - 0x80;
const FRAME_LEN: u32 = 0xc0;

/// Number of stack arguments of a stubbed guest callee. Single source of truth is
/// `callee_argc_or_neg` in c/shim.c (T1815 w8), queried over FFI so the two sides cannot diverge.
fn callee_argc(target: u32) -> u32 {
    let argc = unsafe { shim_callee_argc(target) };
    assert!(argc >= 0, "unknown callee {target:x}");
    argc as u32
}

/// Rust side of the guest-callee stub: the scripted results and the recorded trace.
struct Stub {
    script: Vec<[u32; 3]>,
    /// Seeded callee memory writes {call index, address, value} (T1815 w8), applied by closure.rs.
    writes: Vec<[u32; 3]>,
    trace: Vec<[u32; 16]>,
}
impl Stub {
    fn step(&mut self, m: &View, target: u32, esp: u32, regs: [u32; 7]) -> [u32; 3] {
        let index = self.trace.len();
        let argc = callee_argc(target);
        let mut entry = [0u32; 16];
        entry[0] = target;
        entry[1] = esp;
        entry[2] = m.read32w(esp);
        for a in 0..3 {
            entry[3 + a as usize] = if a < argc { m.read32w(esp.wrapping_add(4 + 4 * a)) } else { 0 };
        }
        entry[6..13].copy_from_slice(&regs);
        self.trace.push(entry);
        self.script.get(index).copied().unwrap_or([0; 3])
    }
}
impl View {
    fn read32w(&self, address: u32) -> u32 {
        u32::from_le_bytes(std::array::from_fn(|i| self.byte(address.wrapping_add(i as u32))))
    }
}
macro_rules! stub_for {
    ($krate:ident) => {
        impl $krate::callee::Callee<View> for Stub {
            fn call(&mut self, m: &mut View, target: u32, cpu: &mut $krate::callee::Cpu) {
                let regs = [cpu.eax, cpu.ecx, cpu.edx, cpu.ebx, cpu.esi, cpu.edi, cpu.ebp];
                let [eax, ecx, edx] = self.step(m, target, cpu.esp, regs);
                cpu.eax = eax;
                cpu.ecx = ecx;
                cpu.edx = edx;
                cpu.esp = cpu.esp.wrapping_add(0); // RET pops its slot, guest_call restores ESP
            }
        }
    };
}
stub_for!(tsfp_network);
stub_for!(tsfp_engine_input);

struct Case2 {
    va: u32,
    args: Vec<u32>,
    /// eax, ecx, edx, ebx, esi, edi, ebp at entry.
    regs: [u32; 7],
    script: Vec<[u32; 3]>,
    /// Callee memory writes {call index, address, value}, performed identically by C and Rust.
    writes: Vec<[u32; 3]>,
    watch: Vec<(u32, u32)>,
    /// Stack words the stdcall root pops (0 for cdecl).
    pop: u32,
}

fn mk2(rng: &mut Rng, va: u32, args: Vec<u32>, pop: u32) -> Case2 {
    Case2 {
        va,
        args,
        regs: std::array::from_fn(|_| rng.next()),
        script: vec![],
        writes: vec![],
        watch: vec![],
        pop,
    }
}

fn script_of(rng: &mut Rng, len: usize, special: &[u32]) -> Vec<[u32; 3]> {
    (0..len).map(|_| [rng.word(special), rng.next(), rng.next()]).collect()
}

fn maybe(rng: &mut Rng, special: &[u32]) -> u32 {
    if rng.below(3) == 0 { 0 } else { rng.word(special) }
}

/// One case for `drive2`: builds, runs C and the port, compares. With `mutant` every case
/// perturbs the Rust result by one of six kinds and the count of detections is returned.
fn drive2(
    seed: u64,
    mutant: bool,
    mut build: impl FnMut(&mut Rng, &mut Pair) -> Case2,
    port: impl Fn(&mut View, &mut Stub, &Case2) -> [u32; 8],
) -> (usize, usize) {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let pair = &mut *guard;
    let mut rng = Rng(seed);
    let (mut detected, mut total) = (0, 0);
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
        let mut regs = [case.regs[0], case.regs[1], case.regs[2], case.regs[3], case.regs[4], case.regs[5], case.regs[6], STACK];
        assert_eq!(unsafe { shim_run(case.va, regs.as_mut_ptr()) }, 1, "unknown root");
        let c_trace: Vec<[u32; 16]> = (0..unsafe { shim_trace_len() })
            .map(|i| unsafe { *(shim_trace_entry(i) as *const [u32; 16]) })
            .collect();
        let mut view = View(pair.rust);
        let mut stub = Stub { script: case.script.clone(), writes: vec![], trace: vec![] };
        let mut expected = port(&mut view, &mut stub, &case);
        expected[7] = expected[7].wrapping_add(4 + 4 * case.pop);
        if mutant {
            match index % 6 {
                0 => expected[0] ^= 1,
                1 => expected[1] ^= 1,
                2 => expected[2] ^= 1,
                3 => {
                    let byte = view.byte(FRAME);
                    view.set(FRAME, byte ^ 0x80);
                }
                4 => expected[4] ^= 1,
                _ => match stub.trace.first_mut() {
                    Some(entry) => entry[2] ^= 1,
                    None => expected[6] ^= 1,
                },
            }
        }
        let mut mismatch = lifted_policy::mismatch(case.va, lifted_policy::diff8(&regs, &expected)) || lifted_policy::trace(case.va, &c_trace, &stub.trace);
        for (start, len) in &watch {
            for offset in 0..*len {
                let address = start.wrapping_add(offset);
                mismatch |= lifted_policy::mem(case.va, address, pair.get(false, address) != pair.get(true, address));
            }
        }
        if mutant {
            total += 1;
            detected += usize::from(mismatch);
            for (start, len) in &watch {
                for offset in 0..*len {
                    let address = start.wrapping_add(offset);
                    let byte = pair.get(false, address);
                    unsafe { *pair.rust.add(address as usize) = byte };
                }
            }
        } else {
            assert!(
                !mismatch,
                "root {:08X} case {index}: C regs {regs:x?} trace {c_trace:x?}, Rust {expected:x?} trace {:x?}, args {:x?}",
                case.va, stub.trace, case.args
            );
        }
    }
    (detected, total)
}

macro_rules! suite2 {
    ($name:ident, $mutant:ident, $seed:expr, $build:expr, $port:expr) => {
        #[test]
        fn $name() {
            drive2($seed, false, $build, $port);
        }
        #[test]
        fn $mutant() {
            let (detected, total) = drive2($seed, true, $build, $port);
            eprintln!("mutant control {}: {detected}/{total} perturbations detected", stringify!($name));
            assert!(total > 0);
            assert_eq!(detected, total);
        }
    };
}

macro_rules! regs_of {
    ($cpu:expr) => {
        [$cpu.eax, $cpu.ecx, $cpu.edx, $cpu.ebx, $cpu.esi, $cpu.edi, $cpu.ebp, $cpu.esp]
    };
}
macro_rules! cpu_of {
    ($krate:ident, $case:expr) => {
        $krate::callee::Cpu {
            eax: $case.regs[0],
            ecx: $case.regs[1],
            edx: $case.regs[2],
            ebx: $case.regs[3],
            esi: $case.regs[4],
            edi: $case.regs[5],
            ebp: $case.regs[6],
            esp: STACK,
        }
    };
}
/// Port with a callee: `f(stub, memory, cpu)`.
macro_rules! with_callee {
    ($krate:ident, $f:path) => {
        |v: &mut View, st: &mut Stub, c: &Case2| {
            let mut cpu = cpu_of!($krate, c);
            $f(st, v, &mut cpu);
            regs_of!(cpu)
        }
    };
}
/// Port without a callee that only reads memory: `f(&memory, cpu)`.
macro_rules! read_only {
    ($krate:ident, $f:path) => {
        |v: &mut View, _: &mut Stub, c: &Case2| {
            let mut cpu = cpu_of!($krate, c);
            $f(&*v, &mut cpu);
            regs_of!(cpu)
        }
    };
}
/// Port without a callee that writes memory: `f(memory, cpu)`.
macro_rules! no_callee {
    ($krate:ident, $f:path) => {
        |v: &mut View, _: &mut Stub, c: &Case2| {
            let mut cpu = cpu_of!($krate, c);
            $f(v, &mut cpu);
            regs_of!(cpu)
        }
    };
}

// ---- RC4 key setup and base64 ----

fn build_rc4(r: &mut Rng, p: &mut Pair) -> Case2 {
    let state = r.pool(POOL_A);
    for offset in 0..0x104u32 {
        p.put8(state + offset, r.next() as u8);
    }
    let alias = r.below(8) == 0;
    let key = if alias { state + r.below(0x100) } else { r.pool(POOL_B) };
    if !alias {
        for offset in 0..0x60u32 {
            p.put8(key + offset, r.next() as u8);
        }
    }
    let length = match r.below(12) {
        0 => 0,
        1 => u32::MAX - r.below(3),
        2 => 0x8000_0000 + r.below(3),
        3 => 1 + r.below(0x7000_0000),
        _ => 1 + r.below(40),
    };
    let passes = match r.below(10) {
        0 => 0,
        1 => u32::MAX,
        2 => 0x8000_0000,
        _ => 1 + r.below(3),
    };
    let mut case = mk2(r, 0x3c6730, vec![state, key, length, passes], 0);
    case.watch.push((state, 0x104));
    case.watch.push((key, 0x60));
    case
}
suite2!(
    r_003c6730,
    mutant_003c6730,
    0x1848_0101,
    build_rc4,
    no_callee!(tsfp_network, tsfp_network::fesl::rc4_key_setup)
);

fn build_base64(r: &mut Rng, p: &mut Pair) -> Case2 {
    for index in 0..64u32 {
        p.put8(0x004b_5de8 + index, r.next() as u8);
    }
    let source = r.pool(POOL_B);
    for offset in 0..0x30u32 {
        p.put8(source + offset, r.next() as u8);
    }
    let count = match r.below(10) {
        0 => u32::MAX - r.below(3),
        1 => r.below(3),
        _ => r.below(37),
    };
    let destination = if r.below(8) == 0 { source.wrapping_add(r.below(10)).wrapping_sub(2) } else { r.pool(POOL_A) };
    let mut case = mk2(r, 0x3ba270, vec![count, source, destination], 0);
    case.watch.push((destination, 0x40));
    case.watch.push((source, 0x30));
    case
}
suite2!(
    r_003ba270,
    mutant_003ba270,
    0x1848_0102,
    build_base64,
    no_callee!(tsfp_network, tsfp_network::fesl::base64_encode)
);

// ---- stdcall roots ----

suite2!(
    r_003b5fe0,
    mutant_003b5fe0,
    0x1848_0103,
    |r, p| {
        let object = r.pool(POOL_A);
        p.put32(object + 0x28, r.next());
        let flag = match r.below(4) {
            0 => 0,
            1 => 0x100,
            2 => 0xff00_0000,
            _ => r.next(),
        };
        let spare = r.next();
        mk2(r, 0x3b5fe0, vec![object, spare, flag], 3)
    },
    read_only!(tsfp_network, tsfp_network::calls::state_003b5fe0)
);

fn build_find_record(r: &mut Rng, p: &mut Pair) -> Case2 {
    let this = if r.below(16) == 0 { 0xffff_fc00 + r.below(0x400) } else { r.pool(POOL_A) };
    let specials = [0u32, 1, 7, 0x20];
    let (a, b) = (r.word(&specials), r.word(&specials));
    for record in 0..5u32 {
        let base = this.wrapping_add(0x60 + 0xac * record);
        p.put32(base.wrapping_add(8), maybe(r, &[1]));
        p.put32(base.wrapping_add(0x24), if r.below(2) == 0 { a } else { r.word(&specials) });
        p.put32(base.wrapping_add(0x20), if r.below(2) == 0 { b } else { r.word(&specials) });
    }
    let mut case = mk2(r, 0x3b3120, vec![a, b], 2);
    case.regs[1] = this;
    case
}
suite2!(
    r_003b3120,
    mutant_003b3120,
    0x1848_0104,
    build_find_record,
    no_callee!(tsfp_network, tsfp_network::calls::find_record_by_key_pair)
);

fn build_header(r: &mut Rng, p: &mut Pair) -> Case2 {
    let object = r.pool(POOL_A);
    p.put32(object + 8, if r.below(4) == 0 { object } else { r.pool(POOL_B) });
    let text = match r.below(6) {
        0 => object + r.below(0x14),
        1 => object + 0x10 + r.below(8),
        _ => r.pool(POOL_B),
    };
    let alias = text >= object && text < object + 0x20;
    if !alias {
        for offset in 0..24u32 {
            p.put8(text + offset, 1 + r.below(255) as u8);
        }
        let length = r.below(22);
        p.put8(text + length, 0);
    } else {
        for offset in 0x14..0x20u32 {
            p.put8(object + offset, if r.below(5) == 0 { 0 } else { 1 + r.below(255) as u8 });
        }
    }
    let (first, second) = (r.next(), r.next());
    let mut case = mk2(r, 0x33db0, vec![object, first, second, text], 4);
    case.script = script_of(r, 1, &[0, 1]);
    case.watch.push((object, 0x20));
    case
}
suite2!(
    r_00033db0,
    mutant_00033db0,
    0x1848_0105,
    build_header,
    with_callee!(tsfp_network, tsfp_network::calls::header_store_magic_and_copy_text)
);

// ---- call-bearing roots ----

fn fill(r: &mut Rng, p: &mut Pair, address: u32, special: &[u32]) {
    p.put32(address, maybe(r, special));
}

macro_rules! scripted {
    ($name:ident, $mutant:ident, $seed:expr, $va:expr, $pop:expr, $entries:expr, $special:expr, $setup:expr, $port:expr) => {
        suite2!(
            $name,
            $mutant,
            $seed,
            |r, p| {
                #[allow(clippy::redundant_closure_call)]
                let args: Vec<u32> = ($setup)(r, p);
                let mut case = mk2(r, $va, args, 0);
                case.pop = $pop;
                case.script = script_of(r, $entries, &$special);
                case
            },
            $port
        );
    };
}

scripted!(
    r_0035e630, mutant_0035e630, 0x1848_0106, 0x35e630, 0, 1, [0u32, 1, 2],
    |r: &mut Rng, p: &mut Pair| { for a in [0x76bbc0, 0x76bbc4, 0x76bbbc] { fill(r, p, a, &[1, u32::MAX]); } vec![] },
    with_callee!(tsfp_network, tsfp_network::calls::net_total_slot_count)
);
scripted!(
    r_00356320, mutant_00356320, 0x1848_0107, 0x356320, 0, 2, [0u32, 1, 15, 16, 17, 40, u32::MAX],
    |_: &mut Rng, _: &mut Pair| vec![],
    with_callee!(tsfp_network, tsfp_network::calls::net_count_free_player_slots)
);
scripted!(
    r_003568c0, mutant_003568c0, 0x1848_0108, 0x3568c0, 0, 1, [0u32, 1, 2],
    |r: &mut Rng, p: &mut Pair| {
        let slot = match r.below(6) { 0 => u32::MAX - r.below(3), 1 => 16 + r.below(3), _ => r.below(16) };
        p.put32(slot.wrapping_mul(0x124).wrapping_add(0x0077_728c), r.next());
        p.put32(0x0079_094c, if r.below(3) == 0 { 0x65 } else { r.next() });
        let base = if r.below(4) == 0 { 0 } else { r.pool(POOL_A) };
        p.put32(0x0073_56d8, base);
        p.put32(slot.wrapping_mul(0xea0).wrapping_add(base), maybe(r, &[1]));
        p.put32(0x007d_e30c, r.next());
        vec![slot]
    },
    with_callee!(tsfp_network, tsfp_network::calls::net_is_player_slot_connected)
);
scripted!(
    r_0035e6a0, mutant_0035e6a0, 0x1848_0109, 0x35e6a0, 0, 1, [0u32, 1],
    |r: &mut Rng, p: &mut Pair| {
        for a in [0x76bbbc, 0x76bbc4, 0x76bbc0] { fill(r, p, a, &[1]); }
        vec![r.next(), r.next(), r.word(&[0, 1, u32::MAX, 0x8000_0000])]
    },
    with_callee!(tsfp_network, tsfp_network::calls::net_set_slot_counts)
);
scripted!(
    r_0035eaf0, mutant_0035eaf0, 0x1848_010a, 0x35eaf0, 0, 1, [0u32, 1],
    |r: &mut Rng, p: &mut Pair| {
        fill(r, p, 0x76bd6c, &[1]);
        p.put32(0x76bd68, r.word(&[0xfd, 0xfe, 0xff]));
        vec![]
    },
    with_callee!(tsfp_network, tsfp_network::calls::net_advance_session_key)
);
scripted!(
    r_00356990, mutant_00356990, 0x1848_010b, 0x356990, 0, 1, [0u32, 1],
    |r: &mut Rng, p: &mut Pair| {
        p.put32(0x79_094c, if r.below(2) == 0 { 0x65 } else { r.next() });
        p.put32(0x76_b140, if r.below(2) == 0 { 2 } else { r.next() });
        p.put32(0x76_b1a4, if r.below(2) == 0 { 0xa } else { r.next() });
        vec![]
    },
    with_callee!(tsfp_network, tsfp_network::calls::net_is_role_2_in_state_0xa)
);
scripted!(
    r_00358130, mutant_00358130, 0x1848_010c, 0x358130, 0, 2, [0u32, 1],
    |r: &mut Rng, p: &mut Pair| { p.put32(0x54_1e9c, r.next()); vec![] },
    with_callee!(tsfp_network, tsfp_network::calls::net_take_pickup_flag)
);
scripted!(
    r_00358160, mutant_00358160, 0x1848_010d, 0x358160, 0, 2, [0u32, 1],
    |r: &mut Rng, p: &mut Pair| {
        p.put32(0x7d_e30c, r.word(&[6, 7, 8]));
        p.put32(0x54_1ea4, r.next());
        p.put32(0x51_9750, r.word(&[u32::MAX]));
        vec![]
    },
    with_callee!(tsfp_network, tsfp_network::calls::net_next_pickup_sequence_id)
);

fn build_session_table(r: &mut Rng, p: &mut Pair) -> (u32, u32) {
    let table = if r.below(5) == 0 { 0 } else { r.pool(POOL_A) };
    p.put32(0x0077_4028, table);
    let count = match r.below(6) { 0 => 0, 1 => u32::MAX, 2 => 0x8000_0000, _ => r.below(7) };
    p.put32(table.wrapping_add(0x20), count);
    let key = r.below(7);
    for slot in 0..8u32 {
        let record = r.pool(POOL_B);
        p.put32(table.wrapping_add(0x0c + 4 * slot), if r.below(6) == 0 { 0 } else { record });
        p.put32(record + 0x3c, if r.below(3) == 0 { key } else { r.below(7) });
    }
    (table, key)
}
scripted!(
    r_00356080, mutant_00356080, 0x1848_010e, 0x356080, 0, 1, [0u32, 1],
    |r: &mut Rng, p: &mut Pair| { let (_, key) = build_session_table(r, p); vec![key] },
    with_callee!(tsfp_network, tsfp_network::calls::net_player_index_in_session)
);
scripted!(
    r_00356870, mutant_00356870, 0x1848_010f, 0x356870, 0, 3, [0u32, 1],
    |r: &mut Rng, p: &mut Pair| { build_session_table(r, p); vec![r.below(9)] },
    with_callee!(tsfp_network, tsfp_network::calls::net_player_slot_field_3c)
);
scripted!(
    r_00358560, mutant_00358560, 0x1848_0110, 0x358560, 0, 2, [0u32, 1],
    |r: &mut Rng, p: &mut Pair| {
        let player = r.pool(POOL_A);
        let object = r.pool(POOL_B);
        p.put32(player + 0x7c, object);
        let base = r.word(&[5, 100]);
        p.put32(0x79_0950, base);
        p.put32(object + 8, base.wrapping_add(r.word(&[0, u32::MAX, 1])));
        vec![player]
    },
    with_callee!(tsfp_network, tsfp_network::calls::net_object_replicates_locally)
);
scripted!(
    r_003560e0, mutant_003560e0, 0x1848_0111, 0x3560e0, 0, 1, [0u32, 1],
    |r: &mut Rng, p: &mut Pair| {
        let index = if r.below(8) == 0 { r.next() } else { r.below(16) };
        let at = index.wrapping_mul(0x124);
        for offset in [0x77_727cu32, 0x77_7284, 0x77_728c, 0x77_7244, 0x77_7360, 0x77_7348] {
            p.put32(at.wrapping_add(offset), r.next());
        }
        vec![index, r.next()]
    },
    with_callee!(tsfp_network, tsfp_network::calls::net_record_124_init_at_index)
);

fn build_min_word(r: &mut Rng, p: &mut Pair) -> Vec<u32> {
    let pointer = r.pool(POOL_A);
    let key = r.below(4);
    p.put32(pointer, key);
    let base = r.pool(POOL_B);
    p.put32(0x0073_56d8, base);
    for slot in 0..16u32 {
        let owner = r.pool(POOL_A) + 0x2000 + 4 * slot;
        p.put32(0x0077_7244 + 0x124 * slot, owner);
        p.put32(owner, if r.below(2) == 0 { key } else { r.below(4) });
        p.put32(base.wrapping_add(slot * 0xea0 + 0xa60), r.word(&[0x7fff, 0x8000, 0xffff, 0x7ffe_8000]));
    }
    vec![pointer]
}
scripted!(
    r_00359910, mutant_00359910, 0x1848_0112, 0x359910, 0, 16, [0u32, 1, 1],
    build_min_word,
    with_callee!(tsfp_network, tsfp_network::calls::net_min_word_over_connected_slots)
);

fn build_release(r: &mut Rng, p: &mut Pair) -> Vec<u32> {
    let count = r.below(5);
    let extra = r.below(6).wrapping_sub(3);
    p.put32(0x007a_2958, count);
    p.put32(0x0079_0950, extra);
    p.put8(0x007d_e455, if r.below(2) == 0 { 0x0a } else { r.next() as u8 });
    let base = r.pool(POOL_A);
    p.put32(0x0073_56d8, base);
    for record in 0..6u32 {
        let at = base + record * 0xea0;
        p.put8(at + 0x2c, r.next() as u8);
        for offset in [0u32, 8, 0x10, 0x14, 0x18, 0x1c] {
            p.put32(at + offset, maybe(r, &[1, 2, u32::MAX]));
        }
    }
    vec![]
}
scripted!(
    r_0015c4e0, mutant_0015c4e0, 0x1848_0113, 0x15c4e0, 0, 6, [0u32, 1],
    build_release,
    with_callee!(tsfp_network, tsfp_network::calls::net_slot_release_each_active)
);

fn build_object_flag(r: &mut Rng, p: &mut Pair) -> Case2 {
    let object = if r.below(5) == 0 { 0 } else { r.pool(POOL_A) };
    let record = r.pool(POOL_B);
    p.put32(object.wrapping_add(0x7c), record);
    p.put32(record + 0x14, r.word(&[0x1201, 1, 0x200, 0x1000, 0x4000_0000]));
    p.put32(record + 0x28, r.next());
    let (arg, ecx, edx) = (r.next(), r.next(), r.next());
    let mut case = mk2(r, 0x235600, vec![arg], 0);
    case.script = vec![[object, ecx, edx]];
    case.watch.push((record + 0x28, 4));
    case
}
suite2!(
    r_00235600,
    mutant_00235600,
    0x1848_0114,
    build_object_flag,
    with_callee!(tsfp_network, tsfp_network::calls::net_slot_object_set_flag)
);

scripted!(
    r_002d74b0, mutant_002d74b0, 0x1848_0115, 0x2d74b0, 0, 8, [0u32, 0, 1, 2, 4, 0x80, u32::MAX],
    |r: &mut Rng, _: &mut Pair| vec![r.word(&[1, 2, 4, 0x80])],
    with_callee!(tsfp_engine_input, tsfp_engine_input::calls::any_pad_has_button_mask)
);

// T1815 closure model (docs/t1815-closure-model.md).
#[path = "closure.rs"]
mod closure;
