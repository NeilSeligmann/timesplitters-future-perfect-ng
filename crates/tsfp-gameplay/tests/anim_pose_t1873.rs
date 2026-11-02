//! T1873: the typed pose evaluator of `tsfp-engine-anim` against the bit-exact guest-memory `svc_pose` (itself
//! MEASURED against the original by the 649 T1872 goldens). Random clips and part trees, all selectors and flags.
use std::collections::BTreeMap;
use tsfp_engine_anim::pose::{self, Chain, Part};
use tsfp_format_anim::{Clip, Node, track_len};
use tsfp_gameplay::story_events::strip_update::GuestMemory;
use tsfp_gameplay::story_events::svc_objects::DestroyEffects;
use tsfp_gameplay::story_events::svc_pose;

#[derive(Default)]
struct Mem(BTreeMap<u32, u32>);

impl GuestMemory for Mem {
    fn read_u8(&self, address: u32) -> u8 {
        (self.0.get(&(address & !3)).copied().unwrap_or(0) >> (8 * (address & 3))) as u8
    }
    fn write_u8(&mut self, address: u32, value: u8) {
        let shift = 8 * (address & 3);
        let word = self.0.entry(address & !3).or_insert(0);
        *word = (*word & !(0xFF << shift)) | (u32::from(value) << shift);
    }
    fn read_u32(&self, address: u32) -> u32 {
        (0..4).fold(0, |acc, k| {
            acc | u32::from(self.read_u8(address.wrapping_add(k))) << (8 * k)
        })
    }
    fn write_u32(&mut self, address: u32, value: u32) {
        (0..4).for_each(|k| self.write_u8(address.wrapping_add(k), (value >> (8 * k)) as u8));
    }
}

