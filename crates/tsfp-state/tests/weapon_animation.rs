// SPDX-License-Identifier: GPL-3.0-or-later
// Tier 1: translated root and translated math, synthetic decoder and metadata.
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use tsfp_ports::{
    abi::{ExtRegisters, Fpu, GuestCall, Xmm},
    c2r::{t1937_math as reference_math, t2049_nx_006 as reference},
    memory::GuestMemory,
};
use tsfp_state::{weapon_animation::*, weapon_select::SelectionPreset};
#[derive(Clone, Default)]
struct Image(Rc<RefCell<BTreeMap<u32, u8>>>);
impl GuestMemory for Image {
    fn read8(&self, a: u32) -> u8 {
        *self.0.borrow().get(&a).unwrap_or(&0)
    }
    fn write8(&mut self, a: u32, v: u8) {
        self.0.borrow_mut().insert(a, v);
    }
}
fn regs() -> ExtRegisters {
    ExtRegisters {
        eax: 0,
        ecx: 0,
        edx: 0,
        esp: 0x240000,
        ebx: 17,
        ebp: 19,
        esi: 23,
        edi: 29,
        eflags: 0,
        fpu: Fpu {
            stack: [0.; 8],
            top: 0,
            control: 0x300,
        },
        xmm: [Xmm::default(); 5],
        xmm_hi: [Xmm::default(); 3],
    }
}
fn put(m: &mut Image, a: u32, v: &[f32]) {
    for (i, x) in v.iter().enumerate() {
        m.write32(a + i as u32 * 4, x.to_bits());
    }
}
fn get<const N: usize>(m: &Image, a: u32) -> [f32; N] {
    std::array::from_fn(|i| f32::from_bits(m.read32(a + i as u32 * 4)))
}
fn identity() -> Matrix {
    let mut v = [0.; 16];
    for i in [0, 5, 10, 15] {
        v[i] = 1.;
    }
    Matrix(v)
}
fn constants(m: &mut Image) {
    put(m, 0x4e7900, &identity().0);
    put(m, 0x475cac, &[0.]);
    put(m, 0x475c78, &[1.]);
    put(m, 0x475d10, &[2.]);
    put(m, 0x475cd4, &[0.5]);
    put(m, 0x4785b0, &[0.000001]);
}
fn key(animation: i32, node: usize) -> NodeKey {
    NodeKey {
        rotation: match animation {
            2 => [0.5, 0.5, 0., 0.], // singular rotation
            3 => [1., 0., 0., 0.],   // negative-trace quaternion branch
            1 => [0., 0., 0.5, 1.],
            _ => [0., 0., 0., 1.],
        },
        translation: [animation as f32 * 0.25, node as f32 * 0.5, -0.75],
    }
}
/// Runs retained math at its actual entry ABI (return slot precedes arguments).
fn math_call(m: &mut Image, va: u32, args: &[u32], r: &mut ExtRegisters) {
    let mut local = *r;
    local.esp -= 4;
    for (i, a) in args.iter().enumerate() {
        m.write32(local.esp + 4 + i as u32 * 4, *a);
    }
    match va {
        0x153240 => reference_math::game_matrix4x4_copy_identity_to_dest(m, &mut local),
        0x1506b0 => reference_math::game_quat_to_matrix4x4(m, &mut local),
        0x153270 => reference_math::game_matrix4x4_multiply(m, &mut local),
        0x152090 => reference_math::game_matrix4x4_transform_direction_vec3_inplace(m, &mut local),
        0x152260 => reference_math::game_matrix4x4_determinant(m, &mut local),
        0x152540 => reference_math::game_matrix4x4_to_quat(m, &mut local),
        0x150920 => reference_math::game_vec4_negate_xyz_copy_w(m, &mut local),
        0x150830 => reference_math::game_quat_multiply(m, &mut local),
        0x1521d0 => reference_math::game_matrix4x4_inverse_rotate_vec3_inplace(m, &mut local),
        _ => panic!("math {va:x}"),
    }
    assert_eq!(local.esp, r.esp - 4, "callee stack {va:x}");
    r.eax = local.eax;
    r.ecx = local.ecx;
    r.edx = local.edx;
    r.fpu = local.fpu;
    r.xmm = local.xmm;
    r.xmm_hi = local.xmm_hi;
}
#[derive(Default)]
struct Math {
    events: Vec<(u32, Vec<u32>)>,
}
impl Math {
    fn run<const N: usize>(
        &mut self,
        va: u32,
        input: &[&[f32]],
        output: &[f32],
        order: &[usize],
    ) -> [f32; N] {
        let mut m = Image::default();
        constants(&mut m);
        put(&mut m, 0x1000, output);
        for (i, data) in input.iter().enumerate() {
            put(&mut m, 0x2000 + i as u32 * 0x100, data);
        }
        let args: Vec<u32> = order
            .iter()
            .map(|&i| {
                if i == 0 {
                    0x1000
                } else {
                    0x2000 + (i as u32 - 1) * 0x100
                }
            })
            .collect();
        self.events.push((va, Vec::new()));
        math_call(&mut m, va, &args, &mut regs());
        get(&m, 0x1000)
    }
}
impl AnimationMath for Math {
    fn decode(&mut self, a: i32, n: usize, f: u32, s: f32) -> NodeKey {
        self.events
            .push((0x59320, vec![a as u32, n as u32, f, s.to_bits()]));
        key(a, n)
    }
    fn identity(&mut self) -> Matrix {
        Matrix(self.run(0x153240, &[], &[0.; 16], &[0]))
    }
    fn rotate_matrix(&mut self, m: &mut Matrix, q: Quaternion) {
        m.0 = self.run(0x1506b0, &[&q], &m.0, &[0, 1]);
    }
    fn multiply(&mut self, a: Matrix, b: Matrix) -> Matrix {
        Matrix(self.run(0x153270, &[&a.0, &b.0], &[0.; 16], &[0, 1, 2]))
    }
    fn transform_direction(&mut self, m: Matrix, d: Direction) -> Direction {
        self.run(0x152090, &[&m.0], &d, &[1, 0])
    }
    fn inverse_into(&mut self, d: &mut Matrix, s: Matrix) {
        d.0 = self.run(0x152260, &[&s.0], &d.0, &[0, 1]);
    }
    fn quaternion(&mut self, m: Matrix) -> Quaternion {
        self.run(0x152540, &[&m.0], &[0.; 4], &[0, 1])
    }
    fn conjugate(&mut self, q: Quaternion) -> Quaternion {
        self.run(0x150920, &[&q], &[0.; 4], &[0, 1])
    }
    fn multiply_quaternions(&mut self, a: Quaternion, b: Quaternion) -> Quaternion {
        self.run(0x150830, &[&a, &b], &[0.; 4], &[0, 1, 2])
    }
    fn inverse_rotate(&mut self, m: Matrix, d: Direction) -> Direction {
        self.run(0x1521d0, &[&m.0], &d, &[1, 0])
    }
}
const SCRATCH: u32 = 0x300000;
struct Oracle {
    m: Image,
    events: Vec<(u32, Vec<u32>)>,
    allocations: Vec<(u32, u32)>,
}
impl GuestCall for Oracle {
    fn call(&mut self, va: u32, r: &mut ExtRegisters) {
        let arg = |i: u32| self.m.read32(r.esp + 4 * i);
        if va == 0x3d6e0 {
            self.allocations.push((va, arg(0)));
            r.eax = SCRATCH;
            return;
        }
        if va == 0x3d6b0 {
            self.allocations.push((va, arg(0)));
            return;
        }
        if va == 0x59320 {
            let a = (arg(0) - 0x400000) / 0x100;
            let a = if a == 4 { u32::MAX } else { a };
            let n = arg(1);
            let out = arg(3);
            self.events.push((va, vec![a, n, arg(2), arg(4)]));
            let k = key(a as i32, n as usize);
            put(&mut self.m, out, &k.translation);
            put(&mut self.m, out + 12, &k.rotation);
            return;
        }
        let count = match va {
            0x153240 => 1,
            0x153270 | 0x150830 => 3,
            _ => 2,
        };
        let args: Vec<u32> = (0..count).map(arg).collect();
        self.events.push((va, Vec::new()));
        math_call(&mut self.m, va, &args, r);
    }
}
#[derive(Clone)]
struct Owned {
    animation: i32,
    parents: Vec<Option<usize>>,
    parts: Vec<Part>,
    mapping: Vec<Option<usize>>,
    outputs: Vec<Matrix>,
    connections: Vec<Connection>,
}
impl Owned {
    fn view(&mut self) -> Template<'_> {
        Template {
            animation: self.animation,
            parents: &self.parents,
            parts: &self.parts,
            node_parts: &self.mapping,
            node_outputs: &mut self.outputs,
            connections: &mut self.connections,
        }
    }
}
fn random(s: &mut u32) -> u32 {
    *s ^= *s << 13;
    *s ^= *s >> 17;
    *s ^= *s << 5;
    *s
}
#[test]
fn translated_root_differential() {
    let mut seed = 0x1831350e;
    for case in 0..4096 {
        let mut owned: Vec<Owned> = Vec::new();
        let node_count = (random(&mut seed) % 5) as usize;
        for index in 0..7 {
            let animation = if index > 0 && random(&mut seed) % 3 != 0 {
                owned[index - 1].animation
            } else {
                (random(&mut seed) % 5) as i32 - 1
            };
            let part_count = if node_count == 0 {
                0
            } else {
                (random(&mut seed) % 9) as usize
            };
            let parts: Vec<Part> = (0..part_count)
                .map(|_| Part {
                    node: (random(&mut seed) % node_count as u32) as usize,
                    offset: [(random(&mut seed) % 9) as f32 * 0.25, -0.5, 0.75],
                })
                .collect();
            let parents = (0..node_count)
                .map(|n| {
                    if n == 0 || random(&mut seed) % 2 == 0 {
                        None
                    } else {
                        Some((random(&mut seed) % node_count as u32) as usize)
                    }
                })
                .collect();
            let mapping = (0..node_count)
                .map(|_| {
                    if part_count == 0 || random(&mut seed) % 3 == 0 {
                        None
                    } else {
                        Some((random(&mut seed) % part_count as u32) as usize)
                    }
                })
                .collect();
            let connections = (0..part_count.saturating_sub(1))
                .map(|_| Connection {
                    row: (random(&mut seed) % 4) as usize,
                    parts: [
                        (random(&mut seed) % part_count as u32) as usize,
                        (random(&mut seed) % part_count as u32) as usize,
                    ],
                    relative_rotation: [7.; 4],
                    directions: [[8.; 3]; 2],
                })
                .collect();
            owned.push(Owned {
                animation,
                parents,
                parts,
                mapping,
                outputs: vec![Matrix([9.; 16]); node_count],
                connections,
            });
        }
        let initial = owned.clone();
        let mut m = Image::default();
        constants(&mut m);
        m.write32(0x7e3954, 0x410000);
        for a in 0..5 {
            let identity = if a == 4 { 0x7e03be } else { 0x7e03c0 + a * 2 };
            m.write8(identity, a as u8);
            m.write8(identity + 1, 0);
            m.write32(0x410000 + a * 4, 0x400000 + a * 0x100);
            m.write32(0x400000 + a * 0x100 + 0x14, node_count as u32);
        }
        for (i, t) in initial.iter().enumerate() {
            let base = 0x53fa58 + i as u32 * 0x4dc;
            let data = 0x500000 + i as u32 * 0x1000;
            m.write32(base, t.parts.len() as u32);
            m.write32(base + 4, t.animation as u32);
            m.write32(base + 12, data);
            m.write32(base + 16, data + 0x300);
            m.write32(base + 20, data + 0x100);
            m.write32(base + 24, data + 0x200);
            for (p, part) in t.parts.iter().enumerate() {
                let a = data + p as u32 * 0x1c;
                m.write32(a, part.node as u32);
                put(&mut m, a + 12, &part.offset);
            }
            for n in 0..node_count {
                m.write32(
                    data + 0x100 + n as u32 * 4,
                    t.mapping[n].map_or(u32::MAX, |p| p as u32),
                );
                m.write32(
                    data + 0x200 + n as u32 * 4,
                    t.parents[n].map_or(u32::MAX, |p| p as u32),
                );
                put(&mut m, base + 28 + n as u32 * 64, &t.outputs[n].0);
            }
            for (c, connection) in t.connections.iter().enumerate() {
                let a = data + 0x300 + c as u32 * 0x4c;
                m.write32(a + 16, connection.row as u32);
                m.write32(a + 20, connection.parts[0] as u32);
                m.write32(a + 24, connection.parts[1] as u32);
                put(&mut m, a + 36, &connection.relative_rotation);
                put(&mut m, a + 52, &connection.directions[0]);
                put(&mut m, a + 64, &connection.directions[1]);
            }
        }
        for n in 0..8 {
            put(&mut m, SCRATCH + n * 64, &[3.; 16]);
            put(&mut m, SCRATCH + 0x200 + n * 64, &[2.; 16]);
        }
        let mut oracle = Oracle {
            m: m.clone(),
            events: Vec::new(),
            allocations: Vec::new(),
        };
        let mut r = regs();
        reference::game_composite_templates_precompute_part_rest_pose_quats_from_animation_tables(
            &mut m,
            &mut oracle,
            &mut r,
        );
        let mut math = Math::default();
        let mut nodes = [Matrix([2.; 16]); 8];
        let mut inverses = [Matrix([3.; 16]); 8];
        let mut templates: Vec<_> = owned.iter_mut().map(Owned::view).collect();
        prepare(&mut templates, &mut nodes, &mut inverses, &mut math).unwrap();
        assert_eq!(oracle.events, math.events, "calls case {case}");
        assert_eq!(
            oracle.allocations,
            vec![(0x3d6e0, 1), (0x3d6b0, SCRATCH + 0x200)]
        );
        for (i, t) in owned.iter().enumerate() {
            let base = 0x53fa58 + i as u32 * 0x4dc;
            let data = 0x500000 + i as u32 * 0x1000;
            for (n, out) in t.outputs.iter().enumerate() {
                assert_eq!(
                    get::<16>(&m, base + 28 + n as u32 * 64).map(f32::to_bits),
                    out.0.map(f32::to_bits),
                    "output case {case} template {i} node {n}"
                );
            }
            for (c, out) in t.connections.iter().enumerate() {
                let a = data + 0x300 + c as u32 * 0x4c;
                assert_eq!(
                    get::<4>(&m, a + 36).map(f32::to_bits),
                    out.relative_rotation.map(f32::to_bits),
                    "rotation case {case}"
                );
                for d in 0..2 {
                    assert_eq!(
                        get::<3>(&m, a + 52 + d * 12).map(f32::to_bits),
                        out.directions[d as usize].map(f32::to_bits),
                        "direction case {case}"
                    );
                }
            }
        }
        for n in 0..8 {
            assert_eq!(
                get::<16>(&m, SCRATCH + 0x200 + n as u32 * 64).map(f32::to_bits),
                nodes[n as usize].0.map(f32::to_bits),
                "scratch nodes {case}"
            );
            assert_eq!(
                get::<16>(&m, SCRATCH + n as u32 * 64).map(f32::to_bits),
                inverses[n as usize].0.map(f32::to_bits),
                "scratch inverse {case}"
            );
        }
    }
}
#[test]
fn reference_default_and_invalid_projection() {
    assert_eq!(
        dispatch(
            SelectionPreset::default(),
            || 17,
            || panic!("readable enabled")
        ),
        17
    );
    let mut math = Math::default();
    assert_eq!(
        prepare(&mut [], &mut [], &mut [], &mut math),
        Err(InvalidTemplate::TemplateCount)
    );
    assert!(math.events.is_empty());
}

