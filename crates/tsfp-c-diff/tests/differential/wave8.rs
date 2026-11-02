// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 8: scalar-SSE roots. Adds XMM0-7 (in and out, raw 128 bits) and an MXCSR control
//! check to the differential, plus a model-versus-hardware test of the scalar op model in
//! `tsfp-engine-memory/src/sse.rs`. See docs/t1836-rust-wave8.md for the model and its limits.
use super::*;
use tsfp_engine_physics::{fp_leaves, gprs::Gprs as PhysGprs, sse};

unsafe extern "C" {
    fn shim_call_fp(va: u32, regs: *mut u32, xmm: *mut u32, mxcsr: *mut u32) -> i32;
    fn shim_ss_op(op: i32, a: u32, b: u32) -> u32;
    fn shim_compare_ah(signalling: i32, a: u32, b: u32) -> u32;
    fn shim_cvttss2si(a: u32) -> u32;
    fn shim_cvtsi2ss(a: u32) -> u32;
}

const POOL_A: u32 = 0x0040_0000;
const POOL_B: u32 = 0x0050_0000;
const POOL_C: u32 = 0x0060_0000;

type Xmms = [[u32; 4]; 8];

/// A float bit pattern biased to the classes the proof uses: signed zero, denormal, normal,
/// huge, infinity, quiet and signalling NaN with varied payloads, and a few exact constants.
fn float_bits(rng: &mut Rng) -> u32 {
    let sign = if rng.below(2) == 0 { 0 } else { 0x8000_0000 };
    if rng.below(24) == 0 {
        // Integer-conversion boundaries (2^31 and its neighbours) and the largest finite value.
        return [0x4f00_0000, 0xcf00_0000, 0x4eff_ffff, 0xcf00_0001, 0x7f7f_ffff, 0x4b00_0000]
            [rng.below(6) as usize];
    }
    sign | match rng.below(12) {
        0 => 0,
        1 => rng.next() & 0x007f_ffff,
        2 => 0x3f80_0000,
        3 => 0x3f00_0000 + (rng.next() & 0x00ff_ffff),
        4 => 0x7f80_0000,
        5 => 0x7f80_0000 | (rng.next() & 0x007f_ffff),
        6 => 0x7fc0_0000 | (rng.next() & 0x003f_ffff),
        7 => 0x7f00_0000 | (rng.next() & 0x00ff_ffff),
        8 => 0x0080_0000 | (rng.next() & 0x000f_ffff),
        _ => rng.next() & 0x7fff_ffff,
    }
}

fn random_xmm(rng: &mut Rng) -> Xmms {
    let mut x = [[0u32; 4]; 8];
    for reg in &mut x {
        for lane in reg.iter_mut() {
            *lane = if rng.below(2) == 0 { float_bits(rng) } else { rng.next() };
        }
    }
    x
}

struct FpCase {
    case: Case,
    xmm: Xmms,
}

/// Flips one bit of one XMM word, any register and lane (kinds 4 and up of the control).
fn perturb_xmm(index: u32, xmm: &mut Xmms) {
    xmm[(index / 4 % 8) as usize][(index / 32 % 4) as usize] ^= 1 << (index % 32);
}

/// Like `drive` but the root also reads and writes XMM0-7 and runs under MXCSR 0x1F80. The
/// mutant control perturbs the Rust result: kinds 0-3 as `drive`, kind 4 flips one XMM bit.
fn fp_drive(
    seed: u64,
    mutant: bool,
    mut build: impl FnMut(&mut Rng, &mut Pair) -> FpCase,
    port: impl Fn(&mut View, &FpCase, &mut Xmms) -> Outcome,
) -> (usize, usize) {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let pair = &mut *guard;
    let mut rng = Rng(seed);
    let (mut detected, mut total) = (0, 0);
    let mut sticky_cases = 0usize;
    for index in 0..CASES {
        let mut fp = build(&mut rng, pair);
        let logged: Vec<u32> = pair.log.drain(..).collect();
        // The stack arguments are always compared (a root may use its argument slots as scratch).
        let slots = 4 * fp.case.args.len().max(1) as u32;
        fp.case.watch.insert(0, (STACK + 4, slots));
        fp.case
            .watch
            .extend(logged.into_iter().map(|address| (address, 4)));
        pair.put32(STACK, 0);
        for (i, arg) in fp.case.args.iter().enumerate() {
            pair.put32(STACK + 4 + 4 * i as u32, *arg);
        }
        pair.log.clear();
        let case = &fp.case;
        let mut regs = [case.regs[0], case.regs[1], case.regs[2], case.esi, STACK, 0];
        let mut c_xmm = fp.xmm;
        let mut mxcsr = 0u32;
        assert_eq!(
            unsafe {
                shim_call_fp(
                    case.va,
                    regs.as_mut_ptr(),
                    c_xmm.as_mut_ptr().cast(),
                    &mut mxcsr,
                )
            },
            1,
            "unknown root"
        );
        assert_eq!(mxcsr & !0x3f, 0x1f80, "C body changed the MXCSR control bits");
        sticky_cases += usize::from(mxcsr & 0x3f != 0);
        let mut view = View(pair.rust);
        let mut rust_xmm = fp.xmm;
        let mut expected = port(&mut view, &fp, &mut rust_xmm);
        if mutant {
            match index as u32 % 5 {
                4 => perturb_xmm(rng.next(), &mut rust_xmm),
                kind => perturb(kind, &mut expected, &mut view, &case.watch),
            }
        }
        let mut mismatch = lifted_policy::mismatch(
            case.va,
            u8::from(regs[0] != expected.eax)
                | u8::from(regs[1] != expected.ecx.unwrap_or(case.regs[1])) << 1
                | u8::from(regs[2] != expected.edx.unwrap_or(case.regs[2])) << 2
                | u8::from(regs[3] != case.esi) << 4
                | u8::from(regs[4] != STACK + 4) << 7,
        ) || lifted_policy::xmm_mismatch(case.va, lifted_policy::xmm_diff(case.va, &c_xmm, &rust_xmm));
        for (start, len) in &case.watch {
            for offset in 0..*len {
                let address = start.wrapping_add(offset);
                let word = |rust: bool| {
                    let base = address & !3;
                    u32::from_le_bytes(std::array::from_fn(|i| pair.get(rust, base.wrapping_add(i as u32))))
                };
                let differs = pair.get(false, address) != pair.get(true, address)
                    && !lifted_policy::nan_equal(case.va, word(false), word(true));
                mismatch |= lifted_policy::mem(case.va, address, differs);
            }
        }
        if mutant {
            total += 1;
            detected += usize::from(mismatch);
            for (start, len) in &case.watch {
                for offset in 0..*len {
                    let address = start.wrapping_add(offset);
                    let byte = pair.get(false, address);
                    unsafe { *pair.rust.add(address as usize) = byte };
                }
            }
        } else {
            assert!(
                !mismatch,
                "root {:08X} case {index}: C regs {regs:x?} xmm {c_xmm:x?}, Rust {expected:x?} xmm {rust_xmm:x?}, args {:x?}",
                case.va, case.args
            );
        }
    }
    eprintln!("fp root: {sticky_cases} of {CASES} cases set MXCSR sticky status (not compared)");
    (detected, total)
}

