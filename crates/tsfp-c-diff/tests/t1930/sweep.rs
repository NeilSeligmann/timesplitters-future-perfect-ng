// SPDX-License-Identifier: GPL-3.0-or-later
//! T1930: seeded differential of actual assets/syslink ports, no production adapter.
use tsfp_network::syslink_bulk::{
    self,
    state::{NetworkCall, NetworkRegisters},
};
use tsfp_ports::{
    abi::{ExtRegisters, GuestCall},
    assets::{self, AssetGuest},
};
unsafe extern "C" {
    fn t1930_run(base: *mut u8, va: u32, regs: *mut u32);
}
const STACK: u32 = 0x10000;
const OBJ: u32 = 0x20000;
const ROOTS: [u32; 27] = [
    0x175d0, 0x18200, 0x64e50, 0x97fb0, 0x257b30, 0x57b80, 0x62055, 0x28af0, 0x29130, 0x29160,
    0x291a0, 0x62040, 0x62260, 0x62380, 0x613c0, 0x626e0, 0x62700, 0x6a660, 0x1cf630, 0x311840,
    0x3118d0, 0x3118e0, 0x156e60, 0x3bd460, 0x3bfbe0, 0x3c44c0, 0x3c4730,
];
const REGIONS: [(u32, usize); 15] = [
    (0, 64),
    (0x30000, 256),
    (0x54e568, 128),
    (0x79094c, 4),
    (0x7de304, 4),
    (0x4b9138, 4),
    (0x6612d0, 0x3640),
    (STACK - 64, 192),
    (OBJ, 0x1000),
    (0x4d19d0, 64),
    (0x5149ac, 64 * 0x34),
    (0x52cf38, 64 * 0x58),
    (0x52d150, 64 * 8),
    (0x6d7880, 0x400),
    (0x75af1c, 8),
];
struct Mem(Vec<u8>);
macro_rules! memory {
    ($tr:path) => {
        impl $tr for Mem {
            fn read8(&self, a: u32) -> u8 {
                self.0[a as usize]
            }
            fn write8(&mut self, a: u32, v: u8) {
                self.0[a as usize] = v;
            }
        }
    };
}
memory!(tsfp_ports::memory::GuestMemory);
memory!(tsfp_network::memory::GuestMemory);
impl Mem {
    fn put(&mut self, a: u32, v: u32) {
        self.0[a as usize..a as usize + 4].copy_from_slice(&v.to_le_bytes());
    }
}
struct NoCalls;
impl GuestCall for NoCalls {
    fn call(&mut self, va: u32, _: &mut ExtRegisters) {
        panic!("unexpected callee {va:x}")
    }
}
impl AssetGuest for NoCalls {
    fn writeback_invalidate(&mut self, _: u32) {
        panic!("unexpected cache operation")
    }
}
impl NetworkCall for NoCalls {
    fn call(&mut self, va: u32, _: &mut NetworkRegisters) {
        panic!("unexpected callee {va:x}")
    }
}
struct Scripted {
    base: *mut u8,
    index: u32,
}
impl GuestCall for Scripted {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        assert!(matches!(
            va,
            0x12c370
                | 0x63b40
                | 0xbdab0
                | 0x1b6130
                | 0x22ce60
                | 0x12c730
                | 0x2fe270
                | 0xa5e40
                | 0x257b30
        ));
        let a = 0x30000 + 44 * self.index;
        // Pointer addresses the fixture allocation; callback runs synchronously and
        // uses it only while the production function has relinquished memory access.
        let read = |addr: u32| unsafe {
            u32::from_le_bytes(
                std::slice::from_raw_parts(self.base.add(addr as usize), 4)
                    .try_into()
                    .unwrap(),
            )
        };
        let words = [
            va,
            r.eax,
            r.ecx,
            r.edx,
            r.ebx,
            r.esi,
            r.edi,
            r.ebp,
            r.esp,
            read(r.esp),
            read(r.esp + 4),
        ];
        for (i, word) in words.into_iter().enumerate() {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    word.to_le_bytes().as_ptr(),
                    self.base.add(a as usize + i * 4),
                    4,
                );
            }
        }
        self.index += 1;
        r.eax ^= va;
        r.ecx = r.ecx.wrapping_add(self.index);
        r.edx ^= 0x19300000 + self.index;
    }
}
impl AssetGuest for Scripted {
    fn writeback_invalidate(&mut self, _: u32) {
        panic!("unexpected cache operation")
    }
}
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}
fn rust_run(va: u32, m: &mut Mem, v: [u32; 8]) -> [u32; 8] {
    if va == 0x156e60 || va >= 0x3bd460 {
        let mut r = NetworkRegisters {
            eax: v[0],
            ecx: v[1],
            edx: v[2],
            ebx: v[3],
            esi: v[4],
            edi: v[5],
            ebp: v[6],
            esp: v[7],
            ..Default::default()
        };
        assert!(syslink_bulk::run_if_enabled(
            va,
            true,
            m,
            &mut NoCalls,
            &mut r
        ));
        [r.eax, r.ecx, r.edx, r.ebx, r.esi, r.edi, r.ebp, r.esp + 4]
    } else {
        let mut r = ExtRegisters {
            eax: v[0],
            ecx: v[1],
            edx: v[2],
            ebx: v[3],
            esi: v[4],
            edi: v[5],
            ebp: v[6],
            esp: v[7],
            eflags: 0,
            fpu: tsfp_ports::abi::Fpu {
                stack: [0.; 8],
                top: 0,
                control: 0x37f,
            },
            xmm: [Default::default(); 5],
            xmm_hi: [Default::default(); 3],
        };
        if matches!(va, 0x175d0 | 0x18200 | 0x64e50 | 0x97fb0 | 0x257b30) {
            let mut calls = Scripted {
                base: m.0.as_mut_ptr(),
                index: 0,
            };
            assert!(assets::dispatch(va, &[va], m, &mut calls, &mut r));
        } else {
            assert!(assets::dispatch(va, &[va], m, &mut NoCalls, &mut r));
        }
        [r.eax, r.ecx, r.edx, r.ebx, r.esi, r.edi, r.ebp, r.esp + 4]
    }
}
#[test]
fn assets_syslink_tier1() {
    let mut c = Mem(vec![0; 0x800000]);
    let mut rust = Mem(vec![0; 0x800000]);
    for va in ROOTS {
        let mut rng = Rng(0x1930_0000 + va as u64);
        for index in 0..3000 {
            for (start, len) in REGIONS {
                for a in start as usize..start as usize + len {
                    c.0[a] = rng.next() as u8;
                }
            }
            let mut regs = std::array::from_fn(|_| rng.next());
            regs[1] = OBJ;
            regs[7] = STACK;
            c.put(STACK, 0xdeadbeef);
            let arg = match va {
                0x6a660 | 0x311840 | 0x3118e0 => index % 64,
                0x156e60 | 0x3c4730 => OBJ + 0x800,
                _ => rng.next(),
            };
            c.put(STACK + 4, arg);
            if va == 0x97fb0 {
                c.put(0x7de304, index % 2);
            }
            if va == 0x257b30 {
                c.put(0x79094c, if index % 2 == 0 { 30 } else { rng.next() });
            }
            if va == 0x57b80 {
                let mode = index % 12;
                let hay = OBJ + 0x400;
                let needle = OBJ + 0x800;
                let len = 1 + index % 48;
                let pattern_len = 1 + index % 8;
                for offset in 0..64 {
                    c.0[(hay + offset) as usize] = b'a' + (rng.next() % 5) as u8;
                }
                for offset in 0..pattern_len {
                    c.0[(needle + offset) as usize] = b'a' + (rng.next() % 5) as u8;
                }
                c.0[(needle + pattern_len) as usize] = 0;
                if mode == 2 {
                    c.0[needle as usize] = 0;
                }
                if mode >= 6 {
                    let pos = if mode == 6 { 0 } else { 1 + index % 24 };
                    c.0[(hay + pos - u32::from(pos != 0)) as usize] =
                        if mode == 8 { b'x' } else { b'\n' };
                    for offset in 0..pattern_len {
                        c.0[(hay + pos + offset) as usize] = c.0[(needle + offset) as usize];
                    }
                    if mode == 9 && pattern_len > 1 {
                        c.0[(hay + pos + pattern_len - 1) as usize] ^= 1;
                    }
                }
                c.put(STACK + 4, if mode == 0 { 0 } else { hay });
                c.put(STACK + 8, if mode == 1 { 0 } else { needle });
                c.put(
                    STACK + 12,
                    match mode {
                        3 => 0,
                        4 => u32::MAX,
                        _ => len,
                    },
                );
            }
            if va == 0x62040 || va == 0x62055 {
                let n = index % 10;
                c.put(0x6d7880, if n == 9 { 30 } else { n });
                c.put(STACK + 4, OBJ);
                c.put(STACK + 8, rng.next());
                c.put(STACK + 12, rng.next());
                for slot in 0..8 {
                    c.put(
                        0x6d7918 + slot * 0x1c,
                        if index % 3 == 0 && slot == index % 8 {
                            u32::MAX
                        } else {
                            slot
                        },
                    );
                }
            }
            if va == 0x62055 {
                // Entry after the parent's 12-byte local allocation and count load.
                regs[1] = index % 9;
                regs[7] = STACK - 12;
            }
            if va == 0x62260 {
                let n = index % 10;
                c.put(0x6d7880, if n == 9 { u32::MAX } else { n });
                c.put(STACK + 4, OBJ + 0x900);
                c.0[(OBJ + 0x900) as usize..(OBJ + 0x908) as usize].copy_from_slice(b"path123\0");
                for slot in 0..9 {
                    let ptr = OBJ + 0x500 + slot * 0x20;
                    c.put(0x6d7914 + slot * 0x1c, if index % 4 == 0 { 0 } else { ptr });
                    let text = if slot == index % 9 {
                        b"path123\0"
                    } else {
                        b"other12\0"
                    };
                    c.0[ptr as usize..ptr as usize + 8].copy_from_slice(text);
                }
            }
            if va == 0x62380 {
                regs[5] = index % 8;
                let base = 0x6d7900 + regs[5] * 0x1c;
                let n = index % 10;
                c.put(base + 0x10, if index % 5 == 0 { 0 } else { OBJ + 0x400 });
                c.put(base + 8, if n == 9 { u32::MAX } else { n });
                for slot in 0..8 {
                    c.put(
                        OBJ + 0x400 + slot * 0x10,
                        if slot == index % 9 {
                            regs[3]
                        } else {
                            regs[3] ^ 1
                        },
                    );
                }
            }
            if va == 0x28af0 {
                c.put(0x4b9138, index % 2);
            }
            if va == 0x29130 {
                let n = index % 9;
                c.put(0x6648d0, if n == 0 { 0x664890 } else { OBJ });
                for slot in 0..n {
                    c.put(
                        OBJ + slot * 0x80 + 0x40,
                        if slot + 1 == n {
                            0x664890
                        } else {
                            OBJ + (slot + 1) * 0x80
                        },
                    );
                }
            }
            if va == 0x29160 {
                regs[0] = OBJ;
                c.put(OBJ + 0x3c, OBJ + 0x200);
                c.put(OBJ + 0x40, OBJ + 0x300);
                c.put(0x6648d4, if index % 2 == 0 { OBJ } else { OBJ + 0x400 });
            }
            if va == 0x3bd460 {
                for slot in 0..8 {
                    c.put(OBJ + 0xd4 + slot * 0x24, arg.wrapping_add(slot + 1));
                }
                if index % 9 < 8 {
                    c.put(OBJ + 0xd4 + (index % 9) * 0x24, arg);
                }
            }
            if va == 0x3c4730 {
                let n = index % 10;
                c.put(OBJ + 4, if n == 9 { u32::MAX } else { n });
                c.put(OBJ + 8, OBJ + 0x400);
                let key1 = rng.next();
                let key2 = rng.next();
                c.put(arg + 4, key1);
                c.put(arg + 8, key2);
                for slot in 0..8 {
                    let ptr = OBJ + 0x500 + slot * 0x20;
                    c.put(OBJ + 0x400 + slot * 4, if index % 3 == 0 { 0 } else { ptr });
                    c.put(ptr + 4, if slot == index % 8 { key1 } else { key1 ^ 1 });
                    c.put(ptr + 8, if index % 4 == 0 { key2 ^ 1 } else { key2 });
                }
            }
            for (start, len) in REGIONS {
                rust.0[start as usize..start as usize + len]
                    .copy_from_slice(&c.0[start as usize..start as usize + len]);
            }
            let mut expected = regs;
            unsafe { t1930_run(c.0.as_mut_ptr(), va, expected.as_mut_ptr()) };
            let actual = rust_run(va, &mut rust, regs);
            assert_eq!(actual, expected, "VA {va:08x} case {index}");
            // Native C CALL writes an internal RET word absent from GuestCall.
            // Exclude only those bridge-owned slots; argument/saved-register bytes remain checked.
            if va == 0x64e50 {
                c.0[(STACK - 4) as usize..STACK as usize]
                    .copy_from_slice(&rust.0[(STACK - 4) as usize..STACK as usize]);
            }
            if va == 0x257b30 {
                c.0[(STACK - 8) as usize..(STACK - 4) as usize]
                    .copy_from_slice(&rust.0[(STACK - 8) as usize..(STACK - 4) as usize]);
            }
            if va == 0x18200 {
                c.0[(STACK - 12) as usize..(STACK - 8) as usize]
                    .copy_from_slice(&rust.0[(STACK - 12) as usize..(STACK - 8) as usize]);
            }
            for (start, len) in REGIONS {
                assert_eq!(
                    &c.0[start as usize..start as usize + len],
                    &rust.0[start as usize..start as usize + len],
                    "VA {va:08x} case {index} region {start:x}"
                );
            }
        }
        eprintln!(
            "VA {va:08x}: 3000 cases EQUAL (seed {:x})",
            0x1930_0000 + va as u64
        );
    }
}
