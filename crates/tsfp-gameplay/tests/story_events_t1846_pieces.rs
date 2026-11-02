//! T1846 batch 8: the whole `0x369940` piece update against the original running under Unicorn (MEASURED on synthetic
//! worlds; engine effects are logging stubs in the golden, see `tools/t1846_gold/pieces.py`).
mod common;
use common::{Call, Case, parse};
use tsfp_gameplay::story_events::face_geometry::*;
use tsfp_gameplay::story_events::group_paths::Lcg;
use tsfp_gameplay::story_events::hooks_scene::*;

const G: u32 = 0x1030000;
const P: u32 = 0x1200000;
const PL: u32 = 0x1300000;
const EXT: u32 = 0x1311000;

fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn face_at(case: &Case, address: u32) -> BlobFace {
    let f = |offset: u32| case.input_f(address + offset);
    BlobFace {
        id: address,
        x0: f(8),
        y_low: f(0xC),
        z0: f(0x10),
        x1: f(0x14),
        y_high: f(0x18),
        z1: f(0x1C),
        x_low: f(0x20),
        z_low: f(0x24),
        x_high: f(0x28),
        z_high: f(0x2C),
    }
}

fn world_from(case: &Case) -> (FaceWorld, [Option<RoomBox>; 7]) {
    let mut world = FaceWorld {
        heading_pending: case.input(G + 0x3360),
        ..FaceWorld::default()
    };
    let mut rooms = [None; 7];
    for n in 0..7u32 {
        let base = 0x1040000 + 4 + 24 * n;
        let f = |o: u32| case.input_f(base + o);
        let area = RoomBox {
            min_x: f(0),
            ground_y: f(4),
            min_z: f(8),
            max_x: f(12),
            max_y: f(16),
            max_z: f(20),
        };
        world.boxes[n as usize] = area;
        rooms[n as usize] = Some(area);
    }
    for room in 0..0x40u32 {
        let object = case.input(0x1050000 + 4 * room);
        if object == 0 {
            continue;
        }
        let count = case.input(object + 0x1E4);
        let list = case.input(object + 0x1F4);
        let mut faces = Vec::new();
        for n in 0..count {
            let face = case.input(list + 4 * n);
            world.faces.insert(face, face_at(case, face));
            faces.push(face);
        }
        world.rooms.insert(room as i32, RoomObject { faces });
    }
    for region in 0..0x40u32 {
        let list = case.input(0x1090000 + region * 0xB4 + 4);
        if list == 0 {
            continue;
        }
        let count = case.input(list);
        world.region_portals.insert(
            region as i32,
            (0..count)
                .map(|n| case.input(list + 4 + 4 * n) as usize)
                .collect(),
        );
    }
    for k in 0..64u32 {
        let pair = case.input(0x1094000 + 4 * k);
        if pair == 0 {
            break;
        }
        world
            .portals
            .push((case.input(pair) as i32, case.input(pair + 4) as i32));
    }
    world
        .faces
        .entry(0x1080000)
        .or_insert_with(|| face_at(case, 0x1080000));
    (world, rooms)
}

fn state_from(case: &Case, world: &FaceWorld) -> BlastState {
    let mut state = BlastState::default();
    state.active = case.input(G + 0x3364) != 0;
    state.piece_count = case.input(G + 4) as i32;
    state.phase = case.input(G + 8);
    state.doof_countdown = case.input(G + 0x360C) as i32;
    state.doof_position = case.vec3(G + 0x3610);
    for (b, blob) in state.blobs.iter_mut().enumerate() {
        let base = G + 0xC + b as u32 * 0x63C;
        blob.ring_index = case.input(base + 0x44) as i32;
        blob.speed_factor = case.input_f(base + 0x4C);
        for (slot, entry) in blob.ring.iter_mut().enumerate() {
            *entry = case.vec3(base + 0x50 + 12 * slot as u32);
        }
    }
    for index in 0..state.piece_count.clamp(0, BLAST_MAX_PIECES as i32) as u32 {
        let at = P + 0x4C * index;
        let w = |n: u32| case.input(at + 4 * n);
        let f = |n: u32| case.input_f(at + 4 * n);
        let piece = &mut state.pieces[index as usize];
        piece.position = [f(0), f(1), f(2)];
        piece.heading_a = f(3);
        piece.heading_b = f(4);
        piece.spin = f(5);
        piece.velocity = [f(6), f(7), f(8)];
        piece.attached = w(9) as i32;
        piece.room = w(10) as i32;
        piece.state = w(11);
        piece.timer = w(13) as i32;
        piece.countdown = f(14);
        piece.face = world.face_copy(w(15));
        piece.object = w(16);
        piece.player = w(17) as usize;
        piece.frames_back = w(18) as i32;
    }
    state
}

