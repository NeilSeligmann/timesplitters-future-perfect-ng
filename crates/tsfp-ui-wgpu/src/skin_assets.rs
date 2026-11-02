//! Skin textures of the original front end, read from the owner's disc at run time (T2307 UI-9).
//!
//! The texture table names (`tsfp_ui_model::skin::SkinTexture`) resolve to pak members keyed by
//! `crc32("assets/ts/textures/xbox/<name>.xbt")` (MEASURED: 822 table names resolve, the frame
//! textures live in the front-end level paks `pak/arcade/l_100.pak` .. `l_106.pak`). Members are
//! XBT: 128-byte header (width, height, ..., format enum at word 5), Morton-swizzled texels.
//! Nothing is cached or written; the decoded RGBA lives only in the renderer.
//!
//! `improved` preset hook: [`SkinOverrides::dir`] names an owner-local directory of
//! replacement images (`<name>.rgba` raw RGBA8 with a `<name>.size` text file "W H", any size, e.g.
//! upscaled or redrawn borders). A missing override keeps the disc texture. Nothing there is
//! committed (the directory is the owner's, under gitignored `tmp/` by convention).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tsfp_format_pak::Pak;
use tsfp_import::source::{self, Platform};
use tsfp_ui_model::skin::{SkinTexture, SkinTextures};
use tsfp_ui_renderer::TextureId;

use crate::{Error, WgpuUiRenderer};

/// Front-end paks searched in order for skin members.
pub const SKIN_PAKS: [&str; 5] = [
    "pak/arcade/l_100.pak",
    "pak/arcade/l_101.pak",
    "pak/arcade/l_102.pak",
    "pak/arcade/l_103.pak",
    "pak/arcade/l_106.pak",
];

/// Renderer ids of the skin pages (outside the font and stand-in ranges).
pub const SKIN_BASE: u64 = 0x534B_0000;
pub const EDGE_TRANSPOSED: TextureId = TextureId(SKIN_BASE + 0xFF);

/// One decoded texture, RGBA8 straight alpha, top-left origin.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    /// The image with x and y swapped.
    pub fn transposed(&self) -> Image {
        let (w, h) = (self.width as usize, self.height as usize);
        let mut rgba = vec![0; self.rgba.len()];
        for y in 0..h {
            for x in 0..w {
                let (src, dst) = ((y * w + x) * 4, (x * h + y) * 4);
                rgba[dst..dst + 4].copy_from_slice(&self.rgba[src..src + 4]);
            }
        }
        Image {
            width: self.height,
            height: self.width,
            rgba,
        }
    }
}

/// Decode an XBT (formats 2 A8R8G8B8, 3 R5G6B5, 5 A8, 6 L8, 7 A8L8; T1761; T2311: the block
/// formats 0 DXT1, 1 DXT3 and 4 DXT5, the enum to D3D format map of the loader jump table
/// `0x185B0`, decoded by the replayer's NV2A texture decoder).
pub fn decode_xbt(bytes: &[u8]) -> Option<Image> {
    let word = |i: usize| -> Option<u32> {
        Some(u32::from_le_bytes(
            bytes.get(i * 4..i * 4 + 4)?.try_into().ok()?,
        ))
    };
    let (width, height, format) = (word(0)?, word(1)?, word(5)?);
    if !width.is_power_of_two() || !height.is_power_of_two() || width > 4096 || height > 4096 {
        return None;
    }
    if let Some(code) = match format {
        0 => Some(0x0Cu32),
        1 => Some(0x0E),
        4 => Some(0x0F),
        _ => None,
    } {
        let format_word = (code << 8) | (width.trailing_zeros() << 20) | (height.trailing_zeros() << 24);
        let decoded = tsfp_wgpu::texture::decode(format_word, 0, bytes.get(128..)?)?;
        return Some(Image {
            width: decoded.width,
            height: decoded.height,
            rgba: decoded.rgba,
        });
    }
    let bpp = match format {
        2 => 4,
        3 | 7 => 2,
        5 | 6 => 1,
        _ => return None,
    };
    let (w, h) = (width as usize, height as usize);
    let texels = bytes.get(128..128 + w * h * bpp)?;
    let linear = tsfp_import::png::unswizzle(texels, w, h, bpp);
    let expand5 = |v: u16| ((u32::from(v) * 255 + 15) / 31) as u8;
    let expand6 = |v: u16| ((u32::from(v) * 255 + 31) / 63) as u8;
    let rgba: Vec<u8> = match format {
        // Memory order B, G, R, A.
        2 => linear
            .chunks(4)
            .flat_map(|p| [p[2], p[1], p[0], p[3]])
            .collect(),
        3 => linear
            .chunks(2)
            .flat_map(|p| {
                let v = u16::from_le_bytes([p[0], p[1]]);
                [
                    expand5(v >> 11),
                    expand6((v >> 5) & 63),
                    expand5(v & 31),
                    255,
                ]
            })
            .collect(),
        // Alpha only: white with that alpha.
        5 => linear.iter().flat_map(|a| [255, 255, 255, *a]).collect(),
        6 => linear.iter().flat_map(|l| [*l, *l, *l, 255]).collect(),
        // Memory order L, A.
        _ => linear
            .chunks(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
    };
    Some(Image {
        width,
        height,
        rgba,
    })
}

/// The decoded member keyed by `crc32(path)` of `pak`, if it is an uncompressed XBT.
fn member_image(pak: &Pak, path: &str) -> Option<Image> {
    decode_xbt(&crate::disc::pak_member(pak, path)?)
}

/// Owner-local replacement images for the opt-in `improved` preset.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkinOverrides {
    pub dir: Option<PathBuf>,
}

