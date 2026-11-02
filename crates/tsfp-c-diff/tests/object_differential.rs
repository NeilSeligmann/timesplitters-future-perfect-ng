// SPDX-License-Identifier: GPL-3.0-or-later
//! T1835: exact Rust object leaves versus unmodified registered C bodies.
//! Bounded mapped-memory evidence; no original or fault-domain admission.
use tsfp_engine_object::{ObjectMemory, exact::{self, Registers}};

unsafe extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
    fn munmap(addr: *mut u8, len: usize) -> i32;
    fn shim_set_base(base: *mut u8);
    fn shim_call(va: u32, regs: *mut u32) -> i32;
}
const ARENA: usize = (1usize << 32) + 0x10000;
const STACK: u32 = 0x300000;
const CASES: usize = 120_000;
struct Memory(*mut u8);
impl ObjectMemory for Memory {
    fn read8(&self, a: u32) -> u8 { unsafe { *self.0.add(a as usize) } }
    fn write8(&mut self, a: u32, v: u8) { unsafe { *self.0.add(a as usize) = v; } }
}
impl Drop for Memory {
    fn drop(&mut self) { assert_eq!(unsafe { munmap(self.0, ARENA) }, 0); }
}
fn memory() -> Memory {
    let p = unsafe { mmap(std::ptr::null_mut(), ARENA, 3, 0x2 | 0x20 | 0x4000, -1, 0) };
    assert_ne!(p as isize, -1);
    Memory(p)
}
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13; self.0 ^= self.0 >> 7; self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

#[test]
fn exact_object_c_differential_and_controls() {
    // One test owns the process-global C memory base. Other suites are separate binaries.
    let mut m = memory();
    unsafe { shim_set_base(m.0); }
    for (va, seed) in [(0x12ba0, 0x1835_0012ba0u64), (0x174e0, 0x1835_00174e0u64)] {
        let mut rng = Rng(seed);
        let mut killed = [0usize; 3];
        let mut nulls = 0;
        let mut selected = 0;
        for i in 0..CASES {
            // Include all-address arithmetic, unaligned pools and stack/data aliases.
            // No multi-byte read crosses 2^32: that fault/host-window domain is unproved.
            let object = match i % 8 {
                0 => STACK - 12,
                1 => 0xffff_ffe0,
                2 => 0,
                _ => (rng.next() & 0x00ff_fff0) | (rng.next() & 3),
            };
            let table = match i % 9 { 0 => STACK, 1 => object, _ => 0x400000 + (rng.next() & 0xfff) };
            let key = match i % 11 { 0 => STACK + 2, 1 => object, _ => 0x500000 + (rng.next() & 0xfff) };
            for a in [object.wrapping_add(12), object.wrapping_add(16), object.wrapping_add(20),
                      object.wrapping_add(24), object.wrapping_add(28), table.wrapping_add(4),
                      key.wrapping_add(6)] {
                m.write32(a, rng.next());
            }
            let flag = if i % 2 == 0 { 0x10 } else { 0 };
            m.write8(object.wrapping_add(12), flag);
            let base = if i % 3 == 0 { 0 } else { rng.next() };
            m.write32(table.wrapping_add(4), base);
            m.write8(0x7de455, if i % 2 == 0 { 10 } else { rng.next() as u8 });
            let args = if va == 0x12ba0 { [object, table, key] }
                else { [object, if i % 3 == 0 { 0 } else { rng.next() },
                         if i % 4 == 0 { 0 } else { rng.next() }] };
            m.write32(STACK, 0x12345678);
            for (n, v) in args.into_iter().enumerate() { m.write32(STACK + 4 + 4*n as u32, v); }
            let mut r = Registers { eax: rng.next(), ecx: rng.next(), edx: rng.next() };
            let mut c = [r.eax, r.ecx, r.edx, rng.next(), STACK];
            let esi = c[3];
            let mut watched = vec![(STACK, 16), (0x7de455, 1), (object, 32),
                                   (table.wrapping_add(4), 4), (key.wrapping_add(6), 2)];
            watched.sort_unstable();
            let before: Vec<_> = watched.iter().flat_map(|(a, n)| (0..*n).map(|j| m.read8(a.wrapping_add(j)))).collect();
            if va == 0x12ba0 {
                nulls += usize::from(m.read32(table.wrapping_add(4)) == 0);
                selected += usize::from(m.read8(object.wrapping_add(12)) & 0x10 != 0);
                exact::indexed_field_pointer(&m, STACK, &mut r);
            } else { let g = tsfp_engine_object::object_leaves::mode_selected_flag_pair_intersects(
                    &m, STACK, tsfp_engine_object::gprs::Gprs { eax: r.eax, ecx: r.ecx, edx: r.edx });
                r = Registers { eax: g.eax, ecx: g.ecx, edx: g.edx }; }
            assert_eq!(unsafe { shim_call(va, c.as_mut_ptr()) }, 1);
            assert_eq!([r.eax, r.ecx, r.edx], [c[0], c[1], c[2]], "va={va:x} case={i}");
            assert_eq!(c[3], esi);
            assert_eq!(c[4], STACK+4);
            let after: Vec<_> = watched.iter().flat_map(|(a, n)| (0..*n).map(|j| m.read8(a.wrapping_add(j)))).collect();
            assert_eq!(before, after, "read-only memory contract");
            // Each independently changed volatile output must be caught by exact comparison.
            for n in 0..3 { let mut mutant = [r.eax, r.ecx, r.edx]; mutant[n] ^= 1;
                killed[n] += usize::from(mutant != [c[0], c[1], c[2]]); }
        }
        assert_eq!(killed, [CASES; 3]);
        if va == 0x12ba0 { assert!(nulls > CASES/5 && selected > CASES/4 && selected < CASES*3/4); }
        eprintln!("T1835 {va:08x}: {CASES} equal, mutants={killed:?}, nulls={nulls}, stride8={selected}");
    }
}
