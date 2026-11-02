//! T1846 batch 8: strip update `0x1F2810` and the `0x1F48A0` spawner against golden vectors from the original
//! machine code under the Unicorn oracle (MEASURED).
mod common;
use common::{Case, parse};
use tsfp_gameplay::story_events::strip_update::*;

const POOL: u32 = 0x200_0000;

fn words16(case: &Case, address: u32, count: usize) -> Vec<i16> {
    (0..count)
        .map(|i| {
            let dword = case.input(address + 4 * (i as u32 / 2));
            (dword >> (16 * (i as u32 % 2))) as u16 as i16
        })
        .collect()
}

fn out_words16(case: &Case, address: u32, count: usize) -> Vec<u16> {
    (0..count)
        .map(|i| (case.output(address + 4 * (i as u32 / 2)) >> (16 * (i as u32 % 2))) as u16)
        .collect()
}

/// Mirrors the engine pool plus the key list `0x23FF20` over the case memory.
struct CaseObjects<'a> {
    case: &'a Case,
    values: [Option<f32>; 4],
}

impl CaseObjects<'_> {
    fn slot_of(handle: u32) -> u32 {
        handle >> 22
    }
    fn base(&self, handle: u32) -> u32 {
        POOL + Self::slot_of(handle) * 0x2E8
    }
}

impl StripObjects for CaseObjects<'_> {
    fn follower(&mut self, handle: u32) -> Option<FollowerView> {
        let base = self.base(handle);
        (self.case.input(base + 0x2C) == handle).then(|| FollowerView {
            kind: self.case.input(base + 0x60),
            offset: self.case.input_f(base + 0xA8),
        })
    }
    fn anchor_value(&mut self, key: u32) -> Option<f32> {
        let mut node = self.case.input(0x75D318);
        while node != 0 {
            let node_key = self.case.input(node + 4);
            if node_key == key {
                let handle = self.case.input(node);
                if handle == 0 {
                    return None;
                }
                let base = self.base(handle);
                return (self.case.input(base + 0x2C) == handle)
                    .then(|| self.case.input_f(base + 0x64));
            }
            if node_key > key {
                return None;
            }
            node = self.case.input(node + 8);
        }
        None
    }
    fn set_follower_value(&mut self, handle: u32, value: f32) {
        self.values[Self::slot_of(handle) as usize] = Some(value);
    }
}

fn tables_from(case: &Case) -> StripTables {
    let mut tables = StripTables {
        kind_a: [0; 20],
        kind_b: [0; 20],
        cells_a: [[0; 3]; 20],
        cells_b: [[0; 3]; 20],
        mask_a: [0; 20],
        mask_b: [0; 20],
        last_slot: case.input(0x4FFE54) as i32,
        queue_pos: case.input(0x75BF10) as u16 as i16,
        queue_a: [0; 4],
        queue_b: [0; 4],
        followers: [0; 4],
        follower_count: case.input(0x75BFF0) as i32,
    };
    tables.kind_a.copy_from_slice(&words16(case, 0x75BCEC, 20));
    tables.kind_b.copy_from_slice(&words16(case, 0x75BD14, 20));
    tables.queue_a.copy_from_slice(&words16(case, 0x75BC20, 4));
    tables.queue_b.copy_from_slice(&words16(case, 0x75BFD4, 4));
    for i in 0..4 {
        tables.followers[i] = case.input(0x75BD90 + 4 * i as u32);
    }
    for i in 0..20 {
        tables.mask_a[i] = case.input(0x75BDA0 + 4 * i as u32);
        tables.mask_b[i] = case.input(0x75BD40 + 4 * i as u32);
    }
    tables
}