impl SkinOverrides {
    /// `TSFP_UI_SKIN_DIR`, the owner's override directory (unset: disc textures only).
    pub fn from_env() -> Self {
        Self {
            dir: std::env::var_os("TSFP_UI_SKIN_DIR").map(PathBuf::from),
        }
    }

    /// `<dir>/<name>.rgba` with `<dir>/<name>.size` ("W H"), when both exist and agree.
    pub fn image(&self, role: SkinTexture) -> Option<Image> {
        self.named_image(role.name())
    }

    /// The override image of texture table name `name` (T2311: also the list row icons).
    pub fn named_image(&self, name: &str) -> Option<Image> {
        let dir = self.dir.as_ref()?;
        let size = std::fs::read_to_string(dir.join(format!("{name}.size"))).ok()?;
        let mut parts = size.split_whitespace().map(str::parse::<u32>);
        let (width, height) = (parts.next()?.ok()?, parts.next()?.ok()?);
        let rgba = std::fs::read(dir.join(format!("{name}.rgba"))).ok()?;
        (width > 0 && height > 0 && rgba.len() == (width * height * 4) as usize).then_some(Image {
            width,
            height,
            rgba,
        })
    }
}

/// Names of the two original font atlases (texture table indices 964 and 965, MEASURED: the
/// table records at `0x4D87BC` and `0x4D87D4` name them; the font records 0 and 1 draw from them).
pub const ORIGINAL_FONT_NAMES: [&str; 2] = ["impact", "arialbold"];

/// An owner-supplied outline font for one original atlas (opt-in `improved` preset).
#[derive(Clone, Debug, PartialEq)]
pub struct FontFileOverride {
    /// Original atlas name (`impact` or `arialbold`).
    pub atlas: &'static str,
    pub path: PathBuf,
    /// "TrueType" or "OpenType" from the file's magic.
    pub kind: &'static str,
}

/// `<dir>/<atlas>.ttf` or `.otf` for each original atlas in `TSFP_UI_FONT_DIR` (owner-local,
/// gitignored, never committed). Files whose magic is not TrueType/OpenType are skipped.
/// The offline build has no outline rasteriser yet, so a found font is reported and the
/// original pages still draw (debt, docs/t2307-ui-visual-fidelity.md).
pub fn font_overrides_from_env() -> Vec<FontFileOverride> {
    let Some(dir) = std::env::var_os("TSFP_UI_FONT_DIR").map(PathBuf::from) else {
        return Vec::new();
    };
    font_overrides_in(&dir)
}

pub fn font_overrides_in(dir: &Path) -> Vec<FontFileOverride> {
    let mut out = Vec::new();
    for atlas in ORIGINAL_FONT_NAMES {
        for ext in ["ttf", "otf", "TTF", "OTF"] {
            let path = dir.join(format!("{atlas}.{ext}"));
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let kind = match bytes.get(0..4) {
                Some([0, 1, 0, 0]) | Some(b"true") => "TrueType",
                Some(b"OTTO") => "OpenType",
                _ => continue,
            };
            out.push(FontFileOverride { atlas, path, kind });
            break;
        }
    }
    out
}

