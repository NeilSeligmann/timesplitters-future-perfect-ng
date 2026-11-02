// SPDX-License-Identifier: GPL-3.0-or-later
//! T3742 runner tests on a SYNTHETIC corpus (no lifted tree needed): small reference functions
//! play the lifted original, Rust twins play the port, and seeded mutations of the port, the
//! capture and the callee list must each come out as the right verdict class.
mod common;

use common::*;
use tsfp_port_runtime::abi::{ExtRegisters, GuestCall};
use tsfp_port_runtime::memory::GuestMemory;
use tsfp_t2_offline::arena::{Arena, GuestView};
use tsfp_t2_offline::digest::input_digest;
use tsfp_t2_offline::spool::{self, Entry, encode_record, encode_spool};
use tsfp_t2_offline::stub::{CalleeStub, Convention};
use tsfp_t2_offline::{EntryResult, Executor, Options, Verdict, run_entry, run_spool};

const VA_INC: u32 = 0x1000;
const VA_CALL: u32 = 0x2000;
const VA_HALF: u32 = 0x3000;
const CALLEE: u32 = 0x5000;
const DST: u32 = 0x0040_0000;
const SRC: u32 = 0x0041_0000;
const GLOBAL: u32 = 0x0060_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mutation {
    None,
    WrongValue,
    WrongEax,
    ExtraRead,
    HostFault,
    Panic,
    WrongCallee,
    SkipCall,
    WrongArg,
    UlpOff,
}

fn view(base: usize) -> GuestView {
    unsafe { GuestView::new(base) }
}

fn push(m: &mut GuestView, r: &mut ExtRegisters, value: u32) {
    r.esp -= 4;
    m.write32(r.esp, value);
}

/// Reference ("lifted") bodies. Nested calls: the caller pushes the return PC (lifted style).
fn reference(
    va: u32,
    base: usize,
    r: &mut ExtRegisters,
    call: &mut dyn FnMut(u32, &mut ExtRegisters, Convention),
) {
    let mut m = view(base);
    match va {
        VA_INC => {
            push(&mut m, r, r.ebx); // dead stack below the entry ESP
            r.ebx = m.read32(r.esp + 12);
            let value = m.read32(r.ebx).wrapping_add(1);
            m.write32(m.read32(r.esp + 8), value);
            r.eax = value;
            r.ebx = m.read32(r.esp);
            r.esp += 4;
        }
        VA_CALL => {
            let object = m.read32(r.esp + 4);
            push(&mut m, r, object);
            push(&mut m, r, 0x2010);
            call(CALLEE, r, Convention::ReturnPushed);
            r.esp += 4;
            let value = m.read32(object) | 2;
            m.write32(object, value);
            r.eax = value;
        }
        VA_HALF => {
            let source = f32::from_bits(m.read32(m.read32(r.esp + 8)));
            m.write32(m.read32(r.esp + 4), (source * 0.5).to_bits());
        }
        _ => unreachable!(),
    }
    r.esp += 4; // ret
}

/// Rust twins with seeded mutations. ESP comes back as after the host wrapper's `ret`.
fn port(va: u32, base: usize, r: &mut ExtRegisters, guest: &mut dyn GuestCall, mutation: Mutation) {
    let mut m = view(base);
    match va {
        VA_INC => {
            push(&mut m, r, 0xdead_beef); // different dead-stack scratch: must not matter
            let dst = m.read32(r.esp + 8);
            let src = m.read32(r.esp + 12);
            r.esp += 4;
            let value = m.read32(src).wrapping_add(1);
            let mut stored = value;
            match mutation {
                Mutation::WrongValue => stored = value.wrapping_add(1),
                Mutation::ExtraRead => {
                    let _ = m.read32(GLOBAL);
                }
                Mutation::HostFault => unsafe {
                    (8usize as *mut u32).write_volatile(1);
                },
                Mutation::Panic => panic!("port bug"),
                _ => {}
            }
            m.write32(dst, stored);
            r.eax = if mutation == Mutation::WrongEax {
                value ^ 1
            } else {
                value
            };
        }
        VA_CALL => {
            let object = m.read32(r.esp + 4);
            let argument = if mutation == Mutation::WrongArg {
                object + 4
            } else {
                object
            };
            push(&mut m, r, argument);
            match mutation {
                Mutation::SkipCall => {}
                Mutation::WrongCallee => guest.call(CALLEE + 4, r),
                _ => guest.call(CALLEE, r),
            }
            r.esp += 4;
            let value = m.read32(object) | 2;
            m.write32(object, value);
            r.eax = value;
        }
        VA_HALF => {
            let source = f32::from_bits(m.read32(m.read32(r.esp + 8)));
            let mut bits = (source * 0.5).to_bits();
            if mutation == Mutation::UlpOff {
                bits += 1;
            }
            m.write32(m.read32(r.esp + 4), bits);
        }
        _ => unreachable!(),
    }
    r.esp += 4;
}

