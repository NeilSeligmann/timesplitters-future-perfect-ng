// SPDX-License-Identifier: GPL-3.0-or-later
//! Scene integration boundary. Address roles are INFERRED; see docs/t-system-scene.md.
//! This adapter keeps guest state borrowed and leaves all cross-system calls to the host.
pub use tsfp_ports::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};

/// Shared guest addresses, rather than copies of the records owned by adjacent systems.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SceneAddresses {
    pub camera_pointer: u32,
    pub current_player_pointer: u32,
    pub viewport_pointer: u32,
    pub room_table_pointer: u32,
}

impl SceneAddresses {
    pub const RETAIL: Self = Self {
        camera_pointer: 0x007b_0cc0,
        current_player_pointer: 0x007b_0c48,
        viewport_pointer: 0x007e_71c8,
        room_table_pointer: 0x0070_8788,
    };
}

/// Borrowed state for one opt-in scene invocation. The host decides how to route calls
/// into object, effects, missions, math and T1898's render-device implementation.
pub struct SceneContext<'a, M: GuestMemory, C: GuestCall> {
    pub memory: &'a mut M,
    pub calls: &'a mut C,
    pub registers: &'a mut ExtRegisters,
}

impl<M: GuestMemory, C: GuestCall> SceneContext<'_, M, C> {
    /// Returns false without changing state for an address outside this slice.
    /// Retaining the original path for false makes this a per-function A/B boundary.
    pub fn run(&mut self, original_address: u32) -> bool {
        super::scene_batch::run(original_address, self.memory, self.calls, self.registers)
            || super::scene_t1938::run(original_address, self.memory, self.calls, self.registers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tsfp_ports::abi::{Fpu, Xmm};

    #[derive(Default)]
    struct Memory(BTreeMap<u32, u8>);
    impl GuestMemory for Memory {
        fn read8(&self, address: u32) -> u8 {
            *self.0.get(&address).unwrap_or(&0)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.insert(address, value);
        }
    }
    #[derive(Default)]
    struct Calls(Vec<u32>);
    impl GuestCall for Calls {
        fn call(&mut self, va: u32, _: &mut ExtRegisters) {
            self.0.push(va);
        }
    }
    fn registers() -> ExtRegisters {
        ExtRegisters {
            eax: 42,
            ecx: 43,
            edx: 44,
            esp: 0x1000,
            ebx: 45,
            ebp: 46,
            esi: 47,
            edi: 48,
            eflags: 0x202,
            fpu: Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0x37f,
            },
            xmm: [Xmm::default(); 5],
            xmm_hi: [Xmm::default(); 3],
        }
    }
    #[test]
    fn room_flag_query_and_original_fallback_share_borrowed_state() {
        let mut memory = Memory::default();
        let mut calls = Calls::default();
        let mut regs = registers();
        let original = regs;
        let mut context = SceneContext {
            memory: &mut memory,
            calls: &mut calls,
            registers: &mut regs,
        };
        assert!(!context.run(0x18ea90)); // T1898 retains this root.
        assert_eq!(*context.registers, original);
        assert!(context.memory.0.is_empty());
        assert!(context.run(0x83790));
        assert_eq!(context.registers.eax, 1); // no active level
        context.memory.write32(0x7b9778, 1);
        context.memory.write32(0x1004, 0x908);
        context.memory.write32(0x7087a4, 0x2000);
        context.memory.write32(0x2000 + 0x908 * 24, 0x82);
        assert!(context.run(0x83790));
        assert_eq!(context.registers.eax, 2);
        context.memory.write32(0x2000 + 0x908 * 24, 0x80);
        assert!(context.run(0x83790));
        assert_eq!(context.registers.eax, 0);
        assert_eq!(context.registers.esp, original.esp);
        assert_eq!(context.registers.eflags, original.eflags);
        assert!(context.calls.0.is_empty());
    }
    #[test]
    fn fault_only_fixture_roots_have_explicit_rust_smoke_cases() {
        struct ZeroCalls(Vec<u32>);
        impl GuestCall for ZeroCalls {
            fn call(&mut self, va: u32, r: &mut ExtRegisters) {
                self.0.push(va);
                r.eax = if va == 0x624c0 { 0x2000 } else { 0 };
                // FABRICATED cdecl host stub, not a production implementation.
            }
        }
        let mut memory = Memory::default();
        memory.write32(0x2000, 4); // record-relative list pointer
        memory.write32(0x2004, u32::MAX); // empty texture list terminator
        let mut calls = ZeroCalls(Vec::new());
        let mut regs = registers();
        let original_esp = regs.esp;
        let mut context = SceneContext {
            memory: &mut memory,
            calls: &mut calls,
            registers: &mut regs,
        };
        assert!(context.run(0x8e4b0));
        assert_eq!(context.registers.esp, original_esp);
        assert_eq!(context.calls.0.last(), Some(&0x8e420));

        let mut memory = Memory::default(); // zero items and zero-sized bounds
        let mut calls = ZeroCalls(Vec::new());
        let mut regs = registers();
        let original_esp = regs.esp;
        let mut context = SceneContext {
            memory: &mut memory,
            calls: &mut calls,
            registers: &mut regs,
        };
        assert!(context.run(0x157970));
        assert_eq!(context.registers.esp, original_esp);
        assert!(context.calls.0.contains(&0x157600));
        assert_eq!(context.memory.read32(0x74a9cc), 0);
    }
    #[test]
    fn t1938_roots_are_disjoint_from_t1929_and_dispatch_through_the_boundary() {
        for root in super::super::scene_t1938::ROOTS {
            assert!(!super::super::scene_batch::ROOTS.contains(&root), "{root:#x} is in both batches");
        }
        let mut memory = Memory::default();
        let mut calls = Calls::default();
        let mut regs = registers();
        let original_esp = regs.esp;
        let mut context = SceneContext {
            memory: &mut memory,
            calls: &mut calls,
            registers: &mut regs,
        };
        // 0x1164a0 returns the address of a 40-byte entry behind the table pointer at 0x73d96c.
        context.memory.write32(0x73d96c, 0x2000);
        context.memory.write32(original_esp + 4, 3);
        assert!(context.run(0x1164a0));
        assert_eq!(context.registers.eax, 0x2000 + 3 * 40);
        // 0x87a70 (recovered from T1929's divergence list) clips nothing when the input polygon has no vertices.
        context.registers.esi = 0x3000;
        context.registers.edi = 0x4000;
        context.memory.write32(0x4000 + 0x194, 7);
        assert!(context.run(0x87a70));
        assert_eq!(context.memory.read32(0x4000 + 0x194), 0);
        assert_eq!(context.registers.esp, original_esp);
        assert!(context.calls.0.is_empty());
    }
}
