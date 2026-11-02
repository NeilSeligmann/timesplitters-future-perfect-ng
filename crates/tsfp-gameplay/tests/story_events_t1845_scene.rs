//! T1845 round 3 (task F): level 1 scene drivers `0x36B7A0`, `0x1DD510`, `0x1DAAB0`. Synthetic worlds only.
use tsfp_gameplay::story_events::hooks_scene::*;

const TIMING: Timing = Timing { steps: 1, rate: 60 };

fn blast_input() -> BlastInput {
    BlastInput {
        timing: TIMING,
        room_visible: true,
        mode_byte: 0,
        local_players: 1,
    }
}

#[test]
fn blast_inactive_or_hidden_does_nothing_more() {
    let mut state = BlastState::default();
    assert!(blast_frame(&mut state, &blast_input(), &mut || 0).is_empty());
    state.active = true;
    let mut input = blast_input();
    input.room_visible = false;
    let effects = blast_frame(&mut state, &input, &mut || 0);
    assert_eq!(effects.len(), 1);
    assert_eq!(
        state.history_position, 0,
        "the ring is untouched when the room is hidden"
    );
}

#[test]
fn blast_refills_four_pieces_from_the_trails_and_rolls_the_ring() {
    let mut state = BlastState::default();
    state.active = true;
    state.piece_count = 10;
    state.blobs[1].ring_index = 50;
    state.blobs[1].ring[47] = [1.0, 2.0, 3.0];
    state.pieces[10].player = 1;
    state.pieces[10].frames_back = 3;
    blast_frame(&mut state, &blast_input(), &mut || 0);
    assert_eq!(state.piece_count, 14);
    assert_eq!(state.pieces[10].position, [1.0, 2.0, 3.0]);
    assert_eq!(state.history[0], 1);
    assert_eq!(state.history_position, 1);
}

#[test]
fn blast_trims_by_eight_down_to_0x4b_when_steps_are_long() {
    let mut state = BlastState::default();
    state.active = true;
    state.piece_count = 100;
    let mut input = blast_input();
    input.timing.steps = 2;
    blast_frame(&mut state, &input, &mut || 0);
    assert_eq!(state.piece_count, 92);
    state.piece_count = 80;
    blast_frame(&mut state, &input, &mut || 0);
    assert_eq!(state.piece_count, 0x4B);
    // Split screen story mode raises the threshold to 2, so 2 steps are fine and the pool refills.
    input.mode_byte = 0x0A;
    input.local_players = 2;
    let before = state.piece_count;
    blast_frame(&mut state, &input, &mut || 0);
    assert_eq!(state.piece_count, before + 4);
}

#[test]
fn blast_ends_when_every_piece_is_dead() {
    let mut state = BlastState::default();
    state.active = true;
    state.piece_count = BLAST_MAX_PIECES as i32;
    state.ambience_handle = 7;
    for piece in state.pieces.iter_mut() {
        piece.state = 3;
    }
    let effects = blast_frame(&mut state, &blast_input(), &mut || 0);
    assert!(effects.contains(&SceneEffect::SlotFinish { slot: 9, arg: 0 }));
    assert!(effects.contains(&SceneEffect::StopSound { handle: 7 }));
    assert!(!state.active);
    assert_eq!(state.ambience_handle, -1);
    assert!(!effects.iter().any(|e| matches!(
        e,
        SceneEffect::Unsupported {
            callee: 0x368A10,
            ..
        }
    )));
}

#[test]
fn blast_spawns_emitters_only_above_720_dead_pieces() {
    let mut state = BlastState::default();
    state.active = true;
    state.piece_count = BLAST_MAX_PIECES as i32;
    for (i, piece) in state.pieces.iter_mut().enumerate() {
        piece.state = if i < 721 { 3 } else { 0 };
    }
    let effects = blast_frame(&mut state, &blast_input(), &mut || 2);
    let particles = effects
        .iter()
        .filter(|e| matches!(e, SceneEffect::Particle { .. }))
        .count();
    assert_eq!(particles, 79);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, SceneEffect::Sound { id: 0x158, .. }))
    );
    state.pieces.iter().for_each(|p| assert_eq!(p.state, 3));
    // At exactly 720 dead nothing spawns.
    let mut state = BlastState::default();
    state.active = true;
    state.piece_count = BLAST_MAX_PIECES as i32;
    for (i, piece) in state.pieces.iter_mut().enumerate() {
        piece.state = if i < 720 { 3 } else { 0 };
    }
    let effects = blast_frame(&mut state, &blast_input(), &mut || 0);
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, SceneEffect::Particle { .. }))
    );
}

fn beam() -> BeamState {
    BeamState {
        phase: 1,
        ramp: 0.0,
        start: [0.0, 0.0, 0.0],
        end: [10.0, 0.0, 0.0],
        particles: [1, 2, 3, 4, 5],
    }
}

fn beam_input() -> BeamInput {
    BeamInput {
        timing: TIMING,
        roll: None,
        fraction: 0.5,
        area: 5,
        second_area: 0,
        mode_byte: 0,
        local_players: 1,
        table_23: false,
        table_19: false,
        targets: vec![],
    }
}