#[test]
fn strip_update_matches_original() {
    let cases = parse(include_str!("data/t1846/strip_update_1f2810.golden"));
    assert!(cases.len() >= 60);
    let (
        mut regenerated,
        mut skipped,
        mut cleared,
        mut follow_first,
        mut follow_second,
        mut scripted,
        mut masks,
    ) = (0, 0, 0, 0, 0, 0, 0);
    for (n, case) in cases.iter().enumerate() {
        let mut tables = tables_from(case);
        // the original keeps the untouched cell words as they were: seed them from the case input
        let cells_a_in = out_words16(
            &Case {
                outs: case.ins.clone(),
                ..case.clone()
            },
            0x75BDF8,
            60,
        );
        let cells_b_in = out_words16(
            &Case {
                outs: case.ins.clone(),
                ..case.clone()
            },
            0x75BE70,
            60,
        );
        for slot in 0..20 {
            for k in 0..3 {
                tables.cells_a[slot][k] = cells_a_in[slot * 3 + k];
                tables.cells_b[slot][k] = cells_b_in[slot * 3 + k];
            }
        }
        let lo = case.input(0x4E7950) as u64;
        let hi = case.input(0x4E7954) as u64;
        let mut rng = Lcg64 {
            state: lo | (hi << 32),
        };
        let mut objects = CaseObjects {
            case,
            values: [None; 4],
        };
        let frame_sum = (case.input(0x7DE318) as i32).wrapping_add(case.input(0x4E7944) as i32);
        let before = tables.clone();
        let positions_live = (0..before.follower_count.clamp(0, 4) as usize)
            .filter(|&i| before.followers[i] != 0)
            .count();
        strip_update(
            &mut tables,
            frame_sum,
            case.input_f(0x4FFE50),
            &mut rng,
            &mut objects,
        );
        if tables.last_slot != before.last_slot {
            regenerated += 1;
            if (0..4).contains(&before.queue_pos) {
                scripted += 1;
            }
            if tables
                .mask_a
                .iter()
                .zip(&before.mask_a)
                .any(|(a, b)| a != b && *a != 0)
            {
                masks += 1;
            }
        } else {
            skipped += 1;
        }
        if positions_live > 0
            && tables
                .followers
                .iter()
                .zip(&before.followers)
                .any(|(a, b)| a != b)
        {
            cleared += 1;
        }
        let ctx = format!("case {n}");
        let out_a = out_words16(case, 0x75BCEC, 20);
        let out_b = out_words16(case, 0x75BD14, 20);
        for slot in 0..20 {
            assert_eq!(
                tables.kind_a[slot] as u16, out_a[slot],
                "{ctx} kind_a {slot}"
            );
            assert_eq!(
                tables.kind_b[slot] as u16, out_b[slot],
                "{ctx} kind_b {slot}"
            );
            assert_eq!(
                tables.mask_a[slot],
                case.output(0x75BDA0 + 4 * slot as u32),
                "{ctx} mask_a {slot}"
            );
            assert_eq!(
                tables.mask_b[slot],
                case.output(0x75BD40 + 4 * slot as u32),
                "{ctx} mask_b {slot}"
            );
        }
        let (out_ca, out_cb) = (
            out_words16(case, 0x75BDF8, 60),
            out_words16(case, 0x75BE70, 60),
        );
        for slot in 0..20 {
            for k in 0..3 {
                assert_eq!(
                    tables.cells_a[slot][k],
                    out_ca[slot * 3 + k],
                    "{ctx} cells_a {slot}.{k}"
                );
                assert_eq!(
                    tables.cells_b[slot][k],
                    out_cb[slot * 3 + k],
                    "{ctx} cells_b {slot}.{k}"
                );
            }
        }
        for i in 0..4 {
            assert_eq!(
                tables.followers[i],
                case.output(0x75BD90 + 4 * i as u32),
                "{ctx} follower {i}"
            );
            let address = POOL + i as u32 * 0x2E8 + 0x64;
            let expected = objects.values[i].map_or(case.input(address), |v| v.to_bits());
            assert_eq!(expected, case.output(address), "{ctx} object value {i}");
            if let Some(view) =
                objects.values[i].and(Some(case.input(POOL + i as u32 * 0x2E8 + 0x60)))
            {
                if view == KIND_FOLLOW_FIRST {
                    follow_first += 1;
                } else if view == KIND_FOLLOW_SECOND {
                    follow_second += 1;
                }
            }
        }
        assert_eq!(
            tables.last_slot as u32,
            case.output(0x4FFE54),
            "{ctx} last slot"
        );
        assert_eq!(
            tables.queue_pos as u16,
            case.output(0x75BF10) as u16,
            "{ctx} queue pos"
        );
        assert_eq!(rng.state as u32, case.output(0x4E7950), "{ctx} rng low");
        assert_eq!(
            (rng.state >> 32) as u32,
            case.output(0x4E7954),
            "{ctx} rng high"
        );
    }
    assert!(
        regenerated > 15 && skipped >= 1 && cleared >= 1,
        "{regenerated} {skipped} {cleared}"
    );
    assert!(follow_first >= 1 && follow_second >= 1 && scripted >= 1 && masks >= 1);
}

