//! Production text measure from the font record tables (T2189 UI-6 slice 3).
//!
//! The tables are static data of the owner's image (`docs/t-ui-data-model.md` section 8): 8 font
//! records of 0x38 bytes at 0x4C8C28 (2 populated), 30 font objects of 0x14 bytes at 0x4C8DE8, per
//! record a glyph table of 96 entries of 12 bytes (kern row, kern column, advance adjust, for the
//! character codes 0x20..0x7F) and a 13 x 13 dword kerning matrix. Nothing here embeds font data:
//! [`FontSet::load`] reads the tables at run time through a [`StaticImage`], tests build synthetic
//! tables with the same structure.
//!
//! Evidence labels. The record and object field layout and the table shapes are MEASURED. The
//! width formula below (`step = cell_width * scale_x + glyph adjust + kerning[prev.column][next.row]
//! + object adjust`, adjustments in unscaled pixels) is INFERRED: the original string core (0x3FCF0)
//! is not ported, no width capture exists in the dumps, so it is opt-in until checked against
//! rendered text. Line heights stay CALIBRATED (`line_px_per_scale`, see [`FontTableMeasure::calibrate`]).
//!
//! Slice 16 (T2189): a populated record also points (+0x8) at a table of 107 glyph rects in its font texture (u16 x, y, w,
//! h in texture pixels and an f32 top offset, MEASURED layout, the texture is resource `resource_id`, 964 and 965). When the
//! rects are loaded the advance follows the original measure (0x3FCF0 calling 0x3FAA0, read from the disassembly, INFERRED
//! until checked against rendered frames): `step = (rect.w + object adjust + accent adjust + glyph adjust +
//! kerning[this.column][next.row]) * scale_x`, the kerning pairing a glyph with the character after it.

use std::collections::BTreeMap;

use crate::layout::TextMeasure;
use crate::model::{TextId, TextRef};

pub const RECORD_ADDRESS: u32 = 0x4C8C28;
pub const RECORD_STRIDE: u32 = 0x38;
pub const RECORD_COUNT: u32 = 8;
pub const OBJECT_ADDRESS: u32 = 0x4C8DE8;
pub const OBJECT_STRIDE: u32 = 0x14;
pub const OBJECT_COUNT: u32 = 30;
pub const GLYPHS: usize = 96;
pub const GLYPH_STRIDE: u32 = 12;
pub const KERN_SIZE: usize = 13;
pub const FIRST_CHAR: u8 = 0x20;

/// T2316: the font escape of `0x40FA0` / `0x3FCF0`: 0x0B and one operand byte. Operands '1'..'<' (12 slots) name an inline
/// picture, texture handle `[0x7E72A0 + (operand - '1') * 4]` (filled by `0x812F0`), drawn as a square of the style's text
/// height at the pen, which then advances by that height (MEASURED by static reading, T2315 item "Controls"). Letter
/// operands ('A'..'T', the animated HUD icon table `0x4C8248`) are laid out by `hud_prompts`, not here: they are skipped
/// with no width.
pub const PICTURE_ESCAPE: u8 = 0x0B;
pub const PICTURE_FIRST: u8 = b'1';
pub const PICTURE_SLOTS: u8 = 12;
/// The picture handle table `[0x7E72A0]` (12 words).
pub const PICTURE_HANDLES: u32 = 0x7E_72A0;

/// One inline picture of a laid-out text: handle table slot (operand minus '1'), pen x and edge (design pixels).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InlinePicture {
    pub slot: u8,
    pub pen: f32,
    pub size: f32,
}

/// Glyph pens `(code, pen, advance)` and pictures of one text, and its width (the pen after the last item).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InlineLayout {
    pub glyphs: Vec<(u8, f32, f32)>,
    pub pictures: Vec<InlinePicture>,
    pub width: f32,
}

/// A piece of a text between picture escapes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Segment<'a> {
    Text(&'a [u8]),
    Picture(u8),
    /// An escape that is not a picture (a HUD letter icon or an operand out of range).
    Skipped,
}

