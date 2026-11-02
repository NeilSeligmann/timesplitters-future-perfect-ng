//! T2312 increment 6: the button prompt panel (weapon help) as native Rust (`0x1476F0`, called by the HUD
//! frame `0x14F0B0` with the player index).
//!
//! The original (MEASURED from the XBE disassembly, docs/t2312-native-hud.md, "Increment 6"):
//!
//! * Four panel records `0x7A5940 + i * 0x114` (one per player): eight lines of 0x1C bytes (`+0` string id,
//!   `+4` font, `+8` outline flag, `+0xC` / `+0x10` binding actions, `+0x18` kind), `+0xE0` line count,
//!   `+0xE4` x, `+0xE8` y, `+0xEC` width (set by the layout pass `0x147330` at show time), `+0xF0` height
//!   (written by the drawer), `+0xF4` active, `+0xF8` phase, `+0xFC` alpha, `+0x100` timer, `+0x104` distance,
//!   `+0x108` the player's position at show time. Show `0x147180`, line setter `0x147200`, dismiss
//!   `0x147300` (phase 3), level init `0x147250`.
//! * The drawer: fade = min(record alpha, 1 - HUD fade), the frosted panel `0x2D7F90` (two black rounded
//!   rectangles of radius 20 around the text, a screen-copy blur pass between them), then per line the
//!   text built from string ids: kind 0 the string, kinds 1 to 3 the string with one to three button
//!   labels (`0x147080` binding, `0x12C9C0` label, `0x3D7F0` `%N` format), kind -1 a button glyph
//!   (`0x42D10`) at the left and the wrapped string 40 pixels right of it. Text is wrapped to the panel
//!   width (`0x40C10`), drawn plain (`0x41DC0`) or with the four-pass outline (`0x41E30`); `\x0B` escapes
//!   draw button icons inline (`0x40FA0`). Then the phase machine: fade in, hold 6 s (moving the player
//!   ends it sooner), linger up to 8 s, fade out.
//!
//! [`PromptPanels::read`] replays it on the live state and returns the panel, texts and icons. The guest
//! is never written: the lifecycle (phase, alpha, timer, distance, height) is Rust-owned ([`PanelState`]);
//! show, dismiss and level init are observed through pass-through gates. Text metrics come from the
//! original font tables ([`FontSet`]) with the text system's scales read from the guest, as `0x3F8D0`
//! computes them. Cases not decoded (alternative font table, style escapes,
//! unknown kinds) return [`PromptCall::Legacy`] and the original draws that frame.
use crate::font::{FontObject, FontSet};
use crate::hud_native::GuestRead;

/// Guest addresses of the prompt panel (MEASURED from the disassembly of `0x1476F0` and its callees).
pub mod prompt_addr {
    /// The drawer (cdecl, `[esp+4]` the player index).
    pub const FN_PROMPTS: u32 = 0x0014_76f0;
    /// Show (`record, x, y, width, player`), dismiss (`index`), level init (all four records).
    pub const FN_SHOW: u32 = 0x0014_7180;
    pub const FN_DISMISS: u32 = 0x0014_7300;
    pub const FN_INIT: u32 = 0x0014_7250;
    /// Panel records.
    pub const RECORDS: u32 = 0x007a_5940;
    pub const RECORD_SIZE: u32 = 0x114;
    pub const RECORD_COUNT: u32 = 4;
    pub const LINE_SIZE: u32 = 0x1c;
    pub const LINES: u32 = 8;
    pub const COUNT: u32 = 0xe0;
    pub const X: u32 = 0xe4;
    pub const Y: u32 = 0xe8;
    pub const WIDTH: u32 = 0xec;
    pub const HEIGHT: u32 = 0xf0;
    pub const ACTIVE: u32 = 0xf4;
    pub const PHASE: u32 = 0xf8;
    pub const ALPHA: u32 = 0xfc;
    pub const TIMER: u32 = 0x100;
    pub const DISTANCE: u32 = 0x104;
    pub const POSITION: u32 = 0x108;
    /// Viewer (`+4` slot) and the HUD records (`+0x1750` fade flag, `+0x1758` fade).
    pub const VIEWER: u32 = 0x007b_0c7c;
    pub const HUD_RECORDS: u32 = 0x007a_6160;
    pub const HUD_RECORD_SIZE: u32 = 0x177c;
    pub const HUD_FADE_FLAG: u32 = 0x1750;
    pub const HUD_FADE: u32 = 0x1758;
    /// Player records `*0x7B0C48 + i * 0x1584`: position `+0xC0`, `+0x16C` (added doubled to the distance);
    /// bindings at `+0x18 + action * 4`.
    pub const PLAYERS_PTR: u32 = 0x007b_0c48;
    pub const PLAYER_SIZE: u32 = 0x1584;
    pub const PLAYER_POSITION: u32 = 0xc0;
    pub const PLAYER_EXTRA: u32 = 0x16c;
    pub const PLAYER_BINDINGS: u32 = 0x18;
    /// Frame rate (int), frame time scale (f32), mode flags (bit 6 shortens the hold).
    pub const FRAME_RATE: u32 = 0x004e_8cec;
    pub const TIME_SCALE: u32 = 0x004e_7948;
    pub const MODE_FLAGS: u32 = 0x007d_e458;
    /// Language (0 English, 2 the "- " trigger format), string tables (`0x3D7B0`).
    pub const LANGUAGE: u32 = 0x004c_0454;
    pub const LEVEL_STRINGS: u32 = 0x006b_7b08;
    pub const GLOBAL_STRINGS: u32 = 0x006b_7b10;
    pub const MISSING_STRING: u32 = 0x004c_0a44;
    pub const LEVEL_STRINGS_FIRST: u32 = 0xd87;
    /// `0x42D10`: code byte map for codes 1..=0x40 and the glyph strings.
    pub const GLYPH_CODE_MAP: u32 = 0x0004_2f44;
    pub const GLYPH_STRINGS: [u32; 7] = [
        0x0047_83d4,
        0x0047_83d0,
        0x0047_83cc,
        0x0047_83c8,
        0x0047_8230,
        0x0047_822c,
        0x0047_8234,
    ];
    pub const GLYPH_80: u32 = 0x0047_8220;
    pub const GLYPH_100: u32 = 0x0047_83bc;
    pub const GLYPH_200: u32 = 0x0047_83c0;
    pub const GLYPH_4000: u32 = 0x0047_83b0;
    pub const GLYPH_8000: u32 = 0x0047_83b4;
    pub const GLYPH_NONE: u32 = 0x0047_83b8;
    /// Codes drawn as bank strings by `0x42D10`.
    pub const GLYPH_IDS: [(u32, u32); 4] = [
        (0x400, 0x195f),
        (0x800, 0x1960),
        (0x1000, 0x1961),
        (0x2000, 0x1962),
    ];
    /// Trigger labels (`0x12C9C0`): strings `0x160C` (code 0x1000) and `0x160D` (0x2000), their icons, formats.
    pub const TRIGGER_LEFT_ID: u32 = 0x160c;
    pub const TRIGGER_RIGHT_ID: u32 = 0x160d;
    pub const TRIGGER_LEFT_ICON: u32 = 0x0047_da34;
    pub const TRIGGER_RIGHT_ICON: u32 = 0x0047_da30;
    pub const FMT_TRIGGER_EN: u32 = 0x0047_da1c;
    pub const FMT_TRIGGER_2: u32 = 0x0047_da10;
    pub const FMT_TRIGGER_OTHER: u32 = 0x0047_da04;
    /// Fixed binding codes of actions 0xF.. (`0x147080`).
    pub const FIXED_CODES: [u32; 14] = [
        1, 2, 4, 8, 0x10, 0x20, 0x40, 0x80, 0x8000, 0x400, 0x4000, 0x800, 0xc, 0,
    ];
    /// Text system globals (`0x3F8D0`, `0x40FA0`, `0x40C10`).
    pub const FONT_OBJECTS: u32 = 0x004c_8de8;
    pub const FONT_RECORDS: u32 = 0x004c_8c28;
    pub const ALT_TABLE: u32 = 0x006b_93a8;
    pub const SCALE_ONE: u32 = 0x004c_0460;
    pub const TEXT_SCALE_X: u32 = 0x007e_7270;
    pub const TEXT_SCALE_TOP: u32 = 0x007e_7274;
    pub const TEXT_SCALE_Y: u32 = 0x007e_7280;
    pub const HEIGHT_UNITS: u32 = 0x004e_8cf4;
    pub const HEIGHT_FACTOR: u32 = 0x0047_8390;
    pub const JUSTIFY: u32 = 0x007e_7260;
    pub const WRAP_SPLIT: u32 = 0x007e_7278;
    pub const WRAP_KEEP_SPACES: u32 = 0x007e_7284;
    pub const FIXED_WIDTH: u32 = 0x007e_72d0;
    pub const TEXT_MODE: u32 = 0x0079_094c;
    pub const CLOCK: u32 = 0x0074_97b0;
    /// Inline icons: `\x0B` + 'A'..'T' (table of 0x1C bytes: texture id, clock shift, scale, per-language
    /// ids, base frame, width shift, frame stride) and `\x0B` + '1'..'9' (texture handles `0x7E72A0`).
    pub const ICON_TABLE: u32 = 0x004c_8248;
    pub const ICON_TABLE_SIZE: u32 = 0x1c;
    pub const DIGIT_HANDLES: u32 = 0x007e_72a0;
    /// Texture ids behind the digit handles (copied by `0x812F0` from the rows of these ids).
    pub const DIGIT_IDS: [u32; 9] = [
        0x29a, 0x29b, 0x29c, 0x29d, 0x29e, 0x29f, 0x2a0, 0x299, 0x2a2,
    ];
    /// Texture table: name at `0x4D2D5C + id * 24`, handle at `0x4D2D64 + id * 24`; records `*(*0x4B85C8) + handle * 0x34`.
    pub const TEXTURE_NAMES: u32 = 0x004d_2d5c;
    pub const TEXTURE_HANDLES: u32 = 0x004d_2d64;
    pub const TEXTURE_RECORDS_PTR: u32 = 0x004b_85c8;
    /// Panel textures of `0x2D7B70` (ids 0x10 and 0x11).
    pub const CORNER_ID: u32 = 0x10;
    pub const FILL_ID: u32 = 0x11;
}

