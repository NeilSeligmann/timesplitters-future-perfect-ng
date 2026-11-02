// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: mirrors=tsfp-engine-physics/fp_leaves
// portreg: mirrors=tsfp-gameplay/story_events/sweep_cells
//! S4 Tier 2 round 2 (T2117): lane adapters for the three heavy leaves of the mesh ray cast that T2099 left running lifted:
//! `0x47B30` (ray record init, 2.4M calls), `0x47BD0` (ray against box slab test, 1.0M) and `0x913F0` (node tree walk of one
//! mesh record, 0.6M). The bodies are the readable ports `tsfp-engine-physics::fp_leaves::ray_init_from_origin_direction_length`
//! and `tsfp-gameplay::story_events::sweep_cells::{ray_box_47bd0, walk_913f0}` (MEASURED against the lifted originals by their own
//! differential tests), restated here over the guest memory with the original cdecl argument form because `tsfp-ports`
//! does not depend on the gameplay crate. The walk calls the entry test `0x47D50` through the guest-call trampoline, so
//! it reaches the `physics_lane` translation when that root is selected too.
//!
//! Register contract: EAX carries the result (the boolean of `0x47BD0` and `0x913F0`); `0x47BD0` also leaves its bounds
//! argument in EDX, matching the original. Other ECX/EDX and XMM scratch values are volatile in callers (the original
//! `0x47B30` leaves a stale ucomiss flag byte in AH). ESP is restored to the entry value like `physics_lane.rs`.
use crate::{
    abi::{ExtRegisters, GuestCall, call_cdecl},
    memory::{GuestMemory, MappedGuest},
};
use tsfp_port_engine_physics_ray::{Ray, read_ray};

/// The lane roots in registration order (appended to `lane_inv::INV_ROOTS`).
pub const ROOTS: [u32; 3] = [0x47b30, 0x47bd0, 0x913f0];

/// `[0x475DCC]`: scale of the quantised child boxes (`1/255`), read from the image like the original.
const BOX_SCALE_ADDRESS: u32 = 0x475dcc;

fn arg<M: GuestMemory>(m: &M, esp: u32, index: u32) -> u32 {
    m.read32(esp.wrapping_add(4 + 4 * index))
}

fn float<M: GuestMemory>(m: &M, address: u32) -> f32 {
    f32::from_bits(m.read32(address))
}

/// `0x47B30(ray, origin, direction, length)`: origin to `ray+0`, direction to `ray+12`, length to `ray+36` and the
/// reciprocal of each direction component to `ray+24..32` (0 for a component that compares equal to 0).
fn ray_init<M: GuestMemory>(m: &mut M, esp: u32, regs: &mut ExtRegisters) {
    regs.eax = tsfp_port_engine_physics_ray::ray_init(m, esp);
}

/// Merges one slab pair `(ta, tb)` into `(near, far)`; `None` is a miss.
fn merge_slab(near: f32, far: f32, ta: f32, tb: f32, length: f32) -> Option<(f32, f32)> {
    let (slab_near, slab_far) = if ta > tb { (tb, ta) } else { (ta, tb) };
    let (mut near, mut far) = (near, far);
    if far > slab_far {
        if 0.0 > slab_far {
            return None;
        }
        far = slab_far;
    }
    if slab_near > near {
        if slab_near > length {
            return None;
        }
        near = slab_near;
    }
    if near > far {
        return None;
    }
    Some((near, far))
}

/// `0x47BD0(ray, bounds)`: slab test of the ray against `[min xyz, max xyz]` within `ray.length`. The final `near > far`
/// check is made by the last non-parallel axis pair among z and y only (a ray parallel to y returns true without it).
fn ray_box(ray: &Ray, bounds: &[f32; 6]) -> bool {
    let (o, inv, length) = (ray.origin, ray.inverse, ray.length);
    let (mut near, mut far);
    if ray.direction[0] != 0.0 || ray.direction[0].is_nan() {
        let ta = (bounds[0] - o[0]) * inv[0];
        let tb = (bounds[3] - o[0]) * inv[0];
        let (n, f) = if ta > tb { (tb, ta) } else { (ta, tb) };
        if 0.0 > f || n > length {
            return false;
        }
        (near, far) = (n, f);
    } else {
        if bounds[0] > o[0] || o[0] > bounds[3] {
            return false;
        }
        (near, far) = (0.0, length);
    }
    if ray.direction[2] != 0.0 || ray.direction[2].is_nan() {
        let ta = (bounds[2] - o[2]) * inv[2];
        let tb = (bounds[5] - o[2]) * inv[2];
        match merge_slab(near, far, ta, tb, length) {
            Some(merged) => (near, far) = merged,
            None => return false,
        }
    } else if bounds[2] > o[2] || o[2] > bounds[5] {
        return false;
    }
    if ray.direction[1] != 0.0 || ray.direction[1].is_nan() {
        let ta = (bounds[1] - o[1]) * inv[1];
        let tb = (bounds[4] - o[1]) * inv[1];
        return merge_slab(near, far, ta, tb, length).is_some();
    }
    !(bounds[1] > o[1] || o[1] > bounds[4])
}

