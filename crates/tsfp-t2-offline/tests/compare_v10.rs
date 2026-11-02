// SPDX-License-Identifier: GPL-3.0-or-later
//! compare v10 (T3839): recursive self calls with a nested callee in the inner frames.
//!
//! The capture is flat per frame: the root's list holds `DRAW(0), ROOT(1)` and the opaque
//! `ROOT(1)` slot hides the inner frame's `DRAW(1), ROOT(2)`, which only the inner activation's
//! own record holds. 0x8F5E0's port runs the recursion natively (its own VA is a declared
//! inlined callee) and makes every frame's draw call (0x18EA90) itself; compare v9 matched those
//! against the root's list (`unexpected call`). Each test here fails on v9 or guards a near miss
//! that must stay diverged.
mod common;

use common::*;
use tsfp_port_runtime::abi::ExtRegisters;
use tsfp_port_runtime::memory::GuestMemory;
use tsfp_t2_offline::arena::{Arena, GuestView};
use tsfp_t2_offline::frames::NestedFrames;
use tsfp_t2_offline::spool::{Entry, Run};
use tsfp_t2_offline::stub::{CalleeArity, CalleeStub, Convention, Inlined};
use tsfp_t2_offline::{EntryResult, Executor, Options, Verdict, run_entry_in};

const ROOT: u32 = 0x8000;
const DRAW: u32 = 0x5200;
const OUT: u32 = 0x0030_0000;
/// Root call: node 0; the original recurses down to node DEPTH (frames at depth 0..=DEPTH).
const DEPTH: u32 = 2;
const RET_EAX: u32 = 0x77;
const STACK_LO: u32 = ESP - 0x200;

fn view(base: usize) -> GuestView {
    unsafe { GuestView::new(base) }
}

fn frame_esp(depth: u32) -> u32 {
    ESP - 8 * depth
}

/// The original 0x8F5E0 shape: `DRAW(node)`, then `ROOT(node + 1)` while `node < DEPTH`.
fn original(base: usize, regs: &mut ExtRegisters, call: &mut dyn FnMut(u32, &mut ExtRegisters)) {
    let mut m = view(base);
    let node = m.read32(regs.esp + 4);
    regs.esp -= 4;
    m.write32(regs.esp, node);
    call(DRAW, regs);
    regs.esp += 4;
    if node < DEPTH {
        regs.esp -= 4;
        m.write32(regs.esp, node + 1);
        call(ROOT, regs);
        regs.esp += 4;
    }
    regs.eax = RET_EAX;
    regs.esp += 4; // ret
}

fn reads(depth: u32) -> Vec<Run> {
    let e = frame_esp(depth);
    let mut stack = vec![0u8; (e + 0x80 - STACK_LO) as usize];
    let arg = (e + 4 - STACK_LO) as usize;
    stack[arg..arg + 4].copy_from_slice(&depth.to_le_bytes());
    vec![run(STACK_LO, &stack), words(OUT, &[0; 8])]
}

fn draw_outcome(node: u32) -> Outcome {
    Outcome {
        eax: regs(0).eax, // leaves EAX as the frame had it: inner entry registers stay simple
        writes: vec![words(OUT + 4 * node, &[0x100 + node])],
    }
}

/// Every activation's record, deepest first; `corpus[d]` is the frame at depth d.
fn corpus() -> Vec<Entry> {
    let mut arena = arena();
    let mut inner: Option<Entry> = None;
    let mut out = Vec::new();
    for depth in (0..=DEPTH).rev() {
        let mut outcomes = vec![draw_outcome(depth)];
        if let Some(child) = &inner {
            outcomes.push(Outcome {
                eax: RET_EAX,
                writes: child.write_runs.clone(),
            });
        }
        let entry = capture(
            &mut arena,
            ROOT,
            regs(frame_esp(depth)),
            reads(depth),
            outcomes,
            &mut |base, r, rec| {
                original(base, r, &mut |va, r| {
                    rec.call(va, r, Convention::BridgePushesReturn)
                })
            },
        );
        inner = Some(entry.clone());
        out.push(entry);
    }
    out.reverse();
    out
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Port {
    /// Native recursion: `DRAW(n)` for n in 0..=last, no guest call to ROOT.
    Flat {
        last: u32,
        wrong_arg_at: Option<u32>,
    },
    /// Calls ROOT(1) through the guest like the original (replayed whole from the capture).
    Recursing,
}

struct Fake {
    base: usize,
    port: Port,
}

impl Executor for Fake {
    fn bind(&mut self, base: usize) {
        self.base = base;
    }
    fn has_port(&self, _va: u32) -> bool {
        true
    }
    fn lifted_stubs_callees(&self) -> bool {
        true
    }
    fn run_lifted(&mut self, _va: u32, regs: &mut ExtRegisters, stub: &mut CalleeStub<'_>) {
        original(self.base, regs, &mut |va, r| {
            stub.dispatch(va, r, Convention::BridgePushesReturn)
        });
    }
    fn run_port(&mut self, _va: u32, regs: &mut ExtRegisters, stub: &mut CalleeStub<'_>) {
        let (last, wrong) = match self.port {
            Port::Recursing => {
                return original(self.base, regs, &mut |va, r| {
                    stub.dispatch(va, r, Convention::BridgePushesReturn)
                });
            }
            Port::Flat { last, wrong_arg_at } => (last, wrong_arg_at),
        };
        let mut m = view(self.base);
        for node in 0..=last {
            regs.esp -= 4;
            m.write32(regs.esp, if wrong == Some(node) { node + 50 } else { node });
            stub.dispatch(DRAW, regs, Convention::BridgePushesReturn);
            regs.esp += 4;
        }
        regs.eax = RET_EAX;
        regs.esp += 4;
    }
}

fn arity() -> &'static CalleeArity {
    // DRAW takes one stack word; the rest of the window is the frames' own stack.
    Box::leak(Box::new(CalleeArity::parse("5200 1").expect("arity")))
}