#[test]
fn descriptor_table_semantics() {
    assert_eq!(STRIP_KINDS.len(), 10);
    assert!(
        STRIP_KINDS
            .iter()
            .step_by(2)
            .all(|k| k.id % 2 == 1 || k.id == 0x310 || k.id % 2 == 0)
    );
    // kinds 2 and 3 are never chosen at random (flag bit 0)
    assert_eq!(strip_kind(2).flags & 1, 1);
    assert_eq!(strip_kind(10).cells, 0);
}

#[test]
fn slot_index_is_stable_without_scroll() {
    // speed 0 gives phase 0: slot = trunc(1187 * 0.00653347) % 20 = 7
    assert_eq!(strip_slot_index(123_456, 0.0), 7);
    assert_eq!(strip_slot_index(0, 4.0), strip_slot_index(0, 4.0));
}

#[test]
fn lcg_matches_recurrence() {
    let mut rng = Lcg64 { state: 1 };
    assert_eq!(rng.next(), (0x343FD_u64 + 0x269EC3) as u32);
}

// ---------------------------------------------------------------------------------------------------------
// 0x1F48A0 against the original with stubbed services
// ---------------------------------------------------------------------------------------------------------

use std::collections::HashMap;

const ENTRY_ESP: u32 = 0x7F00_0000 - 0x1000 - 4;
const LOG: u32 = 0x220_0000;
const OUTQ: u32 = 0x223_0000;
const RETQ: u32 = 0x222_0000;

struct Memory(HashMap<u32, u8>);

impl Memory {
    fn from_case(case: &Case) -> Self {
        let mut bytes = HashMap::new();
        for (&address, &word) in &case.ins {
            for (i, b) in word.to_le_bytes().iter().enumerate() {
                bytes.insert(address + i as u32, *b);
            }
        }
        Memory(bytes)
    }
}

impl GuestMemory for Memory {
    fn read_u8(&self, address: u32) -> u8 {
        *self.0.get(&address).unwrap_or(&0)
    }
    fn write_u8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
    fn read_u32(&self, address: u32) -> u32 {
        u32::from_le_bytes([0, 1, 2, 3].map(|i| self.read_u8(address + i)))
    }
    fn write_u32(&mut self, address: u32, value: u32) {
        for (i, b) in value.to_le_bytes().iter().enumerate() {
            self.0.insert(address + i as u32, *b);
        }
    }
}

/// id, argument count, dereferenced blocks (argument, dwords), written blocks (argument, byte offset, dwords),
/// class lookup from the argument object (+0x300) instead of the return queue.
type ServiceSpec = (
    u32,
    usize,
    &'static [(usize, u32)],
    &'static [(usize, u32, u32)],
    bool,
);

fn spec(service: Service) -> ServiceSpec {
    match service {
        Service::FindObjectA => (1, 1, &[], &[], false),
        Service::ClassOf => (2, 1, &[], &[], true),
        Service::QueryState => (3, 3, &[], &[(1, 0, 1), (2, 0, 1)], false),
        Service::StartEffect => (4, 2, &[(1, 3)], &[], false),
        Service::EmitterOffset => (5, 5, &[(0, 4)], &[(0, 0, 4)], false),
        Service::TransformVec => (6, 2, &[(0, 16), (1, 4)], &[(1, 0, 4)], false),
        Service::SpawnParticle => (7, 3, &[(0, 4), (1, 4)], &[], false),
        Service::WorldMatrix => (8, 4, &[], &[(0, 0, 16)], false),
        Service::LookupByKey => (9, 1, &[], &[], false),
        Service::FlagTest => (10, 2, &[], &[], false),
        Service::CurrentOwner => (11, 1, &[], &[], false),
        Service::SpawnObject => (12, 10, &[(4, 3)], &[], false),
        Service::Poll => (13, 1, &[], &[], false),
        Service::FindObjectB => (14, 1, &[], &[], false),
        Service::SetControllerMode => (15, 3, &[], &[], false),
        Service::Destroy => (16, 1, &[], &[], false),
    }
}

/// Reference services: scripted return values and output words from the golden input, calls logged like the stubs.
struct ScriptedServices<'a> {
    case: &'a Case,
    log: Vec<u32>,
    calls: HashMap<u32, u32>,
    out_taken: HashMap<u32, u32>,
}

