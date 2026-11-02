// SPDX-License-Identifier: GPL-3.0-or-later
//! T2311 UI-13 increment 3: the data the Story level rows (`0x2CA9F0`) and Challenge set rows
//! (`0x2C8B50`) draw, read from the live guest through a typed view, for the pure Rust draw in
//! `tsfp_ui_model::list_rows`.
//!
//! Every address and stride here is MEASURED by static reading of `default.xbe` (doc
//! docs/t2311-ui-pages-batch1.md, increment 3). Read only; nothing is written.

use crate::live_page::{LivePage, LiveRow};
use crate::menu_state::layout::{PAGE_STRIDE, PAGE_TABLE, page, row};
use crate::native_page::GuestUi;
#[cfg(test)]
use tsfp_ui_model::WidgetId;
use tsfp_ui_model::list_rows::{
    CHALLENGE_ROW_DRAW, Edges, LEVEL_ROW_DRAW, ListRow, ListRowKind, ListRows, MEDAL_GLOW_TEXTURE,
    Medal, PADLOCK_TEXTURE, RowIcon, SCROLL_ARROW_TEXTURE, ScrollArrows,
};
use tsfp_ui_model::option_rows::{
    AV_PAGE_DRAW, BUTTON_ROW_DRAW, ButtonPicture, ICON_ROW_DRAW, ICON_TEXTURES, NamedTexture,
    OptionRow, OptionRows, Pulse, SLIDER_ROW_DRAW, SafeFrame, Slider, TURN_SPEED_ROW_DRAW,
    TextSlider, ValueArrows,
};
use tsfp_ui_model::progress_summary::{
    PLAYER_ICON_SLOT, PROGRESS_SUMMARY_DRAW, PlayerIcon, ProgressStats, ProgressSummary,
};

/// Guest addresses of the list row draws.
pub mod addr {
    /// `[0x7BA038]` text unit and `[0x7BA188]` medal row unit (f32 run-time globals).
    pub const TEXT_UNIT: u32 = 0x7B_A038;
    pub const MEDAL_UNIT: u32 = 0x7B_A188;
    /// Page `+0x4` word compared with `0x5238C0` by `0x2CA9F0` (plain page alpha).
    pub const PAGE_CLASS: u32 = 0x04;
    pub const PLAIN_PAGE_CLASS: u32 = 0x52_38C0;
    /// Page style record `+0x1C` (pointer), its `+0x1C` f32 the text margin.
    pub const STYLE_TEXT_MARGIN: u32 = 0x1C;
    pub const PAGE_FADE: u32 = 0xF4;
    /// T2315: the page record's bottom edge (`+0xF0`, design units), the lowest viewport `0x722B0` sets.
    pub const PAGE_BOTTOM: u32 = 0xF0;
    /// T2315: scrolled page fields (f32 units): content height `+0x5C`, view top `+0x6C` and bottom `+0x70`, scroll offset
    /// `+0x84`; page y origin `+0xAC` (design units); hidden flag `+0x40`. Rodata of `0x73FC0` / `0x73E40`: the scroll
    /// threshold, the alpha multiplier, the right edge offsets, the up arrow height and the down arrow end.
    pub const PAGE_CONTENT: u32 = 0x5C;
    pub const PAGE_VIEW_TOP: u32 = 0x6C;
    pub const PAGE_VIEW_BOTTOM: u32 = 0x70;
    pub const PAGE_SCROLL: u32 = 0x84;
    pub const PAGE_Y: u32 = 0xAC;
    pub const PAGE_HIDDEN: u32 = 0x40;
    /// The bit of [`PAGE_HIDDEN`] `0x73FC0` tests (`test ah, 1`).
    pub const PAGE_HIDDEN_BIT: u32 = 0x100;
    /// The page style's first word bit that selects these arrows in `0x76810` (`test al, 0x20`; bit 0x40 selects the
    /// `[style + 0xAC]` / `0x727F0` path instead).
    pub const STYLE_ARROWS: u32 = 0x20;
    /// The page rect's right edge (`+0xEC`, the rect `+0xE4` of `0x73E40`).
    pub const PAGE_RIGHT: u32 = 0xEC;
    pub const ARROW_THRESHOLD: u32 = 0x47_5CD4;
    pub const ARROW_ALPHA: u32 = 0x47_5CC0;
    pub const ARROW_RIGHT: u32 = 0x47_CFF8;
    pub const ARROW_INSET: u32 = 0x47_862C;
    pub const ARROW_UP_HEIGHT: u32 = 0x47_82C4;
    pub const ARROW_DOWN_END: u32 = 0x47_CFF4;
    /// The arrow quad width (`edi + 0xE` in `0x73E40`).
    pub const ARROW_WIDTH: f32 = 14.0;
    /// T2315: row flag (`+0x28`) that lifts the row viewport top by 4 (`0x722B0`, constant `[0x475DB4]`).
    pub const ROW_FLAG_CLIP_LIFT: u32 = 0x0200_0000;
    /// Level table: 0x44-byte records, string id i16 `+0x2`, icon texture table index `+0x14`.
    pub const LEVELS: u32 = 0x4D_1AC0;
    pub const LEVEL_STRIDE: u32 = 0x44;
    pub const LEVEL_NAME: u32 = 0x2;
    pub const LEVEL_ICON: u32 = 0x14;
    /// Texture table (`0x80F20`): 24-byte records from `0x4D2D64`, name pointer at record - 8.
    pub const TEXTURES: u32 = 0x4D_2D64;
    pub const TEXTURE_STRIDE: u32 = 24;
    /// Challenge sets (`0x22E070`): 0x38-byte records, `0x5107E0` for sets 0..6, `0x513B60` after.
    pub const SETS: u32 = 0x51_07E0;
    pub const SETS_HIGH: u32 = 0x51_3B60;
    pub const SETS_LOW_COUNT: u32 = 7;
    pub const SET_STRIDE: u32 = 0x38;
    pub const SET_COUNT: u32 = 0x4;
    pub const SET_IDS: u32 = 0x8;
    pub const SET_ICON: u32 = 0x20;
    /// Challenge records by id (`0x22E050`): `[0x510788 + id * 4]` below 0x15, else `[0x513D04 + id * 4]`.
    pub const CHALLENGES: u32 = 0x51_0788;
    pub const CHALLENGES_HIGH: u32 = 0x51_3D04;
    pub const CHALLENGES_LOW_COUNT: u32 = 0x15;
    /// Challenge record: unlock bits `+0xA0` (bit `[0x5236C4]`), hidden flag `+0x18 & 0x20`.
    pub const CHALLENGE_UNLOCK_BITS: u32 = 0xA0;
    pub const CHALLENGE_FLAGS: u32 = 0x18;
    pub const CHALLENGE_HIDDEN: u32 = 0x20;
    pub const UNLOCK_BIT: u32 = 0x52_36C4;
    /// Results (`0x69930(0, id)`): 24-byte records at `base + 0x490`, medal `+0xC`. Base `0x6F11C0`
    /// when `[0x7DE458] & 0x800000` or profile 0 is not loaded (`[0x7BEDC0] == 0`), else `0x7C0050`.
    pub const RESULT_MODE: u32 = 0x7D_E458;
    pub const RESULT_MODE_BIT: u32 = 0x80_0000;
    pub const PROFILE_LOADED: u32 = 0x7B_EDC0;
    pub const RESULTS_DEFAULT: u32 = 0x6F_11C0;
    pub const RESULTS_PROFILE: u32 = 0x7C_0050;
    pub const RESULTS_OFFSET: u32 = 0x490;
    pub const RESULT_STRIDE: u32 = 24;
    pub const RESULT_MEDAL: u32 = 0xC;
    /// Medal picture texture table indices (`0x2BF4E0`: `[0x5236C8 + medal * 4]`).
    pub const MEDAL_TEXTURES: u32 = 0x52_36C8;
    /// Player Progress (`0x2CC850` and the getters `0x660C0..0x66310`): profile `[0x78AD34]`,
    /// line advance `[0x7E7264]` (f32, see `line_advance`), page margins pointer `+0x24` (f32 left
    /// `[0]`, right `[2]`).
    pub const PROGRESS_PROFILE: u32 = 0x78_AD34;
    pub const LINE_HEIGHT: u32 = 0x7E_7264;
    pub const PAGE_MARGINS: u32 = 0x24;
    /// Story bits: one word per player count and difficulty at results `+0xBF4 + 12 * players +
    /// 4 * difficulty` (`0x66280`), bit n = level n done (13 levels, `0x66160`).
    pub const STORY_WORDS: u32 = 0xBF4;
    pub const LEVEL_COUNT: u32 = 13;
    /// Results of challenges 0..20 and leagues 21..47: medal at `+0x49C + 24 * index` (`0x66100`,
    /// `0x660C0`, `0x66070`).
    pub const RESULT_MEDALS: u32 = 0x49C;
    pub const CHALLENGE_COUNT: u32 = 21;
    pub const LEAGUE_COUNT: u32 = 27;
    /// Profile slot stride (`0x69900`: `[0x7BEDC0 + profile * 0x1F54]`).
    pub const PROFILE_STRIDE: u32 = 0x1F54;
    /// Texture slots `[0x7BA040 + slot * 4]` (handles); texture records `[[0x4B85C8]] + handle *
    /// 0x34`, width u16 `+0x26`, height u16 `+0x2A`.
    pub const TEXTURE_SLOTS: u32 = 0x7B_A040;
    pub const TEXTURE_RECORDS: u32 = 0x4B_85C8;
    pub const TEXTURE_RECORD_STRIDE: u32 = 0x34;
    pub const TEXTURE_WIDTH: u32 = 0x26;
    pub const TEXTURE_HEIGHT: u32 = 0x2A;
    /// Texture table entries scanned for a handle's name.
    pub const TEXTURE_TABLE_SCAN: u32 = 0x1000;
}