/// Constants of the drawer (f32 values of the XBE data).
pub mod k {
    /// Panel margin around the text (x 10, y 5) and the rounded corner radius (`0x2D7F90` argument 0x14).
    pub const MARGIN_X: i32 = 10;
    pub const MARGIN_Y: i32 = 5;
    pub const RADIUS: i32 = 20;
    /// Panel alphas: `fade * 127` (`0x475CC0`) then `fade * 64` (`0x47D1E0`); text `fade * 100` (`0x4785C8`).
    pub const PANEL_ALPHA_1: f32 = 127.0;
    pub const PANEL_ALPHA_2: f32 = 64.0;
    pub const TEXT_ALPHA: f32 = 100.0;
    pub const TEXT_RGB: u32 = 0x6464_6400;
    /// Line gaps: plain 5 (`0x4760CC`), outlined 8 (`0x47D1E4`); the glyph column 40 (`0x4788B8`).
    pub const GAP_PLAIN: f32 = 5.0;
    pub const GAP_OUTLINED: f32 = 8.0;
    pub const GLYPH_COLUMN: f32 = 40.0;
    /// Outline offsets of `0x41E30` (x 2 `0x475D10`, y 1 `0x475C78`).
    pub const OUTLINE_DX: f32 = 2.0;
    pub const OUTLINE_DY: f32 = 1.0;
    /// Phase machine: fade-in delay 0.1 (`0x475D24`), fade-in rate 0.075 (`0x47D5F8`), hold 6 / 3 (`0x475D30`,
    /// `0x475D18`), linger 8 / 1 (`0x47D1E4`), distance limit 16 (`0x475DC0`), fade-out rate 0.02 (`0x4785D4`).
    pub const FADE_IN_DELAY: f32 = 0.1;
    pub const FADE_IN_RATE: f32 = 0.075;
    pub const HOLD: f32 = 6.0;
    pub const HOLD_SHORT: f32 = 3.0;
    pub const LINGER: f32 = 8.0;
    pub const LINGER_SHORT: f32 = 1.0;
    pub const DISTANCE_LIMIT: f32 = 16.0;
    pub const FADE_OUT_RATE: f32 = 0.02;
    /// Tab stops (`0x40FA0`: 1/60 `0x478394`, 60 `0x4760AC`); icon base size in text mode 0x66 (`0x4782C4`).
    pub const TAB: f32 = 60.0;
    pub const ICON_BASE_MODE_66: f32 = 20.0;
}

/// Texture names the panel and the inline icons use (resolved from the disc at run time; the guest names
/// are matched against this list, an unknown name leaves the frame to the original).
pub const PROMPT_TEXTURES: [&str; 23] = [
    "femenubordernewcornerfill",
    "femenubordernewfill",
    "xbstdbutton1new",
    "xbstdbutton2new",
    "xbstdbutton3new",
    "xbstdbutton4new",
    "xbstdbutton5",
    "xbstdbutton6",
    "xbstdbutton7",
    "xbnullchar",
    "xbdpad",
    "xb_dpad_leftright",
    "xb_dpad_updown",
    "xb_dpad_leftrightupdown",
    "xb_thumb_all",
    "xb_thumb_click",
    "xb_ltrigger",
    "xb_rtrigger",
    "xb_gtrigger",
    "xb_strigger",
    "xb_itrigger",
    "xb_dtrigger",
    "shadowcircle",
];
pub const CORNER_TEXTURE: &str = PROMPT_TEXTURES[0];
pub const FILL_TEXTURE: &str = PROMPT_TEXTURES[1];

/// x87/SSE truncation of the original's float-to-int conversions.
fn trunc(v: f32) -> i32 {
    v as i32
}

/// A NUL-terminated string of the running game (strings, formats, glyph strings), at most 1023 bytes.
pub fn guest_cstring<G: GuestRead + ?Sized>(g: &G, va: u32) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut at = va;
    while out.len() < 1023 {
        let word = g.read32(at & !3)?;
        let byte = (word >> ((at & 3) * 8)) as u8;
        if byte == 0 {
            return Some(out);
        }
        out.push(byte);
        at = at.checked_add(1)?;
    }
    None
}

/// `0x3D7B0`: the string of bank id `id` (level strings from 0xD87, the global table below, "" for 0).
pub fn string_by_id<G: GuestRead + ?Sized>(g: &G, id: u32) -> Option<Vec<u8>> {
    use prompt_addr::*;
    let level = g.read32(LEVEL_STRINGS)?;
    let pointer = if level == 0 {
        g.read32(MISSING_STRING)?
    } else if id as i32 >= LEVEL_STRINGS_FIRST as i32 {
        g.read32(
            level
                .wrapping_add(id.wrapping_mul(4))
                .wrapping_sub(LEVEL_STRINGS_FIRST * 4),
        )?
    } else if id == 0 {
        return Some(Vec::new());
    } else {
        match g.read32(GLOBAL_STRINGS)? {
            0 => g.read32(MISSING_STRING)?,
            table => g.read32(table.wrapping_add(id.wrapping_mul(4)))?,
        }
    };
    guest_cstring(g, pointer)
}

/// `0x3D7F0`: `%N` (1..9) takes argument N, `%%` a percent sign; a `%` with another character ends the text.
pub fn format_ids(format: &[u8], args: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::with_capacity(format.len() + 32);
    let mut i = 0;
    while i < format.len() {
        if format[i] != b'%' {
            out.push(format[i]);
            i += 1;
            continue;
        }
        match format.get(i + 1) {
            Some(b'%') => out.push(b'%'),
            Some(&d) if (b'1'..=b'9').contains(&d) => {
                if let Some(arg) = args.get(usize::from(d - b'1')) {
                    out.extend_from_slice(arg);
                }
            }
            _ => return out,
        }
        i += 2;
    }
    out
}

/// `sprintf` with `%s` only (the trigger and label formats of this drawer).
fn format_s(format: &[u8], args: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut next = args.iter();
    let mut i = 0;
    while i < format.len() {
        if format[i] == b'%' && format.get(i + 1) == Some(&b's') {
            out.extend_from_slice(next.next().copied().unwrap_or_default());
            i += 2;
        } else {
            out.push(format[i]);
            i += 1;
        }
    }
    out
}

/// `0x147080(action, player)`: the button code an action is bound to.
pub fn binding_code<G: GuestRead + ?Sized>(g: &G, action: i32, player: u32) -> Option<u32> {
    use prompt_addr::*;
    if (0..0xe).contains(&action) {
        let players = g.read32(PLAYERS_PTR)?;
        return g.read32(players.wrapping_add(PLAYER_BINDINGS).wrapping_add(
            (player.wrapping_mul(0x561).wrapping_add(action as u32)).wrapping_mul(4),
        ));
    }
    // Actions 0xF.. index a fixed table; 0xE and negative actions read outside it (not replayed).
    FIXED_CODES
        .get(usize::try_from(action - 0xf).ok()?)
        .copied()
}

