//! T3402 differential: native resource table updates against the C2R translation of lifted 0x5B980.
//! Inputs and clip tokens are FABRICATED; this test does not load the TSFP_LIFTED_DIR oracle.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use tsfp_engine_anim::{
    binding::{BindingEntry, BindingWorld, LevelBindings},
    resource_catalog::AnimationResourceCatalog,
};
use tsfp_ports::abi::{ExtRegisters, Fpu, GuestCall, Xmm};
use tsfp_ports::c2r::engine_anim::game_anim_set_entries_load_raw_files_and_store_slot_table_pair;
use tsfp_ports::mem::GuestAccess;
use tsfp_ports::memory::GuestMemory;

const STACK: u32 = 0x8000;
const ENTRY: u32 = 0x20000;
const BASE: u32 = 0x10000;
const NAMES: u32 = 0x11000;
const SLOT_OF_ID: u32 = 0x7e03c0;
const SLOT_POLICY: u32 = 0x7de660;
const PATH_FORMAT: u32 = 0x478664;
const RAW_SUFFIX: u32 = 0x47866c;

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<BTreeMap<u32, u8>>>);

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        *self.0.borrow().get(&address).unwrap_or(&0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
    }
}

impl Memory {
    fn write_cstr(&mut self, address: u32, value: &str) {
        for (offset, byte) in value.bytes().chain([0]).enumerate() {
            self.write8(address + offset as u32, byte);
        }
    }
    fn read_cstr(&self, address: u32) -> String {
        let mut bytes = Vec::new();
        for offset in 0..256 {
            let byte = self.read8(address + offset);
            if byte == 0 {
                break;
            }
            bytes.push(byte);
        }
        String::from_utf8(bytes).unwrap()
    }
}

struct Loader {
    memory: Memory,
    slots: Vec<i16>,
    next_slot: usize,
    paths: Vec<String>,
    entry_address: u32,
    remap_entry_id: Option<u16>,
}

fn arg_base(memory: &Memory, esp: u32) -> u32 {
    [esp, esp + 4]
        .into_iter()
        .find(|at| (STACK - 0x200..STACK).contains(&memory.read32(*at)))
        .expect("callee path/destination pointer on the lifted stack")
}

