// SPDX-License-Identifier: GPL-3.0-or-later
//! T2214 readable local assignment and pair binding. See docs/t2214-local-players.md.
use crate::GuestMemory;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Preset {
    #[default]
    Reference,
    ReadableOriginal,
}

impl Preset {
    /// Lazy A/B routing. The adapter supplies the retained C/translated call as `reference`.
    /// Neither branch runs until explicitly selected, and default never runs readable code.
    pub fn run<T>(self, reference: impl FnOnce() -> T, readable: impl FnOnce() -> T) -> T {
        match self {
            Self::Reference => reference(),
            Self::ReadableOriginal => readable(),
        }
    }
}

/// Per-root selection stays off until a host adapter and Tier 2 evidence admit it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Switches {
    pub find_pad: Preset,
    pub get_pad: Preset,
    pub set_pad: Preset,
    pub layout: Preset,
    pub pair: Preset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalSlots {
    pub setup_count: u32,
    pub pads: [u32; 4],
}
impl LocalSlots {
    pub fn load<M: GuestMemory>(m: &M) -> Self {
        Self {
            setup_count: m.read32(0x5236c4),
            pads: core::array::from_fn(|i| m.read32(0x6b93c0 + i as u32 * 4)),
        }
    }
    // portreg: va=0x000426b0 tier=1
    // portreg: replaces=tsfp-ports/pose
    pub fn find(&self, pad: u32) -> Option<usize> {
        self.pads.iter().position(|&p| p == pad)
    }
    // portreg: va=0x000426d0 tier=1
    // portreg: replaces=tsfp-ports/pose_leaf
    pub fn pad(&self, slot: usize) -> Option<u32> {
        self.pads.get(slot).copied()
    }
    // portreg: va=0x00042d00 tier=1
    // portreg: replaces=tsfp-ports/c2r/leaf3_wired
    pub fn assign(&mut self, slot: usize, pad: u32) -> bool {
        if let Some(p) = self.pads.get_mut(slot) {
            *p = pad;
            true
        } else {
            false
        }
    }
    pub fn store<M: GuestMemory>(&self, m: &mut M) {
        m.write32(0x5236c4, self.setup_count);
        for (i, &pad) in self.pads.iter().enumerate() {
            m.write32(0x6b93c0 + i as u32 * 4, pad);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub rect: Rect,
    pub camera_fov: Option<u32>,
}
#[derive(Clone, Copy, Debug)]
pub struct LayoutRequest {
    pub width: i32,
    pub height: i32,
    pub count: u32,
    pub slot: u32,
    pub half_width_single: bool,
}
/// Extension point: mixed input sources can request layouts without changing pad mappings.
pub trait ViewportPolicy {
    fn layout(&self, request: LayoutRequest) -> Option<Layout>;
}
pub struct Original;
impl ViewportPolicy for Original {
    // portreg: va=0x0025a570 tier=1
    fn layout(&self, s: LayoutRequest) -> Option<Layout> {
        let (w, h) = (s.width, s.height);
        let (hw, hh) = (w >> 1, h >> 1);
        let mut r = Rect {
            left: 0,
            top: 0,
            right: w.wrapping_sub(1),
            bottom: h.wrapping_sub(1),
        };
        let mut fov = None;
        match s.count {
            1 => {
                if s.half_width_single {
                    r.right = hw.wrapping_sub(1);
                }
            }
            2 => match s.slot {
                0 => r.bottom = hh.wrapping_sub(1),
                1 => r.top = hh,
                _ => return None,
            },
            3 => {
                fov = Some(if s.slot == 0 { 45f32 } else { 55f32 }.to_bits());
                match s.slot {
                    0 => r.bottom = hh.wrapping_sub(1),
                    1 => {
                        r.top = hh;
                        r.right = hw.wrapping_sub(1);
                    }
                    2 => {
                        r.top = hh;
                        r.left = hw;
                    }
                    _ => return None,
                }
            }
            4 => {
                if s.slot & 1 != 0 {
                    r.left = hw;
                } else {
                    r.right = hw.wrapping_sub(1);
                }
                if s.slot & 2 != 0 {
                    r.top = hh;
                } else {
                    r.bottom = hh.wrapping_sub(1);
                }
            }
            _ => return None,
        }
        Some(Layout {
            rect: r,
            camera_fov: fov,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlayerBinding {
    pub pending_handle: u32,
    pub flags: u32,
    /// None preserves the original null-camera branch.
    pub camera_flags: Option<u32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PairState {
    pub mode: u8,
    pub active_count: usize,
    pub players: [PlayerBinding; 4],
    pub selected_mask: u32,
}
pub trait PairEffects {
    fn handle_valid(&mut self, player: usize, handle: u32) -> bool;
    fn attach(&mut self, player: usize, handle: u32);
    fn full_screen(&mut self, selected: u32, rect: Rect);
}
impl PairState {
    // portreg: va=0x000cb110 tier=1
    // portreg: replaces=tsfp-ports/c2r/gameplay_player
    pub fn select<E: PairEffects>(
        &mut self,
        selected: u32,
        width: i32,
        height: i32,
        effects: &mut E,
    ) {
        assert!(self.active_count <= 4, "local player capacity exceeded");
        for i in 0..self.active_count {
            let handle = self.players[i].pending_handle;
            if handle != 0 {
                if effects.handle_valid(i, handle) {
                    effects.attach(i, handle);
                } else {
                    self.players[i].pending_handle = 0;
                }
            }
        }
        if self.mode != 0xa || self.active_count <= 1 {
            return;
        }
        effects.full_screen(
            selected,
            Rect {
                left: 0,
                top: 0,
                right: width.wrapping_sub(1),
                bottom: height.wrapping_sub(1),
            },
        );
        let player = if selected == 0 { 1 } else { 0 };
        if let Some(flags) = &mut self.players[player].camera_flags {
            *flags &= !1;
            self.players[player].flags |= 1 << player;
        }
        self.selected_mask = 1u32 << (selected & 31);
    }
}

/// Neighbours of 0x25A570. The renderer owns record storage; networking owns remapping.
/// Read shaping after remapping and layout count after shaping, matching original call order.
pub trait ViewportEffects {
    fn remap_slot(&mut self, logical: u32) -> u32;
    fn edge_shift(&mut self) -> f32;
    fn shape(&mut self, horizontal: f32, vertical: f32, depth: f32);
    fn layout_count(&mut self) -> u32;
    fn publish(&mut self, rect: Rect);
    fn camera_fov(&mut self, remapped: u32, bits: u32);
}

/// Original viewport setup orchestration. Dimensions and single-player special mode are
/// typed snapshots; effects may refresh their own network/render state at each call edge.
pub fn setup_viewport<E: ViewportEffects>(mut request: LayoutRequest, effects: &mut E) {
    let remapped = effects.remap_slot(request.slot);
    let shift = effects.edge_shift();
    effects.shape(shift, 0.0 - shift, 256.0);
    request.count = effects.layout_count();
    if let Some(layout) = Original.layout(request) {
        effects.publish(layout.rect);
        if let Some(fov) = layout.camera_fov {
            effects.camera_fov(remapped, fov);
        }
    }
}