macro_rules! fp_suite {
    ($name:ident, $mutant:ident, $seed:expr, $build:expr, $port:expr) => {
        #[test]
        fn $name() {
            fp_drive($seed, false, $build, $port);
        }
        #[test]
        fn $mutant() {
            let (detected, total) = fp_drive($seed, true, $build, $port);
            eprintln!(
                "mutant control {}: {detected}/{total} perturbations detected",
                stringify!($name)
            );
            assert!(total > 0);
            assert_eq!(detected, total);
        }
    };
}

fn fp_case(rng: &mut Rng, va: u32, args: Vec<u32>) -> FpCase {
    FpCase {
        case: Case {
            va,
            args,
            regs: [rng.next(), rng.next(), rng.next()],
            esi: rng.next(),
            watch: vec![],
        },
        xmm: random_xmm(rng),
    }
}

fn phys(case: &Case) -> PhysGprs {
    PhysGprs { eax: case.regs[0], ecx: case.regs[1], edx: case.regs[2] }
}

fn done(g: PhysGprs) -> Outcome {
    Outcome { eax: g.eax, ecx: Some(g.ecx), edx: Some(g.edx) }
}

// ---- model versus hardware: the scalar op model against the real instructions ----

#[test]
fn sse_model_matches_hardware_ops() {
    let mut rng = Rng(0x1836_0800);
    let _hold = pair().lock().unwrap_or_else(|e| e.into_inner());
    for index in 0..CASES * 8 {
        let (a, b) = (float_bits(&mut rng), float_bits(&mut rng));
        let pairs: [(i32, fn(u32, u32) -> u32); 4] =
            [(0, sse::addss), (1, sse::subss), (2, sse::mulss), (3, sse::divss)];
        for (op, model) in pairs {
            assert_eq!(unsafe { shim_ss_op(op, a, b) }, model(a, b), "op {op} {a:08x} {b:08x} #{index}");
        }
        for signalling in 0..2 {
            assert_eq!(
                unsafe { shim_compare_ah(signalling, a, b) },
                sse::ucomiss(a, b).lahf_ah(),
                "compare {a:08x} {b:08x}"
            );
        }
        assert_eq!(unsafe { shim_cvttss2si(a) }, sse::cvttss2si(a), "cvttss2si {a:08x}");
        assert_eq!(unsafe { shim_cvtsi2ss(a) }, sse::cvtsi2ss(a), "cvtsi2ss {a:08x}");
    }
}

/// Negative control: a model whose NaN rule prefers the second operand (the T1631 commuting
/// hazard) must disagree with the hardware, proving the test above can fail.
#[test]
fn sse_model_commuted_nan_rule_is_detected() {
    let mut rng = Rng(0x1836_0801);
    let _hold = pair().lock().unwrap_or_else(|e| e.into_inner());
    let mut detected = 0;
    for _ in 0..20_000 {
        let (a, b) = (float_bits(&mut rng), float_bits(&mut rng));
        let commuted = sse::mulss(b, a);
        detected += usize::from(unsafe { shim_ss_op(2, a, b) } != commuted);
    }
    assert!(detected > 0, "two-NaN operand order never distinguished");
}

// ---- 00047B30 physics ray init ----

const UNIT: u32 = 0x0047_5c78;

fn build_ray(rng: &mut Rng, pair: &mut Pair) -> FpCase {
    let direction = rng.pool(POOL_A) & !3;
    let ray = match rng.below(8) {
        0 => direction,
        1 => direction.wrapping_sub(12),
        2 => direction.wrapping_sub(24),
        3 => direction.wrapping_sub(32),
        4 => direction.wrapping_sub(36),
        5 => STACK + 4 + 16 - 36,
        _ => rng.pool(POOL_B),
    };
    let origin = match rng.below(6) {
        0 => ray,
        1 => ray.wrapping_add(12),
        2 => direction,
        _ => rng.pool(POOL_C),
    };
    for i in 0..3 {
        let component = if rng.below(4) == 0 { 0 } else { float_bits(rng) };
        pair.put32(direction + 4 * i, component);
        let word = float_bits(rng);
        pair.put32(origin + 4 * i, word);
    }
    // 0x475c78 is a table constant in the original (1.0f), vary it so the dividend is exercised.
    let unit = if rng.below(3) == 0 { float_bits(rng) } else { 0x3f80_0000 };
    pair.put32(UNIT, unit);
    let length = float_bits(rng);
    let mut fp = fp_case(rng, 0x47b30, vec![ray, origin, direction, length]);
    fp.case.watch.push((ray, 40));
    fp
}

fp_suite!(
    r_00047b30,
    mutant_00047b30,
    0x1836_0830,
    build_ray,
    |v, fp, x| done(fp_leaves::ray_init_from_origin_direction_length(
        &mut *v,
        STACK,
        phys(&fp.case),
        x
    ))
);

// ---- generic builder for the struct-and-constants style scalar-SSE roots (batch A) ----

use tsfp_engine_physics::fp_scalar::{self, Fp};

#[derive(Clone, Copy)]
enum Arg {
    /// Pointer to a random 0x240 byte block. The links are offsets that hold a pointer to a second block.
    Ptr(&'static [u32]),
    /// Pointer as `Ptr`, but null in a quarter of the cases.
    PtrOrNull(&'static [u32]),
    /// Float bit pattern.
    F,
    /// Integer word biased towards the listed values.
    Word(&'static [u32]),
}

/// Source of a tied value: a stack argument or a guest address.
#[derive(Clone, Copy)]
enum Src {
    Arg(usize),
    Mem(u32),
}

/// One third of the cases copy `src` into `[args[pointer] + offset]` so equality branches fire.
type Tie = (usize, u32, Src);

fn fill_block(rng: &mut Rng, pair: &mut Pair, base: u32) {
    for index in 0..0x240 / 4 {
        let value = match rng.below(4) {
            0 | 1 => float_bits(rng),
            2 => rng.word(&[0x200, 1, 2, 0x400, 0x20, 0xffff_ffff, 0x8000_0000, 0x2000, 0x1_0000, 4, 3, 0x4000]),
            _ => rng.next(),
        };
        pair.put32(base + 4 * index, value);
    }
}

fn read32(pair: &Pair, address: u32) -> u32 {
    u32::from_le_bytes(std::array::from_fn(|i| pair.get(false, address.wrapping_add(i as u32))))
}

/// A float in [-0.5, 2.5) so threshold branches against small constants are reachable.
fn nice(rng: &mut Rng) -> u32 {
    ((rng.below(3000) as f32) / 1000.0 - 0.5).to_bits()
}

struct Spec<'a> {
    va: u32,
    args: &'a [Arg],
    floats: &'a [u32],
    ints: &'a [(u32, &'a [u32])],
    written: &'a [u32],
    ties: &'a [Tie],
}

