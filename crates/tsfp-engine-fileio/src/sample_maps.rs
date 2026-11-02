//! Read-only Map Maker samples discovered from the title's XBE table and PAKs.
//!
//! The title supplies eight `(text id, resource path)` records. Resource paths are
//! resolved with the lifted path-to-PAK rule, then the bounded decoded payloads
//! are parsed into owned map documents. Labels are resolved by the caller from
//! the title's runtime text bank; this module never invents display names.

use std::collections::HashMap;
use std::path::Path;

use crate::map_document::MapDocument;
use crate::pak::PakIndex;
use crate::storage_flow::{MAP_PAYLOAD_BYTES, MapPayload, SampleMapEntry, SampleMaps};
use crate::vfs::{Access, Disposition, NtStatus, Vfs, status};
use crate::xiso::Xiso;

const SAMPLE_TABLE_VA: u32 = 0x52EBB0;
const SAMPLE_COUNT: usize = 8;
const SAMPLE_RECORD_BYTES: usize = 20;
const PAK_PATH_MARKER_VA: u32 = 0x47879C;
const PAK_PATH_MARKER_SKIP: usize = 6;
const MAX_XBE_BYTES: usize = 64 * 1024 * 1024;
const MAX_PAK_TOC_BYTES: usize = 16 * 1024 * 1024;
const MAX_SAMPLE_STORED_BYTES: u64 = 256 * 1024;

#[derive(Clone, Debug)]
struct XbeSection {
    virtual_address: u32,
    virtual_size: u32,
    raw_address: u32,
    raw_size: u32,
}

struct XbeView {
    bytes: Vec<u8>,
    sections: Vec<XbeSection>,
}

impl XbeView {
    fn parse(bytes: Vec<u8>) -> Result<Self, NtStatus> {
        if bytes.len() > MAX_XBE_BYTES || bytes.get(..4) != Some(b"XBEH") {
            return Err(status::UNRECOGNIZED_VOLUME);
        }
        let base = word(&bytes, 0x104).ok_or(status::UNRECOGNIZED_VOLUME)?;
        let count = word(&bytes, 0x11c)
            .ok_or(status::UNRECOGNIZED_VOLUME)?
            .min(64) as usize;
        let table = word(&bytes, 0x120)
            .and_then(|at| at.checked_sub(base))
            .ok_or(status::UNRECOGNIZED_VOLUME)? as usize;
        let mut sections = Vec::with_capacity(count);
        for index in 0..count {
            let at = table
                .checked_add(index.checked_mul(0x38).ok_or(status::UNRECOGNIZED_VOLUME)?)
                .ok_or(status::UNRECOGNIZED_VOLUME)?;
            sections.push(XbeSection {
                virtual_address: word(&bytes, at + 4).ok_or(status::UNRECOGNIZED_VOLUME)?,
                virtual_size: word(&bytes, at + 8).ok_or(status::UNRECOGNIZED_VOLUME)?,
                raw_address: word(&bytes, at + 12).ok_or(status::UNRECOGNIZED_VOLUME)?,
                raw_size: word(&bytes, at + 16).ok_or(status::UNRECOGNIZED_VOLUME)?,
            });
        }
        Ok(Self { bytes, sections })
    }

    fn bytes_at(&self, address: u32, length: usize) -> Option<&[u8]> {
        for section in &self.sections {
            let Some(delta) = address.checked_sub(section.virtual_address) else {
                continue;
            };
            if delta >= section.virtual_size {
                continue;
            }
            let end = delta.checked_add(u32::try_from(length).ok()?)?;
            if end > section.raw_size {
                return None;
            }
            let start = usize::try_from(section.raw_address.checked_add(delta)?).ok()?;
            return self.bytes.get(start..start.checked_add(length)?);
        }
        None
    }

    fn word_at(&self, address: u32) -> Option<u32> {
        let bytes = self.bytes_at(address, 4)?;
        Some(u32::from_le_bytes(bytes.try_into().ok()?))
    }