fn read_box<M: GuestMemory>(m: &M, address: u32) -> [f32; 6] {
    core::array::from_fn(|k| float(m, address.wrapping_add(4 * k as u32)))
}

fn lane_ray_box<M: GuestMemory>(m: &M, ray: u32, bounds: u32) -> bool {
    ray_box(&read_ray(m, ray), &read_box(m, bounds))
}

/// Inputs of `0x913F0` that stay the same through the recursion.
struct Walk {
    class: u32,
    record_index: u32,
    /// The bounds record (`+0x1C` triangle table, `+0x24` node array).
    bounds: u32,
    ray: u32,
    position_out: u32,
    normal_out: u32,
    flags: u32,
}

/// `0x913F0(class, record, bounds, node, parentBox, ray, &position, &normal, flags)`: walks the node tree of one record
/// (12 byte nodes at `[bounds + 0x24]`). A node with non-zero `u16[+6]` is a leaf with `u16[+0xA]` triangle table entries
/// from `u16[+8]`, any other node has two children at `u16[+8]` and `u16[+0xA]` (tested with the slab test against boxes
/// dequantised from the parent box when `byte[mesh + 0xC]` has bit 7, else walked directly). Accepted entries (tested by
/// `0x47D50`) rewrite the impact block `[0x70AA58..0x70AA78]` and the position and normal outputs. Returns whether any
/// entry was accepted.
fn walk_node<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    walk: &Walk,
    node: u32,
    parent_box: u32,
) -> bool {
    let mesh = m.read32(walk.class.wrapping_add(4));
    if m.read16(node.wrapping_add(6)) != 0 {
        return walk_leaf(m, guest, regs, walk, node, mesh);
    }
    let nodes = m.read32(walk.bounds.wrapping_add(0x24));
    let first = nodes.wrapping_add(u32::from(m.read16(node.wrapping_add(8))) * 12);
    let second = nodes.wrapping_add(u32::from(m.read16(node.wrapping_add(0xa))) * 12);
    if (m.read8(mesh.wrapping_add(0xc)) as i8) >= 0 {
        let a = walk_node(m, guest, regs, walk, first, parent_box);
        let b = walk_node(m, guest, regs, walk, second, parent_box);
        return a | b;
    }
    let parent = read_box(m, parent_box);
    let scale = float(m, BOX_SCALE_ADDRESS);
    let extent = [
        (parent[3] - parent[0]) * scale,
        (parent[4] - parent[1]) * scale,
        (parent[5] - parent[2]) * scale,
    ];
    let mut result = false;
    for child in [first, second] {
        let byte = |k: u32| f32::from(m.read8(child.wrapping_add(k)));
        let child_box = [
            byte(0) * extent[0] + parent[0],
            byte(1) * extent[1] + parent[1],
            byte(2) * extent[2] + parent[2],
            byte(3) * extent[0] + parent[0],
            byte(4) * extent[1] + parent[1],
            byte(5) * extent[2] + parent[2],
        ];
        // The slab test reads the ray record each time: the walk of the first child may have shortened it.
        if !ray_box(&read_ray(m, walk.ray), &child_box) {
            if child == first {
                continue;
            }
            return result;
        }
        // The child box is passed by pointer in the original (a stack temporary): spill it below ESP.
        let spill = regs.esp.wrapping_sub(0x20);
        for (k, value) in child_box.iter().enumerate() {
            m.write32(spill.wrapping_add(4 * k as u32), value.to_bits());
        }
        let entry_esp = regs.esp;
        regs.esp = spill;
        result |= walk_node(m, guest, regs, walk, child, spill);
        regs.esp = entry_esp;
    }
    result
}

