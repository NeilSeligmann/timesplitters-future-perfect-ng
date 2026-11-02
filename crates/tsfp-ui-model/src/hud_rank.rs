//! T2312 increment 4: the Arcade rank box as native Rust.
//!
//! The original (MEASURED from the lift and a draw-call probe, docs/t2312-native-hud.md, "Rank box"):
//!
//! * `0x12EC20` (register arguments: `eax` = the player's record, `esi` = pointer to the HUD column's
//!   row cursor; stack arguments: x offset (int), alpha (f32)) formats the rank of record `+0xA68`
//!   with `0x656A0` (ordinal strings, "Joint 1st" / "=" on a tie), fetches the label string id
//!   `0x1653` and calls the labelled row `0x12E3F0`, then adds the row height it returns to `*esi`.
//! * `0x12E3F0(x, y, label, value, alpha, flag, colour)` draws the row frame (texture `newHud_box`:
//!   four corners, a bottom strip and, without a label, a top strip), a nine-slice background panel
//!   (`0x12CC70`, texture `newHud_cornerGradient`, plus a screen-copy pass), the label fitted to the frame
//!   and the value centred, each through the outlined string draw `0x41E30` (four outline passes, then
//!   the text), and returns the row height.
//!
//! [`RankBox::read`] replays `0x12EC20` and [`labelled_row`] replays `0x12E3F0` on typed values; the
//! shell draws the result and the C gate keeps the row cursor advance. Nothing is written to the guest.
use crate::hud_native::{GuestCall, GuestRead};

/// Guest addresses the rank box reads (MEASURED from the lift of `0x12EC20`, `0x656A0`, `0x12E3F0`).
pub mod rank_addr {
    /// The rank box drawer (register arguments, see the module doc).
    pub const FN_RANK_BOX: u32 = 0x0012_ec20;
    /// Player record: rank (i32, 1 = first; `0x8000_0001` = none).
    pub const RECORD_RANK: u32 = 0xa68;
    pub const RANK_NONE: u32 = 0x8000_0001;
    /// Tie predicate (more than one connected player in state 1 shares the leading score), no arguments.
    pub const FN_TIED: u32 = 0x0006_5600;
    /// Language index (0..4) and the ordinal table: five string pointers per language (`0th`, `1st`,
    /// `2nd`, `3rd`, and the suffix for larger ranks).
    pub const LANGUAGE: u32 = 0x004c_0454;
    pub const ORDINALS: u32 = 0x004d_2788;
    /// printf formats of the original (`%s`-like, the tie form, number plus suffix, no rank).
    pub const FMT_TEXT: u32 = 0x0047_5d88;
    pub const FMT_TIED: u32 = 0x0047_8874;
    pub const FMT_NUMBER: u32 = 0x0047_8868;
    pub const FMT_NONE: u32 = 0x0047_8870;
    /// The label's string id (bank string, read from the disc by the shell).
    pub const LABEL_ID: u32 = 0x1653;
    /// The row colour word the rank box passes (R, G, B, A high to low, 0x80 = 1.0).
    pub const ROW_COLOUR: u32 = 0x7f7f_7f60;
    /// Viewport pointer (`+0` x0, `+0x14` y0) and the HUD rectangle (`+0` left, `+4` top margins).
    pub const VIEWPORT_PTR: u32 = 0x007e_71c8;
    pub const HUD_RECT: u32 = 0x007a_6140;
    /// HUD layout scales (x, y) and the HUD font object the row uses.
    pub const SCALE_X: u32 = 0x004e_68f4;
    pub const SCALE_Y: u32 = 0x004e_68f8;
    pub const ROW_FONT: u32 = 0x004e_7278;
    /// Recolour-mode predicate (`0x12CB40`, game modes with a different HUD colour path): legacy then.
    pub const FN_RECOLOUR_MODE: u32 = 0x0012_cb40;
    /// Font tables: objects of 20 bytes (`+0` record, `+0xC` scale y), records of 0x38 bytes
    /// (`+0x18` the line top used by `0x3F8D0`), the alternative-table switch and the text scales.
    pub const TEXTURE_ROWS: u32 = 0x004d_2d64;
    pub const FRAME_TEXTURE_ROW: u32 = 0xdb;
    pub const TEXTURE_RECORDS_PTR: u32 = 0x004b_85c8;
    pub const FONT_OBJECTS: u32 = 0x004c_8de8;
    pub const FONT_RECORDS: u32 = 0x004c_8c28;
    pub const FONT_TABLE_SWITCH: u32 = 0x006b_93a8;
    pub const FONT_HEIGHT_UNITS: u32 = 0x004e_8cf4;
    pub const FONT_HEIGHT_FACTOR: u32 = 0x0047_8390;
    pub const TEXT_SCALE_TOP: u32 = 0x007e_7274;
    pub const TEXT_SCALE_Y: u32 = 0x007e_7280;
}

