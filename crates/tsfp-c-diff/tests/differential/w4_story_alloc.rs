use super::*;
use tsfp_engine_memory::GuestMemory;
use tsfp_types::GuestAddr;

/// Tier 1 for the dynamic Story event-array allocation in lifted function 0x237480.
/// The lifted 0x3DFA0 allocator is the oracle; 0x235630 remains a separate initializer.
#[test]
fn lifted_story_event_array_allocator() {
    const SENTINEL: u32 = 0xA5A5_A5A5;
    ledger_drive(
        0x3DFA0,
        true,
        |r, p| {
            let instance_count = r.below(5);
            let size = instance_count * 0x48;
            arena_state(r, p);
            p.put32(
                0x6B8384,
                [0u32, 0x40, 0x48, 0x80, 0x100, 0xFFFF_FF00][r.below(6) as usize],
            );
            p.put32(0x6B837C, WRAP_REGION.0 + r.below(0x100));
            p.put32(0x78CD90, SENTINEL);
            fast_fill(p, WRAP_REGION.0, 0x1000, 0xAA);
            (
                case(r, 0x3DFA0, vec![size, 1]),
                vec![(WRAP_REGION.0, 0x1000), (0x78CD90, 4)],
            )
        },
        |l, h, size| {
            let block = AllocWrappers {
                memory: &mut *l.memory,
                hooks: h,
            }
            .alloc_story_event_instances_78cd90(size)
            .unwrap();
            assert_eq!(
                l.memory.read_u32(GuestAddr(0x78CD90)).unwrap(),
                block,
                "the allocator wrapper publishes its result before the event initializer"
            );
            l.memory.write_u32(GuestAddr(0x78CD90), SENTINEL).unwrap();
            block
        },
    );
}

/// Tier 1 for the Story active-instance index list, with the lifted arena allocator as oracle.
#[test]
fn lifted_story_event_active_index_allocator() {
    const SENTINEL: u32 = 0xA5A5_A5A5;
    ledger_drive(
        0x3DFA0,
        true,
        |r, p| {
            let size = r.below(5) * 4;
            arena_state(r, p);
            p.put32(
                0x6B8384,
                [0u32, 0x40, 0x48, 0x80, 0x100, 0xFFFF_FF00][r.below(6) as usize],
            );
            p.put32(0x6B837C, WRAP_REGION.0 + r.below(0x100));
            p.put32(0x78CD8C, SENTINEL);
            fast_fill(p, WRAP_REGION.0, 0x1000, 0xAA);
            (
                case(r, 0x3DFA0, vec![size, 1]),
                vec![(WRAP_REGION.0, 0x1000), (0x78CD8C, 4)],
            )
        },
        |l, h, size| {
            let block = AllocWrappers {
                memory: &mut *l.memory,
                hooks: h,
            }
            .alloc_story_event_indices_78cd8c(size)
            .unwrap();
            assert_eq!(l.memory.read_u32(GuestAddr(0x78CD8C)).unwrap(), block);
            l.memory.write_u32(GuestAddr(0x78CD8C), SENTINEL).unwrap();
            block
        },
    );
}

/// Tier 1 for the Story snapshot buffer, with the lifted arena allocator as oracle.
#[test]
fn lifted_story_event_snapshot_allocator() {
    const SENTINEL: u32 = 0xA5A5_A5A5;
    ledger_drive(
        0x3DFA0,
        true,
        |r, p| {
            let size = r.below(5) * 16;
            arena_state(r, p);
            p.put32(
                0x6B8384,
                [0u32, 0x40, 0x48, 0x80, 0x100, 0xFFFF_FF00][r.below(6) as usize],
            );
            p.put32(0x6B837C, WRAP_REGION.0 + r.below(0x100));
            p.put32(0x75CCA4, SENTINEL);
            fast_fill(p, WRAP_REGION.0, 0x1000, 0xAA);
            (
                case(r, 0x3DFA0, vec![size, 1]),
                vec![(WRAP_REGION.0, 0x1000), (0x75CCA4, 4)],
            )
        },
        |l, h, size| {
            let block = AllocWrappers {
                memory: &mut *l.memory,
                hooks: h,
            }
            .alloc_story_event_snapshot_75cca4(size)
            .unwrap();
            assert_eq!(l.memory.read_u32(GuestAddr(0x75CCA4)).unwrap(), block);
            l.memory.write_u32(GuestAddr(0x75CCA4), SENTINEL).unwrap();
            block
        },
    );
}
