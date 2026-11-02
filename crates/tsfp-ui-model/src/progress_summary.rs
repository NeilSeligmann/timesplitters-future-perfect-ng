//! T2311 UI-13 increment 3: the Player Progress summary row draw `0x2CC850` as readable Rust.
//!
//! The row shows the story table (Easy / Normal / Hard headers, one line per player count with
//! the player icon, levels done per difficulty, levels done of the total), then Challenges,
//! Leagues and Complete. Every number comes from a [`ProgressStats`] value the host reads through
//! a typed view of the profile results (`tsfp_ui_input::progress_live`, the getters
//! `0x660C0..0x66310`); this module only lays it out. Static reading of `default.xbe` (MEASURED from
//! code, symbolic stack trace of the lifted listing, docs/t2311-ui-pages-batch1.md increment 3).
//!
//! Units: 640x480 design units like the original; text widths come from the host's font measure
//! (the original measures with `0x405C0` in the active text style, the row's font).

use crate::bridge::TextPlacement;
use crate::list_rows::{Edges, ListDraw, TextureLookup};
use crate::model::{Align, TextId, TextRef, WidgetId};
use crate::skin::PackedColor;
use tsfp_ui_renderer::{Color, Sprite, UvRect, Vec2, Viewport};

/// Row draw callback of the summary row of the Player Progress page.
pub const PROGRESS_SUMMARY_DRAW: u32 = 0x2C_C850;
/// Text ids (`0x3D7B0`): the three difficulty headers, the story label (drawn with ":"), and the
/// Challenges, Leagues and Complete labels.
pub const DIFFICULTY_TEXTS: [u32; 3] = [0x3D5, 0x3D6, 0x3D7];
pub const STORY_TEXT: u32 = 0x403;
pub const CHALLENGES_TEXT: u32 = 0x401;
pub const LEAGUES_TEXT: u32 = 0x402;
pub const COMPLETE_TEXT: u32 = 0x400;
/// Bright and dark text colours (`0x7878787F`, `0x2828287F`), alpha scaled by the page fade
/// (`0x154FE0`).
pub const BRIGHT: PackedColor = PackedColor(0x7878_787F);
pub const DARK: PackedColor = PackedColor(0x2828_287F);
/// The difficulty header centres, as fractions of the free span right of the story label
/// (`0x4785E4` 0.2, `0x4788E8` 0.55, `0x47D594` 0.95).
pub const COLUMN_AT: [f32; 3] = [0.2, 0.55, 0.95];
/// The labels end this many text units left of the right margin (`0x4788FC`, 15.0).
pub const LABEL_GAP_UNITS: f32 = 15.0;
/// Player icon texture slot (`0x81C20(0x16)` / `0x81C60(.., 0x16, ..)`, table `[0x7BA040]`).
pub const PLAYER_ICON_SLOT: u32 = 0x16;
/// The second player icon of the two-player line sits this far left (0xE).
pub const SECOND_ICON_OFFSET: f32 = 14.0;
/// Icon shadow alpha per page fade (`0x47D1E0`, 64.0), black, offset (3, 1) from the icon.
pub const SHADOW_ALPHA: f32 = 64.0;

/// The numbers of the summary, as the original getters return them (profile `[0x78AD34]`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProgressStats {
    /// Levels done per player count (0 one player, 1 two players) and difficulty (`0x662D0`,
    /// `0x662F0`: bits 0..12 of the difficulty word).
    pub story: [[u32; 3]; 2],
    /// Levels done on any difficulty per player count (`0x66200`, `0x66210`) of `levels` (`0x66160`, 13).
    pub levels_done: [u32; 2],
    pub levels: u32,
    /// Challenges with a medal (`0x66100`) of `challenges` (`0x66150`, 21).
    pub challenges_done: u32,
    pub challenges: u32,
    /// Leagues with a medal (`0x660C0`) of `leagues` (`0x66140`, 27).
    pub leagues_done: u32,
    pub leagues: u32,
    /// Percent complete (`0x66310`), 0..100.
    pub complete: f32,
}

