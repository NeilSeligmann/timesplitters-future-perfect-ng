//! T2312 increment 7: the HUD message queue (`0x14AC80`, "Got the ..." pickups, slide-in titles) as native Rust.
//!
//! The original (MEASURED from the XBE disassembly, docs/t2312-native-hud.md, "Message queue"): the HUD frame
//! `0x14F0B0` calls `0x14AC80(only_kind_9)` once per frame. It walks the viewer's 16 ring entries (HUD record
//! `0x7A6160 + slot * 0x177C`, entries at `+0x38`, 0x16C bytes, starting at the ring cursor `+0x30`), skips unused
//! entries, entries whose age is below 0 and entries that conflict with what is already shown (`0x12DF00`: the
//! per-kind weight sum would pass the kind's limit `0x7490AC + 4 * kind`, or an already shown kind pairs with this
//! one in the table `0x4E72E4..0x4E7348`), then draws the entry by kind with a sub-drawer and adds its weight to the
//! per-kind sums (`0x12DF60`, kind 8 excepted):
//!
//! | kind | drawer | what |
//! | --- | --- | --- |
//! | 1 | `0x149590` | centred notice (not seen in the routes, to the original) |
//! | 2 | `0x149810` | pickup line at the bottom, stacked by the running offset ([`pickup`], native) |
//! | 3 | `0x14A280` | wrapped multi-line notice sliding in and out sideways ([`notice`], native since increment 8) |
//! | 4, 5 | `0x148EE0` | title sliding in from both sides ([`title`], native) |
//! | 6 | none | skipped without weight |
//! | 7 | `0x12D9C0` | pad notice (native one-shot action, typed progress draw, exact-three retirement) |
//! | 8 | `0x12D8E0` | corner notice (to the original) |
//! | 9 | `0x149710` | split-screen notice when `0x12CAD0` (to the original) |
//! | 10 | `0x149B30` | scoring notice: text and player name, team coloured ([`scoring`], native since increment 8 for one viewer; split screen to the original) |
//!
//! The owned path uses typed queue state and native scheduling for kinds 1 through 10, with unknown kinds falling
//! back to the legacy path. Kind 6 is a measured no-draw/no-weight entry; kinds 1 through 5 and 7 through 10 have
//! typed native presenters (kind 10 supports one viewer). Kind 7 consumes the input owner's one-shot eligible action
//! before the Rust progress draw and retires at exactly three. The runtime switch stays opt-in until the complete
//! lifecycle passes the owner-route adoption gate. The native path never writes the guest queue.
use crate::font::FontSet;
use crate::hud_native::GuestCall;
use crate::hud_prompts::{FontMetrics, PlacedIcon, TextEnv, TextRun, guest_cstring};
use crate::messages::{Message, RING_LEN, TEXT_BYTES};

/// Guest addresses (MEASURED from the disassembly of `0x14AC80` and its helpers).
pub mod message_addr {
    /// The queue drawer (one stack argument: nonzero draws kind 9 only).
    pub const FN_QUEUE: u32 = 0x0014_ac80;
    /// Sub-drawers by kind (index kind - 1; 0 = none).
    pub const KIND_DRAWERS: [u32; 10] = [
        0x0014_9590,
        0x0014_9810,
        0x0014_a280,
        0x0014_8ee0,
        0x0014_8ee0,
        0,
        0x0012_d9c0,
        0x0012_d8e0,
        0x0014_9710,
        0x0014_9b30,
    ];
    /// The outlined string draw `(x, y, text, colour, outline)` (observed in the shadow check).
    pub const FN_OUTLINED: u32 = 0x0004_1e30;
    /// The font select (`0x3FA10(font)`, returns the previous one).
    pub const FN_SELECT_FONT: u32 = 0x0003_fa10;
    /// The blend mode select (`0x19D70(mode)`).
    pub const FN_BLEND: u32 = 0x0001_9d70;
    /// Player count `0x1CDAE0` -> `0x3567C0` (pure query).
    pub const FN_PLAYERS: u32 = 0x0035_67c0;
    /// Text-mode query `0x12CB40` (title colours), split notice query `0x12CAD0`, `0x3C420` (split height).
    pub const FN_TEXT_MODE: u32 = 0x0012_cb40;
    pub const FN_SPLIT_NOTICE: u32 = 0x0012_cad0;
    pub const FN_SPLIT_TALL: u32 = 0x0003_c420;
    /// Viewer (`+4` slot), HUD records.
    pub const VIEWER: u32 = 0x007b_0c7c;
    pub const HUD_RECORDS: u32 = 0x007a_6160;
    pub const HUD_RECORD_SIZE: u32 = 0x177c;
    /// HUD record fields: flags (`& 3`: only the first pickup line), ring cursor, entries.
    pub const HUD_FLAGS: u32 = 0x8;
    pub const RING_CURSOR: u32 = 0x30;
    pub const RING_ENTRIES: u32 = 0x38;
    pub const ENTRY_SIZE: u32 = 0x16c;
    /// Entry fields: kind, text (0x140 bytes), age, fade start, slide time (kinds 4 and 5), end, weight.
    pub const KIND: u32 = 0;
    pub const TEXT: u32 = 4;
    pub const AGE: u32 = 0x144;
    pub const FADE_START: u32 = 0x148;
    pub const SLIDE: u32 = 0x14c;
    pub const END: u32 = 0x150;
    pub const EVICT_AFTER: u32 = 0x154;
    pub const WEIGHT: u32 = 0x158;
    pub const OPAQUE: u32 = 0x168;
    /// Per-kind weight limits (int, run time), conflict pairs (`0x4E72E4..=0x4E7344`, pairs of ints).
    pub const KIND_LIMITS: u32 = 0x0074_90ac;
    pub const CONFLICT_FIRST: u32 = 0x004e_72e4;
    pub const CONFLICT_LAST: u32 = 0x004e_7344;
    /// Viewport pointer (`+0` x0, `+4` x1, `+0xC` the widest drawable text, `+0x14` y0, `+0x18` y1), HUD margins
    /// (left, top, right, bottom), HUD layout scales (`0x4E68F4` x, `0x4E68F8` y).
    pub const VIEWPORT_PTR: u32 = 0x007e_71c8;
    pub const MARGIN_LEFT: u32 = 0x007a_6140;
    pub const MARGIN_TOP: u32 = 0x007a_6144;
    pub const MARGIN_RIGHT: u32 = 0x007a_6148;
    pub const MARGIN_BOTTOM: u32 = 0x007a_614c;
    pub const SCALE_X: u32 = 0x004e_68f4;
    pub const SCALE_Y: u32 = 0x004e_68f8;
    /// Mode byte and level player count of the bottom-line rule (`0x148680`).
    pub const MODE_BYTE: u32 = 0x007d_e455;
    pub const LEVEL_PLAYERS: u32 = 0x0079_0950;
    /// Font tables by player count (`table + 4 * players`): pickup (`0x149810`, `0x148680`), title (`0x148EE0`,
    /// `0x148830`), title base (`0x148760`), bottom notice.
    pub const FONTS_PICKUP: u32 = 0x004e_72a0;
    pub const FONTS_TITLE: u32 = 0x004e_7278;
    pub const FONTS_TITLE_BASE: u32 = 0x004e_72b4;
    pub const FONTS_SINGLE_BASE: u32 = 0x004e_728c;
    /// Kind 10: player records (`*0x7356D8 + 0xEA0 * entry[+0x168]`: `+0x70` name pointer, `+0x84` team), the team
    /// colour table (20 bytes per team), the game flags (`& 1` teams), the bottom row font table of `0x148910`.
    pub const PLAYERS_PTR: u32 = 0x0073_56d8;
    pub const PLAYER_SIZE: u32 = 0xea0;
    pub const PLAYER_NAME: u32 = 0x70;
    pub const PLAYER_TEAM: u32 = 0x84;
    pub const TEAM_COLOURS: u32 = 0x004d_1e28;
    pub const GAME_FLAGS: u32 = 0x007d_e458;
    /// The sine table (2048 f32, run time) the kind 3 slide reads.
    pub const SINE_TABLE: u32 = 0x007a_38a0;
}

/// Constants of the drawers (MEASURED, XBE `.rdata`).
pub mod k {
    /// Alpha byte scale (`0x475CC0`).
    pub const ALPHA: f32 = 127.0;
    /// Pickup colours: grey text, dark outline (alpha byte OR'd in).
    pub const PICKUP_TEXT: u32 = 0x6464_6400;
    pub const PICKUP_OUTLINE: u32 = 0x1e1e_1e00;
    /// Title colours (text mode: `0x40` text, `0x7F7F7F7F` outline, blend 1; else blend 2, additive).
    pub const TITLE_TEXT: u32 = 0x4069_697f;
    pub const TITLE_OUTLINE: u32 = 0x507f;
    pub const TITLE_TEXT_MODE_TEXT: u32 = 0x40;
    pub const TITLE_TEXT_MODE_OUTLINE: u32 = 0x7f7f_7f7f;
    /// Fit margin of `0x12D150` (one player 0x38, else 0x30, times the x scale, both sides).
    pub const FIT_MARGIN_ONE: i32 = 0x38;
    pub const FIT_MARGIN_SPLIT: i32 = 0x30;
    /// Bottom line (`0x148680`): `y - scale * k * 45 - scale * 6`, k 1 (split: 0.933 or 0.8).
    pub const BOTTOM_STEP: f32 = 45.0;
    pub const BOTTOM_GAP: f32 = 6.0;
    pub const SPLIT_TALL: f32 = 0.933;
    pub const SPLIT_SHORT: f32 = 0.8;
    /// Title base rows (`0x148760`, ebx = 3 from `0x148830`), gap `scale * 4`, the top limit (one player 0x28,
    /// else 0x22, minus `scale * -4`).
    pub const TITLE_ROWS: f32 = 3.0;
    pub const TITLE_GAP: f32 = 4.0;
    pub const TITLE_TOP_ONE: i32 = 0x28;
    pub const TITLE_TOP_SPLIT: i32 = 0x22;
    pub const TITLE_TOP_ADJUST: f32 = -4.0;
    /// Slide factor (`0x475D10`).
    pub const SLIDE: f32 = 2.0;
    /// Stacking: an entry older than `end * 0.9` shrinks its share of the offset (`0x476040`).
    pub const STACK_SHRINK: f32 = 0.9;
    /// Kind 3 slide: table index `trunc(t * pi / 2 (0x475CE4) * 2048 / 2pi (0x475CE0) + 0.5) & 0x7FF`, a quarter turn
    /// (0x200) more when sliding out; colours (text mode: `0x40` / `0x7F7F7F7F`; a line starting with `~`:
    /// `0x40787880` / `0x5080`); a line starting with a backquote drops half a line.
    pub const NOTICE_QUARTER: f32 = std::f32::consts::FRAC_PI_2;
    pub const NOTICE_TABLE_SCALE: f32 = 325.949_3;
    pub const NOTICE_TEXT: u32 = 0x7f7f_7f7f;
    pub const NOTICE_OUTLINE: u32 = 0x160a_3240;
    pub const NOTICE_HIGHLIGHT_TEXT: u32 = 0x4078_7880;
    pub const NOTICE_HIGHLIGHT_OUTLINE: u32 = 0x5080;
    /// Kind 10: text and outline colours (alpha byte OR'd in), the one-line limit (`0x47D568` of the column width),
    /// the rows of `0x148910` (`45` per `0x478900`, single viewer lift `6`, split `2`).
    pub const SCORING_TEXT: u32 = 0x7f7f_7f00;
    pub const SCORING_OUTLINE: u32 = 0x1e1e_1e00;
    pub const SCORING_ONE_LINE: f32 = 0.45;
    pub const SCORING_ROW_STEP: f32 = 45.0;
    pub const SCORING_LIFT: f32 = 6.0;
    pub const SCORING_SPLIT_LIFT: f32 = 2.0;
}

