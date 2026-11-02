//! T1870: `HookServices::tick_effects` runs the ported trail and light service, the class update masking and the late loop over the
//! cell (FABRICATED world: the services answer neutrally and are recorded). Semantic unit tests, not golden comparisons.
use tsfp_gameplay::story_events::svc_particle_efbd0::*;
use tsfp_gameplay::story_events::svc_pool_classes::{ClassSlot, ClassTable};
use tsfp_gameplay::story_events::svc_wiring::{HookServices, TraceEvent};

fn services_with_effect(kind: u32) -> HookServices {
    let mut services = HookServices::new(1);
    let entry = &mut services.ember.pool.entries[0];
    entry.w[W_TYPE] = EFFECT_TYPE;
    entry.w[W_KIND] = kind;
    entry.set_f(W_TIMER, 0.0);
    entry.w[W_POS..W_POS + 3].copy_from_slice(&[
        1.0f32.to_bits(),
        2.0f32.to_bits(),
        3.0f32.to_bits(),
    ]);
    services.ember.rows[kind as usize] = KindRow {
        effect_a: -1,
        effect_b: -1,
        word_8: 6.0f32.to_bits(),
        light_radius: 10.0,
        ..KindRow::default()
    };
    services
}

fn names(services: &HookServices) -> Vec<&'static str> {
    services
        .trace()
        .iter()
        .filter_map(|event| match event {
            TraceEvent::Ember { service, .. } => Some(*service),
            _ => None,
        })
        .collect()
}

#[test]
fn live_effects_emit_light_bursts_every_tenth_of_a_second() {
    let mut services = services_with_effect(5);
    for _ in 0..40 {
        services.tick_effects(1);
    }
    let rooms = names(&services)
        .iter()
        .filter(|name| **name == "light_rooms")
        .count();
    // rate 60: a burst every 6 ticks (0.1 s) once the timer allows it; the emission counter cell `[entry + 0xFC]` counts them
    let counter = services.ember.pool.entries[0].w[0xFC / 4];
    assert_eq!(counter, 7, "emission counter {counter}, rooms {rooms}");
    assert!(rooms >= 1 && rooms <= 7, "{rooms}");
}

#[test]
fn light_stops_after_the_third_phase_and_the_entry_is_released() {
    let mut services = services_with_effect(5);
    for _ in 0..120 {
        services.tick_effects(1);
    }
    // timer passes 1.25 s at tick 75: the entry is released
    assert_eq!(services.ember.pool.entries[0].w[W_TYPE], u32::MAX);
}

#[test]
fn unported_updates_are_recorded_but_late_and_destroy_callbacks_run() {
    // 0xE8490 (type 0x30) stays unported (T1871 and T1876 ported 14 update callbacks, see story_events_t1871_wiring and story_events_t1876_wiring)
    let mut services = HookServices::new(1);
    let mut records = vec![[0u32; 16]; 8];
    records[3][ClassSlot::Update.offset() as usize / 4] = 0xE8490;
    records[3][ClassSlot::Late.offset() as usize / 4] = 0xE29D0;
    records[3][ClassSlot::Destroy.offset() as usize / 4] = 0xE3B00;
    services.classes = ClassTable { records };
    services.ember.pool.entries[2].w[W_TYPE] = 3;
    services.tick_effects(1);
    let seen: Vec<(u32, u32)> = services
        .trace()
        .iter()
        .filter_map(|event| match event {
            TraceEvent::Ember {
                service: "class_callback",
                args,
            } => Some((args[0], args[1])),
            _ => None,
        })
        .collect();
    assert_eq!(seen, vec![(0x34, 0xE8490)]);
    assert!(names(&services).contains(&"effect_late"));
    services.destroy_effects();
    let destroyed = names(&services)
        .iter()
        .filter(|name| **name == "class_callback")
        .count();
    assert_eq!(destroyed, 1);
    assert!(names(&services).contains(&"effect_destroy"));
}