/// The player icon (texture of slot [`PLAYER_ICON_SLOT`]) by table name and native size (texels,
/// drawn one design unit per texel as `0x81C60` does).
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerIcon {
    pub texture: String,
    pub width: f32,
    pub height: f32,
}

/// One summary row and the run-time values its draw reads.
#[derive(Clone, Debug, PartialEq)]
pub struct ProgressSummary {
    pub widget: WidgetId,
    /// The row's layout box (`+0x78`).
    pub area: Edges,
    /// Page `+0x24` margins (left `[0]`, right `[2]`), in text units.
    pub margin_left: f32,
    pub margin_right: f32,
    /// `[0x7BA038]` text unit, `[0x7BA188]` line unit (the first line sits one unit down).
    pub text_unit: f32,
    pub line_unit: f32,
    /// `int([0x7E7264])`, the line advance.
    pub line_height: f32,
    /// Active text style while the row draws (the row's font `+0x1C`).
    pub font: u32,
    pub stats: ProgressStats,
    pub icon: Option<PlayerIcon>,
}

/// Text width in design units: `(text, runtime bytes, font)`; bytes are used for `TextRef::Dynamic`.
pub type TextWidth<'a> = dyn Fn(TextRef, &[u8], u32) -> f32 + 'a;

fn faded(color: PackedColor, fade: f32) -> Color {
    color.with_alpha_scaled(fade.clamp(0.0, 1.0))
}

fn trunc(v: f32) -> f32 {
    v.trunc()
}

