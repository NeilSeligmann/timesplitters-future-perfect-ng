// SPDX-License-Identifier: GPL-3.0-or-later
//! T1661: Rust-owned actor health. Scalar Tier 1 compares against lifted 0x2777E0;
//! guard outcomes use the source-inspected T1661 contract and fabricated state.
use std::collections::BTreeMap;
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    c2r::t1926_characters::game_actor_field_0x1d8_scaled_by_mode_divisor_and_constant,
    memory::GuestMemory as PortMemory,
};
use tsfp_state::{
    actor_health::{
        ActorHealth, ActorHealthSet, HealthTickInputs, HealthTickResult, HealthTickSkip,
    },
    object_pool::ObjectId,
    GuestMemory,
};

#[derive(Default)]
struct Memory(BTreeMap<u32, u8>);

impl PortMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.get(&address).copied().unwrap_or(0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.0.insert(address, value);
    }
}
impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        PortMemory::read8(self, address)
    }
    fn write8(&mut self, address: u32, value: u8) {
        PortMemory::write8(self, address, value);
    }
}

const STACK: u32 = 0x20000;
const OBJECT: u32 = 0x30000;

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

struct NoCalls;
impl GuestCall for NoCalls {
    fn call(&mut self, va: u32, _: &mut ExtRegisters) {
        panic!("unexpected lifted leaf call to {va:#x}");
    }
}

fn lifted_term(
    selector: u32,
    mode_byte: u8,
    frame_numerator: f32,
    frame_divisor: i32,
    mode_divisor: f32,
    tick_scale: f32,
    maximum: f32,
) -> f32 {
    let mut memory = Memory::default();
    PortMemory::write32(&mut memory, STACK + 4, OBJECT);
    PortMemory::write32(&mut memory, 0x79094c, selector);
    PortMemory::write32(&mut memory, 0x475c78, frame_numerator.to_bits());
    PortMemory::write32(&mut memory, 0x475d18, 3.0f32.to_bits());
    PortMemory::write32(&mut memory, 0x4e8cec, frame_divisor as u32);
    PortMemory::write32(&mut memory, 0x4e794c, tick_scale.to_bits());
    PortMemory::write8(&mut memory, 0x277840 + selector.wrapping_sub(1), mode_byte);
    PortMemory::write32(&mut memory, 0x277838, 0x277804);
    PortMemory::write32(&mut memory, 0x27783c, 0x27780c);
    PortMemory::write32(&mut memory, OBJECT + 0x1d8, maximum.to_bits());
    let mut regs = registers();
    game_actor_field_0x1d8_scaled_by_mode_divisor_and_constant(
        &mut memory,
        &mut NoCalls,
        &mut regs,
    );
    let expected_divisor: f32 = if selector <= 81 && mode_byte == 0 {
        3.0
    } else {
        1.0
    };
    assert_eq!(expected_divisor.to_bits(), mode_divisor.to_bits());
    f32::from_bits(regs.xmm[0].lanes[0])
}

fn inputs(referenced_object: ObjectId, damage_timer: f32, selector: u32) -> HealthTickInputs {
    HealthTickInputs {
        referenced_object,
        damage_timer,
        selector,
        frame_numerator: 1.0,
        frame_divisor: 60,
        mode_divisor: 1.0,
        tick_scale: 1.0,
    }
}

#[test]
fn source_inspected_tick_gates_skip_without_mutating_health() {
    let actor_id = ObjectId {
        slot: 4,
        generation: 7,
    };
    let player_id = ObjectId {
        slot: 2,
        generation: 3,
    };
    let mut actors = ActorHealthSet::default();
    actors.insert(
        actor_id,
        ActorHealth {
            health: 4.0,
            maximum: 20.0,
        },
    );

    for (input, reason) in [
        (
            inputs(player_id, 5.0, 2),
            HealthTickSkip::DamageTimerNotAboveFive,
        ),
        (
            inputs(player_id, f32::NAN, 2),
            HealthTickSkip::DamageTimerNotAboveFive,
        ),
        (
            inputs(actor_id, 6.0, 2),
            HealthTickSkip::SameAsReferencedObject,
        ),
        (
            inputs(player_id, 6.0, 26),
            HealthTickSkip::SelectorIsTwentySix,
        ),
    ] {
        assert_eq!(
            actors.tick(actor_id, input),
            HealthTickResult::Skipped(reason)
        );
        assert_eq!(
            actors.get(actor_id).unwrap().health.to_bits(),
            4.0f32.to_bits()
        );
    }
    let stale_generation = ObjectId {
        slot: actor_id.slot,
        generation: actor_id.generation + 1,
    };
    assert_eq!(
        actors.tick(stale_generation, inputs(player_id, 6.0, 2)),
        HealthTickResult::Skipped(HealthTickSkip::ActorMissing)
    );
}