/// `0x42D10(code)`: the button glyph text (icon escapes, a bank string or "START").
pub fn glyph_text<G: GuestRead + ?Sized>(g: &G, code: u32) -> Option<Vec<u8>> {
    use prompt_addr::*;
    let at = match code {
        0x80 => GLYPH_80,
        1..=0x40 => {
            let slot = (g.read32((GLYPH_CODE_MAP + code - 1) & !3)?
                >> (((GLYPH_CODE_MAP + code - 1) & 3) * 8)) as u8;
            GLYPH_STRINGS
                .get(usize::from(slot))
                .copied()
                .unwrap_or(GLYPH_NONE)
        }
        0x100 => GLYPH_100,
        0x200 => GLYPH_200,
        0x4000 => GLYPH_4000,
        0x8000 => GLYPH_8000,
        _ => match GLYPH_IDS.iter().find(|(c, _)| *c == code) {
            Some((_, id)) => return string_by_id(g, *id),
            None => GLYPH_NONE,
        },
    };
    guest_cstring(g, at)
}

/// The trigger text of codes 0x1000 and 0x2000 (`0x12C9C0` and the drawer's own copy): name and icon in the
/// language's order.
fn trigger_text<G: GuestRead + ?Sized>(g: &G, code: u32) -> Option<Vec<u8>> {
    use prompt_addr::*;
    let (id, icon) = if code == 0x1000 {
        (TRIGGER_LEFT_ID, TRIGGER_LEFT_ICON)
    } else {
        (TRIGGER_RIGHT_ID, TRIGGER_RIGHT_ICON)
    };
    let name = string_by_id(g, id)?;
    let icon = guest_cstring(g, icon)?;
    Some(match g.read32(LANGUAGE)? {
        0 => format_s(&guest_cstring(g, FMT_TRIGGER_EN)?, &[&name, &icon, b""]),
        2 => format_s(&guest_cstring(g, FMT_TRIGGER_2)?, &[&icon, &name, b""]),
        _ => format_s(&guest_cstring(g, FMT_TRIGGER_OTHER)?, &[&icon, &name, b""]),
    })
}

/// `0x12C9C0(code)`: the label of a binding inside a sentence ("glyph " or the trigger text).
pub fn binding_label<G: GuestRead + ?Sized>(g: &G, code: u32) -> Option<Vec<u8>> {
    if code == 0x1000 || code == 0x2000 {
        return trigger_text(g, code);
    }
    let mut text = glyph_text(g, code)?;
    text.push(b' ');
    Some(text)
}

/// The text system's values for one font as `0x3FA10` / `0x3F8D0` set them (main font table).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontMetrics {
    pub font: u32,
    pub object: FontObject,
    /// `0x7E728C`: object x scale times the text x scale.
    pub scale_x: f32,
    /// `0x7E7290`: height units times factor times object y scale times the text y scale.
    pub scale_y: f32,
    /// `0x7E7264` (= the line advance `0x7E727C`): record line top times the top scale times `scale_y`.
    pub line_top: f32,
    /// Size of a digit icon and the base of the letter icons (`20` in text mode 0x66, else the line top).
    pub icon_base: f32,
    /// The text x and y scales (`0x7E7270`, `0x7E7280`) for the renderer's glyph scale.
    pub text_scale: [f32; 2],
}

impl FontMetrics {
    pub fn read<G: GuestRead + ?Sized>(g: &G, font: u32) -> Option<Self> {
        use prompt_addr::*;
        if g.read32(ALT_TABLE)? != 0 || g.read32(FIXED_WIDTH)? != 0 {
            return None;
        }
        let object = FONT_OBJECTS + font.checked_mul(20)?;
        let record_index = g.read32(object)?;
        // Object `+4` selects fixed-width cells (not replayed).
        if g.read32(object + 4)? != 0 {
            return None;
        }
        let record = FONT_RECORDS + record_index.checked_mul(0x38)?;
        let (object_sx, object_sy) = (g.read_f32(object + 8)?, g.read_f32(object + 0xc)?);
        let text_scale = [g.read_f32(TEXT_SCALE_X)?, g.read_f32(TEXT_SCALE_Y)?];
        let scale_x = object_sx * g.read_f32(SCALE_ONE)? * text_scale[0];
        let scale_y = g.read32(HEIGHT_UNITS)? as i32 as f32
            * g.read_f32(HEIGHT_FACTOR)?
            * object_sy
            * text_scale[1];
        let line_top = g.read_f32(record + 0x18)? * g.read_f32(TEXT_SCALE_TOP)? * scale_y;
        let icon_base = if g.read32(TEXT_MODE)? == 0x66 {
            k::ICON_BASE_MODE_66
        } else {
            line_top
        };
        let adjust = g.read32(object + 0x10)? as i32;
        Some(FontMetrics {
            font,
            object: FontObject {
                record: record_index,
                scale_x: object_sx,
                scale_y: object_sy,
                advance_adjust: adjust,
            },
            scale_x,
            scale_y,
            line_top,
            icon_base,
            text_scale,
        })
    }
}

/// One inline icon escape (`\x0B` + letter or digit) resolved against the guest tables.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IconRef {
    pub texture: Option<&'static str>,
    /// Texture coordinates over the texture record size (one animation frame of a strip).
    pub uv: [f32; 4],
    /// Edge in pixels (square) and the pen advance.
    pub size: f32,
    pub advance: f32,
}

fn texture_name<G: GuestRead + ?Sized>(g: &G, id: u32) -> Option<&'static str> {
    let name = guest_cstring(
        g,
        g.read32(prompt_addr::TEXTURE_NAMES.wrapping_add(id.wrapping_mul(24)))?,
    )?;
    PROMPT_TEXTURES
        .iter()
        .copied()
        .find(|n| n.as_bytes() == name.as_slice())
}

/// Texture record size (`+0x26`, `+0x2A`) of handle `handle`.
fn texture_size<G: GuestRead + ?Sized>(g: &G, handle: u32) -> Option<(u32, u32)> {
    let record = g
        .read32(g.read32(prompt_addr::TEXTURE_RECORDS_PTR)?)?
        .wrapping_add(handle.wrapping_mul(0x34));
    Some((
        u32::from(g.read16(record + 0x26)?),
        u32::from(g.read16(record + 0x2a)?),
    ))
}

/// The icon of escape character `c` (`0x40FA0`): letters 'A'..'T' from the icon table (animated strips, per
/// language textures), digits '1'..'9' from the handle table. None for escapes not replayed.
pub fn icon<G: GuestRead + ?Sized>(g: &G, m: &FontMetrics, c: u8) -> Option<IconRef> {
    use prompt_addr::*;
    match c {
        b'A'..=b'T' => {
            let entry = ICON_TABLE + u32::from(c - b'A') * ICON_TABLE_SIZE;
            let mut id = g.read32(entry)?;
            let language = g.read32(LANGUAGE)?;
            let alternatives = g.read32(entry + 0xc)?;
            if language != 0 && alternatives != 0 {
                id = g.read32(
                    alternatives
                        .wrapping_add(language.wrapping_mul(4))
                        .wrapping_sub(4),
                )?;
            }
            let size = g.read_f32(entry + 8)? * m.icon_base;
            let handle = g.read32(TEXTURE_HANDLES.wrapping_add(id.wrapping_mul(24)))? as i32;
            if handle <= -1 {
                // No texture: the advance only.
                return Some(IconRef {
                    texture: None,
                    uv: [0.0; 4],
                    size,
                    advance: size,
                });
            }
            let (w, h) = texture_size(g, handle as u32)?;
            let shift = g.read32(entry + 0x14)? & 31;
            let frames = ((w >> shift) as i32)
                .checked_div(h as i32)
                .filter(|&n| n > 0)?;
            let clock = g.read32(CLOCK)? as i32 >> (g.read32(entry + 4)? & 31);
            let frame = clock.checked_rem(frames)?;
            let u0 = (frame * g.read32(entry + 0x18)? as i32 + g.read32(entry + 0x10)? as i32)
                * h as i32;
            let wf = w as f32;
            Some(IconRef {
                texture: Some(texture_name(g, id)?),
                uv: [u0 as f32 / wf, 0.0, (u0 + h as i32) as f32 / wf, 1.0],
                size,
                advance: size,
            })
        }
        b'1'..=b'9' => {
            let index = u32::from(c - b'1');
            let id = *DIGIT_IDS.get(index as usize)?;
            let handle = g.read32(DIGIT_HANDLES + index * 4)?;
            // The handle table is a copy of the id rows (`0x812F0`); anything else is not replayed.
            if handle != g.read32(TEXTURE_HANDLES.wrapping_add(id.wrapping_mul(24)))? {
                return None;
            }
            Some(IconRef {
                texture: Some(texture_name(g, id)?),
                uv: [0.0, 0.0, 1.0, 1.0],
                size: m.icon_base,
                advance: m.line_top,
            })
        }
        _ => None,
    }
}

