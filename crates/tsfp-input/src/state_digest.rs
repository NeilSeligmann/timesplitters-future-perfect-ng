// SPDX-License-Identifier: GPL-3.0-or-later
//! T3819: the periodic game-state digest of input.rec v6 `S` rows (docs/t-replay-desync-l3-l4.md).
//!
//! A recording writes one digest per bucket of [`INTERVAL_POLLS`] host polls, plus one on every change
//! of `lvl`, `ph` or `res` (boot transitions are recorded exactly); a replay logs the same digest
//! (`replay-state poll=<n> tick=<sim tick> <payload>`) so `tools.replay_desync` can find the first
//! tick where the replay's game state leaves the owner's. Fields (key=value, `;` separated, after `v=1;`):
//! `lvl` mode/level dword `[0x79094C]`, `ph` match phase `[0x7DE30C]`, `res` session result
//! `[0x7DE45C]`, and, while an active player exists (`[0x7B0C7C]`), `pos` (+0xA8..+0xB0),
//! `yaw` (+0x100) and `pitch` (+0x104) in game units with two decimals.

use std::sync::atomic::{AtomicU64, Ordering};

pub const INTERVAL_POLLS: u64 = 60;
pub const VERSION: u32 = 1;
const MODE_LEVEL: u32 = 0x0079_094c;
const MATCH_PHASE: u32 = 0x007d_e30c;
const SESSION_RESULT: u32 = 0x007d_e45c;
const ACTIVE_PLAYER: u32 = 0x007b_0c7c;

/// One digest per poll bucket (the first poll seen in a new bucket is due) and one per change of the
/// level/phase/result words.
#[derive(Debug, Default)]
pub struct Clock {
    last_bucket: AtomicU64,
    last_state: AtomicU64,
}

impl Clock {
    pub const fn new() -> Self {
        Self {
            last_bucket: AtomicU64::new(u64::MAX),
            last_state: AtomicU64::new(u64::MAX),
        }
    }

    /// A changed level/phase/result word (see [`boot_state`]) makes a digest due inside a bucket.
    pub fn due(&self, host_poll: u64, read32: impl Fn(u32) -> Option<u32>) -> bool {
        let state = boot_state(read32);
        let bucket = host_poll / INTERVAL_POLLS;
        let new_bucket = self.last_bucket.swap(bucket, Ordering::Relaxed) != bucket;
        let new_state = self.last_state.swap(state, Ordering::Relaxed) != state;
        new_bucket || new_state
    }
}

/// The level, phase and result words folded into one value for [`Clock::due`] (FNV-1a over the
/// words; an unreadable word folds as a value no word has).
fn boot_state(read32: impl Fn(u32) -> Option<u32>) -> u64 {
    [MODE_LEVEL, MATCH_PHASE, SESSION_RESULT]
        .into_iter()
        .map(|va| read32(va).map_or(u64::MAX, u64::from))
        .fold(0xcbf2_9ce4_8422_2325, |hash, word| {
            (hash ^ word).wrapping_mul(0x0000_0100_0000_01b3)
        })
}

/// The digest payload; `read32` returns None for unreadable guest memory.
pub fn payload(read32: impl Fn(u32) -> Option<u32>) -> String {
    let word = |va| read32(va).map_or_else(|| "-".to_owned(), |value| format!("{value:x}"));
    let mut text = format!(
        "v={VERSION};lvl={};ph={};res={}",
        word(MODE_LEVEL),
        word(MATCH_PHASE),
        word(SESSION_RESULT)
    );
    let player = read32(ACTIVE_PLAYER).filter(|&pointer| pointer != 0);
    let float = |offset: u32| {
        player
            .and_then(|base| read32(base.wrapping_add(offset)))
            .map(f32::from_bits)
            .filter(|value| value.is_finite())
    };
    if let (Some(x), Some(y), Some(z)) = (float(0xa8), float(0xac), float(0xb0)) {
        text += &format!(";pos={x:.2},{y:.2},{z:.2}");
        if let (Some(yaw), Some(pitch)) = (float(0x100), float(0x104)) {
            text += &format!(";yaw={yaw:.2};pitch={pitch:.2}");
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn memory(entries: &[(u32, u32)]) -> impl Fn(u32) -> Option<u32> + use<> {
        let map: HashMap<u32, u32> = entries.iter().copied().collect();
        move |va| map.get(&va).copied()
    }

    #[test]
    fn digest_without_a_player_holds_the_globals_only() {
        let read = memory(&[(MODE_LEVEL, 0xb), (MATCH_PHASE, 7), (SESSION_RESULT, 0)]);
        assert_eq!(payload(read), "v=1;lvl=b;ph=7;res=0");
    }

    #[test]
    fn digest_with_a_player_adds_position_and_view() {
        let base = 0x0100_0000;
        let read = memory(&[
            (MODE_LEVEL, 0xb),
            (MATCH_PHASE, 7),
            (ACTIVE_PLAYER, base),
            (base + 0xa8, 18.1f32.to_bits()),
            (base + 0xac, 1.5f32.to_bits()),
            (base + 0xb0, (-2.525f32).to_bits()),
            (base + 0x100, 288.05f32.to_bits()),
            (base + 0x104, 3.0f32.to_bits()),
        ]);
        assert_eq!(
            payload(read),
            "v=1;lvl=b;ph=7;res=-;pos=18.10,1.50,-2.53;yaw=288.05;pitch=3.00"
        );
    }

    #[test]
    fn clock_fires_once_per_bucket() {
        let clock = Clock::new();
        assert!(clock.due(0, memory(&[])));
        assert!(!clock.due(59, memory(&[])));
        assert!(clock.due(60, memory(&[])));
        assert!(!clock.due(61, memory(&[])));
        assert!(clock.due(180, memory(&[])));
    }

    #[test]
    fn clock_fires_when_level_or_phase_changes_inside_a_bucket() {
        let clock = Clock::new();
        let at = |level, phase| memory(&[(MODE_LEVEL, level), (MATCH_PHASE, phase)]);
        assert!(clock.due(10, at(0x67, 1)));
        assert!(!clock.due(11, at(0x67, 1)));
        assert!(clock.due(12, at(0x65, 1)));
        assert!(!clock.due(13, at(0x65, 1)));
        assert!(clock.due(14, at(0x65, 2)));
    }
}
