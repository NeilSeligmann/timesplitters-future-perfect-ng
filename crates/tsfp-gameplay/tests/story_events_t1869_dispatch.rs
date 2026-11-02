//! T1869: the routing of `PortedFiles` and `PortedShapeServices`. For every golden world the ported function is run
//! directly and through the dispatcher with the callee convention of the callers (stack words, then register words);
//! both runs must leave the same memory, make the same calls to the engine below and return the same value. This
//! pins the argument mapping of the dispatchers (the golden replays call the functions directly).
mod common;
use common::{Case, parse};
use std::collections::BTreeMap;
use tsfp_gameplay::story_events::strip_update::GuestMemory;
use tsfp_gameplay::story_events::svc_objects::DestroyEffects;
use tsfp_gameplay::story_events::svc_reloc::*;
use tsfp_gameplay::story_events::svc_shape_services::*;
use tsfp_gameplay::story_events::svc_spawn_c1950::set_callback;

const STACK_ARGS: u32 = 0x2FF_F100;
const REGS: u32 = 0x2FF_F140;
const STACK_TOP: u32 = 0x7F00_0000;

fn entry_of(nargs: u32) -> u32 {
    STACK_TOP - 0x1000 - 4 * nargs - 4
}

#[derive(Clone, Default, PartialEq, Debug)]
struct Mem {
    words: BTreeMap<u32, u32>,
}

impl GuestMemory for Mem {
    fn read_u8(&self, address: u32) -> u8 {
        (self.words.get(&(address & !3)).copied().unwrap_or(0) >> (8 * (address & 3))) as u8
    }
    fn write_u8(&mut self, address: u32, value: u8) {
        let shift = 8 * (address & 3);
        let word = self.words.entry(address & !3).or_insert(0);
        *word = (*word & !(0xFF << shift)) | (u32::from(value) << shift);
    }
    fn read_u32(&self, address: u32) -> u32 {
        (0..4).fold(0, |acc, k| {
            acc | u32::from(self.read_u8(address.wrapping_add(k))) << (8 * k)
        })
    }
    fn write_u32(&mut self, address: u32, value: u32) {
        for k in 0..4 {
            self.write_u8(address.wrapping_add(k), (value >> (8 * k)) as u8);
        }
    }
}

/// The engine below: records every call and answers with a value derived from the address and the argument sum, so
/// both runs see the same answers.
#[derive(Default)]
struct Log {
    calls: Vec<(u32, Vec<u32>)>,
}

impl DestroyEffects for Log {
    fn call(&mut self, _: &mut dyn GuestMemory, address: u32, args: &[u32]) -> u32 {
        self.calls.push((address, args.to_vec()));
        if address == 0x18380 {
            return 3; // a positive archive id: the record is updated, no registration
        }
        (address >> 4).wrapping_add(args.len() as u32)
    }
}

fn cases(dir: &str, name: &str) -> Vec<Case> {
    let path = format!(
        "{}/tests/data/{dir}/{name}.golden",
        env!("CARGO_MANIFEST_DIR")
    );
    parse(&std::fs::read_to_string(path).expect("golden"))
}

fn arg(case: &Case, index: u32) -> u32 {
    case.input(STACK_ARGS + 4 * index)
}

fn reg(case: &Case, index: u32) -> u32 {
    case.input(REGS + 4 * index)
}

fn world(case: &Case) -> Mem {
    Mem {
        words: case.ins.clone(),
    }
}

/// Runs `direct` and `routed` on copies of the golden worlds and compares memory, engine calls and results.
fn compare(
    name: &str,
    direct: impl FnMut(&mut Mem, &mut Log, &Case) -> u32,
    routed: impl FnMut(&mut Mem, &mut Log, &Case) -> u32,
) {
    compare_in("t1869", 72, name, direct, routed);
}

/// As [`compare`] with the goldens of another task directory and a case minimum.
fn compare_in(
    dir: &str,
    minimum: usize,
    name: &str,
    mut direct: impl FnMut(&mut Mem, &mut Log, &Case) -> u32,
    mut routed: impl FnMut(&mut Mem, &mut Log, &Case) -> u32,
) {
    let all = cases(dir, name);
    assert!(all.len() >= minimum, "{name}");
    let mut changed = 0;
    let mut results = std::collections::BTreeSet::new();
    for (index, case) in all.iter().enumerate() {
        let (mut a, mut b) = (world(case), world(case));
        let (mut log_a, mut log_b) = (Log::default(), Log::default());
        let result_a = direct(&mut a, &mut log_a, case);
        let result_b = routed(&mut b, &mut log_b, case);
        assert_eq!(result_a, result_b, "{name} case {index} result");
        results.insert(result_a);
        assert_eq!(log_a.calls, log_b.calls, "{name} case {index} engine calls");
        assert_eq!(a, b, "{name} case {index} memory");
        if a != world(case) {
            changed += 1;
        }
    }
    assert!(
        changed >= 10 || results.len() >= 2,
        "{name}: the cases must change memory or give varied results ({changed}, {results:?})"
    );
}

macro_rules! files {
    ($log:expr, $frame:expr) => {
        PortedFiles {
            inner: $log,
            frame: $frame,
        }
    };
}

macro_rules! services {
    ($log:expr, $entry:expr) => {
        PortedShapeServices {
            inner: $log,
            entry: $entry,
        }
    };
}

#[test]
fn files_route_relocate_names() {
    compare(
        "rel_names",
        |m, _l, c| {
            relocate_names(m, arg(c, 0), arg(c, 1));
            0
        },
        |m, l, c| files!(l, 0).call(m, 0x8E3E0, &[arg(c, 0), arg(c, 1)]),
    );
}

