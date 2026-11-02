//! SETU entry access: locate the event triple (header +0x90: offset, count, stride 0xC8) and
//! reuse `tsfp-format-pak` to reach SETU entries inside `.pak` files.

use super::record::{EventRecord, RECORD_SIZE};

pub const SETU_MAGIC: &[u8; 4] = b"SETU";
const EVENT_TRIPLE: usize = 0x90;

/// Identifies one SETU row in a runtime PAK table. Duplicate keys are ordered in the source
/// archive, so the occurrence is part of the selector rather than an implicit first-match rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetuAssetSelector {
    pub key: u32,
    pub occurrence: usize,
}

impl SetuAssetSelector {
    pub const fn new(key: u32, occurrence: usize) -> Self {
        Self { key, occurrence }
    }
}

fn word(bytes: &[u8], at: usize) -> Option<usize> {
    let raw = bytes.get(at..at + 4)?;
    Some(u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize)
}

/// Byte range `(start, count)` of the event definitions of one SETU blob.
pub fn event_span(blob: &[u8]) -> Result<(usize, usize), String> {
    if blob.get(..4) != Some(SETU_MAGIC.as_slice()) {
        return Err("not a SETU entry".into());
    }
    let start = word(blob, EVENT_TRIPLE).ok_or("short header")?;
    let count = word(blob, EVENT_TRIPLE + 4).ok_or("short header")?;
    let stride = word(blob, EVENT_TRIPLE + 8).ok_or("short header")?;
    if stride != RECORD_SIZE {
        return Err(format!("event stride {stride:#x}, expected 0xC8"));
    }
    let end = count
        .checked_mul(RECORD_SIZE)
        .and_then(|size| start.checked_add(size))
        .ok_or("event span overflow")?;
    if end > blob.len() {
        return Err("event list past end of entry".into());
    }
    Ok((start, count))
}

pub fn parse_events(blob: &[u8]) -> Result<Vec<EventRecord>, String> {
    let (start, count) = event_span(blob)?;
    (0..count)
        .map(|index| {
            let at = start + index * RECORD_SIZE;
            EventRecord::from_bytes(&blob[at..at + RECORD_SIZE]).ok_or_else(|| "record".to_string())
        })
        .collect()
}

/// Read one uncompressed SETU entry from a runtime PAK image by typed key and duplicate
/// occurrence. This selects asset data only; session setup and terminal state remain separate
/// typed inputs to the Story mission owner.
pub fn setu_entry_from_pak(
    pak_bytes: &[u8],
    selector: SetuAssetSelector,
) -> Result<Vec<u8>, String> {
    let pak = tsfp_format_pak::Pak::parse(pak_bytes).map_err(|error| format!("PAK: {error}"))?;
    let index = pak
        .entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.key == selector.key)
        .nth(selector.occurrence)
        .map(|(index, _)| index)
        .ok_or_else(|| {
            format!(
                "SETU key {:#010x} occurrence {} not found",
                selector.key, selector.occurrence
            )
        })?;
    let entry = &pak.entries[index];
    if entry.stored_size != 0 {
        return Err(format!(
            "SETU key {:#010x} occurrence {} is compressed",
            selector.key, selector.occurrence
        ));
    }
    let payload = pak
        .payload(index)
        .map_err(|error| format!("SETU PAK payload: {error}"))?;
    if payload.get(..4) != Some(SETU_MAGIC.as_slice()) {
        return Err(format!(
            "PAK key {:#010x} occurrence {} is not a SETU entry",
            selector.key, selector.occurrence
        ));
    }
    Ok(payload.to_vec())
}

/// Rewrite the event list of `blob` in place from `records` (same count required).
pub fn write_events(blob: &mut [u8], records: &[EventRecord]) -> Result<(), String> {
    let (start, count) = event_span(blob)?;
    if count != records.len() {
        return Err("record count cannot change".into());
    }
    for (index, record) in records.iter().enumerate() {
        let at = start + index * RECORD_SIZE;
        blob[at..at + RECORD_SIZE].copy_from_slice(&record.to_bytes());
    }
    Ok(())
}

/// Every uncompressed SETU entry of a `.pak` file, via the shared pak parser.
pub fn setu_blobs_from_pak(pak_bytes: &[u8]) -> Result<Vec<Vec<u8>>, String> {
    let pak = tsfp_format_pak::Pak::parse(pak_bytes).map_err(|error| format!("{error:?}"))?;
    let mut blobs = Vec::new();
    for (index, entry) in pak.entries.iter().enumerate() {
        if entry.stored_size != 0 {
            continue;
        }
        if let Ok(payload) = pak.payload(index)
            && payload.get(..4) == Some(SETU_MAGIC.as_slice())
        {
            blobs.push(payload.to_vec());
        }
    }
    Ok(blobs)
}