impl GuestCall for Loader {
    fn call(&mut self, va: u32, regs: &mut ExtRegisters) {
        let args = arg_base(&self.memory, regs.esp);
        match va {
            0x3c85d4 => {
                let dest = self.memory.read32(args);
                assert_eq!(self.memory.read32(args + 4), PATH_FORMAT);
                let base = self.memory.read_cstr(self.memory.read32(args + 8));
                let name = self.memory.read_cstr(self.memory.read32(args + 12));
                let suffix = self.memory.read_cstr(self.memory.read32(args + 16));
                self.memory
                    .write_cstr(dest, &format!("{base}{name}{suffix}"));
            }
            0x57f30 => {
                let path = self.memory.read_cstr(self.memory.read32(args));
                let id = self.memory.read16(args + 4);
                let slot = self.slots[self.next_slot];
                self.next_slot += 1;
                self.paths.push(path);
                self.memory
                    .write16(SLOT_OF_ID + 2 * u32::from(id), slot as u16);
                if let Some(remapped_id) = self.remap_entry_id.take() {
                    self.memory.write16(self.entry_address + 0x10, remapped_id);
                }
            }
            other => panic!("unexpected lifted callee {other:#x}"),
        }
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
            control: 0x27f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

struct Row {
    name: String,
    id: u16,
    policy: [u32; 2],
}

fn seed(memory: &mut Memory, rows: &[Row]) {
    memory.write_cstr(BASE, "anim/data/ts3/");
    // The lifted body passes the original `.raw` literal at 0x47866c to its
    // path builder. Seed the literal explicitly instead of relying on zeroed
    // guest memory for a game-owned constant.
    memory.write_cstr(RAW_SUFFIX, ".raw");
    for (index, row) in rows.iter().enumerate() {
        let name = NAMES + index as u32 * 0x100;
        memory.write_cstr(name, &row.name);
        let at = ENTRY + index as u32 * 0x18;
        memory.write32(at, name);
        memory.write32(at + 8, row.policy[0]);
        memory.write32(at + 12, row.policy[1]);
        memory.write16(at + 0x10, row.id);
    }
}

fn run_original(memory: &mut Memory, loader: &mut Loader, rows: &[Row]) {
    let mut regs = registers();
    memory.write32(regs.esp + 4, BASE);
    memory.write32(regs.esp + 8, ENTRY);
    memory.write32(regs.esp + 12, rows.len() as u32);
    game_anim_set_entries_load_raw_files_and_store_slot_table_pair(memory, loader, &mut regs);
}

#[test]
fn resource_id_slot_and_policy_updates_match_lifted_5b980() {
    let rows = [
        Row {
            name: "first".into(),
            id: 5,
            policy: [0x11, 0x12],
        },
        Row {
            name: "shared".into(),
            id: 6,
            policy: [0x21, 0x22],
        },
        Row {
            name: "remap".into(),
            id: 5,
            policy: [0x31, 0x32],
        },
        Row {
            name: "last".into(),
            id: 7,
            policy: [0x41, 0x42],
        },
    ];
    let assigned_slots = [2, 2, -1, 2];
    let mut memory = Memory::default();
    seed(&mut memory, &rows);
    let mut loader = Loader {
        memory: memory.clone(),
        slots: assigned_slots.to_vec(),
        next_slot: 0,
        paths: Vec::new(),
        entry_address: ENTRY,
        remap_entry_id: None,
    };
    run_original(&mut memory, &mut loader, &rows);

    let mut catalog = AnimationResourceCatalog::default();
    for (row, slot) in rows.iter().zip(assigned_slots) {
        let path = format!("anim/data/ts3/{}.raw", row.name);
        catalog
            .publish(row.id, slot, row.policy, Some(path))
            .unwrap();
    }

    assert_eq!(
        loader.paths,
        rows.iter()
            .map(|row| format!("anim/data/ts3/{}.raw", row.name))
            .collect::<Vec<_>>()
    );
    for id in [5, 6, 7] {
        let native = catalog.by_id(id).unwrap();
        let original_slot = memory.read16(SLOT_OF_ID + 2 * u32::from(id)) as i16;
        assert_eq!(native.handle.legacy_slot(), original_slot, "id {id}");
        let original_policy = [
            memory.read32(SLOT_POLICY.wrapping_add((i32::from(original_slot) * 8) as u32)),
            memory.read32(SLOT_POLICY.wrapping_add((i32::from(original_slot) * 8 + 4) as u32)),
        ];
        assert_eq!(
            native.policy, original_policy,
            "id {id} slot {original_slot}"
        );
    }
    assert_eq!(
        catalog.by_id(5).unwrap().clip.map(String::as_str),
        Some("anim/data/ts3/remap.raw")
    );
    assert_eq!(catalog.by_id(6).unwrap().policy, [0x41, 0x42]);
}

#[test]
fn seeded_2000_fabricated_resource_sequences_match_lifted_5b980() {
    let mut random = 0x1668_5b98u32;
    for seed_value in 0..2000u32 {
        let mut next = || {
            random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            random
        };
        let rows: Vec<Row> = (0..4)
            .map(|index| Row {
                name: format!("seed_{seed_value}_row_{index}"),
                id: 5 + (next() % 4) as u16,
                policy: [next(), next()],
            })
            .collect();
        let slots: Vec<i16> = (0..rows.len()).map(|_| (next() % 9) as i16 - 3).collect();
        let mut memory = Memory::default();
        seed(&mut memory, &rows);
        let mut loader = Loader {
            memory: memory.clone(),
            slots: slots.clone(),
            next_slot: 0,
            paths: Vec::new(),
            entry_address: ENTRY,
            remap_entry_id: None,
        };
        run_original(&mut memory, &mut loader, &rows);

        let mut catalog = AnimationResourceCatalog::default();
        for (row, &slot) in rows.iter().zip(&slots) {
            let path = format!("anim/data/ts3/{}.raw", row.name);
            catalog
                .publish(row.id, slot, row.policy, Some(path))
                .unwrap();
        }
        let ids: BTreeSet<u16> = rows.iter().map(|row| row.id).collect();
        for id in ids {
            let native = catalog.by_id(id).unwrap();
            let original_slot = memory.read16(SLOT_OF_ID + 2 * u32::from(id)) as i16;
            assert_eq!(
                native.handle.legacy_slot(),
                original_slot,
                "seed {seed_value}, id {id}"
            );
            let policy_at = SLOT_POLICY.wrapping_add((i32::from(original_slot) * 8) as u32);
            assert_eq!(
                native.policy,
                [memory.read32(policy_at), memory.read32(policy_at + 4)],
                "seed {seed_value}, id {id}"
            );
        }
        for original_slot in slots.iter().copied().collect::<BTreeSet<_>>() {
            let policy_at = SLOT_POLICY.wrapping_add((i32::from(original_slot) * 8) as u32);
            let native = catalog.by_slot(original_slot).unwrap();
            assert_eq!(
                native.policy,
                [memory.read32(policy_at), memory.read32(policy_at + 4)],
                "seed {seed_value}, slot {original_slot}"
            );
        }
        assert_eq!(
            loader.paths,
            rows.iter()
                .map(|row| format!("anim/data/ts3/{}.raw", row.name))
                .collect::<Vec<_>>()
        );
    }
}

struct NativeBindingWorld {
    memory: Memory,
    requested_id: u16,
    load_slot: i16,
    remapped_id: u16,
}

impl BindingWorld for NativeBindingWorld {
    fn resident(&mut self, _pak: &str) -> bool {
        false
    }

    fn load_clip(&mut self, path: &str, requested_id: u16) -> i16 {
        assert_eq!(path, "anim/data/ts3/mutated.raw");
        self.memory.write16(
            SLOT_OF_ID + 2 * u32::from(requested_id),
            self.load_slot as u16,
        );
        self.memory.write16(ENTRY + 0x10, self.remapped_id);
        self.load_slot
    }

    fn binding_after_load(
        &mut self,
        entry_index: usize,
        requested_id: u16,
        _observed_slot: i16,
    ) -> (u16, i16) {
        assert_eq!(entry_index, 0);
        assert_eq!(requested_id, self.requested_id);
        let id = self.memory.read16(ENTRY + 0x10);
        let slot = self.memory.read16(SLOT_OF_ID + 2 * u32::from(id)) as i16;
        (id, slot)
    }
}

#[test]
fn seeded_post_load_id_rereads_match_lifted_5b980() {
    let mut random = 0x3402_5b98u32;
    for seed_value in 0..2000u32 {
        let mut next = || {
            random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            random
        };
        let requested_id = 5 + (next() % 6) as u16;
        let remapped_id = 20 + (next() % 10) as u16;
        let load_slot = (next() % 17) as i16 - 8;
        let remapped_slot = (next() % 17) as i16 - 8;
        let policy = [next(), next()];
        let rows = [Row {
            name: "mutated".into(),
            id: requested_id,
            policy,
        }];

        let mut original_memory = Memory::default();
        seed(&mut original_memory, &rows);
        original_memory.write16(
            SLOT_OF_ID + 2 * u32::from(remapped_id),
            remapped_slot as u16,
        );
        let mut loader = Loader {
            memory: original_memory.clone(),
            slots: vec![load_slot],
            next_slot: 0,
            paths: Vec::new(),
            entry_address: ENTRY,
            remap_entry_id: Some(remapped_id),
        };
        run_original(&mut original_memory, &mut loader, &rows);
        assert_eq!(loader.paths, ["anim/data/ts3/mutated.raw"]);

        let mut native_memory = Memory::default();
        seed(&mut native_memory, &rows);
        native_memory.write16(
            SLOT_OF_ID + 2 * u32::from(remapped_id),
            remapped_slot as u16,
        );
        let mut world = NativeBindingWorld {
            memory: native_memory,
            requested_id,
            load_slot,
            remapped_id,
        };
        let mut bindings = LevelBindings::default();
        let mut catalog = AnimationResourceCatalog::<()>::default();
        let handles = bindings
            .load_set_into_catalog(
                "anim/data/ts3/",
                &[BindingEntry {
                    id: requested_id,
                    policy,
                    name: "mutated",
                }],
                &mut world,
                &mut catalog,
            )
            .unwrap();

        let original_slot = original_memory.read16(SLOT_OF_ID + 2 * u32::from(remapped_id)) as i16;
        assert_eq!(original_memory.read16(ENTRY + 0x10), remapped_id);
        assert_eq!(original_slot, remapped_slot, "seed {seed_value}");
        assert_eq!(handles[0].legacy_slot(), original_slot, "seed {seed_value}");
        assert!(
            !bindings.ids.contains_key(&requested_id),
            "seed {seed_value}"
        );
        assert_eq!(
            bindings.ids[&remapped_id].slot, original_slot,
            "seed {seed_value}"
        );
        let catalog_policy = catalog.by_id(remapped_id).unwrap().policy;
        let original_policy_at = SLOT_POLICY.wrapping_add((i32::from(original_slot) * 8) as u32);
        assert_eq!(
            catalog_policy,
            [
                original_memory.read32(original_policy_at),
                original_memory.read32(original_policy_at + 4),
            ],
            "seed {seed_value}"
        );
        assert_eq!(catalog_policy, policy, "seed {seed_value}");
    }
}

#[test]
fn nonpositive_count_leaves_lifted_tables_unchanged() {
    let mut memory = Memory::default();
    memory.write16(SLOT_OF_ID + 2 * 9, 0x1234);
    memory.write32(STACK + 4, BASE);
    memory.write32(STACK + 8, ENTRY);
    memory.write32(STACK + 12, 0);
    let mut loader = Loader {
        memory: memory.clone(),
        slots: vec![],
        next_slot: 0,
        paths: vec![],
        entry_address: ENTRY,
        remap_entry_id: None,
    };
    let before_id = memory.read16(SLOT_OF_ID + 2 * 9);
    let before_policy = memory.read32(SLOT_POLICY + 8 * 2);
    let mut regs = registers();
    game_anim_set_entries_load_raw_files_and_store_slot_table_pair(
        &mut memory,
        &mut loader,
        &mut regs,
    );

    assert!(loader.paths.is_empty());
    assert_eq!(memory.read16(SLOT_OF_ID + 2 * 9), before_id);
    assert_eq!(memory.read32(SLOT_POLICY + 8 * 2), before_policy);
}
