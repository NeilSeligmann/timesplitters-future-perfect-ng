//! T1668: `0x60B30` against the ORIGINAL (`tools/t1668_instance_gold.py`).
use tsfp_engine_anim::display::{ClipSource, Skeleton, TickPose, Track};
use tsfp_engine_anim::instance::{ClipView, Instance, PostStep, evaluate_instance};
use tsfp_engine_anim::pose::{Mat4, Sample};
use tsfp_engine_anim::posemix::TreeRow;
use tsfp_engine_anim::simulation::{FixedCommand, FixedInstance};

struct NativeSource<'a> {
    keys: &'a [u32],
    duration: f32,
    nodes: usize,
}
impl ClipSource for NativeSource<'_> {
    fn view(&self, _: u32) -> Option<ClipView<'_>> {
        Some(ClipView {
            keys: self.keys,
            duration: self.duration,
            nodes: self.nodes,
        })
    }
    fn sample(&self, _: u32, node: usize, key: i32) -> Option<Sample> {
        Some(sampler(node, key))
    }
}

#[path = "support/hex.rs"]
mod hex_support;
use hex_support::hex;

fn sampler(node: usize, key: i32) -> Sample {
    let angle = 0.11f64 * (key & 0xFFFF) as f64 + 0.3 * node as f64;
    let axis = [
        (angle * 1.3 + 1.0).sin(),
        (angle * 0.7).cos(),
        (angle * 0.4 + 2.0).sin(),
    ];
    let norm = axis.iter().map(|a| a * a).sum::<f64>().sqrt();
    let half = angle * 0.5;
    let mut bits = [0u32; 7];
    for i in 0..3 {
        bits[i] = ((key as f64 * 0.75 + node as f64 + i as f64 * 0.5 - 3.0) as f32).to_bits();
        bits[3 + i] = ((axis[i] / norm * half.sin()) as f32).to_bits();
    }
    bits[6] = (half.cos() as f32).to_bits();
    Sample { bits }
}

fn matrices(words: &[u32]) -> Vec<Mat4> {
    words
        .chunks(16)
        .map(|c| std::array::from_fn(|i| f32::from_bits(c[i])))
        .collect()
}

