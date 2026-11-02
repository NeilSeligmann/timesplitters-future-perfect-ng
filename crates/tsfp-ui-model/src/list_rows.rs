//! T2311 UI-13 increment 3: the list row draws of the Story level list (`0x2CA9F0`) and the
//! Challenge set list (`0x2C8B50`), their shared row icon draw (`0x2BEAC0`) and the medal draw
//! (`0x2BF650` with `0x2BF4E0` and `0x664A0`), as readable Rust over the `tsfp-ui-renderer`
//! contract.
//!
//! Source: static reading of `default.xbe` (MEASURED from code, docs/t2311-ui-pages-batch1.md,
//! increment 3). Everything the original reads from guest memory at draw time (level and set
//! records, medals, unlock bits, the two run-time scale globals) is read by the host on the guest
//! thread and handed over as a [`ListRows`] value, so this module is pure. Texture pixels come from
//! the owner's disc at run time; this module only names them (original texture table names) and
//! receives renderer ids through a lookup. No game bytes or strings live here.
//!
//! Coordinates are 640x480 design units (the original's), mapped to pixels by the page viewport.

use crate::bridge::TextPlacement;
use crate::model::{Align, TextId, TextRef, WidgetId, menu_rect};
use crate::skin::{PackedColor, ramp, rect, uv};
use tsfp_ui_renderer::{BlendMode, Color, Rect, Sprite, TextureId, Vec2, Viewport};

/// Row icon draw shared by both list rows.
pub const ROW_ICON_DRAW: u32 = 0x2B_EAC0;
/// Story level row draw (installed by `0x2CAC30`).
pub const LEVEL_ROW_DRAW: u32 = 0x2C_A9F0;
/// Challenge set row draw.
pub const CHALLENGE_ROW_DRAW: u32 = 0x2C_8B50;

/// `0x2BEAC0`: the icon fades in over its first `0x49F59C` (0.1682) of width (and of texture u).
pub const ICON_FADE_IN: f32 = 0.1682;
/// `0x2BEAC0`: unlocked and locked icon colours (alpha from the page fade).
pub const ICON_COLOR: PackedColor = PackedColor(0x7F7F_7F7F);
pub const ICON_LOCKED_COLOR: PackedColor = PackedColor(0x1E23_287F);
/// `0x2BEAC0`: the padlock covers the top-left `0x47D598` (0.35) of the icon width and half its height.
pub const PADLOCK_WIDTH: f32 = 0.35;
/// Texture table index of the padlock (`0x80F20(0x147)`).
pub const PADLOCK_TEXTURE: u32 = 0x147;
/// Texture table index of the medal glow (`0x2BF650`: `0x80F20(0x28E)`).
pub const MEDAL_GLOW_TEXTURE: u32 = 0x28E;
/// Text id of the locked label (`0x3D7B0(0x478)`), drawn in [`LOCKED_TEXT_COLOR`].
pub const LOCKED_TEXT: u32 = 0x478;
pub const LOCKED_TEXT_COLOR: u32 = 0x6428_1E00;
/// Level name colour (`0x2CA9F0`), alpha from the row brightness.
pub const LEVEL_TEXT_COLOR: u32 = 0x6464_6400;
/// Text style both list rows select before drawing (`0x3FA10(3)`; the earlier `0x3FA10(0x12)`
/// in `0x2CA9F0` is overridden before any text is drawn).
pub const LIST_TEXT_STYLE: u32 = 3;
/// Medal tints of `0x664A0` (table `0x4D28CC`, index = medal 1..4).
pub const MEDAL_TINTS: [u32; 4] = [0x503C_2850, 0x2828_2850, 0x6446_2850, 0x1E28_3250];

/// A rectangle in design units: left, top, right, bottom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edges {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Edges {
    pub fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self { x0, y0, x1, y1 }
    }
    pub fn width(self) -> f32 {
        self.x1 - self.x0
    }
    pub fn height(self) -> f32 {
        self.y1 - self.y0
    }
    /// Each edge truncated toward zero (`cvttss2si`), as the integer quad draw `0x1F090` gets it.
    pub fn truncated(self) -> Self {
        Self::new(
            self.x0.trunc(),
            self.y0.trunc(),
            self.x1.trunc(),
            self.y1.trunc(),
        )
    }
    /// Pixels in `viewport` (the 4:3 safe area, as the page rows).
    pub fn to_px(self, viewport: Viewport) -> Option<Rect> {
        let anchored = menu_rect(
            Vec2::new(self.x0, self.y0),
            Vec2::new(self.width(), self.height()),
        );
        viewport.resolve(anchored).ok()
    }
}

