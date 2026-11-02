// SPDX-License-Identifier: GPL-3.0-or-later
//! Readable player-interaction logic owned by the gameplay-player family crate.

use crate::c2r::rt::RegsExt as _;
use tsfp_port_runtime::abi::{ExtRegisters, GuestCall};
use tsfp_port_runtime::memory::GuestMemory;

const ROOT_CFFA0: u32 = 0x000c_ffa0;
const ROOT_CFF20: u32 = 0x000c_ff20;
const ROOT_LIST_CONTAINS: u32 = 0x0009_e6f0;
const TARGETING_ROSTER_BASE: u32 = 0x0071_6148;
const TARGETING_ROSTER_COUNT: u32 = 0x007b_625c;

/// Typed view of the active targeting roster used by the D00F0 predicate.
///
/// The entry values and query remain opaque 32-bit words: the REA contract does not
/// identify them as object or player identities. The count and fixed table address are
/// the observed guest ABI for 0x9E6F0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TargetingRosterView {
    count: i32,
}

impl TargetingRosterView {
    pub const fn from_count_word(count: u32) -> Self {
        Self {
            count: count as i32,
        }
    }

    pub const fn count(self) -> i32 {
        self.count
    }

    pub fn contains<M: GuestMemory>(self, memory: &M, query: u32) -> bool {
        if self.count <= 0 {
            return false;
        }
        (0..self.count as u32).any(|index| {
            memory.read32(TARGETING_ROSTER_BASE.wrapping_add(index.wrapping_mul(4))) == query
        })
    }
}

/// Readable native implementation of the 0x9E6F0 membership leaf.
///
/// It preserves the original callee's ECX count, EDX query (when count is positive),
/// EAX result, and leaves memory/stack untouched.
pub fn run_targeting_roster_contains<M: GuestMemory>(memory: &M, regs: &mut ExtRegisters) {
    let query = memory.read32(regs.esp.wrapping_add(4));
    run_targeting_roster_contains_query(memory, regs, query);
}

fn run_targeting_roster_contains_query<M: GuestMemory>(
    memory: &M,
    regs: &mut ExtRegisters,
    query: u32,
) {
    let roster = TargetingRosterView::from_count_word(memory.read32(TARGETING_ROSTER_COUNT));
    regs.ecx = roster.count() as u32;
    regs.eax = 0;
    if roster.count() > 0 {
        regs.edx = query;
        regs.eax = u32::from(roster.contains(memory, query));
    }
}

/// Route the one interaction helper already covered by a readable Rust contract; all other
/// helper calls continue through the guest bridge.
pub fn dispatch_candidate_helper<M: GuestMemory, C: GuestCall>(
    va: u32,
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) {
    if va != ROOT_LIST_CONTAINS {
        guest.call(va, regs);
        return;
    }
    // This dispatcher is called before GuestCall supplies its return slot; the single
    // argument is therefore at ESP, matching the other nested helper seam.
    let query = memory.read32(regs.esp);
    run_targeting_roster_contains_query(memory, regs, query);
}

