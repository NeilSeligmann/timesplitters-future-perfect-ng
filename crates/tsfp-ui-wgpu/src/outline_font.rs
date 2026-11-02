//! Outline fonts of the opt-in `improved` preset (T2307 font follow-up), feature `outline-fonts`.
//!
//! The `original` preset draws the disc's bitmap atlases (`original_font`). This module draws the
//! same rows from TrueType/OpenType outlines rasterised by `ab_glyph` at window resolution, so
//! text stays crisp at any window size. Layout is unchanged: the shaper still uses the original
//! font tables (advances, kerning, cell rects), and each outline glyph is fitted into the
//! original glyph's ink box: vertical scale from the original cap height and horizontal scale from
//! the original `H` ink width (ink of `H`), baseline on
//! the original baseline (bottom of the `H` ink), centred on the original glyph's ink centre.
//!
//! Font files (never committed, AGENTS.md rule 5): per original atlas, the owner's own file
//! (`impact.ttf|otf`, `arialbold.ttf|otf` in `TSFP_UI_FONT_DIR`, e.g. from ttf-mscorefonts-installer)
//! is preferred, else the open stand-in fetched by `tools/dev/fetch_fonts.sh`
//! (`impact-openalt.ttf` = Anton, SIL OFL; `arialbold-openalt.ttf` = Liberation Sans Bold, SIL OFL).
//! Record to atlas: record 0 = `impact`, record 1 = `arialbold` (texture table 964/965,
//! docs/t2307-ui-visual-fidelity.md; checked visually, the condensed face lands on the menu rows).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ab_glyph::{Font, FontVec, PxScale, ScaleFont, VariableFont};
use tsfp_ui_renderer::{AtlasGlyph, GlyphKey, TextureId, UvRect, Vec2};

use crate::original_font::{OriginalFont, Page};
use crate::skin_assets::ORIGINAL_FONT_NAMES;
use crate::{Error, WgpuUiRenderer};

/// Texture id of the rasterised outline page (distinct from the original pages and stand-in).
pub const OUTLINE_PAGE: TextureId = TextureId(0x5157_2000);
/// A texel of the original page counts as ink from this coverage on.
const INK: u8 = 96;
/// Default directory of `tools/dev/fetch_fonts.sh` (relative to the working directory).
pub const DEFAULT_FONT_DIR: &str = "tmp/ui-fonts";

/// Which file a record's outline face came from.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceSource {
    pub atlas: &'static str,
    pub path: PathBuf,
    /// "owner" for the real face, "open" for the OFL stand-in.
    pub kind: &'static str,
}

/// Finds the outline file of each original atlas in `dir`: the owner's own face first, then the
/// open stand-in. Files without a TrueType/OpenType magic are skipped.
pub fn find_faces(dir: &Path) -> Vec<FaceSource> {
    let mut out = Vec::new();
    for atlas in ORIGINAL_FONT_NAMES {
        let candidates = [
            (format!("{atlas}.ttf"), "owner"),
            (format!("{atlas}.otf"), "owner"),
            (format!("{atlas}.TTF"), "owner"),
            (format!("{atlas}-openalt.ttf"), "open"),
            (format!("{atlas}-openalt.otf"), "open"),
        ];
        for (name, kind) in candidates {
            let path = dir.join(name);
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            if matches!(
                bytes.get(0..4),
                Some([0, 1, 0, 0]) | Some(b"true") | Some(b"OTTO")
            ) {
                out.push(FaceSource { atlas, path, kind });
                break;
            }
        }
    }
    out
}

/// `TSFP_UI_FONTS=outline` selects the outline fonts (default: the original bitmap atlases).
pub fn selected_from_env() -> bool {
    std::env::var("TSFP_UI_FONTS").is_ok_and(|v| v.eq_ignore_ascii_case("outline"))
}

/// `TSFP_UI_FONT_DIR`, else [`DEFAULT_FONT_DIR`].
pub fn font_dir_from_env() -> PathBuf {
    std::env::var_os("TSFP_UI_FONT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_FONT_DIR))
}

/// Ink box of one original glyph cell, in texels relative to the cell's top-left.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Ink {
    left: f32,
    right: f32,
    bottom: f32,
    height: f32,
}

fn ink_box(page: &Page, x: u16, y: u16, w: u16, h: u16) -> Option<Ink> {
    let (mut l, mut r, mut t, mut b) = (u16::MAX, 0u16, u16::MAX, 0u16);
    for row in 0..h {
        for col in 0..w {
            let (px, py) = (u32::from(x + col), u32::from(y + row));
            if px >= page.width || py >= page.height {
                continue;
            }
            if page.coverage[(py * page.width + px) as usize] >= INK {
                (l, r, t, b) = (l.min(col), r.max(col + 1), t.min(row), b.max(row + 1));
            }
        }
    }
    (l < r).then(|| Ink {
        left: f32::from(l),
        right: f32::from(r),
        bottom: f32::from(b),
        height: f32::from(b - t),
    })
}

/// Outline glyphs placed in the original glyph boxes.
pub struct OutlineFont {
    pub original: OriginalFont,
    /// Record index -> face.
    faces: BTreeMap<u32, (FontVec, FaceSource)>,
}

