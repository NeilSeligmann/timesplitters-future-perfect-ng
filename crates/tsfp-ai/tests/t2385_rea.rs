// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 differentials for T2385's readable REA route/movement boundaries.

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use tsfp_ai::{
    awareness::NodeKey,
    navigation_scoring::NodeId,
    rea_executor::{ExecutorResult, RouteAnimationHost, execute_route_entry},
    rea_movement::{
        LinkFollowDecision, LinkFollowInput, Position3, SecondaryActionEffect, SecondaryMovement,
        approach_xz, dispatch_secondary_action, link_follow_decision,
    },
    rea_routes::{RouteActor, RouteEntry, RouteProgram, RouteWorld, advance_route},
};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    c2r::{ai_pathfinding, objchar_wired7},
    memory::GuestMemory,
};

const STACK: u32 = 0x70000;
const ACTOR: u32 = 0x1000;
const EXT: u32 = 0x2000;
const ROUTE: u32 = 0x4000;

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<HashMap<u32, u8>>>);

impl Memory {
    fn put8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
    }

    fn put32(&mut self, address: u32, value: u32) {
        for (offset, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.put8(address.wrapping_add(offset as u32), byte);
        }
    }

    fn put_f32(&mut self, address: u32, value: f32) {
        self.put32(address, value.to_bits());
    }
}

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.borrow().get(&address).copied().unwrap_or(0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.put8(address, value);
    }
    fn read32(&self, address: u32) -> u32 {
        u32::from_le_bytes(std::array::from_fn(|i| {
            self.read8(address.wrapping_add(i as u32))
        }))
    }
    fn write32(&mut self, address: u32, value: u32) {
        self.put32(address, value);
    }
}

fn registers(esp: u32) -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp,
        ebx: 0,
        ebp: 0,
        esi: 0,
        edi: 0,
        eflags: 0,
        fpu: Fpu {
            stack: [0.0; 8],
            top: 0,
            control: 0,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}

struct EmptyCalls;
impl GuestCall for EmptyCalls {
    fn call(&mut self, address: u32, _: &mut ExtRegisters) {
        panic!("unexpected delegate {address:#x}");
    }
}

struct Seed(u32);
impl Seed {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0
    }
    fn finite(&mut self) -> f32 {
        ((self.next() % 20_001) as i32 - 10_000) as f32 / 113.0
    }
}

#[test]
fn tier1_4096_seeded_xz_approaches_match_lifted_266250() {
    for seed in 0..4096 {
        let mut random = Seed(seed);
        let initial = Position3 {
            x: random.finite(),
            y: random.finite(),
            z: random.finite(),
        };
        let target = Position3 {
            x: random.finite(),
            y: random.finite(),
            z: random.finite(),
        };
        let fraction = (random.next() % 1001) as f32 / 1000.0;
        let mut native = initial;
        approach_xz(&mut native, target, fraction);

        let mut memory = Memory::default();
        memory.put_f32(0x475d1c, 0.01);
        memory.put_f32(ACTOR + 0x64, initial.x);
        memory.put_f32(ACTOR + 0x6c, initial.z);
        memory.put_f32(0x3000 + 0x0c, target.x);
        memory.put_f32(0x3000 + 0x14, target.z);
        memory.put32(STACK + 4, ACTOR);
        memory.put32(STACK + 8, 0x3000);
        memory.put_f32(STACK + 0x0c, fraction);
        let mut regs = registers(STACK);
        objchar_wired7::game_object_nudge_xz_toward_point_when_outside_deadzone(
            &mut memory,
            &mut regs,
        );
        assert_eq!(
            native.x.to_bits(),
            memory.read32(ACTOR + 0x64),
            "seed {seed}"
        );
        assert_eq!(
            native.z.to_bits(),
            memory.read32(ACTOR + 0x6c),
            "seed {seed}"
        );
        assert_eq!(initial.y.to_bits(), native.y.to_bits(), "seed {seed}");
    }
}