#[test]
fn beam_charges_to_one_and_spawns_a_ground_effect_on_the_segment() {
    let mut state = beam();
    let mut input = beam_input();
    input.roll = Some(0);
    input.timing.steps = 120;
    let effects = beam_frame(&mut state, &input);
    assert_eq!(state.ramp, 1.0);
    assert!(effects.contains(&SceneEffect::GroundEffect {
        point: [5.0, 0.0, 0.0]
    }));
    assert!(
        effects.iter().any(
            |e| matches!(e, SceneEffect::LoopSound { id: 0xBFB, volume, .. } if *volume == 2.0)
        )
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, SceneEffect::ParticleField74 { value, .. } if *value == 10.0))
            .count(),
        5
    );
}

#[test]
fn beam_fades_out_and_frees_the_particles() {
    let mut state = beam();
    state.phase = 2;
    state.ramp = 0.001;
    let mut input = beam_input();
    input.timing.steps = 60;
    let effects = beam_frame(&mut state, &input);
    assert_eq!(state.phase, 0);
    assert_eq!(state.ramp, 0.0);
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, SceneEffect::FreeParticle { .. }))
            .count(),
        5
    );
}

#[test]
fn beam_fade_sets_particle_fields_per_handle() {
    let mut state = beam();
    state.phase = 2;
    state.ramp = 1.0;
    let effects = beam_frame(&mut state, &beam_input());
    let pairs = effects
        .iter()
        .filter(|e| matches!(e, SceneEffect::ParticleFields68_6C { .. }))
        .count();
    let zero = effects
        .iter()
        .filter(|e| matches!(e, SceneEffect::ParticleField74 { value, .. } if *value == 0.0))
        .count();
    assert_eq!((pairs, zero), (3, 3));
}

#[test]
fn beam_hurts_objects_near_the_segment_and_skips_far_ones() {
    let mut state = beam();
    state.ramp = 1.0;
    let near = BeamTarget {
        skipped: false,
        position: [5.0, 1.0, 0.0],
        extent: [0.5, 0.5],
        is_character: false,
        controller: None,
    };
    let far = BeamTarget {
        position: [5.0, 20.0, 0.0],
        ..near
    };
    let skipped = BeamTarget {
        skipped: true,
        ..near
    };
    let mut input = beam_input();
    input.targets = vec![near, far, skipped];
    let effects = beam_frame(&mut state, &input);
    let hits: Vec<_> = effects
        .iter()
        .filter(|e| matches!(e, SceneEffect::Damage { .. }))
        .collect();
    assert_eq!(hits.len(), 1);
    match hits[0] {
        SceneEffect::Damage {
            victim,
            amount,
            kind,
            direction,
            ..
        } => {
            assert_eq!(*victim, DamageVictim::ListObject(0));
            assert_eq!((*amount, *kind), (0.2, 0x15));
            assert!((direction[1] - 1.0).abs() < 1e-6);
        }
        _ => unreachable!(),
    }
}

#[test]
fn beam_character_without_controller_is_skipped_player_or_rock_is_hurt() {
    let mut state = beam();
    state.ramp = 1.0;
    let base = BeamTarget {
        skipped: false,
        position: [5.0, 1.0, 0.0],
        extent: [0.5, 0.5],
        is_character: true,
        controller: None,
    };
    let player = BeamTarget {
        controller: Some(BeamController {
            is_player: true,
            is_rock: false,
        }),
        ..base
    };
    let other = BeamTarget {
        controller: Some(BeamController {
            is_player: false,
            is_rock: false,
        }),
        ..base
    };
    let mut input = beam_input();
    input.targets = vec![base, player, other];
    let effects = beam_frame(&mut state, &input);
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, SceneEffect::Damage { .. }))
            .count(),
        1
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(e, SceneEffect::PlayerEffect { .. }))
            .count(),
        1
    );
}

#[test]
fn beam_intensity_depends_on_area_and_tables() {
    let mut input = beam_input();
    input.area = 0x18;
    let mut state = beam();
    state.phase = 0;
    let effects = beam_frame(&mut state, &input);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, SceneEffect::ParticleField74 { value, .. } if *value == 0.01))
    );
    input.table_19 = true;
    let effects = beam_frame(&mut state, &input);
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, SceneEffect::ParticleField74 { value, .. } if *value == 2.5))
    );
    input.table_23 = true;
    let effects = beam_frame(&mut state, &input);
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, SceneEffect::ParticleField74 { .. })),
        "table 0x23 skips, idle phase"
    );
}

// ----------------------------------------------------------------- worms

struct TestWorld {
    players: Vec<PlayerView>,
    inside_box: bool,
    bite: bool,
    attackers: i32,
    flag: i32,
    finished: bool,
    current_animation: u32,
    timer: Option<f32>,
    object_exists: [bool; 5],
    rise: f32,
    ratio: f32,
    ray: Option<RayHit>,
}

impl TestWorld {
    fn new() -> TestWorld {
        TestWorld {
            players: vec![PlayerView {
                position: [-60.0, 0.0, 20.0],
                node: 3,
            }],
            inside_box: true,
            bite: false,
            attackers: 0,
            flag: 1,
            finished: false,
            current_animation: 0,
            timer: Some(100.0),
            object_exists: [true; 5],
            rise: 0.0,
            ratio: 1.0,
            ray: None,
        }
    }
}