impl StripServices for ScriptedServices<'_> {
    fn call(&mut self, memory: &mut dyn GuestMemory, service: Service, args: &[u32]) -> u32 {
        let (id, count, derefs, writes, class_lookup) = spec(service);
        assert_eq!(args.len(), count, "{service:?} argument count");
        self.log.push(id);
        self.log.extend_from_slice(args);
        for &(arg, dwords) in derefs {
            for k in 0..dwords {
                self.log.push(memory.read_u32(args[arg] + 4 * k));
            }
        }
        for &(arg, offset, dwords) in writes {
            for k in 0..dwords {
                let taken = self.out_taken.entry(id).or_insert(0);
                let word = self.case.input(OUTQ + 0x400 * id + 4 * *taken);
                *taken += 1;
                memory.write_u32(args[arg] + offset + 4 * k, word);
            }
        }
        if class_lookup {
            return memory.read_u32(args[0] + 0x300);
        }
        let n = self.calls.entry(id).or_insert(0);
        let value = self.case.input(RETQ + 0x100 * id + 4 * *n);
        *n += 1;
        value
    }
}

#[test]
fn strip_spawner_matches_original() {
    let cases = parse(include_str!("data/t1846/strip_spawner_1f48a0.golden"));
    assert!(cases.len() >= 60);
    let mut seen = HashMap::<u32, usize>::new();
    for (n, case) in cases.iter().enumerate() {
        let mut memory = Memory::from_case(case);
        let mut services = ScriptedServices {
            case,
            log: Vec::new(),
            calls: HashMap::new(),
            out_taken: HashMap::new(),
        };
        let mut rng = Lcg64 {
            state: case.input(0x4E7950) as u64 | (case.input(0x4E7954) as u64) << 32,
        };
        strip_spawner(&mut memory, &mut services, &mut rng, ENTRY_ESP).expect("no fault");
        // log: the pointer cell advances by 4 bytes per word; entries start at LOG + 0x10
        let expected_end = LOG + 0x10 + 4 * services.log.len() as u32;
        assert_eq!(case.output(LOG), expected_end, "case {n} log length");
        for (i, word) in services.log.iter().enumerate() {
            assert_eq!(
                *word,
                case.output(LOG + 0x10 + 4 * i as u32),
                "case {n} log word {i}"
            );
        }
        // service usage statistics (first word of each entry is hard to find: count ids by replay)
        let mut index = 0;
        while index < services.log.len() {
            let id = services.log[index];
            *seen.entry(id).or_default() += 1;
            let (count, derefs): (usize, u32) = match id {
                1 | 2 | 9 | 11 | 13 | 14 | 16 => (1, 0),
                3 => (3, 0),
                4 => (2, 3),
                5 => (5, 4),
                6 => (2, 20),
                7 => (3, 8),
                8 => (4, 0),
                10 => (2, 0),
                12 => (10, 3),
                15 => (3, 0),
                _ => unreachable!(),
            };
            index += 1 + count + derefs as usize;
        }
        assert_eq!(index, services.log.len());
        for (&address, &word) in &case.outs {
            if (LOG..LOG + 0x1000).contains(&address) || address == 0x4E7950 || address == 0x4E7954
            {
                continue;
            }
            assert_eq!(
                memory.read_u32(address),
                word,
                "case {n} memory {address:#x}"
            );
        }
        assert_eq!(rng.state as u32, case.output(0x4E7950), "case {n} rng low");
        assert_eq!(
            (rng.state >> 32) as u32,
            case.output(0x4E7954),
            "case {n} rng high"
        );
    }
    // every service was exercised, the particle path many times
    for id in 1..=16 {
        assert!(
            seen.get(&id).copied().unwrap_or(0) > 1,
            "service {id} seen {:?}",
            seen.get(&id)
        );
    }
    assert!(seen[&7] > 40, "particles {}", seen[&7]);
}

#[test]
fn spawner_faults_on_empty_emitter_table() {
    struct Services;
    impl StripServices for Services {
        fn call(&mut self, memory: &mut dyn GuestMemory, service: Service, args: &[u32]) -> u32 {
            match service {
                Service::FindObjectA => 0x1000,
                Service::ClassOf => 0x2000,
                Service::QueryState => {
                    memory.write_u32(args[1], 0x210);
                    0
                }
                _ => 0,
            }
        }
    }
    let mut memory = Memory(HashMap::new());
    memory.write_u32(0x2000, 0xBF8);
    memory.write_u32(0x1004, 0x3000);
    memory.write_u32(0x3004, 0x4000);
    let mut rng = Lcg64::default();
    let result = strip_spawner(&mut memory, &mut Services, &mut rng, ENTRY_ESP);
    assert_eq!(result, Err(SpawnerFault::ZeroEmitterCount));
}
