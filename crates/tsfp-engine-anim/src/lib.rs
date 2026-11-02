//! Original-verified integer anim leaf; see docs/t1835-rust-wave2.md.
extern crate alloc;

pub mod advance;
pub mod aim;
pub mod blend;
pub mod boundary;
pub mod bounds;
pub mod buffers;
pub mod flag_leaves;
pub mod fmath;
#[path = "../../tsfp-engine-memory/src/gprs.rs"]
pub mod gprs;
pub mod heading;
pub mod instance;
pub mod interp;
pub mod keybracket;
pub mod keyrec;
pub mod layers;
pub mod leaf;
pub mod levelsets;
pub mod loaders;
#[path = "../../tsfp-engine-memory/src/leaf.rs"]
pub mod memory;
pub mod partwalk;
pub mod pose;
pub mod posemath;
pub mod posemix;
pub mod prop;
pub mod queries;
pub mod start;
pub mod tail;

pub mod binding;
pub mod dependency_prefix;
pub mod resource_catalog;
pub mod resource_load;

pub mod display;

pub mod prop_display;

pub mod presentation;

pub mod display_scene;

pub mod simulation;
