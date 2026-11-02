// SPDX-License-Identifier: GPL-3.0-or-later
//! T3402 Tier 1: compare the readable post-load binding model with lifted 0x5B980.
//! Table rows, callback slot assignments and the post-load ID rewrite are FABRICATED.
#![cfg(w4_lifted)]

use std::ptr;

use tsfp_engine_anim::binding::{BindingEntry, BindingWorld, LevelBindings};

unsafe extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
    fn shim_set_base(base: *mut u8);
    fn shim_script(words: *const u32, entries: u32);
    fn shim_script_writes(words: *const u32, entries: u32);
    fn shim_trace_len() -> u32;
    fn shim_trace_entry(index: u32) -> *const u32;
    fn w4_run(va: u32, regs: *mut u32) -> i32;
}

const ARENA_LEN: usize = (1usize << 32) + 0x1_0000;
const STACK: u32 = 0x0030_0000;
const BASE: u32 = 0x0001_0000;
const ENTRY: u32 = 0x0002_0000;
const NAMES: u32 = 0x0003_0000;
const SLOT_OF_ID: u32 = 0x007e_03c0;
const SLOT_POLICY: u32 = 0x007d_e660;

fn arena() -> *mut u8 {
    let base = unsafe { mmap(ptr::null_mut(), ARENA_LEN, 3, 0x2 | 0x20 | 0x4000, -1, 0) };
    assert_ne!(base as isize, -1, "lifted guest arena mmap failed");
    unsafe { shim_set_base(base) };
    base
}

fn put16(base: *mut u8, address: u32, value: u16) {
    unsafe {
        ptr::copy_nonoverlapping(value.to_le_bytes().as_ptr(), base.add(address as usize), 2)
    };
}

fn get16(base: *mut u8, address: u32) -> u16 {
    let mut bytes = [0; 2];
    unsafe { ptr::copy_nonoverlapping(base.add(address as usize), bytes.as_mut_ptr(), 2) };
    u16::from_le_bytes(bytes)
}

fn put32(base: *mut u8, address: u32, value: u32) {
    unsafe {
        ptr::copy_nonoverlapping(value.to_le_bytes().as_ptr(), base.add(address as usize), 4)
    };
}

fn get32(base: *mut u8, address: u32) -> u32 {
    let mut bytes = [0; 4];
    unsafe { ptr::copy_nonoverlapping(base.add(address as usize), bytes.as_mut_ptr(), 4) };
    u32::from_le_bytes(bytes)
}

fn put_cstr(base: *mut u8, address: u32, value: &str) {
    let bytes = value.bytes().chain([0]).collect::<Vec<_>>();
    unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), base.add(address as usize), bytes.len()) };
}

#[derive(Default)]
struct World {
    slots: Vec<i16>,
    post_ids: Vec<u16>,
    next: usize,
}

impl BindingWorld for World {
    fn resident(&mut self, _pak: &str) -> bool {
        false
    }

    fn load_clip(&mut self, _path: &str, _id: u16) -> i16 {
        let slot = self.slots[self.next];
        self.next += 1;
        slot
    }

    fn binding_after_load(
        &mut self,
        entry_index: usize,
        _requested_id: u16,
        observed_slot: i16,
    ) -> (u16, i16) {
        (self.post_ids[entry_index], observed_slot)
    }
}

