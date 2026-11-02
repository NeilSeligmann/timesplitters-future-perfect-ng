//! T2432 Tier 1 (weak): the five Rust-ported callees of the Map Maker update `0x306460`
//! (`mapmaker_update::callees`) versus their lifted translations (`editor_reference`,
//! `editor_next_reference`). Memory, object list, context and widget words are FABRICATED and seeded;
//! the callees' own callees are a seeded mock that logs every call. Compared: callee call log,
//! guest memory outside the stack, returned EAX where the callee has a result. Not real editor state.
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::editor_next_reference::{f2fbf70, f2feb50};
use tsfp_ports::editor_reference::{f2f92e0, f2fc050, f62490};
use tsfp_ports::mapmaker_update::callees::{
    CalleeHost, clear_link_marks, count_selected_objects, idle_allows_cursor_action,
    set_cursor_mode_global, update_hint_for_active_view,
};
use tsfp_ports::memory::GuestMemory;

const STACK: u32 = 0x80_000;
const CTX: u32 = 0xa0_000;
const PAGE: u32 = 0xc2_000;
const LEAF_A: u32 = 0xc0_000;
const LEAF_B: u32 = 0xc1_000;

#[derive(Clone, Default)]
struct Mem(Rc<RefCell<BTreeMap<u32, u8>>>);

impl Mem {
    fn deep_clone(&self) -> Self {
        Mem(Rc::new(RefCell::new(self.0.borrow().clone())))
    }
    fn snapshot(&self) -> BTreeMap<u32, u8> {
        self.0
            .borrow()
            .iter()
            .filter(|(a, _)| **a < STACK - 0x1000 || **a >= STACK + 0x1000)
            .map(|(a, v)| (*a, *v))
            .collect()
    }
}

