//! T1863: the kind handlers and ember services reached through the hook services cell (`HookServices::spawn_effect`).
use tsfp_gameplay::story_events::svc_emitter::{CURRENT_EMITTER, USED_COUNT};
use tsfp_gameplay::story_events::svc_particle_efbd0::SpawnArgs;
use tsfp_gameplay::story_events::svc_particle_kinds::HANDLED_KINDS;
use tsfp_gameplay::story_events::svc_wiring::{HookServices, TraceEvent};

fn args(kind: u32) -> SpawnArgs {
    SpawnArgs {
        kind,
        extra: u32::MAX,
        position: [1.0, 2.0, 3.0],
        direction: Some([0.0, 1.0, 0.0]),
        source: None,
        owner: 0,
        seconds: 0.0,
        sound: false,
    }
}

fn emitted(services: &HookServices) -> usize {
    services
        .trace()
        .iter()
        .filter(|e| {
            matches!(
                e,
                TraceEvent::Ember {
                    service: "emitter",
                    ..
                }
            )
        })
        .count()
}

#[test]
fn every_handled_kind_starts_through_the_cell() {
    for kind in HANDLED_KINDS {
        let mut services = HookServices::new(1);
        let result = services.spawn_effect(&args(kind));
        assert!(result.started, "kind {kind} started");
        assert!(
            !services.trace().iter().any(|e| matches!(
                e,
                TraceEvent::Ember {
                    service: "start_kind_unhandled",
                    ..
                }
            )),
            "kind {kind} has a handler"
        );
        let ram = services.ram.borrow();
        // kinds 43 (beam and flash) and 44 to 46 (owner required) create no emitter
        if ![43, 44, 45, 46].contains(&kind) {
            assert!(emitted(&services) > 0, "kind {kind} created emitters");
            assert!(
                ram.word(USED_COUNT) > 0,
                "kind {kind} used the emitter pool"
            );
        }
    }
}

#[test]
fn kind_10_creates_two_emitters_and_scales_the_last() {
    let mut services = HookServices::new(1);
    services.spawn_effect(&args(10));
    assert_eq!(emitted(&services), 2);
    let ram = services.ram.borrow();
    assert_eq!(ram.word(USED_COUNT), 2);
    let current = ram.word(CURRENT_EMITTER);
    assert_ne!(current, 0);
    assert_eq!(
        f32::from_bits(ram.word(current + 0x64)),
        0.3,
        "Scale(0.3) reached the emitter"
    );
}

#[test]
fn kind_44_builds_three_debris_objects_around_the_owner() {
    let mut services = HookServices::new(1);
    let owner = 0x5000_0000;
    {
        let mut ram = services.ram.borrow_mut();
        for k in 0..3u32 {
            ram.set_word(owner + 0x1C0 + 4 * k, 2.0f32.to_bits());
        }
    }
    let mut spawn = args(44);
    spawn.owner = owner;
    services.spawn_effect(&spawn);
    let creates = services
        .trace()
        .iter()
        .filter(|e| {
            matches!(
                e,
                TraceEvent::Ember {
                    service: "debris_create",
                    ..
                }
            )
        })
        .count();
    assert_eq!(creates, 3);
}

#[test]
fn the_pool_runs_dry_without_hanging() {
    let mut services = HookServices::new(1);
    for _ in 0..200 {
        services.spawn_effect(&args(6));
    }
    // 64 records, 2 emitters per kind 6: the creations after the pool is full return 0 and nothing crashes
    assert_eq!(services.ram.borrow().word(USED_COUNT), 64);
}