/// The skin images by role, plus where each came from.
#[derive(Clone, Debug, Default)]
pub struct SkinImages {
    pub images: Vec<(SkinTexture, Image, &'static str)>,
    /// T2311: the front-end paks, for textures named at run time (list row icons, medals).
    pub named: Option<Arc<NamedTextures>>,
}

/// T2311: front-end textures by original texture table name (`assets/ts/textures/xbox/<name>.xbt`
/// in [`SKIN_PAKS`], MEASURED: the level icons and the padlock are DXT3 members of `l_101.pak`),
/// decoded on first use and kept. Owner overrides apply by the same name as the skin's.
pub struct NamedTextures {
    paks: Vec<Pak>,
    overrides: SkinOverrides,
    cache: Mutex<HashMap<String, Option<Arc<Image>>>>,
}

impl std::fmt::Debug for NamedTextures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NamedTextures").field("paks", &self.paks.len()).finish()
    }
}

impl NamedTextures {
    pub fn new(paks: Vec<Pak>, overrides: SkinOverrides) -> Self {
        Self { paks, overrides, cache: Mutex::new(HashMap::new()) }
    }

    /// The decoded texture `name`, or None when no pak has it (or it does not decode).
    pub fn image(&self, name: &str) -> Option<Arc<Image>> {
        let mut cache = self.cache.lock().ok()?;
        if let Some(hit) = cache.get(name) {
            return hit.clone();
        }
        let found = self.overrides.named_image(name).or_else(|| {
            let path = format!("assets/ts/textures/xbox/{name}.xbt");
            self.paks.iter().find_map(|pak| member_image(pak, &path))
        });
        let found = found.map(Arc::new);
        cache.insert(name.to_string(), found.clone());
        found
    }
}

impl SkinImages {
    /// Read every role from the disc's front-end paks, then apply owner overrides.
    pub fn from_disc(disc_path: &Path, overrides: &SkinOverrides) -> Result<Self, String> {
        let disc = source::open(disc_path, "disc", Platform::Xbox)?;
        let files = disc.list()?;
        let mut images = Vec::new();
        let mut missing: Vec<SkinTexture> = SkinTexture::ALL.to_vec();
        let mut paks = Vec::new();
        for pak_path in SKIN_PAKS {
            let Some(entry) = files.iter().find(|f| f.path.eq_ignore_ascii_case(pak_path)) else {
                continue;
            };
            let bytes = disc.read(entry).map_err(|e| format!("{pak_path}: {e}"))?;
            let pak = Pak::parse(&bytes).map_err(|e| format!("{pak_path}: {e}"))?;
            missing.retain(|role| match member_image(&pak, &role.disc_path()) {
                Some(image) => {
                    images.push((*role, image, "disc"));
                    false
                }
                None => true,
            });
            paks.push(pak);
        }
        for (role, image, source) in &mut images {
            if let Some(better) = overrides.image(*role) {
                *image = better;
                *source = "override";
            }
        }
        if !missing.is_empty() {
            let names: Vec<&str> = missing.iter().map(|r| r.name()).collect();
            return Err(format!(
                "skin textures not on the disc: {}",
                names.join(", ")
            ));
        }
        Ok(Self {
            images,
            named: Some(Arc::new(NamedTextures::new(paks, overrides.clone()))),
        })
    }

