// SPDX-License-Identifier: GPL-3.0-or-later
//! The owner's control lock, inferred from a recording's `S` digests when it has no `control` decision rows
//! (docs/t-replay-desync-l3-l4.md, "Control lock"). INFERRED, labelled so in the replay log.
//!
//! A stretch where `pos`, `yaw` and `pitch` do not change from one digest to the next is a stretch where look and move
//! input had no effect on the owner's player: dropping it in the replay is exact for a standing player and is what
//! the game did for a frozen one. The stretch ends at its last unchanged digest, so rows between that digest and the
//! first changed one are always delivered.

use crate::codec::{DeviceEvent, encode};

/// Prefix the shell gives a state digest row it hands over with the device rows: `s:<payload>`.
pub const STATE_PREFIX: &str = "s:";
const TOLERANCE: f32 = 0.005;

fn floats(text: &str) -> Option<Vec<f32>> {
    text.split(',').map(|part| part.parse().ok()).collect()
}

/// `pos`, `yaw` and `pitch` of a digest payload, `None` without a player.
fn view(payload: &str) -> Option<Vec<f32>> {
    let field = |name: &str| {
        payload.split(';').find_map(|part| {
            part.strip_prefix(name)
                .and_then(|rest| rest.strip_prefix('='))
        })
    };
    let mut all = floats(field("pos")?)?;
    all.extend(floats(field("yaw")?)?);
    all.extend(floats(field("pitch")?)?);
    Some(all)
}

fn same(a: &[f32], b: &[f32]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() <= TOLERANCE)
}

type Rows = Vec<(u64, String)>;

/// Split the digest rows (`s:` prefixed) from the device rows.
pub fn split(rows: Rows) -> (Rows, Rows) {
    rows.into_iter()
        .partition(|(_, payload)| !payload.starts_with(STATE_PREFIX))
}

/// `control` decision rows for the frozen stretches of `states` (empty when the device rows already carry some).
pub fn infer_control_rows(devices: &[(u64, String)], states: &[(u64, String)]) -> Rows {
    let decision = |value: &str| encode(&DeviceEvent::Decision("control".into(), value.into()));
    if devices
        .iter()
        .any(|(_, payload)| payload.starts_with("x:control:"))
    {
        return Vec::new();
    }
    let views: Vec<(u64, Vec<f32>)> = states
        .iter()
        .filter_map(|(poll, payload)| {
            view(payload.strip_prefix(STATE_PREFIX)?).map(|view| (*poll, view))
        })
        .collect();
    let mut rows = Vec::new();
    let mut locked = false;
    for pair in views.windows(2) {
        let frozen = same(&pair[0].1, &pair[1].1);
        if frozen != locked {
            locked = frozen;
            rows.push((pair[0].0, decision(if frozen { "1" } else { "0" })));
        }
    }
    if locked {
        rows.push((views.last().map_or(0, |last| last.0), decision("0")));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(poll: u64, x: f32, yaw: f32) -> (u64, String) {
        (
            poll,
            format!("s:v=1;lvl=2;ph=7;res=0;pos={x:.2},0.00,1.00;yaw={yaw:.2};pitch=3.00"),
        )
    }

    fn polls(rows: &[(u64, String)]) -> Vec<(u64, bool)> {
        rows.iter()
            .map(|(poll, payload)| (*poll, payload.ends_with(":1")))
            .collect()
    }

    #[test]
    fn a_stretch_of_unchanged_digests_is_a_lock_ending_at_its_last_unchanged_digest() {
        let states = [
            digest(10, 0.0, 90.0),
            digest(70, 0.0, 90.0),
            digest(130, 0.0, 90.0),
            digest(190, 0.0, 95.0),
            digest(250, 1.0, 95.0),
        ];
        let rows = infer_control_rows(&[], &states);
        assert_eq!(polls(&rows), vec![(10, true), (130, false)]);
    }

    #[test]
    fn moving_digests_make_no_lock_and_a_trailing_stretch_closes_at_the_last_digest() {
        let moving = [
            digest(1, 0.0, 1.0),
            digest(2, 1.0, 2.0),
            digest(3, 2.0, 3.0),
        ];
        assert!(infer_control_rows(&[], &moving).is_empty());
        let trailing = [
            digest(1, 0.0, 1.0),
            digest(2, 0.0, 1.0),
            digest(3, 0.0, 1.0),
        ];
        assert_eq!(
            polls(&infer_control_rows(&[], &trailing)),
            vec![(1, true), (3, false)]
        );
    }

    #[test]
    fn recorded_control_rows_win_and_digests_without_a_player_are_skipped() {
        let recorded = [(5, "x:control:1".to_owned())];
        let still = [digest(1, 0.0, 1.0), digest(2, 0.0, 1.0)];
        assert!(infer_control_rows(&recorded, &still).is_empty());
        let no_player = [(1, "s:v=1;lvl=67;ph=1;res=0".to_owned())];
        assert!(infer_control_rows(&[], &no_player).is_empty());
    }

    #[test]
    fn split_separates_digest_rows_from_device_rows() {
        let (devices, states) = split(vec![(1, "x:control:1".to_owned()), (2, "s:v=1".to_owned())]);
        assert_eq!((devices.len(), states.len()), (1, 1));
    }
}