/// Row constants of `0x12E3F0` and `0x12CC70` (f32 values of the original, MEASURED by the probe run;
/// addresses in brackets).
pub mod row {
    /// Row height in units of the y scale (`0x47DA5C`), the frame width (`0x47D1E0`).
    pub const HEIGHT: f32 = 42.0;
    pub const WIDTH: f32 = 64.0;
    /// Frame top above y (`0x475D30`) and the bottom piece's lower edge below y (`0x47D9E0`).
    pub const TOP: f32 = 6.0;
    pub const BOTTOM: f32 = 36.0;
    /// Frame alpha per unit of row alpha (`0x47D1E8`), label and value alpha (`0x47D1DC`), outline alpha
    /// (`0x47D1E0`), panel alpha (`0x47DA48`).
    pub const FRAME_ALPHA: f32 = 48.0;
    pub const TEXT_ALPHA: f32 = 96.0;
    pub const OUTLINE_ALPHA: f32 = 64.0;
    pub const PANEL_ALPHA: f32 = 72.0;
    /// Label drop below the frame top (`0x47862C`), panel height (`0x47CFF4`).
    pub const LABEL_DROP: f32 = 10.0;
    pub const PANEL_HEIGHT: f32 = 30.0;
    /// Label fit: fill this share of the gap between the corners (`0x476044`), never below this x scale (`0x4785E0`).
    pub const FIT_FILL: f32 = 0.85;
    pub const FIT_MIN: f32 = 0.7;
    /// Panel corner size cap (`0x475D08`) and outset per unit of x scale (`0x47D1E4`).
    pub const PANEL_CORNER: f32 = 32.0;
    pub const PANEL_OUTSET: f32 = 8.0;
    /// Outline pass offsets of `0x41E30` (`0x475D10` x, `0x475C78` y), design pixels.
    pub const OUTLINE_DX: f32 = 2.0;
    pub const OUTLINE_DY: f32 = 1.0;
    /// Texture names of ids `0xDB` and `0xEF` (the `0x80F20` argument; the name is in the row before,
    /// id = name-table row + 1 as for the reticles; increment 4 read the row of the id itself and drew
    /// `newHud_bagBase` and `newHud_powerupCloak`, corrected in increment 5).
    pub const FRAME_TEXTURE: &str = "newHud_box";
    pub const PANEL_TEXTURE: &str = "newHud_cornerGradient";
}

/// Width of a string in the original font tables (design pixels at text scale 1).
pub trait TextWidth {
    fn width(&self, font: u32, text: &[u8]) -> Option<f32>;
}

/// Values of the running game the labelled row reads besides its arguments.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowEnv {
    pub scale_x: f32,
    pub scale_y: f32,
    /// Font object index the row switches to (`*0x4E7278`).
    pub font: u32,
    /// `newHud_bagBase` size in texels.
    pub frame_texels: [f32; 2],
    /// Top limit: viewport y0 plus the HUD rectangle's top margin.
    pub top_limit: f32,
    /// The text line top of the row font (what `0x3F8D0` stores at `0x7E7264` after the font switch).
    pub text_top: f32,
}

impl RowEnv {
    /// Reads the environment; `frame_texels` comes from the loaded texture. None when unreadable or
    /// when the font tables use the alternative table (not replayed).
    pub fn read<G: GuestRead + ?Sized>(g: &G) -> Option<Self> {
        use rank_addr::*;
        // Texture record of table row 0xDB (`0x80F20`): size at +0x26 / +0x2A (16 x 16 MEASURED; the
        // disc image is 32 x 32, the original lays the row out with the record size).
        let index = g.read32(TEXTURE_ROWS + FRAME_TEXTURE_ROW * 0x18)?;
        let record = g
            .read32(g.read32(TEXTURE_RECORDS_PTR)?)?
            .checked_add(index.checked_mul(0x34)?)?;
        let frame_texels = [
            g.read16(record + 0x26)? as f32,
            g.read16(record + 0x2a)? as f32,
        ];
        let font = g.read32(ROW_FONT)?;
        let viewport = g.read32(VIEWPORT_PTR).filter(|&p| p != 0)?;
        let top_limit = g.read32(viewport + 0x14)? as i32 + g.read32(HUD_RECT + 4)? as i32;
        Some(RowEnv {
            scale_x: g.read_f32(SCALE_X)?,
            scale_y: g.read_f32(SCALE_Y)?,
            font,
            frame_texels,
            top_limit: top_limit as f32,
            text_top: font_text_top(g, font)?,
        })
    }
}

