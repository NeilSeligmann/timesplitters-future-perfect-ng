//! Clock identities captured at a replay's ordered guest-pad boundary (T2046).
//!
//! Three clocks meet at a native replay's end and stay distinct: the Bevy host/window replay
//! poll that detects the end, the fixed simulation tick, and the guest port-0 pad poll count
//! that keys button-dump samples. The guest count is captured at the final recorded row;
//! a later completion-time read could include a poll after that row. The reporter never
//! takes the device lock under the playback lock (docs/t2046-route-clock-contract.md).

use std::sync::Mutex;

/// One armed, not yet reported route end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pending {
    host_poll: u64,
    sim_tick: u64,
    hold: bool,
    guest_exclusive: Option<u64>,
}

static PENDING: Mutex<Option<Pending>> = Mutex::new(None);

fn pending() -> std::sync::MutexGuard<'static, Option<Pending>> {
    PENDING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The host poll an end line names. T3393: the tick clock steps the replay poll, so a route
/// end names the recorded end + 1; a hold (or a recorded marker) names the poll it already has.
pub fn end_line_poll(
    marker: Option<u64>,
    route_end_tick_clock: bool,
    end_poll: u64,
    live: u64,
) -> u64 {
    marker.unwrap_or(if route_end_tick_clock {
        end_poll + 1
    } else {
        live
    })
}

/// Validate the exclusive guest count captured at the final recorded input row.
pub fn stable_guest_exclusive_boundary(final_row_guest_count: Option<u64>) -> Option<u64> {
    final_row_guest_count.filter(|count| *count > 0)
}

/// Record the end the replay detected at `host_poll` / `sim_tick`; the caller owns
/// once-per-replay state and may hold its playback lock. A second arm before a flush keeps
/// the first end (the earliest detection is the boundary).
pub fn arm(host_poll: u64, sim_tick: u64, hold: bool, final_row_guest_count: Option<u64>) {
    pending().get_or_insert(Pending {
        host_poll,
        sim_tick,
        hold,
        guest_exclusive: stable_guest_exclusive_boundary(final_row_guest_count),
    });
}

/// Report an armed end after releasing the playback lock. An unknown boundary emits no clock marker.
pub fn flush() {
    let Some(end) = pending().take() else { return };
    for line in lines(end) {
        eprintln!("{line}");
    }
}

fn lines(end: Pending) -> Vec<String> {
    let Pending {
        host_poll,
        sim_tick,
        hold,
        guest_exclusive,
    } = end;
    let mut out = Vec::new();
    if hold {
        out.push(format!(
            "tsfp-bevy: native input replay completed at recorded sim tick {sim_tick} poll={host_poll}; holding neutral input"
        ));
    }
    if let Some(guest_exclusive) = guest_exclusive {
        out.push(format!(
            "TSFP_ROUTE_CLOCK_V1 host_poll={host_poll} recorded_sim_tick={sim_tick} guest_pad_exclusive={guest_exclusive}"
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    // One test owns the process-wide PENDING slot, so ordering stays deterministic.
    #[test]
    fn arm_records_the_first_boundary_and_reports_once() {
        flush();
        arm(6144, 2666, true, Some(3322));
        arm(6150, 2670, false, Some(3323));
        assert_eq!(
            *pending(),
            Some(Pending {
                host_poll: 6144,
                sim_tick: 2666,
                hold: true,
                guest_exclusive: Some(3322)
            })
        );
        flush();
        flush();
        assert!(pending().is_none());
    }

    #[test]
    fn marker_keeps_the_three_clocks_apart() {
        let end = Pending {
            host_poll: 6144,
            sim_tick: 2666,
            hold: false,
            guest_exclusive: Some(3322),
        };
        assert_eq!(
            lines(end),
            ["TSFP_ROUTE_CLOCK_V1 host_poll=6144 recorded_sim_tick=2666 guest_pad_exclusive=3322"]
        );
        let held = lines(Pending { hold: true, ..end });
        assert_eq!(held.len(), 2);
        // rust_trace.route_end_poll_from_logs reads the host poll from the hold line only.
        assert!(held[0].ends_with("poll=6144; holding neutral input"));
        assert!(!held[1].contains(" poll="));
    }

    #[test]
    fn last_row_excludes_polls_after_completion_and_missing_fails_closed() {
        let a = stable_guest_exclusive_boundary(Some(6591));
        let b = stable_guest_exclusive_boundary(Some(6591));
        assert_eq!((a, b), (Some(6591), Some(6591)));
        assert_eq!(stable_guest_exclusive_boundary(None), None);
        assert_eq!(stable_guest_exclusive_boundary(Some(0)), None);
        let end = Pending {
            host_poll: 1,
            sim_tick: 1,
            hold: false,
            guest_exclusive: None,
        };
        assert!(lines(end).is_empty());
    }
}
