//! Reader for the numeric golden vectors written by `tools/t1846_golden.py` (T1846).
//!
//! A data file holds cases separated by blank lines. Lines: `in <addr> <word>...` (memory before the call),
//! `out <addr> <word>...` (memory after), `call <callee> <eax ecx edx esi edi> <stack dwords>...` (stubbed engine
//! call, in order), `ret <eax> [<st0 f64 bits>]`. Words and addresses are lowercase hex.
//! Values come from running the original function under the Unicorn oracle on synthetic inputs; no code or asset
//! bytes are stored.
#![allow(dead_code)]

pub mod guest;

use std::collections::BTreeMap;

#[derive(Default, Debug, Clone)]
pub struct Case {
    pub ins: BTreeMap<u32, u32>,
    pub outs: BTreeMap<u32, u32>,
    pub eax: u32,
    pub st0: Option<u64>,
    /// Stubbed engine calls in order: (callee, eax, ecx, edx, esi, edi, first stack dwords).
    pub calls: Vec<Call>,
}

#[derive(Debug, Clone, Default)]
pub struct Call {
    pub callee: u32,
    pub regs: [u32; 5],
    pub args: Vec<u32>,
}

impl Case {
    pub fn input(&self, address: u32) -> u32 {
        *self.ins.get(&address).unwrap_or(&0)
    }
    pub fn input_f(&self, address: u32) -> f32 {
        f32::from_bits(self.input(address))
    }
    pub fn vec3(&self, address: u32) -> [f32; 3] {
        [
            self.input_f(address),
            self.input_f(address + 4),
            self.input_f(address + 8),
        ]
    }
    pub fn output(&self, address: u32) -> u32 {
        *self
            .outs
            .get(&address)
            .unwrap_or_else(|| panic!("no output at {address:#x}"))
    }
    pub fn output_f(&self, address: u32) -> f32 {
        f32::from_bits(self.output(address))
    }
}

pub fn parse(text: &str) -> Vec<Case> {
    let mut cases = Vec::new();
    let mut current = Case::default();
    let mut any = false;
    for line in text.lines().chain(std::iter::once("")) {
        let mut parts = line.split_whitespace();
        let Some(kind) = parts.next() else {
            if any {
                cases.push(std::mem::take(&mut current));
                any = false;
            }
            continue;
        };
        any = true;
        let hex = |s: &str| u32::from_str_radix(s, 16).expect("hex word");
        match kind {
            "in" | "out" => {
                let address = hex(parts.next().expect("address"));
                let map = if kind == "in" {
                    &mut current.ins
                } else {
                    &mut current.outs
                };
                for (index, word) in parts.enumerate() {
                    map.insert(address + 4 * index as u32, hex(word));
                }
            }
            "call" => {
                let callee = hex(parts.next().expect("callee"));
                let mut regs = [0u32; 5];
                for slot in regs.iter_mut() {
                    *slot = hex(parts.next().expect("reg"));
                }
                current.calls.push(Call {
                    callee,
                    regs,
                    args: parts.map(hex).collect(),
                });
            }
            "ret" => {
                current.eax = hex(parts.next().expect("eax"));
                current.st0 = parts
                    .next()
                    .map(|s| u64::from_str_radix(s, 16).expect("st0"));
            }
            other => panic!("unknown golden line {other}"),
        }
    }
    cases
}
