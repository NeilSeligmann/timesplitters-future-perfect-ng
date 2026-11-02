// SPDX-License-Identifier: GPL-3.0-or-later
//! Synthetic corpus builder: an entry is made by RUNNING the reference ("lifted") code once in a
//! fault-tracked arena on chosen inputs, so its expectations are real: exit registers as the code
//! left them, the write-set from the bytes that changed on written pages, and every nested call
//! recorded the way the T3740 capture records it (VA, entry registers, 16 stack words, chosen
//! outcome applied, exit registers, the callee's writes).
#![allow(dead_code)]

use tsfp_port_runtime::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_t2_offline::arena::{Arena, GUEST_WINDOW, PAGE, RunEnd};
use tsfp_t2_offline::spool::{CalleeRecord, Entry, Run, encode_regs};
use tsfp_t2_offline::stub::Convention;

pub const RETURN_PC: u32 = 0x00ab_cdef;
pub const ESP: u32 = 0x4100_fd00;

pub fn arena() -> Arena {
    Arena::reserve(GUEST_WINDOW).expect("arena")
}

pub fn regs(esp: u32) -> ExtRegisters {
    ExtRegisters {
        eax: 0x1111_0000,
        ecx: 0x2222_0000,
        edx: 0x3333_0000,
        esp,
        ebx: 0x4444_0000,
        ebp: 0x5555_0000,
        esi: 0x6666_0000,
        edi: 0x7777_0000,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0x027f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

pub fn run(addr: u32, data: &[u8]) -> Run {
    Run {
        addr,
        data: data.to_vec(),
    }
}

pub fn words(addr: u32, values: &[u32]) -> Run {
    run(
        addr,
        &values
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<_>>(),
    )
}

/// The outcome a recorded callee produces (applied while capturing, replayed offline).
#[derive(Clone, Debug)]
pub struct Outcome {
    pub eax: u32,
    pub writes: Vec<Run>,
}

pub struct Recorder<'a> {
    arena: &'a Arena,
    outcomes: Vec<Outcome>,
    pub callees: Vec<CalleeRecord>,
}

impl Recorder<'_> {
    pub fn call(&mut self, va: u32, regs: &mut ExtRegisters, convention: Convention) {
        let entry_esp = match convention {
            Convention::BridgePushesReturn => regs.esp.wrapping_sub(4),
            Convention::ReturnPushed => regs.esp,
        };
        let mut entry = *regs;
        entry.esp = entry_esp;
        let args = (0..16)
            .map(|i| {
                let at = u64::from(entry_esp) + 4 + 4 * i;
                let bytes: [u8; 4] =
                    core::array::from_fn(|b| self.arena.byte(at + b as u64).unwrap_or(0xcd));
                u32::from_le_bytes(bytes)
            })
            .collect();
        let outcome = self.outcomes.remove(0);
        for write in &outcome.writes {
            unsafe { self.arena.write_bytes(write.addr, &write.data) };
        }
        regs.eax = outcome.eax;
        regs.esp = entry_esp.wrapping_add(4);
        let mut exit = *regs;
        exit.esp = entry_esp.wrapping_add(4);
        self.callees.push(CalleeRecord {
            va,
            entry_regs: entry,
            args,
            exit_regs: exit,
            write_runs: outcome.writes,
            exit_implicit: Some([0; 8]),
        });
    }
}

impl GuestCall for Recorder<'_> {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        Recorder::call(self, va, regs, Convention::BridgePushesReturn);
    }
}

/// Capture one call of reference code: `body` runs it with the arena prepared from `reads`.
pub fn capture(
    arena: &mut Arena,
    va: u32,
    entry_regs: ExtRegisters,
    reads: Vec<Run>,
    outcomes: Vec<Outcome>,
    body: &mut dyn FnMut(usize, &mut ExtRegisters, &mut Recorder<'_>),
) -> Entry {
    arena.prepare(&reads).expect("prepare");
    let arena: &Arena = arena;
    let mut regs = entry_regs;
    let mut recorder = Recorder {
        arena,
        outcomes,
        callees: Vec::new(),
    };
    let base = arena.base();
    let end = arena.run(&mut || body(base, &mut regs, &mut recorder));
    assert_eq!(end, RunEnd::Returned, "reference code must run cleanly");
    assert!(recorder.outcomes.is_empty(), "unused callee outcomes");
    let mut write_runs: Vec<Run> = Vec::new();
    for page in arena.written_pages() {
        for addr in (page * PAGE) as u64..((page + 1) * PAGE) as u64 {
            let now = arena.byte(addr).expect("written page is mapped");
            if now == arena.initial_byte(addr) {
                continue;
            }
            match write_runs.last_mut() {
                Some(last) if u64::from(last.addr) + last.data.len() as u64 == addr => {
                    last.data.push(now)
                }
                _ => write_runs.push(run(addr as u32, &[now])),
            }
        }
    }
    Entry {
        va,
        caller_va: 0x0001_0000,
        return_address: RETURN_PC,
        poll: 1,
        guest_thread: 1,
        flags: 0,
        entry_raw: encode_regs(&entry_regs),
        entry_regs,
        exit_regs: regs,
        read_runs: reads,
        write_runs,
        callees: recorder.callees,
        entry_implicit: Some([0; 8]),
        exit_implicit: Some([0; 8]),
        clock_events: None,
    }
}

/// Deterministic xorshift inputs.
pub struct Seed(pub u32);

impl Seed {
    pub fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
    pub fn float(&mut self) -> f32 {
        ((self.next() >> 8) as f32 / 16_777_215.0) * 200.0 - 100.0
    }
}