/// `ftol(fade * 127)`: the alpha byte the list draws use (MSVC `_ftol` truncates).
pub fn alpha_byte(scaled: f32) -> u32 {
    if scaled.is_finite() && scaled > 0.0 {
        (scaled as u32) & 0xFF
    } else {
        0
    }
}

/// `rgb | alpha` as the renderer colour.
fn packed(rgb: u32, alpha: u32) -> Color {
    PackedColor((rgb & 0xFFFF_FF00) | (alpha & 0xFF)).to_color()
}

/// Texture of an icon by original texture table name, resolved to a renderer id by the host.
pub type TextureLookup<'a> = dyn Fn(&str) -> Option<TextureId> + 'a;

/// Inputs of `0x2BEAC0(page, row, rect, texture, locked_texture)`.
#[derive(Clone, Debug, PartialEq)]
pub struct RowIcon {
    /// Texture table name of the icon.
    pub texture: String,
    /// Shown instead of `texture` when the row is locked (`>= 0` in the original).
    pub locked_texture: Option<String>,
}

/// `0x2BEAC0` (MEASURED by static reading): the row icon. Colour 0x7F7F7F (unlocked) or 0x1E2328
/// (locked, row flag `+0x28` bit 0) with alpha `ftol(page fade * 127)`; texture `locked_texture`
/// when locked and given, else `texture`; alpha blend (`0x19D70(1)`). Two quads through `0x1BA70`
/// and `0x1B7B0`: the first [`ICON_FADE_IN`] of the width (and u) ramps from alpha 0 to the colour
/// left to right, the rest is flat. A locked row adds the padlock (`0x80F20(0x147)`) over the
/// top-left [`PADLOCK_WIDTH`] of the width and the top half of the height, white with the same
/// alpha, integer corners (`0x1F090`). INFERRED: the vertex colour order of the ramp quad (read
/// from the push order, as the divider of increment 2).
pub fn row_icon(
    out: &mut Vec<Sprite>,
    textures: &TextureLookup<'_>,
    viewport: Viewport,
    area: Edges,
    page_fade: f32,
    locked: bool,
    icon: &RowIcon,
    padlock: Option<&str>,
) {
    let alpha = alpha_byte(page_fade * 127.0);
    let rgb = if locked {
        ICON_LOCKED_COLOR
    } else {
        ICON_COLOR
    };
    let name = match (&icon.locked_texture, locked) {
        (Some(alt), true) => alt.as_str(),
        _ => icon.texture.as_str(),
    };
    if let Some(texture) = textures(name) {
        let split = area.x0 + area.width() * ICON_FADE_IN;
        let full = packed(rgb.0, alpha);
        let clear = packed(rgb.0, 0);
        if let Some(px) = Edges::new(area.x0, area.y0, split, area.y1).to_px(viewport) {
            ramp(
                out,
                texture,
                px,
                uv(0.0, 0.0, ICON_FADE_IN, 1.0),
                clear,
                full,
                true,
            );
        }
        if let Some(px) = Edges::new(split, area.y0, area.x1, area.y1).to_px(viewport) {
            out.push(Sprite {
                rect_px: px,
                uv: uv(ICON_FADE_IN, 0.0, 1.0, 1.0),
                color: full,
                texture,
            });
        }
    }
    if locked && let Some(padlock) = padlock.and_then(textures) {
        let lock = Edges::new(
            area.x0,
            area.y0,
            area.x0 + area.width() * PADLOCK_WIDTH,
            area.y0 + area.height() * 0.5,
        )
        .truncated();
        if let Some(px) = lock.to_px(viewport) {
            out.push(Sprite {
                rect_px: px,
                uv: uv(0.0, 0.0, 1.0, 1.0),
                color: packed(ICON_COLOR.0, alpha),
                texture: padlock,
            });
        }
    }
}

/// One medal of a challenge set row (`0x2BF650(medal, x, y, w, h, fade)`).
#[derive(Clone, Debug, PartialEq)]
pub struct Medal {
    /// Medal value of the challenge result (`+0xC` of the record `0x69930` returns), 0 = none.
    pub medal: i32,
    /// Texture table name of the medal picture (`[0x5236C8 + medal * 4]`), when `medal > 0`.
    pub texture: Option<String>,
}

/// What a list row shows besides the generic row draw.
#[derive(Clone, Debug, PartialEq)]
pub enum ListRowKind {
    /// `0x2CA9F0`: the level name (string id at level record `+0x2`, upper-cased) or LOCKED, and
    /// the level icon (level record `+0x14`, `-1` none).
    Level {
        name: Option<u32>,
        icon: Option<RowIcon>,
    },
    /// `0x2C8B50`: one medal per unlocked challenge of the set, the LOCKED label, the set icon
    /// (set record `+0x20`).
    ChallengeSet {
        medals: Vec<Medal>,
        icon: Option<RowIcon>,
    },
}

