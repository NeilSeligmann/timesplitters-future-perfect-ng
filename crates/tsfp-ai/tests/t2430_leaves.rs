// SPDX-License-Identifier: GPL-3.0-or-later
//! T2430 Tier 1 differentials (WEAK: fabricated seeded inputs) for the leaves added to
//! `rea_character_leaves` (bits 27 to 31), run through the real `rea_ai_runtime::run` adapter. No
//! lifted Rust body exists for them, so the reference is an independent register-level
//! transliteration of the REA disassembly written in this file (a second reading, not an oracle).

use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
    rea_ai_runtime,
};

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

impl Memory {
    fn deep(&self) -> Memory {
        Memory(Rc::new(RefCell::new(self.0.borrow().clone())))
    }
    fn data(&self) -> BTreeMap<u32, u8> {
        self.0
            .borrow()
            .iter()
            .filter(|(address, value)| !(0x6f000..0x71000).contains(*address) && **value != 0)
            .map(|(a, v)| (*a, *v))
            .collect()
    }
}

fn registers(esp: u32) -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp,
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
}

struct NoCalls;
impl GuestCall for NoCalls {
    fn call(&mut self, va: u32, _regs: &mut ExtRegisters) {
        panic!("leaf made a guest call to {va:#x}");
    }
}

fn enable(va: u32) {
    rea_ai_runtime::configure(true);
    assert!(rea_ai_runtime::configure_function(va, true));
}

/// Run the adapter and return EAX plus the register file and all non-zero memory.
fn adapter(va: u32, memory: &Memory, args: &[u32]) -> (u32, ExtRegisters, BTreeMap<u32, u8>) {
    let mut memory = memory.deep();
    let mut regs = registers(STACK - 16);
    for (i, value) in args.iter().enumerate() {
        memory.write32(regs.esp + 4 + 4 * i as u32, *value);
    }
    assert!(rea_ai_runtime::run(
        va,
        &mut memory,
        &mut NoCalls,
        &mut regs
    ));
    (regs.eax, regs, memory.data())
}

fn expected_data(memory: &Memory, args: &[u32]) -> BTreeMap<u32, u8> {
    let mut memory = memory.deep();
    for (i, value) in args.iter().enumerate() {
        memory.write32(STACK - 16 + 4 + 4 * i as u32, *value);
    }
    memory.data()
}

fn check(
    va: u32,
    seeds: u32,
    build: impl Fn(&mut Rng, &mut Memory) -> Vec<u32>,
    reference: impl Fn(&Memory, &[u32]) -> u32,
) {
    enable(va);
    let mut rng = Rng(0x2430_0000_0000_0002 ^ u64::from(va));
    let mut nonzero = 0;
    for seed in 0..seeds {
        let mut memory = Memory::default();
        let args = build(&mut rng, &mut memory);
        let (eax, regs, data) = adapter(va, &memory, &args);
        let want = reference(&memory, &args);
        assert_eq!(eax, want, "{va:#x} seed {seed} args {args:x?}");
        assert_eq!(
            data,
            expected_data(&memory, &args),
            "{va:#x} seed {seed} wrote memory"
        );
        assert_eq!(
            (regs.ebx, regs.esi, regs.edi, regs.ebp, regs.esp),
            (0, 0, 0, 0, STACK - 16),
            "{va:#x} preserved registers"
        );
        nonzero += u32::from(want != 0);
    }
    assert!(
        nonzero > seeds / 20,
        "{va:#x}: reference result was almost always 0 ({nonzero})"
    );
}

fn pick(rng: &mut Rng, specials: &[u32]) -> u32 {
    match rng.below(4) {
        0 => specials[rng.below(specials.len() as u32) as usize],
        1 => rng.below(64),
        _ => rng.next(),
    }
}