/// Text measure and placement with the original font tables and the guest's text scales.
pub struct TextEnv<'a, G: GuestRead + ?Sized> {
    pub g: &'a G,
    pub fonts: &'a FontSet,
}

/// One run of drawable glyphs at its pen position.
#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    pub bytes: Vec<u8>,
    pub x: f32,
    pub y: f32,
}

/// One icon placed at the pen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedIcon {
    pub texture: &'static str,
    /// x0, y0, x1, y1 (guest pixels).
    pub rect: [f32; 4],
    pub uv: [f32; 4],
}

impl<'a, G: GuestRead + ?Sized> TextEnv<'a, G> {
    fn advance(&self, m: &FontMetrics, code: u8, next: Option<u8>) -> Option<f32> {
        let (_, record) = self.fonts.font(m.font)?;
        if record.rects.is_empty() {
            return None;
        }
        Some(self.fonts.original_advance(&m.object, record, code, next) as f32 * m.scale_x)
    }

    fn cell(&self, m: &FontMetrics, code: u8) -> Option<f32> {
        let (_, record) = self.fonts.font(m.font)?;
        Some(
            self.fonts
                .glyph_rect(record, code)
                .map_or(0.0, |r| (f32::from(r.h) + r.top) * m.scale_y),
        )
    }

    /// `0x3FCF0(text, single_line)`: width (widest pen) and height (lowest glyph bottom).
    pub fn measure(&self, m: &FontMetrics, text: &[u8], single_line: bool) -> Option<(f32, f32)> {
        let (mut pen, mut width, mut height, mut lines) = (0.0f32, 0.0f32, 0.0f32, 0i32);
        let mut i = 0;
        while i < text.len() {
            let c = text[i];
            match c {
                b'\t' => pen = trunc(pen * (1.0 / k::TAB) + 1.0) as f32 * k::TAB,
                b'\n' => {
                    if single_line {
                        break;
                    }
                    lines += 1;
                    pen = 0.0;
                }
                0x0b => {
                    let e = *text.get(i + 1)?;
                    i += 1;
                    let size = match e {
                        b'A'..=b'Z' => icon(self.g, m, e)?.size,
                        b's' | b'h' | b'w' => return None,
                        _ => m.icon_base,
                    };
                    pen += size;
                    width = width.max(pen);
                    height = height.max(lines as f32 * m.line_top + size);
                }
                _ => {
                    pen += self.advance(m, c, text.get(i + 1).copied())?;
                    width = width.max(pen);
                    height = height.max(self.cell(m, c)? + lines as f32 * m.line_top);
                }
            }
            i += 1;
        }
        Some((width, height))
    }

    /// `0x40F00(text, width)` / `0x40C10`: token wrap (words, single spaces, escapes kept whole); a line
    /// break trims the trailing spaces and drops a breaking space. Width 0 keeps the text.
    pub fn wrap(&self, m: &FontMetrics, text: &[u8], limit: f32) -> Option<Vec<u8>> {
        use prompt_addr::*;
        if limit == 0.0 {
            return Some(text.to_vec());
        }
        // Split mode (`0x7E7278` set): a word wider than the whole line is cut where it stops fitting.
        let split = self.g.read32(WRAP_SPLIT)? != 0;
        let keep_spaces = self.g.read32(WRAP_KEEP_SPACES)? != 0;
        let mut out: Vec<u8> = Vec::with_capacity(text.len() + 8);
        let mut line_start = 0usize;
        let mut used = 0.0f32;
        let mut at = 0usize;
        while at < text.len() {
            if text[at] == b'\n' {
                out.push(b'\n');
                at += 1;
                line_start = out.len();
                used = 0.0;
                continue;
            }
            let token: &[u8] = if text[at] == b' ' {
                &text[at..at + 1]
            } else {
                let mut end = at;
                while end < text.len() && text[end] != b' ' && text[end] != b'\n' {
                    end += if text[end] == 0x0b { 2 } else { 1 };
                }
                &text[at..end.min(text.len())]
            };
            if split && token != b" " {
                if let Some(cut) = self.split_point(m, token, used, limit)? {
                    out.extend_from_slice(&token[..cut]);
                    out.push(b'\n');
                    line_start = out.len();
                    used = 0.0;
                    at += cut;
                    continue;
                }
            }
            let width = self.measure(m, token, false)?.0;
            let mut skip = false;
            if used + width > limit {
                // Trailing spaces go (down to the buffer start, as `0x40C10` checks), then the break.
                while !keep_spaces && out.last() == Some(&b' ') {
                    out.pop();
                }
                if out.len() != line_start {
                    out.push(b'\n');
                }
                line_start = out.len();
                used = 0.0;
                skip = token == b" " && !keep_spaces;
            }
            if !skip {
                out.extend_from_slice(token);
            }
            // The original adds the token's width also for a dropped space.
            used += width;
            at += token.len();
        }
        Some(out)
    }

    /// The split of `0x40C10`'s per-character pass: grows the word one character (an escape two) at a time,
    /// remembers the length before the first one that overflows the line (`used` + partial), and cuts there
    /// once the partial word alone is wider than the line (and at least one character fits).
    fn split_point(
        &self,
        m: &FontMetrics,
        token: &[u8],
        used: f32,
        limit: f32,
    ) -> Option<Option<usize>> {
        let mut fit: Option<usize> = None;
        let mut len = 0usize;
        while len < token.len() {
            let previous = len;
            len += if token[len] == 0x0b { 2 } else { 1 };
            let len = len.min(token.len());
            let width = self.measure(m, &token[..len], false)?.0;
            if width + used > limit && fit.is_none() && previous > 0 {
                // Escape prefix and selector are one drawable unit. Never leave
                // a lone prefix at the end of the preceding line. If the first
                // unit is itself too wide, keep it whole and split after it.
                fit = Some(previous);
            }
            if width > limit {
                if let Some(cut) = fit.filter(|&c| c >= 1) {
                    return Some(Some(cut));
                }
            }
        }
        Some(None)
    }

    /// `0x40FA0` placement: glyph runs and icons of `text` drawn at (`x`, `y`), lines `line_top` apart,
    /// tab stops every 60 pixels from `x`, the line start from the justification (`0x40740`).
    pub fn place(
        &self,
        m: &FontMetrics,
        x: f32,
        y: f32,
        text: &[u8],
    ) -> Option<(Vec<TextRun>, Vec<PlacedIcon>)> {
        self.place_with_justification(m, x, y, text, None)
    }

    /// Place every line around `x`, without reading or changing the guest's
    /// global justification state. Split HUD notices set their own centered
    /// presentation mode for one draw.
    pub fn place_centered(
        &self,
        m: &FontMetrics,
        x: f32,
        y: f32,
        text: &[u8],
    ) -> Option<(Vec<TextRun>, Vec<PlacedIcon>)> {
        self.place_with_justification(m, x, y, text, Some(1))
    }

    fn place_with_justification(
        &self,
        m: &FontMetrics,
        x: f32,
        y: f32,
        text: &[u8],
        forced_justify: Option<u32>,
    ) -> Option<(Vec<TextRun>, Vec<PlacedIcon>)> {
        let justify = match forced_justify {
            Some(justify) => justify,
            None => self.g.read32(prompt_addr::JUSTIFY)?,
        };
        let line_start = |rest: &[u8]| -> Option<f32> {
            Some(match justify {
                1 => x - self.measure(m, rest, true)?.0 * 0.5,
                2 => x - self.measure(m, rest, true)?.0,
                _ => x,
            })
        };
        let (mut runs, mut icons) = (Vec::new(), Vec::new());
        let mut pen = line_start(text)?;
        let mut line_y = y;
        let mut run = TextRun {
            bytes: Vec::new(),
            x: pen,
            y: line_y,
        };
        let mut i = 0;
        let flush = |run: &mut TextRun, runs: &mut Vec<TextRun>, pen: f32, y: f32| {
            if !run.bytes.is_empty() {
                runs.push(std::mem::replace(
                    run,
                    TextRun {
                        bytes: Vec::new(),
                        x: pen,
                        y,
                    },
                ));
            } else {
                *run = TextRun {
                    bytes: Vec::new(),
                    x: pen,
                    y,
                };
            }
        };
        while i < text.len() {
            let c = text[i];
            match c {
                b'\t' => {
                    pen = trunc((pen - x) * (1.0 / k::TAB) + 1.0) as f32 * k::TAB + x;
                    flush(&mut run, &mut runs, pen, line_y);
                }
                b'\n' => {
                    pen = line_start(&text[i + 1..])?;
                    line_y += m.line_top;
                    flush(&mut run, &mut runs, pen, line_y);
                }
                0x0b => {
                    let e = *text.get(i + 1)?;
                    i += 1;
                    let icon = icon(self.g, m, e)?;
                    if let Some(texture) = icon.texture {
                        icons.push(PlacedIcon {
                            texture,
                            rect: [pen, line_y, pen + icon.size, line_y + icon.size],
                            uv: icon.uv,
                        });
                    }
                    pen += icon.advance;
                    flush(&mut run, &mut runs, pen, line_y);
                }
                _ => {
                    run.bytes.push(c);
                    pen += self.advance(m, c, text.get(i + 1).copied())?;
                }
            }
            i += 1;
        }
        if !run.bytes.is_empty() {
            runs.push(run);
        }
        Some((runs, icons))
    }
}

