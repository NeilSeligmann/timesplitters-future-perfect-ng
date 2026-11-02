// SPDX-License-Identifier: GPL-3.0-or-later
//! The `Renderer` object, a file of its own so the lane build (tsfp-ports, T1976) can include it at its crate root.
use crate::device::{RenderDevice, RenderHost};
use crate::memory::GuestMemory;

/// The render system as one object: the guest image that holds its state tables, the device
/// it draws on and the host that provides the not yet ported neighbours. Every ported render
/// function is a method on this type, so the HUD, scene and effects systems attach by owning
/// or borrowing one `Renderer`.
pub struct Renderer<'a, M: GuestMemory, D: RenderDevice, H: RenderHost> {
    pub mem: &'a mut M,
    pub device: &'a mut D,
    pub host: &'a mut H,
}

impl<'a, M: GuestMemory, D: RenderDevice, H: RenderHost> Renderer<'a, M, D, H> {
    pub fn new(mem: &'a mut M, device: &'a mut D, host: &'a mut H) -> Self {
        Self { mem, device, host }
    }
}
