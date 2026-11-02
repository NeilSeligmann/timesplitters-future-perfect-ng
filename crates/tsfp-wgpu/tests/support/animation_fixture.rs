//! T2241 FABRICATED native object -> fractional palette -> live GPU draw.
//! No retail bytes; this is a renderer integration test, not gameplay Tier 2.
use tsfp_engine_anim::display::{ClipSource, DisplayInstance, Sampling, Skeleton, TickPose, Track};
use tsfp_engine_anim::display_scene::{AnimationScene, ObjectId, SimulationUpdate};
use tsfp_engine_anim::instance::ClipView;
use tsfp_engine_anim::posemath::Sample;
use tsfp_engine_anim::posemix::TreeRow;
use tsfp_engine_anim::presentation::{AnimatedObject, ObjectTick, Orientation};
use tsfp_wgpu::stream::Stream;

#[path = "mod.rs"]
mod stream_support;
use stream_support::{declaration_hex, hex, vertex};

pub struct Source;
impl ClipSource for Source {
    fn view(&self, _: u32) -> Option<ClipView<'_>> {
        Some(ClipView {
            keys: &[0, 1],
            duration: 1.0 / 60.0,
            nodes: 1,
        })
    }
    fn sample(&self, _: u32, _: usize, key: i32) -> Option<Sample> {
        Some(Sample {
            bits: [0.0, key as f32 * 120.0, 0.0, 0.0, 0.0, 0.0, 1.0].map(f32::to_bits),
        })
    }
}
pub fn scene() -> AnimationScene {
    let identity = std::array::from_fn(|i| if i % 5 == 0 { 1.0 } else { 0.0 });
    let pose = TickPose {
        generation: 1,
        owner_time: 0.0,
        time_step: 1.0 / 60.0,
        track: Track {
            clip: 7,
            offset: 0.0,
            layer_time: 0.0,
            policy: (0, 0),
            snap: false,
        },
        blend_layer: None,
        root: [42.0; 9],
        local: vec![identity],
        base: vec![identity],
        palette: vec![identity],
        full_blend: false,
    };
    let placement = ObjectTick {
        orientation: Orientation::Identity,
        flip_x: false,
        position: [0.0; 3],
    };
    let object = AnimatedObject::new(
        DisplayInstance::new(
            pose.clone(),
            Skeleton {
                tree: vec![TreeRow {
                    parent: -1,
                    child: -1,
                    sibling: -1,
                }],
                weights: vec![(0, 1.0)],
            },
        )
        .unwrap(),
        placement,
    )
    .unwrap();
    let mut scene = AnimationScene::default();
    scene.insert(ObjectId(1), object).unwrap();
    scene
        .simulate_tick(
            0,
            vec![SimulationUpdate {
                object: ObjectId(1),
                command: tsfp_engine_anim::simulation::FixedCommand {
                    owner_time: 0.0,
                    time_step: 1.0 / 60.0,
                    save_base: false,
                    saved_count: 0,
                    blend_owner_time: None,
                },
                placement,
            }],
            &Source,
        )
        .unwrap();
    scene
}
pub fn setup() -> Stream {
    // Synthetic translation-only shader: ADD oPos.xyz, v1, c12;
    // MOV oD0, v2. c12 is the explicitly bound matrix translation row.
    let instructions = [
        [
            0,
            (3 << 21) | (12 << 13) | (1 << 9) | 0x1b,
            (2 << 26) | (0x1b << 2),
            (3 << 28) | (1 << 11) | (0xe << 12),
        ],
        [
            0,
            (1 << 21) | (2 << 9) | 0x1b,
            2 << 26,
            (3 << 3) | (1 << 11) | (0xf << 12) | 1,
        ],
    ];
    let program = format!("0200{}{}", hex(&instructions[0]), hex(&instructions[1]));
    let vertices = format!(
        "{}{}{}",
        vertex(100.0, 100.0, 0xffffffff, 0.0, 0.0),
        vertex(300.0, 100.0, 0xffffffff, 0.0, 0.0),
        vertex(100.0, 300.0, 0xffffffff, 0.0, 0.0)
    );
    Stream::from_text(&format!("mem 40000002 {program}\ncall LoadVertexProgram 40000000 0\nmem 40001000 {}\ncall SelectVertexDeclaration 40001000 0\nhost 19240 40002000 18\nmem 40002000 {vertices}\n", declaration_hex())).unwrap()
}