/// `0x3F8D0` for font object `font` (the line top `0x7E7264`): record `+0x18` times the top scale times
/// the object's height (height units times factor times object scale y times the text y scale).
pub fn font_text_top<G: GuestRead + ?Sized>(g: &G, font: u32) -> Option<f32> {
    use rank_addr::*;
    if g.read32(FONT_TABLE_SWITCH)? != 0 {
        return None;
    }
    let object = FONT_OBJECTS + font.checked_mul(20)?;
    let record = FONT_RECORDS + g.read32(object)?.checked_mul(0x38)?;
    let height = g.read32(FONT_HEIGHT_UNITS)? as i32 as f32
        * g.read_f32(FONT_HEIGHT_FACTOR)?
        * g.read_f32(object + 0xc)?
        * g.read_f32(TEXT_SCALE_Y)?;
    Some(g.read_f32(record + 0x18)? * g.read_f32(TEXT_SCALE_TOP)? * height)
}

/// One labelled-row call (the arguments of `0x12E3F0`).
#[derive(Clone, Debug, PartialEq)]
pub struct RowInput {
    pub x: f32,
    pub y: f32,
    pub label: Option<Vec<u8>>,
    pub value: Option<Vec<u8>>,
    /// Row alpha (0..1, fades the row in).
    pub alpha: f32,
    /// Colour word of the value (R, G, B, A high to low).
    pub colour: u32,
}

/// One textured piece as the original passes it: `rect` = x0, y0, x1, y1 in design pixels (x0 > x1
/// or y0 > y1 mirrors the texture), `uv` normalised (u0 = u1 stretches one texel column).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowPiece {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
}

/// One outlined string (`0x41E30`): pen position (design pixels), x scale, text colour word and the
/// outline alpha byte (outline colour black).
#[derive(Clone, Debug, PartialEq)]
pub struct RowText {
    pub bytes: Vec<u8>,
    pub x: f32,
    pub y: f32,
    pub scale_x: f32,
    pub colour: u32,
    pub outline_alpha: u8,
}

/// Everything one labelled row draws, in the original's order (panel, frame, label, value), and the
/// cursor advance it returns.
#[derive(Clone, Debug, PartialEq)]
pub struct LabelledRowDraw {
    pub panel: Vec<RowPiece>,
    /// Panel vertex alpha byte (`0x1F090` argument, the colour preset leaves the colour white).
    pub panel_alpha: u8,
    pub frame: Vec<RowPiece>,
    /// Frame colour word (R, G, B, A high to low).
    pub frame_colour: u32,
    pub texts: Vec<RowText>,
    /// Font object of the texts (the row font).
    pub font: u32,
    pub advance: u32,
}

/// x87/SSE truncation of the original's float-to-int conversions.
fn trunc(v: f32) -> i32 {
    v as i32
}

/// `0x12CC70(x0, y0, x1, y1, alpha, 2)`: the nine-slice panel (integer rectangles, corner texture
/// stretched by its edge texels). The screen-copy pass that follows is not replayed.
pub fn panel_pieces(env: &RowEnv, rect: [f32; 4]) -> Vec<RowPiece> {
    panel_pieces_scaled(env.scale_x, rect)
}

