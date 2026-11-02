//! engine-render: architecture boundary reserved by T1812.
//! T1848 wave 4 adds call-free proven render leaves in [`leaves`]; see docs/t1848-rust-wave4.md.
//! T1898 adds the render state, draw dispatch and vertex batch slice behind the
//! [`device::RenderDevice`] trait; see docs/t-render-slice.md.

extern crate alloc;
pub mod hook_policy;

#[path = "../../tsfp-engine-memory/src/gprs.rs"]
pub mod gprs;
pub mod leaves;
#[path = "../../tsfp-engine-memory/src/leaf.rs"]
pub mod memory;

/// T1929 scene translations and explicit guest-state boundary.
pub mod scene_batch;
pub mod scene_boundary;

pub mod camera_projection;
pub mod device;
pub mod draw;
pub mod effect;
pub mod effect_setup;
pub mod emitter;
pub mod lighting;
pub mod map;
pub mod mem;
pub mod overlay;
pub mod particle;
pub mod projection;
pub mod render_math;
pub mod scene;
pub mod state;
pub mod trace_wire;
pub mod transform_history;
pub mod vertex;
pub mod vertex_format;
/// Rust-owned particle state for the native render-prep attachment point (T1699).
pub mod visual_effects;

pub mod renderer;
pub use renderer::Renderer;
pub use tsfp_ports::c2r;
// T2060: scene resource loading shares the guest-backed menu click cells.
pub use tsfp_ports::menu_state;

/// T1938 scene translations: recovered T1929 roots and the remaining scene breadth.
pub mod scene_t1938;

// T2330: shared typed camera contracts also compiled into the no_std lane.
/// T2238 immutable native spatial traversal and render preparation.
pub use scene::spatial;
pub use tsfp_ports::camera;
// Share the libm-backed no_std math used by the path-included render sources.
pub use tsfp_ports::fmath;

/// T2238 bridge from lifecycle membership to native render snapshots.
pub mod native_scene;