/// The calls the Rust effects stand for (callee, selected argument words), in order.
fn expected_calls(effects: &[SceneEffect], case: &Case) -> Vec<(u32, Vec<u32>)> {
    let piece_address = |index: usize| P + 0x4C * index as u32;
    let mut out = Vec::new();
    for effect in effects {
        match effect {
            SceneEffect::ObjectVector { object, position } => {
                out.push((
                    0x10E730,
                    vec![
                        *object,
                        position[0].to_bits(),
                        position[1].to_bits(),
                        position[2].to_bits(),
                    ],
                ));
            }
            SceneEffect::Decal {
                kind,
                flags,
                scale,
                room,
                piece,
                offset,
            } => {
                out.push((0x80F20, vec![*kind]));
                out.push((
                    0x1128C0,
                    vec![
                        0x1000 + kind,
                        1,
                        *flags,
                        0,
                        scale.to_bits(),
                        1,
                        *room as u32,
                        piece_address(*piece),
                        offset[0].to_bits(),
                        offset[1].to_bits(),
                        offset[2].to_bits(),
                    ],
                ));
            }
            SceneEffect::Particle {
                template,
                anchor: ParticleAnchor::Piece(index),
                vector,
            } => {
                out.push((
                    0x111AE0,
                    vec![
                        0,
                        G + template,
                        1,
                        piece_address(*index),
                        vector[0].to_bits(),
                        vector[1].to_bits(),
                        vector[2].to_bits(),
                    ],
                ));
            }
            SceneEffect::Sound { id, position } => {
                out.push((
                    0x45820,
                    vec![
                        *id,
                        position[0].to_bits(),
                        position[1].to_bits(),
                        position[2].to_bits(),
                    ],
                ));
            }
            SceneEffect::MarkRoomObject { id, callback } => {
                out.push((0x9E720, vec![case.input(0x1050000 + 4 * id), *callback]));
            }
            _ => {}
        }
    }
    out
}

/// The same selection from the golden's stub log.
fn logged_calls(calls: &[Call]) -> Vec<(u32, Vec<u32>)> {
    let mut out = Vec::new();
    for call in calls {
        let a = &call.args;
        match call.callee {
            0x10E730 => out.push((0x10E730, vec![a[0], a[2], a[3], a[4]])),
            0x80F20 => out.push((0x80F20, vec![a[0]])),
            0x1128C0 => out.push((
                0x1128C0,
                vec![
                    a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[9], a[10], a[11],
                ],
            )),
            0x111AE0 => out.push((0x111AE0, vec![a[0], a[1], a[2], a[3], a[5], a[6], a[7]])),
            // The sound record: id at +0, position at +0x1C (after the 0x45820 pointer argument).
            0x45820 => out.push((0x45820, vec![a[1], a[1 + 7], a[1 + 8], a[1 + 9]])),
            0x9E720 => out.push((0x9E720, vec![a[0], a[1]])),
            _ => {}
        }
    }
    out
}

