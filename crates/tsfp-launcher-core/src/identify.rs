//! Identify a user-supplied disc image or extracted folder before installing it.
//! A dump is recognised by the SHA-256 of its `default.xbe`: the executable the port targets.
use crate::Result;
use std::path::Path;
use tsfp_import::{sha256, source};

/// A dump the port is known to work with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KnownDump {
    /// Version id used for `assets/original/<id>/`.
    pub id: &'static str,
    pub label: &'static str,
    pub xbe_sha256: &'static str,
}

/// Hashes only, never disc bytes. Further regions/platforms land here as they are verified.
pub const KNOWN_DUMPS: &[KnownDump] = &[KnownDump {
    id: "xbox-usa",
    label: "TimeSplitters: Future Perfect (Xbox, USA)",
    xbe_sha256: "3cfd001a84fc3e08175d6c4b2e42ebf49577a089b5d0bd87ac724f61a41816bc",
}];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    /// `xdvdfs`, `iso9660`, `gcm` or `directory`.
    pub layout: String,
    pub file_count: usize,
    pub xbe_sha256: Option<String>,
    pub known: Option<KnownDump>,
}

impl Identity {
    /// Version id to install under: the known dump's id, else a stable fallback.
    pub fn version_id(&self) -> &'static str {
        self.known.map_or("xbox-unknown", |k| k.id)
    }

    /// One line for the setup wizard.
    pub fn verdict(&self) -> String {
        match (&self.known, &self.xbe_sha256) {
            (Some(k), _) => format!("recognised: {}", k.label),
            (None, Some(h)) => {
                format!("unknown dump (default.xbe sha256 {h}); install is allowed but untested")
            }
            (None, None) => "no default.xbe found: this is not an Xbox game disc".into(),
        }
    }
}

pub fn lookup(xbe_sha256: &str) -> Option<KnownDump> {
    KNOWN_DUMPS
        .iter()
        .copied()
        .find(|k| k.xbe_sha256.eq_ignore_ascii_case(xbe_sha256))
}

/// Open `path` (image or folder) and identify it. Reads the file table and `default.xbe` only.
pub fn identify(path: &Path) -> Result<Identity> {
    let src = source::open(path, "probe", source::Platform::Xbox)?;
    let files = src.list()?;
    let xbe = files
        .iter()
        .find(|f| f.path.eq_ignore_ascii_case("default.xbe"));
    let xbe_sha256 = match xbe {
        Some(e) => Some(sha256::hex(&src.read(e)?)),
        None => None,
    };
    let known = xbe_sha256.as_deref().and_then(lookup);
    Ok(Identity {
        layout: src.descriptor().layout.to_string(),
        file_count: files.len(),
        xbe_sha256,
        known,
    })
}
