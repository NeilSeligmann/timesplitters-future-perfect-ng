// SPDX-License-Identifier: GPL-3.0-or-later
//! Wait evaluation and the progress/failure text of the route player (same wording as `xinput_route.c`).
use std::ffi::c_char;

use tsfp_input_route_spec::cond::{self, Cond, CondKind};
use tsfp_input_route_spec::ffi::xinput_route_mem_read;
use tsfp_input_route_spec::{COND_MAX, Wait};

use crate::abi::{Event, FrameState, WatchState};
use crate::route::{Route, WaitRuntime};
use crate::sys;

/// `snprintf(.., size, "%s", text)`: the text cut to `size - 1` bytes.
pub fn clip(mut text: String, size: usize) -> String {
    if size == 0 {
        return String::new();
    }
    if text.len() > size - 1 {
        let mut cut = size - 1;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
    }
    text
}

impl Route {
    pub fn now_ms(&self) -> u64 {
        if let Some(now) = self.hooks.probe.now_ms {
            // SAFETY: host callback with its own user pointer.
            return unsafe { now(self.hooks.probe.user) };
        }
        let mut ts = sys::Timespec { sec: 0, nsec: 0 };
        // SAFETY: CLOCK_MONOTONIC is 1; `ts` is writable.
        unsafe { sys::clock_gettime(1, &mut ts) };
        (ts.sec as u64)
            .wrapping_mul(1000)
            .wrapping_add(ts.nsec as u64 / 1_000_000)
    }

    pub fn emit_detail(&self, kind: u32, index: u32, poll: u64, detail: Option<&str>) {
        let Some(on_event) = self.hooks.on_event else {
            return;
        };
        let spent = if self.waiting.get() || kind == crate::abi::event::WAIT_OK {
            self.now_ms().wrapping_sub(self.stall_start_ms.get())
        } else {
            0
        };
        let text = detail.map(|text| {
            let mut bytes = text.as_bytes().to_vec();
            bytes.push(0);
            bytes
        });
        let event = Event {
            kind,
            index,
            cursor: self.cursor.get(),
            poll,
            stalled: self.stalled.get(),
            stalled_ms: spent,
            detail: text
                .as_ref()
                .map_or(std::ptr::null(), |bytes| bytes.as_ptr().cast::<c_char>()),
        };
        // SAFETY: host callback; `event` and `text` outlive the call.
        unsafe { on_event(&event, self.hooks.event_user) };
    }

    pub fn emit(&self, kind: u32, index: u32, poll: u64) {
        self.emit_detail(kind, index, poll, None);
    }

    /// Register the observers a wait's conditions need and check the others can be observed. `label` is the mark
    /// number (0 for a T1640 nav expect, worded accordingly).
    pub fn register_conditions(
        &self,
        wait: &Wait,
        runtime: &mut WaitRuntime,
        label: u32,
    ) -> Result<(), String> {
        let place = if label != 0 {
            format!("route wait mark{label}")
        } else {
            "route nav expect=".to_string()
        };
        let probe = &self.hooks.probe;
        for (c, cond) in wait.conds[..wait.cond_count as usize].iter().enumerate() {
            runtime.watch[c] = -1;
            let text = clip(cond::format(cond), 80);
            if cond.kind == CondKind::FRAME_CHANGE || cond.kind == CondKind::FRAME_STABLE {
                if probe.frame_state.is_none() {
                    return Err(format!(
                        "{place}: {text} cannot be observed (this host has no presented frame observer)"
                    ));
                }
            } else if cond.kind != CondKind::MEM {
                let id = probe.watch.map_or(-1, |watch| {
                    // SAFETY: host callback; `cond.text` is a NUL terminated array.
                    unsafe { watch(cond.kind, cond.text.as_ptr().cast(), cond.va, probe.user) }
                });
                if id < 0 {
                    let what = if cond.kind == CondKind::CALL {
                        "guest call"
                    } else {
                        "guest file I/O"
                    };
                    return Err(format!(
                        "{place}: {text} cannot be observed (this host has no {what} observer)"
                    ));
                }
                runtime.watch[c] = id;
            } else if self.hooks.read_mem.is_none() {
                return Err(format!(
                    "{place}: {text} cannot be observed (no guest memory reader)"
                ));
            }
        }
        Ok(())
    }

