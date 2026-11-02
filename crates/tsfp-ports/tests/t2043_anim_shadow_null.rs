// SPDX-License-Identifier: GPL-3.0-or-later
//! T2043 (shadow variant of t2031_anim_null.rs, `TSFP_ANIM_SHADOW` set): `tools.rust_trace` turns the shadow mode on and the T1996 run faulted in
//! `anim_lane3::run_inner` because the shadow mode bypassed the guard. T2031: every animation lane root called with a null / unmapped object (or any pointer argument) must not touch unmapped guest memory
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

fn all_roots() -> Vec<u32> {
    let mut roots = Vec::new();
    roots.extend(anim_lane::ROOTS);
    roots.extend(anim_lane2::ROOTS);
    roots.extend(anim_lane3::ROOTS);
    roots.extend(anim_lane3::ROOTS_MORE);
    roots
}

/// One case in a child process (the fake guest cannot reproduce a port's writes, so the shadow comparison may abort the process after the
/// port ran; the parent only cares that the port never touched a raw bad address). Does nothing unless `T2043_CASE=<va>:<field>:<structure>` is set.
#[test]
fn shadow_case() {
    let Ok(case) = std::env::var("T2043_CASE") else { return };
    let parts: Vec<&str> = case.split(':').collect();
    let (va, field, structure) = (u32::from_str_radix(parts[0], 16).unwrap(), u32::from_str_radix(parts[1], 16).unwrap(), parts[2] == "1");
    unsafe { std::env::set_var("TSFP_ANIM_SHADOW", "1") };
    assert!(anim_lane::shadow_enabled());
    let objects = [0x3_0000u32, 0x4_0000, 0x5_0000, 0x6_0000, 0x7_0000, 0x8_0000, 0x9_0000, 0xa_0000];
    let mut m = Fake(vec![0; SIZE]);
    if structure {
        for at in (0..SIZE).step_by(4) {
            m.0[at..at + 4].copy_from_slice(&field.to_le_bytes());
        }
    }
    for i in 0..8u32 {
        m.write32(STACK + 4 + 4 * i, if structure { objects[i as usize] } else { field });
    }
    let (mut guest, mut regs) = (Guest::default(), registers());
    regs.esi = if structure { objects[0] } else { field };
    assert!(run(va, &mut m, &mut guest, &mut regs), "root {va:#x} not dispatched");
}

#[test]
fn shadow_mode_never_touches_a_raw_bad_address() {
    let exe = std::env::current_exe().unwrap();
    let roots = all_roots();
    assert!(roots.len() >= 20);
    let mut completed = 0;
    for &va in &roots {
        for field in [0u32, 4, 0xffff, 0xdead_beef, 0x8000_0000, 0xffff_fffc] {
            for structure in [false, true] {
                let out = std::process::Command::new(&exe)
                    .args(["--exact", "shadow_case", "--nocapture", "--test-threads=1"])
                    .env("T2043_CASE", format!("{va:x}:{field:x}:{}", u8::from(structure)))
                    .output()
                    .unwrap();
                let text = String::from_utf8_lossy(&out.stderr);
                assert!(!text.contains("raw guest access"), "root {va:#x} field {field:#x} structure {structure}: raw bad access\n{text}");
                let mismatch_from_fake = text.contains("ANIM-SHADOW"); // the fake original does not write what the port wrote
                assert!(out.status.success() || mismatch_from_fake, "root {va:#x} field {field:#x} structure {structure}: {:?}\n{text}", out.status);
                completed += 1;
            }
        }
    }
    assert!(completed >= roots.len() * 12);
}
