// SPDX-License-Identifier: GPL-3.0-or-later
// ======== T3621 capdiff: "capture once, differential many" (docs/decisions/capture-differential.md) ========
// Tier 1R: lifted original versus the Rust port on REAL captured inputs. `tools.capdiff` captures every
// call of a ported root during ONE owner-route replay (registers, stack words and the guest regions the
// root reads), flattens the corpus into the line format below and starts this test (one process per
// shard). Each case loads the captured regions into both guest images, runs the lifted original on the C
// image and the Rust port on the Rust image, and compares EAX, the nested-call trace (out-of-memory
// reports) and the watched memory. No game build or replay is needed to rerun.
//
//   case <id> <root> <adapter> <eax> <ecx> <edx> <ebx> <esi> <edi> <ebp> <ret> <arg1> <arg2> <arg3>
//   mem <addr> <hexbytes>            (any number, belongs to the preceding case)
//
// Adapters: `arena_alloc` (0x3DFA0, Arena::alloc_round16) and the three Story event allocations issued
// by lifted entry 0x237480 (`story_instances`, `story_indices`, `story_snapshot`, the AllocWrappers
// slices). The static jump tables of 0x3DFA0 are the constant prelude of the XBE image, not captured.
use super::*;
use tsfp_engine_memory::wrappers::AllocWrappers;
use tsfp_ports::weapon_inventory::{
    AMMO_TYPE_COUNT, ENTRY_COUNT, ProfileWeaponInventory, WeaponInventory, WeaponInventoryStore,
    WeaponPlayerId,
};
use tsfp_ports::{abi::ExtRegisters, memory::GuestMemory as PortMemory, weapon_hands_runtime};

const WINDOW: (u32, u32) = (0x6B8370, 0x50);
const SENTINEL: u32 = 0xA5A5_A5A5;
const GLOBALS: [(u32, &str); 9] = [
    (0x6B8370, "REGION_A_CURSOR"),
    (0x6B8378, "REGION_A_REMAINING"),
    (0x6B837C, "REGION_B_CURSOR"),
    (0x6B8380, "BOTTOM_LIMIT"),
    (0x6B8384, "MAIN_REMAINING"),
    (0x6B8388, "TOP_CURSOR"),
    (0x6B8390, "KIND4_CURSOR"),
    (0x6B839C, "BUMP_CURSOR"),
    (0x6B83B8, "BUMP_TOTAL"),
];

struct Row {
    id: String,
    root: u32,
    adapter: String,
    regs: [u32; 7],
    stack: [u32; 4],
    mem: Vec<(u32, Vec<u8>)>,
}

impl PortMemory for View {
    fn read8(&self, address: u32) -> u8 {
        self.byte(address)
    }

    fn write8(&mut self, address: u32, value: u8) {
        self.set(address, value)
    }
}

fn hex(token: &str) -> u32 {
    u32::from_str_radix(token, 16).unwrap_or_else(|_| panic!("capdiff: bad hex {token:?}"))
}

fn parse(text: &str) -> Vec<Row> {
    let mut rows: Vec<Row> = vec![];
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        match t.first().copied() {
            Some("case") => rows.push(Row {
                id: t[1].to_string(),
                root: hex(t[2]),
                adapter: t[3].to_string(),
                regs: std::array::from_fn(|i| hex(t[4 + i])),
                stack: std::array::from_fn(|i| hex(t[11 + i])),
                mem: vec![],
            }),
            Some("mem") => {
                let bytes = (0..t[2].len() / 2)
                    .map(|i| {
                        u8::from_str_radix(&t[2][2 * i..2 * i + 2], 16).expect("capdiff: bad byte")
                    })
                    .collect();
                rows.last_mut()
                    .expect("capdiff: mem before case")
                    .mem
                    .push((hex(t[1]), bytes));
            }
            _ => {}
        }
    }
    rows
}

fn global_name(address: u32) -> String {
    let base = address & !3;
    match GLOBALS.iter().find(|(a, _)| *a == base) {
        Some((_, name)) => format!("{address:08x} ({name}+{})", address - base),
        None => format!("{address:08x}"),
    }
}