    fn cstring_at(&self, address: u32) -> Result<Vec<u8>, NtStatus> {
        let mut value = Vec::new();
        for offset in 0..256u32 {
            let byte = self
                .bytes_at(
                    address
                        .checked_add(offset)
                        .ok_or(status::UNRECOGNIZED_VOLUME)?,
                    1,
                )
                .and_then(|bytes| bytes.first().copied())
                .ok_or(status::UNRECOGNIZED_VOLUME)?;
            if byte == 0 {
                return if value.is_empty() {
                    Err(status::UNRECOGNIZED_VOLUME)
                } else {
                    Ok(value)
                };
            }
            if !(0x20..=0x7e).contains(&byte) {
                return Err(status::UNRECOGNIZED_VOLUME);
            }
            value.push(byte);
        }
        Err(status::UNRECOGNIZED_VOLUME)
    }
}

fn word(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

#[derive(Clone, Debug)]
struct SampleResource {
    id: u32,
    text_id: u32,
    key: u32,
}

fn sample_resources(xbe: &XbeView) -> Result<Vec<SampleResource>, NtStatus> {
    let marker = xbe.cstring_at(PAK_PATH_MARKER_VA)?;
    if marker.len() < PAK_PATH_MARKER_SKIP {
        return Err(status::UNRECOGNIZED_VOLUME);
    }
    let mut resources = Vec::with_capacity(SAMPLE_COUNT);
    for index in 0..SAMPLE_COUNT {
        let record = SAMPLE_TABLE_VA
            .checked_add(u32::try_from(index * SAMPLE_RECORD_BYTES).unwrap())
            .ok_or(status::UNRECOGNIZED_VOLUME)?;
        let text_id = xbe
            .word_at(record.checked_add(8).ok_or(status::UNRECOGNIZED_VOLUME)?)
            .ok_or(status::UNRECOGNIZED_VOLUME)?;
        let path_address = xbe
            .word_at(record.checked_add(12).ok_or(status::UNRECOGNIZED_VOLUME)?)
            .ok_or(status::UNRECOGNIZED_VOLUME)?;
        let path = xbe.cstring_at(path_address)?;
        let marker_at =
            find_ascii_case_insensitive(&path, &marker).ok_or(status::UNRECOGNIZED_VOLUME)?;
        let key_start = marker_at
            .checked_add(PAK_PATH_MARKER_SKIP)
            .filter(|&start| start <= path.len())
            .ok_or(status::UNRECOGNIZED_VOLUME)?;
        resources.push(SampleResource {
            id: u32::try_from(index).unwrap(),
            text_id,
            key: tsfp_format_core::crc32(&path[key_start..]),
        });
    }
    Ok(resources)
}

fn find_ascii_case_insensitive(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| {
        window
            .iter()
            .zip(needle)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
    })
}

fn read_file(vfs: &mut Vfs, path: &str, limit: usize) -> Result<Vec<u8>, NtStatus> {
    let handle = vfs.open(path, Access::READ, Disposition::Open)?;
    let result = (|| {
        let size = usize::try_from(vfs.size(handle)?).map_err(|_| status::UNRECOGNIZED_VOLUME)?;
        if size > limit {
            return Err(status::UNRECOGNIZED_VOLUME);
        }
        let mut bytes = vec![0; size];
        if vfs.read_at(handle, 0, &mut bytes)? != size {
            return Err(status::UNRECOGNIZED_VOLUME);
        }
        Ok(bytes)
    })();
    let close = vfs.close(handle);
    match (result, close) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(bytes), Ok(())) => Ok(bytes),
    }
}