/// Percent weights of `0x66310`: medal 1..4 (`0x4D2884`) and difficulty (`0x4D2898`), the medal
/// share 60 (`0x4760AC`) over 48 results (`0x4788C0` = 1/48), the story share 40 (`0x4788B8`) over
/// 13 levels (`0x4788BC` = 1/13), clamped to 0..100 (`0x4785C8`).
pub const MEDAL_WEIGHTS: [f32; 4] = [0.4, 0.7, 1.0, 1.0];
pub const DIFFICULTY_WEIGHTS: [f32; 3] = [0.2, 0.3, 0.5];
use addr as a;

fn f32_at(g: &dyn GuestUi, va: u32) -> f32 {
    f32::from_bits(g.read32(va).unwrap_or(0))
}

fn bytes_at(g: &dyn GuestUi, mut va: u32) -> Option<String> {
    let mut out = Vec::new();
    while out.len() < 64 {
        let word = g.read32(va & !3)?;
        let byte = (word >> ((va & 3) * 8)) as u8;
        if byte == 0 {
            break;
        }
        out.push(byte);
        va += 1;
    }
    (!out.is_empty()).then(|| String::from_utf8_lossy(&out).into_owned())
}

/// The table name of texture table entry `index` (`0x80F20`), None for -1 or unreadable.
pub fn texture_name(g: &dyn GuestUi, index: i32) -> Option<String> {
    if !(0..0x1_0000).contains(&index) {
        return None;
    }
    let pointer = g.read32(a::TEXTURES + a::TEXTURE_STRIDE * index as u32 - 8)?;
    if pointer == 0 {
        return None;
    }
    bytes_at(g, pointer)
}

/// The results base of `profile` (`0x69900`): the profile's results when it is loaded and the
/// mode bit is clear, else the default results.
pub fn results_base(g: &dyn GuestUi, profile: u32) -> u32 {
    let mode = g.read32(a::RESULT_MODE).unwrap_or(0);
    let offset = profile.wrapping_mul(a::PROFILE_STRIDE);
    if mode & a::RESULT_MODE_BIT == 0
        && g.read32(a::PROFILE_LOADED.wrapping_add(offset))
            .unwrap_or(0)
            != 0
    {
        a::RESULTS_PROFILE.wrapping_add(offset)
    } else {
        a::RESULTS_DEFAULT
    }
}

fn u16_at(g: &dyn GuestUi, va: u32) -> u32 {
    g.read32(va & !3)
        .map_or(0, |w| (w >> ((va & 3) * 8)) & 0xFFFF)
}

/// The Player Progress numbers of `profile` (the getters `0x660C0..0x66310`).
pub fn read_stats(g: &dyn GuestUi, profile: u32) -> ProgressStats {
    let base = results_base(g, profile);
    let word = |players: u32, difficulty: u32| {
        g.read32(base + a::STORY_WORDS + 12 * players + 4 * difficulty)
            .unwrap_or(0)
    };
    let mask = (1u32 << a::LEVEL_COUNT) - 1;
    let mut story = [[0u32; 3]; 2];
    let mut levels_done = [0u32; 2];
    for players in 0..2u32 {
        let mut any = 0;
        for difficulty in 0..3u32 {
            let bits = word(players, difficulty) & mask;
            story[players as usize][difficulty as usize] = bits.count_ones();
            any |= bits;
        }
        levels_done[players as usize] = any.count_ones();
    }
    let medal = |index: u32| g.read32(base + a::RESULT_MEDALS + 24 * index).unwrap_or(0) as i32;
    let won = |range: std::ops::Range<u32>| range.filter(|i| medal(*i) != 0).count() as u32;
    // `0x66310`: the medal share, then the story share, single precision in the original's order.
    let mut medals = 0.0f32;
    for (m, weight) in MEDAL_WEIGHTS.iter().enumerate() {
        let count = (0..48).filter(|i| medal(*i) == m as i32 + 1).count();
        medals += weight * (count as f32 * (1.0 / 48.0));
    }
    let mut story_share = 0.0f32;
    for (d, weight) in DIFFICULTY_WEIGHTS.iter().enumerate() {
        let either = (word(0, d as u32) | word(1, d as u32)) & mask;
        story_share += weight * (either.count_ones() as f32 * (1.0 / 13.0));
    }
    let complete = (story_share * 40.0 + medals * 60.0).clamp(0.0, 100.0);
    ProgressStats {
        story,
        levels_done,
        levels: a::LEVEL_COUNT,
        challenges_done: won(0..a::CHALLENGE_COUNT),
        challenges: a::CHALLENGE_COUNT,
        leagues_done: won(a::CHALLENGE_COUNT..a::CHALLENGE_COUNT + a::LEAGUE_COUNT),
        leagues: a::LEAGUE_COUNT,
        complete,
    }
}

