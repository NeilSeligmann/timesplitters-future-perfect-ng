// SPDX-License-Identifier: GPL-3.0-or-later
//! T1956 Tier 1: the c2r translations of the settings, input, inventory and pathfinding entries
//! replayed against goldens recorded from the ORIGINAL XBE under Unicorn (`tools/small_diff`,
//! `tools/small_diff/specs_c2r.py`). Callees are hash stubs mirrored by `callee_result`.
//! `python -m tools.small_diff --system settings` (and input, inventory, pathfinding) then
//! `cargo test --release -p tsfp-ports --test small_diff_c2r -- --ignored --nocapture`.
#[path = "../../tsfp-engine-memory/src/golden.rs"]
mod golden;

use golden::{Call, GoldenCase, GoldenWorld, STACK_TOP, compare, golden_dir, parse};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tsfp_port_runtime::abi::{ExtRegisters, GuestCall};
use tsfp_port_runtime::memory::GuestMemory;
use tsfp_ports::c2r;

// The C host provides this symbol in game builds. Differential fixtures do not call the
// timestamp-dependent paths, but the lifted archive still references the ABI symbol.
#[unsafe(no_mangle)]
extern "C" fn xbox_ReadTimeStampCounter() -> u64 {
    0
}

const ARENA: u32 = 0x0c10_0000;
const ARENA_SIZE: u32 = 0x4000;
const RETURN_VA: u32 = 0x7e00_0000;

/// Entries whose replay is known to differ, with the reason. They are reported, not failed, and the
/// test fails when one of them starts to match (remove it then). Names, never counts (T1956).
const KNOWN_DIFFERENT: &[(&str, &str, &str)] = &[];

type Shared = Rc<RefCell<GoldenWorld>>;

struct Mem(Shared);

impl GuestMemory for Mem {
    fn read8(&self, address: u32) -> u8 {
        self.0.borrow().read8(address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().write8(address, value);
    }
}

struct Guest {
    world: Shared,
    pops: Rc<HashMap<u32, (u32, usize, bool)>>,
}

/// Mirror of `stub_result` in tools/small_diff/specs_c2r.py.
fn callee_result(va: u32, args: &[u32]) -> u32 {
    let mut h: u32 = 2166136261;
    for word in std::iter::once(va).chain(args.iter().copied()) {
        for byte in word.to_le_bytes() {
            h = (h ^ u32::from(byte)).wrapping_mul(16777619);
        }
    }
    match h % 6 {
        0 => 0,
        1 => 1,
        2 => (h >> 8) & 0xff,
        3 => ARENA + ((h >> 8) % (ARENA_SIZE / 16)) * 16,
        4 => h,
        _ => 0xffff_ffff,
    }
}

impl GuestCall for Guest {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        let mut world = self.world.borrow_mut();
        let arity = self.pops.get(&va).map_or(0, |entry| entry.1);
        let mut args: Vec<u32> = (0..arity as u32)
            .map(|i| world.read32(regs.esp.wrapping_add(4 * i)))
            .collect();
        args.push(regs.ecx);
        args.push(regs.edx);
        let ret = callee_result(va, &args);
        regs.eax = ret;
        if self.pops.get(&va).is_some_and(|entry| entry.2) {
            regs.fpu.push(callee_float(va, &args));
        }
        regs.esp = regs
            .esp
            .wrapping_add(self.pops.get(&va).map_or(0, |entry| entry.0));
        world.calls.push(Call { va, ret, args });
    }
}

/// Mirror of `stub_float`: an exact float32 in [-32, 96).
fn callee_float(va: u32, args: &[u32]) -> f64 {
    let mut salted = args.to_vec();
    salted.push(0x0046_4c54);
    f64::from((callee_result(va, &salted) >> 4) & 0x3ff) / 8.0 - 32.0
}

fn load_pops(system: &str) -> HashMap<u32, (u32, usize, bool)> {
    let path = golden_dir().join(system).join("callees.txt");
    std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let va = u32::from_str_radix(parts.next()?, 16).ok()?;
            Some((
                va,
                (
                    parts.next()?.parse().ok()?,
                    parts.next()?.parse().ok()?,
                    parts.next()? == "1",
                ),
            ))
        })
        .collect()
}