/// [`panel_pieces`] with the HUD x scale given (the weapon slot panel `0x141E30` calls `0x12CC70` too, T2312 increment 8).
pub fn panel_pieces_scaled(scale_x: f32, [x0, y0, x1, y1]: [f32; 4]) -> Vec<RowPiece> {
    let (w, h) = (x1 - x0, y1 - y0);
    let half = if h > w { w } else { h } * 0.5;
    let c = if half > row::PANEL_CORNER {
        row::PANEL_CORNER
    } else {
        half
    };
    let o = scale_x * row::PANEL_OUTSET;
    let (x0o, x1o) = (x0 - o, x1 + o);
    let (x0i, x1i, y0i, y1i) = (x0o + c, x1o - c, y0 + c, y1 - c);
    let i = |v: f32| trunc(v) as f32;
    let (full, edge_u, edge_v, texel) = (
        [0.0, 0.0, 1.0, 1.0],
        [1.0, 0.0, 1.0, 1.0],
        [0.0, 1.0, 1.0, 1.0],
        [1.0, 1.0, 1.0, 1.0],
    );
    [
        ([x0o, y0, x0i, y0i], full),
        ([x1o, y0, x1i, y0i], full),
        ([x0o, y1, x0i, y1i], full),
        ([x1o, y1, x1i, y1i], full),
        ([x0i, y0, x1i, y0i], edge_u),
        ([x0i, y1, x1i, y1i], edge_u),
        ([x0o, y0i, x0i, y1i], edge_v),
        ([x1o, y0i, x1i, y1i], edge_v),
        ([x0i, y0i, x1i, y1i], texel),
    ]
    .into_iter()
    .map(|(r, uv)| RowPiece { rect: r.map(i), uv })
    .filter(|p| p.rect[0] != p.rect[2] && p.rect[1] != p.rect[3])
    .collect()
}

/// `0x12E3F0` replayed (recolour mode and the extra flag are not: the caller keeps those legacy).
pub fn labelled_row(
    env: &RowEnv,
    input: &RowInput,
    widths: &dyn TextWidth,
) -> Option<LabelledRowDraw> {
    let (sx, sy) = (env.scale_x, env.scale_y);
    let [tw, th] = env.frame_texels;
    let x = input.x;
    let mut y = input.y;
    let mut advance = trunc(sy * row::HEIGHT);
    let total = sx * row::WIDTH;
    let x_end = x + total;
    let x_left = x + tw * sx;
    let x_right = x + (row::WIDTH - tw) * sx;
    let mut top = y - sy * row::TOP;
    let mut top_inner = y + (th - row::TOP) * sy;
    let mut bottom_inner = y + (row::BOTTOM - th) * sy;
    let mut bottom = y + sy * row::BOTTOM;
    // Keep the row below the top limit: everything moves down and the advance grows.
    let label_top = top - env.text_top + sy * row::LABEL_DROP;
    if env.top_limit > label_top {
        let d = env.top_limit - label_top;
        if d > 0.0 {
            y += d;
            top += d;
            top_inner += d;
            bottom_inner += d;
            bottom += d;
            advance += trunc(d);
        }
    }
    let frame_alpha = (trunc(input.alpha * row::FRAME_ALPHA) & 0xff) as u32;
    let panel = panel_pieces(env, [x, y, x_end, y + sy * row::PANEL_HEIGHT]);
    let (full, strip) = ([0.0, 0.0, 1.0, 1.0], [1.0, 0.0, 1.0, 1.0]);
    let mut frame = vec![
        RowPiece {
            rect: [x, top, x_left, top_inner],
            uv: full,
        },
        RowPiece {
            rect: [x_end, top, x_right, top_inner],
            uv: full,
        },
        RowPiece {
            rect: [x, bottom, x_left, bottom_inner],
            uv: full,
        },
        RowPiece {
            rect: [x_end, bottom, x_right, bottom_inner],
            uv: full,
        },
    ];
    if input.label.is_none() {
        frame.push(RowPiece {
            rect: [x_left, top, x_right, top_inner],
            uv: strip,
        });
    }
    frame.push(RowPiece {
        rect: [x_left, bottom, x_right, bottom_inner],
        uv: strip,
    });
    let text_alpha = (trunc(input.alpha * row::TEXT_ALPHA) & 0xff) as u32;
    let outline_alpha = (trunc(input.alpha * row::OUTLINE_ALPHA) & 0xff) as u8;
    let mut texts = Vec::new();
    if let Some(label) = &input.label {
        // MEASURED: the original's strings are scale_x (1.25) wider than the native measure.
        let width = widths.width(env.font, label)? * env.scale_x;
        if width <= 0.0 {
            return None;
        }
        let gap = x_right - x_left;
        let ratio = gap / width * row::FIT_FILL;
        let (target, left_aligned) = if row::FIT_MIN <= ratio {
            (gap * row::FIT_FILL, false)
        } else {
            let target = width * row::FIT_MIN;
            (target, target > total)
        };
        let scale_x = target / width;
        let pen_x = if left_aligned {
            x
        } else {
            (x_right + x_left - width * scale_x) * 0.5
        };
        texts.push(RowText {
            bytes: label.clone(),
            x: pen_x,
            y: top - env.text_top + sy * row::LABEL_DROP,
            scale_x: scale_x * env.scale_x,
            colour: 0x7f7f_7f00 | text_alpha,
            outline_alpha,
        });
    }
    if let Some(value) = &input.value {
        let width = widths.width(env.font, value)? * env.scale_x;
        let value_alpha = (trunc((input.colour & 0xff) as f32 * input.alpha) & 0xff) as u32;
        texts.push(RowText {
            bytes: value.clone(),
            x: (x_right + x_left - width) * 0.5,
            y: (bottom_inner + top_inner - env.text_top) * 0.5,
            scale_x: env.scale_x,
            colour: (input.colour & 0xffff_ff00) | value_alpha,
            outline_alpha,
        });
    }
    Some(LabelledRowDraw {
        panel,
        panel_alpha: (trunc(input.alpha * row::PANEL_ALPHA) & 0xff) as u8,
        frame,
        frame_colour: 0x7f7f_7f00 | frame_alpha,
        texts,
        font: env.font,
        advance: advance as u32,
    })
}