#[test]
fn weapon_transition_adapter_reads_cdecl_arguments_and_typed_state() {
    let object = 0x520000;
    let mut memory = Memory::default();
    memory.write32(object + 0xe8, 0x21);
    memory.write32(object + 0xf8, 0x14);
    enable(0x1b63d0);

    let (pending, _, unchanged) = adapter(0x1b63d0, &memory, &[object, 0x21]);
    assert_eq!(pending, 1);
    assert_eq!(unchanged, memory.data());

    let (settled, _, _) = adapter(0x1b63d0, &memory, &[object, 0x14]);
    assert_eq!(settled, 0);
    let (wrong_current, _, _) = adapter(0x1b63d0, &memory, &[object, 0x33]);
    assert_eq!(wrong_current, 0);
}

#[test]
fn weapon_context_adapter_reads_cdecl_record_and_player_context() {
    let weapon = 0x530000;
    let player = 0x540000;
    let records = 0x560000;
    let record = 2;
    let mut memory = Memory::default();
    memory.write32(weapon + 0x30, record);
    memory.write32(0x7b0c7c, player);
    memory.write32(0x4f9bac, records);
    memory.write32(player + 4, 0);
    memory.write32(player + 0xe84, 0x550000);
    memory.write32(records + record * 0x29c, 4);
    enable(0x1b7cc0);

    let (valid, _, unchanged) = adapter(0x1b7cc0, &memory, &[weapon]);
    assert_eq!(valid, 1);
    assert_eq!(unchanged, memory.data());

    memory.write32(weapon + 0x30, u32::MAX);
    assert_eq!(adapter(0x1b7cc0, &memory, &[weapon]).0, 0);
    memory.write32(weapon + 0x30, record);
    memory.write32(player + 4, u32::MAX);
    assert_eq!(adapter(0x1b7cc0, &memory, &[weapon]).0, 0);
    memory.write32(player + 4, 0);
    memory.write32(player + 0xe84, 0);
    assert_eq!(adapter(0x1b7cc0, &memory, &[weapon]).0, 0);
    memory.write32(player + 0xe84, 0x550000);
    memory.write32(records + record * 0x29c, u32::MAX);
    assert_eq!(adapter(0x1b7cc0, &memory, &[weapon]).0, 0);
    rea_ai_runtime::configure(false);
}

#[test]
fn tier1_pair_field_8_12fe0() {
    check(
        0x12fe0,
        3000,
        |rng, m| {
            m.write32(0x74c368, 0x200000);
            let (a, b) = (rng.below(40), rng.below(40));
            let (hi, lo) = (a.max(b) as i32, a.min(b) as i32);
            let entry =
                0x200000u32 + ((((hi - 1).wrapping_mul(hi)) >> 1) + lo).wrapping_mul(12) as u32;
            m.write32(entry + 8, rng.next() | 1);
            vec![a, b]
        },
        |m, args| {
            let (a, b) = (args[0] as i32, args[1] as i32);
            let (hi, lo) = if a > b { (a, b) } else { (b, a) };
            let index = ((hi - 1).wrapping_mul(hi)) >> 1;
            m.read32(m.read32(0x74c368) + ((index + lo) as u32).wrapping_mul(12) + 8)
        },
    );
}

#[test]
fn tier1_ordinary_type_id_d6ed0() {
    let special = |id: u32| {
        matches!(
            id,
            0x12 | 0x13 | 0x2d | 0x34 | 0x36 | 0x42 | 0x43 | 0x45 | 0x4a | 0x50 | 0x5c
        ) || matches!(
            id,
            0x5e | 0x60 | 0x62 | 0x69 | 0x75 | 0x97 | 0x9c | 0xa2 | 0xa8..=0xaa
        )
    };
    check(
        0xd6ed0,
        3000,
        |rng, _| vec![pick(rng, &[0x4d, 0x4f, 0x64, 0x12, 0xaa, 0xab, 0x11, 0x50])],
        |_, args| u32::from(!special(args[0]) && !matches!(args[0], 0x4d | 0x4f | 0x64)),
    );
    // exhaustive over the table range and beyond: random seeds rarely land on a single id
    for id in 0..0x300u32 {
        let (eax, _, _) = adapter(0xd6ed0, &Memory::default(), &[id]);
        let want = u32::from(!special(id) && !matches!(id, 0x4d | 0x4f | 0x64));
        assert_eq!(eax, want, "type id {id:#x}");
    }
}

