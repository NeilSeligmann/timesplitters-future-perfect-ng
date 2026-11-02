// SPDX-License-Identifier: GPL-3.0-or-later
//! T2200: the two safety nets of the mouse and keyboard player, both pure so they are testable without a window or a guest.
//!
//! * [`RawLookGuard`]: raw look (T2161) hands mouse degrees to a mailbox that only the Rust player look root drains. When
//!   that root is not served by Rust (`--rust-ports=all` leaves the player roots out) the mouse would not aim at all. The
//!   guard watches the host's hook counters and falls back to the interim virtual right stick of T2160 with one clear log
//!   line, and returns to raw look when the hook reports hits again.
//! * [`PadFilter`]: operating systems enumerate keyboards (consumer control, system control nodes) as gamepads. Such a
//!   device must not enter the router, where its noise would count as pad activity and take the player away from the mouse.

use std::collections::{BTreeMap, BTreeSet};

/// Seconds after start at which the host has configured its ports, so `reachable` is trusted.
pub const DECIDE_AFTER_SECONDS: f32 = 3.0;
/// Ticks of mouse look activity without a single hook hit before raw look is given up (about one second at 60 Hz).
pub const STALL_TICKS: u32 = 60;

/// T3819: `TSFP_REPLAY_RAW_LOOK=hold` keeps a replay on raw look (no stall fallback). The recording holds the owner's
/// mouse degrees but not the guard's live decision, and the stall count depends on how sim ticks meet guest frames,
/// which differs between the owner's session and a replay; a replay-only fallback turns the degrees into stick
/// deflection and the player's heading drifts (`docs/t-replay-desync-l3-l4.md`).
pub fn replay_holds_raw(replaying: bool) -> bool {
    static HOLD: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    replaying
        && *HOLD.get_or_init(|| hold_switch(std::env::var("TSFP_REPLAY_RAW_LOOK").ok().as_deref()))
}

/// The `TSFP_REPLAY_RAW_LOOK` value test: only `hold` holds raw look.
pub fn hold_switch(value: Option<&str>) -> bool {
    value == Some("hold")
}

/// Host side counters sampled once per tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostProbe {
    /// The player look root is selected for Rust (`--rust-ports` includes `player`).
    pub reachable: bool,
    /// Calls of the look root for the enabled slot so far (the hook ran).
    pub hits: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuardEvent {
    /// First decision: raw look will work.
    Active,
    /// Raw look gave way to the interim virtual stick. The text is the full log line (after the prefix).
    NotActive(String),
    /// The hook reports hits again after a fallback.
    Reactivated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Raw,
    FallbackNotSelected,
    FallbackNoHits,
}

#[derive(Clone, Debug)]
pub struct RawLookGuard {
    mode: Mode,
    decided: bool,
    last_hits: u32,
    stalled: u32,
}

pub const NOT_SELECTED_TEXT: &str = "raw look NOT ACTIVE: the player look root 0x25D520 is not served by Rust under this --rust-ports (all leaves the player roots out; use --rust-ports=all,player or player). The mouse look uses the interim virtual right stick (T2160)";
pub const NO_HITS_TEXT: &str = "raw look NOT ACTIVE: the look hook was not reached for 60 steps of mouse activity (the player is not on foot, or the player roots do not run in Rust). The mouse look uses the interim virtual right stick (T2160) until the hook reports hits";

impl Default for RawLookGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl RawLookGuard {
    pub fn new() -> Self {
        Self {
            mode: Mode::Raw,
            decided: false,
            last_hits: 0,
            stalled: 0,
        }
    }

    /// True while mouse degrees should go to the raw look mailbox.
    pub fn raw(&self) -> bool {
        self.mode == Mode::Raw
    }

    /// Advance one tick. `elapsed` is seconds since start, `mouse_active` whether this tick carried mouse look counts.
    pub fn update(
        &mut self,
        elapsed: f32,
        mouse_active: bool,
        probe: HostProbe,
    ) -> Option<GuardEvent> {
        let hit = probe.hits != self.last_hits;
        self.last_hits = probe.hits;
        if hit {
            self.stalled = 0;
        }
        if !self.decided && (elapsed >= DECIDE_AFTER_SECONDS || hit) {
            self.decided = true;
            if probe.reachable || hit {
                return Some(GuardEvent::Active);
            }
            self.mode = Mode::FallbackNotSelected;
            return Some(GuardEvent::NotActive(NOT_SELECTED_TEXT.into()));
        }
        match self.mode {
            Mode::Raw => {
                if mouse_active && !hit {
                    self.stalled += 1;
                    if self.stalled >= STALL_TICKS {
                        self.mode = Mode::FallbackNoHits;
                        return Some(GuardEvent::NotActive(NO_HITS_TEXT.into()));
                    }
                }
            }
            Mode::FallbackNotSelected | Mode::FallbackNoHits => {
                if hit {
                    self.mode = Mode::Raw;
                    return Some(GuardEvent::Reactivated);
                }
            }
        }
        None
    }
}

/// Name fragments (lower case) of devices that are not gamepads.
pub const DEFAULT_IGNORED_NAMES: [&str; 7] = [
    "keyboard",
    "mouse",
    "consumer",
    "system control",
    "touchpad",
    "trackpad",
    "hotkey",
];

/// Decides which gamepad connections reach the router. Ids are the shell's pad ids.
#[derive(Clone, Debug)]
pub struct PadFilter {
    patterns: Vec<String>,
    ignored: BTreeSet<u32>,
    names: BTreeMap<u32, String>,
}