#[test]
fn tier1_route_peek_controls_match_lifted_26d690() {
    for cursor in 0..12u32 {
        for (kind, payload_a, expected) in [
            (12, 0, 1),
            (13, cursor, cursor.wrapping_sub(cursor).wrapping_add(1)),
            (7, 0, cursor + 1),
        ] {
            let mut entries = vec![RouteEntry::default(); 20];
            entries[(cursor + 1) as usize] = RouteEntry {
                kind,
                payload_a,
                payload_b: 0,
            };
            let route = RouteProgram { entries, cursor };
            let (selected, _) = route.peek_next();
            assert_eq!(selected, expected);

            let mut memory = Memory::default();
            memory.put32(ACTOR + 0x7c, EXT);
            memory.put32(EXT + 0xaa0, cursor);
            memory.put32(STACK + 4, ACTOR);
            memory.put32(STACK + 8, ROUTE);
            memory.put32(ROUTE + (cursor + 1) * 12, kind);
            memory.put32(ROUTE + (cursor + 1) * 12 + 4, payload_a);
            let mut regs = registers(STACK);
            ai_pathfinding::game_route_step_list_entry_ptr_with_wrap_codes(&mut memory, &mut regs);
            assert_eq!(
                regs.eax,
                ROUTE + expected * 12,
                "cursor={cursor} kind={kind}"
            );
        }
    }
}

struct RouteHost;
impl RouteWorld for RouteHost {
    fn resolve_key(&mut self, key: NodeKey) -> Option<NodeId> {
        Some(NodeId(key.0 as u32 + 100))
    }
    fn player_current_node(&mut self) -> NodeId {
        NodeId(999)
    }
    fn find_intermediate(&mut self, _: NodeId, _: NodeId) -> i32 {
        panic!("intermediate lookup outside this differential case")
    }
    fn abort_route(&mut self) {
        panic!("abort outside this differential case")
    }
}

#[test]
fn tier1_route_advance_controls_match_lifted_26de30() {
    for (control, payload_a, expected_index, expected_kind) in
        [(12, 50, 1, 12), (13, 0, 1, 13), (18, 0, 2, 4), (4, 0, 1, 4)]
    {
        let entries = vec![
            RouteEntry::default(),
            RouteEntry {
                kind: control,
                payload_a,
                payload_b: 500,
            },
            RouteEntry {
                kind: 4,
                payload_a: 70,
                payload_b: 700,
            },
            RouteEntry::default(),
        ];
        let mut route = RouteProgram { entries, cursor: 0 };
        let mut actor = RouteActor {
            current_node: NodeId(3),
            goal_node: NodeId(4),
            reverse_trace_within: 8,
            reverse_trace_other: 9,
            ..RouteActor::default()
        };
        let selected = advance_route(&mut actor, &mut route, &mut RouteHost);

        let mut memory = Memory::default();
        memory.put32(ACTOR + 0x7c, EXT);
        memory.put32(EXT + 0xaa0, 0);
        memory.put32(EXT + 0x678, 3);
        memory.put32(EXT + 0x684, 4);
        memory.put32(EXT + 0x9a8, 8);
        memory.put32(EXT + 0x9ac, 9);
        memory.put32(STACK + 4, ACTOR);
        memory.put32(STACK + 8, ROUTE);
        for (index, entry) in [
            RouteEntry::default(),
            RouteEntry {
                kind: control,
                payload_a,
                payload_b: 500,
            },
            RouteEntry {
                kind: 4,
                payload_a: 70,
                payload_b: 700,
            },
            RouteEntry::default(),
        ]
        .into_iter()
        .enumerate()
        {
            let base = ROUTE + index as u32 * 12;
            memory.put32(base, entry.kind);
            memory.put32(base + 4, entry.payload_a);
            memory.put32(base + 8, entry.payload_b);
        }
        let mut regs = registers(STACK);
        let mut calls = KeyCalls(memory.clone());
        ai_pathfinding::game_actor_advance_route_step_and_resolve_target_node(
            &mut memory,
            &mut calls,
            &mut regs,
        );
        assert_eq!(
            route.cursor,
            memory.read32(EXT + 0xaa0),
            "control={control}"
        );
        assert_eq!(
            actor.goal_node.0,
            memory.read32(EXT + 0x684),
            "control={control}"
        );
        assert_eq!(actor.reverse_trace_within, memory.read32(EXT + 0x9a8));
        assert_eq!(actor.reverse_trace_other, memory.read32(EXT + 0x9ac));
        assert_eq!(route.cursor, expected_index);
        assert_eq!(selected.kind, expected_kind);
    }
}