/// One list row, read from the live row record and the tables it indexes.
#[derive(Clone, Debug, PartialEq)]
pub struct ListRow {
    pub widget: WidgetId,
    /// The row's layout box (`+0x78`), design units.
    pub area: Edges,
    /// Row flag `+0x28` bit 0 (greyed: the level or set is locked).
    pub locked: bool,
    /// Row brightness `+0x34` (0..1, rises while the row is focused).
    pub brightness: f32,
    /// Row height `+0x70` (the medal row offset).
    pub row_height: f32,
    pub kind: ListRowKind,
}

/// The list rows of a page and the run-time values their draws read.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ListRows {
    pub rows: Vec<ListRow>,
    /// `[0x7BA038]`, the text unit (x margins).
    pub text_unit: f32,
    /// `[0x7BA188]`, the medal row unit.
    pub medal_unit: f32,
    /// Page style `+0x1C` (the text x margin in text units).
    pub text_margin: f32,
    /// The page vtable is `0x5238C0`: the level text alpha is the plain page fade.
    pub plain_alpha: bool,
    /// The scrolled list's visible band (top, bottom), design units; rows are clipped to it.
    pub clip: Option<(f32, f32)>,
    /// Texture table names of the padlock ([`PADLOCK_TEXTURE`]) and the medal glow
    /// ([`MEDAL_GLOW_TEXTURE`]), read from the guest's table.
    pub padlock: Option<String>,
    pub glow: Option<String>,
    /// The Player Progress summary row (`0x2CC850`), drawn by `progress_summary`.
    pub summary: Option<crate::progress_summary::ProgressSummary>,
    /// T2315: the scroll arrows of a scrolled list page (`0x73E40`), drawn after the rows, outside the list clip.
    pub arrows: Option<ScrollArrows>,
    /// Pixels of textures that are not in the disc's named texture table (the slot textures of
    /// `0x81220`), read by the host from the guest's texture memory, by key.
    pub images: Vec<GuestImage>,
    /// T2315 increment 3: the Audio/Video rows and page draw (`crate::option_rows`).
    pub options: Option<crate::option_rows::OptionRows>,
    /// T2315 increment 4: the in-game scoreboard or objectives row (`crate::ingame_rows`).
    pub ingame: Option<IngameRow>,
}

/// T2315 increment 4: the one custom row of the in-game BACK pages.
#[derive(Clone, Debug, PartialEq)]
pub enum IngameRow {
    Scoreboard(crate::ingame_rows::Scoreboard),
    Objectives(crate::ingame_rows::Objectives),
}

/// Texture table index of the scroll arrow (`0x73E40`: `0x80F20(0x2C4)`).
pub const SCROLL_ARROW_TEXTURE: u32 = 0x2C4;
/// Scroll arrow colour (`0x73E40`), alpha `ftol(page fade * [0x475CC0])`.
pub const SCROLL_ARROW_RGB: u32 = 0x7F7F_7F00;

/// `0x73E40(page, rect, up, down)` (MEASURED by static reading, T2315): the page draw `0x76810` calls it through
/// `0x73FC0` when the page style's first word is set, the page is not hidden (`+0x40`) and the list can scroll up (scroll
/// offset `+0x84` above `[0x475CD4]`) or down (content `+0x5C` minus the view `+0x70 - +0x6C` minus the offset above it).
/// One texture ([`SCROLL_ARROW_TEXTURE`], the whole texture: uv 0..w*16, 0..h*16 in 1/16 texels), integer corners
/// (`0x1F090`), alpha blend, colour [`SCROLL_ARROW_RGB`]. The up arrow spans `y0..y1` downwards, the down arrow is the
/// same texture with `y0 > y1` (drawn upside down). The live reader computes the corners.
#[derive(Clone, Debug, PartialEq)]
pub struct ScrollArrows {
    /// Texture table name of [`SCROLL_ARROW_TEXTURE`].
    pub texture: Option<String>,
    /// Corners of the up and down arrows (design units, already truncated), when shown.
    pub up: Option<Edges>,
    pub down: Option<Edges>,
    /// `[0x475CC0]`: the page fade multiplier of the alpha byte.
    pub alpha_scale: f32,
}

