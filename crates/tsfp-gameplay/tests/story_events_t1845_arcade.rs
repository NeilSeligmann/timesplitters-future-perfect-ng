//! T1845 item D: arcade objective runtime and level 0x64 facts. Synthetic data only.
use tsfp_gameplay::story_events::arcade::*;

#[derive(Default)]
struct Mock {
    finished: Vec<u32>,
    actors: Vec<Option<ActorState>>,
    complete: bool,
    seed: u32,
    triggered: Vec<bool>,
    candidates: Vec<TargetObject>,
    wanted: u32,
    bits: Option<u32>,
}

impl ArcadeWorld for Mock {
    fn slot_finished(&self, slot: u32) -> bool {
        self.finished.contains(&slot)
    }
    fn random_u32(&mut self) -> u32 {
        self.seed
    }
    fn actor(&self, _slot: u32, index: usize) -> Option<ActorState> {
        self.actors.get(index).copied().flatten()
    }
    fn member_complete(&mut self, _slot: u32, _member: usize) -> bool {
        self.complete
    }
    fn member_bits(&mut self, _slot: u32, _member: usize, _kind: u32) -> MemberBits {
        MemberBits {
            bits: self.bits.unwrap_or(0b101),
            width: 3,
        }
    }
    fn sub_entry_count(&self, _slot: u32, _member: usize) -> usize {
        3
    }
    fn sub_entry_triggered(&self, _slot: u32, _member: usize, index: usize) -> bool {
        self.triggered.get(index).copied().unwrap_or(false)
    }
    fn set_sub_entry_triggered(&mut self, _slot: u32, _member: usize, index: usize) {
        if self.triggered.len() <= index {
            self.triggered.resize(index + 1, false);
        }
        self.triggered[index] = true;
    }
    fn type7_candidates(&mut self, _slot: u32, _member: usize) -> (u32, Vec<TargetObject>) {
        (self.wanted, self.candidates.clone())
    }
    fn type89_targets(&mut self, _slot: u32, _member: usize) -> Vec<TargetObject> {
        self.candidates.clone()
    }
}

fn alive() -> Option<ActorState> {
    Some(ActorState {
        present: true,
        health: 5.0,
        flag: false,
    })
}

#[test]
fn level64_hook_is_never_written() {
    assert_eq!(level64::hook(), None);
    assert_eq!(level64::STRUCT_SIZE, 0x55 * 4);
    assert!(level64::HOOK_FIELD < level64::STRUCT_SIZE);
}

#[test]
fn objective_hooks_follow_the_byte_map() {
    assert_eq!(
        objective_hook_for_level(0x64),
        Some(ObjectiveHook::MapMaker)
    );
    assert_eq!(
        objective_hook_for_level(0x1D).map(ObjectiveHook::address),
        Some(0x208030)
    );
    assert_eq!(
        objective_hook_for_level(0x30).map(ObjectiveHook::address),
        Some(0x389B0)
    );
    assert_eq!(objective_hook_for_level(0x1C), None);
    assert_eq!(objective_hook_for_level(0x10), None);
}

#[test]
fn actors_down_bits_rise_and_notify_once() {
    let mut table = ObjectiveTable::build(
        1,
        &[EntryDescriptor {
            slot: 7,
            kind: 0,
            members: vec![],
        }],
    );
    let mut world = Mock {
        actors: vec![alive(); 10],
        ..Default::default()
    };
    assert!(table.frame(&mut world).is_empty());
    assert_eq!(table.entries[0].current, 0);
    world.actors[3] = Some(ActorState {
        present: true,
        health: 0.0,
        flag: false,
    });
    world.actors[4] = None;
    let events = table.frame(&mut world);
    assert_eq!(table.entries[0].current, 0b11000);
    assert_eq!(events.len(), 2);
    assert!(table.frame(&mut world).is_empty());
}