/// One case. Returns the field-level differences (empty means the Rust port matched the lifted original).
fn run_case(p: &mut Pair, row: &Row) -> Vec<String> {
    if row.adapter == "inventory_lookup" {
        return run_inventory_lookup(p, row);
    }
    if row.adapter == "pickup_tick" {
        return run_pickup_tick(p, row);
    }
    if row.adapter == "audio_mask_bit" {
        return run_audio_mask_bit(p, row);
    }
    let published = match row.adapter.as_str() {
        "story_instances" => Some(0x78CD90),
        "story_indices" => Some(0x78CD8C),
        "story_snapshot" => Some(0x75CCA4),
        _ => None,
    };
    // Constant prelude (jump tables of 0x3DFA0 and 0x3E0B0 inside the XBE image).
    for (i, t) in [0x3DFBFu32, 0x3E032, 0x3E063, 0x3E08D, 0x3E07B]
        .iter()
        .enumerate()
    {
        p.put32(0x3E094 + 4 * i as u32, *t);
    }
    for (address, bytes) in &row.mem {
        for (i, b) in bytes.iter().enumerate() {
            p.put8(address.wrapping_add(i as u32), *b);
        }
    }
    if let Some(address) = published {
        p.put32(address, SENTINEL);
    }
    let (size, kind) = (row.stack[1], row.stack[2]);
    let args = if row.adapter == "weapon_transfer" {
        row.stack[1..4].to_vec()
    } else {
        vec![size, kind]
    };
    let case = W4Case {
        va: row.root,
        args,
        regs: row.regs,
        script: vec![],
    };
    let (cregs, ctrace) = run_c(p, &case);
    let c_reports: Vec<u32> = ctrace
        .iter()
        .filter(|e| e[0] == 0x389B0)
        .map(|e| e[3])
        .collect();
    let mut view = View(p.rust);
    let mut oom = Oom::default();
    let eax = match row.adapter.as_str() {
        "arena_alloc" => Arena {
            memory: &mut AMem(&mut view),
            hooks: &mut oom,
        }
        .alloc_round16(kind, size)
        .unwrap(),
        "story_instances" | "story_indices" | "story_snapshot" => {
            let mut memory = AMem(&mut view);
            let mut wrappers = AllocWrappers {
                memory: &mut memory,
                hooks: &mut oom,
            };
            let block = match row.adapter.as_str() {
                "story_instances" => wrappers.alloc_story_event_instances_78cd90(size),
                "story_indices" => wrappers.alloc_story_event_indices_78cd8c(size),
                _ => wrappers.alloc_story_event_snapshot_75cca4(size),
            }
            .unwrap();
            let address = published.unwrap();
            let seen = rd(&View(p.rust), address);
            let mut why = vec![];
            if seen != block {
                why.push(format!(
                    "publication {address:08x}: wrapper returned {block:x} but published {seen:x}"
                ));
            }
            let mut again = View(p.rust);
            again.set(address, SENTINEL as u8);
            for i in 1..4 {
                again.set(address + i, (SENTINEL >> (8 * i)) as u8);
            }
            if !why.is_empty() {
                return why;
            }
            block
        }
        "weapon_transfer" => {
            let mut memory = View(p.rust);
            let ext = row.stack[1];
            let slot = PortMemory::read32(&memory, ext.wrapping_add(8));
            if slot >= 16 {
                return vec![format!("unsupported Rust player slot {slot}")];
            }
            let mut owned = [false; ENTRY_COUNT];
            for (index, value) in owned.iter_mut().enumerate() {
                *value = PortMemory::read8(&memory, ext.wrapping_add(0x54c + index as u32)) != 0;
            }
            let mut reserve = [0i16; AMMO_TYPE_COUNT];
            let mut loaded_rounds = [[0i16; AMMO_TYPE_COUNT]; 2];
            for index in 0..AMMO_TYPE_COUNT {
                let offset = index as u32 * 2;
                reserve[index] =
                    PortMemory::read16(&memory, ext.wrapping_add(0x61e + offset)) as i16;
                loaded_rounds[0][index] =
                    PortMemory::read16(&memory, ext.wrapping_add(0xdc + offset)) as i16;
                loaded_rounds[1][index] =
                    PortMemory::read16(&memory, ext.wrapping_add(0x20c + offset)) as i16;
            }
            let desired = PortMemory::read32(&memory, ext.wrapping_add(0x94));
            let profile = ProfileWeaponInventory {
                desired_entry: u8::try_from(desired).ok(),
                owned,
                reserve,
                loaded_rounds,
            };
            let player = WeaponPlayerId::new(slot as u16);
            let mut store = WeaponInventoryStore::<16>::default();
            let Some(handle) = store.begin_lifetime(player, WeaponInventory::default()) else {
                return vec![format!("could not initialize captured player slot {slot}")];
            };
            if !store.restore_profile_for_lifetime(handle, profile) {
                return vec![format!("could not restore captured player slot {slot}")];
            }
            let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
            regs.eax = row.regs[0];
            regs.ecx = row.regs[1];
            regs.edx = row.regs[2];
            regs.ebx = row.regs[3];
            regs.esi = row.regs[4];
            regs.edi = row.regs[5];
            regs.ebp = row.regs[6];
            regs.esp = STACK;
            if !weapon_hands_runtime::transfer_from_owned_store(
                &mut memory,
                &mut regs,
                &mut store,
                player,
            ) {
                return vec![format!(
                    "Rust owner declined captured hand {:#x}",
                    row.regs[0]
                )];
            }
            regs.eax
        }
        other => return vec![format!("unknown adapter {other}")],
    };
    let mut why = vec![];
    if cregs[0] != eax {
        why.push(format!(
            "eax: lifted {:#x} rust {eax:#x} (size {size:#x} kind {kind})",
            cregs[0]
        ));
    }
    if c_reports != oom.0 {
        why.push(format!(
            "out-of-memory reports: lifted {c_reports:x?} rust {:x?}",
            oom.0
        ));
    }
    let mut ranges = vec![WINDOW];
    if row.adapter == "weapon_transfer" {
        ranges.extend(
            row.mem
                .iter()
                .map(|(address, bytes)| (*address, bytes.len() as u32)),
        );
    }
    if kind == 0 && size != 0 {
        ranges.push((cregs[0].wrapping_sub(16), 16)); // LOCALMEM header written at the new top
    }
    if let Some(address) = published {
        ranges.push((address, 4));
    }
    for (start, len) in ranges {
        for off in 0..len {
            let a = start.wrapping_add(off);
            let (c, r) = (p.get(false, a), p.get(true, a));
            if c != r {
                why.push(format!(
                    "memory {}: lifted {c:02x} rust {r:02x}",
                    global_name(a)
                ));
            }
        }
    }
    why
}

