// SPDX-License-Identifier: GPL-3.0-or-later
//! Guest addresses of the render state the T1898 slice reads and writes. All are fixed
//! addresses of the retail image. Roles are INFERRED from the lifted code unless a comment
//! says MEASURED (an observed value or an XDK-documented address).

// ---- state slot table and dirty list (written by HUD, scene, effects) ----
/// 35 records of [`SLOT_STRIDE`] bytes (see [`crate::state::StateSlot`]).
pub const SLOT_TABLE: u32 = 0x004B_8D28;
pub const SLOT_STRIDE: u32 = 0x18;
/// Sentinel of the circular dirty list. `+0x10` (= 0x563024) holds the newest dirty slot,
/// `+0x14` (= 0x563028) the oldest.
pub const DIRTY_SENTINEL: u32 = 0x0056_3014;
pub const DIRTY_NEWEST: u32 = 0x0056_3024;
pub const DIRTY_OLDEST: u32 = 0x0056_3028;

// ---- blend and state group (the HUD's 0x19650 / 0x19D70 / 0x19D40 selectors) ----
pub const STATE_GROUP: u32 = 0x0055_FAF8;
pub const BLEND_MODE: u32 = 0x0055_FAFC;
/// Last alpha written into slot 0x15 by the alpha-keyed blend modes.
pub const BLEND_COLOUR_CACHE: u32 = 0x0055_FB00;
pub const BLEND_ALPHA: u32 = 0x0055_FB04;
/// Saved copies for the temporary override (0x8F470 save, 0x8F490 restore).
pub const SAVED_STATE_GROUP: u32 = 0x004D_9708;
pub const SAVED_BLEND_MODE: u32 = 0x004D_970C;
pub const SAVED_BLEND_ALPHA: u32 = 0x004D_9710;
pub const RESTORE_GROUP_PENDING: u32 = 0x0070_AAEC;
pub const RESTORE_BLEND_PENDING: u32 = 0x0070_AAE8;

// ---- D3D8 internal state the game code touches directly (MEASURED, docs/d3d8-usage.md 9.1) ----
pub const METHOD_HEADER_TABLE: u32 = 0x0047_5B08;
pub const DIRTY_BIT_TABLE_BIASED: u32 = 0x0047_58E8;
pub const DEVICE_DIRTY_MASK: u32 = 0x003E_3AB8;
pub const DEVICE_STATE_SHADOW: u32 = 0x003E_3CC0;
pub const STAGE_WORDS: u32 = 0x003E_3AC0;

// ---- effect row, draw batch and constant caches ----
/// Pointer to the active 16 byte effect row (0x476270 + index * 16), 0 when none.
pub const EFFECT_ROW_POINTER: u32 = 0x0066_5470;
pub const EFFECT_ROW_INDEX: u32 = 0x004B_917C;
pub const EFFECT_ROW_TABLE: u32 = 0x0047_6270;
pub const TEXTURE_CACHE: u32 = 0x0056_3944;
pub const TEXTURE_FILTER_CACHE: u32 = 0x004B_907C;
pub const TEXTURE_FILTER_CACHE_B: u32 = 0x004B_908C;

/// Texture index (or -1) the effect code last selected (0x8F4E0).
pub const EFFECT_TEXTURE_INDEX: u32 = 0x0066_52E0;
/// Texture table: `[+0]` records (0x34 bytes each), `[+8]` first valid index, `[+0xC]` count.
pub const TEXTURE_TABLE_POINTER: u32 = 0x004B_85C8;
pub const TEXTURE_PAIR_RECORDS: u32 = 0x0056_3120;
pub const TEXTURE_PAIR_TAIL: u32 = 0x0056_3154;
pub const TEXTURE_PAIR_A: u32 = 0x0056_311C;
pub const TEXTURE_PAIR_B: u32 = 0x0056_3158;
pub const TEXTURE_OVERRIDE_INDEX: u32 = 0x004E_0D6C;
pub const CURRENT_TEXTURE_INDEX: u32 = 0x0055_FAC0;
pub const CURRENT_TEXTURE_RECORD: u32 = 0x0055_FB7C;