impl WormWorld for TestWorld {
    fn enabled(&self) -> bool {
        true
    }
    fn room_flag(&self) -> bool {
        true
    }
    fn global_flag(&self) -> i32 {
        self.flag
    }
    fn set_global_flag(&mut self, value: i32) {
        self.flag = value;
    }
    fn attacker_count(&self) -> i32 {
        self.attackers
    }
    fn set_attacker_count(&mut self, value: i32) {
        self.attackers = value;
    }
    fn players(&self) -> Vec<PlayerView> {
        self.players.clone()
    }
    fn object_alive(&self, entity: usize) -> bool {
        self.object_exists[entity]
    }
    fn object_exists(&self, entity: usize) -> bool {
        self.object_exists[entity]
    }
    fn object_controller_timer(&self, _entity: usize) -> Option<f32> {
        self.timer
    }
    fn players_inside_box(&self) -> bool {
        self.inside_box
    }
    fn bite_cone_hit(&self, _entity: usize, _worm: &Worm) -> bool {
        self.bite
    }
    fn nearest_node(&self, _worm: &Worm) -> i32 {
        1
    }
    fn node_position(&self, id: i32) -> Option<Vec3> {
        Some([id as f32, 0.0, 0.0])
    }
    fn patrol_nodes(&self) -> [i32; 6] {
        [10, 11, 12, 13, 14, 15]
    }
    fn path_next_hop(&self, _from: i32, to: i32) -> i32 {
        to
    }
    fn key_node_position(&self) -> Vec3 {
        [-60.0, 0.0, 20.0]
    }
    fn animation_is(&self, _entity: usize, animation: u32) -> bool {
        self.current_animation == animation
    }
    fn animation_finished(&self, _entity: usize) -> bool {
        self.finished
    }
    fn random(&mut self) -> u32 {
        1
    }
    fn loop_sound_started(&self, _entity: usize) -> bool {
        false
    }
    fn end_marker(&self) -> i32 {
        0
    }
    fn controller_rise(&self, _entity: usize, _special: bool) -> f32 {
        self.rise
    }
    fn object_ratio(&self, _entity: usize) -> f32 {
        self.ratio
    }
    fn breath_ray(&mut self, _entity: usize) -> Option<RayHit> {
        self.ray.clone()
    }
}

fn worms() -> [Worm; WORM_COUNT] {
    [
        Worm::new([-70.0, WORM_Y, 0.0]),
        Worm::new([-70.0, WORM_Y, 10.0]),
        Worm::new([-70.0, WORM_Y, 20.0]),
        Worm::new([-70.0, WORM_Y, 30.0]),
        Worm::new([-60.0, WORM_Y, 30.0]),
    ]
}

#[test]
fn flag_class_table_matches_the_image() {
    for state in 0..=0x10 {
        let expected = match state {
            2 => None,
            9..=14 | 16 => Some(true),
            _ => Some(false),
        };
        assert_eq!(worm_flag_class(state), expected, "state {state}");
    }
    assert_eq!(worm_flag_class(0x11), None);
}

#[test]
fn nearest_player_takes_the_first_minimum() {
    let players = [
        PlayerView {
            position: [10.0, 0.0, 0.0],
            node: 0,
        },
        PlayerView {
            position: [-10.0, 0.0, 0.0],
            node: 0,
        },
        PlayerView {
            position: [1.0, 0.0, 0.0],
            node: 0,
        },
    ];
    assert_eq!(nearest_player([0.0; 3], &players), 2);
    assert_eq!(nearest_player([0.0; 3], &players[..2]), 0);
}

#[test]
fn angle_wrap_and_table_index() {
    assert!((wrap_angle(4.0) - (4.0 - 2.0 * std::f32::consts::PI)).abs() < 1e-5);
    assert!((wrap_angle(-4.0) - (2.0 * std::f32::consts::PI - 4.0)).abs() < 1e-5);
    assert_eq!(angle_index(0.0), 0);
    assert_eq!(
        angle_index(std::f32::consts::PI),
        1024 + 0,
        "pi lands on index 1024 (half of 2048) after rounding"
    );
}

#[test]
fn idle_worm_chases_and_is_pinned_inside_the_arena() {
    let mut set = worms();
    let mut world = TestWorld::new();
    set[0].position = [-56.0, WORM_Y, 0.0];
    set[0].heading = std::f32::consts::FRAC_PI_2;
    worm_frame(&mut set, &mut world, TIMING);
    assert!(set[0].position[0] <= ARENA_MAX[0]);
    assert_eq!(set[0].position[1], WORM_Y);
}

#[test]
fn worm_moves_along_sine_and_cosine_of_the_heading() {
    let mut worm = Worm::new([-70.0, 0.0, 0.0]);
    worm.heading = 0.0;
    worm_move(&mut worm, TIMING);
    assert!((worm.position[0] + 70.0).abs() < 1e-5);
    assert!((worm.position[2] - WORM_SPEED / 60.0).abs() < 1e-4);
}

