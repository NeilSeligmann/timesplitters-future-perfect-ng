// SPDX-License-Identifier: GPL-3.0-or-later
//! INFERRED address boundary; all values remain in the shared GuestMemory image.
/// Shared editor slots; addresses are not detached native state or measured layouts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorGlobals {
    pub editor_context: u32,
    pub overlay_alpha: u32,
    pub selection: u32,
    pub storage_state: u32,
    pub view_state: u32,
    pub object_state: u32,
}
pub const GLOBALS: EditorGlobals = EditorGlobals {
    editor_context: 0x7844a8,
    overlay_alpha: 0x7844ac,
    selection: 0x778d34,
    storage_state: 0x78400c,
    view_state: 0x783dd8,
    object_state: 0x7ba9e0,
};