fn trunc(v: f32) -> i32 {
    v as i32
}

/// `cvttss2si`: out of range or NaN gives the integer indefinite `0x80000000`.
fn cvtt(v: f32) -> i32 {
    if v.is_finite() && (-2_147_483_648.0..2_147_483_648.0).contains(&v) {
        v as i32
    } else {
        i32::MIN
    }
}

/// The arguments of one outlined string draw `0x41E30(x, y, text, colour, outline)`.
#[derive(Clone, Debug, PartialEq)]
pub struct OutlinedCall {
    pub x: f32,
    pub y: f32,
    pub text: Vec<u8>,
    pub colour: u32,
    pub outline: u32,
}

/// One outlined string as the HUD layer draws it: runs and inline icons at their pens, colours and blend.
#[derive(Clone, Debug, PartialEq)]
pub struct MessageText {
    /// Left edge of the viewer text column, independent of centered/animated run pens.
    pub left: f32,
    /// Right edge of the viewer text column, in the same guest pixels as the run pens.
    pub right: f32,
    pub runs: Vec<TextRun>,
    pub icons: Vec<PlacedIcon>,
    pub font: u32,
    /// Colour words (R, G, B, A high to low; 0x80 = 1): the text and its four outline passes.
    pub colour: u32,
    pub outline: u32,
    /// The text system's scales (renderer glyph scale).
    pub text_scale: [f32; 2],
    /// Blend mode 2 (additive) of the title; the pickup draws with alpha (mode 1).
    pub additive: bool,
}

/// Everything one queue call draws natively, plus the entries left to their original drawer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QueueDraw {
    pub texts: Vec<MessageText>,
    /// The outlined draws in the original's order (compared with the original's calls in the shadow check).
    pub calls: Vec<OutlinedCall>,
    /// What the gate runs on the guest, in the original's order: the drawers of the kinds not replayed yet and the
    /// render-state calls (blend mode `0x19D70`, font `0x3FA10`) the native kinds leave behind.
    pub ops: Vec<GuestOp>,
    /// Kinds drawn natively (evidence).
    pub native_kinds: Vec<u32>,
}

/// A guest call of the native queue: an original drawer for an entry, or the render state the original sets.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GuestOp {
    /// Original drawer retained for guest-backed A/B traces and as a fallback
    /// while an owned message kind has no typed Rust presenter.
    Drawer { drawer: u32, entry: u32 },
    /// `0x19D70(mode)`.
    Blend(u32),
    /// `0x3FA10(font)`.
    Font(u32),
}

impl QueueDraw {
    /// The entries drawn by their original drawer.
    pub fn original_calls(&self) -> Vec<(u32, u32)> {
        self.ops
            .iter()
            .filter_map(|op| match op {
                GuestOp::Drawer { drawer, entry } => Some((*drawer, *entry)),
                _ => None,
            })
            .collect()
    }
}

/// What the gate does with one queue call.
#[derive(Clone, Debug, PartialEq)]
pub enum QueueCall {
    Draw(QueueDraw),
    /// No viewer slot: the original returns at once.
    Nothing,
    /// A case the native queue does not reproduce; the original runs.
    Legacy(&'static str),
}

const UNREADABLE: &str = "unreadable state";

/// One ring entry as typed state ([`Message`]; the slide time `+0x14C` beside it).
pub fn read_entry<G: GuestCall + ?Sized>(g: &G, entry: u32) -> Option<(Message, f32)> {
    use message_addr::*;
    let mut text = guest_cstring(g, entry + TEXT)?;
    text.truncate(TEXT_BYTES);
    let message = Message {
        kind: g.read32(entry + KIND)?,
        text,
        age: g.read_f32(entry + AGE)?,
        fade_start: g.read_f32(entry + FADE_START)?,
        end: g.read_f32(entry + END)?,
        evict_after: g.read_f32(entry + EVICT_AFTER)?,
        weight: g.read32(entry + WEIGHT)?,
        opaque: g.read32(entry + OPAQUE)?,
    };
    Some((message, g.read_f32(entry + SLIDE)?))
}

/// Viewport and HUD margins as the drawers read them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x0: i32,
    pub x1: i32,
    pub max_text: i32,
    pub y0: i32,
    pub y1: i32,
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub players: i32,
    pub slot: i32,
}

impl Frame {
    pub fn read<G: GuestCall + ?Sized>(g: &G) -> Option<Self> {
        use message_addr::*;
        let vp = g.read32(VIEWPORT_PTR)?;
        let int = |va: u32| g.read32(va).map(|v| v as i32);
        Some(Frame {
            x0: int(vp)?,
            x1: int(vp + 4)?,
            max_text: int(vp + 0xc)?,
            y0: int(vp + 0x14)?,
            y1: int(vp + 0x18)?,
            left: int(MARGIN_LEFT)?,
            top: int(MARGIN_TOP)?,
            right: int(MARGIN_RIGHT)?,
            bottom: int(MARGIN_BOTTOM)?,
            scale_x: g.read_f32(SCALE_X)?,
            scale_y: g.read_f32(SCALE_Y)?,
            players: g.call(FN_PLAYERS, &[])? as i32,
            slot: g.read32(g.read32(VIEWER)? + 4)? as i32,
        })
    }

    /// The HUD column's inner width and its centre (`((x1 - x0 - right - left) >> 1) + x0 + left`).
    fn inner_width(&self) -> i32 {
        self.x1 - self.x0 - self.right - self.left
    }

    fn centre(&self) -> i32 {
        (self.inner_width() >> 1) + self.x0 + self.left
    }

    /// Four-way split, or three-way for the lower viewers: the drawers fit the text to the viewport (`0x40A40`).
    fn split(&self) -> bool {
        self.players == 4 || (self.players == 3 && self.slot > 0)
    }
}

/// Text drawing state of one call: the font the original has selected and the measure.
struct Text<'a, G: GuestCall + ?Sized> {
    env: TextEnv<'a, G>,
    frame: Frame,
}

impl<'a, G: GuestCall + ?Sized> Text<'a, G> {
    fn font_of(&self, table: u32, index: i32) -> Result<u32, &'static str> {
        self.env
            .g
            .read32(table.wrapping_add((index as u32).wrapping_mul(4)))
            .ok_or(UNREADABLE)
    }

    fn metrics(&self, font: u32) -> Result<FontMetrics, &'static str> {
        FontMetrics::read(self.env.g, font).ok_or("font state not replayed")
    }

    fn width(&self, m: &FontMetrics, text: &[u8]) -> Result<f32, &'static str> {
        Ok(self
            .env
            .measure(m, text, false)
            .ok_or("text escape not replayed")?
            .0)
    }

    /// `0x12D150` (font table in `edi`, text in `ebx`): from the player count's font on, the first one whose
    /// width fits the HUD column (or the fifth entry); the font stays selected.
    fn fit_font(&self, table: u32, text: &[u8]) -> Result<u32, &'static str> {
        let f = &self.frame;
        let margin = if f.players == 1 {
            k::FIT_MARGIN_ONE
        } else {
            k::FIT_MARGIN_SPLIT
        };
        let limit = f.x1 - 2 * trunc(margin as f32 * f.scale_x) - f.left - f.right - f.x0;
        let mut index = f.players - 1;
        loop {
            let font = self.font_of(table, index)?;
            index += 1;
            if index >= 5 || !(self.width(&self.metrics(font)?, text)? > limit as f32) {
                return Ok(font);
            }
        }
    }

    /// `0x148680`: the bottom text line of the HUD column (line top of the pickup font of the player count).
    fn bottom_line(&self) -> Result<i32, &'static str> {
        use message_addr::*;
        let f = &self.frame;
        let g = self.env.g;
        let line_top = self
            .metrics(self.font_of(FONTS_PICKUP, f.players)?)?
            .line_top;
        let mut y = trunc((f.y1 - f.bottom) as f32 - line_top);
        let mode = g.read32(MODE_BYTE & !3).ok_or(UNREADABLE)? >> ((MODE_BYTE & 3) * 8) & 0xff;
        let level_players = g.read32(LEVEL_PLAYERS).ok_or(UNREADABLE)? as i32;
        let notice =
            mode == 10 && level_players > 1 && g.call(FN_SPLIT_NOTICE, &[]).ok_or(UNREADABLE)? != 0;
        if !notice {
            let step = if f.split() {
                if g.call(FN_SPLIT_TALL, &[]).ok_or(UNREADABLE)? & 0xff != 0 {
                    k::SPLIT_TALL
                } else {
                    k::SPLIT_SHORT
                }
            } else {
                1.0
            };
            y = trunc(y as f32 - f.scale_y * step * k::BOTTOM_STEP - f.scale_y * k::BOTTOM_GAP);
        }
        Ok(y)
    }

    /// `0x148760` with `ebx` rows: above the bottom line by `rows` line tops of the title base font.
    fn title_base(&self, rows: f32) -> Result<i32, &'static str> {
        use message_addr::*;
        let f = &self.frame;
        let base_font = self.metrics(self.font_of(FONTS_TITLE_BASE, f.players)?)?;
        let gap = trunc(f.scale_y * k::TITLE_GAP);
        let start = if f.players > 1 {
            (self.bottom_line()? - gap) as f32
        } else {
            let single = self.metrics(self.font_of(FONTS_SINGLE_BASE, f.players)?)?;
            let y = trunc((self.bottom_line()? - gap) as f32 - single.line_top);
            (y - gap) as f32
        };
        Ok(trunc(start - rows * base_font.line_top))
    }

    /// `0x148830`: the title row (centred in the column when it would rise above the top limit).
    fn title_line(&self) -> Result<i32, &'static str> {
        use message_addr::*;
        let f = &self.frame;
        let line_top = self
            .metrics(self.font_of(FONTS_TITLE, f.players)?)?
            .line_top;
        let gap = trunc(f.scale_y * k::TITLE_GAP);
        let mut y = trunc((self.title_base(k::TITLE_ROWS)? - gap) as f32 - line_top);
        let top = if f.players == 1 {
            k::TITLE_TOP_ONE
        } else {
            k::TITLE_TOP_SPLIT
        };
        let limit =
            trunc(top as f32 * f.scale_y) - trunc(f.scale_y * k::TITLE_TOP_ADJUST) + f.y0 + f.top;
        if y < limit {
            let middle = ((f.y1 - f.y0 - f.bottom - f.top) >> 1) + f.y0 + f.top;
            y = trunc(middle as f32 - line_top * 0.5);
        }
        Ok(y)
    }

    /// One `0x41E30` call: the text placed with the font's metrics.
    fn outlined(
        &self,
        draw: &mut QueueDraw,
        m: &FontMetrics,
        call: OutlinedCall,
        additive: bool,
    ) -> Result<(), &'static str> {
        self.outlined_with_placement(draw, m, call, additive, false)
    }

    fn outlined_with_placement(
        &self,
        draw: &mut QueueDraw,
        m: &FontMetrics,
        call: OutlinedCall,
        additive: bool,
        centered: bool,
    ) -> Result<(), &'static str> {
        let placement = if centered {
            self.env.place_centered(m, call.x, call.y, &call.text)
        } else {
            self.env.place(m, call.x, call.y, &call.text)
        };
        let (runs, icons) = placement.ok_or("text escape not replayed")?;
        draw.texts.push(MessageText {
            left: (self.frame.x0 + self.frame.left) as f32,
            right: (self.frame.x1 - self.frame.right) as f32,
            runs,
            icons,
            font: m.font,
            colour: call.colour,
            outline: call.outline,
            text_scale: m.text_scale,
            additive,
        });
        draw.calls.push(call);
        Ok(())
    }

    /// A plain, integer-position message draw (`0x42490`), used by kind 8.
    /// It shares the native text placement boundary but emits no outlined-call
    /// witness and sets no outline colour.
    fn plain(
        &self,
        draw: &mut QueueDraw,
        m: &FontMetrics,
        x: f32,
        y: f32,
        bytes: &[u8],
        colour: u32,
    ) -> Result<(), &'static str> {
        let (runs, icons) = self
            .env
            .place(m, x, y, bytes)
            .ok_or("text escape not replayed")?;
        draw.texts.push(MessageText {
            left: (self.frame.x0 + self.frame.left) as f32,
            right: (self.frame.x1 - self.frame.right) as f32,
            runs,
            icons,
            font: m.font,
            colour,
            outline: 0,
            text_scale: m.text_scale,
            additive: false,
        });
        Ok(())
    }
}

