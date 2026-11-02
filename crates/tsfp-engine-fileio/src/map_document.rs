//! Owned serialized editor document. Chunk framing follows T1717; record payload
//! semantics and pointer/index reconstruction remain the editor's responsibility.
use crate::map_nodes::{MapNodes, NODES, WireTransform};
use crate::map_settings::MapSettings;
use crate::storage_flow::{MAP_PAYLOAD_BYTES, MapPayload};
use crate::vfs::{NtStatus, status};
const MAGIC: u32 = 0xc77667;
const VERSION: u32 = 301;
pub const SETTINGS: u32 = 0xc77668;
const END: u32 = 0xc7766a;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub tag: u32,
    pub data: Vec<u8>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapDocument {
    chunks: Vec<Chunk>,
    // Immutable codec cache and typed editor state are updated together.
    settings: MapSettings,
    nodes: MapNodes,
}
impl MapDocument {
    pub fn chunks(&self) -> &[Chunk] {
        &self.chunks
    }
    /// Native editor title in the original 32-byte settings envelope (T1717).
    /// The native UTF-8 contract is explicit; original undecodable bytes remain
    /// preserved until the caller edits the title. No guest pointer is exposed.
    pub fn with_title(&self, title: &str) -> Result<Self, NtStatus> {
        let mut document = self.clone();
        document.settings = self.settings.with_title(title)?;
        document.chunks[0].data = document.settings.encode()?;
        document.serialize()?;
        Ok(document)
    }
    /// Apply an accepted Rust-owned Map Description edit to the typed settings
    /// record while retaining unrelated settings and chunks.
    pub fn with_description(&self, description: &str) -> Result<Self, NtStatus> {
        let mut document = self.clone();
        document.settings = self.settings.with_description(description)?;
        document.chunks[0].data = document.settings.encode()?;
        document.serialize()?;
        Ok(document)
    }
    pub fn validate_title(title: &str) -> Result<(), NtStatus> {
        MapSettings::validate_title(title)
    }
    pub fn nodes(&self) -> &MapNodes {
        &self.nodes
    }
    pub fn with_node_transform(
        &self,
        index: usize,
        transform: WireTransform,
    ) -> Result<Self, NtStatus> {
        let mut document = self.clone();
        document.nodes = self.nodes.with_transform(index, transform)?;
        let chunk = document
            .chunks
            .iter_mut()
            .find(|chunk| chunk.tag == NODES)
            .ok_or(status::INVALID_PARAMETER)?;
        chunk.data = document.nodes.encode();
        document.serialize()?;
        Ok(document)
    }
    pub fn settings(&self) -> &MapSettings {
        &self.settings
    }
    /// Raw original bytes are a lossless boundary, not assumed UTF-8 text.
    pub fn title_bytes(&self) -> &[u8] {
        self.settings.title_bytes()
    }
    /// Original bytes are retained losslessly until the field is edited.
    pub fn description_bytes(&self) -> &[u8] {
        self.settings.description_bytes()
    }
    pub fn from_chunks(chunks: Vec<Chunk>) -> Result<Self, NtStatus> {
        let settings = chunks
            .first()
            .filter(|chunk| chunk.tag == SETTINGS)
            .ok_or(status::INVALID_PARAMETER)
            .and_then(|chunk| MapSettings::decode(&chunk.data))?;
        let mut node_chunks = chunks.iter().filter(|chunk| chunk.tag == NODES);
        let data = node_chunks
            .next()
            .map_or(&[][..], |chunk| chunk.data.as_slice());
        if node_chunks.next().is_some() {
            return Err(status::INVALID_PARAMETER);
        }
        let nodes = MapNodes::decode(data, settings.node_count)?;
        let document = Self {
            chunks,
            settings,
            nodes,
        };
        document.serialize()?;
        Ok(document)
    }
    pub fn parse(payload: &MapPayload) -> Result<Self, NtStatus> {
        let bytes = payload.bytes();
        let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
        if word(0) != MAGIC || word(4) != VERSION {
            return Err(status::UNRECOGNIZED_VOLUME);
        }
        let mut at = 8;
        let mut chunks = Vec::new();
        while at + 8 <= bytes.len() {
            let tag = word(at);
            let length = word(at + 4) as usize;
            if tag == END {
                if length != at {
                    return Err(status::UNRECOGNIZED_VOLUME);
                }
                let document =
                    Self::from_chunks(chunks).map_err(|_| status::UNRECOGNIZED_VOLUME)?;
                let settings = &document.chunks[0].data;
                if u32::from_le_bytes(settings[0x248..0x24c].try_into().unwrap()) as usize != at + 8
                {
                    return Err(status::UNRECOGNIZED_VOLUME);
                }
                return Ok(document);
            }
            let end = at
                .checked_add(8)
                .and_then(|v| v.checked_add(length))
                .filter(|&v| v <= bytes.len())
                .ok_or(status::UNRECOGNIZED_VOLUME)?;
            chunks.push(Chunk {
                tag,
                data: bytes[at + 8..end].to_vec(),
            });
            at = end;
        }
        Err(status::UNRECOGNIZED_VOLUME)
    }
    /// Local saves carry an inner CRC in addition to outer title authentication.
    /// Earlier native zero-checksum files remain readable and upgrade on save.
    /// Disc samples use `parse`, matching the original sample-path CRC bypass.
    pub fn parse_local(payload: &MapPayload) -> Result<Self, NtStatus> {
        let document = Self::parse(payload)?;
        let bytes = payload.bytes();
        let used = u32::from_le_bytes(bytes[0x258..0x25c].try_into().unwrap()) as usize;
        let stored = bytes
            .get(used..used + 4)
            .map(|word| u32::from_le_bytes(word.try_into().unwrap()))
            .ok_or(status::UNRECOGNIZED_VOLUME)?;
        if stored != 0 && stored != crc32(&bytes[..used]) {
            return Err(status::UNRECOGNIZED_VOLUME);
        }
        Ok(document)
    }
    pub fn serialize(&self) -> Result<MapPayload, NtStatus> {
        if self
            .chunks
            .first()
            .is_none_or(|c| c.tag != SETTINGS || c.data.len() != 0x3dc)
            || self.chunks.iter().skip(1).any(|c| c.tag == SETTINGS)
            || self.chunks.iter().any(|c| c.tag == END)
        {
            return Err(status::INVALID_PARAMETER);
        }
        let mut length = 16usize; // prefix + terminator
        for chunk in &self.chunks {
            length = length
                .checked_add(8)
                .and_then(|v| v.checked_add(chunk.data.len()))
                .filter(|&v| v <= MAP_PAYLOAD_BYTES - 4)
                .ok_or(status::INVALID_PARAMETER)?;
        }
        let mut bytes = vec![0; MAP_PAYLOAD_BYTES];
        bytes[..4].copy_from_slice(&MAGIC.to_le_bytes());
        bytes[4..8].copy_from_slice(&VERSION.to_le_bytes());
        let mut at = 8;
        for chunk in &self.chunks {
            bytes[at..at + 4].copy_from_slice(&chunk.tag.to_le_bytes());
            bytes[at + 4..at + 8].copy_from_slice(&(chunk.data.len() as u32).to_le_bytes());
            bytes[at + 8..at + 8 + chunk.data.len()].copy_from_slice(&chunk.data);
            at += 8 + chunk.data.len();
        }
        bytes[0x258..0x25c].copy_from_slice(&(length as u32).to_le_bytes());
        bytes[at..at + 4].copy_from_slice(&END.to_le_bytes());
        bytes[at + 4..at + 8].copy_from_slice(&(at as u32).to_le_bytes());
        // 0x32C71A..0x32C731: checksum excludes itself and is inside
        // the fixed 0x7fe0 envelope, before the outer title signature.
        let crc = crc32(&bytes[..length]);
        bytes[length..length + 4].copy_from_slice(&crc.to_le_bytes());
        MapPayload::new(bytes)
    }
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = flate2::Crc::new();
    crc.update(bytes);
    crc.sum()
}
