// SPDX-License-Identifier: GPL-3.0-or-later
//! T1836 wave 3: large random differential of nine Rust ports against the unmodified C
//! replacements in src/game, which are themselves proven against the original XBE (T1475-T1481
//! harness). A case fills identical random guest memory into a C arena and a Rust arena, runs
//! both and compares EAX/ECX/EDX, ESI, ESP and every watched byte range. The mutant controls
//! perturb the Rust result and must be detected.
use std::sync::{Mutex, OnceLock};

use tsfp_ai::memory::GuestMemory as AiMemory;
use tsfp_engine_anim::{flag_leaves, memory::GuestMemory as AnimMemory};
use tsfp_engine_audio::memory::GuestMemory as AudioMemory;
use tsfp_engine_input::memory::GuestMemory as InputMemory;
use tsfp_engine_object::{ObjectMemory, closure::memory::GuestMemory as ObjectGuestMemory};
use tsfp_engine_physics::memory::GuestMemory as PhysicsMemory;
use tsfp_engine_render::memory::GuestMemory as RenderMemory;
use tsfp_gameplay::{memory::GuestMemory as GameplayMemory, state_leaves};
use tsfp_network::memory::GuestMemory as NetworkMemory;
use tsfp_ui::{memory::GuestMemory as UiMemory, menu_leaves};

unsafe extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
    fn shim_set_base(base: *mut u8);
    fn shim_call(va: u32, regs: *mut u32) -> i32;
}

const ARENA: usize = (1 << 32) + 0x10000;
const STACK: u32 = 0x0030_0000;
const CASES: usize = 120_000;

/// Two arenas: [`Pair::c`] is what the C replacement sees, [`Pair::rust`] what the port sees.
struct Pair {
    c: *mut u8,
    rust: *mut u8,
    /// Every address `put32` filled since the last drain. `drive` adds them to the watch list,
    /// so a case compares everything its builder set up (T1836 wave 7).
    log: Vec<u32>,
}
unsafe impl Send for Pair {}

fn map() -> *mut u8 {
    // PROT_READ|PROT_WRITE, MAP_PRIVATE|MAP_ANONYMOUS|MAP_NORESERVE
    let base = unsafe { mmap(std::ptr::null_mut(), ARENA, 3, 0x2 | 0x20 | 0x4000, -1, 0) };
    assert!(base as isize != -1, "arena mmap failed");
    base
}

fn pair() -> &'static Mutex<Pair> {
    static PAIR: OnceLock<Mutex<Pair>> = OnceLock::new();
    PAIR.get_or_init(|| {
        let pair = Pair {
            c: map(),
            rust: map(),
            log: Vec::new(),
        };
        unsafe { shim_set_base(pair.c) };
        Mutex::new(pair)
    })
}

impl Pair {
    fn put8(&mut self, address: u32, value: u8) {
        unsafe {
            *self.c.add(address as usize) = value;
            *self.rust.add(address as usize) = value;
        }
    }
    fn put32(&mut self, address: u32, value: u32) {
        self.log.push(address);
        for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.put8(address.wrapping_add(i as u32), byte);
        }
    }
    fn get(&self, rust: bool, address: u32) -> u8 {
        let base = if rust { self.rust } else { self.c };
        unsafe { *base.add(address as usize) }
    }
}