impl GuestMemory for Mem {
    fn read8(&self, address: u32) -> u8 {
        self.0.borrow().get(&address).copied().unwrap_or(0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn chance(&mut self, one_in: u32) -> bool {
        self.below(one_in) == 0
    }
}

/// Mock host for the callees' own callees; logs `(va, args)`.
#[derive(Clone)]
struct Host {
    rng: Rc<RefCell<Rng>>,
    mem: Mem,
    calls: Rc<RefCell<Vec<(u32, Vec<u32>)>>>,
}

impl Host {
    fn answer(&self, address: u32, args: &[u32]) -> u32 {
        self.calls.borrow_mut().push((address, args.to_vec()));
        let mut rng = self.rng.borrow_mut();
        match address {
            0x77470 => [0, LEAF_A, LEAF_B, PAGE, PAGE, PAGE][rng.below(6) as usize],
            0x774b0 => [0, LEAF_A, LEAF_B][rng.below(3) as usize],
            0x314d60 => u32::from(rng.chance(4)),
            _ => rng.next(),
        }
    }
}

impl GuestCall for Host {
    fn call(&mut self, address: u32, registers: &mut ExtRegisters) {
        let count = match address {
            0x77470 => 1,
            _ => 0,
        };
        let args: Vec<u32> = (0..count)
            .map(|i| self.mem.read32(registers.esp.wrapping_add(4 * i)))
            .collect();
        registers.eax = self.answer(address, &args);
    }
}

/// Rust side: the host also owns the memory the callee reads.
struct RustHost(Host);

impl CalleeHost for RustHost {
    type Memory = Mem;
    fn memory(&mut self) -> &mut Mem {
        &mut self.0.mem
    }
    fn guest_call(&mut self, va: u32, args: &[u32]) -> u32 {
        self.0.answer(va, args)
    }
}

fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0x0c0c,
        edx: 0x0d0d,
        esp: STACK,
        ebx: 0x1111,
        ebp: 0x2222,
        esi: 0x3333,
        edi: 0x4444,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0x027f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

fn fabricate(rng: &mut Rng) -> Mem {
    let mut m = Mem::default();
    m.write32(0x7844a8, CTX);
    m.write32(0x7842a8, PAGE);
    m.write32(0x7ba9e0, rng.below(3));
    for view in 0..3u32 {
        m.write32(0x7ba26c + view * 0x1e0, 1 + rng.below(3));
    }
    let mut next = 0u32;
    for index in 0..rng.below(6) {
        let node = 0xe0_000 + index * 0x100;
        m.write32(node + 0x40, next);
        m.write8(node + 0x14, rng.next() as u8);
        next = node;
    }
    m.write32(0x78400c, next);
    for leaf in [LEAF_A, LEAF_B, PAGE] {
        m.write8(leaf + 0x218, rng.below(8) as u8);
    }
    // Mostly calm frame flags so the later gates of the idle check are reached.
    let flags = if rng.chance(3) {
        rng.next()
    } else {
        rng.next() & 0xa0 & rng.next()
    };
    m.write32(CTX + 0x25c, flags);
    m.write32(CTX + 0x2f9c, rng.next());
    m.write32(
        CTX + 0x2f34,
        if rng.chance(8) {
            rng.below(4)
        } else {
            u32::MAX
        },
    );
    let cell: u16 = if rng.chance(8) {
        rng.below(4) as u16
    } else {
        0xffff
    };
    m.write8(CTX + 0x2f10, cell as u8);
    m.write8(CTX + 0x2f11, (cell >> 8) as u8);
    m.write32(CTX + 0x2f7c, if rng.chance(8) { 0xb0_000 } else { 0 });
    m.write32(
        CTX + 0x1185c,
        if rng.chance(8) {
            rng.below(4)
        } else {
            u32::MAX
        },
    );
    m.write8(CTX + 0x11854, [0, 1, 1, 2, 0xff][rng.below(5) as usize]);
    m.write32(0x4d19f0, rng.next());
    m
}

type Lifted = fn(&mut Mem, &mut Host, &mut ExtRegisters);

struct Outcome {
    log: Vec<(u32, Vec<u32>)>,
    mem: BTreeMap<u32, u8>,
    eax: Option<u32>,
}

fn host_for(mem: &Mem, seed: u64) -> Host {
    Host {
        rng: Rc::new(RefCell::new(Rng(seed))),
        mem: mem.clone(),
        calls: Rc::new(RefCell::new(Vec::new())),
    }
}

fn run_lifted(start: &Mem, seed: u64, body: Lifted, arg: Option<u32>, has_result: bool) -> Outcome {
    let mem = start.deep_clone();
    let mut host = host_for(&mem, seed);
    let mut r = regs();
    r.esp = STACK - 4;
    mem.clone().write32(r.esp, 0);
    if let Some(value) = arg {
        mem.clone().write32(r.esp + 4, value);
    }
    let mut working = mem.clone();
    body(&mut working, &mut host, &mut r);
    let log = host.calls.borrow().clone();
    Outcome {
        log,
        mem: mem.snapshot(),
        eax: has_result.then_some(r.eax),
    }
}

fn run_rust(start: &Mem, seed: u64, which: u32) -> Outcome {
    let mem = start.deep_clone();
    let mut host = RustHost(host_for(&mem, seed));
    let mut working = mem.clone();
    let mut eax = None;
    match which {
        0x62490 => eax = Some(set_cursor_mode_global(&mut working, u32::MAX - 6)),
        0x2f92e0 => clear_link_marks(&mut working),
        0x2feb50 => count_selected_objects(&mut working),
        0x2fc050 => update_hint_for_active_view(&mut host),
        0x2fbf70 => eax = Some(u32::from(idle_allows_cursor_action(&mut host))),
        _ => unreachable!(),
    }
    let log = host.0.calls.borrow().clone();
    Outcome {
        log,
        mem: mem.snapshot(),
        eax,
    }
}

#[test]
fn ported_callees_match_lifted() {
    let cases: [(u32, Lifted, Option<u32>, bool); 5] = [
        (0x62490, f62490, Some(u32::MAX - 6), true),
        (0x2f92e0, f2f92e0, None, false),
        (0x2feb50, f2feb50, None, false),
        (0x2fc050, f2fc050, None, false),
        (0x2fbf70, f2fbf70, None, true),
    ];
    let mut rng = Rng(0x2432_0001);
    let mut idle_true = 0;
    let mut idle_false = 0;
    let mut nav_called = 0;
    let mut counted_nonzero = 0;
    for seed in 0..4_000u64 {
        let start = fabricate(&mut rng);
        for (which, body, arg, has_result) in cases {
            let lifted = run_lifted(&start, seed, body, arg, has_result);
            let rust = run_rust(&start, seed, which);
            assert_eq!(lifted.log, rust.log, "{which:#x} seed {seed}: call log");
            assert_eq!(lifted.mem, rust.mem, "{which:#x} seed {seed}: memory");
            assert_eq!(lifted.eax, rust.eax, "{which:#x} seed {seed}: result");
            match which {
                0x2fbf70 if rust.eax == Some(1) => idle_true += 1,
                0x2fbf70 => idle_false += 1,
                0x2fc050 if rust.log.iter().any(|c| c.0 == 0x3159c0) => nav_called += 1,
                0x2feb50 if rust.mem.get(&(CTX + 0x2f9c)).copied().unwrap_or(0) != 0 => {
                    counted_nonzero += 1
                }
                _ => {}
            }
        }
    }
    assert!(
        idle_true > 100 && idle_false > 100,
        "idle {idle_true}/{idle_false}"
    );
    assert!(nav_called > 50, "nav helper reached {nav_called}");
    assert!(counted_nonzero > 100, "selected count {counted_nonzero}");
    println!("T2432 callee comparisons: idle true {idle_true} false {idle_false} nav {nav_called}");
}
