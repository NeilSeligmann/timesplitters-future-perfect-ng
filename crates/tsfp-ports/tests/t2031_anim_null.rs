// SPDX-License-Identifier: GPL-3.0-or-later
//! T2031: every animation lane root called with a null / unmapped object (or any pointer argument) must not touch unmapped guest memory
//! and must hand the call to the retained original. The fake memory panics on any raw access outside its mapping, so a port that
//! dereferences a bad pointer fails the test the way it faulted the host (signal 11, `anim_lane::run::{{closure#0}}`).
use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::memory::GuestMemory;
use tsfp_ports::{anim_lane, anim_lane2, anim_lane3};

/// The guard journal is one process-wide stack (one guest thread in the host), so the tests of this file must not overlap.
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

const BASE: u32 = 0x1_0000;
const SIZE: usize = 0x80_0000;
const STACK: u32 = 0x7f_0000;

struct Fake(Vec<u8>);

impl Fake {
    /// The null page and everything from the kernel window up fault like the host does. In between, memory above the fake's
    /// mapping stands for mapped heap that reads as zero and ignores writes (the real window is 64 MB and more).
    fn index(address: u32) -> Option<usize> {
        assert!((BASE..0x8000_0000).contains(&address), "raw guest access outside the mapped range: {address:#x}");
        let at = (address - BASE) as usize;
        (at < SIZE).then_some(at)
    }
}

impl GuestMemory for Fake {
    fn read8(&self, address: u32) -> u8 {
        Self::index(address).map_or(0, |at| self.0[at])
    }
    fn write8(&mut self, address: u32, value: u8) {
        if let Some(at) = Self::index(address) {
            self.0[at] = value;
        }
    }
}

#[derive(Default)]
struct Guest {
    calls: Vec<u32>,
}

impl GuestCall for Guest {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        self.calls.push(va);
        regs.eax = 0xa5a5;
    }
}

fn registers() -> ExtRegisters {
    ExtRegisters {
        eax: 0, ecx: 0, edx: 0, esp: STACK, ebx: 0, ebp: 0, esi: 0, edi: 0, eflags: 0,
        fpu: Fpu { stack: [0.0; 8], top: 0, control: 0x37f },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

fn run(va: u32, m: &mut Fake, guest: &mut Guest, regs: &mut ExtRegisters) -> bool {
    anim_lane::run(va, m, guest, regs) || anim_lane2::run(va, m, guest, regs) || anim_lane3::run(va, m, guest, regs)
}

fn roots() -> Vec<u32> {
    let mut all = Vec::new();
    all.extend(anim_lane::ROOTS);
    all.extend(anim_lane2::ROOTS);
    all.extend(anim_lane3::ROOTS);
    all.extend(anim_lane3::ROOTS_MORE);
    all
}

#[test]
fn bad_pointers_never_fault_and_fall_back_to_the_original() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let bad = [0u32, 4, 0xffff, 0xdead_beef, 0x8000_0000, 0xffff_fffc, 0x2_0000];
    let roots = roots();
    assert!(roots.len() >= 20);
    let mut delegated = 0;
    for &va in &roots {
        for &pointer in &bad {
            let mut m = Fake(vec![0; SIZE]);
            // return slot at STACK, arguments at STACK + 4 ..: every argument is the bad pointer
            for i in 0..8 {
                m.write32(STACK + 4 + 4 * i, pointer);
            }
            let (mut guest, mut regs) = (Guest::default(), registers());
            let handled = run(va, &mut m, &mut guest, &mut regs);
            assert!(handled, "root {va:#x} not dispatched");
            if guest.calls.last() == Some(&va) {
                delegated += 1;
                assert_eq!(regs.eax, 0xa5a5, "root {va:#x} pointer {pointer:#x}: the original's result must be returned");
                assert_eq!(regs.esp, STACK, "root {va:#x}: esp restored");
            }
        }
    }
    // the roots that dereference their pointer arguments delegate; a few (id <= 0, ESI context, a0 == 0) legitimately never touch memory
    assert!(delegated >= roots.len(), "bad pointers must reach the original (delegated {delegated})");
}

#[test]
fn null_object_body_roots_delegate_exactly_once() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    for va in [0x59f90, 0x61570, 0x5a2b0, 0x59d50, 0x59ec0, 0x5a2f0, 0x614b0, 0x5a070, 0x5a260] {
        let mut m = Fake(vec![0; SIZE]);
        let (mut guest, mut regs) = (Guest::default(), registers());
        assert!(anim_lane::run(va, &mut m, &mut guest, &mut regs));
        // 0x61570 / 0x614B0 first look the animation up through the lifted 0x56FC0, which is a pure query
        assert_eq!(guest.calls.last(), Some(&va), "root {va:#x}");
        assert_eq!(regs.eax, 0xa5a5);
    }
}

/// T2043: the pointer ARGUMENTS are valid, but every pointer read out of the structures they lead to (body, model, clip table, key table,
/// matrix arrays, layer chains, global tables) is bad. The T2031 test above never got past the first dereference, so it missed the ports
/// that read `*(object + n)` and then dereferenced that (`anim_lane3` reached the host with those).
#[test]
fn bad_structure_fields_never_fault_and_fall_back_to_the_original() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let bad_fields = [0u32, 4, 0xffff, 0xdead_beef, 0x8000_0000, 0xffff_fffc, 0x7fff_fff0];
    let objects = [0x3_0000u32, 0x4_0000, 0x5_0000, 0x6_0000, 0x7_0000, 0x8_0000, 0x9_0000, 0xa_0000];
    let roots = roots();
    let mut delegated = 0;
    for &va in &roots {
        for &field in &bad_fields {
            let mut m = Fake(vec![0; SIZE]);
            // every dword of the mapping (structures and global tables included) holds the bad pointer
            for at in (0..SIZE).step_by(4) {
                let word = field.to_le_bytes();
                m.0[at..at + 4].copy_from_slice(&word);
            }
            for i in 0..8 {
                m.write32(STACK + 4 + 4 * i, objects[i as usize]);
            }
            let (mut guest, mut regs) = (Guest::default(), registers());
            regs.esi = objects[0];
            let handled = run(va, &mut m, &mut guest, &mut regs);
            assert!(handled, "root {va:#x} not dispatched");
            if guest.calls.last() == Some(&va) {
                delegated += 1;
                assert_eq!(regs.esp, STACK, "root {va:#x} field {field:#x}: esp restored");
            }
        }
    }
    assert!(delegated >= roots.len(), "bad structure fields must reach the original (delegated {delegated})");
}

/// T2043: a bad field met after the port already wrote must undo those writes before the original runs (the journal covers more than one
/// page of writes). The fake keeps the first structure valid and poisons the matrix array pointer.
#[test]
fn a_late_bad_pointer_rolls_back_every_write() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    use tsfp_ports::anim_lane::{Guarded, guest_ok};
    let mut m = Fake(vec![0; SIZE]);
    let before: Vec<u8> = m.0.clone();
    {
        let mut view = Guarded::new(&mut m);
        for i in 0..5000u32 {
            view.write32(0x2_0000 + 4 * i, 0x1111_1111 + i);
        }
        assert!(!view.is_bad());
        view.write32(0, 1); // refused
        assert!(view.is_bad());
        view.rollback();
    }
    assert!(m.0 == before, "rollback must restore every journalled byte");
    assert!(!guest_ok(0) && guest_ok(0x2_0000));
}
