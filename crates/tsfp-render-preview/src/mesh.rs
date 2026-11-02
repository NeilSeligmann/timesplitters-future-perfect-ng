// SPDX-License-Identifier: GPL-3.0-or-later
//! Validated triangle geometry supplied by a native scene resource producer.

const MAX_INDICES: usize = 3_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeshError {
    EmptyOrOversized,
    IncompleteTriangle,
    NonFinitePosition,
    ClipWCountMismatch,
    NonFiniteClipW,
    ColorCountMismatch,
    NonFiniteColor,
    UvCountMismatch,
    NonFiniteUv,
    IndexOutOfBounds,
}

/// Expand an indexed triangle list to the position-only vertex format used by
/// the preview. Reject malformed resources before creating a GPU buffer.
pub fn triangle_positions(vertices: &[[f32; 3]], indices: &[u32]) -> Result<Vec<u8>, MeshError> {
    if indices.is_empty() || indices.len() > MAX_INDICES {
        return Err(MeshError::EmptyOrOversized);
    }
    if indices.len() % 3 != 0 {
        return Err(MeshError::IncompleteTriangle);
    }
    let mut bytes = Vec::with_capacity(indices.len() * 12);
    for &index in indices {
        let position = vertices
            .get(index as usize)
            .ok_or(MeshError::IndexOutOfBounds)?;
        if !position.iter().all(|value| value.is_finite()) {
            return Err(MeshError::NonFinitePosition);
        }
        for value in position {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
    }
    Ok(bytes)
}

/// Expand indexed triangles to the preview's interleaved position/color layout.
/// Missing colors use white so existing native mesh producers preserve their tint.
pub fn triangle_vertices(
    vertices: &[[f32; 3]],
    colors: Option<&[[f32; 4]]>,
    indices: &[u32],
) -> Result<Vec<u8>, MeshError> {
    triangle_vertices_with_clip_w(vertices, None, colors, indices)
}

/// Expand indexed triangles while retaining an optional homogeneous clip W.
/// Missing W values use 1 so existing scene meshes keep their affine path.
pub fn triangle_vertices_with_clip_w(
    vertices: &[[f32; 3]],
    clip_w: Option<&[f32]>,
    colors: Option<&[[f32; 4]]>,
    indices: &[u32],
) -> Result<Vec<u8>, MeshError> {
    triangle_vertices_with_attributes(vertices, clip_w, colors, None, indices)
}

/// Expand an indexed triangle list to position, colour, and UV attributes.
/// Missing colour defaults to white and missing UV defaults to zero.
pub fn triangle_vertices_with_attributes(
    vertices: &[[f32; 3]],
    clip_w: Option<&[f32]>,
    colors: Option<&[[f32; 4]]>,
    uvs: Option<&[[f32; 2]]>,
    indices: &[u32],
) -> Result<Vec<u8>, MeshError> {
    if indices.is_empty() || indices.len() > MAX_INDICES {
        return Err(MeshError::EmptyOrOversized);
    }
    if indices.len() % 3 != 0 {
        return Err(MeshError::IncompleteTriangle);
    }
    if colors.is_some_and(|colors| colors.len() != vertices.len()) {
        return Err(MeshError::ColorCountMismatch);
    }
    if clip_w.is_some_and(|clip_w| clip_w.len() != vertices.len()) {
        return Err(MeshError::ClipWCountMismatch);
    }
    if uvs.is_some_and(|uvs| uvs.len() != vertices.len()) {
        return Err(MeshError::UvCountMismatch);
    }
    let mut bytes = Vec::with_capacity(indices.len() * 40);
    for &index in indices {
        let position = vertices
            .get(index as usize)
            .ok_or(MeshError::IndexOutOfBounds)?;
        if !position.iter().all(|value| value.is_finite()) {
            return Err(MeshError::NonFinitePosition);
        }
        let w = clip_w.map(|clip_w| clip_w[index as usize]).unwrap_or(1.0);
        if !w.is_finite() {
            return Err(MeshError::NonFiniteClipW);
        }
        let color = colors
            .map(|colors| colors[index as usize])
            .unwrap_or([1.0; 4]);
        if !color.iter().all(|value| value.is_finite()) {
            return Err(MeshError::NonFiniteColor);
        }
        let uv = uvs.map(|uvs| uvs[index as usize]).unwrap_or([0.0; 2]);
        if !uv.iter().all(|value| value.is_finite()) {
            return Err(MeshError::NonFiniteUv);
        }
        for value in position
            .iter()
            .copied()
            .chain([w])
            .chain(color.iter().copied())
            .chain(uv)
        {
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_indexed_geometry_and_rejects_bad_resources() {
        let vertices = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 9.0]];
        let bytes = triangle_positions(&vertices, &[2, 0, 1]).unwrap();
        let floats: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_ne_bytes(chunk.try_into().unwrap()))
            .collect();
        assert_eq!(floats, [7., 8., 9., 1., 2., 3., 4., 5., 6.]);
        assert_eq!(
            triangle_positions(&vertices, &[]),
            Err(MeshError::EmptyOrOversized)
        );
        assert_eq!(
            triangle_positions(&vertices, &[0, 1]),
            Err(MeshError::IncompleteTriangle)
        );
        assert_eq!(
            triangle_positions(&vertices, &[0, 1, 3]),
            Err(MeshError::IndexOutOfBounds)
        );
        assert_eq!(
            triangle_positions(&[[f32::NAN, 0., 0.]], &[0, 0, 0]),
            Err(MeshError::NonFinitePosition)
        );
    }

    #[test]
    fn expands_vertex_colors_and_rejects_malformed_color_data() {
        let positions = [[1., 2., 3.], [4., 5., 6.], [7., 8., 9.]];
        let colors = [[1., 0., 0., 1.], [0., 1., 0., 1.], [0., 0., 1., 1.]];
        let bytes = triangle_vertices(&positions, Some(&colors), &[2, 0, 1]).unwrap();
        let values: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_ne_bytes(chunk.try_into().unwrap()))
            .collect();
        assert_eq!(
            values,
            [
                7., 8., 9., 1., 0., 0., 1., 1., 0., 0., 1., 2., 3., 1., 1., 0., 0., 1., 0., 0., 4.,
                5., 6., 1., 0., 1., 0., 1., 0., 0.,
            ]
        );
        assert_eq!(
            triangle_vertices(&positions, Some(&colors[..2]), &[0, 1, 2]),
            Err(MeshError::ColorCountMismatch)
        );
        assert_eq!(
            triangle_vertices(&positions, Some(&[[0., f32::NAN, 0., 1.]; 3]), &[0, 1, 2]),
            Err(MeshError::NonFiniteColor)
        );
    }

    #[test]
    fn homogeneous_clip_w_is_retained_and_validated() {
        let positions = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]];
        let clip_w = [-1., 1., 1.];
        let bytes =
            triangle_vertices_with_clip_w(&positions, Some(&clip_w), None, &[0, 1, 2]).unwrap();
        let values: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|chunk| f32::from_ne_bytes(chunk.try_into().unwrap()))
            .collect();
        assert_eq!(&values[..4], &[0., 0., 0., -1.]);
        assert_eq!(
            triangle_vertices_with_clip_w(&positions, Some(&clip_w[..2]), None, &[0, 1, 2]),
            Err(MeshError::ClipWCountMismatch)
        );
        assert_eq!(
            triangle_vertices_with_clip_w(
                &positions,
                Some(&[0., f32::INFINITY, 1.]),
                None,
                &[0, 1, 2]
            ),
            Err(MeshError::NonFiniteClipW)
        );
    }
}