/// One line of a panel record.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelLine {
    pub string_id: u32,
    pub font: u32,
    pub outline: bool,
    pub actions: [i32; 2],
    pub kind: i32,
}

/// A panel record as the setters left it.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelRecord {
    pub lines: Vec<PanelLine>,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub active: bool,
    pub position: [f32; 3],
}

impl PanelRecord {
    pub fn read<G: GuestRead + ?Sized>(g: &G, index: u32) -> Option<Self> {
        use prompt_addr::*;
        let base = RECORDS + index * RECORD_SIZE;
        let count = g.read32(base + COUNT)? as i32;
        let lines = (0..count.clamp(0, LINES as i32) as u32)
            .map(|i| {
                let at = base + i * LINE_SIZE;
                Some(PanelLine {
                    string_id: g.read32(at)?,
                    font: g.read32(at + 4)?,
                    outline: g.read32(at + 8)? != 0,
                    actions: [g.read32(at + 0xc)? as i32, g.read32(at + 0x10)? as i32],
                    kind: g.read32(at + 0x18)? as i32,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(PanelRecord {
            lines,
            x: g.read32(base + X)? as i32,
            y: g.read32(base + Y)? as i32,
            width: g.read32(base + WIDTH)? as i32,
            active: g.read32(base + ACTIVE)? != 0,
            position: [
                g.read_f32(base + POSITION)?,
                g.read_f32(base + POSITION + 4)?,
                g.read_f32(base + POSITION + 8)?,
            ],
        })
    }
}

/// The Rust-owned lifecycle of one panel (the guest record's `+0xF0..+0x104` and the clearing of `+0xF4`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PanelState {
    /// Drawing (the guest's active flag stays set natively; this one ends the panel).
    pub live: bool,
    pub phase: u32,
    pub alpha: f32,
    pub timer: f32,
    pub distance: f32,
    /// Height of the last drawn text block (the panel rectangle uses the previous frame's height).
    pub height: i32,
}

/// One string as the original draws it: runs at their pens, plain or with the outline passes.
#[derive(Clone, Debug, PartialEq)]
pub struct PromptText {
    /// Right edge of the original text column, in design pixels.
    pub right: f32,
    pub runs: Vec<TextRun>,
    pub font: u32,
    /// Colour word (R, G, B, A high to low; 0x80 = 1).
    pub colour: u32,
    /// Outline alpha byte (black outline, `0x41E30`); None for plain text (`0x41DC0`).
    pub outline_alpha: Option<u8>,
    /// Renderer glyph scale (the text system's x and y scales).
    pub text_scale: [f32; 2],
}

/// A textured piece of the rounded panel: rect x0, y0, x1, y1 (guest pixels) and its texture coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelPiece {
    pub texture: &'static str,
    pub rect: [f32; 4],
    pub uv: [f32; 4],
}

/// Everything one prompt call draws, in the original's order.
#[derive(Clone, Debug, PartialEq)]
pub struct PromptDraw {
    /// Right edge shared by text and inline icons, independent of the previous-frame panel height.
    pub right: f32,
    pub panel: Vec<PanelPiece>,
    /// The two panel passes' alpha bytes (black; the screen-copy blur between them is not replayed).
    pub panel_alphas: [u8; 2],
    pub texts: Vec<PromptText>,
    pub icons: Vec<(PlacedIcon, u32)>,
    /// The font the original leaves selected (`0x3FA10` of the last line), restored by the gate.
    pub last_font: Option<u32>,
    /// The text block height this call measured (the record's `+0xF0`).
    pub height: i32,
}

/// What the gate does with one prompt call.
#[derive(Clone, Debug, PartialEq)]
pub enum PromptCall {
    Draw(PromptDraw),
    /// No panel this frame (inactive record, finished panel).
    Nothing,
    /// A case the native panel does not reproduce; the original runs.
    Legacy(&'static str),
}

/// `0x2D7B70(x0, y0, x1, y1, r)`: four corner quads (`femenubordernewcornerfill`, mirrored so the curve faces
/// out) and five fill quads (`femenubordernewfill`).
pub fn rounded_panel(x0: i32, y0: i32, x1: i32, y1: i32, r: i32) -> Vec<PanelPiece> {
    let f = |v: i32| v as f32;
    let (xa, ya, xb, yb) = (f(x0 + r), f(y0 + r), f(x1 - r), f(y1 - r));
    let (x0, y0, x1, y1) = (f(x0), f(y0), f(x1), f(y1));
    let corner = |rect, uv| PanelPiece {
        texture: CORNER_TEXTURE,
        rect,
        uv,
    };
    let fill = |rect| PanelPiece {
        texture: FILL_TEXTURE,
        rect,
        uv: [0.0, 0.0, 1.0, 1.0],
    };
    vec![
        corner([x0, y0, xa, ya], [1.0, 0.0, 0.0, 1.0]),
        corner([xb, y0, x1, ya], [0.0, 0.0, 1.0, 1.0]),
        corner([x0, yb, xa, y1], [1.0, 1.0, 0.0, 0.0]),
        corner([xb, yb, x1, y1], [0.0, 1.0, 1.0, 0.0]),
        fill([xa, ya, xb, yb]),
        fill([xa, y0, xb, ya]),
        fill([xa, yb, xb, y1]),
        fill([x0, ya, xa, yb]),
        fill([xb, ya, x1, yb]),
    ]
}

/// Inputs of the phase machine besides the state (frame rate, time scale, mode flag, player movement).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PhaseInput {
    pub frame_rate: i32,
    pub time_scale: f32,
    pub short: bool,
    /// Squared distance of the player from the show position plus twice the player's `+0x16C`.
    pub movement: f32,
}

impl PanelState {
    /// The phase machine at the end of `0x1476F0`.
    pub fn step(&mut self, input: &PhaseInput) {
        let tick = 1.0 / input.frame_rate as f32 * input.time_scale;
        match self.phase {
            0 => {
                if k::FADE_IN_DELAY > self.timer {
                    self.timer += tick;
                    return;
                }
                self.alpha += input.time_scale * k::FADE_IN_RATE;
                if self.alpha > 1.0 {
                    self.phase = 1;
                    self.alpha = 1.0;
                    self.timer = 0.0;
                }
            }
            1 => {
                self.timer += tick;
                let hold = if input.short { k::HOLD_SHORT } else { k::HOLD };
                if !input.short {
                    self.distance += input.movement;
                }
                if self.timer > hold {
                    self.timer = 0.0;
                    self.phase = 2;
                }
            }
            2 => {
                self.timer += tick;
                let linger = if input.short {
                    k::LINGER_SHORT
                } else {
                    k::LINGER
                };
                if self.timer > linger {
                    self.phase = 3;
                    return;
                }
                self.distance += input.movement;
                if self.distance > k::DISTANCE_LIMIT {
                    self.phase = 3;
                }
            }
            _ => {
                self.alpha -= input.time_scale * k::FADE_OUT_RATE;
                if 0.0 >= self.alpha {
                    self.live = false;
                }
            }
        }
    }
}

/// Lifecycle of the four panels, Rust-owned (`None`: not seen yet, seeded from the guest on first use).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PromptPanels {
    pub panels: [Option<PanelState>; 4],
}

/// Result of a shadow comparison (the original keeps drawing; its results of the previous call against
/// the Rust replay of that call).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PromptShadow {
    pub height_equal: bool,
    pub phase_equal: bool,
    pub alpha_error: f32,
    pub timer_error: f32,
    pub rust_height: i32,
    pub guest_height: i32,
}

impl PromptPanels {
    /// Show observed (`0x147180(record, ...)`): the panel restarts (the height is kept, as the record's).
    pub fn show(&mut self, record: u32) {
        let Some(index) = record
            .checked_sub(prompt_addr::RECORDS)
            .map(|o| o / prompt_addr::RECORD_SIZE)
        else {
            return;
        };
        if let Some(slot) = self.panels.get_mut(index as usize) {
            let height = slot.map_or(0, |s| s.height);
            *slot = Some(PanelState {
                live: true,
                height,
                ..PanelState::default()
            });
        }
    }