/// A NUL-terminated string of the running image (static data: formats and ordinal strings).
pub fn guest_string<G: GuestRead + ?Sized>(g: &G, va: u32) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    for i in 0..128 {
        let at = va.checked_add(i)?;
        let byte = (g.read32(at & !3)? >> ((at & 3) * 8)) as u8;
        if byte == 0 {
            return Some(out);
        }
        out.push(byte);
    }
    None
}

/// The original's printf subset the rank text uses: `%s` and `%d` (and `%%`); anything else is None.
pub fn format(fmt: &[u8], args: &[FormatArg]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut next = args.iter();
    let mut bytes = fmt.iter();
    while let Some(&b) = bytes.next() {
        if b != b'%' {
            out.push(b);
            continue;
        }
        match bytes.next()? {
            b'%' => out.push(b'%'),
            b's' => match next.next()? {
                FormatArg::Text(t) => out.extend_from_slice(t),
                FormatArg::Int(_) => return None,
            },
            b'd' => match next.next()? {
                FormatArg::Int(v) => out.extend_from_slice(v.to_string().as_bytes()),
                FormatArg::Text(_) => return None,
            },
            _ => return None,
        }
    }
    Some(out)
}

#[derive(Clone, Debug, PartialEq)]
pub enum FormatArg {
    Text(Vec<u8>),
    Int(i32),
}

/// `0x656A0(buffer, rank, 0, 1)` as the rank box calls it: the ordinal text of `rank`.
pub fn rank_text<G: GuestCall + ?Sized>(g: &G, rank: u32) -> Option<Vec<u8>> {
    use rank_addr::*;
    let language = g.read32(LANGUAGE)?;
    let ordinal = |index: u32| -> Option<Vec<u8>> {
        let pointer = g.read32(ORDINALS + 4 * (language.checked_mul(5)? + index))?;
        guest_string(g, pointer)
    };
    let string = |va: u32| guest_string(g, va);
    // Rank 1 and a tie: the tie form of "1st" (kind 1 as the rank box passes it).
    if rank == 1 && g.call(FN_TIED, &[])? != 0 {
        return format(&string(FMT_TIED)?, &[FormatArg::Text(ordinal(1)?)]);
    }
    if (rank as i32) >= 0 && (rank as i32) <= 3 {
        return format(&string(FMT_TEXT)?, &[FormatArg::Text(ordinal(rank)?)]);
    }
    if rank == RANK_NONE {
        return format(&string(FMT_NONE)?, &[]);
    }
    format(
        &string(FMT_NUMBER)?,
        &[FormatArg::Int(rank as i32), FormatArg::Text(ordinal(4)?)],
    )
}

/// One rank box call: the row as the original would draw it.
#[derive(Clone, Debug, PartialEq)]
pub struct RankBox {
    pub row: LabelledRowDraw,
}

/// What the gate does with a rank box call.
#[derive(Clone, Debug, PartialEq)]
pub enum RankCall {
    Draw(RankBox),
    /// A case the native HUD does not reproduce (recolour mode, unreadable state): the original runs.
    Legacy,
}

