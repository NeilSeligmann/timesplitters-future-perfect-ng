//! Owned C77669 records (T1717/T2319), not guest pointers or runtime nodes.
//! Tier 0 + live-file conformance; world-coordinate and descriptor semantics
//! remain separate contracts. Iterator offsets: 0x32A733/0x32A750.
use crate::vfs::{NtStatus, status};

pub const NODES: u32 = 0xc77669;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DescriptorId(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WireTransform {
    pub position: [u8; 3],
    pub rotation: u8,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeEntry {
    kind: i16,
    values: [u16; 2],
    parameters: [u8; 3],
    words: Vec<[u8; 4]>,
    retained: [u8; 24],
}
impl NodeEntry {
    pub fn kind(&self) -> i16 {
        self.kind
    }
    pub fn values(&self) -> [u16; 2] {
        self.values
    }
    pub fn parameters(&self) -> [u8; 3] {
        self.parameters
    }
    pub fn words(&self) -> &[[u8; 4]] {
        &self.words
    }
    fn encode_into(&self, output: &mut Vec<u8>) {
        let mut header = self.retained;
        header[0xe..0x10].copy_from_slice(&self.kind.to_le_bytes());
        header[0x10..0x12].copy_from_slice(&self.values[0].to_le_bytes());
        header[0x12..0x14].copy_from_slice(&self.values[1].to_le_bytes());
        header[0x14] = self.words.len() as u8;
        header[0x15..0x18].copy_from_slice(&self.parameters);
        output.extend(header);
        for word in &self.words {
            output.extend(word);
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeGroup {
    descriptor: DescriptorId,
    transform: WireTransform,
    classification: u8,
    entries: Vec<NodeEntry>,
    retained: Vec<u8>,
}
impl NodeGroup {
    pub fn descriptor(&self) -> DescriptorId {
        self.descriptor
    }
    pub fn transform(&self) -> WireTransform {
        self.transform
    }
    pub fn classification(&self) -> u8 {
        self.classification
    }
    pub fn entries(&self) -> &[NodeEntry] {
        &self.entries
    }
    pub fn prefix_bytes(&self) -> &[u8] {
        &self.retained
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapNodes {
    groups: Vec<NodeGroup>,
}
impl MapNodes {
    pub fn groups(&self) -> &[NodeGroup] {
        &self.groups
    }
    pub fn decode(bytes: &[u8], count: u32) -> Result<Self, NtStatus> {
        let invalid = || status::UNRECOGNIZED_VOLUME;
        let count = usize::try_from(count).map_err(|_| invalid())?;
        if count > bytes.len() / 40 {
            return Err(invalid());
        }
        let mut groups = Vec::with_capacity(count);
        let mut at = 0usize;
        for _ in 0..count {
            let header = bytes
                .get(at..at.checked_add(40).ok_or_else(invalid)?)
                .ok_or_else(invalid)?;
            let displacement = i16::from_le_bytes([header[0x24], header[0x25]]);
            if displacement < 40 || displacement % 8 != 0 {
                return Err(invalid());
            }
            let end = at.checked_add(displacement as usize).ok_or_else(invalid)?;
            let retained = bytes.get(at..end).ok_or_else(invalid)?.to_vec();
            let n = u16::from_le_bytes([header[0x1e], header[0x1f]]) as usize;
            at = end;
            if n > (bytes.len() - at) / 24 {
                return Err(invalid());
            }
            let mut entries = Vec::with_capacity(n);
            for _ in 0..n {
                let header = bytes
                    .get(at..at.checked_add(24).ok_or_else(invalid)?)
                    .ok_or_else(invalid)?;
                let size = 24 + 4 * header[0x14] as usize;
                let end = at.checked_add(size).ok_or_else(invalid)?;
                let data = bytes.get(at..end).ok_or_else(invalid)?;
                entries.push(NodeEntry {
                    kind: i16::from_le_bytes([header[0xe], header[0xf]]),
                    values: [
                        u16::from_le_bytes([header[0x10], header[0x11]]),
                        u16::from_le_bytes([header[0x12], header[0x13]]),
                    ],
                    parameters: header[0x15..0x18].try_into().unwrap(),
                    words: data[24..]
                        .chunks_exact(4)
                        .map(|word| word.try_into().unwrap())
                        .collect(),
                    retained: header.try_into().unwrap(),
                });
                at = end;
            }
            groups.push(NodeGroup {
                descriptor: DescriptorId(u32::from_le_bytes(retained[..4].try_into().unwrap())),
                transform: WireTransform {
                    position: retained[4..7].try_into().unwrap(),
                    rotation: retained[7],
                },
                classification: retained[8],
                entries,
                retained,
            });
        }
        if at != bytes.len() {
            return Err(invalid());
        }
        Ok(Self { groups })
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut output = Vec::new();
        for group in &self.groups {
            let mut prefix = group.retained.clone();
            prefix[..4].copy_from_slice(&group.descriptor.0.to_le_bytes());
            prefix[4..7].copy_from_slice(&group.transform.position);
            prefix[7] = group.transform.rotation;
            prefix[8] = group.classification;
            prefix[0x1e..0x20].copy_from_slice(&(group.entries.len() as u16).to_le_bytes());
            let displacement = prefix.len() as i16;
            prefix[0x24..0x26].copy_from_slice(&displacement.to_le_bytes());
            output.extend(prefix);
            for entry in &group.entries {
                entry.encode_into(&mut output);
            }
        }
        output
    }
    /// Wire-space edit only: no invented conversion to world coordinates, no
    /// descriptor replacement or cross-record index changes.
    pub fn with_transform(&self, index: usize, transform: WireTransform) -> Result<Self, NtStatus> {
        let mut changed = self.clone();
        let group = changed
            .groups
            .get_mut(index)
            .ok_or(status::INVALID_PARAMETER)?;
        group.transform = transform;
        group.retained[4..7].copy_from_slice(&transform.position);
        group.retained[7] = transform.rotation;
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Vec<u8> {
        let mut bytes = vec![0xa5; 48];
        bytes[..4].copy_from_slice(&42u32.to_le_bytes());
        bytes[4..8].copy_from_slice(&[1, 2, 255, 7]);
        bytes[0x1e..0x20].copy_from_slice(&1u16.to_le_bytes());
        bytes[0x24..0x26].copy_from_slice(&48i16.to_le_bytes());
        let mut entry = vec![0x53; 28];
        entry[0xe..0x10].copy_from_slice(&(-1i16).to_le_bytes());
        entry[0x14] = 1;
        bytes.extend(entry);
        let mut second = vec![0xb6; 40];
        second[0x1e..0x20].fill(0);
        second[0x24..0x26].copy_from_slice(&40i16.to_le_bytes());
        bytes.extend(second);
        bytes
    }
    #[test]
    fn owned_records_preserve_unknown_bytes_and_edit_only_transform() {
        let bytes = fixture();
        let nodes = MapNodes::decode(&bytes, 2).unwrap();
        assert_eq!(nodes.encode(), bytes);
        assert_eq!(nodes.groups()[0].descriptor(), DescriptorId(42));
        assert_eq!(nodes.groups()[0].entries()[0].kind(), -1);
        assert_eq!(nodes.groups()[0].entries()[0].words(), &[[0x53; 4]]);
        let transform = WireTransform {
            position: [9, 8, 7],
            rotation: 6,
        };
        let changed = nodes.with_transform(0, transform).unwrap();
        let mut expected = bytes.clone();
        expected[4..8].copy_from_slice(&[9, 8, 7, 6]);
        assert_eq!(changed.encode(), expected);
        assert_eq!(MapNodes::decode(&expected, 2).unwrap(), changed);
        assert_eq!(nodes.encode(), bytes);
        assert!(nodes.with_transform(2, transform).is_err());
    }
    #[test]
    fn malformed_counts_offsets_and_truncation_are_rejected() {
        let original = fixture();
        for displacement in [-8i16, 32, 41, 32760] {
            let mut bytes = original.clone();
            bytes[0x24..0x26].copy_from_slice(&displacement.to_le_bytes());
            assert!(MapNodes::decode(&bytes, 2).is_err());
        }
        let mut bytes = original.clone();
        bytes[48 + 0x14] = 255;
        assert!(MapNodes::decode(&bytes, 2).is_err());
        assert!(MapNodes::decode(&original, 1).is_err());
        assert!(MapNodes::decode(&original, u32::MAX).is_err());
        assert!(MapNodes::decode(&original[..original.len() - 1], 2).is_err());
        assert!(MapNodes::decode(&[], 0).unwrap().groups().is_empty());
    }
}
