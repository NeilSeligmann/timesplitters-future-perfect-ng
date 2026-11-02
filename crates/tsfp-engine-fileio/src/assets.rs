//! Host-free archive loading over the file I/O contract. The caller supplies guest paths and
//! entry keys; the loader owns no disc data and keeps no archive handles between calls.
use crate::pak::PakIndex;
use crate::vfs::{Access, Disposition, NtStatus, Vfs, status};

/// Selects one entry when a PAK contains the same key more than once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssetKey {
    pub key: u32,
    pub occurrence: usize,
}

/// Open `path` as a PAK, run `load` on the selected entry and always close the handle. Shared by
/// [`AssetLoader`] and by callers that already own a `Vfs` (the Story mission runtime).
pub fn with_entry<T>(
    vfs: &mut Vfs,
    path: &str,
    selected: AssetKey,
    load: impl FnOnce(&PakIndex, &mut Vfs, usize) -> Result<T, NtStatus>,
) -> Result<T, NtStatus> {
    let handle = vfs.open(path, Access::READ, Disposition::Open)?;
    let result = (|| {
        let pak = PakIndex::open(vfs, handle)?;
        let index = pak
            .find(selected.key)
            .nth(selected.occurrence)
            .ok_or(status::OBJECT_NAME_NOT_FOUND)?;
        load(&pak, vfs, index)
    })();
    let closed = vfs.close(handle);
    match (result, closed) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Ok(value), Ok(())) => Ok(value),
    }
}

/// Decoded (gunzipped when compressed) bytes of one PAK entry.
pub fn payload(vfs: &mut Vfs, path: &str, selected: AssetKey) -> Result<Vec<u8>, NtStatus> {
    let bytes = with_entry(vfs, path, selected, |pak, vfs, index| {
        pak.payload(vfs, index)
    })?;
    if std::env::var_os("TSFP_ASSET_LOG").is_some() {
        eprintln!("{}", trace_line(path, selected, &bytes));
    }
    Ok(bytes)
}

/// One asset-load trace record for the Tier 2 Rust-vs-C comparison (T2047): path, key,
/// occurrence, decoded length and FNV-1a 64 of the decoded bytes. No payload bytes are logged.
pub fn trace_line(path: &str, selected: AssetKey, decoded: &[u8]) -> String {
    let hash = decoded.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3)
    });
    format!(
        "asset-load path={path} key={:08x} occ={} len={} fnv64={hash:016x}",
        selected.key,
        selected.occurrence,
        decoded.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_line_is_stable_and_hides_the_bytes() {
        let selected = AssetKey {
            key: 0x4afe_3652,
            occurrence: 1,
        };
        let line = trace_line("D:\\a.pak", selected, b"abc");
        assert_eq!(
            line,
            "asset-load path=D:\\a.pak key=4afe3652 occ=1 len=3 fnv64=e71fa2190541574b"
        );
    }
}

pub struct AssetLoader {
    vfs: Vfs,
}

impl AssetLoader {
    pub fn new(vfs: Vfs) -> Self {
        Self { vfs }
    }
    pub fn vfs(&mut self) -> &mut Vfs {
        &mut self.vfs
    }
    pub fn into_vfs(self) -> Vfs {
        self.vfs
    }

    pub fn stored(&mut self, path: &str, selected: AssetKey) -> Result<Vec<u8>, NtStatus> {
        with_entry(&mut self.vfs, path, selected, |pak, vfs, index| {
            pak.stored(vfs, index)
        })
    }

    pub fn texture(
        &mut self,
        path: &str,
        selected: AssetKey,
    ) -> Result<tsfp_format_xbt::Texture, NtStatus> {
        with_entry(&mut self.vfs, path, selected, |pak, vfs, index| {
            pak.texture(vfs, index)
        })
    }

    pub fn level(
        &mut self,
        path: &str,
        selected: AssetKey,
    ) -> Result<tsfp_format_xbr::Level, NtStatus> {
        with_entry(&mut self.vfs, path, selected, |pak, vfs, index| {
            pak.level(vfs, index)
        })
    }
}
