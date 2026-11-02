// SPDX-License-Identifier: GPL-3.0-or-later
//! Guest addresses and record offsets the HUD uses. Each is MEASURED from the original
//! instructions (docs/t-hud.md, T1710/T1711) unless the comment says INFERRED.

/// Per-player HUD block base (0x7A6160), stride 0x177C, four players.
pub const HUD_BLOCK_BASE: u32 = 0x007a_6160;
pub const HUD_BLOCK_STRIDE: u32 = 0x177c;
pub const HUD_BLOCK_PLAYERS: i32 = 4;

/// Block `+0x30`: next message insertion cursor (wraps at 16).
pub const BLOCK_RING_CURSOR: u32 = 0x30;
/// Block `+0x20`: health-flash timer, counted in frames (see `health_slot_update`).
pub const BLOCK_TIMER_20: u32 = 0x20;
/// Block `+0x38`: first of 16 message entries of 0x16C bytes.
pub const BLOCK_RING_ENTRIES: u32 = 0x38;
pub const RING_LEN: u32 = 16;
pub const RING_ENTRY_SIZE: u32 = 0x16c;
/// Message entry fields.
pub const MSG_KIND: u32 = 0;
pub const MSG_TEXT: u32 = 4;
pub const MSG_TEXT_BYTES: u32 = 0x140;
pub const MSG_AGE: u32 = 0x144;
pub const MSG_FADE_START: u32 = 0x148;
pub const MSG_END: u32 = 0x150;
pub const MSG_EVICT_AFTER: u32 = 0x154;
pub const MSG_WEIGHT: u32 = 0x158;
pub const MSG_OPAQUE: u32 = 0x168;

/// Letterbox state (0x7490E8..0x7490F4).
pub const LETTERBOX_NO_DRAW: u32 = 0x0074_90e8;
pub const LETTERBOX_AMOUNT: u32 = 0x0074_90ec;
pub const LETTERBOX_STATE: u32 = 0x0074_90f0;
pub const LETTERBOX_HOLD: u32 = 0x0074_90f4;

/// Frame timing globals.
pub const FRAME_DELTA_INT: u32 = 0x004e_7944;
pub const FRAME_DELTA_F32: u32 = 0x004e_7948;
pub const FRAME_STEP_F32: u32 = 0x004e_794c;
pub const FRAME_RATE_INT: u32 = 0x004e_8cec;
pub const SCREEN_WIDTH_INT: u32 = 0x004e_8cf0;
pub const SCREEN_HEIGHT_INT: u32 = 0x004e_8cf4;

/// Read-only float constants in the XBE data (their values are read from guest memory, never
/// copied here, so a different build or mod keeps working).
pub const CONST_ONE: u32 = 0x0047_5c78;
pub const CONST_255: u32 = 0x0047_5cc0;
pub const CONST_LETTERBOX_RATE: u32 = 0x0047_5cc8;
pub const CONST_HALF: u32 = 0x0047_5cd4;
pub const CONST_LETTERBOX_MIN: u32 = 0x0047_5cac;
pub const CONST_MSG_END: u32 = 0x0047_5d00;
pub const CONST_FADE: u32 = 0x0047_5d10;
pub const CONST_FLASH: u32 = 0x0047_85e0;
pub const CONST_BAR_SCALE: u32 = 0x0047_d1f8;

/// Health threshold tables: 20 floats at stride 0x14 (fractions of the maximum).
pub const HEALTH_THRESHOLDS: u32 = 0x004e_6d80;
pub const ARMOR_THRESHOLDS: u32 = 0x004e_6f28;
pub const THRESHOLD_COUNT: u32 = 20;
pub const THRESHOLD_STRIDE: u32 = 0x14;

/// Health-flag block (pointed to by object `+0x268`): offsets of its arrays.
pub const FLAGS_HEALTH: u32 = 0x00;
pub const FLAGS_ARMOR: u32 = 0x50;
pub const TIMERS_HEALTH: u32 = 0xa0;
pub const TIMERS_ARMOR: u32 = 0xf0;
pub const FLAGS_FLASH: u32 = 0x140;