type Runner = fn(u32, &mut Mem, &mut Guest, &mut ExtRegisters) -> bool;

fn run_c2r(va: u32, m: &mut Mem, g: &mut Guest, r: &mut ExtRegisters) -> bool {
    c2r::run(va, m, g, r)
}

/// The T1831 hand port (`weapons::run`, readable Rust), replayed against the same goldens (T1991).
fn run_hand(va: u32, m: &mut Mem, g: &mut Guest, r: &mut ExtRegisters) -> bool {
    tsfp_ports::weapons::run(va, m, g, r)
}

fn replay(
    case: &GoldenCase,
    pops: &Rc<HashMap<u32, (u32, usize, bool)>>,
    run: Runner,
) -> Result<(), String> {
    let world: Shared = Rc::new(RefCell::new(GoldenWorld::new(case)));
    let mut regs: ExtRegisters = unsafe { core::mem::zeroed() };
    regs.eax = case.reg("eax");
    regs.ecx = case.reg("ecx");
    regs.edx = case.reg("edx");
    regs.ebx = case.reg("ebx");
    regs.ebp = case.reg("ebp");
    regs.esi = case.reg("esi");
    regs.edi = case.reg("edi");
    regs.eflags = 0x202;
    let esp = STACK_TOP - 0x1000 - 4 * case.stack.len() as u32 - 4;
    regs.esp = esp;
    {
        let mut w = world.borrow_mut();
        for (i, word) in std::iter::once(&RETURN_VA)
            .chain(case.stack.iter())
            .enumerate()
        {
            for (k, byte) in word.to_le_bytes().iter().enumerate() {
                w.write8(esp + 4 * i as u32 + k as u32, *byte);
            }
        }
    }
    let mut mem = Mem(world.clone());
    let mut guest = Guest {
        world: world.clone(),
        pops: pops.clone(),
    };
    assert!(
        run(case.va, &mut mem, &mut guest, &mut regs),
        "no port holds {:#x}",
        case.va
    );
    let eax = if std::env::var("SMALL_DIFF_IGNORE_EAX").is_ok() {
        None
    } else {
        Some(regs.eax)
    };
    let result = compare(case, &world.borrow(), eax);
    result
}

fn replay_system(system: &str) {
    replay_with(system, run_c2r, &[]);
}

/// `only`: entry names to replay (empty = every golden of the system).
fn replay_with(system: &str, run: Runner, only_names: &[&str]) {
    let dir = golden_dir().join(system);
    let pops = Rc::new(load_pops(system));
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| {
            panic!(
                "missing goldens {} ({e}); run python -m tools.small_diff --system {system}",
                dir.display()
            )
        })
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("fn_"))
        })
        .collect();
    files.sort();
    assert!(
        !files.is_empty(),
        "no fn_*.txt goldens in {}",
        dir.display()
    );
    let only = std::env::var("SMALL_DIFF_ONLY").unwrap_or_default();
    let (mut total_ok, mut total_bad, mut report) = (0, 0, Vec::new());
    for path in files {
        let name = path
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        if !only_names.is_empty() && !only_names.contains(&name.as_str()) {
            continue;
        }
        if !only.is_empty() && !only.split(',').any(|o| o == name) {
            continue;
        }
        let cases = parse(&std::fs::read_to_string(&path).expect("golden"));
        let (mut ok, mut bad) = (0, 0);
        for case in &cases {
            match replay(case, &pops, run) {
                Ok(()) => ok += 1,
                Err(message) => {
                    bad += 1;
                    if bad <= 2 {
                        report.push(format!(
                            "{name}: stack={:x?} regs={:x?}: {message}",
                            case.stack, case.regs
                        ));
                    }
                }
            }
        }
        let known = KNOWN_DIFFERENT
            .iter()
            .find(|(s, n, _)| *s == system && *n == name);
        if let Some((_, _, why)) = known {
            eprintln!("{system} {name}: {ok} ok, {bad} differ (KNOWN: {why})");
            if std::env::var("SMALL_DIFF_VERBOSE").is_ok() {
                eprintln!("{}", report.join("\n"));
            }
            assert!(
                bad > 0,
                "{system} {name} now matches the original: remove it from KNOWN_DIFFERENT"
            );
            continue;
        }
        eprintln!("{system} {name}: {ok} ok, {bad} differ");
        total_ok += ok;
        total_bad += bad;
    }
    eprintln!("{system}: {total_ok} cases ok, {total_bad} differ");
    assert!(
        total_bad == 0,
        "{total_bad} cases differ:\n{}",
        report.join("\n")
    );
}