/// Splits `text` at its 0x0B escapes (a trailing 0x0B takes the NUL as its operand, skipped).
pub fn split_pictures(text: &[u8]) -> Vec<Segment<'_>> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < text.len() {
        if text[i] != PICTURE_ESCAPE {
            i += 1;
            continue;
        }
        if start < i {
            out.push(Segment::Text(&text[start..i]));
        }
        let slot = text.get(i + 1).map(|op| op.wrapping_sub(PICTURE_FIRST));
        out.push(match slot {
            Some(slot) if slot < PICTURE_SLOTS => Segment::Picture(slot),
            _ => Segment::Skipped,
        });
        i += 2;
        start = i;
    }
    if start < text.len() {
        out.push(Segment::Text(&text[start..]));
    }
    out
}
/// Glyph rects per record (+0x8): 96 for 0x20..0x7F and 11 extra cells.
pub const RECTS: usize = 107;
pub const RECT_STRIDE: u32 = 12;
/// Shared accent tables for bytes 0x80..0xFF: base character, accent record index.
pub const ACCENT_BASE_ADDRESS: u32 = 0x4C9040;
pub const ACCENT_INDEX_ADDRESS: u32 = 0x4C90C0;
pub const ACCENT_RECORD_STRIDE: u32 = 0x28;

/// Read-only access to static data by virtual address (the XBE image, or a synthetic buffer).
pub trait StaticImage {
    fn byte(&self, address: u32) -> Option<u8>;
    fn u32_at(&self, address: u32) -> Option<u32> {
        let mut out = [0u8; 4];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = self.byte(address.checked_add(i as u32)?)?;
        }
        Some(u32::from_le_bytes(out))
    }
    fn f32_at(&self, address: u32) -> Option<f32> {
        self.u32_at(address).map(f32::from_bits)
    }
}

/// Source of string bytes by id (the language bank loaded at run time). Strings never enter the repo.
pub trait StringSource {
    fn bytes(&self, id: TextId) -> Option<&[u8]>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Glyph {
    pub kern_row: u32,
    pub kern_column: u32,
    pub adjust: i32,
}

/// One cell of the font texture (texture pixels) and its top offset below the line top (unscaled pixels).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct GlyphRect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
    pub top: f32,
}

/// Accented byte (0x80..0xFF) handling shared by all records (0x3FAA0, 0x3FCF0).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct AccentTables {
    /// Base character per `byte & 0x7F` (signed in the original: values >= 0x80 select no glyph).
    pub base: Vec<u8>,
    /// Accent record index per `byte & 0x7F`.
    pub index: Vec<i8>,
}

/// Record metrics used by `0x3FA70(1)` (`0x3FCF0` measure and text draw).
/// Every glyph takes `advance`, while dot/colon and slash have separate widths/offsets.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FixedSpacing {
    pub advance: f32,
    pub digit_offset: f32,
    pub punctuation_offset: f32,
    pub punctuation_ratio: f32,
    pub slash_offset: f32,
    pub slash_ratio: f32,
}

