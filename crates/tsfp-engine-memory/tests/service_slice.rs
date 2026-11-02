//! T1908 Tier 0 specification smoke checks, not lifted differential evidence.
use tsfp_engine_memory::{
    GuestMemory, MappedMemory,
    service_slice::{GameAllocator, MemoryServices},
    w4m_arena::Arena,
};
use tsfp_types::GuestAddr;
#[path = "support/arena_hooks.rs"]
mod arena_hooks;
use arena_hooks::Hooks;
fn put(m: &mut MappedMemory<'_>, a: u32, v: u32) {
    m.write_u32(GuestAddr(a), v).unwrap();
}
fn get(m: &mut MappedMemory<'_>, a: u32) -> u32 {
    m.read_u32(GuestAddr(a)).unwrap()
}
#[test]
fn arena_init_alignment_dispatch_and_secondary_ledger() {
    let mut bytes = vec![0; 0x800000];
    let mut m = MappedMemory {
        base: GuestAddr(0),
        bytes: &mut bytes,
    };
    put(&mut m, 0x4c81b4, 1);
    MemoryServices { memory: &mut m }
        .initialize_arena(0x10001, 0x300000)
        .unwrap();
    assert_eq!(get(&mut m, 0x6b836c), 0x310001 - 0x266000);
    put(&mut m, 0x6b8390, 0x180003);
    let mut h = Hooks::default();
    {
        let mut a = Arena {
            memory: &mut m,
            hooks: &mut h,
        };
        assert_eq!(a.allocate(3, 32).unwrap(), 0);
        assert_eq!(a.allocate(4, 1).unwrap(), 0x180003);
        assert_eq!(a.allocate_aligned(64, 1, 4).unwrap(), 0x180040);
        assert_eq!(a.allocate_aligned(32, 17, 1).unwrap(), 0xadc20);
        assert_eq!(a.allocate_aligned(32, 17, 2).unwrap(), 0x10020);
        let before = a.memory.read_u32(GuestAddr(0x6b8384)).unwrap();
        let p = a.allocate_aligned(64, 17, 0).unwrap();
        assert_eq!(p & 63, 0);
        a.release(p).unwrap();
        assert_eq!(a.memory.read_u32(GuestAddr(0x6b8384)).unwrap(), before);
        assert_eq!(a.allocate_aligned(32, 17, 3).unwrap(), 0);
    }
    let p = MemoryServices { memory: &mut m }
        .append_secondary(13)
        .unwrap();
    assert_eq!(p, 0x310001);
    assert_eq!(get(&mut m, 0x6b7b18), p);
    assert_eq!(get(&mut m, 0x6b7b1c), 13);
    assert_eq!(get(&mut m, 0x6b838c), 1);
    assert!(h.0.is_empty());
}

#[test]
fn arena_zero_and_overflow_requests_keep_wrapping_contract() {
    let mut bytes = vec![0; 0x800000];
    let mut m = MappedMemory {
        base: GuestAddr(0),
        bytes: &mut bytes,
    };
    let mut h = Hooks::default();
    put(&mut m, 0x6b837c, 0x1000);
    put(&mut m, 0x6b8384, 0x100);
    put(&mut m, 0x6b8390, 0x2000);
    {
        let mut a = Arena {
            memory: &mut m,
            hooks: &mut h,
        };
        // Kind 1 executes its normal cursor path for a zero-sized request.
        assert_eq!(a.allocate(1, 0).unwrap(), 0x1000);
        assert_eq!(a.memory.read_u32(GuestAddr(0x6b837c)).unwrap(), 0x1000);
        // (u32::MAX + 15) wraps to 14 before the 16-byte mask, producing zero.
        assert_eq!(a.allocate(1, u32::MAX).unwrap(), 0x1000);
        assert_eq!(a.memory.read_u32(GuestAddr(0x6b837c)).unwrap(), 0x1000);
        // Kind 4 exposes the same wrapped size through its unchecked cursor.
        assert_eq!(a.allocate_aligned(1, u32::MAX, 4).unwrap(), 0x2000);
        assert_eq!(a.memory.read_u32(GuestAddr(0x6b8390)).unwrap(), 0x2000);
    }
    assert!(h.0.is_empty());
}

