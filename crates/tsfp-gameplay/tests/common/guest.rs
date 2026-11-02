//! Replay host for the effect pool goldens (T1871): guest memory from the case, engine answers from the saved tape.
#![allow(dead_code)]
use super::Case;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use tsfp_gameplay::story_events::eff_dispatch;
use tsfp_gameplay::story_events::eff_guest::Guest;

pub const TAPE: u32 = 0xE100_0000;
pub const SPEC: u32 = 0xE0F0_0000;

#[derive(Clone, Debug, Default)]
struct Spec {
    nargs: usize,
    ret: u32,
    args: [u32; 12],
    regs: usize,
    /// One routine called with different arities: only the prefix the call site pushed is compared.
    variadic: bool,
    at: Vec<(usize, u32)>,
}

pub struct GoldenGuest<'a> {
    pub mem: BTreeMap<u32, u32>,
    pub case: &'a Case,
    tape: Vec<u32>,
    cursor: usize,
    specs: BTreeMap<u32, Spec>,
    next_call: usize,
    pub index: usize,
    /// Addresses answered by the ported Rust (`eff_dispatch::native`) instead of the tape: the original ran them for real.
    pub native: BTreeSet<u32>,
}

impl<'a> GoldenGuest<'a> {
    pub fn new(case: &'a Case, index: usize) -> Self {
        let mut mem = case.ins.clone();
        let mut tape = Vec::new();
        let mut address = TAPE;
        while let Some(word) = mem.remove(&address) {
            tape.push(word);
            address += 4;
        }
        let mut specs = BTreeMap::new();
        let mut base = SPEC;
        while mem.contains_key(&base) {
            let word =
                |k: u32, mem: &mut BTreeMap<u32, u32>| mem.remove(&(base + 4 * k)).unwrap_or(0);
            let va = word(0, &mut mem);
            let mut spec = Spec {
                nargs: word(1, &mut mem) as usize,
                ret: word(2, &mut mem),
                args: [0; 12],
                regs: 0,
                variadic: false,
                at: Vec::new(),
            };
            for (k, slot) in spec.args.iter_mut().enumerate() {
                *slot = word(3 + k as u32, &mut mem);
            }
            spec.regs = word(15, &mut mem) as usize;
            let pairs = word(16, &mut mem);
            for k in 0..pairs {
                let index = word(17 + 2 * k, &mut mem) as usize;
                let offset = word(18 + 2 * k, &mut mem);
                spec.at.push((index, offset));
            }
            for k in 17 + 2 * pairs..31 {
                word(k, &mut mem);
            }
            spec.variadic = word(31, &mut mem) != 0;
            specs.insert(va, spec);
            base += 128;
        }
        Self {
            mem,
            case,
            tape,
            cursor: 0,
            specs,
            next_call: 0,
            index,
            native: BTreeSet::new(),
        }
    }

    /// Answer these addresses with the ported helper family.
    pub fn with_native(mut self, vas: &[u32]) -> Self {
        self.native.extend(vas.iter().copied());
        self
    }

    fn take(&mut self) -> u32 {
        let value = *self
            .tape
            .get(self.cursor)
            .unwrap_or_else(|| panic!("case {}: tape exhausted", self.index));
        self.cursor += 1;
        value
    }

    fn engine(&mut self, va: u32, args: &[u32]) -> u32 {
        let spec = self
            .specs
            .get(&va)
            .unwrap_or_else(|| panic!("case {}: unscripted engine call {va:#x}", self.index))
            .clone();
        assert!(
            spec.variadic || args.len() == spec.nargs + spec.regs,
            "case {}: call {va:#x} with {} argument words, the original's call site has {}",
            self.index,
            args.len(),
            spec.nargs + spec.regs
        );
        let limit = if spec.variadic {
            args.len().min(spec.nargs)
        } else {
            spec.nargs
        };
        let mut flat = Vec::new();
        for k in 0..limit {
            let count = spec.args[k] & 0xFF;
            let out_only = spec.args[k] & 0x300 == 0x100;
            if spec.args[k] & 0x400 != 0 {
                flat.push(args[k]);
            }
            if out_only {
                continue;
            }
            if count > 0 {
                for j in 0..count {
                    flat.push(*self.mem.get(&(args[k] + 4 * j)).unwrap_or(&0));
                }
            } else {
                flat.push(args[k]);
            }
        }
        flat.extend_from_slice(&args[limit..]);
        let expected =
            self.case.calls.get(self.next_call).unwrap_or_else(|| {
                panic!("case {}: extra engine call {va:#x} {flat:x?}", self.index)
            });
        assert_eq!(
            (expected.callee, &expected.args),
            (va, &flat),
            "case {} call {}",
            self.index,
            self.next_call
        );
        self.next_call += 1;
        for k in 0..limit {
            if spec.args[k] & 0x100 != 0 && args[k] != 0 {
                for j in 0..(spec.args[k] & 0xFF) {
                    let word = self.take();
                    self.mem.insert(args[k] + 4 * j, word);
                }
            }
        }
        for (index, offset) in &spec.at {
            let word = self.take();
            self.mem.insert(args[*index] + offset, word);
        }
        if spec.ret == 2 { 0 } else { self.take() }
    }

    /// After the call: every logged engine call was made and every `out` word matches.
    pub fn finish(&self) {
        assert_eq!(
            self.next_call,
            self.case.calls.len(),
            "case {}: engine calls missing",
            self.index
        );
        for (address, expected) in &self.case.outs {
            let got = *self.mem.get(address).unwrap_or(&0);
            assert_eq!(got, *expected, "case {}: memory {address:#x}", self.index);
        }
    }
}

impl Guest for GoldenGuest<'_> {
    fn read(&mut self, address: u32) -> u32 {
        *self.mem.get(&address).unwrap_or(&0)
    }
    fn write(&mut self, address: u32, value: u32) {
        self.mem.insert(address, value);
    }
    fn call(&mut self, va: u32, args: &[u32]) -> u32 {
        if self.native.contains(&va) {
            if let Some(result) = eff_dispatch::native(self, va, args) {
                return result;
            }
        }
        self.engine(va, args)
    }
    fn call_f(&mut self, va: u32, args: &[u32]) -> f32 {
        f32::from_bits(self.call(va, args))
    }
}
