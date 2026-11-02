//! Stage B converters and the pipeline driver.
use crate::Result;
use crate::json::quote_latin1;
use crate::manifest::{Output, StageAFile};
use crate::png::{self, Color, unswizzle};
use crate::source::{FileEntry, Source, safe};
use crate::{audio, inflate, manifest, names, sha256, structured};
use std::collections::HashMap;
use std::path::Path;

/// Converter names carry a version so changed output invalidates the incremental cache.
pub const BS_JSON: (&str, u32) = ("bs-json", 1);
pub const XBT_KTX2: (&str, u32) = ("xbt-ktx2", 1);
pub const XBT_PNG: (&str, u32) = ("xbt-png", 2);
/// Bump when the stage B layout changes so older manifests are not reused.
pub const PIPELINE: u32 = 8;
pub const PAK_MEMBER: (&str, u32) = ("pak-member", 1);
pub const XADPCM_FLAC: (&str, u32) = ("xadpcm-flac", 2);
pub const VAG_FLAC: (&str, u32) = ("vag-flac", 2);
pub const AUDIO_SIDECAR: (&str, u32) = ("audio-sidecar", 2);
pub const BS_SNIFF: (&str, u32) = ("bs-json-sniffed", 1);
pub const PLAIN_TEXT: (&str, u32) = ("plain-text", 1);
pub const RAW_MEMBER: (&str, u32) = ("raw-member", 1);
pub const XMV_RAW: (&str, u32) = ("xmv-raw", 1);

fn version_dir(root: &Path, id: &str) -> Result<std::path::PathBuf> {
    if id.is_empty() || id == "." || id.contains('/') || id.contains('\\') {
        return Err(format!("unsafe version id: {id:?}"));
    }
    safe(id)?;
    Ok(version_path(root, id))
}

fn version_path(root: &Path, id: &str) -> std::path::PathBuf {
    root.join("assets").join("original").join(id)
}

fn work_path(root: &Path, id: &str) -> std::path::PathBuf {
    version_path(root, id).join(".importer")
}

fn source_dir(root: &Path, id: &str) -> Result<std::path::PathBuf> {
    Ok(version_dir(root, id)?.join(".source"))
}

fn work_dir(root: &Path, id: &str) -> Result<std::path::PathBuf> {
    Ok(version_dir(root, id)?.join(".importer"))
}