#[test]
fn blast_pieces_matches_the_original() {
    let cases = parse(include_str!("data/t1846/pieces_369940.golden"));
    assert!(cases.len() >= 40, "{} cases", cases.len());
    let mut seen = std::collections::BTreeSet::new();
    for (n, case) in cases.iter().enumerate() {
        let (mut world, rooms) = world_from(case);
        let mut state = state_from(case, &world);
        let steps = case.input(0x4E7944) as i32;
        let visible = case.input(0xF00000) != 0;
        let doof = (case.input(0xF00004) != 0).then(|| case.vec3(EXT + 0xBB4));
        let players: Vec<PlayerSpot> = (0..case.input(0x790950))
            .map(|k| PlayerSpot {
                position: case.vec3(PL + k * 0x1584 + 0xB4),
                object: case.input(PL + k * 0x1584 + 0x14),
            })
            .collect();
        let input = BlastPiecesInput {
            timing: Timing { steps, rate: 60 },
            room_visible: visible,
            doof,
            players: &players,
            rooms: &rooms,
            step_float: case.input_f(0x4E794C),
            stale_offset: [0.0; 3],
        };
        let mut lcg = Lcg {
            state: u64::from(case.input(0x4E7950)) | u64::from(case.input(0x4E7954)) << 32,
            draws: case.input(0x7497C8),
        };
        let count = state.piece_count.clamp(0, BLAST_MAX_PIECES as i32) as u32;
        let before: Vec<BlastPiece> = state.pieces[..count as usize].to_vec();
        let effects = {
            let mut geometry = RoomGeometry { world: &mut world };
            blast_pieces(&mut state, &input, &mut geometry, &mut || lcg.next())
        };
        for index in 0..count {
            let at = P + 0x4C * index;
            let piece = &state.pieces[index as usize];
            let f = |k: u32| case.output_f(at + 4 * k);
            let ctx = format!(
                "case {n} piece {index} (status {} -> {}, attached {} -> {}, phase {}, steps {steps}, frames_back {})",
                before[index as usize].state,
                piece.state,
                before[index as usize].attached,
                piece.attached,
                state.phase,
                piece.frames_back
            );
            for axis in 0..3 {
                assert!(
                    same(piece.position[axis], f(axis as u32)),
                    "{ctx} position{axis} got {} want {}",
                    piece.position[axis],
                    f(axis as u32)
                );
                assert!(
                    same(piece.velocity[axis], f(6 + axis as u32)),
                    "{ctx} velocity{axis} got {} want {}",
                    piece.velocity[axis],
                    f(6 + axis as u32)
                );
            }
            assert!(same(piece.heading_a, f(3)), "{ctx} heading_a");
            assert!(same(piece.heading_b, f(4)), "{ctx} heading_b");
            assert!(same(piece.spin, f(5)), "{ctx} spin");
            assert_eq!(
                piece.attached as u32,
                case.output(at + 36),
                "{ctx} attached"
            );
            assert_eq!(piece.room as u32, case.output(at + 40), "{ctx} room");
            assert_eq!(piece.state, case.output(at + 44), "{ctx} status");
            assert_eq!(piece.timer as u32, case.output(at + 52), "{ctx} timer");
            assert!(same(piece.countdown, f(14)), "{ctx} countdown");
            assert_eq!(piece.face.id, case.output(at + 60), "{ctx} face");
            seen.insert((
                before[index as usize].state,
                piece.state,
                before[index as usize].attached,
            ));
        }
        for (b, blob) in state.blobs.iter().enumerate() {
            let base = G + 0xC + b as u32 * 0x63C;
            for axis in 0..3 {
                assert!(
                    same(
                        blob.centroid[axis],
                        case.output_f(base + 0x18 + 4 * axis as u32)
                    ),
                    "case {n} blob {b} centroid{axis}"
                );
                assert!(
                    same(blob.sum[axis], case.output_f(base + 0x30 + 4 * axis as u32)),
                    "case {n} blob {b} sum{axis}"
                );
            }
            assert_eq!(
                blob.count as u32,
                case.output(base + 0x40),
                "case {n} blob {b} count"
            );
        }
        assert_eq!(
            state.doof_countdown as u32,
            case.output(G + 0x360C),
            "case {n} doof countdown"
        );
        for axis in 0..3 {
            assert!(
                same(
                    state.doof_position[axis],
                    case.output_f(G + 0x3610 + 4 * axis as u32)
                ),
                "case {n} doof position"
            );
        }
        assert_eq!(
            world.heading_pending,
            case.output(G + 0x3360),
            "case {n} flag"
        );
        assert_eq!(lcg.state as u32, case.output(0x4E7950), "case {n} rng low");
        assert_eq!(
            (lcg.state >> 32) as u32,
            case.output(0x4E7954),
            "case {n} rng high"
        );
        assert_eq!(lcg.draws, case.output(0x7497C8), "case {n} rng draws");
        assert_eq!(
            expected_calls(&effects, case),
            logged_calls(&case.calls),
            "case {n} effects"
        );
    }
    // Coverage: the launch (0 -> 1), the death (2 -> 3), attached and free follow motion and a status 1 piece all occur.
    for wanted in [
        (0, 1, 0),
        (0, 1, 1),
        (2, 3, 0),
        (2, 3, 1),
        (0, 0, 1),
        (0, 0, 0),
        (2, 2, 1),
        (1, 0, 1),
    ] {
        assert!(
            seen.contains(&wanted),
            "branch {wanted:?} not covered; saw {seen:?}"
        );
    }
}

/// `blast_step` runs frame, blob update and piece update over the golden worlds for many frames without a panic, keeps
/// the status values in 0..=3 and only emits effects that exist in the image (smoke test of the composition).
#[test]
fn blast_step_composes_frame_update_and_pieces_over_golden_worlds() {
    let cases = parse(include_str!("data/t1846/pieces_369940.golden"));
    let mut frames_with_effects = 0;
    for case in cases.iter().take(20) {
        let (mut world, rooms) = world_from(case);
        let mut state = state_from(case, &world);
        state.active = true;
        state.piece_count = state.piece_count.max(4);
        let players: Vec<PlayerSpot> = Vec::new();
        let mut lcg = Lcg {
            state: 0x1234_5678,
            draws: 0,
        };
        for frame in 0..40 {
            let timing = Timing { steps: 1, rate: 60 };
            let frame_input = BlastInput {
                timing,
                room_visible: true,
                mode_byte: 10,
                local_players: 1,
            };
            let update = BlastUpdateInput {
                timing,
                step_float: 1.0,
                players: &[[1.0, 1.0, 1.0]],
                rooms: &rooms,
                stale_kind: 2,
            };
            let pieces = BlastPiecesInput {
                timing,
                room_visible: true,
                doof: None,
                players: &players,
                rooms: &rooms,
                step_float: 1.0,
                stale_offset: [0.0; 3],
            };
            let mut geometry = RoomGeometry { world: &mut world };
            let effects = blast_step(
                &mut state,
                &frame_input,
                &update,
                &pieces,
                &mut geometry,
                &mut || lcg.next(),
            );
            frames_with_effects += usize::from(!effects.is_empty());
            assert!(state.pieces.iter().all(|p| p.state <= 3), "frame {frame}");
        }
    }
    assert!(frames_with_effects > 0);
}