#[test]
fn finished_slot_sets_bit_15_random_bits_and_finishes() {
    let mut table = ObjectiveTable::build(
        1,
        &[EntryDescriptor {
            slot: 2,
            kind: 1,
            members: vec![],
        }],
    );
    let mut world = Mock {
        finished: vec![2],
        seed: u32::MAX,
        ..Default::default()
    };
    let events = table.frame(&mut world);
    assert_eq!(events, vec![ObjectiveEvent::SlotFinished { slot: 2 }]);
    let word = table.entries[0].current;
    assert_eq!(word & STATE_MASK, FINISHED_BIT);
    assert_eq!(word >> 10 & 0x1F, 31);
    // Unchanged masked state: no new random draw, no second notification.
    world.seed = 0;
    assert!(table.frame(&mut world).is_empty());
}

#[test]
fn flag_entry_keeps_the_actor_flag_in_step() {
    let mut table = ObjectiveTable::build(
        1,
        &[EntryDescriptor {
            slot: 1,
            kind: 2,
            members: vec![],
        }],
    );
    let mut world = Mock {
        actors: vec![
            Some(ActorState {
                present: true,
                health: 1.0,
                flag: true,
            }),
            None,
        ],
        ..Default::default()
    };
    // First frame: actor 0 flag set -> bit 0 set, rising notification, flag already matches the bit so no toggle.
    let events = table.frame(&mut world);
    assert_eq!(
        events,
        vec![ObjectiveEvent::Hook {
            slot: 1,
            index: 0,
            kind: 2,
            rising: true
        }]
    );
}

#[test]
fn member_entry_packs_widths_and_runs_effects() {
    let descriptor = EntryDescriptor {
        slot: 9,
        kind: 3,
        members: vec![Member { kind: 1 }, Member { kind: 0 }, Member { kind: 3 }],
    };
    let mut table = ObjectiveTable::build(1, &[descriptor]);
    let mut world = Mock {
        complete: true,
        ..Default::default()
    };
    let events = table.frame(&mut world);
    // Type 1 gives 0b101 over 3 bits, then two complete members at bits 3 and 4.
    assert_eq!(table.entries[0].current & STATE_MASK, 0b11_101);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(
                e,
                ObjectiveEvent::HookObject {
                    slot: 9,
                    index: 0,
                    object: None
                }
            ))
            .count(),
        2
    );
    // Type 1 completes its group node on the first trigger.
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ObjectiveEvent::GroupNodeComplete { member: 0, .. }))
    );
    // Rising bit of an already complete member: no effect (cur == complete).
    assert!(!events.iter().any(|e| matches!(
        e,
        ObjectiveEvent::ScoreEffect { .. } | ObjectiveEvent::ToggleMemberFlag { .. }
    )));
    // The member becomes incomplete while the bit stays: nothing. Falling bit with a complete member: effect.
    world.complete = false;
    let _ = table.frame(&mut world);
    assert_eq!(table.entries[0].current & STATE_MASK, 0b101);
}

#[test]
fn map_maker_hook_finds_the_entry_on_finish_only() {
    let table = ObjectiveTable::build(
        0x64,
        &[
            EntryDescriptor {
                slot: 4,
                kind: 1,
                members: vec![],
            },
            EntryDescriptor {
                slot: 6,
                kind: 1,
                members: vec![],
            },
        ],
    );
    assert_eq!(table.hook, Some(ObjectiveHook::MapMaker));
    assert_eq!(map_maker_hook(&table, 6, EVENT_FINISHED), Some(1));
    assert_eq!(map_maker_hook(&table, 6, 3), None);
    assert_eq!(map_maker_hook(&table, 99, EVENT_FINISHED), None);
}

#[test]
fn runtime_gate_matches_355ec0() {
    assert!(ObjectiveTable::runtime_enabled(1, false, false));
    assert!(!ObjectiveTable::runtime_enabled(1, true, false));
    assert!(ObjectiveTable::runtime_enabled(1, true, true));
    assert!(ObjectiveTable::runtime_enabled(0x65, true, true));
    assert!(!ObjectiveTable::runtime_enabled(0x65, false, false));
}

