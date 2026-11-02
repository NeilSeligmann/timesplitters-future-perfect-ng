// SPDX-License-Identifier: GPL-3.0-or-later
//! FABRICATED fixtures. No original-game capture or Tier 2 claim.
use std::collections::VecDeque;
use tsfp_scene_compare::*;
use tsfp_scene_model::*;

fn id(slot: u64) -> Lifetime {
    Lifetime {
        slot,
        generation: 1,
    }
}
fn rect() -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: 640,
        height: 480,
    }
}
fn fixture() -> Frame {
    Frame {
        present: 7,
        gaps: Gaps::default(),
        draws: DrawList {
            sample: FrameSample {
                previous_tick: 40,
                current_tick: 41,
                sampling: Sampling::Original,
            },
            targets: vec![],
            cameras: vec![Camera {
                id: CameraId(id(1)),
                view: IDENTITY,
                projection: IDENTITY,
                position: [0.; 3],
                viewport: rect(),
                depth_range: [0., 1.],
                cut: false,
            }],
            models: vec![ModelInstance {
                id: InstanceId(id(2)),
                mesh: MeshRef(ContentId([1; 32])),
                material: MaterialId(id(3)),
                transform: IDENTITY,
                skin_palette: vec![IDENTITY],
                discontinuity: false,
            }],
            materials: vec![Material {
                id: MaterialId(id(3)),
                shader: ShaderRef(ContentId([2; 32])),
                textures: vec![TextureBinding {
                    slot: 0,
                    texture: TextureSource::Asset(TextureRef(ContentId([3; 32]))),
                    uv_set: 0,
                    uv_transform: [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]],
                    sampler: Sampler {
                        min: Filter::Linear,
                        mag: Filter::Linear,
                        mip: Filter::Nearest,
                        address: [AddressMode::Clamp; 3],
                        border: [0.; 4],
                        lod_min: 0.,
                        lod_max: 8.,
                        lod_bias: 0.,
                    },
                }],
                parameters: vec![[1.; 4]],
                raster: RasterState {
                    cull: Cull::Back,
                    front_face: Winding::Clockwise,
                    blend: Some(Blend {
                        colour: BlendComponent {
                            source: BlendFactor::One,
                            destination: BlendFactor::Zero,
                            operation: BlendOperation::Add,
                        },
                        alpha: BlendComponent {
                            source: BlendFactor::One,
                            destination: BlendFactor::Zero,
                            operation: BlendOperation::Add,
                        },
                        constant: [0.; 4],
                    }),
                    depth_compare: Some(Compare::LessEqual),
                    depth_write: true,
                    depth_bias_constant: 0,
                    depth_bias_slope: 0.,
                    stencil: None,
                    colour_write: [true; 4],
                },
            }],
            lights: vec![Light {
                id: LightId(id(4)),
                kind: LightKind::Point {
                    position: [0.; 3],
                    range: 10.,
                    attenuation: [1., 0., 0.],
                },
                diffuse: [1.; 4],
                specular: [0.; 4],
            }],
            effects: vec![EffectBatch {
                id: EffectId(id(5)),
                mesh: MeshRef(ContentId([4; 32])),
                material: MaterialId(id(3)),
                particles: vec![Particle {
                    id: ParticleId(id(6)),
                    transform: IDENTITY,
                    colour: [1.; 4],
                    uv_rect: [0., 0., 1., 1.],
                    discontinuity: false,
                }],
            }],
            passes: vec![DrawPass {
                camera: CameraId(id(1)),
                target: Target::Surface,
                extent: [640, 480],
                draws: vec![
                    Draw::Clear {
                        colour: Some([0.; 4]),
                        depth: Some(1.),
                        stencil: Some(0),
                    },
                    Draw::Mesh {
                        instance: InstanceId(id(2)),
                        lights: vec![LightId(id(4))],
                        scissor: rect(),
                    },
                    Draw::Particles {
                        batch: EffectId(id(5)),
                        lights: vec![],
                        scissor: rect(),
                    },
                ],
            }],
        },
    }
}
fn capture() -> Capture {
    Capture {
        contract_version: CONTRACT_VERSION,
        decoder_version: 1,
        original_revision: [1; 20],
        original_prebuilt: ContentId([2; 32]),
        route: ContentId([3; 32]),
        input: ContentId([4; 32]),
        stream: ContentId([5; 32]),
        resources: ContentId([6; 32]),
        first_present: 7,
        frame_count: 1,
    }
}
struct Source {
    frames: VecDeque<Frame>,
    reads: usize,
}
impl Source {
    fn new(frames: Vec<Frame>) -> Self {
        Self {
            frames: frames.into(),
            reads: 0,
        }
    }
}
impl FrameSource for Source {
    fn next_frame(&mut self, max_frame_bytes: usize) -> Result<Option<Frame>, String> {
        assert_eq!(max_frame_bytes, Limits::default().max_frame_bytes);
        self.reads += 1;
        Ok(self.frames.pop_front())
    }
}
fn run(a: Frame, b: Frame) -> Result<Comparison, ComparisonError> {
    compare(
        &capture(),
        &capture(),
        [2; 20],
        &mut Source::new(vec![a]),
        &mut Source::new(vec![b]),
        Limits::default(),
    )
}
fn different(change: impl FnOnce(&mut DrawList), field: &str) {
    let a = fixture();
    let mut b = a.clone();
    change(&mut b.draws);
    let err = run(a, b).unwrap_err();
    assert_eq!(err.present, Some(7));
    assert_eq!(err.frame_index, 0);
    match err.failure {
        Failure::Different { field: actual, .. } => assert_eq!(actual, field),
        other => panic!("expected field difference at {field}, got {other:?}"),
    }
}

