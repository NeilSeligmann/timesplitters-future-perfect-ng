//! T2225 guest ANSI name and rename boundary. Original C host policy, not Xbox proof.
//! Guest addresses stay at the memory boundary; names are bytes, never UTF-8.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Address(pub u32);
impl Address {
    pub fn field(self, offset: u32) -> Self {
        Self(if self.0 == 0 {
            0
        } else {
            self.0.checked_add(offset).unwrap_or(0)
        })
    }
}
pub trait NameMemory {
    fn byte(&mut self, address: Address) -> Option<u8>;
    fn word(&mut self, address: Address) -> Option<u32>;
}
/// Read ANSI_STRING (length at 0, buffer at 4). MaximumLength is ignored by C.
/// Retain partial output on failed reads and append NUL only after success.
///
/// A zero-length name still needs a valid, nonzero buffer address and output space.
pub fn read_name(memory: &mut impl NameMemory, address: Address, out: &mut [u8]) -> bool {
    let Some(low) = memory.byte(address.field(0)) else {
        return false;
    };
    let Some(high) = memory.byte(address.field(1)) else {
        return false;
    };
    let length = usize::from(u16::from_le_bytes([low, high]));
    let Some(buffer) = memory.word(address.field(4)) else {
        return false;
    };
    if buffer == 0 || length >= out.len() {
        return false;
    }
    for (index, slot) in out[..length].iter_mut().enumerate() {
        let Some(value) = memory.byte(Address(buffer).field(index as u32)) else {
            return false;
        };
        *slot = value;
    }
    out[length] = 0;
    true
}
pub const INFO_LENGTH_MISMATCH: u32 = 0xc0000004;
pub const INVALID_PARAMETER: u32 = 0xc000000d;
pub const NOT_IMPLEMENTED: u32 = 0xc0000002;
pub trait RenameHost: NameMemory {
    fn relative_refused(&mut self, root: u32, name: &[u8]);
    fn rename(&mut self, handle: u32, replace: bool, name: &[u8]) -> u32;
}
/// FILE_RENAME_INFORMATION embeds an ANSI_STRING at +8 in the existing host.
/// This preserves that inferred ABI, length gate and absolute-only root policy.
pub fn rename_from_guest(
    host: &mut impl RenameHost,
    handle: u32,
    address: Address,
    length: u32,
) -> u32 {
    if length < 16 || address.0 == 0 {
        return INFO_LENGTH_MISMATCH;
    }
    let Some(replace) = host.byte(address) else {
        return INFO_LENGTH_MISMATCH;
    };
    let Some(root) = host.word(address.field(4)) else {
        return INFO_LENGTH_MISMATCH;
    };
    let mut target = [0; 261];
    if !read_name(host, address.field(8), &mut target) {
        return INVALID_PARAMETER;
    }
    let end = target.iter().position(|b| *b == 0).unwrap_or(target.len());
    if root != 0 {
        host.relative_refused(root, &target[..end]);
        return NOT_IMPLEMENTED;
    }
    host.rename(handle, replace != 0, &target[..end])
}

/// Fields in the title's 12-byte OBJECT_ATTRIBUTES, not desktop NT's layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributeField {
    RootDirectory,
    ObjectName,
    Attributes,
}
/// Publish each successful word immediately. A failed read leaves that field
/// and all later outputs untouched; it also prevents later memory callbacks.
pub fn read_attributes(
    memory: &mut impl NameMemory,
    address: Address,
    mut available: impl FnMut(AttributeField) -> bool,
    mut publish: impl FnMut(AttributeField, u32),
) -> bool {
    for (field, offset) in [
        (AttributeField::RootDirectory, 0),
        (AttributeField::ObjectName, 4),
        (AttributeField::Attributes, 8),
    ] {
        if !available(field) {
            return false;
        }
        let Some(value) = memory.word(address.field(offset)) else {
            return false;
        };
        publish(field, value);
    }
    true
}

