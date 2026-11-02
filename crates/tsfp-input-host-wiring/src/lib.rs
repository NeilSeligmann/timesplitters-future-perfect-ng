// SPDX-License-Identifier: GPL-3.0-or-later
//! T3975: the input host duties (`tsfp_input_host_*`) in Rust. `pad_plan` is the pure policy (ordered steps); the
//! `extern "C"` entry points below execute it against the C XInput modules declared in `ffi`, and own the wiring state
//! (`STATE`). The C unit `src/host/tsfp_input_host_wiring.c` is the `TSFP_RUST_INPUT_HLE=OFF` fallback and applies the
//! same decisions in the same order. Linked into the host through the `rust-input-host` feature of `tsfp-input-capi`.
// The C modules only exist in the host executable, so unit tests (no C) build the plan model alone.
#[cfg(not(test))]
mod ffi;
#[cfg(not(test))]
mod host;

/// Options the host passes to `tsfp_input_host_configure_pads`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PadConfig {
    pub synthetic_pad: bool,
    pub controllers: bool,
    pub pad_ports: bool,
    pub pad_ports_router_present: bool,
    pub remove_after_polls: u32,
    pub has_script: bool,
    pub script_live: bool,
}

/// One step of pad configuration, in execution order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PadStep {
    SyntheticPad(bool),
    AttachSyntheticPad,
    Multiport,
    PadPortsMultiport,
    RemoveAfterPolls(u32),
    SourceReset,
    LoadLiveScript,
    LoadScript,
}

/// The recorded replay budgets returned by the retained C record reader.
#[repr(C)]
pub struct ReplayBudgets {
    pub owner_waits: u32,
    pub worker_blanks: u32,
    pub thread_timeout_ms: u32,
    pub interactive: bool,
}

/// Per-feed tick bookkeeping for the host hotkey engine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HotkeyPoll {
    last: Option<u64>,
}

impl HotkeyPoll {
    /// Returns true once for each distinct guest poll index.
    pub fn begin(&mut self, poll: u64) -> bool {
        if self.last == Some(poll) {
            return false;
        }
        self.last = Some(poll);
        true
    }
}

/// The ordered steps, or `Err` when `--pad-ports` has no router (exit 2).
pub fn pad_plan(config: &PadConfig) -> Result<Vec<PadStep>, &'static str> {
    let mut steps = vec![PadStep::SyntheticPad(false)];
    if config.synthetic_pad {
        steps.push(PadStep::AttachSyntheticPad);
        steps.push(PadStep::SyntheticPad(true));
    }
    if config.controllers {
        steps.push(PadStep::SyntheticPad(true));
        steps.push(PadStep::Multiport);
    }
    if config.pad_ports {
        if !config.pad_ports_router_present {
            return Err("--pad-ports: needs the Bevy shell's input router");
        }
        steps.push(PadStep::PadPortsMultiport);
    }
    steps.push(PadStep::RemoveAfterPolls(config.remove_after_polls));
    steps.push(PadStep::SourceReset);
    if config.has_script {
        steps.push(if config.script_live {
            PadStep::LoadLiveScript
        } else {
            PadStep::LoadScript
        });
    }
    Ok(steps)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_reset_then_clear_source() {
        let plan = pad_plan(&PadConfig::default()).unwrap();
        assert_eq!(
            plan,
            [
                PadStep::SyntheticPad(false),
                PadStep::RemoveAfterPolls(0),
                PadStep::SourceReset
            ]
        );
    }

    #[test]
    fn pad_ports_needs_router() {
        let config = PadConfig {
            pad_ports: true,
            ..PadConfig::default()
        };
        assert!(pad_plan(&config).is_err());
    }

    #[test]
    fn synthetic_pad_with_live_script_keeps_order() {
        let config = PadConfig {
            synthetic_pad: true,
            has_script: true,
            script_live: true,
            ..PadConfig::default()
        };
        let plan = pad_plan(&config).unwrap();
        assert_eq!(plan[1], PadStep::AttachSyntheticPad);
        assert_eq!(*plan.last().unwrap(), PadStep::LoadLiveScript);
    }

    #[test]
    fn hotkey_feed_ticks_once_per_poll_and_resets_for_a_new_poll() {
        let mut feed = HotkeyPoll::default();
        assert!(feed.begin(10));
        assert!(!feed.begin(10));
        assert!(feed.begin(11));
        assert!(feed.begin(10));
    }
}