const OBJECT: u32 = 0x10000;
const OWNER: u32 = 0x11000;
const EXT: u32 = 0x12000;
const RECORDS: u32 = 0x14000;

/// Object with owner record, extension record index and 0x29C-byte record table, plus the three
/// column tables of `bases` filled with small values so slot matches occur.
fn weapon_world(rng: &mut Rng, m: &mut Memory, bases: [u32; 3]) -> Vec<u32> {
    m.write32(OBJECT + 4, OWNER);
    m.write32(OBJECT + 0x7c, EXT);
    let index = match rng.below(4) {
        0 => u32::MAX,
        _ => rng.below(4),
    };
    m.write32(EXT + 0x204, index);
    m.write32(0x4f9bac, RECORDS);
    for record in 0..4u32 {
        m.write8(RECORDS + record * 0x29c + 0xc, rng.below(256) as u8);
    }
    m.write32(OWNER + 0xd4, rng.below(6));
    m.write32(OWNER + 0x12c, rng.below(6));
    for base in bases {
        m.write32(base, rng.below(6));
        m.write32(base + 4, rng.below(6));
    }
    vec![OBJECT]
}

/// Reference column choice, written the way the disassembly branches.
fn ref_column(m: &Memory) -> u32 {
    let index = m.read32(EXT + 0x204);
    if index == u32::MAX {
        return 1;
    }
    u32::from(m.read8(RECORDS + index * 0x29c + 0xc) & 1 != 0)
}

#[test]
fn tier1_weapon_slot_matches_26cc80() {
    check(
        0x26cc80,
        3000,
        |rng, m| weapon_world(rng, m, [0x5202f8, 0x520308, 0x5202e8]),
        |m, _| {
            let column = ref_column(m);
            let slot = m.read32(OWNER + 0xd4);
            for base in [0x5202f8u32, 0x520308, 0x5202e8] {
                if m.read32(base + 4 * column) == slot {
                    return 1;
                }
            }
            0
        },
    );
}

#[test]
fn tier1_weapon_owner_has_26cd50() {
    check(
        0x26cd50,
        3000,
        |rng, m| weapon_world(rng, m, [0x520338, 0x520328, 0x520318]),
        |m, _| {
            let column = ref_column(m);
            let (a, b) = (m.read32(OWNER + 0xd4), m.read32(OWNER + 0x12c));
            for base in [0x520338u32, 0x520328, 0x520318] {
                let wanted = m.read32(base + 4 * column);
                if a == wanted || b == wanted {
                    return 1;
                }
            }
            0
        },
    );
}

#[test]
fn tier1_message_3eb_27ce70() {
    enable(0x27ce70);
    let mut rng = Rng(0x2430_0000_0000_0003);
    let mut handled = 0;
    for seed in 0..3000u32 {
        let mut m = Memory::default();
        m.write32(OBJECT + 0x7c, EXT);
        m.write32(EXT + 0x2c, rng.next());
        m.write32(OBJECT + 0x28, rng.next());
        let message = if rng.below(2) == 0 {
            0x3eb
        } else {
            rng.below(0x800)
        };
        let (eax, regs, data) = adapter(0x27ce70, &m, &[OBJECT, message]);
        let mut want = expected_data(&m, &[OBJECT, message]);
        let want_eax = if message == 0x3eb {
            handled += 1;
            let flags = m.read32(EXT + 0x2c) | 0x100000;
            let active = m.read32(OBJECT + 0x28) & !1;
            for (address, value) in [(EXT + 0x2c, flags), (OBJECT + 0x28, active)] {
                for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
                    want.insert(address + i as u32, byte);
                }
            }
            want.retain(|_, v| *v != 0);
            0xffff_fffe
        } else {
            0
        };
        assert_eq!(eax, want_eax, "seed {seed} message {message:#x}");
        assert_eq!(data, want, "seed {seed} message {message:#x} memory");
        assert_eq!(regs.esp, STACK - 16);
    }
    assert!(handled > 1000);
}