struct KeyCalls(Memory);
impl GuestCall for KeyCalls {
    fn call(&mut self, address: u32, regs: &mut ExtRegisters) {
        assert_eq!(address, 0x9cc40);
        // The caller has already placed the key at the current guest stack.
        // This test's valid keys map to key + 100, matching RouteHost.
        regs.eax = self.0.read32(regs.esp).wrapping_add(100);
    }
}

#[derive(Default)]
struct ExecutorHost {
    requests: Vec<u32>,
    matches: HashSet<u32>,
    route_matches: bool,
    end_matches: bool,
    finished: bool,
    frame_event: bool,
    primary_matches: HashSet<u32>,
    intermediate_angle: f32,
    random_requests: usize,
    turned_keys: Vec<u32>,
}
impl RouteWorld for ExecutorHost {
    fn resolve_key(&mut self, _: NodeKey) -> Option<NodeId> {
        None
    }
    fn player_current_node(&mut self) -> NodeId {
        NodeId(0)
    }
    fn find_intermediate(&mut self, _: NodeId, _: NodeId) -> i32 {
        -1
    }
    fn abort_route(&mut self) {}
}
impl RouteAnimationHost for ExecutorHost {
    fn actor_has_either_animation(&mut self, animation: u32) -> bool {
        self.matches.contains(&animation)
    }
    fn actor_has_primary_animation(&mut self, animation: u32) -> bool {
        self.primary_matches.contains(&animation)
    }
    fn route_animation_matches(&mut self) -> bool {
        self.route_matches
    }
    fn end_animation_matches(&mut self) -> bool {
        self.end_matches
    }
    fn animation_finished(&mut self) -> bool {
        self.finished
    }
    fn frame_event_crossed(&mut self, _: u32) -> bool {
        self.frame_event
    }
    fn request_animation(&mut self, animation: u32) {
        self.requests.push(animation);
    }
    fn request_random_route_animation(&mut self, _: &RouteProgram) {
        self.random_requests += 1;
    }
    fn set_heading_from_current_node(&mut self) {}
    fn turn_toward_key(&mut self, key: u32) {
        self.turned_keys.push(key);
    }
    fn angle_to_intermediate(&mut self, _: NodeId) -> f32 {
        self.intermediate_angle
    }
    fn key_for_primary_node(&mut self, node: NodeId) -> NodeKey {
        NodeKey(node.0 as i32)
    }
}

struct ExecutorCalls(Memory, Vec<u32>);
impl GuestCall for ExecutorCalls {
    fn call(&mut self, address: u32, regs: &mut ExtRegisters) {
        match address {
            0x59d20 | 0x59dd0 => regs.eax = 0,
            0x5f460 => {
                self.1.push(self.0.read32(regs.esp + 4));
                regs.eax = 0;
            }
            other => panic!("unexpected executor delegate {other:#x}"),
        }
    }
}

struct ConfiguredExecutorCalls {
    memory: Memory,
    requests: Vec<u32>,
    matches: HashSet<u32>,
    finished: bool,
}
impl GuestCall for ConfiguredExecutorCalls {
    fn call(&mut self, address: u32, regs: &mut ExtRegisters) {
        match address {
            0x59d20 => {
                regs.eax = if self.matches.contains(&self.memory.read32(regs.esp + 4)) {
                    1
                } else {
                    0
                }
            }
            0x59dd0 => regs.eax = if self.finished { 1 } else { 0 },
            0x5f460 => self.requests.push(self.memory.read32(regs.esp + 4)),
            other => panic!("unexpected configured executor delegate {other:#x}"),
        }
    }
}