impl<G: GuestCall + ?Sized> crate::hud_producers::PickupNoticeText for Text<'_, G> {
    fn fit_pickup_font(&self, bytes: &[u8]) -> Option<u32> {
        self.fit_font(message_addr::FONTS_PICKUP + 4, bytes).ok()
    }

    fn line_top(&self, font: u32) -> Option<f32> {
        self.metrics(font).ok().map(|metrics| metrics.line_top)
    }

    fn width(&self, font: u32, bytes: &[u8]) -> Option<f32> {
        let metrics = self.metrics(font).ok()?;
        Text::width(self, &metrics, bytes).ok()
    }

    fn integer(&self, value: f32) -> i32 {
        trunc(value)
    }
}

/// Read-only runtime metric adapter for owned notices. Font selection stays in
/// Rust; only neighbouring viewport/font data and pure game facts are read.
/// No message entry, progress field, font global or wrap buffer is written.
pub struct NativeNoticeText<'a, G: GuestCall + ?Sized> {
    text: Text<'a, G>,
    selected: std::cell::Cell<Option<u32>>,
}

impl<'a, G: GuestCall + ?Sized> NativeNoticeText<'a, G> {
    pub fn read(g: &'a G, fonts: &'a FontSet) -> Option<Self> {
        Some(Self {
            text: Text {
                env: TextEnv { g, fonts },
                frame: Frame::read(g)?,
            },
            selected: std::cell::Cell::new(None),
        })
    }

    fn fit(&self, table: u32, bytes: &[u8]) -> Option<u32> {
        let font = self.text.fit_font(table, bytes).ok()?;
        self.selected.set(Some(font));
        Some(font)
    }

    fn wrapped(&self, font: u32, bytes: &[u8], width: f32) -> Option<Vec<u8>> {
        self.text
            .env
            .wrap(&self.text.metrics(font).ok()?, bytes, width)
    }
}

/// Low word of the original truncating int64 text conversion. In particular,
/// this is not Rust's saturating f64-to-i32 conversion or SSE int32 indefinite.
pub fn text_integer(value: f64) -> i32 {
    let integer = if value.is_finite()
        && (-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&value)
    {
        value as i64
    } else {
        i64::MIN
    };
    integer as i32
}

impl<G: GuestCall + ?Sized> crate::hud_producers::MessageTextMetrics for NativeNoticeText<'_, G> {
    fn outlined_weight(&self, bytes: &[u8]) -> Option<i32> {
        // 0x40F30(text, 0): zero width bypasses wrapping. Empty text is
        // zero lines; otherwise newline count plus one, independent of font.
        let bytes = &bytes[..bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len())];
        Some(if bytes.is_empty() {
            0
        } else {
            1 + bytes.iter().filter(|b| **b == b'\n').count() as i32
        })
    }
    fn text_count(&self, bytes: &[u8]) -> Option<i32> {
        Some(bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len()) as i32)
    }
}

impl<G: GuestCall + ?Sized> crate::hud_producers::CenterNoticeText for NativeNoticeText<'_, G> {
    fn fit_font(&self, bytes: &[u8]) -> Option<u32> {
        self.fit(0x4e7290, bytes)
    }
    fn width(&self, font: u32, bytes: &[u8]) -> Option<f64> {
        self.text
            .width(&self.text.metrics(font).ok()?, bytes)
            .ok()
            .map(f64::from)
    }
    fn bottom_line(&self) -> Option<i32> {
        self.text.bottom_line().ok()
    }
    fn integer(&self, value: f64) -> i32 {
        text_integer(value)
    }
}

impl<G: GuestCall + ?Sized> crate::hud_producers::CornerNoticeText for NativeNoticeText<'_, G> {
    fn fit_corner_font(&self, bytes: &[u8]) -> Option<u32> {
        self.fit(0x4e72cc, bytes)
    }
    fn alpha_integer(&self, value: f64) -> i32 {
        text_integer(value)
    }
}

impl<G: GuestCall + ?Sized> crate::hud_producers::ProgressNoticeText for NativeNoticeText<'_, G> {
    fn count(&self, bytes: &[u8]) -> Option<u32> {
        // 0x40650's optional comma parser increments a scratch pointer, not
        // the loop's source pointer; digits still count individually.
        Some(bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len()) as u32)
    }
    fn prefix_offset(&self, bytes: &[u8], count: u32) -> Option<usize> {
        let length = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
        if self.text.env.g.read32(0x6b93a8)? == 0 {
            (count as usize <= length).then_some(count as usize)
        } else {
            // 0x406A0 treats count as signed in optional-scan mode.
            Some(if count as i32 <= 0 {
                0
            } else {
                (count as usize).min(length)
            })
        }
    }
    fn fit_font(&self, bytes: &[u8]) -> Option<u32> {
        self.fit(0x4e72a4, bytes)
    }
    fn wrap(&self, bytes: &[u8], width: f32) -> Option<Vec<u8>> {
        self.wrapped(self.selected.get()?, bytes, width)
    }
    fn alpha_integer(&self, value: f64) -> i32 {
        text_integer(value)
    }
}

impl<G: GuestCall + ?Sized> crate::hud_producers::SplitNoticeText for NativeNoticeText<'_, G> {
    fn wrap(&self, font: u32, bytes: &[u8], width: f32) -> Option<Vec<u8>> {
        self.wrapped(font, bytes, width)
    }
    fn bottom_line(&self) -> Option<i32> {
        self.text.bottom_line().ok()
    }
}

/// Kind 2, `0x149810(entry, offset)`: the pickup line centred at the bottom, `offset` line tops up (the running
/// stack of the pickups before it), fading out after `fade_start`; with HUD flags `& 3` only the first line shows.
fn pickup<G: GuestCall + ?Sized>(
    t: &Text<'_, G>,
    draw: &mut QueueDraw,
    message: &Message,
    offset: f32,
) -> Result<(), &'static str> {
    let f = t.frame;
    if f.split() {
        return Err("split-screen text fit (0x40A40) not replayed");
    }
    let g = t.env.g;
    let mut alpha = 1.0f32;
    if message.age > message.fade_start {
        alpha = 1.0 - (message.age - message.fade_start) / (message.end - message.fade_start);
    }
    let hud = message_addr::HUD_RECORDS
        .wrapping_add((f.slot as u32).wrapping_mul(message_addr::HUD_RECORD_SIZE));
    if g.read32(hud + message_addr::HUD_FLAGS).ok_or(UNREADABLE)? & 3 != 0 {
        let mut share = 1.0 - offset;
        if share > 1.0 {
            share = 1.0;
        } else if 0.0 > share {
            share = 0.0;
        }
        alpha *= share;
    }
    let byte = trunc(alpha * k::ALPHA) as u32;
    let font = t.fit_font(message_addr::FONTS_PICKUP + 4, &message.text)?;
    let m = t.metrics(font)?;
    let base = t.bottom_line()?;
    let y = trunc(base as f32 + m.line_top * offset);
    let width = t.width(&m, &message.text)?;
    let x = (f.centre() as f32 - width * 0.5) as i32;
    draw.ops.extend([GuestOp::Blend(1), GuestOp::Font(font)]);
    if f.max_text as f32 > width {
        let call = OutlinedCall {
            x: x as f32,
            y: y as f32,
            text: message.text.clone(),
            colour: byte | k::PICKUP_TEXT,
            outline: byte | k::PICKUP_OUTLINE,
        };
        t.outlined(draw, &m, call, false)?;
    }
    Ok(())
}

/// Kind 3, `0x14A280(entry)`: the text wrapped to the HUD column (`0x40F00`) in the title base font of the player
/// count, its lines from `0x148760` (rows = line count) down, each centred and shifted sideways by the slide (in over
/// the slide time `+0x14C` along the sine table, out after `fade_start` more). The original selects the font, draws
/// and selects the previous font again; it leaves blend mode 1.
fn notice<G: GuestCall + ?Sized>(
    t: &Text<'_, G>,
    draw: &mut QueueDraw,
    message: &Message,
    slide: f32,
) -> Result<(), &'static str> {
    use message_addr::*;
    let f = t.frame;
    let g = t.env.g;
    let font = t.font_of(FONTS_TITLE_BASE, f.players)?;
    let m = t.metrics(font)?;
    let span = f.inner_width() as f32;
    let text = t
        .env
        .wrap(&m, &message.text, span)
        .ok_or("text wrap not replayed")?;
    draw.ops.push(GuestOp::Blend(1));
    let rows = 1 + text.iter().filter(|&&c| c == b'\n').count();
    let mut y = t.title_base(rows as f32)?;
    let sine = |index: i32| -> Result<f32, &'static str> {
        g.read_f32(SINE_TABLE + 4 * (index & 0x7ff) as u32)
            .ok_or(UNREADABLE)
    };
    let shift = if slide > message.age {
        let index = cvtt(message.age / slide * k::NOTICE_QUARTER * k::NOTICE_TABLE_SCALE + 0.5);
        trunc((1.0 - sine(index)?) * span)
    } else if message.age - (slide + message.fade_start) >= 0.0 {
        let index = cvtt(
            (message.age - (slide + message.fade_start)) / slide
                * k::NOTICE_QUARTER
                * k::NOTICE_TABLE_SCALE
                + 0.5,
        );
        trunc(0.0 - (1.0 - sine(index.wrapping_add(0x200))?) * span)
    } else {
        0
    };
    let (mut colour, mut outline) = if g.call(FN_TEXT_MODE, &[]).ok_or(UNREADABLE)? != 0 {
        (k::TITLE_TEXT_MODE_TEXT, k::TITLE_TEXT_MODE_OUTLINE)
    } else {
        (k::NOTICE_TEXT, k::NOTICE_OUTLINE)
    };
    // The original cuts the text at its first line break (NUL) for every line and draws from the line start to the
    // next NUL: the first line alone, then the rest of the text in one call.
    let first_break = text.iter().position(|&c| c == b'\n');
    let mut cut = text.clone();
    if let Some(i) = first_break {
        cut[i] = 0;
    }
    let centre = f.centre() as f32;
    let mut at = 0usize;
    while at < text.len() {
        match text[at] {
            b'~' => {
                colour = k::NOTICE_HIGHLIGHT_TEXT;
                outline = k::NOTICE_HIGHLIGHT_OUTLINE;
                at += 1;
            }
            b'`' => {
                y = trunc(m.line_top * 0.5 + y as f32);
                at += 1;
            }
            _ => {}
        }
        let line: Vec<u8> = cut[at..].iter().copied().take_while(|&c| c != 0).collect();
        if !line.is_empty() {
            let width = t.width(&m, &line)?;
            let x = trunc(centre - width * 0.5 + shift as f32);
            let call = OutlinedCall {
                x: x as f32,
                y: y as f32,
                text: line.clone(),
                colour,
                outline,
            };
            t.outlined(draw, &m, call, false)?;
        }
        y = trunc(y as f32 + m.line_top);
        at += line.len() + 1;
    }
    Ok(())
}