    /// One condition against the observations; the note says what was seen.
    fn cond_holds(&self, cond: &Cond, runtime: &WaitRuntime, c: usize) -> (bool, String) {
        let name = clip(cond::format(cond), 80);
        let mut holds = false;
        let seen;
        if cond.kind == CondKind::MEM {
            let mut value = 0u32;
            // SAFETY: `cond` is valid; the hook and user pointer come from the host.
            let read = self.hooks.read_mem.is_some()
                && unsafe {
                    xinput_route_mem_read(
                        cond,
                        self.hooks.read_mem,
                        self.hooks.mem_user,
                        &mut value,
                    )
                };
            if read {
                holds = cond::compare(cond.cmp, value & cond.mask, cond.value);
                seen = format!("read 0x{:X}", value & cond.mask);
            } else {
                seen = "unreadable".to_string();
            }
        } else if cond.kind == CondKind::FRAME_CHANGE || cond.kind == CondKind::FRAME_STABLE {
            let mut frame = FrameState::default();
            let ok = self.hooks.probe.frame_state.is_some_and(|state| {
                // SAFETY: host callback.
                unsafe { state(&mut frame, self.hooks.probe.user) }
            });
            if ok {
                let changes = frame.changes.wrapping_sub(runtime.base_frame.changes);
                let quiet = if changes != 0 {
                    self.now_ms().wrapping_sub(frame.last_change_ms)
                } else {
                    0
                };
                holds =
                    changes != 0 && (cond.kind == CondKind::FRAME_CHANGE || quiet >= cond.amount);
                seen = format!("{changes} change(s), quiet {quiet} ms");
            } else {
                seen = "no frame observer".to_string();
            }
        } else {
            let mut state = WatchState::default();
            let ok = runtime.watch[c] >= 0
                && self.hooks.probe.watch_state.is_some_and(|watch_state| {
                    // SAFETY: host callback.
                    unsafe { watch_state(runtime.watch[c], &mut state, self.hooks.probe.user) }
                });
            if ok {
                let count = state.count.wrapping_sub(runtime.base[c].count);
                let bytes = state.bytes.wrapping_sub(runtime.base[c].bytes);
                let quiet = if state.last_ms != 0 {
                    self.now_ms().wrapping_sub(state.last_ms)
                } else {
                    0
                };
                seen = match cond.kind {
                    CondKind::FILE_OPEN => {
                        holds = count >= 1;
                        format!("{count} open(s)")
                    }
                    CondKind::FILE_READ => {
                        holds = bytes >= cond.amount;
                        format!("{bytes} byte(s) in {count} read(s)")
                    }
                    CondKind::FILE_IDLE => {
                        holds = count != 0 && quiet >= cond.amount;
                        if count == 0 {
                            "no file I/O yet".to_string()
                        } else {
                            format!("{count} event(s), last {quiet} ms ago")
                        }
                    }
                    CondKind::CALL => {
                        holds = count >= cond.amount;
                        format!("{count} call(s)")
                    }
                    _ => String::new(),
                };
            } else {
                seen = "no observer".to_string();
            }
        }
        let seen = clip(seen, 96);
        (
            holds,
            format!("{} {name} ({seen})", if holds { "OK  " } else { "WAIT" }),
        )
    }

    /// `stalled` polls and `start_ms` since the wait's position was reached.
    pub fn wait_holds(
        &self,
        wait: &Wait,
        runtime: &WaitRuntime,
        stalled: u64,
        start_ms: u64,
    ) -> bool {
        if stalled < wait.min_polls {
            return false;
        }
        if wait.min_ms != 0 && self.now_ms().wrapping_sub(start_ms) < wait.min_ms {
            return false;
        }
        if wait.has_mem {
            let mut value = 0u32;
            let read = self.hooks.read_mem.is_some_and(|read| {
                // SAFETY: host callback.
                unsafe { read(wait.address, wait.width, &mut value, self.hooks.mem_user) }
            });
            if !read || (value & wait.mask) != wait.value {
                return false;
            }
        }
        if wait.cond_count == 0 {
            return true;
        }
        for (c, cond) in wait.conds[..wait.cond_count as usize].iter().enumerate() {
            let (holds, _) = self.cond_holds(cond, runtime, c);
            if wait.any && holds {
                return true;
            }
            if !wait.any && !holds {
                return false;
            }
        }
        !wait.any
    }

    /// One line per condition, `; ` separated, plus the poll and ms minimums. Empty for a legacy wait.
    pub fn describe_wait(
        &self,
        wait: &Wait,
        runtime: &WaitRuntime,
        stalled: u64,
        out_size: usize,
    ) -> String {
        let mut out = String::new();
        if !wait.event {
            return out;
        }
        let append = |out: &mut String, piece: String| {
            out.push_str(&piece);
            if out.len() >= out_size {
                *out = clip(std::mem::take(out), out_size);
                return false;
            }
            true
        };
        let mut c = 0usize;
        while c < (wait.cond_count as usize).min(COND_MAX) && out.len() + 2 < out_size {
            let (_, note) = self.cond_holds(&wait.conds[c], runtime, c);
            let note = clip(note, 200);
            let prefix = if c != 0 { "; " } else { "" };
            if !append(&mut out, format!("{prefix}{note}")) {
                break;
            }
            c += 1;
        }
        if wait.any && out.len() + 12 < out_size {
            let _ = append(&mut out, " [any of]".to_string());
        }
        if (wait.min_polls != 0 || wait.min_ms != 0) && out.len() + 48 < out_size {
            let prefix = if out.is_empty() { "" } else { "; " };
            let piece = format!(
                "{prefix}min {} polls / {} ms (stalled {stalled} polls)",
                wait.min_polls, wait.min_ms
            );
            let _ = append(&mut out, piece);
        }
        out
    }

    /// The segment of `wait` starts now: take the counters its conditions are measured from.
    pub fn arm_runtime(&self, wait: &Wait, runtime: &mut WaitRuntime) {
        for c in 0..wait.cond_count as usize {
            runtime.base[c] = WatchState::default();
            if runtime.watch[c] >= 0
                && let Some(watch_state) = self.hooks.probe.watch_state
            {
                // SAFETY: host callback.
                unsafe {
                    watch_state(
                        runtime.watch[c],
                        &mut runtime.base[c],
                        self.hooks.probe.user,
                    )
                };
            }
        }
        runtime.base_frame = FrameState::default();
        if let Some(frame_state) = self.hooks.probe.frame_state {
            // SAFETY: host callback.
            unsafe { frame_state(&mut runtime.base_frame, self.hooks.probe.user) };
        }
    }
}
