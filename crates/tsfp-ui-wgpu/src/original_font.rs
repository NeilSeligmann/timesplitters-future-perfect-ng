//! Original font atlas for UI text (T2189 UI-6 slice 16), decoded from the owner's disc at run time.
//!
//! MEASURED on the owner disc: the two font textures (records 0 and 1, resources 964 and 965) are A8 XBT members of
//! `pak/arcade/l_103.pak` (256 x 256, format enum 5, Morton swizzled, one level). Their member names are not recovered (the
//! `textures/%06x.xbt` form does not hash to any key), so each record is matched to the page by content: the share of lit
//! texels inside the record's glyph rects ([`rect_coverage`], 1.0 for the right page). Nothing is cached or written; no font
//! bytes are tracked.
//!
//! Glyph quads: the cell rect at `scale_x` x `scale_y` of the font object times the window scale, its top `rect.top` below
//! the line top (INFERRED from 0x3FCF0, which adds `top` to the cell height when measuring). Accent marks of bytes >= 0x80 are
//! not composited (debt): such bytes draw their base character's cell.

use std::collections::BTreeMap;

use tsfp_ui_model::font::{FontRecord, FontSet};
use tsfp_ui_renderer::{AtlasGlyph, GlyphKey, TextureId, UvRect, Vec2};

use crate::{Error, WgpuUiRenderer};

/// Texture ids of the pages: base + record index (high, never a loader id, distinct from the stand-in page).
pub const PAGE_BASE: u64 = 0x5157_1000;
/// XBT format enum of an A8 texture (T1761 jump table 0x185B0).
pub const FORMAT_A8: u32 = 5;
/// Smallest rect coverage accepted as a match.
pub const MATCH_FLOOR: f32 = 0.98;
/// A texel counts as lit from this coverage on.
const LIT: u8 = 128;

/// One decoded A8 page, row-major.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    pub width: u32,
    pub height: u32,
    pub coverage: Vec<u8>,
}

/// Decodes an A8 XBT (128-byte header: width, height, ..., format at word 5) to a row-major page. None for other formats,
/// non power of two sizes or a short payload.
pub fn decode_a8(bytes: &[u8]) -> Option<Page> {
    let word = |i: usize| -> Option<u32> {
        Some(u32::from_le_bytes(
            bytes.get(i * 4..i * 4 + 4)?.try_into().ok()?,
        ))
    };
    let (width, height) = (word(0)?, word(1)?);
    if word(5)? != FORMAT_A8
        || !width.is_power_of_two()
        || !height.is_power_of_two()
        || width > 4096
        || height > 4096
    {
        return None;
    }
    let texels = (width * height) as usize;
    let pixels = bytes.get(128..128 + texels)?;
    Some(Page {
        width,
        height,
        coverage: tsfp_import::png::unswizzle(pixels, width as usize, height as usize, 1),
    })
}

/// Share of the page's lit texels that fall inside the record's rects (0 for an empty page or a rect off the page).
pub fn rect_coverage(record: &FontRecord, page: &Page) -> f32 {
    let mut inside = vec![false; page.coverage.len()];
    for rect in &record.rects {
        let (x1, y1) = (
            u32::from(rect.x) + u32::from(rect.w),
            u32::from(rect.y) + u32::from(rect.h),
        );
        if x1 > page.width || y1 > page.height {
            return 0.0;
        }
        for y in u32::from(rect.y)..y1 {
            for x in u32::from(rect.x)..x1 {
                inside[(y * page.width + x) as usize] = true;
            }
        }
    }
    let lit: Vec<bool> = page.coverage.iter().map(|&c| c >= LIT).collect();
    let total = lit.iter().filter(|&&l| l).count();
    if total == 0 {
        return 0.0;
    }
    let covered = lit.iter().zip(&inside).filter(|&(&l, &i)| l && i).count();
    covered as f32 / total as f32
}

/// The original glyph pixels of every record that matched a page.
pub struct OriginalFont {
    pub fonts: FontSet,
    /// Record index -> page.
    pub pages: BTreeMap<u32, Page>,
    /// Window pixels per design pixel (1.0 at 640 x 480).
    pub window_scale: Vec2,
}

