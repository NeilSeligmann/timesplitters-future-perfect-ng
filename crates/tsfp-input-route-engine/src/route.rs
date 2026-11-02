// SPDX-License-Identifier: GPL-3.0-or-later
//! The route player state and its per-poll source. All mutable state is in `Cell`/`RefCell` so the nav probe callbacks
//! (which receive the route as `user` while the C machine runs) only ever see shared references.
use std::cell::{Cell, RefCell, UnsafeCell};
use std::ffi::c_void;

use tsfp_input_route_spec::nav_spec::{MenuTable, Step, Timing};
use tsfp_input_route_spec::{COND_MAX, MAX_MARKS, MAX_WAITS, Wait};

use crate::abi::{FrameState, Hooks, PadState, ReadBytes, WatchState, event};
use crate::sys;

pub const PROGRESS_MS: u64 = 5000;

/// Per wait runtime: probe watch ids and the counters taken when the segment (the previous mark) began.
#[derive(Clone, Copy)]
pub struct WaitRuntime {
    pub watch: [i32; COND_MAX],
    pub base: [WatchState; COND_MAX],
    pub base_frame: FrameState,
}

impl WaitRuntime {
    pub const NEW: WaitRuntime = WaitRuntime {
        watch: [-1; COND_MAX],
        base: [WatchState {
            count: 0,
            bytes: 0,
            last_ms: 0,
        }; COND_MAX],
        base_frame: FrameState {
            changes: 0,
            last_change_ms: 0,
        },
    };
}

pub struct Route {
    pub script: *const c_void,
    pub total: u64,
    pub marks: Vec<u64>,
    pub mark_fired: RefCell<Vec<bool>>,
    pub waits: Vec<Wait>,
    pub runtime: RefCell<Vec<WaitRuntime>>,
    pub hooks: Hooks,
    pub cursor: Cell<u64>,
    pub stalled: Cell<u64>,
    pub stalled_total: Cell<u64>,
    pub stall_start_ms: Cell<u64>,
    pub last_progress_ms: Cell<u64>,
    pub stalled_ms_total: Cell<u64>,
    pub segment_started: Cell<bool>,
    pub waiting: Cell<bool>,
    pub ended: Cell<bool>,
    pub failed: Cell<bool>,
    // T1640 closed loop menu navigation.
    pub nav_steps: RefCell<Vec<Step>>,
    pub nav_runtime: RefCell<Vec<WaitRuntime>>,
    pub nav_next: Cell<usize>,
    pub nav_menus: RefCell<Option<Box<MenuTable>>>,
    pub nav_mode: Cell<u32>,
    pub nav_timing: Cell<Timing>,
    pub nav_machine: UnsafeCell<Vec<u64>>,
    pub nav_active: Cell<bool>,
    pub nav_started_poll: Cell<u64>,
    pub nav_polls_total: Cell<u64>,
    pub nav_read_bytes: Cell<ReadBytes>,
    pub nav_bytes_user: Cell<*mut c_void>,
}

impl Route {
    pub fn new(
        script: *const c_void,
        marks: &[u64],
        waits: &[Wait],
        hooks: Hooks,
    ) -> Result<Route, String> {
        // SAFETY: the script handle is valid for the route's lifetime (documented in xinput_route.h).
        let total = unsafe { sys::xinput_script_total_frames(script) };
        for (i, &mark) in marks.iter().enumerate() {
            if mark > total || (i != 0 && mark < marks[i - 1]) {
                return Err(format!(
                    "route: mark {} at {mark} is out of order or past the record ({total} polls)",
                    i + 1
                ));
            }
        }
        for (i, wait) in waits.iter().enumerate() {
            if wait.mark == 0 || wait.mark as usize > marks.len() {
                return Err(format!(
                    "route wait names mark{} but the record has {} mark(s)",
                    wait.mark,
                    marks.len()
                ));
            }
            if waits[..i].iter().any(|other| other.mark == wait.mark) {
                return Err(format!("route: two waits for mark{}", wait.mark));
            }
        }
        // SAFETY: plain size query.
        let machine_words = unsafe { sys::route_nav_sizeof() }.div_ceil(8);
        let route = Route {
            script,
            total,
            marks: marks.to_vec(),
            mark_fired: RefCell::new(vec![false; marks.len()]),
            waits: waits.to_vec(),
            runtime: RefCell::new(vec![WaitRuntime::NEW; waits.len()]),
            hooks,
            cursor: Cell::new(0),
            stalled: Cell::new(0),
            stalled_total: Cell::new(0),
            stall_start_ms: Cell::new(0),
            last_progress_ms: Cell::new(0),
            stalled_ms_total: Cell::new(0),
            segment_started: Cell::new(false),
            waiting: Cell::new(false),
            ended: Cell::new(false),
            failed: Cell::new(false),
            nav_steps: RefCell::new(Vec::new()),
            nav_runtime: RefCell::new(Vec::new()),
            nav_next: Cell::new(0),
            nav_menus: RefCell::new(None),
            nav_mode: Cell::new(0),
            nav_timing: Cell::new(Timing {
                hold: 0,
                gap: 0,
                extra: 0,
            }),
            nav_machine: UnsafeCell::new(vec![0u64; machine_words]),
            nav_active: Cell::new(false),
            nav_started_poll: Cell::new(0),
            nav_polls_total: Cell::new(0),
            nav_read_bytes: Cell::new(None),
            nav_bytes_user: Cell::new(std::ptr::null_mut()),
        };
        for (i, wait) in route.waits.iter().enumerate() {
            let mut runtime = route.runtime.borrow_mut();
            route.register_conditions(wait, &mut runtime[i], wait.mark)?;
        }
        Ok(route)
    }