fn combined_manifest(root: &Path, id: &str) -> Result<String> {
    let work = work_dir(root, id)?;
    let read_stage = |name: &str| -> Result<Option<String>> {
        match std::fs::read_to_string(work.join(name)) {
            Ok(text) => Ok(Some(text.trim().to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    };
    let stage_a = read_stage("stage-a.json")?.unwrap_or_else(|| "null".into());
    let stage_b = read_stage("stage-b.json")?.unwrap_or_else(|| "null".into());
    Ok(format!(
        "{{\"schema\":{},\"version\":{},\"stage_a\":{},\"stage_b\":{}}}\n",
        manifest::SCHEMA,
        crate::json::quote(id),
        stage_a,
        stage_b
    ))
}

fn refresh_manifest(root: &Path, id: &str) -> Result<()> {
    let version = version_dir(root, id)?;
    let text = combined_manifest(root, id)?;
    std::fs::create_dir_all(&version).map_err(|e| e.to_string())?;
    std::fs::write(version.join("manifest.json"), &text).map_err(|e| e.to_string())?;
    std::fs::write(
        version.join("manifest.sha256"),
        format!("{}  manifest.json\n", sha256::hex(text.as_bytes())),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn write_override_contract(root: &Path) -> Result<()> {
    let dir = root.join("assets").join("overrides");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(".contract.json");
    if !path.exists() {
        let contract = "{\"schema\":1,\"enabled\":false,\"description\":\"Overrides are ignored unless explicitly enabled by the user.\",\"sidecar\":\"Each override requires a sibling <file>.override.json with non-empty label, replaces logical id, and sha256 of the override bytes.\"}\n";
        std::fs::write(path, contract).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Logical paths known to exist inside the PAKs. PAK keys are CRC32 of the path, so names
/// can only be recovered from a candidate list. `--names FILE` extends this.
pub const BUILTIN_NAMES: [&str; 3] = [
    "data/str_ts_pxe.bs",
    "data/str_ts_pxe_story.bs",
    "textures/cc95c8.xbt",
];

/// Parsed text bank with everything needed to rebuild the original file exactly.
pub struct BsDoc {
    pub offsets: Vec<u32>,
    pub strings: Vec<Vec<u8>>,
    /// Pool bytes not reached by any string (offset from file start, bytes).
    pub gaps: Vec<(usize, Vec<u8>)>,
    pub total: usize,
}

impl BsDoc {
    /// The entry count is the first offset divided by four (it equals the table size, measured
    /// on both English banks, T1760).
    pub fn parse(b: &[u8]) -> Result<Self> {
        let first = tsfp_format_core::word(b, 0)? as usize;
        if first == 0 || first % 4 != 0 || first > b.len() {
            return Err("bs: first offset is not a table size".into());
        }
        let bank = tsfp_format_bs::Bank::parse(b, first / 4)?;
        let mut covered = vec![false; b.len()];
        let mut strings = Vec::new();
        for (i, off) in bank.offsets.iter().enumerate() {
            let s = bank.string(i)?.to_vec();
            covered[*off as usize..*off as usize + s.len() + 1].fill(true);
            strings.push(s);
        }
        covered[..first].fill(true);
        let mut gaps = Vec::new();
        let mut i = 0;
        while i < b.len() {
            if covered[i] {
                i += 1;
                continue;
            }
            let s = i;
            while i < b.len() && !covered[i] {
                i += 1;
            }
            gaps.push((s, b[s..i].to_vec()));
        }
        Ok(Self {
            offsets: bank.offsets,
            strings,
            gaps,
            total: b.len(),
        })
    }

    /// Rebuild the original bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = vec![0u8; self.total];
        for (i, w) in self.offsets.iter().enumerate() {
            b[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
        for (o, s) in self.offsets.iter().zip(&self.strings) {
            b[*o as usize..*o as usize + s.len()].copy_from_slice(s);
        }
        for (o, g) in &self.gaps {
            b[*o..*o + g.len()].copy_from_slice(g);
        }
        b
    }

    /// JSON: strings are Latin-1 text (byte value = code point, not a claim about the game's
    /// glyph encoding). `offsets` keep alias identity, `gaps` keep unreferenced bytes.
    pub fn to_json(&self) -> String {
        let nums: Vec<String> = self.offsets.iter().map(u32::to_string).collect();
        let strs: Vec<String> = self.strings.iter().map(|s| quote_latin1(s)).collect();
        let gaps: Vec<String> = self
            .gaps
            .iter()
            .map(|(o, g)| {
                format!(
                    "{{\"offset\":{o},\"hex\":\"{}\"}}",
                    g.iter().map(|x| format!("{x:02x}")).collect::<String>()
                )
            })
            .collect();
        format!(
            "{{\n\"format\":\"tsfp-bs\",\"version\":1,\"encoding\":\"latin-1-bytes\",\"file_size\":{},\"count\":{},\n\"offsets\":[{}],\n\"gaps\":[{}],\n\"strings\":[\n{}\n]}}\n",
            self.total,
            self.offsets.len(),
            nums.join(","),
            gaps.join(","),
            strs.join(",\n")
        )
    }
}

/// KTX2 Vulkan formats and Khronos DFD color models for the Xbox XBT enums (T1761).
fn ktx2_format(format: u32) -> Option<(u32, u8, usize, &'static str)> {
    match format {
        0 => Some((133, 128, 8, "BC1")), // VK_FORMAT_BC1_RGBA_UNORM_BLOCK, BC1A
        1 => Some((135, 129, 16, "BC2")), // VK_FORMAT_BC2_UNORM_BLOCK, BC2
        4 => Some((137, 130, 16, "BC3")), // VK_FORMAT_BC3_UNORM_BLOCK, BC3
        _ => None,
    }
}

fn compressed_mip_sizes(w: usize, h: usize, block: usize, payload: usize) -> Result<Vec<usize>> {
    let (mut width, mut height, mut total) = (w, h, 0usize);
    let mut sizes = Vec::new();
    for _ in 0..32 {
        let bytes = width
            .div_ceil(4)
            .checked_mul(height.div_ceil(4))
            .and_then(|blocks| blocks.checked_mul(block))
            .ok_or_else(|| "xbt: compressed mip size overflow".to_string())?;
        total = total
            .checked_add(bytes)
            .ok_or_else(|| "xbt: compressed mip chain overflow".to_string())?;
        sizes.push(bytes);
        if total == payload {
            return Ok(sizes);
        }
        if total > payload || (width == 1 && height == 1) {
            break;
        }
        width = (width / 2).max(1);
        height = (height / 2).max(1);
    }
    Err("xbt: payload is not an exact mip chain".into())
}

fn uncompressed_mip_count(
    w: usize,
    h: usize,
    bytes_per_pixel: usize,
    payload: usize,
) -> Result<usize> {
    let (mut width, mut height, mut total) = (w, h, 0usize);
    for mip in 1..=32 {
        let bytes = width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(bytes_per_pixel))
            .ok_or_else(|| "xbt: uncompressed mip size overflow".to_string())?;
        total = total
            .checked_add(bytes)
            .ok_or_else(|| "xbt: uncompressed mip chain overflow".to_string())?;
        if total == payload {
            return Ok(mip);
        }
        if total > payload || (width == 1 && height == 1) {
            break;
        }
        width = (width / 2).max(1);
        height = (height / 2).max(1);
    }
    Err("xbt: payload is not an exact mip chain".into())
}

/// Convert block-compressed XBT to KTX2 without touching the compressed blocks.
pub fn xbt_to_ktx2(b: &[u8]) -> Result<(Vec<u8>, String)> {
    let t = tsfp_format_xbt::Texture::parse(b)?;
    let (w, h, fmt) = (t.words[0] as usize, t.words[1] as usize, t.words[5]);
    let (vk_format, color_model, block, label) =
        ktx2_format(fmt).ok_or_else(|| format!("xbt: format enum {fmt} has no KTX2 mapping"))?;
    let sizes = compressed_mip_sizes(w, h, block, t.pixels.len())?;
    let mut offset = 0;
    let levels: Vec<&[u8]> = sizes
        .iter()
        .map(|size| {
            let level = &t.pixels[offset..offset + size];
            offset += size;
            level
        })
        .collect();
    let output =
        crate::ktx2::encode_bc(w as u32, h as u32, vk_format, color_model, block, &levels)?;
    Ok((
        output,
        format!("{w}x{h} {label} mips {} preserved", levels.len()),
    ))
}

/// Uncompressed Xbox texture enums: (png colour, bytes per texel). D3D formats per T1761:
/// 2 A8R8G8B8, 3 R5G6B5, 5 A8, 6 L8, 7 A8L8. All are Morton swizzled.
fn raw_layout(format: u32) -> Option<(Color, usize)> {
    match format {
        2 => Some((Color::Rgba, 4)),
        3 => Some((Color::Rgb, 2)),
        5 => Some((Color::GrayAlpha, 1)),
        6 => Some((Color::Gray, 1)),
        7 => Some((Color::GrayAlpha, 2)),
        _ => None,
    }
}

/// Convert a single-level uncompressed XBT to PNG and verify the decoded PNG pixels.
pub fn xbt_to_png(b: &[u8]) -> Result<(Vec<u8>, String)> {
    let t = tsfp_format_xbt::Texture::parse(b)?;
    let (w, h, fmt) = (t.words[0] as usize, t.words[1] as usize, t.words[5]);
    let (color, bpp) =
        raw_layout(fmt).ok_or_else(|| format!("xbt: format enum {fmt} has no PNG mapping"))?;
    if !w.is_power_of_two() || !h.is_power_of_two() {
        return Err(format!(
            "xbt: {w}x{h} is not a power of two, cannot unswizzle"
        ));
    }
    let mips = uncompressed_mip_count(w, h, bpp, t.pixels.len())?;
    let base_len = w
        .checked_mul(h)
        .and_then(|pixels| pixels.checked_mul(bpp))
        .ok_or_else(|| "xbt: uncompressed texture size overflow".to_string())?;
    let lin = unswizzle(&t.pixels[..base_len], w, h, bpp);
    let px: Vec<u8> = match fmt {
        // memory order B, G, R, A
        2 => lin
            .chunks(4)
            .flat_map(|p| [p[2], p[1], p[0], p[3]])
            .collect(),
        3 => lin
            .chunks(2)
            .flat_map(|p| {
                let v = u16::from_le_bytes([p[0], p[1]]);
                let e =
                    |x: u16, n: u32| ((x as u32 * 255 + ((1 << n) - 1) / 2) / ((1 << n) - 1)) as u8;
                [e(v >> 11, 5), e((v >> 5) & 63, 6), e(v & 31, 5)]
            })
            .collect(),
        // alpha only: white with that alpha
        5 => lin.iter().flat_map(|a| [255, *a]).collect(),
        // L8 as gray
        6 => lin,
        // A8L8: low byte L, high byte A in memory order L, A
        _ => lin,
    };
    let name = match fmt {
        2 => "A8R8G8B8",
        3 => "R5G6B5",
        5 => "A8",
        6 => "L8",
        _ => "A8L8",
    };
    let output = png::encode(w, h, color, &px);
    png::verify_encoded_pixels(&output, w, h, color, &px)?;
    Ok((
        output,
        format!("{w}x{h} {name} unswizzled, {mips} mips in source, base PNG pixels verified"),
    ))
}

/// Pick the converter for an XBT by its format enum. Returns (extension, converter, bytes, note).
/// (extension, converter, bytes, note)
pub type XbtOut = (&'static str, (&'static str, u32), Vec<u8>, String);
pub fn xbt_convert(b: &[u8]) -> Result<XbtOut> {
    let fmt = tsfp_format_xbt::Texture::parse(b)?.words[5];
    if ktx2_format(fmt).is_some() {
        let (d, n) = xbt_to_ktx2(b)?;
        Ok(("ktx2", XBT_KTX2, d, n))
    } else {
        let (d, n) = xbt_to_png(b)?;
        Ok(("png", XBT_PNG, d, n))
    }
}

/// Structural test for a nameless PAK member: the 128 byte XBT header carries width and height
/// twice, a zero word and a format enum 0..7, and conversion must accept the payload.
pub fn sniff_xbt(b: &[u8]) -> bool {
    b.len() > 128
        && tsfp_format_core::word(b, 0).is_ok_and(|w| {
            let g = |i: usize| tsfp_format_core::word(b, i * 4).unwrap_or(u32::MAX);
            w > 0
                && w <= 4096
                && g(0) == g(2)
                && g(1) == g(3)
                && g(1) > 0
                && g(1) <= 4096
                && g(4) == 0
                && g(5) <= 7
        })
        && xbt_convert(b).is_ok()
}

pub fn path_key(path: &str) -> u32 {
    tsfp_format_core::crc32(path.as_bytes())
}

fn stem(path: &str) -> &str {
    path.rsplit_once('.').map_or(path, |x| x.0)
}

/// Structural text bank test for unnamed members. Strict on purpose: at least 4 strings,
/// non-decreasing offsets starting right after the table, at least 4 distinct offsets and 4 non-empty strings, 95% printable bytes
/// in the pool, and an exact rebuild.
pub fn sniff_bs(b: &[u8]) -> Option<BsDoc> {
    if b.len() < 24 {
        return None;
    }
    let first = u32::from_le_bytes(b[..4].try_into().ok()?) as usize;
    if first < 16 || first % 4 != 0 || first >= b.len() || first > 1 << 20 {
        return None;
    }
    let offs: Vec<u32> = b[..first]
        .chunks_exact(4)
        .map(|w| u32::from_le_bytes(w.try_into().unwrap()))
        .collect();
    if offs[0] as usize != first || offs.windows(2).any(|w| w[1] < w[0]) {
        return None;
    }
    let pool = &b[first..];
    let printable = pool
        .iter()
        .filter(|c| (0x20..0x7f).contains(*c) || **c == 0 || **c == b'\n')
        .count();
    if printable * 100 < pool.len() * 95 {
        return None;
    }
    let doc = BsDoc::parse(b).ok()?;
    let mut distinct = doc.offsets.clone();
    distinct.dedup();
    let filled = doc.strings.iter().filter(|s| !s.is_empty()).count();
    (distinct.len() >= 4 && filled >= 4 && doc.to_bytes() == b).then_some(doc)
}

/// Members that are already plain ASCII text with line breaks (cutscene `STREAM`/`FRAMES`
/// lists, `headerBegin` pad tables): at least 16 bytes, only printable ASCII, tab, CR and LF,
/// and at least one LF. Copied verbatim to a `.txt` file, so lossless by construction.
pub fn sniff_plain_text(b: &[u8]) -> bool {
    b.len() >= 16
        && b.contains(&b'\n')
        && b.iter()
            .all(|c| (0x20..0x7f).contains(c) || matches!(c, b'\t' | b'\r' | b'\n'))
}

/// Collects outputs and writes them under `root/assets/original/<id>/`.
pub struct Sink<'a> {
    pub root: &'a Path,
    pub id: &'a str,
    pub outputs: Vec<Output>,
}
impl Sink<'_> {
    #[allow(clippy::too_many_arguments)]
    fn emit(
        &mut self,
        logical: String,
        src_path: &str,
        src_sha: &str,
        conv: (&str, u32),
        data: &[u8],
        lossless: bool,
        note: String,
    ) -> Result<()> {
        let rel = safe(&logical)?;
        let dest = version_dir(self.root, self.id)?.join(rel);
        std::fs::create_dir_all(dest.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&dest, data).map_err(|e| format!("{}: {e}", dest.display()))?;
        self.outputs.push(Output {
            logical_id: logical,
            source_path: src_path.into(),
            source_sha256: src_sha.into(),
            converter: conv.0.into(),
            converter_version: conv.1,
            sha256: sha256::hex(data),
            size: data.len() as u64,
            lossless,
            note,
        });
        Ok(())
    }

    fn emit_member(
        &mut self,
        logical: String,
        src_path: &str,
        src_sha: &str,
        conv: (&str, u32),
        data: &[u8],
        lossless: bool,
        note: String,
        original: &[u8],
    ) -> Result<()> {
        let note = if lossless {
            note
        } else {
            format!("{note}; original member retained under raw/")
        };
        self.emit(logical, src_path, src_sha, conv, data, lossless, note)?;
        if !lossless {
            self.raw_member(src_path, src_sha, original)?;
        }
        Ok(())
    }

    fn raw_member(&mut self, src_path: &str, src_sha: &str, data: &[u8]) -> Result<()> {
        self.raw_member_with_reason(
            src_path,
            src_sha,
            data,
            "no lossless converter recognized this member",
        )
    }

    fn raw_member_with_reason(
        &mut self,
        src_path: &str,
        src_sha: &str,
        data: &[u8],
        reason: &str,
    ) -> Result<()> {
        let mut identity = src_path.as_bytes().to_vec();
        identity.push(0);
        identity.extend_from_slice(data);
        let logical = format!("raw/{}.bin", sha256::hex(&identity));
        if self.outputs.iter().any(|o| o.logical_id == logical) {
            return Ok(());
        }
        self.emit(
            logical,
            src_path,
            src_sha,
            RAW_MEMBER,
            data,
            true,
            format!("original member bytes retained raw: {reason}"),
        )
    }

    /// Audio members: a RIFF Xbox ADPCM file, a VAG, or (inside the `sounds_*` / `musicts`
    /// banks only) a raw headerless Xbox ADPCM stream. Rate and channels of raw members are
    /// INFERRED: mono 22050 Hz, or stereo (72 byte blocks) at 22050 Hz, and 32000 Hz stereo for
    /// `musicts` (the music streams in the T1159/T1165 runtime traces).
    fn audio(
        &mut self,
        pak: &str,
        origin: &str,
        src_sha: &str,
        key: u32,
        body: &[u8],
        sound_name: Option<&str>,
    ) -> Result<bool> {
        let lower = pak.to_ascii_lowercase();
        let bank = lower.rsplit('/').next().unwrap_or("");
        let music = bank.starts_with("musicts");
        let raw_bank = music || bank.starts_with("sounds_");
        let (conv, dec) = if let Some(Ok(d)) = audio::convert_vag(body) {
            (VAG_FLAC, d)
        } else if let Some(Ok(d)) = audio::convert_riff_xadpcm(body) {
            (XADPCM_FLAC, d)
        } else if raw_bank {
            let order: [usize; 2] = if music { [2, 1] } else { [1, 2] };
            let how = |c: usize| {
                format!(
                    "headerless member, {} channel(s) and {} Hz INFERRED (not stored in the bank), loop unknown",
                    c,
                    if music { 32000 } else { 22050 }
                )
            };
            let Some(c) = order.into_iter().find(|c| audio::is_xadpcm(body, *c)) else {
                return Ok(false);
            };
            (
                XADPCM_FLAC,
                audio::convert_xadpcm(body, c, if music { 32000 } else { 22050 }, &how(c))?,
            )
        } else {
            return Ok(false);
        };
        let dec = dec.with_sound_key(key, sound_name);
        let dir = format!("audio/{}/{key:08x}", stem(pak));
        let note = format!(
            "{} Hz, {} ch, {} frames; FLAC lossless over decoded PCM, source ADPCM member kept raw",
            dec.rate, dec.channels, dec.frames
        );
        self.emit_member(
            format!("{dir}.flac"),
            origin,
            src_sha,
            conv,
            &dec.flac,
            false,
            note,
            body,
        )?;
        self.emit_member(
            format!("{dir}.json"),
            origin,
            src_sha,
            AUDIO_SIDECAR,
            dec.sidecar.as_bytes(),
            false,
            "derived sidecar: rate, channels, loop and provenance".into(),
            body,
        )?;
        Ok(true)
    }

    /// Unnamed member recognised by content: audio, text bank, then plain text. XBT sniffing
    /// stays in the caller because it shares a converter with named textures.
    fn unnamed_data(
        &mut self,
        pak: &str,
        origin: &str,
        sha: &str,
        key: u32,
        body: &[u8],
    ) -> Result<bool> {
        Ok(self.audio(pak, origin, sha, key, body, None)?
            || (!sniff_xbt(body)
                && (self.sniffed_bank(pak, origin, sha, key, body)?
                    || self.plain_text(pak, origin, sha, key, body)?)))
    }

    fn plain_text(
        &mut self,
        pak: &str,
        origin: &str,
        src_sha: &str,
        key: u32,
        body: &[u8],
    ) -> Result<bool> {
        if !sniff_plain_text(body) {
            return Ok(false);
        }
        self.emit(
            format!("plain/{}/{key:08x}.txt", stem(pak)),
            origin,
            src_sha,
            PLAIN_TEXT,
            body,
            true,
            "member is already plain ASCII text, copied verbatim".into(),
        )?;
        Ok(true)
    }

    /// A nameless member that parses as a text bank: offset table that ascends from the table
    /// size, strings mostly printable, round trip exact. Nothing is claimed about the language.
    fn sniffed_bank(
        &mut self,
        pak: &str,
        origin: &str,
        src_sha: &str,
        key: u32,
        body: &[u8],
    ) -> Result<bool> {
        let Some(doc) = sniff_bs(body) else {
            return Ok(false);
        };
        self.emit(
            format!("text/{}/{key:08x}.json", stem(pak)),
            origin,
            src_sha,
            BS_SNIFF,
            doc.to_json().as_bytes(),
            true,
            "unnamed member recognised as a text bank by structure, rebuilds the original bytes"
                .into(),
        )?;
        Ok(true)
    }

    /// Convert one known-format blob named `name` (a path inside the source or a PAK).
    fn blob(&mut self, name: &str, src_path: &str, src_sha: &str, data: &[u8]) -> Result<bool> {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".xmv") {
            let relative = name.replace('\\', "/");
            self.emit_member(
                format!("movies/{relative}"),
                src_path,
                src_sha,
                XMV_RAW,
                data,
                true,
                "XMV movie retained byte-for-byte; no transcode".into(),
                data,
            )?;
            return Ok(true);
        }
        if lower.ends_with(".bs") {
            let Ok(doc) = BsDoc::parse(data) else {
                return Ok(false);
            };
            let exact = doc.to_bytes() == data;
            self.emit_member(
                format!("text/{}.json", stem(name)),
                src_path,
                src_sha,
                BS_JSON,
                doc.to_json().as_bytes(),
                exact,
                if exact {
                    "rebuilds the original bytes from offsets, strings and gaps".into()
                } else {
                    "LOSSY: rebuild differs".into()
                },
                data,
            )?;
            return Ok(true);
        }
        if lower.ends_with(".xbt") {
            match xbt_convert(data) {
                Ok((ext, conv, out, note)) => self.emit_member(
                    format!("textures/{}.{ext}", stem(name)),
                    src_path,
                    src_sha,
                    conv,
                    &out,
                    ext == "ktx2" || note.contains(" 1 mips in source"),
                    note,
                    data,
                )?,
                Err(e) => {
                    eprintln!("skip conversion {name}: {e}");
                    return Ok(false);
                }
            }
            return Ok(true);
        }
        Ok(false)
    }
}

/// Stage A: write every file loose and return the manifest rows.
pub fn stage_a(src: &dyn Source, root: &Path, only: &str) -> Result<Vec<StageAFile>> {
    let id = &src.descriptor().id;
    let base = source_dir(root, id)?;
    let mut rows = Vec::new();
    for e in src.list()?.iter().filter(|e| e.path.contains(only)) {
        let data = src.read(e)?;
        let dest = base.join(safe(&e.path)?);
        std::fs::create_dir_all(dest.parent().unwrap()).map_err(|x| x.to_string())?;
        std::fs::write(&dest, &data).map_err(|x| format!("{}: {x}", dest.display()))?;
        rows.push(StageAFile {
            path: e.path.clone(),
            size: data.len() as u64,
            sha256: sha256::hex(&data),
        });
    }
    write_manifest(
        root,
        id,
        "stage-a",
        &manifest::stage_a_json(
            id,
            src.descriptor().platform.name(),
            src.descriptor().layout,
            &rows,
        ),
    )?;
    refresh_manifest(root, id)?;
    Ok(rows)
}

pub fn write_manifest(root: &Path, id: &str, kind: &str, text: &str) -> Result<()> {
    let p = work_dir(root, id)?;
    std::fs::create_dir_all(&p).map_err(|e| e.to_string())?;
    std::fs::write(p.join(format!("{kind}.json")), text).map_err(|e| e.to_string())
}

/// Fingerprint of everything besides the source bytes that decides the stage B output.
fn fingerprint(table: &HashMap<u32, String>) -> String {
    let mut keys: Vec<_> = table.iter().collect();
    keys.sort();
    let mut text = format!(
        "{PIPELINE};{:?};{:?};{:?};{:?};{:?};",
        BS_JSON, XBT_KTX2, XBT_PNG, PAK_MEMBER, XMV_RAW
    );
    for (k, v) in keys {
        text.push_str(&format!("{k:08x}={v};"));
    }
    sha256::hex(text.as_bytes())[..16].to_string()
}

/// Rows of the previous stage B manifest grouped by source file, when its fingerprint matches.
fn previous(root: &Path, id: &str, fp: &str) -> HashMap<String, Vec<Output>> {
    let mut by_file: HashMap<String, Vec<Output>> = HashMap::new();
    let Ok(text) = std::fs::read_to_string(work_path(root, id).join("stage-b.json")) else {
        return by_file;
    };
    if !text
        .lines()
        .next()
        .is_some_and(|l| l.contains(&format!("\"fingerprint\":\"{fp}\"")))
    {
        return by_file;
    }
    for r in manifest::parse_rows(&text).unwrap_or_default() {
        let get = |k: &str| r.get(k).cloned().unwrap_or_default();
        let o = Output {
            logical_id: get("logical_id"),
            source_path: get("source_path"),
            source_sha256: get("source_sha256"),
            converter: get("converter"),
            converter_version: get("converter_version").parse().unwrap_or(0),
            sha256: get("sha256"),
            size: get("size").parse().unwrap_or(0),
            lossless: get("lossless") == "true",
            note: get("note"),
        };
        by_file
            .entry(o.source_path.split('#').next().unwrap_or("").to_string())
            .or_default()
            .push(o);
    }
    by_file
}

/// A previous result is reusable when the source hash is unchanged and every output file is
/// still on disk with the recorded size.
fn reusable(root: &Path, id: &str, rows: &[Output], sha: &str) -> bool {
    !rows.is_empty()
        && rows.iter().all(|o| {
            o.source_sha256 == sha
                && safe(&o.logical_id).is_ok_and(|rel| {
                    std::fs::metadata(version_path(root, id).join(rel))
                        .is_ok_and(|m| m.len() == o.size)
                })
        })
}

/// Change-stamp cache (`manifest/<id>.cache.json`): per source file `(size, stamp, sha256)` and
/// the recovered PAK name table keyed by a stamp of its inputs.
#[derive(Default)]
struct Cache {
    files: HashMap<String, (u64, String, String)>,
    names: Option<(String, HashMap<u32, String>)>,
}
impl Cache {
    fn load(root: &Path, id: &str) -> Self {
        let mut c = Cache::default();
        let Ok(text) = std::fs::read_to_string(work_path(root, id).join("cache.json")) else {
            return c;
        };
        for r in manifest::parse_rows(&text).unwrap_or_default() {
            let get = |k: &str| r.get(k).cloned().unwrap_or_default();
            if get("kind") == "names" {
                let t = get("table")
                    .lines()
                    .filter_map(|l| {
                        let (k, v) = l.split_once('=')?;
                        Some((u32::from_str_radix(k, 16).ok()?, v.to_string()))
                    })
                    .collect();
                c.names = Some((get("stamp"), t));
            } else {
                c.files.insert(
                    get("path"),
                    (
                        get("size").parse().unwrap_or(0),
                        get("stamp"),
                        get("sha256"),
                    ),
                );
            }
        }
        c
    }
    fn save(&self, root: &Path, id: &str) -> Result<()> {
        use crate::json::quote;
        let mut rows = Vec::new();
        if let Some((stamp, t)) = &self.names {
            let mut keys: Vec<_> = t.iter().collect();
            keys.sort();
            let table: String = keys.iter().map(|(k, v)| format!("{k:08x}={v}\n")).collect();
            rows.push(format!(
                "{{\"kind\":\"names\",\"stamp\":{},\"table\":{}}}",
                quote(stamp),
                quote(&table)
            ));
        }
        let mut files: Vec<_> = self.files.iter().collect();
        files.sort();
        for (p, (size, stamp, sha)) in files {
            rows.push(format!(
                "{{\"kind\":\"file\",\"path\":{},\"size\":{size},\"stamp\":{},\"sha256\":{}}}",
                quote(p),
                quote(stamp),
                quote(sha)
            ));
        }
        write_manifest(
            root,
            id,
            "cache",
            &format!(
                "{{\"schema\":{},\"stage\":\"cache\"}}\n{}\n",
                manifest::SCHEMA,
                rows.join("\n")
            ),
        )
    }
}

/// Counters from the last stage B run, for the CLI report.
#[derive(Default, Debug)]
pub struct Report {
    pub reused_files: usize,
    pub converted_files: usize,
    pub named_members: usize,
}

/// Stage B: decode containers and convert known formats. `names` are extra PAK path candidates.
pub fn stage_b(src: &dyn Source, root: &Path, only: &str, names: &[String]) -> Result<Vec<Output>> {
    stage_b_report(src, root, only, names).map(|x| x.0)
}

pub fn stage_b_report(
    src: &dyn Source,
    root: &Path,
    only: &str,
    names: &[String],
) -> Result<(Vec<Output>, Report)> {
    let id = src.descriptor().id.clone();
    let version = version_dir(root, &id)?;
    std::fs::create_dir_all(&version).map_err(|e| e.to_string())?;
    let files: Vec<FileEntry> = src
        .list()?
        .into_iter()
        .filter(|e| e.path.contains(only))
        .collect();
    // Name candidates: builtin list, user list, and the string table of every XBE on the image.
    // The table is cached under a stamp of the XBE files so unchanged images are not re-read.
    let listing = src.list()?;
    let xbes: Vec<&FileEntry> = listing
        .iter()
        .filter(|e| e.path.to_ascii_lowercase().ends_with(".xbe"))
        .collect();
    let names_stamp = {
        let mut t = format!("{:?};{:?};", BUILTIN_NAMES, names);
        for e in &xbes {
            t.push_str(&format!("{}:{}:{};", e.path, e.size, src.stamp(e)));
        }
        sha256::hex(t.as_bytes())[..16].to_string()
    };
    let mut cache = Cache::load(root, &id);
    let table = match cache.names.take().filter(|n| n.0 == names_stamp) {
        Some((_, t)) => t,
        None => {
            let mut binaries = Vec::new();
            for e in &xbes {
                binaries.push(src.read(e)?);
            }
            names::table(&BUILTIN_NAMES, names, &binaries)
        }
    };
    let fp = fingerprint(&table);
    let old = previous(root, &id, &fp);
    let mut report = Report::default();
    let mut newcache = Cache {
        names: Some((names_stamp, table.clone())),
        ..Cache::default()
    };
    let mut sink = Sink {
        root,
        id: &id,
        outputs: Vec::new(),
    };
    for e in &files {
        let lower = e.path.to_ascii_lowercase();
        if !(lower.ends_with(".pak")
            || lower.ends_with(".bs")
            || lower.ends_with(".xbt")
            || lower.ends_with(".xmv")
            || structured::supports_path(&e.path))
        {
            continue;
        }
        let stamp = src.stamp(e);
        // Unchanged size and stamp: reuse the recorded hash and, if the outputs are still on
        // disk, the recorded rows, without reading or hashing the source file.
        let known = cache
            .files
            .get(&e.path)
            .filter(|c| !stamp.is_empty() && c.0 == e.size && c.1 == stamp)
            .map(|c| c.2.clone());
        let mut data = Vec::new();
        let sha = match known {
            Some(h) if old.get(&e.path).is_some_and(|r| reusable(root, &id, r, &h)) => h,
            _ => {
                data = src.read(e)?;
                sha256::hex(&data)
            }
        };
        newcache
            .files
            .insert(e.path.clone(), (e.size, stamp, sha.clone()));
        if let Some(rows) = old.get(&e.path).filter(|r| reusable(root, &id, r, &sha)) {
            sink.outputs.extend(rows.iter().map(|o| Output {
                logical_id: o.logical_id.clone(),
                source_path: o.source_path.clone(),
                source_sha256: o.source_sha256.clone(),
                converter: o.converter.clone(),
                converter_version: o.converter_version,
                sha256: o.sha256.clone(),
                size: o.size,
                lossless: o.lossless,
                note: o.note.clone(),
            }));
            report.reused_files += 1;
            continue;
        }
        report.converted_files += 1;
        if data.is_empty() {
            data = src.read(e)?;
        }
        if !lower.ends_with(".pak") {
            if let Some(converted) = structured::convert_path(&e.path, &data) {
                match converted {
                    Ok(converted) => sink.emit(
                        structured::output_path(&e.path),
                        &e.path,
                        &sha,
                        converted.converter,
                        &converted.bytes,
                        true,
                        converted.note.into(),
                    )?,
                    Err(reason) => sink.raw_member_with_reason(&e.path, &sha, &data, &reason)?,
                }
                continue;
            }
            if !sink.blob(&e.path, &e.path, &sha, &data)? {
                sink.raw_member_with_reason(
                    &e.path,
                    &sha,
                    &data,
                    "no lossless converter recognized this source file",
                )?;
            }
            continue;
        }
        let pak = match tsfp_format_pak::Pak::parse(&data) {
            Ok(p) => p,
            Err(err) => {
                eprintln!("skip {}: {err}", e.path);
                continue;
            }
        };
        for (i, m) in pak.entries.iter().enumerate() {
            let stored = pak.stored(i)?;
            let body = if m.stored_size != 0 {
                match inflate::gunzip(stored, m.decoded_size as usize) {
                    Ok(v) => v,
                    Err(err) => {
                        eprintln!("skip {} member {:08x}: {err}", e.path, m.key);
                        continue;
                    }
                }
            } else {
                stored.to_vec()
            };
            let origin = format!("{}#{:08x}", e.path, m.key);
            let how = if m.stored_size != 0 {
                "gunzip of stored member"
            } else {
                "stored member"
            };
            if let Some(n) = table.get(&m.key) {
                report.named_members += 1;
                if let Some(converted) = structured::convert_path(n, &body) {
                    match converted {
                        Ok(converted) => sink.emit(
                            structured::output_path(n),
                            &origin,
                            &sha,
                            converted.converter,
                            &converted.bytes,
                            true,
                            converted.note.into(),
                        )?,
                        Err(reason) => {
                            sink.raw_member_with_reason(&origin, &sha, &body, &reason)?
                        }
                    }
                } else if !sink.audio(&e.path, &origin, &sha, m.key, &body, Some(n))?
                    && !sink.blob(n, &origin, &sha, &body)?
                {
                    sink.raw_member_with_reason(
                        &origin,
                        &sha,
                        &body,
                        &format!("named member {n:?} has no lossless converter"),
                    )?;
                }
            } else if let Some(converted) = structured::convert_sniffed(&body) {
                match converted {
                    Ok(converted) => sink.emit(
                        format!("cutscenes/{}/{:08x}.json", stem(&e.path), m.key),
                        &origin,
                        &sha,
                        converted.converter,
                        &converted.bytes,
                        true,
                        format!("{}, {how}, recognized by format signature", converted.note),
                    )?,
                    Err(reason) => sink.raw_member_with_reason(&origin, &sha, &body, &reason)?,
                }
            } else if sink.unnamed_data(&e.path, &origin, &sha, m.key, &body)? {
                // audio, text bank or plain text, see `Sink::unnamed_data`
            } else if sniff_xbt(&body) {
                match xbt_convert(&body) {
                    Ok((ext, conv, out, note)) => {
                        let lossless = ext == "ktx2" || note.contains(" 1 mips in source");
                        sink.emit_member(
                            format!("textures/{}/{:08x}.{ext}", stem(&e.path), m.key),
                            &origin,
                            &sha,
                            conv,
                            &out,
                            lossless,
                            format!("{note}, unnamed member recognised by header"),
                            &body,
                        )?;
                    }
                    Err(err) => {
                        eprintln!("skip texture conversion {}#{:08x}: {err}", e.path, m.key);
                        sink.raw_member(&origin, &sha, &body)?;
                    }
                }
            } else {
                sink.raw_member_with_reason(
                    &origin,
                    &sha,
                    &body,
                    &format!("unidentified PAK member retained after {how}"),
                )?;
            }
        }
    }
    newcache.save(root, &id)?;
    if only.is_empty() {
        prune(root, &id, &sink.outputs)?;
    }
    write_manifest(
        root,
        &id,
        "stage-b",
        &manifest::stage_b_json(&id, &fp, &sink.outputs),
    )?;
    refresh_manifest(root, &id)?;
    Ok((sink.outputs, report))
}

/// Remove converted files that the manifest no longer lists (outputs of an earlier converter
/// version or sniffing rule) and then empty directories. Only done for full runs.
fn prune(root: &Path, id: &str, outputs: &[Output]) -> Result<()> {
    let keep: std::collections::HashSet<&str> =
        outputs.iter().map(|o| o.logical_id.as_str()).collect();
    fn walk(dir: &Path, prefix: &str, keep: &std::collections::HashSet<&str>) -> Result<bool> {
        let mut empty = true;
        for e in std::fs::read_dir(dir).map_err(|e| e.to_string())?.flatten() {
            let rel = format!("{prefix}{}", e.file_name().to_string_lossy());
            if e.file_type().map_err(|e| e.to_string())?.is_dir() {
                if walk(&e.path(), &format!("{rel}/"), keep)? {
                    std::fs::remove_dir(e.path()).map_err(|e| e.to_string())?;
                } else {
                    empty = false;
                }
            } else if keep.contains(rel.as_str()) {
                empty = false;
            } else {
                std::fs::remove_file(e.path()).map_err(|e| e.to_string())?;
            }
        }
        Ok(empty)
    }
    let base = version_dir(root, id)?;
    if base.is_dir() {
        for e in std::fs::read_dir(&base)
            .map_err(|e| e.to_string())?
            .flatten()
        {
            let name = e.file_name().to_string_lossy().to_string();
            if matches!(
                name.as_str(),
                ".source" | ".importer" | "manifest.json" | "manifest.sha256"
            ) {
                continue;
            }
            if e.file_type().map_err(|e| e.to_string())?.is_dir() {
                if walk(&e.path(), &format!("{name}/"), &keep)? {
                    std::fs::remove_dir(e.path()).map_err(|e| e.to_string())?;
                }
            } else if !keep.contains(name.as_str()) {
                std::fs::remove_file(e.path()).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

/// Verify stage A and B files against their manifests. Returns the number of files checked.
pub fn verify(root: &Path, id: &str) -> Result<usize> {
    let version = version_dir(root, id)?;
    let manifest_path = version.join("manifest.json");
    let manifest_text =
        std::fs::read(&manifest_path).map_err(|e| format!("{}: {e}", manifest_path.display()))?;
    let hash_path = version.join("manifest.sha256");
    let declared =
        std::fs::read_to_string(&hash_path).map_err(|e| format!("{}: {e}", hash_path.display()))?;
    if declared.split_whitespace().next().unwrap_or_default() != sha256::hex(&manifest_text) {
        return Err(format!(
            "manifest hash mismatch: {}",
            manifest_path.display()
        ));
    }

    let work = work_dir(root, id)?;
    let stage_a_path = work.join("stage-a.json");
    let stage_a_text = std::fs::read_to_string(&stage_a_path)
        .map_err(|e| format!("{}: {e}", stage_a_path.display()))?;
    let stage_b_path = work.join("stage-b.json");
    let stage_b_text = match std::fs::read_to_string(&stage_b_path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("{}: {e}", stage_b_path.display())),
    };
    if manifest_text != combined_manifest(root, id)?.as_bytes() {
        return Err(format!(
            "manifest content does not match stage manifests: {}",
            manifest_path.display()
        ));
    }
    let mut count = 0;
    for row in manifest::parse_rows(&stage_a_text)? {
        let path = source_dir(root, id)?.join(safe(&row["path"])?);
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if sha256::hex(&bytes) != row["sha256"] || bytes.len().to_string() != row["size"] {
            return Err(format!("source hash/size mismatch: {}", path.display()));
        }
        count += 1;
    }

    let Some(stage_b_text) = stage_b_text else {
        return Ok(count);
    };
    let outputs = manifest::parse_rows(&stage_b_text)?;
    let mut raw_sources = std::collections::HashSet::new();
    for row in &outputs {
        let source_path = &row["source_path"];
        let path = version.join(safe(&row["logical_id"])?);
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if sha256::hex(&bytes) != row["sha256"] || bytes.len().to_string() != row["size"] {
            return Err(format!("output hash/size mismatch: {}", path.display()));
        }
        let original = original_member(root, id, source_path)?;
        let archive_name = source_path.split('#').next().unwrap_or(source_path);
        let archive = std::fs::read(source_dir(root, id)?.join(safe(archive_name)?))
            .map_err(|e| format!("source {archive_name}: {e}"))?;
        if sha256::hex(&archive) != row["source_sha256"] {
            return Err(format!("source hash mismatch for {source_path}"));
        }

        let converter = row["converter"].as_str();
        if converter == RAW_MEMBER.0 || converter == XMV_RAW.0 {
            if bytes != original {
                return Err(format!("raw output differs from source: {source_path}"));
            }
            if converter == RAW_MEMBER.0 {
                raw_sources.insert(source_path.clone());
            }
        } else if row["lossless"] == "true" {
            let expected = match converter {
                value if value == PAK_MEMBER.0 || value == PLAIN_TEXT.0 => original.clone(),
                value if value == BS_JSON.0 || value == BS_SNIFF.0 => {
                    let doc = BsDoc::parse(&original)?;
                    if doc.to_bytes() != original {
                        return Err(format!(
                            "text bank does not re-encode exactly: {source_path}"
                        ));
                    }
                    doc.to_json().into_bytes()
                }
                value if value == XBT_KTX2.0 || value == XBT_PNG.0 => {
                    let (ext, conv, converted, note) = xbt_convert(&original)?;
                    let exact = ext == "ktx2" || note.contains(" 1 mips in source");
                    if !exact || conv.0 != converter {
                        return Err(format!("texture conversion is not lossless: {source_path}"));
                    }
                    converted
                }
                value if structured::converter_known(value) => {
                    structured::rebuild(value, &original)?
                }
                _ => return Err(format!("no lossless verifier for converter {converter}")),
            };
            if bytes != expected {
                return Err(format!(
                    "conversion output differs from source decode: {source_path}"
                ));
            }
        }
        count += 1;
    }
    for row in &outputs {
        if row["lossless"] == "false" && !raw_sources.contains(&row["source_path"]) {
            return Err(format!(
                "lossy conversion has no raw member: {}",
                row["source_path"]
            ));
        }
    }
    Ok(count)
}

fn original_member(root: &Path, id: &str, source_path: &str) -> Result<Vec<u8>> {
    let base = source_dir(root, id)?;
    if let Some((archive_path, key)) = source_path.split_once('#')
        && key.len() == 8
        && u32::from_str_radix(key, 16).is_ok()
    {
        let path = base.join(safe(archive_path)?);
        let archive = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let pak = tsfp_format_pak::Pak::parse(&archive)?;
        let key = u32::from_str_radix(key, 16).map_err(|e| e.to_string())?;
        let index = pak
            .entries
            .iter()
            .position(|entry| entry.key == key)
            .ok_or_else(|| format!("PAK member {key:08x} is missing from {archive_path}"))?;
        let entry = &pak.entries[index];
        let stored = pak.stored(index)?;
        if entry.stored_size != 0 {
            inflate::gunzip(stored, entry.decoded_size as usize)
        } else {
            Ok(stored.to_vec())
        }
    } else {
        let path = base.join(safe(source_path)?);
        std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))
    }
}
