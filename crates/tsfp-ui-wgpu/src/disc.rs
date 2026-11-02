//! Run-time UI assets from the owner's disc image (T2189 UI-6 slice 15): the font tables of
//! `default.xbe` and the string banks. Read-only, nothing is cached or written.
//!
//! The banks are members of `pak/arcade/l_103.pak`, keyed by the CRC32 of their path (MEASURED,
//! `docs/t1760-bs-labels.md`), gzip or stored. Slice 16: the same pak holds the two font textures (A8 XBT members, MEASURED);
//! every A8 member is decoded as a candidate page and matched to the font records by `original_font`.

use std::path::Path;

use tsfp_format_pak::Pak;
use tsfp_import::source::{self, FileEntry, Platform, Source};

use tsfp_ui_model::strings::{Bank, BankStrings, COMMON_ENTRIES, STORY_ENTRIES, bank_paths};
use tsfp_ui_model::xbe::XbeImage;

use crate::original_font::{Page, decode_a8};

pub struct DiscAssets {
    pub image: XbeImage,
    pub strings: BankStrings,
    /// Every A8 texture of the bank pak (font page candidates).
    pub font_pages: Vec<Page>,
}

/// Decoded A8 texture members of `pak` (unnamed: matched by content later).
pub fn a8_members(pak: &Pak) -> Vec<Page> {
    (0..pak.entries.len())
        .filter_map(|index| {
            let entry = &pak.entries[index];
            let stored = pak.stored(index).ok()?;
            if entry.stored_size == 0 {
                decode_a8(stored)
            } else {
                decode_a8(&tsfp_import::inflate::gunzip(stored, entry.decoded_size as usize).ok()?)
            }
        })
        .collect()
}

fn read(disc: &dyn Source, files: &[FileEntry], path: &str) -> Result<Vec<u8>, String> {
    let entry = files
        .iter()
        .find(|f| f.path.eq_ignore_ascii_case(path))
        .ok_or(format!("{path}: not on the disc"))?;
    disc.read(entry).map_err(|e| format!("{path}: {e}"))
}

pub const BANK_PAK: &str = "pak/arcade/l_103.pak";

/// Decoded bytes of the member named `path` (CRC32 key), None when absent or undecodable.
pub fn pak_member(pak: &Pak, path: &str) -> Option<Vec<u8>> {
    let key = tsfp_format_core::crc32(path.as_bytes());
    let index = pak.entries.iter().position(|e| e.key == key)?;
    let entry = &pak.entries[index];
    let stored = pak.stored(index).ok()?;
    if entry.stored_size == 0 {
        return Some(stored.to_vec());
    }
    let out = tsfp_import::inflate::gunzip(stored, entry.decoded_size as usize).ok()?;
    (out.len() == entry.decoded_size as usize).then_some(out)
}

/// `default.xbe` and the banks of `language` (`e f g i s`). The story bank is optional.
pub fn load(disc_path: &Path, language: char) -> Result<DiscAssets, String> {
    let disc = source::open(disc_path, "disc", Platform::Xbox)?;
    let files = disc.list()?;
    let image =
        XbeImage::parse(read(&*disc, &files, "default.xbe")?).ok_or("default.xbe is not an XBE")?;
    let pak =
        Pak::parse(&read(&*disc, &files, BANK_PAK)?).map_err(|e| format!("{BANK_PAK}: {e}"))?;
    let (common, story) = bank_paths(language);
    let common = pak_member(&pak, &common)
        .and_then(|bytes| Bank::parse(&bytes, COMMON_ENTRIES))
        .ok_or(format!(
            "{common}: missing in {BANK_PAK} or shorter than its table"
        ))?;
    let story = pak_member(&pak, &story).and_then(|bytes| Bank::parse(&bytes, STORY_ENTRIES));
    Ok(DiscAssets {
        image,
        strings: BankStrings { common, story },
        font_pages: a8_members(&pak),
    })
}