/// A texture of the slot table `[0x7BA040]` (`0x81220` loads slot `n` from
/// `textures/misc/mm_cursor<n + 1>.raw`, not through the named texture table): its D3D header
/// (Common, Data, Lock, Format, Size at the start of the 0x34-byte texture record, MEASURED live:
/// slot 0x16 is a 16 x 16 DXT3 texture) and size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotTexture {
    pub slot: u32,
    pub handle: u32,
    pub width: u32,
    pub height: u32,
    /// D3D `Data` (synthetic physical address of the texels), `Format` and `Size` words.
    pub data: u32,
    pub format: u32,
    pub size: u32,
}

impl SlotTexture {
    /// The key the drawer and the host use for this texture's pixels.
    pub fn key(&self) -> String {
        format!("slot:{}", self.slot)
    }
}

/// The texture in slot `slot` (`0x81C20` / `0x81C60`: handle `[0x7BA040 + slot * 4]`).
pub fn slot_texture(g: &dyn GuestUi, slot: u32) -> Option<SlotTexture> {
    let handle = g.read32(a::TEXTURE_SLOTS + 4 * slot)?;
    handle_texture(g, handle, slot)
}

/// T1916: the texture of a texture-table `handle` (the record `0x81C60` binds); `slot` only names
/// it ([`SlotTexture::key`]), e.g. the Map Maker pointer of `page +0x4C`.
pub fn handle_texture(g: &dyn GuestUi, handle: u32, slot: u32) -> Option<SlotTexture> {
    let records = g.read32(g.read32(a::TEXTURE_RECORDS)?)?;
    let record = records.wrapping_add(handle.wrapping_mul(a::TEXTURE_RECORD_STRIDE));
    let (width, height) = (
        u16_at(g, record + a::TEXTURE_WIDTH),
        u16_at(g, record + a::TEXTURE_HEIGHT),
    );
    (handle != u32::MAX && width > 0 && height > 0).then_some(SlotTexture {
        slot,
        handle,
        width,
        height,
        data: g.read32(record + 4)?,
        format: g.read32(record + 0xC)?,
        size: g.read32(record + 0x10)?,
    })
}

/// Text style globals of `0x3FA10` / `0x3F8D0`: style entries `0x4C8DE8` (20 bytes: font index,
/// x scale, y scale at `+0xC`), font records `0x4C8C28` (0x38 bytes, cell height `+0x18`; the
/// alternative font set `[0x6B93A8] != 0` reads `0x4C8C40 + font * 0x38`), and the run-time
/// factors `[0x4E8CF4]` (int), `0x478390`, `[0x7E7274]`, `[0x7E7280]`.
pub mod style_addr {
    pub const STYLES: u32 = 0x4C_8DE8;
    pub const STYLE_STRIDE: u32 = 20;
    pub const STYLE_SCALE_Y: u32 = 0xC;
    pub const FONTS: u32 = 0x4C_8C28;
    pub const FONT_STRIDE: u32 = 0x38;
    pub const FONT_HEIGHT: u32 = 0x18;
    pub const ALT_FONT_SET: u32 = 0x6B_93A8;
    pub const ALT_FONT_HEIGHTS: u32 = 0x4C_8C40;
    pub const SCREEN_LINES: u32 = 0x4E_8CF4;
    pub const SCREEN_FACTOR: u32 = 0x47_8390;
    pub const HEIGHT_FACTOR: u32 = 0x7E_7274;
    pub const STYLE_FACTOR: u32 = 0x7E_7280;
}

/// The line advance `[0x7E7264]` that `0x3F8D0` computes when text style `style` is selected (the
/// summary row draws in its row's style, which the frame boundary does not have active):
/// `font height * [0x7E7274] * (float([0x4E8CF4]) * [0x478390] * style y scale * [0x7E7280])`.
pub fn line_advance(g: &dyn GuestUi, style: u32) -> f32 {
    use style_addr as s;
    let entry = s::STYLES.wrapping_add(style.wrapping_mul(s::STYLE_STRIDE));
    let font = g.read32(entry).unwrap_or(0);
    let screen = g.read32(s::SCREEN_LINES).unwrap_or(0) as i32 as f32;
    let scale = screen
        * f32_at(g, s::SCREEN_FACTOR)
        * f32_at(g, entry + s::STYLE_SCALE_Y)
        * f32_at(g, s::STYLE_FACTOR);
    let height = if g.read32(s::ALT_FONT_SET).unwrap_or(0) != 0 {
        f32_at(
            g,
            s::ALT_FONT_HEIGHTS.wrapping_add(font.wrapping_mul(s::FONT_STRIDE)),
        )
    } else {
        f32_at(
            g,
            s::FONTS.wrapping_add(font.wrapping_mul(s::FONT_STRIDE)) + s::FONT_HEIGHT,
        )
    };
    height * f32_at(g, s::HEIGHT_FACTOR) * scale
}

/// The challenge set record of set `id` (`0x22E070`).
pub fn set_record(id: u32) -> u32 {
    if (id as i32) < a::SETS_LOW_COUNT as i32 {
        a::SETS.wrapping_add(id.wrapping_mul(a::SET_STRIDE))
    } else {
        a::SETS_HIGH + (id - a::SETS_LOW_COUNT) * a::SET_STRIDE
    }
}

/// The challenge record of challenge `id` (`0x22E050`), 0 for none.
pub fn challenge_record(g: &dyn GuestUi, id: u32) -> u32 {
    let table = if (id as i32) < a::CHALLENGES_LOW_COUNT as i32 {
        a::CHALLENGES
    } else {
        a::CHALLENGES_HIGH
    };
    g.read32(table.wrapping_add(id.wrapping_mul(4)))
        .unwrap_or(0)
}