#[test]
fn equal_frames_keep_provenance_and_geometry_counts() {
    let a = fixture();
    let result = run(a.clone(), a).unwrap();
    assert_eq!(result.compared_frames, 1);
    assert_eq!(result.compared_draws, 2);
    assert_eq!(result.capture, capture());
    assert_eq!(result.native_revision, [2; 20]);
}

#[test]
fn exact_float_bits_include_signed_zero() {
    different(
        |f| f.cameras[0].position[0] = -0.0,
        "draw_list.cameras[0].position[0]",
    );
    different(
        |f| f.materials[0].raster.depth_bias_slope = -0.0,
        "draw_list.materials[0].raster.depth_bias_slope",
    );
    different(
        |f| f.models[0].skin_palette[0][0][1] = -0.0,
        "draw_list.models[0].skin_palette[0][0][1]",
    );
}

#[test]
fn semantic_mutations_name_the_first_field() {
    different(
        |f| f.cameras[0].projection[0][0] = 2.,
        "draw_list.cameras[0].projection[0][0]",
    );
    different(
        |f| f.cameras[0].view[1][1] = 2.,
        "draw_list.cameras[0].view[1][1]",
    );
    different(
        |f| f.cameras[0].viewport.width -= 1,
        "draw_list.cameras[0].viewport.width",
    );
    different(|f| f.cameras[0].cut = true, "draw_list.cameras[0].cut");
    different(
        |f| f.models[0].id.0.generation += 1,
        "draw_list.models[0].id.generation",
    );
    different(
        |f| f.models[0].mesh.0.0[0] ^= 1,
        "draw_list.models[0].mesh[0]",
    );
    different(
        |f| f.models[0].transform[3][0] = 1.,
        "draw_list.models[0].transform[3][0]",
    );
    different(
        |f| f.materials[0].shader.0.0[0] ^= 1,
        "draw_list.materials[0].shader[0]",
    );
    different(
        |f| f.materials[0].textures[0].sampler.lod_bias = 1.,
        "draw_list.materials[0].textures[0].sampler.lod_bias",
    );
    different(
        |f| f.materials[0].textures[0].sampler.address[1] = AddressMode::Repeat,
        "draw_list.materials[0].textures[0].sampler.address[1]",
    );
    different(
        |f| f.materials[0].textures[0].uv_transform[0][1] = 1.,
        "draw_list.materials[0].textures[0].uv_transform[0][1]",
    );
    different(
        |f| f.materials[0].parameters[0][3] = 0.,
        "draw_list.materials[0].parameters[0][3]",
    );
    different(
        |f| f.materials[0].raster.depth_write = false,
        "draw_list.materials[0].raster.depth_write",
    );
    different(
        |f| f.materials[0].raster.blend.as_mut().unwrap().constant[0] = 1.,
        "draw_list.materials[0].raster.blend.constant[0]",
    );
    different(
        |f| f.materials[0].raster.colour_write[2] = false,
        "draw_list.materials[0].raster.colour_write[2]",
    );
    different(
        |f| {
            if let LightKind::Point { range, .. } = &mut f.lights[0].kind {
                *range = 11.;
            }
        },
        "draw_list.lights[0].kind.range",
    );
    different(
        |f| f.lights[0].kind = LightKind::Ambient,
        "draw_list.lights[0].kind",
    );
    different(
        |f| f.lights[0].diffuse[1] = 0.,
        "draw_list.lights[0].diffuse[1]",
    );
    different(
        |f| f.effects[0].particles[0].colour[2] = 0.,
        "draw_list.effects[0].particles[0].colour[2]",
    );
    different(
        |f| f.effects[0].particles[0].id.0.generation += 1,
        "draw_list.effects[0].particles[0].id.generation",
    );
    different(
        |f| f.passes[0].camera.0.slot += 1,
        "draw_list.passes[0].camera.slot",
    );
    different(
        |f| f.passes[0].target = Target::Offscreen(RenderTargetId(id(8))),
        "draw_list.passes[0].target",
    );
    different(
        |f| f.passes[0].draws.swap(1, 2),
        "draw_list.passes[0].draws[1]",
    );
    different(
        |f| {
            f.passes[0].draws.pop();
        },
        "draw_list.passes[0].draws.len",
    );
    different(
        |f| {
            if let Draw::Mesh { scissor, .. } = &mut f.passes[0].draws[1] {
                scissor.width -= 1;
            }
        },
        "draw_list.passes[0].draws[1].scissor.width",
    );
}

