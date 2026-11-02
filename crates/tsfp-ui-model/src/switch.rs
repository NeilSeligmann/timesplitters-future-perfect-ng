//! The `--ui=rust|legacy` switch of UI-6 (T2189, docs/t-ui-system-design.md stage 6): which
//! backend builds and lays out each menu page. Host free: the host passes its argument list
//! (and the `TSFP_UI` variable) here and asks per page. Default is `legacy` until the parity
//! bar (menu route traces EQUAL, frame hashes match) is met. A page runs on Rust only when the
//! switch selects Rust, the page is not forced back to legacy, and its layout is complete
//! (`layout::apply_complete_to_page` succeeded), so a bad page can be bisected away.

use std::collections::BTreeSet;

use crate::model::PageId;

/// Backend that owns a menu page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiBackend {
    /// The original (lifted or ported) builders draw the page.
    #[default]
    Legacy,
    /// The Rust UI model builds, lays out and draws the page.
    Rust,
}

/// Parsed switch state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UiSwitch {
    pub backend: UiBackend,
    /// Pages kept on legacy even when `backend` is Rust (`--ui-legacy-page=ADDR`, repeatable).
    pub legacy_pages: BTreeSet<PageId>,
}

/// Argument error, the message names the offending argument.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SwitchError(pub String);

fn backend(text: &str) -> Result<UiBackend, SwitchError> {
    match text {
        "rust" => Ok(UiBackend::Rust),
        "legacy" => Ok(UiBackend::Legacy),
        other => Err(SwitchError(format!(
            "--ui: expected rust or legacy, got {other:?}"
        ))),
    }
}

fn page(text: &str) -> Result<PageId, SwitchError> {
    let digits = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X"));
    let value = match digits {
        Some(hex) => u64::from_str_radix(hex, 16),
        None => text.parse(),
    };
    value
        .map(PageId)
        .map_err(|_| SwitchError(format!("--ui-legacy-page: bad page address {text:?}")))
}

impl UiSwitch {
    /// Reads `env` (the `TSFP_UI` value, if set) then `args`, later wins. Returns the switch and
    /// the arguments it did not consume, in order, for the rest of the host parser.
    pub fn parse<'a>(
        env: Option<&str>,
        args: impl IntoIterator<Item = &'a str>,
    ) -> Result<(Self, Vec<&'a str>), SwitchError> {
        let mut switch = Self::default();
        if let Some(value) = env.filter(|value| !value.is_empty()) {
            switch.backend = backend(value)?;
        }
        let mut rest = Vec::new();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            if let Some(value) = arg.strip_prefix("--ui=") {
                switch.backend = backend(value)?;
            } else if arg == "--ui" {
                let value = args
                    .next()
                    .ok_or(SwitchError("--ui needs a value".into()))?;
                switch.backend = backend(value)?;
            } else if let Some(value) = arg.strip_prefix("--ui-legacy-page=") {
                switch.legacy_pages.insert(page(value)?);
            } else {
                rest.push(arg);
            }
        }
        Ok((switch, rest))
    }

    /// Backend for `page`; `layout_complete` is the result of the checked layout for this build.
    pub fn backend_for(&self, page: PageId, layout_complete: bool) -> UiBackend {
        self.route(page, layout_complete).backend()
    }

    /// Backend for `page` together with the reason, for the route log.
    pub fn route(&self, page: PageId, layout_complete: bool) -> Route {
        if self.backend == UiBackend::Legacy {
            Route::LegacySelected
        } else if self.legacy_pages.contains(&page) {
            Route::LegacyOverride
        } else if !layout_complete {
            Route::LayoutIncomplete
        } else {
            Route::Rust
        }
    }
}

/// Why a page went to its backend. Every reason except `Rust` means legacy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// `--ui=legacy` (the default).
    LegacySelected,
    /// `--ui-legacy-page` forced this page back.
    LegacyOverride,
    /// The checked layout of the page failed, so the Rust model cannot own it.
    LayoutIncomplete,
    /// The Rust model builds, lays out and draws the page.
    Rust,
}

impl Route {
    pub fn backend(self) -> UiBackend {
        if self == Route::Rust {
            UiBackend::Rust
        } else {
            UiBackend::Legacy
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Route::LegacySelected => "legacy",
            Route::LegacyOverride => "legacy-override",
            Route::LayoutIncomplete => "layout-incomplete",
            Route::Rust => "rust",
        }
    }
}