fn route_executor_fixture(kind: u32) -> (Memory, ExtRegisters) {
    let mut memory = Memory::default();
    memory.put32(ACTOR + 0x7c, EXT);
    memory.put32(EXT + 0xaa0, 0);
    memory.put32(EXT + 0x28, 0);
    memory.put32(ROUTE, kind);
    // The lifted path dereferences the animation pair tables before checking
    // the configured delegate response and before issuing a request.
    memory.put32(0x5202b8, 0x5202b8);
    memory.put32(0x5202bc, 0x5202b8);
    memory.put32(0x5202c8, 0x5202c8);
    memory.put32(0x5202cc, 0x5202c8);
    // The static dispatch facet for the kind-1 entry used by this differential.
    memory.put32(0x26e9a8, 0x26e1f5);
    memory.put32(0x26e9b0, 0x26e223);
    memory.put8(0x26e9c0, 2);
    let mut regs = registers(STACK);
    regs.eax = ACTOR;
    regs.ebx = ROUTE;
    (memory, regs)
}

#[test]
fn tier1_route_executor_kind_one_animation_request_matches_lifted_26e150() {
    let mut route = RouteProgram {
        entries: vec![RouteEntry {
            kind: 1,
            payload_a: 0,
            payload_b: 0,
        }],
        cursor: 0,
    };
    let mut native_host = ExecutorHost::default();
    let result = execute_route_entry(0, &mut RouteActor::default(), &mut route, &mut native_host);

    let mut memory = Memory::default();
    memory.put32(ACTOR + 0x7c, EXT);
    memory.put32(EXT + 0xaa0, 0);
    memory.put32(EXT + 0x28, 0);
    memory.put32(ROUTE, 1);
    memory.put32(0x5202b8, 0x5202b8);
    memory.put32(0x5202bc, 0x5202b8);
    memory.put32(0x5202c8, 0x5202c8);
    memory.put32(0x5202cc, 0x5202c8);
    memory.put8(0x26e9c0, 2);
    memory.put32(0x26e9b0, 0x26e223);
    memory.put32(0x26e9a8, 0x26e1f5);
    memory.put32(STACK + 4, ACTOR);
    let mut regs = registers(STACK);
    regs.eax = ACTOR;
    regs.ebx = ROUTE;
    let mut calls = ExecutorCalls(memory.clone(), Vec::new());
    ai_pathfinding::game_actor_route_step_executor_dispatch_by_step_kind_and_start_weapon_anims(
        &mut memory,
        &mut calls,
        &mut regs,
    );
    assert_eq!(result, ExecutorResult::Requested(0x5202b8));
    assert_eq!(native_host.requests, calls.1);
    assert_eq!(native_host.requests, [0x5202b8]);
}

#[test]
fn tier1_route_executor_reset_and_default_match_lifted_26e150() {
    for kind in [0, 10, 11, 12, 13, 14, 15, 18, 0xffff_ffff] {
        let mut route = RouteProgram {
            entries: vec![
                RouteEntry {
                    kind,
                    payload_a: 0,
                    payload_b: 0,
                },
                RouteEntry::default(),
            ],
            cursor: 0,
        };
        let native = execute_route_entry(
            0,
            &mut RouteActor::default(),
            &mut route,
            &mut ExecutorHost::default(),
        );

        let mut memory = Memory::default();
        memory.put32(ACTOR + 0x7c, EXT);
        memory.put32(EXT + 0xaa0, 0);
        memory.put32(EXT + 0x28, 0);
        memory.put32(ROUTE, kind);
        memory.put32(0x26e9a8, 0x26e1f5);
        memory.put32(0x26e9ac, 0x26e212);
        memory.put8(0x26e9c0 + 11, 1);
        let mut regs = registers(STACK);
        regs.eax = ACTOR;
        regs.ebx = ROUTE;
        ai_pathfinding::game_actor_route_step_executor_dispatch_by_step_kind_and_start_weapon_anims(
            &mut memory,
            &mut EmptyCalls,
            &mut regs,
        );
        assert_eq!(route.cursor, memory.read32(EXT + 0xaa0), "kind={kind}");
        let expected = if kind == 12 {
            ExecutorResult::ResetToFirst
        } else {
            ExecutorResult::DefaultReturn
        };
        assert_eq!(native, expected, "kind={kind}");
    }
}