/// The medal of challenge `id` for profile 0 (`0x69930(0, id) + 0xC`).
pub fn medal_of(g: &dyn GuestUi, id: u32) -> i32 {
    let record = results_base(g, 0) + a::RESULTS_OFFSET + id.wrapping_mul(a::RESULT_STRIDE);
    g.read32(record + a::RESULT_MEDAL).unwrap_or(0) as i32
}

/// Whether challenge record `record` is shown (`0x22DF10` unlock bit, not hidden).
fn challenge_shown(g: &dyn GuestUi, record: u32) -> bool {
    let bit = g.read32(a::UNLOCK_BIT).unwrap_or(0) & 31;
    let unlocked = g.read32(record + a::CHALLENGE_UNLOCK_BITS).unwrap_or(0) & (1 << bit) != 0;
    let hidden = g.read32(record + a::CHALLENGE_FLAGS).unwrap_or(0) & a::CHALLENGE_HIDDEN != 0;
    unlocked && !hidden
}

fn icon(g: &dyn GuestUi, index: u32) -> Option<RowIcon> {
    let index = index as i32;
    (index != -1)
        .then(|| texture_name(g, index))
        .flatten()
        .map(|texture| RowIcon {
            texture,
            locked_texture: None,
        })
}

/// T2315 item 4: the scroll arrows of page `record` as `0x73FC0` / `0x73E40` place them (see
/// [`tsfp_ui_model::list_rows::ScrollArrows`]). None when the page style's first word lacks bit 0x20 (the other path
/// `[style + 0xAC]` / `0x727F0`, not decoded), the page is hidden (`+0x40`) or the list does not scroll.
pub fn read_scroll_arrows(g: &dyn GuestUi, record: u32) -> Option<ScrollArrows> {
    let style = g.read32(record + page::STYLE).filter(|s| *s != 0)?;
    if g.read32(style).unwrap_or(0) & a::STYLE_ARROWS == 0
        || g.read32(record + a::PAGE_HIDDEN).unwrap_or(0) & a::PAGE_HIDDEN_BIT != 0
    {
        return None;
    }
    let field = |offset: u32| f32_at(g, record + offset);
    let (content, view_top, view_bottom, scroll) = (
        field(a::PAGE_CONTENT),
        field(a::PAGE_VIEW_TOP),
        field(a::PAGE_VIEW_BOTTOM),
        field(a::PAGE_SCROLL),
    );
    let threshold = f32_at(g, a::ARROW_THRESHOLD);
    let up = scroll > threshold;
    let down = content - (view_bottom - view_top) - scroll > threshold;
    if !(up || down) {
        return None;
    }
    let unit = f32_at(g, a::MEDAL_UNIT);
    let origin = field(a::PAGE_Y);
    let (top, bottom) = (view_top * unit + origin, view_bottom * unit + origin);
    let x = (field(a::PAGE_RIGHT) - f32_at(g, a::ARROW_RIGHT) - f32_at(g, a::ARROW_INSET)).trunc();
    let x1 = x + a::ARROW_WIDTH;
    Some(ScrollArrows {
        texture: texture_name(g, SCROLL_ARROW_TEXTURE as i32),
        up: up.then(|| {
            Edges::new(
                x,
                top.trunc(),
                x1,
                (top + f32_at(g, a::ARROW_UP_HEIGHT)).trunc(),
            )
        }),
        down: down.then(|| {
            Edges::new(
                x,
                (bottom - f32_at(g, a::ARROW_INSET)).trunc(),
                x1,
                (bottom - f32_at(g, a::ARROW_DOWN_END)).trunc(),
            )
        }),
        alpha_scale: f32_at(g, a::ARROW_ALPHA),
    })
}

/// T2315 item 4: the vertical band the original lets the scrolled rows of a page draw into, from the rows'
/// cut layout boxes ([`LiveRow::clip`]) as `0x722B0` turns them into viewports: top `trunc(box top)` (4 higher with
/// row flag [`a::ROW_FLAG_CLIP_LIFT`]), bottom `trunc(min(box bottom + 3, page bottom) - 1)`. The union over the
/// rows is the list view (the boxes of fully shown rows lie inside it). None when no row is a scrolled-list row.
pub fn view_band(rows: &[LiveRow], page_bottom: f32) -> Option<(f32, f32)> {
    rows.iter()
        .filter_map(|r| {
            let [_, y0, _, y1] = r.clip?;
            let lift = if r.flags & a::ROW_FLAG_CLIP_LIFT != 0 {
                4.0
            } else {
                0.0
            };
            let bottom = if page_bottom.is_finite() && page_bottom > 0.0 {
                (y1 + 3.0).min(page_bottom)
            } else {
                y1 + 3.0
            };
            Some(((y0 - lift).trunc(), (bottom - 1.0).trunc()))
        })
        .reduce(|(t0, b0), (t1, b1)| (t0.min(t1), b0.max(b1)))
}

/// The list rows of `live` (rows whose draw callback is `0x2CA9F0` or `0x2C8B50`), None when it has
/// none. `slot` is the page slot (its record is the page object the row draws receive).
pub fn read_list_rows(g: &dyn GuestUi, live: &LivePage, slot: u32) -> Option<ListRows> {
    read_list_rows_with_progress(g, live, slot, None)
}

