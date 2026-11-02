// SPDX-License-Identifier: GPL-3.0-or-later
//! T3742 integration test against REAL code: the private lifted originals of three engine/math
//! functions (extracted by build.rs, never committed) versus their c2r Rust ports
//! (tsfp-port-engine-math). Every corpus entry is made by running the lifted original in-process
//! on seeded inputs (tests/common), so the expectations are what the original did. The runner
//! then performs the lifted-fidelity control and the port comparison exactly as the host mode
//! does, with the lifted callee 0x5F040 routed into the same callee stub.
//!
//! Inputs are FABRICATED (seeded floats and pointers), evidence class Tier 2 harness validation,
//! not game-capture evidence. Compiled only when the lifted tree is present (`cfg(t2o_lifted)`).
#![cfg(t2o_lifted)]

#[path = "../../tsfp-t2-offline/tests/common/mod.rs"]
mod common;

use std::cell::Cell;
use std::ffi::c_void;
use std::ptr::null_mut;

use common::*;
use tsfp_port_engine_math::c2r::{objchar_wired4, t3398_math4};
use tsfp_port_runtime::abi::{ExtRegisters, GuestCall};
use tsfp_port_runtime::memory::GuestMemory;
use tsfp_t2_offline::arena::{Arena, GuestView};
use tsfp_t2_offline::spool::{Entry, encode_record, encode_spool};
use tsfp_t2_offline::stub::{CalleeStub, Convention};
use tsfp_t2_offline::{Executor, Options, Verdict, run_entry, run_spool};

const MATRIX_COPY: u32 = 0x1531D0;
const APPROACH: u32 = 0x1515E0;
const PROP_ADVANCE: u32 = 0x5F430;
const TRACK_SAMPLE: u32 = 0x5F040;
const DST: u32 = 0x0050_0000;
const SRC: u32 = 0x0051_0000;
const OBJECT: u32 = 0x0052_0040;
const PER_FUNCTION: usize = 24;

/// The lifted object reads the process-wide `g_xbox_mem_offset` (as in the host), so tests that
/// run lifted code with their own arenas take turns.
static LIFTED_GLOBALS: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[link(name = "t2o_lifted_test", kind = "static")]
unsafe extern "C" {
    fn t2o_test_run_lifted(va: u32, regs: *mut ExtRegisters, base: isize) -> i32;
    fn t2o_test_set_callee_hook(hook: Option<unsafe extern "C" fn(u32, *mut ExtRegisters)>);
}

type Hook<'a> = dyn FnMut(u32, &mut ExtRegisters) + 'a;

thread_local! {
    static HOOK: Cell<*mut c_void> = const { Cell::new(null_mut()) };
}

unsafe extern "C" fn lifted_callee(va: u32, regs: *mut ExtRegisters) {
    let hook = HOOK.with(Cell::get).cast::<&mut Hook<'static>>();
    assert!(!hook.is_null(), "lifted callee {va:#x} outside a run");
    unsafe { (*hook)(va, &mut *regs) };
}

/// Run the lifted original with its nested calls handed to `on_call` (lifted convention: the
/// return PC is already pushed).
fn lifted(va: u32, base: usize, regs: &mut ExtRegisters, mut on_call: &mut Hook<'_>) {
    let previous = HOOK.with(|h| h.replace((&raw mut on_call).cast()));
    unsafe { t2o_test_set_callee_hook(Some(lifted_callee)) };
    let ran = unsafe { t2o_test_run_lifted(va, regs, base as isize) };
    HOOK.with(|h| h.set(previous));
    assert_eq!(ran, 1, "no lifted extract for {va:#x}");
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PortBug {
    None,
    /// Flip one output bit after the port ran (a wrong port).
    FlipOutput,
}

/// The c2r port as the host dispatch would run it; ESP as after the wrapper's `ret`.
fn port(va: u32, base: usize, regs: &mut ExtRegisters, guest: &mut dyn GuestCallDyn, bug: PortBug) {
    let mut m = unsafe { GuestView::new(base) };
    let mut guest = DynCall(guest);
    match va {
        MATRIX_COPY => objchar_wired4::game_matrix4x4_copy(&mut m, regs),
        APPROACH => t3398_math4::game_float_approach_target_step_returns_arrived(&mut m, regs),
        PROP_ADVANCE => {
            objchar_wired4::game_prop_anim_advance_track_then_set_flags_0x2_and_0x200000(
                &mut m, &mut guest, regs,
            )
        }
        _ => unreachable!(),
    }
    regs.esp = regs.esp.wrapping_add(4);
    if bug == PortBug::FlipOutput {
        let target = match va {
            MATRIX_COPY => DST + 0x24,
            APPROACH => m.read32(regs.esp), // the `current` pointer argument
            _ => OBJECT + 0x28,
        };
        let value = m.read32(target);
        m.write32(target, value ^ 0x10);
    }
}

/// Object-safe forwarding so the stub and the recorder share one port driver.
trait GuestCallDyn {
    fn call_dyn(&mut self, va: u32, regs: &mut ExtRegisters);
}
impl<T: GuestCall> GuestCallDyn for T {
    fn call_dyn(&mut self, va: u32, regs: &mut ExtRegisters) {
        self.call(va, regs)
    }
}
struct DynCall<'a>(&'a mut dyn GuestCallDyn);
impl GuestCall for DynCall<'_> {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        self.0.call_dyn(va, regs)
    }
}