impl OutlineFont {
    /// Loads the faces of `sources` for the records of `original` (record 0 `impact`, 1 `arialbold`).
    /// None when no record has a readable face.
    pub fn load(original: OriginalFont, sources: &[FaceSource]) -> Option<Self> {
        let mut faces = BTreeMap::new();
        for (record, atlas) in ORIGINAL_FONT_NAMES.iter().enumerate() {
            let Some(source) = sources.iter().find(|s| s.atlas == *atlas) else {
                continue;
            };
            let Ok(bytes) = std::fs::read(&source.path) else {
                continue;
            };
            if let Ok(mut face) = FontVec::try_from_vec(bytes) {
                // Variable faces: heaviest weight (Impact and Arial Bold are heavy faces).
                for axis in face.variations() {
                    if &axis.tag == b"wght" {
                        face.set_variation(b"wght", axis.max_value);
                    }
                }
                faces.insert(record as u32, (face, source.clone()));
            }
        }
        (!faces.is_empty()).then_some(Self { original, faces })
    }

    /// One line per face: atlas, kind and file.
    pub fn describe(&self) -> Vec<String> {
        self.faces
            .values()
            .map(|(_, s)| format!("{} <- {} ({})", s.atlas, s.path.display(), s.kind))
            .collect()
    }

    /// The face and original record of `key`'s font object.
    fn face(&self, key: GlyphKey) -> Option<(&FontVec, u32)> {
        let (object, _) = self.original.fonts.font(crate::original_font::split_font_id(key.font).0)?;
        self.faces
            .get(&object.record)
            .map(|(f, _)| (f, object.record))
    }

    pub fn covers(&self, key: GlyphKey) -> bool {
        match self.face(key) {
            Some((face, _)) => {
                let c = char::from(u8::try_from(key.glyph.0).unwrap_or(0));
                c == ' ' || face.glyph_id(c).0 != 0
            }
            None => self.original.glyph(key).is_some(),
        }
    }

    /// Ink of the original glyph `code` of `record` (None without a page, cell or ink).
    fn original_ink(&self, record: u32, code: u8) -> Option<Ink> {
        let page = self.original.pages.get(&record)?;
        let rec = self.original.fonts.records.get(&record)?;
        let rect = self.original.fonts.glyph_rect(rec, code)?;
        ink_box(page, rect.x, rect.y, rect.w, rect.h)
    }

    /// Rasterises every key (outline where a face exists, else the original cell), packs one
    /// coverage page and registers the glyphs. Keys that could not be drawn are returned.
    pub fn register(
        &self,
        renderer: &mut WgpuUiRenderer<'_>,
        keys: &[GlyphKey],
    ) -> Result<Vec<GlyphKey>, Error> {
        let mut fallback = Vec::new();
        let mut bitmaps: Vec<(GlyphKey, u32, u32, Vec<u8>, Vec2, f32)> = Vec::new();
        let mut missing = Vec::new();
        for &key in keys {
            let Some((face, record)) = self.face(key) else {
                fallback.push(key);
                continue;
            };
            let Some(o) = self.original.glyph(key) else {
                missing.push(key);
                continue;
            };
            match self.raster(face, record, key, o) {
                Some((w, h, cov, bearing)) => bitmaps.push((key, w, h, cov, bearing, o.advance_px)),
                // Blank glyph (space): draw nothing, keep the original advance.
                None => bitmaps.push((key, 1, 1, vec![0], o.bearing_px, o.advance_px)),
            }
        }
        // Shelf packing, 1 texel gutter.
        let width = bitmaps.iter().map(|b| b.1 + 1).max().unwrap_or(1).max(512);
        let (mut x, mut y, mut shelf) = (0u32, 0u32, 0u32);
        let mut placed = Vec::with_capacity(bitmaps.len());
        for b in &bitmaps {
            if x + b.1 > width {
                (x, y, shelf) = (0, y + shelf + 1, 0);
            }
            placed.push((x, y));
            x += b.1 + 1;
            shelf = shelf.max(b.2);
        }
        let height = (y + shelf).max(1);
        let mut coverage = vec![0u8; (width * height) as usize];
        for (b, &(px, py)) in bitmaps.iter().zip(&placed) {
            for row in 0..b.2 {
                let dst = ((py + row) * width + px) as usize;
                let src = (row * b.1) as usize;
                coverage[dst..dst + b.1 as usize].copy_from_slice(&b.3[src..src + b.1 as usize]);
            }
        }
        renderer.evict_page(OUTLINE_PAGE)?;
        renderer.upload_coverage(OUTLINE_PAGE, width, height, &coverage)?;
        let (fw, fh) = (width as f32, height as f32);
        let entries: Vec<(GlyphKey, AtlasGlyph)> = bitmaps
            .iter()
            .zip(&placed)
            .map(|(b, &(px, py))| {
                let (x0, y0) = (px as f32, py as f32);
                let blank = b.3.len() == 1 && b.3[0] == 0;
                (
                    b.0,
                    AtlasGlyph {
                        texture: OUTLINE_PAGE,
                        uv: UvRect {
                            min: Vec2::new(x0 / fw, y0 / fh),
                            max: Vec2::new((x0 + b.1 as f32) / fw, (y0 + b.2 as f32) / fh),
                        },
                        size_px: if blank {
                            Vec2::new(0.0, 0.0)
                        } else {
                            Vec2::new(b.1 as f32, b.2 as f32)
                        },
                        bearing_px: b.4,
                        advance_px: b.5,
                    },
                )
            })
            .collect();
        renderer.register_glyphs(&entries)?;
        if !fallback.is_empty() {
            missing.extend(self.original.register(renderer, &fallback)?);
        }
        Ok(missing)
    }