fn first_frame(kinds: &[u32], world: &mut Mock) -> (ObjectiveTable, Vec<ObjectiveEvent>) {
    let members = kinds.iter().map(|&kind| Member { kind }).collect();
    let mut table = ObjectiveTable::build(
        1,
        &[EntryDescriptor {
            slot: 2,
            kind: 3,
            members,
        }],
    );
    let events = table.frame(world);
    (table, events)
}

#[test]
fn type2_completes_only_when_every_sub_entry_triggered() {
    let mut world = Mock::default();
    let (mut table, events) = first_frame(&[2], &mut world);
    assert!(events.iter().any(|e| matches!(
        e,
        ObjectiveEvent::SubEntryTriggered {
            sub: 2,
            triggered: 2,
            ..
        }
    )));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, ObjectiveEvent::GroupNodeComplete { .. }))
    );
    assert_eq!(world.triggered, vec![true, false, true]);
    // MEASURED quirk: the running count restarts every frame, so a later single trigger never reaches the total.
    world.bits = Some(0b111);
    let events = table.frame(&mut world);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, ObjectiveEvent::GroupNodeComplete { .. }))
    );
    // All three bits rising in one frame complete the node.
    let mut world = Mock::default();
    let mut table = ObjectiveTable::build(
        1,
        &[EntryDescriptor {
            slot: 2,
            kind: 3,
            members: vec![Member { kind: 2 }],
        }],
    );
    world.bits = Some(0b111);
    let events = table.frame(&mut world);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, ObjectiveEvent::GroupNodeComplete { member: 0, .. }))
    );
}

#[test]
fn type7_hooks_with_the_flagged_target_and_clears_it() {
    let mut world = Mock {
        wanted: 7,
        candidates: vec![
            TargetObject {
                handle: 11,
                id: 5,
                flagged: true,
            },
            TargetObject {
                handle: 12,
                id: 7,
                flagged: true,
            },
        ],
        ..Default::default()
    };
    // pass 1 for type 7 comes from member_bits (0b101 width 1 => bit 0 set).
    let (_, events) = first_frame(&[7], &mut world);
    assert_eq!(
        events,
        vec![
            ObjectiveEvent::HookObject {
                slot: 2,
                index: 0,
                object: Some(12)
            },
            ObjectiveEvent::GroupNodeDirty { slot: 2, member: 0 },
            ObjectiveEvent::TargetFlagsCleared { object: 12 },
        ]
    );
    // Same wanted id but flag clear: the hook still runs with a null object, nothing is cleared.
    let mut world = Mock {
        wanted: 7,
        candidates: vec![TargetObject {
            handle: 12,
            id: 7,
            flagged: false,
        }],
        ..Default::default()
    };
    let (_, events) = first_frame(&[7], &mut world);
    assert_eq!(
        events[0],
        ObjectiveEvent::HookObject {
            slot: 2,
            index: 0,
            object: None
        }
    );
    assert_eq!(events.len(), 2);
    // A complete member whose bit is clear is skipped (no hook).
    let mut world = Mock {
        complete: true,
        wanted: 7,
        candidates: vec![],
        ..Default::default()
    };
    let (_, events) = first_frame(&[7], &mut world);
    assert!(events.is_empty());
}

#[test]
fn type89_hooks_per_rising_target_without_advancing_the_counter() {
    let mut world = Mock {
        candidates: vec![
            TargetObject {
                handle: 1,
                id: 0,
                flagged: true,
            },
            TargetObject {
                handle: 2,
                id: 0,
                flagged: false,
            },
        ],
        ..Default::default()
    };
    let (_, events) = first_frame(&[8], &mut world);
    // Bit 0 is set by pass 1 (member_bits 0b101), so only target 0 rises (bit 2 has no target).
    assert_eq!(
        events[0],
        ObjectiveEvent::HookObject {
            slot: 2,
            index: 0,
            object: Some(1)
        }
    );
    assert!(events.contains(&ObjectiveEvent::TargetFlagsCleared { object: 1 }));
    assert_eq!(events.len(), 3);
}