fn judge_with(port: Port, corpus: &[Entry]) -> EntryResult {
    let mut arena: Arena = arena();
    let mut fake = Fake { base: 0, port };
    fake.bind(arena.base());
    let options = Options {
        inlined: Inlined::parse("0x8000").expect("inlined"),
        arity: Some(arity()),
        ..Options::default()
    };
    let frames = NestedFrames::new(ROOT, corpus);
    run_entry_in(&mut fake, &mut arena, 0, &corpus[0], ROOT, options, &frames)
}

fn judge(port: Port) -> EntryResult {
    judge_with(port, &corpus())
}

fn class(verdict: &Verdict) -> &'static str {
    match verdict {
        Verdict::Judged(j) => j.class.name(),
        _ => "not judged",
    }
}

#[test]
fn corpus_is_the_recursive_shape() {
    let corpus = corpus();
    let vas = |entry: &Entry| entry.callees.iter().map(|c| c.va).collect::<Vec<_>>();
    assert_eq!(vas(&corpus[0]), [DRAW, ROOT]);
    assert_eq!(vas(&corpus[1]), [DRAW, ROOT]);
    assert_eq!(vas(&corpus[2]), [DRAW]);
    let frames = NestedFrames::new(ROOT, &corpus);
    assert!(std::ptr::eq(
        frames.find(&corpus[0].callees[1]).unwrap(),
        &corpus[1]
    ));
    assert!(std::ptr::eq(
        frames.find(&corpus[1].callees[1]).unwrap(),
        &corpus[2]
    ));
}

#[test]
fn flattened_recursion_pairs_inner_frames_by_depth() {
    // v9: callee_mismatch@2: unexpected call to 0x5200 (captured 2).
    let result = judge(Port::Flat {
        last: DEPTH,
        wrong_arg_at: None,
    });
    assert_eq!(class(&result.lifted), "equal", "{}", result.detail);
    assert_eq!(class(&result.rust), "equal", "{}", result.detail);
}

#[test]
fn port_recursing_through_the_guest_replays_the_slot_whole() {
    let result = judge(Port::Recursing);
    assert_eq!(class(&result.rust), "equal", "{}", result.detail);
}

#[test]
fn wrong_argument_in_the_innermost_frame_diverges() {
    let result = judge(Port::Flat {
        last: DEPTH,
        wrong_arg_at: Some(DEPTH),
    });
    assert_eq!(class(&result.rust), "callee_mismatch", "{}", result.detail);
    assert!(
        result
            .detail
            .contains("0x5200 arg 0 expected 0x2, got 0x34")
            && result
                .detail
                .contains("nested 0x8000 frame depth 2, call 0"),
        "{}",
        result.detail
    );
}

#[test]
fn port_recursing_one_level_deeper_diverges() {
    let result = judge(Port::Flat {
        last: DEPTH + 1,
        wrong_arg_at: None,
    });
    assert_eq!(class(&result.rust), "callee_mismatch", "{}", result.detail);
    assert!(
        result.detail.contains("unexpected call to 0x5200"),
        "{}",
        result.detail
    );
}

#[test]
fn port_recursing_one_level_shallower_diverges() {
    let result = judge(Port::Flat {
        last: DEPTH - 1,
        wrong_arg_at: None,
    });
    assert_eq!(class(&result.rust), "callee_mismatch", "{}", result.detail);
    assert!(
        result.detail.contains("missing call to 0x5200") && result.detail.contains("frame depth 2"),
        "{}",
        result.detail
    );
}

#[test]
fn inner_record_missing_from_the_corpus_stays_diverged() {
    // Fail closed: without the depth-2 record its frame is opaque and its DRAW is unexpected.
    let corpus = corpus();
    let result = judge_with(
        Port::Flat {
            last: DEPTH,
            wrong_arg_at: None,
        },
        &corpus[..2],
    );
    assert_eq!(class(&result.rust), "callee_mismatch", "{}", result.detail);
}

#[test]
fn inner_record_with_other_writes_is_not_the_activation() {
    // Same entry registers, other net write-set: another call, never paired.
    let mut corpus = corpus();
    corpus[2].write_runs = vec![words(OUT + 0x10, &[9])];
    let result = judge_with(
        Port::Flat {
            last: DEPTH,
            wrong_arg_at: None,
        },
        &corpus,
    );
    assert_eq!(class(&result.rust), "callee_mismatch", "{}", result.detail);
}

#[test]
fn self_slot_is_not_expanded_unless_declared_inlined() {
    let corpus = corpus();
    let mut arena: Arena = arena();
    let mut fake = Fake {
        base: 0,
        port: Port::Flat {
            last: DEPTH,
            wrong_arg_at: None,
        },
    };
    fake.bind(arena.base());
    let options = Options {
        arity: Some(arity()),
        ..Options::default()
    };
    let frames = NestedFrames::new(ROOT, &corpus);
    let result = run_entry_in(&mut fake, &mut arena, 0, &corpus[0], ROOT, options, &frames);
    assert_eq!(class(&result.rust), "callee_mismatch", "{}", result.detail);
    assert!(
        result.detail.contains("expected 0x8000, got 0x5200"),
        "{}",
        result.detail
    );
}