#[test]
fn spotting_a_player_starts_the_attack_run_and_counts_the_attacker() {
    let mut set = worms();
    let mut world = TestWorld::new();
    world.bite = true;
    worm_frame(&mut set, &mut world, TIMING);
    assert_eq!(set[0].state, worm_state::ATTACK_RUN);
    assert_eq!(set[0].mode, 0);
    assert_eq!(world.attackers, 5);
}

#[test]
fn despawn_state_destroys_only_after_the_timer_elapsed() {
    let mut set = worms();
    set[1].state = worm_state::DESPAWN;
    let mut world = TestWorld::new();
    world.timer = Some(0.1);
    let effects = worm_frame(&mut set, &mut world, TIMING);
    assert!(!effects.contains(&SceneEffect::DestroyObject { entity: 1 }));
    world.timer = Some(0.2);
    let effects = worm_frame(&mut set, &mut world, TIMING);
    assert!(effects.contains(&SceneEffect::DestroyObject { entity: 1 }));
}

#[test]
fn state_7_arms_the_attack_when_a_player_reaches_the_key_node() {
    let mut set = worms();
    set[2].state = worm_state::WAIT_PLAYER;
    let mut world = TestWorld::new();
    worm_frame(&mut set, &mut world, TIMING);
    assert_eq!(set[2].state, worm_state::ATTACK_RUN);
    assert_eq!(set[2].mode, 7);
}

#[test]
fn leap_needs_height_below_minus_1_1_and_no_busy_neighbour() {
    let mut set = worms();
    set[0].height = -1.2;
    assert!(leap_allowed(&set, 0));
    set[1].state = 9;
    set[1].target = set[0].position;
    assert!(!leap_allowed(&set, 0));
    set[0].height = -1.0;
    set[1].state = 0;
    assert!(!leap_allowed(&set, 0));
}

#[test]
fn attack_chain_b_to_c_to_f_to_despawn() {
    let mut set = worms();
    let mut world = TestWorld::new();
    set[0].state = worm_state::ANIM_10;
    world.finished = true;
    world.current_animation = 0x199;
    worm_frame(&mut set, &mut world, TIMING);
    assert_eq!(set[0].state, worm_state::ANIM_D);

    set[0].state = worm_state::ANIM_C;
    set[0].count = 1;
    world.current_animation = 0x196;
    worm_frame(&mut set, &mut world, TIMING);
    assert_eq!(
        set[0].count, 1,
        "animation 0x196 flips to 0x199 without counting"
    );
    world.current_animation = 0x199;
    worm_frame(&mut set, &mut world, TIMING);
    assert_eq!(set[0].count, 2);
    assert_eq!(set[0].state, worm_state::ANIM_F);

    world.attackers = 3;
    worm_frame(&mut set, &mut world, TIMING);
    assert_eq!(set[0].state, worm_state::DESPAWN);
    assert_eq!(world.attackers, 2);
}

#[test]
fn state_b_chooses_a_or_e_by_facing_and_distance() {
    let mut set = worms();
    let mut world = TestWorld::new();
    world.finished = true;
    world.current_animation = 0x195;
    set[0].state = worm_state::ANIM_B;
    set[0].position = [-60.0, WORM_Y, 15.0];
    set[0].target_heading = 0.0;
    worm_frame(&mut set, &mut world, TIMING);
    assert_eq!(
        set[0].state,
        worm_state::ANIM_A,
        "player 5 ahead along +z and within 8"
    );
    set[1].state = worm_state::ANIM_B;
    set[1].position = [-60.0, WORM_Y, 30.0];
    set[1].target_heading = 0.0;
    worm_frame(&mut set, &mut world, TIMING);
    assert_eq!(set[1].state, worm_state::ANIM_E, "player behind");
}

#[test]
fn separation_pushes_both_apart_but_a_leaping_worm_stays_put() {
    let mut set = worms();
    set[0].position = [-60.0, WORM_Y, 10.0];
    set[1].position = [-59.0, WORM_Y, 10.0];
    separate(&mut set, 0, TIMING);
    assert!(set[0].position[0] < -60.0 + 1e-6 || set[0].position[0] > -60.0);
    let moved_apart = (set[1].position[0] - set[0].position[0]).abs();
    assert!(moved_apart > 1.0);
    set[0].position = [-60.0, WORM_Y, 10.0];
    set[1].position = [-59.0, WORM_Y, 10.0];
    set[0].state = 9;
    separate(&mut set, 0, TIMING);
    assert_eq!(set[0].position[0], -60.0);
    assert!(set[1].position[0] > -59.0);
}

#[test]
fn approach_matches_the_float_helper() {
    let mut value = 0.0;
    assert!(!approach(&mut value, 1.0, 0.1));
    assert!((value - 0.1).abs() < 1e-7);
    assert!(approach(&mut value, 0.15, 0.1));
    assert_eq!(value, 0.15);
}

#[test]
fn breath_height_clamps_between_zero_and_a_fifth() {
    assert_eq!(breath_height(-3.0, false), 0.0);
    assert!((breath_height(0.4, false) - 0.1).abs() < 1e-7);
    assert_eq!(breath_height(5.0, false), 0.2);
    assert_eq!(breath_height(-4.0, true), 0.0, "mode 2 adds 4.0 first");
    assert!((breath_height(-3.2, true) - 0.2).abs() < 1e-6);
}

