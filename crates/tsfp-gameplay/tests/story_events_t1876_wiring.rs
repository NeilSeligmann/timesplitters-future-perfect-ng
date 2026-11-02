//! T1876: `HookServices::tick_effects` runs the eight update callbacks added by T1876 (`0xF5FF0`, `0xF5920`, `0xF61E0`, `0xEBB00`, `0xEC120`,
//! `0xE4800`, `0xF51F0`, `0xECB90`) over the cell (FABRICATED world: unported engine routines are recorded and answer 0). Semantic unit tests;
//! the callbacks are validated bit for bit against the original in `story_events_t1876_update`.
use tsfp_gameplay::story_events::svc_particle_efbd0::*;
use tsfp_gameplay::story_events::svc_pool_classes::{ClassSlot, ClassTable};
use tsfp_gameplay::story_events::svc_wiring::{HookServices, TraceEvent};

fn services_with(entry_type: u32, callback: u32) -> HookServices {
    let mut services = HookServices::new(1);
    let mut records = vec![[0u32; 16]; 0x40];
    records[entry_type as usize][ClassSlot::Update.offset() as usize / 4] = callback;
    services.classes = ClassTable { records };
    let entry = &mut services.ember.pool.entries[0];
    entry.w[W_TYPE] = entry_type;
    entry.set_f(0x88 / 4, 1.0);
    services
}

fn updates(services: &HookServices) -> usize {
    services
        .trace()
        .iter()
        .filter(|event| {
            matches!(
                event,
                TraceEvent::Ember {
                    service: "effect_update",
                    ..
                }
            )
        })
        .count()
}

#[test]
fn a_beam_without_its_live_word_is_released() {
    let mut services = services_with(0x2F, 0xF5FF0);
    services.tick_effects(1);
    assert_eq!(services.ember.pool.entries[0].w[W_TYPE], u32::MAX);
    assert_eq!(updates(&services), 1);
}

#[test]
fn a_bolt_expires_after_three_quarters_of_a_second() {
    let mut services = services_with(0x15, 0xF5920);
    services.ember.pool.entries[0].set_f(W_TIMER, 0.75);
    services.tick_effects(1);
    assert_eq!(services.ember.pool.entries[0].w[W_TYPE], u32::MAX);
}

#[test]
fn a_flame_jet_flickers_between_zero_and_one() {
    let mut services = services_with(0x1A, 0xEBB00);
    services.tick_effects(1);
    let flicker = f32::from_bits(services.ember.pool.entries[0].w[0x200 / 4]);
    // the sine table of the cell is empty (0): 0.5 * 0.15 + 0.5 * 0.1 + 0.5 * 0.2 + 0.5
    assert!((flicker - 0.725).abs() < 1e-6, "{flicker}");
    assert_eq!(updates(&services), 1);
}

#[test]
fn a_drone_without_a_trail_allocator_does_nothing() {
    let mut services = services_with(0x2B, 0xECB90);
    services.tick_effects(1);
    assert_ne!(
        services.ember.pool.entries[0].w[W_TYPE],
        u32::MAX,
        "the unported allocator 0x3ED20 answers 0: the update returns early"
    );
    assert_eq!(updates(&services), 1);
}

fn init_updates(services: &HookServices) -> usize {
    services
        .trace()
        .iter()
        .filter(|event| {
            matches!(
                event,
                TraceEvent::Ember {
                    service: "effect_init",
                    ..
                }
            )
        })
        .count()
}

fn services_with_init(entry_type: u32, callback: u32) -> HookServices {
    let mut services = HookServices::new(1);
    let mut records = vec![[0u32; 16]; 0x40];
    records[entry_type as usize][ClassSlot::Init.offset() as usize / 4] = callback;
    services.classes = ClassTable { records };
    services.ember.pool.entries[0].w[W_TYPE] = entry_type;
    services
}

#[test]
fn the_bolt_init_quadruples_the_velocity() {
    let mut services = services_with_init(0x15, 0xE3640);
    services.ember.pool.entries[0].set_f(0x64 / 4, 1.5);
    services.init_effect(0);
    assert_eq!(services.ember.pool.entries[0].f(0x64 / 4), 6.0);
    assert_eq!(init_updates(&services), 1);
}

#[test]
fn the_beam_init_normalises_the_velocity() {
    let mut services = services_with_init(0x2F, 0xE36D0);
    services.ember.pool.entries[0].set_f(0x6C / 4, 4.0);
    services.init_effect(0);
    let entry = &services.ember.pool.entries[0];
    assert_eq!(entry.w[0x14C / 4], 1);
    assert_eq!(entry.f(0x188 / 4), 1.0);
}

#[test]
fn an_unported_init_is_recorded_not_run() {
    let mut services = services_with_init(0x2F, 0xABCDE);
    services.init_effect(0);
    assert_eq!(init_updates(&services), 0);
    assert!(services.trace().iter().any(|event| matches!(
        event,
        TraceEvent::Ember {
            service: "class_callback",
            ..
        }
    )));
}
