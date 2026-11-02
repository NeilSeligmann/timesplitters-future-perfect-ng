//! Which view of a host-directory volume a name reaches (T2106). Ports `names_raw_view`, `hostdir_start_plus_one`, the
//! view branch of `hostdir_resolve_locked` and the FATX magic test of `cache_image_formatted_locked` (`src/xbox/kernel_file.c`).
//!
//! A plain virtual device (a device file, no cache number) is raw for every rest and has nothing inside it. A formattable
//! cache partition is raw only for its exact name (`PartitionN`) and a host directory for anything under it (`PartitionN\`),
//! MEASURED at 0x003813F4 (raw open) against 0x00380D3F (directory open). The directory view is served only while the raw
//! image starts with the title's "FATX" magic (written first at 0x00381574).

/// What the C volume record says about a volume (`volume_entry`), reduced to the facts the choice reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeFacts {
    /// `device_file[0] != '\0'`: the volume has a raw image.
    pub has_device: bool,
    /// The N of `PartitionN`, 0 for anything that is not a cache partition.
    pub cache_partition: u32,
}

/// The view a rest names. Discriminants are the C ABI values (`KERNEL_FILE_VIEW_*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum View {
    /// An ordinary host directory, walked from the volume root.
    Directory = 0,
    /// A cache partition's directory view, walked from its own content directory once the image is formatted.
    CacheDirectory = 1,
    /// The raw device itself.
    RawDevice = 2,
    /// A name inside a raw device, which has no namespace: not found.
    InsideRawDevice = 3,
}

impl View {
    /// `names_raw_view`: both raw answers.
    pub fn is_raw(self) -> bool {
        matches!(self, View::RawDevice | View::InsideRawDevice)
    }

    /// `hostdir_start_plus_one`: true when the walk starts from the content directory.
    pub fn walks_from_content(self) -> bool {
        self == View::CacheDirectory
    }
}

/// Pick the view `rest` (what follows the matched volume prefix) reaches on `volume`.
pub fn choose(volume: VolumeFacts, rest: &[u8]) -> View {
    let cache = volume.cache_partition != 0;
    if volume.has_device && (!cache || rest.is_empty()) {
        return if rest.is_empty() { View::RawDevice } else { View::InsideRawDevice };
    }
    if cache && !rest.is_empty() { View::CacheDirectory } else { View::Directory }
}

/// Does the image's head (the first bytes read from offset 0) carry the FATX magic? Fewer than four bytes is unformatted.
pub fn image_formatted(head: &[u8]) -> bool {
    head.len() >= 4 && &head[..4] == b"FATX"
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAIN: VolumeFacts = VolumeFacts { has_device: false, cache_partition: 0 };
    const DEVICE: VolumeFacts = VolumeFacts { has_device: true, cache_partition: 0 };
    const CACHE: VolumeFacts = VolumeFacts { has_device: true, cache_partition: 3 };

    #[test]
    fn plain_directory_is_a_directory_everywhere() {
        assert_eq!(choose(PLAIN, b""), View::Directory);
        assert_eq!(choose(PLAIN, b"\\a"), View::Directory);
    }

    #[test]
    fn virtual_device_is_raw_and_empty() {
        assert_eq!(choose(DEVICE, b""), View::RawDevice);
        assert_eq!(choose(DEVICE, b"\\"), View::InsideRawDevice);
    }

    #[test]
    fn cache_partition_splits_on_one_separator() {
        assert_eq!(choose(CACHE, b""), View::RawDevice);
        assert_eq!(choose(CACHE, b"\\"), View::CacheDirectory);
        assert!(choose(CACHE, b"\\x").walks_from_content());
        assert!(!choose(CACHE, b"").walks_from_content());
    }

    #[test]
    fn magic_needs_four_exact_bytes() {
        assert!(image_formatted(b"FATX\0\0"));
        assert!(!image_formatted(b"FAT"));
        assert!(!image_formatted(b"fatx"));
        assert!(!image_formatted(b""));
    }
}