struct Fake {
    mutation: Mutation,
    base: usize,
    has_port: bool,
}

impl Fake {
    fn new(mutation: Mutation) -> Self {
        Self {
            mutation,
            base: 0,
            has_port: true,
        }
    }
}

impl Executor for Fake {
    fn bind(&mut self, base: usize) {
        self.base = base;
    }
    fn has_port(&self, _va: u32) -> bool {
        self.has_port
    }
    fn lifted_stubs_callees(&self) -> bool {
        true
    }
    fn run_lifted(&mut self, va: u32, regs: &mut ExtRegisters, stub: &mut CalleeStub<'_>) {
        reference(va, self.base, regs, &mut |callee, r, conv| {
            stub.dispatch(callee, r, conv)
        });
    }
    fn run_port(&mut self, va: u32, regs: &mut ExtRegisters, stub: &mut CalleeStub<'_>) {
        port(va, self.base, regs, stub, self.mutation);
    }
}

fn entry_inc(arena: &mut Arena, src_value: u32) -> Entry {
    let reads = vec![words(ESP, &[RETURN_PC, DST, SRC]), words(SRC, &[src_value])];
    capture(
        arena,
        VA_INC,
        regs(ESP),
        reads,
        vec![],
        &mut |base, r, rec| reference(VA_INC, base, r, &mut |va, r, conv| rec.call(va, r, conv)),
    )
}

fn entry_call(arena: &mut Arena) -> Entry {
    let reads = vec![words(ESP, &[RETURN_PC, DST])];
    let outcome = Outcome {
        eax: 7,
        writes: vec![words(DST, &[0x100])], // the caller reads this back (subtree-defined)
    };
    capture(
        arena,
        VA_CALL,
        regs(ESP),
        reads,
        vec![outcome],
        &mut |base, r, rec| reference(VA_CALL, base, r, &mut |va, r, conv| rec.call(va, r, conv)),
    )
}

fn entry_half(arena: &mut Arena, value: f32) -> Entry {
    let reads = vec![
        words(ESP, &[RETURN_PC, DST, SRC]),
        words(SRC, &[value.to_bits()]),
    ];
    capture(
        arena,
        VA_HALF,
        regs(ESP),
        reads,
        vec![],
        &mut |base, r, rec| reference(VA_HALF, base, r, &mut |va, r, conv| rec.call(va, r, conv)),
    )
}

fn judge(entry: &Entry, mutation: Mutation) -> EntryResult {
    let mut arena = arena();
    let mut fake = Fake::new(mutation);
    fake.bind(arena.base());
    run_entry(
        &mut fake,
        &mut arena,
        0,
        entry,
        entry.va,
        Options::default(),
    )
}

fn class(verdict: &Verdict) -> &'static str {
    match verdict {
        Verdict::Judged(j) => j.class.name(),
        Verdict::NoAbiEntry => "no_abi_entry",
        Verdict::Skipped => "skipped",
        Verdict::NotRun => "null",
    }
}

#[test]
fn faithful_entries_with_a_correct_port_are_equal() {
    let mut arena = arena();
    let entries = [
        entry_inc(&mut arena, 41),
        entry_call(&mut arena),
        entry_half(&mut arena, 3.0),
    ];
    // The captures are real: the increment wrote 42 to DST, the dead-stack push is recorded.
    assert!(
        entries[0]
            .write_runs
            .iter()
            .any(|w| w.addr == DST && w.data == 42u32.to_le_bytes())
    );
    assert!(entries[0].write_runs.iter().any(|w| w.addr < ESP));
    assert_eq!(entries[1].callees.len(), 1);
    assert_eq!(entries[1].exit_regs.eax, 0x102);
    for entry in &entries {
        let result = judge(entry, Mutation::None);
        assert_eq!(
            (class(&result.lifted), class(&result.rust)),
            ("equal", "equal"),
            "{result:?}"
        );
    }
    let result = judge(&entries[0], Mutation::None);
    assert_eq!(result.dead_stack_writes, 4);
    assert_eq!(result.extra_read, 0);
}