/// Draw the summary (MEASURED layout of `0x2CC850`): `xl = int(x0 + left * unit)`,
/// `xr = int(x1 - right * unit)`, line `n` at `y = int(y0 + line unit) + n * line height`.
/// Line 0: "Story:" at `xl` (bright) and the three headers centred at
/// `after + (xr - 15 unit - after) * {0.2, 0.55, 0.95}` (`after = int(xl + width(Story:))`), bright
/// when either player count has a level on that difficulty, else dark. Lines 1 and 2 (one and two
/// players): the player icon(s) with a black shadow, right-aligned to the end of "Story"; the count
/// per difficulty right-aligned to the header's right edge (bright when non-zero); "done/total"
/// right-aligned to `xr`. Lines 3..5: Challenges, Leagues, Complete labels right-aligned to
/// `xr - 15 unit`, their values (`%d/%d`, `%.1f %%`) right-aligned to `xr`, bright.
pub fn draw_progress_summary(
    summary: &ProgressSummary,
    textures: &TextureLookup<'_>,
    width: &TextWidth<'_>,
    viewport: Viewport,
    fade: f32,
) -> ListDraw {
    let mut draw = ListDraw::default();
    let s = summary;
    let font = s.font;
    let unit = s.text_unit;
    let xl = trunc(s.area.x0 + s.margin_left * unit);
    let xr = trunc(s.area.x1 - s.margin_right * unit);
    let y0 = trunc(s.area.y0 + s.line_unit);
    let line = |n: f32| y0 + n * s.line_height;
    let bright = faded(BRIGHT, fade);
    let dark = faded(DARK, fade);
    let mut next_dynamic = 0u64;
    let mut put = |draw: &mut ListDraw,
                   text: TextRef,
                   bytes: Option<Vec<u8>>,
                   x: f32,
                   y: f32,
                   align: Align,
                   color: Color| {
        let widget = match bytes {
            Some(bytes) => {
                let id = WidgetId::anon(0x5300 + next_dynamic as usize);
                next_dynamic += 1;
                draw.dynamic.push((id, bytes));
                id
            }
            None => s.widget,
        };
        // Start: the run begins at x; End: it ends at x; Center: centred on x.
        let (left, right) = match align {
            Align::Start => (x, x + 400.0),
            Align::End => (x - 400.0, x),
            Align::Center => (x - 200.0, x + 200.0),
        };
        if let Some(rect_px) = Edges::new(left, y, right, y + 1.0).to_px(viewport) {
            draw.texts.push(TextPlacement {
                widget,
                text,
                font_index: font,
                value_font_index: font,
                align,
                rect_px,
                color,
                value: None,
            });
        }
    };
    let id = |n: u32| TextRef::Id(TextId(n));

    // Line 0: "Story:" and the headers.
    let story_w = width(id(STORY_TEXT), &[], font);
    let colon_w = width(TextRef::Dynamic, b":", font);
    put(
        &mut draw,
        id(STORY_TEXT),
        None,
        xl,
        line(0.0),
        Align::Start,
        bright,
    );
    put(
        &mut draw,
        TextRef::Dynamic,
        Some(b":".to_vec()),
        xl + story_w,
        line(0.0),
        Align::Start,
        bright,
    );
    let after = trunc(story_w + colon_w + xl);
    let span = xr - unit * LABEL_GAP_UNITS - after;
    for (d, header) in DIFFICULTY_TEXTS.iter().enumerate() {
        let w = width(id(*header), &[], font);
        let x = trunc(span * COLUMN_AT[d] + after - w * 0.5);
        let any = s.stats.story[0][d] != 0 || s.stats.story[1][d] != 0;
        put(
            &mut draw,
            id(*header),
            None,
            x,
            line(0.0),
            Align::Start,
            if any { bright } else { dark },
        );
        let right = trunc(w + x);
        for players in 0..2 {
            let n = s.stats.story[players][d];
            let color = if n != 0 { bright } else { dark };
            put(
                &mut draw,
                TextRef::Dynamic,
                Some(n.to_string().into_bytes()),
                right,
                line(1.0 + players as f32),
                Align::End,
                color,
            );
        }
    }

    // Lines 1 and 2: player icons and levels done.
    if let Some(icon) = &s.icon
        && let Some(texture) = textures(&icon.texture)
    {
        let x = trunc(story_w + xl - icon.width);
        let shadow = Color([
            0.0,
            0.0,
            0.0,
            (crate::list_rows::alpha_byte(fade * SHADOW_ALPHA)) as f32 / 127.0,
        ]);
        let mut put_icon = |x: f32, y: f32| {
            for (dx, dy, color) in [(3.0, 5.0, shadow), (0.0, 4.0, bright)] {
                if let Some(rect_px) =
                    Edges::new(x + dx, y + dy, x + dx + icon.width, y + dy + icon.height)
                        .to_px(viewport)
                {
                    draw.sprites.push(Sprite {
                        rect_px,
                        uv: UvRect {
                            min: Vec2::new(0.0, 0.0),
                            max: Vec2::new(1.0, 1.0),
                        },
                        color,
                        texture,
                    });
                }
            }
        };
        put_icon(x, line(1.0));
        put_icon(x, line(2.0));
        put_icon(x - SECOND_ICON_OFFSET, line(2.0));
    }
    for players in 0..2 {
        let text = format!("{}/{}", s.stats.levels_done[players], s.stats.levels);
        put(
            &mut draw,
            TextRef::Dynamic,
            Some(text.into_bytes()),
            xr,
            line(1.0 + players as f32),
            Align::End,
            bright,
        );
    }

    // Lines 3..5: Challenges, Leagues, Complete.
    let rows = [
        (
            CHALLENGES_TEXT,
            format!("{}/{}", s.stats.challenges_done, s.stats.challenges),
        ),
        (
            LEAGUES_TEXT,
            format!("{}/{}", s.stats.leagues_done, s.stats.leagues),
        ),
        (COMPLETE_TEXT, format!("{:.1} %", s.stats.complete)),
    ];
    for (n, (label, value)) in rows.into_iter().enumerate() {
        let y = line(3.0 + n as f32);
        put(
            &mut draw,
            id(label),
            None,
            xr - unit * LABEL_GAP_UNITS,
            y,
            Align::End,
            bright,
        );
        put(
            &mut draw,
            TextRef::Dynamic,
            Some(value.into_bytes()),
            xr,
            y,
            Align::End,
            bright,
        );
    }
    draw
}

#[cfg(test)]
mod tests {
    //! Tier 0 with a FABRICATED row, stats and a fixed-width measure (10 units per byte, 40 per id).
    use super::*;