struct Real {
    base: usize,
    bug: PortBug,
}

impl Executor for Real {
    fn bind(&mut self, base: usize) {
        self.base = base;
    }
    fn has_port(&self, va: u32) -> bool {
        matches!(va, MATRIX_COPY | APPROACH | PROP_ADVANCE)
    }
    fn lifted_stubs_callees(&self) -> bool {
        true
    }
    fn run_lifted(&mut self, va: u32, regs: &mut ExtRegisters, stub: &mut CalleeStub<'_>) {
        lifted(va, self.base, regs, &mut |callee, r| {
            stub.dispatch(callee, r, Convention::ReturnPushed)
        });
    }
    fn run_port(&mut self, va: u32, regs: &mut ExtRegisters, stub: &mut CalleeStub<'_>) {
        port(va, self.base, regs, stub, self.bug);
    }
}

fn float_words(seed: &mut Seed, count: usize) -> Vec<u32> {
    (0..count).map(|_| seed.float().to_bits()).collect()
}

/// Capture `PER_FUNCTION` calls of each function by running its lifted original.
fn corpus(arena: &mut Arena) -> Vec<Entry> {
    let mut seed = Seed(0x3742_0001);
    let mut entries = Vec::new();
    for index in 0..PER_FUNCTION {
        let esp = ESP - 0x40 * index as u32;
        let reads = vec![
            words(esp, &[RETURN_PC, DST, SRC]),
            words(SRC, &float_words(&mut seed, 16)),
        ];
        entries.push(capture(
            arena,
            MATRIX_COPY,
            regs(esp),
            reads,
            vec![],
            &mut |base, r, rec| {
                lifted(MATRIX_COPY, base, r, &mut |va, r| {
                    rec.call(va, r, Convention::ReturnPushed)
                })
            },
        ));
    }
    for index in 0..PER_FUNCTION {
        let esp = ESP - 0x40 * index as u32;
        let current = seed.float();
        // Every third target equals the current value, so both branches are covered.
        let target = if index % 3 == 0 {
            current
        } else {
            seed.float()
        };
        let step = seed.float().abs();
        let reads = vec![
            words(esp, &[RETURN_PC, SRC, target.to_bits(), step.to_bits()]),
            words(SRC, &[current.to_bits()]),
        ];
        entries.push(capture(
            arena,
            APPROACH,
            regs(esp),
            reads,
            vec![],
            &mut |base, r, rec| {
                lifted(APPROACH, base, r, &mut |va, r| {
                    rec.call(va, r, Convention::ReturnPushed)
                })
            },
        ));
    }
    for index in 0..PER_FUNCTION {
        let esp = ESP - 0x40 * index as u32;
        let object: Vec<u32> = (0..12).map(|_| seed.next()).collect();
        let reads = vec![words(esp, &[RETURN_PC, OBJECT]), words(OBJECT, &object)];
        // The sampler callee: returns a value and, every other call, rewrites the flags word
        // the caller then reads back (subtree-defined bytes, not in the caller's read-set).
        let mut writes = vec![words(DST, &[seed.next(), seed.next()])];
        if index % 2 == 0 {
            writes.push(words(OBJECT + 0x18, &[seed.next()]));
        }
        let outcome = Outcome {
            eax: seed.next(),
            writes,
        };
        entries.push(capture(
            arena,
            PROP_ADVANCE,
            regs(esp),
            reads,
            vec![outcome],
            &mut |base, r, rec| {
                lifted(PROP_ADVANCE, base, r, &mut |va, r| {
                    rec.call(va, r, Convention::ReturnPushed)
                })
            },
        ));
    }
    entries
}