/// Tier 1R adapter for the inventory Rule 24 lookup roots. Explicitly seeds static table
/// regions too: these roots read them without receiving pointers in their call arguments.
fn run_inventory_lookup(p: &mut Pair, row: &Row) -> Vec<String> {
    zero(p, STACK, ARG_FRAME);
    for (address, bytes) in &row.mem {
        for (offset, byte) in bytes.iter().enumerate() {
            p.put8(address.wrapping_add(offset as u32), *byte);
        }
    }
    let case = W4Case {
        va: row.root,
        args: row.stack[1..4].to_vec(),
        regs: row.regs,
        script: vec![],
    };
    let (cregs, _) = run_c(p, &case);
    struct NoNestedCalls;
    impl tsfp_ports::abi::GuestCall for NoNestedCalls {
        fn call(&mut self, va: u32, _: &mut tsfp_ports::abi::ExtRegisters) {
            panic!("unexpected nested call from inventory lookup {va:#x}");
        }
    }
    let mut regs: tsfp_ports::abi::ExtRegisters = unsafe { core::mem::zeroed() };
    regs.eax = row.regs[0];
    regs.ecx = row.regs[1];
    regs.edx = row.regs[2];
    regs.ebx = row.regs[3];
    regs.esi = row.regs[4];
    regs.edi = row.regs[5];
    regs.ebp = row.regs[6];
    regs.esp = STACK;
    // Keep this bridge local: the general w4 GuestMemory impl is behind `w4_lifted`,
    // while capdiff corpus tests also compile when the private lifted tree is absent.
    struct InventoryMemory(View);
    impl tsfp_ports::memory::GuestMemory for InventoryMemory {
        fn read8(&self, address: u32) -> u8 {
            self.0.byte(address)
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0.set(address, value);
        }
    }
    let mut memory = InventoryMemory(View(p.rust));
    let mut guest = NoNestedCalls;
    assert!(tsfp_ports::rea_inventory_runtime::run_enabled(
        row.root,
        &mut memory,
        &mut guest,
        &mut regs
    ));
    let rregs = [
        regs.eax, regs.ecx, regs.edx, regs.ebx, regs.esi, regs.edi, regs.ebp, regs.esp,
    ];
    let mut why = vec![];
    // The readable lookup owner models only the live return and caller-saved result registers;
    // the lane bridge, not this owner, reproduces callee-saved and ESP mechanics.
    for index in 0..3 {
        if cregs[index] != rregs[index] {
            why.push(format!(
                "reg[{index}]: lifted {:#x} rust {:#x}",
                cregs[index], rregs[index]
            ));
        }
    }
    for (address, bytes) in &row.mem {
        for (offset, _) in bytes.iter().enumerate() {
            let at = address.wrapping_add(offset as u32);
            let (c, rust) = (p.get(false, at), p.get(true, at));
            if c != rust {
                why.push(format!("memory {at:08x}: lifted {c:02x} rust {rust:02x}"));
                if why.len() >= 24 {
                    return why;
                }
            }
        }
    }
    why
}

