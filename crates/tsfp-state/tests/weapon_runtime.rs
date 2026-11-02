// SPDX-License-Identifier: GPL-3.0-or-later
// T1831: translated-root differentials, FABRICATED state and neighbour effects.
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    memory::GuestMemory,
};
use tsfp_state::{weapon_runtime::*, weapon_select::SelectionPreset};

#[derive(Clone, Default)]
struct Image(Rc<RefCell<BTreeMap<u32, u8>>>);
impl GuestMemory for Image {
    fn read8(&self, a: u32) -> u8 {
        *self.0.borrow().get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.borrow_mut().insert(a, v);
    }
}
const STACK: u32 = 0x240000;
const SLOT: u32 = 0x200000;
const OWNER: u32 = 0x210000;
const RECORDS: u32 = 0x220000;
const LIST: u32 = 0x230000;
fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: OWNER,
        ecx: OWNER,
        edx: 0,
        esp: STACK,
        ebx: 17,
        ebp: 19,
        esi: SLOT,
        edi: 23,
        eflags: 0,
        fpu: Fpu {
            stack: [0.; 8],
            top: 0,
            control: 0x300,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}
fn word(m: &mut Image, a: u32, v: u32) {
    m.write32(a, v);
}
fn read(m: &Image, a: u32) -> u32 {
    m.read32(a)
}
fn random(s: &mut u32) -> u32 {
    *s = s.wrapping_mul(1664525).wrapping_add(1013904223);
    *s
}
fn object(v: u32) -> Option<ObjectId> {
    (v != 0).then_some(ObjectId(v))
}
fn slot(m: &Image, base: u32) -> SlotState {
    SlotState {
        record: read(m, base + 0x30) as i32,
        children: [object(read(m, base + 0x54)), object(read(m, base + 0x58))],
        continuity: SlotContinuity {
            word_3c: read(m, base + 0x3c),
            word_3cc: read(m, base + 0x3cc),
        },
    }
}
#[derive(Debug, PartialEq, Eq)]
enum Event {
    Remove(u32, u32, u32),
    Destroy(u32),
    Callback(u32, u32, u32, u32),
    Register(u32, u32, u32),
    Lookup,
    Preload(u32),
}
struct Effects {
    m: Image,
    base: u32,
    change: bool,
    events: Vec<Event>,
}
impl Effects {
    fn mutate(&mut self) {
        if self.change {
            word(&mut self.m, self.base + 0x58, 0x123400);
            word(&mut self.m, self.base + 0x3c, 0x87654321);
            word(&mut self.m, self.base + 0x3cc, 0x12345678);
        }
    }
}
impl GuestCall for Effects {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        let arg = |n: u32| read(&self.m, r.esp + n * 4);
        self.events.push(match va {
            0xa5ce0 => Event::Remove(arg(0), arg(1), arg(2)),
            0xa9070 => Event::Destroy(arg(0)),
            0x123000 => Event::Callback(arg(0), arg(1), arg(2), arg(3)),
            _ => panic!("unexpected call {va:x}"),
        });
        self.mutate();
    }
}
impl SlotEffects for Effects {
    fn child_cleared(&mut self, _: &SlotState, hand: usize) {
        word(&mut self.m, self.base + 0x54 + hand as u32 * 4, 0);
    }
    fn remove_membership(&mut self, s: &mut SlotState, id: ObjectId) {
        self.events.push(Event::Remove(id.0, 8, 0));
        self.mutate();
        *s = slot(&self.m, self.base);
    }
    fn destroy(&mut self, s: &mut SlotState, id: ObjectId) {
        self.events.push(Event::Destroy(id.0));
        self.mutate();
        *s = slot(&self.m, self.base);
    }
    fn record_callback(&mut self, s: &mut SlotState, _: i32, owner: ObjectId) {
        self.events.push(Event::Callback(6, self.base, owner.0, 0));
        self.mutate();
        *s = slot(&self.m, self.base);
    }
}
fn fixture(seed: &mut u32, base: u32) -> Image {
    let mut m = Image::default();
    for i in 0..0xf6 {
        word(&mut m, base + i * 4, random(seed));
    }
    word(&mut m, base + 0x30, (random(seed) % 5).wrapping_sub(1));
    for offset in [0x54, 0x58] {
        let v = if random(seed) & 0x100 == 0 {
            0
        } else {
            random(seed)
        };
        word(&mut m, base + offset, v);
    }
    m
}
fn cloned(m: &Image) -> Image {
    Image(Rc::new(RefCell::new(m.0.borrow().clone())))
}