#[test]
fn reseed_puts_all_64_trail_points_at_the_position_plus_height() {
    let mut worm = Worm::new([1.0, 2.0, 3.0]);
    worm.height = -1.1;
    reseed_trail(&mut worm);
    assert!(
        worm.trail
            .iter()
            .all(|p| p[0] == 1.0 && (p[1] - 0.9).abs() < 1e-6 && p[2] == 3.0)
    );
}

#[test]
fn state_9_normal_mode_roars_rays_and_hands_over_to_b_or_e() {
    let mut set = worms();
    set[0].state = worm_state::BREATH;
    set[0].target_heading = 0.0;
    let mut world = TestWorld::new();
    world.ray = Some(RayHit {
        object: 77,
        arms_event: true,
        point: [1.0, 2.0, 3.0],
        direction: [0.0, 0.0, -1.0],
    });
    let effects = worm_frame(&mut set, &mut world, TIMING);
    assert!(effects.contains(&SceneEffect::SetAnimation {
        entity: 0,
        animation: 0x194
    }));
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, SceneEffect::Sound { id: 0x13AF, .. }))
    );
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, SceneEffect::Sound { id, .. } if (0x13A8..0x13AB).contains(id)))
    );
    assert!(effects.contains(&SceneEffect::ArmEvent {
        key: 0x5346_4343,
        armed: true
    }));
    assert!(effects.iter().any(|e| matches!(e, SceneEffect::Damage { victim: DamageVictim::Object(77), amount, .. } if *amount == 0.8)));
    assert_eq!(set[0].flag, 1);
    // The animation ends with the flag set: state 0xE.
    world.finished = true;
    worm_frame(&mut set, &mut world, TIMING);
    assert_eq!(set[0].state, worm_state::ANIM_E);
    // Without the flag and with the player behind or far away: state 0xB.
    let mut set = worms();
    set[0].state = worm_state::BREATH;
    let mut world = TestWorld::new();
    world.finished = true;
    worm_frame(&mut set, &mut world, TIMING);
    assert_eq!(
        set[0].state,
        worm_state::ANIM_B,
        "player is far (dz = 20) so the facing test sends it to 0xB"
    );
}

#[test]
fn state_9_special_mode_gives_up_after_its_animation() {
    let mut set = worms();
    set[0].state = worm_state::BREATH;
    set[0].mode = 2;
    let mut world = TestWorld::new();
    world.attackers = 2;
    world.finished = true;
    world.rise = -3.0;
    let effects = worm_frame(&mut set, &mut world, TIMING);
    assert_eq!(set[0].mode, -1);
    assert_eq!(set[0].state, worm_state::RECOVER);
    assert_eq!(world.attackers, 1);
    assert!(effects.contains(&SceneEffect::SetAnimation {
        entity: 0,
        animation: 0x149
    }));
}

#[test]
fn state_9_special_mode_bursts_when_the_height_first_rises() {
    let mut set = worms();
    set[0].state = worm_state::BREATH;
    set[0].mode = 2;
    set[0].height = -1.0;
    let mut world = TestWorld::new();
    world.rise = -3.0;
    let effects = worm_frame(&mut set, &mut world, TIMING);
    assert!(effects.iter().any(|e| matches!(e, SceneEffect::Particle { template: 0x75B208, anchor: ParticleAnchor::Point(p), .. } if p[0] == -62.314)));
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, SceneEffect::CameraShake { .. })),
        "height 0.2 < 0.3 and rose from <= 0"
    );
    world.ratio = 0.05;
    set[0].height = -1.0;
    let effects = worm_frame(&mut set, &mut world, TIMING);
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, SceneEffect::Particle { .. }))
    );
}

#[test]
fn state_e_recovers_when_the_height_is_zero_or_the_animation_ends() {
    let mut set = worms();
    set[0].state = worm_state::ANIM_E;
    let mut world = TestWorld::new();
    world.attackers = 1;
    world.rise = 0.4;
    let effects = worm_frame(&mut set, &mut world, TIMING);
    assert_eq!(set[0].state, worm_state::ANIM_E, "still breathing");
    assert!(
        effects
            .iter()
            .any(|e| matches!(e, SceneEffect::CameraShake { .. }))
    );
    assert!(effects.contains(&SceneEffect::SetAnimation {
        entity: 0,
        animation: 0x198
    }));
    world.rise = -1.0;
    worm_frame(&mut set, &mut world, TIMING);
    assert_eq!(set[0].state, worm_state::RECOVER);
    assert_eq!(set[0].height, -1.1);
    assert_eq!(world.attackers, 0);
}

#[test]
fn state_a_turns_the_target_heading_by_a_limited_step_then_goes_to_e() {
    let mut set = worms();
    set[0].state = worm_state::ANIM_A;
    set[0].target_heading = -1.0;
    let mut world = TestWorld::new();
    let before = set[0].target_heading;
    worm_frame(&mut set, &mut world, TIMING);
    let step = (1.0 / 60.0) * TIMING.steps as f32 * std::f32::consts::FRAC_PI_2;
    assert!(
        (set[0].target_heading - before - step).abs() < 1e-5,
        "limited to dt * pi/2"
    );
    world.finished = true;
    worm_frame(&mut set, &mut world, TIMING);
    assert_eq!(set[0].state, worm_state::ANIM_E);
}