/// `0x154FE0(colour, alpha)`: the colour's alpha byte times `alpha` clamped to 0..1 (x87, single precision).
fn scale_alpha(colour: u32, alpha: f32) -> u32 {
    let a = if alpha > 1.0 {
        1.0
    } else if 0.0 > alpha {
        0.0
    } else {
        alpha
    };
    (colour & 0xffff_ff00) | ((colour & 0xff) as f32 * a) as i32 as u32
}

/// The outline of a team-coloured string: each colour byte divided by 8 (`0x149E5E` / `0x14A150`), alpha kept.
fn team_outline(colour: u32) -> u32 {
    let r = ((colour >> 24) << 21) & 0xff00_0000;
    let g = (((colour >> 16) & 0xff) << 13) & 0x00ff_0000;
    let b = (((colour >> 8) & 0xff) << 5) & 0x0000_ff00;
    r | g | b | (colour & 0xff)
}

/// Kind 10, `0x149B30(entry)` (one viewer): the entry text and the player's name on one line (`text + " " + name`,
/// white) when they fit in 45 % of the column, else on two rows; in team games the name is drawn separately in the
/// team colour (`0x154FE0`) with an outline of a eighth of it. Rows above the bottom by `0x148910`. The original
/// selects the title font and leaves it selected (blend mode 1).
fn scoring<G: GuestCall + ?Sized>(
    t: &Text<'_, G>,
    draw: &mut QueueDraw,
    message: &Message,
) -> Result<(), &'static str> {
    use message_addr::*;
    let f = t.frame;
    let g = t.env.g;
    if f.split() {
        return Err("split-screen scoring notice (0x40A40 fit) not replayed");
    }
    let player = g
        .read32(PLAYERS_PTR)
        .ok_or(UNREADABLE)?
        .wrapping_add(message.opaque.wrapping_mul(PLAYER_SIZE));
    let name =
        guest_cstring(g, g.read32(player + PLAYER_NAME).ok_or(UNREADABLE)?).ok_or(UNREADABLE)?;
    let mut alpha = 1.0f32;
    if message.age > message.fade_start {
        let r = (message.age - message.fade_start) / (message.end - message.fade_start);
        alpha = 1.0 - r * r;
    }
    let byte = (alpha * k::ALPHA) as i32 as u32 & 0xff;
    let (text_colour, outline) = (byte | k::SCORING_TEXT, byte | k::SCORING_OUTLINE);
    let font = t.font_of(FONTS_TITLE, 0)?;
    let m = t.metrics(font)?;
    draw.ops.extend([GuestOp::Blend(1), GuestOp::Font(font)]);
    let span = f.inner_width() as f32;
    let left = (f.x0 + f.left) as f32;
    let teams = g.read32(GAME_FLAGS).ok_or(UNREADABLE)? & 1 != 0;
    let mut first = message.text.clone();
    let combined = t.width(&m, &first)? + t.width(&m, &name)?;
    let (two_rows, separate) = if combined > span * k::SCORING_ONE_LINE {
        (true, true)
    } else if teams {
        first.push(b' ');
        (false, true)
    } else {
        first.push(b' ');
        first.extend_from_slice(&name);
        (false, false)
    };
    // `0x148910(rows)`: the pickup font's line top, `rows` up from the bottom margin.
    let rows = if two_rows { 2 } else { 1 };
    let pickup = t.metrics(t.font_of(FONTS_PICKUP, f.players)?)?;
    let base = cvtt(
        (f.y1 - f.bottom) as f32
            - f.scale_y * 1.0 * k::SCORING_ROW_STEP
            - rows as f32 * pickup.line_top,
    );
    let mut y = if f.players > 1 {
        cvtt(f.scale_y * k::SCORING_SPLIT_LIFT + base as f32)
    } else {
        cvtt(base as f32 - f.scale_y * k::SCORING_LIFT)
    } as f32;
    let centre = span * 0.5 + left;
    let team_colour = || -> Result<u32, &'static str> {
        let team = g.read32(player + PLAYER_TEAM).ok_or(UNREADABLE)?;
        Ok(scale_alpha(
            g.read32(TEAM_COLOURS + team.wrapping_mul(20))
                .ok_or(UNREADABLE)?,
            alpha,
        ))
    };
    if !separate {
        let x = centre - t.width(&m, &first)? * 0.5;
        return t.outlined(
            draw,
            &m,
            OutlinedCall {
                x,
                y,
                text: first,
                colour: text_colour,
                outline,
            },
            false,
        );
    }
    if !two_rows {
        let colour = team_colour()?;
        let width_name = t.width(&m, &name)?;
        let width_first = t.width(&m, &first)?;
        let x = centre - (width_name + width_first) * 0.5;
        t.outlined(
            draw,
            &m,
            OutlinedCall {
                x,
                y,
                text: first,
                colour: text_colour,
                outline,
            },
            false,
        )?;
        let x2 = width_first + x;
        return t.outlined(
            draw,
            &m,
            OutlinedCall {
                x: x2,
                y,
                text: name,
                colour,
                outline: team_outline(colour),
            },
            false,
        );
    }
    let x = centre - t.width(&m, &first)? * 0.5;
    t.outlined(
        draw,
        &m,
        OutlinedCall {
            x,
            y,
            text: first,
            colour: text_colour,
            outline,
        },
        false,
    )?;
    y += m.line_top;
    let x = left + span * 0.5 - t.width(&m, &name)? * 0.5;
    if teams {
        let colour = team_colour()?;
        t.outlined(
            draw,
            &m,
            OutlinedCall {
                x,
                y,
                text: name,
                colour,
                outline: team_outline(colour),
            },
            false,
        )
    } else {
        t.outlined(
            draw,
            &m,
            OutlinedCall {
                x,
                y,
                text: name,
                colour: text_colour,
                outline,
            },
            false,
        )
    }
}

/// Kinds 4 and 5, `0x148EE0(entry)`: the title drawn twice, sliding in from the left and the right over the slide
/// time `+0x14C`, meeting in the column centre, and sliding out after `fade_start` more seconds.
fn title<G: GuestCall + ?Sized>(
    t: &Text<'_, G>,
    draw: &mut QueueDraw,
    message: &Message,
    slide: f32,
) -> Result<(), &'static str> {
    let g = t.env.g;
    let f = t.frame;
    let (colour, outline, additive) =
        if g.call(message_addr::FN_TEXT_MODE, &[]).ok_or(UNREADABLE)? != 0 {
            (k::TITLE_TEXT_MODE_TEXT, k::TITLE_TEXT_MODE_OUTLINE, false)
        } else {
            (k::TITLE_TEXT, k::TITLE_OUTLINE, true)
        };
    let font = t.fit_font(message_addr::FONTS_TITLE + 4, &message.text)?;
    let m = t.metrics(font)?;
    let width = trunc(t.width(&m, &message.text)?);
    let centre = (f.inner_width() >> 1) - (width >> 1) + f.x0 + f.left;
    let y = t.title_line()?;
    let phase = if slide > message.age {
        message.age / slide * 0.5
    } else if message.age >= slide + message.fade_start {
        ((message.age - (slide + message.fade_start)) / slide + 1.0) * 0.5
    } else {
        0.5
    };
    let span = f.inner_width() as f32;
    let from_left = trunc((centre - f.inner_width()) as f32 + span * phase * k::SLIDE);
    let from_right = trunc((f.inner_width() + centre) as f32 - span * phase * k::SLIDE);
    draw.ops.extend([
        GuestOp::Blend(if additive { 2 } else { 1 }),
        GuestOp::Font(font),
    ]);
    for x in [from_left, from_right] {
        let call = OutlinedCall {
            x: x as f32,
            y: y as f32,
            text: message.text.clone(),
            colour,
            outline,
        };
        t.outlined(draw, &m, call, additive)?;
    }
    Ok(())
}

/// `0x12DF00`: is the entry hidden by what the call has shown so far (`sums` per kind)?
fn conflicts<G: GuestCall + ?Sized>(
    g: &G,
    kind: u32,
    weight: u32,
    sums: &[i32; 16],
) -> Result<bool, &'static str> {
    use message_addr::*;
    let limit = g.read32(KIND_LIMITS + 4 * kind).ok_or(UNREADABLE)? as i32;
    if sums[kind as usize].wrapping_add(weight as i32) > limit {
        return Ok(true);
    }
    for shown in 1..14u32 {
        if sums[shown as usize] == 0 || shown == kind {
            continue;
        }
        // `0x12DEC0(shown, kind)`: a pair that holds `shown` and also `kind`.
        let mut pair = CONFLICT_LAST;
        while pair >= CONFLICT_FIRST {
            let (a, b) = (
                g.read32(pair - 4).ok_or(UNREADABLE)?,
                g.read32(pair).ok_or(UNREADABLE)?,
            );
            if (a == shown || b == shown) && (a == kind || b == kind) {
                return Ok(true);
            }
            pair -= 8;
        }
    }
    Ok(false)
}

/// Replays `0x14AC80(only_kind_9)` on the guest queue. With `call_originals` false (the shadow check) the entries
/// of kinds not replayed are only listed.
pub fn replay<G: GuestCall + ?Sized>(g: &G, fonts: &FontSet, only_kind_9: u32) -> QueueCall {
    replay_input(g, fonts, only_kind_9, None)
}

/// Draw the authoritative native ring; all message payload and lifecycle reads
/// come from owned Rust state. Guest access is restricted to neighbouring font,
/// viewport and gameplay inputs. Unsupported drawers return a fallback decision.
pub fn replay_owned<G: GuestCall + ?Sized>(
    g: &G,
    fonts: &FontSet,
    only_kind_9: u32,
    queue: &crate::hud_producers::OwnedMessageQueue,
    policy: &crate::hud_producers::MessagePolicy,
) -> QueueCall {
    replay_input(g, fonts, only_kind_9, Some((queue, policy)))
}