/// Fills `floats` with float patterns and `ints` with special words, builds the stack arguments
/// (pointer arguments get random blocks and sometimes alias an earlier pointer), applies the
/// ties and watches every address in `written` (globals the root stores to).
fn build_generic(rng: &mut Rng, pair: &mut Pair, spec: &Spec) -> FpCase {
    let mut values: Vec<u32> = Vec::new();
    let mut pointers: Vec<u32> = Vec::new();
    for (index, arg) in spec.args.iter().enumerate() {
        let value = match *arg {
            Arg::F => float_bits(rng),
            Arg::Word(specials) => rng.word(specials),
            Arg::PtrOrNull(_) if rng.below(4) == 0 => 0,
            Arg::Ptr(links) | Arg::PtrOrNull(links) => {
                let alias = pointers.iter().rev().find(|_| rng.below(8) == 0).copied();
                let pointer = match alias {
                    Some(prior) => prior.wrapping_add(4 * rng.below(3)),
                    None => rng.pool(POOL_A + 0x10_0000 * index as u32) & !3,
                };
                fill_block(rng, pair, pointer);
                for &offset in links {
                    let target = rng.pool(POOL_C) & !3;
                    fill_block(rng, pair, target);
                    pair.put32(pointer.wrapping_add(offset), target);
                }
                pointers.push(pointer);
                pointer
            }
        };
        values.push(value);
    }
    for &address in spec.floats {
        let value = float_bits(rng);
        pair.put32(address, value);
    }
    for &(address, specials) in spec.ints {
        let value = rng.word(specials);
        pair.put32(address, value);
    }
    for &(pointer_arg, offset, src) in spec.ties {
        if rng.below(3) == 0 {
            let value = match src {
                Src::Arg(index) => values[index],
                Src::Mem(address) => read32(pair, address),
            };
            pair.put32(values[pointer_arg].wrapping_add(offset), value);
        }
    }
    let mut fp = fp_case(rng, spec.va, values);
    fp.case.watch.extend(spec.written.iter().map(|&a| (a, 4)));
    fp
}

macro_rules! fp_port {
    ($path:path) => {
        |v: &mut View, fp: &FpCase, x: &mut Xmms| {
            let mut ctx = Fp { m: &mut *v, x, esp: STACK };
            done($path(&mut ctx, phys(&fp.case)))
        }
    };
}

/// A suite over `build_generic` with a spec literal.
macro_rules! generic_suite {
    ($name:ident, $mutant:ident, $seed:expr, $spec:expr, $port:path) => {
        fp_suite!(
            $name,
            $mutant,
            $seed,
            |r, p| build_generic(r, p, &$spec),
            fp_port!($port)
        );
    };
}

generic_suite!(
    r_00046280,
    mutant_00046280,
    0x1836_0831,
    Spec {
        va: 0x46280,
        args: &[Arg::Word(&[0, 1])],
        floats: &[0x4d_1848, 0x4d_1840],
        ints: &[],
        written: &[0x4d_184c, 0x4d_1850, 0x4d_183c],
        ties: &[]
    },
    fp_scalar::store_arg_and_product
);

generic_suite!(
    r_0005a150,
    mutant_0005a150,
    0x1836_0832,
    Spec {
        va: 0x5a150,
        args: &[Arg::Ptr(&[])],
        floats: &[0x47_5cac],
        ints: &[],
        written: &[],
        ties: &[(0, 0x198, Src::Mem(0x47_5cac))]
    },
    fp_scalar::float_198_differs_from_475cac
);

fp_suite!(
    r_0005a1e0,
    mutant_0005a1e0,
    0x1836_0833,
    |r, p| {
        let spec = Spec {
            va: 0x5a1e0,
            args: &[Arg::Ptr(&[4]), Arg::Word(&[0, 1, 2, 0x8000_0000])],
            floats: &[],
            ints: &[],
            written: &[],
            ties: &[],
        };
        let fp = build_generic(r, p, &spec);
        let inner = read32(p, fp.case.args[0].wrapping_add(4));
        let key = fp.case.args[1];
        for offset in [0x12c, 0xd4] {
            if r.below(3) == 0 {
                p.put32(inner.wrapping_add(offset), key);
            }
        }
        if r.below(2) == 0 {
            let positive = nice(r);
            p.put32(inner.wrapping_add(0x198), positive);
        }
        fp
    },
    fp_port!(fp_scalar::float_198_positive_matches_key)
);