fn walk_leaf<M: GuestMemory, C: GuestCall>(
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
    walk: &Walk,
    node: u32,
    mesh: u32,
) -> bool {
    let object = m.read32(walk.class);
    let count = m.read32(mesh);
    let record = mesh.wrapping_add(walk.record_index.wrapping_sub(count).wrapping_mul(0xc8));
    let entries = u32::from(m.read16(node.wrapping_add(0xa)));
    let first_entry = u32::from(m.read16(node.wrapping_add(8)));
    let table = m.read32(walk.bounds.wrapping_add(0x1c));
    let mesh_flags = u32::from(m.read8(mesh.wrapping_add(0xc))) & 0x10;
    let mut result = false;
    for k in 0..entries {
        let entry = table.wrapping_add((first_entry + k) * 12);
        let byte4 = m.read8(entry.wrapping_add(4));
        let selector = u32::from(byte4 >> 6);
        let reference = u32::from(m.read16(entry.wrapping_add(6)));
        let sub_record = record.wrapping_add(selector * 0x18);
        let plane_ref = m
            .read32(record.wrapping_add(0x60 + 4 * selector))
            .wrapping_add(reference * 10);
        if walk.flags & 8 != 0 && byte4 & 0xc0 == 0x40 {
            continue;
        }
        if walk.flags & 0x10 != 0 && byte4 & 0xc0 == 0x80 {
            continue;
        }
        let plane_index = m.read16(plane_ref) as i16;
        let mut plane = 0u32;
        if plane_index >= 0 {
            plane = m
                .read32(mesh.wrapping_add(0x14))
                .wrapping_add((i32::from(plane_index) * 48) as u32);
            let plane_flags = m.read32(plane.wrapping_add(0x1c));
            if plane_flags & 2 != 0 {
                if plane_flags & 1 == 0 {
                    continue;
                }
            } else if plane_flags & 0x8_0000 != 0 || byte4 & 0xc0 == 0x40 {
                continue;
            }
        }
        let vertex_stride = if mesh_flags != 0 { 8 } else { 16 };
        let vertices = m
            .read32(sub_record.wrapping_add(4))
            .wrapping_add(u32::from(m.read16(plane_ref.wrapping_add(6))) * vertex_stride);
        let cull =
            selector == 0 || (selector == 2 && m.read32(object.wrapping_add(0x20)) != 0x1_0000);
        // 0x47D50(ray, vertices, bounds, mesh + 0x38, quantised, entry, cull, &position, &normal) with the two
        // output vectors in a stack temporary, like the original.
        let entry_esp = regs.esp;
        let temporary = entry_esp.wrapping_sub(0x20);
        regs.esp = temporary;
        call_cdecl(
            guest,
            m,
            regs,
            0x47d50,
            &[
                walk.ray,
                vertices,
                walk.bounds,
                mesh.wrapping_add(0x38),
                mesh_flags,
                entry,
                u32::from(cull),
                temporary,
                temporary.wrapping_add(12),
            ],
        );
        let accepted = regs.eax != 0;
        regs.esp = entry_esp;
        if !accepted {
            continue;
        }
        let temporary = entry_esp.wrapping_sub(0x20);
        let (plane_word, plane_byte, plane_value) = if plane != 0 {
            let first_word = m.read32(plane) as i32;
            let word = if first_word >= 0 {
                m.read32(
                    m.read32(mesh.wrapping_add(0x10))
                        .wrapping_add((first_word as u32) << 4),
                )
            } else {
                u32::MAX
            };
            (
                word,
                u32::from(m.read8(plane.wrapping_add(0x18))),
                m.read32(plane.wrapping_add(0x1c)),
            )
        } else {
            (u32::MAX, 0, 0)
        };
        let block = [
            walk.class,
            walk.record_index,
            selector,
            reference,
            first_entry + k,
            plane_word,
            plane_byte,
            plane_value,
        ];
        for (slot, value) in block.iter().enumerate() {
            m.write32(0x70aa58 + 4 * slot as u32, *value);
        }
        for axis in 0..3u32 {
            let position = m.read32(temporary.wrapping_add(4 * axis));
            m.write32(walk.position_out.wrapping_add(4 * axis), position);
            let normal = m.read32(temporary.wrapping_add(12 + 4 * axis));
            m.write32(walk.normal_out.wrapping_add(4 * axis), normal);
        }
        result = true;
    }
    result
}