impl RankBox {
    /// Replays `0x12EC20` for `record` (`eax`), the row cursor `cursor` (`esi`, points at the row's y)
    /// and the stack arguments at `esp` (return slot):
    /// `[esp+4]` x offset (int), `[esp+8]` alpha (f32). `label` is string id `0x1653` from the bank.
    pub fn read<G: GuestCall + ?Sized>(
        g: &G,
        record: u32,
        cursor: u32,
        esp: u32,
        label: Option<Vec<u8>>,
        widths: &dyn TextWidth,
    ) -> RankCall {
        Self::read_input(g, record, cursor, None, esp, label, widths)
    }

    /// Owned column position, with no guest cursor read or write.
    pub fn read_owned<G: GuestCall + ?Sized>(
        g: &G,
        record: u32,
        top: i32,
        esp: u32,
        label: Option<Vec<u8>>,
        widths: &dyn TextWidth,
    ) -> RankCall {
        Self::read_input(g, record, 0, Some(top), esp, label, widths)
    }

    fn read_input<G: GuestCall + ?Sized>(
        g: &G,
        record: u32,
        cursor: u32,
        owned_top: Option<i32>,
        esp: u32,
        label: Option<Vec<u8>>,
        widths: &dyn TextWidth,
    ) -> RankCall {
        use rank_addr::*;
        let read = || -> Option<RankBox> {
            if g.call(FN_RECOLOUR_MODE, &[])? != 0 {
                return None;
            }
            let x_offset = g.read32(esp + 4)? as i32;
            let alpha = g.read_f32(esp + 8)?;
            let rank = g.read32(record.checked_add(RECORD_RANK)?)?;
            let value = rank_text(g, rank)?;
            let viewport = g.read32(VIEWPORT_PTR).filter(|&p| p != 0)?;
            let x = (g.read32(viewport)? as i32)
                .wrapping_add(g.read32(HUD_RECT)? as i32)
                .wrapping_add(x_offset);
            let y = match owned_top {
                Some(top) => top,
                None => g.read32(cursor)? as i32,
            };
            let env = RowEnv::read(g)?;
            Some(RankBox {
                row: labelled_row(
                    &env,
                    &RowInput {
                        x: x as f32,
                        y: y as f32,
                        label: Some(label?),
                        value: Some(value),
                        alpha,
                        colour: ROW_COLOUR,
                    },
                    widths,
                )?,
            })
        };
        match read() {
            Some(rank) => RankCall::Draw(rank),
            None => RankCall::Legacy,
        }
    }
}

/// The Challenge points row (`0x12ECA0`, T2312 increment 8): register `esi` the row cursor pointer, `[esp+4]` the
/// alpha. `0x22D110(&label, &value, 1)` gives the label from the text lines at `0x75C770` (row 1, 255 bytes per row)
/// and the value from `0x75C370` (row `1 + 2 * slot`), each absent when empty; with a label the row is the labelled
/// row `0x12E3F0(x0 + left, *esi, label, value, alpha, 0, 0x7F7F7F60)` and the cursor advances by its height.
pub mod points_addr {
    pub const FN_POINTS_ROW: u32 = 0x0012_eca0;
    pub const LABELS: u32 = 0x0075_c770;
    pub const VALUES: u32 = 0x0075_c370;
    pub const ROW: u32 = 1;
    pub const LINE_SIZE: u32 = 0xff;
    pub const VIEWER: u32 = 0x007b_0c7c;
}

/// What the points row gate does.
#[derive(Clone, Debug, PartialEq)]
pub enum PointsCall {
    Draw(RankBox),
    /// No label: the original draws nothing and leaves the cursor.
    Nothing,
    Legacy,
}

/// Replays `0x12ECA0` (see [`points_addr`]).
pub fn points_row<G: GuestCall + ?Sized>(
    g: &G,
    cursor: u32,
    esp: u32,
    widths: &dyn TextWidth,
) -> PointsCall {
    points_row_input(g, cursor, None, esp, widths)
}

pub fn points_row_owned<G: GuestCall + ?Sized>(
    g: &G,
    top: i32,
    esp: u32,
    widths: &dyn TextWidth,
) -> PointsCall {
    points_row_input(g, 0, Some(top), esp, widths)
}

