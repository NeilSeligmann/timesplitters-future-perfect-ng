// SPDX-License-Identifier: GPL-3.0-or-later
//! T1986: an absent heading track is valid and must not taint the guard or invoke fallback. Synthetic structures only.
use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::anim_lane3;
use tsfp_ports::memory::GuestMemory;

const BASE: u32 = 0x1_0000;
const SIZE: usize = 0x80_0000;
const STACK: u32 = 0x7f_0000;

struct Fake(Vec<u8>);
impl GuestMemory for Fake {
    fn read8(&self, a: u32) -> u8 {
        assert!((BASE..0x8000_0000).contains(&a), "raw access {a:#x}");
        self.0.get((a - BASE) as usize).copied().unwrap_or(0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        assert!((BASE..0x8000_0000).contains(&a), "raw access {a:#x}");
        if let Some(b) = self.0.get_mut((a - BASE) as usize) {
            *b = v;
        }
    }
}

struct Guest;
impl GuestCall for Guest {
    fn call(&mut self, va: u32, _: &mut ExtRegisters) {
        panic!("no-track heading must not call retained original: {va:x}");
    }
}

#[test]
fn missing_heading_track_returns_without_reading_keys_or_falling_back() {
    // One thread: guard counters are process-wide. Both roots must preserve them.
    let mut m = Fake(vec![0; SIZE]);
    let (clip, object, body) = (0x2_0000, 0x2_1000, 0x2_2000);
    m.write32(clip + 0x10, 2); // key count; both key table and values table are absent
    m.write32(object + 4, body);
    m.write32(body + 0xd8, 0);
    m.write32(0x7e3954, 0x2_3000);
    m.write32(0x2_3000, clip);
    let mut regs = ExtRegisters {
        eax: 0, ecx: 0, edx: 0, esp: STACK, ebx: 0, ebp: 0, esi: 0, edi: 0, eflags: 0,
        fpu: Fpu { stack: [0.0; 8], top: 0, control: 0x37f },
        xmm: [Xmm::default(); 5], xmm_hi: [Xmm::default(); 3],
    };
    for (va, arg, expected) in [(0x58690, clip, -1.0), (0x5a220, object, 0.0)] {
        m.write32(STACK + 4, arg);
        let before = tsfp_ports::anim_lane::guard_counters();
        assert!(anim_lane3::run(va, &mut m, &mut Guest, &mut regs));
        assert_eq!(regs.fpu.st(0), expected);
        assert_eq!(tsfp_ports::anim_lane::guard_counters(), before);
    }
}

// Each shadow control needs a fresh process: the shadow selector caches its environment.
#[test]
fn shadow_missing_track_case() {
    let Ok(case) = std::env::var("T1986_SHADOW_CASE") else { return };
    unsafe { std::env::set_var("TSFP_ANIM_SHADOW", "1") };
    let va = if case.starts_with("heading") { 0x58690 } else { 0x5a220 };
    let expected = if va == 0x58690 { -1.0 } else { 0.0 };
    struct Original { va: u32, value: f64, calls: usize }
    impl GuestCall for Original {
        fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
            assert_eq!(va, self.va);
            self.calls += 1;
            regs.fpu.push(self.value);
        }
    }
    let mut guest = Original { va, value: if case.ends_with("mismatch") { 2.0 } else { expected }, calls: 0 };
    let mut m = Fake(vec![0; SIZE]);
    let (clip, object, body) = (0x2_0000, 0x2_1000, 0x2_2000);
    m.write32(clip + 0x10, 2);
    m.write32(object + 4, body);
    m.write32(body + 0xd8, 0);
    m.write32(0x7e3954, 0x2_3000);
    m.write32(0x2_3000, clip);
    let mut regs = ExtRegisters {
        eax: 0, ecx: 0, edx: 0, esp: STACK, ebx: 0, ebp: 0, esi: 0, edi: 0, eflags: 0,
        fpu: Fpu { stack: [0.0; 8], top: 0, control: 0x37f },
        xmm: [Xmm::default(); 5], xmm_hi: [Xmm::default(); 3],
    };
    m.write32(STACK + 4, if va == 0x58690 { clip } else { object });
    let before = tsfp_ports::anim_lane::guard_counters();
    assert!(anim_lane3::run(va, &mut m, &mut guest, &mut regs));
    assert_eq!(guest.calls, 1);
    assert_eq!(tsfp_ports::anim_lane::guard_counters(), before);
    assert_eq!(regs.fpu.st(0), expected);
}

#[test]
fn missing_track_shadow_compares_the_return_instead_of_suppressing_it() {
    for root in ["heading", "current"] {
        for mismatch in [false, true] {
            let case = format!("{root}-{}", if mismatch { "mismatch" } else { "equal" });
            let out = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "shadow_missing_track_case", "--nocapture"])
                .env("T1986_SHADOW_CASE", case)
                .env_remove("TSFP_ANIM_MUTATE")
                .output().unwrap();
            let stderr = String::from_utf8_lossy(&out.stderr);
            if mismatch {
                assert!(!out.status.success());
                assert!(stderr.contains("ANIM-SHADOW double"), "{stderr}");
            } else {
                assert!(out.status.success(), "{stderr}");
                assert!(!stderr.contains("ANIM-SHADOW double"));
            }
            assert!(!stderr.contains("guard-degraded") && !stderr.contains("guard-rollback"), "{stderr}");
        }
    }
}