fn sample_copies(
    vfs: &mut Vfs,
    paks: &[String],
    resources: &[SampleResource],
) -> Result<HashMap<u32, Vec<Vec<u8>>>, NtStatus> {
    let mut copies: HashMap<u32, Vec<Vec<u8>>> = HashMap::new();
    for path in paks {
        let guest_path = format!("D:\\{path}");
        let Ok(handle) = vfs.open(&guest_path, Access::READ, Disposition::Open) else {
            continue;
        };
        let result = (|| {
            let size = vfs.size(handle)?;
            let mut header = [0u8; 12];
            if size < 12 || vfs.read_at(handle, 0, &mut header)? != header.len() {
                return Ok(());
            }
            if header[..4] != *b"P5CK" {
                return Ok(());
            }
            let toc_size = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
            if toc_size > MAX_PAK_TOC_BYTES || toc_size % 16 != 0 {
                return Ok(());
            }
            let Ok(index) = PakIndex::open(vfs, handle) else {
                return Ok(());
            };
            for resource in resources {
                for entry_index in index.find(resource.key) {
                    let entry = &index.entries[entry_index];
                    let stored_size = u64::from(if entry.stored_size == 0 {
                        entry.decoded_size
                    } else {
                        entry.stored_size
                    });
                    if entry.decoded_size as usize > MAP_PAYLOAD_BYTES
                        || stored_size > MAX_SAMPLE_STORED_BYTES
                    {
                        return Err(status::UNRECOGNIZED_VOLUME);
                    }
                    copies
                        .entry(resource.id)
                        .or_default()
                        .push(index.payload(vfs, entry_index)?);
                }
            }
            Ok(())
        })();
        let close = vfs.close(handle);
        result?;
        close?;
    }
    Ok(copies)
}

/// A bounded, owned view of the title's eight read-only sample maps.
pub struct DiscSampleMaps {
    catalogue: Vec<SampleMapEntry>,
    payloads: HashMap<u32, MapPayload>,
}

impl DiscSampleMaps {
    /// Opens the owner's disc and resolves labels with the current title text bank.
    /// The callback receives the XBE text ID, not a resource path or guest address.
    pub fn open(
        path: &Path,
        mut label_for_text_id: impl FnMut(u32) -> Option<String>,
    ) -> Result<Self, NtStatus> {
        let disc = Xiso::open(path).map_err(|_| status::UNRECOGNIZED_VOLUME)?;
        let files = disc.walk().map_err(|_| status::UNRECOGNIZED_VOLUME)?;
        let paks: Vec<_> = files
            .iter()
            .filter(|(path, entry)| {
                !entry.is_directory() && path.to_ascii_lowercase().ends_with(".pak")
            })
            .map(|(path, _)| path.clone())
            .collect();
        let mut vfs = Vfs::new();
        vfs.mount_disc("\\Device\\CdRom0", disc);
        vfs.add_symlink("D:", "\\Device\\CdRom0");
        let xbe = XbeView::parse(read_file(&mut vfs, "D:\\default.xbe", MAX_XBE_BYTES)?)?;
        let resources = sample_resources(&xbe)?;
        let catalogue = resources
            .iter()
            .map(|resource| {
                let label =
                    label_for_text_id(resource.text_id).ok_or(status::OBJECT_NAME_NOT_FOUND)?;
                SampleMapEntry::new(resource.id, label)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let copies = sample_copies(&mut vfs, &paks, &resources)?;
        let mut payloads = HashMap::with_capacity(SAMPLE_COUNT);
        for resource in resources {
            let found = copies
                .get(&resource.id)
                .filter(|values| !values.is_empty())
                .ok_or(status::OBJECT_NAME_NOT_FOUND)?;
            let first = &found[0];
            if found.iter().any(|copy| copy != first) {
                return Err(status::UNRECOGNIZED_VOLUME);
            }
            let mut bytes = vec![0; MAP_PAYLOAD_BYTES];
            bytes[..first.len()].copy_from_slice(first);
            let payload = MapPayload::new(bytes)?;
            MapDocument::parse(&payload)?;
            payloads.insert(resource.id, payload);
        }
        Ok(Self {
            catalogue,
            payloads,
        })
    }
}

impl SampleMaps for DiscSampleMaps {
    fn load_sample(&mut self, id: u32) -> Result<MapPayload, NtStatus> {
        self.payloads
            .get(&id)
            .cloned()
            .ok_or(status::OBJECT_NAME_NOT_FOUND)
    }

    fn catalogue(&mut self) -> Result<Vec<SampleMapEntry>, NtStatus> {
        Ok(self.catalogue.clone())
    }
}