#[test]
fn identical_invalid_and_vacuous_frames_are_rejected() {
    let mut a = fixture();
    a.draws.models[0].material.0.generation = 4;
    assert!(matches!(
        run(a.clone(), a).unwrap_err().failure,
        Failure::Invalid {
            side: Side::Oracle,
            ..
        }
    ));
    let mut a = fixture();
    a.draws.cameras[0].view[0][0] = f32::NAN;
    assert!(matches!(
        run(a.clone(), a).unwrap_err().failure,
        Failure::Invalid { .. }
    ));
    let mut a = fixture();
    a.draws.passes[0].draws.truncate(1);
    assert_eq!(run(a.clone(), a).unwrap_err().failure, Failure::NoDraws);
    let mut a = fixture();
    a.draws.sample.sampling = Sampling::Interpolated { alpha: 1. };
    assert_eq!(
        run(a.clone(), a).unwrap_err().failure,
        Failure::NotOriginal { side: Side::Oracle }
    );
}

#[test]
fn gaps_from_either_source_never_compare_equal() {
    for side in [Side::Oracle, Side::Native] {
        for gaps in [
            Gaps {
                dropped_records: 1,
                ..Gaps::default()
            },
            Gaps {
                missing_resources: 1,
                ..Gaps::default()
            },
            Gaps {
                unsupported_states: 1,
                ..Gaps::default()
            },
            Gaps {
                unmatched_views: 1,
                ..Gaps::default()
            },
        ] {
            let (mut a, mut b) = (fixture(), fixture());
            match side {
                Side::Oracle => a.gaps = gaps,
                Side::Native => b.gaps = gaps,
            }
            assert_eq!(
                run(a, b).unwrap_err().failure,
                Failure::Incomplete { side, gaps }
            );
        }
    }
}

#[test]
fn provenance_and_declared_extent_prevent_equal_truncation() {
    let c = capture();
    let mut altered = c.clone();
    altered.resources.0[0] ^= 1;
    let err = compare(
        &c,
        &altered,
        [2; 20],
        &mut Source::new(vec![]),
        &mut Source::new(vec![]),
        Limits::default(),
    )
    .unwrap_err();
    assert_eq!(err.failure, Failure::Provenance);
    let mut c = capture();
    c.frame_count = 2;
    let err = compare(
        &c,
        &c,
        [2; 20],
        &mut Source::new(vec![fixture()]),
        &mut Source::new(vec![fixture()]),
        Limits::default(),
    )
    .unwrap_err();
    assert_eq!(err.compared_frames, 1);
    assert_eq!(
        err.failure,
        Failure::FrameCount {
            side: Side::Oracle,
            expected: 2,
            actual: 1
        }
    );
    let c = capture();
    let err = compare(
        &c,
        &c,
        [2; 20],
        &mut Source::new(vec![fixture(), fixture()]),
        &mut Source::new(vec![fixture()]),
        Limits::default(),
    )
    .unwrap_err();
    assert_eq!(
        err.failure,
        Failure::FrameCount {
            side: Side::Oracle,
            expected: 1,
            actual: 2
        }
    );
}