/// One page open as seen by the switch, in call order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteEntry {
    pub page: PageId,
    pub route: Route,
}

/// Ordered log of page routing decisions, the Rust side of a future menu route trace
/// (`tools.rust_trace`). Repeated asks for the page already on top (the host may ask every
/// frame) collapse into one entry. Bounded: past `capacity` entries only `dropped` grows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteLog {
    entries: Vec<RouteEntry>,
    capacity: usize,
    pub dropped: u64,
}

impl RouteLog {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: Vec::new(),
            capacity,
            dropped: 0,
        }
    }

    pub fn record(&mut self, page: PageId, route: Route) {
        let entry = RouteEntry { page, route };
        if self.entries.last() == Some(&entry) {
            return;
        }
        if self.entries.len() < self.capacity {
            self.entries.push(entry);
        } else {
            self.dropped += 1;
        }
    }

    pub fn entries(&self) -> &[RouteEntry] {
        &self.entries
    }

    /// One line per entry, `0xPAGE route`, the format a route trace compares.
    pub fn lines(&self) -> Vec<String> {
        self.entries
            .iter()
            .map(|entry| format!("{:#x} {}", entry.page.0, entry.route.name()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_legacy_everywhere() {
        let (switch, rest) = UiSwitch::parse(None, ["--quiet"]).unwrap();
        assert_eq!(switch, UiSwitch::default());
        assert_eq!(rest, ["--quiet"]);
        assert_eq!(switch.backend_for(PageId(1), true), UiBackend::Legacy);
    }

    #[test]
    fn rust_with_page_fallback_and_incomplete_layout() {
        let (switch, rest) = UiSwitch::parse(
            Some("legacy"),
            ["a", "--ui=rust", "--ui-legacy-page=0x10", "b"],
        )
        .unwrap();
        assert_eq!(rest, ["a", "b"]);
        assert_eq!(switch.backend_for(PageId(0x11), true), UiBackend::Rust);
        assert_eq!(switch.backend_for(PageId(0x10), true), UiBackend::Legacy);
        assert_eq!(switch.backend_for(PageId(0x11), false), UiBackend::Legacy);
    }

    #[test]
    fn route_reasons_follow_the_precedence() {
        let (legacy, _) = UiSwitch::parse(None, ["--ui-legacy-page=0x10"]).unwrap();
        assert_eq!(legacy.route(PageId(0x10), false), Route::LegacySelected);
        let (rust, _) = UiSwitch::parse(None, ["--ui=rust", "--ui-legacy-page=0x10"]).unwrap();
        assert_eq!(rust.route(PageId(0x10), false), Route::LegacyOverride);
        assert_eq!(rust.route(PageId(0x11), false), Route::LayoutIncomplete);
        assert_eq!(rust.route(PageId(0x11), true), Route::Rust);
        assert_eq!(Route::Rust.backend(), UiBackend::Rust);
        assert_eq!(Route::LayoutIncomplete.backend(), UiBackend::Legacy);
    }

    #[test]
    fn route_log_collapses_repeats_and_is_bounded() {
        let mut log = RouteLog::new(3);
        log.record(PageId(0x20), Route::Rust);
        log.record(PageId(0x20), Route::Rust);
        log.record(PageId(0x21), Route::LegacyOverride);
        log.record(PageId(0x20), Route::Rust);
        log.record(PageId(0x22), Route::Rust);
        assert_eq!(
            log.lines(),
            ["0x20 rust", "0x21 legacy-override", "0x20 rust"]
        );
        assert_eq!(log.dropped, 1);
    }

    #[test]
    fn env_then_args_and_errors() {
        let (switch, _) = UiSwitch::parse(Some("rust"), []).unwrap();
        assert_eq!(switch.backend, UiBackend::Rust);
        let (switch, _) = UiSwitch::parse(Some("rust"), ["--ui", "legacy"]).unwrap();
        assert_eq!(switch.backend, UiBackend::Legacy);
        assert!(UiSwitch::parse(None, ["--ui=vulkan"]).is_err());
        assert!(UiSwitch::parse(None, ["--ui"]).is_err());
        assert!(UiSwitch::parse(None, ["--ui-legacy-page=zz"]).is_err());
    }
}