/// Object (TsfpObject) and character (TsfpCharacter) fields, tools/data/struct_layouts.json.
pub const OBJ_CLASS: u32 = 0x20;
pub const OBJ_CHARACTER: u32 = 0x7c;
pub const OBJ_HEALTH: u32 = 0x1cc;
pub const OBJ_MAX_HEALTH: u32 = 0x1d8;
pub const OBJ_HUD_FLAGS: u32 = 0x268;
pub const CLASS_CHARACTER: u32 = 8;
pub const CHAR_ARMOR: u32 = 0x78;
pub const CHAR_ARMOR_MAX: u32 = 0x7c;

/// Pointers and records.
pub const CURRENT_PLAYER_PTR: u32 = 0x007b_0c48;
pub const PLAYER_RECORD_SIZE: u32 = 0x1584;
pub const PLAYER_OBJECT: u32 = 0x14;
pub const EXT_ARRAY_PTR: u32 = 0x0073_56d8;
pub const EXT_SIZE: u32 = 0xea0;
pub const VIEWPORT_PTR: u32 = 0x007e_71c8;
pub const MODE_WORD: u32 = 0x0079_094c;
pub const LOCAL_PLAYER_COUNT: u32 = 0x0079_0950;
pub const MODE_BYTE: u32 = 0x007d_e455;
pub const MODE_FLAGS: u32 = 0x007d_e458;
pub const WEAPON_RECORDS: u32 = 0x004e_ef74;
pub const WEAPON_RECORD_SIZE: u32 = 0x29c;
pub const WEAPON_RECORD_COUNT: u32 = 0x42;

/// Player extension block (inventory), docs/t-player-inventory.md.
pub const EXT_CURRENT_WEAPON_ENTRY: u32 = 0x94;
pub const EXT_HAND_B: u32 = 0x1d4;
pub const HAND_RECORD_INDEX: u32 = 0x30;
pub const HAND_LOADED_BY_TYPE: u32 = 0x38;
pub const EXT_OWNED_BY_ENTRY: u32 = 0x54c;
pub const EXT_RESERVE_BY_TYPE: u32 = 0x61e;

/// Weapon record fields.
pub const WEAPON_AMMO_TYPE_1: u32 = 0x14;
pub const WEAPON_CLIP_SIZE_1: u32 = 0x18;

/// Renderer entry points the HUD calls (original addresses of the `Draw` methods).
pub const FN_STATE_GROUP: u32 = 0x19650;
pub const FN_BLEND_MODE: u32 = 0x19d70;
pub const FN_COLOUR_PRESET: u32 = 0x1ed60;
pub const FN_FILL_RECT: u32 = 0x1efb0;
pub const FN_TEXTURED_RECT: u32 = 0x1b7b0;
pub const FN_TEXT: u32 = 0x42490;
pub const FN_RECT_LINE: u32 = 0x1b090;
pub const FN_CORNER_FRAME: u32 = 0x2d76b0;
pub const FN_LOADBAR: u32 = 0x2d7f90;

/// Widget (ui/framework) fields read by the frame drawers: bounds floats, style, alpha.
pub const WIDGET_STYLE: u32 = 0x20;
pub const WIDGET_SIZE_B4: u32 = 0xb4;
pub const WIDGET_X: u32 = 0xe4;
pub const WIDGET_Y: u32 = 0xe8;
pub const WIDGET_RIGHT: u32 = 0xec;
pub const WIDGET_BOTTOM: u32 = 0xf0;
pub const WIDGET_ALPHA: u32 = 0xf4;
pub const STYLE_COLOUR_A: u32 = 0x3c;
pub const STYLE_COLOUR_B: u32 = 0x40;
pub const CONST_CORNER_LIMIT: u32 = 0x0047_d1e0;
pub const CONST_LOADBAR_LIMIT: u32 = 0x0049_f648;
pub const CONST_DEFAULT_THICKNESS: u32 = 0x0047_5d08;
