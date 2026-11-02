//! File I/O, disc and data loading (T2047, step S1 of docs/decisions/rust-only-build.md).
//! Replace-class system: XDVDFS reader, guest path mapping, handle API, streaming PAK access,
//! profile save I/O. The owner's disc is only ever read at run time from a path argument.
pub mod asset_requests;
pub mod assets;
pub mod commit;
pub mod desc;
pub mod dirpick;
pub mod disposition;
pub mod fatx;
pub mod fatx_img;
pub mod glue;
pub mod handles;
pub mod hostdir;
pub mod info;
pub mod local_storage;
pub mod map_document;
pub mod map_history;
pub mod map_nodes;
pub mod map_settings;
pub mod namespace;
pub mod nt;
pub mod pak;
pub mod payload;
pub mod profile_defaults;
pub mod profile_input;
pub mod sample_maps;
pub mod save;
pub mod storage_bootstrap;
pub mod storage_flow;
pub mod storage_frontend;
pub mod storage_lifecycle;
pub mod storage_route;
pub mod trace_roots;
pub mod vfs;
pub mod views;
pub mod xiso;

/// Advances the 64-bit FNV-1a checksum used by the dump tool.
///
/// Pass the returned state to the next call when data arrives in chunks. The input is
/// hashed as raw bytes; callers must choose the byte order of structured values.
///
/// ```
/// use tsfp_engine_fileio::{fnv1a64, FNV_OFFSET};
/// let whole = fnv1a64(FNV_OFFSET, b"disc path");
/// let first = fnv1a64(FNV_OFFSET, b"disc ");
/// assert_eq!(whole, fnv1a64(first, b"path"));
/// ```
pub fn fnv1a64(state: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(state, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}
pub const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

pub mod async_io;

pub mod file_object;

pub mod kernel_io;

pub mod guest_names;

pub mod acquisition;

pub mod create;

pub mod lifecycle;

pub mod activity;

#[cfg(test)]
mod checksum_tests {
    use super::{FNV_OFFSET, fnv1a64};

    #[test]
    fn checksum_chunks_and_byte_order() {
        // Synthetic data checks the dump-tool checksum contract without a retail disc.
        for length in 0..=256 {
            let bytes: Vec<u8> = (0..length).map(|i| (i * 37 + 11) as u8).collect();
            let whole = fnv1a64(FNV_OFFSET, &bytes);
            for split in [0, length / 2, length] {
                let head = fnv1a64(FNV_OFFSET, &bytes[..split]);
                assert_eq!(fnv1a64(head, &bytes[split..]), whole);
            }
            assert_eq!(fnv1a64(whole, &[]), whole);
        }
        assert_eq!(fnv1a64(FNV_OFFSET, b"abc"), 0xe71f_a219_0541_574b);
        assert_ne!(
            fnv1a64(FNV_OFFSET, &[0x01, 0x02]),
            fnv1a64(FNV_OFFSET, &[0x02, 0x01])
        );
    }
}

pub mod profile_progress;
pub mod story_progress;