#[test]
fn files_route_register_by_index() {
    let frame = entry_of(0);
    compare(
        "rel_by_index",
        |m, l, c| {
            let mut files = files!(l, frame);
            register_by_index(m, &mut files, frame, reg(c, 0), reg(c, 1));
            0
        },
        |m, l, c| files!(l, frame).call(m, 0x8E310, &[reg(c, 0), reg(c, 1)]),
    );
}

#[test]
fn files_route_register_id() {
    let frame = entry_of(3);
    compare(
        "rel_register",
        |m, l, c| {
            let mut files = files!(l, frame);
            register_id(m, &mut files, frame, arg(c, 0), arg(c, 1), arg(c, 2));
            0
        },
        |m, l, c| files!(l, frame).call(m, 0x637E0, &[arg(c, 0), arg(c, 1), arg(c, 2)]),
    );
}

#[test]
fn files_route_relocate_table() {
    compare(
        "rel_table",
        |m, _l, c| {
            relocate_table(m, arg(c, 0), arg(c, 1), arg(c, 2));
            0
        },
        |m, l, c| files!(l, 0).call(m, 0x8DD10, &[arg(c, 0), arg(c, 1), arg(c, 2)]),
    );
}

#[test]
fn files_route_classify_and_present() {
    compare(
        "rel_classify",
        |m, l, c| {
            let mut files = files!(l, 0);
            classify_streams(m, &mut files, arg(c, 0));
            0
        },
        |m, l, c| files!(l, 0).call(m, 0x18FB10, &[arg(c, 0)]),
    );
    compare(
        "rel_present",
        |m, _, c| texture_present(m, arg(c, 0), arg(c, 1)),
        |m, l, c| files!(l, 0).call(m, 0x94620, &[arg(c, 0), arg(c, 1)]),
    );
}

#[test]
fn files_route_texture_register() {
    let frame = entry_of(2);
    compare(
        "rel_texture",
        |m, l, c| {
            let mut files = files!(l, frame);
            texture_register(m, &mut files, frame, reg(c, 0), arg(c, 0), arg(c, 1))
        },
        // the callers pass the two stack words first and edi last
        |m, l, c| files!(l, frame).call(m, 0x18390, &[arg(c, 0), arg(c, 1), reg(c, 0)]),
    );
}

#[test]
fn services_route_shape_done_and_dynamic() {
    let entry = entry_of(1) + 0x30;
    compare(
        "shs_done",
        |m, l, c| {
            let mut svc = services!(l, entry);
            shape_done(m, &mut svc, reg(c, 1), reg(c, 0), arg(c, 0));
            0
        },
        |m, l, c| services!(l, entry).call(m, 0x8FE30, &[arg(c, 0), reg(c, 0), reg(c, 1)]),
    );
    // T1872: the pose engine runs ported inside the dispatcher, so the worlds must hold acyclic part trees (the
    // random parts of `shs_dynamic` loop forever in the real walks): the whole root goldens of the pose engine.
    compare_in(
        "t1872",
        40,
        "pose_root",
        |m, l, c| {
            let mut svc = services!(l, entry);
            shape_dynamic(m, &mut svc, entry - 0x30, reg(c, 0), reg(c, 1), arg(c, 0));
            0
        },
        |m, l, c| services!(l, entry).call(m, 0x976E0, &[arg(c, 0), reg(c, 1), reg(c, 0)]),
    );
}

#[test]
fn services_route_block_retag_light_weapon() {
    let entry = entry_of(2);
    compare(
        "shs_block",
        |m, _, c| {
            register_block(m, reg(c, 0), reg(c, 1), arg(c, 0));
            0
        },
        |m, l, c| services!(l, entry).call(m, 0x8FB20, &[arg(c, 0), reg(c, 0), reg(c, 1)]),
    );
    compare(
        "shs_retag",
        |m, _, c| {
            retag_streams(m, arg(c, 0), arg(c, 1), arg(c, 2), arg(c, 3), arg(c, 4));
            0
        },
        |m, l, c| {
            services!(l, entry).call(
                m,
                0x18FDC0,
                &[arg(c, 0), arg(c, 1), arg(c, 2), arg(c, 3), arg(c, 4)],
            )
        },
    );
    compare(
        "rel_light",
        |m, _, _| light_slot(m),
        |m, l, _| services!(l, entry).call(m, 0x6A540, &[]),
    );
    compare(
        "rel_weapon",
        |m, _, c| weapon_class(m, arg(c, 0)),
        |m, l, c| services!(l, entry).call(m, 0x1B99B0, &[arg(c, 0)]),
    );
}

#[test]
fn services_route_detach_to_the_callback_store() {
    let mut direct = Mem::default();
    let mut routed = Mem::default();
    direct.write_u32(0x7D_E318, 0x1234);
    routed.write_u32(0x7D_E318, 0x1234);
    set_callback(&mut direct, 0x0200_0000, 3, 0x0005_CEF0);
    let mut log = Log::default();
    services!(&mut log, 0).call(&mut routed, 0xA5CE0, &[0x0200_0000, 3, 0x0005_CEF0]);
    assert_eq!(direct, routed);
    assert_eq!(routed.read_u32(0x0200_0000 + 0x154 + 12), 0x0005_CEF0);
    assert_eq!(routed.read_u32(0x0200_0000 + 0x90), 0x1234, "frame stamp");
    assert!(log.calls.is_empty());
}