pub fn read_list_rows_with_progress(
    g: &dyn GuestUi,
    live: &LivePage,
    slot: u32,
    samples: Option<&crate::ingame_live::ProgressSamples>,
) -> Option<ListRows> {
    let record = PAGE_TABLE + slot * PAGE_STRIDE;
    let mut rows = Vec::new();
    for (index, r) in live.rows.iter().enumerate() {
        if r.draw != LEVEL_ROW_DRAW && r.draw != CHALLENGE_ROW_DRAW {
            continue;
        }
        let Some([x0, y0, x1, y1]) = r.rect else {
            continue;
        };
        let id = r.id as u32;
        let kind = if r.draw == LEVEL_ROW_DRAW {
            let level = a::LEVELS.wrapping_add(id.wrapping_mul(a::LEVEL_STRIDE));
            // The i16 at `+0x2` (LEVEL_NAME) is the high half of the aligned word at `+0x0`.
            let name = g
                .read32(level + a::LEVEL_NAME - 2)
                .map(|w| ((w >> 16) as i16) as i32)
                .filter(|n| *n > 0)
                .map(|n| n as u32);
            ListRowKind::Level {
                name,
                icon: icon(g, g.read32(level + a::LEVEL_ICON).unwrap_or(u32::MAX)),
            }
        } else {
            let set = set_record(id);
            let count = g.read32(set + a::SET_COUNT).unwrap_or(0).min(32);
            let mut medals = Vec::new();
            for n in 0..count {
                let challenge = g.read32(set + a::SET_IDS + 4 * n).unwrap_or(0);
                let rec = challenge_record(g, challenge);
                if rec == 0 || !challenge_shown(g, rec) {
                    continue;
                }
                let medal = medal_of(g, challenge);
                let texture = (medal > 0)
                    .then(|| g.read32(a::MEDAL_TEXTURES + 4 * medal.min(4) as u32))
                    .flatten()
                    .and_then(|t| texture_name(g, t as i32));
                medals.push(Medal { medal, texture });
            }
            ListRowKind::ChallengeSet {
                medals,
                icon: icon(g, g.read32(set + a::SET_ICON).unwrap_or(u32::MAX)),
            }
        };
        rows.push(ListRow {
            widget: crate::live_page::widget_id(index),
            area: Edges::new(x0, y0, x1, y1),
            locked: r.flags & row::FLAG_GREYED != 0,
            brightness: r.brightness,
            row_height: r.height,
            kind,
        });
    }
    let summary = live
        .rows
        .iter()
        .enumerate()
        .find(|(_, r)| r.draw == PROGRESS_SUMMARY_DRAW)
        .and_then(|(index, r)| {
            let [x0, y0, x1, y1] = r.rect?;
            // The row draw callback runs in the text style the default row drawer selected last
            // (the summary's own `+0x1C` is not selected for it): the font of the nearest earlier
            // row without a draw callback. INFERRED from the legacy picture (line advance 22 = style
            // 4 of the rows above, 19 with the summary's style 3; glyphs 31 px tall, not 27).
            let style = live.rows[..index]
                .iter()
                .rev()
                .find(|p| p.draw == 0)
                .map_or(r.font, |p| p.font);
            let margins = g.read32(record + a::PAGE_MARGINS).unwrap_or(0);
            Some(ProgressSummary {
                widget: crate::live_page::widget_id(index),
                area: Edges::new(x0, y0, x1, y1),
                margin_left: if margins != 0 {
                    f32_at(g, margins)
                } else {
                    0.0
                },
                margin_right: if margins != 0 {
                    f32_at(g, margins + 8)
                } else {
                    0.0
                },
                text_unit: f32_at(g, a::TEXT_UNIT),
                line_unit: f32_at(g, a::MEDAL_UNIT),
                line_height: line_advance(g, style).trunc(),
                font: style,
                stats: read_stats(g, g.read32(a::PROGRESS_PROFILE).unwrap_or(0)),
                icon: slot_texture(g, PLAYER_ICON_SLOT).map(|t| PlayerIcon {
                    texture: t.key(),
                    width: t.width as f32,
                    height: t.height as f32,
                }),
            })
        });
    let options = read_option_rows(g, live, slot);
    let ingame = crate::ingame_live::read_with_progress(g, live, samples);
    if rows.is_empty()
        && summary.is_none()
        && options.is_none()
        && ingame.is_none()
        && read_scroll_arrows(g, record).is_none()
    {
        return None;
    }
    let style = g.read32(record + page::STYLE).unwrap_or(0);
    Some(ListRows {
        rows,
        text_unit: f32_at(g, a::TEXT_UNIT),
        medal_unit: f32_at(g, a::MEDAL_UNIT),
        text_margin: if style != 0 {
            f32_at(g, style + a::STYLE_TEXT_MARGIN)
        } else {
            0.0
        },
        plain_alpha: g.read32(record + a::PAGE_CLASS) == Some(a::PLAIN_PAGE_CLASS),
        // The original page layout leaves rows outside the view without a layout box (they are
        // not drawn, MEASURED live) and cuts the box of a row partly inside the view; the row is
        // drawn at its full geometry with the viewport set from that box (`0x722B0`, T2315).
        clip: view_band(&live.rows, f32_at(g, record + a::PAGE_BOTTOM)),
        padlock: texture_name(g, PADLOCK_TEXTURE as i32),
        glow: texture_name(g, MEDAL_GLOW_TEXTURE as i32),
        summary,
        arrows: read_scroll_arrows(g, record),
        images: Vec::new(),
        options,
        ingame,
    })
}

/// T2315 increment 3: guest addresses of the Audio/Video draws (`0x2C4190`, `0x2C3500`, `0x814B0`, `0x2C3410`), MEASURED
/// by static reading.
pub mod av {
    /// Slider values (ints in 1/16): `[0x5236BC]` for row id 1, `[0x5236C0]` for the other slider rows; `[0x475DC4]`
    /// (1/16) the scale.
    pub const VALUE_ROW1: u32 = 0x52_36BC;
    pub const VALUE_OTHER: u32 = 0x52_36C0;
    pub const VALUE_SCALE: u32 = 0x47_5DC4;
    /// `[0x49F470]` (0.31): the strength of a greyed slider row.
    pub const GREYED_STRENGTH: u32 = 0x49_F470;
    /// Page state `+0x18C`: the cursor row id, 4 in the screen position mode (the page draw's frame).
    pub const PAGE_STATE: u32 = 0x18C;
    pub const SCREEN_POSITION_STATE: u32 = 4;
    /// The bar texture handle `[0x4D45AC]` (`0x814B0`).
    pub const BAR_TEXTURE: u32 = 0x4D_45AC;
    /// The safe area record pointer `[0x7E71C8]` (ints `+0x0` left, `+0x4` right, `+0x14` top, `+0x18` bottom) and the
    /// frame line alpha byte `[0x7BA034]`.
    pub const SAFE_AREA: u32 = 0x7E_71C8;
    pub const SAFE_ALPHA: u32 = 0x7B_A034;
    /// The pulse `0x2E6430`: period `[0x4E8CEC]` (int frames), step `[0x475C94] * [0x475CE0]`, amplitude `[0x4765E0]`,
    /// base `[0x475CD4]`.
    pub const PULSE_PERIOD: u32 = 0x4E_8CEC;
    pub const PULSE_STEP_A: u32 = 0x47_5C94;
    pub const PULSE_STEP_B: u32 = 0x47_5CE0;
    pub const PULSE_AMPLITUDE: u32 = 0x47_65E0;
    pub const PULSE_BASE: u32 = 0x47_5CD4;
    /// The side arrow texture handle `[0x4D6C7C]` (`0x81AD0` / `0x81B20`).
    pub const ARROW_TEXTURE: u32 = 0x4D_6C7C;
    /// Page `+0xC8`: bit 4 lights the left arrow, bit 8 the right one (state 2 of `0x7B7B0`).
    pub const PAGE_ARROW_BITS: u32 = 0xC8;
    pub const LEFT_ARROW_LIT: u32 = 4;
    pub const RIGHT_ARROW_LIT: u32 = 8;
    /// Row colour table pointer `+0x90`; `0x70D10(row, state)` reads word `10 + state` of it.
    pub const ROW_COLOURS: u32 = 0x90;
    pub const ARROW_COLOUR_WORD: u32 = 10;
    /// Controls `0x2EA080`: the picture string `[0x478220]` (0x0B and a digit '1'..'<'), the picture handle table
    /// `[0x7E72A0 + (digit - '1') * 4]`, the text mode word `[0x79094C]` (above 100: style 1 and scale 1.5; 0x66: the
    /// picture is 20 units).
    pub const PICTURE_STRING: u32 = 0x47_8220;
    pub const PICTURE_HANDLES: u32 = 0x7E_72A0;
    pub const TEXT_MODE: u32 = 0x79_094C;
    /// Controls `0x2EA250`: turn speed `[0x7BA3DC + [0x7BA9E0] * 0x1E0]` (f32).
    pub const TURN_SPEED: u32 = 0x7B_A3DC;
    pub const SETTINGS_INDEX: u32 = 0x7B_A9E0;
    pub const SETTINGS_STRIDE: u32 = 0x1E0;
}

