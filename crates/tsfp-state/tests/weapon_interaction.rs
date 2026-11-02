use tsfp_state::{
    object_pool::ObjectId,
    scene_membership::{MembershipError, SceneMembership},
    weapon_interaction::{
        CapturedInteractionPlayer, InteractionCallbackId, InteractionHost,
        InteractionSubmissionOutcome, InteractionTargetStore, InteractionTargetStoreError,
        MissionMarkerEntry, MissionMarkerRule, NativeInteractionTarget,
        apply_post_callback_mission_flags, submit_interaction,
    },
    weapon_inventory::{WeaponInventory, WeaponInventoryStore, WeaponPlayerHandle, WeaponPlayerId},
};

#[derive(Default)]
struct Host {
    calls: Vec<&'static str>,
    player: Option<CapturedInteractionPlayer>,
    callback: Option<InteractionCallbackId>,
    mission_key: u32,
    observed_mission_key: Option<u32>,
}

impl InteractionHost for Host {
    type Error = ();

    fn capture_player(&mut self, _: WeaponPlayerId) -> Option<CapturedInteractionPlayer> {
        self.calls.push("capture");
        self.player
    }

    fn send_interaction(&mut self, _: WeaponPlayerId, _: ObjectId) {
        self.calls.push("send");
    }

    fn callback_for_target(
        &mut self,
        _: ObjectId,
    ) -> Result<Option<InteractionCallbackId>, Self::Error> {
        self.calls.push("callback_lookup");
        Ok(self.callback)
    }

    fn invoke_target_callback(
        &mut self,
        _: InteractionCallbackId,
        _: ObjectId,
        actor: Option<ObjectId>,
    ) -> Result<(), Self::Error> {
        self.calls.push("callback");
        assert_eq!(
            actor,
            Some(ObjectId {
                slot: 2,
                generation: 4
            })
        );
        self.mission_key = 0xfeed;
        Ok(())
    }

    fn set_captured_player_interaction_flag(
        &mut self,
        player: WeaponPlayerHandle,
    ) -> Result<(), Self::Error> {
        self.calls.push("player_flag");
        assert_eq!(player, self.player.unwrap().player);
        Ok(())
    }

    fn apply_post_callback_mission_flags(&mut self, _: ObjectId) -> Result<(), Self::Error> {
        self.calls.push("mission_flags");
        self.observed_mission_key = Some(self.mission_key);
        Ok(())
    }
}

fn player() -> CapturedInteractionPlayer {
    let player_id = WeaponPlayerId::new(1);
    let mut store = WeaponInventoryStore::<2>::default();
    let player_handle = store
        .begin_lifetime(player_id, WeaponInventory::default())
        .unwrap();
    CapturedInteractionPlayer {
        player: player_handle,
        actor: Some(ObjectId {
            slot: 2,
            generation: 4,
        }),
    }
}

#[test]
fn interaction_captures_before_send_and_reads_mission_key_after_callback() {
    let mut host = Host {
        player: Some(player()),
        callback: Some(InteractionCallbackId(8)),
        mission_key: 7,
        ..Host::default()
    };
    let outcome = submit_interaction(
        &mut host,
        WeaponPlayerId::new(1),
        ObjectId {
            slot: 5,
            generation: 2,
        },
    )
    .unwrap();

    assert_eq!(
        outcome,
        InteractionSubmissionOutcome::Completed {
            callback_invoked: true
        }
    );
    assert_eq!(
        host.calls,
        [
            "capture",
            "send",
            "callback_lookup",
            "callback",
            "player_flag",
            "mission_flags"
        ]
    );
    assert_eq!(host.observed_mission_key, Some(0xfeed));
}

#[test]
fn interaction_without_callback_still_sets_flag_then_mission_flags() {
    let mut host = Host {
        player: Some(player()),
        mission_key: 7,
        ..Host::default()
    };
    assert_eq!(
        submit_interaction(
            &mut host,
            WeaponPlayerId::new(1),
            ObjectId {
                slot: 5,
                generation: 2
            },
        )
        .unwrap(),
        InteractionSubmissionOutcome::Completed {
            callback_invoked: false
        }
    );
    assert_eq!(
        host.calls,
        [
            "capture",
            "send",
            "callback_lookup",
            "player_flag",
            "mission_flags"
        ]
    );
    assert_eq!(host.observed_mission_key, Some(7));
}

#[test]
fn interaction_without_native_player_makes_no_side_effects() {
    let mut host = Host::default();
    assert_eq!(
        submit_interaction(
            &mut host,
            WeaponPlayerId::new(1),
            ObjectId {
                slot: 5,
                generation: 2
            },
        )
        .unwrap(),
        InteractionSubmissionOutcome::MissingPlayer
    );
    assert_eq!(host.calls, ["capture"]);
}

#[test]
fn post_callback_marker_pass_updates_independent_matching_rules() {
    let mut entries = [
        Some(MissionMarkerEntry {
            first_rule: Some(MissionMarkerRule {
                kind: 0xC,
                mission_key: 0xfeed,
            }),
            second_rule: Some(MissionMarkerRule {
                kind: 0xD,
                mission_key: 0xfeed,
            }),
            ..MissionMarkerEntry::default()
        }),
        Some(MissionMarkerEntry {
            first_rule: Some(MissionMarkerRule {
                kind: 0xB,
                mission_key: 0xfeed,
            }),
            second_rule: Some(MissionMarkerRule {
                kind: 0xC,
                mission_key: 0xbeef,
            }),
            ..MissionMarkerEntry::default()
        }),
        None,
    ];

    apply_post_callback_mission_flags(0xfeed, &mut entries);

    assert!(entries[0].unwrap().first_marked);
    assert!(entries[0].unwrap().second_marked);
    assert!(!entries[1].unwrap().first_marked);
    assert!(!entries[1].unwrap().second_marked);
    assert_eq!(entries[2], None);
}

#[test]
fn native_target_store_reacquires_callback_state_and_rejects_old_generation() {
    let mut scene = SceneMembership::default();
    let original = scene.spawn(4).unwrap();
    let mut targets = InteractionTargetStore::default();
    targets
        .attach(
            &scene,
            original,
            NativeInteractionTarget::new(
                Some(InteractionCallbackId(7)),
                0x10,
                vec![Some(MissionMarkerEntry {
                    first_rule: Some(MissionMarkerRule {
                        kind: 0xC,
                        mission_key: 0x20,
                    }),
                    ..MissionMarkerEntry::default()
                })],
            ),
        )
        .unwrap();

    assert_eq!(
        targets.callback_for_target(&scene, original).unwrap(),
        Some(InteractionCallbackId(7))
    );
    // A synchronous callback changes the mission key before the continuation reacquires it.
    targets.set_mission_key(&scene, original, 0x20).unwrap();
    assert_eq!(targets.mission_key(&scene, original).unwrap(), 0x20);
    targets
        .apply_post_callback_mission_flags(&scene, original)
        .unwrap();
    assert!(
        targets.mission_markers(&scene, original).unwrap()[0]
            .unwrap()
            .first_marked
    );

    targets.retire(&mut scene, original).unwrap();
    let replacement = scene.spawn(original.slot).unwrap();
    targets
        .attach(
            &scene,
            replacement,
            NativeInteractionTarget::new(None, 0, Vec::new()),
        )
        .unwrap();
    assert_eq!(
        targets.callback_for_target(&scene, original),
        Err(InteractionTargetStoreError::Scene(
            MembershipError::NotActive
        ))
    );
    assert_eq!(targets.callback_for_target(&scene, replacement), Ok(None));
}