#[test]
fn port_mutations_are_diverged_with_the_right_class() {
    let mut arena = arena();
    let inc = entry_inc(&mut arena, 41);
    let call = entry_call(&mut arena);
    let half = entry_half(&mut arena, 3.0);
    let cases = [
        (&inc, Mutation::WrongValue, "write_set", "write@0x00400000"),
        (&inc, Mutation::WrongEax, "exit_regs", "eax"),
        (&inc, Mutation::HostFault, "fault", "signal 11"),
        (&inc, Mutation::Panic, "fault", "panic: port bug"),
        (
            &call,
            Mutation::WrongCallee,
            "callee_mismatch",
            "callee_mismatch@0: expected 0x5000, got 0x5004",
        ),
        (
            &call,
            Mutation::SkipCall,
            "callee_mismatch",
            "callee_mismatch@0: missing call to 0x5000",
        ),
        (&call, Mutation::WrongArg, "callee_mismatch", "arg 0"),
        (&half, Mutation::UlpOff, "fp_ulp", "fp_ulp write@0x00400000"),
    ];
    for (entry, mutation, want, detail) in cases {
        let result = judge(entry, mutation);
        assert_eq!(class(&result.lifted), "equal", "{mutation:?}");
        assert_eq!(class(&result.rust), want, "{mutation:?}: {result:?}");
        assert!(
            result.detail.contains(detail),
            "{mutation:?}: {}",
            result.detail
        );
    }
}

#[test]
fn an_extra_read_is_a_warning_not_a_divergence() {
    let mut arena = arena();
    let result = judge(&entry_inc(&mut arena, 1), Mutation::ExtraRead);
    assert_eq!(class(&result.rust), "equal");
    assert_eq!(result.extra_read, 1);
}

#[test]
fn a_tampered_capture_fails_the_lifted_fidelity_control() {
    let mut arena = arena();
    let mut wrong_write = entry_inc(&mut arena, 41);
    let run = wrong_write
        .write_runs
        .iter_mut()
        .find(|w| w.addr == DST)
        .unwrap();
    run.data[0] ^= 0x10;
    let result = judge(&wrong_write, Mutation::None);
    assert_eq!(class(&result.lifted), "write_set");
    assert!(
        result.detail.starts_with("lifted: write@0x00400000"),
        "{}",
        result.detail
    );

    let mut wrong_exit = entry_inc(&mut arena, 41);
    wrong_exit.exit_regs.esi ^= 1;
    assert_eq!(
        class(&judge(&wrong_exit, Mutation::None).lifted),
        "exit_regs"
    );

    let mut lost_callee = entry_call(&mut arena);
    lost_callee.callees.clear();
    let result = judge(&lost_callee, Mutation::None);
    assert_eq!(class(&result.lifted), "callee_mismatch");
    assert_eq!(class(&result.rust), "callee_mismatch");

    // A write the port makes where the original wrote nothing is caught too.
    let mut missing_write = entry_inc(&mut arena, 41);
    missing_write.write_runs.retain(|w| w.addr != DST);
    assert_eq!(
        class(&judge(&missing_write, Mutation::None).lifted),
        "write_set"
    );
}

#[test]
fn callee_argument_words_outside_the_known_frame_are_not_compared() {
    // The capture records 16 stack words per callee; those above the entry ESP that the function
    // never read or wrote are its caller's frame (here: never captured, so 0xCD in the arena).
    let mut arena = arena();
    let mut entry = entry_call(&mut arena);
    let args = &mut entry.callees[0].args;
    assert_eq!(args[0], DST);
    args[5] = 0x1234_5678; // the word at entry ESP + 8: caller frame, unknowable
    assert_eq!(class(&judge(&entry, Mutation::None).rust), "equal");
    entry.callees[0].args[1] = 0x1234_5678; // the captured return slot of the function: compared
    assert_eq!(
        class(&judge(&entry, Mutation::None).rust),
        "callee_mismatch"
    );
}