/// Replay a Rust-owned queue through the native input-progress scheduler and
/// Rust message drawers. Fallible drawing is staged with the queue so an
/// unsupported renderer cannot consume input or retire a message before the
/// legacy path is selected.
pub fn replay_owned_native<G: GuestCall + ?Sized>(
    g: &G,
    fonts: &FontSet,
    only_kind_9: u32,
    state: &mut crate::hud_native::HudState,
    player: i32,
    policy: &crate::hud_producers::MessagePolicy,
) -> QueueCall {
    let Some(slot) = usize::try_from(player).ok().filter(|slot| *slot < 4) else {
        return QueueCall::Legacy("invalid native HUD player");
    };
    let queue = &state.messages[slot];
    // Kind 6 is a known scheduler no-op: it is owned state, but has no drawer
    // and contributes no weight (T2329 checkpoint 9).
    if !queue
        .ring
        .entries
        .iter()
        .any(|entry| matches!(entry.kind, 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10))
    {
        return QueueCall::Legacy("native message family absent");
    }
    if queue.ring.entries.iter().any(|entry| {
        entry.kind != 0 && !matches!(entry.kind, 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10)
    }) {
        return QueueCall::Legacy("native message drawer unavailable");
    }
    let Some(frame) = Frame::read(g) else {
        return QueueCall::Legacy(UNREADABLE);
    };
    let Some(notice_text) = NativeNoticeText::read(g, fonts) else {
        return QueueCall::Legacy(UNREADABLE);
    };
    let has_progress = queue.ring.entries.iter().any(|message| message.kind == 7);
    let progress_layout = if has_progress {
        let Some(layout) = progress_layout(g, frame) else {
            return QueueCall::Legacy(UNREADABLE);
        };
        Some(layout)
    } else {
        None
    };
    let has_center_notice = queue.ring.entries.iter().any(|message| message.kind == 1);
    let center_layout = if has_center_notice {
        let Some(layout) = center_notice_layout(g, frame) else {
            return QueueCall::Legacy(UNREADABLE);
        };
        Some(layout)
    } else {
        None
    };
    let has_corner_notice = queue.ring.entries.iter().any(|message| message.kind == 8);
    let corner_layout = if has_corner_notice {
        let Some(layout) = corner_notice_layout(g, frame) else {
            return QueueCall::Legacy(UNREADABLE);
        };
        Some(layout)
    } else {
        None
    };
    let has_split_notice = queue.ring.entries.iter().any(|message| message.kind == 9);
    let show_split_notice = if has_split_notice {
        let Some(show) = g.call(message_addr::FN_SPLIT_NOTICE, &[]) else {
            return QueueCall::Legacy(UNREADABLE);
        };
        show != 0
    } else {
        false
    };
    let split_layout = if show_split_notice {
        let Some(layout) = split_notice_layout(g, frame) else {
            return QueueCall::Legacy(UNREADABLE);
        };
        Some(layout)
    } else {
        None
    };
    let text = Text {
        env: TextEnv { g, fonts },
        frame,
    };
    let has_pickup = queue.ring.entries.iter().any(|message| message.kind == 2);
    let pickup_layout = if has_pickup {
        if frame.split() {
            return QueueCall::Legacy("split-screen text fit (0x40A40) not replayed");
        }
        let Some(bottom_line) = text.bottom_line().ok() else {
            return QueueCall::Legacy(UNREADABLE);
        };
        let Some(flags) = g.read32(
            message_addr::HUD_RECORDS
                .wrapping_add((frame.slot as u32).wrapping_mul(message_addr::HUD_RECORD_SIZE))
                .wrapping_add(message_addr::HUD_FLAGS),
        ) else {
            return QueueCall::Legacy(UNREADABLE);
        };
        Some(crate::hud_producers::PickupNoticeLayout {
            center_x: frame.centre(),
            bottom_line,
            max_width: frame.max_text as f32,
            hide_stacked: flags & 3 != 0,
            alpha_scale: k::ALPHA,
            foreground_base: k::PICKUP_TEXT,
            outline_base: k::PICKUP_OUTLINE,
        })
    } else {
        None
    };
    let mut draw = QueueDraw {
        ops: vec![GuestOp::Blend(1)],
        ..QueueDraw::default()
    };
    let result = state.try_replay_messages_native(
        player,
        policy,
        only_kind_9 != 0,
        show_split_notice,
        |message, slide, offset, progress| {
            match message.kind {
                1 => {
                    let layout = center_layout
                        .as_ref()
                        .ok_or("native centered notice layout unavailable")?;
                    let presented =
                        crate::hud_producers::center_notice(message, layout, &notice_text)
                            .ok_or("native centered notice metrics unavailable")?;
                    let metrics = text.metrics(presented.font)?;
                    draw.ops
                        .extend([GuestOp::Blend(1), GuestOp::Font(presented.font)]);
                    text.outlined(
                        &mut draw,
                        &metrics,
                        OutlinedCall {
                            x: presented.position[0],
                            y: presented.position[1],
                            text: presented.text,
                            colour: presented.foreground,
                            outline: presented.outline,
                        },
                        false,
                    )?;
                }
                2 => {
                    let presented = crate::hud_producers::pickup_notice(
                        message,
                        offset,
                        pickup_layout
                            .as_ref()
                            .ok_or("native pickup layout unavailable")?,
                        &text,
                    )
                    .ok_or("native pickup metrics unavailable")?;
                    let metrics = text.metrics(presented.font)?;
                    draw.ops
                        .extend([GuestOp::Blend(1), GuestOp::Font(presented.font)]);
                    if presented.visible {
                        text.outlined(
                            &mut draw,
                            &metrics,
                            OutlinedCall {
                                x: presented.position[0] as f32,
                                y: presented.position[1] as f32,
                                text: presented.text,
                                colour: presented.foreground,
                                outline: presented.outline,
                            },
                            false,
                        )?;
                    }
                }
                3 => notice(&text, &mut draw, message, slide)?,
                4 | 5 => title(&text, &mut draw, message, slide)?,
                7 => {
                    let presented = crate::hud_producers::progress_notice(
                        message,
                        progress,
                        progress_layout
                            .as_ref()
                            .ok_or("native progress notice layout unavailable")?,
                        &notice_text,
                    )
                    .ok_or("native progress notice metrics unavailable")?;
                    let metrics = text.metrics(presented.font)?;
                    draw.ops
                        .extend([GuestOp::Blend(1), GuestOp::Font(presented.font)]);
                    text.outlined(
                        &mut draw,
                        &metrics,
                        OutlinedCall {
                            x: presented.position[0],
                            y: presented.position[1],
                            text: presented.text,
                            colour: presented.foreground,
                            outline: presented.outline,
                        },
                        false,
                    )?;
                }
                8 => {
                    let layout = corner_layout
                        .as_ref()
                        .ok_or("native corner notice layout unavailable")?;
                    let presented =
                        crate::hud_producers::corner_notice(message, layout, &notice_text)
                            .ok_or("native corner notice metrics unavailable")?;
                    let metrics = text.metrics(presented.font)?;
                    draw.ops.push(GuestOp::Font(presented.font));
                    text.plain(
                        &mut draw,
                        &metrics,
                        presented.position[0] as f32,
                        presented.position[1] as f32,
                        &presented.text,
                        presented.colour,
                    )?;
                }
                9 => {
                    let layout = split_layout
                        .as_ref()
                        .ok_or("native split notice layout unavailable")?;
                    let presented =
                        crate::hud_producers::split_notice(message, layout, &notice_text)
                            .ok_or("native split notice metrics unavailable")?;
                    let metrics = text.metrics(presented.font)?;
                    draw.ops
                        .extend([GuestOp::Blend(1), GuestOp::Font(presented.font)]);
                    text.outlined_with_placement(
                        &mut draw,
                        &metrics,
                        OutlinedCall {
                            x: presented.position[0],
                            y: presented.position[1],
                            text: presented.text,
                            colour: presented.foreground,
                            outline: presented.outline,
                        },
                        false,
                        presented.centered,
                    )?;
                }
                10 => scoring(&text, &mut draw, message)?,
                _ => return Err("native message drawer unavailable"),
            }
            draw.native_kinds.push(message.kind);
            Ok(())
        },
    );
    match result {
        Ok(_) => QueueCall::Draw(draw),
        Err(why) => QueueCall::Legacy(why),
    }
}

fn split_notice_layout<G: GuestCall + ?Sized>(
    g: &G,
    frame: Frame,
) -> Option<crate::hud_producers::SplitNoticeLayout> {
    use crate::hud_producers::SplitNoticeLayout;

    let mode =
        g.read32(message_addr::MODE_BYTE & !3)? >> ((message_addr::MODE_BYTE & 3) * 8) & 0xff;
    let level_players = g.read32(message_addr::LEVEL_PLAYERS)? as i32;
    let alternate_context = if mode == 10 && level_players > 1 {
        g.call(0x0006_4cd0, &[])? != 0
    } else {
        false
    };
    Some(SplitNoticeLayout {
        viewport_left: frame.x0,
        viewport_right: frame.x1,
        margin_left: frame.left,
        margin_right: frame.right,
        mode: mode as u8,
        level_players,
        alternate_context,
        // The drawer selects descriptor 0x12 only for the alternate context;
        // ordinary split notices use descriptor 0x15.
        normal_font: 0x15,
        alternate_font: 0x12,
        width_factor: g.read_f32(0x0047_5cc8)?,
        foreground: 0x5f5f_5fff,
        outline: 0x0f0f_0f7f,
    })
}

fn center_notice_layout<G: GuestCall + ?Sized>(
    g: &G,
    frame: Frame,
) -> Option<crate::hud_producers::CenterNoticeLayout> {
    use crate::hud_producers::CenterNoticeLayout;

    Some(CenterNoticeLayout {
        viewport_left: frame.x0,
        viewport_right: frame.x1,
        margin_left: frame.left,
        margin_right: frame.right,
        scale_y: frame.scale_y,
        bottom_offset: g.read_f32(0x0047_5db4)?,
        line_height: g.read_f32(0x007e_7264)?,
        half_width: g.read_f32(0x0047_5cd4)?,
        full_alpha: g.read_f32(0x0047_5c78)?,
        alpha_scale: g.read_f32(0x0047_5cc0)?,
        foreground_base: 0x4064_6400,
        outline_base: 0x0000_5000,
    })
}

fn corner_notice_layout<G: GuestCall + ?Sized>(
    g: &G,
    frame: Frame,
) -> Option<crate::hud_producers::CornerNoticeLayout> {
    use crate::hud_producers::CornerNoticeLayout;

    Some(CornerNoticeLayout {
        viewport_left: frame.x0,
        margin_left: frame.left,
        viewport_bottom: frame.y1,
        line_height: g.read_f32(0x0074_90ec)?,
        height_factor: g.read_f32(0x0047_d1f8)?,
        baseline_offset: g.read_f32(0x0047_60cc)?,
        full_alpha: g.read_f32(0x0047_5c78)?,
        alpha_scale: g.read_f32(0x0047_5cc0)?,
        // The translated 0x12D8E0 source ORs the converted alpha into this
        // fixed RGB base before calling the plain text renderer.
        colour_base: 0x6464_6400,
    })
}

fn progress_layout<G: GuestCall + ?Sized>(
    g: &G,
    frame: Frame,
) -> Option<crate::hud_producers::ProgressNoticeLayout> {
    use crate::hud_producers::ProgressNoticeLayout;

    Some(ProgressNoticeLayout {
        viewport_left: frame.x0,
        viewport_right: frame.x1,
        // The original lower-origin viewport uses y0 and its top margin as
        // the progress notice's bottom anchor.
        viewport_bottom: frame.y0,
        margin_left: frame.left,
        margin_right: frame.right,
        margin_bottom: frame.top,
        effective_players: frame.players.max(0) as u32,
        scale: [frame.scale_x, frame.scale_y],
        reveal_rate: g.read_f32(0x0047_82c4)?,
        full_alpha: g.read_f32(0x0047_5c78)?,
        alpha_scale: g.read_f32(0x0047_5cc0)?,
        foreground_base: k::PICKUP_TEXT,
        outline_base: k::PICKUP_OUTLINE,
    })
}