/// T2315: the guest key of a texture handle not in the named table (`ui_pages` decodes it from texture memory).
pub fn handle_key_slot(handle: u32) -> u32 {
    0x1_0000 + handle
}

/// Controls `0x2EA080`: the picture string `[0x478220]` (read up to its NUL, at most 32 bytes), the texture of every
/// picture slot it names (`[0x7E72A0 + slot * 4]`, T2316 inline escapes) and the style height. None without a picture.
fn button_picture(g: &dyn GuestUi, area: Edges, cursor: bool) -> Option<ButtonPicture> {
    let mut text = Vec::new();
    for at in 0..PICTURE_STRING_MAX {
        let word = g.read32((av::PICTURE_STRING + at) & !3)?;
        let byte = (word >> (8 * ((av::PICTURE_STRING + at) & 3))) as u8;
        if byte == 0 {
            break;
        }
        text.push(byte);
    }
    let mut textures = Vec::new();
    for segment in tsfp_ui_model::font::split_pictures(&text) {
        let tsfp_ui_model::font::Segment::Picture(slot) = segment else {
            continue;
        };
        if textures.iter().any(|(s, _)| *s == slot) {
            continue;
        }
        let handle = g.read32(av::PICTURE_HANDLES + 4 * u32::from(slot))?;
        let texture = match handle_named_texture(g, handle) {
            Some(named) => named.name,
            None => handle_texture(g, handle, handle_key_slot(handle))?.key(),
        };
        textures.push((slot, texture));
    }
    if textures.is_empty() {
        return None;
    }
    let mode = g.read32(av::TEXT_MODE).unwrap_or(0) as i32;
    let (font, scale) = if mode > 100 { (1, 1.5) } else { (3, 2.0) };
    let height = crate::live_page::style_height(g, font)?;
    let size = if mode == 0x66 { 20.0 } else { height };
    Some(ButtonPicture {
        area,
        text,
        font,
        textures,
        size,
        height,
        scale,
        cursor,
    })
}

/// The longest picture string read from `[0x478220]`.
const PICTURE_STRING_MAX: u32 = 32;

/// The texture table name and size of texture table entry `index` (the handle `0x80F20` returns, then its record).
pub fn table_texture(g: &dyn GuestUi, index: i32) -> Option<NamedTexture> {
    let name = texture_name(g, index)?;
    let handle = g.read32(a::TEXTURES + a::TEXTURE_STRIDE * index as u32)?;
    let t = handle_texture(g, handle, 0)?;
    Some(NamedTexture {
        name,
        width: t.width as f32,
        height: t.height as f32,
    })
}

/// The texture table name and size of a texture `handle` (the table scanned for the entry that holds it).
pub fn handle_named_texture(g: &dyn GuestUi, handle: u32) -> Option<NamedTexture> {
    if handle == u32::MAX {
        return None;
    }
    let index = (0..a::TEXTURE_TABLE_SCAN)
        .find(|i| g.read32(a::TEXTURES + a::TEXTURE_STRIDE * i) == Some(handle))?;
    table_texture(g, index as i32)
}

/// T2315 increment 3: the Audio/Video rows (`0x2C3500` slider rows, `0x2C3410` icon rows) and the page draw `0x2C4190`
/// of page `slot`, None when the page has neither.
pub fn read_option_rows(g: &dyn GuestUi, live: &LivePage, slot: u32) -> Option<OptionRows> {
    let record = PAGE_TABLE + slot * PAGE_STRIDE;
    let page_draw = g.read32(record + 0x10).unwrap_or(0);
    let state = g.read32(record + av::PAGE_STATE).unwrap_or(u32::MAX);
    let mut rows = Vec::new();
    for r in &live.rows {
        if r.draw != SLIDER_ROW_DRAW && r.draw != ICON_ROW_DRAW {
            continue;
        }
        let Some([x0, y0, x1, y1]) = r.rect else {
            continue;
        };
        let slider = (r.draw == SLIDER_ROW_DRAW).then(|| {
            let source = if r.id == 1 {
                av::VALUE_ROW1
            } else {
                av::VALUE_OTHER
            };
            let raw = g.read32(source).unwrap_or(0) as i32;
            Slider {
                value: raw as f32 * f32_at(g, av::VALUE_SCALE),
                greyed: r.flags & row::FLAG_GREYED != 0,
                cursor: state == r.id as u32,
            }
        });
        let icon = ICON_TEXTURES
            .iter()
            .find(|(id, _)| *id == r.id)
            .and_then(|(_, index)| table_texture(g, *index as i32));
        rows.push(OptionRow {
            area: Edges::new(x0, y0, x1, y1),
            slider,
            icon,
        });
    }
    // `0x7B7B0` side arrows of the kind-1 value rows (row `+0xB8` bit 0x400000 clear).
    let lit = g.read32(record + av::PAGE_ARROW_BITS).unwrap_or(0);
    let values: Vec<ValueArrows> = live
        .rows
        .iter()
        .enumerate()
        .filter_map(|(index, r)| {
            let value = r.value.filter(|v| v.arrows)?;
            let (left, right) = if r.flags & row::FLAG_GREYED != 0 {
                (3, 3)
            } else if live.cursor_node == r.node {
                (
                    1 + u32::from(lit & av::LEFT_ARROW_LIT != 0),
                    1 + u32::from(lit & av::RIGHT_ARROW_LIT != 0),
                )
            } else {
                (0, 0)
            };
            let table = g.read32(r.node + av::ROW_COLOURS).filter(|t| *t != 0)?;
            let colour = |state: u32| {
                g.read32(table + 4 * (av::ARROW_COLOUR_WORD + state))
                    .unwrap_or(0)
            };
            Some(ValueArrows {
                widget: crate::live_page::widget_id(index),
                anchor: value.anchor,
                y: value.y,
                box_x: value.box_x,
                mode: value.mode,
                height: value.height,
                left: colour(left),
                right: colour(right),
            })
        })
        .collect();
    let mut pictures = Vec::new();
    let mut text_sliders = Vec::new();
    for (index, r) in live.rows.iter().enumerate() {
        let Some([x0, y0, x1, y1]) = r.rect else {
            continue;
        };
        let area = Edges::new(x0, y0, x1, y1);
        if r.draw == BUTTON_ROW_DRAW {
            pictures.extend(button_picture(g, area, live.cursor_node == r.node));
        } else if r.draw == TURN_SPEED_ROW_DRAW {
            let index_word = g.read32(av::SETTINGS_INDEX).unwrap_or(0);
            let value = f32_at(
                g,
                av::TURN_SPEED.wrapping_add(index_word.wrapping_mul(av::SETTINGS_STRIDE)),
            );
            text_sliders.push(TextSlider {
                widget: crate::live_page::widget_id(index),
                area,
                value: if value.is_finite() { value } else { 0.0 },
                cursor: state == r.id as u32,
                text_height: crate::live_page::style_height(g, r.font).unwrap_or(0.0),
            });
        }
    }
    if rows.is_empty()
        && values.is_empty()
        && pictures.is_empty()
        && text_sliders.is_empty()
        && page_draw != AV_PAGE_DRAW
    {
        return None;
    }
    let safe_frame = (page_draw == AV_PAGE_DRAW && state == av::SCREEN_POSITION_STATE)
        .then(|| {
            let area = g.read32(av::SAFE_AREA)?;
            let int = |offset: u32| g.read32(area + offset).map(|v| v as i32);
            Some(SafeFrame {
                left: int(0)? + 5,
                right: int(4)? - 5,
                top: int(0x14)? + 5,
                bottom: int(0x18)? - 5,
                alpha: g
                    .read32(av::SAFE_ALPHA & !3)
                    .map_or(0, |w| (w >> ((av::SAFE_ALPHA & 3) * 8)) & 0xFF),
            })
        })
        .flatten();
    let period_word = g.read32(av::PULSE_PERIOD).unwrap_or(0) as i32;
    let step = f32_at(g, av::PULSE_STEP_A) * f32_at(g, av::PULSE_STEP_B);
    Some(OptionRows {
        rows,
        bar: g
            .read32(av::BAR_TEXTURE)
            .and_then(|h| handle_named_texture(g, h)),
        greyed_strength: f32_at(g, av::GREYED_STRENGTH),
        pulse: Pulse {
            base: f32_at(g, av::PULSE_BASE),
            amplitude: f32_at(g, av::PULSE_AMPLITUDE),
            period: if period_word > 0 && step > 0.0 {
                2048.0 * period_word as f32 / step
            } else {
                0.0
            },
        },
        safe_frame,
        arrow: (!values.is_empty())
            .then(|| {
                g.read32(av::ARROW_TEXTURE)
                    .and_then(|h| handle_named_texture(g, h))
            })
            .flatten(),
        values,
        pictures,
        text_sliders,
    })
}

