// SPDX-License-Identifier: GPL-3.0-or-later
//! Tier 1 differentials for T2395's bounded mechanism and link branches.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use tsfp_ai::{
    navigation_scoring::{ActorId, NodeId, ObjectId},
    rea_links::{
        LinkActorState, LinkFollowResult, LinkWorld, RootEndpointSamples, follow_route_link,
    },
    rea_mechanism::{
        ActorInteractionState, BehaviorRequest, MechanismHandle, MechanismPool, MechanismRecord,
        MechanismTickResult, MechanismView, MechanismWorld, SpeedRampInputs, mechanism_speed_ramp,
        tick_selected_state,
    },
    rea_movement::Position3,
    rea_routes::{RouteActor, RouteEntry, RouteProgram, RouteWorld},
};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    c2r::{ai_pathfinding, engine_object_3, t1892_pathfinding},
    memory::GuestMemory,
};

const STACK: u32 = 0x70000;
const ACTOR: u32 = 0x1000;
const EXT: u32 = 0x2000;
const BEHAVIOR: u32 = 0x3000;
const ROUTE: u32 = 0x4000;
const MECHANISM: u32 = 0x5000;
const SUBRECORD: u32 = 0x6000;

#[derive(Clone, Default)]
struct Memory(Rc<RefCell<HashMap<u32, u8>>>);

impl Memory {
    fn put8(&mut self, address: u32, value: u8) {
        self.0.borrow_mut().insert(address, value);
    }
    fn put16(&mut self, address: u32, value: u16) {
        for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.put8(address.wrapping_add(i as u32), byte);
        }
    }
    fn put32(&mut self, address: u32, value: u32) {
        for (i, byte) in value.to_le_bytes().into_iter().enumerate() {
            self.put8(address.wrapping_add(i as u32), byte);
        }
    }
    fn put_f32(&mut self, address: u32, value: f32) {
        self.put32(address, value.to_bits());
    }
    fn get32(&self, address: u32) -> u32 {
        u32::from_le_bytes(std::array::from_fn(|i| self.read8(address + i as u32)))
    }
}