fn call(slot: u32, event: u32, state: u16) -> TableHookCall {
    TableHookCall { slot, event, state }
}

#[test]
fn level1b_hook_slots() {
    assert_eq!(level1b_hook(call(0, 0xF, 0)).len(), 3);
    assert_eq!(
        level1b_hook(call(1, 0, 0)),
        vec![TableHookOp::HudMessageAllPlayers { text_id: 0x11D8 }]
    );
    assert!(level1b_hook(call(1, 0, FINISHED_BIT)).is_empty());
    assert!(level1b_hook(call(1, 5, 0)).is_empty());
    let ops = level1b_hook(call(3, 0xF, 0));
    assert!(ops.contains(&TableHookOp::Sound { id: 0x688 }));
    assert_eq!(
        level1b_hook(call(2, 0xF, 0)),
        vec![TableHookOp::Destroy(ObjectRef::Tag(0x444F_5233))]
    );
    assert!(level1b_hook(call(5, 0xF, 0)).is_empty());
}

#[test]
fn level3b_and_1d_hooks() {
    assert_eq!(level3b_hook(call(1, 5, 0)).len(), 1);
    assert!(level3b_hook(call(1, 6, 0)).is_empty());
    assert_eq!(
        level3b_hook(call(2, 3, 0)),
        vec![TableHookOp::KillCredited(ObjectRef::SlotActor {
            slot: 2,
            index: 3
        })]
    );
    assert!(level3b_hook(call(2, 4, 0)).is_empty());
    // Level 0x1D slot 3 picks the spawn record from the random bits of the state word.
    let state = (7u16 << 10) | FINISHED_BIT;
    assert!(
        level1d_hook(call(3, 0xF, state)).contains(&TableHookOp::PlacePropsAtSpawn {
            record_id: SLOT3_SPAWN_IDS[2]
        })
    );
    assert_eq!(
        level1d_hook(call(0, 0, 0))[1],
        TableHookOp::Destroy(ObjectRef::Argument)
    );
    assert_eq!(
        run_table_hook(ObjectiveHook::Level30NoOp, call(0, 0xF, 0)),
        vec![]
    );
}

struct Groups {
    nodes: Vec<(bool, bool)>,
    refresh_completes: bool,
    any: bool,
}

impl GroupWorld for Groups {
    fn member_count(&self, _group: usize) -> usize {
        self.nodes.len()
    }
    fn node_status(&mut self, _group: usize, member: usize) -> (bool, bool) {
        self.nodes[member]
    }
    fn clear_dirty(&mut self, _group: usize, member: usize) {
        self.nodes[member].1 = false;
    }
    fn refresh_member(&mut self, _group: usize, member: usize) -> bool {
        self.nodes[member].0 = self.refresh_completes;
        self.refresh_completes
    }
    fn any_member_suffices(&self, _group: usize) -> bool {
        self.any
    }
}

#[test]
fn group_progress_counts_members_and_drains_dirty_nodes() {
    let mut world = Groups {
        nodes: vec![(true, true), (false, false)],
        refresh_completes: false,
        any: false,
    };
    let progress = group_progress(&mut world, 0, true);
    assert_eq!(
        progress,
        GroupProgress {
            completed: 1,
            total: 2,
            finished: false,
            rebuild_point_lists: 1
        }
    );
    assert!(!world.nodes[0].1);
    let mut world = Groups {
        nodes: vec![(false, false), (false, false)],
        refresh_completes: true,
        any: false,
    };
    assert!(group_progress(&mut world, 0, false).finished);
    let mut world = Groups {
        nodes: vec![(true, false), (false, false)],
        refresh_completes: false,
        any: true,
    };
    assert!(group_progress(&mut world, 0, false).finished);
}

