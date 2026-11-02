// SPDX-License-Identifier: GPL-3.0-or-later
//! T2024: a wgpu renderer behind the T1898 `RenderDevice` boundary, developed offline.
//!
//! Flow: the render code (tsfp-engine-render) runs against a [`stream::Recorder`], which keeps
//! the `RenderDevice`/`RenderHost` calls, the vertex bytes the draws read and the textures as a
//! [`stream::Stream`]. The [`replay::Replayer`] executes a stream on a headless wgpu device
//! ([`context::GpuContext`]) and returns frames. See docs/t-wgpu-renderer.md.
pub mod animation;
pub mod binary;
mod clear_policy;
pub mod combiner;
pub mod context;
pub mod cutscene;
pub mod frame_layer;
pub mod grade;
pub mod live;
pub mod native_scene;
pub mod nv2a;
mod pipeline_key;
pub mod png;
pub mod present;
pub mod readback;
pub mod replay;
pub mod scenes;
mod scissor;
pub mod stream;
pub mod texture;
pub mod vertex;
pub mod vsh;
pub mod vsh_wgsl;
pub use tsfp_engine_render::trace_wire;

pub mod shared;
