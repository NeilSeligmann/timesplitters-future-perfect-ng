// SPDX-License-Identifier: GPL-3.0-or-later
//! T1640 closed loop menu navigation as driven by the player: the step bookkeeping is Rust, the machine is C (`sys`).
use std::ffi::{CStr, c_char, c_void};

use tsfp_input_route_spec::nav_spec::{self, MAX_STEPS, Step, c_text, menu_find};

use crate::abi::{NavConfig, NavProbe, PadState, event};
use crate::describe::clip;
use crate::route::{Route, WaitRuntime};
use crate::sys;

pub enum NavResult {
    Idle,
    Pad,
    Jumped,
}

const MODE_OFF: u32 = 0;
const MODE_STRICT: u32 = 2;
const RUNNING: u32 = 0;
const FALLBACK: u32 = 2;
const FAILED: u32 = 3;

unsafe extern "C" fn probe_now(user: *mut c_void) -> u64 {
    // SAFETY: `user` is the route that created the probe and outlives the machine run.
    unsafe { &*user.cast::<Route>() }.now_ms()
}

unsafe extern "C" fn probe_read(
    address: u32,
    width: u32,
    value: *mut u32,
    user: *mut c_void,
) -> bool {
    // SAFETY: as above.
    let route = unsafe { &*user.cast::<Route>() };
    route
        .hooks
        .read_mem
        .is_some_and(|read| unsafe { read(address, width, value, route.hooks.mem_user) })
}

unsafe extern "C" fn probe_read_bytes(
    address: u32,
    buffer: *mut c_void,
    length: usize,
    user: *mut c_void,
) -> bool {
    // SAFETY: as above.
    let route = unsafe { &*user.cast::<Route>() };
    route
        .nav_read_bytes
        .get()
        .is_some_and(|read| unsafe { read(address, buffer, length, route.nav_bytes_user.get()) })
}

unsafe extern "C" fn probe_expect_arm(number: u32, user: *mut c_void) {
    // SAFETY: as above.
    let route = unsafe { &*user.cast::<Route>() };
    let steps = route.nav_steps.borrow();
    let mut runtime = route.nav_runtime.borrow_mut();
    let index = number as usize - 1;
    route.arm_runtime(&steps[index].expect_wait, &mut runtime[index]);
}

unsafe extern "C" fn probe_expect_holds(
    number: u32,
    polls: u64,
    started_ms: u64,
    note: *mut c_char,
    note_size: usize,
    user: *mut c_void,
) -> bool {
    // SAFETY: as above.
    let route = unsafe { &*user.cast::<Route>() };
    let steps = route.nav_steps.borrow();
    let runtime = route.nav_runtime.borrow();
    let index = number as usize - 1;
    let wait = &steps[index].expect_wait;
    let text = route.describe_wait(wait, &runtime[index], polls, note_size);
    if !note.is_null() && note_size > 0 {
        let kept = clip(text, note_size);
        // SAFETY: `note` is writable for `note_size` bytes.
        unsafe {
            std::ptr::copy_nonoverlapping(kept.as_ptr(), note.cast::<u8>(), kept.len());
            *note.add(kept.len()) = 0;
        }
    }
    route.wait_holds(wait, &runtime[index], polls, started_ms)
}

unsafe extern "C" fn probe_progress(line: *const c_char, user: *mut c_void) {
    // SAFETY: as above; `line` is NUL terminated.
    let route = unsafe { &*user.cast::<Route>() };
    let line = unsafe { CStr::from_ptr(line) }.to_string_lossy();
    route.emit_detail(
        event::NAV_PROGRESS,
        route.nav_next.get() as u32 + 1,
        route.nav_started_poll.get(),
        Some(&line),
    );
}

fn cstr(pointer: *const c_char) -> String {
    // SAFETY: the machine returns NUL terminated text living in its own buffer.
    unsafe { CStr::from_ptr(pointer) }
        .to_string_lossy()
        .into_owned()
}

impl Route {
    fn machine(&self) -> *mut c_void {
        // SAFETY: only this thread's source calls touch the machine buffer; no Rust reference to it is kept.
        unsafe { (*self.nav_machine.get()).as_mut_ptr().cast() }
    }