#[test]
fn referenced_object_guard_uses_slot_and_generation() {
    let actor_id = ObjectId {
        slot: 4,
        generation: 7,
    };
    let reused_slot = ObjectId {
        slot: actor_id.slot,
        generation: actor_id.generation + 1,
    };
    let mut actors = ActorHealthSet::default();
    actors.insert(
        actor_id,
        ActorHealth {
            health: 4.0,
            maximum: 20.0,
        },
    );

    assert!(matches!(
        actors.tick(actor_id, inputs(reused_slot, 6.0, 2)),
        HealthTickResult::Updated { .. }
    ));
}

#[test]
fn health_tick_clamps_and_keeps_unordered_sse_comparisons() {
    let actor_id = ObjectId {
        slot: 4,
        generation: 1,
    };
    let player_id = ObjectId {
        slot: 2,
        generation: 1,
    };
    let mut actors = ActorHealthSet::default();
    actors.insert(
        actor_id,
        ActorHealth {
            health: 19.9,
            maximum: 20.0,
        },
    );
    let mut tuning = inputs(player_id, 6.0, 2);
    tuning.tick_scale = 20.0;
    assert!(matches!(
        actors.tick(actor_id, tuning),
        HealthTickResult::Updated { health: 20.0, .. }
    ));
    actors.insert(
        actor_id,
        ActorHealth {
            health: 0.1,
            maximum: 20.0,
        },
    );
    tuning.tick_scale = -20.0;
    assert!(matches!(
        actors.tick(actor_id, tuning),
        HealthTickResult::Updated { health: 0.0, .. }
    ));
    actors.insert(
        actor_id,
        ActorHealth {
            health: 4.0,
            maximum: 20.0,
        },
    );
    tuning.frame_numerator = f32::NAN;
    let HealthTickResult::Updated { health, .. } = actors.tick(actor_id, tuning) else {
        panic!("tick should pass its guards");
    };
    assert!(health.is_nan());
}

#[test]
fn scalar_term_matches_lifted_2777e0_for_3000_fabricated_inputs() {
    let actor_id = ObjectId {
        slot: 9,
        generation: 11,
    };
    let player_id = ObjectId {
        slot: 10,
        generation: 4,
    };
    let mut seed = 0x1661_2777u32;
    let mut actors = ActorHealthSet::default();
    for case in 0..3_000u32 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let selector = case % 90 + 1;
        let selector = if selector == 26 {
            selector + 1
        } else {
            selector
        };
        let mode_byte = (seed >> 12) as u8 & 1;
        // The established numerator constant at 0x475c78 is 1.0 in this leaf contract.
        let numerator = 1.0f32;
        let divisor = match (seed >> 20) & 3 {
            0 => -120,
            1 => -1,
            2 => 1,
            _ => 60,
        };
        let scale = (seed as i16 as f32) / 4096.0;
        let maximum = ((seed.rotate_left(9) & 0xffff) as f32) / 1024.0;
        let mode_divisor = if selector <= 81 && mode_byte == 0 {
            3.0
        } else {
            1.0
        };
        let expected = lifted_term(
            selector,
            mode_byte,
            numerator,
            divisor,
            mode_divisor,
            scale,
            maximum,
        );

        actors.insert(
            actor_id,
            ActorHealth {
                health: 12.0,
                maximum,
            },
        );
        let mut tick = inputs(player_id, 6.0, selector);
        tick.frame_numerator = numerator;
        tick.frame_divisor = divisor;
        tick.mode_divisor = mode_divisor;
        tick.tick_scale = scale;
        let HealthTickResult::Updated { term, .. } = actors.tick(actor_id, tick) else {
            panic!("case {case} unexpectedly skipped");
        };
        assert_eq!(
            term.to_bits(),
            expected.to_bits(),
            "lifted 0x2777E0 case {case}"
        );
    }
}
