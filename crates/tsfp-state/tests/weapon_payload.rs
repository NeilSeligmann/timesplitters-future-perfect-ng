// SPDX-License-Identifier: GPL-3.0-or-later
// T2386: typed payload pose through the lifted transform reader and shot reserve edge.
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    c2r::{engine_object, gameplay_inventory, t1937_math},
    memory::GuestMemory,
    object_transform::{RotationMode, TransformState, ViewTransform, WorldTransform},
};
use tsfp_state::{
    inventory::{AMMO_TYPES, ENTRY_COUNT, Inventory},
    weapon_hands::WeaponRecordId,
    weapon_payload::{
        PayloadRotationMode, ShotAdmission, ShotAmmoSource, ShotAmmoType, WeaponPayloadPose,
        admit_weapon_shot,
    },
};

const EXT: u32 = 0x200000;
const ACTOR: u32 = 0x210000;
const STACK: u32 = 0x300000;
const RECORDS: u32 = 0x400000;
const ENTRY_RECORDS: u32 = 0x4fa138;
const ENTRY_STRIDE: u32 = 0x3c;
const RECORD_STRIDE: u32 = 0x29c;
const OBJECT: u32 = 0x5000;
const OUTPUT: u32 = 0x6000;

#[derive(Clone, Default)]
struct Image(Rc<RefCell<BTreeMap<u32, u8>>>);
impl GuestMemory for Image {
    fn read8(&self, address: u32) -> u8 {
        *self.0.borrow().get(&address).unwrap_or(&0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
    }
}
impl tsfp_state::GuestMemory for Image {
    fn read8(&self, address: u32) -> u8 {
        GuestMemory::read8(self, address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        GuestMemory::write8(self, address, value);
    }
}
impl Image {
    fn identity(&mut self, at: u32) {
        let matrix = WorldTransform::IDENTITY.matrix;
        for (i, word) in matrix.into_iter().enumerate() {
            GuestMemory::write32(self, at + i as u32 * 4, word.to_bits());
        }
    }
    fn half(&mut self, at: u32, value: u16) {
        GuestMemory::write8(self, at, value as u8);
        GuestMemory::write8(self, at + 1, (value >> 8) as u8);
    }
}

fn regs() -> ExtRegisters {
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
            stack: [0.; 8],
            top: 0,
            control: 0x37f,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

struct Math(Image);
impl GuestCall for Math {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        // The lifted transform calls its real trigonometry helpers through this bridge.
        let mut m = self.0.clone();
        r.esp = r.esp.wrapping_sub(4);
        GuestMemory::write32(&mut m, r.esp, 0);
        if va == 0x1518f0 {
            engine_object::game_object_set_vec3_0x30(&mut m, r);
        } else {
            assert!(
                t1937_math::run(va, &mut m, self, r),
                "unexpected transform helper {va:#x}"
            );
        }
        r.esp = r.esp.wrapping_add(4);
    }
}

#[test]
fn payload_pose_matches_lifted_transform_for_2048_seeded_finite_poses() {
    let mut seed = 0x2386_a14b_u32;
    for case in 0..2048u32 {
        let mut next = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            seed
        };
        let mut components = [0f32; 3];
        for value in &mut components {
            *value = (next() as i32 as f32 / i32::MAX as f32) * 180.;
        }
        if case % 4 == 0 {
            components[0] = if case & 4 == 0 { 0.0 } else { -0.0 };
            components[2] = 0.0;
        }
        let staged = next() as i32 as f32 / i32::MAX as f32 * 360.;
        let mut pose = WeaponPayloadPose::from_definition(components, staged);
        let template = [
            next() as i32 as f32 / i32::MAX as f32 * 90.,
            next() as i32 as f32 / i32::MAX as f32 * 360.,
            next() as i32 as f32 / i32::MAX as f32 * 90.,
        ];
        if case % 2 == 0 {
            assert!(pose.apply_c1950_template(2, template));
        } else {
            assert!(!pose.apply_c1950_template(0, template));
        }
        assert_eq!(pose.staged_yaw_degrees, staged, "staged yaw case {case}");

        let mut memory = Image::default();
        memory.identity(0x4e7900);
        memory.identity(OBJECT + 0x274);
        memory.write32(0x475c78, 1f32.to_bits());
        memory.write32(0x475d0c, (core::f32::consts::PI / 180.).to_bits());
        for (i, va) in [0xa165d, 0xa1672, 0xa16db, 0xa1744].into_iter().enumerate() {
            memory.write32(0xa17ac + i as u32 * 4, va);
        }
        memory.write32(STACK + 4, OUTPUT);
        memory.write32(STACK + 8, OBJECT);
        memory.write32(STACK + 12, 1);
        memory.write32(STACK + 16, 0);
        memory.write8(
            OBJECT + 0x32,
            match pose.rotation_mode {
                PayloadRotationMode::Yaw => 1,
                PayloadRotationMode::Euler => 2,
            },
        );
        memory.write32(OBJECT + 0x28, 0x1000);
        memory.write32(OBJECT + 0x2b4, 0);
        memory.write32(OBJECT + 0x78, 0);
        for (offset, value) in [0x64, 0x68, 0x6c].into_iter().zip([3.25f32, -7.5, 12.0]) {
            memory.write32(OBJECT + offset, value.to_bits());
        }
        for (offset, value) in [0xc0, 0x70, 0xc8]
            .into_iter()
            .zip(pose.pitch_yaw_roll_degrees)
        {
            memory.write32(OBJECT + offset, value.to_bits());
        }
        let mut oracle = regs();
        let mut math = Math(memory.clone());
        engine_object::game_object_build_matrix_for_player_by_state_field_0x2b4_and_mode_0x32(
            &mut memory,
            &mut math,
            &mut oracle,
        );

        let mode = match pose.rotation_mode {
            PayloadRotationMode::Yaw => RotationMode::Yaw,
            PayloadRotationMode::Euler => RotationMode::Euler,
        };
        let mut typed = TransformState {
            rotation_mode: mode,
            rotation_dirty: true,
            reflected_rotation: false,
            angles_degrees: pose.pitch_yaw_roll_degrees,
            parent_uses_euler: false,
            position: [3.25, -7.5, 12.0],
            cached_rotation: WorldTransform::IDENTITY,
        };
        let actual = typed.produce(
            ViewTransform {
                mirror_axis: None,
                mirror_plane: 0.,
                translate: true,
            },
            core::f32::consts::PI / 180.,
            |v| v,
        );
        for (i, word) in actual.matrix.into_iter().enumerate() {
            assert_eq!(
                word.to_bits(),
                GuestMemory::read32(&memory, OUTPUT + i as u32 * 4),
                "case {case}, matrix word {i}"
            );
        }
    }
}

struct ResourceLookup;
impl GuestCall for ResourceLookup {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        assert_eq!(va, 0xdc270, "unexpected shot helper {va:#x}");
        // Each generated record fixture uses a resource row with bit 0x400 set.
        r.eax = 0x400;
    }
}