/// Dispatches a root; `false` when `va` is not one of the adapters.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    m: &mut M,
    guest: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    let entry_esp = regs.esp;
    match va {
        0x47b30 => ray_init(m, entry_esp, regs),
        0x47bd0 => {
            regs.edx = arg(m, entry_esp, 1);
            regs.eax = u32::from(lane_ray_box(m, arg(m, entry_esp, 0), regs.edx));
        }
        0x913f0 => {
            let walk = Walk {
                class: arg(m, entry_esp, 0),
                record_index: arg(m, entry_esp, 1),
                bounds: arg(m, entry_esp, 2),
                ray: arg(m, entry_esp, 5),
                position_out: arg(m, entry_esp, 6),
                normal_out: arg(m, entry_esp, 7),
                flags: arg(m, entry_esp, 8),
            };
            let node = arg(m, entry_esp, 3);
            let parent_box = arg(m, entry_esp, 4);
            // Stack temporaries live below the entry ESP.
            regs.esp = entry_esp.wrapping_sub(0x40);
            let hit = walk_node(m, guest, regs, &walk, node, parent_box);
            regs.eax = u32::from(hit);
        }
        _ => return false,
    }
    regs.esp = entry_esp;
    true
}

macro_rules! export {
    ($name:ident, $va:expr) => {
        /// # Safety
        /// `cpu` is a live C rust_cpu object and offset maps the guest window.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(cpu: *mut crate::lane::LaneCpu, offset: usize) {
            let mut memory = unsafe { MappedGuest::new(offset) };
            crate::lane_inv::execute($va, &mut memory, unsafe { &mut *cpu });
        }
    };
}
export!(tsfp_lane_00047b30, 0x47b30);
export!(tsfp_lane_00047bd0, 0x47bd0);
export!(tsfp_lane_000913f0, 0x913f0);

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{vec, vec::Vec};

    struct Flat(Vec<u8>);
    impl GuestMemory for Flat {
        fn read8(&self, address: u32) -> u8 {
            self.0[address as usize]
        }
        fn write8(&mut self, address: u32, value: u8) {
            self.0[address as usize] = value;
        }
    }

    struct NoCalls;
    impl GuestCall for NoCalls {
        fn call(&mut self, _: u32, _: &mut ExtRegisters) {
            panic!("unexpected guest call")
        }
    }

    #[test]
    fn roots_match_the_dispatch_table() {
        let text = include_str!("physics_lane2.rs");
        let table = text
            .split("match va {")
            .nth(1)
            .expect("table")
            .split("_ => return false")
            .next()
            .expect("end");
        assert_eq!(table.matches(" => ").count(), ROOTS.len());
        assert!(
            table.contains("0x47b30 =>")
                && table.contains("0x47bd0 =>")
                && table.contains("0x913f0 =>")
        );
    }

    #[test]
    fn ray_init_and_box_follow_the_original_forms() {
        let mut m = Flat(vec![0; 0x1000]);
        let esp = 0x800u32;
        let (ray, origin, direction, bounds) = (0x100u32, 0x200u32, 0x220u32, 0x240u32);
        for (k, v) in [1.0f32, 2.0, 3.0].iter().enumerate() {
            m.write32(origin + 4 * k as u32, v.to_bits());
        }
        for (k, v) in [0.0f32, 2.0, -4.0].iter().enumerate() {
            m.write32(direction + 4 * k as u32, v.to_bits());
        }
        for (k, v) in [0.0f32, 0.0, 0.0, 2.0, 8.0, 9.0].iter().enumerate() {
            m.write32(bounds + 4 * k as u32, v.to_bits());
        }
        for (k, v) in [ray, origin, direction, 100.0f32.to_bits()]
            .iter()
            .enumerate()
        {
            m.write32(esp + 4 + 4 * k as u32, *v);
        }
        let mut regs = ExtRegisters {
            eax: 0,
            ecx: 0,
            edx: 0,
            ebx: 0,
            ebp: 0,
            esi: 0,
            edi: 0,
            esp,
            eflags: 0,
            fpu: crate::abi::Fpu {
                stack: [0.0; 8],
                top: 0,
                control: 0x27f,
            },
            xmm: [crate::abi::Xmm::default(); 5],
            xmm_hi: [crate::abi::Xmm::default(); 3],
        };
        ray_init(&mut m, esp, &mut regs);
        assert_eq!(float(&m, ray + 24), 0.0);
        assert_eq!(float(&m, ray + 28), 0.5);
        assert_eq!(float(&m, ray + 32), -0.25);
        assert_eq!(float(&m, ray + 36), 100.0);
        m.write32(esp + 4, ray);
        m.write32(esp + 8, bounds);
        let mut guest = NoCalls;
        assert!(run(0x47bd0, &mut m, &mut guest, &mut regs));
        assert_eq!(regs.eax, 1);
        assert_eq!(regs.edx, bounds);
        m.write32(ray, 5.0f32.to_bits());
        assert!(run(0x47bd0, &mut m, &mut guest, &mut regs));
        assert_eq!(regs.eax, 0);
        assert_eq!(regs.edx, bounds);
    }
}