#[test]
fn evaluate_instance_equals_the_original() {
    let text = std::fs::read_to_string(format!(
        "{}/tests/data/t1668/instance.golden",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let (mut total, mut roots, mut saves) = (0, 0, 0);
    for line in text.lines() {
        let parts: Vec<&str> = line.split(" | ").collect();
        let h: Vec<&str> = parts[0].split(' ').collect();
        let keys: Vec<u32> = h[1].split(',').map(|k| k.parse().unwrap()).collect();
        let duration = f32::from_bits(hex(h[2]));
        let nodes: usize = h[3].parse().unwrap();
        let (owner_time, offset, old) = (
            f32::from_bits(hex(h[4])),
            f32::from_bits(hex(h[5])),
            f32::from_bits(hex(h[6])),
        );
        let (p0, p1) = (hex(h[7]), hex(h[8]));
        let (snap, save): (u32, u32) = (h[9].parse().unwrap(), h[10].parse().unwrap());
        let model_count: usize = h[11].parse().unwrap();
        let root: [f32; 9] = std::array::from_fn(|i| f32::from_bits(hex(h[12 + i])));
        let sentinel = f32::from_bits(0xA5A5_A5A5);
        let (mut matrices, mut base) =
            ([[sentinel; 16]; 4], [[f32::from_bits(0x5A5A_5A5A); 16]; 4]);
        let mut instance = Instance {
            layer_time: old,
            root,
            matrices: &mut matrices,
            base: &mut base,
            saved_count: model_count,
        };
        let clip = ClipView {
            keys: &keys,
            duration,
            nodes,
        };
        let steps = evaluate_instance(
            &clip,
            owner_time,
            offset,
            (p0, p1),
            snap != 0,
            save != 0,
            sampler,
            &mut instance,
        )
        .unwrap();
        let want_matrices: Vec<u32> = parts[1].split(' ').map(hex).collect();
        let got: Vec<u32> = instance.matrices[..nodes]
            .iter()
            .flat_map(|m| m.map(f32::to_bits))
            .collect();
        assert_eq!(got, want_matrices, "{line}");
        let state: Vec<u32> = parts[2].split(' ').map(hex).collect();
        assert_eq!(instance.layer_time.to_bits(), state[0], "{line}");
        let got_root: Vec<u32> = instance.root.map(f32::to_bits).to_vec();
        assert_eq!(got_root, state[1..10], "{line}");
        let want_base: Vec<u32> = parts[3].split(' ').map(hex).collect();
        let got_base: Vec<u32> = instance
            .base
            .iter()
            .flat_map(|m| m.map(f32::to_bits))
            .collect();
        assert_eq!(got_base, want_base, "{line}");
        let calls: Vec<String> = steps
            .iter()
            .map(|s| match s {
                PostStep::BlendByWeight { full } => format!("60540/{:x}", u32::from(*full)),
                PostStep::ComposeTree => "604d0".to_string(),
                PostStep::ComposeOrCopy => "605d0".to_string(),
            })
            .collect();
        let want_calls: Vec<&str> = parts[4].split(',').collect();
        assert_eq!(calls.len(), want_calls.len(), "{line}");
        assert!(
            want_calls[0] == calls[0]
                && want_calls[1].starts_with("604d0")
                && want_calls[2] == "605d0",
            "{line}"
        );
        // The native fixed producer must retain the ORIGINAL root/base/local
        // results, including saved matrices beyond the clip's sampled prefix.
        // Original post-step callees were stubbed in these goldens, so their
        // composed palette is verified elsewhere, not asserted here.
        let mut native = FixedInstance::new(
            TickPose {
                generation: 1,
                owner_time,
                time_step: 0.0,
                track: Track {
                    clip: 7,
                    offset,
                    layer_time: old,
                    policy: (p0, p1),
                    snap: snap != 0,
                },
                blend_layer: None,
                root,
                local: vec![[sentinel; 16]; 4],
                base: vec![[f32::from_bits(0x5A5A_5A5A); 16]; 4],
                palette: vec![[sentinel; 16]; 4],
                full_blend: false,
            },
            Skeleton {
                tree: vec![
                    TreeRow {
                        parent: -1,
                        child: -1,
                        sibling: -1
                    };
                    4
                ],
                weights: (0..4).map(|i| (i, 1.0)).collect(),
            },
        )
        .unwrap();
        native
            .tick(
                FixedCommand {
                    owner_time,
                    time_step: 0.0,
                    save_base: save != 0,
                    saved_count: model_count,
                    blend_owner_time: None,
                },
                &NativeSource {
                    keys: &keys,
                    duration,
                    nodes,
                },
            )
            .unwrap();
        let fixed = native.display().ticks().1;
        assert_eq!(
            fixed.local[..nodes]
                .iter()
                .flat_map(|m| m.map(f32::to_bits))
                .collect::<Vec<_>>(),
            want_matrices,
            "native local {line}"
        );
        assert_eq!(
            fixed.track.layer_time.to_bits(),
            state[0],
            "native clock {line}"
        );
        assert_eq!(
            fixed.root.map(f32::to_bits).to_vec(),
            state[1..10],
            "native root {line}"
        );
        assert_eq!(
            fixed
                .base
                .iter()
                .flat_map(|m| m.map(f32::to_bits))
                .collect::<Vec<_>>(),
            want_base,
            "native base {line}"
        );
        total += 1;
        roots += usize::from(tsfp_engine_anim::tail::tracks_root_motion(p0, p1));
        saves += usize::from(save != 0 && model_count > 0);
    }
    assert!(
        total > 400 && roots > 100 && saves > 50,
        "{total} {roots} {saves}"
    );
}