/// Tier 1R adapter for the ported `0xC1850` inventory/weapons pickup tick.
fn run_pickup_tick(p: &mut Pair, row: &Row) -> Vec<String> {
    zero(p, STACK, ARG_FRAME);
    for (address, bytes) in &row.mem {
        for (offset, byte) in bytes.iter().enumerate() {
            p.put8(address.wrapping_add(offset as u32), *byte);
        }
    }
    let case = W4Case {
        va: row.root,
        args: row.stack[1..4].to_vec(),
        regs: row.regs,
        script: vec![],
    };
    let (cregs, mut ctrace) = run_c(p, &case);
    let mut stub = Stub {
        script: vec![],
        trace: vec![],
    };
    let mut cpu = Cpu {
        eax: row.regs[0],
        ecx: row.regs[1],
        edx: row.regs[2],
        ebx: row.regs[3],
        esi: row.regs[4],
        edi: row.regs[5],
        ebp: row.regs[6],
        esp: STACK,
    };
    let mut view = View(p.rust);
    game_b::pickup_frame_tick(&mut stub, &mut view, &mut cpu);
    if std::env::var("TSFP_CAPDIFF_SEED_BUG").as_deref() == Ok("pickup_tick") {
        cpu.ebx ^= 1;
    }
    let rregs = [
        cpu.eax,
        cpu.ecx,
        cpu.edx,
        cpu.ebx,
        cpu.esi,
        cpu.edi,
        cpu.ebp,
        cpu.esp.wrapping_add(4),
    ];
    let mut why = vec![];
    for index in 3..8 {
        if cregs[index] != rregs[index] {
            why.push(format!(
                "reg[{index}]: lifted {:#x} rust {:#x}",
                cregs[index], rregs[index]
            ));
        }
    }
    mask_entry(&mut ctrace);
    mask_entry(&mut stub.trace);
    if ctrace != stub.trace {
        why.push(format!(
            "nested calls: lifted {ctrace:x?} rust {:x?}",
            stub.trace
        ));
    }
    for (address, bytes) in &row.mem {
        for (offset, _) in bytes.iter().enumerate() {
            let at = address.wrapping_add(offset as u32);
            let (c, rust) = (p.get(false, at), p.get(true, at));
            if c != rust {
                why.push(format!("memory {at:08x}: lifted {c:02x} rust {rust:02x}"));
                if why.len() >= 24 {
                    return why;
                }
            }
        }
    }
    why
}