fn points_row_input<G: GuestCall + ?Sized>(
    g: &G,
    cursor: u32,
    owned_top: Option<i32>,
    esp: u32,
    widths: &dyn TextWidth,
) -> PointsCall {
    use points_addr::*;
    let read = || -> Option<PointsCall> {
        if g.call(rank_addr::FN_RECOLOUR_MODE, &[])? != 0 {
            return None;
        }
        let label = guest_string(g, LABELS + ROW * LINE_SIZE)?;
        if label.is_empty() {
            return Some(PointsCall::Nothing);
        }
        let slot = g.read32(g.read32(VIEWER)? + 4)?;
        let value = guest_string(
            g,
            VALUES + (ROW + slot.wrapping_mul(2)).wrapping_mul(LINE_SIZE),
        )?;
        let alpha = g.read_f32(esp + 4)?;
        let viewport = g.read32(rank_addr::VIEWPORT_PTR).filter(|&p| p != 0)?;
        let x = (g.read32(viewport)? as i32).wrapping_add(g.read32(rank_addr::HUD_RECT)? as i32);
        let y = match owned_top {
            Some(top) => top,
            None => g.read32(cursor)? as i32,
        };
        let env = RowEnv::read(g)?;
        let input = RowInput {
            x: x as f32,
            y: y as f32,
            label: Some(label),
            value: (!value.is_empty()).then_some(value),
            alpha,
            colour: rank_addr::ROW_COLOUR,
        };
        Some(PointsCall::Draw(RankBox {
            row: labelled_row(&env, &input, widths)?,
        }))
    };
    read().unwrap_or(PointsCall::Legacy)
}

#[cfg(test)]
mod tests {
    //! Tier 0: the row replay against the MEASURED probe values of one Arcade frame (frame 2700:
    //! x 32, y 24, scales 1.25 x 1, font 19, frame texture 16 x 16, line top 18.26786, label width
    //! 48.94 at scale 1); rank text with FABRICATED strings on the real addresses.
    use super::*;
    use std::collections::HashMap;

    struct Widths;
    impl TextWidth for Widths {
        fn width(&self, _font: u32, text: &[u8]) -> Option<f32> {
            Some(match text {
                b"RANK" => 48.9375 / 1.25,
                b"5th" => 26.4375 / 1.25,
                _ => 8.0 * text.len() as f32,
            })
        }
    }