/// A font object index with a glyph scale over the source's window scale (T2312: HUD strings the
/// original draws at their own text scale, e.g. a label fitted to its frame). The scale rides in
/// the high bits of the renderer's font id (x and y in thousandths; 0 = 1.0), so one glyph source
/// serves every scale.
pub fn scaled_font_id(font: u32, scale: [f32; 2]) -> tsfp_ui_renderer::FontId {
    tsfp_ui_renderer::FontId::scaled(font, scale)
}

/// The font object index and glyph scale of a renderer font id ([`scaled_font_id`]).
pub fn split_font_id(id: tsfp_ui_renderer::FontId) -> (u32, [f32; 2]) {
    id.source_and_scale()
}

impl OriginalFont {
    /// Matches each record with rects to its best candidate page (at least [`MATCH_FLOOR`]). Records without a match are
    /// left out; their glyphs report as missing.
    pub fn match_pages(fonts: FontSet, candidates: &[Page], window_scale: Vec2) -> Self {
        let mut pages = BTreeMap::new();
        for (&index, record) in &fonts.records {
            if record.rects.is_empty() {
                continue;
            }
            let best = candidates
                .iter()
                .map(|page| (rect_coverage(record, page), page))
                .filter(|(score, _)| *score >= MATCH_FLOOR)
                .max_by(|a, b| a.0.total_cmp(&b.0));
            if let Some((_, page)) = best {
                pages.insert(index, page.clone());
            }
        }
        Self {
            fonts,
            pages,
            window_scale,
        }
    }

    pub fn texture(record: u32) -> TextureId {
        TextureId(PAGE_BASE + u64::from(record))
    }

    /// Atlas entry for `key` (`font` = font object index, `glyph` = character code). The bearing puts the cell top `top`
    /// below the line top, the line top being one raster height above the baseline. None without a page or cell.
    pub fn glyph(&self, key: GlyphKey) -> Option<AtlasGlyph> {
        let code = u8::try_from(key.glyph.0).ok()?;
        let (font, [mx, my]) = split_font_id(key.font);
        let (object, record) = self.fonts.font(font)?;
        let page = self.pages.get(&object.record)?;
        let rect = self.fonts.glyph_rect(record, code)?;
        let (sx, sy) = (
            object.scale_x * self.window_scale.x * mx,
            object.scale_y * self.window_scale.y * my,
        );
        let (w, h) = (page.width as f32, page.height as f32);
        let (x, y) = (f32::from(rect.x), f32::from(rect.y));
        Some(AtlasGlyph {
            texture: Self::texture(object.record),
            uv: UvRect {
                min: Vec2::new(x / w, y / h),
                max: Vec2::new((x + f32::from(rect.w)) / w, (y + f32::from(rect.h)) / h),
            },
            size_px: Vec2::new(f32::from(rect.w) * sx, f32::from(rect.h) * sy),
            bearing_px: Vec2::new(0.0, rect.top * sy - key.raster_height_px as f32),
            advance_px: f32::from(rect.w) * sx,
        })
    }

    /// Uploads the pages the keys need (idempotent) and registers every key with a cell. Keys without one are returned.
    pub fn register(
        &self,
        renderer: &mut WgpuUiRenderer<'_>,
        keys: &[GlyphKey],
    ) -> Result<Vec<GlyphKey>, Error> {
        let mut entries = Vec::new();
        let mut missing = Vec::new();
        for &key in keys {
            match self.glyph(key) {
                Some(glyph) => entries.push((key, glyph)),
                None => missing.push(key),
            }
        }
        for (&record, page) in &self.pages {
            let id = Self::texture(record);
            if entries.iter().any(|(_, g)| g.texture == id) && !renderer.has_texture(id) {
                renderer.upload_coverage(id, page.width, page.height, &page.coverage)?;
            }
        }
        renderer.register_glyphs(&entries)?;
        Ok(missing)
    }
}

#[cfg(test)]
mod tests {
    //! Tier 0, FABRICATED pages and tables.
    use super::*;
    use tsfp_ui_model::font::{FontObject, GlyphRect, KERN_SIZE, RECTS};
    use tsfp_ui_renderer::{FontId, GlyphId};