    /// Coverage bitmap (w, h, rows) and bearing of the outline glyph fitted into the original box.
    fn raster(
        &self,
        face: &FontVec,
        record: u32,
        key: GlyphKey,
        o: AtlasGlyph,
    ) -> Option<(u32, u32, Vec<u8>, Vec2)> {
        let code = u8::try_from(key.glyph.0).ok()?;
        let cap = self.original_ink(record, b'H')?;
        let rec = self.original.fonts.records.get(&record)?;
        let cell = self.original.fonts.glyph_rect(rec, code)?;
        // Window pixels per texel of this glyph (the original quad's scale).
        let sx = o.size_px.x / f32::from(cell.w.max(1));
        let sy = o.size_px.y / f32::from(cell.h.max(1));
        // Uniform px scale: outline cap height == original cap height in window pixels.
        let probe = face.as_scaled(PxScale::from(100.0));
        let h_outline = probe.outline_glyph(probe.scaled_glyph('H'))?;
        let h_bounds = h_outline.px_bounds();
        // Cap height sets the vertical scale, the `H` ink width the horizontal one, so the face
        // takes the original atlas's proportions (the disc font is a widened Impact).
        let scale = PxScale {
            x: 100.0 * ((cap.right - cap.left) * sx) / h_bounds.width(),
            y: 100.0 * (cap.height * sy) / h_bounds.height(),
        };
        let scaled = face.as_scaled(scale);
        let glyph = scaled.scaled_glyph(char::from(code));
        let outlined = scaled.outline_glyph(glyph)?;
        let bounds = outlined.px_bounds();
        let (w, h) = (bounds.width() as u32, bounds.height() as u32);
        if w == 0 || h == 0 {
            return None;
        }
        let mut cov = vec![0u8; (w * h) as usize];
        outlined.draw(|gx, gy, c| {
            if gx < w && gy < h {
                cov[(gy * w + gx) as usize] = (c.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        });
        // Original baseline (bottom of the H ink) relative to the pen baseline.
        let h_glyph = self.original.glyph(GlyphKey {
            glyph: tsfp_ui_renderer::GlyphId(u32::from(b'H')),
            ..key
        })?;
        let baseline = h_glyph.bearing_px.y + cap.bottom * sy;
        // Horizontal: centre on the original ink when the glyph has ink, else on the cell.
        let centre = match self.original_ink(record, code) {
            Some(ink) => o.bearing_px.x + (ink.left + ink.right) * 0.5 * sx,
            None => o.bearing_px.x + o.size_px.x * 0.5,
        };
        let bearing = Vec2::new(
            (centre - bounds.width() * 0.5).round(),
            (baseline + bounds.min.y).round(),
        );
        Some((w, h, cov, bearing))
    }
}

#[cfg(test)]
mod tests {
    //! Tier 0: FABRICATED page; the face is the system Liberation Sans Bold when installed.
    use super::*;

    #[test]
    fn ink_box_finds_the_lit_texels() {
        let mut coverage = vec![0u8; 16 * 16];
        for (x, y) in [(3, 2), (5, 7), (4, 4)] {
            coverage[y * 16 + x] = 255;
        }
        let page = Page {
            width: 16,
            height: 16,
            coverage,
        };
        let ink = ink_box(&page, 1, 1, 10, 10).unwrap();
        assert_eq!(
            ink,
            Ink {
                left: 2.0,
                right: 5.0,
                bottom: 7.0,
                height: 6.0
            }
        );
        assert!(ink_box(&page, 8, 8, 4, 4).is_none());
    }

    #[test]
    fn owner_face_wins_over_the_open_stand_in() {
        let dir = std::env::temp_dir().join(format!("t2307-faces-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("impact-openalt.ttf"), [0u8, 1, 0, 0, 9]).unwrap();
        std::fs::write(dir.join("arialbold-openalt.ttf"), b"OTTOx").unwrap();
        std::fs::write(dir.join("arialbold.ttf"), b"nope").unwrap();
        let faces = find_faces(&dir);
        assert_eq!(faces.len(), 2);
        assert_eq!((faces[0].atlas, faces[0].kind), ("impact", "open"));
        assert_eq!(
            (faces[1].atlas, faces[1].kind),
            ("arialbold", "open"),
            "bad magic skipped"
        );
        std::fs::write(dir.join("impact.ttf"), b"true....").unwrap();
        assert_eq!(find_faces(&dir)[0].kind, "owner");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
