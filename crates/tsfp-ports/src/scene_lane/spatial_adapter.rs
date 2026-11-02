// SPDX-License-Identifier: GPL-3.0-or-later
//! TEMPORARY-SHIM (removal T2244): decode the hybrid spatial index and invoke
//! legacy leaf handlers. Native traversal is in spatial.rs; it never writes
//! guest state or recursively calls lifted/translated tree code.
use super::spatial::{self, Bounds, TreeNode, TreeSource};
use crate::c2r::rt::RegsExt;
use crate::{
    abi::{ExtRegisters, GuestCall},
    memory::GuestMemory,
};
use alloc::vec::Vec;
use core::cell::RefCell;

struct HybridTree<'a, M> {
    memory: &'a RefCell<&'a mut M>,
    nodes: u32,
    leaves: u32,
    scale: f32,
}
#[derive(Clone, Copy)]
struct Leaf {
    address: u32,
    count: u32,
}
impl<M: GuestMemory> TreeSource for HybridTree<'_, M> {
    type Leaf = Leaf;
    fn node(&self, index: u32) -> Option<TreeNode<Leaf>> {
        let memory = self.memory.borrow();
        let value = memory.read16(self.nodes.checked_add(index.checked_mul(2)?)?);
        if value & 0xc000 == 0xc000 {
            let address = self.leaves.checked_add(u32::from(value & 0x3fff) * 2)?;
            Some(TreeNode::Leaf(Leaf {
                address: address + 2,
                count: u32::from(memory.read16(address)),
            }))
        } else {
            Some(TreeNode::Split {
                axis: usize::from(value >> 14),
                fraction: value & 0x3fff,
            })
        }
    }
    fn split_scale(&self) -> f32 {
        self.scale
    }
}
fn vector<M: GuestMemory>(memory: &M, address: u32) -> [f32; 3] {
    core::array::from_fn(|i| f32::from_bits(memory.read32(address.wrapping_add(i as u32 * 4))))
}

/// Original cdecl roots, per-root lane A/B retains --rust-ports=off as oracle.
/// Only ABI stack writes occur here; simulation output belongs to the callback.
pub fn run<M: GuestMemory, C: GuestCall>(
    va: u32,
    memory: &mut M,
    calls: &mut C,
    regs: &mut ExtRegisters,
) -> bool {
    if va == 0x8bdd0 {
        let polygon = memory.read32(regs.esp + 4);
        let header = memory.read32(polygon);
        let count = i32::from(memory.read16(header + 0x10) as i16) + 1;
        let points: Vec<_> = (0..(regs.ebx as i32).max(0) as u32)
            .map(|i| vector(memory, regs.eax.wrapping_add(i * 12)))
            .collect();
        let planes: Vec<_> = (0..count.max(0) as u32)
            .map(|i| {
                let address = polygon.wrapping_add(8 + i * 16);
                spatial::Plane {
                    normal: vector(memory, address),
                    offset: f32::from_bits(memory.read32(address + 12)),
                }
            })
            .collect();
        regs.eax = u32::from(spatial::points_inside(&points, &planes));
        return true;
    }
    if va != 0x1571c0 && va != 0x157440 {
        return false;
    }
    let entry = regs.esp;
    let arg = |n: u32| memory.read32(entry.wrapping_add(4 + n * 4));
    let tree = arg(0);
    let min = arg(1);
    let max = arg(2);
    let query = Bounds {
        min: vector(memory, min),
        max: vector(memory, max),
    };
    let (root, cell, callback, context) = if va == 0x1571c0 {
        (
            arg(3),
            Bounds {
                min: vector(memory, arg(4)),
                max: vector(memory, arg(5)),
            },
            arg(6),
            arg(7),
        )
    } else {
        (
            1,
            Bounds {
                min: vector(memory, tree + 8),
                max: vector(memory, tree + 0x14),
            },
            arg(3),
            arg(4),
        )
    };
    let nodes = memory.read32(tree);
    let leaves = memory.read32(tree + 4);
    let scale = f32::from_bits(memory.read32(0x47dd70));
    if va == 0x157440 && !query.overlaps(cell) {
        regs.eax = 0;
        return true;
    }
    let memory = RefCell::new(memory);
    let source = HybridTree {
        memory: &memory,
        nodes,
        leaves,
        scale,
    };
    let saved = (regs.ebx, regs.ebp, regs.esi, regs.edi);
    let mut callback_started = false;
    let result = spatial::walk(&source, query, cell, root, |leaf| {
        callback_started = true;
        // Enough room below the caller for callback arguments. r.call supplies
        // its synthetic return word; callbacks retain their original convention.
        regs.esp = entry.wrapping_sub(0x40);
        {
            let mut memory = memory.borrow_mut();
            for value in [leaf.address, leaf.count, context, max, min] {
                regs.push(&mut **memory, value);
            }
        }
        regs.call(calls, callback);
        regs.eax
    });
    regs.esp = entry;
    (regs.ebx, regs.ebp, regs.esi, regs.edi) = saved;
    // Invalid trees are refused by guarded access. For arithmetic overflow the
    // callback-free path gives original fallback, never a fabricated success.
    match result {
        Ok(value) => {
            regs.eax = value;
            true
        }
        Err(_) if !callback_started => false,
        // Re-running the reference after a callback would repeat its simulation
        // writes. Invalid post-callback trees fail closed rather than double-submit.
        Err(error) => panic!("invalid native spatial tree after callback: {error:?}"),
    }
}