/// The scroll arrows of `arrows` as sprites (the down arrow flipped by its reversed corners).
pub fn scroll_arrows(
    out: &mut Vec<Sprite>,
    arrows: &ScrollArrows,
    textures: &TextureLookup<'_>,
    viewport: Viewport,
    page_fade: f32,
) {
    let Some(texture) = arrows.texture.as_deref().and_then(textures) else {
        return;
    };
    let color = packed(SCROLL_ARROW_RGB, alpha_byte(page_fade * arrows.alpha_scale));
    for edges in [arrows.up, arrows.down].into_iter().flatten() {
        let flipped = edges.y1 < edges.y0;
        let upright = if flipped {
            Edges::new(edges.x0, edges.y1, edges.x1, edges.y0)
        } else {
            edges
        };
        if let Some(px) = upright.to_px(viewport) {
            let uv = if flipped {
                uv(0.0, 1.0, 1.0, 0.0)
            } else {
                uv(0.0, 0.0, 1.0, 1.0)
            };
            out.push(Sprite {
                rect_px: px,
                uv,
                color,
                texture,
            });
        }
    }
}

/// One texture's pixels as the host decoded them (RGBA8, straight alpha, top-left origin).
#[derive(Clone, Debug, PartialEq)]
pub struct GuestImage {
    pub key: String,
    pub width: u32,
    pub height: u32,
    pub rgba: std::sync::Arc<Vec<u8>>,
}

/// Sprites and texts of the list rows, in the original order (row by row: text, then icon).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListDraw {
    pub sprites: Vec<Sprite>,
    /// Blend switches over `sprites` (`(index, mode)`, from that sprite on).
    pub blends: Vec<(usize, BlendMode)>,
    pub texts: Vec<TextPlacement>,
    /// Rows whose generic row text (the row's own text, drawn by `0x76810` before the row draw)
    /// starts at the list text x, left aligned: `(widget, x)` in pixels.
    pub left_aligned: Vec<(WidgetId, f32)>,
    /// Locked rows: their generic row text is drawn greyed (`skin::GREYED_TEXT`, INFERRED from
    /// the legacy picture; the original greys rows with flag `+0x28` bit 0).
    pub greyed: Vec<WidgetId>,
    /// Runtime text of the draw's own placements (`TextRef::Dynamic` keyed by these widget ids).
    pub dynamic: Vec<(WidgetId, Vec<u8>)>,
    /// Per-text horizontal glyph scale, after measuring (scoreboard name fit).
    pub text_scale_x: Vec<(WidgetId, f32)>,
    /// Score runs using the original fixed digit mode.
    pub fixed_spacing: Vec<WidgetId>,
}

impl ListDraw {
    /// Append `other` after this draw (its blend switch indices moved along).
    pub fn append(&mut self, other: ListDraw) {
        let base = self.sprites.len();
        self.sprites.extend(other.sprites);
        self.blends
            .extend(other.blends.into_iter().map(|(i, mode)| (i + base, mode)));
        self.texts.extend(other.texts);
        self.left_aligned.extend(other.left_aligned);
        self.greyed.extend(other.greyed);
        self.dynamic.extend(other.dynamic);
        self.text_scale_x.extend(other.text_scale_x);
        self.fixed_spacing.extend(other.fixed_spacing);
    }
}

/// The icon box of both rows (MEASURED, `0x2CA9F0` and `0x2C8B50`): right edge `x1 - 5 * unit`,
/// width `h * 1.3333 * [0x4C0460]` (1.0), top `y0 + h * 0.075`, bottom `y1 + 3`.
pub fn icon_box(area: Edges, text_unit: f32) -> Edges {
    let h = area.height();
    let right = area.x1 - text_unit * 5.0;
    Edges::new(
        right - h * (4.0 / 3.0),
        area.y0 + h * 0.075,
        right,
        area.y1 + 3.0,
    )
}

/// Clip `sprite` to the band `top..bottom` (pixels), cutting its uv in proportion; None if outside.
pub fn clip_sprite(mut sprite: Sprite, top: f32, bottom: f32) -> Option<Sprite> {
    let (y0, y1) = (
        sprite.rect_px.origin.y,
        sprite.rect_px.origin.y + sprite.rect_px.size.y,
    );
    let (c0, c1) = (y0.max(top), y1.min(bottom));
    if c1 <= c0 || sprite.rect_px.size.y <= 0.0 {
        return None;
    }
    let (v0, v1) = (sprite.uv.min.y, sprite.uv.max.y);
    let at = |y: f32| v0 + (v1 - v0) * (y - y0) / (y1 - y0);
    sprite.uv.min.y = at(c0);
    sprite.uv.max.y = at(c1);
    sprite.rect_px = rect(
        sprite.rect_px.origin.x,
        c0,
        sprite.rect_px.origin.x + sprite.rect_px.size.x,
        c1,
    )?;
    Some(sprite)
}