#[test]
fn bounded_walk_and_early_failure_do_not_consume_the_route() {
    let c = capture();
    let mut oracle = Source::new(vec![fixture(), fixture()]);
    let mut native = Source::new(vec![fixture(), fixture()]);
    let limits = Limits {
        max_field_visits: 1,
        ..Limits::default()
    };
    assert!(matches!(
        compare(&c, &c, [2; 20], &mut oracle, &mut native, limits)
            .unwrap_err()
            .failure,
        Failure::Limit { .. }
    ));
    assert_eq!((oracle.reads, native.reads), (1, 1));
    let mut limits = Limits::default();
    limits.max_frames = 0;
    assert!(matches!(
        compare(&c, &c, [2; 20], &mut oracle, &mut native, limits)
            .unwrap_err()
            .failure,
        Failure::Limit { .. }
    ));
    assert_eq!((oracle.reads, native.reads), (1, 1));
}

#[test]
fn sequences_ticks_and_decode_errors_are_not_eof() {
    let a = fixture();
    let mut b = a.clone();
    b.present += 1;
    assert_eq!(
        run(a, b).unwrap_err().failure,
        Failure::Present {
            side: Side::Native,
            expected: 7,
            actual: 8
        }
    );
    let mut c = capture();
    c.frame_count = 2;
    let mut second = fixture();
    second.present = 8;
    second.draws.sample.previous_tick = 39;
    second.draws.sample.current_tick = 40;
    let frames = vec![fixture(), second];
    let err = compare(
        &c,
        &c,
        [2; 20],
        &mut Source::new(frames.clone()),
        &mut Source::new(frames),
        Limits::default(),
    )
    .unwrap_err();
    assert_eq!(err.failure, Failure::TickRegression { side: Side::Oracle });
    struct Broken;
    impl FrameSource for Broken {
        fn next_frame(&mut self, _: usize) -> Result<Option<Frame>, String> {
            Err("truncated record".into())
        }
    }
    let c = capture();
    assert_eq!(
        compare(
            &c,
            &c,
            [2; 20],
            &mut Broken,
            &mut Source::new(vec![]),
            Limits::default()
        )
        .unwrap_err()
        .failure,
        Failure::Decode {
            side: Side::Oracle,
            message: "truncated record".into()
        }
    );
}

fn encoded(frame: &Frame, side: Side) -> Vec<u8> {
    let header = wire::Header {
        capture: capture(),
        side,
        producer_revision: if side == Side::Oracle {
            [1; 20]
        } else {
            [2; 20]
        },
    };
    let mut writer = wire::Writer::new(Vec::new(), header).unwrap();
    writer
        .frame(frame, Limits::default().max_frame_bytes)
        .unwrap();
    writer.finish().unwrap()
}

#[test]
fn canonical_transport_preserves_exact_fields_and_header() {
    let mut frame = fixture();
    frame.draws.cameras[0].position[1] = -0.0;
    let a = encoded(&frame, Side::Oracle);
    let b = encoded(&frame, Side::Native);
    assert_eq!(&a[..8], b"TSDRAW01");
    assert_eq!(&a[8..12], &1u32.to_le_bytes());
    let mut oracle = wire::Reader::new(a.as_slice()).unwrap();
    let mut native = wire::Reader::new(b.as_slice()).unwrap();
    assert_eq!(oracle.header().capture, capture());
    assert_eq!(native.header().side, Side::Native);
    let result = compare(
        &capture(),
        &capture(),
        [2; 20],
        &mut oracle,
        &mut native,
        Limits::default(),
    )
    .unwrap();
    assert_eq!(result.compared_draws, 2);
    let mut reader = wire::Reader::new(a.as_slice()).unwrap();
    let roundtrip = reader
        .next_frame(Limits::default().max_frame_bytes)
        .unwrap()
        .unwrap();
    assert_eq!(
        roundtrip.draws.cameras[0].position[1].to_bits(),
        (-0.0f32).to_bits()
    );
    assert_eq!(encoded(&roundtrip, Side::Oracle), a);
}

#[test]
fn canonical_reader_rejects_length_before_reading_payload() {
    let bytes = encoded(&fixture(), Side::Oracle);
    let prefix = 8 + wire::HEADER_BYTES;
    // Only the length prefix is available; a bounded reader must reject it
    // before attempting the absent body, not return UnexpectedEof instead.
    let mut malicious = bytes[..prefix].to_vec();
    malicious.extend_from_slice(&u64::MAX.to_le_bytes());
    let mut reader = wire::Reader::new(malicious.as_slice()).unwrap();
    assert_eq!(
        reader.next_frame(1024).unwrap_err(),
        "canonical frame byte limit"
    );
    assert_eq!(
        reader.next_frame(1024).unwrap_err(),
        "canonical reader failed previously"
    );
}