/// Rust-side view of the Rust arena. Each port crate carries its own copy of the leaf trait
/// (path included), so one wrapper implements all three. Addresses wrap at 32 bits.
struct View(*mut u8);
impl View {
    fn byte(&self, address: u32) -> u8 {
        unsafe { *self.0.add(address as usize) }
    }
    fn set(&mut self, address: u32, value: u8) {
        unsafe { *self.0.add(address as usize) = value }
    }
}
macro_rules! guest_memory {
    ($($trait:ident),*) => {$(
        impl $trait for View {
            fn read8(&self, address: u32) -> u8 { self.byte(address) }
            fn write8(&mut self, address: u32, value: u8) { self.set(address, value) }
        }
    )*};
}
guest_memory!(
    AiMemory,
    ObjectGuestMemory,
    AnimMemory,
    GameplayMemory,
    UiMemory,
    ObjectMemory,
    AudioMemory,
    InputMemory,
    RenderMemory,
    NetworkMemory,
    PhysicsMemory
);

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    /// Mostly small or special words so branches fire, sometimes a full random word.
    fn word(&mut self, special: &[u32]) -> u32 {
        match self.below(8) {
            0..=2 => self.below(16),
            3..=5 => special[self.below(special.len() as u32) as usize],
            _ => self.next(),
        }
    }
    /// A pointer into one of eight 0x1000 pools starting at `base`, 4-byte aligned.
    fn pool(&mut self, base: u32) -> u32 {
        base + self.below(8) * 0x1000 + self.below(0x100) * 4
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
struct Outcome {
    eax: u32,
    /// `None` means the C body must leave the register untouched.
    ecx: Option<u32>,
    edx: Option<u32>,
}

struct Case {
    va: u32,
    args: Vec<u32>,
    regs: [u32; 3],
    esi: u32,
    watch: Vec<(u32, u32)>,
}

fn perturb(kind: u32, outcome: &mut Outcome, view: &mut View, watch: &[(u32, u32)]) {
    match kind {
        0 => outcome.eax ^= 1,
        1 => outcome.ecx = Some(outcome.ecx.unwrap_or(0) ^ 1),
        2 => outcome.edx = Some(outcome.edx.unwrap_or(0) ^ 1),
        _ => {
            let (start, _) = watch[0];
            let byte = view.byte(start);
            view.set(start, byte ^ 0x80);
        }
    }
}

/// Runs `CASES` cases. `build` fills memory and returns the case, `port` runs the Rust port on
/// the Rust arena. With `mutant` set, every case perturbs the Rust result by one of four
/// kinds and the return value is the number of cases whose perturbation was detected.
fn drive(
    seed: u64,
    mutant: bool,
    mut build: impl FnMut(&mut Rng, &mut Pair) -> Case,
    port: impl Fn(&mut View, &Case) -> Outcome,
) -> (usize, usize) {
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let pair = &mut *guard;
    let mut rng = Rng(seed);
    let (mut detected, mut cases_with_watch) = (0, 0);
    for index in 0..CASES {
        let mut case = build(&mut rng, pair);
        let logged: Vec<u32> = pair.log.drain(..).collect();
        case.watch
            .extend(logged.into_iter().map(|address| (address, 4)));
        pair.put32(STACK, 0);
        for (i, arg) in case.args.iter().enumerate() {
            pair.put32(STACK + 4 + 4 * i as u32, *arg);
        }
        pair.log.clear();
        let mut regs = [case.regs[0], case.regs[1], case.regs[2], case.esi, STACK, 0];
        assert_eq!(
            unsafe { shim_call(case.va, regs.as_mut_ptr()) },
            1,
            "unknown root"
        );
        let mut view = View(pair.rust);
        let mut expected = port(&mut view, &case);
        if mutant {
            perturb(index as u32 % 4, &mut expected, &mut view, &case.watch);
        }
        let mut mismatch = lifted_policy::mismatch(
            case.va,
            u8::from(regs[0] != expected.eax)
                | u8::from(regs[1] != expected.ecx.unwrap_or(case.regs[1])) << 1
                | u8::from(regs[2] != expected.edx.unwrap_or(case.regs[2])) << 2
                | u8::from(regs[3] != case.esi) << 4
                | u8::from(regs[4] != STACK + 4) << 7,
        );
        for (start, len) in &case.watch {
            for offset in 0..*len {
                let address = start.wrapping_add(offset);
                mismatch |= lifted_policy::mem(
                    case.va,
                    address,
                    pair.get(false, address) != pair.get(true, address),
                );
            }
        }
        if mutant {
            cases_with_watch += 1;
            detected += usize::from(mismatch);
            // Restore the Rust arena so the next case starts equal (the control wrote into it).
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
                "root {:08X} case {index}: C regs {regs:x?}, Rust {expected:x?}, args {:x?}",
                case.va, case.args
            );
        }
    }
    (detected, cases_with_watch)
}

fn special_states() -> Vec<u32> {
    vec![
        1,
        3,
        7,
        8,
        9,
        11,
        12,
        13,
        0x400,
        0x100,
        u32::MAX,
        0x8000_0000,
    ]
}