// ------------------------------------------------------ 0x368A10 blobs and 0x3697C0 ambience

struct NoGeometry {
    slides: usize,
    collides: usize,
    hits: usize,
    attaches: usize,
}

impl BlastGeometry for NoGeometry {
    fn slide_on_face(&mut self, _b: &mut Blob) {
        self.slides += 1;
    }
    fn collide(&mut self, _b: &mut Blob) {
        self.collides += 1;
    }
    fn find_hit_and_reflect(&mut self, _b: &mut Blob, _p: [f32; 3]) {
        self.hits += 1;
    }
    fn attach_to_nearest_face(&mut self, _b: &mut Blob) {
        self.attaches += 1;
    }
}

fn geometry() -> NoGeometry {
    NoGeometry {
        slides: 0,
        collides: 0,
        hits: 0,
        attaches: 0,
    }
}

fn update_input<'a>(
    players: &'a [[f32; 3]],
    rooms: &'a [Option<RoomBox>; 7],
) -> BlastUpdateInput<'a> {
    BlastUpdateInput {
        timing: TIMING,
        step_float: 1.0,
        players,
        rooms,
        stale_kind: 0,
    }
}

#[test]
fn schedule_table_has_the_measured_shape() {
    assert_eq!(BLAST_SCHEDULE.len(), 3);
    assert!(BLAST_SCHEDULE.iter().all(|e| e.kind == 2));
    assert_eq!(BLAST_SCHEDULE.map(|e| e.until), [2.5, 3.7, 6.5]);
    assert_eq!(BLAST_SCHEDULE[2].aim[0], [-17.79, 12.0, 33.33]);
    assert_eq!(BLAST_ROOM_IDS, [0x23, 0x25, 0x27, 0x28, 0x30, 0x31, 0x29]);
}

#[test]
fn update_advances_the_clock_and_fills_the_phase() {
    let mut state = BlastState::default();
    let rooms = [None; 7];
    let mut geo = geometry();
    state.blobs[1].position = [3.0, 4.0, 5.0];
    blast_update(&mut state, &update_input(&[], &rooms), &mut geo, &mut || 0);
    assert!((state.time - 1.0 / 60.0).abs() < 1e-7);
    assert_eq!(state.phase, 1);
    assert!(
        state.blobs[1].ring.iter().all(|p| *p == [3.0, 4.0, 5.0]),
        "blob 1's ring is pre-rolled while entries 0 and 1 run"
    );
    state.time = 3.8;
    blast_update(&mut state, &update_input(&[], &rooms), &mut geo, &mut || 0);
    assert_eq!(state.phase, 2);
    state.time = 7.0;
    blast_update(&mut state, &update_input(&[], &rooms), &mut geo, &mut || 0);
    assert_eq!(state.phase, 0);
}

#[test]
fn an_idle_clock_does_nothing_and_speed_decays_toward_one() {
    let mut state = BlastState::default();
    let rooms = [None; 7];
    let mut input = update_input(&[], &rooms);
    input.timing.steps = 0;
    blast_update(&mut state, &input, &mut geometry(), &mut || 0);
    assert_eq!(state.time, 0.0);
    state.blobs[2].speed_factor = 3.0;
    let input = update_input(&[], &rooms);
    blast_update(&mut state, &input, &mut geometry(), &mut || 0);
    let expected = (3.0f32 - 1.0) * 0.99 + 1.0;
    assert!((state.blobs[2].speed_factor - expected).abs() < 1e-6);
}

#[test]
fn a_blob_flies_toward_its_schedule_point_and_rolls_its_ring() {
    let mut state = BlastState::default();
    let rooms = [None; 7];
    let mut geo = geometry();
    state.blobs[0].count = 5;
    state.blobs[0].position = [-37.0, 12.0, 30.0];
    state.blobs[0].direction = [0.0, 0.0, 1.0];
    blast_update(
        &mut state,
        &update_input(&[], &rooms),
        &mut geo,
        &mut || 0x8000_0000,
    );
    let blob = &state.blobs[0];
    // One sub step only: the final move is dt * 4.0 along the direction.
    assert!(
        (blob.position[2] - (30.0 + 4.0 / 60.0)).abs() < 1e-3,
        "{:?}",
        blob.position
    );
    assert_eq!(blob.ring_index, 1);
    assert_eq!(blob.ring[0], blob.position);
    assert_eq!(
        geo.hits, 1,
        "not attached: the segment is tested against the room faces"
    );
    assert!(
        (blob.turn - 3.0).abs() < 1e-6,
        "a half-range random value leaves the turn rate alone"
    );
}