#[test]
fn reset_matches_translation_4096_states() {
    let mut seed = 1831;
    for case in 0..4096 {
        let mut m = fixture(&mut seed, SLOT);
        let owner_type = (random(&mut seed) % 5).wrapping_sub(1) as i32;
        let membership = (random(&mut seed) % 3).wrapping_sub(1) as i32;
        word(&mut m, OWNER + 4, owner_type as u32);
        if owner_type >= 0 {
            word(&mut m, 0x791d50 + owner_type as u32 * 4, membership as u32);
        }
        let mut reference = Effects {
            m: m.clone(),
            base: SLOT,
            change: case % 3 == 0,
            events: vec![],
        };
        let typed_image = cloned(&m);
        let mut typed = Effects {
            m: typed_image.clone(),
            base: SLOT,
            change: reference.change,
            events: vec![],
        };
        let mut state = slot(&typed_image, SLOT);
        let result = reset_slot(&mut state, owner_type >= 0 && membership >= 0, &mut typed);
        let mut r = regs();
        tsfp_ports::c2r::t2049_nx_006::game_slot_state_struct_destroy_children_and_zero_preserving_0x3cc_0x3c(&mut m, &mut reference, &mut r);
        assert_eq!(typed.events, reference.events, "case {case}");
        assert_eq!(state, slot(&m, SLOT), "case {case}");
        // Adapter contract: every byte cleared except both sentinels and continuity.
        let mut expected = [0u8; 0x3d8];
        for (offset, value) in [
            (0x30, u32::MAX),
            (0xc8, u32::MAX),
            (0x3c, result.continuity.word_3c),
            (0x3cc, result.continuity.word_3cc),
        ] {
            expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (i, v) in expected.iter().enumerate() {
            assert_eq!(m.read8(SLOT + i as u32), *v, "case {case} byte {i}");
        }
    }
}

#[test]
fn child_release_matches_translation_4096_states() {
    let mut seed = 1832;
    for case in 0..4096 {
        let base = if case % 8 == 0 { 0 } else { SLOT };
        let mut m = fixture(&mut seed, base);
        let owner_record = (random(&mut seed) % 5).wrapping_sub(1) as i32;
        word(&mut m, OWNER + 0x30, owner_record as u32);
        word(&mut m, 0x4f9bac, RECORDS);
        let record = if base == 0 {
            owner_record
        } else {
            slot(&m, base).record
        };
        let callback = case % 3 != 0;
        if record >= 0 {
            word(
                &mut m,
                RECORDS + record as u32 * 0x29c + 0x294,
                if callback { 0x123000 } else { 0 },
            );
        }
        let original = cloned(&m);
        let mut reference = Effects {
            m: m.clone(),
            base,
            change: case % 4 == 0,
            events: vec![],
        };
        let typed_image = cloned(&m);
        let mut typed = Effects {
            m: typed_image.clone(),
            base,
            change: reference.change,
            events: vec![],
        };
        let mut state = slot(&typed_image, base);
        release_children(&mut state, record, callback, ObjectId(OWNER), &mut typed);
        let mut r = regs();
        r.esi = base;
        tsfp_ports::c2r::t2125_001::game_slot_state_call_record_callback_6_then_destroy_child_objects_0x54_0x58(&mut m, &mut reference, &mut r);
        assert_eq!(typed.events, reference.events, "case {case}");
        assert_eq!(state, slot(&m, base), "case {case}");
        for i in 0..0x3d8 {
            if (0x54..0x5c).contains(&i)
                || (reference.change
                    && (!reference.events.is_empty())
                    && ((0x3c..0x40).contains(&i) || (0x3cc..0x3d0).contains(&i)))
            {
                continue;
            }
            assert_eq!(
                m.read8(base + i),
                original.read8(base + i),
                "case {case} unaffected byte {i}"
            );
        }
    }
}

#[test]
fn availability_matches_translation_4096_states() {
    let mut m = Image::default();
    let mut seed = 1833;
    for case in 0..4096 {
        let s = PoolAvailability {
            free: random(&mut seed) as i16,
            active: random(&mut seed) as i16,
            total: random(&mut seed) as i16,
            capacity: random(&mut seed) as u16,
        };
        for (a, v) in [
            (0x74c45c, s.free as u16),
            (0x74c45e, s.active as u16),
            (0x74c460, s.total as u16),
            (0x74c49c, s.capacity),
        ] {
            m.write8(a, v as u8);
            m.write8(a + 1, (v >> 8) as u8);
        }
        let mode = (case % 5) as u32 | (random(&mut seed) & 0xffffff00);
        word(&mut m, STACK + 4, mode);
        let mut r = regs();
        tsfp_ports::c2r::t2049_nx_006::game_entry_pool_74c458_mode_has_available_count_check(
            &mut m, &mut r,
        );
        assert_eq!(s.available(mode as u8), r.eax != 0, "case {case}");
    }
}
struct Loadout {
    m: Image,
    events: Vec<Event>,
    second: bool,
    change_mode: bool,
}
impl Loadout {
    fn lookup_effect(&mut self) {
        if self.change_mode {
            word(&mut self.m, 0x7de458, 0x400);
            word(&mut self.m, 0x79094c, 0x63);
        }
    }
}
impl GuestCall for Loadout {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        assert_eq!(va, 0x6bd30);
        self.events.push(Event::Lookup);
        self.lookup_effect();
        r.eax = if self.second && self.events.len() == 2 {
            LIST + 0x100
        } else {
            LIST
        };
    }
}
impl LoadoutSource for Loadout {
    fn mode_flags(&self) -> u32 {
        read(&self.m, 0x7de458)
    }
    fn level(&self) -> u32 {
        read(&self.m, 0x79094c)
    }
    fn count(&mut self) -> i32 {
        self.events.push(Event::Lookup);
        self.lookup_effect();
        read(&self.m, LIST + 4) as i32
    }
    fn entry(&mut self, index: i32) -> u32 {
        self.events.push(Event::Lookup);
        let list = if self.second { LIST + 0x100 } else { LIST };
        read(
            &self.m,
            list.wrapping_add((index as u32).wrapping_mul(4))
                .wrapping_add(8),
        )
    }
}
#[test]
fn loadout_matches_translation_4096_states() {
    let mut seed = 1834;
    for case in 0..4096 {
        let mut m = Image::default();
        let kind = [2, 0x40, 0x800, 0, 3, 0xffffffff][case % 6];
        let index = (random(&mut seed) % 9) as i32 - 2;
        let count = (random(&mut seed) % 8) as i32 - 2;
        let flags = if case % 3 == 0 { 0x400 } else { 0 };
        let level = if case % 4 == 0 { 0x64 } else { 0x63 };
        word(&mut m, LIST + 4, count as u32);
        for list in [LIST, LIST + 0x100] {
            let entry = if case % 5 == 0 { 0 } else { random(&mut seed) };
            word(
                &mut m,
                list.wrapping_add((index as u32).wrapping_mul(4))
                    .wrapping_add(8),
                entry,
            );
        }
        word(&mut m, 0x7de458, flags);
        word(&mut m, 0x79094c, level);
        word(&mut m, STACK + 4, kind);
        word(&mut m, STACK + 8, index as u32);
        let mut reference = Loadout {
            m: m.clone(),
            events: vec![],
            second: case % 2 == 0,
            change_mode: case % 7 == 0,
        };
        let mut typed = Loadout {
            m: cloned(&m),
            events: vec![],
            second: reference.second,
            change_mode: reference.change_mode,
        };
        let expected = loadout_entry(kind, index, &mut typed);
        let mut r = regs();
        tsfp_ports::c2r::t2049_nx_002::game_current_mode_loadout_entry_by_kind_2_0x40_0x800_index_or_default(&mut m,&mut reference,&mut r);
        assert_eq!(expected, r.eax, "case {case}");
        assert_eq!(typed.events, reference.events, "case {case}");
    }
}
struct Registry {
    m: Image,
    events: Vec<Event>,
}
impl CallbackRegistry for Registry {
    fn register(&mut self, mask: u32, slot: u32, callback: WeaponCallback) {
        self.events.push(Event::Register(
            mask,
            slot,
            match callback {
                WeaponCallback::Primary => 0xac940,
                WeaponCallback::Secondary => 0xa92f0,
            },
        ));
    }
}
impl GuestCall for Registry {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        assert_eq!(va, 0x9e2d0);
        self.events.push(Event::Register(
            read(&self.m, r.esp),
            read(&self.m, r.esp + 4),
            read(&self.m, r.esp + 8),
        ));
    }
}
#[test]
fn registration_matches_translation_4096_states() {
    let mut seed = 1835;
    for _ in 0..4096 {
        let mut m = Image::default();
        let mut r = regs();
        r.eax = random(&mut seed);
        r.ecx = random(&mut seed);
        let mut reference = Registry {
            m: m.clone(),
            events: vec![],
        };
        let mut typed = Registry {
            m: m.clone(),
            events: vec![],
        };
        register_callbacks(&mut typed);
        tsfp_ports::c2r::t2049_nx_005::game_register_callbacks_mask_0x80(
            &mut m,
            &mut reference,
            &mut r,
        );
        assert_eq!(typed.events, reference.events);
    }
}
struct Resources {
    m: Image,
    events: Vec<Event>,
    shrink: bool,
}
impl Resources {
    fn perform(&mut self, resource: u32) {
        self.events.push(Event::Preload(resource));
        if self.shrink {
            let template = (resource >> 8) as usize;
            word(&mut self.m, 0x53fa58 + template as u32 * 0x4dc, 0);
        }
    }
}
impl GuestCall for Resources {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        assert_eq!(va, 0xa5e40);
        self.perform(read(&self.m, r.esp));
    }
}
impl CompositeResources for Resources {
    fn part_count(&mut self, t: usize) -> i32 {
        read(&self.m, 0x53fa58 + t as u32 * 0x4dc) as i32
    }
    fn resource(&mut self, t: usize, p: usize) -> u32 {
        let table = read(&self.m, 0x53fa58 + t as u32 * 0x4dc + 0xc);
        read(&self.m, table + p as u32 * 0x1c + 4)
    }
    fn preload(&mut self, r: u32) {
        self.perform(r);
    }
}
#[test]
fn preload_matches_translation_4096_states() {
    let mut seed = 1836;
    for case in 0..4096 {
        let mut m = Image::default();
        let flags = if case % 5 == 0 { 0x20000000 } else { 0 };
        word(&mut m, 0x7de458, flags);
        for t in 0..7 {
            let table = LIST + t * 0x100;
            word(
                &mut m,
                0x53fa58 + t * 0x4dc,
                (random(&mut seed) % 5).wrapping_sub(1),
            );
            word(&mut m, 0x53fa58 + t * 0x4dc + 0xc, table);
            for p in 0..4 {
                word(&mut m, table + p * 0x1c + 4, (t << 8) | p);
            }
        }
        let mut reference = Resources {
            m: m.clone(),
            events: vec![],
            shrink: case % 3 == 0,
        };
        let mut typed = Resources {
            m: cloned(&m),
            events: vec![],
            shrink: reference.shrink,
        };
        preload_resources(flags, &mut typed);
        let mut r = regs();
        tsfp_ports::c2r::t2049_nx_008::game_composite_templates_preload_object_resources_for_all_entries(&mut m,&mut reference,&mut r);
        assert_eq!(typed.events, reference.events, "case {case}");
        for t in 0..7 {
            assert_eq!(
                read(&typed.m, 0x53fa58 + t * 0x4dc),
                read(&m, 0x53fa58 + t * 0x4dc)
            );
        }
    }
}
#[test]
fn preset_is_independent_and_defaults_to_reference() {
    assert_eq!(
        dispatch(
            SelectionPreset::default(),
            || 1,
            || panic!("readable must be off")
        ),
        1
    );
    assert_eq!(
        dispatch(
            SelectionPreset::ReadableOriginal,
            || panic!("reference must be off"),
            || 2
        ),
        2
    );
}