#[test]
fn pool_reset_permanent_split_coalesce_and_retention_gate() {
    let mut bytes = vec![0; 0x800000];
    let mut m = MappedMemory {
        base: GuestAddr(0),
        bytes: &mut bytes,
    };
    put(&mut m, 0x4c81b8, 0x400);
    put(&mut m, 0x6b83c8, 0x10000);
    MemoryServices { memory: &mut m }.reset_pool().unwrap();
    assert_eq!(get(&mut m, 0x6b83d0), 0x10000); // original publishes base, not zero
    assert_eq!(
        MemoryServices { memory: &mut m }
            .allocate_pool_permanent(1)
            .unwrap(),
        0x10020
    );
    assert_eq!(get(&mut m, 0x6b83cc), 0x10040);
    assert_eq!(get(&mut m, 0x6b83d0), 0x10040);
    assert_eq!(get(&mut m, 0x6b83c0), 0x3c0);
    assert_eq!(
        MemoryServices { memory: &mut m }
            .allocate_pool_permanent(0x400)
            .unwrap(),
        0
    );
    // Two adjacent free blocks and one reclaimable middle block.
    for (a, v) in [
        (0x20000, 0),
        (0x20004, 64),
        (0x20008, 0),
        (0x20040, 1),
        (0x20044, 64),
        (0x20048, 64),
        (0x20080, 0),
        (0x20084, 64),
        (0x6b83d4, 0x200c0),
        (0x6b83cc, 0x20080),
        (0x6b83c0, 128),
    ] {
        put(&mut m, a, v);
    }
    MemoryServices { memory: &mut m }
        .free_pool_nullable(0x20060)
        .unwrap();
    assert_eq!(get(&mut m, 0x20004), 192);
    assert_eq!(get(&mut m, 0x6b83cc), 0x20000);
    assert_eq!(get(&mut m, 0x6b83c0), 192);
    MemoryServices { memory: &mut m }
        .free_pool_nullable(0)
        .unwrap();
    MemoryServices { memory: &mut m }
        .free_pool(0x10020)
        .unwrap(); // permanent block retained
    assert_eq!(get(&mut m, 0x10000), 2);
    put(&mut m, 0x6b7aa8, 1);
    MemoryServices { memory: &mut m }.clear_pool().unwrap();
    assert_eq!(get(&mut m, 0x6b83c8), 0x10000);
    put(&mut m, 0x6b7aa8, 0);
    MemoryServices { memory: &mut m }.clear_pool().unwrap();
    assert_eq!(get(&mut m, 0x6b83c8), 0);
    assert_eq!(get(&mut m, 0x6b83cc), 0);
}

#[test]
fn pool_free_ignores_every_state_except_exactly_one_without_mutation() {
    let mut bytes = vec![0; 0x800000];
    let mut m = MappedMemory {
        base: GuestAddr(0),
        bytes: &mut bytes,
    };
    for state in [0, 2, 3, u32::MAX] {
        for (address, value) in [
            (0x200c0, 0),
            (0x200c4, 64),
            (0x200c8, 0),
            (0x20100, state),
            (0x20104, 64),
            (0x20108, 64),
            (0x2010c, 0x1234),
            (0x20110, 0x5678),
            (0x20140, 0),
            (0x20144, 64),
            (0x20148, 64),
            (0x6b83c0, 0x80),
            (0x6b83cc, 0x20100),
            (0x6b83d4, 0x20200),
        ] {
            put(&mut m, address, value);
        }
        let before = [
            0x200c4, 0x20100, 0x20104, 0x20108, 0x2010c, 0x20110, 0x20144, 0x20148, 0x6b83c0,
            0x6b83cc,
        ]
        .map(|address| get(&mut m, address));

        MemoryServices { memory: &mut m }
            .free_pool(0x20120)
            .unwrap();

        let after = [
            0x200c4, 0x20100, 0x20104, 0x20108, 0x2010c, 0x20110, 0x20144, 0x20148, 0x6b83c0,
            0x6b83cc,
        ]
        .map(|address| get(&mut m, address));
        assert_eq!(
            after, before,
            "state {state:#x} must not mutate pool metadata"
        );
    }
}

