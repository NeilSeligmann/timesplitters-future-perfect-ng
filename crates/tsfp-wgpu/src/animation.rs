//! Native model-to-vertex-program palette binding (T2241).
//! Shader/model data supplies the layout; no guessed bone registers or guest writes.
use crate::stream::Event;
use crate::vsh::CONSTANT_COUNT;
use tsfp_engine_anim::posemath::Mat4;
use tsfp_engine_render::device::DeviceCall;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatrixLayout {
    Rows,
    Columns,
}

/// One model-selected node mapped to explicit vertex-program constants.
#[derive(Clone, Copy, Debug)]
pub struct NodeBinding {
    pub node: usize,
    pub first_register: u32,
    pub vectors: u8,
    pub layout: MatrixLayout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingError {
    MissingNode,
    InvalidRange,
    Overlap,
    NonFinite,
}

/// Validate all bindings and prepare uploads before mutating renderer state.
/// The model may bind a subset of nodes. Missing required nodes fail closed.
pub fn prepare(nodes: &[Mat4], bindings: &[NodeBinding]) -> Result<Vec<Event>, BindingError> {
    let mut occupied = [false; CONSTANT_COUNT];
    let mut events = Vec::with_capacity(bindings.len());
    for binding in bindings {
        let matrix = nodes.get(binding.node).ok_or(BindingError::MissingNode)?;
        if !matrix.iter().all(|v| v.is_finite()) {
            return Err(BindingError::NonFinite);
        }
        let first = binding.first_register as usize;
        let vectors = usize::from(binding.vectors);
        let end = first
            .checked_add(vectors)
            .ok_or(BindingError::InvalidRange)?;
        if vectors == 0 || vectors > 4 || end > CONSTANT_COUNT {
            return Err(BindingError::InvalidRange);
        }
        if occupied[first..end].iter().any(|value| *value) {
            return Err(BindingError::Overlap);
        }
        occupied[first..end].fill(true);
        let mut words = Vec::with_capacity(vectors * 4);
        for vector in 0..vectors {
            for component in 0..4 {
                let index = match binding.layout {
                    MatrixLayout::Rows => vector * 4 + component,
                    MatrixLayout::Columns => component * 4 + vector,
                };
                words.push(matrix[index].to_bits());
            }
        }
        events.push(Event::Call(DeviceCall::SetVertexConstants {
            first_register: binding.first_register,
            words,
        }));
    }
    Ok(events)
}

/// Model-owned draw command: geometry/program/texture state is installed by the
/// native model owner before submission. Each submission binds one object's
/// sampled palette immediately before its draw, without touching simulation.
#[derive(Clone, Copy, Debug)]
pub enum Geometry {
    Vertices {
        primitive: u32,
        start: u32,
        count: u32,
    },
    Indexed {
        primitive: u32,
        count: u32,
        indices: u32,
    },
}

impl Geometry {
    pub fn event(self) -> Event {
        Event::Call(match self {
            Self::Vertices {
                primitive,
                start,
                count,
            } => DeviceCall::DrawVertices {
                primitive,
                start,
                count,
            },
            Self::Indexed {
                primitive,
                count,
                indices,
            } => DeviceCall::DrawIndexedVertices {
                primitive,
                count,
                indices,
            },
        })
    }
}

/// Prepare the complete palette + geometry packet before queueing anything.
/// Object lifetimes, clip cache and display policy are native Rust inputs.
/// No guest pointer dereferences, recorded-frame replay or simulation advances.
pub fn prepare_draw(
    scene: &tsfp_engine_anim::display_scene::AnimationScene,
    view: tsfp_engine_anim::display_scene::ViewObject,
    alpha: f32,
    source: &impl tsfp_engine_anim::display::ClipSource,
    bindings: &[NodeBinding],
    geometry: Geometry,
) -> Result<Vec<Event>, String> {
    let batch = scene
        .sample_view(&[view], alpha, source)
        .map_err(|error| format!("native animation sample: {error:?}"))?;
    let mut events = prepare(&batch.palettes[0].nodes, bindings)
        .map_err(|error| format!("native animation binding: {error:?}"))?;
    events.push(geometry.event());
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn binding(node: usize, first_register: u32, layout: MatrixLayout) -> NodeBinding {
        NodeBinding {
            node,
            first_register,
            vectors: 3,
            layout,
        }
    }
    #[test]
    fn model_selects_subset_and_explicit_transposition() {
        let matrix = std::array::from_fn(|i| i as f32);
        let events = prepare(&[matrix], &[binding(0, 12, MatrixLayout::Columns)]).unwrap();
        let Event::Call(DeviceCall::SetVertexConstants {
            first_register,
            words,
        }) = &events[0]
        else {
            panic!()
        };
        assert_eq!(*first_register, 12);
        assert_eq!(
            words
                .iter()
                .copied()
                .map(f32::from_bits)
                .collect::<Vec<_>>(),
            [
                0.0, 4.0, 8.0, 12.0, 1.0, 5.0, 9.0, 13.0, 2.0, 6.0, 10.0, 14.0
            ]
        );
        let rows = prepare(&[matrix], &[binding(0, 0, MatrixLayout::Rows)]).unwrap();
        let Event::Call(DeviceCall::SetVertexConstants { words, .. }) = &rows[0] else {
            panic!()
        };
        assert_eq!(
            words,
            &matrix[..12].iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
    }
    #[test]
    fn rejects_invalid_model_contract_before_upload() {
        let matrix = [1.0; 16];
        assert_eq!(
            prepare(&[matrix], &[binding(1, 0, MatrixLayout::Rows)]).unwrap_err(),
            BindingError::MissingNode
        );
        assert_eq!(
            prepare(&[matrix], &[binding(0, 191, MatrixLayout::Rows)]).unwrap_err(),
            BindingError::InvalidRange
        );
        assert_eq!(
            prepare(&[matrix], &[binding(0, u32::MAX, MatrixLayout::Rows)]).unwrap_err(),
            BindingError::InvalidRange
        );
        assert_eq!(
            prepare(
                &[matrix],
                &[
                    binding(0, 0, MatrixLayout::Rows),
                    binding(0, 2, MatrixLayout::Columns)
                ]
            )
            .unwrap_err(),
            BindingError::Overlap
        );
        assert_eq!(
            prepare(&[[f32::NAN; 16]], &[binding(0, 0, MatrixLayout::Rows)]).unwrap_err(),
            BindingError::NonFinite
        );
        let mut zero = binding(0, 0, MatrixLayout::Rows);
        zero.vectors = 0;
        assert_eq!(
            prepare(&[matrix], &[zero]).unwrap_err(),
            BindingError::InvalidRange
        );
    }
}