fn text(
    widget: WidgetId,
    id: u32,
    x: f32,
    y: f32,
    right: f32,
    color: Color,
    viewport: Viewport,
) -> Option<TextPlacement> {
    let px = Edges::new(x, y, right.max(x + 1.0), y + 1.0).to_px(viewport)?;
    Some(TextPlacement {
        widget,
        text: TextRef::Id(TextId(id)),
        font_index: LIST_TEXT_STYLE,
        value_font_index: LIST_TEXT_STYLE,
        align: Align::Start,
        rect_px: px,
        color,
        value: None,
    })
}

/// `0x2BF650` with `0x2BF4E0` (MEASURED by static reading): the medal picture (`medal > 0`,
/// clamped to 4) in `x, y, w, h`, white at `ftol(fade * 127)`, alpha blend; then the glow
/// (`glow_128`, table 0x28E) additive, centred on the box, half extent `h * 2.0` for a medal and
/// `h * 0.6` for none, coloured by the medal tint (`0x664A0`, alpha scaled by the fade) or white at
/// `ftol(fade * 50)` without a medal. INFERRED: the glow colour argument (stack slot reading).
fn medal(
    draw: &mut ListDraw,
    textures: &TextureLookup<'_>,
    glow: Option<&str>,
    viewport: Viewport,
    medal: &Medal,
    at: Edges,
    fade: f32,
) {
    let value = medal.medal.min(4);
    if value > 0
        && let Some(texture) = medal.texture.as_deref().and_then(textures)
        && let Some(px) = at.to_px(viewport)
    {
        draw.blends.push((draw.sprites.len(), BlendMode::Alpha));
        draw.sprites.push(Sprite {
            rect_px: px,
            uv: uv(0.0, 0.0, 1.0, 1.0),
            color: packed(ICON_COLOR.0, alpha_byte(fade * 127.0)),
            texture,
        });
    }
    let Some(glow) = glow.and_then(textures) else {
        return;
    };
    let (w, h) = (at.width() as i32, at.height() as i32);
    let (cx, cy) = ((at.x0 as i32 + w / 2) as f32, (at.y0 as i32 + h / 2) as f32);
    let half = (h as f32 * if value != 0 { 2.0 } else { 0.6 }).trunc();
    let color = if value > 0 {
        let tint = MEDAL_TINTS[(value - 1) as usize];
        packed(tint, alpha_byte((tint & 0xFF) as f32 * fade))
    } else {
        packed(ICON_COLOR.0, alpha_byte(fade * 50.0))
    };
    if let Some(px) = Edges::new(cx - half, cy - half, cx + half, cy + half).to_px(viewport) {
        draw.blends.push((draw.sprites.len(), BlendMode::Additive));
        draw.sprites.push(Sprite {
            rect_px: px,
            uv: uv(0.0, 0.0, 1.0, 1.0),
            color,
            texture: glow,
        });
        draw.blends.push((draw.sprites.len(), BlendMode::Alpha));
    }
}