    /// Dismiss observed (`0x147300(index)`): a live panel fades out.
    pub fn dismiss(&mut self, index: i32) {
        if let Some(Some(state)) = usize::try_from(index)
            .ok()
            .and_then(|i| self.panels.get_mut(i))
        {
            if state.live {
                state.phase = 3;
            }
        }
    }

    /// Level init observed (`0x147250`): every panel ends, heights 0.
    pub fn reset(&mut self) {
        self.panels = [Some(PanelState::default()); 4];
    }

    fn seeded<G: GuestRead + ?Sized>(&mut self, g: &G, index: u32) -> Option<&mut PanelState> {
        use prompt_addr::*;
        let slot = self.panels.get_mut(index as usize)?;
        if slot.is_none() {
            let base = RECORDS + index * RECORD_SIZE;
            *slot = Some(PanelState {
                live: g.read32(base + ACTIVE)? != 0,
                phase: g.read32(base + PHASE)?,
                alpha: g.read_f32(base + ALPHA)?,
                timer: g.read_f32(base + TIMER)?,
                distance: g.read_f32(base + DISTANCE)?,
                height: g.read32(base + HEIGHT)? as i32,
            });
        }
        slot.as_mut()
    }

    /// Replays `0x1476F0(index)` on the live state (the native path: Rust-owned lifecycle).
    pub fn read<G: GuestRead + ?Sized>(
        &mut self,
        g: &G,
        fonts: &FontSet,
        index: i32,
    ) -> PromptCall {
        self.read_input(g, fonts, index, None)
    }

    /// Neighbouring weapon lifecycle is supplied directly by its native owner.
    /// This path never reads the old weapon phase/alpha guest fields.
    pub fn read_owned<G: GuestRead + ?Sized>(
        &mut self,
        g: &G,
        fonts: &FontSet,
        index: i32,
        weapon: &crate::hud_producers::WeaponAnimation,
    ) -> PromptCall {
        self.read_input(g, fonts, index, Some(weapon))
    }

    fn read_input<G: GuestRead + ?Sized>(
        &mut self,
        g: &G,
        fonts: &FontSet,
        index: i32,
        weapon: Option<&crate::hud_producers::WeaponAnimation>,
    ) -> PromptCall {
        if index < 0 || index as u32 >= prompt_addr::RECORD_COUNT {
            return PromptCall::Nothing;
        }
        let index = index as u32;
        let Some(record) = PanelRecord::read(g, index) else {
            return PromptCall::Legacy("unreadable record");
        };
        if !record.active {
            return PromptCall::Nothing;
        }
        let Some(state) = self.seeded(g, index).copied() else {
            return PromptCall::Legacy("unreadable record");
        };
        if !state.live {
            return PromptCall::Nothing;
        }
        let (draw, next) = match replay_with_weapon(g, fonts, index, &record, state, weapon) {
            Ok(v) => v,
            Err(why) => return PromptCall::Legacy(why),
        };
        if let Some(Some(slot)) = self.panels.get_mut(index as usize) {
            *slot = next;
        }
        PromptCall::Draw(draw)
    }

    /// Shadow check before the original runs: the guest record holds the original's results of the previous
    /// call; compares them with the Rust replay of that call, then replays this call from the guest state
    /// (so the comparison is per call, not accumulated).
    pub fn shadow_check<G: GuestRead + ?Sized>(
        &mut self,
        g: &G,
        fonts: &FontSet,
        index: i32,
    ) -> Option<PromptShadow> {
        use prompt_addr::*;
        let i = u32::try_from(index).ok().filter(|&i| i < RECORD_COUNT)?;
        let base = RECORDS + i * RECORD_SIZE;
        let guest = PanelState {
            live: g.read32(base + ACTIVE)? != 0,
            phase: g.read32(base + PHASE)?,
            alpha: g.read_f32(base + ALPHA)?,
            timer: g.read_f32(base + TIMER)?,
            distance: g.read_f32(base + DISTANCE)?,
            height: g.read32(base + HEIGHT)? as i32,
        };
        let previous = self.panels.get(i as usize).copied().flatten();
        let check = previous.filter(|p| p.live).map(|rust| PromptShadow {
            height_equal: rust.height == guest.height,
            phase_equal: rust.phase == guest.phase,
            alpha_error: (rust.alpha - guest.alpha).abs(),
            timer_error: (rust.timer - guest.timer).abs(),
            rust_height: rust.height,
            guest_height: guest.height,
        });
        // Replay this call from the original's state; the result is compared at the next call.
        self.panels[i as usize] = None;
        let record = PanelRecord::read(g, i)?;
        let next = if record.active && guest.live {
            replay(g, fonts, i, &record, guest).ok().map(|(_, s)| s)
        } else {
            None
        };
        self.panels[i as usize] = next.or(Some(PanelState {
            live: false,
            ..guest
        }));
        check
    }
}

/// The drawer's body for a live panel: the draw and the state after the phase machine.
fn replay<G: GuestRead + ?Sized>(
    g: &G,
    fonts: &FontSet,
    index: u32,
    record: &PanelRecord,
    state: PanelState,
) -> Result<(PromptDraw, PanelState), &'static str> {
    replay_with_weapon(g, fonts, index, record, state, None)
}