impl PadFilter {
    /// `extra` are further name fragments; `defaults` false drops the built in list (`--no-pad-filter`).
    pub fn new(defaults: bool, extra: &[String]) -> Self {
        let mut patterns: Vec<String> = if defaults {
            DEFAULT_IGNORED_NAMES
                .iter()
                .map(|s| (*s).to_string())
                .collect()
        } else {
            Vec::new()
        };
        patterns.extend(
            extra
                .iter()
                .map(|s| s.to_lowercase())
                .filter(|s| !s.is_empty()),
        );
        Self {
            patterns,
            ignored: BTreeSet::new(),
            names: BTreeMap::new(),
        }
    }

    /// The fragment of the ignore list that `name` matches, if any.
    pub fn matches(&self, name: &str) -> Option<&str> {
        let lower = name.to_lowercase();
        self.patterns
            .iter()
            .find(|pattern| lower.contains(pattern.as_str()))
            .map(String::as_str)
    }

    /// A connection event. Returns `Err(fragment)` when the device is ignored (and remembers it), `Ok(())` when it is a pad.
    pub fn connect(&mut self, id: u32, name: &str) -> Result<(), String> {
        self.names.insert(id, name.to_string());
        match self.matches(name).map(str::to_string) {
            Some(fragment) => {
                self.ignored.insert(id);
                Err(fragment)
            }
            None => {
                self.ignored.remove(&id);
                Ok(())
            }
        }
    }

    pub fn disconnect(&mut self, id: u32) {
        self.ignored.remove(&id);
        self.names.remove(&id);
    }

    pub fn is_ignored(&self, id: u32) -> bool {
        self.ignored.contains(&id)
    }

    pub fn name(&self, id: u32) -> Option<&str> {
        self.names.get(&id).map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t3819_only_hold_holds_raw_look_and_only_in_replay() {
        assert!(hold_switch(Some("hold")));
        assert!(!hold_switch(None) && !hold_switch(Some("1")) && !hold_switch(Some("")));
        assert!(!replay_holds_raw(false));
    }

    fn probe(reachable: bool, hits: u32) -> HostProbe {
        HostProbe { reachable, hits }
    }

    #[test]
    fn unreachable_root_falls_back_once_after_the_ports_are_configured() {
        let mut guard = RawLookGuard::new();
        assert_eq!(
            guard.update(0.5, true, probe(false, 0)),
            None,
            "ports not configured yet at start"
        );
        assert!(guard.raw());
        let event = guard.update(3.1, false, probe(false, 0));
        assert!(
            matches!(event, Some(GuardEvent::NotActive(text)) if text.contains("--rust-ports=all,player"))
        );
        assert!(!guard.raw());
        assert_eq!(
            guard.update(4.0, true, probe(false, 0)),
            None,
            "one line only"
        );
    }

    #[test]
    fn reachable_root_with_hits_stays_raw() {
        let mut guard = RawLookGuard::new();
        assert_eq!(
            guard.update(3.2, false, probe(true, 0)),
            Some(GuardEvent::Active)
        );
        for hits in 1..500 {
            assert_eq!(guard.update(4.0, true, probe(true, hits)), None);
        }
        assert!(guard.raw());
    }

    #[test]
    fn selected_but_never_reached_falls_back_after_sixty_active_ticks_then_returns_on_a_hit() {
        let mut guard = RawLookGuard::new();
        assert_eq!(
            guard.update(3.2, false, probe(true, 0)),
            Some(GuardEvent::Active)
        );
        for _ in 0..59 {
            assert_eq!(guard.update(5.0, true, probe(true, 0)), None);
        }
        assert!(guard.raw());
        assert!(matches!(
            guard.update(5.0, true, probe(true, 0)),
            Some(GuardEvent::NotActive(_))
        ));
        assert!(!guard.raw());
        assert_eq!(guard.update(6.0, true, probe(true, 0)), None);
        assert_eq!(
            guard.update(6.0, true, probe(true, 7)),
            Some(GuardEvent::Reactivated)
        );
        assert!(guard.raw());
    }

    #[test]
    fn idle_mouse_never_trips_the_stall_rule() {
        let mut guard = RawLookGuard::new();
        guard.update(3.2, false, probe(true, 0));
        for _ in 0..10_000 {
            assert_eq!(guard.update(9.0, false, probe(true, 0)), None);
        }
        assert!(guard.raw());
    }

    #[test]
    fn a_hit_before_the_decision_time_activates_even_if_reachable_is_unknown() {
        let mut guard = RawLookGuard::new();
        assert_eq!(
            guard.update(1.0, true, probe(false, 4)),
            Some(GuardEvent::Active)
        );
        assert!(guard.raw());
    }

    #[test]
    fn keyboard_consumer_control_nodes_are_not_pads() {
        let mut filter = PadFilter::new(true, &[]);
        let fragment = filter
            .connect(6, "SmartCloud AL68 Keyboard System Control")
            .unwrap_err();
        assert_eq!(fragment, "keyboard");
        assert!(filter.is_ignored(6));
        assert!(filter.connect(7, "Xbox Wireless Controller").is_ok());
        assert!(!filter.is_ignored(7));
        assert!(filter.connect(8, "Logitech Consumer Control").is_err());
        filter.disconnect(6);
        assert!(!filter.is_ignored(6));
    }

    #[test]
    fn ignore_list_is_configurable() {
        let mut filter = PadFilter::new(true, &["Dongle".to_string()]);
        assert!(filter.connect(1, "Weird USB Dongle").is_err());
        let mut none = PadFilter::new(false, &[]);
        assert!(
            none.connect(2, "Some Keyboard").is_ok(),
            "--no-pad-filter keeps every device"
        );
    }
}
