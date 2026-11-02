// SPDX-License-Identifier: GPL-3.0-or-later
// portreg: replaces=tsfp-ports/editor
//! T2432: Rust ports of the hottest small callees of the Map Maker per-frame update `0x306460`
//! (frontier expansion of T2429, rule 22). Each function replaces a guest call of
//! `run_editor_update` when `TSFP_MAPMAKER_OVERLAY=rust`.
//!
//! Ported (original address, role):
//! - `0x62490` [`set_cursor_mode_global`]: stores its argument in the global `[0x4D19F0]`.
//! - `0x2F92E0` [`clear_link_marks`]: clears bit 1 of the flag byte of every node of the editor
//!   object list and resets the selection count.
//! - `0x2FEB50` [`count_selected_objects`]: promotes bit 3 to bit 1 on every node, clears bits 3
//!   and 4, then stores the number of nodes with bit 1 set.
//! - `0x2FC050` [`update_hint_for_active_view`]: hint refresh gate, tail calls `0x3159C0`.
//! - `0x2FBF70` [`idle_allows_cursor_action`]: whether a new cursor action may start.
//!
//! Evidence: Tier 1 differential against the lifted `editor_reference` bodies
//! (`tests/mapmaker_callees_tier1.rs`, fabricated seeds, weak) plus the T2429 whole-function
//! differential. Names are INFERRED. The remaining callees stay guest calls (TEMPORARY-SHIM),
//! see `docs/t2427-mapmaker-306460-map.md`.
use super::{CONTEXT_POINTER, EditorContext, PAGE, VIEW_INDEX, VIEW_STRIDE, VIEW_WINDOW, frame};
use crate::memory::GuestMemory;

/// Global written by `0x62490`.
pub const CURSOR_MODE_GLOBAL: u32 = 0x004d_19f0;
/// Head pointer of the editor object list; nodes link through [`NODE_NEXT`].
pub const OBJECT_LIST_HEAD: u32 = 0x0078_400c;
const NODE_FLAGS: u32 = 0x14;
const NODE_NEXT: u32 = 0x40;
/// Node flag bits.
mod node {
    /// Counted as selected by `0x2FEB50`.
    pub const SELECTED: u8 = 0x02;
    /// Becomes [`SELECTED`] on the next count.
    pub const PENDING: u8 = 0x08;
    /// Both transient bits cleared by the count (`0x08 | 0x10`).
    pub const TRANSIENT: u8 = 0x18;
}
/// Leaf widget flag byte `+0x218`: bit 2 blocks a new cursor action.
const LEAF_BLOCKS_ACTION: u8 = 0x04;
const LEAF_STATE: u32 = 0x218;
/// Cursor flag `0x20` / `0x80` of the frame flag low byte.
const FRAME_SIGN_BYTE: u32 = frame::SIGN;
/// Frame flags that stop the hint refresh (`0x4044088`).
const HINT_BLOCK_MASK: u32 = 0x0404_4088;
/// Tool sub-state that runs the nav helper after the hint refresh.
const SUBSTATE_NAV: u8 = 1;

/// Guest calls the ported callees still make (the callees' own callees, TEMPORARY-SHIM).
pub trait CalleeHost {
    type Memory: GuestMemory;
    /// The guest memory the callee reads and writes.
    fn memory(&mut self) -> &mut Self::Memory;
    /// cdecl guest call returning EAX.
    fn guest_call(&mut self, va: u32, args: &[u32]) -> u32;
}

/// `0x62490(value)`: store the cursor mode global. Returns the value (EAX).
pub fn set_cursor_mode_global<M: GuestMemory>(m: &mut M, value: u32) -> u32 {
    m.write32(CURSOR_MODE_GLOBAL, value);
    value
}

/// Iterator over the node addresses of the editor object list, in list order.
fn object_nodes<M: GuestMemory>(m: &M) -> impl Iterator<Item = u32> + '_ {
    let mut node = m.read32(OBJECT_LIST_HEAD);
    core::iter::from_fn(move || {
        if node == 0 {
            return None;
        }
        let current = node;
        node = m.read32(current.wrapping_add(NODE_NEXT));
        Some(current)
    })
}