#[cfg(test)]
mod tests {
    //! Tier 0 on FABRICATED guest memory (no game data).
    use super::*;
    use crate::live_page::{LiveRow, LiveText};
    use std::collections::HashMap;

    #[derive(Default)]
    struct Mem(HashMap<u32, u32>);
    impl GuestUi for Mem {
        fn read32(&self, va: u32) -> Option<u32> {
            Some(self.0.get(&va).copied().unwrap_or(0))
        }
        fn write32(&mut self, _: u32, _: u32) -> bool {
            false
        }
        fn call(&mut self, _: u32, _: &[u32]) -> Option<u32> {
            None
        }
    }
    impl Mem {
        fn name(&mut self, index: u32, at: u32, text: &[u8; 4]) {
            self.0
                .insert(a::TEXTURES + a::TEXTURE_STRIDE * index - 8, at);
            self.0.insert(at, u32::from_le_bytes(*text));
            self.0.insert(at + 4, 0);
        }
    }

    /// FABRICATED rodata (the real values are read live, docs/t2315-ui-page-completion.md item 4).
    #[test]
    fn scroll_arrows_follow_the_scroll_offset_and_flip_the_down_arrow() {
        let mut m = Mem::default();
        let record = PAGE_TABLE + 2 * PAGE_STRIDE;
        let f = |m: &mut Mem, va: u32, v: f32| {
            m.0.insert(va, v.to_bits());
        };
        m.0.insert(record + page::STYLE, 0x9000);
        m.0.insert(0x9000, 0x5060);
        f(&mut m, a::MEDAL_UNIT, 4.8);
        for (offset, v) in [
            (a::PAGE_CONTENT, 253.0),
            (a::PAGE_VIEW_TOP, 2.2),
            (a::PAGE_VIEW_BOTTOM, 59.2),
            (a::PAGE_Y, 105.72),
            (a::PAGE_RIGHT, 560.0),
        ] {
            f(&mut m, record + offset, v);
        }
        for (va, v) in [
            (a::ARROW_RIGHT, 20.0),
            (a::ARROW_INSET, 4.0),
            (a::ARROW_UP_HEIGHT, 14.0),
            (a::ARROW_DOWN_END, 18.0),
            (a::ARROW_ALPHA, 127.0),
        ] {
            f(&mut m, va, v);
        }
        m.name(SCROLL_ARROW_TEXTURE, 0x8000, b"arw\0");
        let at_top = read_scroll_arrows(&m, record).expect("the list scrolls");
        assert_eq!(at_top.up, None, "no up arrow at offset 0");
        let down = at_top.down.unwrap();
        assert_eq!((down.x0, down.x1), (536.0, 550.0));
        assert!(
            down.y0 > down.y1,
            "the down arrow is drawn upside down: {down:?}"
        );
        assert_eq!(at_top.texture.as_deref(), Some("arw"));
        f(&mut m, record + a::PAGE_SCROLL, 196.0);
        let at_bottom = read_scroll_arrows(&m, record).unwrap();
        assert!(at_bottom.up.is_some() && at_bottom.down.is_none());
        m.0.insert(0x9000, 0x40);
        assert_eq!(
            read_scroll_arrows(&m, record),
            None,
            "the other style path draws no arrows here"
        );
    }

    #[test]
    fn view_band_is_the_union_of_the_row_viewports() {
        let mut top = row(1, LEVEL_ROW_DRAW, 0);
        top.clip = Some([86.4, 116.72, 553.6, 207.48]);
        let mut cut = row(2, LEVEL_ROW_DRAW, 0);
        cut.clip = Some([86.4, 303.48, 553.6, 389.72]);
        assert_eq!(
            view_band(&[top.clone(), cut.clone()], 399.48),
            Some((116.0, 391.0))
        );
        assert_eq!(
            view_band(&[cut.clone()], 390.0),
            Some((303.0, 389.0)),
            "page bottom bounds the viewport"
        );
        let mut lifted = top.clone();
        lifted.flags = a::ROW_FLAG_CLIP_LIFT;
        assert_eq!(view_band(&[lifted], 399.48), Some((112.0, 209.0)));
        assert_eq!(
            view_band(&[row(3, 0, 0)], 399.48),
            None,
            "no scrolled rows, no band"
        );
    }