#[test]
fn spool_round_trip_json_lines_flags_and_no_abi_entry() {
    let mut arena = arena();
    let mut entries = vec![entry_inc(&mut arena, 5), entry_inc(&mut arena, 6)];
    entries[1].flags = 2; // thread_switch
    let mut other = entry_inc(&mut arena, 7);
    other.va = 0x9999;
    entries.push(other);
    let mut records: Vec<Vec<u8>> = entries.iter().map(encode_record).collect();
    records.push(vec![1, 2, 3]); // malformed
    let data = encode_spool("{\"lifted_tree_hash\": \"t\"}", &records);
    assert_eq!(spool::decode_record(&records[0]).unwrap(), entries[0]);

    let mut out = Vec::new();
    let mut fake = Fake::new(Mutation::None);
    let summary = run_spool(
        &mut fake,
        &mut arena,
        &data,
        VA_INC,
        Options::default(),
        &mut out,
    )
    .unwrap();
    let text = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 4);
    let digest = input_digest(&entries[0]);
    assert_eq!(
        lines[0],
        format!(
            "{{\"index\": 0, \"digest\": \"{digest}\", \"lifted\": \"equal\", \"rust\": \"equal\", \"detail\": \"\", \"extra_read\": 0, \"dead_stack_writes\": 4, \"no_work\": false}}"
        )
    );
    assert!(
        lines[1].contains("\"lifted\": \"skipped\"")
            && lines[1].contains("capture_incomplete:thread_switch")
    );
    assert!(lines[2].contains("record va 0x9999 is not 0x1000"));
    assert!(lines[3].contains("\"index\": 3") && lines[3].contains("malformed"));
    assert_eq!(
        (summary.entries, summary.skipped, summary.lifted_equal),
        (4, 3, 1)
    );
    assert_eq!(summary.faithful_rate(), Some(1.0));
    assert!(!summary.no_abi_entry);

    let mut out = Vec::new();
    let mut no_port = Fake::new(Mutation::None);
    no_port.has_port = false;
    let only = encode_spool("{}", &records[..1]);
    let summary = run_spool(
        &mut no_port,
        &mut arena,
        &only,
        VA_INC,
        Options::default(),
        &mut out,
    )
    .unwrap();
    assert!(summary.no_abi_entry);
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.contains("\"lifted\": \"equal\", \"rust\": \"no_abi_entry\""),
        "{text}"
    );

    let mut out = Vec::new();
    let lifted_only = Options {
        lifted: true,
        rust: false,
        from: 0,
        ..Options::default()
    };
    run_spool(&mut fake, &mut arena, &only, VA_INC, lifted_only, &mut out).unwrap();
    assert!(String::from_utf8(out).unwrap().contains("\"rust\": null"));
}

#[test]
fn spools_from_the_python_writer_parse_with_matching_digests() {
    // tests/fixtures/python_writer.spool: written by tools/t2corpus/spool.py write_spool with
    // synthetic bytes (one record with a callee, one flagged, then a truncated tail); the
    // digests are tools/t2corpus/keys.py input_digest of the two records.
    let data = include_bytes!("fixtures/python_writer.spool");
    let spool = spool::parse(data).unwrap();
    assert_eq!(spool.truncated, 1);
    assert_eq!(
        spool.header_value("lifted_tree_hash").as_deref(),
        Some("tree-x")
    );
    let entries: Vec<&Entry> = spool.records.iter().map(|r| r.as_ref().unwrap()).collect();
    assert_eq!(entries.len(), 2);
    assert_eq!(
        input_digest(entries[0]),
        "94433ae30f855e313b6c8eec5a997008f614a0c66cd95c7328bcc31897ac31d4"
    );
    assert_eq!(
        input_digest(entries[1]),
        "489baaf0fe70e42d9d8f2f9fbb50b444befcada5a58332fe051f6d03acb8e960"
    );
    let first = entries[0];
    assert_eq!(
        (first.va, first.entry_regs.eax, first.entry_regs.esp),
        (0x123450, 1, 0x300000)
    );
    assert_eq!(first.entry_regs.fpu.stack[0], 1.5);
    assert_eq!(first.entry_regs.fpu.control, 0x27f);
    assert_eq!(first.entry_regs.xmm[0].lanes, [1, 2, 3, 4]);
    assert_eq!(first.callees[0].args, vec![0x400000, 3]);
    assert_eq!(
        spool::flag_names(entries[1].flags),
        vec!["mmio", "thread_switch", "old_format"]
    );
    // T3740: a format 1 record (no implicit-state tail) is flagged, so it is skipped, never judged.
    assert_eq!(spool::flag_names(entries[0].flags), vec!["old_format"]);
    // A raw shard (records back to back, as in <va>/entries.bin) parses the same.
    let shard: Vec<u8> = entries.iter().flat_map(|e| encode_record(e)).collect();
    let raw = spool::parse(&shard).unwrap();
    assert_eq!(raw.records.len(), 2);
    assert_eq!(raw.records[0].as_ref().unwrap().entry_raw, first.entry_raw);
}
