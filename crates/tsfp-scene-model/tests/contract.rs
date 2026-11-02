// SPDX-License-Identifier: GPL-3.0-or-later
//! FABRICATED contract fixtures, not captured game or pixel evidence.
use std::sync::Arc;
use tsfp_scene_model::*;

fn id(slot: u64) -> Lifetime {
    Lifetime {
        slot,
        generation: 1,
    }
}
fn content(n: u8) -> ContentId {
    ContentId([n; 32])
}
fn rect() -> Rect {
    Rect {
        x: 0,
        y: 0,
        width: 640,
        height: 480,
    }
}
fn fixture() -> DrawList {
    DrawList {
        sample: FrameSample {
            previous_tick: 40,
            current_tick: 41,
            sampling: Sampling::Original,
        },
        targets: vec![RenderTarget {
            id: RenderTargetId(id(8)),
            extent: [640, 480],
            colour: ColourFormat::Rgba8Unorm,
            depth: Some(DepthFormat::Depth24Stencil8),
        }],
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
            mesh: MeshRef(content(1)),
            material: MaterialId(id(3)),
            transform: IDENTITY,
            skin_palette: vec![IDENTITY],
            discontinuity: false,
        }],
        materials: vec![Material {
            id: MaterialId(id(3)),
            shader: ShaderRef(content(2)),
            textures: vec![TextureBinding {
                slot: 0,
                texture: TextureSource::Asset(TextureRef(content(3))),
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
                blend: None,
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
            kind: LightKind::Ambient,
            diffuse: [1.; 4],
            specular: [0.; 4],
        }],
        effects: vec![EffectBatch {
            id: EffectId(id(5)),
            mesh: MeshRef(content(4)),
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
    }
}
fn error(frame: &DrawList) -> ContractError {
    match frame.validate() {
        Ok(_) => panic!("invalid frame admitted"),
        Err(e) => e,
    }
}

#[test]
fn original_uses_current_and_interpolation_respects_cuts() {
    let mut sample = fixture().sample;
    assert_eq!(sample.current_weight(false), Ok(1.));
    for alpha in [0., 0.25, 0.75, 1.] {
        sample.sampling = Sampling::Interpolated { alpha };
        assert_eq!(sample.current_weight(false), Ok(alpha));
        assert_eq!(sample.current_weight(true), Ok(1.));
    }
    sample.previous_tick = sample.current_tick;
    sample.sampling = Sampling::Interpolated { alpha: 0. };
    assert_eq!(sample.current_weight(false), Ok(1.));
}

#[test]
fn malformed_intervals_and_fractions_fail_before_sampling() {
    let mut sample = fixture().sample;
    for alpha in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
        sample.sampling = Sampling::Interpolated { alpha };
        assert_eq!(sample.validate(), Err(ContractError::InvalidAlpha));
    }
    sample.sampling = Sampling::Interpolated { alpha: 0.5 };
    sample.previous_tick = 39;
    assert_eq!(sample.validate(), Err(ContractError::InvalidInterval));
    sample.sampling = Sampling::Original;
    assert!(sample.validate().is_ok());
    sample.previous_tick = u64::MAX;
    assert_eq!(sample.validate(), Err(ContractError::InvalidInterval));
}

#[test]
fn repeated_display_samples_leave_endpoints_immutable() {
    let frame = fixture();
    let current = Arc::new(SceneState {
        tick: 41,
        targets: frame.targets,
        cameras: frame.cameras,
        models: frame.models,
        materials: frame.materials,
        lights: frame.lights,
        effects: frame.effects,
    });
    let mut previous = (*current).clone();
    previous.tick = 40;
    previous.models[0].transform[3][0] = -3.;
    let endpoints = SceneFrame {
        previous: Arc::new(previous),
        current,
    };
    let before = ((*endpoints.previous).clone(), (*endpoints.current).clone());
    for alpha in [0., 0.5, 1., 0.25] {
        assert_eq!(
            endpoints
                .sample(Sampling::Interpolated { alpha })
                .unwrap()
                .current_weight(false),
            Ok(alpha)
        );
    }
    assert_eq!(*endpoints.previous, before.0);
    assert_eq!(*endpoints.current, before.1);
}

#[test]
fn backend_receives_all_passes_in_exact_submission_order() {
    #[derive(Default)]
    struct Backend(Vec<DrawPass>);
    impl SceneRenderer for Backend {
        type Error = ();
        fn submit(&mut self, frame: ValidatedDrawList<'_>) -> Result<(), ()> {
            self.0.extend_from_slice(&frame.frame().passes);
            Ok(())
        }
    }
    let mut frame = fixture();
    let mut offscreen = frame.passes[0].clone();
    offscreen.target = Target::Offscreen(RenderTargetId(id(8)));
    offscreen.draws.reverse();
    frame.passes.insert(0, offscreen);
    let before = frame.clone();
    let mut backend = Backend::default();
    backend.submit(frame.validate().unwrap()).unwrap();
    assert_eq!(backend.0, frame.passes);
    assert_eq!(frame, before);
}

#[test]
fn stale_and_missing_bindings_fail_atomically() {
    let mut frame = fixture();
    frame.passes[0].camera.0.generation += 1;
    assert_eq!(error(&frame), ContractError::MissingBinding("pass.camera"));
    let mut frame = fixture();
    frame.models[0].material.0.generation += 1;
    assert_eq!(
        error(&frame),
        ContractError::MissingBinding("instance.material")
    );
    let mut frame = fixture();
    frame.models[0].id.0.generation += 1;
    assert_eq!(
        error(&frame),
        ContractError::MissingBinding("draw.instance")
    );
    let mut frame = fixture();
    frame.effects[0].id.0.generation += 1;
    assert_eq!(error(&frame), ContractError::MissingBinding("draw.effect"));
    let mut frame = fixture();
    frame.lights[0].id.0.generation += 1;
    assert_eq!(error(&frame), ContractError::MissingBinding("draw.light"));
    let mut frame = fixture();
    frame.effects[0].material.0.generation += 1;
    assert_eq!(
        error(&frame),
        ContractError::MissingBinding("effect.material")
    );
}

#[test]
fn two_generations_of_one_slot_cannot_be_live_together() {
    let mut frame = fixture();
    let mut recycled = frame.models[0].clone();
    recycled.id.0.generation += 1;
    frame.models.push(recycled);
    assert_eq!(error(&frame), ContractError::DuplicateIdentity("instance"));
    let mut frame = fixture();
    frame.models[0].id.0.generation = 0;
    assert_eq!(error(&frame), ContractError::InvalidIdentity("instance"));
    let mut frame = fixture();
    let duplicate = frame.effects[0].particles[0].clone();
    frame.effects[0].particles.push(duplicate);
    assert_eq!(error(&frame), ContractError::DuplicateIdentity("particle"));
}

#[test]
fn nonfinite_values_cannot_enter_backend() {
    let mutations: &[fn(&mut DrawList)] = &[
        |f| f.cameras[0].view[0][0] = f32::NAN,
        |f| f.cameras[0].projection[1][2] = f32::INFINITY,
        |f| f.cameras[0].position[0] = f32::NAN,
        |f| f.models[0].skin_palette[0][0][0] = f32::NAN,
        |f| f.models[0].transform[3][0] = f32::INFINITY,
        |f| f.materials[0].parameters[0][0] = f32::NAN,
        |f| f.materials[0].textures[0].uv_transform[0][0] = f32::NAN,
        |f| f.materials[0].textures[0].sampler.border[0] = f32::NAN,
        |f| f.materials[0].raster.depth_bias_slope = f32::NAN,
        |f| f.lights[0].diffuse[0] = f32::NAN,
        |f| f.effects[0].particles[0].colour[0] = f32::NAN,
        |f| f.effects[0].particles[0].transform[0][0] = f32::INFINITY,
        |f| f.effects[0].particles[0].uv_rect[0] = f32::NAN,
    ];
    for mutate in mutations {
        let mut frame = fixture();
        mutate(&mut frame);
        assert!(matches!(error(&frame), ContractError::InvalidNumber(_)));
    }
}

#[test]
fn rectangles_check_bounds_overflow_and_empty_scissors() {
    let mut frame = fixture();
    if let Draw::Mesh { scissor, .. } = &mut frame.passes[0].draws[1] {
        scissor.width = 0;
    }
    assert!(frame.validate().is_ok());
    if let Draw::Mesh { scissor, .. } = &mut frame.passes[0].draws[1] {
        scissor.x = u32::MAX;
        scissor.width = 2;
    }
    assert_eq!(error(&frame), ContractError::InvalidRectangle);
    let mut frame = fixture();
    frame.passes[0].extent = [320, 240];
    assert_eq!(error(&frame), ContractError::InvalidRectangle);
    let mut frame = fixture();
    frame.cameras[0].viewport.width = 0;
    assert_eq!(error(&frame), ContractError::InvalidRectangle);
}

#[test]
fn invalid_depth_and_sampler_state_is_explicit() {
    let mut frame = fixture();
    frame.materials[0].raster.depth_compare = None;
    assert_eq!(
        error(&frame),
        ContractError::InvalidState("material.depth_write")
    );
    let mut frame = fixture();
    frame.materials[0].textures[0].sampler.lod_min = 9.;
    assert_eq!(error(&frame), ContractError::InvalidState("sampler.lod"));
    let mut frame = fixture();
    let duplicate = frame.materials[0].textures[0].clone();
    frame.materials[0].textures.push(duplicate);
    assert_eq!(
        error(&frame),
        ContractError::DuplicateIdentity("texture.slot")
    );
    let mut frame = fixture();
    frame.cameras[0].depth_range = [1., 0.];
    assert_eq!(
        error(&frame),
        ContractError::InvalidState("camera.depth_range")
    );
    let mut frame = fixture();
    frame.passes[0].draws.push(Draw::Clear {
        colour: None,
        depth: Some(f32::NAN),
        stencil: None,
    });
    assert_eq!(error(&frame), ContractError::InvalidNumber("clear.depth"));
}

#[test]
fn light_shapes_reject_invalid_range_direction_and_cones() {
    let mut frame = fixture();
    frame.lights[0].kind = LightKind::Point {
        position: [1.; 3],
        range: 20.,
        attenuation: [1., 0., 0.],
    };
    assert!(frame.validate().is_ok());
    frame.lights[0].kind = LightKind::Point {
        position: [1.; 3],
        range: -1.,
        attenuation: [1., 0., 0.],
    };
    assert_eq!(error(&frame), ContractError::InvalidNumber("light.range"));
    frame.lights[0].kind = LightKind::Directional { direction: [0.; 3] };
    assert_eq!(
        error(&frame),
        ContractError::InvalidState("light.direction")
    );
    frame.lights[0].kind = LightKind::Spot {
        position: [0.; 3],
        direction: [0., 1., 0.],
        range: 10.,
        attenuation: [1., 0., 0.],
        inner_cos: 0.2,
        outer_cos: 0.9,
        falloff: 1.,
    };
    assert_eq!(error(&frame), ContractError::InvalidState("light.spot"));
}

#[test]
fn render_targets_have_lifetimes_and_forbid_attachment_feedback() {
    let mut frame = fixture();
    frame.materials[0].textures[0].texture = TextureSource::RenderTarget(RenderTargetId(id(8)));
    assert!(frame.validate().is_ok());
    frame.passes[0].target = Target::Offscreen(RenderTargetId(id(8)));
    assert_eq!(
        error(&frame),
        ContractError::InvalidState("target.feedback")
    );
    let mut frame = fixture();
    frame.passes[0].target = Target::Offscreen(RenderTargetId(Lifetime {
        generation: 2,
        ..id(8)
    }));
    assert_eq!(error(&frame), ContractError::MissingBinding("pass.target"));
    let mut frame = fixture();
    frame.materials[0].textures[0].texture = TextureSource::RenderTarget(RenderTargetId(id(99)));
    assert_eq!(
        error(&frame),
        ContractError::MissingBinding("texture.target")
    );
}