    fn env() -> RowEnv {
        RowEnv {
            scale_x: 1.25,
            scale_y: 1.0,
            font: 19,
            frame_texels: [16.0, 16.0],
            top_limit: 24.0,
            text_top: 18.26786,
        }
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn row_matches_the_probed_draw_calls() {
        let input = RowInput {
            x: 32.0,
            y: 24.0,
            label: Some(b"RANK".to_vec()),
            value: Some(b"5th".to_vec()),
            alpha: 1.0,
            colour: 0x7f7f7f60,
        };
        let draw = labelled_row(&env(), &input, &Widths).unwrap();
        // 0x1BA70 rectangles of the probe (corners mirrored by coordinate order, then the bottom strip).
        let want = [
            [32.0, 32.2679, 52.0, 48.2679],
            [112.0, 32.2679, 92.0, 48.2679],
            [32.0, 74.2679, 52.0, 58.2679],
            [112.0, 74.2679, 92.0, 58.2679],
            [52.0, 74.2679, 92.0, 58.2679],
        ];
        assert_eq!(draw.frame.len(), want.len());
        for (piece, want) in draw.frame.iter().zip(want) {
            assert!(
                piece.rect.iter().zip(want).all(|(a, b)| close(*a, b)),
                "{:?} vs {want:?}",
                piece.rect
            );
        }
        assert_eq!(draw.frame[4].uv, [1.0, 0.0, 1.0, 1.0]);
        assert_eq!(draw.frame_colour, 0x7f7f7f30);
        // 0x1F090 rectangles of the probe (zero-height middle row dropped).
        let panel: Vec<[f32; 4]> = draw.panel.iter().map(|p| p.rect).collect();
        assert_eq!(
            panel,
            vec![
                [22.0, 38.0, 37.0, 53.0],
                [122.0, 38.0, 107.0, 53.0],
                [22.0, 68.0, 37.0, 53.0],
                [122.0, 68.0, 107.0, 53.0],
                [37.0, 38.0, 107.0, 53.0],
                [37.0, 68.0, 107.0, 53.0],
            ]
        );
        assert_eq!(draw.panel_alpha, 72);
        // 0x41E30 calls: label at (54.8719, 24) scale 0.7, value at (58.7812, 44.1339).
        assert_eq!(draw.texts.len(), 2);
        let (label, value) = (&draw.texts[0], &draw.texts[1]);
        assert!(
            close(label.x, 54.8719) && close(label.y, 24.0) && close(label.scale_x, 0.875),
            "{label:?}"
        );
        assert!(
            close(value.x, 58.7812) && close(value.y, 44.1339) && value.scale_x == 1.25,
            "{value:?}"
        );
        assert_eq!((label.colour, label.outline_alpha), (0x7f7f7f60, 64));
        assert_eq!((value.colour, value.outline_alpha), (0x7f7f7f60, 64));
        // 42 plus the 14.27 shift (truncated).
        assert_eq!(draw.advance, 56);
    }

    #[test]
    fn row_fades_in_and_without_a_label_draws_the_top_strip() {
        let input = RowInput {
            x: 32.0,
            y: 24.0,
            label: None,
            value: Some(b"1st=".to_vec()),
            alpha: 0.0,
            colour: 0x7f7f7f60,
        };
        let draw = labelled_row(&env(), &input, &Widths).unwrap();
        assert_eq!(draw.frame.len(), 6);
        assert_eq!(draw.frame[4].rect[0], 52.0);
        assert_eq!((draw.frame_colour, draw.panel_alpha), (0x7f7f7f00, 0));
        assert_eq!(
            (draw.texts[0].colour, draw.texts[0].outline_alpha),
            (0x7f7f7f00, 0)
        );
        // Below the top limit nothing moves.
        let low = RowInput { y: 200.0, ..input };
        assert_eq!(labelled_row(&env(), &low, &Widths).unwrap().advance, 42);
    }

    #[test]
    fn a_long_label_keeps_the_minimum_scale() {
        let input = RowInput {
            x: 32.0,
            y: 200.0,
            label: Some(b"VERYLONGLABEL".to_vec()),
            value: None,
            alpha: 1.0,
            colour: 0x7f7f7f60,
        };
        let draw = labelled_row(&env(), &input, &Widths).unwrap();
        // 130 * 0.7 = 91 > 80: left aligned at x.
        assert!(close(draw.texts[0].scale_x, 0.875) && draw.texts[0].x == 32.0);
    }

    struct Guest(HashMap<u32, u32>);
    impl GuestRead for Guest {
        fn read32(&self, va: u32) -> Option<u32> {
            self.0.get(&va).copied()
        }
    }
    impl GuestCall for Guest {
        fn call(&self, va: u32, _args: &[u32]) -> Option<u32> {
            self.0.get(&(0xf000_0000 + va)).copied()
        }
    }

    fn put_string(m: &mut HashMap<u32, u32>, va: u32, s: &[u8]) {
        let mut bytes = s.to_vec();
        bytes.push(0);
        while bytes.len() % 4 != 0 {
            bytes.push(0);
        }
        for (i, chunk) in bytes.chunks(4).enumerate() {
            m.insert(
                va + 4 * i as u32,
                u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]),
            );
        }
    }

    #[test]
    fn rank_text_follows_the_original_cases() {
        use rank_addr::*;
        let mut m = HashMap::new();
        m.insert(LANGUAGE, 1);
        for (i, s) in [&b"0e"[..], b"1e", b"2e", b"3e", b"e"].iter().enumerate() {
            let at = 0x0090_0000 + 0x10 * i as u32;
            put_string(&mut m, at, s);
            m.insert(ORDINALS + 4 * (5 + i as u32), at);
        }
        put_string(&mut m, FMT_TEXT, b"%s");
        put_string(&mut m, FMT_TIED, b"=%s");
        put_string(&mut m, FMT_NUMBER, b"%d%s");
        put_string(&mut m, FMT_NONE, b"-");
        m.insert(0xf000_0000 + FN_TIED, 0);
        let g = Guest(m);
        assert_eq!(rank_text(&g, 2).unwrap(), b"2e");
        assert_eq!(rank_text(&g, 1).unwrap(), b"1e");
        assert_eq!(rank_text(&g, 7).unwrap(), b"7e");
        assert_eq!(rank_text(&g, RANK_NONE).unwrap(), b"-");
        let mut tied = g.0.clone();
        tied.insert(0xf000_0000 + FN_TIED, 1);
        assert_eq!(rank_text(&Guest(tied), 1).unwrap(), b"=1e");
        assert_eq!(format(b"%x", &[]), None);
    }
}