#[test]
fn tier1_route_executor_kind_one_guards_match_lifted_26e150() {
    let cases = [
        (HashSet::from([0x5202c8]), false, ExecutorResult::Held),
        (
            HashSet::from([0x5202c8]),
            true,
            ExecutorResult::Requested(0x5202b8),
        ),
        (HashSet::from([0x5202b8]), false, ExecutorResult::Held),
        (HashSet::new(), false, ExecutorResult::Requested(0x5202b8)),
    ];
    for (matches, finished, expected) in cases {
        let mut native_host = ExecutorHost {
            matches: matches.clone(),
            finished,
            ..ExecutorHost::default()
        };
        let native = execute_route_entry(
            0,
            &mut RouteActor::default(),
            &mut RouteProgram {
                entries: vec![RouteEntry {
                    kind: 1,
                    payload_a: 0,
                    payload_b: 0,
                }],
                cursor: 0,
            },
            &mut native_host,
        );

        let (mut memory, mut regs) = route_executor_fixture(1);
        let mut oracle = ConfiguredExecutorCalls {
            memory: memory.clone(),
            requests: Vec::new(),
            matches,
            finished,
        };
        ai_pathfinding::game_actor_route_step_executor_dispatch_by_step_kind_and_start_weapon_anims(
            &mut memory,
            &mut oracle,
            &mut regs,
        );
        assert_eq!(native, expected);
        let expected_requests = match expected {
            ExecutorResult::Requested(animation) => vec![animation],
            _ => Vec::new(),
        };
        assert_eq!(oracle.requests, expected_requests);
        assert_eq!(native_host.requests, oracle.requests);
    }
}

#[test]
fn tier0_route_executor_grouped_contract_paths() {
    for (kind, animation) in [(5, 0x520318), (6, 0x520328), (7, 0x520338)] {
        let mut route = RouteProgram {
            entries: vec![RouteEntry {
                kind,
                payload_a: 0,
                payload_b: 0,
            }],
            cursor: 0,
        };
        let mut host = ExecutorHost {
            route_matches: true,
            finished: true,
            ..ExecutorHost::default()
        };
        assert_eq!(
            execute_route_entry(0, &mut RouteActor::default(), &mut route, &mut host),
            ExecutorResult::Requested(animation)
        );
        assert_eq!(host.requests, [animation]);
        assert_eq!(route.cursor, 0);
    }

    let mut actor = RouteActor {
        current_node: NodeId(8),
        reverse_trace_within: 3,
        reverse_trace_other: 4,
        ..RouteActor::default()
    };
    let mut route = RouteProgram {
        entries: vec![
            RouteEntry {
                kind: 8,
                payload_a: 1,
                payload_b: 2,
            },
            RouteEntry {
                kind: 1,
                payload_a: 0,
                payload_b: 77,
            },
        ],
        cursor: 0,
    };
    let mut host = ExecutorHost {
        route_matches: true,
        finished: true,
        ..ExecutorHost::default()
    };
    assert_eq!(
        execute_route_entry(0, &mut actor, &mut route, &mut host),
        ExecutorResult::Requested(0x5202c8)
    );
    assert_eq!(host.requests, [0x5202c8]);
    assert_eq!(route.cursor, 0);

    route.entries[1] = RouteEntry {
        kind: 3,
        payload_a: 23,
        payload_b: 0,
    };
    host = ExecutorHost {
        route_matches: true,
        finished: true,
        ..ExecutorHost::default()
    };
    assert_eq!(
        execute_route_entry(0, &mut actor, &mut route, &mut host),
        ExecutorResult::AdvancedThenRandom
    );
    assert_eq!(route.cursor, 1);
    assert_eq!(actor.goal_node, NodeId(0));
    assert_eq!(actor.reverse_trace_within, 0);
    assert_eq!(actor.reverse_trace_other, 0);
    assert_eq!(host.random_requests, 1);
}