fn fill_state(rng: &mut Rng, pair: &mut Pair) {
    let specials = special_states();
    pair.put32(0x006d_60d8, rng.word(&specials));
    let target = rng.word(&specials);
    pair.put32(0x006d_60a8, target);
    pair.put32(
        0x007e_3f6c,
        if rng.below(3) == 0 {
            target
        } else {
            rng.word(&specials)
        },
    );
}

fn case_state(rng: &mut Rng, pair: &mut Pair, va: u32) -> Case {
    fill_state(rng, pair);
    Case {
        va,
        args: vec![],
        regs: [rng.next(), rng.next(), rng.next()],
        esi: rng.next(),
        watch: vec![(0x006d_60a8, 4), (0x006d_60d8, 4), (0x007e_3f6c, 4)],
    }
}

fn eax_only(eax: u32) -> Outcome {
    Outcome {
        eax,
        ecx: None,
        edx: None,
    }
}

macro_rules! suite {
    ($name:ident, $mutant:ident, $seed:expr, $build:expr, $port:expr) => {
        #[test]
        fn $name() {
            drive($seed, false, $build, $port);
        }
        #[test]
        fn $mutant() {
            let (detected, total) = drive($seed, true, $build, $port);
            eprintln!(
                "mutant control {}: {detected}/{total} perturbations detected",
                stringify!($name)
            );
            // Every perturbation of a register is detected. A memory perturbation is only
            // detectable when the case watches something, which every case here does.
            assert_eq!(detected, total);
        }
    };
}

suite!(
    r_00056e90,
    mutant_00056e90,
    0x1836_0001,
    |r, p| case_state(r, p, 0x56e90),
    |v, _| eax_only(state_leaves::state_is_one_of_1_3_7_11_12_13(&*v))
);

suite!(
    r_00056f70,
    mutant_00056f70,
    0x1836_0002,
    |r, p| case_state(r, p, 0x56f70),
    |v, _| eax_only(state_leaves::state_is_playable_or_target_matches(&*v))
);

fn build_player_sync(rng: &mut Rng, pair: &mut Pair) -> Case {
    let player = match rng.below(6) {
        0 => 0x007b_0c7c - 0xa8, // the owner store aliases the player pointer itself
        1 => rng.next(),
        _ => rng.pool(0x0040_0000),
    };
    pair.put32(0x007b_0c7c, player);
    let specials = [0x400u32, 0x100];
    pair.put32(player.wrapping_add(0x114), rng.word(&specials));
    let owner = match rng.below(6) {
        0 => 0x007b_0c7c - 0x64,
        1 => 0x007b_0c7c - 0x68,
        2 => 0x007b_0c7c - 0x6c,
        _ => rng.pool(0x0050_0000),
    };
    pair.put32(player.wrapping_add(0x14), owner);
    for offset in [0xa8, 0xac, 0xb0] {
        pair.put32(player.wrapping_add(offset), rng.next());
    }
    Case {
        va: 0xd0ca0,
        args: vec![],
        regs: [rng.next(), rng.next(), rng.next()],
        esi: rng.next(),
        watch: vec![
            (0x007b_0c7c - 4, 12),
            (owner.wrapping_add(0x60), 0x14),
            (player.wrapping_add(0xa0), 0x18),
            (player.wrapping_add(0x10), 8),
        ],
    }
}
suite!(
    r_000d0ca0,
    mutant_000d0ca0,
    0x1836_0003,
    build_player_sync,
    |v, c| {
        let g = state_leaves::player_sync_owner_position(v, c.regs[2]);
        Outcome {
            eax: g.eax,
            ecx: Some(g.ecx),
            edx: Some(g.edx),
        }
    }
);