    fn record(offset: u16) -> FontRecord {
        let rects = (0..RECTS as u16)
            .map(|i| GlyphRect {
                x: offset + (i % 8) * 8,
                y: (i / 8) * 8,
                w: 6,
                h: 7,
                top: 3.0,
            })
            .collect();
        FontRecord {
            cell_width: 10.0,
            cell_height: 20.0,
            glyphs: vec![Default::default(); 96],
            kerning: vec![0; KERN_SIZE * KERN_SIZE],
            rects,
            ..FontRecord::default()
        }
    }

    /// Lights the middle texel of every cell of `record`.
    fn page_for(record: &FontRecord) -> Page {
        let mut coverage = vec![0u8; 128 * 128];
        for rect in &record.rects {
            coverage[(usize::from(rect.y) + 3) * 128 + usize::from(rect.x) + 3] = 255;
        }
        Page {
            width: 128,
            height: 128,
            coverage,
        }
    }

    fn fonts() -> FontSet {
        FontSet {
            records: [(0, record(0)), (1, record(64))].into(),
            objects: vec![
                FontObject {
                    record: 0,
                    scale_x: 0.5,
                    scale_y: 0.5,
                    advance_adjust: 0,
                },
                FontObject {
                    record: 1,
                    scale_x: 1.0,
                    scale_y: 1.0,
                    advance_adjust: 0,
                },
            ],
            ..FontSet::default()
        }
    }

    #[test]
    fn decode_reads_a8_and_rejects_other_formats() {
        let mut xbt = vec![0u8; 128 + 16];
        for (i, word) in [4u32, 4, 4, 4, 0, FORMAT_A8].iter().enumerate() {
            xbt[i * 4..i * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        xbt[128 + 2] = 200; // Morton index 2 = (x 0, y 1)
        let page = decode_a8(&xbt).unwrap();
        assert_eq!((page.width, page.height), (4, 4));
        assert_eq!(page.coverage[4], 200, "unswizzled to row 1");
        xbt[20] = 6;
        assert!(decode_a8(&xbt).is_none(), "L8 is not a font page");
        xbt[20] = FORMAT_A8 as u8;
        assert!(decode_a8(&xbt[..130]).is_none(), "short payload");
    }

    #[test]
    fn each_record_gets_the_page_its_rects_cover() {
        let fonts = fonts();
        let (a, b) = (page_for(&fonts.records[&0]), page_for(&fonts.records[&1]));
        assert_eq!(rect_coverage(&fonts.records[&0], &a), 1.0);
        assert!(rect_coverage(&fonts.records[&0], &b) < MATCH_FLOOR);
        // Candidate order must not matter.
        let font = OriginalFont::match_pages(fonts, &[b.clone(), a.clone()], Vec2::new(1.0, 1.0));
        assert_eq!(font.pages.get(&0), Some(&a));
        assert_eq!(font.pages.get(&1), Some(&b));
    }

    #[test]
    fn glyph_uses_the_cell_scale_and_top_offset() {
        let fonts = fonts();
        let pages = [page_for(&fonts.records[&0]), page_for(&fonts.records[&1])];
        let font = OriginalFont::match_pages(fonts, &pages, Vec2::new(2.0, 2.0));
        let key = |font: u64, code: u8| GlyphKey {
            font: FontId(font),
            glyph: GlyphId(u32::from(code)),
            raster_height_px: 20,
        };
        // 'A' = cell 33: x (33 % 8) * 8 = 8, y 32. Font 0: scale 0.5 x window 2 = 1.
        let a = font.glyph(key(0, b'A')).unwrap();
        assert_eq!(a.texture, OriginalFont::texture(0));
        assert_eq!(a.uv.min, Vec2::new(8.0 / 128.0, 32.0 / 128.0));
        assert_eq!(a.size_px, Vec2::new(6.0, 7.0));
        assert_eq!(a.bearing_px, Vec2::new(0.0, 3.0 - 20.0));
        // Font 1 draws from the second page at scale 2.
        let b = font.glyph(key(1, b'A')).unwrap();
        assert_eq!(
            (b.texture, b.size_px),
            (OriginalFont::texture(1), Vec2::new(12.0, 14.0))
        );
        assert!(font.glyph(key(0, 0x1F)).is_none());
        assert!(font.glyph(key(7, b'A')).is_none(), "unknown font object");
    }
}