#[test]
fn tier0_turn_route_executor_contract_paths() {
    for (kind, animation) in [(2, 0x5202f8), (3, 0x520308)] {
        let mut host = ExecutorHost {
            frame_event: true,
            ..ExecutorHost::default()
        };
        let result = execute_route_entry(
            0,
            &mut RouteActor::default(),
            &mut RouteProgram {
                entries: vec![RouteEntry {
                    kind,
                    payload_a: 0,
                    payload_b: 0,
                }],
                cursor: 0,
            },
            &mut host,
        );
        assert_eq!(result, ExecutorResult::Requested(animation));
        assert_eq!(host.requests, [animation]);
    }

    let mut actor = RouteActor {
        intermediate_node: Some(NodeId(44)),
        ..RouteActor::default()
    };
    let mut route = RouteProgram {
        entries: vec![
            RouteEntry {
                kind: 16,
                payload_a: 0,
                payload_b: 0,
            },
            RouteEntry {
                kind: 4,
                payload_a: 23,
                payload_b: 0,
            },
        ],
        cursor: 0,
    };
    let mut host = ExecutorHost {
        frame_event: true,
        intermediate_angle: 60.0,
        ..ExecutorHost::default()
    };
    assert_eq!(
        execute_route_entry(0, &mut actor, &mut route, &mut host),
        ExecutorResult::Requested(0x5202f8)
    );
    host.intermediate_angle = -60.0;
    assert_eq!(
        execute_route_entry(0, &mut actor, &mut route, &mut host),
        ExecutorResult::Requested(0x520308)
    );
    host.intermediate_angle = 140.0;
    assert_eq!(
        execute_route_entry(0, &mut actor, &mut route, &mut host),
        ExecutorResult::Requested(0x5202e8)
    );
    host.intermediate_angle = 20.0;
    assert_eq!(
        execute_route_entry(0, &mut actor, &mut route, &mut host),
        ExecutorResult::Advanced
    );
    assert_eq!(route.cursor, 1);
    assert_eq!(actor.goal_node, NodeId(0));

    let mut route = RouteProgram {
        entries: vec![
            RouteEntry {
                kind: 2,
                payload_a: 0,
                payload_b: 0,
            },
            RouteEntry {
                kind: 1,
                payload_a: 0,
                payload_b: 0x7654,
            },
        ],
        cursor: 0,
    };
    let mut host = ExecutorHost {
        primary_matches: HashSet::from([0x520308]),
        ..ExecutorHost::default()
    };
    assert_eq!(
        execute_route_entry(0, &mut actor, &mut route, &mut host),
        ExecutorResult::TurnToward(0x7654)
    );
    assert_eq!(host.turned_keys, [0x7654]);

    for (angle, animation) in [(20.0, 0x5202c8), (60.0, 0x520328), (-60.0, 0x520338)] {
        let mut host = ExecutorHost {
            route_matches: true,
            finished: true,
            intermediate_angle: angle,
            ..ExecutorHost::default()
        };
        let mut actor = RouteActor {
            intermediate_node: Some(NodeId(44)),
            ..RouteActor::default()
        };
        let mut route = RouteProgram {
            entries: vec![RouteEntry {
                kind: 17,
                payload_a: 0,
                payload_b: 0,
            }],
            cursor: 0,
        };
        assert_eq!(
            execute_route_entry(0, &mut actor, &mut route, &mut host),
            ExecutorResult::Requested(animation)
        );
        assert_eq!(host.requests, [animation]);
    }
}

struct LinkCalls(bool);
impl GuestCall for LinkCalls {
    fn call(&mut self, address: u32, regs: &mut ExtRegisters) {
        assert_eq!(address, 0x59d20);
        regs.eax = if self.0 { 1 } else { 0 };
    }
}

