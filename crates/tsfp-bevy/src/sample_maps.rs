//! Live read-only Map Maker samples from the same disc argument as the host.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use tsfp_engine_fileio::sample_maps::DiscSampleMaps;
use tsfp_engine_fileio::storage_flow::{MapPayload, SampleMapEntry, SampleMaps};
use tsfp_engine_fileio::vfs::{NtStatus, status};
use tsfp_ui_model::font::StringSource;
use tsfp_ui_model::model::TextId;
use tsfp_ui_model::strings::BankStrings;

pub(crate) enum Provider {
    Empty,
    Disc(DiscSampleMaps),
}

impl SampleMaps for Provider {
    fn load_sample(&mut self, id: u32) -> Result<MapPayload, NtStatus> {
        match self {
            Self::Empty => Err(status::OBJECT_NAME_NOT_FOUND),
            Self::Disc(maps) => maps.load_sample(id),
        }
    }

    fn catalogue(&mut self) -> Result<Vec<SampleMapEntry>, NtStatus> {
        match self {
            Self::Empty => Ok(Vec::new()),
            Self::Disc(maps) => maps.catalogue(),
        }
    }
}

enum State {
    Uninitialized,
    Ready(Provider),
    Failed(NtStatus),
}

static DISC_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();
static PROVIDER: Mutex<State> = Mutex::new(State::Uninitialized);

/// Retain the same disc selected for the guest host; do not inspect guest memory
/// for sample paths or text IDs.
pub fn init(arguments: &[String]) {
    let _ = DISC_PATH.set(disc_path(arguments));
}

fn disc_path(arguments: &[String]) -> Option<PathBuf> {
    let mut index = 0;
    while index < arguments.len() {
        if arguments[index] == "--disc" {
            if let Some(path) = arguments.get(index + 1) {
                return Some(PathBuf::from(path));
            }
        } else if let Some(path) = arguments[index].strip_prefix("--disc=") {
            return Some(PathBuf::from(path));
        }
        index += 1;
    }
    std::env::var_os("TSFP_UI_DISC")
        .or_else(|| std::env::var_os("TSFP_DISC"))
        .map(PathBuf::from)
}

fn title_strings(path: &Path) -> Result<BankStrings, NtStatus> {
    let selected_ui_disc = std::env::var_os("TSFP_UI_DISC").map(PathBuf::from);
    if selected_ui_disc.as_deref() == Some(path)
        && let Some(strings) = crate::ui_overlay::prepared()
            .and_then(|prepared| prepared.text.as_ref())
            .map(|text| text.strings.clone())
    {
        return Ok(strings);
    }
    tsfp_ui_wgpu::overlay::TextAssets::from_disc(path, 'e')
        .map(|assets| assets.strings)
        .map_err(|_| status::UNRECOGNIZED_VOLUME)
}

fn title_label(strings: &BankStrings, id: u32) -> Option<String> {
    strings
        .bytes(TextId(id))
        .map(|bytes| bytes.iter().map(|byte| char::from(*byte)).collect())
}

fn load(path: &Path) -> Result<Provider, NtStatus> {
    let strings = title_strings(path)?;
    let maps = DiscSampleMaps::open(path, |text_id| title_label(&strings, text_id))?;
    Ok(Provider::Disc(maps))
}

/// Run one native storage operation with the cached title sample provider.
/// Cache errors fail the operation instead of publishing an empty catalogue.
pub(crate) fn with<T>(
    operation: impl FnOnce(&mut Provider) -> Result<T, NtStatus>,
) -> Result<T, NtStatus> {
    let mut state = PROVIDER.lock().map_err(|_| status::INVALID_PARAMETER)?;
    if matches!(*state, State::Uninitialized) {
        let loaded = match DISC_PATH.get().and_then(Option::as_deref) {
            Some(path) => load(path),
            None => Ok(Provider::Empty),
        };
        *state = match loaded {
            Ok(provider) => State::Ready(provider),
            Err(status) => State::Failed(status),
        };
    }
    match &mut *state {
        State::Uninitialized => Err(status::INVALID_PARAMETER),
        State::Failed(status) => Err(*status),
        State::Ready(provider) => operation(provider),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disc_path_accepts_host_disc_arguments() {
        let args = ["game.xbe", "--disc", "owner.iso"]
            .map(str::to_owned)
            .to_vec();
        assert_eq!(disc_path(&args), Some(PathBuf::from("owner.iso")));
        let args = ["game.xbe", "--disc=owner.iso"].map(str::to_owned).to_vec();
        assert_eq!(disc_path(&args), Some(PathBuf::from("owner.iso")));
    }

    #[test]
    #[ignore = "requires TSFP_DISC_ISO naming the private owner's Xbox ISO"]
    fn private_disc_sample_labels_and_maps_load_without_printing_title_text() {
        let path = std::env::var_os("TSFP_DISC_ISO").expect("private disc path");
        let mut provider = load(Path::new(&path)).unwrap();
        let catalogue = provider.catalogue().unwrap();
        assert_eq!(catalogue.len(), 8);
        assert!(catalogue.iter().all(|entry| !entry.label.is_empty()));
        for entry in &catalogue {
            let payload = provider.load_sample(entry.id).unwrap();
            tsfp_engine_fileio::map_document::MapDocument::parse(&payload).unwrap();
        }
        eprintln!(
            "private title sample labels and owned maps resolved: {}",
            catalogue.len()
        );
    }
}