fn replay_input<G: GuestCall + ?Sized>(
    g: &G,
    fonts: &FontSet,
    only_kind_9: u32,
    owned: Option<(
        &crate::hud_producers::OwnedMessageQueue,
        &crate::hud_producers::MessagePolicy,
    )>,
) -> QueueCall {
    use message_addr::*;
    let Some(frame) = Frame::read(g) else {
        return QueueCall::Legacy(UNREADABLE);
    };
    if frame.slot < 0 {
        return QueueCall::Nothing;
    }
    let hud = HUD_RECORDS.wrapping_add((frame.slot as u32).wrapping_mul(HUD_RECORD_SIZE));
    let text = Text {
        env: TextEnv { g, fonts },
        frame,
    };
    let run = || -> Result<QueueDraw, &'static str> {
        let cursor = match owned {
            Some((queue, _)) => queue.ring.cursor as i32,
            None => g.read32(hud + RING_CURSOR).ok_or(UNREADABLE)? as i32,
        };
        let mut draw = QueueDraw {
            ops: vec![GuestOp::Blend(1)],
            ..QueueDraw::default()
        };
        let mut sums = [0i32; 16];
        let mut offset = 0.0f32;
        // The centered presenter is only selected for a Rust-owned queue. The
        // guest-backed path remains the A/B oracle until lifecycle adoption.
        let mut center_layout = None;
        let mut corner_layout = None;
        let mut split_layout = None;
        let mut notice_text = None;
        for i in 0..RING_LEN as i32 {
            let index = cursor.wrapping_add(i) % RING_LEN as i32;
            if index < 0 {
                return Err("negative ring cursor");
            }
            let entry = hud + RING_ENTRIES + index as u32 * ENTRY_SIZE;
            let native = owned.map(|(queue, _)| {
                (
                    &queue.ring.entries[index as usize],
                    queue.slides[index as usize],
                )
            });
            let kind = match native {
                Some((message, _)) => message.kind,
                None => g.read32(entry + KIND).ok_or(UNREADABLE)?,
            };
            if kind == 0 {
                continue;
            }
            let age = match native {
                Some((message, _)) => message.age,
                None => g.read_f32(entry + AGE).ok_or(UNREADABLE)?,
            };
            // `comiss; jb`: below 0 (or NaN) is not shown yet.
            if age
                .partial_cmp(&0.0)
                .is_none_or(|o| o == std::cmp::Ordering::Less)
            {
                continue;
            }
            let weight = match native {
                Some((message, _)) => message.weight,
                None => g.read32(entry + WEIGHT).ok_or(UNREADABLE)?,
            };
            if kind >= 16 {
                return Err("message kind outside the sums");
            }
            let conflict = match owned {
                Some((_, policy)) => {
                    let k = kind as usize;
                    if k >= policy.limits.len() {
                        return Err("message kind outside policy");
                    }
                    sums[k].wrapping_add(weight as i32) > policy.limits[k]
                        || (1..14).any(|other| {
                            other != k
                                && sums[other] != 0
                                && (policy.conflicts.contains(&(other, k))
                                    || policy.conflicts.contains(&(k, other)))
                        })
                }
                None => conflicts(g, kind, weight, &sums)?,
            };
            if conflict {
                continue;
            }
            if only_kind_9 != 0 && kind != 9 {
                continue;
            }
            if !(1..=10).contains(&kind) {
                continue;
            }
            let (message, slide) = match native {
                Some((message, slide)) => (message.clone(), slide),
                None => read_entry(g, entry).ok_or(UNREADABLE)?,
            };
            let mut add_weight = true;
            match kind {
                1 if owned.is_some() => {
                    if center_layout.is_none() {
                        center_layout = center_notice_layout(g, frame);
                    }
                    let layout = center_layout
                        .as_ref()
                        .ok_or("native centered notice layout unavailable")?;
                    if notice_text.is_none() {
                        notice_text = NativeNoticeText::read(g, fonts);
                    }
                    let metrics = notice_text
                        .as_ref()
                        .ok_or("native centered notice metrics unavailable")?;
                    let presented = crate::hud_producers::center_notice(&message, layout, metrics)
                        .ok_or("native centered notice metrics unavailable")?;
                    let font_metrics = text.metrics(presented.font)?;
                    draw.ops
                        .extend([GuestOp::Blend(1), GuestOp::Font(presented.font)]);
                    text.outlined(
                        &mut draw,
                        &font_metrics,
                        OutlinedCall {
                            x: presented.position[0],
                            y: presented.position[1],
                            text: presented.text,
                            colour: presented.foreground,
                            outline: presented.outline,
                        },
                        false,
                    )?;
                    draw.native_kinds.push(kind);
                }
                8 if owned.is_some() => {
                    if corner_layout.is_none() {
                        corner_layout = corner_notice_layout(g, frame);
                    }
                    let layout = corner_layout
                        .as_ref()
                        .ok_or("native corner notice layout unavailable")?;
                    if notice_text.is_none() {
                        notice_text = NativeNoticeText::read(g, fonts);
                    }
                    let metrics = notice_text
                        .as_ref()
                        .ok_or("native corner notice metrics unavailable")?;
                    let presented = crate::hud_producers::corner_notice(&message, layout, metrics)
                        .ok_or("native corner notice metrics unavailable")?;
                    let font_metrics = text.metrics(presented.font)?;
                    draw.ops.push(GuestOp::Font(presented.font));
                    text.plain(
                        &mut draw,
                        &font_metrics,
                        presented.position[0] as f32,
                        presented.position[1] as f32,
                        &presented.text,
                        presented.colour,
                    )?;
                    draw.native_kinds.push(kind);
                }
                9 if owned.is_some() => {
                    if g.call(FN_SPLIT_NOTICE, &[]).ok_or(UNREADABLE)? != 0 {
                        if split_layout.is_none() {
                            split_layout = split_notice_layout(g, frame);
                        }
                        let layout = split_layout
                            .as_ref()
                            .ok_or("native split notice layout unavailable")?;
                        if notice_text.is_none() {
                            notice_text = NativeNoticeText::read(g, fonts);
                        }
                        let metrics = notice_text
                            .as_ref()
                            .ok_or("native split notice metrics unavailable")?;
                        let presented =
                            crate::hud_producers::split_notice(&message, layout, metrics)
                                .ok_or("native split notice metrics unavailable")?;
                        let font_metrics = text.metrics(presented.font)?;
                        draw.ops
                            .extend([GuestOp::Blend(1), GuestOp::Font(presented.font)]);
                        text.outlined_with_placement(
                            &mut draw,
                            &font_metrics,
                            OutlinedCall {
                                x: presented.position[0],
                                y: presented.position[1],
                                text: presented.text,
                                colour: presented.foreground,
                                outline: presented.outline,
                            },
                            false,
                            presented.centered,
                        )?;
                        draw.native_kinds.push(kind);
                    }
                }
                2 => {
                    let shrink_from = message.end * k::STACK_SHRINK;
                    pickup(&text, &mut draw, &message, offset)?;
                    draw.native_kinds.push(kind);
                    offset += if message.age >= shrink_from {
                        (1.0 - (message.age - shrink_from) / (message.end - shrink_from))
                            * weight as i32 as f32
                    } else {
                        weight as i32 as f32
                    };
                }
                3 => {
                    notice(&text, &mut draw, &message, slide)?;
                    draw.native_kinds.push(kind);
                }
                4 | 5 => {
                    title(&text, &mut draw, &message, slide)?;
                    draw.native_kinds.push(kind);
                }
                10 => {
                    scoring(&text, &mut draw, &message)?;
                    draw.native_kinds.push(kind);
                }
                6 => add_weight = false,
                7 => return Err("pad notice (kind 7 counts pad presses and clears the entry)"),
                9 => {
                    if g.call(FN_SPLIT_NOTICE, &[]).ok_or(UNREADABLE)? != 0 {
                        draw.ops.push(GuestOp::Drawer {
                            drawer: KIND_DRAWERS[8],
                            entry,
                        });
                    }
                }
                _ => {
                    if owned.is_some() {
                        return Err("native message drawer required");
                    }
                    draw.ops.push(GuestOp::Drawer {
                        drawer: KIND_DRAWERS[kind as usize - 1],
                        entry,
                    });
                }
            }
            // `0x12DF60`: kind 8 adds nothing (the age is at least 0 here).
            if add_weight && kind != 8 {
                sums[kind as usize] = sums[kind as usize].wrapping_add(weight as i32);
            }
        }
        Ok(draw)
    };
    match run() {
        Ok(draw) => QueueCall::Draw(draw),
        Err(why) => QueueCall::Legacy(why),
    }
}

/// Shadow comparison of one original `0x41E30` call against the expected native one.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OutlinedCheck {
    pub equal: bool,
    pub max_position_error: f32,
}

/// Compares an observed `0x41E30` call with the expected one (position within `tolerance`, text and colours equal).
pub fn compare(expected: &OutlinedCall, observed: &OutlinedCall, tolerance: f32) -> OutlinedCheck {
    let error = (expected.x - observed.x)
        .abs()
        .max((expected.y - observed.y).abs());
    OutlinedCheck {
        equal: error <= tolerance
            && expected.text == observed.text
            && expected.colour == observed.colour
            && expected.outline == observed.outline,
        max_position_error: error,
    }
}

#[cfg(test)]
mod tests {
    //! Tier 0, FABRICATED guest values on the real offsets (no font table: only the paths that need no measure).
    use super::*;
    use crate::hud_native::GuestRead;
    use std::collections::HashMap;

    struct Guest {
        words: HashMap<u32, u32>,
        players: u32,
        fail_font_metrics: bool,
    }
    impl GuestRead for Guest {
        fn read32(&self, va: u32) -> Option<u32> {
            if self.fail_font_metrics && va == crate::hud_prompts::prompt_addr::ALT_TABLE {
                return None;
            }
            Some(self.words.get(&va).copied().unwrap_or(0))
        }
    }
    impl GuestCall for Guest {
        fn call(&self, va: u32, _args: &[u32]) -> Option<u32> {
            Some(if va == message_addr::FN_PLAYERS {
                self.players
            } else {
                u32::from(va == message_addr::FN_SPLIT_NOTICE)
            })
        }
    }

    fn guest() -> Guest {
        use message_addr::*;
        let mut words = HashMap::new();
        words.insert(VIEWER, 0x1000);
        words.insert(0x1004, 0);
        words.insert(VIEWPORT_PTR, 0x2000);
        words.insert(0x2004, 640);
        words.insert(0x2018, 480);
        Guest {
            words,
            players: 1,
            fail_font_metrics: false,
        }
    }

    fn notice_fonts() -> FontSet {
        use crate::font::{FontObject, FontRecord, GlyphRect};
        use std::collections::BTreeMap;

        let record = FontRecord {
            cell_width: 8.0,
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
        FontSet {
            records: BTreeMap::from([(0, record)]),
            objects: vec![
                FontObject {
                    record: 0,
                    scale_x: 1.0,
                    scale_y: 1.0,
                    advance_adjust: 0,
                };
                22
            ],
            ..Default::default()
        }
    }

    fn notice_guest() -> Guest {
        use crate::hud_prompts::prompt_addr as p;
        use message_addr::*;

        let mut g = guest();
        for font in 0..22 {
            let object = p::FONT_OBJECTS + font * 20;
            g.words.insert(object, 0);
            g.words.insert(object + 4, 0);
            g.words.insert(object + 8, 1.0f32.to_bits());
            g.words.insert(object + 12, 1.0f32.to_bits());
            g.words.insert(object + 16, 0);
        }
        for table in [FONTS_PICKUP, 0x004e_7290, 0x004e_72cc] {
            for index in 0..5 {
                g.words.insert(table + index * 4, 0);
            }
        }
        for address in [
            SCALE_X,
            SCALE_Y,
            p::SCALE_ONE,
            p::TEXT_SCALE_X,
            p::TEXT_SCALE_TOP,
            p::TEXT_SCALE_Y,
            p::HEIGHT_FACTOR,
        ] {
            g.words.insert(address, 1.0f32.to_bits());
        }
        g.words.insert(p::HEIGHT_UNITS, 12);
        g.words.insert(p::FONT_RECORDS + 0x18, 12.0f32.to_bits());
        g.words.insert(0x0074_90ec, 12.0f32.to_bits());
        g.words.insert(0x0047_d1f8, 1.0f32.to_bits());
        g.words.insert(0x0047_60cc, 5.0f32.to_bits());
        g.words.insert(0x0047_5c78, 1.0f32.to_bits());
        g.words.insert(0x0047_5cc0, 100.0f32.to_bits());
        g
    }

    fn owned_notice(kind: u32, text: &[u8]) -> crate::hud_native::HudState {
        let mut state = crate::hud_native::HudState::default();
        state.messages[0].ring.entries[0] = Message {
            kind,
            text: text.to_vec(),
            age: 0.5,
            fade_start: 2.0,
            end: 4.0,
            weight: 1,
            ..Default::default()
        };
        state
    }

    #[test]
    fn native_corner_notice_renders_owned_text_through_queue() {
        use crate::hud_producers::MessagePolicy;

        let g = notice_guest();
        let fonts = notice_fonts();
        let mut state = owned_notice(8, b"corner notice");
        let before = state.messages[0].clone();
        let policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };

        let QueueCall::Draw(draw) = replay_owned_native(&g, &fonts, 0, &mut state, 0, &policy)
        else {
            panic!("readable kind-8 notice should use the owned presenter");
        };
        assert_eq!(draw.native_kinds, [8]);
        assert_eq!(draw.texts.len(), 1);
        assert_eq!(draw.texts[0].font, 0);
        assert_eq!(draw.texts[0].colour, 0x6464_6464);
        assert_eq!(draw.texts[0].runs[0].bytes, b"corner notice");
        assert_eq!(draw.texts[0].runs[0].x, 0.0);
        assert_eq!(draw.texts[0].runs[0].y, 473.0);
        assert_eq!(state.messages[0], before);
    }