#[test]
fn tier1_other_kind_link_targets_match_lifted_263ed0() {
    for animation_matches in [false, true] {
        let actor_position = Position3 {
            x: 1.25,
            y: -3.5,
            z: 7.75,
        };
        let goal_position = Position3 {
            x: -9.0,
            y: 2.0,
            z: 0.375,
        };
        let input = LinkFollowInput {
            route_kind: 2,
            behavior_route_selector: 4,
            animation_matches,
            actor_position,
            goal_position,
            root_first_z: 0.0,
            root_last_z: 0.0,
        };
        let native = link_follow_decision(input);

        let mut memory = Memory::default();
        memory.put32(ACTOR + 0x7c, EXT);
        memory.put32(EXT + 0xbf0, 0x3000);
        memory.put32(0x3000 + 0x68, 4);
        memory.put32(EXT + 0x684, 3);
        memory.put32(0x715f2c, 0x5000);
        memory.put_f32(ACTOR + 0x64, actor_position.x);
        memory.put_f32(ACTOR + 0x68, actor_position.y);
        memory.put_f32(ACTOR + 0x6c, actor_position.z);
        let node = 0x5000 + 3 * 32;
        memory.put_f32(node + 0x0c, goal_position.x);
        memory.put_f32(node + 0x10, goal_position.y);
        memory.put_f32(node + 0x14, goal_position.z);
        memory.put32(ROUTE, 2);
        let mut regs = registers(STACK);
        regs.edi = ACTOR;
        regs.ebx = ROUTE;
        ai_pathfinding::game_char_class_tick_route_step_decode_two_key_transforms_and_advance_nav_link(
            &mut memory,
            &mut LinkCalls(animation_matches),
            &mut regs,
        );

        let expected = if animation_matches {
            goal_position
        } else {
            actor_position
        };
        assert_eq!(
            native,
            if animation_matches {
                LinkFollowDecision::TargetGoalPosition
            } else {
                LinkFollowDecision::TargetActorPosition
            }
        );
        assert_eq!(memory.read32(EXT + 0x30), expected.x.to_bits());
        assert_eq!(memory.read32(EXT + 0x34), expected.y.to_bits());
        assert_eq!(memory.read32(EXT + 0x38), expected.z.to_bits());
    }
}

#[test]
fn documented_secondary_action_map_and_link_follow_gates() {
    let expected = [
        (0, 3, None, false),
        (1, 4, Some(0x520970), true),
        (2, 5, Some(0x520990), true),
        (3, 6, Some(0x520968), false),
        (4, 7, Some(0x5209a0), false),
        (5, 8, Some(0x5209b0), true),
        (6, 9, Some(0x5209d0), true),
        (7, 10, Some(0x5209e0), false),
        (8, 11, None, false),
        (9, 11, None, false),
    ];
    for (action, state, pair, clears_timers) in expected {
        let mut movement = SecondaryMovement {
            local_state: 2,
            elapsed: 5.0,
            fire_deadline: 6.0,
            movement_timer_b34: 7.0,
            direction: -1,
            ..SecondaryMovement::default()
        };
        let effect = dispatch_secondary_action(&mut movement, action);
        assert_eq!(movement.local_state, state);
        assert_eq!(movement.previous_state, 2);
        assert_eq!(movement.queued_state, Some(state));
        assert_eq!(movement.elapsed, 0.0);
        assert_eq!(movement.fire_deadline == 0.0, clears_timers);
        assert_eq!(movement.movement_timer_b34 == 0.0, clears_timers);
        match (pair, action) {
            (Some(pair), _) => {
                assert_eq!(
                    effect,
                    SecondaryActionEffect::RequestDirectionPair {
                        pair,
                        direction: -1
                    }
                )
            }
            (None, 0) => assert_eq!(effect, SecondaryActionEffect::None),
            (None, action @ (8 | 9)) => assert_eq!(
                effect,
                SecondaryActionEffect::ClaimAndSelectSecondary {
                    action,
                    angle_code: 0
                },
            ),
            _ => unreachable!(),
        }
    }
    let mut invalid = SecondaryMovement::default();
    assert_eq!(
        dispatch_secondary_action(&mut invalid, 10),
        SecondaryActionEffect::RequestBehaviorMask4
    );

    let base = LinkFollowInput {
        route_kind: 1,
        behavior_route_selector: 0,
        animation_matches: false,
        actor_position: Position3 {
            x: 0.0,
            y: 2.0,
            z: 0.0,
        },
        goal_position: Position3 {
            x: 3.0,
            y: 5.0,
            z: 4.0,
        },
        root_first_z: 0.0,
        root_last_z: 5.0,
    };
    assert_eq!(
        link_follow_decision(base),
        LinkFollowDecision::AskNavigationToAdvance
    );
    assert_eq!(
        link_follow_decision(LinkFollowInput {
            root_last_z: 6.0,
            ..base
        }),
        LinkFollowDecision::AdvanceCurrentToGoal
    );
    assert_eq!(
        link_follow_decision(LinkFollowInput {
            behavior_route_selector: -2,
            ..base
        }),
        LinkFollowDecision::Return
    );
}
