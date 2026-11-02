// SPDX-License-Identifier: GPL-3.0-or-later
//! T2434 Tier 1 differential (WEAK: fabricated seeded pools) for the particle pool step `0x288030`
//! (mask bit 32), run through the real `rea_ai_runtime::run` adapter. The reference is an independent
//! register-level transliteration of the REA disassembly in this file (word offsets relative to the
//! loop pointer EAX = pool + 0xC, exactly as the original addresses them), NOT a lifted body or oracle run.
//! The x87 speed sum is modelled in f64 in both the core and the reference, so that part is a shared
//! assumption rather than an independent check.

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
    rea_ai_runtime,
};

const POOL: u32 = 0x200000;
const STACK: u32 = 0x70000;

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<BTreeMap<u32, u8>>>);

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.borrow().get(&address).copied().unwrap_or(0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
    }
}

struct NoCalls;
impl GuestCall for NoCalls {
    fn call(&mut self, va: u32, _regs: &mut ExtRegisters) {
        panic!("particle step made a guest call to {va:#x}");
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
    fn unit(&mut self) -> f32 {
        (self.next() % 20001) as f32 / 10000.0 - 1.0
    }
}

fn registers() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: STACK,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

/// Register-level reference. `w` is the pool as f32 words, `p` the index of the original's EAX
/// (starts at word 3 = byte 0xC and advances 8 words per entry).
fn reference(w: &mut [f32], div: i32, factor: i32) -> u32 {
    let mut live = 0;
    let mut p = 3usize;
    for _ in 0..500 {
        if 1.0f32 > w[p + 3] {
            let f = (1.0f32 / div as f32) * factor as f32;
            w[p + 3] += f * 0.9;
            w[p - 3] += f * w[p];
            w[p - 2] += f * w[p + 1];
            let life = w[p + 3];
            w[p - 1] += f * w[p + 2];
            if life > 0.75 {
                w[p + 1] += f;
            }
            if 0.0f32 > w[p + 1] && -0.4f32 > w[p - 2] {
                let old_vy = w[p + 1];
                let vz2 = w[p + 2] * w[p + 2];
                let vx2 = w[p] * w[p];
                let new_vy = old_vy * -0.1f32;
                let new_sq = new_vy * new_vy + vz2 + vx2;
                let speed = (f64::from(old_vy) * f64::from(old_vy)
                    + f64::from(vz2)
                    + f64::from(vx2))
                .sqrt() as f32;
                w[p + 1] = new_vy;
                let k = speed * (1.0f32 / new_sq.sqrt());
                w[p] *= k;
                w[p + 1] *= k;
                w[p + 2] *= k;
            }
            live += 1;
        } else {
            w[p + 3] = f32::from_bits(0x7f7fffff);
        }
        p += 8;
    }
    live
}

fn run_seed(rng: &mut Rng, seed: u32) {
    let mut memory = Memory::default();
    memory.write32(0x78b1ec, POOL);
    let (div, factor) = (1 + (rng.next() % 200) as i32, 1 + (rng.next() % 60) as i32);
    memory.write32(0x4e8cec, div as u32);
    memory.write32(0x4e7944, factor as u32);
    let mut words = vec![0f32; 500 * 8 + 4];
    for entry in 0..500 {
        let base = entry * 8;
        for k in 0..3 {
            words[base + k] = rng.unit();
        }
        for k in 3..6 {
            words[base + k] = rng.unit() * 2.0;
        }
        words[base + 6] = match rng.next() % 4 {
            0 => 1.0 + rng.unit().abs(),
            1 => 0.7 + rng.unit().abs() * 0.3,
            _ => rng.unit().abs(),
        };
    }
    for (i, word) in words.iter().enumerate() {
        memory.write32(POOL + 4 * i as u32, word.to_bits());
    }
    let mut want = words.clone();
    let want_live = reference(&mut want, div, factor);
    let mut regs = registers();
    assert!(rea_ai_runtime::run(
        0x288030,
        &mut memory,
        &mut NoCalls,
        &mut regs
    ));
    for entry in 0..500usize {
        for k in 0..7 {
            let at = entry * 8 + k;
            let got = memory.read32(POOL + 4 * at as u32);
            assert_eq!(
                got,
                want[at].to_bits(),
                "seed {seed} entry {entry} field {k}"
            );
        }
        // padding word and untouched words stay as seeded
        assert_eq!(
            memory.read32(POOL + 4 * (entry * 8 + 7) as u32),
            words[entry * 8 + 7].to_bits()
        );
    }
    assert_eq!(
        memory.read32(POOL + 0x3e84),
        want_live,
        "seed {seed} live count"
    );
    assert_eq!(
        (regs.ebx, regs.esi, regs.edi, regs.ebp, regs.esp),
        (0, 0, 0, 0, STACK)
    );
}

#[test]
fn tier1_particle_pool_288030() {
    rea_ai_runtime::configure(true);
    assert!(rea_ai_runtime::configure_function(0x288030, true));
    let mut rng = Rng(0x2434_0000_0000_0001);
    for seed in 0..3000 {
        run_seed(&mut rng, seed);
    }
}

#[test]
fn bounce_branch_is_exercised() {
    // one entry that must bounce: deterministic spot check of the floor-bounce rescale
    let mut p = tsfp_ports::rea_particle_pool::Particle {
        position: [0.0, -0.5, 0.0],
        velocity: [0.0, -1.0, 0.0],
        age: 0.0,
    };
    assert!(tsfp_ports::rea_particle_pool::step_particle(&mut p, 0.0));
    // vy scaled by -0.1 then renormalised back to the old speed 1.0 (pointing up)
    assert!((p.velocity[1] - 1.0).abs() < 1e-6, "{:?}", p.velocity);
}