#[test]
fn canonical_reader_rejects_truncation_unknown_tags_and_extra_bytes() {
    let bytes = encoded(&fixture(), Side::Oracle);
    let prefix = 8 + wire::HEADER_BYTES;
    for cut in [prefix + 1, prefix + 7, bytes.len() - 1] {
        let mut reader = wire::Reader::new(&bytes[..cut]).unwrap();
        assert!(
            reader
                .next_frame(Limits::default().max_frame_bytes)
                .is_err(),
            "cut {cut}"
        );
    }
    let mut malformed = bytes.clone();
    let at = malformed.windows(8).position(|v| v == b"Original").unwrap();
    malformed[at..at + 8].copy_from_slice(b"Bogus___");
    let mut reader = wire::Reader::new(malformed.as_slice()).unwrap();
    assert_eq!(
        reader
            .next_frame(Limits::default().max_frame_bytes)
            .unwrap_err(),
        "unknown Sampling"
    );
    let mut extra = bytes.clone();
    let n = u64::from_le_bytes(extra[prefix..prefix + 8].try_into().unwrap());
    extra[prefix..prefix + 8].copy_from_slice(&(n + 1).to_le_bytes());
    extra.push(0);
    let mut reader = wire::Reader::new(extra.as_slice()).unwrap();
    assert_eq!(
        reader
            .next_frame(Limits::default().max_frame_bytes)
            .unwrap_err(),
        "trailing bytes in canonical frame"
    );
}

#[test]
fn canonical_vector_counts_and_decoded_allocations_are_bounded() {
    let mut bytes = encoded(&fixture(), Side::Oracle);
    // Frame: present(8), gaps(32), ticks(16), Original tag(9), targets length(4).
    let at = 8 + wire::HEADER_BYTES + 8 + 8 + 32 + 16 + 9;
    assert_eq!(&bytes[at..at + 4], &[0; 4]);
    bytes[at..at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    let mut reader = wire::Reader::new(bytes.as_slice()).unwrap();
    assert_eq!(
        reader
            .next_frame(Limits::default().max_frame_bytes)
            .unwrap_err(),
        "element count exceeds remaining frame"
    );
    // The encoded-byte budget alone is insufficient for short enum values:
    // their Rust representation can be larger than their serialized fields.
    let mut frame = fixture();
    frame.draws.passes[0].draws = vec![
        Draw::Clear {
            colour: None,
            depth: None,
            stencil: None
        };
        1000
    ];
    let bytes = encoded(&frame, Side::Oracle);
    let mut reader = wire::Reader::new(bytes.as_slice()).unwrap();
    let err = reader.next_frame(bytes.len()).unwrap_err();
    assert_eq!(err, "decoded allocation limit");
}

#[test]
fn canonical_writer_refuses_partial_or_over_limit_files() {
    let header = wire::Header {
        capture: capture(),
        side: Side::Oracle,
        producer_revision: [1; 20],
    };
    let writer = wire::Writer::new(Vec::new(), header.clone()).unwrap();
    assert!(writer.finish().is_err());
    let mut writer = wire::Writer::new(Vec::new(), header).unwrap();
    assert_eq!(
        writer.frame(&fixture(), 1).unwrap_err().to_string(),
        "canonical frame byte limit"
    );
    assert!(writer.finish().is_err());
}

#[test]
fn cli_compares_independent_files_and_rejects_wrong_roles() {
    use std::process::Command;
    // Per-process unique private files; no network, clock, shared fixture or port.
    let dir = std::env::temp_dir().join(format!("tsfp-scene-compare-{}", std::process::id()));
    std::fs::create_dir(&dir).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(dir.clone());
    let a = dir.join("oracle.drawlist");
    let b = dir.join("native.drawlist");
    std::fs::write(&a, encoded(&fixture(), Side::Oracle)).unwrap();
    std::fs::write(&b, encoded(&fixture(), Side::Native)).unwrap();
    let run = |a: &std::path::Path, b: &std::path::Path| {
        Command::new(env!("CARGO_BIN_EXE_scene-compare"))
            .args([a, b])
            .output()
            .unwrap()
    };
    let result = run(&a, &b);
    assert!(result.status.success(), "{:?}", result);
    assert!(result.stdout.starts_with(b"EQUAL "));
    let mut changed = fixture();
    changed.draws.cameras[0].position[0] = 1.;
    std::fs::write(&b, encoded(&changed, Side::Native)).unwrap();
    let result = run(&a, &b);
    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stdout).contains("draw_list.cameras[0].position[0]"));
    assert_eq!(run(&b, &a).status.code(), Some(2));
}
