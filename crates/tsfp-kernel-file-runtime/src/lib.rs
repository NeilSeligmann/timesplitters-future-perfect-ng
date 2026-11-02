//! Native owner of the kernel file namespace, mounts and open file resources.
//! Opt-in replacement: capture/replay adoption evidence is required before retiring C.
pub mod host;
pub mod storage;
pub mod runtime;
pub mod runtime_lock;
pub mod ffi;
pub mod guest;
mod guest_memory;
pub use runtime::{runtime, Runtime};
pub use storage::Metadata;
#[cfg(test)]
mod tests;
