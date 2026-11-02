use super::*;

#[test]
fn lifted_00271910() {
    ledger_drive(
        0x271910,
        true,
        |r, p| {
            arena_state(r, p);
            p.put32(
                0x6B8384,
                [0u32, 0x100, 0x1000, 0x3000, 0xFFFF_FF00][r.below(5) as usize],
            );
            p.put32(0x6B837C, WRAP_REGION.0 + r.below(0x100));
            p.put32(0x74C3B0, 0xA5A5_A5A5);
            p.put32(0x74C3B4, 0x5A5A_5A5A);
            p.put32(0x7DE400, r.below(256) << 16);
            fast_fill(p, WRAP_REGION.0, 0x3000, 0xAA);
            (
                case(r, 0x271910, vec![]),
                vec![(WRAP_REGION.0, 0x3000), (0x74C3B0, 8), (0x7DE400, 4)],
            )
        },
        |l, h, _| {
            AllocWrappers {
                memory: &mut *l.memory,
                hooks: h,
            }
            .alloc_weather_tables_74c3b0()
            .unwrap()
        },
    );
}
