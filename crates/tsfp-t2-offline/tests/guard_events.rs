// SPDX-License-Identifier: GPL-3.0-or-later
//! T3765 (I-T3765-2): a lane port run whose guard fired (an access refused and repaired by the
//! guard, a callee skipped, or the original run instead) is never judged equal, even when its
//! exit state matches the capture exactly.
mod common;

use std::cell::Cell;

use common::*;
use tsfp_port_runtime::abi::ExtRegisters;
use tsfp_t2_offline::spool::{Entry, encode_regs};
use tsfp_t2_offline::stub::CalleeStub;
use tsfp_t2_offline::{Executor, Options, Verdict, run_entry};

const VA: u32 = 0x7000;

/// A port that returns exactly the captured exit state; `fire` moves the guard count like a
/// degraded or rolled-back lane run would.
struct Port {
    events: Cell<u64>,
    fire: bool,
}

impl Executor for Port {
    fn bind(&mut self, _base: usize) {}
    fn has_port(&self, _va: u32) -> bool {
        true
    }
    fn lifted_stubs_callees(&self) -> bool {
        true
    }
    fn run_lifted(&mut self, _va: u32, _regs: &mut ExtRegisters, _stub: &mut CalleeStub<'_>) {
        unreachable!("port side only")
    }
    fn run_port(&mut self, _va: u32, regs: &mut ExtRegisters, _stub: &mut CalleeStub<'_>) {
        regs.eax = 7;
        regs.esp += 4;
        if self.fire {
            self.events.set(self.events.get() + 1);
        }
    }
    fn guard_events(&self) -> Option<u64> {
        Some(self.events.get())
    }
}

fn captured() -> Entry {
    let entry_regs = regs(ESP);
    let mut exit_regs = entry_regs;
    exit_regs.eax = 7;
    exit_regs.esp = ESP + 4;
    Entry {
        va: VA,
        caller_va: 0x0001_0000,
        return_address: RETURN_PC,
        poll: 1,
        guest_thread: 1,
        flags: 0,
        entry_raw: encode_regs(&entry_regs),
        entry_regs,
        exit_regs,
        read_runs: vec![],
        write_runs: vec![],
        callees: vec![],
        entry_implicit: Some([0; 8]),
        exit_implicit: Some([0; 8]),
        clock_events: None,
    }
}

fn port_class(fire: bool) -> (String, String) {
    let mut exec = Port {
        events: Cell::new(3),
        fire,
    };
    let mut arena = arena();
    exec.bind(arena.base());
    let options = Options {
        lifted: false,
        rust: true,
        from: 0,
        ..Options::default()
    };
    let result = run_entry(&mut exec, &mut arena, 0, &captured(), VA, options);
    let class = match &result.rust {
        Verdict::Judged(j) => j.class.name().to_string(),
        other => format!("{other:?}"),
    };
    (class, result.detail)
}

#[test]
fn quiet_guard_keeps_an_equal_port_equal() {
    let (class, detail) = port_class(false);
    assert_eq!(class, "equal", "{detail}");
}

#[test]
fn fired_guard_refuses_an_otherwise_equal_port() {
    let (class, detail) = port_class(true);
    assert_eq!(class, "fault", "{detail}");
    assert!(detail.contains("lane_guard_fired"), "{detail}");
}
