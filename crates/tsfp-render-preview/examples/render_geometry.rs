// SPDX-License-Identifier: GPL-3.0-or-later
//! UNVERIFIED offline CPU rasteriser for geometry sidecar v2 frames.
//! Usage: render_geometry <sidecar> <frame> <out.png> <yaw-deg> <pitch-deg> [stride] [pos-offset]
//! The camera is FITTED to the geometry bounds (the recorded owner camera is zero);
//! it is not the game camera. Vertex layout (stride/offset) is a guess, see docs.
use tsfp_render_preview::capture::{self, CapturedGeometry};
use tsfp_wgpu::vsh;

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt().max(1e-9);
    [a[0] / l, a[1] / l, a[2] / l]
}

// INFERRED diagnostic only: derive projection * view when the sidecar's combined matrix is missing.
fn reconstruct_transform(constants: &mut [[f32; 4]; vsh::CONSTANT_COUNT]) {
    let projection = [constants[60], constants[61], constants[62], constants[63]];
    let view = [constants[64], constants[65], constants[66], constants[67]];
    for row in 0..4 {
        for col in 0..4 {
            constants[68 + row][col] = (0..4).map(|k| projection[row][k] * view[k][col]).sum();
        }
    }
}

/// D3D8 vertex-program constants (MEASURED layout, rows of 4): c58 viewport scale, c59 offset,
/// c60..c63 projection and c64..c66 view (column-vector 3x4). The per-object world matrix is
/// not in the captured constants, so vertices are treated as already world-space.
/// Run the draw's own NV2A vertex program (tsfp-wgpu `vsh`) on register 1 = position; oPos is in
/// viewport pixels (the program applies the c58/c59 viewport map itself).
fn run_program(g: &CapturedGeometry, start: usize, c: &[f32], p: [f32; 3]) -> Option<[f32; 3]> {
    let mut memory = vsh::ProgramMemory::default();
    for (slot, bytes) in g
        .program
        .chunks_exact(16)
        .enumerate()
        .take(memory.slots.len())
    {
        for k in 0..4 {
            memory.slots[slot][k] = u32::from_le_bytes(bytes[k * 4..k * 4 + 4].try_into().unwrap());
        }
    }
    let mut constants = [[0f32; 4]; vsh::CONSTANT_COUNT];
    for (row, chunk) in constants.iter_mut().zip(c.chunks_exact(4)) {
        row.copy_from_slice(chunk);
    }
    if std::env::var_os("RECONSTRUCT_TRANSFORM").is_some() {
        reconstruct_transform(&mut constants);
    }
    let mut inputs = [[0.0, 0.0, 0.0, 1.0]; vsh::INPUT_COUNT];
    // MEASURED for the first draw of capture 42b1 present 6646; other declarations are unverified.
    inputs[1] = [p[0], p[1], p[2], 1.0];
    let o = vsh::run(&memory, start, &inputs, &constants).position();
    if std::env::var_os("DEBUG_VSH").is_some() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        let n = COUNT.fetch_add(1, Ordering::Relaxed);
        if n == 0 {
            eprintln!("captured program:\n{}", vsh::describe(&memory, start));
            for (i, row) in constants.iter().enumerate().skip(58).take(14) {
                eprintln!("captured c[{i}] = {row:?}");
            }
        }
        if n < 8 {
            eprintln!(
                "vsh start={start} input={p:?} output={o:?} program_nonzero={} constants_nonzero={}",
                g.program.iter().filter(|&&b| b != 0).count(),
                c.iter().filter(|&&v| v != 0.0).count()
            );
        }
    }
    (o[3] > 1e-6 && o.iter().all(|v| v.is_finite())).then(|| [o[0], o[1], o[3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_strip_remaps_source_vertices_and_preserves_winding() {
        let mut vertices = Vec::new();
        for point in [
            [0.0f32, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        ] {
            for value in point {
                vertices.extend_from_slice(&value.to_le_bytes());
            }
        }
        let geometry = CapturedGeometry {
            stride: 12,
            indexed: 1,
            primitive: 5,
            first: 0,
            vertex_first: 4,
            vertices,
            indices: [4u16, 5, 6, 7]
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect(),
            constants: Vec::new(),
            program: Vec::new(),
            program_start: None,
            vertex_declaration: None,
            vertex_declaration_data: Vec::new(),
            register1: None,
        };

        let result = triangles(&geometry, 12, 0, None, None);
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0],
            [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
        );
        assert_eq!(
            result[1],
            [[0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]]
        );
    }

    #[test]
    fn reconstructs_projection_times_view() {
        let mut c = [[0.0; 4]; vsh::CONSTANT_COUNT];
        c[60] = [2.0, 0.0, 0.0, 0.0];
        c[61] = [0.0, 3.0, 0.0, 0.0];
        c[62] = [0.0, 0.0, 4.0, 0.0];
        c[63] = [0.0, 0.0, 0.0, 1.0];
        c[64] = [1.0, 0.0, 0.0, 5.0];
        c[65] = [0.0, 1.0, 0.0, 6.0];
        c[66] = [0.0, 0.0, 1.0, 7.0];
        c[67] = [0.0, 0.0, 0.0, 1.0];
        reconstruct_transform(&mut c);
        assert_eq!(
            &c[68..72],
            &[
                [2.0, 0.0, 0.0, 10.0],
                [0.0, 3.0, 0.0, 18.0],
                [0.0, 0.0, 4.0, 28.0],
                [0.0, 0.0, 0.0, 1.0]
            ]
        );
    }

    #[test]
    fn declaration_selects_register_one_position_offset() {
        let mut declaration = vec![0u8; capture::VERTEX_DECLARATION_BYTES];
        for entry in 0..16 {
            let at = entry * 16 + 8;
            declaration[at..at + 4].copy_from_slice(&2u32.to_le_bytes());
        }
        declaration[16 + 4..16 + 8].copy_from_slice(&4u32.to_le_bytes());
        declaration[16 + 8..16 + 12].copy_from_slice(&0x32u32.to_le_bytes());
        let mut vertices = vec![0u8; 3 * 28];
        for (index, point) in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
            .into_iter()
            .enumerate()
        {
            for (component, value) in point.into_iter().enumerate() {
                let at = index * 28 + 4 + component * 4;
                vertices[at..at + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        let geometry = CapturedGeometry {
            stride: 28,
            indexed: 0,
            primitive: 4,
            first: 0,
            vertex_first: 0,
            vertices,
            indices: Vec::new(),
            constants: Vec::new(),
            program: Vec::new(),
            program_start: None,
            vertex_declaration: Some(0x003e_2c68),
            vertex_declaration_data: declaration,
            register1: None,
        };

        assert_eq!(
            triangles(&geometry, 28, 0, None, None),
            vec![[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]]
        );
    }

    #[test]
    fn captured_register_one_stream_supersedes_guest_stream_bytes() {
        let mut positions = Vec::new();
        for point in [[1.0f32, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]] {
            for value in point {
                positions.extend_from_slice(&value.to_le_bytes());
            }
            positions.extend_from_slice(&[0; 4]);
        }
        let geometry = CapturedGeometry {
            stride: 28,
            indexed: 1,
            primitive: 4,
            first: 0,
            vertex_first: 0,
            vertices: vec![0; 3 * 28],
            indices: [4u16, 5, 6]
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect(),
            constants: Vec::new(),
            program: Vec::new(),
            program_start: None,
            vertex_declaration: None,
            vertex_declaration_data: Vec::new(),
            register1: Some(capture::CapturedVertexInput {
                stride: 16,
                first: 4,
                format: 0x32,
                vertices: positions,
            }),
        };

        assert_eq!(
            triangles(&geometry, 28, 0, None, None),
            vec![[[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]]]
        );
    }
}

fn screen(c: &[f32], p: [f32; 3]) -> Option<[f32; 3]> {
    let row = |r: usize| &c[r * 4..r * 4 + 4];
    let m = |r: usize, v: [f32; 4]| row(r).iter().zip(v).map(|(a, b)| a * b).sum::<f32>();
    let v4 = [p[0], p[1], p[2], 1.0];
    let view = [m(64, v4), m(65, v4), m(66, v4), 1.0];
    let clip = [m(60, view), m(61, view), m(62, view), m(63, view)];
    if clip[3] <= 0.01 {
        return None;
    }
    let (sc, off) = (row(58), row(59));
    Some([
        clip[0] / clip[3] * sc[0] + off[0],
        clip[1] / clip[3] * sc[1] + off[1],
        clip[3],
    ])
}

fn plausible(v: &[u8], stride: usize, off: usize) -> usize {
    (0..(v.len().saturating_sub(off)) / stride)
        .filter(|&i| {
            (0..3).all(|k| {
                let b = i * stride + off + k * 4;
                let x = f32::from_le_bytes(v[b..b + 4].try_into().unwrap());
                x.is_finite() && x.abs() < 5e3 && (x == 0.0 || x.abs() > 1e-3)
            })
        })
        .count()
}

/// Use register 1's FLOAT3 offset from a captured declaration when available.
/// Legacy sidecars still use a plausibility heuristic because they lack the block.
fn triangles(
    g: &CapturedGeometry,
    stride: usize,
    fallback_off: usize,
    consts: Option<&[f32]>,
    vsh_start: Option<usize>,
) -> Vec<[[f32; 3]; 3]> {
    let (vertices, stride, source_first, register1_offset) = match g.register1.as_ref() {
        Some(input) if input.format == 0x32 => (
            &input.vertices,
            input.stride as usize,
            input.first,
            Some(0usize),
        ),
        Some(_) => return Vec::new(),
        None => (&g.vertices, stride, g.vertex_first, None),
    };
    if stride < 12 || vertices.len() < 12 || vertices.len() % stride != 0 {
        return Vec::new();
    }
    if vsh_start.is_some() && g.program_start.is_some_and(|start| start >= 136) {
        return Vec::new();
    }
    let off = if let Some(offset) = register1_offset {
        offset
    } else if g.vertex_declaration_data.is_empty() {
        if fallback_off != 0 && fallback_off + 12 <= stride {
            fallback_off
        } else {
            (0..=stride - 12)
                .step_by(4)
                .max_by_key(|&o| plausible(&g.vertices, stride, o))
                .unwrap_or(0)
        }
    } else {
        let Some(offset) = capture::decode_vertex_declaration(&g.vertex_declaration_data)
            .and_then(|attributes| {
                attributes
                    .into_iter()
                    .find(|attribute| attribute.register == 1 && attribute.kind == 0x32)
                    .map(|attribute| attribute.offset as usize)
            })
            .filter(|&offset| offset + 12 <= stride)
        else {
            return Vec::new();
        };
        offset
    };
    let nv = vertices.len() / stride;
    // Older sidecars lack an entry slot; their fallback remains an unverified heuristic.
    let vsh_start = match (vsh_start, consts) {
        (Some(_), Some(_)) if g.program_start.is_some() => g.program_start.map(|v| v as usize),
        (Some(_), Some(c)) => {
            let samples: Vec<[f32; 3]> = (0..nv.min(24))
                .filter_map(|i| {
                    let b = i * stride + off;
                    let f = |k: usize| {
                        f32::from_le_bytes(g.vertices[b + k * 4..b + k * 4 + 4].try_into().unwrap())
                    };
                    let p = [f(0), f(1), f(2)];
                    p.iter()
                        .all(|v| v.is_finite() && v.abs() < 1e6)
                        .then_some(p)
                })
                .collect();
            (0..136usize)
                .max_by_key(|&st| {
                    // Count distinct on-screen-ish outputs: a real program varies with position,
                    // an empty one returns the constant (0, 0, 0, 1) for every vertex.
                    let mut seen: Vec<[u32; 3]> = Vec::new();
                    for &p in &samples {
                        if let Some(o) = run_program(g, st, c, p) {
                            if o[0].abs() < 4000.0 && o[1].abs() < 4000.0 && o[2] != 1.0 {
                                let key = [o[0].to_bits(), o[1].to_bits(), o[2].to_bits()];
                                if !seen.contains(&key) {
                                    seen.push(key);
                                }
                            }
                        }
                    }
                    seen.len()
                })
                .filter(|_| !samples.is_empty())
        }
        _ => vsh_start,
    };
    // Indexed sidecar bytes retain original vertex IDs while the captured
    // vertex span starts at `vertex_first`; convert IDs to span-local indices.
    let pos = |source_index: usize| -> Option<[f32; 3]> {
        let i = if g.indexed == 1 {
            source_index.checked_sub(source_first as usize)?
        } else {
            source_index
        };
        if i >= nv {
            return None;
        }
        let b = i * stride + off;
        let f =
            |k: usize| f32::from_le_bytes(vertices[b + k * 4..b + k * 4 + 4].try_into().unwrap());
        let p = [f(0), f(1), f(2)];
        if !p.iter().all(|v| v.is_finite() && v.abs() < 1e6) {
            return None;
        }
        Some(match consts {
            Some(c) => match vsh_start {
                Some(start) => run_program(g, start, c, p)?,
                None => screen(c, p)?,
            },
            None => p,
        })
    };
    let idx: Vec<usize> = if g.indexed == 1 {
        g.indices
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]) as usize)
            .collect()
    } else {
        (0..nv).collect()
    };
    let mut out = Vec::new();
    let mut tri = |a: usize, b: usize, c: usize| {
        if let (Some(a), Some(b), Some(c)) = (pos(a), pos(b), pos(c)) {
            out.push([a, b, c]);
        }
    };
    match g.primitive {
        4 => idx.chunks_exact(3).for_each(|t| tri(t[0], t[1], t[2])),
        5 => (2..idx.len()).for_each(|i| {
            // Reverse the first two vertices on alternate triangles to
            // preserve winding across the assembled strip.
            if i % 2 == 0 {
                tri(idx[i - 2], idx[i - 1], idx[i]);
            } else {
                tri(idx[i - 1], idx[i - 2], idx[i]);
            }
        }),
        6 => (2..idx.len()).for_each(|i| tri(idx[0], idx[i - 1], idx[i])),
        _ => {}
    }
    out
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let frames = capture::read_frames(&std::fs::read_to_string(&a[1]).unwrap()).unwrap();
    let frame = &frames[a[2].parse::<usize>().unwrap()];
    let yaw = a[4].parse::<f32>().unwrap().to_radians();
    let pitch = a[5].parse::<f32>().unwrap().to_radians();
    let stride = a.get(6).map_or(28, |s| s.parse().unwrap());
    let off = a.get(7).map_or(4, |s| s.parse().unwrap());
    let mut tris: Vec<([[f32; 3]; 3], u64)> = Vec::new();
    for (i, g) in frame.geometry.iter().enumerate() {
        let only = std::env::var("ONLY_DRAW")
            .ok()
            .and_then(|v| v.parse::<usize>().ok());
        if only.is_some_and(|d| d != i) {
            continue;
        }
        if let Some(g) = g {
            let key = frame.draws[i].1;
            let consts: Option<Vec<f32>> = (std::env::var_os("GAME_CONSTS").is_some()
                && g.constants.len() >= 68 * 16)
                .then(|| {
                    g.constants
                        .chunks_exact(4)
                        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                        .collect()
                });
            tris.extend(
                triangles(
                    g,
                    stride,
                    off,
                    consts.as_deref(),
                    std::env::var("GAME_VSH")
                        .ok()
                        .map(|v| v.parse().unwrap_or(0)),
                )
                .into_iter()
                .map(|t| (t, key)),
            );
        }
    }
    assert!(!tris.is_empty(), "no triangles decoded");
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for (t, _) in &tris {
        for p in t {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
    }
    let center = [
        (lo[0] + hi[0]) / 2.,
        (lo[1] + hi[1]) / 2.,
        (lo[2] + hi[2]) / 2.,
    ];
    let radius = dot(sub(hi, lo), sub(hi, lo)).sqrt() / 2.;
    let dir = [
        pitch.cos() * yaw.sin(),
        pitch.sin(),
        pitch.cos() * yaw.cos(),
    ];
    let eye = [
        center[0] + dir[0] * radius * 1.6,
        center[1] + dir[1] * radius * 1.6,
        center[2] + dir[2] * radius * 1.6,
    ];
    let fwd = norm(sub(center, eye));
    let right = norm(cross(fwd, [0., 1., 0.]));
    let up = cross(right, fwd);
    let (w, h) = (640usize, 480usize);
    let focal = h as f32 * 1.1;
    let mut color = vec![[10u8, 14, 22, 255]; w * h];
    let mut depth = vec![f32::MAX; w * h];
    let light = norm([0.4, 0.8, 0.5]);
    // `game` mode (stride arg 5th): row-vector clip = [x y z 1] * M with the guest camera matrix.
    let screen_space = std::env::var_os("GAME_CONSTS").is_some();
    let game = std::env::var_os("GAME_CAMERA")
        .and_then(|_| frame.camera.as_ref())
        .map(|c| c.matrix);
    let proj = |p: [f32; 3]| -> Option<[f32; 3]> {
        if screen_space {
            return Some(p);
        }
        if let Some(m) = game {
            let c = |k: usize| p[0] * m[k] + p[1] * m[4 + k] + p[2] * m[8 + k] + m[12 + k];
            let wv = c(3);
            if wv <= 0.01 {
                return None;
            }
            return Some([
                w as f32 / 2. * (1. + c(0) / wv),
                h as f32 / 2. * (1. - c(1) / wv),
                wv,
            ]);
        }
        let d = sub(p, eye);
        let z = dot(d, fwd);
        (z > 1.0).then(|| {
            [
                w as f32 / 2. + focal * dot(d, right) / z,
                h as f32 / 2. - focal * dot(d, up) / z,
                z,
            ]
        })
    };
    for (t, key) in &tris {
        let n = norm(cross(sub(t[1], t[0]), sub(t[2], t[0])));
        let shade = 0.25 + 0.75 * dot(n, light).abs();
        let base = [
            60.0 + (key & 0x7f) as f32,
            90.0 + ((key >> 8) & 0x7f) as f32,
            120.0 + ((key >> 16) & 0x7f) as f32,
        ];
        let rgb =
            [base[0] * shade, base[1] * shade, base[2] * shade].map(|v| v.clamp(0., 255.) as u8);
        let (Some(p0), Some(p1), Some(p2)) = (proj(t[0]), proj(t[1]), proj(t[2])) else {
            continue;
        };
        let minx = p0[0].min(p1[0]).min(p2[0]).floor().max(0.) as usize;
        let maxx = (p0[0].max(p1[0]).max(p2[0]).ceil().min(w as f32 - 1.)).max(0.) as usize;
        let miny = p0[1].min(p1[1]).min(p2[1]).floor().max(0.) as usize;
        let maxy = (p0[1].max(p1[1]).max(p2[1]).ceil().min(h as f32 - 1.)).max(0.) as usize;
        let area = (p1[0] - p0[0]) * (p2[1] - p0[1]) - (p1[1] - p0[1]) * (p2[0] - p0[0]);
        if area.abs() < 1e-6 {
            continue;
        }
        for y in miny..=maxy {
            for x in minx..=maxx {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let w0 = ((p1[0] - px) * (p2[1] - py) - (p1[1] - py) * (p2[0] - px)) / area;
                let w1 = ((p2[0] - px) * (p0[1] - py) - (p2[1] - py) * (p0[0] - px)) / area;
                let w2 = 1. - w0 - w1;
                if w0 < 0. || w1 < 0. || w2 < 0. {
                    continue;
                }
                let z = w0 * p0[2] + w1 * p1[2] + w2 * p2[2];
                if z < depth[y * w + x] {
                    depth[y * w + x] = z;
                    color[y * w + x] = [rgb[0], rgb[1], rgb[2], 255];
                }
            }
        }
    }
    let rgba: Vec<u8> = color.iter().flatten().copied().collect();
    std::fs::write(&a[3], capture::encode_png(w as u32, h as u32, &rgba)).unwrap();
    println!(
        "UNVERIFIED fitted-camera geometry render: {} triangles, bounds {lo:?}..{hi:?}",
        tris.len()
    );
}
