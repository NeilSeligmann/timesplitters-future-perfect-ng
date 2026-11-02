//! T2046 audio replace system. Contract and inventory: docs/t-audio-replace.md.
//! No C, no guest memory: the guest-visible DirectSound state machine stays in the host adapter,
//! this crate is the host side (decode, mix, sink) behind [`contract::SoundHost`].

pub mod adapter;
pub mod adapter_replay;
pub mod adpcm;
mod byte_words;
pub mod completion;
pub mod contract;
pub mod frames;
pub mod gain;
mod guest_ranges;
pub mod lifecycle;
pub mod line;
mod log_hex;
pub mod mixer;
pub mod movie;
pub mod recording;
pub mod replay;
mod report_counts;
pub mod routing;
pub mod scope;
pub mod setters;
pub mod sink;
pub mod stream_lease;

pub mod stretch;

pub mod hrtf;

pub mod device_frames;

pub mod buffer_frames;

pub mod listener;

pub mod mixbin;

pub mod effects_binding;
pub mod effects_cipher;
pub mod effects_identity;
pub mod effects_metadata;

pub mod game_boundary;
