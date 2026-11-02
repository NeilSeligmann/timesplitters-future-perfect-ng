//! T1846 batch 8: face geometry callees against emulated-original golden vectors (MEASURED, xemu-level arithmetic).
mod common;
use common::{Case, parse};
use tsfp_gameplay::story_events::face_geometry::*;
use tsfp_gameplay::story_events::hooks_scene::{BLAST_ROOM_IDS, Blob, BlobFace, RoomBox};

fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

const B: u32 = 0x1020000;
const HEADER: u32 = B + 0x5F0;
const G: u32 = 0x1030000;

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

/// Rebuilds the world from the oracle memory image (layout in `tools/t1846_gold/face.py`).
fn world_from(case: &Case) -> FaceWorld {
    let mut world = FaceWorld {
        heading_pending: case.input(G + 0x3360),
        ..FaceWorld::default()
    };
    for (n, &id) in BLAST_ROOM_IDS.iter().enumerate() {
        let base = 0x1040000 + 4 + 24 * n as u32;
        let f = |o: u32| case.input_f(base + o);
        let _ = id;
        world.boxes[n] = RoomBox {
            min_x: f(0),
            ground_y: f(4),
            min_z: f(8),
            max_x: f(12),
            max_y: f(16),
            max_z: f(20),
        };
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
        let entries = (0..count)
            .map(|n| case.input(list + 4 + 4 * n) as usize)
            .collect();
        world.region_portals.insert(region as i32, entries);
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
    // The null face the generator may point at: an all-zero record (id 0x1080000).
    world
        .faces
        .entry(0x1080000)
        .or_insert_with(|| face_at(case, 0x1080000));
    world
}

fn work_from(case: &Case) -> FaceWork {
    FaceWork {
        p: case.vec3(HEADER),
        heading: case.vec3(HEADER + 0xC),
        q: case.vec3(HEADER + 0x18),
        attached: case.input(HEADER + 0x24) as i32,
        room: case.input(HEADER + 0x28) as i32,
        state: case.input(HEADER + 0x2C) as i32,
        face: case.input(HEADER + 0x3C),
    }
}

fn check_work(case: &Case, work: &FaceWork, label: &str, n: usize) {
    for i in 0..3u32 {
        assert!(
            same(work.p[i as usize], case.output_f(HEADER + 4 * i)),
            "{label} case {n} p{i}"
        );
        assert!(
            same(
                work.heading[i as usize],
                case.output_f(HEADER + 0xC + 4 * i)
            ),
            "{label} case {n} heading{i}"
        );
        assert!(
            same(work.q[i as usize], case.output_f(HEADER + 0x18 + 4 * i)),
            "{label} case {n} q{i}"
        );
    }
    assert_eq!(
        work.attached as u32,
        case.output(HEADER + 0x24),
        "{label} case {n} attached"
    );
    assert_eq!(
        work.room as u32,
        case.output(HEADER + 0x28),
        "{label} case {n} room"
    );
    assert_eq!(
        work.state as u32,
        case.output(HEADER + 0x2C),
        "{label} case {n} state"
    );
    assert_eq!(
        work.face,
        case.output(HEADER + 0x3C),
        "{label} case {n} face"
    );
}

#[test]
fn clamp_to_face_segment_matches_original() {
    let cases = parse(include_str!("data/t1846/face_clamp_3675e0.golden"));
    assert!(cases.len() >= 40);
    let mut with_normal = 0;
    for (n, case) in cases.iter().enumerate() {
        let (p, q) = (case.vec3(0x1010000), case.vec3(0x1010018));
        let mut work = FaceWork {
            p,
            q,
            ..FaceWork::default()
        };
        let mut edge = [0.0; 6];
        for (i, e) in edge.iter_mut().enumerate() {
            *e = case.input_f(0x1020008 + 4 * i as u32);
        }
        let face = BlobFace {
            x0: edge[0],
            y_low: edge[1],
            z0: edge[2],
            x1: edge[3],
            y_high: edge[4],
            z1: edge[5],
            ..BlobFace::default()
        };
        let mut flag = case.input(0x1033360);
        let wrote = case.input(0x1040000) != 0;
        let got = clamp_to_face_segment(&mut work, &face, &mut flag, wrote);
        for i in 0..3u32 {
            assert!(
                same(work.p[i as usize], case.output_f(0x1010000 + 4 * i)),
                "case {n} p{i}"
            );
            assert!(
                same(work.q[i as usize], case.output_f(0x1010018 + 4 * i)),
                "case {n} q{i}"
            );
            assert!(
                same(work.heading[i as usize], case.output_f(0x101000C + 4 * i)),
                "case {n} w{i}"
            );
        }
        assert_eq!(flag, case.output(0x1033360), "case {n} flag");
        if let Some(normal) = got {
            with_normal += 1;
            for i in 0..3u32 {
                assert!(
                    same(normal[i as usize], case.output_f(0x1010400 + 4 * i)),
                    "case {n} normal{i}"
                );
            }
        }
    }
    assert!(with_normal > 5);
}

#[test]
fn bounce_inside_face_matches_original() {
    let cases = parse(include_str!("data/t1846/face_bounce_3680a0.golden"));
    assert!(cases.len() >= 40);
    for (n, case) in cases.iter().enumerate() {
        let world = world_from(case);
        let mut work = work_from(case);
        let face = world.face_copy(work.face);
        bounce_inside_face(&mut work, &face);
        check_work(case, &work, "bounce", n);
    }
}

#[test]
fn collide_faces_matches_original() {
    let cases = parse(include_str!("data/t1846/face_collide_368280.golden"));
    assert!(cases.len() >= 40);
    let (mut moved, mut stayed) = (0, 0);
    for (n, case) in cases.iter().enumerate() {
        let mut world = world_from(case);
        let mut work = work_from(case);
        let result = collide_faces(&mut world, &mut work);
        assert_eq!(u32::from(result), case.eax & 1, "collide case {n} result");
        check_work(case, &work, "collide", n);
        assert_eq!(
            world.heading_pending,
            case.output(G + 0x3360),
            "collide case {n} flag"
        );
        if result {
            moved += 1;
        } else {
            stayed += 1;
        }
    }
    assert!(
        moved > 3 && stayed > 3,
        "branch coverage: {moved} moved, {stayed} failed"
    );
}

#[test]
fn attach_nearest_face_matches_original() {
    let cases = parse(include_str!("data/t1846/face_attach_367ce0.golden"));
    assert!(cases.len() >= 40);
    let (mut attached, mut other) = (0, 0);
    for (n, case) in cases.iter().enumerate() {
        let mut world = world_from(case);
        let mut work = work_from(case);
        let normal = attach_nearest_face(&mut world, &mut work);
        check_work(case, &work, "attach", n);
        assert_eq!(
            world.heading_pending,
            case.output(G + 0x3360),
            "attach case {n} flag"
        );
        // The out pointer (stack argument) receives the normal only when a face was attached and the pointer is non null.
        let out_given = case.input(0x1021100) != 0;
        if let (Some(normal), true) = (normal, out_given) {
            attached += 1;
            for i in 0..3u32 {
                assert!(
                    same(normal[i as usize], case.output_f(0x1021000 + 4 * i)),
                    "attach case {n} normal{i}"
                );
            }
        } else {
            other += 1;
        }
    }
    assert!(
        attached > 3 && other > 3,
        "branch coverage: {attached} attached, {other} other"
    );
}

#[test]
fn find_hit_and_reflect_matches_original() {
    let cases = parse(include_str!("data/t1846/face_hit_367820.golden"));
    assert!(cases.len() >= 40);
    let mut hits = 0;
    for (n, case) in cases.iter().enumerate() {
        let mut world = world_from(case);
        let mut blob = Blob {
            position: case.vec3(B),
            work_position: case.vec3(HEADER),
            work_direction: case.vec3(HEADER + 0x18),
            heading: case.vec3(HEADER + 0xC),
            attached: case.input(HEADER + 0x24) as i32,
            room: case.input(HEADER + 0x28) as i32,
            axis: case.vec3(B + 0x24),
            ..Blob::default()
        };
        blob.face = world.face_copy(case.input(HEADER + 0x3C));
        let previous = case.vec3(0x1021000);
        find_hit_and_reflect(&mut world, &mut blob, previous);
        let work = FaceWork::from_blob(&blob);
        check_work(case, &work, "hit", n);
        for i in 0..3u32 {
            assert!(
                same(blob.axis[i as usize], case.output_f(B + 0x24 + 4 * i)),
                "hit case {n} axis{i}"
            );
        }
        assert_eq!(
            world.heading_pending,
            case.output(G + 0x3360),
            "hit case {n} flag"
        );
        if blob.axis != case.vec3(B + 0x24) {
            hits += 1;
        }
    }
    assert!(hits > 3, "{hits} hits");
}