    fn row(id: i32, draw: u32, flags: u32) -> LiveRow {
        LiveRow {
            node: 0x100 + id as u32,
            node_type: 7,
            id,
            flags,
            text: LiveText::None,
            font: 3,
            value_font: 0,
            align: 0,
            rect: Some([
                100.0,
                100.0 + 20.0 * id as f32,
                500.0,
                119.0 + 20.0 * id as f32,
            ]),
            clip: None,
            value: None,
            activate: 0x10,
            draw,
            kind: 0,
            selected: 0,
            options: Vec::new(),
            brightness: 0.5,
            height: 19.0,
            text_align: 0,
            text_x: 0.0,
            text_y: 0.0,
        }
    }

    #[test]
    fn level_rows_read_name_icon_and_lock() {
        let mut m = Mem::default();
        let level = a::LEVELS + 2 * a::LEVEL_STRIDE;
        m.0.insert(level, 0x1706_0000);
        m.0.insert(level + a::LEVEL_ICON, 9);
        m.name(9, 0x9000, b"lvl9");
        m.name(PADLOCK_TEXTURE, 0x9100, b"lock");
        let live = LivePage {
            slot: 2,
            builder: 0x2C_B090,
            state: 2,
            cursor_node: 0,
            rows: vec![row(0, 0, 0), row(2, LEVEL_ROW_DRAW, row::FLAG_GREYED)],
            units: (6.4, 4.8),
        };
        let rows = read_list_rows(&m, &live, 2).unwrap();
        assert_eq!(rows.rows.len(), 1);
        let r = &rows.rows[0];
        assert_eq!(r.widget, WidgetId(1));
        assert!(r.locked);
        assert_eq!(
            r.kind,
            ListRowKind::Level {
                name: Some(0x1706),
                icon: Some(RowIcon {
                    texture: "lvl9".into(),
                    locked_texture: None
                })
            }
        );
        assert_eq!(rows.padlock.as_deref(), Some("lock"));
        assert_eq!(rows.clip, None, "not in a scrolled list");
    }

    #[test]
    fn progress_stats_count_bits_medals_and_weigh_the_percent() {
        let mut m = Mem::default();
        let base = a::RESULTS_DEFAULT;
        // One player: levels 0 and 1 on Easy, level 0 on Hard; two players: level 5 on Normal.
        m.0.insert(base + a::STORY_WORDS, 0b11);
        m.0.insert(base + a::STORY_WORDS + 8, 0b1);
        m.0.insert(base + a::STORY_WORDS + 12 + 4, 1 << 5);
        // Bits above the 13 levels are ignored.
        m.0.insert(base + a::STORY_WORDS + 4, 1 << 20);
        // Challenge 0 gold (3), league 21 bronze (1).
        m.0.insert(base + a::RESULT_MEDALS, 3);
        m.0.insert(base + a::RESULT_MEDALS + 24 * 21, 1);
        let s = read_stats(&m, 0);
        assert_eq!(s.story, [[2, 0, 1], [0, 1, 0]]);
        assert_eq!(s.levels_done, [2, 1]);
        assert_eq!(
            (s.challenges_done, s.challenges, s.leagues_done, s.leagues),
            (1, 21, 1, 27)
        );
        // Medals (1.0 + 0.4) / 48 * 60 = 1.75; story (0.2 * 2 + 0.3 * 1 + 0.5 * 1) / 13 * 40.
        assert!(
            (s.complete - (1.75 + 1.2 / 13.0 * 40.0)).abs() < 1e-3,
            "{}",
            s.complete
        );
    }

    #[test]
    fn slot_texture_reads_the_d3d_header_and_size() {
        let mut m = Mem::default();
        m.0.insert(a::TEXTURE_SLOTS + 4 * PLAYER_ICON_SLOT, 7);
        m.0.insert(a::TEXTURE_RECORDS, 0x6000);
        m.0.insert(0x6000, 0x7000);
        let record = 0x7000 + 7 * 0x34;
        m.0.insert(record + 4, 0x0123_4500);
        m.0.insert(record + 0xC, 0x0441_0E29);
        m.0.insert(record + 0x24, 16 << 16);
        m.0.insert(record + 0x28, 12 << 16);
        let t = slot_texture(&m, PLAYER_ICON_SLOT).unwrap();
        assert_eq!(
            (t.handle, t.width, t.height, t.data, t.format, t.size),
            (7, 16, 12, 0x0123_4500, 0x0441_0E29, 0)
        );
        assert_eq!(t.key(), "slot:22");
        m.0.insert(a::TEXTURE_SLOTS + 4 * PLAYER_ICON_SLOT, u32::MAX);
        assert_eq!(
            slot_texture(&m, PLAYER_ICON_SLOT),
            None,
            "an unloaded slot is -1"
        );
    }

    #[test]
    fn challenge_rows_list_shown_challenges_with_medals() {
        let mut m = Mem::default();
        let set = set_record(1);
        m.0.insert(set + a::SET_COUNT, 3);
        for (n, id) in [3u32, 4, 5].iter().enumerate() {
            m.0.insert(set + a::SET_IDS + 4 * n as u32, *id);
            m.0.insert(a::CHALLENGES + 4 * id, 0x8000 + 0x100 * id);
        }
        m.0.insert(a::UNLOCK_BIT, 2);
        m.0.insert(0x8300 + a::CHALLENGE_UNLOCK_BITS, 4);
        m.0.insert(0x8400 + a::CHALLENGE_UNLOCK_BITS, 4);
        m.0.insert(0x8400 + a::CHALLENGE_FLAGS, a::CHALLENGE_HIDDEN);
        // Challenge 5 locked (bit clear). Challenge 3: gold (3) in the default results.
        m.0.insert(
            a::RESULTS_DEFAULT + a::RESULTS_OFFSET + 3 * 24 + a::RESULT_MEDAL,
            3,
        );
        m.0.insert(a::MEDAL_TEXTURES + 12, 7);
        m.name(7, 0x9200, b"gold");
        m.0.insert(set + a::SET_ICON, u32::MAX);
        let live = LivePage {
            slot: 1,
            builder: 0x2C_8E00,
            state: 2,
            cursor_node: 0,
            rows: vec![row(1, CHALLENGE_ROW_DRAW, 0)],
            units: (6.4, 4.8),
        };
        let rows = read_list_rows(&m, &live, 1).unwrap();
        assert_eq!(
            rows.rows[0].kind,
            ListRowKind::ChallengeSet {
                medals: vec![Medal {
                    medal: 3,
                    texture: Some("gold".into())
                }],
                icon: None
            }
        );
        // Profile results replace the defaults once a profile is loaded.
        m.0.insert(a::PROFILE_LOADED, 1);
        assert_eq!(results_base(&m, 0), a::RESULTS_PROFILE);
        assert_eq!(medal_of(&m, 3), 0);
        assert_eq!(set_record(8), a::SETS_HIGH + a::SET_STRIDE);
    }
}
