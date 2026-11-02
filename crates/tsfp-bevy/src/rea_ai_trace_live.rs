// SPDX-License-Identifier: GPL-3.0-or-later
//! T2407 diagnostic stream for reference guest state versus the Rust actor
//! store. This observer is opt-in through tools.rust_trace and never writes
//! guest memory. The measured list at 0x7a3580 stores extension handles.

use crate::ui_live::HostUiGuestApi;
use std::io::{BufWriter, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

const ACTOR_LIST: u32 = 0x007a_3580;
const ACTOR_COUNT: u32 = 0x0074_c37c;
const GAME_MODE: u32 = 0x007d_e455;
const MAX_ACTORS: u32 = 256;
const STATE_WORDS: usize = 24;

fn actor_entry_address(index: u32) -> u32 {
    ACTOR_LIST.wrapping_add(index.wrapping_mul(4))
}

static TRACE: OnceLock<Mutex<Option<BufWriter<std::fs::File>>>> = OnceLock::new();
static LAST_POLL: AtomicU64 = AtomicU64::new(u64::MAX);
static LAST_ROUTE_CURSOR: AtomicU64 = AtomicU64::new(u64::MAX);
static SAMPLE_INDEX: AtomicU64 = AtomicU64::new(0);

unsafe extern "C" {
    fn xinput_source_port_poll_count(port: u32) -> u64;
    fn tsfp_rust_rea_ai_trace_snapshot(extension: u32, output: *mut u32) -> i32;
}

fn trace() -> &'static Mutex<Option<BufWriter<std::fs::File>>> {
    TRACE.get_or_init(|| {
        let file = std::env::var_os("TSFP_REA_AI_STATE_TRACE").and_then(|path| {
            std::fs::File::create(path)
                .map(BufWriter::new)
                .map_err(|error| {
                    eprintln!("tsfp-bevy: REA AI state trace open failed: {error}");
                })
                .ok()
        });
        Mutex::new(file)
    })
}

fn reference_words(api: &HostUiGuestApi, extension: u32) -> Option<[u32; STATE_WORDS]> {
    let read = |offset| api.read32_raw(extension.wrapping_add(offset));
    let pending_local = read(0x9d0)? as i32;
    Some([
        (api.read32_raw(GAME_MODE)? & 0xff == 10) as u32,
        read(0x10)?,
        read(0x14)?,
        read(0x18)?,
        read(0x1c)?,
        read(0xab0)?,
        read(0xab4)?,
        read(0xaa8)?,
        read(0xaac)?,
        read(0x20)?,
        read(0x9d4)?,
        if pending_local < 0 {
            u32::MAX
        } else {
            pending_local as u32
        },
        read(0x9d8)?,
        read(0xadc)?,
        read(0x678)?,
        read(0x684)?,
        read(0x9dc)?,
        read(0x9a8)?,
        read(0x9ac)?,
        read(0x680)?,
        read(0x30)?,
        read(0x34)?,
        read(0x38)?,
        0b111,
    ])
}

/// Samples each live actor once per recorded gameplay T row, using that row's
/// cursor so the two hosts stay aligned despite different controller-poll rates.
/// The reference run reads guest state; an owned run requires a Rust store row.
pub(crate) fn service(api: &HostUiGuestApi) {
    if std::env::var_os("TSFP_REA_AI_STATE_TRACE").is_none() {
        return;
    }
    // SAFETY: this read-only host counter is available for the process lifetime.
    let poll = unsafe { xinput_source_port_poll_count(0) };
    if LAST_POLL.swap(poll, Ordering::Relaxed) == poll {
        return;
    }
    let Some((route_cursor, route_tick)) = crate::input_recording::replay_cursor() else {
        return;
    };
    let Ok(mut trace) = trace().lock() else {
        return;
    };
    let Some(output) = trace.as_mut() else {
        return;
    };
    let Some(count) = api.read32_raw(ACTOR_COUNT) else {
        let _ = output.flush();
        return;
    };
    if count == 0 {
        let _ = output.flush();
        return;
    }
    if count > MAX_ACTORS {
        if LAST_ROUTE_CURSOR.swap(route_cursor, Ordering::Relaxed) == route_cursor {
            return;
        }
        let sample = SAMPLE_INDEX.fetch_add(1, Ordering::Relaxed);
        let _ = write!(
            output,
            "{sample} {poll} {route_cursor} {route_tick} 0x0 -1 {} {}",
            u32::MAX,
            u32::MAX
        );
        for _ in 0..STATE_WORDS {
            let _ = write!(output, " 0");
        }
        let _ = writeln!(output);
        let _ = output.flush();
        return;
    }
    if LAST_ROUTE_CURSOR.swap(route_cursor, Ordering::Relaxed) == route_cursor {
        return;
    }
    let owned_selection =
        std::env::var("TSFP_REA_AI_STATE_SELECTION").is_ok_and(|value| value == "owned");
    for index in 0..count {
        let Some(extension) = api
            .read32_raw(actor_entry_address(index))
            .filter(|v| *v != 0)
        else {
            continue;
        };
        let mut owner = [0u32; STATE_WORDS + 2];
        // SAFETY: the owner record has two identity words and STATE_WORDS data words.
        let owner_status =
            unsafe { tsfp_rust_rea_ai_trace_snapshot(extension, owner.as_mut_ptr()) };
        let (source, slot, generation, words) = if owned_selection {
            if owner_status == 1 {
                (1, owner[0], owner[1], owner[2..].try_into().unwrap())
            } else {
                (-1, u32::MAX, u32::MAX, [0; STATE_WORDS])
            }
        } else if owner_status == 0 {
            match reference_words(api, extension) {
                Some(words) => (0, u32::MAX, 0, words),
                None => (-1, u32::MAX, u32::MAX, [0; STATE_WORDS]),
            }
        } else {
            (-1, u32::MAX, u32::MAX, [0; STATE_WORDS])
        };
        let sample = SAMPLE_INDEX.fetch_add(1, Ordering::Relaxed);
        if write!(
            output,
            "{sample} {poll} {route_cursor} {route_tick} {extension:#x} {source} {slot} {generation}"
        )
        .is_err()
        {
            continue;
        }
        for word in words {
            if write!(output, " {word}").is_err() {
                break;
            }
        }
        let _ = writeln!(output);
    }
    // Preserve rows if the guest later reaches a named stop or callback limit.
    let _ = output.flush();
}

#[cfg(test)]
mod tests {
    use super::{ACTOR_LIST, actor_entry_address};

    #[test]
    fn actor_entries_are_inline_in_the_measured_extension_array() {
        assert_eq!(actor_entry_address(0), ACTOR_LIST);
        assert_eq!(actor_entry_address(1), ACTOR_LIST + 4);
        assert_eq!(actor_entry_address(255), ACTOR_LIST + 255 * 4);
    }
}