/// Tier 1R adapter for the call-free audio mask query.
fn run_audio_mask_bit(p: &mut Pair, row: &Row) -> Vec<String> {
    zero(p, STACK, ARG_FRAME);
    for (address, bytes) in &row.mem {
        for (offset, byte) in bytes.iter().enumerate() {
            p.put8(address.wrapping_add(offset as u32), *byte);
        }
    }
    let bit = row.stack[1];
    let case = W4Case {
        va: row.root,
        args: vec![bit],
        regs: row.regs,
        script: vec![],
    };
    let (cregs, _) = run_c(p, &case);
    let view = View(p.rust);
    let port = tsfp_engine_audio::leaves::global_mask_bit_test(
        &view,
        bit,
        tsfp_engine_audio::gprs::Gprs {
            eax: row.regs[0],
            ecx: row.regs[1],
            edx: row.regs[2],
        },
    );
    let mut why = vec![];
    let port_eax = if std::env::var("TSFP_CAPDIFF_SEED_BUG").as_deref() == Ok("audio_mask_bit") {
        port.eax ^ 1
    } else {
        port.eax
    };
    for (index, rust) in [port_eax, port.ecx, port.edx].into_iter().enumerate() {
        if cregs[index] != rust {
            why.push(format!(
                "reg[{index}]: lifted {:#x} rust {rust:#x}",
                cregs[index]
            ));
        }
    }
    for (address, bytes) in &row.mem {
        for (offset, _) in bytes.iter().enumerate() {
            let at = address.wrapping_add(offset as u32);
            let (c, rust) = (p.get(false, at), p.get(true, at));
            if c != rust {
                why.push(format!("memory {at:08x}: lifted {c:02x} rust {rust:02x}"));
            }
        }
    }
    why
}

/// Entry point used by `python3 -m tools.capdiff run`. Without TSFP_CAPDIFF_CASES the test is a no-op
/// that prints `CAPDIFF-SKIP` (the runner treats a missing summary line as a failure).
pub(crate) fn run_corpus() {
    let Ok(path) = std::env::var("TSFP_CAPDIFF_CASES") else {
        eprintln!("CAPDIFF-SKIP no TSFP_CAPDIFF_CASES");
        return;
    };
    let (shard, shards) = std::env::var("TSFP_CAPDIFF_SHARD")
        .ok()
        .and_then(|s| {
            s.split_once('/')
                .map(|(a, b)| (a.parse::<usize>().unwrap(), b.parse::<usize>().unwrap()))
        })
        .unwrap_or((0, 1));
    let max_fails: usize = std::env::var("TSFP_CAPDIFF_MAX_FAILS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    let rows = parse(&std::fs::read_to_string(&path).expect("capdiff: cannot read cases"));
    let mut guard = pair().lock().unwrap_or_else(|e| e.into_inner());
    let p = &mut *guard;
    let (mut ran, mut failed) = (0usize, 0usize);
    for (index, row) in rows.iter().enumerate() {
        if index % shards != shard {
            continue;
        }
        zero(p, STACK, ARG_FRAME);
        zero(p, WINDOW.0, WINDOW.1);
        let why = run_case(p, row);
        ran += 1;
        if !why.is_empty() {
            failed += 1;
            if failed <= max_fails {
                eprintln!(
                    "CAPDIFF-FAIL case={} root={:#x} adapter={} stack={:x?} diff={why:?}",
                    row.id, row.root, row.adapter, row.stack
                );
            }
        }
    }
    eprintln!("CAPDIFF-RESULT shard={shard}/{shards} cases={ran} failed={failed}");
}