fn replay_with_weapon<G: GuestRead + ?Sized>(
    g: &G,
    fonts: &FontSet,
    index: u32,
    record: &PanelRecord,
    state: PanelState,
    weapon: Option<&crate::hud_producers::WeaponAnimation>,
) -> Result<(PromptDraw, PanelState), &'static str> {
    use prompt_addr::*;
    const UNREADABLE: &str = "unreadable state";
    // Fade: the record alpha, limited by the HUD fade of the viewer's slot.
    let slot = g
        .read32(VIEWER)
        .and_then(|v| g.read32(v + 4))
        .ok_or(UNREADABLE)?;
    let hud = HUD_RECORDS.wrapping_add(slot.wrapping_mul(HUD_RECORD_SIZE));
    let mut limit = 1.0f32;
    let phase = match weapon {
        Some(w) => w.phase,
        None => g.read32(hud + HUD_FADE_FLAG).ok_or(UNREADABLE)?,
    };
    if phase != 0 {
        let alpha = match weapon {
            Some(w) => w.alpha,
            None => g.read_f32(hud + HUD_FADE).ok_or(UNREADABLE)?,
        };
        limit -= (alpha * 2.0).min(1.0);
    }
    let fade = if limit > state.alpha {
        state.alpha
    } else {
        limit
    };
    let (x0, y0) = (record.x - k::MARGIN_X, record.y - k::MARGIN_Y);
    let (x1, y1) = (
        record.x + record.width + k::MARGIN_X,
        record.y + state.height + k::MARGIN_Y,
    );
    let text_alpha = (trunc(fade * k::TEXT_ALPHA) & 0xff) as u32;
    let colour = k::TEXT_RGB | text_alpha;
    let env = TextEnv { g, fonts };
    let mut draw = PromptDraw {
        right: (record.x + record.width) as f32,
        panel: rounded_panel(x0, y0, x1, y1, k::RADIUS),
        panel_alphas: [
            (trunc(fade * k::PANEL_ALPHA_1) & 0xff) as u8,
            (trunc(fade * k::PANEL_ALPHA_2) & 0xff) as u8,
        ],
        texts: Vec::new(),
        icons: Vec::new(),
        last_font: None,
        height: state.height,
    };
    let mut y = record.y;
    let (x, width) = (record.x as f32, record.width as f32);
    let emit = |draw: &mut PromptDraw,
                m: &FontMetrics,
                at_x: f32,
                at_y: f32,
                text: &[u8],
                outline: Option<u8>|
     -> Result<(), &'static str> {
        let (runs, icons) = env
            .place(m, at_x, at_y, text)
            .ok_or("text escape not replayed")?;
        draw.icons.extend(icons.into_iter().map(|i| (i, colour)));
        draw.texts.push(PromptText {
            right: x + width,
            runs,
            font: m.font,
            colour,
            outline_alpha: outline,
            text_scale: m.text_scale,
        });
        Ok(())
    };
    let mut text_buffer: Vec<u8> = Vec::new();
    for line in &record.lines {
        let player = index;
        let label = |action: i32| -> Result<Vec<u8>, &'static str> {
            let code = binding_code(g, action, player).ok_or("binding not replayed")?;
            binding_label(g, code).ok_or(UNREADABLE)
        };
        let string = || string_by_id(g, line.string_id).ok_or(UNREADABLE);
        match line.kind {
            0 => text_buffer = string()?,
            1 => text_buffer = format_ids(&string()?, &[label(line.actions[0])?]),
            2 => {
                text_buffer = format_ids(
                    &string()?,
                    &[label(line.actions[0])?, label(line.actions[1])?],
                )
            }
            // The third label reads the kind field itself (action 3), as the original does.
            3 => {
                text_buffer = format_ids(
                    &string()?,
                    &[label(line.actions[0])?, label(line.actions[1])?, label(3)?],
                )
            }
            kind if kind > 3 => return Err("line kind not replayed"),
            _ => {}
        }
        let m = FontMetrics::read(g, line.font).ok_or("font state not replayed")?;
        draw.last_font = Some(line.font);
        let yf = y as f32;
        if line.kind > -1 {
            env.measure(&m, &text_buffer, false)
                .ok_or("text measure not replayed")?;
            let wrapped = env
                .wrap(&m, &text_buffer, width)
                .ok_or("wrap not replayed")?;
            let (_, h) = env
                .measure(&m, &wrapped, false)
                .ok_or("text escape not replayed")?;
            if line.outline {
                emit(&mut draw, &m, x, yf, &wrapped, Some(text_alpha as u8))?;
                y = trunc(h + yf + k::GAP_OUTLINED);
            } else {
                emit(&mut draw, &m, x, yf, &wrapped, None)?;
                y = trunc(h + yf + k::GAP_PLAIN);
            }
            continue;
        }
        // A binding line: the glyph (or the trigger text) left, the wrapped string to its right.
        let code = binding_code(g, line.actions[0], player).ok_or("binding not replayed")?;
        let label_text = string()?;
        let column = g.read_f32(SCALE_ONE).ok_or(UNREADABLE)? * k::GLYPH_COLUMN;
        let (left, offset) = if code == 0x1000 || code == 0x2000 {
            let id = if code == 0x1000 {
                TRIGGER_LEFT_ID
            } else {
                TRIGGER_RIGHT_ID
            };
            let name_width = env
                .measure(&m, &string_by_id(g, id).ok_or(UNREADABLE)?, false)
                .ok_or("text escape not replayed")?
                .0;
            (
                trigger_text(g, code).ok_or(UNREADABLE)?,
                column + name_width,
            )
        } else {
            (glyph_text(g, code).ok_or(UNREADABLE)?, column)
        };
        emit(&mut draw, &m, x, yf, &left, None)?;
        env.measure(&m, &label_text, false)
            .ok_or("text measure not replayed")?;
        let wrapped = env
            .wrap(&m, &label_text, width - offset)
            .ok_or("wrap not replayed")?;
        emit(&mut draw, &m, offset + x, yf, &wrapped, None)?;
        let (_, h) = env
            .measure(&m, &wrapped, false)
            .ok_or("text escape not replayed")?;
        y = trunc(h + yf + k::GAP_PLAIN);
    }
    let mut next = state;
    next.height = y - record.y;
    draw.height = next.height;
    let frame_rate = g.read32(FRAME_RATE).ok_or(UNREADABLE)? as i32;
    let short = g.read32(MODE_FLAGS).ok_or(UNREADABLE)? & 0x40 != 0;
    let movement = if next.phase == 1 || next.phase == 2 {
        let players = g.read32(PLAYERS_PTR).ok_or(UNREADABLE)?;
        let player = players.wrapping_add(index.wrapping_mul(PLAYER_SIZE));
        let mut sum = 0.0f32;
        for axis in 0..3u32 {
            let d = g
                .read_f32(player + PLAYER_POSITION + 4 * axis)
                .ok_or(UNREADABLE)?
                - record.position[axis as usize];
            sum += d * d;
        }
        sum + g.read_f32(player + PLAYER_EXTRA).ok_or(UNREADABLE)? * 2.0
    } else {
        0.0
    };
    next.step(&PhaseInput {
        frame_rate,
        time_scale: g.read_f32(TIME_SCALE).ok_or(UNREADABLE)?,
        short,
        movement,
    });
    Ok((draw, next))
}

#[cfg(test)]
mod tests {
    //! Tier 0, FABRICATED guest values on the real offsets.
    use super::*;

    use std::collections::HashMap;

    struct Guest(HashMap<u32, u32>);
    impl GuestRead for Guest {
        fn read32(&self, va: u32) -> Option<u32> {
            self.0.get(&va).copied()
        }
    }
    impl Guest {
        fn string(&mut self, va: u32, text: &[u8]) {
            let mut bytes = text.to_vec();
            bytes.push(0);
            while bytes.len() % 4 != 0 {
                bytes.push(0);
            }
            for (i, chunk) in bytes.chunks(4).enumerate() {
                self.0.insert(
                    va + 4 * i as u32,
                    u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
                );
            }
        }
    }

    #[test]
    fn bindings_glyphs_and_icons_read_the_original_tables() {
        use prompt_addr::*;
        let mut g = Guest(HashMap::new());
        // Player 1 binds action 2 to code 0x20; actions from 0xF use the fixed table.
        g.0.insert(PLAYERS_PTR, 0x0100_0000);
        g.0.insert(0x0100_0000 + PLAYER_BINDINGS + (0x561 + 2) * 4, 0x20);
        assert_eq!(binding_code(&g, 2, 1), Some(0x20));
        assert_eq!(binding_code(&g, 0xf + 8, 1), Some(0x8000));
        assert_eq!(binding_code(&g, 0xe, 1), None);
        // Code 0x20 goes through the code byte map (slot 5) to its glyph string; the label adds a space.
        g.0.insert(GLYPH_CODE_MAP + 0x1c, 5 << 24);
        g.string(GLYPH_STRINGS[5], b"\x0b1");
        assert_eq!(glyph_text(&g, 0x20), Some(b"\x0b1".to_vec()));
        assert_eq!(binding_label(&g, 0x20), Some(b"\x0b1 ".to_vec()));
        // Digit icon '1': texture id 0x29A, its handle row copied into the handle table, name from the name row.
        g.0.insert(DIGIT_HANDLES, 7);
        g.0.insert(TEXTURE_HANDLES + 0x29a * 24, 7);
        g.0.insert(TEXTURE_NAMES + 0x29a * 24, 0x0200_0000);
        g.string(0x0200_0000, b"xbstdbutton1new");
        let m = FontMetrics {
            font: 18,
            object: FontObject {
                record: 1,
                scale_x: 1.0,
                scale_y: 1.0,
                advance_adjust: 0,
            },
            scale_x: 1.0,
            scale_y: 1.0,
            line_top: 16.0,
            icon_base: 16.0,
            text_scale: [1.0, 1.0],
        };
        assert_eq!(
            icon(&g, &m, b'1'),
            Some(IconRef {
                texture: Some("xbstdbutton1new"),
                uv: [0.0, 0.0, 1.0, 1.0],
                size: 16.0,
                advance: 16.0
            })
        );
        // A handle table that is not the row's copy is left to the original.
        g.0.insert(DIGIT_HANDLES, 8);
        assert_eq!(icon(&g, &m, b'1'), None);
        // Letter icon 'N' (row 13): an animated strip, frame (clock >> 3) % frames, u0 = (frame * stride + base) * h.
        let entry = ICON_TABLE + 13 * ICON_TABLE_SIZE;
        for (offset, value) in [
            (0, 0x2ab),
            (4, 3),
            (8, 1.2f32.to_bits()),
            (0xc, 0),
            (0x10, 1),
            (0x14, 0),
            (0x18, 1),
        ] {
            g.0.insert(entry + offset, value);
        }
        g.0.insert(LANGUAGE, 0);
        g.0.insert(CLOCK, 5 << 3);
        g.0.insert(TEXTURE_HANDLES + 0x2ab * 24, 3);
        g.0.insert(TEXTURE_NAMES + 0x2ab * 24, 0x0300_0000);
        g.string(0x0300_0000, b"xb_dpad_updown");
        g.0.insert(TEXTURE_RECORDS_PTR, 0x0400_0000);
        g.0.insert(0x0400_0000, 0x0500_0000);
        let record = 0x0500_0000 + 3 * 0x34;
        g.0.insert(record + 0x24, 64 << 16);
        g.0.insert(record + 0x28, 16 << 16);
        let n = icon(&g, &m, b'N').unwrap();
        // 4 frames of 16 x 16; frame 5 % 4 = 1, u0 = (1 * 1 + 1) * 16 = 32.
        assert_eq!(n.texture, Some("xb_dpad_updown"));
        assert_eq!(n.uv, [0.5, 0.0, 0.75, 1.0]);
        assert!((n.size - 19.2).abs() < 1e-4);
    }

