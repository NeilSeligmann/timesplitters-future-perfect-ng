// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 vs unchanged C host handlers, FABRICATED seeded inputs; no Xbox evidence.
use tsfp_engine_memory::{
    Access, GuestMemory, MappedMemory, MemoryFault,
    kernel_rtl::{CountedString, KernelRtl, TimeFields},
};
use tsfp_types::GuestAddr;
unsafe extern "C" {
    fn shim_init() -> u32;
    fn memshim_call(ordinal: u32, args: *const u32, count: u32) -> u32;
    fn shim_statistics_host(total: u64, free: u64, unit: u32, image: u64, failed: i32, writes: i32);
    fn kernel_rtl_reset();
    fn kernel_rtl_string_register() -> usize;
    fn kernel_rtl_truncated_count() -> u32;
    fn kernel_rtl_nonascii_fold_count() -> u32;
    fn kernel_rtl_last_nonascii_fold() -> u8;
    fn kernel_rtl_string_nonascii_count() -> u32;
    fn kernel_rtl_compare_refused_count() -> u32;
    fn kernel_rtl_time_refused_count() -> u32;
    fn kernel_rtl_unmapped_count() -> u32;
    fn kernel_rtl_last_unmapped() -> u32;
    fn mmap(
        address: *mut u8,
        length: usize,
        protection: i32,
        flags: i32,
        fd: i32,
        offset: i64,
    ) -> *mut u8;
    fn munmap(address: *mut u8, length: usize) -> i32;
}
struct Memory<'a> {
    inner: MappedMemory<'a>,
    writes: i32,
}
impl GuestMemory for Memory<'_> {
    fn read(&mut self, address: GuestAddr, bytes: &mut [u8]) -> Result<(), MemoryFault> {
        if address.0 == 0 {
            return Err(MemoryFault {
                address,
                length: bytes.len(),
                access: Access::Read,
            });
        }
        self.inner.read(address, bytes)
    }
    fn write(&mut self, address: GuestAddr, bytes: &[u8]) -> Result<(), MemoryFault> {
        if self.writes == 0 {
            return Err(MemoryFault {
                address,
                length: bytes.len(),
                access: Access::Write,
            });
        }
        if self.writes > 0 {
            self.writes -= 1;
        }
        self.inner.write(address, bytes)
    }
}
fn next(seed: &mut u64) -> u32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed as u32
}
fn descriptor(bytes: &mut [u8], offset: usize, length: u16, capacity: u16, buffer: u32) {
    bytes[offset..offset + 8].copy_from_slice(
        &CountedString {
            length,
            capacity,
            buffer: GuestAddr(buffer),
        }
        .bytes(),
    );
}
fn compare(base: u32, initial: &[u8], ordinal: u32, args: &[u32], writes: i32) {
    unsafe {
        kernel_rtl_reset();
        kernel_rtl_string_register();
        shim_statistics_host(0, 0, 0, 0, 0, writes);
        std::ptr::copy_nonoverlapping(initial.as_ptr(), base as *mut u8, initial.len());
    }
    let c = unsafe { memshim_call(ordinal, args.as_ptr(), args.len() as u32) };
    let expected = unsafe { std::slice::from_raw_parts(base as *const u8, initial.len()) }.to_vec();
    let mut actual = initial.to_vec();
    let mut rtl = KernelRtl::default();
    let r = rtl.dispatch(
        &mut Memory {
            inner: MappedMemory {
                base: GuestAddr(base),
                bytes: &mut actual,
            },
            writes,
        },
        ordinal,
        args,
    );
    assert_eq!(
        r,
        Some(c),
        "ordinal {ordinal} args {args:?} writes {writes}"
    );
    assert_eq!(
        actual, expected,
        "memory ordinal {ordinal} args {args:?} writes {writes}"
    );
    unsafe {
        assert_eq!(rtl.truncated, kernel_rtl_truncated_count());
        assert_eq!(rtl.nonascii_folds, kernel_rtl_nonascii_fold_count());
        assert_eq!(rtl.last_nonascii, kernel_rtl_last_nonascii_fold());
        assert_eq!(rtl.nonascii_conversions, kernel_rtl_string_nonascii_count());
        assert_eq!(rtl.compare_refused, kernel_rtl_compare_refused_count());
        assert_eq!(rtl.time_refused, kernel_rtl_time_refused_count());
        assert_eq!(rtl.unmapped, kernel_rtl_unmapped_count());
        assert_eq!(rtl.last_unmapped, kernel_rtl_last_unmapped());
    }
}
#[test]
fn seeded_rtl_properties_against_c_host() {
    assert_ne!(unsafe { shim_init() }, 0);
    // Private low-4GB mapping supports the 16-bit descriptor limits without disc data.
    let length = 0x40000;
    let pointer = unsafe { mmap(std::ptr::null_mut(), length, 3, 0x62, -1, 0) };
    assert_ne!(pointer as isize, -1);
    assert!(pointer as usize <= u32::MAX as usize);
    let base = pointer as u32;
    let mut seed = 0x2209_1234_5678_9abc;
    let mut calls = 0;
    for case in 0..3000 {
        let mut bytes = vec![0xa5; length];
        let n = (next(&mut seed) % 256) as u16;
        for index in 0..usize::from(n) * 2 + 4 {
            bytes[1024 + index] = next(&mut seed) as u8;
        }
        bytes[1024 + usize::from(n)] = 0;
        bytes.copy_within(1024..1024 + usize::from(n), 4096);
        if case % 3 == 0 && n > 0 {
            bytes[4096] ^= 1;
        }
        descriptor(&mut bytes, 64, n, 0, base + 1024);
        descriptor(
            &mut bytes,
            80,
            if case % 7 == 0 { n.wrapping_add(1) } else { n },
            0,
            base + 4096,
        );
        compare(
            base,
            &bytes,
            279,
            &[base + 64, base + 80, next(&mut seed)],
            -1,
        );
        compare(
            base,
            &bytes,
            289,
            &[base + 96, if case % 11 == 0 { 0 } else { base + 1024 }],
            -1,
        );
        let pattern = next(&mut seed);
        let prefix = (next(&mut seed) % 64) as usize;
        for word in bytes[8193..8193 + prefix * 4].chunks_exact_mut(4) {
            word.copy_from_slice(&pattern.to_le_bytes());
        }
        compare(
            base,
            &bytes,
            269,
            &[base + 8193, next(&mut seed) % 260, pattern],
            -1,
        );
        let statuses = [
            0, 0x103, 0xc0000008, 0xc0000017, 0xc000000d, 0xc0000003, 0xc0000034, 0x80000005,
            0x80000006, 0xc0000004, 0xc000000f, 0xc0000010, 0xc0000011, 0xc0000022, 0xc0000023,
            0xc0000024, 0xc0000033, 0xc0000035, 0xc000003a, 0xc0000043, 0xc000007f, 0xc000009a,
            0xc00000ba, 0xc0000101, 0xc0000103, 0xc000014f,
        ];
        compare(
            base,
            &bytes,
            301,
            &[if case % 2 == 0 {
                statuses[case % statuses.len()]
            } else {
                next(&mut seed)
            }],
            -1,
        );
        for widening in [true, false] {
            descriptor(
                &mut bytes,
                64,
                if widening {
                    n
                } else {
                    n * 2 + u16::from(case % 13 == 0)
                },
                65535,
                base + 1024,
            );
            descriptor(
                &mut bytes,
                80,
                0xabcd,
                if case % 5 == 0 { n / 2 } else { 1024 },
                base + 12288,
            );
            compare(
                base,
                &bytes,
                if widening { 260 } else { 308 },
                &[base + 80, base + 64, if case % 17 == 0 { 1 } else { 256 }],
                if case % 19 == 0 { 1 } else { -1 },
            );
        }
        let ticks = (u64::from(next(&mut seed)) << 32) | u64::from(next(&mut seed));
        bytes[128..136].copy_from_slice(&ticks.to_le_bytes());
        compare(base, &bytes, 305, &[base + 128, base + 160], -1);
        let fields = if case % 3 == 0 {
            TimeFields {
                year: (1601 + next(&mut seed) % 31167) as i16,
                month: (1 + next(&mut seed) % 12) as i16,
                day: (1 + next(&mut seed) % 28) as i16,
                hour: (next(&mut seed) % 24) as i16,
                minute: (next(&mut seed) % 60) as i16,
                second: (next(&mut seed) % 60) as i16,
                millisecond: (next(&mut seed) % 1000) as i16,
                weekday: next(&mut seed) as i16,
            }
        } else {
            let mut raw = [0; 16];
            for byte in &mut raw {
                *byte = next(&mut seed) as u8;
            }
            TimeFields::from_bytes(raw)
        };
        bytes[160..176].copy_from_slice(&fields.bytes());
        compare(base, &bytes, 304, &[base + 160, base + 128], -1);
        if let Some(ticks) = fields.to_ticks() {
            let restored = TimeFields::from_ticks(ticks).unwrap();
            assert_eq!(
                TimeFields {
                    weekday: restored.weekday,
                    ..fields
                },
                restored
            );
        }
        calls += 8;
    }
    let mut bytes = vec![b'x'; length];
    // Truncation, maximum counted conversion, malformed descriptors and NULL faults.
    compare(base, &bytes, 289, &[base + 64, base + 1024], -1);
    bytes[1024 + 65534] = 0;
    compare(base, &bytes, 289, &[base + 64, base + 1024], -1);
    for n in [0, 1, 32766, 32767, 32768, 65534, 65535] {
        descriptor(&mut bytes, 64, n, 65535, base + 1024);
        descriptor(&mut bytes, 80, 0xabcd, 65535, base + 100000);
        for ordinal in [260, 308] {
            compare(base, &bytes, ordinal, &[base + 80, base + 64, 0], -1);
        }
    }
    for ordinal in [260, 269, 279, 289, 304, 305, 308] {
        compare(base, &bytes, ordinal, &[0, 16, 0], -1);
    }
    // Century leap boundaries, signed invalid fields, ignored weekday and tick limits.
    for (year, month, day) in [
        (1601, 1, 1),
        (1700, 2, 29),
        (1900, 2, 29),
        (2000, 2, 29),
        (2001, 2, 29),
        (32767, 12, 31),
        (-1, 1, 1),
        (1600, 12, 31),
        (2024, 0, 1),
        (2024, 13, 1),
    ] {
        let fields = TimeFields {
            year,
            month,
            day,
            weekday: -32768,
            ..TimeFields::default()
        };
        bytes[160..176].copy_from_slice(&fields.bytes());
        compare(base, &bytes, 304, &[base + 160, base + 128], -1);
    }
    for ticks in [
        0,
        9999,
        10000,
        864_000_000_000,
        i64::MAX as u64,
        1u64 << 63,
        u64::MAX,
    ] {
        bytes[128..136].copy_from_slice(&ticks.to_le_bytes());
        compare(base, &bytes, 305, &[base + 128, base + 160], -1);
    }
    // Empty equality does not touch NULL buffers; compare validates past a mismatch.
    descriptor(&mut bytes, 64, 0, 0, 0);
    descriptor(&mut bytes, 80, 0, 0, 0);
    compare(base, &bytes, 279, &[base + 64, base + 80, 256], -1);
    compare(base, &bytes, 269, &[base + length as u32 - 4, 8, 0], -1);
    // Conversion aliases and partial destination writes retain the C ordering.
    for ordinal in [260, 308] {
        descriptor(&mut bytes, 64, 16, 65535, base + 1024);
        descriptor(&mut bytes, 80, 0xabcd, 65535, base + 1024);
        compare(base, &bytes, ordinal, &[base + 80, base + 64, 0], -1);
        for writes in [0, 1, 2] {
            compare(base, &bytes, ordinal, &[base + 80, base + 64, 0], writes);
        }
    }
    assert_eq!(
        KernelRtl::default().dispatch(
            &mut Memory {
                inner: MappedMemory {
                    base: GuestAddr(base),
                    bytes: &mut bytes
                },
                writes: -1
            },
            302,
            &[0]
        ),
        None
    );
    assert_eq!(unsafe { munmap(pointer, length) }, 0);
    eprintln!(
        "T2209: {calls} seeded ordinal calls plus boundary cases, return/memory/counters equal"
    );
}