    /// Upload every image (bilinear) and the transposed rim edge; returns the id map.
    pub fn register(&self, renderer: &mut WgpuUiRenderer<'_>) -> Result<SkinTextures, Error> {
        let mut ids = Vec::new();
        let mut edge_transposed = None;
        for (role, image, _) in &self.images {
            let id = TextureId(SKIN_BASE + u64::from(role.table_index()));
            renderer.upload_rgba_filtered(id, image.width, image.height, &image.rgba)?;
            ids.push((*role, id));
            if *role == SkinTexture::FrameEdge {
                let t = image.transposed();
                renderer.upload_rgba_filtered(EDGE_TRANSPOSED, t.width, t.height, &t.rgba)?;
                edge_transposed = Some(EDGE_TRANSPOSED);
            }
        }
        Ok(SkinTextures {
            ids,
            edge_transposed,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Tier 0 on FABRICATED XBT bytes (no game data).
    use super::*;

    fn xbt(width: u32, height: u32, format: u32, texels: &[u8]) -> Vec<u8> {
        let mut out = vec![0u8; 128];
        for (i, w) in [width, height, width, height, 0, format].iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
        out.extend_from_slice(texels);
        out
    }

    #[test]
    fn decodes_a8_as_white_with_alpha() {
        let image = decode_xbt(&xbt(2, 2, 5, &[0, 64, 128, 255])).unwrap();
        assert_eq!((image.width, image.height), (2, 2));
        assert_eq!(&image.rgba[0..4], &[255, 255, 255, 0]);
        assert!(image.rgba.chunks(4).all(|p| p[0] == 255));
    }

    #[test]
    fn decodes_l8_opaque_and_argb_order() {
        let l8 = decode_xbt(&xbt(1, 1, 6, &[77])).unwrap();
        assert_eq!(l8.rgba, vec![77, 77, 77, 255]);
        let argb = decode_xbt(&xbt(1, 1, 2, &[1, 2, 3, 4])).unwrap();
        assert_eq!(argb.rgba, vec![3, 2, 1, 4]);
    }

    #[test]
    fn rejects_short_compressed_and_odd_sizes() {
        assert!(decode_xbt(&xbt(2, 2, 5, &[0, 1])).is_none());
        // T2311: block formats decode; a short block payload and the unknown enum 8 do not.
        assert!(decode_xbt(&xbt(4, 4, 0, &[0; 4])).is_none());
        assert!(decode_xbt(&xbt(4, 4, 8, &[0; 64])).is_none());
        // DXT1, colour 0 = white (0xFFFF), colour 1 = black, every texel index 0: opaque white.
        let dxt1 = decode_xbt(&xbt(4, 4, 0, &[0xFF, 0xFF, 0, 0, 0, 0, 0, 0])).unwrap();
        assert_eq!((dxt1.width, dxt1.height), (4, 4));
        assert!(dxt1.rgba.chunks(4).all(|p| p == [255, 255, 255, 255]));
        // DXT3 (enum 1): explicit alpha nibbles 0xF, same colour block.
        let mut dxt3 = vec![0xFFu8; 8];
        dxt3.extend_from_slice(&[0xFF, 0xFF, 0, 0, 0, 0, 0, 0]);
        let dxt3 = decode_xbt(&xbt(4, 4, 1, &dxt3)).unwrap();
        assert!(dxt3.rgba.chunks(4).all(|p| p == [255, 255, 255, 255]));
        assert!(decode_xbt(&xbt(3, 2, 5, &[0; 6])).is_none());
    }

    #[test]
    fn transpose_swaps_axes() {
        let image = Image {
            width: 2,
            height: 1,
            rgba: vec![1, 1, 1, 1, 2, 2, 2, 2],
        };
        let t = image.transposed();
        assert_eq!((t.width, t.height), (1, 2));
        assert_eq!(t.rgba, vec![1, 1, 1, 1, 2, 2, 2, 2]);
        let square = Image {
            width: 2,
            height: 2,
            rgba: (0..16).collect(),
        };
        assert_eq!(square.transposed().transposed(), square);
        assert_eq!(&square.transposed().rgba[4..8], &[8, 9, 10, 11]);
    }

    #[test]
    fn font_overrides_need_outline_magic() {
        let dir = std::env::temp_dir().join(format!("t2307-font-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("impact.ttf"), [0u8, 1, 0, 0, 9, 9]).unwrap();
        std::fs::write(dir.join("arialbold.otf"), b"nope").unwrap();
        let found = font_overrides_in(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].atlas, found[0].kind), ("impact", "TrueType"));
        std::fs::write(dir.join("arialbold.otf"), b"OTTOxx").unwrap();
        assert_eq!(font_overrides_in(&dir).len(), 2);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn overrides_need_matching_size() {
        let dir = std::env::temp_dir().join(format!("t2307-skin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let name = SkinTexture::FrameCorner.name();
        std::fs::write(dir.join(format!("{name}.size")), "2 1").unwrap();
        std::fs::write(dir.join(format!("{name}.rgba")), [9u8; 8]).unwrap();
        let overrides = SkinOverrides {
            dir: Some(dir.clone()),
        };
        assert_eq!(overrides.image(SkinTexture::FrameCorner).unwrap().width, 2);
        std::fs::write(dir.join(format!("{name}.rgba")), [9u8; 7]).unwrap();
        assert!(overrides.image(SkinTexture::FrameCorner).is_none());
        assert!(overrides.image(SkinTexture::FrameEdge).is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
