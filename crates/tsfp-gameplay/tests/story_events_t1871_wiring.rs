//! T1871: `HookServices::tick_effects` runs the ported class update callbacks (`0xF4A50`, `0xF08D0`, `0xF2B80`, `0xF39A0`) over the cell
//! (FABRICATED world: engine routines the port does not implement are recorded as `engine_call` and answer 0). Semantic unit tests; the
//! callbacks themselves are validated bit for bit against the original in `story_events_t1871_update`.
use tsfp_gameplay::story_events::svc_particle_efbd0::*;
use tsfp_gameplay::story_events::svc_pool_classes::{ClassSlot, ClassTable};
use tsfp_gameplay::story_events::svc_wiring::{HookServices, TraceEvent};

fn services_with_projectile(velocity: [f32; 3]) -> HookServices {
    let mut services = HookServices::new(1);
    let mut records = vec![[0u32; 16]; 8];
    records[3][ClassSlot::Update.offset() as usize / 4] = 0xF4A50;
    services.classes = ClassTable { records };
    let entry = &mut services.ember.pool.entries[0];
    entry.w[W_TYPE] = 3;
    entry.set_f(0x88 / 4, 1.0);
    entry.w[0x64 / 4..0x64 / 4 + 3].copy_from_slice(&velocity.map(f32::to_bits));
    services
}

fn engine_calls(services: &HookServices) -> Vec<u32> {
    services
        .trace()
        .iter()
        .filter_map(|event| match event {
            TraceEvent::Ember {
                service: "engine_call",
                args,
            } => Some(args[0]),
            _ => None,
        })
        .collect()
}

#[test]
fn a_moving_projectile_advances_by_velocity_times_dt() {
    let mut services = services_with_projectile([1.0, 0.0, 0.0]);
    services.tick_effects(1);
    let entry = &services.ember.pool.entries[0];
    let expected = (1.0f32 / 60.0) * 1.0;
    assert_eq!(entry.f(W_POS), expected);
    assert_eq!(entry.f(W_POS + 1), 0.0);
    let calls = engine_calls(&services);
    // no cell, no ray (0xE97A0 casts only with a positive cell); the update ends with the settle call (0xEDBE0); nothing was released
    assert!(
        calls.contains(&0xEDBE0) && !calls.contains(&0x88C60),
        "{calls:x?}"
    );
    assert_ne!(entry.w[W_TYPE], u32::MAX);
    let updates = services
        .trace()
        .iter()
        .filter(|e| {
            matches!(
                e,
                TraceEvent::Ember {
                    service: "effect_update",
                    ..
                }
            )
        })
        .count();
    assert_eq!(updates, 1);
}

#[test]
fn a_projectile_without_velocity_is_retired() {
    let mut services = services_with_projectile([0.0, 0.0, 0.0]);
    services.tick_effects(1);
    assert_eq!(
        services.ember.pool.entries[0].w[W_TYPE],
        u32::MAX,
        "released through 0xDC550"
    );
    assert!(engine_calls(&services).contains(&0xE1EB0));
}

#[test]
fn a_projectile_expires_at_two_seconds() {
    let mut services = services_with_projectile([1.0, 0.0, 0.0]);
    services.ember.pool.entries[0].set_f(W_TIMER, 2.0);
    services.tick_effects(1);
    assert_eq!(services.ember.pool.entries[0].w[W_TYPE], u32::MAX);
}