    pub fn limits_ok(marks: usize, waits: usize) -> bool {
        marks as u64 <= MAX_MARKS && waits <= MAX_WAITS
    }

    /// The pad of one host poll (`xinput_route_source`).
    pub fn poll(&self, poll_index: u64, out: &mut PadState) {
        *out = PadState::default();
        if let Some(on_poll) = self.hooks.on_poll {
            // SAFETY: the host supplied the callback and its user pointer.
            unsafe { on_poll(poll_index, self.cursor.get(), self.hooks.poll_user) };
        }
        let mut live_state = PadState::default();
        let mut live_supplied = false;
        if let Some(live) = self.hooks.live {
            // SAFETY: as above; `live_state` is writable.
            live_supplied = unsafe { live(poll_index, &mut live_state, self.hooks.live_user) };
        }
        if self.failed.get() {
            return;
        }
        if !self.segment_started.get() {
            self.segment_started.set(true);
            self.arm_for_mark(1);
        }
        loop {
            if !self.fire_marks(poll_index) {
                return;
            }
            match self.nav_poll_step(poll_index, out) {
                crate::nav::NavResult::Pad => return,
                crate::nav::NavResult::Idle => break,
                crate::nav::NavResult::Jumped => {}
            }
        }
        if self.cursor.get() >= self.total {
            if !self.ended.get() {
                self.ended.set(true);
                self.emit(event::END, 0, poll_index);
            }
            if live_supplied {
                *out = live_state;
            }
            return;
        }
        let at = self.cursor.get();
        self.cursor.set(at + 1);
        // SAFETY: the script handle is valid.
        *out = unsafe { sys::xinput_script_state_at(self.script, at) };
    }

    /// Fire the marks at the cursor (after their waits). False when the route stalls at one or has failed.
    fn fire_marks(&self, poll_index: u64) -> bool {
        for i in 0..self.marks.len() {
            if self.mark_fired.borrow()[i] || self.marks[i] != self.cursor.get() {
                continue;
            }
            let number = i as u32 + 1;
            if let Some(wait_index) = self.waits.iter().position(|wait| wait.mark == number) {
                let wait = &self.waits[wait_index];
                if !self.waiting.get() {
                    self.waiting.set(true);
                    let now = self.now_ms();
                    self.stall_start_ms.set(now);
                    self.last_progress_ms.set(now);
                }
                let stalled = self.stalled.get();
                let start = self.stall_start_ms.get();
                if !self.wait_holds(wait, &self.runtime.borrow()[wait_index], stalled, start) {
                    let spent = self.now_ms().wrapping_sub(start);
                    if (wait.max_polls != 0 && stalled >= wait.max_polls)
                        || (wait.timeout_ms != 0 && spent >= wait.timeout_ms)
                    {
                        let detail = self.describe_wait(
                            wait,
                            &self.runtime.borrow()[wait_index],
                            stalled,
                            640,
                        );
                        self.failed.set(true);
                        self.emit_detail(event::WAIT_TIMEOUT, number, poll_index, Some(&detail));
                        return false;
                    }
                    if wait.event
                        && self.now_ms().wrapping_sub(self.last_progress_ms.get()) >= PROGRESS_MS
                    {
                        self.last_progress_ms.set(self.now_ms());
                        let detail = self.describe_wait(
                            wait,
                            &self.runtime.borrow()[wait_index],
                            stalled,
                            640,
                        );
                        self.emit_detail(event::WAIT_PROGRESS, number, poll_index, Some(&detail));
                    }
                    self.stalled.set(stalled + 1);
                    self.stalled_total.set(self.stalled_total.get() + 1);
                    return false; // the pad stays at rest, the record does not advance
                }
                let detail =
                    self.describe_wait(wait, &self.runtime.borrow()[wait_index], stalled, 640);
                self.stalled_ms_total.set(
                    self.stalled_ms_total
                        .get()
                        .wrapping_add(self.now_ms().wrapping_sub(start)),
                );
                self.emit_detail(
                    event::WAIT_OK,
                    number,
                    poll_index,
                    wait.event.then_some(detail.as_str()),
                );
                self.waiting.set(false);
                self.stalled.set(0);
            }
            self.mark_fired.borrow_mut()[i] = true;
            self.arm_for_mark(number + 1); // the next segment begins at this mark
            self.emit(event::MARK, number, poll_index);
        }
        true
    }

    fn arm_for_mark(&self, mark: u32) {
        for (i, wait) in self.waits.iter().enumerate() {
            if wait.mark == mark {
                self.arm_runtime(wait, &mut self.runtime.borrow_mut()[i]);
            }
        }
    }
}