#[test]
fn post_load_id_slot_and_policy_match_lifted_5b980() {
    assert_eq!(std::env::var("TSFP_REF_LIFTED").as_deref(), Ok("1"));
    let rows = [
        BindingEntry {
            id: 4,
            policy: [0x11, 0x12],
            name: "first",
        },
        BindingEntry {
            id: 6,
            policy: [0x21, 0x22],
            name: "remapped",
        },
        BindingEntry {
            id: 4,
            policy: [0x31, 0x32],
            name: "duplicate",
        },
    ];
    let assigned_slots = vec![2i16, -1, 2];
    let final_ids = vec![4u16, 8, 4];
    let base = arena();

    unsafe {
        ptr::write_bytes(base.add(0x0000_f000), 0, 0x1000);
        ptr::write_bytes(base.add(BASE as usize), 0, 0x1000);
        ptr::write_bytes(base.add(ENTRY as usize), 0, 0x1000);
        ptr::write_bytes(base.add(NAMES as usize), 0, 0x1000);
        ptr::write_bytes(base.add(SLOT_POLICY as usize - 8), 0, 0x5000);
        ptr::write_bytes(base.add(STACK as usize - 0x1000), 0, 0x2000);
    }
    put_cstr(base, BASE, "anim/data/t3402/");
    for (index, row) in rows.iter().enumerate() {
        let name = NAMES + index as u32 * 0x100;
        let entry = ENTRY + index as u32 * 0x18;
        put_cstr(base, name, row.name);
        put32(base, entry, name);
        put32(base, entry + 8, row.policy[0]);
        put32(base, entry + 12, row.policy[1]);
        put16(base, entry + 0x10, row.id);
    }
    // The callback rewrites row 1's ID to 8. The measured routine rereads that
    // ID after the call and sign-extends its slot-table value (-1).
    put16(base, SLOT_OF_ID + 2 * 4, 2);
    put16(base, SLOT_OF_ID + 2 * 8, (-1i16) as u16);
    let mut callback_writes = [3, ENTRY + 0x18 + 0x10, 8];
    let callback_script = [0u32; 18];
    unsafe { shim_script(callback_script.as_ptr(), 6) };
    unsafe { shim_script_writes(callback_writes.as_mut_ptr(), 1) };

    put32(base, STACK + 4, BASE);
    put32(base, STACK + 8, ENTRY);
    put32(base, STACK + 12, rows.len() as u32);
    let mut regs = [0u32; 8];
    regs[7] = STACK;
    assert_eq!(unsafe { w4_run(0x5b980, regs.as_mut_ptr()) }, 1);

    let callbacks = (0..unsafe { shim_trace_len() })
        .filter_map(|index| {
            let row = unsafe { shim_trace_entry(index) };
            (unsafe { *row } == 0x57f30).then(|| unsafe { *row.add(4) as u16 })
        })
        .collect::<Vec<_>>();
    assert_eq!(callbacks, [4, 6, 4]);
    assert_eq!(get16(base, ENTRY + 0x18 + 0x10), 8);

    let mut rust_world = World {
        slots: assigned_slots,
        post_ids: final_ids,
        next: 0,
    };
    let mut bindings = LevelBindings::default();
    bindings.load_set("anim/data/t3402/", &rows, &mut rust_world);
    assert_eq!(rust_world.next, rows.len());

    for id in [4u16, 8] {
        let native_slot = bindings.ids.get(&id).unwrap().slot;
        assert_eq!(
            get16(base, SLOT_OF_ID + 2 * u32::from(id)) as i16,
            native_slot,
            "id {id}"
        );
    }
    for slot in [-1i16, 2] {
        let words = bindings.policy.get(&slot).unwrap();
        let address = (SLOT_POLICY as i64 + i64::from(slot) * 8) as u32;
        assert_eq!(
            [get32(base, address), get32(base, address + 4)],
            *words,
            "slot {slot}"
        );
    }

    // Bulk Tier 1 coverage: keep table data and slots FABRICATED, but compare
    // every resulting ID slot and per-slot policy word against the direct
    // lifted body over varied sizes, aliases-by-slot, negative slots, and
    // policy overwrites. The single case above retains callback ID rewriting.
    for seed in 0..2_000u32 {
        let mut state = seed.wrapping_add(0x5b98_0001);
        let mut next = || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            state
        };
        let count = (next() % 6 + 1) as usize;
        let id_base = 0x100u16 + (seed as u16 % 0x3000) * 8;
        let mut rows = Vec::with_capacity(count);
        let mut slots = Vec::with_capacity(count);
        for index in 0..count {
            let slot = (next() % 9) as i16 - 4;
            slots.push(slot);
            rows.push(BindingEntry {
                id: id_base + index as u16,
                policy: [next(), next()],
                name: ["anim0", "anim1", "anim2", "anim3", "anim4", "anim5"][index],
            });
        }
        let post_ids: Vec<_> = rows.iter().map(|row| row.id).collect();

        unsafe {
            ptr::write_bytes(base.add(0x0000_f000), 0, 0x1000);
            ptr::write_bytes(base.add(BASE as usize), 0, 0x1000);
            ptr::write_bytes(base.add(ENTRY as usize), 0, 0x1000);
            ptr::write_bytes(base.add(NAMES as usize), 0, 0x1000);
            ptr::write_bytes(base.add(SLOT_POLICY as usize - 0x100), 0, 0x500);
            ptr::write_bytes(base.add(STACK as usize - 0x1000), 0, 0x2000);
        }
        put_cstr(base, BASE, "anim/data/t3402/");
        for (index, row) in rows.iter().enumerate() {
            let name = NAMES + index as u32 * 0x100;
            let entry = ENTRY + index as u32 * 0x18;
            put_cstr(base, name, row.name);
            put32(base, entry, name);
            put32(base, entry + 8, row.policy[0]);
            put32(base, entry + 12, row.policy[1]);
            put16(base, entry + 0x10, row.id);
            put16(
                base,
                SLOT_OF_ID + 2 * u32::from(row.id),
                slots[index] as u16,
            );
        }
        let callback_script = [0u32; 18];
        unsafe {
            shim_script(callback_script.as_ptr(), 6);
            shim_script_writes(ptr::null(), 0);
        }
        put32(base, STACK + 4, BASE);
        put32(base, STACK + 8, ENTRY);
        put32(base, STACK + 12, rows.len() as u32);
        let mut regs = [0u32; 8];
        regs[7] = STACK;
        assert_eq!(
            unsafe { w4_run(0x5b980, regs.as_mut_ptr()) },
            1,
            "seed {seed}"
        );

        let mut rust_world = World {
            slots: slots.clone(),
            post_ids: post_ids.clone(),
            next: 0,
        };
        let mut bindings = LevelBindings::default();
        bindings.load_set("anim/data/t3402/", &rows, &mut rust_world);
        for id in &post_ids {
            let native_slot = bindings.ids.get(id).unwrap().slot;
            assert_eq!(
                get16(base, SLOT_OF_ID + 2 * u32::from(*id)) as i16,
                native_slot,
                "seed {seed}, id {id}"
            );
        }
        for slot in -4i16..=4 {
            let address = (SLOT_POLICY as i64 + i64::from(slot) * 8) as u32;
            let observed = [get32(base, address), get32(base, address + 4)];
            assert_eq!(
                observed,
                bindings.policy.get(&slot).copied().unwrap_or([0, 0]),
                "seed {seed}, slot {slot}"
            );
        }
    }
}