    fn summary() -> ProgressSummary {
        ProgressSummary {
            widget: WidgetId(4),
            area: Edges::new(100.0, 200.0, 540.0, 380.0),
            margin_left: 2.0,
            margin_right: 2.0,
            text_unit: 6.4,
            line_unit: 4.8,
            line_height: 20.0,
            font: 4,
            stats: ProgressStats {
                story: [[2, 0, 0], [0, 0, 0]],
                levels_done: [2, 0],
                levels: 13,
                challenges_done: 1,
                challenges: 21,
                leagues_done: 0,
                leagues: 27,
                complete: 12.34,
            },
            icon: Some(PlayerIcon {
                texture: "p1".into(),
                width: 16.0,
                height: 16.0,
            }),
        }
    }

    fn measure(text: TextRef, bytes: &[u8], _font: u32) -> f32 {
        match text {
            TextRef::Id(_) => 40.0,
            _ => 10.0 * bytes.len() as f32,
        }
    }

    #[test]
    fn summary_lays_out_the_story_table_and_the_totals() {
        let viewport = crate::anchor::window_viewport(640, 480).unwrap();
        let lookup = |name: &str| (name == "p1").then_some(tsfp_ui_renderer::TextureId(9));
        let draw = draw_progress_summary(&summary(), &lookup, &measure, viewport, 1.0);
        // xl = int(100 + 12.8) = 112, xr = int(540 - 12.8) = 527, y0 = int(204.8) = 204.
        let story = &draw.texts[0];
        assert_eq!(story.text, TextRef::Id(TextId(STORY_TEXT)));
        assert_eq!(story.rect_px.origin, Vec2::new(112.0, 204.0));
        // after = int(40 + 10 + 112) = 162; span = 527 - 96 - 162 = 269; Easy x = int(269 * 0.2 + 162 - 20) = 195.
        let easy = draw
            .texts
            .iter()
            .find(|t| t.text == TextRef::Id(TextId(0x3D5)))
            .unwrap();
        assert_eq!(easy.rect_px.origin.x, 195.0);
        assert_eq!(
            easy.color,
            BRIGHT.to_color(),
            "a level done on Easy lights the header"
        );
        let hard = draw
            .texts
            .iter()
            .find(|t| t.text == TextRef::Id(TextId(0x3D7)))
            .unwrap();
        assert_eq!(hard.color, DARK.to_color());
        // Runtime texts: "2" right-aligned at the Easy header's right edge 235 on line 1 (224).
        let two = draw
            .texts
            .iter()
            .find(|t| {
                draw.dynamic
                    .iter()
                    .any(|(w, b)| *w == t.widget && b == b"2")
            })
            .unwrap();
        assert_eq!(
            (
                two.align,
                two.rect_px.origin.x + two.rect_px.size.x,
                two.rect_px.origin.y
            ),
            (Align::End, 235.0, 224.0)
        );
        let values: Vec<&[u8]> = draw.dynamic.iter().map(|(_, b)| b.as_slice()).collect();
        for expected in [&b"2/13"[..], b"0/13", b"1/21", b"0/27", b"12.3 %"] {
            assert!(values.contains(&expected), "{expected:?} missing");
        }
        // Icons: three, each with a shadow first; x = int(40 + 112 - 16) = 136, line 1 at y 224 + 4.
        assert_eq!(draw.sprites.len(), 6);
        assert_eq!(draw.sprites[1].rect_px.origin, Vec2::new(136.0, 228.0));
        assert_eq!(draw.sprites[0].color.0[0], 0.0);
        assert_eq!(draw.sprites[5].rect_px.origin.x, 136.0 - 14.0);
        // Complete label right-aligned to xr - 15 units on line 5.
        let complete = draw
            .texts
            .iter()
            .find(|t| t.text == TextRef::Id(TextId(COMPLETE_TEXT)))
            .unwrap();
        assert_eq!(
            complete.rect_px.origin.x + complete.rect_px.size.x,
            527.0 - 96.0
        );
        assert_eq!(complete.rect_px.origin.y, 304.0);
    }
}
