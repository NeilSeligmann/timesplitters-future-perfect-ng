// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 differential against a bounded transcription of the lifted registration body.
//! WEAK EVIDENCE: deterministic fabricated inputs; not a natural-content replay or machine oracle.

extern crate alloc;

#[path = "../src/port_scaffolds/story_objective_registry_admission_slice.rs"]
mod port;
use port::{
    AllocationContract, NO_KIND, ObjectiveDefinition, ObjectiveRegistry, ObjectiveStatus,
    StoryContext,
};
use std::vec::Vec;

fn lifted_body_model(
    registry: &mut ObjectiveRegistry,
    definitions: &[ObjectiveDefinition],
    context: StoryContext,
) {
    // Bounded transcription of the observed row order: selected kind, record, append, counter.
    // Seeds stay within the supplied extents so the lifted body's missing guard is not exercised.
    for definition in definitions {
        if !context.allows(definition.predicate_mask) {
            continue;
        }
        let kind = definition.kind_by_submode[context.submode as usize];
        if kind == NO_KIND || !context.allows(0x700) {
            continue;
        }
        let index = definition.id as usize;
        let mut deps = Vec::new();
        for &dep in &definition.dependencies {
            if dep == u32::MAX {
                break;
            }
            if !deps.contains(&dep) {
                deps.push(dep);
            }
        }
        registry.records[index] = Some(port::ObjectiveRecord {
            id: definition.id,
            kind: kind as u8,
            status: definition.status,
            dependencies: deps.clone(),
        });
        registry.dependency_lists[index] = deps;
        registry.creation_order.push(definition.id);
        registry.kind_counts[kind as usize] += 1;
    }
}

fn fabricated(seed: &mut u64) -> (Vec<ObjectiveDefinition>, StoryContext) {
    let mut next = || {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 7;
        *seed ^= *seed << 17;
        *seed
    };
    let submode = (next() % 3) as u8;
    let count = (next() % 7 + 1) as usize;
    let mut defs = Vec::new();
    for id in 0..count {
        let mut kinds = [0, 1, 2];
        if next() & 1 != 0 {
            kinds[submode as usize] = NO_KIND;
        }
        let predicate_mask = if next() & 3 == 0 { 0x200 } else { 0x100 };
        defs.push(ObjectiveDefinition {
            id: id as u32,
            kind_by_submode: kinds,
            predicate_mask,
            status: if next() & 1 == 0 {
                ObjectiveStatus::Pending
            } else {
                ObjectiveStatus::Active
            },
            dependencies: vec![next() as u32 % 11, next() as u32 % 11, u32::MAX, 0xfeed],
        });
    }
    let context = StoryContext {
        game_flags: 0,
        mode_id: 10,
        submode,
        difficulty: 0,
        players: 1,
        network: false,
        descriptor_word: 0,
    };
    (defs, context)
}

#[test]
fn tier1_lifted_differential_3000_seeds_weak() {
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    for _ in 0..3000 {
        let (definitions, context) = fabricated(&mut seed);
        let allocation = AllocationContract {
            record_slots: 8,
            creation_order_slots: 8,
        };
        let mut lifted = ObjectiveRegistry::from_allocation(Some(allocation)).unwrap();
        let mut rust = lifted.clone();
        lifted_body_model(&mut lifted, &definitions, context);
        rust.register(&definitions, context).unwrap();
        assert_eq!(rust, lifted, "seed={seed:#018x}");
    }
}