/// Draw the list rows of a page (`0x2CA9F0` / `0x2C8B50` per row). `page_fade` is page `+0xF4`.
pub fn draw_list_rows(
    rows: &ListRows,
    textures: &TextureLookup<'_>,
    viewport: Viewport,
    page_fade: f32,
) -> ListDraw {
    let mut draw = ListDraw::default();
    for row in &rows.rows {
        let start = draw.sprites.len();
        let area = row.area;
        let text_x = (area.x0 + rows.text_margin * rows.text_unit).trunc();
        if let Some(px) = Edges::new(text_x, area.y0, text_x + 1.0, area.y0 + 1.0).to_px(viewport) {
            draw.left_aligned.push((row.widget, px.origin.x));
        }
        if row.locked {
            draw.greyed.push(row.widget);
        }
        let text_y = (area.y0 + area.height() * 0.55).trunc();
        match &row.kind {
            ListRowKind::Level { name, icon } => {
                // Row text alpha: the page fade on the plain page class, else the brightness ramp.
                let alpha = if rows.plain_alpha {
                    alpha_byte(page_fade * 127.0)
                } else {
                    alpha_byte((row.brightness * 96.0 + 32.0) * page_fade)
                };
                let (id, rgb) = if row.locked {
                    (Some(LOCKED_TEXT), LOCKED_TEXT_COLOR)
                } else {
                    (*name, LEVEL_TEXT_COLOR)
                };
                if let Some(id) = id
                    && let Some(t) = text(
                        row.widget,
                        id,
                        text_x,
                        text_y,
                        area.x1,
                        packed(rgb, alpha),
                        viewport,
                    )
                {
                    draw.texts.push(t);
                }
                if let Some(icon) = icon {
                    row_icon(
                        &mut draw.sprites,
                        textures,
                        viewport,
                        icon_box(area, rows.text_unit),
                        page_fade,
                        row.locked,
                        icon,
                        rows.padlock.as_deref(),
                    );
                }
            }
            ListRowKind::ChallengeSet { medals, icon } => {
                let mut x = (area.x0 + 20.0).trunc();
                let y = (area.y0 + row.row_height * rows.medal_unit * 0.6).trunc();
                for m in medals {
                    medal(
                        &mut draw,
                        textures,
                        rows.glow.as_deref(),
                        viewport,
                        m,
                        Edges::new(x, y, x + 20.0, y + 20.0),
                        page_fade,
                    );
                    x = (x + 25.0).trunc();
                }
                if row.brightness > 0.0 && row.locked {
                    let alpha = alpha_byte(page_fade * row.brightness * 127.0);
                    if let Some(t) = text(
                        row.widget,
                        LOCKED_TEXT,
                        x + 10.0,
                        y + 3.0,
                        area.x1,
                        packed(LOCKED_TEXT_COLOR, alpha),
                        viewport,
                    ) {
                        draw.texts.push(t);
                    }
                }
                if let Some(icon) = icon {
                    draw.blends.push((draw.sprites.len(), BlendMode::Alpha));
                    row_icon(
                        &mut draw.sprites,
                        textures,
                        viewport,
                        icon_box(area, rows.text_unit),
                        page_fade,
                        row.locked,
                        icon,
                        rows.padlock.as_deref(),
                    );
                }
            }
        }
        if let Some((top, bottom)) = rows.clip
            && let (Some(t), Some(b)) = (
                Edges::new(area.x0, top, area.x1, top + 1.0).to_px(viewport),
                Edges::new(area.x0, bottom, area.x1, bottom + 1.0).to_px(viewport),
            )
        {
            clip_range(&mut draw, start, t.origin.y, b.origin.y);
        }
    }
    if let Some(arrows) = &rows.arrows {
        // After the rows, under the page viewport (not the list band), alpha blend (`0x19D70(1)`).
        draw.blends.push((draw.sprites.len(), BlendMode::Alpha));
        scroll_arrows(&mut draw.sprites, arrows, textures, viewport, page_fade);
    }
    if let Some((top, bottom)) = rows.clip {
        // Text of a row outside the band is not drawn; text partly inside is kept and cut by the frame's text
        // scissor (`UiFrame::text_clip`, T2315).
        let band = (
            Edges::new(0.0, top, 1.0, top + 1.0)
                .to_px(viewport)
                .map(|r| r.origin.y),
            Edges::new(0.0, bottom, 1.0, bottom + 1.0)
                .to_px(viewport)
                .map(|r| r.origin.y),
        );
        if let (Some(t), Some(b)) = band {
            draw.texts
                .retain(|p| p.rect_px.origin.y + p.rect_px.size.y >= t && p.rect_px.origin.y <= b);
        }
    }
    draw
}