fn class(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Judged(j) => j.class.name().to_string(),
        other => format!("{other:?}"),
    }
}

#[test]
fn lifted_control_is_faithful_and_the_c2r_ports_are_equal() {
    let _turn = LIFTED_GLOBALS.lock().unwrap_or_else(|p| p.into_inner());
    let mut arena = arena();
    let entries = corpus(&mut arena);
    assert_eq!(entries.len(), 3 * PER_FUNCTION);
    // The captures are real: the copy wrote 64 bytes, the prop advance called its sampler.
    let copy = &entries[0];
    assert!(copy.write_runs.iter().map(|w| w.data.len()).sum::<usize>() >= 64);
    assert_eq!(entries[2 * PER_FUNCTION].callees[0].va, TRACK_SAMPLE);
    let mut real = Real {
        base: 0,
        bug: PortBug::None,
    };
    for va in [MATRIX_COPY, APPROACH, PROP_ADVANCE] {
        let records: Vec<Vec<u8>> = entries
            .iter()
            .filter(|e| e.va == va)
            .map(encode_record)
            .collect();
        let data = encode_spool("{\"lifted_tree_hash\": \"test\"}", &records);
        let mut out = Vec::new();
        let summary = run_spool(
            &mut real,
            &mut arena,
            &data,
            va,
            Options::default(),
            &mut out,
        )
        .unwrap();
        let text = String::from_utf8(out).unwrap();
        eprintln!("{} va={va:#x}", summary.line());
        assert_eq!(summary.faithful_rate(), Some(1.0), "{va:#x}: {text}");
        assert_eq!(
            summary.rust.get("equal").copied(),
            Some(PER_FUNCTION),
            "{va:#x}: {text}"
        );
    }
}

#[test]
fn a_broken_port_or_capture_is_diverged_on_real_code() {
    let _turn = LIFTED_GLOBALS.lock().unwrap_or_else(|p| p.into_inner());
    let mut arena = arena();
    let entries = corpus(&mut arena);
    let mut broken = Real {
        base: 0,
        bug: PortBug::FlipOutput,
    };
    broken.bind(arena.base());
    for entry in [
        &entries[0],
        &entries[PER_FUNCTION + 1],
        &entries[2 * PER_FUNCTION],
    ] {
        let result = run_entry(
            &mut broken,
            &mut arena,
            0,
            entry,
            entry.va,
            Options::default(),
        );
        assert_eq!(class(&result.lifted), "equal", "{result:?}");
        assert_eq!(class(&result.rust), "write_set", "{result:?}");
    }
    let mut real = Real {
        base: 0,
        bug: PortBug::None,
    };
    real.bind(arena.base());
    // The comparison is live: one flipped expected byte fails the lifted control.
    let mut tampered = entries[0].clone();
    tampered
        .write_runs
        .iter_mut()
        .find(|w| w.addr >= DST)
        .unwrap()
        .data[3] ^= 1;
    let result = run_entry(
        &mut real,
        &mut arena,
        0,
        &tampered,
        MATRIX_COPY,
        Options::default(),
    );
    assert_eq!(class(&result.lifted), "write_set", "{result:?}");
    // A captured callee the code does not make (wrong VA) is a mismatch on both sides.
    let mut wrong_callee = entries[2 * PER_FUNCTION].clone();
    wrong_callee.callees[0].va = TRACK_SAMPLE + 0x10;
    let result = run_entry(
        &mut real,
        &mut arena,
        0,
        &wrong_callee,
        PROP_ADVANCE,
        Options::default(),
    );
    assert_eq!(class(&result.lifted), "callee_mismatch", "{result:?}");
    assert_eq!(class(&result.rust), "callee_mismatch", "{result:?}");
    // A wrong return value of the approach function is an exit-register divergence.
    let mut wrong_eax = entries[PER_FUNCTION + 1].clone();
    wrong_eax.exit_regs.eax ^= 1;
    let result = run_entry(
        &mut real,
        &mut arena,
        0,
        &wrong_eax,
        APPROACH,
        Options::default(),
    );
    assert_eq!(class(&result.lifted), "exit_regs", "{result:?}");
}