struct NoCalls;
impl DestroyEffects for NoCalls {
    fn call(&mut self, _: &mut dyn GuestMemory, address: u32, _: &[u32]) -> u32 {
        panic!("unexpected engine call {address:#x}");
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
}

/// The image float constants the guest routines read.
fn image_constants(memory: &mut Mem) {
    for (address, bits) in [
        (0x475C78, 0x3F80_0000u32),
        (0x475D10, 0x4000_0000),
        (0x475D60, 0x3727_C5AC),
        (0x475DB4, 0x4080_0000),
        (0x478650, 0x3800_0080),
        (0x478654, 0x3F35_C28F),
        (0x478658, 0x3AB5_F00B),
        (0x47865C, 0x3900_0080),
    ] {
        memory.write_u32(address, bits);
    }
}

fn random_float(rng: &mut Rng) -> u32 {
    match rng.below(12) {
        0 => 0,
        1 => 0x3F80_0000,
        _ => ((rng.below(2000) as f32 - 1000.0) / 700.0).to_bits(),
    }
}

fn random_clip(rng: &mut Rng) -> Clip {
    let flags = [0, 1, 0x10, 0x11][rng.below(4) as usize];
    let keys = 1 + rng.below(6);
    let selectors = [0, 2, 4, 8, 10, 11, 12, 13];
    let nodes: Vec<u32> = (0..1 + rng.below(5))
        .map(|_| selectors[rng.below(8) as usize])
        .collect();
    let mut header = [0u32; 14];
    header[..6].copy_from_slice(&[u32::MAX, u32::MAX, flags, 9, keys, nodes.len() as u32]);
    header[9] = nodes.len() as u32;
    let tracks = nodes
        .iter()
        .map(|selector| {
            let length = track_len(*selector, flags, keys as usize).unwrap();
            // floats for the float forms, random bytes for the packed forms
            let float_form = flags & 1 == 0 && matches!(selector, 0 | 2 | 4 | 8 | 10);
            let mut bytes = Vec::new();
            while bytes.len() < length {
                let word = if float_form {
                    random_float(rng)
                } else {
                    rng.next()
                };
                bytes.extend_from_slice(&word.to_le_bytes());
            }
            bytes.truncate(length);
            bytes
        })
        .collect();
    Clip {
        header,
        key_frames: (0..keys).collect(),
        nodes: nodes
            .iter()
            .map(|s| Node {
                words: [0, *s, 0, keys, 0, 0, 0, 0],
            })
            .collect(),
        distances: vec![],
        tracks,
        tail: vec![],
    }
}

#[test]
fn sampler_equals_guest_pose_bone_on_random_clips() {
    let mut rng = Rng(0x1873_0001);
    let (mut compared, mut selectors_seen) = (0, std::collections::BTreeSet::new());
    for _ in 0..600 {
        let clip = random_clip(&mut rng);
        let mut memory = Mem::default();
        image_constants(&mut memory);
        let (context, channels, out) = (0x10_0000u32, 0x10_1000u32, 0x10_2000u32);
        memory.write_u32(context + 8, clip.flags());
        memory.write_u32(context + 0x10, clip.key_count());
        memory.write_u32(context + 0x28, channels);
        for (index, (node, track)) in clip.nodes.iter().zip(&clip.tracks).enumerate() {
            let record = channels + 0x20 * index as u32;
            let data = 0x20_0000 + 0x1_0000 * index as u32;
            memory.write_u32(record + 4, node.selector());
            memory.write_u32(record + 0x14, data);
            track
                .iter()
                .enumerate()
                .for_each(|(k, b)| memory.write_u8(data + k as u32, *b));
            selectors_seen.insert((node.selector(), clip.flags()));
        }
        for node in 0..clip.nodes.len() {
            for key in [
                0,
                1,
                clip.key_count() as i32 - 1,
                clip.key_count() as i32 + 2,
            ] {
                for scale in [1.0f32, 0.37] {
                    for k in 0..7 {
                        memory.write_u32(out + 4 * k, 0xDEAD_BEEF);
                    }
                    svc_pose::pose_bone(
                        &mut memory,
                        &mut NoCalls,
                        context,
                        node as u32,
                        key as u32,
                        out,
                        scale.to_bits(),
                    );
                    let want: Vec<u32> = (0..7).map(|k| memory.read_u32(out + 4 * k)).collect();
                    let got = pose::sample(&clip, node, key, scale);
                    // a typed track that is too short reports an error where the guest reads zeros; sizes are exact here
                    let got = got.unwrap_or_else(|e| {
                        panic!("node {node} key {key} flags {:#x}: {e:?}", clip.flags())
                    });
                    assert_eq!(
                        got.bits.to_vec(),
                        want,
                        "node {node} key {key} selector {} flags {:#x}",
                        clip.nodes[node].selector(),
                        clip.flags()
                    );
                    compared += 1;
                }
            }
        }
    }
    assert!(
        compared > 10_000 && selectors_seen.len() >= 28,
        "{compared} {}",
        selectors_seen.len()
    );
}

#[test]
fn quaternion_matrix_equals_guest() {
    let mut rng = Rng(0x1873_0002);
    for _ in 0..2000 {
        let mut memory = Mem::default();
        image_constants(&mut memory);
        let q: [u32; 4] = [0; 4].map(|_| random_float(&mut rng));
        for (k, word) in q.iter().enumerate() {
            memory.write_u32(0x1000 + 4 * k as u32, *word);
        }
        svc_pose::quat_matrix(&mut memory, 0x2000, 0x1000);
        let want: Vec<u32> = (0..16).map(|k| memory.read_u32(0x2000 + 4 * k)).collect();
        let got = pose::quat_matrix(q.map(f32::from_bits));
        assert_eq!(got.iter().map(|c| c.to_bits()).collect::<Vec<_>>(), want);
    }
}

fn random_tree(rng: &mut Rng, count: usize) -> Vec<Part> {
    let mut parts: Vec<Part> = (0..count)
        .map(|i| Part {
            kind: [0, 0, 2, 3][rng.below(4) as usize],
            cell: i as i8,
            child: -1,
            sibling: -1,
        })
        .collect();
    for i in 1..count {
        let parent = rng.below(i as u32) as usize;
        if parts[parent].child < 0 {
            parts[parent].child = i as i8;
        } else {
            let mut last = parts[parent].child as usize;
            while parts[last].sibling >= 0 {
                last = parts[last].sibling as usize;
            }
            parts[last].sibling = i as i8;
        }
    }
    parts
}

#[test]
fn part_tree_walks_equal_guest_chains() {
    let mut rng = Rng(0x1873_0003);
    for round in 0..600 {
        let count = 1 + rng.below(9) as usize;
        let parts = random_tree(&mut rng, count);
        let chain = if round % 2 == 0 { Chain::A } else { Chain::B };
        let (start, parent) = if round % 3 == 0 && count > 1 {
            (1usize, 0i32)
        } else {
            (0, -1)
        };
        let mut memory = Mem::default();
        image_constants(&mut memory);
        let (context, table, matrices) = (0x1000u32, 0x10_0000u32, 0x20_0000u32);
        memory.write_u32(context + 4, table);
        memory.write_u32(table, 0); // number of the first part
        for (i, part) in parts.iter().enumerate() {
            let at = table + 0xC8 * i as u32;
            memory.write_u8(at + 0x48, part.kind);
            memory.write_u8(at + 0x49, part.cell as u8);
            memory.write_u8(at + 0x4B, part.child as u8);
            memory.write_u8(at + 0x4C, part.sibling as u8);
        }
        // part 0's first dword doubles as the "first part number" word: keep it zero, the kind byte is at +0x48
        let mut typed: Vec<pose::Mat4> = Vec::new();
        for cell in 0..count {
            let m: pose::Mat4 = [0; 16].map(|_| f32::from_bits(random_float(&mut rng)));
            for (k, v) in m.iter().enumerate() {
                memory.write_u32(matrices + 64 * cell as u32 + 4 * k as u32, v.to_bits());
            }
            typed.push(m);
        }
        let walk = if matches!(chain, Chain::A) {
            svc_pose::pose_chain_a
        } else {
            svc_pose::pose_chain_b
        };
        walk(&mut memory, context, matrices, start as u32, parent as u32);
        pose::compose_chain(chain, &parts, &mut typed, start, parent).unwrap();
        for (cell, m) in typed.iter().enumerate() {
            let want: Vec<u32> = (0..16)
                .map(|k| memory.read_u32(matrices + 64 * cell as u32 + 4 * k))
                .collect();
            assert_eq!(
                m.iter().map(|c| c.to_bits()).collect::<Vec<_>>(),
                want,
                "round {round} cell {cell}"
            );
        }
    }
}