impl GuestMemory for Memory {
    fn read8(&self, address: u32) -> u8 {
        self.0.borrow().get(&address).copied().unwrap_or(0)
    }
    fn write8(&mut self, address: u32, value: u8) {
        self.put8(address, value);
    }
    fn read16(&self, address: u32) -> u16 {
        u16::from_le_bytes([self.read8(address), self.read8(address + 1)])
    }
    fn read32(&self, address: u32) -> u32 {
        self.get32(address)
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

#[derive(Clone, Debug, PartialEq, Eq)]
enum CallEvent {
    Navigate,
    AdvanceRoute,
    Behavior(u32),
    Refresh,
    Prepare,
}

struct LinkCalls {
    memory: Memory,
    animation: bool,
    first_z: f32,
    last_z: f32,
    events: Vec<CallEvent>,
}

impl GuestCall for LinkCalls {
    fn call(&mut self, address: u32, regs: &mut ExtRegisters) {
        match address {
            0x59d20 => regs.eax = u32::from(self.animation),
            0x59320 => {
                let out = self.memory.get32(regs.esp + 12);
                let sample_z = if self.memory.get32(regs.esp + 8) == 0 {
                    self.first_z
                } else {
                    self.last_z
                };
                let mut mem = self.memory.clone();
                mem.put_f32(out + 8, sample_z);
            }
            0x15d500 => self.events.push(CallEvent::Navigate),
            0x26de30 => self.events.push(CallEvent::AdvanceRoute),
            other => panic!("unexpected link delegate {other:#x}"),
        }
    }
}

#[derive(Default)]
struct LinkHost {
    root: RootEndpointSamples,
    animation: bool,
    node_position: Position3,
    events: Vec<CallEvent>,
}

impl RouteWorld for LinkHost {
    fn resolve_key(&mut self, key: tsfp_ai::awareness::NodeKey) -> Option<NodeId> {
        Some(NodeId(key.0 as u32 + 100))
    }
    fn player_current_node(&mut self) -> NodeId {
        NodeId(999)
    }
    fn find_intermediate(&mut self, _: NodeId, _: NodeId) -> i32 {
        -1
    }
    fn abort_route(&mut self) {}
}

impl LinkWorld for LinkHost {
    fn animation_matches(&mut self, _: &LinkActorState) -> bool {
        self.animation
    }
    fn sample_root_endpoints(&mut self, _: &LinkActorState) -> RootEndpointSamples {
        self.root
    }
    fn primary_position(&mut self, _: NodeId) -> Position3 {
        self.node_position
    }
    fn advance_navigation_link(&mut self, _: &mut LinkActorState, _: NodeId) {
        self.events.push(CallEvent::Navigate);
    }
}

fn link_fixture(kind: u32, selector: i32, actor_position: Position3, goal: Position3) -> Memory {
    let mut memory = Memory::default();
    memory.put32(ACTOR + 0x7c, EXT);
    memory.put32(EXT + 0xbf0, BEHAVIOR);
    memory.put32(BEHAVIOR + 0x68, selector as u32);
    memory.put32(EXT + 0x684, 3);
    memory.put32(EXT + 0x678, 7);
    memory.put32(0x715f2c, 0x50000);
    let node = 0x50000 + 3 * 32;
    memory.put_f32(node + 0x0c, goal.x);
    memory.put_f32(node + 0x10, goal.y);
    memory.put_f32(node + 0x14, goal.z);
    memory.put_f32(ACTOR + 0x64, actor_position.x);
    memory.put_f32(ACTOR + 0x68, actor_position.y);
    memory.put_f32(ACTOR + 0x6c, actor_position.z);
    memory.put32(ROUTE, kind);
    memory.put32(EXT + 0xaa0, 0);
    memory.put16(0x7e1808, 0);
    memory.put32(0x7e3954, 0x80000);
    memory.put32(0x80000, 0x90000);
    memory.put32(0x90000 + 0x10, 2);
    memory.put32(STACK + 4, ACTOR);
    memory.put32(STACK + 8, ROUTE);
    memory
}

#[test]
fn tier1_link_route_gates_match_lifted_263ed0() {
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
    for (kind, selector, animation, first_z, last_z, expected) in [
        (0, 1, false, 0.0, 0.0, LinkFollowResult::AdvancedRoute),
        (
            1,
            1,
            false,
            0.0,
            5.0,
            LinkFollowResult::AskedNavigationToAdvance,
        ),
        (1, 1, false, 0.0, 15.0, LinkFollowResult::AdvancedRoute),
        (2, 1, false, 0.0, 0.0, LinkFollowResult::TargetedActor),
        (2, 1, true, 0.0, 0.0, LinkFollowResult::TargetedGoal),
    ] {
        let mut memory = link_fixture(kind, selector, actor_position, goal_position);
        let mut regs = registers(STACK);
        regs.edi = ACTOR;
        regs.ebx = ROUTE;
        let mut oracle = LinkCalls {
            memory: memory.clone(),
            animation,
            first_z,
            last_z,
            events: Vec::new(),
        };
        ai_pathfinding::game_char_class_tick_route_step_decode_two_key_transforms_and_advance_nav_link(
            &mut memory, &mut oracle, &mut regs,
        );

        let mut native = LinkHost {
            root: RootEndpointSamples { first_z, last_z },
            animation,
            node_position: goal_position,
            ..LinkHost::default()
        };
        let mut link_actor = LinkActorState {
            position: actor_position,
            current_node: NodeId(7),
            goal_node: NodeId(3),
            movement_target: Position3::default(),
        };
        let mut route_actor = RouteActor {
            current_node: NodeId(7),
            goal_node: NodeId(3),
            ..RouteActor::default()
        };
        let mut route = RouteProgram {
            entries: vec![
                RouteEntry {
                    kind,
                    payload_a: 0,
                    payload_b: 0,
                },
                RouteEntry {
                    kind: 4,
                    payload_a: 12,
                    payload_b: 13,
                },
            ],
            cursor: 0,
        };
        if kind == 1 && last_z - first_z > 5.0 {
            route.entries[1] = RouteEntry {
                kind: 1,
                payload_a: 12,
                payload_b: 13,
            };
        }
        let got = follow_route_link(
            selector,
            &mut link_actor,
            &mut route_actor,
            &mut route,
            &mut native,
        );
        assert_eq!(
            got, expected,
            "kind={kind} selector={selector} samples={first_z},{last_z}"
        );
        if kind == 1 && last_z - first_z <= 5.0 {
            assert_eq!(oracle.events, [CallEvent::Navigate]);
            assert_eq!(native.events, [CallEvent::Navigate]);
        } else if kind == 0 || (kind == 1 && last_z - first_z > 5.0) {
            assert_eq!(oracle.events, [CallEvent::AdvanceRoute]);
        } else {
            let target = if animation {
                goal_position
            } else {
                actor_position
            };
            assert_eq!(memory.get32(EXT + 0x30), target.x.to_bits());
            assert_eq!(memory.get32(EXT + 0x34), target.y.to_bits());
            assert_eq!(memory.get32(EXT + 0x38), target.z.to_bits());
            assert_eq!(link_actor.movement_target, target);
        }
        if kind == 1 && last_z - first_z > 5.0 {
            assert_eq!(memory.get32(EXT + 0x678), 3);
            assert_eq!(link_actor.current_node, NodeId(3));
        }
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
fn tier1_2048_seeded_kind_one_link_decisions_match_lifted_263ed0() {
    use tsfp_ai::rea_movement::{LinkFollowDecision, LinkFollowInput, link_follow_decision};

    for seed in 0..2048 {
        let mut random = Seed(seed);
        let actor_position = Position3 {
            x: random.finite(),
            y: random.finite(),
            z: random.finite(),
        };
        let goal_position = Position3 {
            x: random.finite(),
            y: random.finite(),
            z: random.finite(),
        };
        let first_z = random.finite();
        let last_z = random.finite();
        let native = link_follow_decision(LinkFollowInput {
            route_kind: 1,
            behavior_route_selector: 1,
            animation_matches: false,
            actor_position,
            goal_position,
            root_first_z: first_z,
            root_last_z: last_z,
        });

        let mut memory = link_fixture(1, 1, actor_position, goal_position);
        let mut regs = registers(STACK);
        regs.edi = ACTOR;
        regs.ebx = ROUTE;
        let mut oracle = LinkCalls {
            memory: memory.clone(),
            animation: false,
            first_z,
            last_z,
            events: Vec::new(),
        };
        ai_pathfinding::game_char_class_tick_route_step_decode_two_key_transforms_and_advance_nav_link(
            &mut memory,
            &mut oracle,
            &mut regs,
        );

        match native {
            LinkFollowDecision::AskNavigationToAdvance => {
                assert_eq!(oracle.events, [CallEvent::Navigate], "seed {seed}");
            }
            LinkFollowDecision::AdvanceCurrentToGoal => {
                assert_eq!(oracle.events, [CallEvent::AdvanceRoute], "seed {seed}");
                assert_eq!(memory.get32(EXT + 0x678), 3, "seed {seed}");
            }
            other => panic!("unexpected kind-one decision {other:?}, seed {seed}"),
        }
    }

    for (first_z, last_z) in [(f32::NAN, 2.0), (0.0, f32::NAN)] {
        let actor_position = Position3::default();
        let goal_position = Position3 {
            x: 1.0,
            y: 0.0,
            z: 0.0,
        };
        let native = link_follow_decision(LinkFollowInput {
            route_kind: 1,
            behavior_route_selector: 1,
            animation_matches: false,
            actor_position,
            goal_position,
            root_first_z: first_z,
            root_last_z: last_z,
        });
        let mut memory = link_fixture(1, 1, actor_position, goal_position);
        let mut regs = registers(STACK);
        regs.edi = ACTOR;
        regs.ebx = ROUTE;
        let mut oracle = LinkCalls {
            memory: memory.clone(),
            animation: false,
            first_z,
            last_z,
            events: Vec::new(),
        };
        ai_pathfinding::game_char_class_tick_route_step_decode_two_key_transforms_and_advance_nav_link(
            &mut memory,
            &mut oracle,
            &mut regs,
        );
        assert_eq!(native, LinkFollowDecision::AskNavigationToAdvance);
        assert_eq!(oracle.events, [CallEvent::Navigate]);
    }
}

struct MechanismHost {
    result: i32,
    class: u32,
    choose_eight: bool,
    count: i32,
    node: NodeId,
    cell: i16,
    same_extension: bool,
    target: Option<ObjectId>,
    ready: bool,
    exit_bit: bool,
    link_field: Option<u32>,
    ramp: SpeedRampInputs,
    args: Vec<u32>,
    events: Vec<CallEvent>,
}

impl Default for MechanismHost {
    fn default() -> Self {
        Self {
            result: 0,
            class: 0,
            choose_eight: false,
            count: 0,
            node: NodeId(0),
            cell: 0,
            same_extension: false,
            target: None,
            ready: false,
            exit_bit: false,
            link_field: None,
            ramp: SpeedRampInputs::default(),
            args: Vec::new(),
            events: Vec::new(),
        }
    }
}

impl MechanismWorld for MechanismHost {
    fn interact(&mut self, _: &MechanismView<'_>, _: ActorId, _: u32, argument: f32) -> i32 {
        self.args.push(argument.to_bits());
        self.result
    }
    fn prepare_interaction(&mut self) {
        self.events.push(CallEvent::Prepare);
    }
    fn actor_predicate(&mut self, _: ActorId) -> bool {
        self.choose_eight
    }
    fn current_target(&mut self) -> Option<ObjectId> {
        self.target
    }
    fn mechanism_ready(&mut self, _: ObjectId) -> bool {
        self.ready
    }
    fn mechanism_begin(&mut self, _: ObjectId) {}
    fn mechanism_exit_bit(&mut self, _: ObjectId) -> bool {
        self.exit_bit
    }
    fn mechanism_link_field(&mut self, _: ObjectId) -> Option<u32> {
        self.link_field
    }
    fn speed_ramp_inputs(&mut self, _: ObjectId) -> SpeedRampInputs {
        self.ramp
    }
    fn frame_step(&mut self) -> f32 {
        1.0f32 / 60.0 * 1.0
    }
    fn mechanism_probe(&mut self, _: ActorId, _: ObjectId) -> bool {
        self.choose_eight
    }
    fn classification(&mut self, _: ActorId) -> u32 {
        self.class
    }
    fn choose_state_eight(&mut self, _: ActorId, _: ObjectId) -> bool {
        self.choose_eight
    }
    fn mechanism_count(&mut self, _: ObjectId) -> i32 {
        self.count
    }
    fn project_position(&mut self, _: Position3) -> (NodeId, i16) {
        (self.node, self.cell)
    }
    fn controls_same_actor_extension(&mut self, _: ActorId) -> bool {
        self.same_extension
    }
    fn queue_behavior(&mut self, _: ActorId, request: BehaviorRequest) {
        self.events.push(CallEvent::Behavior(request.mask));
    }
    fn refresh_actor(&mut self, _: ActorId) {
        self.events.push(CallEvent::Refresh);
    }
}

struct MechanismCalls {
    memory: Memory,
    interaction_result: u32,
    class: u32,
    predicate: bool,
    count: u32,
    projection: u32,
    cell: i16,
    controlled_extension: u32,
    ready: bool,
    exit_bit: bool,
    link_field: Option<u32>,
    args: Vec<u32>,
    events: Vec<CallEvent>,
}

impl GuestCall for MechanismCalls {
    fn call(&mut self, address: u32, regs: &mut ExtRegisters) {
        match address {
            0x15e120 | 0x15d470 | 0x15f0d0 => {}
            0x12f00 => {
                let slot_ptr = self.memory.get32(regs.esp);
                let token = self.memory.get32(slot_ptr);
                let entry = self.memory.get32(0x72f5e8) + (token >> 22) * 0x2e8;
                if token != 0 && self.memory.get32(entry + 0x2c) == token {
                    regs.eax = entry;
                } else {
                    if token != 0 {
                        let mut m = self.memory.clone();
                        m.put32(slot_ptr, 0);
                    }
                    regs.eax = 0;
                }
            }
            0x177920 => regs.eax = SUBRECORD,
            0x172cd0 => {
                let mut memory = self.memory.clone();
                regs.esp -= 4; // lifted callee expects a return-address slot
                engine_object_3::game_component_signed_ratio_of_arg_minus_field_0x52c_via_fast_sqrt_clamped_unit_unless_fla(&mut memory, regs);
                regs.esp += 4;
            }
            0x1596f0 => regs.eax = 0x77,
            0x177fb0 => regs.eax = u32::from(self.predicate),
            0x17e010 => {
                self.args.push(self.memory.get32(regs.esp + 12));
                regs.eax = self.interaction_result;
            }
            0x29fe60 => self.events.push(CallEvent::Prepare),
            0x177e90 => regs.eax = u32::from(self.ready),
            0x177f20 => {}
            0x177950 => regs.eax = if self.exit_bit { 0x10 } else { 0 },
            0x179f70 => {
                let out = self.memory.get32(regs.esp + 4);
                let mut m = self.memory.clone();
                m.put32(out, 0xa0000);
                m.put32(0xa0000 + 0x150, self.link_field.unwrap_or(0));
                regs.eax = u32::from(self.link_field.is_some());
            }
            0x17a0a0 => regs.eax = self.class,
            0x29fe20 => regs.eax = u32::from(self.predicate),
            0x17a200 => regs.eax = self.count,
            0x9bcf0 => regs.eax = self.projection,
            0x9a420 => {
                let mut memory = self.memory.clone();
                memory.put16(0x90000 + 0xa, self.cell as u16);
                regs.eax = 0x90000;
            }
            0x277fe0 => regs.eax = self.controlled_extension,
            0x17570 => self
                .events
                .push(CallEvent::Behavior(self.memory.get32(regs.esp + 4))),
            0x9cc40 => regs.eax = 1,
            other => panic!("unexpected mechanism delegate {other:#x}"),
        }
    }
}

fn mechanism_fixture(local_state: u32, handle: u32) -> Memory {
    let mut memory = Memory::default();
    memory.put32(STACK + 4, ACTOR);
    memory.put32(ACTOR + 0x7c, EXT);
    memory.put32(ACTOR + 0x64, 0x3f800000);
    memory.put32(ACTOR + 0x68, 0x40000000);
    memory.put32(ACTOR + 0x6c, 0x40400000);
    memory.put32(EXT + 0xbf0, BEHAVIOR);
    memory.put32(BEHAVIOR + 0x68, 0);
    memory.put32(EXT + 0x20, local_state);
    memory.put32(EXT + 0x9d0, u32::MAX);
    memory.put32(EXT + 0xc08, handle);
    let dispatch = match local_state {
        0 => 0x2a05a9,
        1 => 0x2a06e9,
        14 => 0x2a0696,
        _ => 0,
    };
    memory.put32(0x2a0c7c + local_state * 4, dispatch);
    memory.put32(0x2a0cb8, 0x2a05fc);
    memory.put32(0x2a0cbc, 0x2a0639);
    memory.put32(0x2a0cc0, 0x2a0639);
    memory.put32(0x2a0cc4, 0x2a0bd2);
    memory.put32(0x2a0cc8, 0x2a0bd2);
    memory.put_f32(0x47d1e4, 8.0);
    memory.put32(0x4e8cec, 60);
    memory.put_f32(0x4e794c, 1.0);
    memory.put_f32(0x475c78, 1.0);
    memory.put_f32(0x475d10, 2.0);
    memory.put_f32(0x47d5b0, -0.8);
    memory.put_f32(0x475cd0, 0.8);
    memory.put32(0x72f5e8, 0x80000);
    let entry = 0x80000 + (handle >> 22) * 0x2e8;
    memory.put32(entry + 0x2c, handle);
    memory.put32(MECHANISM + 0x7c, 0x61000);
    memory.put32(0x61000 + 0x568, SUBRECORD);
    memory.put32(0x90000 + 0xa, 17);
    memory
}

#[test]
fn tier1_mechanism_selected_states_match_lifted_2a0500() {
    let handle = (1 << 22) | 0x12345;
    for state in [0, 1] {
        for (result, class, predicate, count, expected_state) in [
            (0, 0, false, 0, None),
            (2, 0, false, 0, Some(1)),
            (2, 0, true, 1, Some(8)),
            (2, 0, true, 2, Some(1)),
            (2, 1, false, 0, Some(11)),
            (2, 2, false, 0, Some(11)),
            (2, 3, false, 0, Some(9)),
            (2, 4, false, 0, Some(9)),
            (2, 5, false, 0, None),
        ] {
            if state == 1 && class != 0 {
                continue;
            }
            let mut memory = mechanism_fixture(state, handle);
            memory.put32(MECHANISM + 0x7c, 0x61000);
            let mut regs = registers(STACK);
            let mut oracle = MechanismCalls {
                memory: memory.clone(),
                interaction_result: result,
                class,
                predicate,
                count,
                projection: 22,
                cell: 17,
                controlled_extension: EXT,
                ready: false,
                exit_bit: false,
                link_field: None,
                args: Vec::new(),
                events: Vec::new(),
            };
            t1892_pathfinding::game_ai_nav_class_tick_state_machine_15_states(
                &mut memory,
                &mut oracle,
                &mut regs,
            );

            let mut native = ActorInteractionState {
                local_state: state,
                mechanism: Some(MechanismHandle(handle)),
                ..ActorInteractionState::default()
            };
            let mut pool = MechanismPool {
                slots: vec![
                    None,
                    Some(MechanismRecord {
                        identity: handle,
                        object: ObjectId(MECHANISM),
                        speed: 0.0,
                        flags: 0,
                    }),
                ],
            };
            let mut host = MechanismHost {
                result: result as i32,
                class,
                choose_eight: predicate,
                count: count as i32,
                ..MechanismHost::default()
            };
            let got = tick_selected_state(
                ActorId(ACTOR),
                Position3::default(),
                &mut native,
                &mut pool,
                &mut host,
            );
            let expected_state = if state == 1 && result == 2 {
                Some(4)
            } else {
                expected_state
            };
            assert_eq!(
                got,
                expected_state.map_or(
                    MechanismTickResult::Returned,
                    MechanismTickResult::RequestedLocalState
                )
            );
            assert_eq!(native.local_state, memory.get32(EXT + 0x20));
            assert_eq!(native.previous_state, memory.get32(EXT + 0x9d4));
            assert_eq!(
                native.pending_state.map_or(u32::MAX, |v| v),
                memory.get32(EXT + 0x9d0)
            );
            assert_eq!(native.elapsed.to_bits(), memory.get32(EXT + 0x9d8));
            if state == 0 && result == 2 && class == 0 {
                assert_eq!(
                    pool.slots[1].as_ref().unwrap().speed.to_bits(),
                    memory.get32(SUBRECORD + 0x394)
                );
            }
        }
    }
}

#[test]
fn tier1_mechanism_stale_handle_and_state14_exit_match_lifted_originals() {
    let handle = (1 << 22) | 0x12345;
    let mut memory = mechanism_fixture(0, handle);
    memory.put32(0x80000 + 0x2e8 + 0x2c, handle ^ 1);
    let mut regs = registers(STACK);
    let mut oracle = MechanismCalls {
        memory: memory.clone(),
        interaction_result: 2,
        class: 0,
        predicate: false,
        count: 0,
        projection: 22,
        cell: 17,
        controlled_extension: EXT,
        ready: false,
        exit_bit: false,
        link_field: None,
        args: Vec::new(),
        events: Vec::new(),
    };
    t1892_pathfinding::game_ai_nav_class_tick_state_machine_15_states(
        &mut memory,
        &mut oracle,
        &mut regs,
    );
    let mut native = ActorInteractionState {
        local_state: 0,
        mechanism: Some(MechanismHandle(handle)),
        ..ActorInteractionState::default()
    };
    let mut pool = MechanismPool {
        slots: vec![
            None,
            Some(MechanismRecord {
                identity: handle ^ 1,
                object: ObjectId(MECHANISM),
                speed: 0.0,
                flags: 0,
            }),
        ],
    };
    let mut host = MechanismHost::default();
    assert_eq!(
        tick_selected_state(
            ActorId(ACTOR),
            Position3::default(),
            &mut native,
            &mut pool,
            &mut host
        ),
        MechanismTickResult::RequestedLocalState(14)
    );
    assert_eq!(native.mechanism, None);
    assert_eq!(memory.get32(EXT + 0xc08), 0);
    assert_eq!(native.local_state, memory.get32(EXT + 0x20));
    assert_eq!(
        native.pending_state.map_or(u32::MAX, |v| v),
        memory.get32(EXT + 0x9d0)
    );

    for (same_extension, target, mask) in [
        (true, Some(ObjectId(88)), 0x2000),
        (false, Some(ObjectId(88)), 4),
    ] {
        let mut memory = mechanism_fixture(14, 0);
        memory.put32(EXT + 4, target.unwrap().0);
        let mut regs = registers(STACK);
        let mut oracle = MechanismCalls {
            memory: memory.clone(),
            interaction_result: 0,
            class: 0,
            predicate: false,
            count: 0,
            projection: 22,
            cell: 17,
            controlled_extension: if same_extension { EXT } else { 0x9999 },
            ready: false,
            exit_bit: false,
            link_field: None,
            args: Vec::new(),
            events: Vec::new(),
        };
        t1892_pathfinding::game_ai_nav_class_tick_state_machine_15_states(
            &mut memory,
            &mut oracle,
            &mut regs,
        );
        let mut native = ActorInteractionState {
            local_state: 14,
            target: target.map(|v| v),
            ..ActorInteractionState::default()
        };
        let mut pool = MechanismPool::default();
        let mut host = MechanismHost {
            node: NodeId(22),
            cell: 17,
            same_extension,
            ..MechanismHost::default()
        };
        assert_eq!(
            tick_selected_state(
                ActorId(ACTOR),
                Position3::default(),
                &mut native,
                &mut pool,
                &mut host
            ),
            MechanismTickResult::ExitedToBehavior(BehaviorRequest { mask })
        );
        assert_eq!(native.current_node, Some(NodeId(memory.get32(EXT + 0x678))));
        assert_eq!(native.cell, Some(memory.read16(ACTOR + 0x34) as i16));
        assert_eq!(native.target.map_or(0, |v| v.0), memory.get32(EXT + 4));
        assert_eq!(native.behavior_request, Some(BehaviorRequest { mask }));
        assert_eq!(oracle.events, [CallEvent::Behavior(mask)]);
        assert_eq!(host.events, [CallEvent::Behavior(mask), CallEvent::Refresh]);
        assert_eq!(memory.get32(ACTOR + 0x64), 0x3f800000);
        assert_eq!(memory.get32(ACTOR + 0x6c), 0x40400000);
    }
}

#[test]
fn tier1_mechanism_states_2_3_6_to_11_13_match_lifted_2a0500() {
    // Tier 1 (lifted differential, fabricated mocked-service inputs). States 4 and
    // 5 remain unported (x87-returning 0x172CD0 and the +0x9E0 timer).
    let handle = (1 << 22) | 0x12345;
    let table = [
        0x2a05a9u32,
        0x2a06e9,
        0x2a075d,
        0x2a0710,
        0x2a07a6,
        0x2a092e,
        0x2a0a50,
        0x2a09c8,
        0x2a0a01,
        0x2a0abc,
        0x2a0b4a,
        0x2a0bf3,
        0x2a0c75,
        0x2a0643,
        0x2a0696,
    ];
    let mut cases = 0;
    for state in [2u32, 3, 6, 7, 8, 9, 10, 11, 13] {
        for result in 0..=11u32 {
            for bits in 0..64u32 {
                let (ready, exit_bit, same, target_set) =
                    (bits & 1 != 0, bits & 2 != 0, bits & 4 != 0, bits & 8 != 0);
                let link = [None, Some(0), Some(5)][(bits as usize >> 4) % 3];
                for (count, flags, stale) in [(0u32, 0u32, false), (2, 3, false), (1, 1, true)] {
                    let mut memory = mechanism_fixture(state, handle);
                    for (i, v) in table.iter().enumerate() {
                        memory.put32(0x2a0c7c + i as u32 * 4, *v);
                    }
                    for (i, v) in [0x2a09bcu32, 0x2a0b98, 0x2a0c42, 0x2a0c75]
                        .iter()
                        .enumerate()
                    {
                        memory.put32(0x2a0cf4 + i as u32 * 4, *v);
                    }
                    for (i, v) in [0u8, 2, 2, 1, 0, 2, 2, 2, 0].iter().enumerate() {
                        memory.put8(0x2a0d04 + i as u32, *v);
                    }
                    if stale {
                        memory.put32(0x80000 + (handle >> 22) * 0x2e8 + 0x2c, handle ^ 1);
                    }
                    memory.put32(SUBRECORD + 0x390, flags);
                    if target_set {
                        memory.put32(EXT + 4, 0x1234);
                    }
                    let mut regs = registers(STACK);
                    let mut oracle = MechanismCalls {
                        memory: memory.clone(),
                        interaction_result: result,
                        class: 0,
                        predicate: bits & 32 != 0,
                        count,
                        projection: 22,
                        cell: 17,
                        controlled_extension: if same { EXT } else { 0 },
                        ready,
                        exit_bit,
                        link_field: link,
                        args: Vec::new(),
                        events: Vec::new(),
                    };
                    t1892_pathfinding::game_ai_nav_class_tick_state_machine_15_states(
                        &mut memory,
                        &mut oracle,
                        &mut regs,
                    );

                    let mut native = ActorInteractionState {
                        local_state: state,
                        mechanism: Some(MechanismHandle(handle)),
                        target: target_set.then_some(ObjectId(0x1234)),
                        ..ActorInteractionState::default()
                    };
                    let mut pool = MechanismPool {
                        slots: vec![
                            None,
                            Some(MechanismRecord {
                                identity: if stale { handle ^ 1 } else { handle },
                                object: ObjectId(MECHANISM),
                                speed: 0.0,
                                flags,
                            }),
                        ],
                    };
                    let mut host = MechanismHost {
                        result: result as i32,
                        choose_eight: bits & 32 != 0,
                        count: count as i32,
                        same_extension: same,
                        target: native.target,
                        ready,
                        exit_bit,
                        link_field: link,
                        ..MechanismHost::default()
                    };
                    tick_selected_state(
                        ActorId(ACTOR),
                        Position3::default(),
                        &mut native,
                        &mut pool,
                        &mut host,
                    );
                    let ctx = format!("state {state} result {result} bits {bits} stale {stale}");
                    assert_eq!(native.local_state, memory.get32(EXT + 0x20), "{ctx}");
                    assert_eq!(native.previous_state, memory.get32(EXT + 0x9d4), "{ctx}");
                    assert_eq!(
                        native.pending_state.map_or(u32::MAX, |v| v),
                        memory.get32(EXT + 0x9d0),
                        "{ctx}"
                    );
                    assert_eq!(native.elapsed.to_bits(), memory.get32(EXT + 0x9d8), "{ctx}");
                    assert_eq!(
                        native.target.map_or(0, |t| t.0),
                        memory.get32(EXT + 4),
                        "{ctx}"
                    );
                    if !stale {
                        assert_eq!(
                            pool.slots[1].as_ref().unwrap().flags,
                            memory.get32(SUBRECORD + 0x390),
                            "{ctx}"
                        );
                    }
                    assert_eq!(host.args, oracle.args, "{ctx}");
                    let prepares =
                        |e: &[CallEvent]| e.iter().filter(|v| **v == CallEvent::Prepare).count();
                    assert_eq!(prepares(&host.events), prepares(&oracle.events), "{ctx}");
                    cases += 1;
                }
            }
        }
    }
    assert!(cases > 10_000);
}

#[test]
fn tier1_mechanism_states_4_5_12_and_speed_ramp_match_lifted() {
    // Tier 1 (lifted differential, fabricated mocked-service inputs, weak evidence:
    // not Tier 2). Covers 0x172CD0 (as the real lifted function), +0x9E0 timer and
    // states 4, 5, 12 of 0x2A0500.
    let handle = (1 << 22) | 0x12345;
    let table = [
        0x2a05a9u32,
        0x2a06e9,
        0x2a075d,
        0x2a0710,
        0x2a07a6,
        0x2a092e,
        0x2a0a50,
        0x2a09c8,
        0x2a0a01,
        0x2a0abc,
        0x2a0b4a,
        0x2a0bf3,
        0x2a0c75,
        0x2a0643,
        0x2a0696,
    ];
    let requests = [
        0.0f32, 0.3, -0.3, 0.8, -0.8, 1.0, 3.5, -5.0, 8.0, 1.0e-30, 0.15,
    ];
    let references = [0.0f32, 0.1, -0.4, 2.0];
    let mut cases = 0;
    for state in [4u32, 5, 12] {
        for result in 0..=11u32 {
            for (flags, timer, probe, sign_only) in (0..16u32).map(|b| {
                (
                    b & 3,
                    [0.0f32, 0.01, 2.0, 1.0e-6][(b as usize >> 2) & 3],
                    b & 1 != 0,
                    b & 8 != 0,
                )
            }) {
                for (speed, reference) in requests.iter().zip(references.iter().cycle()) {
                    let mut memory = mechanism_fixture(state, handle);
                    for (i, v) in table.iter().enumerate() {
                        memory.put32(0x2a0c7c + i as u32 * 4, *v);
                    }
                    for (i, v) in [
                        0x2a082eu32,
                        0x2a089e,
                        0x2a08c9,
                        0x2a0c75,
                        0x2a0c75,
                        0x2a08e9,
                        0x2a0924,
                        0x2a0c75,
                        0x2a0c75,
                        0x2a0c75,
                        0x2a09bc,
                    ]
                    .iter()
                    .enumerate()
                    {
                        memory.put32(0x2a0ccc + i as u32 * 4, *v);
                    }
                    memory.put32(SUBRECORD + 0x390, flags);
                    memory.put_f32(SUBRECORD + 0x394, *speed);
                    memory.put_f32(EXT + 0x9e0, timer);
                    memory.put32(0x61000 + 0x58, u32::from(sign_only));
                    memory.put_f32(0x61000 + 0x52c, *reference);
                    // The oracle's 0x12F00 mock returns the pool entry as the mechanism object.
                    memory.put32(0x80000 + 0x2e8 + 0x7c, 0x61000);
                    let mut regs = registers(STACK);
                    let mut oracle = MechanismCalls {
                        memory: memory.clone(),
                        interaction_result: result,
                        class: 0,
                        predicate: probe,
                        count: 0,
                        projection: 22,
                        cell: 17,
                        controlled_extension: 0,
                        ready: false,
                        exit_bit: false,
                        link_field: None,
                        args: Vec::new(),
                        events: Vec::new(),
                    };
                    t1892_pathfinding::game_ai_nav_class_tick_state_machine_15_states(
                        &mut memory,
                        &mut oracle,
                        &mut regs,
                    );
                    let inputs = SpeedRampInputs {
                        sign_only,
                        reference: *reference,
                    };
                    let mut native = ActorInteractionState {
                        local_state: state,
                        timer,
                        mechanism: Some(MechanismHandle(handle)),
                        ..ActorInteractionState::default()
                    };
                    let mut pool = MechanismPool {
                        slots: vec![
                            None,
                            Some(MechanismRecord {
                                identity: handle,
                                object: ObjectId(MECHANISM),
                                speed: *speed,
                                flags,
                            }),
                        ],
                    };
                    let mut host = MechanismHost {
                        result: result as i32,
                        choose_eight: probe,
                        ramp: inputs,
                        ..MechanismHost::default()
                    };
                    tick_selected_state(
                        ActorId(ACTOR),
                        Position3::default(),
                        &mut native,
                        &mut pool,
                        &mut host,
                    );
                    let ctx = format!(
                        "state {state} result {result} flags {flags} timer {timer} speed {speed} ref {reference} sign_only {sign_only}"
                    );
                    assert_eq!(native.local_state, memory.get32(EXT + 0x20), "{ctx}");
                    assert_eq!(native.previous_state, memory.get32(EXT + 0x9d4), "{ctx}");
                    assert_eq!(
                        native.pending_state.map_or(u32::MAX, |v| v),
                        memory.get32(EXT + 0x9d0),
                        "{ctx}"
                    );
                    assert_eq!(native.elapsed.to_bits(), memory.get32(EXT + 0x9d8), "{ctx}");
                    assert_eq!(native.timer.to_bits(), memory.get32(EXT + 0x9e0), "{ctx}");
                    assert_eq!(
                        native.target.map_or(0, |t| t.0),
                        memory.get32(EXT + 4),
                        "{ctx}"
                    );
                    assert_eq!(
                        pool.slots[1].as_ref().unwrap().flags,
                        memory.get32(SUBRECORD + 0x390),
                        "{ctx}"
                    );
                    if state != 12 {
                        assert_eq!(host.args, oracle.args, "{ctx}");
                    }
                    cases += 1;
                }
            }
        }
    }
    assert!(cases > 5_000);
}

#[test]
fn tier1_speed_ramp_matches_lifted_172cd0_directly() {
    let mut cases = 0;
    for sign_only in [false, true] {
        for reference in [0.0f32, 0.25, -1.5, 100.0] {
            for request in [
                0.0f32,
                -0.0,
                1.0e-20,
                0.001,
                0.3,
                -0.3,
                0.8,
                -0.8,
                1.0,
                2.0,
                -7.5,
                64.0,
                1.0e10,
                f32::NAN,
            ] {
                let mut memory = Memory::default();
                memory.put32(STACK + 4, MECHANISM);
                memory.put_f32(STACK + 8, request);
                memory.put32(MECHANISM + 0x7c, 0x61000);
                memory.put32(0x61000 + 0x58, u32::from(sign_only));
                memory.put_f32(0x61000 + 0x52c, reference);
                memory.put_f32(0x475d10, 2.0);
                memory.put_f32(0x475c78, 1.0);
                let mut regs = registers(STACK);
                engine_object_3::game_component_signed_ratio_of_arg_minus_field_0x52c_via_fast_sqrt_clamped_unit_unless_fla(&mut memory, &mut regs);
                let lifted = regs.fpu.st(0) as f32;
                let rust = mechanism_speed_ramp(
                    request,
                    SpeedRampInputs {
                        sign_only,
                        reference,
                    },
                );
                assert!(
                    lifted.to_bits() == rust.to_bits() || (lifted.is_nan() && rust.is_nan()),
                    "request {request} ref {reference} sign_only {sign_only}: {lifted} vs {rust}"
                );
                cases += 1;
            }
        }
    }
    assert!(cases > 100);
}