    #[test]
    fn native_split_notice_renders_visible_owned_text_through_queue() {
        use crate::hud_producers::MessagePolicy;

        let g = notice_guest();
        let fonts = notice_fonts();
        let mut state = owned_notice(9, b"split notice");
        let before = state.messages[0].clone();
        let policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };

        let QueueCall::Draw(draw) = replay_owned_native(&g, &fonts, 0, &mut state, 0, &policy)
        else {
            panic!("readable visible kind-9 notice should use the owned presenter");
        };
        assert_eq!(draw.native_kinds, [9]);
        assert_eq!(draw.texts.len(), 1);
        assert_eq!(draw.texts[0].font, 0x15);
        assert_eq!(draw.texts[0].colour, 0x5f5f_5fff);
        assert_eq!(draw.texts[0].outline, 0x0f0f_0f7f);
        assert_eq!(draw.texts[0].runs[0].bytes, b"split notice");
        assert_eq!(draw.calls.len(), 1);
        assert_eq!(draw.calls[0].x, 320.0);
        assert_eq!(state.messages[0], before);
    }

    #[test]
    fn owned_queue_split_notice_uses_native_presenter() {
        use crate::hud_producers::MessagePolicy;

        let g = notice_guest();
        let fonts = notice_fonts();
        let state = owned_notice(9, b"split notice");
        let before = state.messages[0].clone();
        let policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };

        let QueueCall::Draw(draw) = replay_owned(&g, &fonts, 0, &state.messages[0], &policy) else {
            panic!("readable visible kind-9 queue should use the native presenter");
        };
        assert_eq!(draw.native_kinds, [9]);
        assert_eq!(draw.texts.len(), 1);
        assert_eq!(draw.texts[0].font, 0x15);
        assert_eq!(draw.texts[0].colour, 0x5f5f_5fff);
        assert_eq!(draw.texts[0].outline, 0x0f0f_0f7f);
        assert_eq!(draw.texts[0].runs[0].bytes, b"split notice");
        assert_eq!(draw.calls.len(), 1);
        assert_eq!(draw.calls[0].x, 320.0);
        assert_eq!(state.messages[0], before);
    }

    #[test]
    fn native_progress_notice_uses_rust_presenter_without_lifted_drawer() {
        use crate::hud_producers::MessagePolicy;

        let mut g = notice_guest();
        g.words.insert(0x0047_82c4, 2.0f32.to_bits());
        let fonts = notice_fonts();
        let mut state = crate::hud_native::HudState::default();
        state.messages[0].ring.entries[0] = Message {
            kind: 7,
            text: b"continue".to_vec(),
            age: 0.5,
            fade_start: 2.0,
            end: 4.0,
            weight: 1,
            ..Default::default()
        };
        state.input_progress[0] = 2;
        let before = state.messages[0].clone();
        let policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };

        let QueueCall::Draw(draw) = replay_owned_native(&g, &fonts, 0, &mut state, 0, &policy)
        else {
            panic!("readable kind-7 notice should use the owned presenter");
        };
        assert_eq!(draw.native_kinds, [7]);
        assert!(draw.original_calls().is_empty());
        assert_eq!(draw.texts.len(), 1);
        assert_eq!(draw.texts[0].runs[0].bytes, b"continue");
        assert_eq!(state.messages[0], before);
        assert_eq!(state.input_progress[0], 2);
    }

    #[test]
    fn native_pickup_uses_rust_presenter_without_lifted_drawer() {
        use crate::hud_producers::MessagePolicy;

        let mut g = notice_guest();
        g.words.insert(0x200c, 640);
        let fonts = notice_fonts();
        let mut state = owned_notice(2, b"pickup notice");
        let before = state.messages[0].clone();
        let policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };

        let QueueCall::Draw(draw) = replay_owned_native(&g, &fonts, 0, &mut state, 0, &policy)
        else {
            panic!("readable kind-2 pickup should use the owned presenter");
        };

        assert_eq!(draw.native_kinds, [2]);
        assert!(draw.original_calls().is_empty());
        assert_eq!(draw.texts.len(), 1);
        assert_eq!(draw.texts[0].runs[0].bytes, b"pickup notice");
        assert_eq!(state.messages[0], before);
    }

    #[test]
    fn native_wrapped_notice_and_titles_do_not_need_a_second_notice_kind() {
        use crate::hud_producers::MessagePolicy;
        use message_addr::{FONTS_SINGLE_BASE, FONTS_TITLE, FONTS_TITLE_BASE};

        let mut g = notice_guest();
        for table in [FONTS_TITLE, FONTS_TITLE_BASE, FONTS_SINGLE_BASE] {
            for index in 0..5 {
                g.words.insert(table + index * 4, 0);
            }
        }
        let fonts = notice_fonts();
        let policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };

        for kind in [3, 4, 5] {
            let mut state = owned_notice(kind, b"owned message");
            state.messages[0].slides[0] = 1.0;
            let QueueCall::Draw(draw) = replay_owned_native(&g, &fonts, 0, &mut state, 0, &policy)
            else {
                panic!("kind {kind} should use its native owned presenter");
            };
            assert_eq!(draw.native_kinds, [kind]);
            assert!(draw.original_calls().is_empty());
            assert!(!draw.texts.is_empty());
        }
    }

    #[test]
    fn native_scoring_notice_uses_owned_payload_and_single_viewer_presenter() {
        use crate::hud_producers::MessagePolicy;
        use message_addr::*;

        let mut g = notice_guest();
        g.words.insert(PLAYERS_PTR, 0x1000);
        g.words.insert(0x1000 + PLAYER_NAME, 0x3000);
        g.words.insert(0x3000, u32::from_le_bytes(*b"Ada\0"));
        g.words.insert(GAME_FLAGS, 0);
        for table in [FONTS_TITLE, FONTS_PICKUP] {
            g.words.insert(table, 0);
        }
        let fonts = notice_fonts();
        let mut state = owned_notice(10, b"Round won");
        state.messages[0].ring.entries[0].opaque = 0;
        let policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };

        let QueueCall::Draw(draw) = replay_owned_native(&g, &fonts, 0, &mut state, 0, &policy)
        else {
            panic!("single-viewer kind 10 should use its native owned presenter");
        };
        assert_eq!(draw.native_kinds, [10]);
        assert!(draw.original_calls().is_empty());
        assert!(draw.calls.iter().any(|call| call.text == b"Round won Ada"));
    }

    #[test]
    fn native_queue_dispatches_all_supported_owned_notice_kinds_together() {
        use crate::hud_producers::MessagePolicy;
        use message_addr::*;

        let mut g = notice_guest();
        g.words.insert(PLAYERS_PTR, 0x1000);
        g.words.insert(0x1000 + PLAYER_NAME, 0x3000);
        g.words.insert(0x3000, u32::from_le_bytes(*b"Ada\0"));
        g.words.insert(GAME_FLAGS, 0);
        for table in [FONTS_TITLE, FONTS_TITLE_BASE, FONTS_SINGLE_BASE] {
            for index in 0..5 {
                g.words.insert(table + index * 4, 0);
            }
        }
        let fonts = notice_fonts();
        let mut state = crate::hud_native::HudState::default();
        for kind in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] {
            let entry = &mut state.messages[0].ring.entries[kind as usize - 1];
            *entry = Message {
                kind,
                text: b"owned message".to_vec(),
                age: 0.5,
                fade_start: 2.0,
                end: 4.0,
                weight: 1,
                opaque: 0,
                ..Default::default()
            };
            state.messages[0].slides[kind as usize - 1] = 1.0;
        }
        let policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };

        let QueueCall::Draw(draw) = replay_owned_native(&g, &fonts, 0, &mut state, 0, &policy)
        else {
            panic!("supported owned notice kinds should use the native queue path");
        };
        assert_eq!(draw.native_kinds, [1, 2, 3, 4, 5, 7, 8, 9, 10]);
        assert!(draw.original_calls().is_empty());
        assert!(!draw.texts.is_empty());
    }

    #[test]
    fn native_progress_render_failure_keeps_typed_input_and_queue() {
        use crate::hud_producers::MessagePolicy;

        let mut g = guest();
        // Force a late renderer failure after native action/progress has been
        // staged, so the adapter must leave the owned state untouched.
        g.fail_font_metrics = true;
        let fonts = FontSet::default();
        let mut state = crate::hud_native::HudState::default();
        state.messages[0].ring.entries[0].kind = 7;
        state.messages[0].ring.entries[0].text = b"press to continue".to_vec();
        assert!(state.publish_message_buttons(0, 0x08, 0x08));
        let policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };

        assert!(matches!(
            replay_owned_native(&g, &fonts, 0, &mut state, 0, &policy),
            QueueCall::Legacy(_)
        ));
        assert_eq!(state.messages[0].ring.entries[0].kind, 7);
        assert_eq!(state.input_progress[0], 0);
        let mut progress = Vec::new();
        state
            .try_replay_messages_native(0, &policy, false, true, |_, _, _, value| {
                progress.push(value);
                Ok(())
            })
            .unwrap();
        assert_eq!(progress, [1]);
        assert_eq!(state.input_progress[0], 1);
    }

    #[test]
    fn native_center_notice_metric_failure_preserves_owned_message() {
        use crate::hud_producers::MessagePolicy;

        let g = guest();
        let fonts = FontSet::default();
        let mut state = crate::hud_native::HudState::default();
        state.messages[0].ring.entries[0] = Message {
            kind: 1,
            text: b"objective complete".to_vec(),
            age: 0.5,
            fade_start: 2.0,
            end: 4.0,
            weight: 1,
            ..Default::default()
        };
        let before = state.messages[0].clone();
        let policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };

        assert_eq!(
            replay_owned_native(&g, &fonts, 0, &mut state, 0, &policy),
            QueueCall::Legacy("native centered notice metrics unavailable")
        );
        assert_eq!(state.messages[0], before);
    }

    #[test]
    fn owned_center_notice_uses_rust_presenter_without_lifted_drawer() {
        use crate::hud_producers::MessagePolicy;

        let g = notice_guest();
        let fonts = notice_fonts();
        let state = owned_notice(1, b"objective complete");
        let policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };

        let QueueCall::Draw(draw) = replay_owned(&g, &fonts, 0, &state.messages[0], &policy) else {
            panic!("owned kind-1 notice should use the Rust presenter");
        };

        assert_eq!(draw.native_kinds, [1]);
        assert!(draw.original_calls().is_empty());
        assert_eq!(draw.texts.len(), 1);
        assert_eq!(draw.texts[0].runs[0].bytes, b"objective complete");
    }

    #[test]
    fn owned_corner_notice_uses_rust_presenter_without_lifted_drawer() {
        use crate::hud_producers::MessagePolicy;

        let g = notice_guest();
        let fonts = notice_fonts();
        let state = owned_notice(8, b"corner notice");
        let policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };

        let QueueCall::Draw(draw) = replay_owned(&g, &fonts, 0, &state.messages[0], &policy) else {
            panic!("owned kind-8 notice should use the Rust presenter");
        };

        assert_eq!(draw.native_kinds, [8]);
        assert!(draw.original_calls().is_empty());
        assert_eq!(draw.texts.len(), 1);
        assert_eq!(draw.texts[0].runs[0].bytes, b"corner notice");
        assert_eq!(draw.texts[0].runs[0].x, 0.0);
        assert_eq!(draw.texts[0].runs[0].y, 473.0);
        assert_eq!(draw.texts[0].colour, 0x6464_6464);
    }

    #[test]
    fn native_corner_notice_metric_failure_preserves_owned_message() {
        use crate::hud_producers::MessagePolicy;

        let g = guest();
        let fonts = FontSet::default();
        let mut state = crate::hud_native::HudState::default();
        state.messages[0].ring.entries[0] = Message {
            kind: 8,
            text: b"corner notice".to_vec(),
            age: 0.5,
            fade_start: 2.0,
            end: 4.0,
            weight: 1,
            ..Default::default()
        };
        let before = state.messages[0].clone();
        let policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };

        assert_eq!(
            replay_owned_native(&g, &fonts, 0, &mut state, 0, &policy),
            QueueCall::Legacy("native corner notice metrics unavailable")
        );
        assert_eq!(state.messages[0], before);
    }

    #[test]
    fn corner_notice_layout_uses_source_inspected_colour_base() {
        let g = guest();
        let frame = Frame::read(&g).expect("fabricated viewport fixture is readable");
        let layout = corner_notice_layout(&g, frame).expect("fabricated HUD values are readable");

        assert_eq!(layout.colour_base, 0x6464_6400);
    }

    #[test]
    fn native_kind6_message_is_owned_no_draw_no_weight_case() {
        use crate::hud_producers::MessagePolicy;

        let g = guest();
        let fonts = FontSet::default();
        let mut state = crate::hud_native::HudState::default();
        state.messages[0].ring.entries[0] = Message {
            kind: 6,
            text: b"objective event".to_vec(),
            age: 0.5,
            fade_start: 2.0,
            end: 4.0,
            weight: 1,
            ..Default::default()
        };
        let before = state.messages[0].clone();
        let policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };

        let QueueCall::Draw(draw) = replay_owned_native(&g, &fonts, 0, &mut state, 0, &policy)
        else {
            panic!("kind-6's measured no-draw case should remain native");
        };
        assert!(draw.native_kinds.is_empty());
        assert_eq!(state.messages[0], before);
    }

    #[test]
    fn native_kind6_producer_record_reaches_no_draw_scheduler() {
        use crate::hud_producers::{Kind6MessageTuning, MessagePolicy};

        let g = guest();
        let fonts = FontSet::default();
        let mut state = crate::hud_native::HudState::default();
        let cue = state
            .post_kind6_message(
                0,
                0x1234,
                0.25,
                Kind6MessageTuning {
                    fade_start: 3.0,
                    end: 5.0,
                    evict_after: 3.5,
                    cue_scale: 100.0,
                },
            )
            .expect("non-null source creates kind-6 event");
        assert_eq!(cue.event_id, 0x0d6c);
        assert_eq!(cue.parameter, 25);
        assert!(state.messages[0].ring.entries[0].text.is_empty());
        assert_eq!(state.messages[0].progress[0], 0x1234);

        let before = state.messages[0].clone();
        let policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };
        let QueueCall::Draw(draw) = replay_owned_native(&g, &fonts, 0, &mut state, 0, &policy)
        else {
            panic!("kind-6's measured no-draw case should remain native");
        };
        assert!(draw.native_kinds.is_empty());
        assert_eq!(state.messages[0], before);
    }

    #[test]
    fn native_split_notice_render_failure_preserves_owned_message() {
        use crate::hud_producers::MessagePolicy;

        let g = guest();
        let fonts = FontSet::default();
        let mut state = crate::hud_native::HudState::default();
        state.messages[0].ring.entries[0] = Message {
            kind: 9,
            text: b"split notice".to_vec(),
            age: 0.5,
            fade_start: 2.0,
            end: 4.0,
            weight: 1,
            ..Default::default()
        };
        let before = state.messages[0].clone();
        let policy = MessagePolicy {
            limits: [i32::MAX; 14],
            conflicts: vec![],
        };

        // The failure may occur while deriving wrapped text or while building
        // the renderer's font metrics. Both paths must fall back transactionally.
        assert!(matches!(
            replay_owned_native(&g, &fonts, 0, &mut state, 0, &policy),
            QueueCall::Legacy(_)
        ));
        assert_eq!(state.messages[0], before);
    }

    #[test]
    fn notice_adapter_preserves_byte_scan_and_low_integer_word() {
        use crate::hud_producers::{MessageTextMetrics, ProgressNoticeText};
        let mut g = guest();
        let fonts = FontSet::default();
        let text = NativeNoticeText::read(&g, &fonts).unwrap();
        let bytes = b"a,12345\x0b1\nlast\0ignored";
        assert_eq!(text.count(bytes), Some(14));
        assert_eq!(text.text_count(bytes), Some(14));
        assert_eq!(text.outlined_weight(bytes), Some(2));
        assert_eq!(text.outlined_weight(b"\0ignored"), Some(0));
        assert_eq!(text.prefix_offset(bytes, 7), Some(7));
        assert_eq!(text.prefix_offset(bytes, u32::MAX), None);
        drop(text);
        g.words.insert(0x6b93a8, 1);
        let text = NativeNoticeText::read(&g, &fonts).unwrap();
        assert_eq!(text.prefix_offset(bytes, 7), Some(7));
        assert_eq!(text.prefix_offset(bytes, u32::MAX), Some(0));
        assert_eq!(text.prefix_offset(bytes, 100), Some(14));
        for (input, expected) in [
            (2.9, 2),
            (-2.9, -2),
            (4_294_967_297.5, 1),
            (f64::NAN, 0),
            (f64::INFINITY, 0),
            (-9_223_372_036_854_775_808.0, 0),
        ] {
            assert_eq!(text_integer(input), expected);
        }
    }

    fn entry(g: &mut Guest, index: u32, kind: u32, age: f32, weight: u32) -> u32 {
        use message_addr::*;
        let at = HUD_RECORDS + RING_ENTRIES + index * ENTRY_SIZE;
        g.words.insert(at + KIND, kind);
        g.words.insert(at + AGE, age.to_bits());
        g.words.insert(at + WEIGHT, weight);
        at
    }

    #[test]
    fn kinds_not_replayed_go_to_their_drawer_in_ring_order_with_limits_and_conflicts() {
        use message_addr::*;
        let mut g = guest();
        g.words.insert(HUD_RECORDS + RING_CURSOR, 14);
        for kind in [1u32, 9, 8] {
            g.words.insert(KIND_LIMITS + 4 * kind, 1);
        }
        let a = entry(&mut g, 14, 1, 0.5, 1);
        // Second kind 1 entry passes the limit (sum 1 + 1 > 1): hidden.
        entry(&mut g, 15, 1, 0.5, 1);
        let b = entry(&mut g, 0, 9, 0.0, 1);
        // Not shown yet (negative age), and an unused entry.
        entry(&mut g, 1, 8, -0.5, 1);
        entry(&mut g, 2, 0, 1.0, 1);
        // Kind 8 conflicts with the shown kind 1 through a pair (1, 8).
        g.words.insert(CONFLICT_LAST - 4, 1);
        g.words.insert(CONFLICT_LAST, 8);
        entry(&mut g, 3, 8, 0.5, 1);
        let fonts = FontSet::default();
        let QueueCall::Draw(draw) = replay(&g, &fonts, 0) else {
            panic!("expected a draw")
        };
        assert_eq!(
            draw.original_calls(),
            vec![(0x0014_9590, a), (0x0014_9710, b)]
        );
        assert_eq!(draw.ops[0], GuestOp::Blend(1));
        assert!(draw.texts.is_empty());
        // Only kind 9 requested: nothing else drawn.
        let QueueCall::Draw(draw) = replay(&g, &fonts, 1) else {
            panic!("expected a draw")
        };
        assert_eq!(draw.original_calls(), vec![(0x0014_9710, b)]);
    }

    #[test]
    fn owned_queue_never_reads_guest_entries_or_policy() {
        struct Inputs(Guest);
        impl GuestRead for Inputs {
            fn read32(&self, va: u32) -> Option<u32> {
                use message_addr::*;
                assert!(!(HUD_RECORDS..HUD_RECORDS + HUD_RECORD_SIZE * 4).contains(&va));
                assert!(!(KIND_LIMITS..KIND_LIMITS + 64).contains(&va));
                self.0.read32(va)
            }
        }
        impl GuestCall for Inputs {
            fn call(&self, va: u32, args: &[u32]) -> Option<u32> {
                self.0.call(va, args)
            }
        }
        let mut queue = crate::hud_producers::OwnedMessageQueue::default();
        queue.ring.entries[0] = Message {
            kind: 6,
            age: 1.0,
            weight: 1,
            ..Default::default()
        };
        let policy = crate::hud_producers::MessagePolicy {
            limits: [2; 14],
            conflicts: vec![],
        };
        let result = replay_owned(&Inputs(guest()), &FontSet::default(), 0, &queue, &policy);
        let QueueCall::Draw(draw) = result else {
            panic!("{result:?}")
        };
        assert_eq!(draw.ops, vec![GuestOp::Blend(1)]);
        assert!(draw.texts.is_empty());
    }

    #[test]
    fn pad_notice_and_missing_viewer_are_not_replayed() {
        let mut g = guest();
        g.words.insert(message_addr::KIND_LIMITS + 28, 5);
        entry(&mut g, 0, 7, 0.0, 1);
        let fonts = FontSet::default();
        assert!(matches!(replay(&g, &fonts, 0), QueueCall::Legacy(_)));
        g.words.insert(0x1004, u32::MAX);
        assert_eq!(replay(&g, &fonts, 0), QueueCall::Nothing);
    }

    #[test]
    fn team_outline_and_alpha_scale() {
        assert_eq!(team_outline(0xff80_4060), 0x1f10_0860);
        assert_eq!(scale_alpha(0x1020_3080, 0.5), 0x1020_3040);
        assert_eq!(scale_alpha(0x1020_3080, 2.0), 0x1020_3080);
    }

    #[test]
    fn shadow_compare_needs_text_colours_and_position() {
        let a = OutlinedCall {
            x: 10.0,
            y: 20.0,
            text: b"x".to_vec(),
            colour: 1,
            outline: 2,
        };
        assert!(compare(&a, &a, 0.0).equal);
        let moved = OutlinedCall {
            x: 11.0,
            ..a.clone()
        };
        assert!(!compare(&a, &moved, 0.5).equal);
        assert!(compare(&a, &moved, 1.0).equal);
        let recoloured = OutlinedCall {
            colour: 3,
            ..a.clone()
        };
        assert!(!compare(&a, &recoloured, 1.0).equal);
    }
}