/// Existing host's bounded, INFERRED pseudo-directory policy. Root zero is
/// unchanged; only 0xfffffffd plus an absolute ASCII drive path is expanded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootPath {
    Unchanged,
    DriveAbsolute,
    Refused,
}
pub fn root_path(root: u32, path: &[u8], capacity: usize) -> RootPath {
    if root == 0 {
        return RootPath::Unchanged;
    }
    if root != 0xfffffffd
        || path.len() < 2
        || !path[0].is_ascii_alphabetic()
        || path[1] != b':'
        || (path.len() > 2 && !matches!(path[2], b'\\' | b'/'))
        || path
            .len()
            .checked_add(4)
            .is_none_or(|length| length >= capacity)
    {
        return RootPath::Refused;
    }
    let components = &path[path.len().min(3)..];
    if components
        .split(|byte| matches!(byte, b'\\' | b'/'))
        .any(|component| component == b"." || component == b"..")
    {
        return RootPath::Refused;
    }
    RootPath::DriveAbsolute
}

/// T3786: a live DIRECTORY handle as root. XAPI's recursive delete (`_XapiNukeDirectoryFromHandle@8` 0x3817A4,
/// relative NtOpenFile at 0x38183D; reached from XDeleteSaveGame 0x37E37D) opens each entry of the save directory
/// by its bare name relative to the directory handle. The result is the directory's guest path, one separator and
/// the relative name. `None` refuses: an empty or absolute name (leading separator or a drive), a `.` or `..`
/// component, an empty directory path, or a result (plus NUL) that does not fit `capacity`.
pub fn join_directory_root(directory: &[u8], name: &[u8], capacity: usize) -> Option<Vec<u8>> {
    let separator = |byte: &u8| matches!(byte, b'\\' | b'/');
    let trimmed = directory
        .strip_suffix(b"\\")
        .or_else(|| directory.strip_suffix(b"/"))
        .unwrap_or(directory);
    if trimmed.is_empty()
        || name.is_empty()
        || name.first().is_some_and(separator)
        || name.contains(&b':')
        || name.contains(&0)
        || name
            .split(separator)
            .any(|component| component.is_empty() || component == b"." || component == b"..")
    {
        return None;
    }
    let mut joined = Vec::with_capacity(trimmed.len() + 1 + name.len());
    joined.extend_from_slice(trimmed);
    joined.push(b'\\');
    joined.extend_from_slice(name);
    (joined.len() < capacity).then_some(joined)
}

#[cfg(test)]
mod directory_root_tests {
    use super::join_directory_root;

    #[test]
    fn a_save_directory_handle_roots_its_entries() {
        let joined =
            join_directory_root(b"\\??\\U:\\17A81BE8F2A4\\", b"SaveMeta.xbx", 260).unwrap();
        assert_eq!(joined, b"\\??\\U:\\17A81BE8F2A4\\SaveMeta.xbx".to_vec());
        let joined = join_directory_root(b"\\??\\U:\\17A81BE8F2A4", b"sub\\file", 260).unwrap();
        assert_eq!(joined, b"\\??\\U:\\17A81BE8F2A4\\sub\\file".to_vec());
    }

    #[test]
    fn escapes_absolute_names_and_overflow_are_refused() {
        for name in [
            b"".as_slice(),
            b"\\x",
            b"/x",
            b"..",
            b"a\\..\\b",
            b".",
            b"D:\\x",
            b"a\\\\b",
            b"a\0b",
        ] {
            assert_eq!(
                join_directory_root(b"\\??\\U:\\SAVE", name, 260),
                None,
                "{name:?}"
            );
        }
        assert_eq!(join_directory_root(b"", b"x", 260), None);
        let directory = b"\\??\\U:\\SAVE";
        let fits = directory.len() + 1 + 1;
        assert!(join_directory_root(directory, b"x", fits + 1).is_some());
        assert_eq!(join_directory_root(directory, b"x", fits), None);
    }
}