fn context<M: GuestMemory>(m: &M) -> EditorContext {
    EditorContext {
        base: m.read32(CONTEXT_POINTER),
    }
}

/// `0x2F92E0`: clear bit 1 of every node flag byte and reset the selection count.
pub fn clear_link_marks<M: GuestMemory>(m: &mut M) {
    let mut node_address = m.read32(OBJECT_LIST_HEAD);
    while node_address != 0 {
        let address = node_address.wrapping_add(NODE_FLAGS);
        let flags = m.read8(address);
        m.write8(address, flags & !node::SELECTED);
        node_address = m.read32(node_address.wrapping_add(NODE_NEXT));
    }
    context(m).set_drag_source(m, 0);
}

/// `0x2FEB50`: nodes marked pending become selected, the transient bits are cleared, then the
/// selected nodes are counted into the context.
pub fn count_selected_objects<M: GuestMemory>(m: &mut M) {
    let mut node_address = m.read32(OBJECT_LIST_HEAD);
    while node_address != 0 {
        let address = node_address.wrapping_add(NODE_FLAGS);
        let mut flags = m.read8(address);
        if flags & node::PENDING != 0 {
            flags |= node::SELECTED;
            m.write8(address, flags);
        }
        let flags = m.read8(address);
        m.write8(address, flags & !node::TRANSIENT);
        node_address = m.read32(node_address.wrapping_add(NODE_NEXT));
    }
    let selected = object_nodes(m)
        .filter(|node| m.read8(node.wrapping_add(NODE_FLAGS)) & node::SELECTED != 0)
        .count() as u32;
    context(m).set_drag_source(m, selected);
}

/// The widget under the active pointer view (guest `0x77470`).
fn view_window<H: CalleeHost>(host: &mut H) -> u32 {
    let m = host.memory();
    let offset = m.read32(VIEW_INDEX).wrapping_mul(VIEW_STRIDE);
    let root = m.read32(VIEW_WINDOW.wrapping_add(offset));
    host.guest_call(0x77470, &[root])
}

/// `0x2FC050`: refresh the leaf widget hint unless a blocking frame flag is set or the pointer is
/// over the editor page; runs the nav helper `0x3159C0` when the tool sub-state is 1.
pub fn update_hint_for_active_view<H: CalleeHost>(host: &mut H) {
    let ctx = context(host.memory());
    let blocked = ctx.frame_flags(host.memory()) & HINT_BLOCK_MASK != 0;
    if !blocked && view_window(host) == host.memory().read32(PAGE) {
        return;
    }
    let m = host.memory();
    if ctx.tool_substate_byte(m) as i8 == SUBSTATE_NAV as i8 {
        host.guest_call(0x3159c0, &[]);
    }
}

/// `0x2FBF70`: true when the editor is idle enough to start a new cursor action.
pub fn idle_allows_cursor_action<H: CalleeHost>(host: &mut H) -> bool {
    let ctx = context(host.memory());
    if ctx.frame_flags(host.memory()) & frame::BAIL_MASK != 0 {
        return false;
    }
    if host.guest_call(0x774b0, &[]) != 0 {
        let leaf = host.guest_call(0x774b0, &[]);
        if host.memory().read8(leaf.wrapping_add(LEAF_STATE)) & LEAF_BLOCKS_ACTION != 0 {
            return false;
        }
    }
    // Bit 7 of the flag byte skips the pointer-over-page check.
    let skip_window_check = ctx.frame_flags(host.memory()) as u8 & FRAME_SIGN_BYTE as u8 != 0;
    if !skip_window_check && view_window(host) != host.memory().read32(PAGE) {
        return false;
    }
    if ctx.frame_flags(host.memory()) as u8 & frame::SHIFTED as u8 != 0 {
        return false;
    }
    if host.guest_call(0x314d60, &[]) != 0 {
        return false;
    }
    let m = host.memory();
    ctx.pick_kind(m) == u32::MAX
        && ctx.picked_cell(m) == -1
        && ctx.hover_record(m) == 0
        && ctx.tool_substate(m) == u32::MAX
}