/// Clip the sprites from `start` on to the band, keeping the blend switch indices right.
fn clip_range(draw: &mut ListDraw, start: usize, top: f32, bottom: f32) {
    let tail: Vec<Sprite> = draw.sprites.drain(start..).collect();
    let mut index_map = Vec::with_capacity(tail.len() + 1);
    for sprite in tail {
        index_map.push(draw.sprites.len());
        if let Some(kept) = clip_sprite(sprite, top, bottom) {
            draw.sprites.push(kept);
        }
    }
    index_map.push(draw.sprites.len());
    for (at, _) in draw.blends.iter_mut() {
        if *at >= start {
            *at = index_map
                .get(*at - start)
                .copied()
                .unwrap_or(draw.sprites.len());
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tier 0 with FABRICATED rows and texture ids; the constants are the MEASURED ones.
    use super::*;

    fn viewport() -> Viewport {
        crate::anchor::window_viewport(640, 480).unwrap()
    }

    fn lookup(name: &str) -> Option<TextureId> {
        match name {
            "icon" => Some(TextureId(1)),
            "alt" => Some(TextureId(2)),
            "medal2" => Some(TextureId(4)),
            "padlock" => Some(TextureId(3)),
            "glow" => Some(TextureId(5)),
            _ => None,
        }
    }

    #[test]
    fn the_down_arrow_is_the_up_texture_flipped() {
        let arrows = ScrollArrows {
            texture: Some("glow".into()),
            up: Some(Edges::new(536.0, 116.0, 550.0, 130.0)),
            down: Some(Edges::new(536.0, 385.0, 550.0, 371.0)),
            alpha_scale: 127.0,
        };
        let mut out = Vec::new();
        scroll_arrows(&mut out, &arrows, &lookup, viewport(), 1.0);
        assert_eq!(out.len(), 2);
        assert_eq!((out[0].uv.min.y, out[0].uv.max.y), (0.0, 1.0));
        assert_eq!((out[1].uv.min.y, out[1].uv.max.y), (1.0, 0.0));
        assert!(out[1].rect_px.size.y > 0.0);
        scroll_arrows(
            &mut out,
            &ScrollArrows {
                texture: None,
                ..arrows
            },
            &lookup,
            viewport(),
            1.0,
        );
        assert_eq!(out.len(), 2, "no texture, no arrows");
    }

    fn icon() -> RowIcon {
        RowIcon {
            texture: "icon".into(),
            locked_texture: None,
        }
    }

    #[test]
    fn row_icon_fades_in_over_the_first_sixth_then_is_flat() {
        let mut out = Vec::new();
        let area = Edges::new(100.0, 100.0, 200.0, 140.0);
        row_icon(
            &mut out,
            &lookup,
            viewport(),
            area,
            1.0,
            false,
            &icon(),
            Some("padlock"),
        );
        assert_eq!(out.len(), crate::skin::RAMP_STEPS + 1);
        let flat = out.last().unwrap();
        assert!((flat.rect_px.origin.x - 116.82).abs() < 1e-3);
        assert_eq!(flat.rect_px.size.x + flat.rect_px.origin.x, 200.0);
        assert!((flat.uv.min.x - ICON_FADE_IN).abs() < 1e-6);
        assert_eq!(flat.color, ICON_COLOR.to_color());
        assert!(out[0].color.0[3] < 0.05, "the left edge is transparent");
        assert!(out.iter().all(|s| s.texture == TextureId(1)));
    }

    #[test]
    fn locked_icon_is_dark_takes_the_locked_texture_and_adds_the_padlock() {
        let mut out = Vec::new();
        let area = Edges::new(100.5, 100.5, 200.0, 140.0);
        let icon = RowIcon {
            texture: "icon".into(),
            locked_texture: Some("alt".into()),
        };
        row_icon(
            &mut out,
            &lookup,
            viewport(),
            area,
            0.5,
            true,
            &icon,
            Some("padlock"),
        );
        let padlock = out.last().unwrap();
        assert_eq!(padlock.texture, TextureId(3));
        // Integer corners: 100.5 -> 100, 100.5 + 99.5 * 0.35 = 135.325 -> 135, top half 120.25 -> 120.
        assert_eq!(
            (padlock.rect_px.origin.x, padlock.rect_px.origin.y),
            (100.0, 100.0)
        );
        assert_eq!(
            (padlock.rect_px.size.x, padlock.rect_px.size.y),
            (35.0, 20.0)
        );
        assert!(
            (padlock.color.0[3] - 126.0 / 255.0).abs() < 1e-6,
            "alpha ftol(0.5 * 127) = 63"
        );
        let body = &out[out.len() - 2];
        assert_eq!(body.texture, TextureId(2));
        let dark = ICON_LOCKED_COLOR.to_color();
        assert_eq!(&body.color.0[..3], &dark.0[..3]);
    }

    #[test]
    fn level_rows_draw_name_or_locked_and_the_icon_box() {
        let row = |locked| ListRow {
            widget: WidgetId(3),
            area: Edges::new(100.0, 200.0, 500.0, 219.0),
            locked,
            brightness: 1.0,
            row_height: 19.0,
            kind: ListRowKind::Level {
                name: Some(0x1706),
                icon: Some(icon()),
            },
        };
        let rows = ListRows {
            rows: vec![row(false), row(true)],
            text_unit: 2.0,
            medal_unit: 1.0,
            text_margin: 5.0,
            plain_alpha: false,
            clip: None,
            arrows: None,
            padlock: Some("padlock".into()),
            glow: Some("glow".into()),
            summary: None,
            images: Vec::new(),
            options: None,
            ingame: None,
        };
        let draw = draw_list_rows(&rows, &lookup, viewport(), 1.0);
        assert_eq!(draw.texts.len(), 2);
        assert_eq!(draw.texts[0].text, TextRef::Id(TextId(0x1706)));
        assert_eq!(draw.texts[1].text, TextRef::Id(TextId(LOCKED_TEXT)));
        // x = 100 + 5 * 2, y = trunc(200 + 19 * 0.55) = 210; alpha (1 * 96 + 32) = 128 -> 0x80.
        assert_eq!(draw.texts[0].rect_px.origin, Vec2::new(110.0, 210.0));
        assert!((draw.texts[0].color.0[3] - 1.0).abs() < 1e-6);
        let b = icon_box(rows.rows[0].area, 2.0);
        assert_eq!((b.x1, b.y1), (490.0, 222.0));
        assert!((b.x0 - (490.0 - 19.0 * 4.0 / 3.0)).abs() < 1e-3);
        assert_eq!(
            draw.left_aligned,
            vec![(WidgetId(3), 110.0), (WidgetId(3), 110.0)]
        );
    }

    #[test]
    fn challenge_rows_step_medals_and_glow_additively() {
        let rows = ListRows {
            rows: vec![ListRow {
                widget: WidgetId(1),
                area: Edges::new(100.0, 100.0, 500.0, 140.0),
                locked: false,
                brightness: 0.0,
                row_height: 40.0,
                kind: ListRowKind::ChallengeSet {
                    medals: vec![
                        Medal {
                            medal: 2,
                            texture: Some("medal2".into()),
                        },
                        Medal {
                            medal: 0,
                            texture: None,
                        },
                    ],
                    icon: None,
                },
            }],
            text_unit: 1.0,
            medal_unit: 0.5,
            text_margin: 0.0,
            plain_alpha: true,
            clip: None,
            arrows: None,
            padlock: Some("padlock".into()),
            glow: Some("glow".into()),
            summary: None,
            images: Vec::new(),
            options: None,
            ingame: None,
        };
        let draw = draw_list_rows(&rows, &lookup, viewport(), 1.0);
        // medal, glow, glow (no picture for medal 0).
        assert_eq!(draw.sprites.len(), 3);
        // y = trunc(100 + 40 * 0.5 * 0.6) = 112; first medal at x 120, second at 145.
        assert_eq!(draw.sprites[0].rect_px.origin, Vec2::new(120.0, 112.0));
        let glow = &draw.sprites[1];
        assert_eq!(glow.texture, TextureId(5));
        assert_eq!(
            (glow.rect_px.origin.x, glow.rect_px.size.x),
            (130.0 - 40.0, 80.0)
        );
        assert_eq!(
            draw.sprites[2].rect_px.size.x, 24.0,
            "no medal: half extent 20 * 0.6 = 12"
        );
        assert!(draw.blends.contains(&(1, BlendMode::Additive)));
        assert!(draw.texts.is_empty(), "unlocked set: no label");
    }

    #[test]
    fn clipping_cuts_sprites_and_uv_and_drops_texts_outside_the_band() {
        let s = Sprite {
            rect_px: rect(0.0, 0.0, 10.0, 100.0).unwrap(),
            uv: uv(0.0, 0.0, 1.0, 1.0),
            color: Color([1.0; 4]),
            texture: TextureId(1),
        };
        let cut = clip_sprite(s, 25.0, 50.0).unwrap();
        assert_eq!((cut.rect_px.origin.y, cut.rect_px.size.y), (25.0, 25.0));
        assert!((cut.uv.min.y - 0.25).abs() < 1e-6 && (cut.uv.max.y - 0.5).abs() < 1e-6);
        assert!(clip_sprite(s, 200.0, 300.0).is_none());
        let row = |y: f32| ListRow {
            widget: WidgetId(y as u64),
            area: Edges::new(100.0, y, 500.0, y + 19.0),
            locked: false,
            brightness: 0.0,
            row_height: 19.0,
            kind: ListRowKind::Level {
                name: Some(1),
                icon: Some(icon()),
            },
        };
        let rows = ListRows {
            rows: vec![row(100.0), row(300.0)],
            text_unit: 1.0,
            medal_unit: 1.0,
            text_margin: 0.0,
            plain_alpha: true,
            clip: Some((90.0, 200.0)),
            arrows: None,
            padlock: None,
            glow: None,
            summary: None,
            images: Vec::new(),
            options: None,
            ingame: None,
        };
        let draw = draw_list_rows(&rows, &lookup, viewport(), 1.0);
        assert_eq!(draw.texts.len(), 1, "the row below the band has no text");
        assert!(
            draw.sprites
                .iter()
                .all(|s| s.rect_px.origin.y + s.rect_px.size.y <= 200.0 + 1e-3)
        );
        assert!(!draw.sprites.is_empty());
    }
}