mod round4 {
    use tsfp_gameplay::story_events::arcade::*;

    #[test]
    fn mark_helpers_use_the_measured_pairs() {
        let ops = level1b_hook(TableHookCall {
            slot: 0,
            event: EVENT_FINISHED,
            state: 0,
        });
        assert!(ops.contains(&TableHookOp::MarkConnections {
            pairs: &CONNECTIONS_221EC0
        }));
        assert_eq!(CONNECTIONS_221EC0.len() + CONNECTIONS_221F20.len(), 5);
        assert_eq!(CONNECTIONS_207D10[4], (0x563, 0x57D));
        let slot3 = level1b_hook(TableHookCall {
            slot: 3,
            event: EVENT_FINISHED,
            state: 0,
        });
        assert!(slot3.contains(&TableHookOp::MarkConnections {
            pairs: &CONNECTIONS_221F20
        }));
    }

    #[test]
    fn helper_222050_removes_ticks_then_destroys() {
        let object = ObjectRef::SlotLookup {
            slot: 4,
            id: 0xC4,
            sub: 0,
        };
        let ops = level1b_hook(TableHookCall {
            slot: 4,
            event: EVENT_FINISHED,
            state: 0,
        });
        assert_eq!(ops, helper_222050(object));
        assert!(matches!(
            ops[0],
            TableHookOp::SpawnEffect(EffectSpawn {
                kind: 0x21,
                extra: 0x17,
                sound: true,
                ..
            })
        ));
        let states: Vec<u32> = ops
            .iter()
            .filter_map(|op| {
                if let TableHookOp::ClearTick { state, .. } = op {
                    Some(*state)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(states, [3, 2, 10]);
        assert_eq!(ops.last(), Some(&TableHookOp::Destroy(object)));
    }

    #[test]
    fn wall_helper_mirrors_with_the_player() {
        let plain = helper_207d10(false);
        let mirrored = helper_207d10(true);
        let position = |ops: &[TableHookOp]| match ops[0] {
            TableHookOp::SpawnEffect(EffectSpawn {
                position: EffectPosition::Fixed(p),
                velocity: Some(v),
                ..
            }) => (p, v),
            _ => panic!(),
        };
        assert_eq!(position(&plain).0[0], 5.83);
        assert_eq!(position(&mirrored).0[0], -2.17);
        assert_eq!(position(&mirrored).1[0], -0.75);
        assert!(plain.contains(&TableHookOp::ClearEntryWord { index: 0x2A }));
    }

    #[test]
    fn effect_entry_offsets_position_by_a_tenth_of_the_velocity() {
        let (p, v) = effect_entry_vectors([1.0, 2.0, 3.0], Some([10.0, 0.0, -10.0]));
        assert_eq!((p, v), ([2.0, 2.0, 2.0], [10.0, 0.0, -10.0]));
        assert_eq!(
            effect_entry_vectors([1.0, 2.0, 3.0], None),
            ([1.0, 2.0, 3.0], [0.0, 1.0, 0.0])
        );
    }

    #[test]
    fn effect_sounds_follow_the_kind_table() {
        assert_eq!(effect_sounds(0xB, 1.0, 0).len(), 2);
        assert_eq!(effect_sounds(0xF, 1.0, 0), vec![]);
        assert_eq!(
            effect_sounds(0x2D, 0.5, 0),
            vec![EffectSound::Plain {
                id: 0x631,
                ticks: 30
            }]
        );
        assert_eq!(
            effect_sounds(0x21, 1.0, 13),
            vec![EffectSound::Plain {
                id: 0xC33 + 1,
                ticks: 60
            }]
        );
        assert_eq!(
            effect_sounds(0x10, 1.0, 0),
            vec![EffectSound::Ranged {
                id: 0xC33,
                ticks: 60,
                range: 150.0
            }]
        );
    }

    #[test]
    fn proximity_beeper_rate_and_trigger() {
        // Nobody on one side: nothing changes.
        assert_eq!(proximity_beep(&[], &[[0.0; 3]], 0.5, 1.0), (0.5, false));
        // Far apart (distance 50): rate 0.04 per second.
        let (acc, beep) = proximity_beep(&[[0.0, 0.0, 0.0]], &[[50.0, 0.0, 0.0]], 1.0, 1.0);
        assert!(!beep);
        assert!((acc - (1.0 - 0.04 / 60.0)).abs() < 1e-6, "{acc}");
        // Touching: the distance is clamped to 2, rate 1.0, and a drained accumulator beeps and gains 0.1.
        let (acc, beep) = proximity_beep(&[[0.0; 3]], &[[0.0; 3]], 0.0, 1.0);
        assert!(beep);
        assert!((acc - (0.1 - 1.0 / 60.0)).abs() < 1e-6, "{acc}");
    }

    #[test]
    fn type89_pass1_counts_matches_and_skips_flag_2() {
        let entry = |flag2| NetSlotEntry {
            active: true,
            class_key: 7,
            value: 3,
            flag2,
        };
        let other = NetSlotEntry {
            active: true,
            class_key: 8,
            value: 3,
            flag2: false,
        };
        let bits = type89_pass1_bits(&[entry(false), other, entry(true), entry(false)], 7, 3);
        assert_eq!(bits, 0b101);
        assert_eq!(type89_pass1_bits(&[], 7, 3), 0);
    }

    #[test]
    fn group_member_collection_uses_the_kind_predicate() {
        let nodes = [
            ScriptNode { kind: 0, group: 1 },
            ScriptNode { kind: 1, group: 2 },
            ScriptNode { kind: 8, group: 2 },
            ScriptNode { kind: 3, group: 2 },
        ];
        let groups = [
            GroupRecord {
                node: 0,
                members: vec![],
            },
            GroupRecord {
                node: 1,
                members: vec![2, 3],
            },
        ];
        let mut seen = Vec::new();
        let got = collect_group_members(&nodes, &groups, 2, &mut |predicate, node| {
            seen.push((predicate, node));
            node == 3
        });
        assert_eq!(got, Some(vec![3]));
        assert_eq!(seen, [(0x33B5B0, 2), (0x33B5B0, 3)]);
        // A node kind outside the table cannot be evaluated.
        let bad = [ScriptNode { kind: 5, group: 1 }];
        assert_eq!(
            collect_group_members(
                &bad,
                &[GroupRecord {
                    node: 0,
                    members: vec![0]
                }],
                1,
                &mut |_, _| true
            ),
            None
        );
    }

    #[test]
    fn point_lists_split_by_flag_and_cap_at_sixteen() {
        let mut points = vec![
            PointEntry {
                id: 9,
                flags: 0x80,
                group: 2,
            },
            PointEntry {
                id: 10,
                flags: 0x100,
                group: 2,
            },
            PointEntry {
                id: 11,
                flags: 0x100,
                group: 3,
            },
        ];
        assert!(group_has_both_point_kinds(&points, 2));
        assert!(!group_has_both_point_kinds(&points, 3));
        assert_eq!(group_point_short_lists(&points, 2), (vec![9], vec![10]));
        for id in 20..40 {
            points.push(PointEntry {
                id,
                flags: 0x80,
                group: 5,
            });
        }
        assert_eq!(group_point_short_lists(&points, 5).0.len(), 16);
        let setup = group_setup(&points, 3);
        assert_eq!(setup.len(), 3);
        assert_eq!(setup[1].index, 1);
        assert_eq!(setup[1].phases, [0, 1, 2, 3]);
        assert_eq!(setup[1].low_points, vec![9]);
        assert!(setup[2].low_points.is_empty());
    }
}