generic_suite!(
    r_00073530,
    mutant_00073530,
    0x1836_0834,
    Spec {
        va: 0x73530,
        args: &[Arg::Ptr(&[])],
        floats: &[0x7b_a188],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_scalar::float_88_copy_and_scale
);

fp_suite!(
    r_000a4ca0,
    mutant_000a4ca0,
    0x1836_0835,
    |r, p| {
        let spec = Spec {
            va: 0xa4ca0,
            args: &[Arg::Ptr(&[])],
            floats: &[0x47_5ce8],
            ints: &[],
            written: &[],
            ties: &[],
        };
        let fp = build_generic(r, p, &spec);
        if r.below(3) != 0 {
            // A window [218, 218 + 0.5) that [AC] falls in or just outside of.
            let object = fp.case.args[0];
            let low = nice(r);
            p.put32(0x47_5ce8, 0.5f32.to_bits());
            p.put32(object + 0x218, low);
            let ac = f32::from_bits(low) + (r.below(1400) as f32 - 200.0) / 1000.0;
            p.put32(object + 0xac, ac.to_bits());
        }
        fp
    },
    fp_port!(fp_scalar::float_ac_within_218_plus_475ce8)
);

generic_suite!(
    r_000aee20,
    mutant_000aee20,
    0x1836_0836,
    Spec {
        va: 0xaee20,
        args: &[Arg::Ptr(&[0x7c, 4])],
        floats: &[0x47_d5a8, 0x4e_794c],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_scalar::actor_float_ac_decrement_or_clear_flag
);

generic_suite!(
    r_000c9bc0,
    mutant_000c9bc0,
    0x1836_0837,
    Spec {
        va: 0xc9bc0,
        args: &[],
        floats: &[0x73_30bc],
        ints: &[(0x73_3108, &[0, 1, 5])],
        written: &[],
        ties: &[]
    },
    fp_scalar::camera_set_and_timer_nonpositive
);

generic_suite!(
    r_00120220,
    mutant_00120220,
    0x1836_0838,
    Spec {
        va: 0x120220,
        args: &[Arg::Ptr(&[]), Arg::F, Arg::F],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[(0, 0xa0, Src::Arg(1))]
    },
    fp_scalar::store_float_pair_a0_a4
);

generic_suite!(
    r_00151f20,
    mutant_00151f20,
    0x1836_0839,
    Spec {
        va: 0x151f20,
        args: &[Arg::Ptr(&[]), Arg::Ptr(&[])],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_scalar::transform_vector_by_matrix_30_38
);

generic_suite!(
    r_00347fa0,
    mutant_00347fa0,
    0x1836_083a,
    Spec {
        va: 0x347fa0,
        args: &[Arg::F],
        floats: &[0x47_5c78],
        ints: &[],
        written: &[0x76_70fc],
        ties: &[]
    },
    fp_scalar::store_float_clamped_0_to_475c78
);

generic_suite!(
    r_000d1e20,
    mutant_000d1e20,
    0x1836_083b,
    Spec {
        va: 0xd1e20,
        args: &[Arg::Ptr(&[]), Arg::F, Arg::F, Arg::F],
        floats: &[0x47_8620, 0x47_5d10],
        ints: &[(0x79_094c, &[0x63, 0x64, 0x65, 0x8000_0000, 0x7fff_ffff, 0])],
        written: &[],
        ties: &[]
    },
    fp_scalar::store_clamped_delta_target
);

generic_suite!(
    r_00238d90,
    mutant_00238d90,
    0x1836_083c,
    Spec {
        va: 0x238d90,
        args: &[Arg::F, Arg::Ptr(&[])],
        floats: &[0x49_ef8c, 0x47_5db8],
        ints: &[(0x4c_0454, &[0, 1, 2])],
        written: &[],
        ties: &[]
    },
    fp_scalar::store_float_divided_by_flag
);

fp_suite!(
    r_002daf90,
    mutant_002daf90,
    0x1836_083d,
    |r, p| {
        let spec = Spec {
            va: 0x2daf90,
            args: &[Arg::Ptr(&[]), Arg::F],
            floats: &[0x47_5cac, 0x47_5c84, 0x47_5c88],
            ints: &[],
            written: &[],
            ties: &[],
        };
        let fp = build_generic(r, p, &spec);
        if r.below(3) != 0 {
            let object = fp.case.args[0];
            p.put32(0x47_5cac, 0);
            p.put32(0x47_5c84, 1.0f32.to_bits());
            p.put32(0x47_5c88, (1.0f32 + r.below(3) as f32 / 4.0).to_bits());
            let (phase, rate) = (nice(r), nice(r));
            p.put32(object + 0x20, phase);
            p.put32(object + 0x24, rate);
            let delta = (r.below(1500) as f32 / 1000.0).to_bits();
            let mut fp = fp;
            fp.case.args[1] = delta;
            let latch = [0u32, 1, 1, 2][r.below(4) as usize];
            p.put32(object + 0x3c, latch);
            let counter = [0u32, 1, 0xffff_ffff][r.below(3) as usize];
            p.put32(object + 0x38, counter);
            return fp;
        }
        fp
    },
    fp_port!(fp_scalar::menu_background_advance_phase)
);

generic_suite!(
    r_00309120,
    mutant_00309120,
    0x1836_083e,
    Spec {
        va: 0x309120,
        args: &[Arg::F, Arg::F],
        floats: &[0x47_5c78],
        ints: &[],
        written: &[0x78_3f2c, 0x78_3f28, 0x78_3dd4, 0x78_3f20],
        ties: &[]
    },
    fp_scalar::set_scale_pair_and_reciprocals
);

generic_suite!(
    r_00341980,
    mutant_00341980,
    0x1836_083f,
    Spec {
        va: 0x341980,
        args: &[Arg::Ptr(&[]), Arg::Word(&[0, 1, 2, 3]), Arg::Word(&[0, 1, 5]), Arg::F],
        floats: &[0x47_5cac, 0x47_5c78],
        ints: &[],
        written: &[],
        ties: &[(0, 0x78, Src::Arg(1))]
    },
    fp_scalar::weather_slot_set_state_with_fade
);

generic_suite!(
    r_003441a0,
    mutant_003441a0,
    0x1836_0840,
    Spec {
        va: 0x3441a0,
        args: &[Arg::Ptr(&[]), Arg::Ptr(&[])],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_scalar::translate_anchor_and_dependent_points
);

generic_suite!(
    r_001543a0,
    mutant_001543a0,
    0x1836_0841,
    Spec {
        va: 0x1543a0,
        args: &[Arg::Ptr(&[]), Arg::F, Arg::Ptr(&[]), Arg::Ptr(&[]), Arg::Ptr(&[]), Arg::Ptr(&[])],
        floats: &[0x47_5c78, 0x47_5d18],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_scalar::ui_curved_menu_bezier_sample_point
);

generic_suite!(
    r_00154470,
    mutant_00154470,
    0x1836_0842,
    Spec {
        va: 0x154470,
        args: &[Arg::Ptr(&[]), Arg::F, Arg::Ptr(&[]), Arg::Ptr(&[]), Arg::Ptr(&[]), Arg::Ptr(&[])],
        floats: &[0x47_5c78, 0x47_5d18],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_scalar::bezier_sample_point_vec3
);

// ---- batch B: further scalar-SSE roots (game_fp_math.c and the vector census sources) ----

use tsfp_engine_physics::fp_scalar_b;

generic_suite!(
    r_00150580,
    mutant_00150580,
    0x1836_0850,
    Spec {
        va: 0x150580,
        args: &[Arg::Ptr(&[]), Arg::Ptr(&[]), Arg::Ptr(&[])],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_scalar_b::quat_multiply_to_out
);

generic_suite!(
    r_00151190,
    mutant_00151190,
    0x1836_0851,
    Spec {
        va: 0x151190,
        args: &[Arg::F; 10],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_scalar_b::point_in_triangle_2d
);

generic_suite!(
    r_00152120,
    mutant_00152120,
    0x1836_0852,
    Spec {
        va: 0x152120,
        args: &[Arg::Ptr(&[]), Arg::Ptr(&[]), Arg::Ptr(&[])],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_scalar_b::matrix4x4_transform_direction_vec4_copy
);

fp_suite!(
    r_001515e0,
    mutant_001515e0,
    0x1836_0853,
    |r, p| {
        let spec = Spec {
            va: 0x1515e0,
            args: &[Arg::Ptr(&[]), Arg::F, Arg::F],
            floats: &[],
            ints: &[],
            written: &[],
            ties: &[],
        };
        let mut fp = build_generic(r, p, &spec);
        // The root uses stack slot esp+8 as scratch, so the stack arguments are compared too.
        fp.case.watch.push((STACK + 4, 12));
        match r.below(10) {
            0 => fp.case.args[0] = STACK + 8,
            1 => fp.case.args[0] = STACK + 12,
            2 => {
                let target = nice(r);
                fp.case.args[1] = target;
                fp.case.args[2] = (r.below(1000) as f32 / 1000.0).to_bits();
                p.put32(fp.case.args[0], nice(r));
            }
            _ => {}
        }
        fp
    },
    fp_port!(fp_scalar_b::float_approach_target_step)
);

generic_suite!(
    r_001553b0,
    mutant_001553b0,
    0x1836_0854,
    Spec {
        va: 0x1553b0,
        args: &[Arg::F],
        floats: &[0x47_5c78, 0x74_a990],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_scalar_b::global_74a990_raise_to_clamped_arg
);

fp_suite!(
    r_0012b8e0,
    mutant_0012b8e0,
    0x1836_0855,
    |r, p| {
        let spec = Spec {
            va: 0x12b8e0,
            args: &[Arg::F],
            floats: &[0x47_5d24, 0x47_5d1c, 0x47_85d8],
            ints: &[(0x74_50d0, &[0, 1, 2, 0xc8, 0xc9, 0x8000_0000, 0x8000_0001])],
            written: &[],
            ties: &[],
        };
        let mut fp = build_generic(r, p, &spec);
        let mut index = read32(p, 0x74_50d0).wrapping_sub(1);
        if index >> 31 != 0 {
            index = index.wrapping_add(0xc8);
        }
        let target = index.wrapping_mul(0x38).wrapping_add(0x74_2510).wrapping_add(0x28);
        fp.case.watch.push((target, 4));
        if r.below(2) == 0 {
            // Scale 1, range [0, 1] around the clamp constants so every clamp arm fires.
            p.put32(0x47_5d24, 1.0f32.to_bits());
            p.put32(0x47_5d1c, 1.0f32.to_bits());
            p.put32(0x47_85d8, 0.0f32.to_bits());
            fp.case.args[0] = nice(r);
        }
        fp
    },
    fp_port!(fp_scalar_b::ring_742510_set_prev_entry_field_0x28_clamped)
);

fp_suite!(
    r_0011fdc0,
    mutant_0011fdc0,
    0x1836_0856,
    |r, p| {
        let spec = Spec {
            va: 0x11fdc0,
            args: &[Arg::Ptr(&[]), Arg::F, Arg::Word(&[0, 1])],
            floats: &[0x47_5c78],
            ints: &[],
            written: &[],
            ties: &[],
        };
        let mut fp = build_generic(r, p, &spec);
        if r.below(2) == 0 {
            p.put32(0x47_5c78, 1.0f32.to_bits());
            fp.case.args[1] = nice(r);
        }
        fp
    },
    fp_port!(fp_scalar_b::effect_set_ratio_0x98_when_in_unit_range)
);

generic_suite!(
    r_00309160,
    mutant_00309160,
    0x1836_0857,
    Spec {
        va: 0x309160,
        args: &[Arg::F, Arg::F],
        floats: &[0x78_3dd8, 0x78_3cc0, 0x78_3de0, 0x78_3ddc],
        ints: &[],
        written: &[0x78_3dc0, 0x78_3dcc],
        ties: &[]
    },
    fp_scalar_b::accumulate_scroll_globals
);

generic_suite!(
    r_00319440,
    mutant_00319440,
    0x1836_0858,
    Spec {
        va: 0x319440,
        args: &[Arg::Ptr(&[]), Arg::Ptr(&[])],
        floats: &[0x47_60dc, 0x47_60cc],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_scalar_b::clamp_pair_to_limits
);

generic_suite!(
    r_0036e920,
    mutant_0036e920,
    0x1836_0859,
    Spec {
        va: 0x36e920,
        args: &[Arg::Ptr(&[]), Arg::F, Arg::F],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_scalar_b::add_pairs_20_to_0
);

generic_suite!(
    r_0036ec40,
    mutant_0036ec40,
    0x1836_085a,
    Spec {
        va: 0x36ec40,
        args: &[Arg::Ptr(&[]), Arg::F, Arg::F],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_scalar_b::add_pairs_10_to_0
);

generic_suite!(
    r_00377560,
    mutant_00377560,
    0x1836_085b,
    Spec {
        va: 0x377560,
        args: &[Arg::Ptr(&[]), Arg::F, Arg::F, Arg::F, Arg::F],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_scalar_b::store_converted_float_quad
);

fp_suite!(
    r_00307e60,
    mutant_00307e60,
    0x1836_085c,
    |r, p| {
        let spec = Spec {
            va: 0x307e60,
            args: &[Arg::F, Arg::F, Arg::F],
            floats: &[],
            ints: &[],
            written: &[],
            ties: &[],
        };
        let mut fp = build_generic(r, p, &spec);
        let table = r.pool(POOL_B) & !3;
        p.put32(0x78_4004, table);
        if r.below(4) != 0 {
            // A cell inside the 40 x 40 x 6 grid, in range, with a filled table word.
            let x = r.below(4400) as f32 / 100.0 - 2.0;
            let y = r.below(4400) as f32 / 100.0 - 2.0;
            let z = r.below(800) as f32 / 100.0 - 3.0;
            fp.case.args = vec![x.to_bits(), y.to_bits(), z.to_bits()];
            let (cx, cy, cz) = (x as i32, y as i32, z as i32 + 2);
            let cell = cx.wrapping_add(40i32.wrapping_mul(cy.wrapping_add(40i32.wrapping_mul(cz))));
            let address = table.wrapping_add((cell as u32).wrapping_mul(4));
            let word = r.next();
            p.put32(address, word);
        }
        fp
    },
    fp_port!(fp_scalar_b::float_grid_cell_lookup)
);

// ---- batch C: legacy-SSE move roots (xmm0 loads, stores and clears) ----

use tsfp_engine_physics::fp_vector;

generic_suite!(
    r_000425e0,
    mutant_000425e0,
    0x18360901,
    Spec {
        va: 0x425e0,
        args: &[],
        floats: &[0x475d18, 0x6b98b0],
        ints: &[(0x7de458, &[0x0, 0x800000, 0x800001, 0xffffffff]), (0x6b98a4, &[0x3, 0x5, 0x4, 0x0, 0x6])],
        written: &[],
        ties: &[]
    },
    fp_vector::promote_mode_3_or_5_to_5
);

generic_suite!(
    r_00046210,
    mutant_00046210,
    0x18360902,
    Spec {
        va: 0x46210,
        args: &[],
        floats: &[0x4785d4, 0x4785c8, 0x475c78, 0x4d1838, 0x4d1844, 0x4d183c, 0x4d1840, 0x4d1848, 0x6d5ec0],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::init_float_block_4d1838
);

generic_suite!(
    r_00065230,
    mutant_00065230,
    0x18360903,
    Spec {
        va: 0x65230,
        args: &[Arg::Word(&[0, 1, 5])],
        floats: &[0x7de300],
        ints: &[(0x7de30c, &[0x0, 0x1, 0x5])],
        written: &[],
        ties: &[]
    },
    fp_vector::set_7de30c_clear_float_7de300
);

generic_suite!(
    r_000746a0,
    mutant_000746a0,
    0x18360904,
    Spec {
        va: 0x746a0,
        args: &[Arg::Ptr(&[])],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::object_clear_three
);

generic_suite!(
    r_000aee60,
    mutant_000aee60,
    0x18360905,
    Spec {
        va: 0xaee60,
        args: &[Arg::Ptr(&[0x7c])],
        floats: &[0x475d18],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::extension_store_a8
);

generic_suite!(
    r_000b2d20,
    mutant_000b2d20,
    0x18360906,
    Spec {
        va: 0xb2d20,
        args: &[Arg::Ptr(&[])],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::gp_clear_object_c0_c8
);

generic_suite!(
    r_000c9f60,
    mutant_000c9f60,
    0x18360907,
    Spec {
        va: 0xc9f60,
        args: &[Arg::F, Arg::F],
        floats: &[0x7330ac, 0x7330b0],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::store_float_arg_pair
);

generic_suite!(
    r_000e10e0,
    mutant_000e10e0,
    0x18360908,
    Spec {
        va: 0xe10e0,
        args: &[Arg::PtrOrNull(&[]), Arg::F],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::object_store_9c
);

generic_suite!(
    r_000ff3d0,
    mutant_000ff3d0,
    0x18360909,
    Spec {
        va: 0xff3d0,
        args: &[Arg::PtrOrNull(&[]), Arg::F],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::effect_store_38
);

generic_suite!(
    r_0010ea50,
    mutant_0010ea50,
    0x1836090a,
    Spec {
        va: 0x10ea50,
        args: &[Arg::PtrOrNull(&[]), Arg::F],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::object_set_field_0x60_mark_dirty
);

generic_suite!(
    r_0010ea90,
    mutant_0010ea90,
    0x1836090b,
    Spec {
        va: 0x10ea90,
        args: &[Arg::PtrOrNull(&[]), Arg::F],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::object_set_field_0x5c_mark_dirty
);

generic_suite!(
    r_0010eab0,
    mutant_0010eab0,
    0x1836090c,
    Spec {
        va: 0x10eab0,
        args: &[Arg::PtrOrNull(&[]), Arg::F],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::object_set_field_0x84_mark_dirty_0x4
);

generic_suite!(
    r_0010eb10,
    mutant_0010eb10,
    0x1836090d,
    Spec {
        va: 0x10eb10,
        args: &[Arg::PtrOrNull(&[]), Arg::F, Arg::F],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::object_set_fields_0x68_0x6c
);

generic_suite!(
    r_0010eb50,
    mutant_0010eb50,
    0x1836090e,
    Spec {
        va: 0x10eb50,
        args: &[Arg::PtrOrNull(&[]), Arg::F],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::object_set_field_0x70
);

generic_suite!(
    r_0010eb90,
    mutant_0010eb90,
    0x1836090f,
    Spec {
        va: 0x10eb90,
        args: &[Arg::PtrOrNull(&[]), Arg::F],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::object_set_field_0x74
);

generic_suite!(
    r_0010ecb0,
    mutant_0010ecb0,
    0x18360910,
    Spec {
        va: 0x10ecb0,
        args: &[Arg::PtrOrNull(&[]), Arg::F],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::object_set_field_0x64
);

generic_suite!(
    r_00119ab0,
    mutant_00119ab0,
    0x18360911,
    Spec {
        va: 0x119ab0,
        args: &[Arg::Word(&[0, 1, 5]), Arg::Word(&[0, 7, 0x4e63f8])],
        floats: &[0x47d580, 0x7ac814, 0x7ac818, 0x7ac81c, 0x7ac820, 0x7ac810, 0x7ac800, 0x7ac804, 0x7ac808, 0x7ac80c],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::init_state_block_7ac800
);

generic_suite!(
    r_0011f510,
    mutant_0011f510,
    0x18360912,
    Spec {
        va: 0x11f510,
        args: &[Arg::Ptr(&[])],
        floats: &[0x47cff4],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::object_set_field_0xa8_default
);

generic_suite!(
    r_0011f540,
    mutant_0011f540,
    0x18360913,
    Spec {
        va: 0x11f540,
        args: &[Arg::Ptr(&[])],
        floats: &[0x4782c4],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::object_set_field_0xac_default
);

generic_suite!(
    r_0011f570,
    mutant_0011f570,
    0x18360914,
    Spec {
        va: 0x11f570,
        args: &[Arg::Ptr(&[])],
        floats: &[0x47cff4],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::object_set_field_0xb0_default
);

generic_suite!(
    r_0011fe40,
    mutant_0011fe40,
    0x18360915,
    Spec {
        va: 0x11fe40,
        args: &[Arg::Ptr(&[]), Arg::F],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::effect_arm_mode_2
);

generic_suite!(
    r_0011fe80,
    mutant_0011fe80,
    0x18360916,
    Spec {
        va: 0x11fe80,
        args: &[Arg::Ptr(&[]), Arg::F],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::effect_arm_mode_1
);

generic_suite!(
    r_00120ff0,
    mutant_00120ff0,
    0x18360917,
    Spec {
        va: 0x120ff0,
        args: &[Arg::Ptr(&[])],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::effect_flag_0x2000_arm
);

generic_suite!(
    r_0012c130,
    mutant_0012c130,
    0x18360918,
    Spec {
        va: 0x12c130,
        args: &[],
        floats: &[0x475c78, 0x4e6680, 0x4e6684, 0x4e6688, 0x4e668c, 0x4e6690, 0x4e6694, 0x7abf7c, 0x7abf64, 0x7abf68, 0x7abf6c, 0x7abf70, 0x7abf74, 0x7abf60, 0x7abf78],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::init_float_block_7abf60
);

generic_suite!(
    r_00155040,
    mutant_00155040,
    0x18360919,
    Spec {
        va: 0x155040,
        args: &[],
        floats: &[0x74984c, 0x749850, 0x749854, 0x749858, 0x74985c, 0x749860, 0x749864, 0x749868, 0x74986c, 0x749870, 0x749874, 0x749878, 0x74987c, 0x749880, 0x7a3890, 0x7a388c, 0x749884],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::clear_globals_74984c_to_749884
);

generic_suite!(
    r_00155410,
    mutant_00155410,
    0x1836091a,
    Spec {
        va: 0x155410,
        args: &[Arg::Word(&[0, 1, 5])],
        floats: &[0x4e7968, 0x74a990],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::set_global_4e7968_reset_74a990
);

generic_suite!(
    r_00196170,
    mutant_00196170,
    0x1836091b,
    Spec {
        va: 0x196170,
        args: &[],
        floats: &[0x4e8cec, 0x756e7c, 0x74fe24],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::copy_global_4e8cec_and_clear_float
);

generic_suite!(
    r_001b5680,
    mutant_001b5680,
    0x1836091c,
    Spec {
        va: 0x1b5680,
        args: &[Arg::Ptr(&[]), Arg::Word(&[0, 1, 5])],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::set_dword_44_and_clear_float_48
);

generic_suite!(
    r_0024c920,
    mutant_0024c920,
    0x1836091d,
    Spec {
        va: 0x24c920,
        args: &[],
        floats: &[0x78b600, 0x78b664, 0x78b668, 0x78b66c, 0x78b670],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::reset_state_block_78b600
);

generic_suite!(
    r_00309080,
    mutant_00309080,
    0x1836091e,
    Spec {
        va: 0x309080,
        args: &[],
        floats: &[0x783f2c, 0x783f28, 0x783dd4, 0x783f20, 0x762d58, 0x762d5c, 0x762d60, 0x762d64],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::copy_four_floats_783f_to_762d
);

generic_suite!(
    r_003090d0,
    mutant_003090d0,
    0x1836091f,
    Spec {
        va: 0x3090d0,
        args: &[],
        floats: &[0x783f2c, 0x783f28, 0x783dd4, 0x783f20, 0x762d58, 0x762d5c, 0x762d60, 0x762d64],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::copy_four_floats_762d_to_783f
);

generic_suite!(
    r_00341630,
    mutant_00341630,
    0x18360920,
    Spec {
        va: 0x341630,
        args: &[],
        floats: &[0x7789dc, 0x7789f4, 0x7789e4],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::clear_7789dc_group
);

generic_suite!(
    r_00341a10,
    mutant_00341a10,
    0x18360921,
    Spec {
        va: 0x341a10,
        args: &[Arg::PtrOrNull(&[]), Arg::F],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::object_set_state_3_store_float_0xc
);

generic_suite!(
    r_00349390,
    mutant_00349390,
    0x18360922,
    Spec {
        va: 0x349390,
        args: &[Arg::Ptr(&[]), Arg::Ptr(&[]), Arg::Ptr(&[])],
        floats: &[],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_vector::init_entry_copy_two_vec3
);

macro_rules! custom_vector_suite {
    ($name:ident, $mutant:ident, $seed:expr, $va:expr, $args:expr, $post:expr, $port:path) => {
        fp_suite!(
            $name,
            $mutant,
            $seed,
            |r, p| {
                let spec = Spec {
                    va: $va,
                    args: $args,
                    floats: &[],
                    ints: &[],
                    written: &[],
                    ties: &[],
                };
                let mut fp = build_generic(r, p, &spec);
                let post: fn(&mut Rng, &mut Pair, &mut FpCase) = $post;
                post(r, p, &mut fp);
                fp
            },
            fp_port!($port)
        );
    };
}

/// A random word written to `address` so a store there is detectable and watched.
fn scribble(r: &mut Rng, p: &mut Pair, address: u32) {
    let word = if r.below(2) == 0 { float_bits(r) } else { r.next() };
    p.put32(address, word);
}

custom_vector_suite!(
    r_000c9440, mutant_000c9440, 0x1836_0950, 0xc9440,
    &[Arg::Ptr(&[])],
    |r, p, fp| {
        for address in [0x7b_0cd0u32, 0x7b_0cd4, 0x7b_0cd8] {
            scribble(r, p, address);
        }
        if r.below(4) == 0 {
            // The source overlaps the destination globals, so each read sees the earlier store.
            fp.case.args[0] = [0x7b_0cd0u32, 0x7b_0ccc, 0x7b_0cd4, 0x7b_0cc8][r.below(4) as usize];
        }
    },
    fp_vector::ordered_vector_globals
);

custom_vector_suite!(
    r_0010e970, mutant_0010e970, 0x1836_0951, 0x10e970,
    &[Arg::F],
    |r, p, _fp| {
        let object = if r.below(4) == 0 { 0 } else { r.pool(POOL_B) & !3 };
        if object != 0 {
            fill_block(r, p, object);
        }
        p.put32(0x7a_d28c, object);
    },
    fp_vector::set_obj_7ad28c_float_ac_flag_20000
);

custom_vector_suite!(
    r_0012b940, mutant_0012b940, 0x1836_0952, 0x12b940,
    &[Arg::F],
    |r, p, fp| {
        let counter = [0u32, 1, 2, 0xc8, 0xc9, 0x8000_0000, r.next()][r.below(7) as usize];
        p.put32(0x74_50d0, counter);
        let mut index = counter.wrapping_sub(1);
        if (index as i32) < 0 {
            index = index.wrapping_add(0xc8);
        }
        let target = index.wrapping_mul(0x38).wrapping_add(0x74_253c);
        scribble(r, p, target);
        fp.case.watch.push((target, 4));
    },
    fp_vector::ring_742510_set_prev_entry_field_0x2c
);

custom_vector_suite!(
    r_0012de80, mutant_0012de80, 0x1836_0953, 0x12de80,
    &[Arg::Word(&[0, 1, 2, 3, 0x7fff_ffff, 0x8000_0000, 0xffff_ffff]), Arg::Word(&[0, 1, 2])],
    |r, p, fp| {
        let group = fp.case.args[0].wrapping_mul(0x177c).wrapping_add(0x7a_78c8);
        for offset in [0u32, 4, 8, 0xc] {
            scribble(r, p, group.wrapping_add(offset));
        }
        if r.below(2) == 0 {
            p.put32(group, fp.case.args[1]);
        }
        p.put32(0x47_5d10, float_bits(r));
    },
    fp_vector::slot_7a6160_reset_group_if_equal
);

custom_vector_suite!(
    r_001427d0, mutant_001427d0, 0x1836_0954, 0x1427d0,
    &[Arg::Ptr(&[])],
    |r, p, fp| {
        let slot = [0u32, 1, 2, 5, 0x8000_0000, 0xffff_ffff][r.below(6) as usize];
        p.put32(fp.case.args[0].wrapping_add(4), slot);
        let group = slot.wrapping_mul(0x177c).wrapping_add(0x7a_6160);
        for offset in [0x1750u32, 0x1754, 0x175c, 0x1760] {
            scribble(r, p, group.wrapping_add(offset));
        }
        if r.below(2) == 0 {
            p.put32(group.wrapping_add(0x1750), 0);
        }
    },
    fp_vector::slot_7a6160_set_flag_0x1750
);

custom_vector_suite!(
    r_00177930, mutant_00177930, 0x1836_0955, 0x177930,
    &[Arg::Ptr(&[0x7c]), Arg::F],
    |r, p, fp| {
        let child = read32(p, fp.case.args[0].wrapping_add(0x7c));
        scribble(r, p, child.wrapping_add(0x8fc));
    },
    fp_vector::object_set_child_0x7c_float_0x8fc
);

custom_vector_suite!(
    r_001b62d0, mutant_001b62d0, 0x1836_0956, 0x1b62d0,
    &[Arg::Ptr(&[]), Arg::F],
    |r, p, fp| {
        let object = fp.case.args[0];
        if r.below(2) == 0 {
            let word = read32(p, object.wrapping_add(0xf8));
            p.put32(object.wrapping_add(0xe8), word);
        }
    },
    fp_vector::object_set_flag_0x34_bit3_store_float_0xf0
);

custom_vector_suite!(
    r_002649d0, mutant_002649d0, 0x1836_0957, 0x2649d0,
    &[Arg::Ptr(&[0x7c])],
    |r, p, fp| {
        let child = read32(p, fp.case.args[0].wrapping_add(0x7c));
        scribble(r, p, child.wrapping_add(0x9d0));
        scribble(r, p, child.wrapping_add(0x9e0));
        p.put32(0x47_5c78, float_bits(r));
        if r.below(2) == 0 {
            p.put32(child.wrapping_add(0x14), 0);
        }
        if r.below(3) == 0 {
            p.put32(child.wrapping_add(0x20), 4);
        }
    },
    fp_vector::object_ext_request_state_4
);

// ---- batch D: remaining straight-line scalar-SSE and move roots ----

use tsfp_engine_physics::fp_scalar_d;

generic_suite!(
    r_000a5a60,
    mutant_000a5a60,
    0x1836_0960,
    Spec {
        va: 0xa5a60,
        args: &[Arg::Ptr(&[])],
        floats: &[0x71_5f98, 0x71_5f9c, 0x71_5fa0],
        ints: &[],
        written: &[],
        ties: &[]
    },
    fp_scalar_d::position_sum
);

custom_vector_suite!(
    r_000b0e30, mutant_000b0e30, 0x1836_0961, 0xb0e30,
    &[Arg::PtrOrNull(&[0x7c])],
    |r, p, fp| {
        let object = fp.case.args[0];
        if object == 0 {
            return;
        }
        match r.below(6) {
            0 => p.put32(object + 0x7c, 0),
            1 => {
                let child = read32(p, object + 0x7c);
                p.put32(child + 0x10, 0);
            }
            _ => {}
        }
        let child = read32(p, object + 0x7c);
        if child != 0 {
            let flags = [0x8000u32, 0x400_0000, 0x400_8000, 0, r.next()][r.below(5) as usize];
            p.put32(child + 0xc, flags);
        }
    },
    fp_scalar_d::extension_deactivate
);

custom_vector_suite!(
    r_000ff420, mutant_000ff420, 0x1836_0962, 0xff420,
    &[Arg::PtrOrNull(&[0x18]), Arg::F],
    |r, p, fp| {
        let object = fp.case.args[0];
        if object == 0 {
            return;
        }
        let kind = [0x27u32, 0x1c, 0x1d, 0x23, 0x24, 0xd, 0xe, 7, 0][r.below(9) as usize];
        p.put32(object, kind);
        p.put32(0x47_5c84, if r.below(2) == 0 { 1.0f32.to_bits() } else { float_bits(r) });
        if r.below(2) == 0 {
            fp.case.args[1] = nice(r);
        }
        let child = read32(p, object + 0x18);
        scribble(r, p, child.wrapping_add(0x2ac));
        scribble(r, p, child.wrapping_add(0x138));
    },
    fp_scalar_d::effect_clamp_store
);

custom_vector_suite!(
    r_00124c90, mutant_00124c90, 0x1836_0963, 0x124c90,
    &[Arg::Ptr(&[])],
    |r, p, fp| {
        let object = fp.case.args[0];
        let flags = read32(p, object + 4);
        p.put32(object + 4, if r.below(2) == 0 { flags | 0x20_0000 } else { flags & !0x20_0000 });
        let counter = [2u32, 0, 1][r.below(3) as usize];
        p.put32(object + 0x17c, counter);
        scribble(r, p, 0x47_8900);
    },
    fp_scalar_d::effect_enable
);

custom_vector_suite!(
    r_00124f30, mutant_00124f30, 0x1836_0964, 0x124f30,
    &[Arg::Ptr(&[])],
    |r, p, fp| {
        let object = fp.case.args[0];
        let flags = read32(p, object + 4);
        p.put32(object + 4, if r.below(2) == 0 { flags | 0x800_0000 } else { flags & !0x800_0000 });
        scribble(r, p, 0x47_5cd4);
        if r.below(3) == 0 {
            let word = read32(p, 0x47_5cd4);
            p.put32(object + 0x194, word);
        }
    },
    fp_scalar_d::object_flag_float_predicate
);

custom_vector_suite!(
    r_0022b130, mutant_0022b130, 0x1836_0965, 0x22b130,
    &[Arg::Ptr(&[])],
    |r, p, fp| {
        let object = fp.case.args[0];
        let target = match r.below(6) {
            0 => object,
            1 => object.wrapping_sub(0x7c).wrapping_add(4 * r.below(40)),
            _ => r.pool(POOL_B) & !3,
        };
        fill_block(r, p, target);
        p.put32(0x75_cc98, target);
        scribble(r, p, 0x47_5d00);
        scribble(r, p, 0x47_5c78);
    },
    fp_scalar_d::global_position_copy
);

custom_vector_suite!(
    r_002f7a20, mutant_002f7a20, 0x1836_0966, 0x2f7a20,
    &[],
    |r, p, _fp| {
        for address in [0x78_3ee0u32, 0x78_3dc4, 0x78_3de4, 0x78_3ccc] {
            let word = [0x7fff_ffffu32, 0x8000_0000, 0x100_0001, 0xffff_ffff, 0x300_0001, r.next()][r.below(6) as usize];
            p.put32(address, word);
        }
        for address in [0x78_3dd8u32, 0x78_3cc0, 0x78_3de0, 0x78_40a0, 0x78_40a4, 0x78_40a8, 0x78_40ac, 0x78_40b0, 0x78_40b4, 0x78_40b8, 0x78_40bc] {
            scribble(r, p, address);
        }
        let record = r.pool(POOL_B) & !3;
        fill_block(r, p, record);
        p.put32(0x78_44a8, record);
    },
    fp_scalar_d::scene_global_copy
);

generic_suite!(
    r_00164960,
    mutant_00164960,
    0x1836_0967,
    Spec {
        va: 0x164960,
        args: &[],
        floats: &[0x47_cff4, 0x47_5d28, 0x47_d1f4, 0x74_c3cc, 0x74_c408, 0x74_c3e4, 0x74_c3e8, 0x74_c3ec, 0x74_c3f0, 0x74_c400, 0x74_c404],
        ints: &[(0x79_094c, &[0, 4, 5, 3, 0xffff_ffff])],
        written: &[],
        ties: &[]
    },
    fp_scalar_d::reset_state_block_74c3cc
);

/// Entry table of the 00166060 family: `[74C458] + ((index & 0xFFFF) << 9)`, floats at +130..+160.
fn fill_entry_table(r: &mut Rng, p: &mut Pair, index: u32) {
    let base = r.pool(POOL_B) & !3;
    p.put32(0x74_c458, base);
    let entry = base.wrapping_add((index & 0xffff) << 9);
    for slot in 0..14 {
        let value = float_bits(r);
        p.put32(entry + 0x130 + 4 * slot, value);
    }
}

custom_vector_suite!(
    r_00166060, mutant_00166060, 0x1836_0968, 0x166060,
    &[Arg::Word(&[0, 1, 2, 3, 0x1_0000, 0x1_0001, 0x1_ffff]), Arg::Ptr(&[]), Arg::Ptr(&[])],
    |r, p, fp| fill_entry_table(r, p, fp.case.args[0]),
    fp_scalar_d::entry_point_velocity_cross
);

custom_vector_suite!(
    r_00166140, mutant_00166140, 0x1836_0969, 0x166140,
    &[Arg::Word(&[0, 1, 2, 3, 0x1_0000, 0x1_0001, 0x1_ffff]), Arg::Ptr(&[]), Arg::Ptr(&[]), Arg::Ptr(&[])],
    |r, p, fp| fill_entry_table(r, p, fp.case.args[0]),
    fp_scalar_d::entry_cross_closest_point_on_line
);