fn build_slot_summary(rng: &mut Rng, pair: &mut Pair) -> Case {
    let index = match rng.below(8) {
        0 => u32::MAX,
        1 => rng.next(),
        _ => rng.below(5),
    };
    let array = if rng.below(8) == 0 {
        rng.next()
    } else {
        rng.pool(0x0080_0000)
    };
    pair.put32(0x0073_56d8, array);
    let slot = index.wrapping_mul(0xea0).wrapping_add(array);
    let player = rng.pool(0x00a0_0000);
    pair.put32(slot, player);
    pair.put32(player.wrapping_add(0x34), rng.next());
    for offset in [0xbf8, 0xbfc, 0xc00, 0xc04, 0xc1c, 0xc20] {
        pair.put32(slot.wrapping_add(offset), rng.next());
    }
    let out = match rng.below(6) {
        0 => slot.wrapping_add(0xbf0), // the output overlaps the fields still to be read
        1 => STACK - 8,                // the output overlaps the argument slots
        _ => rng.pool(0x00b0_0000),
    };
    Case {
        va: 0xc4560,
        args: vec![index, out],
        regs: [rng.next(), rng.next(), rng.next()],
        esi: rng.next(),
        watch: vec![
            (out, 0x20),
            (slot.wrapping_add(0xbf0), 0x40),
            (slot, 4),
            (player.wrapping_add(0x30), 8),
            (0x0073_56d8, 4),
        ],
    }
}
suite!(
    r_000c4560,
    mutant_000c4560,
    0x1836_0004,
    build_slot_summary,
    |v, c| {
        let g = state_leaves::player_slot_summary_fill(v, c.args[0], c.args[1]);
        Outcome {
            eax: g.eax,
            ecx: Some(g.ecx),
            edx: Some(g.edx),
        }
    }
);

fn build_hud(rng: &mut Rng, pair: &mut Pair) -> Case {
    let flags = if rng.below(2) == 0 {
        rng.next() & !0xe000_0200
    } else {
        rng.word(&[0x200, 0x2000_0000, 0x8000_0000, 0x4000_0000])
    };
    pair.put32(0x007d_e458, flags);
    pair.put32(
        0x0079_094c,
        rng.word(&[0x64, 0x65, 0x63, 0x7fff_ffff, 0x8000_0000]),
    );
    let player = if rng.below(4) == 0 {
        0
    } else {
        rng.pool(0x0040_0000)
    };
    pair.put32(0x007b_0c48, player);
    pair.put32(player.wrapping_add(0x114), rng.word(&[0x100, 0x101]));
    Case {
        va: 0x42f90,
        args: vec![],
        regs: [rng.next(), rng.next(), rng.next()],
        esi: rng.next(),
        watch: vec![
            (0x007d_e458, 4),
            (0x0079_094c, 4),
            (0x007b_0c48, 4),
            (player.wrapping_add(0x114), 4),
        ],
    }
}
suite!(
    r_00042f90,
    mutant_00042f90,
    0x1836_0005,
    build_hud,
    |v, _| eax_only(menu_leaves::hud_prompt_allowed(&*v))
);

fn build_nibble(rng: &mut Rng, pair: &mut Pair) -> Case {
    pair.put32(0x0075_6e58, rng.next());
    let object = if rng.below(8) == 0 {
        0x0075_6e58 - 0x184
    } else {
        rng.pool(0x0040_0000)
    };
    pair.put32(
        object.wrapping_add(0x184),
        if rng.below(2) == 0 {
            rng.below(16)
        } else {
            rng.next()
        },
    );
    Case {
        va: 0x1957d0,
        args: vec![object],
        regs: [rng.next(), rng.next(), rng.next()],
        esi: rng.next(),
        watch: vec![(0x0075_6e58, 4), (object.wrapping_add(0x184), 4)],
    }
}
suite!(
    r_001957d0,
    mutant_001957d0,
    0x1836_0006,
    build_nibble,
    |v, c| {
        let g = menu_leaves::menu_store_field_into_low_nibble(v, c.args[0]);
        Outcome {
            eax: g.eax,
            ecx: Some(g.ecx),
            edx: Some(g.edx),
        }
    }
);

fn build_flag_mask(rng: &mut Rng, pair: &mut Pair) -> Case {
    pair.put8(
        0x007d_e455,
        if rng.below(2) == 0 {
            0x0a
        } else {
            rng.next() as u8
        },
    );
    let entry = if rng.below(6) == 0 {
        rng.next()
    } else {
        rng.below(40)
    };
    let address = (entry << 5).wrapping_add(0x004d_1f88);
    pair.put32(address, if rng.below(3) == 0 { 0 } else { rng.next() });
    Case {
        va: 0x2d2be0,
        args: vec![rng.next(), entry],
        regs: [rng.next(), rng.next(), rng.next()],
        esi: rng.next(),
        watch: vec![(0x007d_e455, 1), (address, 4)],
    }
}
suite!(
    r_002d2be0,
    mutant_002d2be0,
    0x1836_0007,
    build_flag_mask,
    |v, c| {
        let g = menu_leaves::menu_flag_mask(&*v, c.args[1]);
        Outcome {
            eax: g.eax,
            ecx: Some(g.ecx),
            edx: Some(g.edx),
        }
    }
);