#[test]
fn leaving_the_room_box_attaches_to_the_nearest_face() {
    let mut state = BlastState::default();
    let mut rooms = [None; 7];
    rooms[0] = Some(RoomBox {
        min_x: 0.0,
        ground_y: 7.0,
        min_z: 0.0,
        max_x: 1.0,
        max_y: 50.0,
        max_z: 1.0,
    });
    state.blobs[0].count = 1;
    state.blobs[0].room = 0x23;
    state.blobs[0].position = [-37.0, 12.0, 30.0];
    let mut geo = geometry();
    blast_update(&mut state, &update_input(&[], &rooms), &mut geo, &mut || 0);
    assert_eq!(geo.attaches, 1);
    assert_eq!(
        state.blobs[0].position[1], 7.0,
        "the floor height is written to the work y"
    );
}

#[test]
fn attached_blob_outside_its_face_collides_and_resets_the_axis() {
    let mut state = BlastState::default();
    let rooms = [None; 7];
    state.blobs[0].count = 1;
    state.blobs[0].attached = 1;
    state.blobs[0].face = BlobFace {
        y_low: 0.0,
        y_high: 1.0,
        x_low: 0.0,
        x_high: 1.0,
        z_low: 0.0,
        z_high: 1.0,
        ..BlobFace::default()
    };
    state.blobs[0].axis = [1.0, 0.0, 0.0];
    state.blobs[0].position = [-37.0, 12.0, 30.0];
    struct Detach;
    impl BlastGeometry for Detach {
        fn collide(&mut self, blob: &mut Blob) {
            blob.attached = 0;
        }
    }
    blast_update(
        &mut state,
        &update_input(&[], &rooms),
        &mut Detach,
        &mut || 0,
    );
    assert_eq!(state.blobs[0].axis, [0.0, 1.0, 0.0]);
}

#[test]
fn split_deals_the_pieces_round_robin_and_rotates_the_children_by_80_degrees() {
    let mut state = BlastState::default();
    state.blobs[0].count = 6;
    state.blobs[0].direction = [0.0, 0.0, 1.0];
    state.blobs[0].axis = [0.0, 1.0, 0.0];
    for n in 0..6 {
        state.pieces[n].player = 0;
    }
    for piece in state.pieces.iter_mut().skip(7) {
        piece.player = 7;
    }
    state.pieces[6].player = 0;
    state.pieces[6].state = 3;
    blast_split(&mut state, 0);
    let counts: Vec<i32> = state.blobs.iter().map(|b| b.count).collect();
    assert_eq!(counts[..3], [2, 2, 2]);
    assert_eq!(
        state.pieces[6].player, 0,
        "a dead piece (status 3) stays with the source"
    );
    assert_eq!(state.pieces[0].player, 0);
    assert_eq!(state.pieces[1].player, 1);
    assert_eq!(state.pieces[2].player, 2);
    let a = state.blobs[1].direction;
    let b = state.blobs[2].direction;
    let cosine = a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    assert!(
        (cosine - (160.0f32.to_radians()).cos()).abs() < 1e-4,
        "children are 160 degrees apart: {cosine}"
    );
}

#[test]
fn ambience_follows_the_live_pieces_and_counts_unspawned_entries() {
    let mut state = BlastState::default();
    state.piece_count = 2;
    state.pieces[0].position = [10.0, 0.0, 0.0];
    state.pieces[1].position = [20.0, 0.0, 0.0];
    state.pieces[1].state = 3;
    let effect = blast_ambience(&mut state);
    // One live piece out of two: centroid (10, 0, 0), a tenth of the way.
    assert_eq!(state.ambient_position, [1.0, 0.0, 0.0]);
    // Entries 2..0x320 are not status 3, so they count as live (0x31F + 1) and the target is capped at 1.0.
    assert!(
        (state.ambient_volume - 0.1).abs() < 1e-6,
        "approach step 0.1"
    );
    assert!(
        matches!(effect, SceneEffect::LoopSound { id: 0x15A, range, pitch, flags: 4, .. } if range == 20.0 && pitch == 1.0)
    );
    for _ in 0..20 {
        blast_ambience(&mut state);
    }
    assert_eq!(state.ambient_volume, 1.0);
}

// ------------------------------------------------------------- 0x369940 pieces

fn pieces_input<'a>(
    players: &'a [PlayerSpot],
    rooms: &'a [Option<RoomBox>; 7],
) -> BlastPiecesInput<'a> {
    BlastPiecesInput {
        timing: TIMING,
        room_visible: true,
        doof: None,
        players,
        rooms,
        step_float: 1.0,
        stale_offset: [0.0; 3],
    }
}

#[test]
fn pieces_do_nothing_when_idle_hidden_or_stopped() {
    let rooms = [None; 7];
    let mut state = BlastState::default();
    assert!(
        blast_pieces(
            &mut state,
            &pieces_input(&[], &rooms),
            &mut geometry(),
            &mut || 0
        )
        .is_empty(),
        "inactive"
    );
    state.active = true;
    let mut input = pieces_input(&[], &rooms);
    input.room_visible = false;
    assert!(blast_pieces(&mut state, &input, &mut geometry(), &mut || 0).is_empty());
    let mut input = pieces_input(&[], &rooms);
    input.timing.steps = 0;
    assert!(blast_pieces(&mut state, &input, &mut geometry(), &mut || 0).is_empty());
}