#[cfg(test)]
mod attribute_tests {
    use super::*;
    #[test]
    fn failed_words_stop_without_publishing() {
        struct Memory {
            failed: usize,
            calls: Vec<u32>,
        }
        impl NameMemory for Memory {
            fn byte(&mut self, _: Address) -> Option<u8> {
                unreachable!()
            }
            fn word(&mut self, address: Address) -> Option<u32> {
                self.calls.push(address.0);
                (self.calls.len() != self.failed).then_some(address.0)
            }
        }
        for failed in 1..=4 {
            let mut memory = Memory {
                failed,
                calls: Vec::new(),
            };
            let mut outputs = Vec::new();
            let success = read_attributes(
                &mut memory,
                Address(100),
                |_| true,
                |field, value| outputs.push((field, value)),
            );
            assert_eq!(success, failed == 4);
            assert_eq!(memory.calls, [100, 104, 108][..failed.min(3)]);
            assert_eq!(outputs.len(), (failed - 1).min(3));
            for (index, (_, value)) in outputs.iter().enumerate() {
                assert_eq!(*value, 100 + 4 * index as u32);
            }
        }
    }
}

#[cfg(test)]
mod name_boundary_tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Memory {
        bytes: BTreeMap<u32, u8>,
        words: BTreeMap<u32, u32>,
    }
    impl NameMemory for Memory {
        fn byte(&mut self, address: Address) -> Option<u8> {
            self.bytes.get(&address.0).copied()
        }
        fn word(&mut self, address: Address) -> Option<u32> {
            self.words.get(&address.0).copied()
        }
    }

    #[test]
    fn names_preserve_partial_output_and_require_nul_space() {
        for length in 0..=32u16 {
            let mut memory = Memory::default();
            memory.bytes.insert(100, length as u8);
            memory.bytes.insert(101, 0);
            memory.words.insert(104, 200);
            for index in 0..u32::from(length) {
                memory
                    .bytes
                    .insert(200 + index, (index as u8).wrapping_mul(7));
            }
            let mut output = vec![0xa5; usize::from(length) + 2];
            assert!(read_name(&mut memory, Address(100), &mut output));
            for index in 0..usize::from(length) {
                assert_eq!(output[index], (index as u8).wrapping_mul(7));
            }
            assert_eq!(output[usize::from(length)], 0);
            assert_eq!(output[usize::from(length) + 1], 0xa5);

            let mut short = vec![0xa5; usize::from(length)];
            assert!(!read_name(&mut memory, Address(100), &mut short));
            assert!(short.iter().all(|byte| *byte == 0xa5));

            if length > 0 {
                memory.bytes.remove(&(200 + u32::from(length) - 1));
                let mut partial = vec![0xa5; usize::from(length) + 1];
                assert!(!read_name(&mut memory, Address(100), &mut partial));
                assert!(
                    partial[usize::from(length) - 1..]
                        .iter()
                        .all(|byte| *byte == 0xa5)
                );
            }
        }
    }

    #[test]
    fn root_path_rejects_components_and_capacity_boundary() {
        for length in 2..=64 {
            let mut path = b"D:\\".to_vec();
            path.extend(std::iter::repeat_n(b'a', length - 2));
            assert_eq!(
                root_path(0xfffffffd, &path, path.len() + 4),
                RootPath::Refused
            );
            assert_eq!(
                root_path(0xfffffffd, &path, path.len() + 5),
                RootPath::DriveAbsolute
            );
            assert_eq!(root_path(0, &path, 0), RootPath::Unchanged);
        }
        for path in [
            b"D:\\..\\file".as_slice(),
            b"d:/./file",
            b"D:../file",
            b"1:/file",
        ] {
            assert_eq!(root_path(0xfffffffd, path, 1024), RootPath::Refused);
        }
        assert_eq!(root_path(0xfffffffc, b"D:/file", 1024), RootPath::Refused);
    }
}
