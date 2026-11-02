use super::*;
use tsfp_engine_memory::GuestMemory;
use tsfp_types::GuestAddr;

/// Tier 1 for 0x188900's memory seam: compare its Rust aligned allocation/publication path
/// against the lifted 0x3E0B0 allocator. The adjacent 0x188870 initializer is intentionally
/// outside this allocator fixture; retained REA notes do not establish its runtime outcome.
#[test]
fn lifted_00188900_allocator() {
    const SENTINEL: u32 = 0xA5A5_A5A5;
    ledger_drive(
        0x3E0B0,
        true,
        |r, p| {
            arena_state(r, p);
            p.put32(
                0x6B8384,
                [0u32, 0x100, 0x1A0, 0x200, 0x400, 0xFFFF_FF00][r.below(6) as usize],
            );
            p.put32(0x6B837C, WRAP_REGION.0 + r.below(0x100));
            p.put32(0x7A156C, SENTINEL);
            fast_fill(p, WRAP_REGION.0, 0x1000, 0xAA);
            (
                case(r, 0x3E0B0, vec![0x20, 0x194, 1]),
                vec![(WRAP_REGION.0, 0x1000), (0x7A156C, 4)],
            )
        },
        |l, h, _| {
            let block = AllocWrappers {
                memory: &mut *l.memory,
                hooks: h,
            }
            .alloc_effect_slots_7a156c()
            .unwrap();
            assert_eq!(
                l.memory.read_u32(GuestAddr(0x7A156C)).unwrap(),
                block,
                "the wrapper publishes the allocator result before its neighboring initializer"
            );
            l.memory.write_u32(GuestAddr(0x7A156C), SENTINEL).unwrap();
            block
        },
    );
}