    /// At the position `at` of the next step the nav machine takes the pad.
    pub fn nav_poll_step(&self, poll_index: u64, out: &mut PadState) -> NavResult {
        let next = self.nav_next.get();
        let number = next as u32 + 1;
        let step: Step;
        if !self.nav_active.get() {
            {
                let steps = self.nav_steps.borrow();
                if next >= steps.len() || steps[next].at != self.cursor.get() {
                    return NavResult::Idle;
                }
                step = steps[next];
            }
            let probe = NavProbe {
                read_mem: Some(probe_read),
                read_bytes: Some(probe_read_bytes),
                now_ms: Some(probe_now),
                expect_arm: Some(probe_expect_arm),
                expect_holds: Some(probe_expect_holds),
                progress: Some(probe_progress),
                user: std::ptr::from_ref(self).cast_mut().cast(),
            };
            let menus = self.nav_menus.borrow();
            let menu = menus
                .as_ref()
                .and_then(|table| menu_find(table, c_text(&step.menu)))
                .map_or(std::ptr::null(), std::ptr::from_ref);
            let timing = self.nav_timing.get();
            // SAFETY: the machine buffer is `route_nav_sizeof` bytes; the step lives in `nav_steps` until the route
            // is freed (the vector is never resized after `set_nav`); menu and probe outlive the call.
            unsafe {
                sys::route_nav_begin(
                    self.machine(),
                    &self.nav_steps.borrow()[next],
                    number,
                    menu,
                    self.nav_mode.get(),
                    &timing,
                    &probe,
                );
            }
            drop(menus);
            self.nav_active.set(true);
            self.nav_started_poll.set(poll_index);
            let mut label = [0 as c_char; 160];
            // SAFETY: `label` is writable for its length.
            unsafe { sys::route_nav_step_label(&step, label.as_mut_ptr(), label.len()) };
            let detail = clip(
                format!(
                    "step {number} ({}) at record position {} replaces the recorded presses up to {}",
                    cstr(label.as_ptr()),
                    step.at,
                    step.to
                ),
                220,
            );
            self.emit_detail(event::NAV_START, number, poll_index, Some(&detail));
        } else {
            step = self.nav_steps.borrow()[next];
        }
        // SAFETY: the machine was begun above (or on an earlier poll); `out` is a writable pad state.
        let status = unsafe { sys::route_nav_poll(self.machine(), poll_index, out) };
        self.nav_polls_total.set(self.nav_polls_total.get() + 1);
        if status == RUNNING {
            return NavResult::Pad;
        }
        self.nav_active.set(false);
        self.nav_next.set(next + 1);
        // SAFETY: the machine holds its final message.
        let message = cstr(unsafe { sys::route_nav_message(self.machine()) });
        if status == FAILED {
            self.failed.set(true);
            self.emit_detail(event::NAV_FAIL, number, poll_index, Some(&message));
            return NavResult::Pad;
        }
        if status == FALLBACK {
            self.emit_detail(event::NAV_FALLBACK, number, poll_index, Some(&message));
            return NavResult::Idle;
        }
        let (mut presses, mut lost, mut polls, mut ready) = (0u32, 0u32, 0u64, 0u64);
        // SAFETY: all four outputs are writable.
        unsafe {
            sys::route_nav_summary(
                self.machine(),
                &mut presses,
                &mut lost,
                &mut polls,
                &mut ready,
            )
        };
        let detail = clip(
            format!(
                "step {number} done after {presses} press(es), {lost} lost, {polls} polls: the record continues at position {} (menu first ready after {ready} polls)",
                step.to
            ),
            240,
        );
        self.cursor.set(step.to);
        self.emit_detail(event::NAV_OK, number, poll_index, Some(&detail));
        NavResult::Jumped
    }

    /// `xinput_route_set_nav`.
    pub fn set_nav(&self, config: &NavConfig) -> Result<(), String> {
        if self.nav_steps.borrow().len() != 0
            || (config.step_count != 0 && config.steps.is_null())
            || config.step_count > MAX_STEPS
        {
            return Err("route nav: bad arguments".to_string());
        }
        self.nav_mode.set(config.mode);
        self.nav_timing.set(if config.timing.is_null() {
            nav_spec::timing_default()
        } else {
            // SAFETY: a non-null timing is valid.
            unsafe { *config.timing }
        });
        self.nav_read_bytes.set(config.read_bytes);
        self.nav_bytes_user.set(config.bytes_user);
        if config.mode == MODE_OFF || config.step_count == 0 {
            return Ok(());
        }
        if !config.menus.is_null() {
            // SAFETY: a non-null table is valid.
            *self.nav_menus.borrow_mut() = Some(Box::new(unsafe { *config.menus }));
        }
        // SAFETY: `steps` has `step_count` entries.
        let steps = unsafe { std::slice::from_raw_parts(config.steps, config.step_count) };
        let mut kept: Vec<Step> = Vec::with_capacity(steps.len());
        let mut runtimes = vec![WaitRuntime::NEW; steps.len()];
        for (i, step) in steps.iter().enumerate() {
            if step.at > step.to || step.to > self.total {
                return Err(format!(
                    "route nav step {}: at={} to={} is not inside the record ({} polls)",
                    i + 1,
                    step.at,
                    step.to,
                    self.total
                ));
            }
            let id = c_text(&step.menu);
            if config.mode == MODE_STRICT
                && self
                    .nav_menus
                    .borrow()
                    .as_ref()
                    .and_then(|table| menu_find(table, id))
                    .is_none()
            {
                return Err(format!(
                    "route nav step {} names menu '{}' which the menu table does not define (--route-nav strict)",
                    i + 1,
                    String::from_utf8_lossy(id)
                ));
            }
            if step.has_expect {
                self.register_conditions(&step.expect_wait, &mut runtimes[i], 0)?;
            }
            kept.push(*step);
        }
        *self.nav_runtime.borrow_mut() = runtimes;
        *self.nav_steps.borrow_mut() = kept;
        Ok(())
    }
}