    #[test]
    fn split_wrap_keeps_inline_icon_escapes_atomic() {
        use prompt_addr::*;
        let mut g = Guest(HashMap::new());
        g.0.insert(WRAP_SPLIT, 1);
        g.0.insert(WRAP_KEEP_SPACES, 0);
        g.0.insert(JUSTIFY, 0);
        g.0.insert(DIGIT_HANDLES, 7);
        g.0.insert(TEXTURE_HANDLES + 0x29a * 24, 7);
        g.0.insert(TEXTURE_NAMES + 0x29a * 24, 0x0200_0000);
        g.string(0x0200_0000, b"xbstdbutton1new");
        let m = FontMetrics {
            font: 18,
            object: FontObject {
                record: 1,
                scale_x: 1.0,
                scale_y: 1.0,
                advance_adjust: 0,
            },
            scale_x: 1.0,
            scale_y: 1.0,
            line_top: 16.0,
            icon_base: 16.0,
            text_scale: [1.0; 2],
        };
        let fonts = FontSet::default();
        let env = TextEnv {
            g: &g,
            fonts: &fonts,
        };
        for limit in [8.0, 16.0, 24.0] {
            let wrapped = env.wrap(&m, b"\x0b1\x0b1\x0b1", limit).unwrap();
            assert_eq!(wrapped, b"\x0b1\n\x0b1\n\x0b1", "limit {limit}");
            let (runs, icons) = env.place(&m, 40.0, 60.0, &wrapped).unwrap();
            assert!(runs.is_empty());
            assert_eq!(icons.len(), 3);
            for (i, icon) in icons.iter().enumerate() {
                assert_eq!(
                    icon.rect,
                    [40.0, 60.0 + i as f32 * 16.0, 56.0, 76.0 + i as f32 * 16.0]
                );
            }
        }
        // A single indivisible icon wider than the column stays whole.
        assert_eq!(env.wrap(&m, b"\x0b1", 8.0), Some(b"\x0b1".to_vec()));
        assert_eq!(
            env.wrap(&m, b"\x0b1\x0b1", 0.0),
            Some(b"\x0b1\x0b1".to_vec())
        );
    }

    #[test]
    fn explicit_centered_placement_ignores_guest_justification() {
        use crate::font::{FontObject, FontRecord, GlyphRect};
        use std::collections::BTreeMap;

        let mut guest = Guest(HashMap::from([(prompt_addr::JUSTIFY, 2)]));
        guest.0.insert(prompt_addr::WRAP_SPLIT, 0);
        guest.0.insert(prompt_addr::WRAP_KEEP_SPACES, 0);
        let record = FontRecord {
            rects: vec![
                GlyphRect {
                    w: 5,
                    h: 10,
                    ..Default::default()
                };
                96
            ],
            ..Default::default()
        };
        let fonts = FontSet {
            records: BTreeMap::from([(0, record)]),
            objects: vec![FontObject {
                record: 0,
                scale_x: 1.0,
                scale_y: 1.0,
                advance_adjust: 0,
            }],
            ..Default::default()
        };
        let metrics = FontMetrics {
            font: 0,
            object: FontObject {
                record: 0,
                scale_x: 1.0,
                scale_y: 1.0,
                advance_adjust: 0,
            },
            scale_x: 1.0,
            scale_y: 1.0,
            line_top: 10.0,
            icon_base: 10.0,
            text_scale: [1.0; 2],
        };
        let env = TextEnv {
            g: &guest,
            fonts: &fonts,
        };

        let normal = env.place(&metrics, 100.0, 20.0, b"ab").unwrap().0;
        let centered = env.place_centered(&metrics, 100.0, 20.0, b"ab").unwrap().0;
        assert_eq!(normal[0].x, 90.0, "guest JUSTIFY=2 remains right aligned");
        assert_eq!(
            centered[0].x, 95.0,
            "explicit mode centers around the anchor"
        );
    }

    #[test]
    fn format_takes_numbered_arguments() {
        let args = vec![b"A".to_vec(), b"B".to_vec()];
        assert_eq!(
            format_ids(b"x %2 y %1 %% z", &args),
            b"x B y A % z".to_vec()
        );
        assert_eq!(format_ids(b"stop %x here", &args), b"stop ".to_vec());
        assert_eq!(
            format_s(b"%s %s - %s", &[b"\x0bL", b"Name", b""]),
            b"\x0bL Name - ".to_vec()
        );
    }

    #[test]
    fn panel_has_mirrored_corners_and_five_fills() {
        let pieces = rounded_panel(0, 0, 100, 60, 20);
        assert_eq!(pieces.len(), 9);
        assert_eq!(
            pieces[0],
            PanelPiece {
                texture: CORNER_TEXTURE,
                rect: [0.0, 0.0, 20.0, 20.0],
                uv: [1.0, 0.0, 0.0, 1.0]
            }
        );
        assert_eq!(pieces[3].rect, [80.0, 40.0, 100.0, 60.0]);
        assert_eq!(pieces[3].uv, [0.0, 1.0, 1.0, 0.0]);
        let area: f32 = pieces
            .iter()
            .map(|p| (p.rect[2] - p.rect[0]) * (p.rect[3] - p.rect[1]))
            .sum();
        assert_eq!(area, 100.0 * 60.0);
    }

    #[test]
    fn prompt_fade_consumes_owned_weapon_without_guest_lifecycle_reads() {
        use prompt_addr::*;
        struct Inputs;
        impl GuestRead for Inputs {
            fn read32(&self, address: u32) -> Option<u32> {
                assert!(!(HUD_RECORDS..HUD_RECORDS + 4 * HUD_RECORD_SIZE).contains(&address));
                Some(match address {
                    VIEWER => 0x1000,
                    0x1004 => 0,
                    FRAME_RATE => 60,
                    TIME_SCALE => 1.0f32.to_bits(),
                    _ => 0,
                })
            }
        }
        let record = PanelRecord {
            lines: vec![],
            x: 40,
            y: 50,
            width: 200,
            active: true,
            position: [0.0; 3],
        };
        let state = PanelState {
            live: true,
            phase: 0,
            alpha: 1.0,
            timer: 0.0,
            distance: 0.0,
            height: 30,
        };
        let weapon = crate::hud_producers::WeaponAnimation {
            phase: 1,
            alpha: 0.25,
            ..Default::default()
        };
        let (draw, _) = replay_with_weapon(
            &Inputs,
            &FontSet::default(),
            0,
            &record,
            state,
            Some(&weapon),
        )
        .unwrap();
        assert_eq!(draw.right, 240.0);
        assert_eq!(
            draw.panel_alphas,
            [
                (trunc(0.5 * k::PANEL_ALPHA_1) & 0xff) as u8,
                (trunc(0.5 * k::PANEL_ALPHA_2) & 0xff) as u8
            ]
        );
        assert_eq!(draw.height, 0);
    }

    #[test]
    fn phase_machine_fades_in_holds_and_fades_out() {
        let input = PhaseInput {
            frame_rate: 60,
            time_scale: 1.0,
            short: false,
            movement: 0.0,
        };
        let mut s = PanelState {
            live: true,
            ..PanelState::default()
        };
        // Delay of 0.1 s: six frames at 60 fps, then 14 fade-in steps of 0.075 (alpha above 1).
        let mut frames = 0;
        while s.phase == 0 {
            s.step(&input);
            frames += 1;
        }
        assert_eq!((s.alpha, s.timer), (1.0, 0.0));
        assert_eq!(frames, 6 + 14);
        // Hold 6 s without moving, then linger 8 s.
        let mut frames = 0;
        while s.phase == 1 {
            s.step(&input);
            frames += 1;
        }
        assert!((360..=362).contains(&frames), "{frames}");
        // Moving ends the linger once the accumulated distance passes 16.
        let moving = PhaseInput {
            movement: 5.0,
            ..input
        };
        s.step(&moving);
        s.step(&moving);
        s.step(&moving);
        assert_eq!(s.phase, 2);
        s.step(&moving);
        assert_eq!(s.phase, 3);
        let mut frames = 0;
        while s.live {
            s.step(&input);
            frames += 1;
        }
        // 1 - 50 * 0.02 stays a hair above 0 in f32: the 51st step ends the panel.
        assert_eq!(frames, 51);
    }

    #[test]
    fn show_dismiss_and_init_drive_the_rust_state() {
        let mut panels = PromptPanels::default();
        panels.show(prompt_addr::RECORDS + prompt_addr::RECORD_SIZE);
        assert_eq!(panels.panels[1].map(|s| (s.live, s.phase)), Some((true, 0)));
        panels.dismiss(1);
        assert_eq!(panels.panels[1].map(|s| s.phase), Some(3));
        panels.dismiss(0);
        assert_eq!(panels.panels[0], None);
        panels.reset();
        assert!(
            panels
                .panels
                .iter()
                .all(|s| s.is_some_and(|s| !s.live && s.height == 0))
        );
    }
}
