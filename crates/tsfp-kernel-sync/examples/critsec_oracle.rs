// SPDX-License-Identifier: GPL-3.0-or-later
use std::{
    collections::BTreeMap,
    io::{self, BufRead},
};
use tsfp_kernel_sync::{ThreadId, critsec::*};
#[path = "../tests/support/words.rs"]
mod words;
#[derive(Default)]
struct Memory(BTreeMap<u32, u32>);
impl GuestWords for Memory {
    fn read(&mut self, address: u32) -> Option<u32> {
        words::read_word(&self.0, address)
    }
    fn write(&mut self, address: u32, value: u32) {
        if let Some(word) = self.0.get_mut(&address) {
            *word = value;
        }
    }
}
fn snapshot(model: &CriticalSections, memory: &mut Memory, address: u32, result: u32) {
    let r = model.record(GuestAddress(address));
    print!(
        "{} {} {} {} {}",
        result,
        model.tracked(),
        r.and_then(|r| r.owner).map_or(0, |o| o.0),
        r.map_or(0, |r| r.recursion),
        u32::from(r.is_some_and(|r| r.adopted))
    );
    for index in 0..7 {
        print!(" {}", memory.read(address + index * 4).unwrap_or(0));
    }
    let d = &model.diagnostics;
    println!(
        " {} {} {} {} {} {} {} {} {}",
        d.adopted,
        d.bad_leave,
        d.contended,
        d.recursive_enter,
        d.implausible,
        d.unreadable,
        d.table_full,
        d.reinit_held,
        d.retired
    );
}
fn main() {
    let mut memory = Memory::default();
    for address in (0x21000000..0x21010000).step_by(4) {
        memory.0.insert(address, 0);
    }
    let mut model = CriticalSections::default();
    for line in io::stdin().lock().lines() {
        let v: Vec<u32> = line
            .unwrap()
            .split_whitespace()
            .map(|v| v.parse().unwrap())
            .collect();
        let (op, address, caller, value) = (v[0], v[1], v[2], v[3]);
        if op == 1 {
            memory.write(address, value);
            continue;
        }
        if op == 2 {
            model.reset();
            continue;
        }
        let result = if op == 900 {
            memory.write(0x21000004, address);
            let frame = Some(CallFrame {
                stack: 0x21000000,
                limit: 0x21000008,
            });
            model.call(291, frame, ThreadId(1), &mut memory);
            model.call(277, frame, ThreadId(1), &mut memory);
            let CallOutcome::Pending(ticket) = model.call(277, frame, ThreadId(2), &mut memory)
            else {
                panic!("expected contention");
            };
            snapshot(&model, &mut memory, address, 0);
            model.call(294, frame, ThreadId(1), &mut memory);
            assert!(matches!(
                model.complete_enter(ticket, &mut memory),
                CallOutcome::Returned(0)
            ));
            snapshot(&model, &mut memory, address, 0);
            model.call(294, frame, ThreadId(2), &mut memory);
            0
        } else {
            memory.write(0x21000004, address);
            let frame = if value == 1 {
                None
            } else {
                Some(CallFrame {
                    stack: 0x21000000,
                    limit: if value == 2 { 0x21000004 } else { 0x21000008 },
                })
            };
            match model.call(op, frame, ThreadId(caller), &mut memory) {
                CallOutcome::Returned(result) => result,
                CallOutcome::Unregistered => 0, // C HLE fallback, not TryEnter success.
                CallOutcome::Pending(_) => panic!("unplanned blocking call"),
            }
        };
        snapshot(&model, &mut memory, address, result);
    }
}