/// Reduce the three CFFA0 record-kind values to the D0090 maximum.
///
/// REA's static contract identifies D0090 as the max over kinds 0..2 and the lifted body
/// supplies the exact call order and x87 comparison behavior. Record/kind meaning remains
/// opaque; CFFA0 is still the reference helper behind `GuestCall`.
pub fn run_nested_candidate_limit<M: GuestMemory, C: GuestCall>(
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) {
    // GuestCall enters at the callee boundary: ESP addresses the caller's return slot and
    // D0090's record argument is at ESP+4.
    let record = memory.read32(regs.esp.wrapping_add(4));
    regs.esp = regs.esp.wrapping_sub(8);
    // The lifted path loads the shared 1.0f guest constant at 0x475c78.
    regs.xmm[0] = tsfp_port_runtime::abi::Xmm::scalar(memory.read32(0x0047_5c78));
    regs.push(memory, regs.esi);
    regs.push(memory, regs.edi);
    regs.edi = record;
    memory.write32(regs.esp.wrapping_add(8), regs.xmm[0].lanes[0]);

    for kind in 0..3u32 {
        regs.push(memory, kind);
        regs.push(memory, regs.edi);
        regs.call(guest, ROOT_CFFA0);
        memory.write32(
            regs.esp.wrapping_add(0x14),
            (regs.fpu.st(0) as f32).to_bits(),
        );
        regs.fpu
            .push(f32::from_bits(memory.read32(regs.esp.wrapping_add(0x10))) as f64);
        regs.release_stack(8);
        let comparison = crate::c2r::rt::fcmp(regs.fpu.st(0), regs.fpu.st(1));
        regs.fpu.pop();
        regs.fpu.pop();
        if comparison == 1 {
            regs.xmm[0] =
                tsfp_port_runtime::abi::Xmm::scalar(memory.read32(regs.esp.wrapping_add(0x0c)));
            memory.write32(regs.esp.wrapping_add(8), regs.xmm[0].lanes[0]);
        }
    }

    regs.fpu
        .push(f32::from_bits(memory.read32(regs.esp.wrapping_add(8))) as f64);
    regs.edi = regs.pop(memory);
    regs.esi = regs.pop(memory);
    regs.release_stack(8);
    // Preserve the translated GuestCall stack effects from the three opaque CFFA0 calls.
}

/// Dispatch the readable nested player-interaction roots before the lifted guest call.
pub fn dispatch_nested<M, C, F>(
    va: u32,
    interaction_enabled: bool,
    memory: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    mut cff20: F,
) -> bool
where
    M: GuestMemory,
    C: GuestCall,
    F: FnMut(u32, &mut M, &mut C, &mut ExtRegisters) -> bool,
{
    match va {
        0x000d_0090 if interaction_enabled => {
            run_nested_candidate_limit(memory, guest, regs);
            true
        }
        ROOT_CFF20 => cff20(va, memory, guest, regs),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    struct Memory(BTreeMap<u32, u8>);

    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            self.0.get(&address).copied().unwrap_or(0)
        }

        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }

    #[test]
    fn targeting_roster_membership_matches_lifted_leaf_abi() {
        for (count, entries, query) in [
            (0, [0x11, 0x22, 0x33], 0x22),
            (3, [0x11, 0x22, 0x33], 0x22),
            (3, [0x11, 0x22, 0x33], 0x44),
            (0x8000_0000, [0x11, 0x22, 0x33], 0x11),
        ] {
            let mut original = Memory::default();
            original.write32(TARGETING_ROSTER_COUNT, count);
            for (index, value) in entries.into_iter().enumerate() {
                original.write32(TARGETING_ROSTER_BASE + index as u32 * 4, value);
            }
            let stack = 0x10000;
            original.write32(stack + 4, query);
            let mut native = original.clone();
            let mut native_regs = ExtRegisters {
                esp: stack,
                eax: 0xaabb_ccdd,
                ecx: 0x1234_5678,
                edx: 0x8765_4321,
                ..unsafe { core::mem::zeroed() }
            };
            let mut lifted_regs = native_regs;

            // Explicit oracle for lifted 0x9E6F0: signed-positive count, linear
            // search, and the translated leaf's ECX/EDX/EAX effects.
            lifted_regs.ecx = original.read32(TARGETING_ROSTER_COUNT);
            lifted_regs.eax = 0;
            if (lifted_regs.ecx as i32) > 0 {
                lifted_regs.edx = original.read32(stack + 4);
                while lifted_regs.eax < lifted_regs.ecx {
                    if original.read32(TARGETING_ROSTER_BASE + lifted_regs.eax * 4)
                        == lifted_regs.edx
                    {
                        lifted_regs.eax = 1;
                        break;
                    }
                    lifted_regs.eax += 1;
                }
                if lifted_regs.eax != 1 {
                    lifted_regs.eax = 0;
                }
            }
            run_targeting_roster_contains(&native, &mut native_regs);

            assert_eq!(native, original, "count {count:#x}, query {query:#x}");
            assert_eq!(
                native_regs, lifted_regs,
                "count {count:#x}, query {query:#x}"
            );
        }
    }
}