fn build_anim(rng: &mut Rng, pair: &mut Pair, va: u32) -> Case {
    let offset = if va == 0x59f10 { 0xd8 } else { 0x130 };
    let object = rng.pool(0x0040_0000);
    let record = rng.pool(0x0050_0000);
    pair.put32(object.wrapping_add(4), record);
    let slot = if rng.below(8) == 0 {
        rng.next()
    } else {
        rng.below(6)
    };
    pair.put32(record.wrapping_add(offset), slot);
    let table = if rng.below(8) == 0 {
        rng.next()
    } else {
        rng.pool(0x00c0_0000)
    };
    pair.put32(0x007e_3954, table);
    pair.put32(
        table.wrapping_add(slot.wrapping_mul(4)),
        if rng.below(3) == 0 { 0 } else { rng.next() },
    );
    let mask_address = slot.wrapping_mul(8).wrapping_add(0x007d_e660);
    for half in 0..2 {
        pair.put32(
            mask_address.wrapping_add(4 * half),
            if rng.below(3) == 0 { 0 } else { rng.next() },
        );
    }
    let (lo, hi) = (rng.next() & rng.next(), rng.next() & rng.next());
    Case {
        va,
        args: vec![object, lo, hi],
        regs: [rng.next(), rng.next(), rng.next()],
        esi: rng.next(),
        watch: vec![
            (object.wrapping_add(4), 4),
            (record.wrapping_add(offset), 4),
            (0x007e_3954, 4),
            (table.wrapping_add(slot.wrapping_mul(4)), 4),
            (mask_address, 8),
        ],
    }
}
suite!(
    r_00059f10,
    mutant_00059f10,
    0x1836_0008,
    |r, p| build_anim(r, p, 0x59f10),
    |v, c| {
        let g = flag_leaves::obj_flag_mask_test(&*v, c.args[0], c.args[1], c.args[2], 0xd8);
        Outcome {
            eax: g.eax,
            ecx: Some(g.ecx),
            edx: Some(g.edx),
        }
    }
);
suite!(
    r_00059f50,
    mutant_00059f50,
    0x1836_0009,
    |r, p| build_anim(r, p, 0x59f50),
    |v, c| {
        let g = flag_leaves::obj_flag_mask_test(&*v, c.args[0], c.args[1], c.args[2], 0x130);
        Outcome {
            eax: g.eax,
            ecx: Some(g.ecx),
            edx: Some(g.edx),
        }
    }
);

#[path = "differential/wave4.rs"]
mod wave4;
#[path = "differential/wave4b.rs"]
mod wave4b;
#[path = "differential/wave7.rs"]
mod wave7;

#[path = "differential/wave8.rs"]
mod wave8;
#[macro_use]
#[path = "differential/wave9.rs"]
mod wave9;
#[path = "differential/lifted_policy.rs"]
mod lifted_policy;
#[path = "differential/w4.rs"]
mod w4;
#[path = "differential/wave10.rs"]
mod wave10;
#[path = "differential/wave11.rs"]
mod wave11;
#[path = "differential/wave12.rs"]
mod wave12;
#[path = "differential/x87_roots.rs"]
mod x87_roots;
// Keep a discoverable test in ordinary worker builds, but put the real adapter implementation
// inside `w4` so it shares the lifted harness helpers and is compiled only with the private tree.
mod capdiff {
    #[test]
    fn capdiff_corpus() {
        #[cfg(w4_lifted)]
        super::w4::capdiff::run_corpus();
        #[cfg(not(w4_lifted))]
        eprintln!("CAPDIFF-SKIP private lifted tree unavailable");
    }
}
#[path = "differential/mapmaker_t2371.rs"]
mod mapmaker_t2371;
#[path = "differential/mapmaker_t2383.rs"]
mod mapmaker_t2383;
#[path = "differential/modes_t1.rs"]
mod modes_t1;
#[path = "differential/save_t1.rs"]
mod save_t1;
#[path = "differential/t1900.rs"]
mod t1900;