#[test]
#[ignore = "needs goldens: python -m tools.small_diff --system settings"]
fn settings_match_original() {
    replay_system("settings");
}

#[test]
#[ignore = "needs goldens: python -m tools.small_diff --system input"]
fn input_matches_original() {
    replay_system("input");
}

#[test]
#[ignore = "needs goldens: python -m tools.small_diff --system inventory"]
fn inventory_matches_original() {
    replay_system("inventory");
}

#[test]
#[ignore = "needs goldens: python -m tools.small_diff --system pathfinding"]
fn pathfinding_matches_original() {
    replay_system("pathfinding");
}

#[test]
#[ignore = "needs goldens: python -m tools.small_diff --system inventory"]
fn weapon_shot_hand_port_matches_original() {
    replay_with("inventory", run_hand, &["fn_1c0200"]);
}

/// T1991 cross-check report: replays every golden directory named in `XCHECK_SYSTEMS` (comma
/// separated, default all) through the c2r ports and prints one machine-readable line per entry,
/// `XCHECK <system> <entry> <ok> <differ> <first difference>`; it never fails on a difference
/// (`python -m tools.small_diff.crosscheck` consumes it). A panic inside a port counts as differ.
#[test]
#[ignore = "needs goldens: python -m tools.small_diff.crosscheck"]
fn crosscheck_report() {
    let systems = std::env::var("XCHECK_SYSTEMS").unwrap_or_default();
    let mut names: Vec<String> = std::fs::read_dir(golden_dir())
        .map(|dir| {
            dir.filter_map(|e| e.ok())
                .filter(|e| e.path().join("callees.txt").exists())
                .filter_map(|e| e.file_name().into_string().ok())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    std::panic::set_hook(Box::new(|_| {}));
    for system in names
        .iter()
        .filter(|n| systems.is_empty() || systems.split(',').any(|s| s == n.as_str()))
    {
        let dir = golden_dir().join(system);
        let pops = Rc::new(load_pops(system));
        let mut files: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("fn_"))
            })
            .collect();
        files.sort();
        for path in files {
            let name = path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_string();
            let (mut ok, mut bad, mut first) = (0, 0, String::new());
            for case in &parse(&std::fs::read_to_string(&path).expect("golden")) {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    replay(case, &pops, run_c2r)
                }));
                match outcome {
                    Ok(Ok(())) => ok += 1,
                    Ok(Err(message)) => {
                        bad += 1;
                        if first.is_empty() {
                            first = message;
                        }
                    }
                    Err(_) => {
                        bad += 1;
                        if first.is_empty() {
                            first = "port panicked".to_string();
                        }
                    }
                }
            }
            let first: String = first
                .chars()
                .take(160)
                .collect::<String>()
                .replace('\n', " ");
            println!("XCHECK {system} {name} {ok} {bad} {first}");
        }
    }
}

/// T2065 Tier 1: player root 0xCCA10 (`alloc_free_slot_or_append_and_init`) against goldens of the original.
/// `python -m tools.small_diff --system player --only fn_cca10 --cases 3000 --fresh`.
#[test]
#[ignore = "needs goldens: python -m tools.small_diff --system player --only fn_cca10"]
fn player_cca10_matches_original() {
    replay_with("player", run_c2r, &["fn_cca10"]);
}