impl FixedSpacing {
    fn advance(self, code: u8) -> f32 {
        match code {
            b'.' | b':' => self.advance * self.punctuation_ratio + self.punctuation_offset,
            b'/' => self.advance * self.slash_ratio + self.slash_offset,
            _ => self.advance,
        }
    }
    fn offset(self, code: u8) -> f32 {
        match code {
            b'.' | b':' => self.punctuation_offset,
            b'/' => self.slash_offset,
            b'1' => self.digit_offset,
            _ => 0.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct FontRecord {
    /// Font texture resource (964 / 965 in the owner's image).
    pub resource_id: i32,
    /// None when the optional runtime record metrics are unavailable.
    pub fixed_spacing: Option<FixedSpacing>,
    /// Historical field name: record +0x18, the text-top height used by 0x3F8D0.
    pub cell_width: f32,
    pub cell_height: f32,
    pub glyphs: Vec<Glyph>,
    /// Row-major 13 x 13.
    pub kerning: Vec<i32>,
    /// Glyph cells in the font texture; empty selects the slice 3 cell-width formula.
    pub rects: Vec<GlyphRect>,
    /// Per accent index n: the f32 the advance adds (+0xC table at `n * 0x28 - 4`).
    pub accent_adjust: Vec<f32>,
    /// Per accent index n >= 1: the character whose cell an accented space draws (`(n - 1) * 0x28`).
    pub accent_cell: Vec<u8>,
}

impl FontRecord {
    pub fn glyph(&self, code: u8) -> Glyph {
        let index = code.wrapping_sub(FIRST_CHAR) as usize;
        self.glyphs.get(index).copied().unwrap_or_default()
    }
    pub fn kern(&self, previous: Glyph, next: Glyph) -> i32 {
        let (row, column) = (previous.kern_column as usize, next.kern_row as usize);
        if row < KERN_SIZE && column < KERN_SIZE {
            self.kerning
                .get(row * KERN_SIZE + column)
                .copied()
                .unwrap_or(0)
        } else {
            0
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontObject {
    pub record: u32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub advance_adjust: i32,
}

/// Records (by index, only populated ones) and font objects (by font index).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontSet {
    pub records: BTreeMap<u32, FontRecord>,
    pub objects: Vec<FontObject>,
    pub accents: AccentTables,
}

impl FontSet {
    /// Reads the tables from `image`. A record is populated when its glyph and kerning pointers
    /// and its cell size read back; unused placeholders are skipped. Objects stop at the first
    /// unreadable entry or the first record index that is not a populated record.
    pub fn load(image: &dyn StaticImage) -> Self {
        let mut set = FontSet::default();
        let table = |address: u32| -> Vec<u8> {
            (0..128u32).map_while(|i| image.byte(address + i)).collect()
        };
        let (base, index) = (table(ACCENT_BASE_ADDRESS), table(ACCENT_INDEX_ADDRESS));
        if base.len() == 128 && index.len() == 128 {
            set.accents = AccentTables {
                base,
                index: index.into_iter().map(|b| b as i8).collect(),
            };
        }
        let accents = set.accents.index.iter().copied().max().unwrap_or(0).max(0) as u32;
        for index in 0..RECORD_COUNT {
            if let Some(record) =
                Self::load_record(image, RECORD_ADDRESS + index * RECORD_STRIDE, accents)
            {
                set.records.insert(index, record);
            }
        }
        for index in 0..OBJECT_COUNT {
            let base = OBJECT_ADDRESS + index * OBJECT_STRIDE;
            let (Some(record), Some(sx), Some(sy), Some(adj)) = (
                image.u32_at(base),
                image.f32_at(base + 8),
                image.f32_at(base + 12),
                image.u32_at(base + 16),
            ) else {
                break;
            };
            if !set.records.contains_key(&record) {
                break;
            }
            set.objects.push(FontObject {
                record,
                scale_x: sx,
                scale_y: sy,
                advance_adjust: adj as i32,
            });
        }
        set
    }

    fn load_record(image: &dyn StaticImage, base: u32, accents: u32) -> Option<FontRecord> {
        let glyph_table = image.u32_at(base + 0x10)?;
        let kerning = image.u32_at(base + 0x14)?;
        let (width, height) = (image.f32_at(base + 0x18)?, image.f32_at(base + 0x1C)?);
        if glyph_table == 0
            || kerning == 0
            || !(width > 0.0 && width.is_finite())
            || !height.is_finite()
        {
            return None;
        }
        let mut glyphs = Vec::with_capacity(GLYPHS);
        for i in 0..GLYPHS as u32 {
            let at = glyph_table + i * GLYPH_STRIDE;
            glyphs.push(Glyph {
                kern_row: image.u32_at(at)?,
                kern_column: image.u32_at(at + 4)?,
                adjust: image.u32_at(at + 8)? as i32,
            });
        }
        let mut matrix = Vec::with_capacity(KERN_SIZE * KERN_SIZE);
        for i in 0..(KERN_SIZE * KERN_SIZE) as u32 {
            matrix.push(image.u32_at(kerning + i * 4)? as i32);
        }
        let mut record = FontRecord {
            resource_id: image.u32_at(base).unwrap_or(0) as i32,
            cell_width: width,
            cell_height: height,
            glyphs,
            kerning: matrix,
            ..FontRecord::default()
        };
        let metrics: Option<Vec<f32>> = (0..6).map(|n| image.f32_at(base + 0x1C + 4 * n)).collect();
        record.fixed_spacing = metrics
            .filter(|m| m.iter().all(|v| v.is_finite()) && m[0] > 0.0)
            .map(|m| FixedSpacing {
                advance: m[0],
                digit_offset: m[1],
                punctuation_offset: m[2],
                punctuation_ratio: m[3],
                slash_offset: m[4],
                slash_ratio: m[5],
            });
        // Optional parts: a record without them keeps the slice 3 formula.
        if let Some(rects) = image
            .u32_at(base + 0x8)
            .and_then(|at| Self::load_rects(image, at))
        {
            record.rects = rects;
        }
        if let Some(table) = image.u32_at(base + 0xC).filter(|&at| at != 0) {
            let adjust: Option<Vec<f32>> = (0..=accents)
                .map(|n| image.f32_at((table + n * ACCENT_RECORD_STRIDE).checked_sub(4)?))
                .collect();
            let cell: Option<Vec<u8>> = (1..=accents)
                .map(|n| image.byte(table + (n - 1) * ACCENT_RECORD_STRIDE))
                .collect();
            if let (Some(adjust), Some(cell)) = (adjust, cell) {
                record.accent_adjust = adjust;
                record.accent_cell = cell;
            }
        }
        Some(record)
    }

    fn load_rects(image: &dyn StaticImage, table: u32) -> Option<Vec<GlyphRect>> {
        if table == 0 {
            return None;
        }
        let half = |at: u32| -> Option<u16> {
            Some(u16::from_le_bytes([image.byte(at)?, image.byte(at + 1)?]))
        };
        (0..RECTS as u32)
            .map(|i| {
                let at = table + i * RECT_STRIDE;
                Some(GlyphRect {
                    x: half(at)?,
                    y: half(at + 2)?,
                    w: half(at + 4)?,
                    h: half(at + 6)?,
                    top: image.f32_at(at + 8)?,
                })
            })
            .collect()
    }

    /// Font object and record of font index `font`.
    pub fn font(&self, font: u32) -> Option<(&FontObject, &FontRecord)> {
        self.resolve(font)
    }

    /// Original text-top metric (+0x18), in design pixels before window/draw scaling.
    /// This differs from the +0x1C line advance used by the ordinary shaper baseline.
    pub fn text_top_height(&self, font: u32) -> Option<f32> {
        let (object, record) = self.font(font)?;
        Some(record.cell_width * object.scale_y)
    }

    /// The cell `code` draws with (0x3FCF0): accented bytes use their base character, an accented space the cell of its
    /// accent record (none: the space cell), '|' the cell at 0x180 (entry 32). None for codes without a cell.
    pub fn glyph_rect(&self, record: &FontRecord, code: u8) -> Option<GlyphRect> {
        let accented = code & 0x80 != 0;
        let low = usize::from(code & 0x7F);
        let base = if accented {
            *self.accents.base.get(low)?
        } else {
            code
        };
        let cell = if accented && base == b' ' {
            match *self.accents.index.get(low)? {
                n if n >= 1 => *record.accent_cell.get(n as usize - 1)?,
                _ => b' ',
            }
        } else if !accented && code == b'|' {
            return record.rects.get(32).copied();
        } else {
            base
        };
        record
            .rects
            .get(usize::from(cell.checked_sub(FIRST_CHAR)?))
            .copied()
    }

    /// Glyph table index of `code` for adjust and kerning (0x3FAA0): 0xC7 and 0xE7 map to 'C' and 'c', '|' to the space,
    /// accented bytes to their base character. None beyond the 96 entries.
    fn glyph_index(&self, code: u8) -> Option<usize> {
        let index = match code {
            0xC7 => 0x23,
            0xE7 => 0x43,
            b'|' => 0,
            c if c & 0x80 != 0 => (*self.accents.base.get(usize::from(c & 0x7F))? as i8 as i32
                - 0x20)
                .try_into()
                .ok()?,
            c => usize::from(c.checked_sub(FIRST_CHAR)?),
        };
        (index < GLYPHS).then_some(index)
    }

    /// Unscaled advance of `code` followed by `next` (0x3FAA0 plus the rect width): object adjust, accent adjust
    /// (truncated toward zero), glyph adjust, kerning with the next character (none before 0, '\n' or a code without a glyph).
    pub fn original_advance(
        &self,
        object: &FontObject,
        record: &FontRecord,
        code: u8,
        next: Option<u8>,
    ) -> i32 {
        let width = self.glyph_rect(record, code).map_or(0, |r| i32::from(r.w));
        let mut adjust = object.advance_adjust;
        if code & 0x80 != 0 {
            let n = self
                .accents
                .index
                .get(usize::from(code & 0x7F))
                .copied()
                .unwrap_or(0);
            if let Some(extra) = usize::try_from(n)
                .ok()
                .and_then(|n| record.accent_adjust.get(n))
            {
                adjust = (adjust as f32 + extra) as i32;
            }
        }
        let Some(index) = self.glyph_index(code) else {
            return width + adjust;
        };
        let glyph = record.glyphs.get(index).copied().unwrap_or_default();
        adjust += glyph.adjust;
        let next_index = next.filter(|&n| n != 0 && n != b'\n').and_then(|n| {
            let base = if n & 0x80 != 0 {
                *self.accents.base.get(usize::from(n & 0x7F))? as i8 as i32
            } else {
                i32::from(n)
            };
            usize::try_from(base - 0x20).ok().filter(|&i| i < GLYPHS)
        });
        if let Some(next_index) = next_index {
            let next = record.glyphs.get(next_index).copied().unwrap_or_default();
            let cell = glyph.kern_column as i64 * KERN_SIZE as i64 + next.kern_row as i64;
            if (0..(KERN_SIZE * KERN_SIZE) as i64).contains(&cell) {
                adjust += record.kerning.get(cell as usize).copied().unwrap_or(0);
            }
        }
        width + adjust
    }

    fn resolve(&self, font: u32) -> Option<(&FontObject, &FontRecord)> {
        let object = self.objects.get(font as usize)?;
        Some((object, self.records.get(&object.record)?))
    }
}

/// Text measure from the font tables. Heights and widths in grid units (percent of the screen).
pub struct FontTableMeasure<'a> {
    pub fonts: &'a FontSet,
    pub strings: &'a dyn StringSource,
    /// Grid units per pixel (design space 640 x 480: x 1/6.4, y 1/4.8).
    pub grid_x: f32,
    pub grid_y: f32,
    /// CALIBRATED: line height in pixels per unit of object `scale_y`, per record.
    pub line_px_per_scale: BTreeMap<u32, f32>,
    /// Calibrated heights (grid units) that win over the table, e.g. `TableMeasure` values.
    pub overrides: BTreeMap<u32, f32>,
}

impl<'a> FontTableMeasure<'a> {
    pub fn new(fonts: &'a FontSet, strings: &'a dyn StringSource) -> Self {
        Self {
            fonts,
            strings,
            grid_x: 1.0 / 6.4,
            grid_y: 1.0 / 4.8,
            line_px_per_scale: BTreeMap::new(),
            overrides: BTreeMap::new(),
        }
    }

    /// Derives `line_px_per_scale` of the record of `font` from one observed line height (grid
    /// units). Returns the factor in pixels per unit of scale_y.
    pub fn calibrate(&mut self, font: u32, observed_height: f32) -> Option<f32> {
        let (object, _) = self.fonts.resolve(font)?;
        if object.scale_y <= 0.0 {
            return None;
        }
        let factor = observed_height / self.grid_y / object.scale_y;
        self.line_px_per_scale.insert(object.record, factor);
        Some(factor)
    }

    /// Advance of `text` in pixels (INFERRED formula, see the module doc). Control codes (below
    /// 0x20) take no width.
    pub fn width_px(&self, font: u32, text: &[u8]) -> f32 {
        self.pen_positions_px(font, text)
            .last()
            .map_or(0.0, |&(_, pen, step)| pen + step)
    }

    /// Per drawn character: `(code, pen x before it, advance)` in pixels, same INFERRED formula
    /// as `width_px` (which is the pen after the last character). Control codes are skipped.
    /// Empty for an unknown font.
    pub fn pen_positions_px(&self, font: u32, text: &[u8]) -> Vec<(u8, f32, f32)> {
        let Some((object, record)) = self.fonts.resolve(font) else {
            return Vec::new();
        };
        if !record.rects.is_empty() {
            return self.original_pens(object, record, text);
        }
        let base = record.cell_width * object.scale_x;
        let mut out = Vec::with_capacity(text.len());
        let mut pen = 0.0f32;
        let mut previous: Option<Glyph> = None;
        let mut operand = false;
        for &code in text {
            // `0x0B` escapes take their operand byte with them (the picture is laid out by `inline_layout`).
            if std::mem::take(&mut operand) {
                continue;
            }
            if code < FIRST_CHAR {
                operand = code == PICTURE_ESCAPE;
                continue;
            }
            let glyph = record.glyph(code);
            let mut step = base + glyph.adjust as f32 + object.advance_adjust as f32;
            if let Some(previous) = previous {
                step += record.kern(previous, glyph) as f32;
            }
            let step = step.max(0.0);
            out.push((code, pen, step));
            pen += step;
            previous = Some(glyph);
        }
        out
    }

    /// Fixed-mode glyph offsets and measured width are deliberately distinct: digit1 draws
    /// at its bearing inside the cell, but their bearing does not increase the cell advance.
    pub fn fixed_layout(&self, font: u32, text: &[u8]) -> Option<(Vec<(u8, f32, f32)>, f32)> {
        let (object, record) = self.fonts.resolve(font)?;
        let metrics = record.fixed_spacing?;
        let mut pen = 0.0;
        let mut glyphs = Vec::new();
        let mut operand = false;
        for &code in text {
            if std::mem::take(&mut operand) {
                continue;
            }
            if code < FIRST_CHAR {
                operand = code == PICTURE_ESCAPE;
                continue;
            }
            let step = metrics.advance(code) * object.scale_x;
            glyphs.push((code, pen + metrics.offset(code) * object.scale_x, step));
            pen += step;
        }
        Some((glyphs, pen))
    }

    /// Slice 16: per-glyph advances from the glyph rects (module doc). Control codes are skipped.
    fn original_pens(
        &self,
        object: &FontObject,
        record: &FontRecord,
        text: &[u8],
    ) -> Vec<(u8, f32, f32)> {
        let mut out = Vec::with_capacity(text.len());
        let mut pen = 0.0f32;
        let mut operand = false;
        for (i, &code) in text.iter().enumerate() {
            if std::mem::take(&mut operand) {
                continue;
            }
            if code < FIRST_CHAR {
                operand = code == PICTURE_ESCAPE;
                continue;
            }
            let next = text.get(i + 1).copied();
            let step = (self.fonts.original_advance(object, record, code, next) as f32
                * object.scale_x)
                .max(0.0);
            out.push((code, pen, step));
            pen += step;
        }
        out
    }

    /// T2316: glyph pens and inline pictures of `text` (`0x40FA0` / `0x3FCF0`). A picture escape (0x0B and '1'..'<')
    /// advances the pen by `picture_size` (design pixels, the style's text height); see [`inline_layout`].
    pub fn inline_layout(&self, font: u32, text: &[u8], picture_size: f32) -> InlineLayout {
        let mut layout = InlineLayout::default();
        let mut pen = 0.0f32;
        for segment in split_pictures(text) {
            match segment {
                Segment::Text(bytes) => {
                    let pens = self.pen_positions_px(font, bytes);
                    let width = pens.last().map_or(0.0, |&(_, p, step)| p + step);
                    layout.glyphs.extend(
                        pens.into_iter()
                            .map(|(code, p, step)| (code, pen + p, step)),
                    );
                    pen += width;
                }
                Segment::Picture(slot) => {
                    layout.pictures.push(InlinePicture {
                        slot,
                        pen,
                        size: picture_size,
                    });
                    pen += picture_size;
                }
                Segment::Skipped => {}
            }
        }
        layout.width = pen;
        layout
    }

    /// Bytes of a `TextRef::Id` from the string source; other refs have none here.
    pub fn text_bytes(&self, text: TextRef) -> Option<&[u8]> {
        self.string(text)
    }

    fn string(&self, text: TextRef) -> Option<&[u8]> {
        match text {
            TextRef::Id(id) => self.strings.bytes(id),
            _ => None,
        }
    }
}

impl TextMeasure for FontTableMeasure<'_> {
    fn line_height(&self, font: u32) -> f32 {
        if let Some(height) = self.overrides.get(&font) {
            return *height;
        }
        let Some((object, record)) = self.fonts.resolve(font) else {
            return 0.0;
        };
        let px = match self.line_px_per_scale.get(&object.record) {
            Some(factor) => factor * object.scale_y,
            None => record.cell_height * object.scale_y,
        };
        px * self.grid_y
    }

    fn text_width(&self, font: u32, text: TextRef) -> f32 {
        self.string(text)
            .map_or(0.0, |bytes| self.width_px(font, bytes) * self.grid_x)
    }

    /// Greedy word wrap on spaces (original wrapper 0x40C10 breaks at tokens; line break codes
    /// are not modelled). An unknown string is one line.
    fn lines(&self, font: u32, text: TextRef, max_width: f32) -> u32 {
        let Some(bytes) = self.string(text) else {
            return 1;
        };
        if max_width <= 0.0 {
            return 1;
        }
        let limit = max_width / self.grid_x;
        let space = self.width_px(font, b" ");
        let mut lines = 1u32;
        let mut used = 0.0f32;
        for word in bytes.split(|&b| b == b' ').filter(|w| !w.is_empty()) {
            let width = self.width_px(font, word);
            if used > 0.0 && used + space + width > limit {
                lines += 1;
                used = width;
            } else {
                used += if used > 0.0 { space } else { 0.0 } + width;
            }
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic image with the real structure and made-up numbers.
    struct Synthetic(BTreeMap<u32, u8>);
    impl Synthetic {
        fn put(&mut self, at: u32, value: u32) {
            for (i, b) in value.to_le_bytes().iter().enumerate() {
                self.0.insert(at + i as u32, *b);
            }
        }
    }
    impl StaticImage for Synthetic {
        fn byte(&self, address: u32) -> Option<u8> {
            self.0.get(&address).copied()
        }
    }
    struct Strings(Vec<Vec<u8>>);
    impl StringSource for Strings {
        fn bytes(&self, id: TextId) -> Option<&[u8]> {
            self.0.get(id.0 as usize).map(|v| v.as_slice())
        }
    }

    fn image() -> Synthetic {
        let mut img = Synthetic(BTreeMap::new());
        let (glyphs, kern) = (0x900000u32, 0x901000u32);
        let rec = RECORD_ADDRESS;
        img.put(rec + 0x10, glyphs);
        img.put(rec + 0x14, kern);
        img.put(rec + 0x18, 20.0f32.to_bits());
        img.put(rec + 0x1C, 10.0f32.to_bits());
        for i in 0..96u32 {
            img.put(glyphs + i * 12, i % 3);
            img.put(glyphs + i * 12 + 4, i % 2);
            img.put(glyphs + i * 12 + 8, 0);
        }
        for i in 0..169u32 {
            img.put(kern + i * 4, 0);
        }
        img.put(kern + (1 * 13 + 1) * 4, (-2i32) as u32); // column 1 followed by row 1
        for index in 0..2u32 {
            let o = OBJECT_ADDRESS + index * OBJECT_STRIDE;
            img.put(o, 0);
            img.put(o + 8, 0.5f32.to_bits());
            img.put(o + 12, (0.5f32 + index as f32 * 0.5).to_bits());
            img.put(o + 16, (-1i32) as u32);
        }
        img
    }

    #[test]
    fn loads_structure_and_skips_unused_records() {
        let set = FontSet::load(&image());
        assert_eq!(set.records.len(), 1);
        assert_eq!(set.objects.len(), 2);
        let record = &set.records[&0];
        assert_eq!((record.glyphs.len(), record.kerning.len()), (96, 169));
    }

    #[test]
    fn width_uses_scale_adjusts_and_kerning() {
        let set = FontSet::load(&image());
        let strings = Strings(vec![]);
        let measure = FontTableMeasure::new(&set, &strings);
        // base = 20 * 0.5 = 10, object adjust -1: 9 per glyph. 'A'=0x41 -> index 33: row 0, col 1;
        // 'B' index 34: row 1, col 0. A then B: kern[1][1] = -2.
        assert_eq!(measure.width_px(0, b"A"), 9.0);
        assert_eq!(measure.width_px(0, b"AB"), 9.0 + 9.0 - 2.0);
        assert_eq!(
            measure.width_px(0, b"\x01A"),
            9.0,
            "control codes take no width"
        );
        assert_eq!(measure.width_px(5, b"A"), 0.0, "unknown font");
    }

    #[test]
    fn picture_escapes_take_their_operand_and_advance_by_the_picture() {
        // Tier 0, FABRICATED fonts (9 per glyph); the escape layout is MEASURED from `0x40FA0`.
        let set = FontSet::load(&image());
        let strings = Strings(vec![]);
        let measure = FontTableMeasure::new(&set, &strings);
        assert_eq!(
            measure.width_px(0, b"\x0b2A"),
            9.0,
            "the operand is not a glyph"
        );
        let layout = measure.inline_layout(0, b"A\x0b2A\x0bLA\x0b", 14.0);
        let codes: Vec<u8> = layout.glyphs.iter().map(|g| g.0).collect();
        assert_eq!(codes, b"AAA");
        assert_eq!(
            layout.pictures,
            vec![InlinePicture {
                slot: 1,
                pen: 9.0,
                size: 14.0
            }]
        );
        assert_eq!(layout.glyphs[1].1, 23.0, "the pen moves past the picture");
        assert_eq!(
            layout.width,
            9.0 + 14.0 + 9.0 + 9.0,
            "the letter icon and the trailing escape take no width here"
        );
        assert_eq!(
            split_pictures(b"\x0b<\x0b="),
            vec![Segment::Picture(11), Segment::Skipped]
        );
    }

    /// `image()` plus glyph rects (w = 10 + index % 4), accent tables and two accent records.
    fn image_with_rects() -> Synthetic {
        let mut img = image();
        let (rects, accents) = (0x902000u32, 0x903000u32);
        img.put(RECORD_ADDRESS, 964);
        img.put(RECORD_ADDRESS + 0x8, rects);
        img.put(RECORD_ADDRESS + 0xC, accents);
        for i in 0..RECTS as u32 {
            img.put(rects + i * 12, (i * 3) | (7 << 16));
            img.put(rects + i * 12 + 4, (10 + i % 4) | (20 << 16));
            img.put(rects + i * 12 + 8, 2.5f32.to_bits());
        }
        for i in 0..128u32 {
            img.0.insert(ACCENT_BASE_ADDRESS + i, 0);
            img.0.insert(ACCENT_INDEX_ADDRESS + i, 0);
        }
        // 0xC1 -> 'A' with accent 2, 0xA0 -> accented space with accent 1 (cell of 'B').
        img.0.insert(ACCENT_BASE_ADDRESS + 0x41, b'A');
        img.0.insert(ACCENT_INDEX_ADDRESS + 0x41, 2);
        img.0.insert(ACCENT_BASE_ADDRESS + 0x20, b' ');
        img.0.insert(ACCENT_INDEX_ADDRESS + 0x20, 1);
        img.put(accents - 4, 0.0f32.to_bits());
        img.0.insert(accents, b'B');
        img.0.insert(accents + 0x28, b'C');
        img.put(accents + 0x24, 0.0f32.to_bits());
        img.put(accents + 0x28 + 0x24, 3.7f32.to_bits());
        img
    }

    #[test]
    fn rects_give_per_glyph_advances() {
        let set = FontSet::load(&image_with_rects());
        let record = &set.records[&0];
        assert_eq!((record.resource_id, record.rects.len()), (964, RECTS));
        assert_eq!(set.accents.index.len(), 128, "{:?}", set.accents);
        assert_eq!(record.accent_adjust.len(), 3);
        let strings = Strings(vec![]);
        let measure = FontTableMeasure::new(&set, &strings);
        // 'A' index 33: w = 10 + 1 = 11, object adjust -1, scale 0.5 -> 5. 'B' index 34: w 12 -> 5.5.
        assert_eq!(measure.width_px(0, b"A"), 5.0);
        assert_eq!(measure.width_px(0, b"B"), 5.5);
        // A then B: kerning -2 belongs to the A step: (11 - 1 - 2) * 0.5 = 4.
        let pens = measure.pen_positions_px(0, b"AB");
        assert_eq!(pens, vec![(b'A', 0.0, 4.0), (b'B', 4.0, 5.5)]);
        // 0xC1: cell and glyph of 'A', accent adjust 3.7 truncated: (11 + trunc(-1 + 3.7)) * 0.5 = 6.5.
        assert_eq!(measure.width_px(0, &[0xC1]), 6.5);
        // Accented space: cell of 'B' from accent record 1.
        let b = set.glyph_rect(record, b'B').unwrap();
        assert_eq!(set.glyph_rect(record, 0xA0), Some(b));
        assert_eq!(set.glyph_rect(record, b'|'), Some(record.rects[32]));
        assert_eq!(set.glyph_rect(record, 0x1F), None);
        assert_eq!((b.x, b.y, b.h, b.top), (34 * 3, 7, 20, 2.5));
    }

    #[test]
    fn line_height_calibration_and_fallback() {
        let set = FontSet::load(&image());
        let strings = Strings(vec![]);
        let mut measure = FontTableMeasure::new(&set, &strings);
        // fallback: cell height * scale_y = 10 * 0.5 px = 5 px -> 5 / 4.8 grid units.
        assert!((measure.line_height(0) - 5.0 / 4.8).abs() < 1e-5);
        let factor = measure.calibrate(1, 4.0).unwrap();
        assert!(
            (measure.line_height(1) - 4.0).abs() < 1e-4,
            "calibrated font reproduces itself"
        );
        assert!(factor > 0.0);
        measure.overrides.insert(0, 3.5);
        assert_eq!(measure.line_height(0), 3.5);
    }

    #[test]
    fn wraps_on_spaces() {
        let set = FontSet::load(&image());
        let strings = Strings(vec![b"CCC CCC CCC".to_vec(), b"".to_vec()]);
        let measure = FontTableMeasure::new(&set, &strings);
        let word = measure.text_width(0, TextRef::Id(TextId(0)));
        assert!(word > 0.0);
        // each word is 3 glyphs; width of a box fitting two words but not three
        let one = measure.width_px(0, b"CCC") / 6.4;
        let space = measure.width_px(0, b" ") / 6.4;
        let two = one * 2.0 + space + 0.01;
        assert_eq!(measure.lines(0, TextRef::Id(TextId(0)), two), 2);
        assert_eq!(measure.lines(0, TextRef::Id(TextId(0)), 1000.0), 1);
        assert_eq!(measure.lines(0, TextRef::Id(TextId(0)), 0.5), 3);
        assert_eq!(measure.lines(0, TextRef::Id(TextId(1)), 10.0), 1);
        assert_eq!(measure.lines(0, TextRef::Dynamic, 10.0), 1);
    }
}
