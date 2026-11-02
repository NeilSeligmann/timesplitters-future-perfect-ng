// SPDX-License-Identifier: GPL-3.0-or-later
//! compare v11 (2026-10-27, T3867), INFERRED: a callee argument that is a pointer into the root's
//! own stack frame is compared relative to the call's ESP, not absolutely. The readable port
//! reserves a different frame than the original (0x3D7F0 passed 0x4015fe6c, the port 0x4015fde4),
//! so the absolute address differs while the frame slot it names is the same. A pointer that
//! names another slot, and a plain value difference, stay diverged.
mod common;

use common::*;
use tsfp_port_runtime::abi::ExtRegisters;
use tsfp_port_runtime::memory::GuestMemory;
use tsfp_t2_offline::arena::{Arena, GuestView};
use tsfp_t2_offline::spool::Entry;
use tsfp_t2_offline::stub::{CalleeStub, Convention};
use tsfp_t2_offline::{EntryResult, Executor, Options, Verdict, run_entry};

const VA_ROOT: u32 = 0x6000;
const CALLEE: u32 = 0x5100;
const ORIGINAL_FRAME: u32 = 0x40;
const SLOT: u32 = 0x10; // the local's distance from ESP at the call

/// `sub esp, frame; lea eax, [esp + slot]; push eax; call CALLEE; add esp, 4 + frame; ret`.
fn body(
    base: usize,
    r: &mut ExtRegisters,
    frame: u32,
    arg: impl Fn(u32) -> u32,
    call: &mut dyn FnMut(&mut ExtRegisters),
) {
    let mut m = view(base);
    r.esp -= frame;
    let local = r.esp + SLOT;
    r.esp -= 4;
    m.write32(r.esp, arg(local));
    call(r);
    r.esp += 4 + frame;
    r.esp += 4; // ret
}

fn view(base: usize) -> GuestView {
    unsafe { GuestView::new(base) }
}

struct Fake {
    base: usize,
    frame: u32,
    arg: fn(u32) -> u32,
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
        body(self.base, regs, ORIGINAL_FRAME, |l| l, &mut |r| {
            stub.dispatch(CALLEE, r, Convention::BridgePushesReturn)
        });
    }
    fn run_port(&mut self, _va: u32, regs: &mut ExtRegisters, stub: &mut CalleeStub<'_>) {
        body(self.base, regs, self.frame, self.arg, &mut |r| {
            stub.dispatch(CALLEE, r, Convention::BridgePushesReturn)
        });
    }
}

fn captured(arena: &mut Arena) -> Entry {
    let outcomes = vec![Outcome {
        eax: 5,
        writes: vec![],
    }];
    let entry = capture(
        arena,
        VA_ROOT,
        regs(ESP),
        vec![words(ESP, &[RETURN_PC])],
        outcomes,
        &mut |base, r, rec| {
            body(base, r, ORIGINAL_FRAME, |l| l, &mut |r| {
                rec.call(CALLEE, r, Convention::BridgePushesReturn)
            })
        },
    );
    assert_eq!(entry.callees[0].args[0], ESP - ORIGINAL_FRAME + SLOT);
    entry
}

fn class(frame: u32, arg: fn(u32) -> u32) -> String {
    let mut arena = arena();
    let entry = captured(&mut arena);
    let mut fake = Fake {
        base: 0,
        frame,
        arg,
    };
    fake.bind(arena.base());
    let result: EntryResult = run_entry(
        &mut fake,
        &mut arena,
        0,
        &entry,
        VA_ROOT,
        Options::default(),
    );
    match &result.rust {
        Verdict::Judged(j) => format!("{}: {}", j.class.name(), result.detail),
        _ => "not judged".into(),
    }
}

#[test]
fn same_frame_slot_in_a_larger_frame_is_equal() {
    assert!(class(ORIGINAL_FRAME + 0x88, |l| l).starts_with("equal"));
}

#[test]
fn pointer_to_another_slot_still_diverges() {
    let got = class(ORIGINAL_FRAME + 0x88, |l| l + 4);
    assert!(got.starts_with("callee_mismatch"), "{got}");
}

#[test]
fn value_outside_the_stack_still_diverges() {
    let got = class(ORIGINAL_FRAME, |_| 0x0050_0000);
    assert!(got.starts_with("callee_mismatch"), "{got}");
}