#[test]
fn doof_object_rearms_the_countdown_and_its_absence_runs_it_down() {
    let rooms = [None; 7];
    let mut state = BlastState::default();
    state.active = true;
    let mut input = pieces_input(&[], &rooms);
    input.doof = Some([1.0, 2.0, 3.0]);
    blast_pieces(&mut state, &input, &mut geometry(), &mut || 0);
    assert_eq!(state.doof_countdown, 180);
    assert_eq!(state.doof_position, [1.0, 2.0, 3.0]);
    blast_pieces(
        &mut state,
        &pieces_input(&[], &rooms),
        &mut geometry(),
        &mut || 0,
    );
    assert_eq!(state.doof_countdown, 179);
}

#[test]
fn flying_piece_falls_spins_hurts_a_nearby_player_and_lands_below_the_floor() {
    let mut rooms = [None; 7];
    rooms[0] = Some(RoomBox {
        min_x: -100.0,
        ground_y: 0.0,
        min_z: -100.0,
        max_x: 100.0,
        max_y: 50.0,
        max_z: 100.0,
    });
    let mut state = BlastState::default();
    state.active = true;
    state.piece_count = 1;
    state.pieces[0].state = 1;
    state.pieces[0].room = 0x23;
    state.pieces[0].player = 2;
    state.pieces[0].position = [0.0, 0.01, 0.0];
    state.pieces[0].velocity = [1.0, 0.0, 0.0];
    let players = [PlayerSpot {
        position: [0.5, 0.0, 0.0],
        object: 99,
    }];
    let effects = blast_pieces(
        &mut state,
        &pieces_input(&players, &rooms),
        &mut geometry(),
        &mut || 0,
    );
    let piece = &state.pieces[0];
    assert!((piece.velocity[1] + 9.8 / 60.0).abs() < 1e-5);
    assert!((piece.position[0] - 1.0 / 60.0).abs() < 1e-6);
    assert!((piece.spin - std::f32::consts::PI / 60.0).abs() < 1e-6);
    assert!(effects.iter().any(|e| matches!(e, SceneEffect::Damage { victim: DamageVictim::Object(99), amount, direction, .. } if *amount == 0.01 && *direction == [0.0, -1.0, 0.0])));
    assert_eq!(
        state.blobs[2].count, 1,
        "only blob 2 owns the live piece before the tail"
    );
}

#[test]
fn piece_leaving_the_room_box_attaches_or_lands() {
    struct Attach(usize);
    impl BlastGeometry for Attach {
        fn attach_piece(&mut self, _p: &mut BlastPiece) {
            self.0 += 1;
        }
    }
    let mut rooms = [None; 7];
    rooms[0] = Some(RoomBox {
        min_x: -1.0,
        ground_y: 0.0,
        min_z: -1.0,
        max_x: 1.0,
        max_y: 5.0,
        max_z: 1.0,
    });
    let mut state = BlastState::default();
    state.active = true;
    state.piece_count = 2;
    for n in 0..2 {
        state.pieces[n].state = 1;
        state.pieces[n].room = 0x23;
        state.pieces[n].player = 7;
        state.pieces[n].attached = 1;
    }
    state.pieces[0].position = [5.0, 2.0, 0.0];
    state.pieces[1].position = [0.0, -3.0, 0.0];
    let mut geo = Attach(0);
    blast_pieces(&mut state, &pieces_input(&[], &rooms), &mut geo, &mut || 0);
    assert_eq!(geo.0, 1, "only the x exit attaches");
    assert_eq!(state.pieces[0].state, 0);
    assert_eq!(state.pieces[1].state, 0);
    assert_eq!(
        state.pieces[1].attached, 0,
        "falling through the floor detaches"
    );
}

#[test]
fn pieces_with_status_0_follow_and_the_tail_marks_room_objects() {
    let rooms = [None; 7];
    let mut state = BlastState::default();
    state.active = true;
    state.piece_count = 1;
    state.pieces[0].state = 0;
    state.pieces[0].player = 4;
    state.pieces[0].position = [3.0, 6.0, 9.0];
    for piece in state.pieces.iter_mut().skip(1) {
        piece.state = 3;
    }
    let effects = blast_pieces(
        &mut state,
        &pieces_input(&[], &rooms),
        &mut geometry(),
        &mut || 0,
    );
    assert!(
        !effects
            .iter()
            .any(|e| matches!(e, SceneEffect::Unsupported { .. })),
        "status 0 motion is ported (T1846 batch 8)"
    );
    assert_eq!(
        effects
            .iter()
            .filter(|e| matches!(
                e,
                SceneEffect::MarkRoomObject {
                    callback: 0x366440,
                    ..
                }
            ))
            .count(),
        7
    );
    assert_eq!(state.blobs[4].count, 1);
    assert_eq!(state.blobs[4].centroid, [3.0, 6.0, 9.0]);
}

#[test]
fn unspawned_pool_entries_count_toward_their_blob() {
    let rooms = [None; 7];
    let mut state = BlastState::default();
    state.active = true;
    state.piece_count = 0;
    for piece in state.pieces.iter_mut() {
        piece.state = 3;
    }
    state.pieces[10].state = 0;
    state.pieces[10].player = 5;
    blast_pieces(
        &mut state,
        &pieces_input(&[], &rooms),
        &mut geometry(),
        &mut || 0,
    );
    assert_eq!(state.blobs[5].count, 1);
}