#[test]
fn crt_copy_obeys_rea_overlap_direction() {
    use tsfp_engine_memory::crt_memmove::move_payload;

    let mut bytes = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
    let mut memory = MappedMemory {
        base: GuestAddr(0x1000),
        bytes: &mut bytes,
    };
    move_payload(&mut memory, 0x1004, 0x1000, 8).unwrap();
    assert_eq!(
        memory.bytes,
        &[0, 1, 2, 3, 0, 1, 2, 3, 4, 5, 6, 7, 12, 13, 14, 15]
    );

    memory
        .bytes
        .copy_from_slice(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
    move_payload(&mut memory, 0x1000, 0x1004, 8).unwrap();
    assert_eq!(
        memory.bytes,
        &[4, 5, 6, 7, 8, 9, 10, 11, 8, 9, 10, 11, 12, 13, 14, 15]
    );
}

#[test]
fn crt_copy_preserves_exact_short_tail_extent() {
    use tsfp_engine_memory::crt_memmove::move_payload;

    for count in 0..=7 {
        let initial = (0..32).map(|value| value as u8).collect::<Vec<_>>();
        let mut bytes = initial.clone();
        let mut memory = MappedMemory {
            base: GuestAddr(0x1000),
            bytes: &mut bytes,
        };
        move_payload(&mut memory, 0x1009, 0x1002, count).unwrap();
        let destination = 9usize;
        let source = 2usize;
        for index in 0..count as usize {
            assert_eq!(
                memory.bytes[destination + index],
                initial[source + index],
                "count={count}, copied byte={index}"
            );
        }
        for index in 0..memory.bytes.len() {
            if index < destination || index >= destination + count as usize {
                assert_eq!(
                    memory.bytes[index], initial[index],
                    "count={count}, untouched byte={index}"
                );
            }
        }
    }
}

#[test]
fn scratch_signed_high_water_slots_and_fault_prefix() {
    let mut bytes = vec![0; 0x800000];
    let mut m = MappedMemory {
        base: GuestAddr(0),
        bytes: &mut bytes,
    };
    for (a, v) in [
        (0x6b839c, 0x1020),
        (0x6b83a0, 0x1000),
        (0x6b83a4, 0x2000),
        (0x6b83b0, 16),
    ] {
        put(&mut m, a, v);
    }
    MemoryServices { memory: &mut m }.swap_scratch().unwrap();
    assert_eq!(get(&mut m, 0x6b83b0), 32);
    assert_eq!(get(&mut m, 0x6b839c), 0x2000);
    put(&mut m, 0x6b839c, 0x1fff);
    MemoryServices { memory: &mut m }.swap_scratch().unwrap();
    assert_eq!(get(&mut m, 0x6b83b0), 32); // -1 must not win unsigned comparison
    for i in 0..17 {
        assert_eq!(
            MemoryServices { memory: &mut m }.allocate_slot().unwrap(),
            0x745938 + i * 0x144
        );
    }
    assert_eq!(
        MemoryServices { memory: &mut m }.allocate_slot().unwrap(),
        0
    );
    put(&mut m, 0x6b838c, 0x200000); // invalid ledger destination: preceding counter update survives
    assert!(
        MemoryServices { memory: &mut m }
            .append_secondary(7)
            .is_err()
    );
    assert_eq!(get(&mut m, 0x6b8368), 7);
}
