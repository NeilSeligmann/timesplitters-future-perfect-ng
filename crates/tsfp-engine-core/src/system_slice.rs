// SPDX-License-Identifier: GPL-3.0-or-later
//! T1901 standalone candidate boundary. No host registration or default admission.
//! Arguments use logical callee order, not x86 push order. Register inputs are explicit.

pub trait Memory {
    fn read8(&self, address: u32) -> u8;
    fn write8(&mut self, address: u32, value: u8);
    fn read16(&self, address: u32) -> u16 {
        u16::from_le_bytes([self.read8(address), self.read8(address.wrapping_add(1))])
    }
    fn read32(&self, address: u32) -> u32 {
        u32::from_le_bytes(core::array::from_fn(|i| {
            self.read8(address.wrapping_add(i as u32))
        }))
    }
    fn write32(&mut self, address: u32, value: u32) {
        for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.write8(address.wrapping_add(i as u32), byte);
        }
    }
    fn read_float(&self, address: u32) -> f32 {
        f32::from_bits(self.read32(address))
    }
    fn write_float(&mut self, address: u32, value: f32) {
        self.write32(address, value.to_bits());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Register {
    Eax,
    Edi,
    Esi,
}

/// Implementations execute real neighbouring services, retain their memory mutations,
/// and fail explicitly on unresolved targets. Never synthesize success or zero returns.
/// These behavioural candidates do not promise volatile-register or stack-byte equivalence.
pub trait Services: Memory {
    fn invoke(&mut self, target: u32, args: &[u32], registers: &[(Register, u32)]) -> u32;
    fn call(&mut self, target: u32, args: &[u32]) -> u32 {
        self.invoke(target, args, &[])
    }
    fn register_call(&mut self, target: u32, register: Register, value: u32) -> u32 {
        self.invoke(target, &[], &[(register, value)])
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FunctionTier {
    pub address: u32,
    pub tier: u8,
    pub stack_cleanup: u16,
}

/// Per-function A/B control; all functions retain the original path by default.
/// The embedding host must route on this selector before invoking a candidate.
#[derive(Default, Debug)]
pub struct Selection {
    enabled: std::collections::BTreeSet<u32>,
}
impl Selection {
    pub fn set_candidate(&mut self, address: u32, enabled: bool) {
        if enabled {
            self.enabled.insert(address);
        } else {
            self.enabled.remove(&address);
        }
    }
    pub fn candidate_enabled(&self, address: u32) -> bool {
        self.enabled.contains(&address)
    }
}

#[cfg(test)]
pub mod test_support {
    use super::*;
    use std::collections::{BTreeMap, VecDeque};
    #[derive(Debug, PartialEq, Eq)]
    pub struct Call {
        pub target: u32,
        pub args: Vec<u32>,
        pub registers: Vec<(Register, u32)>,
    }
    #[derive(Default)]
    pub struct Host {
        pub bytes: BTreeMap<u32, u8>,
        pub calls: Vec<Call>,
        pub returns: BTreeMap<u32, VecDeque<u32>>,
        pub mutations: BTreeMap<u32, VecDeque<Vec<(u32, u32)>>>,
    }
    impl Memory for Host {
        fn read8(&self, a: u32) -> u8 {
            *self.bytes.get(&a).unwrap_or(&0)
        }
        fn write8(&mut self, a: u32, v: u8) {
            self.bytes.insert(a, v);
        }
    }
    impl Services for Host {
        fn invoke(&mut self, t: u32, args: &[u32], registers: &[(Register, u32)]) -> u32 {
            self.calls.push(Call {
                target: t,
                args: args.to_vec(),
                registers: registers.to_vec(),
            });
            if let Some(changes) = self.mutations.get_mut(&t).and_then(VecDeque::pop_front) {
                for (a, v) in changes {
                    self.write32(a, v);
                }
            }
            self.returns
                .get_mut(&t)
                .and_then(VecDeque::pop_front)
                .unwrap_or(0)
        }
    }
    /// FNV64 over target/arity/arguments matches the independent Unicorn smoke.
    pub fn call_digest(calls: &[Call]) -> u64 {
        let mut hash = 0xcbf29ce484222325u64;
        for call in calls {
            for word in [call.target, call.args.len() as u32]
                .into_iter()
                .chain(call.args.iter().copied())
            {
                for byte in word.to_le_bytes() {
                    hash = (hash ^ byte as u64).wrapping_mul(0x100000001b3);
                }
            }
        }
        hash
    }
    #[test]
    fn selection_is_per_function_and_defaults_to_original() {
        let mut s = Selection::default();
        assert!(!s.candidate_enabled(0x3127d0));
        s.set_candidate(0x3127d0, true);
        assert!(s.candidate_enabled(0x3127d0));
        assert!(!s.candidate_enabled(0x3127f0));
        s.set_candidate(0x3127d0, false);
        assert!(!s.candidate_enabled(0x3127d0));
    }
}