#[test]
fn invalid_projections_have_no_neighbour_effects() {
    for expected in [
        InvalidTemplate::ScratchCapacity,
        InvalidTemplate::NodeIndex,
        InvalidTemplate::PartIndex,
        InvalidTemplate::ConnectionCount,
        InvalidTemplate::RowIndex,
    ] {
        let base = Owned {
            animation: 0,
            parents: vec![None],
            parts: vec![Part {
                node: 0,
                offset: [0.; 3],
            }],
            mapping: vec![Some(0)],
            outputs: vec![identity()],
            connections: vec![],
        };
        let mut owned = vec![base; 7];
        match expected {
            InvalidTemplate::ScratchCapacity => {
                owned[6].parts = vec![
                    Part {
                        node: 0,
                        offset: [0.; 3]
                    };
                    9
                ]
            }
            InvalidTemplate::NodeIndex => owned[6].parents[0] = Some(1),
            InvalidTemplate::PartIndex => owned[6].mapping[0] = Some(1),
            InvalidTemplate::ConnectionCount => owned[6].connections.push(Connection {
                row: 0,
                parts: [0, 0],
                relative_rotation: [0.; 4],
                directions: [[0.; 3]; 2],
            }),
            InvalidTemplate::RowIndex => {
                owned[6].parts.push(Part {
                    node: 0,
                    offset: [0.; 3],
                });
                owned[6].connections.push(Connection {
                    row: 4,
                    parts: [0, 1],
                    relative_rotation: [0.; 4],
                    directions: [[0.; 3]; 2],
                });
            }
            _ => unreachable!(),
        }
        let mut nodes = [Matrix([2.; 16]); 8];
        let mut inverses = [Matrix([3.; 16]); 8];
        let mut math = Math::default();
        let mut views: Vec<_> = owned.iter_mut().map(Owned::view).collect();
        assert_eq!(
            prepare(&mut views, &mut nodes, &mut inverses, &mut math),
            Err(expected)
        );
        assert!(math.events.is_empty());
        assert_eq!(nodes, [Matrix([2.; 16]); 8]);
        assert_eq!(inverses, [Matrix([3.; 16]); 8]);
    }
    assert_eq!(
        dispatch(
            SelectionPreset::ReadableOriginal,
            || panic!("reference selected"),
            || 23
        ),
        23
    );
}