fn random(seed: &mut u32) -> u32 {
    *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    *seed
}

#[test]
fn shot_ammo_boundary_matches_lifted_original_for_2048_seeded_states() {
    let mut seed = 0x2386_bc65_u32;
    for case in 0..2048u32 {
        let mut memory = Image::default();
        GuestMemory::write32(&mut memory, 0x4f9bac, RECORDS);
        GuestMemory::write32(&mut memory, ACTOR + 0x7c, EXT);
        GuestMemory::write32(&mut memory, ACTOR + 0x28, 0x100);
        GuestMemory::write32(&mut memory, STACK + 4, ACTOR);
        GuestMemory::write32(&mut memory, STACK + 8, 0);
        GuestMemory::write32(&mut memory, STACK + 12, 0);
        GuestMemory::write32(&mut memory, STACK + 16, 0);
        let game_type = if case % 7 == 0 { 10 } else { 0 };
        GuestMemory::write32(&mut memory, 0x7de455, game_type);

        let mut inventory = Inventory {
            base: EXT,
            flags: 0,
            current: 0,
            previous: 0,
            throwable: 0,
            owned: [0; ENTRY_COUNT],
            equipped: [0; ENTRY_COUNT],
            ammo: [0; AMMO_TYPES],
        };
        let mut sources = [ShotAmmoSource::default(); ENTRY_COUNT];
        for entry in 0..ENTRY_COUNT {
            let weapon = entry as u32 + 1;
            let record = RECORDS + weapon * RECORD_STRIDE;
            let ammo = random(&mut seed) as usize % AMMO_TYPES;
            let reserve = random(&mut seed) as u16;
            inventory.owned[entry] = if random(&mut seed) % 3 == 0 { 0 } else { 1 };
            inventory.ammo[ammo] = reserve;
            sources[entry] = ShotAmmoSource {
                weapon: (random(&mut seed) % 9 != 0).then_some(WeaponRecordId(weapon as u16)),
                resource_flags: 0x400,
                ammo_type: ShotAmmoType::new(ammo as u8),
            };
            let entry_weapon = sources[entry].weapon.map_or(u32::MAX, |id| id.0 as u32);
            GuestMemory::write32(
                &mut memory,
                ENTRY_RECORDS + entry as u32 * ENTRY_STRIDE,
                entry_weapon,
            );
            GuestMemory::write32(&mut memory, record + 0x14, ammo as u32);
            GuestMemory::write32(&mut memory, record + 0x6c, 0);
            GuestMemory::write8(
                &mut memory,
                EXT + 0x54c + entry as u32,
                inventory.owned[entry],
            );
        }
        for ammo in 0..AMMO_TYPES {
            Image::half(
                &mut memory,
                EXT + 0x61e + ammo as u32 * 2,
                inventory.ammo[ammo],
            );
        }
        let expected = admit_weapon_shot(&mut inventory, &sources, game_type as u8, 0x100, 0);
        assert!(matches!(
            expected,
            ShotAdmission::NoEligibleAmmo | ShotAdmission::Selected(_)
        ));
        let mut oracle = regs();
        gameplay_inventory::game_weapon_slot_consume_one_flagged_inventory_item_count_and_launch_via_matrix(
            &mut memory, &mut ResourceLookup, &mut oracle,
        );
        for ammo in 0..AMMO_TYPES {
            assert_eq!(
                inventory.ammo[ammo],
                GuestMemory::read16(&memory, EXT + 0x61e + ammo as u32 * 2),
                "case {case}, ammo {ammo}, decision {expected:?}",
            );
        }
    }
}
