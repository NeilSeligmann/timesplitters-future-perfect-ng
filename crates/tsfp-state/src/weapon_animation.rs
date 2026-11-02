// SPDX-License-Identifier: GPL-3.0-or-later
//! T1831: readable 0x350EC0 composite preparation over owned projections.
//! Tier 1 translated-reference/synthetic evidence; no production admission.
//! Contract and limitations: docs/t1831-weapon-animation.md.
use crate::weapon_select::SelectionPreset;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Matrix(pub [f32; 16]);
pub type Quaternion = [f32; 4];
pub type Direction = [f32; 3];
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NodeKey {
    pub rotation: Quaternion,
    pub translation: Direction,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Part {
    pub node: usize,
    pub offset: Direction,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Connection {
    pub row: usize,
    pub parts: [usize; 2],
    pub relative_rotation: Quaternion,
    pub directions: [Direction; 2],
}
/// Runtime loader owns these arrays. None contains a guest address.
pub struct Template<'a> {
    pub animation: i32,
    pub parents: &'a [Option<usize>],
    pub parts: &'a [Part],
    pub node_parts: &'a [Option<usize>],
    pub node_outputs: &'a mut [Matrix],
    pub connections: &'a mut [Connection],
}
/// Extension boundary for the animation decoder and shared math system.
/// Implementations must not mutate template metadata during this operation.
pub trait AnimationMath {
    fn decode(&mut self, animation: i32, node: usize, frame: u32, scale: f32) -> NodeKey;
    fn identity(&mut self) -> Matrix;
    fn rotate_matrix(&mut self, matrix: &mut Matrix, rotation: Quaternion);
    fn multiply(&mut self, left: Matrix, right: Matrix) -> Matrix;
    fn transform_direction(&mut self, matrix: Matrix, direction: Direction) -> Direction;
    /// 0x152260 writes an inverse to the first argument; singular input may
    /// leave its previous contents intact. Its historical name is incomplete.
    fn inverse_into(&mut self, destination: &mut Matrix, source: Matrix);
    fn quaternion(&mut self, matrix: Matrix) -> Quaternion;
    fn conjugate(&mut self, quaternion: Quaternion) -> Quaternion;
    fn multiply_quaternions(&mut self, left: Quaternion, right: Quaternion) -> Quaternion;
    fn inverse_rotate(&mut self, matrix: Matrix, direction: Direction) -> Direction;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidTemplate {
    TemplateCount,
    ScratchCapacity,
    NodeIndex,
    PartIndex,
    ConnectionCount,
    RowIndex,
}
/// Caller owns initialized scratch; retained contents matter for singular
/// inverses, reused animations, and the original initial animation -1 sentinel.
/// Scratch arrays must be disjoint and part scratch must fit the original 0x200
/// byte prefix. Reject malformed projections before any neighbour is invoked.
pub fn prepare<M: AnimationMath>(
    templates: &mut [Template<'_>],
    nodes: &mut [Matrix],
    inverses: &mut [Matrix],
    math: &mut M,
) -> Result<(), InvalidTemplate> {
    if templates.len() != 7 {
        return Err(InvalidTemplate::TemplateCount);
    }
    for template in templates.iter() {
        let count = template.parents.len();
        if count > nodes.len() || template.parts.len() > inverses.len() || template.parts.len() > 8
        {
            return Err(InvalidTemplate::ScratchCapacity);
        }
        if template.node_parts.len() != count
            || template.node_outputs.len() != count
            || template.parents.iter().flatten().any(|&n| n >= count)
            || template.parts.iter().any(|part| part.node >= count)
        {
            return Err(InvalidTemplate::NodeIndex);
        }
        if template
            .node_parts
            .iter()
            .flatten()
            .any(|&p| p >= template.parts.len())
            || template
                .connections
                .iter()
                .any(|c| c.parts.iter().any(|&p| p >= template.parts.len()))
        {
            return Err(InvalidTemplate::PartIndex);
        }
        if template.connections.len() != template.parts.len().saturating_sub(1) {
            return Err(InvalidTemplate::ConnectionCount);
        }
        if template.connections.iter().any(|c| c.row >= 4) {
            return Err(InvalidTemplate::RowIndex);
        }
    }
    let mut previous_animation = -1;
    for template in templates {
        if template.animation != previous_animation {
            previous_animation = template.animation;
            for (node, parent) in template.parents.iter().enumerate() {
                let key = math.decode(template.animation, node, 0, 1.0);
                let mut transform = math.identity();
                math.rotate_matrix(&mut transform, key.rotation);
                transform.0[12..15].copy_from_slice(&key.translation);
                nodes[node] = transform;
                if let Some(parent) = parent {
                    nodes[node] = math.multiply(nodes[node], nodes[*parent]);
                }
            }
        }
        for (part_index, part) in template.parts.iter().enumerate() {
            let offset = math.transform_direction(nodes[part.node], part.offset);
            for (axis, value) in offset.into_iter().enumerate() {
                nodes[part.node].0[12 + axis] += value;
            }
            math.inverse_into(&mut inverses[part_index], nodes[part.node]);
        }
        for (node, part) in template.node_parts.iter().enumerate() {
            if let Some(part) = part {
                template.node_outputs[node] = math.multiply(nodes[node], inverses[*part]);
            }
        }
        for connection in template.connections.iter_mut() {
            let first = nodes[template.parts[connection.parts[0]].node];
            let second = nodes[template.parts[connection.parts[1]].node];
            let first_rotation = math.quaternion(first);
            let second_rotation = math.quaternion(second);
            let inverse_rotation = math.conjugate(first_rotation);
            connection.relative_rotation =
                math.multiply_quaternions(inverse_rotation, second_rotation);
            let row = connection.row * 4;
            let direction = [first.0[row], first.0[row + 1], first.0[row + 2]];
            connection.directions[0] = math.inverse_rotate(first, direction);
            connection.directions[1] = math.inverse_rotate(second, direction);
        }
    }
    Ok(())
}
/// Reference is the shared default-off A/B preset. No host flag is registered.
pub fn dispatch<T>(
    preset: SelectionPreset,
    reference: impl FnOnce() -> T,
    readable: impl FnOnce() -> T,
) -> T {
    crate::weapon_runtime::dispatch(preset, reference, readable)
}