// ---- pending draw batch (indexed ring, vertex run, constants) ----
pub const INDEX_FIRST: u32 = 0x0055_F96C;
pub const INDEX_LAST: u32 = 0x0055_F970;
pub const INDEX_FLAG: u32 = 0x0055_F978;
pub const INDEX_ARRAY: u32 = 0x0054_F968;
pub const VERTEX_RUN_START: u32 = 0x004B_85D0;
pub const VERTEX_RUN_END: u32 = 0x004B_85D4;
pub const BATCH_PRIMITIVE: u32 = 0x004B_85D8;
pub const BATCH_FORMAT: u32 = 0x004B_85DC;
pub const BATCH_STRIDE: u32 = 0x004B_85E0;
pub const BATCH_EXTRA: u32 = 0x004B_85E4;
pub const VERTEX_CURSOR: u32 = 0x0055_F974;
pub const VERTEX_RING_INDEX: u32 = 0x0055_F988;
pub const VERTEX_RING_BASE: u32 = 0x0054_F128;
pub const VERTEX_RING_STRIDE: u32 = 20;
pub const VERTEX_BUFFER_BASE_SLOT: u32 = 0x0055_F98C;
pub const MATRIX_DIRTY_ANY: u32 = 0x004B_9074;
pub const MATRIX_DIRTY_COUNT: u32 = 0x0056_3914;
pub const MATRIX_DIRTY_FLAGS: u32 = 0x0056_3034;
pub const MATRIX_STORE: u32 = 0x0056_3188;
pub const CONSTANT_BLOCK_DIRTY: u32 = 0x0056_3910;
pub const CONSTANT_BLOCK: u32 = 0x0056_3078;
pub const SCISSOR_TARGET_WIDTH: u32 = 0x0055_FAD4;
pub const SCISSOR_TARGET_HEIGHT: u32 = 0x0055_FAD8;
pub const SCISSOR_CACHE_LEFT: u32 = 0x0055_FADC;
pub const SCISSOR_CACHE_TOP: u32 = 0x0055_FAE0;
pub const SCISSOR_CACHE_RIGHT: u32 = 0x0055_FAE4;
pub const SCISSOR_CACHE_BOTTOM: u32 = 0x0055_FAE8;
pub const VERTEX_PROGRAM_CACHE: u32 = 0x0056_5164;
pub const PIXEL_PROGRAM_CACHE: u32 = 0x0056_5168;
pub const DECLARATION_BLOCK: u32 = 0x0055_FB90;
pub const DECLARATION_VALID: u32 = 0x0056_08C8;
pub const STREAM_TABLE: u32 = 0x0055_F9BC;
/// Draw statistics: vertices, triangles, batches (INFERRED).
pub const STAT_VERTICES: u32 = 0x006B_7AB0;
pub const STAT_TRIANGLES: u32 = 0x006B_7AB4;
pub const STAT_BATCHES: u32 = 0x006B_7AB8;
pub const BATCH_OPEN: u32 = 0x0055_FB6C;
pub const BATCH_POINT_MODE: u32 = 0x0055_FB20;
pub const CONST_ONE: u32 = 0x0047_5C78;

// ---- effect pass and draw prepare caches (0x22BE0, 0x2A2D0) ----
pub const EFFECT_PASS: u32 = 0x004B_9180;
pub const PREPARE_STREAM_BASE: u32 = 0x004B_90A8;
pub const PREPARE_STREAM_STRIDE: u32 = 0x004B_90A4;
pub const PREPARE_DECLARATION_FORMAT: u32 = 0x004B_90A0;
pub const PREPARE_EXTRA_FLAGS: u32 = 0x004B_909C;
/// Four texture records bound by the draw prepare step, slot 0 is the current record.
pub const BOUND_TEXTURES: u32 = 0x0055_FB7C;
pub const BATCH_CLEANUP_A: u32 = 0x0055_FB64;
pub const BATCH_CLEANUP_B: u32 = 0x0055_FB68;
pub const PIXEL_BYTE_SCALE: u32 = 0x0047_5DCC;

// ---- HUD colour state (0x19CC0) ----
pub const MODULATE_HIGH: u32 = 0x0055_FB08;
pub const MODULATE_LOW: u32 = 0x0055_FB0C;
pub const MODULATE_COLOUR: u32 = 0x0055_FB10;
